//! Retrieve SMBIOS tables and extract information from them

use core::str::FromStr;
use std::prelude::v1::*;

#[cfg(all(not(feature = "uefi"), not(target_os = "freebsd")))]
use std::io::ErrorKind;

use crate::serialnum::Cfg0;
use crate::serialnum::FrameworkSerial;
use crate::util::Config;
pub use crate::util::{Platform, PlatformFamily};
use num_traits::FromPrimitive;
use smbioslib::*;
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

    let smbios = if let Some(smbios) = get_smbios() {
        smbios
    } else {
        return false;
    };

    for undefined_struct in smbios.iter() {
        if let DefinedStruct::SystemInformation(data) = undefined_struct.defined_struct() {
            if let Some(manufacturer) = dmidecode_string_val(&data.manufacturer()) {
                return manufacturer == "Framework";
            }
        }
    }

    false
}

pub fn dmidecode_string_val(s: &SMBiosString) -> Option<String> {
    match s.as_ref() {
        Ok(val) if val.is_empty() => Some("Not Specified".to_owned()),
        Ok(val) => Some(val.to_owned()),
        Err(SMBiosStringError::FieldOutOfBounds) => None,
        Err(SMBiosStringError::InvalidStringNumber(_)) => Some("<BAD INDEX>".to_owned()),
        Err(SMBiosStringError::Utf8(val)) => {
            Some(String::from_utf8_lossy(&val.clone().into_bytes()).to_string())
        }
    }
}

#[cfg(target_os = "freebsd")]
#[repr(C)]
pub struct Smbios3 {
    pub anchor: [u8; 5],
    pub checksum: u8,
    pub length: u8,
    pub major_version: u8,
    pub minor_version: u8,
    pub docrev: u8,
    pub revision: u8,
    _reserved: u8,
    pub table_length: u32,
    pub table_address: u64,
}

#[cfg(target_os = "freebsd")]
#[repr(C, packed)]
pub struct Smbios {
    pub anchor: [u8; 4],
    pub checksum: u8,
    pub length: u8,
    pub major_version: u8,
    pub minor_version: u8,
    pub max_structure_size: u16,
    pub revision: u8,
    pub formatted: [u8; 5],
    pub inter_anchor: [u8; 5],
    pub inter_checksum: u8,
    pub table_length: u16,
    pub table_address: u32,
    pub structure_count: u16,
    pub bcd_revision: u8,
}

#[cfg(target_os = "freebsd")]
pub fn get_smbios() -> Option<SMBiosData> {
    trace!("get_smbios() FreeBSD entry");
    // Get the SMBIOS entrypoint address from the kernel environment
    let addr_hex = kenv_get("hint.smbios.0.mem").ok()?;
    let addr_hex = addr_hex.trim_start_matches("0x");
    let addr = u64::from_str_radix(addr_hex, 16).unwrap();
    trace!("SMBIOS Entrypoint Addr: {} 0x{:x}", addr_hex, addr);

    let mut dev_mem = std::fs::File::open("/dev/mem").ok()?;
    // Smbios struct is larger than Smbios3 struct
    let mut header_buf = [0; std::mem::size_of::<Smbios>()];
    dev_mem.seek(SeekFrom::Start(addr)).ok()?;
    dev_mem.read_exact(&mut header_buf).ok()?;

    let entrypoint = unsafe { &*(header_buf.as_ptr() as *const Smbios3) };

    trace!("SMBIOS Anchor {:?} = ", entrypoint.anchor);
    let (addr, len, version) = match entrypoint.anchor {
        [b'_', b'S', b'M', b'3', b'_'] => {
            trace!("_SM3_");
            let entrypoint = unsafe { &*(header_buf.as_ptr() as *const Smbios3) };
            let ver = Some(SMBiosVersion {
                major: entrypoint.major_version,
                minor: entrypoint.minor_version,
                revision: 0,
            });

            (entrypoint.table_address, entrypoint.table_length, ver)
        }
        [b'_', b'S', b'M', b'_', _] => {
            trace!("_SM_");
            let entrypoint = unsafe { &*(header_buf.as_ptr() as *const Smbios) };
            let ver = Some(SMBiosVersion {
                major: entrypoint.major_version,
                minor: entrypoint.minor_version,
                revision: 0,
            });

            (
                entrypoint.table_address as u64,
                entrypoint.table_length as u32,
                ver,
            )
        }
        [b'_', b'D', b'M', b'I', b'_'] => {
            error!("_DMI_ - UNSUPPORTED");
            return None;
        }
        _ => {
            error!(" Unknown - UNSUPPORTED");
            return None;
        }
    };

    // Get actual SMBIOS table data
    let mut smbios_buf = vec![0; len as usize];
    dev_mem.seek(SeekFrom::Start(addr)).ok()?;
    dev_mem.read_exact(&mut smbios_buf).ok()?;

    let smbios = SMBiosData::from_vec_and_version(smbios_buf, version);
    Some(smbios)
}

