use num::FromPrimitive;
use std::sync::{Arc, Mutex};
/// Implementation to talk to DHowett's Windows Chrome EC driver
#[allow(unused_imports)]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::{
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::*},
    },
};

use crate::chromium_ec::protocol::HEADER_LEN;
use crate::chromium_ec::EC_MEMMAP_SIZE;
use crate::chromium_ec::{EcError, EcResponseStatus, EcResult};
use crate::smbios;
use crate::util::Platform;

// Create a wrapper around HANDLE to mark it as Send.
// I'm not sure, but I think it's safe to do that for this type of HANDL.
#[derive(Copy, Clone)]
struct DevHandle(HANDLE);
unsafe impl Send for DevHandle {}

lazy_static! {
    static ref DEVICE: Arc<Mutex<Option<DevHandle>>> = Arc::new(Mutex::new(None));
}

fn init() -> bool {
    let mut device = DEVICE.lock().unwrap();
    if (*device).is_some() {
        return true;
    }

    let path = w!(r"\\.\GLOBALROOT\Device\CrosEC");
    let res = unsafe {
        CreateFileW(
            path,
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    };
    let handle = match res {
        Ok(h) => h,
        Err(err) => {
            let platform = smbios::get_platform();
            match platform {
                Some(platform @ Platform::IntelGen11)
                | Some(platform @ Platform::IntelGen12)
                | Some(platform @ Platform::IntelGen13) => {
                    println!("The windows driver is not enabled on {:?}.", platform);
                    println!("Please stay tuned for future BIOS and driver updates.");
                    println!();
                }
                Some(Platform::IntelCoreUltra1) => {
                    println!("The windows driver has been enabled since BIOS 3.06.");
                    println!("Please install the latest BIOS and drivers");
                    println!();
                }
                Some(Platform::Framework13Amd7080) => {
                    println!("The windows driver has been enabled since BIOS 3.16.");
                    println!("Please install the latest BIOS and drivers");
                    println!();
                }
                Some(Platform::Framework16Amd7080) => {
                    println!("The windows driver has been enabled since BIOS 3.06.");
                    println!("Please install the latest BIOS and drivers");
                    println!();
                }
                _ => (),
            }

            error!("Failed to find Windows driver. {:?}", err);
            return false;
        }
    };

    *device = Some(DevHandle(handle));
    true
}

pub fn read_memory(offset: u16, length: u16) -> EcResult<Vec<u8>> {
    if !init() {
        return Err(EcError::DeviceError("Failed to initialize".to_string()));
    }
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
        let device = if let Some(device) = *device {
            device
        } else {
            return EcResult::Err(EcError::DeviceError("No EC device".to_string()));
        };
        DeviceIoControl(
            device.0,
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
    Ok(output.to_vec())
}

pub fn send_command(command: u16, command_version: u8, data: &[u8]) -> EcResult<Vec<u8>> {
    init();

    let mut cmd = CrosEcCommand {
        version: command_version as u32,
        command: command as u32,
        outsize: data.len() as u32,
        insize: (CROSEC_CMD_MAX_REQUEST - HEADER_LEN) as u32,
        result: 0xFF,
        buffer: [0_u8; CROSEC_CMD_MAX_REQUEST],
    };
    cmd.buffer[0..data.len()].clone_from_slice(data);

    let buf_size = std::mem::size_of::<CrosEcCommand>();
    // Must keep 8 bytes of space for the EC command request/response headers
    let cmd_len = buf_size - HEADER_LEN;
    let out_len = buf_size - HEADER_LEN;
    let const_ptr = &mut cmd as *const _ as *const ::core::ffi::c_void;
    let mut_ptr = &mut cmd as *mut _ as *mut ::core::ffi::c_void;

    let mut returned: u32 = 0;

    unsafe {
        let device = DEVICE.lock().unwrap();
        let device = if let Some(device) = *device {
            device
        } else {
            return EcResult::Err(EcError::DeviceError("No EC device".to_string()));
        };
        DeviceIoControl(
            device.0,
            IOCTL_CROSEC_XCMD,
            Some(const_ptr),
            cmd_len.try_into().unwrap(),
            Some(mut_ptr),
            out_len.try_into().unwrap(),
            Some(&mut returned as *mut u32),
            None,
        )
        .unwrap();
    }

    match FromPrimitive::from_u32(cmd.result) {
        None => return Err(EcError::UnknownResponseCode(cmd.result)),
        Some(EcResponseStatus::Success) => {}
        Some(status) => return Err(EcError::Response(status)),
    }

    // TODO: Figure out why that's sometimes bigger
    let end = std::cmp::min(returned, CROSEC_CMD_MAX_REQUEST as u32);

    let out_buffer = &cmd.buffer[..(end as usize)];
    Ok(out_buffer.to_vec())
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
