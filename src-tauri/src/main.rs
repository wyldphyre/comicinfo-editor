// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if comicinfo_editor_lib::cli::is_cli_mode(&args) {
        std::process::exit(comicinfo_editor_lib::cli::run(args));
    }
    comicinfo_editor_lib::run();
}
