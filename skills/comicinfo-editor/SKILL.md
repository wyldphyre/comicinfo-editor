---
name: comicinfo-editor
description: Read, set, clear, and infer ComicInfo.xml metadata in comic archives (CBZ), and convert CBR/RAR and CB7/7z archives to CBZ, using the comicinfo-editor command-line batch mode. Use when the user wants to tag, retag, fix, remove, or inspect comic or manga metadata (series, volume, number, year, writer, manga reading direction, age rating, etc.), fill metadata from filenames, or convert .cbr/.cb7 files to .cbz — for one file or a whole library.
---

# comicinfo-editor

`comicinfo-editor` is a desktop app (Tauri) for editing ComicInfo v2.1 metadata
in comic archives. The same binary has a **batch CLI mode** that never opens a
window. Always drive it through the CLI. Don't try to automate the GUI.

## Locating the binary

| Platform | Path |
|---|---|
| macOS (installed) | `/Applications/ComicInfo Editor.app/Contents/MacOS/comicinfo-editor` |
| Linux (.deb) | `comicinfo-editor` on `PATH` |
| Windows (.msi) | `comicinfo-editor.exe` in the install directory |
| Built from source | `src-tauri/target/release/comicinfo-editor` (or `target/debug/`) |

The macOS path has a space, so quote it. Setting a variable once keeps
commands readable:

```bash
CIE="/Applications/ComicInfo Editor.app/Contents/MacOS/comicinfo-editor"
"$CIE" --help
```

**CLI mode only starts when the arguments include `--read`, `--infer`,
`--set`, `--clear`, `--convert`, or `--help`.** Anything else, such as a bare path or only
`--recursive`, launches the GUI, which blocks or hands the file to an
already-running window. Always include at least one operation flag.

## Command syntax

```
comicinfo-editor --read [--json] [--recursive|-r] <PATH>...
comicinfo-editor [--infer] [--convert] [--set Field=Value]... [--clear Field]... [--recursive|-r] <PATH>...
```

| Option | Effect |
|---|---|
| `--read` | Print each file's metadata. Read-only: it can't be combined with `--infer`, `--set`, `--clear`, or `--convert`. |
| `--json` | With `--read`, print a JSON array instead of text. Only valid with `--read`. |
| `--infer` | Fill only **missing** fields from the filename: Series, Volume, Number, Year, Title, Writer. Never overwrites existing values. |
| `--set Field=Value` | Set one field on every matched file, overwriting any existing value. **Repeat `--set` for each field.** `--set=Field=Value` also works. |
| `--clear Field` | Remove a field entirely: the element is deleted from the XML. **Repeat `--clear` for each field.** `--clear=Field` also works. A field can't be both set and cleared in one run. |
| `--convert` | Convert `.cbr`/`.rar`/`.cb7`/`.7z` to `.cbz` next to the original. The original is kept. |
| `--recursive`, `-r` | Descend into subfolders when a PATH is a folder. |
| `--help`, `-h` | Print usage and the full field list to stdout. |

Order of operations for each file: convert (if needed), then `--infer`, then
each `--set` in order, then each `--clear`. A `--set` therefore overrides an
inferred value, and a `--clear` removes a field even if `--infer` just filled it.

### PATH arguments

- **Folder**: every `.cbz` directly inside it, or in all subfolders with `-r`.
- **Glob**: quote it so the tool expands it rather than the shell:
  `"/comics/**/*.cbz"`.
- **File**: a `.cbz`, or with `--convert` also `.cbr`/`.rar`/`.cb7`/`.7z`.
- Extensions are matched case-insensitively. Other files (`.zip`, `.pdf`,
  `.epub`) are ignored.
- Paths must be valid UTF-8.

### Quoting values

Put quotes around any value with spaces or shell metacharacters:

```bash
--set Series="Manatsu no Hanazono" --set AgeRating="Mature 17+"
```

