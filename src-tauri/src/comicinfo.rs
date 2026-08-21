use quick_xml::de::from_str;
use quick_xml::se::to_string;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename = "ComicInfo")]
pub struct ComicInfo {
    #[serde(rename = "Title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(rename = "Series", skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,

    #[serde(rename = "Number", skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,

    #[serde(rename = "Count", skip_serializing_if = "Option::is_none")]
    pub count: Option<i32>,

    #[serde(rename = "Volume", skip_serializing_if = "Option::is_none")]
    pub volume: Option<i32>,

    #[serde(rename = "AlternateSeries", skip_serializing_if = "Option::is_none")]
    pub alternate_series: Option<String>,

    #[serde(rename = "AlternateNumber", skip_serializing_if = "Option::is_none")]
    pub alternate_number: Option<String>,

    #[serde(rename = "AlternateCount", skip_serializing_if = "Option::is_none")]
    pub alternate_count: Option<i32>,

    #[serde(rename = "Summary", skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,

    #[serde(rename = "Notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    #[serde(rename = "Year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,

    #[serde(rename = "Month", skip_serializing_if = "Option::is_none")]
    pub month: Option<i32>,

    #[serde(rename = "Day", skip_serializing_if = "Option::is_none")]
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

    #[serde(rename = "PageCount", skip_serializing_if = "Option::is_none")]
    pub page_count: Option<i32>,

    #[serde(rename = "LanguageISO", skip_serializing_if = "Option::is_none")]
    pub language_iso: Option<String>,

    #[serde(rename = "Format", skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    #[serde(rename = "BlackAndWhite", skip_serializing_if = "Option::is_none")]
    pub black_and_white: Option<YesNo>,

    #[serde(rename = "Manga", skip_serializing_if = "Option::is_none")]
    pub manga: Option<Manga>,

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

    #[serde(rename = "AgeRating", skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<AgeRating>,

    #[serde(rename = "CommunityRating", skip_serializing_if = "Option::is_none")]
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
    #[serde(rename = "@Image", skip_serializing_if = "Option::is_none")]
    pub image: Option<i32>,

    #[serde(rename = "@Type", skip_serializing_if = "Option::is_none")]
    pub page_type: Option<String>,

    #[serde(rename = "@DoublePage", skip_serializing_if = "Option::is_none")]
    pub double_page: Option<bool>,

    #[serde(rename = "@ImageSize", skip_serializing_if = "Option::is_none")]
    pub image_size: Option<i64>,

    #[serde(rename = "@Key", skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    #[serde(rename = "@Bookmark", skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,

    #[serde(rename = "@ImageWidth", skip_serializing_if = "Option::is_none")]
    pub image_width: Option<i32>,

    #[serde(rename = "@ImageHeight", skip_serializing_if = "Option::is_none")]
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
}
