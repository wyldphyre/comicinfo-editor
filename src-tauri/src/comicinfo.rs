use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::de::{self, IntoDeserializer, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum YesNo {
    Unknown,
    No,
    Yes,
}

impl Default for YesNo {
    fn default() -> Self {
        YesNo::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Manga {
    Unknown,
    No,
    Yes,
    YesAndRightToLeft,
}

impl Default for Manga {
    fn default() -> Self {
        Manga::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgeRating {
    Unknown,
    #[serde(rename = "Adults Only 18+")]
    AdultsOnly18,
    #[serde(rename = "Early Childhood")]
    EarlyChildhood,
    Everyone,
    #[serde(rename = "Everyone 10+")]
    Everyone10,
    G,
    #[serde(rename = "Kids to Adults")]
    KidsToAdults,
    #[serde(rename = "M")]
    M,
    #[serde(rename = "MA15+")]
    MA15,
    #[serde(rename = "Mature 17+")]
    Mature17,
    PG,
    #[serde(rename = "R18+")]
    R18,
    #[serde(rename = "Rating Pending")]
    RatingPending,
    Teen,
    #[serde(rename = "X18+")]
    X18,
}

impl Default for AgeRating {
    fn default() -> Self {
        AgeRating::Unknown
    }
}

/// An enum-valued field that tolerates text the spec does not list.
///
/// Real files carry `PG-13` for AgeRating, `true` for BlackAndWhite, and other
/// values no ComicInfo enum defines. Rejecting them failed the entire parse, so
/// one stray word made the archive impossible to open at all. Mapping them to
/// `None` instead would silently throw the value away, so `Other` keeps the
/// original text and writes it back out unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum Lenient<T> {
    Known(T),
    Other(String),
}

impl<T: Serialize> Serialize for Lenient<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Lenient::Known(value) => value.serialize(serializer),
            Lenient::Other(raw) => serializer.serialize_str(raw),
        }
    }
}

/// Read an optional enum field. An absent or empty element is `None`; anything
/// the enum does not define is kept verbatim as `Lenient::Other`.
fn lenient_opt_enum<'de, D, T>(deserializer: D) -> Result<Option<Lenient<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let raw: Option<String> = Option::deserialize(deserializer)?;
    let Some(raw) = raw else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let as_str: de::value::StrDeserializer<de::value::Error> = trimmed.into_deserializer();
    Ok(Some(match T::deserialize(as_str) {
        Ok(value) => Lenient::Known(value),
        Err(_) => Lenient::Other(raw),
    }))
}

/// Read an optional number. Empty elements (`<Count/>`, `<Count></Count>`) are
/// common in files written by other taggers and mean "absent", not "malformed";
/// treating them as an error made the whole file unopenable. A value that is
/// not a number at all is treated the same way, for the same reason.
///
/// Accepts both a string (how quick-xml hands us element text and attributes)
/// and a real number (how the frontend's JSON payload sends it).
fn lenient_opt_num<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
{
    deserializer.deserialize_option(OptScalar::<T>(PhantomData))
}

/// Read an optional boolean, accepting the spellings other writers use
/// (`True`, `TRUE`, `1`, `yes`) alongside the canonical `true`/`false`.
fn lenient_opt_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(OptBool)
}

/// Turns whatever scalar form the value arrived in into a string, so a single
/// `FromStr` parse covers quick-xml's text and serde_json's numbers alike.
fn scalar_text<T: FromStr>(text: &str) -> Option<T> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse::<T>().ok()
    }
}

struct OptScalar<T>(PhantomData<T>);

impl<'de, T: FromStr> Visitor<'de> for OptScalar<T> {
    type Value = Option<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a number, or an empty value")
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(scalar_text(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(scalar_text(&value.to_string()))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(scalar_text(&value.to_string()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(scalar_text(&value.to_string()))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        Ok(element_text(map)?.as_deref().and_then(scalar_text))
    }
}

/// The key quick-xml reports an element's own text content under.
const XML_TEXT_KEY: &str = "$text";

/// Pull an element's text out of the map form quick-xml hands to
/// `deserialize_any`, ignoring any attributes or children. An element carrying
/// nothing at all (`<Count/>`) is an empty map, i.e. an absent value.
fn element_text<'de, A: de::MapAccess<'de>>(mut map: A) -> Result<Option<String>, A::Error> {
    let mut text: Option<String> = None;
    while let Some(key) = map.next_key::<String>()? {
        if key == XML_TEXT_KEY {
            text = Some(map.next_value::<String>()?);
        } else {
            map.next_value::<de::IgnoredAny>()?;
        }
    }
    Ok(text)
}

struct OptBool;

impl<'de> Visitor<'de> for OptBool {
    type Value = Option<bool>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a boolean, or an empty value")
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Some(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(match value.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        })
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Some(value != 0))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Some(value != 0))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        match element_text(map)? {
            Some(text) => self.visit_str(&text),
            None => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename = "ComicInfo")]
pub struct ComicInfo {
    #[serde(rename = "Title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(rename = "Series", skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,

