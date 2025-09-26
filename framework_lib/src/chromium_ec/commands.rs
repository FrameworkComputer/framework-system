use core::fmt;

use num_derive::FromPrimitive;

use super::{command::*, input_deck::INPUT_DECK_SLOTS};
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

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

pub struct EcRequestFlashInfo {}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct EcResponseFlashInfo {
    pub flash_size: u32,
    pub write_block_size: u32,
    pub erase_block_size: u32,
    pub protect_block_size: u32,
    // New fields in version 1 of the command
    pub write_ideal_size: u32,
    pub flags: u32,
}

impl EcRequest<EcResponseFlashInfo> for EcRequestFlashInfo {
    fn command_version() -> u8 {
        1
    }
    fn command_id() -> EcCommands {
        EcCommands::FlashInfo
    }
}

pub struct EcRequestFlashRead {
    pub offset: u32,
    pub size: u32,
}

impl EcRequest<()> for EcRequestFlashRead {
    fn command_id() -> EcCommands {
        EcCommands::FlashRead
    }
}

#[repr(C, packed)]
pub struct EcRequestFlashWrite {
    pub offset: u32,
    pub size: u32,
    /// Dynamically sized array (data copied after this struct)
    pub data: [u8; 0],
}
impl EcRequest<()> for EcRequestFlashWrite {
    fn command_id() -> EcCommands {
        EcCommands::FlashWrite
    }
}

#[repr(C, packed)]
pub struct EcRequestFlashErase {
    pub offset: u32,
    pub size: u32,
}

