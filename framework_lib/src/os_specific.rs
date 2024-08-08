//! Helper functions that need OS/platform specific implementations

#[cfg(not(feature = "uefi"))]
use std::{thread, time};

/// Sleep a number of microseconds
pub fn sleep(micros: u64) {
    #[cfg(not(feature = "uefi"))]
    {
        let duration = time::Duration::from_micros(micros);
        thread::sleep(duration);
    }
    #[cfg(feature = "uefi")]
    {
        // TODO: It's not recommended to use this for sleep more than 10ms
        // Should use a one-shot timer event
        // TODO: Log if no table
        let st = uefi::table::system_table_boot().unwrap();
        let bs = st.boot_services();
        bs.stall(micros as usize);
    }
}
