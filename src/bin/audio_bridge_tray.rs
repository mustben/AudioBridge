#![windows_subsystem = "windows"]
use audio_bridge::{app, protocol};

// Keep the executable's worker contract checked against the shared library.
const _: fn() -> Result<protocol::BackendHandle, String> = app::spawn_worker;
#[path = "../tray.rs"]
mod tray;
fn main() {
    tray::run();
}