    #[serde(rename = "Number", skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,

    #[serde(
        rename = "Count",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub count: Option<i32>,

    #[serde(
        rename = "Volume",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub volume: Option<i32>,

    #[serde(rename = "AlternateSeries", skip_serializing_if = "Option::is_none")]
    pub alternate_series: Option<String>,

    #[serde(rename = "AlternateNumber", skip_serializing_if = "Option::is_none")]
    pub alternate_number: Option<String>,

    #[serde(
        rename = "AlternateCount",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub alternate_count: Option<i32>,

    #[serde(rename = "Summary", skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(
        rename = "Year",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub year: Option<i32>,

    #[serde(
        rename = "Month",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub month: Option<i32>,

    #[serde(
        rename = "Day",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub day: Option<i32>,

    #[serde(rename = "Writer", skip_serializing_if = "Option::is_none")]
    pub writer: Option<String>,

    #[serde(rename = "Penciller", skip_serializing_if = "Option::is_none")]
    pub penciller: Option<String>,

    #[serde(rename = "Inker", skip_serializing_if = "Option::is_none")]
    pub inker: Option<String>,

    #[serde(rename = "Colorist", skip_serializing_if = "Option::is_none")]
    pub colorist: Option<String>,

    #[serde(rename = "Letterer", skip_serializing_if = "Option::is_none")]
    pub letterer: Option<String>,

    #[serde(rename = "CoverArtist", skip_serializing_if = "Option::is_none")]
    pub cover_artist: Option<String>,

    #[serde(rename = "Editor", skip_serializing_if = "Option::is_none")]
    pub editor: Option<String>,

    #[serde(rename = "Translator", skip_serializing_if = "Option::is_none")]
    pub translator: Option<String>,

    #[serde(rename = "Publisher", skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,

    #[serde(rename = "Imprint", skip_serializing_if = "Option::is_none")]
    pub imprint: Option<String>,

    #[serde(rename = "Genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,

    #[serde(rename = "Tags", skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,

    #[serde(rename = "Web", skip_serializing_if = "Option::is_none")]
    pub web: Option<String>,

    #[serde(
        rename = "PageCount",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub page_count: Option<i32>,

    #[serde(rename = "LanguageISO", skip_serializing_if = "Option::is_none")]
    pub language_iso: Option<String>,

    #[serde(rename = "Format", skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    #[serde(
        rename = "BlackAndWhite",
        default,
        deserialize_with = "lenient_opt_enum",
        skip_serializing_if = "Option::is_none"
    )]
    pub black_and_white: Option<Lenient<YesNo>>,

    #[serde(
        rename = "Manga",
        default,
        deserialize_with = "lenient_opt_enum",
        skip_serializing_if = "Option::is_none"
    )]
    pub manga: Option<Lenient<Manga>>,

    #[serde(rename = "Characters", skip_serializing_if = "Option::is_none")]
    pub characters: Option<String>,

    #[serde(rename = "Teams", skip_serializing_if = "Option::is_none")]
    pub teams: Option<String>,

    #[serde(rename = "Locations", skip_serializing_if = "Option::is_none")]
    pub locations: Option<String>,

    #[serde(rename = "ScanInformation", skip_serializing_if = "Option::is_none")]
    pub scan_information: Option<String>,

    #[serde(rename = "StoryArc", skip_serializing_if = "Option::is_none")]
    pub story_arc: Option<String>,

    #[serde(rename = "StoryArcNumber", skip_serializing_if = "Option::is_none")]
    pub story_arc_number: Option<String>,

    #[serde(rename = "SeriesGroup", skip_serializing_if = "Option::is_none")]
    pub series_group: Option<String>,

    #[serde(
        rename = "AgeRating",
        default,
        deserialize_with = "lenient_opt_enum",
        skip_serializing_if = "Option::is_none"
    )]
    pub age_rating: Option<Lenient<AgeRating>>,

    #[serde(
        rename = "CommunityRating",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub community_rating: Option<f64>,

    #[serde(rename = "MainCharacterOrTeam", skip_serializing_if = "Option::is_none")]
    pub main_character_or_team: Option<String>,

    #[serde(rename = "Review", skip_serializing_if = "Option::is_none")]
    pub review: Option<String>,

    #[serde(rename = "GTIN", skip_serializing_if = "Option::is_none")]
    pub gtin: Option<String>,

    /// Per-page metadata. Not editable in the UI, but preserved on save so it
    /// is not destroyed when re-writing the archive.
    #[serde(rename = "Pages", skip_serializing_if = "Option::is_none")]
    pub pages: Option<Pages>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Pages {
    #[serde(rename = "Page", default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Page {
    #[serde(
        rename = "@Image",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub image: Option<i32>,

    #[serde(rename = "@Type", skip_serializing_if = "Option::is_none")]
    pub page_type: Option<String>,

    #[serde(
        rename = "@DoublePage",
        default,
        deserialize_with = "lenient_opt_bool",
        skip_serializing_if = "Option::is_none"
    )]
    pub double_page: Option<bool>,

    #[serde(
        rename = "@ImageSize",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub image_size: Option<i64>,

    #[serde(rename = "@Key", skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(rename = "@Bookmark", skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,

    #[serde(
        rename = "@ImageWidth",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub image_width: Option<i32>,

    #[serde(
        rename = "@ImageHeight",
        default,
        deserialize_with = "lenient_opt_num",
        skip_serializing_if = "Option::is_none"
    )]
    pub image_height: Option<i32>,
}

