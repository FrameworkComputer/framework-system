//! Helper functions that need OS/platform specific implementations

#[cfg(not(feature = "uefi"))]
use std::{thread, time};

/// Sleep a number of microseconds
pub fn sleep(micros: u64) {
    #[cfg(not(feature = "uefi"))]
    {
        let ten_millis = time::Duration::from_micros(micros);
        thread::sleep(ten_millis);
    }
    #[cfg(feature = "uefi")]
    {
        let uefi = std::system_table();
        let _ = (uefi.BootServices.Stall)(micros as usize);
    }
}
