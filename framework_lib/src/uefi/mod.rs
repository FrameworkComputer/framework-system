use core::slice;
use std::proto::Protocol;
use std::uefi::guid::GuidKind;

pub mod fs;

/// Returns true when the execution break was requested, false otherwise
pub fn shell_get_execution_break_flag() -> bool {
    let shell = if let Ok(shell) = fs::Shell::locate_protocol() {
        shell
    } else {
        println!("Failed to open Shell Protocol");
        return false;
    };

    let uefi = std::system_table();
    let check_event = uefi.BootServices.CheckEvent;
    let status = (check_event)(shell.0.ExecutionBreak);
    status.0 == 0
}

pub fn smbios_data() -> Option<Vec<u8>> {
    for config_table in std::system_table().config_tables() {
        let table_data = match config_table.VendorGuid.kind() {
            GuidKind::Smbios => unsafe {
                let smbios = &*(config_table.VendorTable as *const dmi::Smbios);
                // TODO: Seems to be invalid. Is the calculation correct?
                //smbios.is_valid();
                Some(slice::from_raw_parts(
                    smbios.table_address as *const u8,
                    smbios.table_length as usize,
                ))
            },
            GuidKind::Smbios3 => unsafe {
                // TODO: Seems to be invalid. Is the calculation correct?
                //smbios.is_valid();
                let smbios = &*(config_table.VendorTable as *const dmi::Smbios3);
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