/// True for characters XML 1.0 permits in a document. Control characters other
/// than tab/CR/LF are forbidden outright — they cannot even be escaped as
/// numeric references — so the only valid handling is to drop them.
fn is_valid_xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || matches!(c, ' '..='\u{D7FF}')
        || matches!(c, '\u{E000}'..='\u{FFFD}')
        || matches!(c, '\u{10000}'..='\u{10FFFF}')
}

/// Strip characters that would make the emitted XML unparseable. Text pasted
/// into a Summary or Notes field can carry NULs and other control codes;
/// quick-xml writes them through verbatim, producing a file that other
/// ComicInfo readers (Komga, ComicRack) reject.
fn strip_invalid_xml_chars(s: &str) -> String {
    s.chars().filter(|&c| is_valid_xml_char(c)).collect()
}

impl ComicInfo {
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        from_str(xml).map_err(|e| format!("Failed to parse ComicInfo.xml: {}", e))
    }

    pub fn to_xml(&self) -> Result<String, String> {
        let xml_body = to_string(self).map_err(|e| format!("Failed to serialize ComicInfo: {}", e))?;
        // Only pay for the rebuild when something actually needs removing.
        let xml_body = if xml_body.chars().all(is_valid_xml_char) {
            xml_body
        } else {
            strip_invalid_xml_chars(&xml_body)
        };
        Ok(format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}", xml_body))
    }

    /// Reject values that are out of range for the ComicInfo schema before they
    /// reach a file. The GUI's `min`/`max` attributes only apply on form
    /// submission, which never happens, and the CLI has no bounds at all — so
    /// this is the single place both paths are actually checked.
    ///
    /// Note that -1 is the schema's "unset" sentinel for the numeric fields
    /// that declare it as their default, and must stay acceptable.
    pub fn validate(&self) -> Result<(), String> {
        fn check_range(name: &str, value: Option<i32>, min: i32, max: i32) -> Result<(), String> {
            match value {
                Some(v) if v != -1 && (v < min || v > max) => Err(format!(
                    "{} must be between {} and {} (or -1 for unset), got {}",
                    name, min, max, v
                )),
                _ => Ok(()),
            }
        }

        check_range("Year", self.year, 1, 9999)?;
        check_range("Month", self.month, 1, 12)?;
        check_range("Day", self.day, 1, 31)?;
        check_range("Count", self.count, 0, i32::MAX)?;
        check_range("Volume", self.volume, 0, i32::MAX)?;
        check_range("AlternateCount", self.alternate_count, 0, i32::MAX)?;

        if let Some(pc) = self.page_count {
            if pc < 0 {
                return Err(format!("PageCount cannot be negative, got {}", pc));
            }
        }

        if let Some(r) = self.community_rating {
            if !r.is_finite() || !(0.0..=5.0).contains(&r) {
                return Err(format!("CommunityRating must be between 0 and 5, got {}", r));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_round_trip_is_preserved() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo>
  <Title>Issue 1</Title>
  <Series>My Series</Series>
  <Pages>
    <Page Image="0" Type="FrontCover" ImageSize="123456" ImageWidth="800" ImageHeight="1200"/>
    <Page Image="1" ImageSize="98765"/>
  </Pages>
</ComicInfo>"#;

        let info = ComicInfo::from_xml(xml).expect("parse");
        let pages = info.pages.as_ref().expect("pages parsed");
        assert_eq!(pages.pages.len(), 2);
        assert_eq!(pages.pages[0].image, Some(0));
        assert_eq!(pages.pages[0].page_type.as_deref(), Some("FrontCover"));
        assert_eq!(pages.pages[0].image_size, Some(123456));

        // Re-serialize and re-parse: the page data must survive.
        let out = info.to_xml().expect("serialize");
        let reparsed = ComicInfo::from_xml(&out).expect("reparse");
        assert_eq!(info.pages, reparsed.pages);
        assert_eq!(reparsed.pages.unwrap().pages.len(), 2);
    }

    /// A value no ComicInfo enum lists must not fail the parse — one stray word
    /// used to make the whole archive impossible to open. The original text is
    /// kept so saving does not quietly rewrite it to something else.
    #[test]
    fn off_spec_enum_values_parse_and_round_trip() {
        let xml = r#"<ComicInfo><Series>S</Series><AgeRating>PG-13</AgeRating><BlackAndWhite>true</BlackAndWhite></ComicInfo>"#;
        let info = ComicInfo::from_xml(xml).expect("must not reject an off-spec enum value");
        assert_eq!(info.age_rating, Some(Lenient::Other("PG-13".to_string())));
        assert_eq!(info.black_and_white, Some(Lenient::Other("true".to_string())));

        let out = info.to_xml().expect("serialize");
        assert!(out.contains("<AgeRating>PG-13</AgeRating>"), "got: {}", out);
        assert_eq!(ComicInfo::from_xml(&out).unwrap().age_rating, info.age_rating);
    }

    #[test]
    fn spec_enum_values_still_parse_as_known() {
        let xml = r#"<ComicInfo><AgeRating>Mature 17+</AgeRating><Manga>YesAndRightToLeft</Manga><BlackAndWhite>Yes</BlackAndWhite></ComicInfo>"#;
        let info = ComicInfo::from_xml(xml).expect("parse");
        assert_eq!(info.age_rating, Some(Lenient::Known(AgeRating::Mature17)));
        assert_eq!(info.manga, Some(Lenient::Known(Manga::YesAndRightToLeft)));
        assert_eq!(info.black_and_white, Some(Lenient::Known(YesNo::Yes)));
        assert!(info.to_xml().unwrap().contains("<AgeRating>Mature 17+</AgeRating>"));
    }

    /// Other taggers write `<Count/>` and `<Count></Count>` for a field they
    /// have no value for. Treating that as a malformed number rejected the file.
    #[test]
    fn empty_numeric_elements_are_absent_not_errors() {
        let xml = r#"<ComicInfo><Series>S</Series><Count></Count><Volume/><Year> </Year><CommunityRating/></ComicInfo>"#;
        let info = ComicInfo::from_xml(xml).expect("must not reject empty numeric elements");
        assert_eq!(info.count, None);
        assert_eq!(info.volume, None);
        assert_eq!(info.year, None);
        assert_eq!(info.community_rating, None);
        assert_eq!(info.series.as_deref(), Some("S"));

        // Real numbers, and the -1 unset sentinel, still come through.
        let xml = r#"<ComicInfo><Count>12</Count><Volume>-1</Volume><CommunityRating>4.5</CommunityRating></ComicInfo>"#;
        let info = ComicInfo::from_xml(xml).expect("parse");
        assert_eq!((info.count, info.volume, info.community_rating), (Some(12), Some(-1), Some(4.5)));
    }

    /// `True`/`1` are how other writers spell a true boolean attribute.
    #[test]
    fn page_booleans_accept_other_spellings() {
        for (raw, expected) in [("True", true), ("true", true), ("1", true), ("False", false), ("0", false)] {
            let xml = format!(r#"<ComicInfo><Pages><Page Image="0" DoublePage="{}"/></Pages></ComicInfo>"#, raw);
            let info = ComicInfo::from_xml(&xml).unwrap_or_else(|e| panic!("{} rejected: {}", raw, e));
            assert_eq!(info.pages.unwrap().pages[0].double_page, Some(expected), "input: {}", raw);
        }
    }

    /// The shape src/main.js sends on save must still deserialize, including a
    /// non-standard enum value the form preserved in an injected <option>.
    #[test]
    fn frontend_json_payload_still_deserializes() {
        let payload = r#"{"Title":"T","Year":2020,"Month":null,"Count":-1,
            "CommunityRating":4.5,"AgeRating":"Mature 17+","BlackAndWhite":"Yes","Manga":null}"#;
        let info: ComicInfo = serde_json::from_str(payload).expect("frontend payload");
        assert_eq!(info.year, Some(2020));
        assert_eq!(info.month, None);
        assert_eq!(info.count, Some(-1));
        assert_eq!(info.community_rating, Some(4.5));
        assert_eq!(info.age_rating, Some(Lenient::Known(AgeRating::Mature17)));

        let info: ComicInfo = serde_json::from_str(r#"{"AgeRating":"PG-13"}"#).unwrap();
        assert!(info.to_xml().unwrap().contains("<AgeRating>PG-13</AgeRating>"));
    }
}

