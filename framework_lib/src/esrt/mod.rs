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
use std::slice;
#[cfg(feature = "uefi")]
use uefi::{guid, Guid};

#[cfg(feature = "linux")]
use std::fs;
#[cfg(feature = "linux")]
use std::io;
#[cfg(feature = "linux")]
use std::path::Path;

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

pub const BIOS_GUID: Guid = guid!("a30a8cf3-847f-5e59-bd59-f9ec145c1a8c");
pub const RETIMER01_GUID: Guid = guid!("a9c91b0c-c0b8-463d-a7da-a5d6ec646333");
pub const RETIMER23_GUID: Guid = guid!("ba2e4e6e-3b0c-4f25-8a59-4c553fc86ea2");
pub const GEN13_RETIMER01_GUID: Guid = guid!("0c42b824-818f-428f-8687-5efcaf059bea");
pub const GEN13_RETIMER23_GUID: Guid = guid!("268ccbde-e087-420b-bf82-2212bd3f9bfc");
// TODO AMD13 BIOS
pub const FL16_BIOS_GUID: Guid = guid!("4496aebc-2421-5dfb-9e75-03ec44245994");
pub const AMD13_BIOS_GUID: Guid = guid!("11111111-1111-1111-1111-111111111111");

// In EDK2
// Handled by MdeModulePkg/Library/DxeCapsuleLibFmp/DxeCapsuleLib.c
// Defined by MdePkg/Include/IndustryStandard/WindowsUxCapsule.h
/// gWindowsUxCapsuleGuid from MdePkg/MdePkg.dec
pub const WINUX_GUID: Guid = guid!("3b8c8162-188c-46a4-aec9-be43f1d65697");

#[derive(Debug)]
pub enum FrameworkGuidKind {
    Bios,
    Retimer01,
    Retimer23,
    Gen13Retimer01,
    Gen13Retimer23,
    Fl16Bios,
    Amd13Bios,
    WinUx,
    Unknown,
}

pub fn match_guid_kind(guid: &Guid) -> FrameworkGuidKind {
    match *guid {
        BIOS_GUID => FrameworkGuidKind::Bios,
        RETIMER01_GUID => FrameworkGuidKind::Retimer01,
        RETIMER23_GUID => FrameworkGuidKind::Retimer23,
        GEN13_RETIMER01_GUID => FrameworkGuidKind::Gen13Retimer01,
        GEN13_RETIMER23_GUID => FrameworkGuidKind::Gen13Retimer23,
        FL16_BIOS_GUID => FrameworkGuidKind::Fl16Bios,
        AMD13_BIOS_GUID => FrameworkGuidKind::Amd13Bios,
        WINUX_GUID => FrameworkGuidKind::WinUx,
        _ => FrameworkGuidKind::Unknown,
    }
}

#[repr(packed)]
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

#[cfg(all(not(feature = "uefi"), feature = "std", feature = "linux"))]
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

#[cfg(all(not(feature = "uefi"), feature = "linux"))]
pub fn get_esrt() -> Option<Esrt> {
    let res = esrt_from_sysfs(Path::new("/sys/firmware/efi/esrt/entries")).ok();
    if res.is_none() {
        error!("Make sure you're root to access ESRT from sysfs on Linux");
    }
    res
}

#[cfg(all(not(feature = "uefi"), feature = "windows"))]
pub fn get_esrt() -> Option<Esrt> {
    // TODO: Implement
    error!("Reading ESRT is not implemented on Windows yet.");
    None
}

#[cfg(all(not(feature = "uefi"), target_os = "freebsd"))]
pub fn get_esrt() -> Option<Esrt> {
    // TODO: Implement
    println!("Reading ESRT is not implemented on FreeBSD yet.");
    None
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
                let raw_esrt = &*(table.address as *const _Esrt);
                let mut esrt = Esrt {
                    resource_count: raw_esrt.resource_count,
                    resource_count_max: raw_esrt.resource_count_max,
                    resource_version: raw_esrt.resource_version,
                    entries: vec![],
                };

                // Make sure it's the version we expect
                debug_assert!(esrt.resource_version == ESRT_FIRMWARE_RESOURCE_VERSION);

                let src_ptr = std::ptr::addr_of!(raw_esrt.entries) as *const EsrtResourceEntry;
                let slice_entries = slice::from_raw_parts(src_ptr, esrt.resource_count as usize);

                esrt.entries = slice_entries.to_vec();

                return Some(esrt);
            },
            _ => {}
        }
    }
    None
}
