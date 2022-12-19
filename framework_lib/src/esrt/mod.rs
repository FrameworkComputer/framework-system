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

use core::fmt;
use core::prelude::v1::derive;
#[cfg(feature = "uefi")]
use std::slice;
#[cfg(feature = "uefi")]
use std::uefi::guid::GuidKind;

#[cfg(feature = "linux")]
use std::fs;
#[cfg(feature = "linux")]
use std::io;
#[cfg(feature = "linux")]
use std::path::Path;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct Guid(pub u32, pub u16, pub u16, pub [u8; 8]);
impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:>08x}, {:>04x}, {:>04x}, [", self.0, self.1, self.2)?;
        for (i, b) in self.3.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{:>02x}", b)?;
        }
        write!(f, "])")?;
        Ok(())
    }
}

/// Decode from GUID string version
/// Example: a9c91b0c-c0b8-463d-a7da-a5d6ec646333
/// Result: (a30a8cf3, 847f, 5e59, \[bd,59,f9,ec,14,5c,1a,8c\])
/// TODO: Could add a test for this
pub fn guid_from_str(string: &str) -> Option<Guid> {
    let sections: Vec<&str> = string.split('-').collect();
    let first = u32::from_str_radix(sections[0], 16).ok()?;
    let second = u16::from_str_radix(sections[1], 16).ok()?;
    let third = u16::from_str_radix(sections[2], 16).ok()?;

    let fourth = {
        [
            u8::from_str_radix(&sections[3][0..2], 16).ok()?,
            u8::from_str_radix(&sections[3][2..4], 16).ok()?,
            u8::from_str_radix(&sections[4][0..2], 16).ok()?,
            u8::from_str_radix(&sections[4][2..4], 16).ok()?,
            u8::from_str_radix(&sections[4][4..6], 16).ok()?,
            u8::from_str_radix(&sections[4][6..8], 16).ok()?,
            u8::from_str_radix(&sections[4][8..10], 16).ok()?,
            u8::from_str_radix(&sections[4][10..12], 16).ok()?,
        ]
    };

    Some(Guid(first, second, third, fourth))
}

pub const BIOS_GUID: Guid = Guid(
    0xa30a8cf3,
    0x847f,
    0x5e59,
    [0xbd, 0x59, 0xf9, 0xec, 0x14, 0x5c, 0x1a, 0x8c],
);
pub const RETIMER01_GUID: Guid = Guid(
    0xa9c91b0c,
    0xc0b8,
    0x463d,
    [0xa7, 0xda, 0xa5, 0xd6, 0xec, 0x64, 0x63, 0x33],
);
pub const RETIMER23_GUID: Guid = Guid(
    0xba2e4e6e,
    0x3b0c,
    0x4f25,
    [0x8a, 0x59, 0x4c, 0x55, 0x3f, 0xc8, 0x6e, 0xa2],
);
// In EDK2
// Handled by MdeModulePkg/Library/DxeCapsuleLibFmp/DxeCapsuleLib.c
// Defined by MdePkg/Include/IndustryStandard/WindowsUxCapsule.h
pub const WINUX_GUID: Guid = Guid(
    0x3b8c8162,
    0x188c,
    0x46a4,
    [0xae, 0xc9, 0xbe, 0x43, 0xf1, 0xd6, 0x56, 0x97],
);

#[derive(Debug)]
enum FrameworkGuidKind {
    Bios,
    Retimer01,
    Retimer23,
    WinUx,
    Unknown,
}

fn match_guid_kind(guid: &Guid) -> FrameworkGuidKind {
    match *guid {
        BIOS_GUID => FrameworkGuidKind::Bios,
        RETIMER01_GUID => FrameworkGuidKind::Retimer01,
        RETIMER23_GUID => FrameworkGuidKind::Retimer23,
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
        println!("Make sure you're root to access ESRT from sysfs on Linux");
    }
    res
}

#[cfg(all(not(feature = "uefi"), feature = "windows"))]
pub fn get_esrt() -> Option<Esrt> {
    // TODO: Implement
    println!("Reading ESRT is not implemented on Windows yet.");
    None
}

#[cfg(all(not(feature = "uefi"), target_os = "freebsd"))]
pub fn get_esrt() -> Option<Esrt> {
    // TODO: Implement
    println!("Reading ESRT is not implemented on FreeBSD yet.");
    None
}

#[cfg(feature = "uefi")]
pub fn get_esrt() -> Option<Esrt> {
    for table in std::system_table().config_tables() {
        match table.VendorGuid.kind() {
            GuidKind::SystemResource => unsafe {
                let raw_esrt = &*(table.VendorTable as *const _Esrt);
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
