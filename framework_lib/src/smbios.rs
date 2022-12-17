#[cfg(not(feature = "uefi"))]
use std::io::ErrorKind;

use crate::util::Platform;
use smbioslib::*;

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
    let smbios = get_smbios();
    if smbios.is_none() {
        println!("Failed to find SMBIOS");
        return None;
    }
    for undefined_struct in smbios.unwrap().iter() {
        if let DefinedStruct::SystemInformation(data) = undefined_struct.defined_struct() {
            if let Some(product_name) = dmidecode_string_val(&data.product_name()) {
                if product_name == "Laptop (12th Gen Intel Core)" {
                    return Some(Platform::IntelGen12);
                }
            }
            if let Some(family) = dmidecode_string_val(&data.family()) {
                if family == "FRANBMCP" {
                    return Some(Platform::IntelGen11);
                }
            }
        }
    }

    println!("Failed to find PlatformFamily");
    None
}
