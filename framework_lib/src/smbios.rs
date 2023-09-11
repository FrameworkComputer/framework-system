//! Retrieve SMBIOS tables and extract information from them

use std::prelude::v1::*;

#[cfg(not(feature = "uefi"))]
use std::io::ErrorKind;

use crate::util::Platform;
use smbioslib::*;
#[cfg(feature = "uefi")]
use spin::Mutex;
#[cfg(not(feature = "uefi"))]
use std::sync::Mutex;

/// Current platform. Won't ever change during the program's runtime
static CACHED_PLATFORM: Mutex<Option<Option<Platform>>> = Mutex::new(None);

// TODO: Should cache SMBIOS and values gotten from it
// SMBIOS is fixed after boot. Oh, so maybe not cache when we're running in UEFI

/// Check whether the manufacturer in the SMBIOS says Framework
pub fn is_framework() -> bool {
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
    let data = crate::uefi::smbios_data().unwrap();
    let version = None; // TODO: Maybe add the version here
    let smbios = SMBiosData::from_vec_and_version(data, version);
    Some(smbios)
}
// On Linux this reads either from /dev/mem or sysfs
// On FreeBSD from /dev/mem
// On Windows from the kernel API
#[cfg(not(feature = "uefi"))]
pub fn get_smbios() -> Option<SMBiosData> {
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

pub fn get_platform() -> Option<Platform> {
    #[cfg(feature = "uefi")]
    let mut cached_platform = CACHED_PLATFORM.lock();
    #[cfg(not(feature = "uefi"))]
    let mut cached_platform = CACHED_PLATFORM.lock().unwrap();

    if let Some(platform) = *cached_platform {
        return platform;
    }

    let smbios = get_smbios();
    if smbios.is_none() {
        println!("Failed to find SMBIOS");
    }
    let mut smbios = smbios.into_iter().flatten();
    let platform = smbios.find_map(|undefined_struct| {
        if let DefinedStruct::SystemInformation(data) = undefined_struct.defined_struct() {
            if let Some(product_name) = dmidecode_string_val(&data.product_name()) {
                match product_name.as_str() {
                    "Laptop" => return Some(Platform::IntelGen11),
                    "Laptop (12th Gen Intel Core)" => return Some(Platform::IntelGen12),
                    "Laptop (13th Gen Intel Core)" => return Some(Platform::IntelGen13),
                    "Laptop 13 (AMD Ryzen 7040Series)" => return Some(Platform::Framework13Amd),
                    "Laptop 16 (AMD Ryzen 7040Series)" => return Some(Platform::Framework16),
                    _ => {}
                }
            }
            if let Some(family) = dmidecode_string_val(&data.family()) {
                match family.as_str() {
                    // TGL Mainboard (I don't this ever appears in family)
                    "FRANBMCP" => return Some(Platform::IntelGen11),
                    // ADL Mainboard (I don't this ever appears in family)
                    "FRANMACP" => return Some(Platform::IntelGen12),
                    // RPL Mainboard (I don't this ever appears in family)
                    "FRANMCCP" => return Some(Platform::IntelGen13),
                    // Framework 13 AMD Mainboard
                    "FRANMDCP" => return Some(Platform::Framework13Amd),
                    // Framework 16 Mainboard
                    "FRANMZCP" => return Some(Platform::Framework16),
                    _ => {}
                }
            }
        }
        None
    });

    if platform.is_none() {
        println!("Failed to find PlatformFamily");
    }

    assert!(cached_platform.is_none());
    *cached_platform = Some(platform);
    platform
}
