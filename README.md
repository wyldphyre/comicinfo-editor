# ComicInfo Editor

A cross-platform desktop application for editing ComicInfo.xml metadata in CBZ comic archive files.

## Features

- **Full ComicInfo v2.1 Support** - Edit all metadata fields including title, series, credits, ratings, and more
- **Cover Preview** - Displays the cover image from the CBZ archive
- **Drag & Drop** - Drop CBZ files directly onto the app to open them
- **Modern UI** - Clean interface with tabbed navigation and dark/light theme toggle
- **Cross-Platform** - Runs on Windows, macOS, and Linux

## Screenshots

*Coming soon*

## Installation

### Download

Download the latest release for your platform from the [Releases](https://github.com/yourusername/comicinfo-editor/releases) page:

- **Windows**: `.msi` installer
- **macOS**: `.dmg` disk image
- **Linux**: `.AppImage` or `.deb`

### Build from Source

Prerequisites:
- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/)
- Tauri CLI: `cargo install tauri-cli`

```bash
# Clone the repository
git clone https://github.com/yourusername/comicinfo-editor.git
cd comicinfo-editor

# Install dependencies
npm install

# Run in development mode
cargo tauri dev

# Build for production
cargo tauri build
```

## Usage

1. **Open a CBZ file** - Click "Open CBZ" or drag a file onto the window
2. **Edit metadata** - Navigate through tabs to edit different field categories:
   - **Basic** - Title, series, issue number, volume
   - **Description** - Summary, notes, review
   - **Publication** - Year, month, day
   - **Credits** - Writer, artist, colorist, etc.
   - **Publisher** - Publisher name and imprint
   - **Categories** - Genre, tags, characters, teams, locations
   - **Story Arc** - Story arc name and number
   - **Technical** - Format, language, page count, etc.
   - **Rating** - Age rating and community rating
3. **Save changes** - Click "Save" to write the metadata back to the CBZ file

## Supported Fields

All ComicInfo v2.1 specification fields are supported:

| Category | Fields |
|----------|--------|
| Basic | Title, Series, Number, Count, Volume, AlternateSeries, AlternateNumber, AlternateCount |
| Description | Summary, Notes, Review |
| Publication | Year, Month, Day |
| Credits | Writer, Penciller, Inker, Colorist, Letterer, CoverArtist, Editor, Translator |
| Publisher | Publisher, Imprint |
| Categories | Genre, Tags, Characters, Teams, Locations, MainCharacterOrTeam |
| Story Arc | StoryArc, StoryArcNumber, SeriesGroup |
| Technical | Format, PageCount, LanguageISO, Web, GTIN, ScanInformation, BlackAndWhite, Manga |
| Rating | AgeRating, CommunityRating |

## Tech Stack

- **[Tauri](https://tauri.app/)** - Rust-based framework for building desktop apps
- **Rust** - Backend for file operations and XML parsing
- **Vanilla JavaScript** - Frontend UI

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## Acknowledgments

- [ComicInfo Specification](https://github.com/anansi-project/comicinfo) by the Anansi Project

## TODO

- Allow passing a file to the app/bundle via the command line to open it
- Convert non-zip files to cbz files (support at least 7z and rar)