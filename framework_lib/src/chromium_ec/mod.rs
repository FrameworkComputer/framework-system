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
use crate::power;
use crate::smbios;
#[cfg(feature = "uefi")]
use crate::uefi::shell_get_execution_break_flag;
use crate::util::{self, Platform};

use log::Level;
use num_derive::FromPrimitive;

pub mod command;
pub mod commands;
#[cfg(target_os = "linux")]
mod cros_ec;
pub mod i2c_passthrough;
pub mod input_deck;
#[cfg(not(windows))]
mod portio;
#[cfg(not(windows))]
mod portio_mec;
#[allow(dead_code)]
mod protocol;
#[cfg(windows)]
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

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum Framework12Adc {
    MainboardBoardId,
    PowerButtonBoardId,
    Psys,
    AdapterCurrent,
    TouchpadBoardId,
    AudioBoardId,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum FrameworkHx20Hx30Adc {
    AdapterCurrent,
    Psys,
    BattTemp,
    TouchpadBoardId,
    MainboardBoardId,
    AudioBoardId,
}

/// So far on all Nuvoton/Zephyr EC based platforms
/// Until at least Framework 13 AMD Ryzen AI 300
#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum Framework13Adc {
    MainboardBoardId,
    Psys,
    AdapterCurrent,
    TouchpadBoardId,
    AudioBoardId,
    BattTemp,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum Framework16Adc {
    MainboardBoardId,
    HubBoardId,
    GpuBoardId0,
    GpuBoardId1,
    AdapterCurrent,
    Psys,
}

/*
 * PLATFORM_EC_ADC_RESOLUTION default 10 bit
 *
 * +------------------+-----------+----------+-------------+---------+----------------------+
 * |  BOARD VERSION   |  voltage  | NPC DB V | main board  |   GPU   |     Input module     |
 * +------------------+-----------+----------|-------------+---------+----------------------+
 * | BOARD_VERSION_0  |  0    mV  | 100  mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_1  |  173  mV  | 310  mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_2  |  300  mV  | 520  mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_3  |  430  mV  | 720  mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_4  |  588  mV  | 930  mV  |  EVT1       |         |       Reserved       |
 * | BOARD_VERSION_5  |  783  mV  | 1130 mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_6  |  905  mV  | 1340 mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_7  |  1033 mV  | 1550 mV  |  DVT1       |         |       Reserved       |
 * | BOARD_VERSION_8  |  1320 mV  | 1750 mV  |  DVT2       |         |    Generic A size    |
 * | BOARD_VERSION_9  |  1500 mV  | 1960 mV  |  PVT        |         |    Generic B size    |
 * | BOARD_VERSION_10 |  1650 mV  | 2170 mV  |  MP         |         |    Generic C size    |
 * | BOARD_VERSION_11 |  1980 mV  | 2370 mV  |  Unused     | RID_0   |    10 Key B size     |
 * | BOARD_VERSION_12 |  2135 mV  | 2580 mV  |  Unused     | RID_0,1 |       Keyboard       |
 * | BOARD_VERSION_13 |  2500 mV  | 2780 mV  |  Unused     | RID_0   |       Touchpad       |
 * | BOARD_VERSION_14 |  2706 mV  | 2990 mV  |  Unused     |         |       Reserved       |
 * | BOARD_VERSION_15 |  2813 mV  | 3200 mV  |  Unused     |         |    Not installed     |
 * +------------------+-----------+----------+-------------+---------+----------------------+
 */

const BOARD_VERSION_COUNT: usize = 16;
const BOARD_VERSION: [i32; BOARD_VERSION_COUNT] = [
    85, 233, 360, 492, 649, 844, 965, 1094, 1380, 1562, 1710, 2040, 2197, 2557, 2766, 2814,
];

const BOARD_VERSION_NPC_DB: [i32; BOARD_VERSION_COUNT] = [
    100, 311, 521, 721, 931, 1131, 1341, 1551, 1751, 1961, 2171, 2370, 2580, 2780, 2990, 3200,
];

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
    let mut drivers = vec![];

    #[cfg(windows)]
    drivers.push(CrosEcDriverType::Windows);

    #[cfg(target_os = "linux")]
    if std::path::Path::new(cros_ec::DEV_PATH).exists() {
        drivers.push(CrosEcDriverType::CrosEc);
    }

    #[cfg(not(windows))]
    drivers.push(CrosEcDriverType::Portio);

    drivers
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

    pub fn check_mem_magic(&self) -> EcResult<()> {
        match self.read_memory(EC_MEMMAP_ID, 2) {
            Some(ec_id) => {
                if ec_id.len() != 2 {
                    Err(EcError::DeviceError(format!(
                        "  Unexpected length returned: {:?}",
                        ec_id.len()
                    )))
                } else if ec_id[0] != b'E' || ec_id[1] != b'C' {
                    Err(EcError::DeviceError(
                        "This machine doesn't look like it has a Framework EC".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            None => Err(EcError::DeviceError(
                "Failed to read EC ID from memory map".to_string(),
            )),
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

    pub fn motionsense_sensor_count(&self) -> EcResult<u8> {
        EcRequestMotionSenseDump {
            cmd: MotionSenseCmd::Dump as u8,
            max_sensor_count: 0,
        }
        .send_command(self)
        .map(|res| res.sensor_count)
    }

    pub fn motionsense_sensor_info(&self) -> EcResult<Vec<MotionSenseInfo>> {
        let count = self.motionsense_sensor_count()?;

        let mut sensors = vec![];
        for sensor_num in 0..count {
            let info = EcRequestMotionSenseInfo {
                cmd: MotionSenseCmd::Info as u8,
                sensor_num,
            }
            .send_command(self)?;
            sensors.push(MotionSenseInfo {
                sensor_type: FromPrimitive::from_u8(info.sensor_type).unwrap(),
                location: FromPrimitive::from_u8(info.location).unwrap(),
                chip: FromPrimitive::from_u8(info.chip).unwrap(),
            });
        }
        Ok(sensors)
    }

    pub fn motionsense_sensor_list(&self) -> EcResult<u8> {
        EcRequestMotionSenseDump {
            cmd: MotionSenseCmd::Dump as u8,
            max_sensor_count: 0,
        }
        .send_command(self)
        .map(|res| res.sensor_count)
    }

    pub fn remap_caps_to_ctrl(&self) -> EcResult<()> {
        self.remap_key(6, 15, 0x0014)
    }

    pub fn remap_key(&self, row: u8, col: u8, scanset: u16) -> EcResult<()> {
        let _current_matrix = EcRequestUpdateKeyboardMatrix {
            num_items: 1,
            write: 1,
            scan_update: [KeyboardMatrixMap { row, col, scanset }],
        }
        .send_command(self)?;
        Ok(())
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

    pub fn set_charge_current_limit(&self, current: u32, battery_soc: Option<u32>) -> EcResult<()> {
        if let Some(battery_soc) = battery_soc {
            let battery_soc = battery_soc as u8;
            EcRequestCurrentLimitV1 {
                current,
                battery_soc,
            }
            .send_command(self)
        } else {
            EcRequestCurrentLimitV0 { current }.send_command(self)
        }
    }

    pub fn set_charge_rate_limit(&self, rate: f32, battery_soc: Option<f32>) -> EcResult<()> {
        let power_info = power::power_info(self).ok_or(EcError::DeviceError(
            "Failed to get battery info".to_string(),
        ))?;
        let battery = power_info
            .battery
            .ok_or(EcError::DeviceError("No battery present".to_string()))?;
        println!("Requested Rate:      {}C", rate);
        println!("Design Current:      {}mA", battery.design_capacity);
        let current = (rate * (battery.design_capacity as f32)) as u32;
        println!("Limiting Current to: {}mA", current);
        if let Some(battery_soc) = battery_soc {
            let battery_soc = battery_soc as u8;
            EcRequestCurrentLimitV1 {
                current,
                battery_soc,
            }
            .send_command(self)
        } else {
            EcRequestCurrentLimitV0 { current }.send_command(self)
        }
    }

    pub fn set_fp_led_percentage(&self, percentage: u8) -> EcResult<()> {
        // Sending bytes manually because the Set command, as opposed to the Get command,
        // does not return any data
        let limits = &[percentage, 0x00];
        let data = self.send_command(EcCommands::FpLedLevelControl as u16, 1, limits)?;

        util::assert_win_len(data.len(), 0);

        Ok(())
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
    pub fn get_fp_led_level(&self) -> EcResult<(u8, Option<FpLedBrightnessLevel>)> {
        let res = EcRequestFpLedLevelControlV1 {
            set_percentage: 0xFF,
            get_level: 0xFF,
        }
        .send_command(self);

        // If V1 does not exist, fall back
        if let Err(EcError::Response(EcResponseStatus::InvalidVersion)) = res {
            let res = EcRequestFpLedLevelControlV0 {
                set_level: 0xFF,
                get_level: 0xFF,
            }
            .send_command(self)?;
            debug!("Current Brightness: {}%", res.percentage);
            return Ok((res.percentage, None));
        }

        let res = res?;

        debug!("Current Brightness: {}%", res.percentage);
        debug!("Level Raw:          {}", res.level);

        // TODO: can turn this into None and log
        let level = FromPrimitive::from_u8(res.level)
            .ok_or(EcError::DeviceError(format!("Invalid level {}", res.level)))?;
        Ok((res.percentage, Some(level)))
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

    pub fn print_fw12_inputdeck_status(&self) -> EcResult<()> {
        let intrusion = self.get_intrusion_status()?;
        let pwrbtn = self.read_board_id(Framework12Adc::PowerButtonBoardId as u8)?;
        let audio = self.read_board_id(Framework12Adc::AudioBoardId as u8)?;
        let tp = self.read_board_id(Framework12Adc::TouchpadBoardId as u8)?;

        let is_present = |p| if p { "Present" } else { "Missing" };

        println!("Input Deck");
        println!("  Chassis Closed:      {}", !intrusion.currently_open);
        println!("  Power Button Board:  {}", is_present(pwrbtn.is_some()));
        println!("  Audio Daughterboard: {}", is_present(audio.is_some()));
        println!("  Touchpad:            {}", is_present(tp.is_some()));

        Ok(())
    }

    pub fn print_fw13_inputdeck_status(&self) -> EcResult<()> {
        let intrusion = self.get_intrusion_status()?;

        let (audio, tp) = match smbios::get_platform() {
            Some(Platform::IntelGen11)
            | Some(Platform::IntelGen12)
            | Some(Platform::IntelGen13) => (
                self.read_board_id(FrameworkHx20Hx30Adc::AudioBoardId as u8)?,
                self.read_board_id(FrameworkHx20Hx30Adc::TouchpadBoardId as u8)?,
            ),

            _ => (
                self.read_board_id_npc_db(Framework13Adc::AudioBoardId as u8)?,
                self.read_board_id_npc_db(Framework13Adc::TouchpadBoardId as u8)?,
            ),
        };

        let is_present = |p| if p { "Present" } else { "Missing" };

        println!("Input Deck");
        println!("  Chassis Closed:      {}", !intrusion.currently_open);
        println!("  Audio Daughterboard: {}", is_present(audio.is_some()));
        println!("  Touchpad:            {}", is_present(tp.is_some()));

        Ok(())
    }

    pub fn print_fw16_inputdeck_status(&self) -> EcResult<()> {
        let intrusion = self.get_intrusion_status()?;
        let status = self.get_input_deck_status()?;
        let sleep_l = self.get_gpio("sleep_l")?;
        println!("Chassis Closed:   {}", !intrusion.currently_open);
        println!("Input Deck State: {:?}", status.state);
        println!("Touchpad present: {}", status.touchpad_present);
        println!("SLEEP# GPIO high: {}", sleep_l);
        println!("Positions:");
        println!("  Pos 0: {:?}", status.top_row.pos0);
        println!("  Pos 1: {:?}", status.top_row.pos1);
        println!("  Pos 2: {:?}", status.top_row.pos2);
        println!("  Pos 3: {:?}", status.top_row.pos3);
        println!("  Pos 4: {:?}", status.top_row.pos4);
        Ok(())
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
    pub fn get_keyboard_backlight(&self) -> EcResult<u8> {
        let kblight = EcRequestPwmGetDuty {
            pwm_type: PwmType::KbLight as u8,
            index: 0,
        }
        .send_command(self)?;

        Ok((kblight.duty / (PWM_MAX_DUTY / 100)) as u8)
    }

    pub fn ps2_emulation_enable(&self, enable: bool) -> EcResult<()> {
        EcRequestDisablePs2Emulation {
            disable: !enable as u8,
        }
        .send_command(self)
    }

    pub fn fan_set_rpm(&self, fan: Option<u32>, rpm: u32) -> EcResult<()> {
        if let Some(fan_idx) = fan {
            EcRequestPwmSetFanTargetRpmV1 { rpm, fan_idx }.send_command(self)
        } else {
            EcRequestPwmSetFanTargetRpmV0 { rpm }.send_command(self)
        }
    }

    pub fn fan_set_duty(&self, fan: Option<u32>, percent: u32) -> EcResult<()> {
        if percent > 100 {
            return Err(EcError::DeviceError("Fan duty must be <= 100".to_string()));
        }
        if let Some(fan_idx) = fan {
            EcRequestPwmSetFanDutyV1 { fan_idx, percent }.send_command(self)
        } else {
            EcRequestPwmSetFanDutyV0 { percent }.send_command(self)
        }
    }

    pub fn autofanctrl(&self, fan: Option<u8>) -> EcResult<()> {
        if let Some(fan_idx) = fan {
            EcRequestAutoFanCtrlV1 { fan_idx }.send_command(self)
        } else {
            EcRequestAutoFanCtrlV0 {}.send_command(self)
        }
    }

    /// Set tablet mode
    pub fn set_tablet_mode(&self, mode: TabletModeOverride) {
        let mode = mode as u8;
        let res = EcRequestSetTabletMode { mode }.send_command(self);
        print_err(res);
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
    pub fn reflash(&self, data: &[u8], ft: EcFlashType, dry_run: bool) -> EcResult<()> {
        let mut res = Ok(());
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

        // Determine recommended flash parameters
        let info = EcRequestFlashInfo {}.send_command(self)?;

        // Check that our hardcoded offsets are valid for the available flash
        if FLASH_RO_SIZE + FLASH_RW_SIZE > info.flash_size {
            return Err(EcError::DeviceError(format!(
                "RO+RW larger than flash 0x{:X}",
                { info.flash_size }
            )));
        }
        if FLASH_RW_BASE + FLASH_RW_SIZE > info.flash_size {
            return Err(EcError::DeviceError(format!(
                "RW overruns end of flash 0x{:X}",
                { info.flash_size }
            )));
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

            println!(
                "Erasing RW region{}",
                if dry_run { " (DRY RUN)" } else { "" }
            );
            self.erase_ec_flash(
                FLASH_BASE + FLASH_RW_BASE,
                FLASH_RW_SIZE,
                dry_run,
                info.erase_block_size,
            )?;
            println!("  Done");

            println!(
                "Writing RW region{}",
                if dry_run { " (DRY RUN)" } else { "" }
            );
            self.write_ec_flash(FLASH_BASE + FLASH_RW_BASE, rw_data, dry_run)?;
            println!("  Done");

            println!("Verifying RW region");
            let flash_rw_data = self.read_ec_flash(FLASH_BASE + FLASH_RW_BASE, FLASH_RW_SIZE)?;
            if rw_data == flash_rw_data {
                println!("  RW verify success");
            } else {
                error!("RW verify fail!");
                res = Err(EcError::DeviceError("RW verify fail!".to_string()));
            }
        }

        if ft == EcFlashType::Full || ft == EcFlashType::Ro {
            let ro_data = &data[FLASH_RO_BASE as usize..(FLASH_RO_BASE + FLASH_RO_SIZE) as usize];

            println!("Erasing RO region");
            self.erase_ec_flash(
                FLASH_BASE + FLASH_RO_BASE,
                FLASH_RO_SIZE,
                dry_run,
                info.erase_block_size,
            )?;
            println!("  Done");

            println!("Writing RO region");
            self.write_ec_flash(FLASH_BASE + FLASH_RO_BASE, ro_data, dry_run)?;
            println!("  Done");

            println!("Verifying RO region");
            let flash_ro_data = self.read_ec_flash(FLASH_BASE + FLASH_RO_BASE, FLASH_RO_SIZE)?;
            if ro_data == flash_ro_data {
                println!("  RO verify success");
            } else {
                error!("RO verify fail!");
                res = Err(EcError::DeviceError("RW verify fail!".to_string()));
            }
        }

        println!("Locking flash");
        self.flash_notify(MecFlashNotify::AccessSpiDone)?;
        self.flash_notify(MecFlashNotify::FirmwareDone)?;

        if res.is_ok() {
            println!("Flashing EC done. You can reboot the EC now");
        }

        res
    }

    /// Write a big section of EC flash. Must be unlocked already
    fn write_ec_flash(&self, addr: u32, data: &[u8], dry_run: bool) -> EcResult<()> {
        // TODO: Use flash info to help guide ideal chunk size
        // let info = EcRequestFlashInfo {}.send_command(self)?;
        //let chunk_size = ((0x80 / info.write_ideal_size) * info.write_ideal_size) as usize;

        let chunk_size = 0x80;

        let chunks = data.len() / chunk_size;
        println!(
            "  Will write flash from 0x{:X} to 0x{:X} in {}*{}B chunks",
            addr,
            data.len(),
            chunks,
            chunk_size
        );
        for chunk_no in 0..chunks {
            let offset = chunk_no * chunk_size;
            // Current chunk might be smaller if it's the last
            let cur_chunk_size = std::cmp::min(chunk_size, data.len() - chunk_no * chunk_size);

            if chunk_no % 100 == 0 {
                if chunk_no != 0 {
                    println!();
                }
                print!("  Chunk {:>4}: X", chunk_no);
            } else {
                print!("X");
            }

            let chunk = &data[offset..offset + cur_chunk_size];
            if !dry_run {
                let res = self.write_ec_flash_chunk(addr + offset as u32, chunk);
                if let Err(err) = res {
                    println!("  Failed to write chunk: {:?}", err);
                    return Err(err);
                }
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

    fn erase_ec_flash(
        &self,
        offset: u32,
        size: u32,
        dry_run: bool,
        chunk_size: u32,
    ) -> EcResult<()> {
        // Erasing a big section takes too long sometimes and the linux kernel driver times out, so
        // split it up into chunks.
        let mut cur_offset = offset;

        while cur_offset < offset + size {
            let rem_size = offset + size - cur_offset;
            let cur_size = if rem_size < chunk_size {
                rem_size
            } else {
                chunk_size
            };
            debug!(
                "EcRequestFlashErase (0x{:05X}, 0x{:05X})",
                cur_offset, cur_size
            );
            if !dry_run {
                EcRequestFlashErase {
                    offset: cur_offset,
                    size: cur_size,
                }
                .send_command(self)?;
            }
            cur_offset += chunk_size;
        }
        Ok(())
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
        debug!("    Check MCHP magic bytes at start of firmware code.");
        // Make sure we can read at an offset and with arbitrary length
        let data = self.read_ec_flash(FLASH_PROGRAM_OFFSET, 16).unwrap();
        debug!("Expecting beginning with 50 48 43 4D ('PHCM' in ASCII)");
        debug!("{:02X?}", data);
        debug!(
            "      {:02X?} ASCII:{:?}",
            &data[..4],
            core::str::from_utf8(&data[..4])
        );

        let has_mec = data[0..4] == [0x50, 0x48, 0x43, 0x4D];
        if has_mec {
            debug!("    Found MCHP magic bytes at start of firmware code.");
        }

        // ===== Test 4 =====
        println!("    Read flash flags");
        let data = if has_mec {
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

    pub fn check_bay_status(&self) -> EcResult<()> {
        println!("Expansion Bay");

        let info = EcRequestExpansionBayStatus {}.send_command(self)?;
        println!("  Enabled:       {}", info.module_enabled());
        println!("  No fault:      {}", !info.module_fault());
        println!("  Door closed:   {}", info.hatch_switch_closed());
        match info.expansion_bay_board() {
            Ok(board) => println!("  Board:         {:?}", board),
            Err(err) => println!("  Board:         {:?}", err),
        }

        if let Ok(sn) = self.get_gpu_serial() {
            println!("  Serial Number: {}", sn);
        } else {
            println!("  Serial Number: Unknown");
        }

        let res = EcRequestGetGpuPcie {}.send_command(self)?;
        let config: Option<GpuPcieConfig> = FromPrimitive::from_u8(res.gpu_pcie_config);
        let vendor: Option<GpuVendor> = FromPrimitive::from_u8(res.gpu_vendor);
        if let Some(config) = config {
            println!("  Config:        {:?}", config);
        } else {
            println!("  Config:        Unknown ({})", res.gpu_pcie_config);
        }
        if let Some(vendor) = vendor {
            println!("  Vendor:        {:?}", vendor);
        } else {
            println!("  Vendor:        Unknown ({})", res.gpu_vendor);
        }

        Ok(())
    }

    /// Get the GPU Serial
    ///
    pub fn get_gpu_serial(&self) -> EcResult<String> {
        let gpuserial: EcResponseGetGpuSerial =
            EcRequestGetGpuSerial { idx: 0 }.send_command(self)?;
        let serial: String = String::from_utf8(gpuserial.serial.to_vec()).unwrap();

        if gpuserial.valid == 0 {
            return Err(EcError::DeviceError("No valid GPU serial".to_string()));
        }

        Ok(serial)
    }

    /// Set the GPU Serial
    ///
    /// # Arguments
    /// `newserial` - a string that is 18 characters long
    pub fn set_gpu_serial(&self, magic: u8, newserial: String) -> EcResult<u8> {
        let mut array_tmp: [u8; 20] = [0; 20];
        array_tmp[..18].copy_from_slice(newserial.as_bytes());
        let result = EcRequestSetGpuSerial {
            magic,
            idx: 0,
            serial: array_tmp,
        }
        .send_command(self)?;
        Ok(result.valid)
    }

    pub fn read_ec_gpu_chunk(&self, addr: u16, len: u16) -> EcResult<Vec<u8>> {
        let eeprom_port = 0x05;
        let eeprom_addr = 0x50;
        let mut data: Vec<u8> = Vec::with_capacity(len.into());

        while data.len() < len.into() {
            let remaining = len - data.len() as u16;
            let chunk_len = std::cmp::min(i2c_passthrough::MAX_I2C_CHUNK, remaining.into());
            let offset = addr + data.len() as u16;
            let i2c_response = i2c_passthrough::i2c_read(
                self,
                eeprom_port,
                eeprom_addr,
                offset,
                chunk_len as u16,
            )?;
            if let Err(EcError::DeviceError(err)) = i2c_response.is_successful() {
                return Err(EcError::DeviceError(format!(
                    "I2C read was not successful: {:?}",
                    err
                )));
            }
            data.extend(i2c_response.data);
        }

        Ok(data)
    }

    pub fn write_ec_gpu_chunk(&self, offset: u16, data: &[u8]) -> EcResult<()> {
        let result = i2c_passthrough::i2c_write(self, 5, 0x50, offset, data)?;
        result.is_successful()
    }

    /// Writes EC GPU descriptor to the GPU EEPROM.
    pub fn set_gpu_descriptor(&self, data: &[u8], dry_run: bool) -> EcResult<()> {
        println!(
            "Writing GPU EEPROM {}",
            if dry_run { " (DRY RUN)" } else { "" }
        );
        // Need to program the EEPROM 32 bytes at a time.
        let chunk_size = 32;

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
            if dry_run {
                continue;
            }

            let chunk = &data[offset..offset + cur_chunk_size];
            let res = self.write_ec_gpu_chunk((offset as u16).to_be(), chunk);
            // Don't read too fast, wait 100ms before writing more to allow for page erase/write cycle.
            os_specific::sleep(100_000);
            if let Err(err) = res {
                println!("  Failed to write chunk: {:?}", err);
                return Err(err);
            }
        }
        println!();
        Ok(())
    }

    pub fn read_gpu_descriptor(&self) -> EcResult<Vec<u8>> {
        let header = self.read_gpu_desc_header()?;
        if header.magic != [0x32, 0xAC, 0x00, 0x00] {
            return Err(EcError::DeviceError(
                "Invalid descriptor hdr magic".to_string(),
            ));
        }
        self.read_ec_gpu_chunk(0x00, header.descriptor_length as u16)
    }

    pub fn read_gpu_desc_header(&self) -> EcResult<GpuCfgDescriptor> {
        let bytes =
            self.read_ec_gpu_chunk(0x00, core::mem::size_of::<GpuCfgDescriptor>() as u16)?;
        let header: *const GpuCfgDescriptor = unsafe { std::mem::transmute(bytes.as_ptr()) };
        let header = unsafe { *header };

        Ok(header)
    }

    /// Requests recent console output from EC and constantly asks for more
    /// Prints the output and returns it when an error is encountered
    pub fn console_read(&self) -> EcResult<()> {
        EcRequestConsoleSnapshot {}.send_command(self)?;

        let mut cmd = EcRequestConsoleRead {
            subcmd: ConsoleReadSubCommand::ConsoleReadNext as u8,
        };
        loop {
            match cmd.send_command_vec(self) {
                Ok(data) => {
                    // EC Buffer is empty. That means we've read everything from the snapshot.
                    // The windows crosecbus driver returns all NULL with a leading 0x01 instead of
                    // an empty response.
                    if data.is_empty() || data.iter().all(|x| *x == 0 || *x == 1) {
                        debug!("Empty EC response. Stopping console read");
                        // Don't read too fast, wait 100ms before reading more
                        os_specific::sleep(100_000);
                        EcRequestConsoleSnapshot {}.send_command(self)?;
                        cmd.subcmd = ConsoleReadSubCommand::ConsoleReadRecent as u8;
                        continue;
                    }

                    let utf8 = std::str::from_utf8(&data).unwrap();
                    let full_ascii = utf8.replace(|c: char| !c.is_ascii(), "");
                    let ascii = full_ascii.replace(['\0'], "");

                    print!("{}", ascii);
                }
                Err(err) => {
                    error!("Err: {:?}", err);
                    return Err(err);
                }
            };

            // Need to explicitly handle CTRL-C termination on UEFI Shell
            #[cfg(feature = "uefi")]
            if shell_get_execution_break_flag() {
                return Ok(());
            }
        }
    }

    /// Read all of EC console buffer and return it
    pub fn console_read_one(&self) -> EcResult<String> {
        EcRequestConsoleSnapshot {}.send_command(self)?;

        let mut console = String::new();
        let cmd = EcRequestConsoleRead {
            subcmd: ConsoleReadSubCommand::ConsoleReadNext as u8,
        };
        loop {
            match cmd.send_command_vec(self) {
                Ok(data) => {
                    // EC Buffer is empty. That means we've read everything
                    // The windows crosecbus driver returns all NULL instead of empty response
                    if data.is_empty() || data.iter().all(|x| *x == 0) {
                        debug!("Empty EC response. Stopping console read");
                        return Ok(console);
                    }

                    let utf8 = std::str::from_utf8(&data).unwrap();
                    let ascii = utf8
                        .replace(|c: char| !c.is_ascii(), "")
                        .replace(['\0'], "");

                    console.push_str(ascii.as_str());
                }
                Err(err) => {
                    error!("Err: {:?}", err);
                    return Err(err);
                }
            };
        }
    }

    pub fn get_charge_state(&self, power_info: &power::PowerInfo) -> EcResult<()> {
        let res = EcRequestChargeStateGetV0 {
            cmd: ChargeStateCmd::GetState as u8,
            param: 0,
        }
        .send_command(self)?;
        println!("Charger Status");
        println!(
            "  AC is:            {}",
            if res.ac == 1 {
                "connected"
            } else {
                "not connected"
            }
        );
        println!("  Charger Voltage:  {}mV", { res.chg_voltage });
        println!("  Charger Current:  {}mA", { res.chg_current });
        if let Some(battery) = &power_info.battery {
            let charge_rate = (res.chg_current as f32) / (battery.design_capacity as f32);
            println!("                    {:.2}C", charge_rate);
        }
        println!("  Chg Input Current:{}mA", { res.chg_input_current });
        println!("  Battery SoC:      {}%", { res.batt_state_of_charge });

        Ok(())
    }

    pub fn set_ec_hib_delay(&self, seconds: u32) -> EcResult<()> {
        EcRequesetHibernationDelay { seconds }.send_command(self)?;
        Ok(())
    }

    pub fn get_ec_hib_delay(&self) -> EcResult<u32> {
        let res = EcRequesetHibernationDelay { seconds: 0 }.send_command(self)?;
        debug!("Time in G3:        {:?}", { res.time_g3 });
        debug!("Time remaining:    {:?}", { res.time_remaining });
        println!("EC Hibernation Delay: {:?}s", { res.hibernation_delay });
        Ok(res.hibernation_delay)
    }

    /// Check features supported by the firmware
    pub fn get_features(&self) -> EcResult<()> {
        let data = EcRequestGetFeatures {}.send_command(self)?;
        println!(" ID | Name                        | Enabled?");
        println!(" -- | --------------------------- | --------");
        for i in 0..64 {
            let byte = i / 32;
            let bit = i % 32;
            let val = (data.flags[byte] & (1 << bit)) > 0;
            let feat: Option<EcFeatureCode> = FromPrimitive::from_usize(i);

            if let Some(feat) = feat {
                let name = format!("{:?}", feat);
                println!(" {:>2} | {:<27} | {:>5}", i, name, val);
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
        request.name[..end].copy_from_slice(&name.as_bytes()[..end]);

        let res = request.send_command(self)?;
        Ok(res.val == 1)
    }

    pub fn get_all_gpios(&self) -> EcResult<u8> {
        let res = EcRequestGpioGetV1Count {
            subcmd: GpioGetSubCommand::Count as u8,
        }
        .send_command(self)?;
        let gpio_count = res.val;

        debug!("Found {} GPIOs", gpio_count);

        for index in 0..res.val {
            let res = EcRequestGpioGetV1Info {
                subcmd: GpioGetSubCommand::Info as u8,
                index,
            }
            .send_command(self)?;

            let name = std::str::from_utf8(&res.name)
                .map_err(|utf8_err| {
                    EcError::DeviceError(format!("Failed to decode GPIO name: {:?}", utf8_err))
                })?
                .trim_end_matches(char::from(0))
                .to_string();

            if log_enabled!(Level::Info) {
                // Same output as ectool
                println!("{:>32}: {:>2} 0x{:04X}", res.val, name, { res.flags });
            } else {
                // Simple output, just name and level high/low
                println!("{:<32} {}", name, res.val);
            }
        }

        Ok(gpio_count)
    }

    pub fn adc_read(&self, adc_channel: u8) -> EcResult<i32> {
        let res = EcRequestAdcRead { adc_channel }.send_command(self)?;
        Ok(res.adc_value)
    }

    fn read_board_id(&self, channel: u8) -> EcResult<Option<u8>> {
        self.read_board_id_raw(channel, BOARD_VERSION)
    }
    fn read_board_id_npc_db(&self, channel: u8) -> EcResult<Option<u8>> {
        self.read_board_id_raw(channel, BOARD_VERSION_NPC_DB)
    }

    fn read_board_id_raw(
        &self,
        channel: u8,
        table: [i32; BOARD_VERSION_COUNT],
    ) -> EcResult<Option<u8>> {
        let mv = self.adc_read(channel)?;
        if mv < 0 {
            return Err(EcError::DeviceError(format!(
                "Failed to read ADC channel {}",
                channel
            )));
        }

        debug!("ADC Channel {} - Measured {}mv", channel, mv);
        for (board_id, board_id_res) in table.iter().enumerate() {
            if mv < *board_id_res {
                debug!("ADC Channel {} - Board ID {}", channel, board_id);
                // 15 is not present, less than 2 is undefined
                return Ok(if board_id == 15 || board_id < 2 {
                    None
                } else {
                    Some(board_id as u8)
                });
            }
        }

        Err(EcError::DeviceError(format!(
            "Unknown board id. ADC mv: {}",
            mv
        )))
    }

    pub fn rgbkbd_set_color(&self, start_key: u8, colors: Vec<RgbS>) -> EcResult<()> {
        for (chunk, colors) in colors.chunks(EC_RGBKBD_MAX_KEY_COUNT).enumerate() {
            let mut request = EcRequestRgbKbdSetColor {
                start_key: start_key + ((chunk * EC_RGBKBD_MAX_KEY_COUNT) as u8),
                length: colors.len() as u8,
                color: [(); EC_RGBKBD_MAX_KEY_COUNT].map(|()| Default::default()),
            };

            for (i, color) in colors.iter().enumerate() {
                request.color[i] = *color;
            }

            let _res = request.send_command(self)?;
        }
        Ok(())
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
        if offset + length > EC_MEMMAP_SIZE {
            return None;
        }

        // TODO: Change this function to return EcResult instead and print the error only in UI code
        print_err(match self.driver {
            #[cfg(not(windows))]
            CrosEcDriverType::Portio => portio::read_memory(offset, length),
            #[cfg(windows)]
            CrosEcDriverType::Windows => windows::read_memory(offset, length),
            #[cfg(target_os = "linux")]
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
            #[cfg(not(windows))]
            CrosEcDriverType::Portio => portio::send_command(command, command_version, data),
            #[cfg(windows)]
            CrosEcDriverType::Windows => windows::send_command(command, command_version, data),
            #[cfg(target_os = "linux")]
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
#[derive(PartialEq, Debug)]
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

#[derive(Clone, Debug, Copy, PartialEq)]
#[repr(C, packed)]
pub struct GpuCfgDescriptor {
    /// Expansion bay card magic value that is unique
    pub magic: [u8; 4],
    /// Length of header following this field
    pub length: u32,
    /// descriptor version, if EC max version is lower than this, ec cannot parse
    pub desc_ver_major: u16,
    pub desc_ver_minor: u16,
    /// Hardware major version
    pub hardware_version: u16,
    /// Hardware minor revision
    pub hardware_revision: u16,
    /// 18 digit Framework Serial that starts with FRA
    /// the first 10 digits must be allocated by framework
    pub serial: [u8; 20],
    /// Length of descriptor following heade
    pub descriptor_length: u32,
    /// CRC of descriptor
    pub descriptor_crc32: u32,
    /// CRC of header before this value
    pub crc32: u32,
}
