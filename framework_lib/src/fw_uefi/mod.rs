use alloc::vec::Vec;
use core::slice;
use uefi::system::with_config_table;
use uefi::table::cfg::ConfigTableEntry;

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams};
use uefi::proto::shell::Shell;
use uefi_raw::protocol::shell::ShellProtocol;

pub mod fs;

fn get_shell_protocol() -> &'static ShellProtocol {
    let handle = boot::get_handle_for_protocol::<Shell>().expect("No Shell handles");

    // Use GetProtocol instead of Exclusive since we're running inside the shell
    let shell = unsafe {
        boot::open_protocol::<Shell>(
            OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
        .expect("Failed to open Shell protocol")
    };

    // SAFETY: The Shell wrapper contains the raw ShellProtocol
    unsafe {
        let proto: &ShellProtocol = core::mem::transmute(shell.get().unwrap());
        // Leak to get 'static lifetime - protocol stays valid while shell is running
        core::mem::forget(shell);
        proto
    }
}

/// Returns true when the execution break was requested, false otherwise
pub fn shell_get_execution_break_flag() -> bool {
    let shell = get_shell_protocol();
    unsafe { (shell.get_page_break)().into() }
}

/// Enable pagination in UEFI shell
///
/// Pagination is handled by the UEFI shell environment automatically, whenever
/// the application prints more than fits on the screen.
pub fn enable_page_break() {
    let shell = get_shell_protocol();
    unsafe { (shell.enable_page_break)() }
}

/// Size of SMBIOS v2 entry point structure (31 bytes)
const SMBIOS_V2_EP_SIZE: usize = 31;
/// Size of SMBIOS v3 entry point structure (24 bytes)
const SMBIOS_V3_EP_SIZE: usize = 24;

pub fn smbios_data() -> Option<(Vec<u8>, Vec<u8>)> {
    use dmidecode::EntryPoint;

    with_config_table(|slice| {
        for i in slice {
            let ep_size = match i.guid {
                ConfigTableEntry::SMBIOS3_GUID => Some(SMBIOS_V3_EP_SIZE),
                ConfigTableEntry::SMBIOS_GUID => Some(SMBIOS_V2_EP_SIZE),
                _ => None,
            };
            if let Some(size) = ep_size {
                unsafe {
                    let ep_ptr = i.address as *const u8;
                    let ep_bytes = slice::from_raw_parts(ep_ptr, size);
                    if let Ok(entry) = EntryPoint::search(ep_bytes) {
                        debug!(
                            "SMBIOS entry point found, version {}.{}",
                            entry.major(),
                            entry.minor()
                        );
                        let table_data = slice::from_raw_parts(
                            entry.smbios_address() as *const u8,
                            entry.smbios_len() as usize,
                        );
                        // Return directly here because there is only ever the old config
                        // table or the new V3 config table. Never both.
                        return Some((ep_bytes.to_vec(), table_data.to_vec()));
                    }
                }
            }
        }

        None
    })
}
