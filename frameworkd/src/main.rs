// In debug builds don't disable terminal
// Otherwise we can't see println and can't exit with CTRL+C
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod tray;

#[cfg(windows)]
fn main() {
    tray::main();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("frameworkd is currently only supported on Windows");
    std::process::exit(1);
}
