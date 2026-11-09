//! The program that opens the easime settings window (`easime_settings_windows`).

// No console window behind the app, except in debug builds (their output is
// what the CI reads).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    easime_settings_windows::app::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("the easime settings app for Windows runs on Windows only");
    std::process::exit(1);
}
