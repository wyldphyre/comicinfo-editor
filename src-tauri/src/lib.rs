mod comicinfo;
mod filename_parser;
pub mod cli;

use comicinfo::ComicInfo;
use std::cmp::Ordering;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use zip::read::ZipArchive;
use zip::write::ZipWriter;
use zip::write::SimpleFileOptions;
use base64::{engine::general_purpose::STANDARD, Engine};


struct AppState {
    /// File path received from "Open With" before the frontend listener was ready.
    pending_file: Mutex<Option<String>>,
    /// Set to true once the frontend has called frontend_ready().
    frontend_ready: Mutex<bool>,
}

const IMAGE_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];

/// Extensions we will accept from a file association, a drop, or the command
/// line. Anything else is not a comic archive we know how to open.
const ARCHIVE_EXTENSIONS: [&str; 6] = ["cbz", "zip", "cbr", "rar", "cb7", "7z"];

/// Everything the frontend needs after opening a file, gathered from a single
/// pass over the archive instead of three separate opens/scans.
#[derive(serde::Serialize)]
struct OpenResult {
    #[serde(rename = "comicInfo")]
    comic_info: ComicInfo,
    #[serde(rename = "pageCount")]
    page_count: i32,
    cover: Option<String>,
}

/// Strip leading zeros from a digit run so "007" and "7" compare equal.
fn trim_zeros(digits: &[u8]) -> &[u8] {
    let first_significant = digits.iter().position(|&b| b != b'0').unwrap_or(digits.len());
    &digits[first_significant..]
}

/// Order archive entry names the way a human orders pages: digit runs compare
/// numerically, so "page2" sorts before "page10". A plain lexicographic sort
/// gets that backwards for any archive without zero-padded page names.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    // Byte-wise is safe for UTF-8: its encoding preserves code point order,
    // and to_ascii_lowercase leaves multi-byte sequences untouched.
    let (ab, bb) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0usize, 0usize);

    while i < ab.len() && j < bb.len() {
        if ab[i].is_ascii_digit() && bb[j].is_ascii_digit() {
            let (a_start, b_start) = (i, j);
            while i < ab.len() && ab[i].is_ascii_digit() { i += 1; }
            while j < bb.len() && bb[j].is_ascii_digit() { j += 1; }

            // Comparing significant-digit count first means arbitrarily long
            // runs work without parsing into an integer that could overflow.
            let (na, nb) = (trim_zeros(&ab[a_start..i]), trim_zeros(&bb[b_start..j]));
            let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
            if ord != Ordering::Equal {
                return ord;
            }
        } else {
            let ord = ab[i].to_ascii_lowercase().cmp(&bb[j].to_ascii_lowercase());
            if ord != Ordering::Equal {
                return ord;
            }
            i += 1;
            j += 1;
        }
    }

    // Equal so far: shorter remainder first, then fall back to a byte compare
    // so names differing only in case or padding still get a stable order.
    (ab.len() - i).cmp(&(bb.len() - j)).then_with(|| a.cmp(b))
}

fn cover_data_url(contents: &[u8], name_lower: &str) -> String {
    let mime_type = if name_lower.ends_with(".png") {
        "image/png"
    } else if name_lower.ends_with(".gif") {
        "image/gif"
    } else if name_lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/jpeg"
    };
    format!("data:{};base64,{}", mime_type, STANDARD.encode(contents))
}