#[cfg(feature = "uefi")]
pub fn get_smbios() -> Option<SMBiosData> {
    trace!("get_smbios() uefi entry");
    let data = crate::uefi::smbios_data().unwrap();
    let version = None; // TODO: Maybe add the version here
    let smbios = SMBiosData::from_vec_and_version(data, version);
    Some(smbios)
}
// On Linux this reads either from /dev/mem or sysfs
// On Windows from the kernel API
#[cfg(all(not(feature = "uefi"), not(target_os = "freebsd")))]
pub fn get_smbios() -> Option<SMBiosData> {
    trace!("get_smbios() linux entry");
    match smbioslib::table_load_from_device() {
        Ok(data) => Some(data),
        Err(ref e) if e.kind() == ErrorKind::PermissionDenied => {
            println!("Must be root to get SMBIOS data.");
            None
        }
        Err(err) => {
            println!("Failed to get SMBIOS: {:?}", err);
            None
        }
    }
}

pub fn get_product_name() -> Option<String> {
    // On FreeBSD we can short-circuit and avoid parsing SMBIOS
    #[cfg(target_os = "freebsd")]
    if let Ok(product) = kenv_get("smbios.system.product") {
        return Some(product);
    }

    let smbios = get_smbios();
    if smbios.is_none() {
        println!("Failed to find SMBIOS");
        return None;
    }
    let mut smbios = smbios.into_iter().flatten();
    smbios.find_map(|undefined_struct| {
        if let DefinedStruct::SystemInformation(data) = undefined_struct.defined_struct() {
            if let Some(product_name) = dmidecode_string_val(&data.product_name()) {
                return Some(product_name.as_str().to_string());
            }
        }
        None
    })
}

