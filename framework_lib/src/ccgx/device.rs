//! Communicate with CCGX (CCG5, CCG6) PD controllers
//!
//! The current implementation talks to them by tunneling I2C through EC host commands.

use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::ccgx::{AppVersion, BaseVersion, ControllerVersion};
use crate::chromium_ec::command::EcCommands;
use crate::chromium_ec::{CrosEc, CrosEcDriver, EcError, EcResult};
use crate::util::{self, Config, Platform};
use std::mem::size_of;

use super::*;

/// Maximum transfer size for one I2C transaction supported by the chip
const MAX_I2C_CHUNK: usize = 128;

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
}

impl PdPort {
    /// SMBUS/I2C Address
    fn i2c_address(&self) -> u16 {
        match self {
            PdPort::Left01 => 0x08,
            PdPort::Right23 => 0x40,
        }
    }

    /// I2C port on the EC
    fn i2c_port(&self) -> u8 {
        let config = Config::get();
        let platform = &(*config).as_ref().unwrap().platform;

        match (platform, self) {
            (Platform::IntelGen11, _) => 6,
            (Platform::IntelGen12, PdPort::Left01) => 6,
            (Platform::IntelGen12, PdPort::Right23) => 7,
            (Platform::IntelGen13, PdPort::Left01) => 6,
            (Platform::IntelGen13, PdPort::Right23) => 7,
            (_, _) => panic!("Unsupported platform: {:?} {:?}", platform, self),
        }
    }
}

pub struct PdController {
    port: PdPort,
}

fn passthrough_offset(dev_index: u16) -> u16 {
    dev_index * 0x4000
}

#[repr(C, packed)]
struct EcParamsI2cPassthruMsg {
    /// Slave address and flags
    addr_and_flags: u16,
    transfer_len: u16,
}

#[repr(C, packed)]
struct EcParamsI2cPassthru {
    port: u8,
    /// How many messages
    messages: u8,
    msg: [EcParamsI2cPassthruMsg; 0],
}

#[repr(C, packed)]
struct _EcI2cPassthruResponse {
    i2c_status: u8,
    /// How many messages
    messages: u8,
    data: [u8; 0],
}

struct EcI2cPassthruResponse {
    i2c_status: u8, // TODO: Can probably use enum
    data: Vec<u8>,
}

impl EcI2cPassthruResponse {
    fn is_successful(&self) -> EcResult<()> {
        if self.i2c_status & 1 > 0 {
            return Err(EcError::DeviceError(
                "I2C Transfer not acknowledged".to_string(),
            ));
        }
        if self.i2c_status & (1 << 1) > 0 {
            return Err(EcError::DeviceError("I2C Transfer timeout".to_string()));
        }
        // I'm not aware of any other errors, but there might be.
        // But I don't think multiple errors can be indicated at the same time
        assert_eq!(self.i2c_status, 0);
        Ok(())
    }
}

/// Indicate that it's a read, not a write
const I2C_READ_FLAG: u16 = 1 << 15;

#[derive(Debug)]
pub enum FwMode {
    BootLoader = 0,
    /// Backup CCGX firmware (No 1)
    BackupFw = 1,
    /// Main CCGX firmware (No 2)
    MainFw = 2,
}

impl PdController {
    pub fn new(port: PdPort) -> Self {
        PdController { port }
    }
    /// Wrapped with support for dev id
    /// TODO: Should move into chromium_ec module
    /// TODO: Must not call CrosEc::new() otherwise the driver isn't configurable!
    fn send_ec_command(&self, code: u16, dev_index: u16, data: &[u8]) -> EcResult<Vec<u8>> {
        let command_id = code + passthrough_offset(dev_index);
        CrosEc::new().send_command(command_id, 0, data)
    }

