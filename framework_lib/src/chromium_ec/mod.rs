use crate::smbios;
use crate::util;

use num_derive::FromPrimitive;

pub mod command;
pub mod commands;
#[cfg(feature = "cros_ec_driver")]
mod cros_ec;
mod portio;
#[cfg(feature = "win_driver")]
mod windows;

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use command::EcRequest;
use commands::*;

use self::command::EcCommands;

/// Total size of EC memory mapped region
const EC_MEMMAP_SIZE: u16 = 255;

// Framework Specific commands

const EC_MEMMAP_ID: u16 = 0x20; /* 0x20 == 'E', 0x21 == 'C' */

pub type EcResult<T> = Result<T, EcError>;

#[derive(Debug, PartialEq)]
pub enum EcError {
    Response(EcResponseStatus),
    UnknownResponseCode(u32),
    // Failed to communicate with the EC
    DeviceError(String),
}

/// Response codes returned by commands
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
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

pub trait CrosEcDriver {
    fn read_memory(&self, offset: u16, length: u16) -> Option<Vec<u8>>;
    fn send_command(&self, command: u16, command_version: u8, data: &[u8]) -> EcResult<Vec<u8>>;
}

pub struct CrosEc {
    driver: CrosEcDriverType,
}

impl Default for CrosEc {
    fn default() -> Self {
        Self::new()
    }
}

// Depending on the availability we choose the first one as default
fn available_drivers() -> Vec<CrosEcDriverType> {
    vec![
        #[cfg(feature = "win_driver")]
        CrosEcDriverType::Windows,
        #[cfg(feature = "cros_ec_driver")]
        CrosEcDriverType::CrosEc,
        #[cfg(not(feature = "windows"))]
        CrosEcDriverType::Portio,
    ]
}

impl CrosEc {
    pub fn new() -> CrosEc {
        CrosEc {
            driver: available_drivers()[0],
        }
    }

    pub fn with(driver: CrosEcDriverType) -> Option<CrosEc> {
        if !available_drivers().contains(&driver) {
            return None;
        }
        Some(CrosEc { driver })
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

    /// Get EC firmware build information
    pub fn version_info(&self) -> EcResult<String> {
        // Response is null-terminated string.
        let data = self.send_command(EcCommands::GetBuildInfo as u16, 0, &[])?;
        Ok(std::str::from_utf8(&data)
            .map_err(|utf8_err| {
                EcError::DeviceError(format!("Failed to decode version: {:?}", utf8_err))
            })?
            .trim_end_matches(char::from(0))
            .to_string())
    }

    pub fn flash_version(&self) -> Option<(String, String, EcCurrentImage)> {
        // Unlock SPI
        // TODO: Lock flash again again
        let _data = EcRequestFlashNotify { flags: 0 }.send_command(self).ok()?;

        let v = EcRequestGetVersion {}.send_command(self).ok()?;

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

    pub fn get_privacy_info(&self) -> EcResult<(bool, bool)> {
        let status = EcRequestPrivacySwitches {}.send_command(self)?;

        Ok((status.microphone == 1, status.camera == 1))
    }

    pub fn get_intrusion_status(&self) -> EcResult<IntrusionStatus> {
        let status = EcRequestChassisOpenCheck {}.send_command(self)?;

        let intrusion = EcRequestChassisIntrusionControl {
            clear_magic: 0,
            clear_chassis_status: 0,
        }
        .send_command(self)?;

        Ok(IntrusionStatus {
            currently_open: status.status == 1,
            coin_cell_ever_removed: intrusion.coin_batt_ever_remove == 1,
            ever_opened: intrusion.chassis_ever_opened == 1,
            total_opened: intrusion.total_open_count,
            vtr_open_count: intrusion.vtr_open_count,
        })
    }

    pub fn set_keyboard_backlight(&self, percent: u8) {
        let res = EcRequestPwmSetKeyboardBacklight { percent }.send_command(self);
        debug_assert!(res.is_ok());
    }

    pub fn get_keyboard_backlight(&self) -> EcResult<u8> {
        let kblight = EcRequestPwmGetKeyboardBacklight {}.send_command(self)?;

        // The enabled field is deprecated and must always be 1
        debug_assert_eq!(kblight.enabled, 1);
        if !kblight.enabled == 0 {
            println!("Should always be enabled, even if OFF");
        }

        Ok(kblight.percent)
    }
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, Copy, PartialEq)]
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

        if util::is_debug() {
            println!("read_memory(offset={:#}, size={:#})", offset, length);
        }
        if offset + length > EC_MEMMAP_SIZE {
            return None;
        }

        // TODO: Change this function to return EcResult instead and print the error only in UI code
        print_err(match self.driver {
            CrosEcDriverType::Portio => portio::read_memory(offset, length),
            #[cfg(feature = "win_driver")]
            CrosEcDriverType::Windows => windows::read_memory(offset, length),
            #[cfg(feature = "cros_ec_driver")]
            CrosEcDriverType::CrosEc => cros_ec::read_memory(offset, length),
            _ => Err(EcError::DeviceError("No EC driver available".to_string())),
        })
    }
    fn send_command(&self, command: u16, command_version: u8, data: &[u8]) -> EcResult<Vec<u8>> {
        if util::is_debug() {
            println!(
                "send_command(command={:?}, ver={:?}, data_len={:?})",
                command,
                command_version,
                data.len()
            );
        }

        if !smbios::is_framework() {
            return Err(EcError::DeviceError("Not a Framework Laptop".to_string()));
        }

        match self.driver {
            CrosEcDriverType::Portio => portio::send_command(command, command_version, data),
            #[cfg(feature = "win_driver")]
            CrosEcDriverType::Windows => windows::send_command(command, command_version, data),
            #[cfg(feature = "cros_ec_driver")]
            CrosEcDriverType::CrosEc => cros_ec::send_command(command, command_version, data),
            _ => Err(EcError::DeviceError("No EC driver available".to_string())),
        }
    }
}

/// Print the error
pub fn print_err_ref<T>(something: &EcResult<T>) {
    match something {
        Ok(_) => {}
        // TODO: Some errors we can handle and retry, like Busy, Timeout, InProgress, ...
        Err(EcError::Response(status)) => {
            println!("Error code returned by EC: {:?}", status);
        }
        Err(EcError::UnknownResponseCode(code)) => {
            println!("Invalid response code from EC command: {}", code);
        }
        Err(EcError::DeviceError(str)) => {
            println!("Failed to communicate with EC. Reason: {}", str);
        }
    }
}

/// Print the error and turn Result into Option
/// TODO: This is here because of refactoring, might want to remove this function
pub fn print_err<T>(something: EcResult<T>) -> Option<T> {
    print_err_ref(&something);
    something.ok()
}

/// Which of the two EC images is currently in-use
#[derive(PartialEq)]
pub enum EcCurrentImage {
    Unknown = 0,
    RO = 1,
    RW = 2,
}

///Framework Specific commands

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
