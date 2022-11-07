use crate::util;

use num_derive::FromPrimitive;
mod cros_ec;
mod portio;
//mod windows;

/// Total size of EC memory mapped region
const EC_MEMMAP_SIZE: u16 = 255;

/// Command to read data from EC memory map
const EC_CMD_READ_MEMMAP: u16 = 0x0007;

/// Response codes returned by commands
#[derive(FromPrimitive, Debug)]
enum EcResponseStatus {
    Success = 0,
    InvalidCommand = 1,
    Error = 2,
    InvalidParameter = 3,
    AccessDenied = 4,
    InvalidResponse = 5,
    InvalidVersion = 6,
    InvalidChecksum = 7,
    /// Accepted, command in progress
    InProgress = 8,
    /// No response available
    Unavailable = 9,
    /// We got a timeout
    Timeout = 10,
    /// Table / data overflow
    Overflow = 11,
    /// Header contains invalid data
    InvalidHeader = 12,
    /// Didn't get the entire request
    RequestTruncated = 13,
    /// Response was too big to handle
    ResponseTooBig = 14,
    /// Communications bus error
    BusError = 15,
    /// Up but too busy.  Should retry
    Busy = 16,
}

#[repr(C, packed)]
struct FlashNotifiedParams {
    flags: u8,
}

/// OS Independent implementation of host to EC communication
/// - [ ] Direct Port I/O (Works on UEFI and Linux without SecureBoot)
/// - [ ] Linux cros_ec driver
/// - [ ] Windows Driver

pub fn read_memory(offset: u16, length: u16) -> Option<Vec<u8>> {
    // TODO: Choose implementation based on support and/or configuration
    match 0 {
        0 => portio::read_memory(offset, length),
        //1 => windows::read_memory(offset, length),
        _ => cros_ec::read_memory(offset, length),
    }
}

pub fn send_command(command: u16, command_version: u8, data: &[u8]) -> Option<Vec<u8>> {
    if util::is_debug() {
        println!(
            "send_command_lpc_v3(command={:?}, ver={:?}, data_len={:?})",
            command,
            command_version,
            data.len()
        );
    }

    // TODO: Choose implementation based on support and/or configuration

    match 0 {
        0 => portio::send_command(command, command_version, data),
        //1 => windows::send_command(command, command_version, data),
        _ => cros_ec::send_command(command, command_version, data),
    }
}

/*
 * Get build information
 *
 * Response is null-terminated string.
 */
const EC_CMD_GET_BUILD_INFO: u16 = 0x04;
pub fn version_info() -> Option<String> {
    println!("Trying to get version");
    let data = send_command(EC_CMD_GET_BUILD_INFO, 0, &[])?;
    Some(
        std::str::from_utf8(&data)
            .ok()?
            .trim_end_matches(char::from(0))
            .to_string(),
    )
}

/// Command ID to get the EC FW version
const EC_CMD_GET_VERSION: u16 = 0x02;

/// Which of the two EC images is currently in-use
#[derive(PartialEq)]
pub enum EcCurrentImage {
    Unknown = 0,
    RO = 1,
    RW = 2,
}

#[repr(C, packed)]
struct EcResponseGetVersion {
    /// Null-terminated version of the RO firmware
    version_string_ro: [u8; 32],
    /// Null-terminated version of the RW firmware
    version_string_rw: [u8; 32],
    /// Used to be the RW-B string
    reserved: [u8; 32],
    /// Which EC image is currently in-use. See enum EcCurrentImage
    current_image: u32,
}

///Framework Specific commands

///Configure the behavior of the flash notify
const EC_CMD_FLASH_NOTIFIED: u16 = 0x3E01;

pub fn flash_version() -> Option<(String, String, EcCurrentImage)> {
    // Unlock SPI
    // TODO: Lock flash again again
    let params = FlashNotifiedParams { flags: 0 };
    let params: &[u8] = unsafe { util::any_as_u8_slice(&params) };
    let _data = send_command(EC_CMD_FLASH_NOTIFIED, 0, params);

    let data = send_command(EC_CMD_GET_VERSION, 0, &[])?;
    let v: EcResponseGetVersion = unsafe {
        // TODO: Why does transmute not work?
        //std::mem::transmute(bytes.as_ptr())
        std::ptr::read(data.as_ptr() as *const _)
    };

    let curr = match v.current_image {
        1 => EcCurrentImage::RO,
        2 => EcCurrentImage::RW,
        _ => EcCurrentImage::Unknown,
    };

    Some((
        std::str::from_utf8(&v.version_string_rw)
            .ok()?
            .trim_end_matches(char::from(0))
            .to_string(),
        std::str::from_utf8(&v.version_string_ro)
            .ok()?
            .trim_end_matches(char::from(0))
            .to_string(),
        curr,
    ))
}
