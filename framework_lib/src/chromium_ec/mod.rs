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

use log::Level;
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

// 512K
pub const EC_FLASH_SIZE: usize = 512 * 1024;

/// Total size of EC memory mapped region
const EC_MEMMAP_SIZE: u16 = 0xFF;

/// Offset in mapped memory where there are two magic bytes
/// representing 'EC' in ASCII (0x20 == 'E', 0x21 == 'C')
const EC_MEMMAP_ID: u16 = 0x20;

const FLASH_BASE: u32 = 0x0; // 0x80000
const FLASH_RO_BASE: u32 = 0x0;
const FLASH_RO_SIZE: u32 = 0x3C000;
const FLASH_RW_BASE: u32 = 0x40000;
const FLASH_RW_SIZE: u32 = 0x39000;
const FLASH_PROGRAM_OFFSET: u32 = 0x1000;

#[derive(PartialEq)]
pub enum MecFlashNotify {
    AccessSpi = 0x00,
    FirmwareStart = 0x01,
    FirmwareDone = 0x02,
    AccessSpiDone = 0x03,
    FlashPd = 0x16,
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
            MecFlashNotify::FirmwareDone
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
                    error!("  Unexpected length returned: {:?}", ec_id.len());
                    return None;
                }
                if ec_id[0] != b'E' || ec_id[1] != b'C' {
                    error!("  This machine doesn't look like it has a Framework EC");
                    None
                } else {
                    println!("  Verified that Framework EC is present!");
                    Some(())
                }
            }
            None => {
                error!("  Failed to read EC ID from memory map");
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
            error!("Should always be enabled, even if OFF");
        }

        Ok(kblight.percent)
    }

    /// Overwrite RO and RW regions of EC flash
    /// | Start | End   | Size  | Region    |
    /// | 00000 | 3BFFF | 3C000 | RO Region |
    /// | 3C000 | 3FFFF | 04000 | Preserved |
    /// | 40000 | 3C000 | 39000 | RO Region |
    /// | 79000 | 79FFF | 07000 | Preserved |
    pub fn reflash(&self, data: &[u8]) -> EcResult<()> {
        let mut _flash_bin: Vec<u8> = Vec::with_capacity(EC_FLASH_SIZE);
        println!("Unlocking flash");
        self.flash_notify(MecFlashNotify::AccessSpi)?;
        self.flash_notify(MecFlashNotify::FirmwareStart)?;

        //println!("Erasing RO region");
        //self.erase_ec_flash(FLASH_BASE + FLASH_RO_BASE, FLASH_RO_SIZE)?;
        println!("Erasing RW region");
        self.erase_ec_flash(FLASH_BASE + FLASH_RW_BASE, FLASH_RW_SIZE)?;

        let ro_data = &data[FLASH_RO_BASE as usize..(FLASH_RO_BASE + FLASH_RO_SIZE) as usize];
        //println!("Writing RO region");
        //self.write_ec_flash(FLASH_BASE + FLASH_RO_BASE, ro_data);

        let rw_data = &data[FLASH_RW_BASE as usize..(FLASH_RW_BASE + FLASH_RW_SIZE) as usize];
        println!("Writing RW region");
        self.write_ec_flash(FLASH_BASE + FLASH_RW_BASE, rw_data)?;

        println!("Verifying");
        let flash_ro_data = self.read_ec_flash(FLASH_BASE + FLASH_RO_BASE, FLASH_RO_SIZE)?;
        if ro_data == flash_ro_data {
            println!("RO verify success");
        } else {
            println!("RO verify fail");
        }
        let flash_rw_data = self.read_ec_flash(FLASH_BASE + FLASH_RW_BASE, FLASH_RW_SIZE)?;
        if rw_data == flash_rw_data {
            println!("RW verify success");
        } else {
            println!("RW verify fail");
        }

        println!("Locking flash");
        self.flash_notify(MecFlashNotify::AccessSpiDone)?;
        self.flash_notify(MecFlashNotify::FirmwareDone)?;

        println!("Flashing EC done. You can reboot the EC now");

        Ok(())
    }

    /// Write a big section of EC flash. Must be unlocked already
    fn write_ec_flash(&self, addr: u32, data: &[u8]) -> EcResult<()> {
        let info = EcRequestFlashInfo {}.send_command(self)?;
        println!("Flash info: {:?}", info);
        //let chunk_size = ((0x80 / info.write_ideal_size) * info.write_ideal_size) as usize;
        let chunk_size = 0x80;

        let chunks = data.len() / chunk_size;
        for chunk_no in 0..chunks {
            let offset = chunk_no * chunk_size;
            // Current chunk might be smaller if it's the last
            let cur_chunk_size = std::cmp::min(chunk_size, data.len() - chunk_no * chunk_size);

            if chunk_no % 100 == 0 {
                println!();
                print!(
                    "Writing chunk {:>4}/{:>4} ({:>6}/{:>6}): X",
                    chunk_no,
                    chunks,
                    offset,
                    cur_chunk_size * chunks
                );
            } else {
                print!("X");
            }

            let chunk = &data[offset..offset + cur_chunk_size];
            let res = self.write_ec_flash_chunk(addr + offset as u32, chunk);
            if let Err(err) = res {
                println!("  Failed to write chunk: {:?}", err);
                return Err(err);
            }
        }
        println!();

        Ok(())
    }

    fn write_ec_flash_chunk(&self, offset: u32, data: &[u8]) -> EcResult<()> {
        assert!(data.len() <= 0x80); // TODO: I think this is EC_LPC_HOST_PACKET_SIZE - size_of::<EcHostResponse>()
        EcRequestFlashWrite {
            offset,
            size: data.len() as u32,
            data: [],
        }
        .send_command_extra(self, data)
    }

    fn erase_ec_flash(&self, offset: u32, size: u32) -> EcResult<()> {
        EcRequestFlashErase { offset, size }.send_command(self)
    }

    pub fn flash_notify(&self, flag: MecFlashNotify) -> EcResult<()> {
        let _data = EcRequestFlashNotify { flags: flag as u8 }.send_command(self)?;
        Ok(())
    }

    /// Read a section of EC flash
    /// Maximum size to read is 0x80/128 bytes at a time
    /// Must `self.flash_notify(MecFlashNotify::AccessSpi)?;` first, otherwise it'll return all 0s
    pub fn read_ec_flash_chunk(&self, offset: u32, size: u32) -> EcResult<Vec<u8>> {
        // TODO: Windows asserts
        //assert!(size <= 0x80); // TODO: I think this is EC_LPC_HOST_PACKET_SIZE - size_of::<EcHostResponse>()
        let data = EcRequestFlashRead { offset, size }.send_command_vec(self);
        let data = match data {
            Ok(data) => data,
            Err(err) => return Err(err),
        };

        // TODO: Windows asserts because it returns more data
        //debug_assert!(data.len() == size as usize); // Make sure we get back what was requested
        Ok(data[..size as usize].to_vec())
    }

    pub fn read_ec_flash(&self, offset: u32, size: u32) -> EcResult<Vec<u8>> {
        let mut flash_bin: Vec<u8> = Vec::with_capacity(EC_FLASH_SIZE);

        // Read in chunks of size 0x80 or just a single small chunk
        let (chunk_size, chunks) = if size <= 0x80 {
            (size, 1)
        } else {
            (0x80, size / 0x80)
        };
        for chunk_no in 0..chunks {
            #[cfg(feature = "uefi")]
            if shell_get_execution_break_flag() {
                return Err(EcError::DeviceError("Execution interrupted".to_string()));
            }

            let offset = offset + chunk_no * chunk_size;
            let cur_chunk_size = std::cmp::min(chunk_size, size - chunk_no * chunk_size);
            if log_enabled!(Level::Warn) {
                if chunk_no % 10 == 0 {
                    println!();
                    print!(
                        "Reading chunk {:>4}/{:>4} ({:>6}/{:>6}): X",
                        chunk_no,
                        chunks,
                        offset,
                        cur_chunk_size * chunks
                    );
                } else {
                    print!("X");
                }
            }

            let chunk = self.read_ec_flash_chunk(offset, cur_chunk_size);
            match chunk {
                Ok(chunk) => {
                    flash_bin.extend(chunk);
                }
                Err(err) => {
                    error!("  Failed to read chunk: {:?}", err);
                }
            }
            os_specific::sleep(100);
        }

        Ok(flash_bin)
    }

    pub fn get_entire_ec_flash(&self) -> EcResult<Vec<u8>> {
        self.flash_notify(MecFlashNotify::AccessSpi)?;

        let flash_bin = self.read_ec_flash(0, EC_FLASH_SIZE as u32)?;

        self.flash_notify(MecFlashNotify::AccessSpiDone)?;

        Ok(flash_bin)
    }

    pub fn protect_ec_flash(
        &self,
        mask: u32,
        flags: &[FlashProtectFlags],
    ) -> EcResult<EcResponseFlashProtect> {
        EcRequestFlashProtect {
            mask,
            flags: flags.iter().fold(0, |x, y| x + (*y as u32)),
        }
        .send_command(self)
    }

    pub fn test_ec_flash_read(&self) -> EcResult<()> {
        // TODO: Perhaps we could have some more global flag to avoid setting and unsetting that ever time
        self.flash_notify(MecFlashNotify::AccessSpi)?;

        println!("  EC Test");
        println!("    Read first row of flash.");
        // Make sure we can read a full flash row
        let data = self.read_ec_flash(0, 0x80).unwrap();
        if data[0..4] != [0x10, 0x00, 0x00, 0xF7] {
            println!("      INVALID start");
            return Err(EcError::DeviceError("INVALID start".to_string()));
        }
        if !data[4..].iter().all(|x| *x == 0xFF) {
            println!("      INVALID end");
            return Err(EcError::DeviceError("INVALID end".to_string()));
        }
        debug!("Expected 10 00 00 F7 and rest all FF");
        debug!("{:02X?}", data);

        println!("    Read first 16 bytes of firmware.");
        // Make sure we can read at an offset and with arbitrary length
        let data = self.read_ec_flash(FLASH_PROGRAM_OFFSET, 16).unwrap();
        if data[0..4] != [0x50, 0x48, 0x43, 0x4D] {
            println!("      INVALID: {:02X?}", &data[0..3]);
            return Err(EcError::DeviceError(format!(
                "INVALID: {:02X?}",
                &data[0..3]
            )));
        }
        debug!("Expected beginning with 50 48 43 4D ('PHCM' in ASCII)");
        debug!("{:02X?}", data);

        self.flash_notify(MecFlashNotify::AccessSpiDone)?;
        Ok(())
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
                    error!("Err: {:?}", err);
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

    /// Instantly reboot EC and host
    pub fn reboot(&self) -> EcResult<()> {
        EcRequestReboot {}.send_command(self)
    }

    pub fn reboot_ec(&self, command: RebootEcCmd) -> EcResult<()> {
        EcRequestRebootEc {
            cmd: command as u8,
            flags: RebootEcFlags::None as u8,
        }
        .send_command(self)
    }

    pub fn jump_rw(&self) -> EcResult<()> {
        // Note: AP Turns off
        EcRequestRebootEc {
            cmd: RebootEcCmd::JumpRw as u8,
            flags: 0,
            // flags: RebootEcFlags::OnApShutdown as u8,
        }
        .send_command(self)
    }

    pub fn jump_ro(&self) -> EcResult<()> {
        EcRequestRebootEc {
            cmd: RebootEcCmd::JumpRo as u8,
            flags: 0,
            // flags: RebootEcFlags::OnApShutdown as u8,
        }
        .send_command(self)
    }

    pub fn cancel_jump(&self) -> EcResult<()> {
        EcRequestRebootEc {
            cmd: RebootEcCmd::Cancel as u8,
            flags: 0,
        }
        .send_command(self)
    }

    pub fn disable_jump(&self) -> EcResult<()> {
        EcRequestRebootEc {
            cmd: RebootEcCmd::DisableJump as u8,
            flags: 0,
        }
        .send_command(self)
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
