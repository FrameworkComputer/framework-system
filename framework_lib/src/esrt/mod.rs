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

use core::prelude::v1::derive;
use guid_create::{CGuid, GUID};

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

pub const TGL_BIOS_GUID: GUID = GUID::build_from_components(
    0xb3bdb2e4,
    0xc5cb,
    0x5c1b,
    &[0xbd, 0xc3, 0xe6, 0xfc, 0x13, 0x24, 0x62, 0xff],
);
pub const ADL_BIOS_GUID: GUID = GUID::build_from_components(
    0xa30a8cf3,
    0x847f,
    0x5e59,
    &[0xbd, 0x59, 0xf9, 0xec, 0x14, 0x5c, 0x1a, 0x8c],
);
pub const RPL_BIOS_GUID: GUID = GUID::build_from_components(
    0x13fd4ed2,
    0xcba9,
    0x50ba,
    &[0xbb, 0x91, 0xae, 0xce, 0x0a, 0xcb, 0x4c, 0xc3],
);
pub const MTL_BIOS_GUID: GUID = GUID::build_from_components(
    0x72cecb9b,
    0x2b37,
    0x5ec2,
    &[0xa9, 0xff, 0xc7, 0x39, 0xaa, 0xba, 0xad, 0xf3],
);
pub const FW12_RPL_BIOS_GUID: GUID = GUID::build_from_components(
    0x6bc0986c,
    0xd281,
    0x5ba3,
    &[0x96, 0x5c, 0x2f, 0x8d, 0x13, 0xe1, 0xee, 0xe8],
);

pub const TGL_RETIMER01_GUID: GUID = GUID::build_from_components(
    0x832af090,
    0x2ef9,
    0x7c47,
    &[0x8f, 0x6d, 0xb4, 0x05, 0xc8, 0xc7, 0xf1, 0x56],
);
pub const TGL_RETIMER23_GUID: GUID = GUID::build_from_components(
    0x20ef4108,
    0x6c64,
    0xd049,
    &[0xb6, 0xde, 0x11, 0xee, 0x35, 0x98, 0x0b, 0x8f],
);
pub const ADL_RETIMER01_GUID: GUID = GUID::build_from_components(
    0xa9c91b0c,
    0xc0b8,
    0x463d,
    &[0xa7, 0xda, 0xa5, 0xd6, 0xec, 0x64, 0x63, 0x33],
);
pub const ADL_RETIMER23_GUID: GUID = GUID::build_from_components(
    0xba2e4e6e,
    0x3b0c,
    0x4f25,
    &[0x8a, 0x59, 0x4c, 0x55, 0x3f, 0xc8, 0x6e, 0xa2],
);
pub const RPL_RETIMER01_GUID: GUID = GUID::build_from_components(
    0x0c42b824,
    0x818f,
    0x428f,
    &[0x86, 0x87, 0x5e, 0xfc, 0xaf, 0x05, 0x9b, 0xea],
);
pub const RPL_RETIMER23_GUID: GUID = GUID::build_from_components(
    0x268ccbde,
    0xe087,
    0x420b,
    &[0xbf, 0x82, 0x22, 0x12, 0xbd, 0x3f, 0x9b, 0xfc],
);
pub const MTL_RETIMER01_GUID: GUID = GUID::build_from_components(
    0xc57fd615,
    0x2ac9,
    0x4154,
    &[0xbf, 0x34, 0x4d, 0xc7, 0x15, 0x34, 0x44, 0x08],
);
pub const MTL_RETIMER23_GUID: GUID = GUID::build_from_components(
    0xbdffce36,
    0x809c,
    0x4fa6,
    &[0xae, 0xcc, 0x54, 0x53, 0x69, 0x22, 0xf0, 0xe0],
);

pub const FL16_BIOS_GUID: GUID = GUID::build_from_components(
    0x6ae76af1,
    0xc002,
    0x5d64,
    &[0x8e, 0x18, 0x65, 0x8d, 0x20, 0x5a, 0xcf, 0x34],
);
pub const AMD16_AI300_BIOS_GUID: GUID = GUID::build_from_components(
    0x820436ee,
    0x8208,
    0x463b,
    &[0x92, 0xb8, 0x82, 0x77, 0xd6, 0x38, 0x4d, 0x93],
);
pub const AMD13_RYZEN7040_BIOS_GUID: GUID = GUID::build_from_components(
    0xb5f7dcc1,
    0x568c,
    0x50f8,
    &[0xa4, 0xdd, 0xe3, 0x9d, 0x1f, 0x93, 0xfd, 0xa1],
);
pub const AMD13_AI300_BIOS_GUID: GUID = GUID::build_from_components(
    0x9c13b7f1,
    0xd618,
    0x5d68,
    &[0xbe, 0x61, 0x6b, 0x17, 0x88, 0x10, 0x14, 0xa7],
);
pub const DESKTOP_AMD_AI300_BIOS_GUID: GUID = GUID::build_from_components(
    0xeb68dbae,
    0x3aef,
    0x5077,
    &[0x92, 0xae, 0x90, 0x16, 0xd1, 0xf0, 0xc8, 0x56],
);
pub const RPL_CSME_GUID: GUID = GUID::build_from_components(
    0x865d322c,
    0x6ac7,
    0x4734,
    &[0xb4, 0x3e, 0x55, 0xdb, 0x5a, 0x55, 0x7d, 0x63],
);
pub const RPL_U_CSME_GUID: GUID = GUID::build_from_components(
    0x0f74c56d,
    0xd5ba,
    0x4942,
    &[0x96, 0xfa, 0xd3, 0x75, 0x60, 0xf4, 0x05, 0x54],
);
pub const MTL_CSME_GUID: GUID = GUID::build_from_components(
    0x32d8d677,
    0xeebc,
    0x4947,
    &[0x8f, 0x8a, 0x06, 0x93, 0xa4, 0x52, 0x40, 0xe5],
);

