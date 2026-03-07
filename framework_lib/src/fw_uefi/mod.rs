use alloc::vec::Vec;
use core::slice;
use uefi::table::boot::{OpenProtocolAttributes, OpenProtocolParams, ScopedProtocol, SearchType};

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::proto::shell::Shell;
use uefi::table::cfg::{SMBIOS3_GUID, SMBIOS_GUID};
use uefi::table::{Boot, SystemTable};
use uefi::Identify;

pub mod fs;

pub fn get_system_table() -> &'static SystemTable<Boot> {
    unsafe { uefi_services::system_table().as_ref() }
}

fn find_shell_handle() -> Option<ScopedProtocol<'static, Shell>> {
    let st = unsafe { uefi_services::system_table().as_ref() };
    let boot_services = st.boot_services();
    let shell_handles = boot_services.locate_handle_buffer(SearchType::ByProtocol(&Shell::GUID));
    if let Ok(sh_buf) = shell_handles {
        for handle in &*sh_buf {
            return Some(unsafe {
                boot_services
                    .open_protocol::<Shell>(
                        OpenProtocolParams {
                            handle: *handle,
                            agent: boot_services.image_handle(),
                            controller: None,
                        },
                        OpenProtocolAttributes::GetProtocol,
                    )
                    .expect("Failed to open Shell handle")
            });
        }
    } else {
        panic!("No shell handle found!");
    }
    None
}

/// Returns true when the execution break was requested, false otherwise
pub fn shell_get_execution_break_flag() -> bool {
    let st = unsafe { uefi_services::system_table().as_ref() };
    let boot_services = st.boot_services();
    let shell_handles = boot_services.locate_handle_buffer(SearchType::ByProtocol(&Shell::GUID));
    if let Ok(sh_buf) = shell_handles {
        for handle in &*sh_buf {
            let shell_handle = unsafe {
                boot_services
                    .open_protocol::<Shell>(
                        OpenProtocolParams {
                            handle: *handle,
                            agent: boot_services.image_handle(),
                            controller: None,
                        },
                        OpenProtocolAttributes::GetProtocol,
                    )
                    .expect("Failed to open Shell handle")
            };

            let event = unsafe { shell_handle.execution_break.unsafe_clone() };
            return boot_services.check_event(event).unwrap();
        }
        return false;
    } else {
        panic!("No shell handle found!");
    }
}

/// Enable pagination in UEFI shell
///
/// Pagination is handled by the UEFI shell environment automatically, whenever
/// the application prints more than fits on the screen.
pub fn enable_page_break() {
    let st = unsafe { uefi_services::system_table().as_ref() };
    let boot_services = st.boot_services();
    let shell_handles = boot_services.locate_handle_buffer(SearchType::ByProtocol(&Shell::GUID));
    if let Ok(sh_buf) = shell_handles {
        for handle in &*sh_buf {
            //trace!("Calling enable_page_break");
            let shell_handle = unsafe {
                boot_services
                    .open_protocol::<Shell>(
                        OpenProtocolParams {
                            handle: *handle,
                            agent: boot_services.image_handle(),
                            controller: None,
                        },
                        OpenProtocolAttributes::GetProtocol,
                    )
                    .expect("Failed to open Shell handle")
            };
            shell_handle.enable_page_break();
        }
    } else {
        panic!("No shell handle found!");
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
    let st = unsafe { uefi_services::system_table().as_ref() };
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
