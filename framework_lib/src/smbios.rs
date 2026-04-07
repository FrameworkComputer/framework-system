//! Retrieve SMBIOS tables and extract information from them

use std::prelude::v1::*;

use crate::util::Config;
pub use crate::util::{Platform, PlatformFamily};
use dmidecode::{EntryPoint, InfoType, RawStructure, Structure};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
#[cfg(feature = "uefi")]
use spin::Mutex;
#[cfg(not(feature = "uefi"))]
use std::sync::Mutex;

#[cfg(target_os = "freebsd")]
use std::io::{Read, Seek, SeekFrom};

/// Current platform. Won't ever change during the program's runtime
static CACHED_PLATFORM: Mutex<Option<Option<Platform>>> = Mutex::new(None);

// TODO: Should cache SMBIOS and values gotten from it
// SMBIOS is fixed after boot. Oh, so maybe not cache when we're running in UEFI

/// Wrapper around dmidecode's EntryPoint + raw table data.
/// Owns the data and provides iteration over SMBIOS structures.
pub struct SmbiosStore {
    entry_point: EntryPoint,
    table_data: Vec<u8>,
}

impl SmbiosStore {
    /// Parse from raw table data with a synthetic entry point.
    /// Used for tests, dump files, and Windows (where only table data is available).
    pub fn from_table_data(data: Vec<u8>, major: u8, minor: u8) -> Option<Self> {
        let ep_bytes = synthetic_entry_point_v3(major, minor, data.len() as u32);
        let entry_point = EntryPoint::search(&ep_bytes).ok()?;
        Some(SmbiosStore {
            entry_point,
            table_data: data,
        })
    }

    /// Parse from entry point bytes + table data.
    /// Used for Linux sysfs, FreeBSD, and UEFI where both are available separately.
    pub fn from_parts(entry_point_bytes: &[u8], table_data: Vec<u8>) -> Option<Self> {
        let entry_point = EntryPoint::search(entry_point_bytes).ok()?;
        Some(SmbiosStore {
            entry_point,
            table_data,
        })
    }

    /// Iterate SMBIOS structures
    pub fn structures(&self) -> dmidecode::Structures<'_> {
        self.entry_point.structures(&self.table_data)
    }
}

/// Build a valid 24-byte SMBIOS v3 entry point with correct checksum.
fn synthetic_entry_point_v3(major: u8, minor: u8, table_len: u32) -> [u8; 24] {
    let mut ep = [0u8; 24];
    ep[0..5].copy_from_slice(b"_SM3_");
    // [5] = checksum (computed below)
    ep[6] = 24; // length
    ep[7] = major;
    ep[8] = minor;
    // [9] = docrev, [11] = reserved — left as 0
    ep[10] = 1; // entry point revision
    ep[12..16].copy_from_slice(&table_len.to_le_bytes());
    // [16..24] = smbios_address — left as 0, we pass table data directly

    // Compute checksum so all bytes sum to 0
    let sum: u8 = ep.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    ep[5] = 0u8.wrapping_sub(sum);

    ep
}

#[repr(u8)]
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
pub enum ConfigDigit0 {
    Poc1 = 0x01,
    Proto1 = 0x02,
    Proto2 = 0x03,
    Evt1 = 0x04,
    Evt2 = 0x05,
    Dvt1 = 0x07,
    Dvt2 = 0x08,
    Pvt = 0x09,
    MassProduction = 0x0A,
    MassProductionB = 0x0B,
    MassProductionC = 0x0C,
    MassProductionD = 0x0D,
    MassProductionE = 0x0E,
    MassProductionF = 0x0F,
}

/// Check whether the manufacturer in the SMBIOS says Framework
pub fn is_framework() -> bool {
    if matches!(
        get_platform(),
        Some(Platform::GenericFramework((_, _, _), (_, _, _))) | Some(Platform::UnknownSystem)
    ) {
        return true;
    }

    // If we match any of our platforms, it's our platform
    if get_platform().is_some() {
        return true;
    }

    // Don't need to parse SMBIOS on FreeBSD, can just read kenv
    #[cfg(target_os = "freebsd")]
    if let Ok(maker) = kenv_get("smbios.system.maker") {
        return maker == "Framework";
    }

    let Some(smbios) = get_smbios() else {
        return false;
    };

    for result in smbios.structures() {
        if let Ok(Structure::System(sys)) = result {
            return sys.manufacturer == "Framework";
        }
    }

    false
}

pub fn get_product_name() -> Option<String> {
    // On FreeBSD we can short-circuit and avoid parsing SMBIOS
    #[cfg(target_os = "freebsd")]
    if let Ok(product) = kenv_get("smbios.system.product") {
        return Some(product);
    }

    let Some(smbios) = get_smbios() else {
        println!("Failed to find SMBIOS");
        return None;
    };
    smbios.structures().find_map(|result| match result {
        Ok(Structure::System(sys)) if !sys.product.is_empty() => Some(sys.product.to_string()),
        _ => None,
    })
}

