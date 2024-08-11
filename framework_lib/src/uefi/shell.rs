use uefi::table::boot::{BootServices, OpenProtocolAttributes, OpenProtocolParams, SearchType};

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::proto::shell::Shell;
use uefi::Identify;

use uefi::table::boot::ScopedProtocol;
pub fn find_shell_handle(bt: &BootServices) -> Option<ScopedProtocol<Shell>> {
    let shell_handles = bt.locate_handle_buffer(SearchType::ByProtocol(&Shell::GUID));
    if let Ok(sh_buf) = shell_handles {
        if let Some(handle) = (*sh_buf).iter().next() {
            return Some(unsafe {
                bt.open_protocol::<Shell>(
                    OpenProtocolParams {
                        handle: *handle,
                        agent: bt.image_handle(),
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
pub fn get_execution_break_flag() -> Option<bool> {
    // TODO: Avoid unwrap
    let st = uefi::table::system_table_boot().unwrap();
    let boot_services = st.boot_services();
    let shell_handles = boot_services.locate_handle_buffer(SearchType::ByProtocol(&Shell::GUID));
    if let Ok(sh_buf) = shell_handles {
        if let Some(handle) = (*sh_buf).iter().next() {
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

            let event = shell_handle.execution_break()?;
            return boot_services.check_event(event).ok();
        }
        Some(false)
    } else {
        error!("No shell handle found!");
        None
    }
}

/// Enable pagination in UEFI shell
///
/// Pagination is handled by the UEFI shell environment automatically, whenever
/// the application prints more than fits on the screen.
pub fn enable_page_break() -> Option<()> {
    let st = uefi::table::system_table_boot()?;
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

        Some(())
    } else {
        error!("No shell handle found!");
        None
    }
}