// In EDK2
// Handled by MdeModulePkg/Library/DxeCapsuleLibFmp/DxeCapsuleLib.c
// Defined by MdePkg/Include/IndustryStandard/WindowsUxCapsule.h
/// gWindowsUxCapsuleGuid from MdePkg/MdePkg.dec
pub const WINUX_GUID: GUID = GUID::build_from_components(
    0x3b8c8162,
    0x188c,
    0x46a4,
    &[0xae, 0xc9, 0xbe, 0x43, 0xf1, 0xd6, 0x56, 0x97],
);

#[derive(Debug)]
pub enum FrameworkGuidKind {
    TglBios,
    AdlBios,
    RplBios,
    MtlBios,
    Fw12RplBios,
    TglRetimer01,
    TglRetimer23,
    AdlRetimer01,
    AdlRetimer23,
    RplRetimer01,
    RplRetimer23,
    MtlRetimer01,
    MtlRetimer23,
    RplCsme,
    RplUCsme,
    MtlCsme,
    Fl16Bios,
    Amd16Ai300Bios,
    Amd13Ryzen7040Bios,
    Amd13Ai300Bios,
    DesktopAmdAi300Bios,
    WinUx,
    Unknown,
}

pub fn match_guid_kind(guid: &CGuid) -> FrameworkGuidKind {
    match GUID::from(*guid) {
        TGL_BIOS_GUID => FrameworkGuidKind::TglBios,
        ADL_BIOS_GUID => FrameworkGuidKind::AdlBios,
        RPL_BIOS_GUID => FrameworkGuidKind::RplBios,
        MTL_BIOS_GUID => FrameworkGuidKind::MtlBios,
        FW12_RPL_BIOS_GUID => FrameworkGuidKind::Fw12RplBios,
        FL16_BIOS_GUID => FrameworkGuidKind::Fl16Bios,
        AMD16_AI300_BIOS_GUID => FrameworkGuidKind::Amd16Ai300Bios,
        AMD13_RYZEN7040_BIOS_GUID => FrameworkGuidKind::Amd13Ryzen7040Bios,
        AMD13_AI300_BIOS_GUID => FrameworkGuidKind::Amd13Ai300Bios,
        DESKTOP_AMD_AI300_BIOS_GUID => FrameworkGuidKind::DesktopAmdAi300Bios,
        TGL_RETIMER01_GUID => FrameworkGuidKind::TglRetimer01,
        TGL_RETIMER23_GUID => FrameworkGuidKind::TglRetimer23,
        ADL_RETIMER01_GUID => FrameworkGuidKind::AdlRetimer01,
        ADL_RETIMER23_GUID => FrameworkGuidKind::AdlRetimer23,
        RPL_RETIMER01_GUID => FrameworkGuidKind::RplRetimer01,
        RPL_RETIMER23_GUID => FrameworkGuidKind::RplRetimer23,
        MTL_RETIMER01_GUID => FrameworkGuidKind::MtlRetimer01,
        MTL_RETIMER23_GUID => FrameworkGuidKind::MtlRetimer23,
        RPL_CSME_GUID => FrameworkGuidKind::RplCsme,
        RPL_U_CSME_GUID => FrameworkGuidKind::RplUCsme,
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
    pub fw_class: CGuid,
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
                    fw_class: CGuid::from(
                        GUID::parse(fw_class.trim()).expect("Kernel provided wrong value"),
                    ),
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

                let guid = GUID::parse(guid_str.trim()).expect("Kernel provided wrong value");
                let guid_kind = match_guid_kind(&CGuid::from(guid));
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
                    fw_class: CGuid::from(guid),
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
        uuid: SYSTEM_RESOURCE_TABLE_GUID_BYTES,
        table_len: 0,
        buf_len: 0,
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
pub const SYSTEM_RESOURCE_TABLE_GUID: GUID = GUID::build_from_components(
    0xb122a263,
    0x3661,
    0x4f68,
    &[0x99, 0x29, 0x78, 0xf8, 0xb0, 0xd6, 0x21, 0x80],
);
pub const SYSTEM_RESOURCE_TABLE_GUID_BYTES: [u8; 16] = [
    0x63, 0xa2, 0x22, 0xb1, 0x61, 0x36, 0x68, 0x4f, 0x99, 0x29, 0x78, 0xf8, 0xb0, 0xd6, 0x21, 0x80,
];

#[cfg(feature = "uefi")]
pub fn get_esrt() -> Option<Esrt> {
    let st = unsafe { uefi_services::system_table().as_ref() };
    let config_tables = st.config_table();

    for table in config_tables {
        // TODO: Why aren't they the same type?
        //debug!("Table: {:?}", table);
        let table_guid: CGuid = unsafe { std::mem::transmute(table.guid) };
        let table_guid = GUID::from(table_guid);
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
