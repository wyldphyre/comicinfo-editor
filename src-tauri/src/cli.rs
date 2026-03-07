use std::path::{Path, PathBuf};

use crate::comicinfo::{AgeRating, ComicInfo, Manga, YesNo};
use crate::filename_parser;

pub struct CliArgs {
    pub infer: bool,
    pub set_fields: Vec<(String, String)>,
    pub recursive: bool,
    pub paths: Vec<String>,
}

/// Returns true when the argument list should trigger CLI (batch) mode.
pub fn is_cli_mode(args: &[String]) -> bool {
    args.iter()
        .skip(1)
        .any(|a| a == "--infer" || a.starts_with("--set") || a == "--help" || a == "-h")
}

pub fn run(args_vec: Vec<String>) -> i32 {
    let cli = match parse_args(args_vec) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}\n", e);
            print_usage();
            return 1;
        }
    };

    let files = collect_files(&cli);

    if files.is_empty() {
        eprintln!("No CBZ files found matching the given paths.");
        return 1;
    }

    let mut processed = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for file in &files {
        match process_file(file, &cli) {
            Ok(Some(summary)) => {
                println!("processed: {} ({})", file.display(), summary);
                processed += 1;
            }
            Ok(None) => {
                println!("skipped:   {} (no changes)", file.display());
                skipped += 1;
            }
            Err(e) => {
                eprintln!("error:     {}: {}", file.display(), e);
                errors += 1;
            }
        }
    }

    println!("\nDone: {} processed, {} skipped, {} errors", processed, skipped, errors);

    if errors > 0 { 1 } else { 0 }
}

fn parse_args(args: Vec<String>) -> Result<CliArgs, String> {
    let mut cli = CliArgs {
        infer: false,
        set_fields: Vec::new(),
        recursive: false,
        paths: Vec::new(),
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--infer" => cli.infer = true,
            "--recursive" | "-r" => cli.recursive = true,
            "--set" => {
                i += 1;
                if i >= args.len() {
                    return Err("--set requires a value in Field=Value format".to_string());
                }
                cli.set_fields.push(split_field_value(&args[i])?);
            }
            arg if arg.starts_with("--set=") => {
                cli.set_fields.push(split_field_value(&arg["--set=".len()..])?);
            }
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            path => {
                cli.paths.push(path.to_string());
            }
        }
        i += 1;
    }

    if !cli.infer && cli.set_fields.is_empty() {
        return Err("No operation specified. Use --infer and/or --set Field=Value.".to_string());
    }

    if cli.paths.is_empty() {
        return Err("No paths specified.".to_string());
    }

    Ok(cli)
}

fn split_field_value(s: &str) -> Result<(String, String), String> {
    match s.find('=') {
        Some(pos) => Ok((s[..pos].to_string(), s[pos + 1..].to_string())),
        None => Err(format!("--set value must be in Field=Value format, got: {}", s)),
    }
}

fn collect_files(cli: &CliArgs) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    for path_str in &cli.paths {
        let p = Path::new(path_str);
        if p.is_dir() {
            collect_from_dir(p, cli.recursive, &mut files);
        } else if path_str.contains('*') || path_str.contains('?') {
            if let Ok(entries) = glob::glob(path_str) {
                for entry in entries.flatten() {
                    if entry.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("cbz")) {
                        files.push(entry);
                    }
                }
            } else {
                eprintln!("warning: invalid glob pattern: {}", path_str);
            }
        } else if p.exists() {
            if p.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("cbz")) {
                files.push(p.to_path_buf());
            } else {
                eprintln!("warning: {} is not a CBZ file, skipping", path_str);
            }
        } else {
            eprintln!("warning: {} not found, skipping", path_str);
        }
    }

    files.sort();
    files
}

fn collect_from_dir(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_from_dir(&path, recursive, files);
        } else if path.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("cbz")) {
            files.push(path);
        }
    }
}

