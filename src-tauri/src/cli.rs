use std::path::{Path, PathBuf};

use crate::comicinfo::{AgeRating, ComicInfo, Lenient, Manga, YesNo};
use crate::filename_parser;

pub struct CliArgs {
    pub infer: bool,
    pub convert: bool,
    pub set_fields: Vec<(String, String)>,
    pub recursive: bool,
    pub paths: Vec<String>,
}

/// Returns true when the argument list should trigger CLI (batch) mode.
pub fn is_cli_mode(args: &[String]) -> bool {
    args.iter()
        .skip(1)
        .any(|a| a == "--infer" || a == "--convert" || a.starts_with("--set") || a == "--help" || a == "-h")
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

    // Check every --set name and value against a throwaway record first. A typo
    // would otherwise fail identically on every file in the batch, after the
    // run had already started rewriting the ones that came before it.
    {
        let mut probe = ComicInfo::default();
        for (name, value) in &cli.set_fields {
            if let Err(e) = apply_field(&mut probe, name, value) {
                eprintln!("Error: {}", e);
                return 1;
            }
        }
    }

    let edits_metadata = cli.infer || !cli.set_fields.is_empty();

    let mut files = collect_files(&cli);
    // With --convert alone there is nothing to do to a file that is already a
    // CBZ; listing it would only report a bogus "converted: X -> X".
    if !edits_metadata {
        files.retain(|f| is_convertible(f));
    }

    if files.is_empty() {
        if !edits_metadata {
            eprintln!("No CBR/RAR or CB7/7z files found matching the given paths.");
        } else if cli.convert {
            eprintln!("No CBZ, CBR/RAR or CB7/7z files found matching the given paths.");
        } else {
            eprintln!("No CBZ files found matching the given paths.");
        }
        return 1;
    }

    let mut processed = 0usize;
    let mut skipped = 0usize;
    let mut errors = 0usize;

    for file in &files {
        // Non-CBZ archives are only collected under --convert. Convert first,
        // then run any metadata edits against the new CBZ.
        let (target, converted) = if is_convertible(file) {
            let dest = file.with_extension("cbz");
            // Never overwrite: the existing CBZ may hold edits the source
            // lacks. Treating this as a skip keeps re-runs over a folder clean.
            if dest.exists() {
                println!("skipped:   {} ({} already exists)", file.display(), dest.display());
                skipped += 1;
                continue;
            }
            match convert_file(file, &dest) {
                Ok(()) => (dest, true),
                Err(e) => {
                    eprintln!("error:     {}: {}", file.display(), e);
                    errors += 1;
                    continue;
                }
            }
        } else {
            (file.clone(), false)
        };

        if !edits_metadata {
            println!("converted: {} -> {}", file.display(), target.display());
            processed += 1;
            continue;
        }

        let prefix = if converted { format!("converted from {}; ", file.display()) } else { String::new() };
        match process_file(&target, &cli) {
            Ok(Some(summary)) => {
                println!("processed: {} ({}{})", target.display(), prefix, summary);
                processed += 1;
            }
            Ok(None) if converted => {
                println!("processed: {} ({}no metadata changes)", target.display(), prefix);
                processed += 1;
            }
            Ok(None) => {
                println!("skipped:   {} (no changes)", target.display());
                skipped += 1;
            }
            Err(e) => {
                eprintln!("error:     {}: {}{}", target.display(), prefix, e);
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
        convert: false,
        set_fields: Vec::new(),
        recursive: false,
        paths: Vec::new(),
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                // Requested help is output, not an error — it belongs on stdout
                // so it can be piped into a pager or grep.
                println!("{}", usage());
                std::process::exit(0);
            }
            "--infer" => cli.infer = true,
            "--convert" => cli.convert = true,
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

    if !cli.infer && !cli.convert && cli.set_fields.is_empty() {
        return Err("No operation specified. Use --infer, --convert and/or --set Field=Value.".to_string());
    }

    if cli.paths.is_empty() {
        return Err("No paths specified.".to_string());
    }

    // "--set A=1 B=2" reads B=2 as a path. It then only drew a "not found"
    // warning mid-run, so the field was silently dropped while the run went
    // ahead and edited every file without it.
    for path in &cli.paths {
        if let Some(field) = misplaced_set_field(path) {
            return Err(format!(
                "'{}' looks like a field assignment, not a path. Each field needs its own --set: --set {}",
                path, field
            ));
        }
    }

    Ok(cli)
}

/// Returns the argument when it is `Field=Value` for a known field and does not
/// exist on disk (a real file named like that is still treated as a path).
fn misplaced_set_field(arg: &str) -> Option<&str> {
    if Path::new(arg).exists() {
        return None;
    }
    let (name, value) = split_field_value(arg).ok()?;
    match apply_field(&mut ComicInfo::default(), &name, &value) {
        Err(e) if e.starts_with("Unknown field") => None,
        _ => Some(arg),
    }
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
            collect_from_dir(p, cli.recursive, cli.convert, &mut files);
        } else if path_str.contains('*') || path_str.contains('?') {
            if let Ok(entries) = glob::glob(path_str) {
                for entry in entries.flatten() {
                    if entry.is_file() && is_wanted(&entry, cli.convert) {
                        files.push(entry);
                    }
                }
            } else {
                eprintln!("warning: invalid glob pattern: {}", path_str);
            }
        } else if p.exists() {
            if is_wanted(p, cli.convert) {
                files.push(p.to_path_buf());
            } else if is_convertible(p) {
                eprintln!("warning: {} is not a CBZ file, skipping (use --convert to convert it)", path_str);
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

fn collect_from_dir(dir: &Path, recursive: bool, convert: bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        // A *directory* named "Series.cbz" has a .cbz extension too. Testing
        // the extension first queued it as a file, and the run then reported a
        // bogus "Failed to read archive" against it.
        if path.is_dir() {
            if recursive {
                collect_from_dir(&path, recursive, convert, files);
            }
        } else if is_wanted(&path, convert) {
            files.push(path);
        }
    }
}

fn has_cbz_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cbz"))
}

/// Archive formats `--convert` turns into CBZ — the same set the GUI offers.
fn is_convertible(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ["cbr", "rar", "cb7", "7z"].iter().any(|c| e.eq_ignore_ascii_case(c)))
}

fn is_wanted(path: &Path, convert: bool) -> bool {
    has_cbz_extension(path) || (convert && is_convertible(path))
}

fn convert_file(source: &Path, dest: &Path) -> Result<(), String> {
    let source_str = source.to_str().ok_or_else(|| "path is not valid UTF-8".to_string())?;
    let dest_str = dest.to_str().ok_or_else(|| "path is not valid UTF-8".to_string())?;
    crate::convert_to_cbz_blocking(source_str, Some(dest_str.to_string())).map(|_| ())
}

fn process_file(path: &Path, cli: &CliArgs) -> Result<Option<String>, String> {
    // Silently becoming "" here turned a path problem into a confusing
    // "Failed to open file" against an empty filename.
    let path_str = path
        .to_str()
        .ok_or_else(|| "path is not valid UTF-8".to_string())?;
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
            info.black_and_white = Some(Lenient::Known(v));
            Ok(fmt_str("BlackAndWhite", value))
        }
        "manga" => {
            let v = parse_manga("Manga", value)?;
            info.manga = Some(Lenient::Known(v));
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
            info.age_rating = Some(Lenient::Known(v));
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
        "yesandrighttoleft" | "rtl" | "right-to-left" => Ok(Manga::YesAndRightToLeft),
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
    eprintln!("{}", usage());
}

fn usage() -> String {
    String::from(
        "Usage: comicinfo-editor [OPTIONS] <PATH>...

Options:
  --infer              Fill missing metadata fields inferred from the filename
  --convert            Convert CBR/RAR and CB7/7z archives to CBZ alongside the
                       original (kept). Skipped if the .cbz already exists.
                       Combine with --infer/--set to edit the new CBZ.
                       RAR conversion requires unar.
  --set Field=Value    Set a specific metadata field (repeatable)
  --recursive, -r      Scan folders recursively (default: non-recursive)
  --help, -h           Show this help

PATH can be a folder, a glob pattern (quoted), or individual .cbz files
(plus .cbr/.rar/.cb7/.7z files with --convert).

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

  # Convert every CBR/CB7 in a folder to CBZ, filling metadata from filenames
  comicinfo-editor --convert --infer /path/to/comics/

  # Use a glob pattern (quote it to prevent shell expansion)
  comicinfo-editor --set Publisher=\"Viz\" \"/Volumes/Comics/**/*.cbz\" --recursive

  # Run directly from the macOS app bundle
  /Applications/ComicInfo\\ Editor.app/Contents/MacOS/comicinfo-editor --infer /path/to/comics/
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical spelling used in --help must round-trip. It previously did
    /// not: the match arm was missing a letter, so only the "rtl" aliases worked.
    #[test]
    fn manga_accepts_its_canonical_name() {
        for input in ["YesAndRightToLeft", "yesandrighttoleft", "YESANDRIGHTTOLEFT", "rtl", "right-to-left"] {
            assert_eq!(
                parse_manga("Manga", input).unwrap(),
                Manga::YesAndRightToLeft,
                "input: {}",
                input
            );
        }
        assert_eq!(parse_manga("Manga", "Yes").unwrap(), Manga::Yes);
        assert_eq!(parse_manga("Manga", "").unwrap(), Manga::Unknown);
        assert!(parse_manga("Manga", "sideways").is_err());
    }

    #[test]
    fn every_age_rating_in_the_help_text_parses() {
        let help = usage();
        for value in [
            "Unknown", "Everyone", "Teen", "M", "MA15+", "Mature 17+", "Adults Only 18+",
            "PG", "G", "R18+", "X18+", "Early Childhood", "Everyone 10+", "Kids to Adults",
            "Rating Pending",
        ] {
            assert!(help.contains(value), "{} is missing from --help", value);
            assert!(parse_age_rating("AgeRating", value).is_ok(), "{} does not parse", value);
        }
    }

    #[test]
    fn yes_no_parses_both_cases() {
        assert_eq!(parse_yes_no("BlackAndWhite", "YES").unwrap(), YesNo::Yes);
        assert_eq!(parse_yes_no("BlackAndWhite", "no").unwrap(), YesNo::No);
        assert!(parse_yes_no("BlackAndWhite", "maybe").is_err());
    }

    /// Every field named in the --help text must actually be settable.
    #[test]
    fn all_documented_set_fields_are_accepted() {
        let fields = [
            "Series", "Title", "Number", "AlternateSeries", "AlternateNumber", "Summary",
            "Notes", "Writer", "Penciller", "Inker", "Colorist", "Letterer", "CoverArtist",
            "Editor", "Translator", "Publisher", "Imprint", "Genre", "Tags", "Characters",
            "Teams", "Locations", "Format", "LanguageISO", "Web", "GTIN", "ScanInformation",
            "StoryArc", "StoryArcNumber", "SeriesGroup", "MainCharacterOrTeam", "Review",
        ];
        for field in fields {
            let mut info = ComicInfo::default();
            assert!(apply_field(&mut info, field, "x").is_ok(), "{} rejected", field);
        }

        for field in ["Volume", "Count", "AlternateCount", "Year", "Month", "Day", "PageCount"] {
            let mut info = ComicInfo::default();
            assert!(apply_field(&mut info, field, "3").is_ok(), "{} rejected", field);
            assert!(apply_field(&mut info, field, "abc").is_err(), "{} took a non-integer", field);
        }
    }

    #[test]
    fn set_value_splits_on_the_first_equals_only() {
        let (name, value) = split_field_value("Web=https://example.com/?a=1").unwrap();
        assert_eq!(name, "Web");
        assert_eq!(value, "https://example.com/?a=1");
        assert!(split_field_value("NoEquals").is_err());
    }

    /// A directory can be named "Series.cbz" too. Testing the extension before
    /// the file type queued it as a file, and the run reported a bogus
    /// "Failed to read archive" against it.
    #[test]
    fn a_directory_named_cbz_is_not_collected_as_a_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Series.cbz")).unwrap();
        std::fs::write(dir.path().join("real.cbz"), b"not a real zip, just a file").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        for recursive in [false, true] {
            let mut found = Vec::new();
            collect_from_dir(dir.path(), recursive, false, &mut found);
            let names: Vec<String> = found
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
                .collect();
            assert_eq!(names, vec!["real.cbz".to_string()], "recursive: {}", recursive);
        }
    }

    #[test]
    fn convertible_archives_are_collected_only_with_convert() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["a.cbz", "b.cbr", "c.RAR", "d.cb7", "e.7z", "f.zip", "g.txt"] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let names = |convert: bool| {
            let mut found = Vec::new();
            collect_from_dir(dir.path(), false, convert, &mut found);
            let mut names: Vec<String> = found
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
                .collect();
            names.sort();
            names
        };
        assert_eq!(names(false), vec!["a.cbz"]);
        assert_eq!(names(true), vec!["a.cbz", "b.cbr", "c.RAR", "d.cb7", "e.7z"]);
    }

    #[test]
    fn convert_alone_is_a_valid_operation() {
        let args = ["comicinfo-editor", "--convert", "x.cbr"].map(String::from).to_vec();
        assert!(is_cli_mode(&args));
        let cli = parse_args(args).unwrap();
        assert!(cli.convert && !cli.infer && cli.set_fields.is_empty());
    }

    #[test]
    fn field_assignment_without_set_is_rejected() {
        let args = ["comicinfo-editor", "--set", "Series=X", "Manga=YesAndRightToLeft", "comics"]
            .map(String::from)
            .to_vec();
        let err = parse_args(args).err().expect("should be rejected");
        assert!(err.contains("--set Manga=YesAndRightToLeft"), "got: {}", err);

        // An invalid value for a known field is still clearly a misplaced --set.
        assert!(misplaced_set_field("Year=abc").is_some());
        // Not a known field: leave it as an ordinary (missing) path.
        assert!(misplaced_set_field("draft=v2").is_none());
        assert!(misplaced_set_field("comics").is_none());
    }

    #[test]
    fn unknown_set_field_is_reported() {
        let mut info = ComicInfo::default();
        let err = apply_field(&mut info, "Sereis", "x").unwrap_err();
        assert!(err.contains("Unknown field"), "got: {}", err);
    }
}
