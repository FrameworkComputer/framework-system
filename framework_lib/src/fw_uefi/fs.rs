use alloc::vec;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams};
use uefi::proto::shell::Shell;
use uefi::{CString16, Result, Status, StatusExt};
use uefi_raw::protocol::file_system::FileMode;
use uefi_raw::protocol::shell::ShellProtocol;
use uefi_raw::protocol::shell_params::ShellFileHandle;

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

pub fn shell_read_file(path: &str) -> Option<Vec<u8>> {
    let shell = get_shell_protocol();
    let c_path = CString16::try_from(path).ok()?;

    unsafe {
        let mut handle: MaybeUninit<ShellFileHandle> = MaybeUninit::zeroed();
        let status = (shell.open_file_by_name)(
            c_path.as_ptr().cast(),
            handle.as_mut_ptr(),
            FileMode::READ.bits(),
        );
        if status.is_error() {
            return None;
        }

        let file_handle = handle.assume_init();

        let mut file_size: u64 = 0;
        let status = (shell.get_file_size)(file_handle, &mut file_size);
        if status.is_error() {
            let _ = (shell.close_file)(file_handle);
            return None;
        }

        let mut buffer: Vec<u8> = vec![0; file_size as usize];
        let mut read_size = file_size as usize;
        let status = (shell.read_file)(
            file_handle,
            &mut read_size,
            buffer.as_mut_ptr().cast(),
        );

        let _ = (shell.close_file)(file_handle);

        if status.is_error() {
            return None;
        }

        buffer.truncate(read_size);
        Some(buffer)
    }
}

pub fn shell_write_file(path: &str, data: &[u8]) -> Result {
    let shell = get_shell_protocol();
    let c_path = CString16::try_from(path).map_err(|_| uefi::Error::from(Status::INVALID_PARAMETER))?;

    unsafe {
        let mode = FileMode::READ | FileMode::WRITE | FileMode::CREATE;
        let mut handle: MaybeUninit<ShellFileHandle> = MaybeUninit::zeroed();
        (shell.open_file_by_name)(
            c_path.as_ptr().cast(),
            handle.as_mut_ptr(),
            mode.bits(),
        )
        .to_result()?;

        let file_handle = handle.assume_init();

        let mut write_size = data.len();
        let status = (shell.write_file)(
            file_handle,
            &mut write_size,
            data.as_ptr() as *mut _,
        );

        let _ = (shell.close_file)(file_handle);

        status.to_result()
    }
}