/// Open the archive once and gather the ComicInfo, page count, and cover image.
fn read_archive_full(path: &str) -> Result<OpenResult, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    let mut comic_xml: Option<String> = None;
    let mut page_count = 0;
    let mut images: Vec<(String, usize)> = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = entry.name().to_string();
        let name_lower = name.to_lowercase();

        if name_lower == "comicinfo.xml" {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)
                .map_err(|e| format!("Failed to read ComicInfo.xml: {}", e))?;
            comic_xml = Some(contents);
        } else if let Some(ext) = Path::new(&name_lower).extension() {
            if IMAGE_EXTENSIONS.contains(&ext.to_str().unwrap_or("")) {
                page_count += 1;
                images.push((name, i));
            }
        }
    }

    let comic_info = match comic_xml {
        Some(xml) => ComicInfo::from_xml(&xml)?,
        None => {
            // No ComicInfo.xml found — try to infer metadata from the filename
            let parsed = filename_parser::parse(path);
            ComicInfo {
                series: parsed.series,
                volume: parsed.volume,
                number: parsed.number,
                title: parsed.name,
                writer: parsed.artist,
                year: parsed.year,
                ..ComicInfo::default()
            }
        }
    };

    images.sort_by(|a, b| natural_cmp(&a.0, &b.0));

    // Prefer whichever page the metadata explicitly marks as the front cover;
    // fall back to the first page in reading order. `Image` indexes the page
    // list, not the zip entry list, so it applies after the sort above.
    let cover_index = comic_info
        .pages
        .as_ref()
        .and_then(|p| p.pages.iter().find(|pg| pg.page_type.as_deref() == Some("FrontCover")))
        .and_then(|pg| pg.image)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|&n| n < images.len())
        .unwrap_or(0);

    let cover = match images.get(cover_index) {
        Some((name, index)) => {
            let name_lower = name.to_lowercase();
            let mut entry = archive.by_index(*index)
                .map_err(|e| format!("Failed to read image: {}", e))?;
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents)
                .map_err(|e| format!("Failed to read image data: {}", e))?;
            Some(cover_data_url(&contents, &name_lower))
        }
        None => None,
    };

    Ok(OpenResult { comic_info, page_count, cover })
}

/// Read the ComicInfo from a CBZ archive. Returns `None` if no ComicInfo.xml is present.
pub fn read_comic_info(path: &str) -> Result<Option<ComicInfo>, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("Failed to read archive entry: {}", e))?;
        if entry.name().to_lowercase() == "comicinfo.xml" {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)
                .map_err(|e| format!("Failed to read ComicInfo.xml: {}", e))?;
            return Ok(Some(ComicInfo::from_xml(&contents)?));
        }
    }

    Ok(None)
}

/// Read the ComicInfo out of an already-open archive, ignoring any failure —
/// callers use this only to recover fields the incoming payload omitted.
fn existing_comic_info(archive: &mut ZipArchive<File>) -> Option<ComicInfo> {
    for i in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(i) else { continue };
        if entry.name().to_lowercase() != "comicinfo.xml" {
            continue;
        }
        let mut contents = String::new();
        if entry.read_to_string(&mut contents).is_ok() {
            return ComicInfo::from_xml(&contents).ok();
        }
        return None;
    }
    None
}

