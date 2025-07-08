//! Helper functions that need OS/platform specific implementations

#[cfg(not(feature = "uefi"))]
use std::{thread, time};

#[cfg(windows)]
use windows::{core::*, Win32::System::WindowsProgramming::*};

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
        let st = unsafe { uefi_services::system_table().as_ref() };
        let bs = st.boot_services();
        bs.stall(micros as usize);
    }
}

#[cfg(windows)]
pub fn set_dbx() -> Option<()> {
    set_uefi_var("dbx", "d719b2cb-3d3a-4596-a3bc-dad00e67656f", &[], 0)
}

#[cfg(windows)]
pub fn set_uefi_var(name: &str, guid: &str, value: &[u8], attributes: u32) -> Option<()> {
    unsafe {
        SetFirmwareEnvironmentVariableExW(
            // PCWSTR
            &HSTRING::from(name),
            // PCWSTR
            &HSTRING::from(guid),
            Some(value.as_ptr() as *const core::ffi::c_void),
            value.len() as u32,
            attributes,
        ).ok()
    }
}
