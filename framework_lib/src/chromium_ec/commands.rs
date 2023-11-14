use core::fmt;

use num_derive::FromPrimitive;

use super::{command::*, input_deck::INPUT_DECK_SLOTS};

#[repr(C, packed)]
pub struct EcRequestGetVersion {}

#[repr(C, packed)]
pub struct EcResponseGetVersion {
    /// Null-terminated version of the RO firmware
    pub version_string_ro: [u8; 32],
    /// Null-terminated version of the RW firmware
    pub version_string_rw: [u8; 32],
    /// Used to be the RW-B string
    pub reserved: [u8; 32],
    /// Which EC image is currently in-use. See enum EcCurrentImage
    pub current_image: u32,
}
impl EcRequest<EcResponseGetVersion> for EcRequestGetVersion {
    fn command_id() -> EcCommands {
        EcCommands::GetVersion
    }
}

#[repr(C, packed)]
pub struct EcRequestGetCmdVersionsV0 {
    pub cmd: u8,
}
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EcResponseGetCmdVersionsV0 {
    pub version_mask: u32,
}
impl EcRequest<EcResponseGetCmdVersionsV0> for EcRequestGetCmdVersionsV0 {
    fn command_id() -> EcCommands {
        EcCommands::GetCmdVersions
    }
    fn command_version() -> u8 {
        0
    }
}