pub fn get_baseboard_version() -> Option<ConfigDigit0> {
    let Some(smbios) = get_smbios() else {
        error!("Failed to find SMBIOS");
        return None;
    };
    smbios.structures().find_map(|result| {
        let Ok(Structure::BaseBoard(board)) = result else {
            return None;
        };
        let version = board.version;
        if version.is_empty() {
            return None;
        }
        // Assumes it's ASCII, which is guaranteed by SMBIOS
        let config_digit0 = u8::from_str_radix(&version[0..1], 16);
        match config_digit0.map(<ConfigDigit0 as FromPrimitive>::from_u8) {
            Ok(version_config) => version_config,
            Err(_) => {
                debug!("  Invalid BaseBoard Version: {}'", version);
                None
            }
        }
    })
}

pub fn get_family() -> Option<PlatformFamily> {
    get_platform().and_then(Platform::which_family)
}

/// Minimum size of an Additional Information entry (SMBIOS Type 40)
const DMI_A_INFO_ENT_MIN_SIZE: usize = 6;

/// Extract AGESA version string from an SMBIOS Type 40 (Additional Information) structure.
///
/// On AMD Zen systems, the AGESA version is stored here.
/// Sample string: "AGESA!V9 StrixKrackanPI-FP8 1.1.0.0c"
fn find_agesa_in_type40(raw: &RawStructure) -> Option<String> {
    if raw.info != InfoType::Oem(40) {
        return None;
    }

    // raw.data layout (after the 4-byte header):
    //   [0]:    count — number of Additional Information entries
    //   [1..]:  entries, each starting with a length byte
    let count = *raw.data.first()? as usize;
    let mut remaining = raw.data.get(1..)?;

    for _ in 0..count {
        if remaining.len() < DMI_A_INFO_ENT_MIN_SIZE {
            break;
        }
        let entry_len = remaining[0] as usize;
        if entry_len == 0 || entry_len > remaining.len() {
            break;
        }
        // String number is at offset 4 within the entry
        let str_num = remaining[4];
        if let Ok(s) = raw.find_string(str_num) {
            if s.starts_with("AGESA") {
                return Some(s.to_string());
            }
        }
        remaining = &remaining[entry_len..];
    }

    None
}

/// Get the AGESA version from SMBIOS Type 40 Additional Information entries
pub fn get_agesa_version() -> Option<String> {
    let smbios = get_smbios()?;
    smbios.structures().find_map(|result| {
        if let Ok(Structure::Other(ref raw)) = result {
            find_agesa_in_type40(raw)
        } else {
            None
        }
    })
}

pub fn get_platform() -> Option<Platform> {
    #[cfg(feature = "uefi")]
    let mut cached_platform = CACHED_PLATFORM.lock();
    #[cfg(not(feature = "uefi"))]
    let mut cached_platform = CACHED_PLATFORM.lock().unwrap();

    if let Some(platform) = *cached_platform {
        return platform;
    }

    if Config::is_set() {
        // Config::get() recursively calls get_platform.
        // Except if it's a GenericFramework platform
        let config = Config::get();
        let platform = &(*config).as_ref().unwrap().platform;
        if matches!(
            platform,
            Platform::GenericFramework((_, _, _), (_, _, _)) | Platform::UnknownSystem
        ) {
            return Some(*platform);
        }
    }

    let product_name = get_product_name()?;

    let platform = match product_name.as_str() {
        "Laptop" => Some(Platform::IntelGen11),
        "Laptop (12th Gen Intel Core)" => Some(Platform::IntelGen12),
        "Laptop (13th Gen Intel Core)" => Some(Platform::IntelGen13),
        "Laptop 13 (AMD Ryzen 7040Series)" => Some(Platform::Framework13Amd7080),
        "Laptop 13 (AMD Ryzen 7040 Series)" => Some(Platform::Framework13Amd7080),
        "Laptop 13 (AMD Ryzen AI 300 Series)" => Some(Platform::Framework13AmdAi300),
        "Laptop 12 (13th Gen Intel Core)" => Some(Platform::Framework12IntelGen13),
        "Laptop 13 (Intel Core Ultra Series 1)" => Some(Platform::IntelCoreUltra1),
        "Laptop 16 (AMD Ryzen 7040 Series)" => Some(Platform::Framework16Amd7080),
        "Laptop 16 (AMD Ryzen AI 300 Series)" => Some(Platform::Framework16AmdAi300),
        "Desktop (AMD Ryzen AI Max 300 Series)" => Some(Platform::FrameworkDesktopAmdAiMax300),
        _ => Some(Platform::UnknownSystem),
    };

    if let Some(platform) = platform {
        Config::set(platform);
    } else {
        println!("Failed to find PlatformFamily");
    }

    assert!(cached_platform.is_none());
    *cached_platform = Some(platform);
    platform
}

