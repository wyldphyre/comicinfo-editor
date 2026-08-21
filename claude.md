# ComicInfo Editor - Development Guide

## Project Overview
A cross-platform desktop application for editing ComicInfo v2.1 metadata in CBZ comic archive files, built with Tauri (Rust backend + vanilla JS frontend).

## Technology Stack
- **Framework**: Tauri v2 (Rust backend + web frontend)
- **Frontend**: Vanilla HTML/CSS/JavaScript
- **Archive Handling**: Rust `zip` crate, `sevenz-rust2` crate, `unar` CLI (for RAR)
- **XML Parsing**: Rust `quick-xml` crate with serde

## Project Structure
```
comicinfo-editor/
├── src/                    # Frontend
│   ├── index.html          # Main UI with tabbed form
│   ├── styles.css          # Dark/light theme styling
│   └── main.js             # Frontend logic & Tauri API calls
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs         # Entry point (GUI vs CLI dispatch)
│   │   ├── lib.rs          # Tauri commands & archive read/write
│   │   ├── cli.rs          # Batch mode (--infer / --set)
│   │   ├── filename_parser.rs  # Metadata inference from filenames
│   │   └── comicinfo.rs    # ComicInfo struct, XML handling, validation
│   ├── Cargo.toml          # Rust dependencies
│   ├── tauri.conf.json     # App configuration
│   └── capabilities/       # Tauri permissions
├── .vscode/                # VS Code debug config
└── package.json
```

## Implemented Features

### Core Functionality
- Open CBZ files via native file dialog
- Parse ComicInfo.xml from archives
- Edit all ComicInfo v2.1 fields (organized in 9 tabs)
- Save changes back to CBZ files
- Cover image preview (first image in archive)
- Page count detection
- Drag-and-drop file opening
- Loading spinner during operations
- CBR/RAR and 7z/CB7 to CBZ conversion (prompts user before converting)
- Unsaved-changes prompt before opening another file or closing the window
- Batch CLI mode: `--infer` and `--set Field=Value` (see `--help`)

### UI Features
- Dark and light themes with toggle button (persisted in localStorage)
- Tabbed interface: Basic, Description, Publication, Credits, Publisher, Categories, Story Arc, Technical, Rating
- File name display with full path tooltip
- Status bar messages

## ComicInfo v2.1 Fields Supported

**Basic**: Title, Series, Number, Count, Volume, AlternateSeries, AlternateNumber, AlternateCount

**Description**: Summary, Notes, Review

**Publication**: Year, Month, Day

**Credits**: Writer, Penciller, Inker, Colorist, Letterer, CoverArtist, Editor, Translator

**Publisher**: Publisher, Imprint

**Categories**: Genre, Tags, Characters, Teams, Locations, MainCharacterOrTeam

**Story Arc**: StoryArc, StoryArcNumber, SeriesGroup

**Technical**: Format, PageCount, LanguageISO, Web, GTIN, ScanInformation, BlackAndWhite, Manga

**Rating**: AgeRating (14 enum values), CommunityRating (0-5)

## Development

### Prerequisites
- Rust (install via rustup)
- Node.js
- Tauri CLI: `cargo install tauri-cli`

### Run Development Server
```bash
cargo tauri dev
```

### Build for Production
```bash
cargo tauri build
```
Binaries output to `src-tauri/target/release/bundle/`

### Debug in VS Code
1. Install CodeLLDB extension
2. Use "Debug Tauri (Rust)" launch config
3. Set breakpoints in Rust files

## Future Enhancements

### Phase 2
- Directory browsing (load multiple CBZ files)
- Batch editing (apply changes to multiple files)
- Field validation
- Templates (save/load metadata presets)
- Recent files list

### Phase 3
- Page metadata editing (individual page types/attributes)
- Keyboard shortcuts
- Undo/redo support

## Technical Notes

- CBZ files are ZIP archives with .cbz extension
- ComicInfo.xml is stored at the root of the archive
- The app preserves other files in the archive when saving
- XML field names use PascalCase (e.g., "AlternateSeries")
- Serde renames are used for XML compatibility, affecting JSON keys sent to frontend
- The GUI form has no `<Pages>` editor, so save payloads arrive without it;
  `write_comic_info` recovers Pages from the archive rather than dropping it
- Saves go to a temp file that is fsync'd and renamed, so an interrupted write
  cannot leave a truncated archive in place of the original
- Numeric fields use -1 as the schema's "unset" sentinel; `ComicInfo::validate`
  accepts it and is the single bounds check for both the GUI and CLI paths
- "Open With" arrives via `RunEvent::Opened` on macOS and via argv elsewhere;
  `tauri-plugin-single-instance` routes a second launch into the running window

## Resources

- [Tauri Documentation](https://tauri.app/)
- [ComicInfo Spec](https://github.com/anansi-project/comicinfo)
- [ComicInfo XSD](https://raw.githubusercontent.com/anansi-project/comicinfo/main/drafts/v2.1/ComicInfo.xsd)
