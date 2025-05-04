//! Get the ESRT UEFI table and extract the version information.
//!
//! Currently only implemented on Linux and UEFI.
//! I haven't found how to get it on Windows.
//!
//! ESRT (EFI System Resource Table) holds information about updateable firmware
//! components in the system. It includes the current version, as well as if
//! and how they can be updated via a UEFI capsule. Windows and LVFS take advantage
//! of this information.
//!
//! Not all firmware components are reported here.

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use std::prelude::v1::*;

#[cfg(not(feature = "uefi"))]
use crate::guid::Guid;
use core::prelude::v1::derive;
#[cfg(not(feature = "uefi"))]
use guid_macros::guid;
#[cfg(feature = "uefi")]
use uefi::{guid, Guid};

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::path::Path;

#[cfg(target_os = "freebsd")]
use nix::ioctl_readwrite;
#[cfg(target_os = "freebsd")]
use std::fs::OpenOptions;
#[cfg(target_os = "freebsd")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "freebsd")]
use std::os::unix::fs::OpenOptionsExt;

/// Decode from GUID string version
///
/// # Examples
/// ```
/// use framework_lib::esrt::*;
/// use framework_lib::guid::*;
///
/// let valid_guid = Guid::from_values(0xA9C91B0C, 0xC0B8, 0x463D, 0xA7DA, 0xA5D6EC646333);
/// // Works with lower-case
/// let guid = guid_from_str("a9c91b0c-c0b8-463d-a7da-a5d6ec646333");
/// assert_eq!(guid, Some(valid_guid));
/// // And upper-case
/// let guid = guid_from_str("A9C91B0C-C0B8-463D-A7DA-A5D6EC646333");
/// assert_eq!(guid, Some(valid_guid));
///
/// let guid = guid_from_str("invalid-guid");
/// assert_eq!(guid, None);
/// ```
pub fn guid_from_str(string: &str) -> Option<Guid> {
    let string = string.strip_suffix('\n').unwrap_or(string);
    let sections: Vec<&str> = string.split('-').collect();
    let time_low = u32::from_str_radix(sections[0], 16).ok()?;
    let time_mid = u16::from_str_radix(sections[1], 16).ok()?;
    let time_high_and_version = u16::from_str_radix(sections[2], 16).ok()?;
    let clock_seq_and_variant = u16::from_str_radix(sections[3], 16).ok()?;
    let node = u64::from_str_radix(sections[4], 16).ok()?;

    Some(Guid::from_values(
        time_low,
        time_mid,
        time_high_and_version,
        clock_seq_and_variant,
        node,
    ))
}

pub const TGL_BIOS_GUID: Guid = guid!("b3bdb2e4-c5cb-5c1b-bdc3-e6fc132462ff");
pub const ADL_BIOS_GUID: Guid = guid!("a30a8cf3-847f-5e59-bd59-f9ec145c1a8c");
pub const RPL_BIOS_GUID: Guid = guid!("13fd4ed2-cba9-50ba-bb91-aece0acb4cc3");
pub const MTL_BIOS_GUID: Guid = guid!("72cecb9b-2b37-5ec2-a9ff-c739aabaadf3");

pub const TGL_RETIMER01_GUID: Guid = guid!("832af090-2ef9-7c47-8f6d-b405c8c7f156");
pub const TGL_RETIMER23_GUID: Guid = guid!("20ef4108-6c64-d049-b6de-11ee35980b8f");
pub const ADL_RETIMER01_GUID: Guid = guid!("a9c91b0c-c0b8-463d-a7da-a5d6ec646333");
pub const ADL_RETIMER23_GUID: Guid = guid!("ba2e4e6e-3b0c-4f25-8a59-4c553fc86ea2");
pub const RPL_RETIMER01_GUID: Guid = guid!("0c42b824-818f-428f-8687-5efcaf059bea");
pub const RPL_RETIMER23_GUID: Guid = guid!("268ccbde-e087-420b-bf82-2212bd3f9bfc");
pub const MTL_RETIMER01_GUID: Guid = guid!("c57fd615-2ac9-4154-bf34-4dc715344408");
pub const MTL_RETIMER23_GUID: Guid = guid!("bdffce36-809c-4fa6-aecc-54536922f0e0");

