use nix::ioctl_readwrite;
use num_traits::FromPrimitive;
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};

use crate::chromium_ec::command::EcCommands;
use crate::chromium_ec::{EcError, EcResponseStatus, EcResult, EC_MEMMAP_SIZE};
use crate::util;

// TODO: There's no actual limit. I hope this is enough.
// Should make sure we properly re-allocate more memory if it isn't.
const IN_SIZE: usize = 256;

// TODO: Not sure if reading via cmd is supported.
// Seems to return: INVALID_COMMAND
const READ_DIRECTLY: bool = true;

/// Struct to pass the kernel for reading EC mapped memory
#[repr(C)]
pub struct CrosEcReadMem {
    /// Offset in the memory mapped region
    offset: u32,
    /// Number of bytes to read. 0 means reading a string until and including the NULL-byte
    length: u32,
    /// Response buffer
    out_buffer: [u8; EC_MEMMAP_SIZE as usize],
}

// Must be public for the ioctl macro to generate the function
// And this struct must shadow the struct in the kernel exactly,
// otherwise the ioctl returns ENOTTY.
// In Rust we don't use this to allocate memory, but we use the CrosEcCommandV2
#[repr(C)]
pub struct _CrosEcCommandV2 {
    version: u32,
    command: u32,
    outsize: u32,
    insize: u32,
    result: u32,
    data: [u8; 0],
}
#[repr(C)]
struct CrosEcCommandV2 {
    /// Version of the command (usually 0)
    version: u32,
    /// Command ID
    command: u32,
    /// Size of the request in bytes
    outsize: u32,
    /// Maximum number of bytes to accept. Buffer must be big enough!
    insize: u32,
    /// Response status code
    result: u32,
    /// Buffer to send and receive data
    data: [u8; IN_SIZE],
}

pub const DEV_PATH: &str = "/dev/cros_ec";

lazy_static! {
    static ref CROS_EC_FD: Arc<Mutex<Option<std::fs::File>>> = Arc::new(Mutex::new(None));
}

const CROS_EC_IOC_MAGIC: u8 = 0xEC;
ioctl_readwrite!(cros_ec_cmd, CROS_EC_IOC_MAGIC, 0, _CrosEcCommandV2);
ioctl_readwrite!(cros_ec_mem, CROS_EC_IOC_MAGIC, 1, CrosEcReadMem);
// TODO: Implement polling and re-try mechanism
//ioctl_none!(cros_ec_eventmask, CROS_EC_IOC_MAGIC, 2);

fn get_fildes() -> i32 {
    let fd = CROS_EC_FD.lock().unwrap();
    fd.as_ref().unwrap().as_raw_fd()
}

// TODO: Also de-init
fn init() {
    let mut device = CROS_EC_FD.lock().unwrap();
    if (*device).is_some() {
        return;
    }
    match std::fs::File::open(DEV_PATH) {
        Err(why) => println!("Failed to open {}. Because: {:?}", DEV_PATH, why),
        Ok(file) => *device = Some(file),
    };
    // 2. Read max 80 bytes and check if equal to "1.0.0"
    // 3. Make sure it's v2
    // 4. Read memory EC_MEMMAP_ID and check if it has "EC"
}

/// Parameters for command to read memory map
#[repr(C)]
struct EcParamsReadMemMap {
    /// Offset in memory map
    offset: u8,
    /// How many bytes to read
    size: u8,
}

pub fn read_memory(offset: u16, length: u16) -> EcResult<Vec<u8>> {
    if READ_DIRECTLY {
        read_mem_directly(offset, length)
    } else {
        // TODO: Could fallback automatically to reading via command
        read_mem_via_cmd(offset, length)
    }
}

// TODO: Doesn't seem to work
fn read_mem_via_cmd(offset: u16, length: u16) -> EcResult<Vec<u8>> {
    println!(
        "Trying to read via cmd. Offset: {}, length: {}",
        offset, length
    );
    init();

    let cmd = EcParamsReadMemMap {
        offset: offset as u8,
        size: length as u8,
    };
    let data: &[u8] = unsafe { util::any_as_u8_slice(&cmd) };
    send_command(EcCommands::ReadMemMap as u16, 0, data)
}

fn read_mem_directly(offset: u16, length: u16) -> EcResult<Vec<u8>> {
    init();

    let mut data = CrosEcReadMem {
        offset: offset as u32,
        length: length as u32,
        out_buffer: [0; EC_MEMMAP_SIZE as usize],
    };
    unsafe {
        // TODO: Check result
        let _result = cros_ec_mem(get_fildes(), &mut data).unwrap();
    }
    Ok(data.out_buffer[0..length as usize].to_vec())
}

// TODO: Clean this up
pub fn send_command(command: u16, command_version: u8, data: &[u8]) -> EcResult<Vec<u8>> {
    init();

    let size = std::cmp::min(IN_SIZE, data.len());

    let mut cmd = CrosEcCommandV2 {
        version: command_version as u32,
        command: command as u32,
        outsize: size as u32,
        insize: IN_SIZE as u32,
        result: 0xFF,
        // TODO: There is no max length!!
        // ec-tool handles this by having the out-len as a parameter
        // I don't want to do this...
        // Probably I should find out how much each command is expected to return and dynamically allocate that
        data: [0; IN_SIZE],
    };

    cmd.data[0..size].copy_from_slice(data);
    let cmd_ptr = &mut cmd as *mut _ as *mut _CrosEcCommandV2;

    unsafe {
        let result = cros_ec_cmd(get_fildes(), cmd_ptr);
        let status: Option<EcResponseStatus> = FromPrimitive::from_u32(cmd.result);
        match &status {
            None => return Err(EcError::UnknownResponseCode(cmd.result)),
            Some(EcResponseStatus::Success) => {}
            Some(status) => return Err(EcError::Response(*status)),
        }

        match result {
            Ok(result) => {
                let result_size = result as usize; // How many bytes were returned
                let result_data = &cmd.data[0..result_size];

                Ok(result_data.to_vec())
            }
            Err(err) => Err(EcError::DeviceError(format!(
                "ioctl to send command to EC failed with {:?}",
                err
            ))),
        }
    }
}
