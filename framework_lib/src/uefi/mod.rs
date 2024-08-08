use alloc::vec::Vec;
use core::slice;

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::table::cfg::{SMBIOS3_GUID, SMBIOS_GUID};

pub mod fs;
pub mod shell;

#[repr(packed)]
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

impl Smbios {
    pub fn checksum_valid(&self) -> bool {
        let mut sum: u8 = self.anchor.iter().sum::<u8>();
        sum += self.checksum;
        sum += self.length;
        sum += self.major_version;
        sum += self.minor_version;
        sum += self.max_structure_size as u8;
        sum += self.revision;
        sum += self.formatted.iter().sum::<u8>();
        sum == 0
    }
}

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

pub fn smbios_data() -> Option<Vec<u8>> {
    let st = uefi::table::system_table_boot()?;
    let config_tables = st.config_table();

    for table in config_tables {
        let table_data = match table.guid {
            SMBIOS3_GUID => unsafe {
                let smbios = &*(table.address as *const Smbios3);
                debug!("SMBIOS3 valid: {:?}", smbios.anchor == *b"_SM3_");
                Some(slice::from_raw_parts(
                    smbios.table_address as *const u8,
                    smbios.table_length as usize,
                ))
            },
            SMBIOS_GUID => unsafe {
                let smbios = &*(table.address as *const Smbios);
                debug!("SMBIOS valid: {:?}", smbios.checksum_valid());
                Some(slice::from_raw_parts(
                    smbios.table_address as *const u8,
                    smbios.table_length as usize,
                ))
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
