mod comicinfo;
mod filename_parser;
pub mod cli;

use comicinfo::ComicInfo;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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

    // Cover is the first image by sorted name.
    images.sort_by(|a, b| a.0.cmp(&b.0));
    let cover = match images.first() {
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

/// Write ComicInfo back into a CBZ archive, auto-populating PageCount if not set.
pub fn write_comic_info(path: &str, comic_info: ComicInfo) -> Result<(), String> {
    let mut comic_info = comic_info;

    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

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
    let temp_path = format!("{}.tmp", path);

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

        writer.finish().map_err(|e| format!("Failed to finalize archive: {}", e))?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(e);
    }

    if let Err(e) = std::fs::rename(&temp_path, path) {
        let _ = std::fs::remove_file(&temp_path);
        return Err(format!("Failed to replace original file: {}", e));
    }

    Ok(())
}

#[tauri::command]
async fn open_cbz(path: String) -> Result<OpenResult, String> {
    read_archive_full(&path)
}

#[tauri::command]
async fn save_cbz(path: String, comic_info: ComicInfo) -> Result<(), String> {
    write_comic_info(&path, comic_info)
}

#[tauri::command]
async fn convert_to_cbz(source_path: String) -> Result<String, String> {
    let source = Path::new(&source_path);
    let ext = source.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let dest_path = source.with_extension("cbz");

    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    match ext.as_str() {
        "7z" | "cb7" => extract_7z(&source_path, temp_dir.path())?,
        "rar" | "cbr" => extract_rar(&source_path, temp_dir.path())?,
        _ => return Err(format!("Unsupported format: .{}", ext)),
    }

    pack_to_cbz(temp_dir.path(), &dest_path)?;

    Ok(dest_path.to_string_lossy().to_string())
}

fn extract_7z(source: &str, dest_dir: &Path) -> Result<(), String> {
    sevenz_rust::decompress_file(source, dest_dir)
        .map_err(|e| format!("Failed to extract 7z archive: {}", e))
}

fn find_unar() -> Option<PathBuf> {
    // Bundled apps don't inherit shell PATH, so check known locations directly
    let candidates = [
        "/opt/homebrew/bin/unar",  // Apple Silicon Homebrew
        "/usr/local/bin/unar",     // Intel Mac Homebrew
        "/usr/bin/unar",
    ];
    candidates.iter().find(|p| Path::new(p).exists()).map(PathBuf::from)
}

fn extract_rar(source: &str, dest_dir: &Path) -> Result<(), String> {
    let unar = find_unar()
        .ok_or_else(|| "unar is not installed. Install it with: brew install unar".to_string())?;

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
    let image_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];

    let dest_file = File::create(dest_path)
        .map_err(|e| format!("Failed to create CBZ file: {}", e))?;
    let mut writer = ZipWriter::new(dest_file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    let mut image_files: Vec<PathBuf> = Vec::new();
    collect_images(source_dir, &image_extensions, &mut image_files)?;
    image_files.sort();

    for path in &image_files {
        let rel_path = path.strip_prefix(source_dir)
            .map_err(|e| format!("Path error: {}", e))?;
        let entry_name = rel_path.to_string_lossy().to_string();

        let mut contents = Vec::new();
        File::open(path)
            .and_then(|mut f| f.read_to_end(&mut contents))
            .map_err(|e| format!("Failed to read {}: {}", entry_name, e))?;

        writer.start_file(&entry_name, options)
            .map_err(|e| format!("Failed to write {}: {}", entry_name, e))?;
        writer.write_all(&contents)
            .map_err(|e| format!("Failed to write content: {}", e))?;
    }

    writer.finish()
        .map_err(|e| format!("Failed to finalize CBZ: {}", e))?;

    Ok(())
}

fn collect_images(dir: &Path, extensions: &[&str], files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_images(&path, extensions, files)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if extensions.contains(&ext.to_lowercase().as_str()) {
                files.push(path);
            }
        }
    }

    Ok(())
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
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            pending_file: Mutex::new(None),
            frontend_ready: Mutex::new(false),
        })
        .setup(|_app| {
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_cbz, save_cbz, convert_to_cbz, frontend_ready])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            if let tauri::RunEvent::Opened { urls } = &event {
                use tauri::{Emitter, Manager};
                for url in urls {
                    if url.scheme() == "file" {
                        if let Ok(path) = url.to_file_path() {
                            let path_str = path.to_string_lossy().to_string();
                            let state = app_handle.state::<AppState>();
                            // Hold the frontend_ready lock across the check-and-set so we
                            // can't race frontend_ready(): otherwise the frontend could
                            // flip to ready (and take an empty pending_file) between our
                            // check and our store, dropping the file silently.
                            let ready = state.frontend_ready.lock().unwrap();
                            if *ready {
                                drop(ready);
                                let _ = app_handle.emit("open-file", path_str);
                            } else {
                                *state.pending_file.lock().unwrap() = Some(path_str);
                            }
                        }
                    }
                }
            }
            let _ = event;
        });
}