pub fn get_baseboard_version() -> Option<Cfg0> {
    // TODO: On FreeBSD we can short-circuit and avoid parsing SMBIOS
    // #[cfg(target_os = "freebsd")]
    // if let Ok(product) = kenv_get("smbios.system.product") {
    //     return Some(product);
    // }

    let smbios = get_smbios();
    if smbios.is_none() {
        error!("Failed to find SMBIOS");
        return None;
    }
    let mut smbios = smbios.into_iter().flatten();
    smbios.find_map(|undefined_struct| {
        if let DefinedStruct::BaseBoardInformation(data) = undefined_struct.defined_struct() {
            if let Some(version) = dmidecode_string_val(&data.version()) {
                // Assumes it's ASCII, which is guaranteed by SMBIOS
                let config_digit0 = &version[0..1];
                let config_digit0 = u8::from_str_radix(config_digit0, 16);
                if let Ok(version_config) = config_digit0.map(<Cfg0 as FromPrimitive>::from_u8) {
                    return version_config;
                } else {
                    debug!("  Invalid BaseBoard Version: {}'", version);
                }
            }
        }
        None
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

#[derive(Debug)]
enum SmbiosSerialNumber {
    Mainboard = 1,
    Laptop,
    Camera,
    Display,
    Battery,
    Touchpad,
    Keyboard,
    Fingerprint,
    AudioDaughtercard,
    ACover,
    BCover,
    CCover,
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

    Heatsink,
    Fan,
    Chassis,
    LeftPanel,
    RightPanel,
    FrontPanel,
    PowerSupply,
}

pub fn dump_oem_strings(strings: &SMBiosStringSet) {
    for (i, s) in strings.into_iter().enumerate() {
        let idx = i + 1;
        let sn = if get_family() == Some(PlatformFamily::FrameworkDesktop) {
            match idx {
                1 => Some(SmbiosSerialNumber::Mainboard),
                2 => Some(SmbiosSerialNumber::Heatsink),
                3 => Some(SmbiosSerialNumber::Fan),
                4 => Some(SmbiosSerialNumber::Chassis),
                5 => Some(SmbiosSerialNumber::AntennaMain),
                6 => Some(SmbiosSerialNumber::WifiModule),
                7 => Some(SmbiosSerialNumber::LeftPanel),
                8 => Some(SmbiosSerialNumber::RightPanel),
                9 => Some(SmbiosSerialNumber::FrontPanel),
                10 => Some(SmbiosSerialNumber::PowerSupply),
                11 => Some(SmbiosSerialNumber::RamSlot1),
                12 => Some(SmbiosSerialNumber::RamSlot2),
                13 => Some(SmbiosSerialNumber::Ssd),
                14 => Some(SmbiosSerialNumber::AudioFfc),
                _ => None,
            }
        } else {
            match idx {
                1 => Some(SmbiosSerialNumber::Mainboard),
                2 => Some(SmbiosSerialNumber::Laptop),
                3 => Some(SmbiosSerialNumber::Camera),
                4 => Some(SmbiosSerialNumber::Display),
                5 => Some(SmbiosSerialNumber::Battery),
                6 => Some(SmbiosSerialNumber::Touchpad),
                7 => Some(SmbiosSerialNumber::Keyboard),
                8 => Some(SmbiosSerialNumber::Fingerprint),
                10 => Some(SmbiosSerialNumber::AudioDaughtercard),
                11 => Some(SmbiosSerialNumber::ACover),
                12 => Some(SmbiosSerialNumber::BCover),
                13 => Some(SmbiosSerialNumber::CCover),
                14 => Some(SmbiosSerialNumber::AntennaMain),
                15 => Some(SmbiosSerialNumber::AntennaAux),
                16 => Some(SmbiosSerialNumber::TouchpadFpc),
                17 => Some(SmbiosSerialNumber::FingerprintFfc),
                18 => Some(SmbiosSerialNumber::EdpCable),
                19 => Some(SmbiosSerialNumber::LcdCable),
                20 => Some(SmbiosSerialNumber::ThermalAssembly),
                21 => Some(SmbiosSerialNumber::WifiModule),
                22 => Some(SmbiosSerialNumber::Speaker),
                23 => Some(SmbiosSerialNumber::RamSlot1),
                24 => Some(SmbiosSerialNumber::RamSlot2),
                25 => Some(SmbiosSerialNumber::Ssd),
                26 => Some(SmbiosSerialNumber::AudioFfc),
                _ => None,
            }
        };
        match sn {
            Some(SmbiosSerialNumber::RamSlot1)
            | Some(SmbiosSerialNumber::RamSlot2)
            | Some(SmbiosSerialNumber::Ssd)
            | Some(SmbiosSerialNumber::WifiModule) => {
                println!("{} {:<20} (Unused)", s, format!("{:?}", sn.unwrap()))
            }
            Some(SmbiosSerialNumber::Fingerprint) | Some(SmbiosSerialNumber::CCover) => {
                println!("{}", s);
                println!("  {:<20} (Only Pre-Built)", format!("{:?}", sn.unwrap()));
                if let Ok(serial) = FrameworkSerial::from_str(&format!("{:?}", s)) {
                    println!(
                        "  {} (Config {}) by {}, {:>4} Phase ({:?}, Week {}, {})",
                        serial.product,
                        serial.cfg1,
                        serial.oem,
                        format!("{:?}", serial.cfg0),
                        serial.day,
                        serial.week,
                        serial.year,
                    );
                }
            }
            Some(sn) => {
                println!("{}", s);
                println!("  {:?}", sn);
                if let Ok(serial) = FrameworkSerial::from_str(&format!("{:?}", s)) {
                    println!(
                        "  {} (Config {}) by {}, {:>4} Phase ({:?}, Week {}, {})",
                        serial.product,
                        serial.cfg1,
                        serial.oem,
                        format!("{:?}", serial.cfg0),
                        serial.day,
                        serial.week,
                        serial.year,
                    );
                }
            }
            _ => println!("{} Unknown/Reserved", s),
        }
    }
}
