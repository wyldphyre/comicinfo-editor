use std::path::Path;

#[derive(Debug, Default, PartialEq)]
pub struct ParsedFilenameData {
    pub series: Option<String>,
    pub volume: Option<i32>,
    pub number: Option<String>,
    pub name: Option<String>,
    pub artist: Option<String>,
    pub year: Option<i32>,
}

pub fn parse(filename: &str) -> ParsedFilenameData {
    let mut result = ParsedFilenameData::default();
    let tokens = tokenise_to_words(filename);
    let mut previous_token: Option<String> = None;
    let mut series_tokens: Vec<String> = Vec::new();
    let mut is_past_series = false;

    const VOLUME_PRECEDING: &[&str] = &["vol", "vol.", "volume", "volume."];
    const CHAPTER_PRECEDING: &[&str] = &["ch", "ch.", "chp.", "chapter"];

    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        let trimmed = token.trim();
        let next_token = tokens.get(index + 1).map(|t| t.trim());

        if VOLUME_PRECEDING.iter().any(|t| t.eq_ignore_ascii_case(trimmed)) {
            is_past_series = true;
            if let Some(next) = tokens.get(index + 1) {
                if let Ok(v) = next.trim().parse::<i32>() {
                    result.volume = Some(v);
                    index += 1;
                }
            }
        } else if let Some(v) = try_volume_parse(trimmed) {
            result.volume = Some(v);
            is_past_series = true;
        } else if CHAPTER_PRECEDING.iter().any(|t| t.eq_ignore_ascii_case(trimmed)) {
            is_past_series = true;
            if let Some(next) = tokens.get(index + 1) {
                let potential = next.trim();
                if potential.parse::<f64>().is_ok() {
                    result.number = Some(trim_leading_zeros(potential));
                    index += 1;
                }
            }
        } else if let Some(num) = try_parse_number(trimmed) {
            result.number = Some(num);
            is_past_series = true;
            index += 1; // extra increment matching C# behaviour
        } else if let Some(year) = try_parse_year(trimmed) {
            result.year = Some(year);
            is_past_series = true;
        } else if let Some(artist) = try_parse_artist(trimmed) {
            // Only accept bracketed content in the first position
            if index == 0 {
                result.artist = Some(artist);
            }
        } else if trimmed == "-" {
            // What follows a '-' is the issue name, excluding bracketed content
            let remaining_text = tokens[index + 1..].join(" ");
            let remaining_text = remove_bracketed(remaining_text, '(', ')');
            let remaining_text = remove_bracketed(remaining_text, '[', ']');
            let remaining_text = remaining_text.split_whitespace().collect::<Vec<_>>().join(" ");
            let remaining_text = remaining_text.trim().to_string();
            if !remaining_text.is_empty() {
                result.name = Some(remaining_text);
            }
            is_past_series = true;
            if !tokens.is_empty() {
                index = tokens.len() - 1; // outer loop will increment to len()
            }
        } else if trimmed.parse::<f64>().is_ok()
            && (trimmed.contains('.')
                || !series_tokens.is_empty()
                || tokens.len() == 1
                || next_token == Some("-"))
        {
            if result.number.is_none() {
                let prev_is_vol = previous_token
                    .as_deref()
                    .map_or(false, |p| p == "vol");
                if !prev_is_vol {
                    let has_following_numbers = tokens[index + 1..]
                        .iter()
                        .any(|t| t.trim().parse::<f64>().is_ok());
                    if !has_following_numbers {
                        result.number = Some(trim_leading_zeros(trimmed));
                        is_past_series = true;
                    }
                }
            }
        } else if !is_past_series {
            series_tokens.push(trimmed.to_string());
        }

        previous_token = Some(token.to_string());
        index += 1;
    }

    if !series_tokens.is_empty() {
        result.series = Some(series_tokens.join(" "));
    }

    result
}

