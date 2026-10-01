//! Communicate with CCGX (CCG5, CCG6, CCG8) PD controllers
//!
//! The current implementation talks to them by tunneling I2C through EC host commands.

use alloc::format;
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::ccgx::{AppVersion, BaseVersion, ControllerVersion};
use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcError, EcResult};
use crate::os_specific;
use crate::util::{assert_win_len, Config, Platform};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use super::binary::{PdFirmware, PdFirmwareFile};
use super::*;

const HPI_FLASH_ENTER_SIGNATURE: char = 'P';
const HPI_JUMP_TO_ALT_SIGNATURE: char = 'A';
const HPI_JUMP_TO_BOOT_SIGNATURE: char = 'J';
const HPI_RESET_SIGNATURE: char = 'R';
const HPI_FLASH_RW_SIGNATURE: char = 'F';
const HPI_RESET_DEV_CMD: u8 = 1;
const HPI_FLASH_READ_CMD: u8 = 0;
const HPI_FLASH_WRITE_CMD: u8 = 1;

/// Response codes in the device RESPONSE register (HPI spec 4.1.1)
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
enum HpiResponse {
    NoResponse = 0x00,
    Success = 0x02,
    /// Flash read successful and data is available
    FlashRead = 0x03,
    InvalidCommand = 0x05,
    InvalidState = 0x06,
    FlashOperationFailed = 0x07,
    BadFirmware = 0x08,
    BadArguments = 0x09,
    NotSupported = 0x0A,
    ResetComplete = 0x80,
    MessageQueueOverflow = 0x81,
}

/// Keeps the EC away from the PD controllers while flashing
///
/// The EC stops talking to the PD controllers when locked and re-initializes
/// them when unlocked. Unlocking happens automatically when dropped, so that
/// the EC is not left in that state if flashing fails.
struct PdBusLock<'a> {
    ec: &'a CrosEc,
}

impl<'a> PdBusLock<'a> {
    fn new(ec: &'a CrosEc) -> EcResult<Self> {
        ec.lock_pd_bus(true)?;
        Ok(Self { ec })
    }
}

impl Drop for PdBusLock<'_> {
    fn drop(&mut self) {
        if let Err(err) = self.ec.lock_pd_bus(false) {
            error!("Failed to hand PD controllers back to the EC: {:?}", err);
        }
    }
}

/// Which firmware images to update
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PdImageSelection {
    /// Update the inactive image first, then the other one
    Both,
    /// Only FW2
    Main,
    /// Only FW1
    Backup,
}

/// Why the controller booted the way it did (BOOT_MODE_REASON register)
///
/// Only updated at boot, does not reflect firmware updates done since.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BootModeReason {
    /// Firmware asked to stay in the bootloader (JUMP_TO_BOOT)
    pub boot_mode_requested: bool,
    pub fw1_invalid: bool,
    pub fw2_invalid: bool,
    pub raw: u8,
}

impl BootModeReason {
    fn from_raw(raw: u8) -> Option<Self> {
        // Bits 5-7 are reserved and always 0. Older firmware might not have
        // this register at all and return something else.
        if raw & 0xE0 != 0 {
            return None;
        }
        Some(Self {
            boot_mode_requested: raw & 0x01 != 0,
            fw1_invalid: raw & 0x04 != 0,
            fw2_invalid: raw & 0x08 != 0,
            raw,
        })
    }
}

/// HPI version and feature set of the running firmware (HPI_VERSION register)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HpiVersion {
    pub major: u8,
    pub minor: u8,
    /// Uses .cyacd2 style metadata and checksums
    pub cyacd2: bool,
    pub ucsi: bool,
    pub epr: bool,
    pub pd_commands: bool,
    pub raw: u32,
}

impl HpiVersion {
    fn from_raw(raw: u32) -> Option<Self> {
        let major = ((raw >> 4) & 0xF) as u8;
        let minor = (raw & 0xF) as u8;
        // Only HPI v1 and v2 exist. Anything else is garbage from a firmware
        // that does not implement this register.
        if raw == 0xFFFF_FFFF || !(1..=2).contains(&major) {
            return None;
        }
        Some(Self {
            major,
            minor,
            cyacd2: (raw >> 26) & 0b11 == 0b01,
            ucsi: raw & (1 << 16) != 0,
            epr: raw & (1 << 21) != 0,
            pd_commands: raw & (1 << 9) != 0,
            raw,
        })
    }
}