Common mistake: `--set Series=X Manga=Yes dir` treats `Manga=Yes` as a path.
The tool rejects this before touching any file ("looks like a field
assignment, not a path"). The fix is `--set Series=X --set Manga=Yes dir`.
The same applies to `--clear Tags Review dir`; write
`--clear Tags --clear Review dir`.

## Fields for `--set` and `--clear`

Field names ignore case and underscores (`CoverArtist`, `coverartist`,
`cover_artist` all work). `bw` means BlackAndWhite and `language` means
LanguageISO.

- **Text**: Series, Title, Number, AlternateSeries, AlternateNumber, Summary,
  Notes, Review, Writer, Penciller, Inker, Colorist, Letterer, CoverArtist,
  Editor, Translator, Publisher, Imprint, Genre, Tags, Characters, Teams,
  Locations, MainCharacterOrTeam, Format, LanguageISO, Web, GTIN,
  ScanInformation, StoryArc, StoryArcNumber, SeriesGroup
- **Integer**: Volume (≥0), Count (≥0), AlternateCount (≥0), Year (1–9999),
  Month (1–12), Day (1–31), PageCount (≥0). `-1` is the schema's "unset" value.
- **Decimal**: CommunityRating (0–5)
- **BlackAndWhite**: `Yes`, `No`, `Unknown` (case-insensitive)
- **Manga**: `Yes`, `No`, `YesAndRightToLeft` (aliases `rtl`,
  `right-to-left`), `Unknown` (case-insensitive). Use `YesAndRightToLeft`
  for Japanese manga that reads right to left.
- **AgeRating** (exact spelling, case-sensitive): `Unknown`, `Everyone`,
  `Teen`, `M`, `MA15+`, `Mature 17+`, `Adults Only 18+`, `PG`, `G`, `R18+`,
  `X18+`, `Early Childhood`, `Everyone 10+`, `Kids to Adults`,
  `Rating Pending`. Forms without spaces or `+` are also accepted
  (`Mature17`, `AdultsOnly18`).

Number is text, not an integer, so `12.5` or `1a` are valid issue numbers.

Multi-value text fields (Writer, Genre, Tags, Characters, …) are
comma-separated by ComicInfo convention: `--set Genre="Romance, Slice of Life"`.

## What `--infer` recognises

The filename (without extension) is parsed like this:

| Pattern | Field |
|---|---|
| `v01`, `vol01`, `vol 1`, `vol. 1`, `volume 1` | Volume |
| `ch01`, `c01`, `ch 1`, `chapter 1`, or a bare trailing number (`Series 01`) | Number (leading zeros dropped) |
| `(2014)` or `[2014]` (exactly four digits) | Year |
| `[Name]` or `(Name)` as the **first** token | Writer |
| Text after a standalone ` - ` (brackets removed) | Title |
| Words before the first volume/number/year marker | Series |

Example: `Assassination Classroom v01 (2014) (Digital).cbz` gives
Series="Assassination Classroom", Volume=1, Year=2014.

Inference only fills fields that are absent. A field that already exists,
**even with an empty value**, is left alone. When a filename is ambiguous,
combine flags: `--infer --set Series="Correct Name"`.

## Reading current metadata

Use `--read --json` when you need to process the result. It accepts the same
PATH forms as editing (folder, glob, files, `-r`):

```bash
"$CIE" --read --json "Saga v01 (2012).cbz" "No Metadata.cbz" "Broken.cbz"
```

```json
[
  {
    "path": "Saga v01 (2012).cbz",
    "metadata": {
      "Series": "Saga",
      "Volume": 1,
      "Year": 2012,
      "PageCount": 22,
      "Manga": "No",
      "AgeRating": "Mature 17+",
      "CommunityRating": 4.5
    }
  },
  { "path": "No Metadata.cbz", "metadata": null },
  { "path": "Broken.cbz", "metadata": null, "error": "Failed to read archive: ..." }
]
```

- Keys are the ComicInfo field names, in schema order. Absent fields are
  omitted. Integers and CommunityRating are JSON numbers; everything else,
  enums included, is a string.
- `"metadata": null` with no `error` means the archive has no ComicInfo.xml.
  `--infer`/`--set` will create one.
- An existing `<Pages>` block appears as `"Pages": {"Page": [{"@Image": 0, ...}]}`.
- stdout carries only the JSON. Read errors are also printed to stderr, and
  the exit code is 1 if any file failed.

Plain `--read` prints a human-readable version, one block per file:

```
/comics/Saga v01 (2012).cbz
  Series: Saga
  Volume: 1
  Year: 2012
```

It shows `(no ComicInfo.xml)` or `(empty ComicInfo.xml)` for files without
values. Prefer JSON for anything you parse. In text mode an empty value
(`Series: `) looks very similar to a missing field.

## Output and exit codes

stdout gets one line per file, then a summary:

```
processed: /path/A v01.cbz (Series="A", Volume=1)
processed: /path/B.cbz (converted from /path/B.cbr; Year=2020)
converted: /path/C.cb7 -> /path/C.cbz
skipped:   /path/D.cbz (no changes)
skipped:   /path/E.cbr (/path/E.cbz already exists)

Done: 3 processed, 2 skipped, 1 errors
```

Errors go to stderr as `error:     <path>: <message>`. Warnings for missing or
unsupported paths go to stderr as `warning: …`.

- Exit `0`: every file succeeded or was skipped.
- Exit `1`: at least one file failed, no files matched, or the arguments were
  invalid (usage is printed to stderr).

## Behaviour to rely on

- **Upfront checks**: unknown field names, unparseable values (`Year=abc`,
  `Manga=maybe`), misplaced `Field=Value` or field-name arguments, and
  setting and clearing the same field all abort the run before any file is
  modified.
- **Range checks happen per file at write time**: `Month=13` or
  `CommunityRating=7` fails each file with an error and leaves it unchanged.
  Validate ranges yourself before a large batch.
- **Safe writes**: each archive is rewritten through a temp file and renamed,
  so an interrupted run never leaves a truncated archive. Images, other files,
  and existing `<Pages>` data are preserved.
- **PageCount** is counted from the images in the archive whenever a file is
  written without one. An existing PageCount is kept as is. To correct a wrong
  count, `--clear PageCount` and it is recounted on that write.
- **No-op detection**: if `--infer` finds nothing to fill, there is no
  `--set`, and every `--clear` names a field the file doesn't have, the file
  is not rewritten ("skipped … no changes").
- **Conversion**:
  - `--convert` never overwrites an existing `.cbz`, so re-running over a
    folder is safe.
  - With `--convert` alone, files that are already `.cbz` are ignored.
  - With `--infer`/`--set`/`--clear` added, the edits go into the newly
    created `.cbz` and are also applied to `.cbz` files already present.
  - The source archive is never deleted. Remove it yourself only after
    confirming the `.cbz`.
  - RAR/CBR conversion requires `unar` (macOS: `brew install unar`; Linux:
    the `unar` package). 7z/CB7 needs nothing extra.

## Limitations

- `--set Field=` (empty value) is **not** a clear. It writes an empty element
  (`<Series/>`), or `Unknown` for Manga/BlackAndWhite/AgeRating. `--infer`
  treats an empty element as already set and won't fill it. Use `--clear` to
  remove a field.
- No per-page (`<Pages>`) editing, and `Pages` can't be cleared.
- Only CBZ is read and written. PDF/EPUB are not supported. A CBR/CB7 must
  be converted before `--read` can see it.

## Recommended workflow

1. **Inspect first.** Run `--read --json` on the target files and look at the
   filenames to predict what `--infer` will produce.
2. **Try one file** before a whole library. Run the command on a single file
   and confirm with `--read --json`.
3. **Back up** before large `--set`/`--clear` batches. Both overwrite or
   remove existing values and there is no undo. Saving the `--read --json`
   output first gives you a record of the previous values.
4. **Run the batch**, then check the `Done:` line and the exit code. Re-run
   just the files listed under `error:`.
5. For **mixed series** in one folder, run a separate command per series
   (glob by series name) instead of one `--set Series=…` over everything.

## Examples

```bash
# Inspect a folder's metadata as JSON
"$CIE" --read --json ~/Comics/Saga

# Fill missing metadata from filenames across a library
"$CIE" --infer -r ~/Comics

# Convert all CBR/CB7 in a folder and tag them from their filenames
"$CIE" --convert --infer ~/Downloads/manga

# Tag a manga series: series name, right-to-left reading, rating
"$CIE" --infer \
  --set Series="Manatsu no Hanazono" \
  --set Manga=YesAndRightToLeft \
  --set AgeRating="Teen" \
  "$HOME/Comics/Manatsu no Hanazono"

# Set publisher on one series via glob (quoted)
"$CIE" --set Publisher="Viz" --set LanguageISO=en "/Volumes/Comics/Naruto/*.cbz"

# Remove scanner tags and a bogus review from a whole library
"$CIE" --clear Tags --clear Review --clear ScanInformation -r ~/Comics

# Fix a wrong page count (cleared, then recounted from the archive)
"$CIE" --clear PageCount "Watchmen 001.cbz"

# Fix a single issue's date and credits
"$CIE" --set Year=1986 --set Month=9 --set Writer="Alan Moore" --set Penciller="Dave Gibbons" \
  "Watchmen 001.cbz"
```
