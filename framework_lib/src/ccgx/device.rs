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
use crate::util::{assert_win_len, Config, Platform};

use super::*;

enum ControlRegisters {
    DeviceMode = 0,
    SiliconId = 2, // Two bytes long, First LSB, then MSB
    BootLoaderVersion = 0x10,
    Firmware1Version = 0x18,
    Firmware2Version = 0x20,
}

#[derive(Debug)]
pub enum PdPort {
    Left01,
    Right23,
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
            (Platform::GenericFramework((left, _), _), PdPort::Left01) => *left,
            (Platform::GenericFramework((_, right), _), PdPort::Right23) => *right,
            // Framework AMD Platforms (CCG8)
            (
                Platform::Framework13Amd7080
                | Platform::Framework13AmdAi300
                | Platform::Framework16Amd7080,
                PdPort::Left01,
            ) => 0x42,
            (
                Platform::Framework13Amd7080
                | Platform::Framework13AmdAi300
                | Platform::Framework16Amd7080,
                PdPort::Right23,
            ) => 0x40,
            (Platform::Framework16Amd7080, PdPort::Back) => 0x42,
            (Platform::FrameworkDesktopAmdAiMax300, PdPort::Back) => 0x08,
            (Platform::FrameworkDesktopAmdAiMax300, _) => unsupported?,
            // Framework Intel Platforms (CCG5 and CCG6)
            (
                Platform::Framework12IntelGen13
                | Platform::IntelGen11
                | Platform::IntelGen12
                | Platform::IntelGen13
                | Platform::IntelCoreUltra1,
                PdPort::Left01,
            ) => 0x08,
            (
                Platform::Framework12IntelGen13
                | Platform::IntelGen11
                | Platform::IntelGen12
                | Platform::IntelGen13
                | Platform::IntelCoreUltra1,
                PdPort::Right23,
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
            (Platform::GenericFramework(_, (left, _)), PdPort::Left01) => *left,
            (Platform::GenericFramework(_, (_, right)), PdPort::Right23) => *right,
            (Platform::IntelGen11, _) => 6,
            (Platform::IntelGen12 | Platform::IntelGen13, PdPort::Left01) => 6,
            (Platform::IntelGen12 | Platform::IntelGen13, PdPort::Right23) => 7,
            (
                Platform::Framework13Amd7080
                | Platform::Framework16Amd7080
                | Platform::IntelCoreUltra1
                | Platform::Framework13AmdAi300
                | Platform::Framework12IntelGen13,
                PdPort::Left01,
            ) => 1,
            (
                Platform::Framework13Amd7080
                | Platform::Framework16Amd7080
                | Platform::IntelCoreUltra1
                | Platform::Framework13AmdAi300
                | Platform::Framework12IntelGen13,
                PdPort::Right23,
            ) => 2,
            (Platform::Framework16Amd7080, PdPort::Back) => 5,
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

#[derive(Debug, PartialEq)]
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
        i2c_read(
            &self.ec,
            self.port.i2c_port()?,
            self.port.i2c_address()?,
            addr,
            len,
        )
    }

    fn ccgx_read(&self, reg: ControlRegisters, len: u16) -> EcResult<Vec<u8>> {
        let mut data: Vec<u8> = Vec::with_capacity(len.into());

        let addr = reg as u16;

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
}
