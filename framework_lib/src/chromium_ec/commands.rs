use super::command::*;

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
