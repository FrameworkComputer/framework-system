use num::FromPrimitive;
use std::sync::{Arc, Mutex};
/// Implementation to talk to DHowett's Windows Chrome EC driver
#[allow(unused_imports)]
use windows::{
    core::*,
    w,
    Win32::Foundation::*,
    Win32::{
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::*},
    },
};

use crate::chromium_ec::EcResponseStatus;
use crate::chromium_ec::EC_MEMMAP_SIZE;

lazy_static! {
    static ref DEVICE: Arc<Mutex<Option<HANDLE>>> = Arc::new(Mutex::new(None));
}

fn init() {
    let mut device = DEVICE.lock().unwrap();
    if (*device).is_some() {
        return;
    }

    let path = w!(r"\\.\GLOBALROOT\Device\CrosEC");
    unsafe {
        *device = Some(
            CreateFileW(
                path,
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
            .unwrap(),
        );
    }
}

pub fn read_memory(offset: u16, length: u16) -> Option<Vec<u8>> {
    init();
    let mut rm = CrosEcReadMem {
        offset: offset as u32,
        bytes: length as u32,
        buffer: [0_u8; EC_MEMMAP_SIZE as usize],
    };

    let const_ptr = &mut rm as *const _ as *const ::core::ffi::c_void;
    let mut_ptr = &mut rm as *mut _ as *mut ::core::ffi::c_void;
    let ptr_size = std::mem::size_of::<CrosEcReadMem>() as u32;
    let retb: u32 = 0;
    unsafe {
        let device = DEVICE.lock().unwrap();
        DeviceIoControl(
            *device,
            IOCTL_CROSEC_RDMEM,
            Some(const_ptr),
            ptr_size,
            Some(mut_ptr),
            ptr_size,
            Some(retb as *mut u32),
            None,
        )
        .unwrap();
    }
    let output = &rm.buffer[..(length as usize)];
    Some(output.to_vec())
}

pub fn send_command(command: u16, command_version: u8, data: &[u8]) -> Option<Vec<u8>> {
    init();

    let mut cmd = CrosEcCommand {
        version: command_version as u32,
        command: command as u32,
        outsize: data.len() as u32,
        insize: CROSEC_CMD_MAX_REQUEST as u32,
        result: 0xFF,
        buffer: [0_u8; 256],
    };
    cmd.buffer[0..data.len()].clone_from_slice(data);

    let size = std::mem::size_of::<CrosEcCommand>();
    let const_ptr = &mut cmd as *const _ as *const ::core::ffi::c_void;
    let mut_ptr = &mut cmd as *mut _ as *mut ::core::ffi::c_void;
    let _ptr_size = std::mem::size_of::<CrosEcCommand>() as u32;

    let mut returned: u32 = 0;

    unsafe {
        let device = DEVICE.lock().unwrap();
        DeviceIoControl(
            *device,
            IOCTL_CROSEC_XCMD,
            Some(const_ptr),
            size.try_into().unwrap(),
            Some(mut_ptr),
            size.try_into().unwrap(),
            Some(&mut returned as *mut u32),
            None,
        );
    }

    match FromPrimitive::from_u32(cmd.result) {
        Some(EcResponseStatus::Success) => {}
        Some(EcResponseStatus::InvalidCommand) => {
            println!("Unsupported Command");
            return None;
        }
        err => panic!(
            "Error: {:?}, command: {}, cmd_ver: {}, data: {:?}",
            err, command, command_version, data
        ),
    }

    let out_buffer = &cmd.buffer[..(returned as usize)];
    Some(out_buffer.to_vec())
}

const CROSEC_CMD_MAX_REQUEST: usize = 0x100;

const FILE_DEVICE_CROS_EMBEDDED_CONTROLLER: u32 = 0x80EC;

const IOCTL_CROSEC_XCMD: u32 = ctl_code(
    FILE_DEVICE_CROS_EMBEDDED_CONTROLLER,
    0x801,
    METHOD_BUFFERED,
    FILE_READ_DATA.0 | FILE_WRITE_DATA.0,
);
const IOCTL_CROSEC_RDMEM: u32 = ctl_code(
    FILE_DEVICE_CROS_EMBEDDED_CONTROLLER,
    0x802,
    METHOD_BUFFERED,
    FILE_READ_ACCESS,
);

/// Shadows CTL_CODE from microsoft headers
const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    ((device_type) << 16) + ((access) << 14) + ((function) << 2) + method
}

#[repr(C)]
struct CrosEcReadMem {
    /// Offset in memory mapped region
    offset: u32,
    /// How many bytes to read
    bytes: u32,
    /// Buffer to receive requested bytes
    buffer: [u8; EC_MEMMAP_SIZE as usize],
}
#[derive(Copy, Clone, Debug)]
#[repr(C)]
struct CrosEcCommand {
    /// Command version. Almost always 0
    version: u32,
    /// Command type
    command: u32,
    /// Size of request in bytes
    outsize: u32,
    /// Maximum response size in bytes
    insize: u32,
    /// Response status code
    result: u32,
    /// Request and response data buffer
    buffer: [u8; CROSEC_CMD_MAX_REQUEST],
}