#[derive(Debug, Copy, Clone)]
enum ControlRegisters {
    DeviceMode = 0,
    BootModeReason = 0x01,
    SiliconId = 2, // Two bytes long, First LSB, then MSB
    BootLoaderLastRow = 0x04,
    InterruptStatus = 0x06,
    JumpToBoot = 0x07,
    ResetRequest = 0x08,
    FlashmodeEnter = 0x0A,
    ValidateFw = 0x0B,
    FlashSignature = 0x0C,
    BootLoaderVersion = 0x10,
    Firmware1Version = 0x18,
    Firmware2Version = 0x20,
    /// FW1_START and FW2_START, two bytes each
    FirmwareLocation = 0x28,
    PdPortsEnable = 0x2C,
    WdtResetCount = 0x32,
    CfgTableVersion = 0x3A,
    HpiVersion = 0x3C,
    ResponseType = 0x7E,
    FlashRwMem = 0x0200,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PdPort {
    Right01,
    Left23,
    Back,
}

impl PdPort {
    /// SMBUS/I2C Address
    fn i2c_address(&self) -> EcResult<u16> {
        let config = Config::get();
        let platform = &(*config).as_ref().unwrap().platform;
        let unsupported = Err(EcError::DeviceError(
            "Controller does not exist on this platform".to_string(),
        ));

        Ok(match (platform, self) {
            (Platform::GenericFramework((left, _, _), _), PdPort::Right01) => *left,
            (Platform::GenericFramework((_, right, _), _), PdPort::Left23) => *right,
            (Platform::GenericFramework((_, _, back), _), PdPort::Back) => *back,
            // Framework AMD Platforms (CCG8)
            (
                Platform::Framework13Amd7080
                | Platform::Framework13AmdAi300
                | Platform::Framework16Amd7080
                | Platform::IntelCoreUltra3
                | Platform::Framework16AmdAi300,
                PdPort::Right01,
            ) => 0x42,
            (
                Platform::Framework13Amd7080
                | Platform::Framework13AmdAi300
                | Platform::Framework16Amd7080
                | Platform::Framework16AmdAi300,
                PdPort::Left23,
            ) => 0x40,
            (Platform::Framework16Amd7080 | Platform::Framework16AmdAi300, PdPort::Back) => 0x42,
            (Platform::FrameworkDesktopAmdAiMax300, PdPort::Back) => 0x08,
            (Platform::FrameworkDesktopAmdAiMax300, _) => unsupported?,
            // Framework Intel Platforms (CCG5 and CCG6)
            (
                Platform::Framework12IntelGen13
                | Platform::Framework12IntelCore3
                | Platform::IntelGen11
                | Platform::IntelGen12
                | Platform::IntelGen13
                | Platform::IntelCoreUltra1,
                PdPort::Right01,
            ) => 0x08,
            (
                Platform::Framework12IntelGen13
                | Platform::Framework12IntelCore3
                | Platform::IntelGen11
                | Platform::IntelGen12
                | Platform::IntelGen13
                | Platform::IntelCoreUltra3
                | Platform::IntelCoreUltra1,
                PdPort::Left23,
            ) => 0x40,
            (Platform::UnknownSystem, _) => {
                Err(EcError::DeviceError("Unsupported platform".to_string()))?
            }
            (_, PdPort::Back) => unsupported?,
        })
    }

    /// I2C port on the EC
    fn i2c_port(&self) -> EcResult<u8> {
        let config = Config::get();
        let platform = &(*config).as_ref().unwrap().platform;
        let unsupported = Err(EcError::DeviceError(format!(
            "Controller {:?}, does not exist on {:?}",
            self, platform
        )));

        Ok(match (platform, self) {
            (Platform::GenericFramework(_, (left, _, _)), PdPort::Right01) => *left,
            (Platform::GenericFramework(_, (_, right, _)), PdPort::Left23) => *right,
            (Platform::GenericFramework(_, (_, _, back)), PdPort::Back) => *back,
            (Platform::IntelGen11, _) => 6,
            (Platform::IntelGen12 | Platform::IntelGen13, PdPort::Right01) => 6,
            (Platform::IntelGen12 | Platform::IntelGen13, PdPort::Left23) => 7,
            (
                Platform::Framework13Amd7080
                | Platform::Framework16Amd7080
                | Platform::Framework16AmdAi300
                | Platform::IntelCoreUltra1
                | Platform::IntelCoreUltra3
                | Platform::Framework13AmdAi300
                | Platform::Framework12IntelGen13
                | Platform::Framework12IntelCore3,
                PdPort::Right01,
            ) => 1,
            (
                Platform::Framework13Amd7080
                | Platform::Framework16Amd7080
                | Platform::Framework16AmdAi300
                | Platform::IntelCoreUltra1
                | Platform::IntelCoreUltra3
                | Platform::Framework13AmdAi300
                | Platform::Framework12IntelGen13
                | Platform::Framework12IntelCore3,
                PdPort::Left23,
            ) => 2,
            (Platform::Framework16Amd7080 | Platform::Framework16AmdAi300, PdPort::Back) => 5,
            (Platform::FrameworkDesktopAmdAiMax300, PdPort::Back) => 1,
            (Platform::FrameworkDesktopAmdAiMax300, _) => unsupported?,
            (Platform::UnknownSystem, _) => {
                Err(EcError::DeviceError("Unsupported platform".to_string()))?
            }
            (_, PdPort::Back) => unsupported?,
        })
    }
}

pub struct PdController {
    port: PdPort,
    ec: CrosEc,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FwMode {
    BootLoader = 0,
    /// Backup CCGX firmware (No 1)
    BackupFw = 1,
    /// Main CCGX firmware (No 2)
    MainFw = 2,
}

impl TryFrom<u8> for FwMode {
    type Error = u8;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            0 => Ok(Self::BootLoader),
            1 => Ok(Self::BackupFw),
            2 => Ok(Self::MainFw),
            _ => Err(byte),
        }
    }
}

pub fn decode_flash_row_size(mode_byte: u8) -> u16 {
    match (mode_byte & 0b0011_0000) >> 4 {
        0 => 128, // 0x80
        1 => 256, // 0x100
        2 => panic!("Reserved"),
        3 => 64, // 0x40
        x => panic!("Unexpected value: {}", x),
    }
}