pub const FL16_BIOS_GUID: Guid = guid!("6ae76af1-c002-5d64-8e18-658d205acf34");
pub const AMD13_BIOS_GUID: Guid = guid!("b5f7dcc1-568c-50f8-a4dd-e39d1f93fda1");
pub const RPL_CSME_GUID: Guid = guid!("865d322c-6ac7-4734-b43e-55db5a557d63");
pub const MTL_CSME_GUID: Guid = guid!("32d8d677-eebc-4947-8f8a-0693a45240e5");

// In EDK2
// Handled by MdeModulePkg/Library/DxeCapsuleLibFmp/DxeCapsuleLib.c
// Defined by MdePkg/Include/IndustryStandard/WindowsUxCapsule.h
/// gWindowsUxCapsuleGuid from MdePkg/MdePkg.dec
pub const WINUX_GUID: Guid = guid!("3b8c8162-188c-46a4-aec9-be43f1d65697");

#[derive(Debug)]
pub enum FrameworkGuidKind {
    TglBios,
    AdlBios,
    RplBios,
    MtlBios,
    TglRetimer01,
    TglRetimer23,
    AdlRetimer01,
    AdlRetimer23,
    RplRetimer01,
    RplRetimer23,
    MtlRetimer01,
    MtlRetimer23,
    RplCsme,
    MtlCsme,
    Fl16Bios,
    Amd13Bios,
    WinUx,
    Unknown,
}

pub fn match_guid_kind(guid: &Guid) -> FrameworkGuidKind {
    match *guid {
        TGL_BIOS_GUID => FrameworkGuidKind::TglBios,
        ADL_BIOS_GUID => FrameworkGuidKind::AdlBios,
        RPL_BIOS_GUID => FrameworkGuidKind::RplBios,
        MTL_BIOS_GUID => FrameworkGuidKind::MtlBios,
        FL16_BIOS_GUID => FrameworkGuidKind::Fl16Bios,
        AMD13_BIOS_GUID => FrameworkGuidKind::Amd13Bios,
        TGL_RETIMER01_GUID => FrameworkGuidKind::TglRetimer01,
        TGL_RETIMER23_GUID => FrameworkGuidKind::TglRetimer23,
        ADL_RETIMER01_GUID => FrameworkGuidKind::AdlRetimer01,
        ADL_RETIMER23_GUID => FrameworkGuidKind::AdlRetimer23,
        RPL_RETIMER01_GUID => FrameworkGuidKind::RplRetimer01,
        RPL_RETIMER23_GUID => FrameworkGuidKind::RplRetimer23,
        MTL_RETIMER01_GUID => FrameworkGuidKind::MtlRetimer01,
        MTL_RETIMER23_GUID => FrameworkGuidKind::MtlRetimer23,
        RPL_CSME_GUID => FrameworkGuidKind::RplCsme,
        MTL_CSME_GUID => FrameworkGuidKind::MtlCsme,
        WINUX_GUID => FrameworkGuidKind::WinUx,
        _ => FrameworkGuidKind::Unknown,
    }
}

#[repr(C, packed)]
struct _Esrt {
    resource_count: u32,
    resource_count_max: u32,
    resource_version: u64,
    entries: [EsrtResourceEntry; 0],
}

pub struct Esrt {
    pub resource_count: u32,
    pub resource_count_max: u32,
    pub resource_version: u64,
    pub entries: Vec<EsrtResourceEntry>,
}

// Current Entry Version
pub const ESRT_FIRMWARE_RESOURCE_VERSION: u64 = 1;

#[derive(Debug)]
pub enum ResourceType {
    Unknown = 0,
    SystemFirmware = 1,
    DeviceFirmware = 2,
    UefiDriver = 3,
    Fmp = 4,
    DellTpmFirmware = 5,
}

