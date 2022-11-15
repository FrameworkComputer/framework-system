use core::slice;
use std::uefi::guid::GuidKind;

pub fn smbios_data() -> Option<Vec<u8>> {
    for config_table in std::system_table().config_tables() {
        let table_data = match config_table.VendorGuid.kind() {
            GuidKind::Smbios => unsafe {
                let smbios = &*(config_table.VendorTable as *const dmi::Smbios);
                if smbios.is_valid() {
                    Some(slice::from_raw_parts(
                        smbios.table_address as *const u8,
                        smbios.table_length as usize,
                    ))
                } else {
                    panic!("SMBIOS Config table is invalid! Can't fetch tables.");
                }
            },
            GuidKind::Smbios3 => unsafe {
                let smbios = &*(config_table.VendorTable as *const dmi::Smbios3);
                if smbios.is_valid() {
                    Some(slice::from_raw_parts(
                        smbios.table_address as *const u8,
                        smbios.table_length as usize,
                    ))
                } else {
                    panic!("SMBIOS Config table v3 is invalid! Can't fetch tables.");
                }
            },
            _ => None,
        };

        if let Some(data) = table_data {
            // Return directly here because there is only ever the old config
            // table or the new V3 config table. Never both.
            return Some(data.to_vec());
        }
    }
    None
}
