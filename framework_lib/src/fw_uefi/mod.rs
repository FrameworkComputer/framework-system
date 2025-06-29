use alloc::vec::Vec;
use core::slice;
use uefi::system::with_config_table;
use uefi::table::cfg::ConfigTableEntry;

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::boot;
use uefi::proto::shell::{Shell, ShellProtocol};

pub mod fs;

/// Returns true when the execution break was requested, false otherwise
pub fn shell_get_execution_break_flag() -> bool {
    let handle = boot::get_handle_for_protocol::<Shell>().expect("No Shell handles");
    let shell =
        boot::open_protocol_exclusive::<Shell>(handle).expect("Failed to open Shell protocol");
    unsafe {
        let proto: &ShellProtocol = std::mem::transmute(shell.get().unwrap());
        (proto.get_page_break)()
    }
}

/// Enable pagination in UEFI shell
///
/// Pagination is handled by the UEFI shell environment automatically, whenever
/// the application prints more than fits on the screen.
pub fn enable_page_break() {
    let handle = boot::get_handle_for_protocol::<Shell>().expect("No Shell handles");
    let shell =
        boot::open_protocol_exclusive::<Shell>(handle).expect("Failed to open Shell protocol");
    unsafe {
        let proto: &ShellProtocol = std::mem::transmute(shell.get().unwrap());
        (proto.enable_page_break)()
    }
}

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
    with_config_table(|slice| {
        for i in slice {
            let table_data = match i.guid {
                ConfigTableEntry::SMBIOS3_GUID => unsafe {
                    let smbios = &*(i.address as *const Smbios3);
                    debug!("SMBIOS3 valid: {:?}", smbios.anchor == *b"_SM3_");
                    Some(slice::from_raw_parts(
                        smbios.table_address as *const u8,
                        smbios.table_length as usize,
                    ))
                },
                ConfigTableEntry::SMBIOS_GUID => unsafe {
                    let smbios = &*(i.address as *const Smbios);
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
    })
}