impl PdController {
    pub fn new(port: PdPort, ec: CrosEc) -> Self {
        PdController { port, ec }
    }

    fn i2c_read(&self, addr: u16, len: u16) -> EcResult<EcI2cPassthruResponse> {
        trace!(
            "I2C passthrough from I2C Port {} to I2C Addr {}",
            self.port.i2c_port()?,
            self.port.i2c_address()?
        );
        // HPIv2 always uses two byte register addresses
        i2c_read_addr16(
            &self.ec,
            self.port.i2c_port()?,
            self.port.i2c_address()?,
            addr,
            len,
        )
    }

    pub fn i2c_write(&self, addr: u16, data: &[u8]) -> EcResult<EcI2cPassthruResponse> {
        trace!(
            "I2C passthrough from I2C Port {} to I2C Addr {}",
            self.port.i2c_port()?,
            self.port.i2c_address()?
        );
        i2c_write(
            &self.ec,
            self.port.i2c_port()?,
            self.port.i2c_address()?,
            addr,
            data,
        )
    }

    fn ccgx_read(&self, reg: ControlRegisters, len: u16) -> EcResult<Vec<u8>> {
        let mut data: Vec<u8> = Vec::with_capacity(len.into());

        let addr = reg as u16;
        debug!("ccgx_read(reg: {:?}, addr: {}, len(): {}", reg, addr, len);

        while data.len() < len.into() {
            let remaining = len - data.len() as u16;
            let chunk_len = std::cmp::min(MAX_I2C_CHUNK, remaining.into());
            let offset = addr + data.len() as u16;
            let i2c_response = self.i2c_read(offset, chunk_len as u16)?;
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

    fn ccgx_write(&self, reg: ControlRegisters, data: &[u8]) -> EcResult<()> {
        let addr = reg as u16;
        let mut data_written = 0;

        while data_written < data.len() {
            let chunk_len = std::cmp::min(MAX_I2C_CHUNK, data.len() - data_written);
            let buffer = &data[data_written..data_written + chunk_len];
            let offset = addr + data_written as u16;

            let i2c_response = self.i2c_write(offset, buffer)?;

            if let Err(EcError::DeviceError(err)) = i2c_response.is_successful() {
                return Err(EcError::DeviceError(format!(
                    "I2C write was not successful: {:?}",
                    err
                )));
            }

            data_written += chunk_len;
        }

        Ok(())
    }

    pub fn get_silicon_id(&self) -> EcResult<u16> {
        let data = self.ccgx_read(ControlRegisters::SiliconId, 2)?;
        assert_win_len(data.len(), 2);
        Ok(u16::from_le_bytes([data[0], data[1]]))
    }

    /// Get device info (fw_mode, flash_row_size)
    pub fn get_device_info(&self) -> EcResult<(FwMode, u16)> {
        let data = self.ccgx_read(ControlRegisters::DeviceMode, 1)?;
        let byte = data[0];

        // Currently used firmware
        let fw_mode = match FwMode::try_from(byte & 0b0000_0011) {
            Ok(mode) => mode,
            Err(err_byte) => {
                return Err(EcError::DeviceError(format!(
                    "FW Mode invalid: {}",
                    err_byte
                )))
            }
        };

        let flash_row_size = decode_flash_row_size(byte);

        // All our devices support HPI v2 and we expect to use that to interact with them
        let hpi_v2 = (byte & (1 << 7)) > 0;
        debug_assert!(hpi_v2);

        Ok((fw_mode, flash_row_size))
    }
    pub fn get_fw_versions(&self) -> EcResult<ControllerFirmwares> {
        let (active_fw, _row_size) = self.get_device_info()?;
        Ok(ControllerFirmwares {
            active_fw,
            bootloader: self.get_single_fw_ver(FwMode::BootLoader)?,
            backup_fw: self.get_single_fw_ver(FwMode::BackupFw)?,
            main_fw: self.get_single_fw_ver(FwMode::MainFw)?,
        })
    }

    fn get_single_fw_ver(&self, mode: FwMode) -> EcResult<ControllerVersion> {
        let register = match mode {
            FwMode::BootLoader => ControlRegisters::BootLoaderVersion,
            FwMode::BackupFw => ControlRegisters::Firmware1Version,
            FwMode::MainFw => ControlRegisters::Firmware2Version,
        };
        let data = self.ccgx_read(register, 8)?;
        Ok(ControllerVersion {
            base: BaseVersion::from(&data[..4]),
            app: AppVersion::from(&data[4..]),
        })
    }

    pub fn print_fw_info(&self) {
        let data = self.ccgx_read(ControlRegisters::BootLoaderVersion, 8);
        let data = match data {
            Ok(data) => data,
            Err(err) => {
                println!("Failed to get PD Info: {:?}", err);
                return;
            }
        };

        assert_win_len(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  Bootloader Version:   Base: {},  App: {}",
            base_ver, app_ver
        );

        let data = self.ccgx_read(ControlRegisters::Firmware1Version, 8);
        let data = data.unwrap();
        assert_win_len(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  FW1 (Backup) Version: Base: {},  App: {}",
            base_ver, app_ver
        );

        let data = self.ccgx_read(ControlRegisters::Firmware2Version, 8);
        let data = data.unwrap();
        assert_win_len(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  FW2 (Main)   Version: Base: {},  App: {}",
            base_ver, app_ver
        );
    }

    /// Whether this platform has this PD controller at all
    pub fn present_on_platform(&self) -> bool {
        self.port.i2c_address().is_ok() && self.port.i2c_port().is_ok()
    }

    /// Why the controller booted into the current firmware
    ///
    /// None if the firmware does not implement the register properly.
    pub fn get_boot_mode_reason(&self) -> EcResult<Option<BootModeReason>> {
        let data = self.ccgx_read(ControlRegisters::BootModeReason, 1)?;
        Ok(BootModeReason::from_raw(data[0]))
    }

    /// Watchdog resets since the controller was powered up
    pub fn get_wdt_reset_count(&self) -> EcResult<u8> {
        let data = self.ccgx_read(ControlRegisters::WdtResetCount, 1)?;
        Ok(data[0])
    }

    /// Version of the configuration table structure (major, minor)
    pub fn get_cfg_table_version(&self) -> EcResult<(u8, u8)> {
        let data = self.ccgx_read(ControlRegisters::CfgTableVersion, 1)?;
        Ok((data[0] >> 4, data[0] & 0xF))
    }

    /// HPI version and features of the running firmware
    ///
    /// None if the firmware does not implement the register properly.
    pub fn get_hpi_version(&self) -> EcResult<Option<HpiVersion>> {
        let data = self.ccgx_read(ControlRegisters::HpiVersion, 4)?;
        assert_win_len(data.len(), 4);
        Ok(HpiVersion::from_raw(u32::from_le_bytes([
            data[0], data[1], data[2], data[3],
        ])))
    }

    /// Number of PD ports the controller has
    fn get_port_count(&self) -> EcResult<u8> {
        let data = self.ccgx_read(ControlRegisters::DeviceMode, 1)?;
        // Bits 2-3: 0 = 1 port, 1 = 2 ports
        Ok(if (data[0] >> 2) & 0b11 != 0 { 2 } else { 1 })
    }

    /// Last flash row occupied by the bootloader
    pub fn get_bootloader_last_row(&self) -> EcResult<u16> {
        let data = self.ccgx_read(ControlRegisters::BootLoaderLastRow, 2)?;
        assert_win_len(data.len(), 2);
        Ok(u16::from_le_bytes([data[0], data[1]]))
    }

    /// First flash row of FW1 and FW2, as the controller reports them
    ///
    /// If an image is invalid, the controller reports the row after the last
    /// flash row for it.
    pub fn get_fw_locations(&self) -> EcResult<(u16, u16)> {
        let data = self.ccgx_read(ControlRegisters::FirmwareLocation, 4)?;
        assert_win_len(data.len(), 4);
        Ok((
            u16::from_le_bytes([data[0], data[1]]),
            u16::from_le_bytes([data[2], data[3]]),
        ))
    }

    /// Clear the device interrupt, discarding any pending response
    fn clear_interrupt(&self) -> EcResult<()> {
        self.ccgx_write(ControlRegisters::InterruptStatus, &[0x01])
    }

    /// Wait for the device to raise its interrupt and read the response
    ///
    /// Returns NoResponse if none arrives within the timeout.
    fn poll_for_response(&self, timeout_ms: u32) -> EcResult<HpiResponse> {
        const STEP_MS: u32 = 10;
        let mut waited = 0;
        loop {
            match self.ccgx_read(ControlRegisters::InterruptStatus, 1) {
                Ok(data) if (data[0] & 0x01) != 0 => break,
                Ok(_) => {}
                // Device might be resetting, keep polling
                Err(err) => trace!("Interrupt register not readable: {:?}", err),
            }
            if waited >= timeout_ms {
                debug!("No response within {}ms", timeout_ms);
                return Ok(HpiResponse::NoResponse);
            }
            os_specific::sleep(STEP_MS as u64 * 1000);
            waited += STEP_MS;
        }

        let data = self.ccgx_read(ControlRegisters::ResponseType, 2)?;
        debug!(
            "HPI response: 0x{:02X}, length: {} (after {}ms)",
            data[0], data[1], waited
        );
        // Clear the interrupt to acknowledge the response
        self.clear_interrupt()?;

        FromPrimitive::from_u8(data[0]).ok_or_else(|| {
            EcError::DeviceError(format!("Unknown HPI response code: 0x{:02X}", data[0]))
        })
    }

    /// Send a command and wait for the device to respond
    fn command(
        &self,
        reg: ControlRegisters,
        data: &[u8],
        timeout_ms: u32,
    ) -> EcResult<HpiResponse> {
        // Make sure we don't pick up a stale response
        self.clear_interrupt()?;
        self.ccgx_write(reg, data)?;
        self.poll_for_response(timeout_ms)
    }

    /// Reset the PD controller
    ///
    /// The PD ports must be disabled first, otherwise the controller rejects
    /// the request (unless it's running the bootloader).
    pub fn reset_device(&self) -> EcResult<()> {
        println!("Resetting PD controller {:?}", self.port);
        // Handling the reset takes 5ms, booting into firmware again 200ms
        let response = self.command(
            ControlRegisters::ResetRequest,
            &[HPI_RESET_SIGNATURE as u8, HPI_RESET_DEV_CMD],
            2000,
        )?;
        match response {
            HpiResponse::ResetComplete => Ok(()),
            HpiResponse::InvalidCommand => Err(EcError::DeviceError(
                "Controller refused to reset. Are the PD ports still enabled?".to_string(),
            )),
            other => {
                // Without the PD bus lock the EC may have consumed the response
                println!("Unexpected response to reset: {:?}", other);
                Ok(())
            }
        }
    }

    /// Disable the PD ports and reset the controller, then wait for it
    ///
    /// The bootloader decides which firmware to boot, normally the main one.
    pub fn reset(&self) -> EcResult<()> {
        let _lock = PdBusLock::new(&self.ec)?;
        println!("Disabling PD ports");
        self.enable_ports(false)?;
        self.reset_device()?;
        let mode = self.wait_for_device(3000)?;
        println!("Controller is running {:?}", mode);
        Ok(())
    }

    /// Wait until the controller answers again and return its mode
    ///
    /// After a reset the bootloader answers on HPI for a short boot-wait
    /// window (50-200ms) before it starts the firmware. Only report the
    /// bootloader if it stays there.
    fn wait_for_device(&self, timeout_ms: u32) -> EcResult<FwMode> {
        const STEP_MS: u32 = 100;
        const BOOT_WAIT_GRACE_MS: u32 = 1000;
        let mut waited = 0;
        loop {
            match self.get_device_info() {
                Ok((FwMode::BootLoader, _)) if waited < BOOT_WAIT_GRACE_MS => {
                    trace!("Controller is in the bootloader, giving it time to start firmware");
                }
                Ok((mode, _)) => return Ok(mode),
                Err(err) => {
                    if waited >= timeout_ms {
                        return Err(EcError::DeviceError(format!(
                            "Controller did not come back within {}ms: {:?}",
                            timeout_ms, err
                        )));
                    }
                }
            }
            os_specific::sleep(STEP_MS as u64 * 1000);
            waited += STEP_MS;
        }
    }

    /// Enable or disable all PD ports
    ///
    /// Disabling waits until the controller confirms. That can take up to a
    /// second if it currently provides power.
    pub fn enable_ports(&self, enable: bool) -> EcResult<()> {
        let mask = if enable {
            (1u8 << self.get_port_count()?) - 1
        } else {
            0
        };
        let response = self.command(ControlRegisters::PdPortsEnable, &[mask], 2000)?;
        match response {
            HpiResponse::Success => Ok(()),
            HpiResponse::NoResponse => {
                // Without the PD bus lock the EC may have consumed the response
                debug!("No confirmation for PD port change, continuing anyway");
                Ok(())
            }
            other => Err(EcError::DeviceError(format!(
                "Failed to change PD port enable mask to {:#04b}: {:?}",
                mask, other
            ))),
        }
    }

    pub fn get_port_status(&self) -> EcResult<u8> {
        let data = self.ccgx_read(ControlRegisters::PdPortsEnable, 1)?;
        assert_win_len(data.len(), 1);
        Ok(data[0])
    }

    /// Jump to bootloader firmware
    pub fn jump_to_boot(&self) -> EcResult<()> {
        let _lock = PdBusLock::new(&self.ec)?;
        let (current_mode, _) = self.get_device_info()?;
        println!("Current mode: {:?}, jumping to BootLoader", current_mode);
        self.jump_to_other_fw(current_mode, FwMode::BootLoader)
    }

    /// Jump to backup firmware (FW1)
    pub fn jump_to_backup(&self) -> EcResult<()> {
        let _lock = PdBusLock::new(&self.ec)?;
        let (current_mode, _) = self.get_device_info()?;
        println!("Current mode: {:?}, jumping to BackupFw", current_mode);
        self.jump_to_other_fw(current_mode, FwMode::BackupFw)
    }

    /// Jump to main firmware (FW2)
    pub fn jump_to_main(&self) -> EcResult<()> {
        let _lock = PdBusLock::new(&self.ec)?;
        let (current_mode, _) = self.get_device_info()?;
        println!("Current mode: {:?}, jumping to MainFw", current_mode);
        self.jump_to_other_fw(current_mode, FwMode::MainFw)
    }

    /// Switch to a different firmware
    ///
    /// JUMP_TO_ALT_FW is a one-time switch, on the next reset the bootloader
    /// picks the firmware by its normal rules again (FW2 if it's valid).
    fn jump_to_other_fw(&self, current_mode: FwMode, target_mode: FwMode) -> EcResult<()> {
        debug!(
            "jump_to_other_fw(current_mode: {:?}, target_mode: {:?})",
            current_mode, target_mode
        );
        if current_mode == target_mode {
            println!("Already in {:?}, nothing to do", target_mode);
            return Ok(());
        }

        let mut current_mode = current_mode;
        if current_mode == FwMode::BootLoader {
            // The bootloader can't switch firmwares, it only boots the
            // preferred one. Reset to get into a regular firmware first.
            self.reset_device()?;
            current_mode = self.wait_for_device(3000)?;
            println!("After reset the controller is in {:?}", current_mode);
            if current_mode == target_mode {
                return Ok(());
            }
            if current_mode == FwMode::BootLoader {
                return Err(EcError::DeviceError(
                    "Controller stays in bootloader. Both firmwares invalid?".to_string(),
                ));
            }
        }

        // Ports must be disabled before the firmware lets go of control
        println!("Disabling PD ports");
        self.enable_ports(false)?;

        let target_sig = if target_mode == FwMode::BootLoader {
            HPI_JUMP_TO_BOOT_SIGNATURE
        } else {
            HPI_JUMP_TO_ALT_SIGNATURE
        };
        println!("Jumping from {:?} to {:?}", current_mode, target_mode);
        // The new firmware raises RESET_COMPLETE once it's up
        let response = self.command(ControlRegisters::JumpToBoot, &[target_sig as u8], 3000)?;
        match response {
            HpiResponse::ResetComplete => {}
            HpiResponse::InvalidCommand => {
                return Err(EcError::DeviceError(
                    "Controller refused to jump. Are the PD ports still enabled?".to_string(),
                ))
            }
            HpiResponse::BadFirmware => {
                return Err(EcError::DeviceError(format!(
                    "Controller refused to jump, {:?} is not valid",
                    target_mode
                )))
            }
            other => warn!("Unexpected response to jump: {:?}", other),
        }

        let new_mode = self.wait_for_device(3000)?;
        if new_mode != target_mode {
            return Err(EcError::DeviceError(format!(
                "Failed to jump to {:?}. Controller is in {:?}",
                target_mode, new_mode
            )));
        }
        println!("Now running {:?}", new_mode);
        Ok(())
    }

    /// Flash reads and writes are only accepted in flashing mode
    fn enter_flashing_mode(&self) -> EcResult<()> {
        let response = self.command(
            ControlRegisters::FlashmodeEnter,
            &[HPI_FLASH_ENTER_SIGNATURE as u8],
            1000,
        )?;
        if response != HpiResponse::Success {
            return Err(EcError::DeviceError(format!(
                "Failed to enter flashing mode: {:?}",
                response
            )));
        }
        Ok(())
    }

    fn leave_flashing_mode(&self) -> EcResult<()> {
        let response = self.command(ControlRegisters::FlashmodeEnter, &[0x00], 1000)?;
        if response != HpiResponse::Success {
            // Not critical, a reset also leaves flashing mode
            debug!("Failed to leave flashing mode: {:?}", response);
        }
        Ok(())
    }

    /// Ask the controller to check the checksum of a firmware image
    ///
    /// Requires flashing mode.
    pub fn validate_firmware(&self, mode: FwMode) -> EcResult<bool> {
        let response = self.command(ControlRegisters::ValidateFw, &[mode as u8], 2000)?;
        match response {
            HpiResponse::Success => Ok(true),
            HpiResponse::BadFirmware => Ok(false),
            other => Err(EcError::DeviceError(format!(
                "Failed to validate {:?}: {:?}",
                mode, other
            ))),
        }
    }

    fn write_flash_row(&self, row: u32, data: &[u8]) -> EcResult<()> {
        const RETRIES: u32 = 3;
        let row = u16::try_from(row)
            .map_err(|_| EcError::DeviceError(format!("Row {} out of range", row)))?;
        let cmd = [
            HPI_FLASH_RW_SIGNATURE as u8,
            HPI_FLASH_WRITE_CMD,
            (row & 0xFF) as u8,
            (row >> 8) as u8,
        ];

        let mut response = HpiResponse::NoResponse;
        for attempt in 1..=RETRIES {
            self.clear_interrupt()?;
            // First write the data to the flash write memory, then trigger the write
            self.ccgx_write(ControlRegisters::FlashRwMem, data)?;
            self.ccgx_write(ControlRegisters::FlashSignature, &cmd)?;
            // A row write takes up to 50ms
            response = self.poll_for_response(500)?;
            if response == HpiResponse::Success {
                return Ok(());
            }
            warn!(
                "Writing flash row {} failed (attempt {}/{}): {:?}",
                row, attempt, RETRIES, response
            );
        }
        Err(EcError::DeviceError(format!(
            "Failed to write flash row {}: {:?}",
            row, response
        )))
    }

    fn read_flash_row(&self, row: u32, flash_row_size: u16) -> EcResult<Vec<u8>> {
        let row = u16::try_from(row)
            .map_err(|_| EcError::DeviceError(format!("Row {} out of range", row)))?;
        let cmd = [
            HPI_FLASH_RW_SIGNATURE as u8,
            HPI_FLASH_READ_CMD,
            (row & 0xFF) as u8,
            (row >> 8) as u8,
        ];
        let response = self.command(ControlRegisters::FlashSignature, &cmd, 500)?;
        if response != HpiResponse::FlashRead {
            return Err(EcError::DeviceError(format!(
                "Failed to read flash row {}: {:?}",
                row, response
            )));
        }

        let data = self.ccgx_read(ControlRegisters::FlashRwMem, flash_row_size)?;
        if data.len() != flash_row_size.into() {
            return Err(EcError::DeviceError(format!(
                "Invalid size returned: {}",
                data.len()
            )));
        }
        Ok(data)
    }

    /// Validate firmware images and print results
    pub fn validate_and_print(&self) -> EcResult<()> {
        let _lock = PdBusLock::new(&self.ec)?;
        self.enter_flashing_mode()?;

        let main_valid = self.validate_firmware(FwMode::MainFw)?;
        println!("  Main FW (FW2) Valid:   {}", main_valid);
        let backup_valid = self.validate_firmware(FwMode::BackupFw)?;
        println!("  Backup FW (FW1) Valid: {}", backup_valid);

        self.leave_flashing_mode()
    }

    /// Check the firmware on the controller and compare it with a file
    ///
    /// Runs the same checks as flashing and reads back the inactive image,
    /// so the whole flash path except writing gets exercised.
    pub fn compare_firmware(&self, fw_file: &PdFirmwareFile, fw_bin: &[u8]) -> EcResult<bool> {
        let _lock = PdBusLock::new(&self.ec)?;

        self.check_firmware_compatible(fw_file)?;
        println!("Firmware file layout matches the controller");

        let (mode, flash_row_size) = self.get_device_info()?;
        self.enter_flashing_mode()?;

        let main_valid = self.validate_firmware(FwMode::MainFw)?;
        println!("  Main FW (FW2) Valid:   {}", main_valid);
        let backup_valid = self.validate_firmware(FwMode::BackupFw)?;
        println!("  Backup FW (FW1) Valid: {}", backup_valid);

        // Only the image that's not running can be read
        let (name, image) = match mode {
            FwMode::MainFw => ("Backup FW (FW1)", &fw_file.backup_fw),
            FwMode::BackupFw => ("Main FW (FW2)", &fw_file.main_fw),
            FwMode::BootLoader => {
                self.leave_flashing_mode()?;
                println!("Controller is in bootloader, not comparing flash contents");
                return Ok(main_valid && backup_valid);
            }
        };
        println!("Comparing {} on the controller with the file", name);
        let mut differing_rows = 0;
        let rows = image.rows();
        for (i, row) in (image.start_row..image.start_row + rows).enumerate() {
            if i % 32 == 0 || i as u32 == rows - 1 {
                println!("  Row {}/{}", i + 1, rows);
            }
            let offset = row as usize * image.row_size;
            let expected = fw_bin.get(offset..offset + image.row_size).ok_or_else(|| {
                EcError::DeviceError(format!("Row {} is beyond the end of the file", row))
            })?;
            let actual = self.read_flash_row(row, flash_row_size)?;
            if actual != expected {
                debug!("Row {} differs from the file", row);
                differing_rows += 1;
            }
        }
        self.leave_flashing_mode()?;

        if differing_rows == 0 {
            println!("{} on the controller is identical to the file", name);
        } else {
            println!(
                "{} on the controller differs from the file in {} of {} rows",
                name, differing_rows, rows
            );
        }
        Ok(main_valid && backup_valid && differing_rows == 0)
    }

    /// Read the whole flash of the controller
    ///
    /// The controller only lets us read the inactive firmware image. The rows
    /// of the bootloader and the running firmware are filled with zeros.
    pub fn dump_firmware(&self) -> EcResult<Vec<u8>> {
        // The largest CCGx flash has 1024 rows
        const MAX_ROWS: u32 = 1024;

        let _lock = PdBusLock::new(&self.ec)?;
        let (mode, flash_row_size) = self.get_device_info()?;
        println!(
            "Controller runs {:?}, flash row size: {}",
            mode, flash_row_size
        );
        self.enter_flashing_mode()?;

        let mut firmware_data: Vec<u8> =
            Vec::with_capacity(MAX_ROWS as usize * flash_row_size as usize);
        let mut last_readable_row = 0;
        for row in 0..MAX_ROWS {
            if row % 64 == 0 {
                println!("  Reading row {}/{}...", row, MAX_ROWS);
            }
            match self.read_flash_row(row, flash_row_size) {
                Ok(data) => {
                    firmware_data.extend(data);
                    last_readable_row = row;
                }
                Err(err) => {
                    debug!("Row {} not readable: {:?}", row, err);
                    firmware_data.extend(vec![0u8; flash_row_size as usize]);
                }
            }
        }
        self.leave_flashing_mode()?;

        // Smaller chips have only 512 rows. Nothing above is readable then.
        if last_readable_row < 512 {
            firmware_data.truncate(512 * flash_row_size as usize);
        }
        println!("Read {} bytes total", firmware_data.len());
        Ok(firmware_data)
    }

    /// Check that the firmware file fits this controller
    fn check_firmware_compatible(&self, fw_file: &PdFirmwareFile) -> EcResult<()> {
        let (_, flash_row_size) = self.get_device_info()?;
        if fw_file.backup_fw.row_size != flash_row_size as usize {
            return Err(EcError::DeviceError(format!(
                "Firmware is for chips with {} byte flash rows, controller has {} byte rows",
                fw_file.backup_fw.row_size, flash_row_size
            )));
        }

        // READ_SILICON_ID reports the value stored at offset 0xEA of the
        // image, which the binary parser calls silicon family.
        let silicon_id = self.get_silicon_id()?;
        if !silicon_id_compatible(silicon_id, fw_file.main_fw.silicon_family)
            || !silicon_id_compatible(silicon_id, fw_file.backup_fw.silicon_family)
        {
            return Err(EcError::DeviceError(format!(
                "Firmware is for silicon ID {:#06X}, controller is {:#06X}",
                fw_file.main_fw.silicon_family, silicon_id
            )));
        }

        // The layout in the file has to match the layout the controller expects.
        // Otherwise the running firmware refuses to write the rows anyway.
        let total_rows = fw_file.backup_fw.metadata_row + 1;
        let bootloader_last_row = self.get_bootloader_last_row()? as u32;
        let (fw1_start, fw2_start) = self.get_fw_locations()?;
        let (fw1_start, fw2_start) = (fw1_start as u32, fw2_start as u32);
        debug!(
            "Controller layout: Bootloader ends at row {}, FW1 at {}, FW2 at {}",
            bootloader_last_row, fw1_start, fw2_start
        );
        if fw_file.backup_fw.start_row != bootloader_last_row + 1 {
            return Err(EcError::DeviceError(format!(
                "FW1 in the file starts at row {}, but the bootloader ends at row {}",
                fw_file.backup_fw.start_row, bootloader_last_row
            )));
        }
        // An invalid image is reported as located after the end of flash
        if fw1_start != fw_file.backup_fw.start_row && fw1_start != total_rows {
            return Err(EcError::DeviceError(format!(
                "FW1 in the file starts at row {}, on the controller at row {}",
                fw_file.backup_fw.start_row, fw1_start
            )));
        }
        if fw2_start != fw_file.main_fw.start_row && fw2_start != total_rows {
            return Err(EcError::DeviceError(format!(
                "FW2 in the file starts at row {}, on the controller at row {}",
                fw_file.main_fw.start_row, fw2_start
            )));
        }
        Ok(())
    }

    /// Write one firmware image (FW1 or FW2) and its metadata
    ///
    /// Must not be the image that is currently running. Follows the HPI spec
    /// section 5.2.2.2: Clear metadata, write all rows, write metadata last.
    /// The controller adjusts the boot sequence number in the metadata.
    fn flash_image(&self, target: FwMode, image: &PdFirmware, fw_bin: &[u8]) -> EcResult<bool> {
        let row_size = image.row_size;
        let rows = image.rows();
        let row_data = |row: u32| -> EcResult<&[u8]> {
            let offset = row as usize * row_size;
            fw_bin.get(offset..offset + row_size).ok_or_else(|| {
                EcError::DeviceError(format!("Row {} is beyond the end of the file", row))
            })
        };
        // Check before touching the flash
        row_data(image.metadata_row)?;
        row_data(image.start_row + rows - 1)?;

        self.enter_flashing_mode()?;

        // Make sure a partially written image is never considered valid
        debug!("Clearing metadata row {}", image.metadata_row);
        self.write_flash_row(image.metadata_row, &vec![0u8; row_size])?;

        println!(
            "Writing rows {} to {} ({} rows)",
            image.start_row,
            image.start_row + rows - 1,
            rows
        );
        for (i, row) in (image.start_row..image.start_row + rows).enumerate() {
            if i % 32 == 0 || i as u32 == rows - 1 {
                println!("  Row {}/{}", i + 1, rows);
            }
            self.write_flash_row(row, row_data(row)?)?;
        }

        debug!("Writing metadata row {}", image.metadata_row);
        self.write_flash_row(image.metadata_row, row_data(image.metadata_row)?)?;

        let valid = self.validate_firmware(target)?;
        println!("Controller validated {:?}: {}", target, valid);

        self.leave_flashing_mode()?;
        Ok(valid)
    }

    /// Update firmware images on the controller
    ///
    /// Each firmware can only write the other one. If the image to update is
    /// running, the controller is switched to the other one first. When
    /// updating both, the inactive image is written first, then the
    /// controller switches to it and the previously active image is written.
    /// A final reset boots the main firmware.
    pub fn flash_firmware(
        &self,
        fw_file: &PdFirmwareFile,
        fw_bin: &[u8],
        images: PdImageSelection,
    ) -> EcResult<()> {
        let _lock = PdBusLock::new(&self.ec)?;

        self.check_firmware_compatible(fw_file)?;

        let (mode, _) = self.get_device_info()?;
        let mut current = mode;
        if current == FwMode::BootLoader {
            // Both images should not be invalid at the same time. Try to get
            // into a firmware, the bootloader is not able to flash.
            println!("Controller is in bootloader, resetting to get into a firmware");
            self.reset_device()?;
            current = self.wait_for_device(3000)?;
            if current == FwMode::BootLoader {
                return Err(EcError::DeviceError(
                    "Controller stays in bootloader. Flashing from bootloader is not supported"
                        .to_string(),
                ));
            }
        }

        let targets: &[FwMode] = match (images, current) {
            // Update the image that's not running first
            (PdImageSelection::Both, FwMode::MainFw) => &[FwMode::BackupFw, FwMode::MainFw],
            (PdImageSelection::Both, _) => &[FwMode::MainFw, FwMode::BackupFw],
            (PdImageSelection::Main, _) => &[FwMode::MainFw],
            (PdImageSelection::Backup, _) => &[FwMode::BackupFw],
        };

        for target in targets {
            let (image, other) = match target {
                FwMode::BackupFw => (&fw_file.backup_fw, FwMode::MainFw),
                FwMode::MainFw => (&fw_file.main_fw, FwMode::BackupFw),
                FwMode::BootLoader => unreachable!(),
            };
            // Can't write the running image, switch to the other one
            if current == *target {
                println!(
                    "\nSwitching to {:?} to be able to update {:?}",
                    other, target
                );
                self.jump_to_other_fw(current, other)?;
                current = other;
            }

            println!(
                "\nFlashing {:?} ({}) from {:?}",
                target, image.app_version, current
            );
            if !self.flash_image(*target, image, fw_bin)? {
                return Err(EcError::DeviceError(format!(
                    "{:?} is not valid after flashing. Check the firmware file and try again.",
                    target
                )));
            }
        }

        // Boot normally again, the bootloader prefers the main firmware
        println!("\nRestarting controller");
        self.enable_ports(false)?;
        self.reset_device()?;
        let final_mode = self.wait_for_device(3000)?;
        println!("Controller is running {:?}", final_mode);
        self.print_fw_info();

        Ok(())
    }
}
