//! Helper functions that need OS/platform specific implementations

#[cfg(not(feature = "uefi"))]
use std::thread;
#[cfg(not(feature = "uefi"))]
use core::time::Duration;

/// Sleep a number of microseconds
pub fn sleep(micros: u64) {
    #[cfg(not(feature = "uefi"))]
    {
        let duration = Duration::from_micros(micros);
        thread::sleep(duration);
    }
    #[cfg(feature = "uefi")]
    {
        // TODO: It's not recommended to use this for sleep more than 10ms
        // Should use a one-shot timer event
        uefi::boot::stall(micros as usize);
    }
}
