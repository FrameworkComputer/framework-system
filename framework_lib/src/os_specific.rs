//! Helper functions that need OS/platform specific implementations

#[cfg(not(feature = "uefi"))]
use std::{thread, time};

#[cfg(feature = "uefi")]
use alloc::string::{String, ToString};

// Could report the implemented UEFI spec version
// But that's not very useful. Just look at the BIOS version
// But at least it's useful to see that the tool was run on UEFI
#[cfg(feature = "uefi")]
pub fn get_os_version() -> String {
    "UEFI".to_string()
}

#[cfg(target_family = "windows")]
pub fn get_os_version() -> String {
    let ver = windows_version::OsVersion::current();
    format!("{}.{}.{}.{}", ver.major, ver.minor, ver.pack, ver.build)
}

#[cfg(target_family = "unix")]
pub fn get_os_version() -> String {
    if let Ok(uts) = nix::sys::utsname::uname() {
        // uname -a without hostname
        format!(
            "{} {} {} {}",
            uts.sysname().to_string_lossy(),
            uts.release().to_string_lossy(),
            uts.version().to_string_lossy(),
            uts.machine().to_string_lossy(),
        )
    } else {
        "Unknown".to_string()
    }
}

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
