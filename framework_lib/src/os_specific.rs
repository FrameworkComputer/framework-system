//! Helper functions that need OS/platform specific implementations

use core::time::Duration;
#[cfg(not(feature = "uefi"))]
use std::thread;

#[cfg(feature = "uefi")]
use alloc::string::{String, ToString};
#[cfg(feature = "uefi")]
use alloc::vec::Vec;

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

#[cfg(windows)]
use windows::{core::*, Win32::Foundation::*, Win32::System::WindowsProgramming::*};

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

/// Look up the GUID for well-known UEFI variable names
pub fn known_uefi_guid(name: &str) -> Option<&'static str> {
    match name {
        // EFI_IMAGE_SECURITY_DATABASE_GUID
        "db" | "dbx" | "dbt" | "dbr" => Some("d719b2cb-3d3a-4596-a3bc-dad00e67656f"),
        // EFI_GLOBAL_VARIABLE
        "PK" | "KEK" | "SecureBoot" | "SetupMode" | "BootOrder" | "BootCurrent" | "Timeout" => {
            Some("8be4df61-93ca-11d2-aa0d-00e098032b8c")
        }
        // Insyde Security
        "SecureFlashInfo" | "SecureFlashSetupMode" | "SecureFlashCertData" => {
            Some("382af2bb-ffff-abcd-aaee-cce099338877")
        }
        _ => None,
    }
}

pub const EFI_VARIABLE_NON_VOLATILE: u32 = 0x00000001;
pub const EFI_VARIABLE_BOOTSERVICE_ACCESS: u32 = 0x00000002;
pub const EFI_VARIABLE_RUNTIME_ACCESS: u32 = 0x00000004;

#[cfg(windows)]
pub fn get_uefi_var(name: &str, guid: &str) -> Option<(Vec<u8>, u32)> {
    let mut data = [0u8; 65536];
    let mut attributes: u32 = 0;
    let res = unsafe {
        GetFirmwareEnvironmentVariableExW(
            &HSTRING::from(name),
            &HSTRING::from(format!("{{{guid}}}")),
            Some(data.as_mut_ptr() as *mut core::ffi::c_void),
            data.len() as u32,
            Some(&mut attributes),
        )
    };
    if res == 0 {
        let error = unsafe { GetLastError() };
        error!("GetFirmwareEnvironmentVariableExW failed: {:?}", error);
        return None;
    }
    Some((data[..res as usize].to_vec(), attributes))
}

#[cfg(windows)]
pub fn set_uefi_var(name: &str, guid: &str, value: &[u8], attributes: u32) -> Option<()> {
    let res = unsafe {
        SetFirmwareEnvironmentVariableExW(
            &HSTRING::from(name),
            &HSTRING::from(format!("{{{guid}}}")),
            Some(value.as_ptr() as *const core::ffi::c_void),
            value.len() as u32,
            attributes,
        )
    };
    if let Err(e) = res {
        error!("SetFirmwareEnvironmentVariableExW failed: {:?}", e);
        return None;
    }
    Some(())
}

#[cfg(target_os = "linux")]
pub fn get_uefi_var(name: &str, guid: &str) -> Option<(Vec<u8>, u32)> {
    let path = format!("/sys/firmware/efi/efivars/{}-{}", name, guid);
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to read efivar {}: {:?}", path, e);
            return None;
        }
    };
    if data.len() < 4 {
        error!("efivar file too short: {} bytes", data.len());
        return None;
    }
    let attributes = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    Some((data[4..].to_vec(), attributes))
}

#[cfg(target_os = "linux")]
pub fn set_uefi_var(name: &str, guid: &str, value: &[u8], attributes: u32) -> Option<()> {
    use std::io::Write;
    let path = format!("/sys/firmware/efi/efivars/{}-{}", name, guid);

    // Clear the immutable flag if the file already exists
    if std::path::Path::new(&path).exists() {
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open efivar {}: {:?}", path, e);
                return None;
            }
        };
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let mut flags: libc::c_long = 0;
        // FS_IOC_GETFLAGS
        unsafe {
            if libc::ioctl(fd, 0x80086601, &mut flags) != 0 {
                error!("FS_IOC_GETFLAGS failed on {}", path);
                return None;
            }
        }
        // Clear FS_IMMUTABLE_FL (0x00000010)
        flags &= !(0x00000010 as libc::c_long);
        unsafe {
            if libc::ioctl(fd, 0x40086602, &flags) != 0 {
                error!("FS_IOC_SETFLAGS failed on {}", path);
                return None;
            }
        }
    }

    // Write attributes (4 bytes LE) + data
    let mut file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create efivar {}: {:?}", path, e);
            return None;
        }
    };
    let mut buf = Vec::with_capacity(4 + value.len());
    buf.extend_from_slice(&attributes.to_le_bytes());
    buf.extend_from_slice(value);
    if let Err(e) = file.write_all(&buf) {
        error!("Failed to write efivar {}: {:?}", path, e);
        return None;
    }
    Some(())
}

#[cfg(target_os = "freebsd")]
pub fn get_uefi_var(_name: &str, _guid: &str) -> Option<(Vec<u8>, u32)> {
    error!("Getting UEFI variable not yet supported on FreeBSD");
    None
}

#[cfg(target_os = "freebsd")]
pub fn set_uefi_var(_name: &str, _guid: &str, _value: &[u8], _attributes: u32) -> Option<()> {
    error!("Setting UEFI variable not yet supported on FreeBSD");
    None
}

#[cfg(feature = "uefi")]
pub fn get_uefi_var(_name: &str, _guid: &str) -> Option<(Vec<u8>, u32)> {
    error!("Getting UEFI variable not yet supported in UEFI shell");
    None
}

#[cfg(feature = "uefi")]
pub fn set_uefi_var(_name: &str, _guid: &str, _value: &[u8], _attributes: u32) -> Option<()> {
    error!("Setting UEFI variable not yet supported in UEFI shell");
    None
}

#[cfg(target_os = "linux")]
pub fn list_uefi_vars() -> Option<Vec<(String, String)>> {
    let entries = match std::fs::read_dir("/sys/firmware/efi/efivars/") {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read /sys/firmware/efi/efivars/: {:?}", e);
            return None;
        }
    };
    let mut vars = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let filename = entry.file_name().to_string_lossy().to_string();
        // Format: {name}-{8-4-4-4-12 guid}  (guid is always 36 chars)
        if filename.len() > 37 {
            let split_pos = filename.len() - 37;
            if filename.as_bytes()[split_pos] == b'-' {
                let name = filename[..split_pos].to_string();
                let guid = filename[split_pos + 1..].to_string();
                vars.push((name, guid));
            }
        }
    }
    vars.sort();
    Some(vars)
}

#[cfg(windows)]
pub fn list_uefi_vars() -> Option<Vec<(String, String)>> {
    error!("Listing UEFI variables not yet supported on Windows");
    None
}

#[cfg(target_os = "freebsd")]
pub fn list_uefi_vars() -> Option<Vec<(String, String)>> {
    error!("Listing UEFI variables not yet supported on FreeBSD");
    None
}

#[cfg(feature = "uefi")]
pub fn list_uefi_vars() -> Option<Vec<(String, String)>> {
    error!("Listing UEFI variables not yet supported in UEFI shell");
    None
}
