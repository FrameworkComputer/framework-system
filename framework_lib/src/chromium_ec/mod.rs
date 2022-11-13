use crate::smbios;
use crate::util;

#[cfg(not(feature = "uefi"))]
use num_derive::FromPrimitive;

#[cfg(feature = "cros_ec_driver")]
mod cros_ec;
mod portio;
#[cfg(feature = "win_driver")]
mod windows;

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

/// Total size of EC memory mapped region
const EC_MEMMAP_SIZE: u16 = 255;

const EC_CMD_GET_BUILD_INFO: u16 = 0x04;

// Framework Specific commands

/// Configure the behavior of the flash notify
const EC_CMD_FLASH_NOTIFIED: u16 = 0x3E01;
const EC_CMD_PRIVACY_SWITCHES_CHECK_MODE: u16 = 0x3E14; /* Get information about current state of privacy switches */

#[repr(C, packed)]
struct EcResponsePrivacySwitches {
    microphone: u8,
    camera: u8,
}

/// Command to read data from EC memory map
#[cfg(feature = "cros_ec_driver")]
const EC_CMD_READ_MEMMAP: u16 = 0x0007;

const EC_MEMMAP_ID: u16 = 0x20; /* 0x20 == 'E', 0x21 == 'C' */

/// Response codes returned by commands
#[cfg_attr(not(feature = "uefi"), derive(FromPrimitive))]
#[derive(Debug)]
pub enum EcResponseStatus {
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

pub trait CrosEcDriver {
    fn read_memory(&self, offset: u16, length: u16) -> Option<Vec<u8>>;
    fn send_command(&self, command: u16, command_version: u8, data: &[u8]) -> Option<Vec<u8>>;
}

pub struct CrosEc {
    driver: CrosEcDriverType,
}

impl Default for CrosEc {
    fn default() -> Self {
        Self::new()
    }
}

impl CrosEc {
    pub fn new() -> CrosEc {
        CrosEc {
            driver: CrosEcDriverType::CrosEc,
        }
    }

    pub fn check_mem_magic(&self) -> Option<()> {
        match self.read_memory(EC_MEMMAP_ID, 2) {
            Some(ec_id) => {
                if ec_id.len() != 2 {
                    println!("  Unexpected length returned: {:?}", ec_id.len());
                    return None;
                }
                if ec_id[0] != b'E' || ec_id[1] != b'C' {
                    println!("  This machine doesn't look like it has a Framework EC");
                    None
                } else {
                    println!("  Verified that Framework EC is present!");
                    Some(())
                }
            }
            None => {
                println!("  Failed to read EC ID from memory map");
                None
            }
        }
    }

    /*
     * Get build information
     *
     * Response is null-terminated string.
     */
    pub fn version_info(&self) -> Option<String> {
        let data = self.send_command(EC_CMD_GET_BUILD_INFO, 0, &[])?;
        Some(
            std::str::from_utf8(&data)
                .ok()?
                .trim_end_matches(char::from(0))
                .to_string(),
        )
    }

