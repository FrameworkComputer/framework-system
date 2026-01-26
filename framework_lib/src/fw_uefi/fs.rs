use alloc::vec;
use alloc::vec::Vec;
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams};
use uefi::proto::shell::{FileMode, Shell};
use uefi::{CString16, Result, Status};

pub fn shell_read_file(path: &str) -> Option<Vec<u8>> {
    let handle = boot::get_handle_for_protocol::<Shell>().expect("No Shell handles");
    let shell = unsafe {
        boot::open_protocol::<Shell>(
            OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )
        .ok()?
    };

    let c_path = CString16::try_from(path).ok()?;
    let mut file = shell.open(&c_path, FileMode::READ).ok()?;

    let file_size = file.size().ok()?;
    let mut buffer: Vec<u8> = vec![0; file_size as usize];
    let read_size = file.read(&mut buffer).ok()?;

    buffer.truncate(read_size);
    Some(buffer)
}

pub fn shell_write_file(path: &str, data: &[u8]) -> Result {
    let handle =
        boot::get_handle_for_protocol::<Shell>().map_err(|e| uefi::Error::from(e.status()))?;
    let shell = unsafe {
        boot::open_protocol::<Shell>(
            OpenProtocolParams {
                handle,
                agent: boot::image_handle(),
                controller: None,
            },
            OpenProtocolAttributes::GetProtocol,
        )?
    };

    let c_path =
        CString16::try_from(path).map_err(|_| uefi::Error::from(Status::INVALID_PARAMETER))?;

    let mode = FileMode::READ | FileMode::WRITE | FileMode::CREATE;
    let mut file = shell.open(&c_path, mode)?;

    file.write(data)?;
    Ok(())
}
