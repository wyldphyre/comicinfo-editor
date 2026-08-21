// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Release builds are linked as a GUI subsystem app (see the attribute above),
/// which starts with no console attached — so every println!/eprintln! in CLI
/// mode went nowhere, including --help. Reattach to the console that launched
/// us so batch mode is actually usable from a terminal.
#[cfg(windows)]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    // Fails harmlessly when there is no parent console (e.g. launched from
    // Explorer), in which case there is nothing to attach to anyway.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if comicinfo_editor_lib::cli::is_cli_mode(&args) {
        #[cfg(windows)]
        attach_parent_console();
        std::process::exit(comicinfo_editor_lib::cli::run(args));
    }
    comicinfo_editor_lib::run();
}