fn process_file(path: &Path, cli: &CliArgs) -> Result<Option<String>, String> {
    let path_str = path.to_str().unwrap_or_default();
    let existing = crate::read_comic_info(path_str)?;
    let mut info = existing.unwrap_or_default();
    let mut changes: Vec<String> = Vec::new();

    if cli.infer {
        let parsed = filename_parser::parse(path_str);

        macro_rules! infer {
            ($field:ident, $label:expr) => {
                if info.$field.is_none() {
                    if let Some(val) = parsed.$field {
                        changes.push(format!("{}={:?}", $label, val));
                        info.$field = Some(val);
                    }
                }
            };
        }

        infer!(series, "Series");
        infer!(volume, "Volume");
        infer!(number, "Number");
        infer!(year, "Year");

        // name → title, artist → writer (field name mismatch between structs)
        if info.title.is_none() {
            if let Some(val) = parsed.name {
                changes.push(format!("Title={:?}", val));
                info.title = Some(val);
            }
        }
        if info.writer.is_none() {
            if let Some(val) = parsed.artist {
                changes.push(format!("Writer={:?}", val));
                info.writer = Some(val);
            }
        }
    }

    for (name, value) in &cli.set_fields {
        changes.push(apply_field(&mut info, name, value)?);
    }

    if changes.is_empty() {
        return Ok(None);
    }

    crate::write_comic_info(path_str, info)?;
    Ok(Some(changes.join(", ")))
}

