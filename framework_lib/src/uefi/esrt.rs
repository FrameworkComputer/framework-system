use core::prelude::v1::derive;
use std::slice;
use std::uefi;
use std::uefi::guid::GuidKind;
use uefi::guid::Guid;

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
const ESRT_FIRMWARE_RESOURCE_VERSION: u64 = 1;

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
