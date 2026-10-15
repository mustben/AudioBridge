//! Standalone read-only Core Audio diagnostic; does not connect Bluetooth.
use audio_bridge::audio_diagnostics;
fn main() -> windows::core::Result<()> {
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};
    // SAFETY: balanced MTA initialization owned by this thread, including on errors.
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    for line in audio_diagnostics::collect()? {
        println!("{line}");
    }
    Ok(())
}
