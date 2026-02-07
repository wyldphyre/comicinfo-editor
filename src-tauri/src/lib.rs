mod comicinfo;

use comicinfo::ComicInfo;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use zip::read::ZipArchive;
use zip::write::ZipWriter;
use zip::write::SimpleFileOptions;
use base64::{engine::general_purpose::STANDARD, Engine};

#[tauri::command]
fn open_cbz(path: String) -> Result<ComicInfo, String> {
    let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = file.name().to_lowercase();

        if name == "comicinfo.xml" {
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .map_err(|e| format!("Failed to read ComicInfo.xml: {}", e))?;
            return ComicInfo::from_xml(&contents);
        }
    }

    Ok(ComicInfo::default())
}

#[tauri::command]
fn save_cbz(path: String, comic_info: ComicInfo) -> Result<(), String> {
    let mut comic_info = comic_info;

    let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    // Auto-populate PageCount if not provided
    if comic_info.page_count.is_none() {
        let image_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];
        let mut count = 0;
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index_raw(i) {
                let name = entry.name().to_lowercase();
                if let Some(ext) = Path::new(&name).extension() {
                    if image_extensions.contains(&ext.to_str().unwrap_or("")) {
                        count += 1;
                    }
                }
            }
        }
        comic_info.page_count = Some(count);
    }

    let xml_content = comic_info.to_xml()?;

    let temp_path = format!("{}.tmp", path);
    let temp_file = File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {}", e))?;
    let mut writer = ZipWriter::new(temp_file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    let mut comicinfo_exists = false;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Failed to read archive entry: {}", e))?;
        let name = file.name().to_string();

        if name.to_lowercase() == "comicinfo.xml" {
            comicinfo_exists = true;
            writer.start_file(&name, options).map_err(|e| format!("Failed to write entry: {}", e))?;
            writer.write_all(xml_content.as_bytes()).map_err(|e| format!("Failed to write content: {}", e))?;
        } else {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents).map_err(|e| format!("Failed to read entry: {}", e))?;

            writer.start_file(&name, options).map_err(|e| format!("Failed to write entry: {}", e))?;
            writer.write_all(&contents).map_err(|e| format!("Failed to write content: {}", e))?;
        }
    }

    if !comicinfo_exists {
        writer.start_file("ComicInfo.xml", options).map_err(|e| format!("Failed to write ComicInfo.xml: {}", e))?;
        writer.write_all(xml_content.as_bytes()).map_err(|e| format!("Failed to write content: {}", e))?;
    }

    writer.finish().map_err(|e| format!("Failed to finalize archive: {}", e))?;

    std::fs::rename(&temp_path, &path).map_err(|e| format!("Failed to replace original file: {}", e))?;

    Ok(())
}

#[tauri::command]
fn get_page_count(path: String) -> Result<i32, String> {
    let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    let image_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];
    let mut count = 0;

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index_raw(i) {
            let name = file.name().to_lowercase();
            if let Some(ext) = Path::new(&name).extension() {
                if image_extensions.contains(&ext.to_str().unwrap_or("")) {
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

#[tauri::command]
fn extract_cover(path: String) -> Result<String, String> {
    let file = File::open(&path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read archive: {}", e))?;

    let image_extensions = ["jpg", "jpeg", "png", "gif", "webp", "bmp"];
    let mut images: Vec<(String, usize)> = Vec::new();

    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index_raw(i) {
            let name = file.name().to_string();
            let name_lower = name.to_lowercase();
            if let Some(ext) = Path::new(&name_lower).extension() {
                if image_extensions.contains(&ext.to_str().unwrap_or("")) {
                    images.push((name, i));
                }
            }
        }
    }

    images.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some((_, index)) = images.first() {
        let mut file = archive.by_index(*index).map_err(|e| format!("Failed to read image: {}", e))?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents).map_err(|e| format!("Failed to read image data: {}", e))?;

        let name = file.name().to_lowercase();
        let mime_type = if name.ends_with(".png") {
            "image/png"
        } else if name.ends_with(".gif") {
            "image/gif"
        } else if name.ends_with(".webp") {
            "image/webp"
        } else {
            "image/jpeg"
        };

        let base64_data = STANDARD.encode(&contents);
        return Ok(format!("data:{};base64,{}", mime_type, base64_data));
    }

    Err("No images found in archive".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![open_cbz, save_cbz, get_page_count, extract_cover])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