/// Write ComicInfo back into a CBZ archive, auto-populating PageCount if not set.
pub fn write_comic_info(path: &str, comic_info: ComicInfo) -> Result<(), String> {
    let mut comic_info = comic_info;
    comic_info.validate()?;

    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    // The GUI form has no <Pages> editor, so a save from the UI always arrives
    // with pages == None. Carry over whatever the archive already had instead
    // of destroying per-page bookmarks, cover markers, and dimensions.
    if comic_info.pages.is_none() {
        comic_info.pages = existing_comic_info(&mut archive).and_then(|existing| existing.pages);
    }

    // Auto-populate PageCount if not provided
    if comic_info.page_count.is_none() {
        let mut count = 0;
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index_raw(i) {
                let name = entry.name().to_lowercase();
                if let Some(ext) = Path::new(&name).extension() {
                    if IMAGE_EXTENSIONS.contains(&ext.to_str().unwrap_or("")) {
                        count += 1;
                    }
                }
            }
        }
        comic_info.page_count = Some(count);
    }

    let xml_content = comic_info.to_xml()?;
    // Include the pid so two concurrent CLI runs over the same folder can't
    // land on the same temp path and corrupt each other's output.
    let temp_path = format!("{}.{}.tmp", path, std::process::id());

    // Write to a temp file, then atomically rename over the original. On any
    // failure, remove the temp file so we don't leave a partial .tmp behind.
    let write_result = (|| -> Result<(), String> {
        let temp_file = File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {}", e))?;
        let mut writer = ZipWriter::new(temp_file);
        let xml_options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let mut comicinfo_exists = false;

        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| format!("Failed to read archive entry: {}", e))?;

            if entry.name().to_lowercase() == "comicinfo.xml" {
                comicinfo_exists = true;
                let name = entry.name().to_string();
                writer.start_file(&name, xml_options).map_err(|e| format!("Failed to write entry: {}", e))?;
                writer.write_all(xml_content.as_bytes()).map_err(|e| format!("Failed to write content: {}", e))?;
            } else {
                // Copy the raw, already-compressed bytes straight through. This
                // preserves each entry's original compression method and skips a
                // needless decompress/recompress round-trip.
                writer.raw_copy_file(entry).map_err(|e| format!("Failed to copy entry: {}", e))?;
            }
        }

        if !comicinfo_exists {
            writer.start_file("ComicInfo.xml", xml_options).map_err(|e| format!("Failed to write ComicInfo.xml: {}", e))?;
            writer.write_all(xml_content.as_bytes()).map_err(|e| format!("Failed to write content: {}", e))?;
        }

        let finished = writer.finish().map_err(|e| format!("Failed to finalize archive: {}", e))?;
        // Force the data out to disk before the rename. Without this, a crash
        // or power loss in between can leave the rename durable while the
        // contents are not — i.e. a truncated file where the original was.
        finished.sync_all().map_err(|e| format!("Failed to flush temp file: {}", e))?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    // A fresh temp file gets default permissions, so copy the original's over
    // before it takes its place.
    if let Ok(meta) = std::fs::metadata(path) {
        let _ = std::fs::set_permissions(&temp_path, meta.permissions());
    }

    if let Err(e) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to replace original file: {}", e));
    }

    Ok(())
}

/// Run blocking archive work off the async runtime. These commands are `async`,
/// so their bodies execute on a tokio worker; doing multi-hundred-megabyte
/// file I/O there starves the runtime and stalls every other command.
async fn blocking<T, F>(work: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("Background task failed: {}", e))?
}

#[tauri::command]
async fn open_cbz(path: String) -> Result<OpenResult, String> {
    blocking(move || read_archive_full(&path)).await
}

#[tauri::command]
async fn save_cbz(path: String, comic_info: ComicInfo) -> Result<(), String> {
    blocking(move || write_comic_info(&path, comic_info)).await
}

/// The default CBZ path a conversion would produce, and whether it already exists.
#[derive(serde::Serialize)]
struct ConversionTarget {
    path: String,
    exists: bool,
}

#[tauri::command]
async fn get_conversion_target(source_path: String) -> Result<ConversionTarget, String> {
    blocking(move || {
        let dest = Path::new(&source_path).with_extension("cbz");
        Ok(ConversionTarget {
            exists: dest.exists(),
            path: dest.to_string_lossy().to_string(),
        })
    })
    .await
}

#[tauri::command]
async fn convert_to_cbz(source_path: String, dest_path: Option<String>) -> Result<String, String> {
    blocking(move || convert_to_cbz_blocking(&source_path, dest_path)).await
}

