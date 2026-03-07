//! Retrieve SMBIOS tables and extract information from them

use std::prelude::v1::*;

#[cfg(not(feature = "uefi"))]
use std::io::ErrorKind;

use crate::util::Config;
pub use crate::util::{Platform, PlatformFamily};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use smbioslib::*;
#[cfg(feature = "uefi")]
use spin::Mutex;
#[cfg(not(feature = "uefi"))]
use std::sync::Mutex;

/// Current platform. Won't ever change during the program's runtime
static CACHED_PLATFORM: Mutex<Option<Option<Platform>>> = Mutex::new(None);

// TODO: Should cache SMBIOS and values gotten from it
// SMBIOS is fixed after boot. Oh, so maybe not cache when we're running in UEFI

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
#[cfg(not(feature = "uefi"))]
pub fn get_smbios() -> Option<SMBiosData> {
    trace!("get_smbios() entry");
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

pub fn get_baseboard_version() -> Option<ConfigDigit0> {
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
                if let Ok(version_config) =
                    config_digit0.map(<ConfigDigit0 as FromPrimitive>::from_u8)
                {
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