#[cfg(target_os = "freebsd")]
pub fn get_smbios() -> Option<SmbiosStore> {
    trace!("get_smbios() FreeBSD entry");
    // Get the SMBIOS entrypoint address from the kernel environment
    let addr_hex = kenv_get("hint.smbios.0.mem").ok()?;
    let addr_hex = addr_hex.trim_start_matches("0x");
    let addr = u64::from_str_radix(addr_hex, 16).unwrap();
    trace!("SMBIOS Entrypoint Addr: {} 0x{:x}", addr_hex, addr);

    let mut dev_mem = std::fs::File::open("/dev/mem").ok()?;
    // Read enough bytes for either V2 (31 bytes) or V3 (24 bytes) entry point
    let mut header_buf = [0u8; 32];
    dev_mem.seek(SeekFrom::Start(addr)).ok()?;
    dev_mem.read_exact(&mut header_buf).ok()?;

    let entry = EntryPoint::search(&header_buf).ok()?;
    let table_addr = entry.smbios_address();
    let table_len = entry.smbios_len() as usize;

    let mut table_data = vec![0u8; table_len];
    dev_mem.seek(SeekFrom::Start(table_addr)).ok()?;
    dev_mem.read_exact(&mut table_data).ok()?;

    SmbiosStore::from_parts(&header_buf, table_data)
}

#[cfg(feature = "uefi")]
pub fn get_smbios() -> Option<SmbiosStore> {
    trace!("get_smbios() uefi entry");
    let (ep_bytes, table_data) = crate::fw_uefi::smbios_data()?;
    SmbiosStore::from_parts(&ep_bytes, table_data)
}

#[cfg(target_os = "linux")]
pub fn get_smbios() -> Option<SmbiosStore> {
    trace!("get_smbios() linux entry");
    let ep_bytes = match std::fs::read("/sys/firmware/dmi/tables/smbios_entry_point") {
        Ok(data) => data,
        Err(ref e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            println!("Must be root to get SMBIOS data.");
            return None;
        }
        Err(err) => {
            println!("Failed to get SMBIOS: {:?}", err);
            return None;
        }
    };
    let table_data = match std::fs::read("/sys/firmware/dmi/tables/DMI") {
        Ok(data) => data,
        Err(err) => {
            println!("Failed to read SMBIOS table: {:?}", err);
            return None;
        }
    };
    SmbiosStore::from_parts(&ep_bytes, table_data)
}

#[cfg(windows)]
pub fn get_smbios() -> Option<SmbiosStore> {
    trace!("get_smbios() windows entry");
    use windows::Win32::System::SystemInformation::{
        GetSystemFirmwareTable, FIRMWARE_TABLE_PROVIDER,
    };

    let signature = FIRMWARE_TABLE_PROVIDER(u32::from_be_bytes(*b"RSMB"));
    let size = unsafe { GetSystemFirmwareTable(signature, 0, None) };
    if size == 0 {
        println!("Failed to get SMBIOS table size");
        return None;
    }

    let mut buf = vec![0u8; size as usize];
    let written = unsafe { GetSystemFirmwareTable(signature, 0, Some(&mut buf)) };
    if written == 0 {
        println!("Failed to read SMBIOS table data");
        return None;
    }

    // RSMB format: [Used20CallingMethod(1), Major(1), Minor(1), DmiRevision(1), Length(4), TableData...]
    if buf.len() < 8 {
        return None;
    }
    let major = buf[1];
    let minor = buf[2];
    let table_data = buf[8..].to_vec();
    SmbiosStore::from_table_data(table_data, major, minor)
}

#[cfg(target_os = "freebsd")]
fn kenv_get(name: &str) -> nix::Result<String> {
    use libc::{c_int, KENV_GET, KENV_MVALLEN};
    use nix::errno::Errno;
    use std::ffi::{CStr, CString};

    let cname = CString::new(name).unwrap();
    let name_ptr = cname.as_ptr();

    let mut value_buf = [0; 1 + KENV_MVALLEN as usize];

    unsafe {
        let res: c_int = libc::kenv(
            KENV_GET,
            name_ptr,
            value_buf.as_mut_ptr(),
            value_buf.len() as c_int,
        );
        Errno::result(res)?;

        let cvalue = CStr::from_ptr(value_buf.as_ptr());
        let value = cvalue.to_string_lossy().into_owned();

        Ok(value)
    }
}