fn convert_to_cbz_blocking(source_path: &str, dest_path: Option<String>) -> Result<String, String> {
    let source = Path::new(source_path);
    let ext = source.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // The frontend supplies an explicit destination when it had to resolve a
    // name collision; otherwise fall back to the default <source>.cbz.
    let dest_path = match dest_path {
        Some(p) => PathBuf::from(p),
        None => source.with_extension("cbz"),
    };

    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    // Writing the converted archive over its own source would truncate the file
    // we are still reading from.
    if dest_path == source {
        return Err("The converted file would overwrite the source archive. Choose a different name.".to_string());
    }

    match ext.as_str() {
        "7z" | "cb7" => extract_7z(source_path, temp_dir.path())?,
        "rar" | "cbr" => extract_rar(source_path, temp_dir.path())?,
        _ => return Err(format!("Unsupported format: .{}", ext)),
    }

    pack_to_cbz(temp_dir.path(), &dest_path)?;

    Ok(dest_path.to_string_lossy().to_string())
}

/// Resolve `.` and `..` lexically. `canonicalize` is not an option here: the
/// extraction target does not exist yet, and we need the answer before writing.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn extract_7z(source: &str, dest_dir: &Path) -> Result<(), String> {
    // sevenz-rust2's default extractor joins the entry name onto the
    // destination without checking the result stays inside it, so an archive
    // with "../" entry names can write anywhere the process can reach. Vet
    // every destination ourselves before handing off to the real extractor.
    let root = normalize(dest_dir);
    sevenz_rust2::decompress_file_with_extract_fn(source, dest_dir, move |entry, reader, dest| {
        if !normalize(dest).starts_with(&root) {
            return Err(sevenz_rust2::Error::Other(
                format!("Archive entry \"{}\" would extract outside the destination directory", entry.name()).into(),
            ));
        }
        sevenz_rust2::default_entry_extract_fn(entry, reader, dest)
    })
    .map_err(|e| format!("Failed to extract 7z archive: {}", e))
}

fn find_unar() -> Option<PathBuf> {
    // Bundled apps don't inherit the shell PATH, so check known install
    // locations directly first.
    let candidates = [
        "/opt/homebrew/bin/unar", // Apple Silicon Homebrew
        "/usr/local/bin/unar",    // Intel Mac Homebrew
        "/usr/bin/unar",          // Linux package managers
    ];
    if let Some(found) = candidates.iter().find(|p| Path::new(p).exists()).map(PathBuf::from) {
        return Some(found);
    }

    // Fall back to searching PATH — covers Linux, Windows, and custom installs.
    let exe_names: &[&str] = if cfg!(windows) { &["unar.exe", "unar"] } else { &["unar"] };
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in exe_names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn extract_rar(source: &str, dest_dir: &Path) -> Result<(), String> {
    let unar = find_unar()
        .ok_or_else(|| "unar is not installed or was not found on PATH. Install it (e.g. 'brew install unar' on macOS).".to_string())?;

    let output = std::process::Command::new(unar)
        .args(["-output-directory", &dest_dir.to_string_lossy(), "-force-overwrite", source])
        .output()
        .map_err(|e| format!("Failed to run unar: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("unar failed: {}", stderr));
    }
    Ok(())
}

fn pack_to_cbz(source_dir: &Path, dest_path: &Path) -> Result<(), String> {
    // Build into a temp file alongside the destination and rename on success,
    // so a failure part-way through leaves no half-written .cbz behind.
    let temp_path = dest_path.with_file_name(format!(
        "{}.{}.tmp",
        dest_path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));

    let write_result = (|| -> Result<(), String> {
        let dest_file = File::create(&temp_path)
            .map_err(|e| format!("Failed to create CBZ file: {}", e))?;
        let mut writer = ZipWriter::new(dest_file);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        // Collect every file (images, ComicInfo.xml, and any other sidecars) so
        // conversion doesn't silently drop existing metadata. Sorting keeps image
        // pages in order; ComicInfo.xml naturally sorts after numeric page names.
        let mut files: Vec<PathBuf> = Vec::new();
        collect_files(source_dir, &mut files)?;
        files.sort_by(|a, b| natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));

        for path in &files {
            let rel_path = path.strip_prefix(source_dir)
                .map_err(|e| format!("Path error: {}", e))?;
            // Zip entry names must use '/' separators on every platform, so
            // normalise the backslashes that Windows paths would otherwise produce.
            let entry_name = rel_path.to_string_lossy().replace('\\', "/");

            let mut contents = Vec::new();
            File::open(path)
                .and_then(|mut f| f.read_to_end(&mut contents))
                .map_err(|e| format!("Failed to read {}: {}", entry_name, e))?;

            writer.start_file(&entry_name, options)
                .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
            writer.write_all(&contents)
                .map_err(|e| format!("Failed to write content: {}", e))?;
        }

        let finished = writer.finish()
            .map_err(|e| format!("Failed to finalize CBZ: {}", e))?;
        finished.sync_all()
            .map_err(|e| format!("Failed to flush CBZ: {}", e))?;

        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    std::fs::rename(&temp_path, dest_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        format!("Failed to write converted file: {}", e)
    })
}

/// True for OS-generated junk that should never be packed into the archive.
fn is_junk_file(path: &Path) -> bool {
    // AppleDouble/resource-fork artifacts and thumbnail caches that archive
    // extraction may produce; not part of the user's actual content.
    path.components().any(|c| c.as_os_str() == "__MACOSX")
        || matches!(
            path.file_name().and_then(|n| n.to_str()),
            Some(".DS_Store") | Some("Thumbs.db")
        )
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |n| n.starts_with("._"))
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files(&path, files)?;
        } else if !is_junk_file(&path) {
            files.push(path);
        }
    }

    Ok(())
}

