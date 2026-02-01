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
}

impl ComicInfo {
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        from_str(xml).map_err(|e| format!("Failed to parse ComicInfo.xml: {}", e))
    }

    pub fn to_xml(&self) -> Result<String, String> {
        let xml_body = to_string(self).map_err(|e| format!("Failed to serialize ComicInfo: {}", e))?;
        Ok(format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}", xml_body))
    }
}
