//! Interact with the Chrome EC controller firmware.
//!
//! It's used on Chromebooks as well as non-Chromebook Framework laptops.
//!
//! Currently three drivers are supported:
//!
//! - `cros_ec` - It uses the `cros_ec` kernel module in Linux
//! - `portio` - It uses raw port I/O. This works on UEFI and on Linux if the system isn't in lockdown mode (SecureBoot disabled).
//! - `windows` - It uses [DHowett's Windows driver](https://github.com/DHowett/FrameworkWindowsUtils)

use crate::os_specific;
use crate::smbios;
#[cfg(feature = "uefi")]
use crate::uefi::shell_get_execution_break_flag;
use crate::util::assert_win_len;

use num_derive::FromPrimitive;

pub mod command;
pub mod commands;
#[cfg(feature = "cros_ec_driver")]
mod cros_ec;
pub mod input_deck;
mod portio;
mod portio_mec;
#[cfg(feature = "win_driver")]
mod windows;

use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;
use num_traits::FromPrimitive;

pub use command::EcRequestRaw;
use commands::*;

use self::command::EcCommands;
use self::input_deck::InputDeckStatus;

/// Total size of EC memory mapped region
const EC_MEMMAP_SIZE: u16 = 0xFF;

/// Offset in mapped memory where there are two magic bytes
/// representing 'EC' in ASCII (0x20 == 'E', 0x21 == 'C')
const EC_MEMMAP_ID: u16 = 0x20;

#[derive(PartialEq)]
enum MecFlashNotify {
    //Start = 0x01,
    Finished = 0x02,
    FlashPd = 0x11,
}

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

#[derive(Clone)]
pub struct CrosEc {
    driver: CrosEcDriverType,
}

impl Default for CrosEc {
    fn default() -> Self {
        Self::new()
    }
}