/// Pick the first argument that names a comic archive we can open. Used for
/// "Open With" on Windows and Linux, where the shell passes the file as a
/// command-line argument rather than through an OS event.
fn file_arg_from(args: &[String]) -> Option<String> {
    args.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .find(|a| {
            let p = Path::new(a.as_str());
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| ARCHIVE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        })
        .cloned()
}

/// Hand a path to the frontend, or hold it until the frontend reports ready.
///
/// The `frontend_ready` lock is held across the check-and-set so we can't race
/// `frontend_ready()`: otherwise the frontend could flip to ready (and take an
/// empty pending_file) between our check and our store, dropping the file.
fn dispatch_open_file(app: &tauri::AppHandle, path: String) {
    use tauri::{Emitter, Manager};
    let state = app.state::<AppState>();
    let ready = state.frontend_ready.lock().unwrap();
    if *ready {
        drop(ready);
        let _ = app.emit("open-file", path);
    } else {
        *state.pending_file.lock().unwrap() = Some(path);
    }
}

/// Called by the frontend once its `open-file` event listener is confirmed
/// registered. Emits any file that arrived before the frontend was ready,
/// and marks the frontend as ready so future opens emit directly.
#[tauri::command]
fn frontend_ready(state: tauri::State<AppState>, app: tauri::AppHandle) {
    use tauri::Emitter;
    let pending = {
        let mut ready = state.frontend_ready.lock().unwrap();
        *ready = true;
        state.pending_file.lock().unwrap().take()
    };
    if let Some(path) = pending {
        let _ = app.emit("open-file", path);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();

    // Must be registered before any other plugin. Without it, double-clicking a
    // second comic launches a whole separate app instead of opening the file in
    // the window that is already running.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            use tauri::Manager;
            if let Some(path) = file_arg_from(&argv) {
                dispatch_open_file(app, path);
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            pending_file: Mutex::new(None),
            frontend_ready: Mutex::new(false),
        })
        .setup(|_app| {
            // macOS delivers "Open With" through RunEvent::Opened, handled
            // below. Every other platform passes the path as argv[1], which
            // nothing was reading — so file associations silently opened an
            // empty editor on Windows and Linux.
            #[cfg(not(any(target_os = "macos", target_os = "ios")))]
            {
                use tauri::Manager;
                let args: Vec<String> = std::env::args().collect();
                if let Some(path) = file_arg_from(&args) {
                    // The frontend cannot be ready this early, so stash it
                    // directly; frontend_ready() will pick it up.
                    *_app.state::<AppState>().pending_file.lock().unwrap() = Some(path);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_cbz, save_cbz, get_conversion_target, convert_to_cbz, frontend_ready])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                for url in urls {
                    if url.scheme() == "file" {
                        if let Ok(path) = url.to_file_path() {
                            dispatch_open_file(app_handle, path.to_string_lossy().to_string());
                        }
                    }
                }
            }
            let _ = event;
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a CBZ in a temp dir: `entries` are (name, bytes) written in order.
    fn make_cbz(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (entry_name, bytes) in entries {
            writer.start_file(*entry_name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    fn entry_names(path: &Path) -> Vec<String> {
        let archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        archive.file_names().map(String::from).collect()
    }

    fn read_entry(path: &Path, name: &str) -> Vec<u8> {
        let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut buf = Vec::new();
        archive.by_name(name).unwrap().read_to_end(&mut buf).unwrap();
        buf
    }

    const XML_WITH_PAGES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo>
  <Title>Old Title</Title>
  <Series>My Series</Series>
  <Pages>
    <Page Image="0" Type="FrontCover" ImageSize="123456" ImageWidth="800" ImageHeight="1200"/>
    <Page Image="1" Bookmark="Chapter 2" ImageSize="98765"/>
  </Pages>
</ComicInfo>"#;

    /// The GUI builds its save payload field-by-field from the form, and the
    /// form has no <Pages> editor — so Pages arrives as None on every save.
    /// It must be recovered from the archive rather than written away.
    #[test]
    fn gui_save_preserves_pages() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "test.cbz", &[
            ("001.jpg", b"fake-jpeg-1"),
            ("002.jpg", b"fake-jpeg-2"),
            ("ComicInfo.xml", XML_WITH_PAGES.as_bytes()),
        ]);

        // Exactly the shape src/main.js collectFormData() produces: only the
        // fieldMap keys, nulls for blanks, and no Pages key at all.
        let payload = r#"{
            "Title": "New Title", "Series": "My Series", "Number": null,
            "Summary": null, "PageCount": null, "Writer": null
        }"#;
        let from_frontend: ComicInfo = serde_json::from_str(payload).unwrap();
        assert!(from_frontend.pages.is_none(), "precondition: frontend sends no Pages");

        write_comic_info(cbz.to_str().unwrap(), from_frontend).unwrap();

        let saved = read_comic_info(cbz.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(saved.title.as_deref(), Some("New Title"), "edit applied");

        let pages = saved.pages.expect("Pages must survive a GUI save");
        assert_eq!(pages.pages.len(), 2);
        assert_eq!(pages.pages[0].page_type.as_deref(), Some("FrontCover"));
        assert_eq!(pages.pages[0].image_width, Some(800));
        assert_eq!(pages.pages[1].bookmark.as_deref(), Some("Chapter 2"));
    }

    /// An explicit Pages value from the caller (the CLI read-modify-write path)
    /// still wins over whatever is already in the file.
    #[test]
    fn explicit_pages_are_not_overwritten_by_existing() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "test.cbz", &[
            ("001.jpg", b"x"),
            ("ComicInfo.xml", XML_WITH_PAGES.as_bytes()),
        ]);

        let info = ComicInfo {
            pages: Some(comicinfo::Pages {
                pages: vec![comicinfo::Page { image: Some(7), ..Default::default() }],
            }),
            ..Default::default()
        };
        write_comic_info(cbz.to_str().unwrap(), info).unwrap();

        let pages = read_comic_info(cbz.to_str().unwrap()).unwrap().unwrap().pages.unwrap();
        assert_eq!(pages.pages.len(), 1);
        assert_eq!(pages.pages[0].image, Some(7));
    }

    #[test]
    fn save_preserves_other_entries_and_their_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "test.cbz", &[
            ("001.jpg", b"page-one-bytes"),
            ("002.jpg", b"page-two-bytes"),
            ("extra/notes.txt", b"sidecar"),
        ]);

        write_comic_info(cbz.to_str().unwrap(), ComicInfo::default()).unwrap();

        let names = entry_names(&cbz);
        assert!(names.contains(&"001.jpg".to_string()));
        assert!(names.contains(&"extra/notes.txt".to_string()));
        assert!(names.contains(&"ComicInfo.xml".to_string()), "ComicInfo.xml added when absent");
        assert_eq!(read_entry(&cbz, "002.jpg"), b"page-two-bytes");
        assert_eq!(read_entry(&cbz, "extra/notes.txt"), b"sidecar");

        // PageCount is filled in from the actual image count.
        let saved = read_comic_info(cbz.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(saved.page_count, Some(2));
    }

    /// A failed write must leave the original file untouched and clean up after
    /// itself rather than stranding a .tmp beside it.
    #[test]
    fn rejected_save_leaves_original_intact() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "test.cbz", &[
            ("001.jpg", b"page-one"),
            ("ComicInfo.xml", XML_WITH_PAGES.as_bytes()),
        ]);

        let info = ComicInfo { month: Some(13), ..Default::default() };
        let err = write_comic_info(cbz.to_str().unwrap(), info).unwrap_err();
        assert!(err.contains("Month"), "got: {}", err);

        // Original still readable, still has its old title.
        let saved = read_comic_info(cbz.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(saved.title.as_deref(), Some("Old Title"));

        let strays: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(strays.is_empty(), "left temp files behind: {:?}", strays);
    }

    #[test]
    fn validate_accepts_the_schema_unset_sentinel() {
        let info = ComicInfo {
            year: Some(-1),
            month: Some(-1),
            volume: Some(-1),
            count: Some(-1),
            ..Default::default()
        };
        assert!(info.validate().is_ok(), "-1 is the ComicInfo default for these fields");
    }

    #[test]
    fn validate_rejects_out_of_range_values() {
        let cases: [(&str, ComicInfo); 5] = [
            ("Month", ComicInfo { month: Some(13), ..Default::default() }),
            ("Day", ComicInfo { day: Some(32), ..Default::default() }),
            ("Year", ComicInfo { year: Some(99999), ..Default::default() }),
            ("PageCount", ComicInfo { page_count: Some(-5), ..Default::default() }),
            ("CommunityRating", ComicInfo { community_rating: Some(47.0), ..Default::default() }),
        ];
        for (field, info) in cases {
            let err = info.validate().unwrap_err();
            assert!(err.contains(field), "expected {} in error, got: {}", field, err);
        }
    }

    /// Control characters cannot appear in XML 1.0 even as escapes, so they must
    /// be dropped rather than written into a file no other reader will accept.
    #[test]
    fn control_characters_are_stripped_from_output() {
        let info = ComicInfo {
            summary: Some("bad\u{0}text\u{8}here".to_string()),
            notes: Some("keeps\ttabs\nand newlines".to_string()),
            ..Default::default()
        };

        let xml = info.to_xml().unwrap();
        assert!(!xml.contains('\u{0}') && !xml.contains('\u{8}'), "control chars survived");
        assert!(xml.contains("badtexthere"));
        assert!(xml.contains("keeps\ttabs\nand newlines"), "legal whitespace must be kept");

        let reparsed = ComicInfo::from_xml(&xml).unwrap();
        assert_eq!(reparsed.summary.as_deref(), Some("badtexthere"));
    }

    #[test]
    fn natural_cmp_orders_pages_like_a_human() {
        let mut names = vec![
            "page10.jpg", "page2.jpg", "page1.jpg", "page20.jpg", "page3.jpg",
        ];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["page1.jpg", "page2.jpg", "page3.jpg", "page10.jpg", "page20.jpg"]);

        // Zero padding must not change the ordering.
        let mut mixed = vec!["p007.jpg", "p7a.jpg", "p10.jpg", "p1.jpg"];
        mixed.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(mixed, ["p1.jpg", "p007.jpg", "p7a.jpg", "p10.jpg"]);

        assert_eq!(natural_cmp("a", "a"), Ordering::Equal);
        assert_eq!(natural_cmp("a", "ab"), Ordering::Less);
    }

    /// Lexicographic order puts "page10" before "page2", picking the wrong cover
    /// for any archive whose page names are not zero-padded.
    #[test]
    fn cover_uses_first_page_in_natural_order() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "test.cbz", &[
            ("page10.jpg", b"tenth"),
            ("page2.jpg", b"second"),
            ("page1.jpg", b"first"),
        ]);

        let result = read_archive_full(cbz.to_str().unwrap()).unwrap();
        assert_eq!(result.page_count, 3);
        let expected = STANDARD.encode(b"first");
        assert!(result.cover.unwrap().ends_with(&expected), "cover should be page1.jpg");
    }

    #[test]
    fn cover_prefers_the_page_marked_front_cover() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<ComicInfo>
  <Pages>
    <Page Image="0" Type="Other"/>
    <Page Image="1" Type="FrontCover"/>
  </Pages>
