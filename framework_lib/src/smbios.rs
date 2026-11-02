//! Retrieve SMBIOS tables and extract information from them

use core::str::FromStr;
use std::prelude::v1::*;

use crate::serialnum::{Cfg0, FrameworkSerial};
use crate::util::Config;
pub use crate::util::{Platform, PlatformFamily};
use dmidecode::{EntryPoint, Structure, StructureStrings};
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

pub fn get_baseboard_version() -> Option<Cfg0> {
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
        match config_digit0.map(<Cfg0 as FromPrimitive>::from_u8) {
            Ok(version_config) => version_config,
            Err(_) => {
                debug!("  Invalid BaseBoard Version: {}'", version);
                None
            }
        }
    })
}

/// Year of a BIOS release date in SMBIOS format: mm/dd/yyyy or mm/dd/yy
fn parse_release_year(date: &str) -> Option<u16> {
    let year = date.trim().rsplit('/').next()?;
    let year = year.parse::<u16>().ok()?;
    match year {
        0..=99 => Some(2000 + year),
        1000..=9999 => Some(year),
        _ => None,
    }
}

/// Year in which the running BIOS was released
pub fn get_bios_release_year() -> Option<u16> {
    get_smbios()?.structures().find_map(|result| match result {
        Ok(Structure::Bios(bios)) => parse_release_year(bios.bios_release_date),
        _ => None,
    })
}