/// Find out which drivers are available
///
/// Depending on the availability we choose the first one as default
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
        debug!("Chromium EC Driver: {:?}", available_drivers()[0]);
        CrosEc {
            driver: available_drivers()[0],
        }
    }

    pub fn with(driver: CrosEcDriverType) -> Option<CrosEc> {
        if !available_drivers().contains(&driver) {
            return None;
        }
        debug!("Chromium EC Driver: {:?}", driver);
        Some(CrosEc { driver })
    }

    /// Lock bus to PD controller in the beginning of flashing
    /// TODO: Perhaps I could return a struct that will lock the bus again in its destructor
    pub fn lock_pd_bus(&self, lock: bool) -> EcResult<()> {
        let lock = if lock {
            MecFlashNotify::FlashPd
        } else {
            MecFlashNotify::Finished
        } as u8;
        match self.send_command(EcCommands::FlashNotified as u16, 0, &[lock]) {
            Ok(vec) if !vec.is_empty() => Err(EcError::DeviceError(
                "Didn't expect a response!".to_string(),
            )),
            Ok(_) => Ok(()),
            Err(err) => Err(err),
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

    pub fn cmd_version_supported(&self, cmd: u16, version: u8) -> EcResult<bool> {
        let res = EcRequestGetCmdVersionsV1 { cmd: cmd.into() }.send_command(self);
        let mask = if let Ok(res) = res {
            res.version_mask
        } else {
            let res = EcRequestGetCmdVersionsV0 { cmd: cmd as u8 }.send_command(self)?;
            res.version_mask
        };

        Ok(mask & (1 << version) > 0)
    }

    pub fn dump_mem_region(&self) -> Option<Vec<u8>> {
        // Crashes on Linux cros_ec driver if we read the last byte
        self.read_memory(0x00, EC_MEMMAP_SIZE - 1)
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

    /// Get current status of Framework Laptop's microphone and camera privacy switches
    /// [true = device enabled/connected, false = device disabled]
    pub fn get_privacy_info(&self) -> EcResult<(bool, bool)> {
        let status = EcRequestPrivacySwitches {}.send_command(self)?;

        Ok((status.microphone == 1, status.camera == 1))
    }

    pub fn set_charge_limit(&self, min: u8, max: u8) -> EcResult<()> {
        // Sending bytes manually because the Set command, as opposed to the Get command,
        // does not return any data
        let limits = &[ChargeLimitControlModes::Set as u8, max, min];
        let data = self.send_command(EcCommands::ChargeLimitControl as u16, 0, limits)?;

        assert_win_len(data.len(), 0);

        Ok(())
    }

    /// Get charge limit in percent (min, max)
    pub fn get_charge_limit(&self) -> EcResult<(u8, u8)> {
        let limits = EcRequestChargeLimitControl {
            modes: ChargeLimitControlModes::Get as u8,
            max_percentage: 0xFF,
            min_percentage: 0xFF,
        }
        .send_command(self)?;

        debug!(
            "Min Raw: {}, Max Raw: {}",
            limits.min_percentage, limits.max_percentage
        );

        Ok((limits.min_percentage, limits.max_percentage))
    }

    pub fn set_fp_led_level(&self, level: FpLedBrightnessLevel) -> EcResult<()> {
        // Sending bytes manually because the Set command, as opposed to the Get command,
        // does not return any data
        let limits = &[level as u8, 0x00];
        let data = self.send_command(EcCommands::FpLedLevelControl as u16, 0, limits)?;

        assert_win_len(data.len(), 0);

        Ok(())
    }

    /// Get fingerprint led brightness level
    pub fn get_fp_led_level(&self) -> EcResult<u8> {
        let res = EcRequestFpLedLevelControl {
            set_level: 0xFF,
            get_level: 0xFF,
        }
        .send_command(self)?;

        debug!("Level Raw: {}", res.level);

        Ok(res.level)
    }

    /// Get the intrusion switch status (whether the chassis is open or not)
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

    pub fn get_input_deck_status(&self) -> EcResult<InputDeckStatus> {
        let status = EcRequestDeckState {
            mode: DeckStateMode::ReadOnly,
        }
        .send_command(self)?;

        Ok(InputDeckStatus::from(status))
    }

    pub fn set_input_deck_mode(&self, mode: DeckStateMode) -> EcResult<InputDeckStatus> {
        let status = EcRequestDeckState { mode }.send_command(self)?;

        Ok(InputDeckStatus::from(status))
    }

    /// Change the keyboard baclight brightness
    ///
    /// # Arguments
    /// * `percent` - An integer from 0 to 100. 0 being off, 100 being full brightness
    pub fn set_keyboard_backlight(&self, percent: u8) {
        debug_assert!(percent <= 100);
        let res = EcRequestPwmSetKeyboardBacklight { percent }.send_command(self);
        debug_assert!(res.is_ok());
    }

    /// Check the current brightness of the keyboard backlight
    ///
    pub fn get_keyboard_backlight(&self) -> EcResult<u8> {
        let kblight = EcRequestPwmGetKeyboardBacklight {}.send_command(self)?;

        // The enabled field is deprecated and must always be 1
        debug_assert_eq!(kblight.enabled, 1);
        if !kblight.enabled == 0 {
            println!("Should always be enabled, even if OFF");
        }

        Ok(kblight.percent)
    }

    /// Requests recent console output from EC and constantly asks for more
    /// Prints the output and returns it when an error is encountered
    pub fn console_read(&self) -> EcResult<String> {
        let mut console = String::new();
        let mut cmd = EcRequestConsoleRead {
            subcmd: ConsoleReadSubCommand::ConsoleReadRecent as u8,
        };

        EcRequestConsoleSnapshot {}.send_command(self)?;
        loop {
            match cmd.send_command_vec(self) {
                Ok(data) => {
                    // EC Buffer is empty. We can wait a bit and see if there's more
                    // Can't run it too quickly, otherwise the commands might fail
                    if data.is_empty() {
                        trace!("Empty EC response");
                        println!("---");
                        os_specific::sleep(1_000_000); // 1s
                    }

                    let utf8 = std::str::from_utf8(&data).unwrap();
                    let ascii = utf8
                        .replace(|c: char| !c.is_ascii(), "")
                        .replace(|c: char| c == '\0', "");

                    print!("{}", ascii);
                    console.push_str(ascii.as_str());
                }
                Err(err) => {
                    println!("Err: {:?}", err);
                    return Ok(console);
                    //return Err(err)
                }
            };
            cmd.subcmd = ConsoleReadSubCommand::ConsoleReadNext as u8;

            // Need to explicitly handle CTRL-C termination on UEFI Shell
            #[cfg(feature = "uefi")]
            if shell_get_execution_break_flag() {
                return Ok(console);
            }
        }
    }

    pub fn console_read_one(&self) -> EcResult<String> {
        EcRequestConsoleSnapshot {}.send_command(self)?;
        let data = EcRequestConsoleRead {
            subcmd: ConsoleReadSubCommand::ConsoleReadRecent as u8,
        }
        .send_command_vec(self)?;
        let utf8 = std::str::from_utf8(&data).unwrap();
        let ascii = utf8
            .replace(|c: char| !c.is_ascii(), "")
            .replace(|c: char| c == '\0', "");
        Ok(ascii)
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

        debug!("read_memory(offset={:#X}, size={:#X})", offset, length);
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
        debug!(
            "send_command(command={:X?}, ver={:?}, data_len={:?})",
            <EcCommands as FromPrimitive>::from_u16(command),
            command_version,
            data.len()
        );

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
            error!("EC Response Code: {:?}", status);
        }
        Err(EcError::UnknownResponseCode(code)) => {
            error!("Invalid response code from EC command: {:X}", code);
        }
        Err(EcError::DeviceError(str)) => {
            error!("Failed to communicate with EC. Reason: {:?}", str);
        }
    }
}

/// Print the error and turn Result into Option
///
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
