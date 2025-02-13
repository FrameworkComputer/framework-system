//! Interact with the Chrome EC controller firmware.
//!
//! It's used on Chromebooks as well as non-Chromebook Framework laptops.
//!
//! Currently three drivers are supported:
//!
//! - `cros_ec` - It uses the `cros_ec` kernel module in Linux
//! - `portio` - It uses raw port I/O. This works on UEFI and on Linux if the system isn't in lockdown mode (SecureBoot disabled).
//! - `windows` - It uses [DHowett's Windows driver](https://github.com/DHowett/FrameworkWindowsUtils)

use crate::ec_binary;
use crate::os_specific;
use crate::smbios;
#[cfg(feature = "uefi")]
use crate::uefi::shell_get_execution_break_flag;
use crate::util::{self, Platform};

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
const MEC_FLASH_FLAGS: u32 = 0x80000;
const NPC_FLASH_FLAGS: u32 = 0x7F000;
const FLASH_PROGRAM_OFFSET: u32 = 0x1000;

#[derive(Clone, Debug, PartialEq)]
pub enum EcFlashType {
    Full,
    Ro,
    Rw,
}

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

pub fn has_mec() -> bool {
    let platform = smbios::get_platform().unwrap();
    if let Platform::GenericFramework(_, _, has_mec) = platform {
        return has_mec;
    }

    !matches!(
        smbios::get_platform().unwrap(),
        Platform::Framework13Amd | Platform::Framework16 | Platform::IntelCoreUltra1
    )
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
            std::str::from_utf8(&v.version_string_ro)
                .ok()?
                .trim_end_matches(char::from(0))
                .to_string(),
            std::str::from_utf8(&v.version_string_rw)
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

        util::assert_win_len(data.len(), 0);

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

        util::assert_win_len(data.len(), 0);

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
        let res = EcRequestPwmSetDuty {
            duty: percent as u16 * (PWM_MAX_DUTY / 100),
            pwm_type: PwmType::KbLight as u8,
            index: 0,
        }
        .send_command(self);
        debug_assert!(res.is_ok());
    }

    /// Check the current brightness of the keyboard backlight
    ///
    pub fn get_keyboard_backlight(&self) -> EcResult<u8> {
        let kblight = EcRequestPwmGetDuty {
            pwm_type: PwmType::KbLight as u8,
            index: 0,
        }
        .send_command(self)?;

        Ok((kblight.duty / (PWM_MAX_DUTY / 100)) as u8)
    }

    /// Overwrite RO and RW regions of EC flash
    /// MEC/Legacy EC
    /// | Start | End   | Size  | Region      |
    /// | 00000 | 3BFFF | 3C000 | RO Region   |
    /// | 3C000 | 3FFFF | 04000 | Preserved   |
    /// | 40000 | 3C000 | 39000 | RO Region   |
    /// | 79000 | 79FFF | 01000 | Preserved   |
    /// | 80000 | 80FFF | 01000 | Flash Flags |
    ///
    /// NPC/Zephyr
    /// | Start | End   | Size  | Region      |
    /// | 00000 | 3BFFF | 3C000 | RO Region   |
    /// | 3C000 | 3FFFF | 04000 | Preserved   |
    /// | 40000 | 3C000 | 39000 | RO Region   |
    /// | 79000 | 79FFF | 01000 | Flash Flags |
    pub fn reflash(&self, data: &[u8], ft: EcFlashType) -> EcResult<()> {
        if ft == EcFlashType::Full || ft == EcFlashType::Ro {
            if let Some(version) = ec_binary::read_ec_version(data, true) {
                println!("EC RO Version in File: {:?}", version.version);
            } else {
                return Err(EcError::DeviceError(
                    "File does not contain valid EC RO firmware".to_string(),
                ));
            }
        }
        if ft == EcFlashType::Full || ft == EcFlashType::Rw {
            if let Some(version) = ec_binary::read_ec_version(data, false) {
                println!("EC RW Version in File: {:?}", version.version);
            } else {
                return Err(EcError::DeviceError(
                    "File does not contain valid EW RO firmware".to_string(),
                ));
            }
        }

        if ft == EcFlashType::Full || ft == EcFlashType::Ro {
            println!("For safety reasons flashing RO firmware is disabled.");
            return Ok(());
        }

        println!("Unlocking flash");
        self.flash_notify(MecFlashNotify::AccessSpi)?;
        self.flash_notify(MecFlashNotify::FirmwareStart)?;

        // TODO: Check if erase was successful
        // 1. First erase 0x10000 bytes
        // 2. Read back two rows and make sure it's all 0xFF
        // 3. Write each row (128B) individually

        if ft == EcFlashType::Full || ft == EcFlashType::Rw {
            let rw_data = &data[FLASH_RW_BASE as usize..(FLASH_RW_BASE + FLASH_RW_SIZE) as usize];

            println!("Erasing RW region");
            self.erase_ec_flash(FLASH_BASE + FLASH_RW_BASE, FLASH_RW_SIZE)?;

            println!("Writing RW region");
            self.write_ec_flash(FLASH_BASE + FLASH_RW_BASE, rw_data)?;

            println!("Verifying RW region");
            let flash_rw_data = self.read_ec_flash(FLASH_BASE + FLASH_RW_BASE, FLASH_RW_SIZE)?;
            if rw_data == flash_rw_data {
                println!("RW verify success");
            } else {
                println!("RW verify fail");
            }
        }

        if ft == EcFlashType::Full || ft == EcFlashType::Ro {
            let ro_data = &data[FLASH_RO_BASE as usize..(FLASH_RO_BASE + FLASH_RO_SIZE) as usize];

            println!("Erasing RO region");
            self.erase_ec_flash(FLASH_BASE + FLASH_RO_BASE, FLASH_RO_SIZE)?;

            println!("Writing RO region");
            self.write_ec_flash(FLASH_BASE + FLASH_RO_BASE, ro_data)?;

            println!("Verifying RO region");
            let flash_ro_data = self.read_ec_flash(FLASH_BASE + FLASH_RO_BASE, FLASH_RO_SIZE)?;
            if ro_data == flash_ro_data {
                println!("RO verify success");
            } else {
                println!("RO verify fail");
            }
        }

        println!("Locking flash");
        self.flash_notify(MecFlashNotify::AccessSpiDone)?;
        self.flash_notify(MecFlashNotify::FirmwareDone)?;

        println!("Flashing EC done. You can reboot the EC now");
        // TODO: Should we force a reboot if currently running one was reflashed?

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
        let data = EcRequestFlashRead { offset, size }.send_command_vec(self)?;

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
                // TODO: We don't want to crash here. But returning no data doesn't seem optimal
                // either
                // return Err(EcError::DeviceError("Execution interrupted".to_string()));
                println!("Execution interrupted");
                return Ok(vec![]);
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
        let mut res = Ok(());
        // TODO: Perhaps we could have some more global flag to avoid setting and unsetting that ever time
        self.flash_notify(MecFlashNotify::AccessSpi)?;

        // ===== Test 1 =====
        // Read the first row of flash.
        // It's the beginning of RO firmware
        println!("    Read first row of flash (RO FW)");
        let data = self.read_ec_flash(0, 0x80).unwrap();

        debug!("{:02X?}", data);
        println!("      {:02X?}", &data[..8]);
        if data.iter().all(|x| *x == 0xFF) {
            println!("      Erased!");
        }

        // 4 magic bytes at the beginning
        let legacy_start = [0x10, 0x00, 0x00, 0xF7];
        // TODO: Does zephyr always start like this?
        let zephyr_start = [0x5E, 0x4D, 0x3B, 0x2A];
        if data[0..4] != legacy_start && data[0..4] != zephyr_start {
            println!("      INVALID start");
            res = Err(EcError::DeviceError("INVALID start".to_string()));
        }
        // Legacy EC is all 0xFF until the end of the row
        // Zephyr EC I'm not quite sure but it has a section of 0x00
        let legacy_comp = !data[4..].iter().all(|x| *x == 0xFF);
        let zephyr_comp = !data[0x20..0x40].iter().all(|x| *x == 0x00);
        if legacy_comp && zephyr_comp {
            println!("      INVALID end");
            res = Err(EcError::DeviceError("INVALID end".to_string()));
        }

        // ===== Test 2 =====
        // DISABLED
        // TODO: Haven't figure out a pattern yet
        //
        // Read the first row of the second half of flash
        // It's the beginning of RW firmware
        println!("    Read first row of RW FW");
        let data = self.read_ec_flash(0x40000, 0x80).unwrap();

        println!("      {:02X?}", &data[..8]);
        if data.iter().all(|x| *x == 0xFF) {
            println!("      Erased!");
            res = Err(EcError::DeviceError("RW Erased".to_string()));
        }

        // TODO: How can we identify if the RO image is valid?
        // //debug!("Expected TODO and rest all FF");
        // debug!("Expecting 80 7D 0C 20 and 0x20-0x2C all 00");
        // let legacy_start = []; // TODO
        // let zephyr_start = [0x80, 0x7D, 0x0C, 0x20];
        // if data[0..4] != legacy_start && data[0..4] != zephyr_start {
        //     println!("      INVALID start");
        //     res = Err(EcError::DeviceError("INVALID start".to_string()));
        // }
        // let legacy_comp = !data[4..].iter().all(|x| *x == 0xFF);
        // let zephyr_comp = !data[0x20..0x2C].iter().all(|x| *x == 0x00);
        // if legacy_comp && zephyr_comp {
        //     println!("      INVALID end");
        //     res = Err(EcError::DeviceError("INVALID end".to_string()));
        // }

        // ===== Test 3 =====
        //
        // MEC EC has program code at 0x1000 with magic bytes that spell
        // MCHP (Microchip) in ASCII backwards.
        // Everything before is probably a header.
        // TODO: I don't think there are magic bytes on zephyr firmware
        //
        if has_mec() {
            println!("    Check MCHP magic byte at start of firmware code.");
            // Make sure we can read at an offset and with arbitrary length
            let data = self.read_ec_flash(FLASH_PROGRAM_OFFSET, 16).unwrap();
            debug!("Expecting beginning with 50 48 43 4D ('PHCM' in ASCII)");
            debug!("{:02X?}", data);
            println!(
                "      {:02X?} ASCII:{:?}",
                &data[..4],
                core::str::from_utf8(&data[..4])
            );

            if data[0..4] != [0x50, 0x48, 0x43, 0x4D] {
                println!("      INVALID: {:02X?}", &data[0..3]);
                res = Err(EcError::DeviceError(format!(
                    "INVALID: {:02X?}",
                    &data[0..3]
                )));
            }
        }

        // ===== Test 4 =====
        println!("    Read flash flags");
        let data = if has_mec() {
            self.read_ec_flash(MEC_FLASH_FLAGS, 0x80).unwrap()
        } else {
            self.read_ec_flash(NPC_FLASH_FLAGS, 0x80).unwrap()
        };
        let flash_flags_magic = [0xA3, 0xF1, 0x00, 0x00];
        let flash_flags_ver = [0x01, 0x0, 0x00, 0x00];
        // All 0xFF if just reflashed and not reinitialized by EC
        if data[0..4] == flash_flags_magic && data[8..12] == flash_flags_ver {
            println!("      Valid flash flags");
        } else if data.iter().all(|x| *x == 0xFF) {
            println!("      Erased flash flags");
            res = Err(EcError::DeviceError("Erased flash flags".to_string()));
        } else {
            println!("      INVALID flash flags: {:02X?}", &data[0..12]);
            // TODO: Disable error until I confirm flash flags on MEC
            // res = Err(EcError::DeviceError("INVALID flash flags".to_string()));
        }

        self.flash_notify(MecFlashNotify::AccessSpiDone)?;

        res
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
                        .replace(['\0'], "");

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
            .replace(['\0'], "");
        Ok(ascii)
    }

    /// Check features supported by the firmware
    pub fn get_features(&self) -> EcResult<()> {
        let data = EcRequestGetFeatures {}.send_command(self)?;
        for i in 0..64 {
            let byte = i / 32;
            let bit = i % 32;
            let val = (data.flags[byte] & (1 << bit)) > 0;
            let feat: Option<EcFeatureCode> = FromPrimitive::from_usize(i);

            if let Some(feat) = feat {
                println!("{:>2}: {:>5} {:?}", i, val, feat);
            }
        }

        Ok(())
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

    pub fn get_gpio(&self, name: &str) -> EcResult<bool> {
        const MAX_LEN: usize = 32;
        let mut request = EcRequestGpioGetV0 { name: [0; MAX_LEN] };

        let end = MAX_LEN.min(name.len());
        request.name[..end].copy_from_slice(name[..end].as_bytes());

        let res = request.send_command(self)?;
        Ok(res.val == 1)
    }
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum CrosEcDriverType {
    Portio,
    CrosEc,
    Windows,
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum HardwareDeviceType {
    BIOS,
    EC,
    PD0,
    PD1,
    RTM01,
    RTM23,
    AcLeft,
    AcRight,
}

impl CrosEcDriver for CrosEc {
    fn read_memory(&self, offset: u16, length: u16) -> Option<Vec<u8>> {
        if !smbios::is_framework() {
            return None;
        }

        debug!("read_memory(offset={:#X}, size={:#X})", offset, length);
        if offset + length > (EC_MEMMAP_SIZE * 2) {
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