pub fn get_family() -> Option<PlatformFamily> {
    get_platform().and_then(Platform::which_family)
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
        "Laptop 12 (Intel Core Series 3)" => Some(Platform::Framework12IntelCore3),
        "Laptop 13 (Intel Core Ultra Series 1)" => Some(Platform::IntelCoreUltra1),
        "Laptop 13 Pro (Intel Core Ultra Series 3)" => Some(Platform::IntelCoreUltra3),
        "Laptop 16 (AMD Ryzen 7040 Series)" => Some(Platform::Framework16Amd7080),
        "Laptop 16 (AMD Ryzen AI 300 Series)" => Some(Platform::Framework16AmdAi300),
        "Desktop (AMD Ryzen AI Max 300 Series)" => Some(Platform::FrameworkDesktopAmdAiMax300),
        "Desktop (AMD Ryzen AI Max 400 Series)" => Some(Platform::FrameworkDesktopAmdAiMax400),
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

/// Not a Framework system, there's no point in reading SMBIOS on macOS
#[cfg(target_os = "macos")]
pub fn get_smbios() -> Option<SmbiosStore> {
    trace!("get_smbios() macos entry");
    None
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

/// Part whose serial number is in an OEM string (SMBIOS Type 11)
#[derive(Debug, Clone, Copy, PartialEq)]
enum SmbiosSerialNumber {
    Mainboard,
    Laptop,
    Camera,
    Display,
    Battery,
    Touchpad,
    Keyboard,
    Fingerprint,
    AudioDaughtercard,
    ACover,
    Bezel,
    CCover,
    MidPlate,
    DCover,
    AntennaMain,
    AntennaAux,
    TouchpadFpc,
    FingerprintFfc,
    EdpCable,
    LcdCable,
    ThermalAssembly,
    WifiModule,
    Speaker,
    RamSlot1,
    RamSlot2,
    Ssd,
    AudioFfc,

    // Laptop 16 only
    LeftFan,
    RightFan,
    InputModuleHub,
    LidSensor,
    GpuModule,
    InputModuleHubFpc,
    ExpansionBayInterposer,
    GpuThermal,

    System,
    Heatsink,
    Fan,
    Chassis,
    Antenna,
    LeftPanel,
    RightPanel,
    FrontPanel,
    PowerSupply,
    AudioCable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SlotUsage {
    Used,
    /// Only programmed on pre-built systems
    PrebuiltOnly,
    /// Slot is reserved, but not programmed
    Unused,
}

/// Which part's serial is in the OEM string at 1-based index `idx`
///
/// Layouts are defined in the BIOS ERS, section "OEM Strings (Type 11)"
fn oem_string_slot(
    family: Option<PlatformFamily>,
    idx: usize,
) -> Option<(SmbiosSerialNumber, SlotUsage)> {
    use SlotUsage::*;
    use SmbiosSerialNumber as Sn;

    // Laptop 12, Laptop 13 and Laptop 16
    const LAPTOP: &[(SmbiosSerialNumber, SlotUsage)] = &[
        (Sn::Mainboard, Used),
        (Sn::Laptop, Used),
        (Sn::Camera, Used),
        (Sn::Display, Used),
        (Sn::Battery, Used),
        (Sn::Touchpad, Used),
        (Sn::Keyboard, Used),
        (Sn::Fingerprint, PrebuiltOnly),
        (Sn::AudioDaughtercard, Used),
        (Sn::ACover, Used),
        (Sn::Bezel, Used),
        (Sn::CCover, PrebuiltOnly),
        (Sn::DCover, Used),
        (Sn::AntennaMain, Used),
        (Sn::AntennaAux, Used),
        (Sn::TouchpadFpc, Used),
        (Sn::FingerprintFfc, Used),
        (Sn::EdpCable, Used),
        (Sn::LcdCable, Used),
        (Sn::ThermalAssembly, Used),
        (Sn::WifiModule, Unused),
        (Sn::Speaker, Used),
        (Sn::RamSlot1, Unused),
        (Sn::RamSlot2, Unused),
        (Sn::Ssd, Unused),
        (Sn::AudioFfc, Used),
    ];
    // Laptop 16 continues after the common laptop slots.
    // Not in the BIOS ERS, but in the serialization tracker and on real systems.
    const LAPTOP16_EXTRA: &[(SmbiosSerialNumber, SlotUsage)] = &[
        (Sn::LeftFan, Used),
        (Sn::RightFan, Used),
        (Sn::InputModuleHub, Used),
        (Sn::LidSensor, Used),
        (Sn::GpuModule, Used),
        (Sn::InputModuleHubFpc, Used),
        (Sn::ExpansionBayInterposer, Used),
        (Sn::GpuThermal, Used),
    ];
    const DESKTOP: &[(SmbiosSerialNumber, SlotUsage)] = &[
        (Sn::Mainboard, Used),
        (Sn::System, Used),
        (Sn::Heatsink, Used),
        (Sn::Fan, Used),
        (Sn::Chassis, Used),
        (Sn::Antenna, Used),
        (Sn::WifiModule, Used),
        (Sn::LeftPanel, PrebuiltOnly),
        (Sn::RightPanel, Used),
        (Sn::FrontPanel, Used),
        (Sn::PowerSupply, Used),
        (Sn::RamSlot1, Unused),
        (Sn::RamSlot2, Unused),
        (Sn::Ssd, Used),
        (Sn::AudioCable, Used),
    ];

    let i = idx.checked_sub(1)?;
    let slot = match family {
        Some(PlatformFamily::FrameworkDesktop) => *DESKTOP.get(i)?,
        Some(PlatformFamily::Framework16) if i >= LAPTOP.len() => {
            *LAPTOP16_EXTRA.get(i - LAPTOP.len())?
        }
        _ => *LAPTOP.get(i)?,
    };
    // Laptop 16 has a mid plate where the others have the C cover
    if family == Some(PlatformFamily::Framework16) && slot.0 == Sn::CCover {
        return Some((Sn::MidPlate, Used));
    }
    Some(slot)
}

pub fn dump_oem_strings(strings: StructureStrings) {
    let family = get_family();
    // Some factories wrote invalid years, don't decode those.
    // Parts are often made a few months after the BIOS running on the system
    // was released, because it's flashed onto the mainboard before final
    // assembly. So allow up to one year after the BIOS release.
    let max_year = get_bios_release_year().map(|year| year + 1);
    for (i, s) in strings.enumerate() {
        let idx = i + 1;
        // Unprogrammed entries are all zeros or "*" padded with spaces
        let trimmed = s.trim();
        if trimmed.is_empty() || trimmed == "*" || trimmed.chars().all(|c| c == '0') {
            continue;
        }
        let Some((sn, usage)) = oem_string_slot(family, idx) else {
            println!("{} Unknown/Reserved", s);
            continue;
        };
        let note = match usage {
            SlotUsage::Unused => {
                println!("{} {:<20} (Unused)", s, format!("{:?}", sn));
                continue;
            }
            SlotUsage::PrebuiltOnly => " (Only Pre-Built)",
            SlotUsage::Used => "",
        };
        println!("{}", s);
        println!("  {:<20}{}", format!("{:?}", sn), note);
        match FrameworkSerial::from_str(s) {
            Ok(serial) if matches!(max_year, Some(max) if serial.year > max) => {
                debug!(
                    "  Year {} is more than a year after the BIOS release",
                    serial.year
                );
            }
            Ok(serial) => println!("  {}", serial),
            Err(err) => debug!("  {}", err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_year() {
        assert_eq!(parse_release_year("03/17/2025"), Some(2025));
        assert_eq!(parse_release_year("03/17/25"), Some(2025));
        assert_eq!(parse_release_year(" 03/17/2025 "), Some(2025));
        assert_eq!(parse_release_year(""), None);
        assert_eq!(parse_release_year("03/17/202"), None);
        assert_eq!(parse_release_year("unknown"), None);
    }

    #[test]
    fn oem_string_slots() {
        use SlotUsage::*;
        use SmbiosSerialNumber as Sn;
        let laptop = Some(PlatformFamily::Framework13);
        let fw16 = Some(PlatformFamily::Framework16);
        let desktop = Some(PlatformFamily::FrameworkDesktop);

        assert_eq!(oem_string_slot(laptop, 0), None);
        assert_eq!(oem_string_slot(laptop, 1), Some((Sn::Mainboard, Used)));
        assert_eq!(
            oem_string_slot(laptop, 12),
            Some((Sn::CCover, PrebuiltOnly))
        );
        assert_eq!(oem_string_slot(laptop, 21), Some((Sn::WifiModule, Unused)));
        assert_eq!(oem_string_slot(laptop, 26), Some((Sn::AudioFfc, Used)));
        assert_eq!(oem_string_slot(laptop, 27), None);
        assert_eq!(oem_string_slot(fw16, 12), Some((Sn::MidPlate, Used)));
        assert_eq!(oem_string_slot(fw16, 26), Some((Sn::AudioFfc, Used)));
        assert_eq!(oem_string_slot(fw16, 27), Some((Sn::LeftFan, Used)));
        assert_eq!(oem_string_slot(fw16, 31), Some((Sn::GpuModule, Used)));
        assert_eq!(oem_string_slot(fw16, 34), Some((Sn::GpuThermal, Used)));
        assert_eq!(oem_string_slot(fw16, 35), None);
        assert_eq!(oem_string_slot(None, 2), Some((Sn::Laptop, Used)));

        assert_eq!(oem_string_slot(desktop, 1), Some((Sn::Mainboard, Used)));
        assert_eq!(oem_string_slot(desktop, 2), Some((Sn::System, Used)));
        assert_eq!(oem_string_slot(desktop, 7), Some((Sn::WifiModule, Used)));
        assert_eq!(
            oem_string_slot(desktop, 8),
            Some((Sn::LeftPanel, PrebuiltOnly))
        );
        assert_eq!(oem_string_slot(desktop, 12), Some((Sn::RamSlot1, Unused)));
        assert_eq!(oem_string_slot(desktop, 14), Some((Sn::Ssd, Used)));
        assert_eq!(oem_string_slot(desktop, 15), Some((Sn::AudioCable, Used)));
        assert_eq!(oem_string_slot(desktop, 16), None);
    }
}