impl EcRequest<()> for EcRequestFlashErase {
    fn command_id() -> EcCommands {
        EcCommands::FlashErase
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FlashProtectFlags {
    ProtectRoAtBoot = 1 << 0,
    ProtectRoNow = 1 << 1,
    ProtectAllNow = 1 << 2,
    ProtectGpioAsserted = 1 << 3,
    /// At least one flash bank is stuck and can't be unlocked
    ErrorStruck = 1 << 4,
    ErrorInconsistent = 1 << 5,
    ProtectAllAtBoot = 1 << 6,
}

#[repr(C, packed)]
pub struct EcRequestFlashProtect {
    pub mask: u32,
    pub flags: u32,
}

pub struct EcResponseFlashProtect {
    /// Current flash protect flags
    pub flags: u32,
    /// Flags that are valid on this platform
    pub valid_flags: u32,
    /// Flags that can be currently written (depending on protection status)
    pub writeable_flags: u32,
}

impl EcRequest<EcResponseFlashProtect> for EcRequestFlashProtect {
    fn command_id() -> EcCommands {
        EcCommands::FlashProtect
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

#[repr(u8)]
pub enum PwmType {
    Generic = 0,
    KbLight,
    DisplayLight,
}

impl EcRequest<EcResponsePwmGetKeyboardBacklight> for EcRequestPwmGetKeyboardBacklight {
    fn command_id() -> EcCommands {
        EcCommands::PwmGetKeyboardBacklight
    }
}

#[repr(C, packed)]
pub struct EcRequestPwmSetFanTargetRpmV0 {
    /// Duty cycle in percent
    pub rpm: u32,
}

impl EcRequest<()> for EcRequestPwmSetFanTargetRpmV0 {
    fn command_id() -> EcCommands {
        EcCommands::PwmSetFanTargetRpm
    }
}

pub struct EcRequestPwmSetFanTargetRpmV1 {
    /// Fan RPM
    pub rpm: u32,
    /// Fan index
    pub fan_idx: u32,
}

impl EcRequest<()> for EcRequestPwmSetFanTargetRpmV1 {
    fn command_id() -> EcCommands {
        EcCommands::PwmSetFanTargetRpm
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequestPwmSetFanDutyV0 {
    /// Duty cycle in percent
    pub percent: u32,
}

impl EcRequest<()> for EcRequestPwmSetFanDutyV0 {
    fn command_id() -> EcCommands {
        EcCommands::PwmSetFanDuty
    }
}

#[repr(C, packed)]
pub struct EcRequestPwmSetFanDutyV1 {
    /// Duty cycle in percent
    pub percent: u32,
    /// Fan index
    pub fan_idx: u32,
}

impl EcRequest<()> for EcRequestPwmSetFanDutyV1 {
    fn command_id() -> EcCommands {
        EcCommands::PwmSetFanDuty
    }
    fn command_version() -> u8 {
        1
    }
}

pub const PWM_MAX_DUTY: u16 = 0xFFFF;

#[repr(C, packed)]
pub struct EcRequestPwmSetDuty {
    /// Duty cycle, min 0, max 0xFFFF
    pub duty: u16,
    /// See enum PwmType
    pub pwm_type: u8,
    /// Type-specific index, or 0 if unique
    pub index: u8,
}

impl EcRequest<()> for EcRequestPwmSetDuty {
    fn command_id() -> EcCommands {
        EcCommands::PwmSetDuty
    }
}

#[repr(C, packed)]
pub struct EcRequestPwmGetDuty {
    /// See enum PwmType
    pub pwm_type: u8,
    /// Type-specific index, or 0 if unique
    pub index: u8,
}

#[repr(C, packed)]
pub struct EcResponsePwmGetDuty {
    /// Duty cycle, min 0, max 0xFFFF
    pub duty: u16,
}

impl EcRequest<EcResponsePwmGetDuty> for EcRequestPwmGetDuty {
    fn command_id() -> EcCommands {
        EcCommands::PwmGetDuty
    }
}

#[repr(u8)]
pub enum MotionSenseCmd {
    Dump = 0,
    Info = 1,
}

#[repr(C, packed)]
pub struct EcRequestMotionSenseDump {
    /// MotionSenseCmd::Dump
    pub cmd: u8,
    /// Maximal number of sensor the host is expecting.
    /// 0 means the host is only interested in the number
    /// of sensors controlled by the EC.
    pub max_sensor_count: u8,
}

#[repr(C, packed)]
pub struct EcResponseMotionSenseDump {
    /// Flags representing the motion sensor module
    pub module_flags: u8,

    /// Number of sensors managed directly by the EC
    pub sensor_count: u8,

    /// Sensor data is truncated if response_max is too small
    /// for holding all the data.
    pub sensor: [u8; 0],
}

impl EcRequest<EcResponseMotionSenseDump> for EcRequestMotionSenseDump {
    fn command_id() -> EcCommands {
        EcCommands::MotionSense
    }
    fn command_version() -> u8 {
        1
    }
}

#[derive(Debug, FromPrimitive, PartialEq)]
pub enum MotionSenseType {
    Accel = 0,
    Gyro = 1,
    Mag = 2,
    Prox = 3,
    Light = 4,
    Activity = 5,
    Baro = 6,
    Sync = 7,
    LightRgb = 8,
}

#[derive(Debug, FromPrimitive)]
pub enum MotionSenseLocation {
    Base = 0,
    Lid = 1,
    Camera = 2,
}

#[derive(Debug, FromPrimitive)]
pub enum MotionSenseChip {
    Kxcj9 = 0,
    Lsm6ds0 = 1,
    Bmi160 = 2,
    Si1141 = 3,
    Si1142 = 4,
    Si1143 = 5,
    Kx022 = 6,
    L3gd20h = 7,
    Bma255 = 8,
    Bmp280 = 9,
    Opt3001 = 10,
    Bh1730 = 11,
    Gpio = 12,
    Lis2dh = 13,
    Lsm6dsm = 14,
    Lis2de = 15,
    Lis2mdl = 16,
    Lsm6ds3 = 17,
    Lsm6dso = 18,
    Lng2dm = 19,
    Tcs3400 = 20,
    Lis2dw12 = 21,
    Lis2dwl = 22,
    Lis2ds = 23,
    Bmi260 = 24,
    Icm426xx = 25,
    Icm42607 = 26,
    Bma422 = 27,
    Bmi323 = 28,
    Bmi220 = 29,
    Cm32183 = 30,
    Veml3328 = 31,
}

#[repr(C, packed)]
pub struct EcRequestMotionSenseInfo {
    /// MotionSenseCmd::Info
    pub cmd: u8,
    /// Sensor index
    pub sensor_num: u8,
}

#[repr(C)]
pub struct EcResponseMotionSenseInfo {
    /// See enum MotionSenseInfo
    pub sensor_type: u8,
    /// See enum MotionSenseLocation
    pub location: u8,
    /// See enum MotionSenseChip
    pub chip: u8,
}

#[derive(Debug)]
pub struct MotionSenseInfo {
    pub sensor_type: MotionSenseType,
    pub location: MotionSenseLocation,
    pub chip: MotionSenseChip,
}

impl EcRequest<EcResponseMotionSenseInfo> for EcRequestMotionSenseInfo {
    fn command_id() -> EcCommands {
        EcCommands::MotionSense
    }
    fn command_version() -> u8 {
        1
    }
}

pub enum TabletModeOverride {
    Default = 0,
    ForceTablet = 1,
    ForceClamshell = 2,
}

#[repr(C, packed)]
pub struct EcRequestSetTabletMode {
    /// See TabletModeOverride
    pub mode: u8,
}

impl EcRequest<()> for EcRequestSetTabletMode {
    fn command_id() -> EcCommands {
        EcCommands::SetTabletMode
    }
}

#[repr(C, packed)]
pub struct EcRequestAutoFanCtrlV0 {}

impl EcRequest<()> for EcRequestAutoFanCtrlV0 {
    fn command_id() -> EcCommands {
        EcCommands::AutoFanCtrl
    }
}

#[repr(C, packed)]
pub struct EcRequestAutoFanCtrlV1 {
    /// Fan id
    pub fan_idx: u8,
}

impl EcRequest<()> for EcRequestAutoFanCtrlV1 {
    fn command_id() -> EcCommands {
        EcCommands::AutoFanCtrl
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequestGpioGetV0 {
    pub name: [u8; 32],
}

#[repr(C, packed)]
pub struct EcResponseGpioGetV0 {
    pub val: u8,
}

impl EcRequest<EcResponseGpioGetV0> for EcRequestGpioGetV0 {
    fn command_id() -> EcCommands {
        EcCommands::GpioGet
    }
    fn command_version() -> u8 {
        0
    }
}

pub enum GpioGetSubCommand {
    ByName = 0,
    Count = 1,
    Info = 2,
}

#[repr(C, packed)]
pub struct EcRequestGpioGetV1Count {
    pub subcmd: u8,
}

#[repr(C, packed)]
pub struct EcRequestGpioGetV1ByName {
    pub subcmd: u8,
    pub name: [u8; 32],
}

#[repr(C, packed)]
pub struct EcRequestGpioGetV1Info {
    pub subcmd: u8,
    pub index: u8,
}

#[repr(C)]
pub struct EcResponseGpioGetV1Info {
    pub val: u8,
    pub name: [u8; 32],
    pub flags: u32,
}

impl EcRequest<EcResponseGpioGetV0> for EcRequestGpioGetV1Count {
    fn command_id() -> EcCommands {
        EcCommands::GpioGet
    }
    fn command_version() -> u8 {
        1
    }
}
impl EcRequest<EcResponseGpioGetV0> for EcRequestGpioGetV1ByName {
    fn command_id() -> EcCommands {
        EcCommands::GpioGet
    }
    fn command_version() -> u8 {
        1
    }
}
impl EcRequest<EcResponseGpioGetV1Info> for EcRequestGpioGetV1Info {
    fn command_id() -> EcCommands {
        EcCommands::GpioGet
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequestReboot {}

impl EcRequest<()> for EcRequestReboot {
    fn command_id() -> EcCommands {
        EcCommands::Reboot
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

#[repr(u8)]
pub enum ChargeStateCmd {
    GetState = 0,
    GetParam,
    SetParam,
    NumCmds,
}

#[repr(C, packed)]
pub struct EcRequestChargeStateGetV0 {
    pub cmd: u8,
    pub param: u32,
}

#[repr(C, packed)]
pub struct EcResponseChargeStateGetV0 {
    pub ac: u32,
    pub chg_voltage: u32,
    pub chg_current: u32,
    pub chg_input_current: u32,
    pub batt_state_of_charge: u32,
}

impl EcRequest<EcResponseChargeStateGetV0> for EcRequestChargeStateGetV0 {
    fn command_id() -> EcCommands {
        EcCommands::ChargeState
    }
    fn command_version() -> u8 {
        0
    }
}

pub struct EcRequestCurrentLimitV0 {
    /// Current limit in mA
    pub current: u32,
}

impl EcRequest<()> for EcRequestCurrentLimitV0 {
    fn command_id() -> EcCommands {
        EcCommands::ChargeCurrentLimit
    }
}

pub struct EcRequestCurrentLimitV1 {
    /// Current limit in mA
    pub current: u32,
    /// Battery state of charge is the minimum charge percentage at which
    /// the battery charge current limit will apply.
    /// When not set, the limit will apply regardless of state of charge.
    pub battery_soc: u8,
}

impl EcRequest<()> for EcRequestCurrentLimitV1 {
    fn command_id() -> EcCommands {
        EcCommands::ChargeCurrentLimit
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequesetHibernationDelay {
    /// Seconds in G3 after EC turns off, 0 to read current
    pub seconds: u32,
}

#[repr(C, packed)]
pub struct EcResponseHibernationDelay {
    pub time_g3: u32,
    pub time_remaining: u32,
    /// How long to wait in G3 until turn off
    pub hibernation_delay: u32,
}

impl EcRequest<EcResponseHibernationDelay> for EcRequesetHibernationDelay {
    fn command_id() -> EcCommands {
        EcCommands::HibernationDelay
    }
}

/// Supported features
#[derive(Debug, FromPrimitive)]
pub enum EcFeatureCode {
    /// This image contains a limited set of features. Another image
    /// in RW partition may support more features.
    Limited = 0,
    /// Commands for probing/reading/writing/erasing the flash in the
    /// EC are present.
    Flash = 1,
    /// Can control the fan speed directly.
    PwmFan = 2,
    /// Can control the intensity of the keyboard backlight.
    PwmKeyboardBacklight = 3,
    /// Support Google lightbar, introduced on Pixel.
    Lightbar = 4,
    /// Control of LEDs
    Led = 5,
    /// Exposes an interface to control gyro and sensors.
    /// The host goes through the EC to access these sensors.
    /// In addition, the EC may provide composite sensors, like lid angle.
    MotionSense = 6,
    /// The keyboard is controlled by the EC
    Keyboard = 7,
    /// The AP can use part of the EC flash as persistent storage.
    PersistentStorage = 8,
    /// The EC monitors BIOS port 80h, and can return POST codes.
    Port80 = 9,
    /// Thermal management: include TMP specific commands.
    /// Higher level than direct fan control.
    Thermal = 10,
    /// Can switch the screen backlight on/off
    BacklightSwitch = 11,
    /// Can switch the wifi module on/off
    WifiSwitch = 12,
    /// Monitor host events, through for example SMI or SCI
    HostEvents = 13,
    /// The EC exposes GPIO commands to control/monitor connected devices.
    Gpio = 14,
    /// The EC can send i2c messages to downstream devices.
    I2c = 15,
    /// Command to control charger are included
    Charger = 16,
    /// Simple battery support.
    Battery = 17,
    /// Support Smart battery protocol
    /// (Common Smart Battery System Interface Specification)
    SmartBattery = 18,
    /// EC can detect when the host hangs.
    HangDetect = 19,
    /// Report power information, for pit only
    Pmu = 20,
    /// Another Cros EC device is present downstream of this one
    SubMcu = 21,
    /// Support USB Power delivery (PD) commands
    UsbPd = 22,
    /// Control USB multiplexer, for audio through USB port for instance.
    UsbMux = 23,
    /// Motion Sensor code has an internal software FIFO
    MotionSenseFifo = 24,
    /// Support temporary secure vstore
    SecureVstore = 25,
    /// EC decides on USB-C SS mux state, muxes configured by host
    UsbcSsMuxVirtual = 26,
    /// EC has RTC feature that can be controlled by host commands
    Rtc = 27,
    /// The MCU exposes a Fingerprint sensor
    Fingerprint = 28,
    /// The MCU exposes a Touchpad
    Touchpad = 29,
    /// The MCU has RWSIG task enabled
    RwSig = 30,
    /// EC has device events support
    DeviceEvent = 31,
    /// EC supports the unified wake masks for LPC/eSPI systems
    UnifiedWakeMasks = 32,
    /// EC supports 64-bit host events
    HostEvent64 = 33,
    /// EC runs code in RAM (not in place, a.k.a. XIP)
    ExecInRam = 34,
    /// EC supports CEC commands
    Cec = 35,
    /// EC supports tight sensor timestamping.
    MotionSenseTightTimesStamps = 36,
    ///
    /// EC supports tablet mode detection aligned to Chrome and allows
    /// setting of threshold by host command using
    /// MOTIONSENSE_CMD_TABLET_MODE_LID_ANGLE.
    RefinedTabletModeHysteresis = 37,
    /// Early Firmware Selection ver.2. Enabled by CONFIG_VBOOT_EFS2.
    /// Note this is a RO feature. So, a query (EC_CMD_GET_FEATURES) should
    /// be sent to RO to be precise.
    Efs2 = 38,
    /// The MCU is a System Companion Processor (SCP).
    Scp = 39,
    /// The MCU is an Integrated Sensor Hub
    Ish = 40,
    /// New TCPMv2 TYPEC_ prefaced commands supported
    TypecCmd = 41,
    /// The EC will wait for direction from the AP to enter Type-C alternate
    /// modes or USB4.
    TypecRequireApModeEntry = 42,
    /// The EC will wait for an acknowledge from the AP after setting the
    /// mux.
    TypeCMuxRequireApAck = 43,
    /// The EC supports entering and residing in S4.
    S4Residency = 44,
    /// The EC supports the AP directing mux sets for the board.
    TypeCApMuxSet = 45,
    /// The EC supports the AP composing VDMs for us to send.
    TypeCApVdmSend = 46,
    /// The EC supports system safe mode panic recovery.
    SystemSafeMode = 47,
    /// The EC will reboot on runtime assertion failures.
    AssertReboots = 48,
    /// The EC image is built with tokenized logging enabled.
    TokenizedLogging = 49,
    /// The EC supports triggering an STB dump.
    AmdStbDump = 50,
    /// The EC supports memory dump commands.
    MemoryDump = 51,
    /// The EC supports DP2.1 capability
    Dp21 = 52,
    /// The MCU is System Companion Processor Core 1
    ScpC1 = 53,
    /// The EC supports UCSI PPM.
    UcsiPpm = 54,
}

pub struct EcRequestGetFeatures {}

pub struct EcResponseGetFeatures {
    pub flags: [u32; 2],
}

impl EcRequest<EcResponseGetFeatures> for EcRequestGetFeatures {
    fn command_id() -> EcCommands {
        EcCommands::GetFeatures
    }
}

#[repr(u8)]
pub enum RebootEcCmd {
    /// Cancel a pending reboot
    Cancel = 0,
    /// Jump to RO firmware without rebooting
    JumpRo = 1,
    /// Jump to RW firmware without rebooting
    JumpRw = 2,
    /// DEPRECATED: Was jump to RW-B
    DeprecatedJumpToRwB = 3,
    /// Cold reboot of the EC. Causes host reset as well
    ColdReboot = 4,
    /// Disable jumping until the next EC reboot
    DisableJump = 5,
    /// Hibernate the EC
    Hibernate = 6,
    /// DEPRECATED: Hibernate EC and clears AP_IDLE flag.
    /// Use EC_REBOOT_HIBERNATE and EC_REBOOT_FLAG_CLEAR_AP_IDLE, instead.
    DeprecatedClearApOff = 7,
    ///  Cold-reboot and don't boot AP
    ColdApOff = 8,
    /// Do nothing but apply the flags
    NoOp = 9,
}

#[repr(u8)]
pub enum RebootEcFlags {
    /// Default
    None = 0x00,
    DeprecatedRecoveryRequest = 0x01,
    /// Reboot after AP shutdown
    OnApShutdown = 0x02,
    /// Switch RW slot
    SwitchRwSlot = 0x04,
    /// Clear AP_IDLE flag
    ClearApidle = 0x08,
}

pub struct EcRequestRebootEc {
    /// See enum RebootEcCmd
    pub cmd: u8,
    pub flags: u8,
}

impl EcRequest<()> for EcRequestRebootEc {
    fn command_id() -> EcCommands {
        EcCommands::RebootEc
    }
    fn command_version() -> u8 {
        0
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

#[repr(C, packed)]
pub struct EcRequestAdcRead {
    /// ADC Channel, specific to each mainboard schematic
    pub adc_channel: u8,
}

pub struct EcResponseAdcRead {
    pub adc_value: i32,
}

impl EcRequest<EcResponseAdcRead> for EcRequestAdcRead {
    fn command_id() -> EcCommands {
        EcCommands::AdcRead
    }
}

#[repr(C)]
pub struct EcRequestApReset {}

impl EcRequest<()> for EcRequestApReset {
    fn command_id() -> EcCommands {
        EcCommands::ApReset
    }
}

#[repr(C)]
pub struct EcRequestRebootApOnG3V0 {}

impl EcRequest<()> for EcRequestRebootApOnG3V0 {
    fn command_id() -> EcCommands {
        EcCommands::RebootApOnG3
    }
    fn command_version() -> u8 {
        0
    }
}

#[repr(C)]
pub struct EcRequestRebootApOnG3V1 {
    /// Delay in seconds after entering G3 state
    pub delay: u32,
}

impl EcRequest<()> for EcRequestRebootApOnG3V1 {
    fn command_id() -> EcCommands {
        EcCommands::RebootApOnG3
    }
    fn command_version() -> u8 {
        1
    }
}

// TODO: Actually 128, but if we go above ~80 EC returns REQUEST_TRUNCATED
// At least when I use the portio driver
pub const EC_RGBKBD_MAX_KEY_COUNT: usize = 64;

#[repr(C, packed)]
#[derive(Default, Clone, Copy, Debug)]
pub struct RgbS {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[repr(C, packed)]
pub struct EcRequestRgbKbdSetColor {
    /// Specifies the starting key ID whose color is being changed
    pub start_key: u8,
    /// Specifies # of elements in color
    pub length: u8,
    /// RGB color data array of length up to MAX_KEY_COUNT
    pub color: [RgbS; EC_RGBKBD_MAX_KEY_COUNT],
}

#[repr(C, packed)]
pub struct EcResponseRgbKbdSetColor {}

impl EcRequest<EcResponseRgbKbdSetColor> for EcRequestRgbKbdSetColor {
    fn command_id() -> EcCommands {
        EcCommands::RgbKbdSetColor
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
pub struct KeyboardMatrixMap {
    pub row: u8,
    pub col: u8,
    pub scanset: u16,
}
#[repr(C, packed)]
pub struct EcRequestUpdateKeyboardMatrix {
    pub num_items: u32,
    pub write: u32,
    pub scan_update: [KeyboardMatrixMap; 1],
}
#[repr(C, packed)]
pub struct EcResponseUpdateKeyboardMatrix {
    pub num_items: u32,
    pub write: u32,
    pub scan_update: [KeyboardMatrixMap; 32],
}

impl EcRequest<EcResponseUpdateKeyboardMatrix> for EcRequestUpdateKeyboardMatrix {
    fn command_id() -> EcCommands {
        EcCommands::UpdateKeyboardMatrix
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
pub struct EcRequestReadPdVersionV0 {}

#[repr(C, packed)]
pub struct _EcResponseReadPdVersionV0 {
    pub controller01: [u8; 8],
    pub controller23: [u8; 8],
}

impl EcRequest<_EcResponseReadPdVersionV0> for EcRequestReadPdVersionV0 {
    fn command_id() -> EcCommands {
        EcCommands::ReadPdVersion
    }
    fn command_version() -> u8 {
        0
    }
}

#[repr(C, packed)]
pub struct EcRequestReadPdVersionV1 {}
#[repr(C, packed)]
pub struct _EcResponseReadPdVersionV1 {
    pub pd_chip_count: u8,
    pub pd_controllers: [u8; 0],
}

impl EcRequest<_EcResponseReadPdVersionV1> for EcRequestReadPdVersionV1 {
    fn command_id() -> EcCommands {
        EcCommands::ReadPdVersion
    }
    fn command_version() -> u8 {
        1
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
    pub device_complete: u8,
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

/// Configure the behavior of the charge limit control.
/// TODO: Use this
pub const EC_CHARGE_LIMIT_RESTORE: u8 = 0x7F;

#[repr(C, packed)]
pub struct EcRequestDisablePs2Emulation {
    pub disable: u8,
}

impl EcRequest<()> for EcRequestDisablePs2Emulation {
    fn command_id() -> EcCommands {
        EcCommands::DisablePs2Emulation
    }
}

#[repr(u8)]
#[derive(Debug, FromPrimitive)]
pub enum FpLedBrightnessLevel {
    High = 0,
    Medium = 1,
    Low = 2,
    UltraLow = 3,
    /// Custom: Only get, never set
    Custom = 0xFE,
    Auto = 0xFF,
}

#[repr(C, packed)]
pub struct EcRequestFpLedLevelControlV0 {
    /// See enum FpLedBrightnessLevel
    pub set_level: u8,
    /// Boolean. >1 to get the level
    pub get_level: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseFpLedLevelControlV0 {
    /// Current brightness, 1-100%
    pub percentage: u8,
}

impl EcRequest<EcResponseFpLedLevelControlV0> for EcRequestFpLedLevelControlV0 {
    fn command_id() -> EcCommands {
        EcCommands::FpLedLevelControl
    }
    fn command_version() -> u8 {
        0
    }
}

#[repr(C, packed)]
pub struct EcRequestFpLedLevelControlV1 {
    /// Percentage 1-100
    pub set_percentage: u8,
    /// Boolean. >1 to get the level
    pub get_level: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseFpLedLevelControlV1 {
    /// Current brightness, 1-100%
    pub percentage: u8,
    /// Requested level. See enum FpLedBrightnessLevel
    pub level: u8,
}

impl EcRequest<EcResponseFpLedLevelControlV1> for EcRequestFpLedLevelControlV1 {
    fn command_id() -> EcCommands {
        EcCommands::FpLedLevelControl
    }
    fn command_version() -> u8 {
        1
    }
}

#[repr(C, packed)]
pub struct EcRequestGetGpuSerial {
    pub idx: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseGetGpuSerial {
    pub idx: u8,
    pub valid: u8,
    pub serial: [u8; 20],
}

impl EcRequest<EcResponseGetGpuSerial> for EcRequestGetGpuSerial {
    fn command_id() -> EcCommands {
        EcCommands::GetGpuSerial
    }
}

#[repr(C, packed)]
pub struct EcRequestGetGpuPcie {}

#[repr(u8)]
#[derive(Debug, FromPrimitive)]
pub enum GpuPcieConfig {
    /// PCIe 8x1
    Pcie8x1 = 0,
    /// PCIe 4x1
    Pcie4x1 = 1,
    /// PCIe 4x2
    Pcie4x2 = 2,
}

#[repr(u8)]
#[derive(Debug, FromPrimitive, PartialEq)]
pub enum GpuVendor {
    Initializing = 0x00,
    FanOnly = 0x01,
    GpuAmdR23M = 0x02,
    SsdHolder = 0x03,
    PcieAccessory = 0x4,
    NvidiaGn22 = 0x5,
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseGetGpuPcie {
    pub gpu_pcie_config: u8,
    pub gpu_vendor: u8,
}

impl EcRequest<EcResponseGetGpuPcie> for EcRequestGetGpuPcie {
    fn command_id() -> EcCommands {
        EcCommands::GetGpuPcie
    }
}

#[repr(u8)]
pub enum SetGpuSerialMagic {
    /// 7700S config magic value
    WriteGPUConfig = 0x0D,
    /// SSD config magic value
    WriteSSDConfig = 0x55,
}

#[repr(C, packed)]
pub struct EcRequestSetGpuSerial {
    pub magic: u8,
    pub idx: u8,
    pub serial: [u8; 20],
}

#[repr(C, packed)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EcResponseSetGpuSerial {
    pub valid: u8,
}

impl EcRequest<EcResponseSetGpuSerial> for EcRequestSetGpuSerial {
    fn command_id() -> EcCommands {
        EcCommands::ProgramGpuEeprom
    }
}