    fn i2c_read(&self, addr: u16, len: u16) -> EcResult<EcI2cPassthruResponse> {
        trace!("i2c_read(addr: {}, len: {})", addr, len);
        let addr_bytes = u16::to_le_bytes(addr);
        let messages = vec![
            EcParamsI2cPassthruMsg {
                addr_and_flags: self.port.i2c_address(),
                transfer_len: addr_bytes.len() as u16,
            },
            EcParamsI2cPassthruMsg {
                addr_and_flags: self.port.i2c_address() + I2C_READ_FLAG,
                transfer_len: len, // How much to read
            },
        ];
        let msgs_len = size_of::<EcParamsI2cPassthruMsg>() * messages.len();
        let msgs_buffer: &[u8] = unsafe { util::any_vec_as_u8_slice(&messages) };

        let params = EcParamsI2cPassthru {
            port: self.port.i2c_port(),
            messages: messages.len() as u8,
            msg: [], // Messages are copied right after this struct
        };
        let params_len = size_of::<EcParamsI2cPassthru>();
        let params_buffer: &[u8] = unsafe { util::any_as_u8_slice(&params) };

        let mut buffer: Vec<u8> = vec![0; params_len + msgs_len + addr_bytes.len()];
        buffer[0..params_len].copy_from_slice(params_buffer);
        buffer[params_len..params_len + msgs_len].copy_from_slice(msgs_buffer);
        buffer[params_len + msgs_len..].copy_from_slice(&addr_bytes);

        let data = self.send_ec_command(EcCommands::I2cPassthrough as u16, 0, &buffer);
        let data = match data {
            Ok(data) => data,
            Err(err) => return Err(err),
        };
        let res: _EcI2cPassthruResponse = unsafe { std::ptr::read(data.as_ptr() as *const _) };
        let res_data = &data[size_of::<_EcI2cPassthruResponse>()..];
        // TODO: Seems to be either one, non-deterministically
        debug_assert!(res.messages as usize == messages.len() || res.messages == 0);
        Ok(EcI2cPassthruResponse {
            i2c_status: res.i2c_status,
            data: res_data.to_vec(),
        })
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
        assert!(data.len() >= 2);
        debug_assert_eq!(data.len(), 2);
        Ok(u16::from_le_bytes([data[0], data[1]]))
    }

    /// Get device info (fw_mode, flash_row_size)
    pub fn get_device_info(&self) -> EcResult<(FwMode, u16)> {
        let data = self.ccgx_read(ControlRegisters::DeviceMode, 1)?;
        let byte = data[0];

        // Currently used firmware
        let fw_mode = match byte & 0b0000_0011 {
            0 => FwMode::BootLoader,
            1 => FwMode::BackupFw,
            2 => FwMode::MainFw,
            x => return Err(EcError::DeviceError(format!("FW Mode invalid: {}", x))),
        };

        let flash_row_size = match (byte & 0b0011_0000) >> 4 {
            0 => 128, // 0x80
            1 => 256, // 0x100
            2 => panic!("Reserved"),
            3 => 64, // 0x40
            x => panic!("Unexpected value: {}", x),
        };

        // All our devices support HPI v2 and we expect to use that to interact with them
        let hpi_v2 = (byte & (1 << 7)) > 0;
        debug_assert!(hpi_v2);

        Ok((fw_mode, flash_row_size))
    }
    pub fn get_fw_versions(&self) -> EcResult<ControllerFirmwares> {
        Ok(ControllerFirmwares {
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
        let data = data.unwrap();
        assert!(data.len() >= 8);
        debug_assert_eq!(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  Bootloader Version: Base: {},  App: {}",
            base_ver, app_ver
        );

        let data = self.ccgx_read(ControlRegisters::Firmware1Version, 8);
        let data = data.unwrap();
        assert!(data.len() >= 8);
        debug_assert_eq!(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  FW1 (Backup) Version: Base: {},  App: {}",
            base_ver, app_ver
        );

        let data = self.ccgx_read(ControlRegisters::Firmware2Version, 8);
        let data = data.unwrap();
        assert!(data.len() >= 8);
        debug_assert_eq!(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  FW2 (Main)   Version: Base: {},  App: {}",
            base_ver, app_ver
        );
    }
}