/// Split the filename up into parsable tokens/words.
pub fn tokenise_to_words(filename: &str) -> Vec<String> {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let name = stem.replace('_', " ");

    // If contains '.' but no ' ', split by '.' (dot-separated filenames)
    if name.contains('.') && !name.contains(' ') {
        return name
            .split('.')
            .filter(|t| !t.trim().is_empty())
            .map(String::from)
            .collect();
    }

    let chars: Vec<char> = name.chars().collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut in_brackets = false;
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];

        if !in_brackets && character == ' ' {
            index += 1;
            continue;
        }

        let start_pos = index;
        let mut character = character;

        loop {
            // Exit when we reach a space outside brackets, or end of string
            if !in_brackets && character == ' ' {
                break;
            }
            if index >= chars.len() {
                break;
            }

            index += 1;

            if index >= chars.len() {
                // Reached end of string; C# does 'continue' here which exits
                // the while loop on next check
                break;
            }

            // Apply bracket state change based on the character we just left.
            // Note: operator precedence quirk from C# is preserved:
            //   ')' always sets in_brackets = false
            //   '(' always sets in_brackets = true
            //   ']' only closes when in_brackets is true
            //   '[' only opens when in_brackets is false
            if (in_brackets && character == ']') || character == ')' {
                in_brackets = false;
            } else if (!in_brackets && character == '[') || character == '(' {
                in_brackets = true;
            }

            character = chars[index];
        }

        let token: String = chars[start_pos..index].iter().collect();
        tokens.push(token);
    }

    tokens
}

fn try_volume_parse(token: &str) -> Option<i32> {
    let lower = token.to_ascii_lowercase();
    // Check "vol" before "v" — more specific first
    if lower.starts_with("vol") {
        if let Ok(v) = token[3..].parse::<i32>() {
            return Some(v);
        }
    }
    if lower.starts_with('v') {
        if let Ok(v) = token[1..].parse::<i32>() {
            return Some(v);
        }
    }
    None
}

fn try_parse_number(token: &str) -> Option<String> {
    let lower = token.to_ascii_lowercase();
    // Check "ch" before "c" — more specific first
    if lower.starts_with("ch") {
        let rest = &token[2..];
        if rest.parse::<f64>().is_ok() {
            return Some(trim_leading_zeros(rest));
        }
    } else if lower.starts_with('c') {
        let rest = &token[1..];
        if rest.parse::<f64>().is_ok() {
            return Some(trim_leading_zeros(rest));
        }
    }
    None
}

fn try_parse_year(token: &str) -> Option<i32> {
    if token.len() != 6 {
        return None;
    }
    let bytes = token.as_bytes();
    let (open, close) = (bytes[0] as char, bytes[5] as char);
    if (open == '[' && close == ']') || (open == '(' && close == ')') {
        token[1..5].parse::<i32>().ok()
    } else {
        None
    }
}

fn try_parse_artist(token: &str) -> Option<String> {
    if token.len() < 2 {
        return None;
    }
    let mut chars = token.chars();
    let first = chars.next().unwrap();
    let last = token.chars().last().unwrap();
    if (first == '[' && last == ']') || (first == '(' && last == ')') {
        Some(token[1..token.len() - 1].trim().to_string())
    } else {
        None
    }
}

/// Remove the substring from the first occurrence of `open` to the last
/// occurrence of `close` (inclusive), matching the behaviour of a greedy
/// regex like `\(.*\)` or `\[.*\]`.
fn remove_bracketed(s: String, open: char, close: char) -> String {
    if let Some(start) = s.find(open) {
        if let Some(end) = s.rfind(close) {
            if start < end {
                return format!("{}{}", &s[..start], &s[end + 1..]);
            }
        }
    }
    s
}

