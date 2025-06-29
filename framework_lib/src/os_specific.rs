//! Helper functions that need OS/platform specific implementations

use core::time::Duration;
#[cfg(not(feature = "uefi"))]
use std::thread;

/// Sleep a number of microseconds
pub fn sleep(micros: u64) {
    let duration = Duration::from_micros(micros);
    #[cfg(not(feature = "uefi"))]
    {
        thread::sleep(duration);
    }
    #[cfg(feature = "uefi")]
    {
        // TODO: It's not recommended to use this for sleep more than 10ms
        // Should use a one-shot timer event
        uefi::boot::stall(duration);
    }
}