    pub fn flash_version(&self) -> Option<(String, String, EcCurrentImage)> {
        // Unlock SPI
        // TODO: Lock flash again again
        let params = FlashNotifiedParams { flags: 0 };
        let params: &[u8] = unsafe { util::any_as_u8_slice(&params) };
        let _data = self.send_command(EC_CMD_FLASH_NOTIFIED, 0, params);

        let data = self.send_command(EC_CMD_GET_VERSION, 0, &[])?;
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

    pub fn privacy_info(&self) -> Option<(bool, bool)> {
        let data = self.send_command(EC_CMD_PRIVACY_SWITCHES_CHECK_MODE, 0, &[])?;
        // TODO: Rust complains that when accessing this struct, we're reading
        // from unaligned pointers. How can I fix this? Maybe create another struct to shadow it,
        // which isn't packed. And copy the data to there.
        let status: EcResponsePrivacySwitches =
            unsafe { std::ptr::read(data.as_ptr() as *const _) };

        println!(
            "Microphone privacy switch: {}",
            if status.microphone == 1 {
                "Open"
            } else {
                "Closed"
            }
        );
        println!(
            "Camera privacy switch:     {}",
            if status.camera == 1 { "Open" } else { "Closed" }
        );

        Some((status.microphone == 1, status.camera == 1))
    }

    pub fn get_intrusion_status(&self) -> Option<IntrusionStatus> {
        let data = self.send_command(EC_CMD_CHASSIS_OPEN_CHECK, 0, &[])?;
        let status: EcResponseChassisOpenCheck = unsafe { std::ptr::read(data.as_ptr() as *const _) };

        let data = self.send_command(EC_CMD_CHASSIS_INTRUSION, 0, &[])?;
        let intrusion: EcResponseChassisIntrusionControl =
            unsafe { std::ptr::read(data.as_ptr() as *const _) };

        Some(IntrusionStatus {
            currently_open: status.status == 1,
            coin_cell_ever_removed: intrusion.coin_batt_ever_remove == 1,
            ever_opened: intrusion.chassis_ever_opened == 1,
            total_opened: intrusion.total_open_count,
            vtr_open_count: intrusion.vtr_open_count,
        })
    }
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug)]
pub enum CrosEcDriverType {
    Portio,
    CrosEc,
    Windows,
}

impl CrosEcDriver for CrosEc {
    fn read_memory(&self, offset: u16, length: u16) -> Option<Vec<u8>> {
        if !smbios::is_framework() {
            return None;
        }
        // TODO: Choose implementation based on support and/or configuration
        match self.driver {
            CrosEcDriverType::Portio => portio::read_memory(offset, length),
            #[cfg(feature = "win_driver")]
            CrosEcDriverType::Windows => windows::read_memory(offset, length),
            #[cfg(feature = "cros_ec_driver")]
            CrosEcDriverType::CrosEc => cros_ec::read_memory(offset, length),
            _ => None,
        }
    }

    fn send_command(&self, command: u16, command_version: u8, data: &[u8]) -> Option<Vec<u8>> {
        if util::is_debug() {
            println!(
                "send_command_lpc_v3(command={:?}, ver={:?}, data_len={:?})",
                command,
                command_version,
                data.len()
            );
        }

        if !smbios::is_framework() {
            return None;
        }

        match self.driver {
            CrosEcDriverType::Portio => portio::send_command(command, command_version, data),
            #[cfg(feature = "win_driver")]
            CrosEcDriverType::Windows => windows::send_command(command, command_version, data),
            #[cfg(feature = "cros_ec_driver")]
            CrosEcDriverType::CrosEc => cros_ec::send_command(command, command_version, data),
            _ => None,
        }
    }
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

/// Command to get information about the current chassis open/close status
const EC_CMD_CHASSIS_OPEN_CHECK: u16 = 0x3E0F;

#[repr(C, packed)]
struct EcResponseChassisOpenCheck {
    status: u8,
}

const EC_CMD_CHASSIS_INTRUSION: u16 = 0x3E09;

#[repr(C, packed)]
struct EcResponseChassisIntrusionControl {
    chassis_ever_opened: u8,
    coin_batt_ever_remove: u8,
    total_open_count: u8,
    vtr_open_count: u8,
}

pub struct IntrusionStatus {
    /// Whether the chassis is currently open
    pub currently_open: bool,
    /// If the coin cell battery has ever been removed
    pub coin_cell_ever_removed: bool,
    /// Whether the chassis has ever been opened
    /// TODO: Is this the same as total_opened > 0?
    pub ever_opened: bool,
    /// How often the chassis has been opened in total
    pub total_opened: u8,
    /// How often the chassis was opened while off
    /// We can tell because opening the chassis, even when off, leaves a sticky bit that the EC can read when it powers back on.
    /// That means we only know if it was opened at least once, while off, not how many times.
    pub vtr_open_count: u8,
}