fn apply_field(info: &mut ComicInfo, name: &str, value: &str) -> Result<String, String> {
    match name.to_ascii_lowercase().as_str() {
        "title" => { info.title = Some(value.into()); Ok(fmt_str("Title", value)) }
        "series" => { info.series = Some(value.into()); Ok(fmt_str("Series", value)) }
        "number" => { info.number = Some(value.into()); Ok(fmt_str("Number", value)) }
        "volume" => { let v = parse_i32("Volume", value)?; info.volume = Some(v); Ok(fmt_int("Volume", v)) }
        "count" => { let v = parse_i32("Count", value)?; info.count = Some(v); Ok(fmt_int("Count", v)) }
        "alternateseries" | "alternate_series" => { info.alternate_series = Some(value.into()); Ok(fmt_str("AlternateSeries", value)) }
        "alternatenumber" | "alternate_number" => { info.alternate_number = Some(value.into()); Ok(fmt_str("AlternateNumber", value)) }
        "alternatecount" | "alternate_count" => { let v = parse_i32("AlternateCount", value)?; info.alternate_count = Some(v); Ok(fmt_int("AlternateCount", v)) }
        "summary" => { info.summary = Some(value.into()); Ok(fmt_str("Summary", value)) }
        "notes" => { info.notes = Some(value.into()); Ok(fmt_str("Notes", value)) }
        "year" => { let v = parse_i32("Year", value)?; info.year = Some(v); Ok(fmt_int("Year", v)) }
        "month" => { let v = parse_i32("Month", value)?; info.month = Some(v); Ok(fmt_int("Month", v)) }
        "day" => { let v = parse_i32("Day", value)?; info.day = Some(v); Ok(fmt_int("Day", v)) }
        "writer" => { info.writer = Some(value.into()); Ok(fmt_str("Writer", value)) }
        "penciller" => { info.penciller = Some(value.into()); Ok(fmt_str("Penciller", value)) }
        "inker" => { info.inker = Some(value.into()); Ok(fmt_str("Inker", value)) }
        "colorist" => { info.colorist = Some(value.into()); Ok(fmt_str("Colorist", value)) }
        "letterer" => { info.letterer = Some(value.into()); Ok(fmt_str("Letterer", value)) }
        "coverartist" | "cover_artist" => { info.cover_artist = Some(value.into()); Ok(fmt_str("CoverArtist", value)) }
        "editor" => { info.editor = Some(value.into()); Ok(fmt_str("Editor", value)) }
        "translator" => { info.translator = Some(value.into()); Ok(fmt_str("Translator", value)) }
        "publisher" => { info.publisher = Some(value.into()); Ok(fmt_str("Publisher", value)) }
        "imprint" => { info.imprint = Some(value.into()); Ok(fmt_str("Imprint", value)) }
        "genre" => { info.genre = Some(value.into()); Ok(fmt_str("Genre", value)) }
        "tags" => { info.tags = Some(value.into()); Ok(fmt_str("Tags", value)) }
        "web" => { info.web = Some(value.into()); Ok(fmt_str("Web", value)) }
        "pagecount" | "page_count" => { let v = parse_i32("PageCount", value)?; info.page_count = Some(v); Ok(fmt_int("PageCount", v)) }
        "languageiso" | "language_iso" | "language" => { info.language_iso = Some(value.into()); Ok(fmt_str("LanguageISO", value)) }
        "format" => { info.format = Some(value.into()); Ok(fmt_str("Format", value)) }
        "blackandwhite" | "black_and_white" | "bw" => {
            let v = parse_yes_no("BlackAndWhite", value)?;
            info.black_and_white = Some(v);
            Ok(fmt_str("BlackAndWhite", value))
        }
        "manga" => {
            let v = parse_manga("Manga", value)?;
            info.manga = Some(v);
            Ok(fmt_str("Manga", value))
        }
        "characters" => { info.characters = Some(value.into()); Ok(fmt_str("Characters", value)) }
        "teams" => { info.teams = Some(value.into()); Ok(fmt_str("Teams", value)) }
        "locations" => { info.locations = Some(value.into()); Ok(fmt_str("Locations", value)) }
        "scaninformation" | "scan_information" => { info.scan_information = Some(value.into()); Ok(fmt_str("ScanInformation", value)) }
        "storyarc" | "story_arc" => { info.story_arc = Some(value.into()); Ok(fmt_str("StoryArc", value)) }
        "storyarcnumber" | "story_arc_number" => { info.story_arc_number = Some(value.into()); Ok(fmt_str("StoryArcNumber", value)) }
        "seriesgroup" | "series_group" => { info.series_group = Some(value.into()); Ok(fmt_str("SeriesGroup", value)) }
        "agerating" | "age_rating" => {
            let v = parse_age_rating("AgeRating", value)?;
            info.age_rating = Some(v);
            Ok(fmt_str("AgeRating", value))
        }
        "communityrating" | "community_rating" => {
            let v: f64 = value.parse().map_err(|_| format!("'{}' is not a valid number for CommunityRating", value))?;
            info.community_rating = Some(v);
            Ok(format!("CommunityRating={}", v))
        }
        "maincharacterorteam" | "main_character_or_team" => { info.main_character_or_team = Some(value.into()); Ok(fmt_str("MainCharacterOrTeam", value)) }
        "review" => { info.review = Some(value.into()); Ok(fmt_str("Review", value)) }
        "gtin" => { info.gtin = Some(value.into()); Ok(fmt_str("GTIN", value)) }
        _ => Err(format!("Unknown field: '{}'. Run --help for a list of valid fields.", name)),
    }
}

fn fmt_str(name: &str, value: &str) -> String {
    format!("{}={:?}", name, value)
}

fn fmt_int(name: &str, value: i32) -> String {
    format!("{}={}", name, value)
}

fn parse_i32(field: &str, value: &str) -> Result<i32, String> {
    value.parse::<i32>().map_err(|_| format!("'{}' is not a valid integer for {}", value, field))
}

fn parse_yes_no(field: &str, value: &str) -> Result<YesNo, String> {
    match value.to_ascii_lowercase().as_str() {
        "yes" => Ok(YesNo::Yes),
        "no" => Ok(YesNo::No),
        "unknown" | "" => Ok(YesNo::Unknown),
        _ => Err(format!("'{}' is not valid for {} — use Yes, No, or Unknown", value, field)),
    }
}

