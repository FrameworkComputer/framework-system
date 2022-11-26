#[cfg(not(feature = "uefi"))]
use std::io::ErrorKind;

use smbioslib::*;

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