#[repr(C, packed)]
pub struct EcRequestGetCmdVersionsV1 {
    pub cmd: u32,
}
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct EcResponseGetCmdVersionsV1 {
    pub version_mask: u32,
}
impl EcRequest<EcResponseGetCmdVersionsV1> for EcRequestGetCmdVersionsV1 {
    fn command_id() -> EcCommands {
        EcCommands::GetCmdVersions
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequestPwmSetKeyboardBacklight {
    pub percent: u8,
}

impl EcRequest<()> for EcRequestPwmSetKeyboardBacklight {
    fn command_id() -> EcCommands {
        EcCommands::PwmSetKeyboardBacklight
    }
}

#[repr(C, packed)]
pub struct EcRequestPwmGetKeyboardBacklight {}

#[repr(C, packed)]
pub struct EcResponsePwmGetKeyboardBacklight {
    pub percent: u8,
    pub enabled: u8,
}

impl EcRequest<EcResponsePwmGetKeyboardBacklight> for EcRequestPwmGetKeyboardBacklight {
    fn command_id() -> EcCommands {
        EcCommands::PwmGetKeyboardBacklight
    }
}

pub struct EcRequestConsoleSnapshot {}
impl EcRequest<()> for EcRequestConsoleSnapshot {
    fn command_id() -> EcCommands {
        EcCommands::ConsoleSnapshot
    }
}

pub enum ConsoleReadSubCommand {
    ConsoleReadNext = 0,
    ConsoleReadRecent = 1,
}

pub struct EcRequestConsoleRead {
    pub subcmd: u8,
}

impl EcRequest<()> for EcRequestConsoleRead {
    fn command_id() -> EcCommands {
        EcCommands::ConsoleRead
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequestUsbPdPowerInfo {
    pub port: u8,
}

#[repr(C, packed)]
pub struct _UsbChargeMeasures {
    pub voltage_max: u16,
    pub voltage_now: u16,
    pub current_max: u16,
    pub current_lim: u16,
}

#[repr(C, packed)]
pub struct EcResponseUsbPdPowerInfo {
    pub role: u8,          // UsbPowerRoles
    pub charging_type: u8, // UsbChargingType
    pub dualrole: u8,      // I think this is a boolean?
    pub reserved1: u8,
    pub meas: _UsbChargeMeasures,
    pub max_power: u32,
}

impl EcRequest<EcResponseUsbPdPowerInfo> for EcRequestUsbPdPowerInfo {
    fn command_id() -> EcCommands {
        EcCommands::UsbPdPowerInfo
    }
}

// --- Framework Specific commands ---

#[repr(C, packed)]
pub struct EcRequestFlashNotify {
    // TODO: Use types to help build the flags
    pub flags: u8,
}

#[repr(C, packed)]
pub struct EcResponseFlashNotify {}

impl EcRequest<EcResponseFlashNotify> for EcRequestFlashNotify {
    fn command_id() -> EcCommands {
        EcCommands::FlashNotified
    }
}

#[repr(C, packed)]
pub struct EcRequestChassisOpenCheck {}

#[repr(C, packed)]
pub struct EcResponseChassisOpenCheck {
    pub status: u8,
}

impl EcRequest<EcResponseChassisOpenCheck> for EcRequestChassisOpenCheck {
    fn command_id() -> EcCommands {
        EcCommands::ChassisOpenCheck
    }
}

#[repr(C, packed)]
pub struct EcRequestChassisIntrusionControl {
    pub clear_magic: u8,
    pub clear_chassis_status: u8,
}

#[repr(C, packed)]
pub struct EcResponseChassisIntrusionControl {
    pub chassis_ever_opened: u8,
    pub coin_batt_ever_remove: u8,
    pub total_open_count: u8,
    pub vtr_open_count: u8,
}

impl EcRequest<EcResponseChassisIntrusionControl> for EcRequestChassisIntrusionControl {
    fn command_id() -> EcCommands {
        EcCommands::ChassisIntrusion
    }
}

#[repr(C, packed)]
pub struct EcRequestReadPdVersion {}

#[repr(C, packed)]
pub struct _EcResponseReadPdVersion {
    pub controller01: [u8; 8],
    pub controller23: [u8; 8],
}

impl EcRequest<_EcResponseReadPdVersion> for EcRequestReadPdVersion {
    fn command_id() -> EcCommands {
        EcCommands::ReadPdVersion
    }
}

#[repr(C, packed)]
pub struct EcRequestPrivacySwitches {}

#[repr(C, packed)]
pub struct EcResponsePrivacySwitches {
    pub microphone: u8,
    pub camera: u8,
}

impl EcRequest<EcResponsePrivacySwitches> for EcRequestPrivacySwitches {
    fn command_id() -> EcCommands {
        EcCommands::PriavcySwitchesCheckMode
    }
}

#[repr(u8)]
pub enum DeckStateMode {
    ReadOnly = 0x00,
    Required = 0x01,
    ForceOn = 0x02,
    ForceOff = 0x04,
}

#[repr(C, packed)]
pub struct EcRequestDeckState {
    pub mode: DeckStateMode,
}

#[repr(C, packed)]
pub struct EcResponseDeckState {
    pub board_id: [u8; INPUT_DECK_SLOTS],
    pub deck_state: u8,
}

impl EcRequest<EcResponseDeckState> for EcRequestDeckState {
    fn command_id() -> EcCommands {
        EcCommands::CheckDeckState
    }
}

// TODO
#[repr(C, packed)]
pub struct EcRequestUefiAppMode {
    pub enable: u8,
}

impl EcRequest<()> for EcRequestUefiAppMode {
    fn command_id() -> EcCommands {
        EcCommands::UefiAppMode
    }
}

#[repr(C, packed)]
pub struct EcRequestUefiAppBtnStatus {}

#[repr(C, packed)]
pub struct EcResponseUefiAppBtnStatus {
    pub status: u8,
}

impl EcRequest<EcResponseUefiAppBtnStatus> for EcRequestUefiAppBtnStatus {
    fn command_id() -> EcCommands {
        EcCommands::UefiAppBtnStatus
    }
}

#[derive(Debug)]
pub enum ExpansionByStates {
    ModuleEnabled = 0x01,
    ModuleFault = 0x02,
    HatchSwitchClosed = 0x04,
}
#[derive(Debug)]
pub enum ExpansionBayBoard {
    DualInterposer,
    SingleInterposer,
    UmaFans,
}

#[derive(Debug)]
pub enum ExpansionBayIssue {
    NoModule,
    BadConnection(u8, u8),
}

// pub to disable unused warnings
pub const BOARD_VERSION_0: u8 = 0;
pub const BOARD_VERSION_1: u8 = 1;
pub const BOARD_VERSION_2: u8 = 2;
pub const BOARD_VERSION_3: u8 = 3;
pub const BOARD_VERSION_4: u8 = 4;
pub const BOARD_VERSION_5: u8 = 5;
pub const BOARD_VERSION_6: u8 = 6;
pub const BOARD_VERSION_7: u8 = 7;
pub const BOARD_VERSION_8: u8 = 8;
pub const BOARD_VERSION_9: u8 = 9;
pub const BOARD_VERSION_10: u8 = 10;
pub const BOARD_VERSION_11: u8 = 11;
pub const BOARD_VERSION_12: u8 = 12;
pub const BOARD_VERSION_13: u8 = 13;
pub const BOARD_VERSION_14: u8 = 14;
pub const BOARD_VERSION_15: u8 = 15;

#[repr(C, packed)]
pub struct EcRequestExpansionBayStatus {}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseExpansionBayStatus {
    pub state: u8,
    pub board_id_0: u8,
    pub board_id_1: u8,
}

impl EcResponseExpansionBayStatus {
    pub fn module_enabled(&self) -> bool {
        self.state & (ExpansionByStates::ModuleEnabled as u8) != 0
    }
    pub fn module_fault(&self) -> bool {
        self.state & (ExpansionByStates::ModuleFault as u8) != 0
    }
    pub fn hatch_switch_closed(&self) -> bool {
        self.state & (ExpansionByStates::HatchSwitchClosed as u8) != 0
    }
    pub fn expansion_bay_board(&self) -> Result<ExpansionBayBoard, ExpansionBayIssue> {
        match (self.board_id_0, self.board_id_1) {
            (BOARD_VERSION_12, BOARD_VERSION_12) => Ok(ExpansionBayBoard::DualInterposer),
            (BOARD_VERSION_13, BOARD_VERSION_15) => Ok(ExpansionBayBoard::UmaFans),
            (BOARD_VERSION_11, BOARD_VERSION_15) => Ok(ExpansionBayBoard::SingleInterposer),
            (BOARD_VERSION_15, BOARD_VERSION_15) => Err(ExpansionBayIssue::NoModule),
            // Invalid board IDs. Something wrong, could be interposer not connected
            _ => Err(ExpansionBayIssue::BadConnection(
                self.board_id_0,
                self.board_id_1,
            )),
        }
    }
}

impl EcRequest<EcResponseExpansionBayStatus> for EcRequestExpansionBayStatus {
    fn command_id() -> EcCommands {
        EcCommands::ExpansionBayStatus
    }
}

pub const DIAGNOSTICS_START: usize = 0;
pub const DIAGNOSTICS_HW_NO_BATTERY: usize = 1;
pub const DIAGNOSTICS_HW_PGOOD_3V5V: usize = 2;
pub const DIAGNOSTICS_VCCIN_AUX_VR: usize = 3;
pub const DIAGNOSTICS_SLP_S4: usize = 4;
pub const DIAGNOSTICS_HW_PGOOD_VR: usize = 5;

// Lotus: Start
pub const DIAGNOSTICS_INPUT_MODULE_FAULT: usize = 6;
pub const DIAGNOSTICS_NO_LEFT_FAN: usize = 7;
pub const DIAGNOSTICS_NO_RIGHT_FAN: usize = 8;
pub const DIAGNOSTICS_GPU_MODULE_FAULT: usize = 9;
// Lotus: End
// Azalea: Start
pub const DIAGNOSTICS_TOUCHPAD: usize = 6;
pub const DIAGNOSTICS_AUDIO_DAUGHTERBOARD: usize = 7;
pub const DIAGNOSTICS_THERMAL_SENSOR: usize = 8;
pub const DIAGNOSTICS_NOFAN: usize = 9;
// Azalea: End

// Different on azalea and lotus
// pub const DIAGNOSTICS_NO_S0: usize = 10;
// pub const DIAGNOSTICS_NO_DDR: usize = 11;
// pub const DIAGNOSTICS_NO_EDP: usize = 12;
// pub const DIAGNOSTICS_HW_FINISH: usize = 13;
/*BIOS BITS*/
// pub const DIAGNOSTICS_BIOS_BIT0: usize = 18;
// pub const DIAGNOSTICS_BIOS_BIT1: usize = 19;
// pub const DIAGNOSTICS_BIOS_BIT2: usize = 21;
// pub const DIAGNOSTICS_BIOS_BIT3: usize = 22;
// pub const DIAGNOSTICS_BIOS_BIT4: usize = 23;
// pub const DIAGNOSTICS_BIOS_BIT5: usize = 24;
// pub const DIAGNOSTICS_BIOS_BIT6: usize = 25;
// pub const DIAGNOSTICS_BIOS_BIT7: usize = 26;

#[repr(C, packed)]
pub struct EcRequestGetHwDiag {}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseGetHwDiag {
    pub hw_diag: u32,
    pub bios_complete: u8,
}

impl EcResponseGetHwDiag {
    pub fn fan_fault(&self) -> (bool, bool) {
        (
            self.hw_diag & (1 << DIAGNOSTICS_NO_LEFT_FAN) != 0,
            self.hw_diag & (1 << DIAGNOSTICS_NO_RIGHT_FAN) != 0,
        )
    }
}
impl fmt::Display for EcResponseGetHwDiag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (left_fan, right_fan) = self.fan_fault();
        write!(
            f,
            "BIOS Done: {}, Fan Fault Left: {}, Right: {}",
            self.bios_complete != 0,
            left_fan,
            right_fan
        )
    }
}

impl EcRequest<EcResponseGetHwDiag> for EcRequestGetHwDiag {
    fn command_id() -> EcCommands {
        EcCommands::GetHwDiag
    }
}

#[repr(u8)]
pub enum ChargeLimitControlModes {
    /// Disable all settings, handled automatically
    Disable = 0x01,
    /// Set maxiumum and minimum percentage
    Set = 0x02,
    /// Get current setting
    /// ATTENTION!!! This is the only mode that will return a response
    Get = 0x08,
    /// Allow charge to full this time
    Override = 0x80,
}

#[repr(C, packed)]
pub struct EcRequestChargeLimitControl {
    pub modes: u8,
    pub max_percentage: u8,
    pub min_percentage: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseChargeLimitControl {
    pub max_percentage: u8,
    pub min_percentage: u8,
}

impl EcRequest<EcResponseChargeLimitControl> for EcRequestChargeLimitControl {
    fn command_id() -> EcCommands {
        EcCommands::ChargeLimitControl
    }
}

/*
 * Configure the behavior of the charge limit control.
 * TODO: Use this
 */
pub const EC_CHARGE_LIMIT_RESTORE: u8 = 0x7F;

#[repr(u8)]
#[derive(Debug, FromPrimitive)]
pub enum FpLedBrightnessLevel {
    High = 0,
    Medium = 1,
    Low = 2,
}

#[repr(C, packed)]
pub struct EcRequestFpLedLevelControl {
    /// See enum FpLedBrightnessLevel
    pub set_level: u8,
    /// Boolean. >1 to get the level
    pub get_level: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseFpLedLevelControl {
    pub level: u8,
}

impl EcRequest<EcResponseFpLedLevelControl> for EcRequestFpLedLevelControl {
    fn command_id() -> EcCommands {
        EcCommands::FpLedLevelControl
    }
}