impl ResourceType {
    fn from_int(i: u32) -> Self {
        match i {
            1 => Self::SystemFirmware,
            2 => Self::DeviceFirmware,
            3 => Self::UefiDriver,
            4 => Self::Fmp,
            5 => Self::DellTpmFirmware,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug)]
pub enum UpdateStatus {
    Success = 0x00,
    Unsuccessful = 0x01,
    InsufficientResources = 0x02,
    IncorrectVersion = 0x03,
    InvalidFormat = 0x04,
    AuthError = 0x05,
    PowerEventAc = 0x06,
    PowerEventBattery = 0x07,
    Reserved = 0xFF, // TODO: I added this, since there's no unknown type, is there?
}
impl UpdateStatus {
    fn from_int(i: u32) -> Self {
        match i {
            0 => Self::Success,
            1 => Self::Unsuccessful,
            2 => Self::InsufficientResources,
            3 => Self::IncorrectVersion,
            4 => Self::InvalidFormat,
            5 => Self::AuthError,
            6 => Self::PowerEventAc,
            7 => Self::PowerEventBattery,
            _ => Self::Reserved,
        }
    }
}

// TODO: Decode into proper Rust types
#[derive(Clone)]
pub struct EsrtResourceEntry {
    pub fw_class: Guid,
    pub fw_type: u32, // ResourceType
    pub fw_version: u32,
    pub lowest_supported_fw_version: u32,
    pub capsule_flags: u32,
    pub last_attempt_version: u32, // UpdateStatus
    pub last_attempt_status: u32,
}

pub fn print_esrt(esrt: &Esrt) {
    println!("ESRT Table");
    println!("  ResourceCount:        {}", esrt.resource_count);
    println!("  ResourceCountMax:     {}", esrt.resource_count_max);
    println!("  ResourceVersion:      {}", esrt.resource_version);

    for (i, entry) in esrt.entries.iter().enumerate() {
        println!("ESRT Entry {}", i);
        println!("  GUID:                 {}", entry.fw_class);
        println!(
            "  GUID:                 {:?}",
            match_guid_kind(&entry.fw_class)
        );
        println!(
            "  Type:                 {:?}",
            ResourceType::from_int(entry.fw_type)
        );
        println!(
            "  Version:              0x{:X} ({})",
            entry.fw_version, entry.fw_version
        );
        println!(
            "  Min FW Version:       0x{:X} ({})",
            entry.lowest_supported_fw_version, entry.lowest_supported_fw_version
        );
        println!("  Capsule Flags:        0x{:X}", entry.capsule_flags);
        println!(
            "  Last Attempt Version: 0x{:X} ({})",
            entry.last_attempt_version, entry.last_attempt_version
        );
        println!(
            "  Last Attempt Status:  {:?}",
            UpdateStatus::from_int(entry.last_attempt_status)
        );
    }
}

#[cfg(target_os = "linux")]
/// On Linux read the ESRT table from the sysfs
/// resource_version and resource_count_max are reported by sysfs, so they're defaulted to reaesonable values
/// capsule_flags in sysfs seems to be 0 always. Not sure why.
fn esrt_from_sysfs(dir: &Path) -> io::Result<Esrt> {
    let mut esrt_table = Esrt {
        resource_count: 0,
        resource_count_max: 0,
        resource_version: ESRT_FIRMWARE_RESOURCE_VERSION,
        entries: vec![],
    };
    if dir.is_dir() {
        for esrt_entry in fs::read_dir(dir)? {
            let esrt_entry = esrt_entry?;
            let path = esrt_entry.path();
            if path.is_dir() {
                let fw_class = fs::read_to_string(path.join("fw_class"))?;
                let fw_type = fs::read_to_string(path.join("fw_type"))?;
                let fw_version = fs::read_to_string(path.join("fw_version"))?;
                let lowest_supported_fw_version =
                    fs::read_to_string(path.join("lowest_supported_fw_version"))?;
                let raw_capsule_flags = fs::read_to_string(path.join("capsule_flags"))?;
                let capsule_flags = raw_capsule_flags.trim_start_matches("0x");
                let last_attempt_version = fs::read_to_string(path.join("last_attempt_version"))?;
                let last_attempt_status = fs::read_to_string(path.join("last_attempt_status"))?;
                let esrt = EsrtResourceEntry {
                    // TODO: Parse GUID
                    fw_class: guid_from_str(&fw_class).expect("Kernel provided wrong value"),
                    fw_type: fw_type
                        .trim()
                        .parse::<u32>()
                        .expect("Kernel provided wrong value"),
                    fw_version: fw_version
                        .trim()
                        .parse::<u32>()
                        .expect("Kernel provided wrong value"),
                    lowest_supported_fw_version: lowest_supported_fw_version
                        .trim()
                        .parse::<u32>()
                        .expect("Kernel provided wrong value"),
                    // TODO: Flags seem to be 0 always
                    capsule_flags: u32::from_str_radix(capsule_flags.trim(), 16)
                        .expect("Kernel provided wrong value"),
                    last_attempt_version: last_attempt_version
                        .trim()
                        .parse::<u32>()
                        .expect("Kernel provided wrong value"), // UpdateStatus
                    last_attempt_status: last_attempt_status
                        .trim()
                        .parse::<u32>()
                        .expect("Kernel provided wrong value"),
                };
                esrt_table.resource_count += 1;
                esrt_table.resource_count_max += 1;
                esrt_table.entries.push(esrt);
            }
        }
    }
    Ok(esrt_table)
}

#[cfg(target_os = "linux")]
pub fn get_esrt() -> Option<Esrt> {
    let res = esrt_from_sysfs(Path::new("/sys/firmware/efi/esrt/entries")).ok();
    if res.is_none() {
        error!("Make sure you're root to access ESRT from sysfs on Linux");
    }
    res
}

#[cfg(all(not(feature = "uefi"), windows))]
pub fn get_esrt() -> Option<Esrt> {
    let mut esrt_table = Esrt {
        resource_count: 0,
        resource_count_max: 0,
        resource_version: ESRT_FIRMWARE_RESOURCE_VERSION,
        entries: vec![],
    };
    use wmi::*;
    debug!("Opening WMI");
    let wmi_con = WMIConnection::new(COMLibrary::new().unwrap()).unwrap();
    use std::collections::HashMap;
    use wmi::Variant;
    debug!("Querying WMI");
    let results: Vec<HashMap<String, Variant>> = wmi_con.raw_query("SELECT HardwareID, Name FROM Win32_PnPEntity WHERE ClassGUID = '{f2e7dd72-6468-4e36-b6f1-6488f42c1b52}'").unwrap();

    let re = regex::Regex::new(r"([\-a-h0-9]+)\}&REV_([A-F0-9]+)").expect("Bad regex");
    for (i, val) in results.iter().enumerate() {
        let hwid = &val["HardwareID"];
        if let Variant::Array(strs) = hwid {
            if let Variant::String(s) = &strs[0] {
                // Sample "UEFI\\RES_{c57fd615-2ac9-4154-bf34-4dc715344408}&REV_CF"
                let caps = re.captures(s).expect("No caps");
                let guid_str = caps.get(1).unwrap().as_str().to_string();
                let ver_str = caps.get(2).unwrap().as_str().to_string();

                let guid = guid_from_str(&guid_str).unwrap();
                let guid_kind = match_guid_kind(&guid);
                let ver = u32::from_str_radix(&ver_str, 16).unwrap();
                debug!("ESRT Entry {}", i);
                debug!("  Name:    {:?}", guid_kind);
                debug!("  GUID:    {}", guid_str);
                debug!("  Version: {:X} ({})", ver, ver);

                let fw_type = if let Variant::String(name) = &val["Name"] {
                    match name.as_str() {
                        "System Firmware" => 1,
                        "Device Firmware" => 2,
                        _ => 0,
                    }
                } else {
                    0
                };

                // TODO: The missing fields are present in Device Manager
                // So there must be a way to get at them
                let esrt = EsrtResourceEntry {
                    fw_class: guid,
                    fw_type,
                    fw_version: ver,
                    // TODO: Not exposed by windows
                    lowest_supported_fw_version: 0,
                    // TODO: Not exposed by windows
                    capsule_flags: 0,
                    // TODO: Not exposed by windows
                    last_attempt_version: 0,
                    // TODO: Not exposed by windows
                    last_attempt_status: 0,
                };
                esrt_table.resource_count += 1;
                esrt_table.resource_count_max += 1;
                esrt_table.entries.push(esrt);
            } else {
                error!("Strs: {:#?}", strs[0]);
            }
        } else {
            error!("{:#?}", hwid);
        }
    }
    Some(esrt_table)
}

#[cfg(target_os = "freebsd")]
#[repr(C)]
pub struct EfiGetTableIoc {
    buf: *mut u8,
    uuid: [u8; 16],
    table_len: usize,
    buf_len: usize,
}
#[cfg(target_os = "freebsd")]
ioctl_readwrite!(efi_get_table, b'E', 1, EfiGetTableIoc);

#[cfg(all(not(feature = "uefi"), target_os = "freebsd"))]
pub fn get_esrt() -> Option<Esrt> {
    let path = "/dev/efi";
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .unwrap();

    let mut buf: Vec<u8> = Vec::new();
    let mut table = EfiGetTableIoc {
        buf: std::ptr::null_mut(),
        uuid: SYSTEM_RESOURCE_TABLE_GUID.to_bytes(),
        buf_len: 0,
        table_len: 0,
    };
    unsafe {
        let fd = file.as_raw_fd();
        if let Err(err) = efi_get_table(fd, &mut table) {
            error!("Failed to access ESRT at {}: {:?}", path, err);
            return None;
        }
        buf.resize(table.table_len, 0);
        table.buf_len = table.table_len;
        table.buf = buf.as_mut_ptr();

        let _res = efi_get_table(fd, &mut table).unwrap();
        esrt_from_buf(table.buf)
    }
}

/// gEfiSystemResourceTableGuid from MdePkg/MdePkg.dec
pub const SYSTEM_RESOURCE_TABLE_GUID: Guid = guid!("b122a263-3661-4f68-9929-78f8b0d62180");

#[cfg(feature = "uefi")]
pub fn get_esrt() -> Option<Esrt> {
    let st = unsafe { uefi_services::system_table().as_ref() };
    let config_tables = st.config_table();

    for table in config_tables {
        // TODO: Why aren't they the same type?
        //debug!("Table: {:?}", table);
        let table_guid: Guid = unsafe { std::mem::transmute(table.guid) };
        match table_guid {
            SYSTEM_RESOURCE_TABLE_GUID => unsafe {
                return esrt_from_buf(table.address as *const u8);
            },
            _ => {}
        }
    }
    None
}

/// Parse the ESRT table buffer
#[cfg(any(feature = "uefi", target_os = "freebsd"))]
unsafe fn esrt_from_buf(ptr: *const u8) -> Option<Esrt> {
    let raw_esrt = &*(ptr as *const _Esrt);
    let mut esrt = Esrt {
        resource_count: raw_esrt.resource_count,
        resource_count_max: raw_esrt.resource_count_max,
        resource_version: raw_esrt.resource_version,
        entries: vec![],
    };

    // Make sure it's the version we expect
    debug_assert!(esrt.resource_version == ESRT_FIRMWARE_RESOURCE_VERSION);

    let src_ptr = core::ptr::addr_of!(raw_esrt.entries) as *const EsrtResourceEntry;
    let slice_entries = core::slice::from_raw_parts(src_ptr, esrt.resource_count as usize);

    esrt.entries = slice_entries.to_vec();

    Some(esrt)
}