</ComicInfo>"#;
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "test.cbz", &[
            ("000_insert.jpg", b"advert"),
            ("001_cover.jpg", b"the-real-cover"),
            ("ComicInfo.xml", xml.as_bytes()),
        ]);

        let result = read_archive_full(cbz.to_str().unwrap()).unwrap();
        let expected = STANDARD.encode(b"the-real-cover");
        assert!(result.cover.unwrap().ends_with(&expected), "should honour FrontCover");
    }

    #[test]
    fn file_arg_picks_a_real_archive_and_ignores_flags() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "comic.cbz", &[("001.jpg", b"x")]);
        let txt = dir.path().join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();

        let args = vec![
            "comicinfo-editor".to_string(),
            "--some-flag".to_string(),
            txt.to_string_lossy().to_string(),
            cbz.to_string_lossy().to_string(),
        ];
        assert_eq!(file_arg_from(&args).as_deref(), cbz.to_str());

        // argv[0] is the binary, never a file to open.
        assert_eq!(file_arg_from(&[cbz.to_string_lossy().to_string()]), None);
        // A path that does not exist is not an archive to open.
        assert_eq!(file_arg_from(&["exe".into(), "/nope/missing.cbz".into()]), None);
    }

    #[test]
    fn normalize_detects_traversal_out_of_the_extraction_root() {
        let root = normalize(Path::new("/tmp/extract"));
        assert!(normalize(Path::new("/tmp/extract/a/b.jpg")).starts_with(&root));
        assert!(normalize(Path::new("/tmp/extract/a/../b.jpg")).starts_with(&root));
        assert!(!normalize(Path::new("/tmp/extract/../../etc/passwd")).starts_with(&root));
        assert!(!normalize(Path::new("/tmp/extract/a/../../../evil")).starts_with(&root));
    }

    #[test]
    fn zip_without_comicinfo_infers_from_filename() {
        let dir = tempfile::tempdir().unwrap();
        let cbz = make_cbz(dir.path(), "My Series v03 ch07 - The Title.cbz", &[("001.jpg", b"x")]);

        let result = read_archive_full(cbz.to_str().unwrap()).unwrap();
        assert_eq!(result.comic_info.series.as_deref(), Some("My Series"));
        assert_eq!(result.comic_info.volume, Some(3));
        assert_eq!(result.comic_info.number.as_deref(), Some("7"));
        assert_eq!(result.comic_info.title.as_deref(), Some("The Title"));
    }
}