fn parse_manga(field: &str, value: &str) -> Result<Manga, String> {
    match value.to_ascii_lowercase().as_str() {
        "yes" => Ok(Manga::Yes),
        "no" => Ok(Manga::No),
        "yesandrightoleft" | "rtl" | "right-to-left" => Ok(Manga::YesAndRightToLeft),
        "unknown" | "" => Ok(Manga::Unknown),
        _ => Err(format!("'{}' is not valid for {} — use Yes, No, YesAndRightToLeft, or Unknown", value, field)),
    }
}

fn parse_age_rating(_field: &str, value: &str) -> Result<AgeRating, String> {
    match value {
        "Adults Only 18+" | "AdultsOnly18" => Ok(AgeRating::AdultsOnly18),
        "Early Childhood" | "EarlyChildhood" => Ok(AgeRating::EarlyChildhood),
        "Everyone" => Ok(AgeRating::Everyone),
        "Everyone 10+" | "Everyone10" => Ok(AgeRating::Everyone10),
        "G" => Ok(AgeRating::G),
        "Kids to Adults" | "KidsToAdults" => Ok(AgeRating::KidsToAdults),
        "M" => Ok(AgeRating::M),
        "MA15+" | "MA15" => Ok(AgeRating::MA15),
        "Mature 17+" | "Mature17" => Ok(AgeRating::Mature17),
        "PG" => Ok(AgeRating::PG),
        "R18+" | "R18" => Ok(AgeRating::R18),
        "Rating Pending" | "RatingPending" => Ok(AgeRating::RatingPending),
        "Teen" => Ok(AgeRating::Teen),
        "X18+" | "X18" => Ok(AgeRating::X18),
        "Unknown" | "" => Ok(AgeRating::Unknown),
        _ => Err(format!("'{}' is not a valid AgeRating value", value)),
    }
}

fn print_usage() {
    eprintln!(
        "Usage: comicinfo-editor [OPTIONS] <PATH>...

Options:
  --infer              Fill missing metadata fields inferred from the filename
  --set Field=Value    Set a specific metadata field (repeatable)
  --recursive, -r      Scan folders recursively (default: non-recursive)
  --help, -h           Show this help

PATH can be a folder, a glob pattern (quoted), or individual .cbz files.

Fields for --set:
  String:  Series, Title, Number, AlternateSeries, AlternateNumber, Summary,
           Notes, Writer, Penciller, Inker, Colorist, Letterer, CoverArtist,
           Editor, Translator, Publisher, Imprint, Genre, Tags, Characters,
           Teams, Locations, Format, LanguageISO, Web, GTIN, ScanInformation,
           StoryArc, StoryArcNumber, SeriesGroup, MainCharacterOrTeam, Review
  Integer: Volume, Count, AlternateCount, Year, Month, Day, PageCount
  Decimal: CommunityRating (0–5)
  Enum:    BlackAndWhite (Yes/No/Unknown)
           Manga (Yes/No/YesAndRightToLeft/Unknown)
           AgeRating (Unknown/Everyone/Teen/M/MA15+/Mature 17+/Adults Only 18+/
                      PG/G/R18+/X18+/Early Childhood/Everyone 10+/
                      Kids to Adults/Rating Pending)

Examples:
  # Infer metadata from filenames for all CBZ files in a folder
  comicinfo-editor --infer /path/to/comics/

  # Set series and year on all CBZ files in a folder
  comicinfo-editor --set Series=\"My Series\" --set Year=2020 /path/to/comics/

  # Infer, then override the series name
  comicinfo-editor --infer --set Series=\"Override\" /path/to/comics/

  # Use a glob pattern (quote it to prevent shell expansion)
  comicinfo-editor --set Publisher=\"Viz\" \"/Volumes/Comics/**/*.cbz\" --recursive

  # Run directly from the macOS app bundle
  /Applications/ComicInfo\\ Editor.app/Contents/MacOS/comicinfo-editor --infer /path/to/comics/
"
    );
}