fn trim_leading_zeros(s: &str) -> String {
    s.trim_start_matches('0').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Tokeniser tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_tokenise_to_words() {
        let cases: &[(&str, &[&str])] = &[
            ("Vampeerz v1 ch01.cbz", &["Vampeerz", "v1", "ch01"]),
            (
                "Assassination Classroom v01 (2014) (Digital) (Lovag-Empire).cbz",
                &["Assassination", "Classroom", "v01", "(2014)", "(Digital)", "(Lovag-Empire)"],
            ),
            (
                "Assassination Classroom v01.cbz",
                &["Assassination", "Classroom", "v01"],
            ),
            (
                "Assassination Classroom v10.cbz",
                &["Assassination", "Classroom", "v10"],
            ),
            (
                "Assassination Classroom vol01.cbz",
                &["Assassination", "Classroom", "vol01"],
            ),
            (
                "Assassination Classroom vol 10.cbz",
                &["Assassination", "Classroom", "vol", "10"],
            ),
            (
                "Assassination Classroom ch01.cbz",
                &["Assassination", "Classroom", "ch01"],
            ),
            (
                "Assassination Classroom ch 01.cbz",
                &["Assassination", "Classroom", "ch", "01"],
            ),
            ("Ah My Goddess 1.cbz", &["Ah", "My", "Goddess", "1"]),
            ("Ah My Goddess 10.cbz", &["Ah", "My", "Goddess", "10"]),
            (
                "Claymore 001 - Silver-eyed Slayer[m-s].cbz",
                &["Claymore", "001", "-", "Silver-eyed", "Slayer[m-s]"],
            ),
            (
                "[Tokuwotsumu] Tea Brown and Milk Tea [TZdY].cbz",
                &["[Tokuwotsumu]", "Tea", "Brown", "and", "Milk", "Tea", "[TZdY]"],
            ),
            (
                "(Isoya Yuki) The Day the Cherryfruit Ripens (Hirari 14) [project].cbz",
                &[
                    "(Isoya Yuki)",
                    "The",
                    "Day",
                    "the",
                    "Cherryfruit",
                    "Ripens",
                    "(Hirari 14)",
                    "[project]",
                ],
            ),
            (
                "[Garun] I Could Just Tell.cbz",
                &["[Garun]", "I", "Could", "Just", "Tell"],
            ),
            (
                "[Takemiya Jin] Yaezakura Sympathy 1 [TZdY].cbz",
                &["[Takemiya Jin]", "Yaezakura", "Sympathy", "1", "[TZdY]"],
            ),
            ("04.cbz", &["04"]),
            (
                "05 - Let's Be Careful With Summer.cbz",
                &["05", "-", "Let's", "Be", "Careful", "With", "Summer"],
            ),
            ("2000 AD 0001.cbz", &["2000", "AD", "0001"]),
            (
                "2000 AD 0345 (Cclay).cbz",
                &["2000", "AD", "0345", "(Cclay)"],
            ),
            // Missing closing brackets
            (
                "(Isoya Yuki) The Day (Hirari 14) [project].cbz",
                &["(Isoya Yuki)", "The", "Day", "(Hirari 14)", "[project]"],
            ),
            (
                "(Isoya Yuki) The Day (Hirari 14) [project.cbz",
                &["(Isoya Yuki)", "The", "Day", "(Hirari 14)", "[project"],
            ),
            (
                "(Isoya Yuki) The Day (Hirari 14) [project sd.cbz",
                &["(Isoya Yuki)", "The", "Day", "(Hirari 14)", "[project sd"],
            ),
            (
                "(Isoya Yuki The Day (Hirari 14) [project].cbz",
                &["(Isoya Yuki The Day (Hirari 14)", "[project]"],
            ),
            (
                "[Garun I Could Just Tell.cbz",
                &["[Garun I Could Just Tell"],
            ),
            // Uncommon patterns
            (
                "all.you.need.is.kill.001..kiriya.keij.cbz",
                &["all", "you", "need", "is", "kill", "001", "kiriya", "keij"],
            ),
            (
                "01 Prologue_-_Hidden_Name_[lililicious].cbz",
                &["01", "Prologue", "-", "Hidden", "Name", "[lililicious]"],
            ),
        ];

        for (filename, expected) in cases {
            let result = tokenise_to_words(filename);
            let expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
            assert_eq!(result, expected, "tokenise_to_words({:?})", filename);
        }
    }

    // -------------------------------------------------------------------------
    // Parse tests
    // -------------------------------------------------------------------------

    fn p(
        series: Option<&str>,
        volume: Option<i32>,
        number: Option<&str>,
        name: Option<&str>,
        artist: Option<&str>,
        year: Option<i32>,
    ) -> ParsedFilenameData {
        ParsedFilenameData {
            series: series.map(String::from),
            volume,
            number: number.map(String::from),
            name: name.map(String::from),
            artist: artist.map(String::from),
            year,
        }
    }

    #[test]
    fn test_parse() {
        let cases: &[(&str, ParsedFilenameData)] = &[
            // Volume + chapter
            (
                "Vampeerz v1 ch01.cbz",
                p(Some("Vampeerz"), Some(1), Some("1"), None, None, None),
            ),
            (
                "Vampeerz volume 1 ch01.cbz",
                p(Some("Vampeerz"), Some(1), Some("1"), None, None, None),
            ),
            (
                "Vampeerz volume. 1 ch01.cbz",
                p(Some("Vampeerz"), Some(1), Some("1"), None, None, None),
            ),
            // Year in brackets
            (
                "Assassination Classroom v01 (2014) (Digital) (Lovag-Empire).cbz",
                p(Some("Assassination Classroom"), Some(1), None, None, None, Some(2014)),
            ),
            // Volume variants
            (
                "Assassination Classroom v01.cbz",
                p(Some("Assassination Classroom"), Some(1), None, None, None, None),
            ),
            (
                "Assassination Classroom v10.cbz",
                p(Some("Assassination Classroom"), Some(10), None, None, None, None),
            ),
            (
                "Assassination Classroom vol01.cbz",
                p(Some("Assassination Classroom"), Some(1), None, None, None, None),
            ),
            (
                "Assassination Classroom vol10.cbz",
                p(Some("Assassination Classroom"), Some(10), None, None, None, None),
            ),
            (
                "Assassination Classroom vol 10.cbz",
                p(Some("Assassination Classroom"), Some(10), None, None, None, None),
            ),
            (
                "Assassination Classroom vol. 10.cbz",
                p(Some("Assassination Classroom"), Some(10), None, None, None, None),
            ),
            // Non-numeric after vol keyword — volume not parsed
            (
                "Assassination Classroom vol NotA Volume.cbz",
                p(Some("Assassination Classroom"), None, None, None, None, None),
            ),
            // Bare number
            (
                "Assassination Classroom 01.cbz",
                p(Some("Assassination Classroom"), None, Some("1"), None, None, None),
            ),
            // ch prefix variants
            (
                "Assassination Classroom ch01.cbz",
                p(Some("Assassination Classroom"), None, Some("1"), None, None, None),
            ),
            (
                "Assassination Classroom c01.cbz",
                p(Some("Assassination Classroom"), None, Some("1"), None, None, None),
            ),
            (
                "Assassination Classroom ch 01.cbz",
                p(Some("Assassination Classroom"), None, Some("1"), None, None, None),
            ),
            (
                "Assassination Classroom ch. 01.cbz",
                p(Some("Assassination Classroom"), None, Some("1"), None, None, None),
            ),
            (
                "Assassination Classroom ch 10.1.cbz",
                p(Some("Assassination Classroom"), None, Some("10.1"), None, None, None),
            ),
            (
                "Assassination Classroom ch. 10.1.cbz",
                p(Some("Assassination Classroom"), None, Some("10.1"), None, None, None),
            ),
            (
                "Assassination Classroom chp. 10.1.cbz",
                p(Some("Assassination Classroom"), None, Some("10.1"), None, None, None),
            ),
            (
                "Assassination Classroom Chp. 10.1.cbz",
                p(Some("Assassination Classroom"), None, Some("10.1"), None, None, None),
            ),
            (
                "Assassination Classroom chapter 01.cbz",
                p(Some("Assassination Classroom"), None, Some("1"), None, None, None),
            ),
            (
                "Assassination Classroom Chapter 10.1.cbz",
                p(Some("Assassination Classroom"), None, Some("10.1"), None, None, None),
            ),
            // Vol + Ch keywords
            (
                "Assassination Classroom Vol. 001 Ch. 001.cbz",
                p(Some("Assassination Classroom"), Some(1), Some("1"), None, None, None),
            ),
            // Bare float as chapter number
            (
                "Ah My Goddess 1.cbz",
                p(Some("Ah My Goddess"), None, Some("1"), None, None, None),
            ),
            (
                "Ah My Goddess 10.cbz",
                p(Some("Ah My Goddess"), None, Some("10"), None, None, None),
            ),
            // Issue name after '-'
            (
                "Claymore 001 - Silver-eyed Slayer[m-s].cbz",
                p(Some("Claymore"), None, Some("1"), Some("Silver-eyed Slayer"), None, None),
            ),
            (
                "Claymore 002 - Claws in the Sky [m-s].cbz",
                p(Some("Claymore"), None, Some("2"), Some("Claws in the Sky"), None, None),
            ),
            (
                "Claymore 002 - [m-s] Claws in the Sky.cbz",
                p(Some("Claymore"), None, Some("2"), Some("Claws in the Sky"), None, None),
            ),
            // Artist in brackets at position 0
            (
                "[Tokuwotsumu] Tea Brown and Milk Tea [TZdY].cbz",
                p(Some("Tea Brown and Milk Tea"), None, None, None, Some("Tokuwotsumu"), None),
            ),
            (
                "(Isoya Yuki) The Day the Cherryfruit Ripens (Hirari 14) [project].cbz",
                p(
                    Some("The Day the Cherryfruit Ripens"),
                    None,
                    None,
                    None,
                    Some("Isoya Yuki"),
                    None,
                ),
            ),
            (
                "[Garun] I Could Just Tell.cbz",
                p(Some("I Could Just Tell"), None, None, None, Some("Garun"), None),
            ),
            (
                "[Takemiya Jin] Yaezakura Sympathy 1 [TZdY].cbz",
                p(
                    Some("Yaezakura Sympathy"),
                    None,
                    Some("1"),
                    None,
                    Some("Takemiya Jin"),
                    None,
                ),
            ),
            // Single bare number
            ("04.cbz", p(None, None, Some("4"), None, None, None)),
            // Volume-only filenames
            ("vol04.cbz", p(None, Some(4), None, None, None, None)),
            ("vol 04.cbz", p(None, Some(4), None, None, None, None)),
            ("volume 04.cbz", p(None, Some(4), None, None, None, None)),
            ("volume. 04.cbz", p(None, Some(4), None, None, None, None)),
            // Number + name
            (
                "05 - Let's Be Careful With Summer.cbz",
                p(None, None, Some("5"), Some("Let's Be Careful With Summer"), None, None),
            ),
            (
                "05 - Let's Be Careful With Summer (test) [test].cbz",
                p(None, None, Some("5"), Some("Let's Be Careful With Summer"), None, None),
            ),
            (
                "05 - Let's Be Careful With Summer(test)[test].cbz",
                p(None, None, Some("5"), Some("Let's Be Careful With Summer"), None, None),
            ),
            // Series starting with a number
            (
                "2000 AD 0001.cbz",
                p(Some("2000 AD"), None, Some("1"), None, None, None),
            ),
            (
                "2000 AD 0345 (Cclay).cbz",
                p(Some("2000 AD"), None, Some("345"), None, None, None),
            ),
            // Number + year
            (
                "Ascender 001 (2019) (Digital) (Zone-Empire).cbz",
                p(Some("Ascender"), None, Some("1"), None, None, Some(2019)),
            ),
        ];

        for (filename, expected) in cases {
            let result = parse(filename);
            assert_eq!(result, *expected, "parse({:?})", filename);
        }
    }
}
