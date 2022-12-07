#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::ccgx::{AppVersion, BaseVersion, ControllerVersion};
use crate::chromium_ec::{CrosEc, CrosEcDriver};
use crate::util::{self, Config, Platform};
use std::mem::size_of;

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
            (_, _) => panic!("Unsupported platform"),
        }
    }
}

pub struct PdController {
    port: PdPort,
}

fn passthrough_offset(dev_index: u16) -> u16 {
    dev_index * 0x4000
}

const EC_CMD_I2C_PASSTHROUGH: u16 = 0x9e;

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
    fn is_successful(&self) -> bool {
        if self.i2c_status & 1 > 0 {
            // Transfer not acknowledged
            return false;
        }
        if self.i2c_status & (1 << 1) > 0 {
            // Transfer timeout
            return false;
        }
        // I'm not aware of any other errors, but there might be.
        // But I don't think multiple errors can be indicated at the same time
        assert_eq!(self.i2c_status, 0);
        true
    }
}

/// Indicate that it's a read, not a write
const I2C_READ_FLAG: u16 = 1 << 15;

#[derive(Debug)]
pub enum FwMode {
    BootLoader,
    BackupFw,
    MainFw,
}

impl PdController {
    pub fn new(port: PdPort) -> Self {
        PdController { port }
    }
    /// Wrapped with support for dev id
    /// TODO: Should move into chromium_ec module
    fn send_ec_command(&self, code: u16, dev_index: u16, data: &[u8]) -> Option<Vec<u8>> {
        let command_id = code + passthrough_offset(dev_index);
        CrosEc::new().send_command(command_id, 0, data)
    }

    fn i2c_read(&self, addr: u16, len: u16) -> Option<EcI2cPassthruResponse> {
        if util::is_debug() {
            println!("i2c_read(addr: {}, len: {})", addr, len);
        }
        let messages = vec![
            EcParamsI2cPassthruMsg {
                addr_and_flags: self.port.i2c_address(),
                transfer_len: 2, // How much we write. Address is u16, so 2 bytes
            },
            EcParamsI2cPassthruMsg {
                addr_and_flags: self.port.i2c_address() + I2C_READ_FLAG,
                transfer_len: len, // How much we read
            },
        ];
        let msgs_len = size_of::<EcParamsI2cPassthruMsg>() * 2;

        let msgs_buffer: &[u8] = unsafe { util::any_vec_as_u8_slice(&messages) };

        let params = EcParamsI2cPassthru {
            port: self.port.i2c_port(),
            messages: messages.len() as u8,
            msg: [],
        };
        let params_len = size_of::<EcParamsI2cPassthru>();
        let params_buffer: &[u8] = unsafe { util::any_as_u8_slice(&params) };

        let addr_bytes = [addr as u8, (addr >> 8) as u8];

        let mut buffer: Vec<u8> = vec![0; params_len + msgs_len + addr_bytes.len()];
        buffer[0..params_len].copy_from_slice(params_buffer);
        buffer[params_len..params_len + msgs_len].copy_from_slice(msgs_buffer);
        buffer[params_len + msgs_len..].copy_from_slice(&addr_bytes);

        let data = self.send_ec_command(EC_CMD_I2C_PASSTHROUGH, 0, &buffer);
        let data = if let Some(data) = data {
            data
        } else {
            println!("Failed to send I2C read command");
            return None;
        };
        let res: _EcI2cPassthruResponse = unsafe { std::ptr::read(data.as_ptr() as *const _) };
        let res_data = &data[size_of::<_EcI2cPassthruResponse>()..];
        debug_assert_eq!(res.messages as usize, messages.len());
        Some(EcI2cPassthruResponse {
            i2c_status: res.i2c_status,
            data: res_data.to_vec(),
        })
    }

    fn ccgx_read(&self, addr: u16, len: u16) -> Option<Vec<u8>> {
        // TODO: Read more than that chunk
        let chunk_len = 128; // Our chip supports this max transfer size
        assert!(len <= chunk_len);
        // TODO: It has an error code
        let i2c_response = self.i2c_read(addr, std::cmp::min(chunk_len, len))?;
        if !i2c_response.is_successful() {
            println!("I2C read was not successful");
            return None;
        }
        Some(i2c_response.data)
    }

    pub fn get_silicon_id(&self) -> Option<u16> {
        let data = self.ccgx_read(ControlRegisters::SiliconId as u16, 2)?;
        assert!(data.len() >= 2);
        debug_assert_eq!(data.len(), 2);
        Some(((data[1] as u16) << 8) + (data[0] as u16))
    }

    pub fn get_device_info(&self) -> Option<(FwMode, u16)> {
        let data = self.ccgx_read(ControlRegisters::DeviceMode as u16, 1)?;
        let byte = data[0];

        // Currently used firmware
        let fw_mode = match byte & 0b0000_0011 {
            0 => FwMode::BootLoader,
            1 => FwMode::BackupFw,
            2 => FwMode::MainFw,
            _ => return None,
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

        Some((fw_mode, flash_row_size))
    }

    pub fn flash_pd(&self) {
        println!("Flashing port: {:?}", self.port);

        // Seems TGL silicon ID is 0x2100 and ADL is 0x3000
        // TODO: Make sure silicon ID is the same in binary and device

        // TODO: Implement the rest
    }

    pub fn get_fw_versions(&self) -> Option<ControllerVersion> {
        let data = self.ccgx_read(ControlRegisters::Firmware1Version as u16, 8)?;
        Some(ControllerVersion {
            base: BaseVersion::from(&data[..4]),
            app: AppVersion::from(&data[4..]),
        })
    }

    pub fn print_fw_info(&self) {
        let data = self.ccgx_read(ControlRegisters::BootLoaderVersion as u16, 8);
        let data = data.unwrap();
        assert!(data.len() >= 8);
        debug_assert_eq!(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!(
            "  Bootloader Version: Base: {},  App: {}",
            base_ver, app_ver
        );

        let data = self.ccgx_read(ControlRegisters::Firmware1Version as u16, 8);
        let data = data.unwrap();
        assert!(data.len() >= 8);
        debug_assert_eq!(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!("  FW1 Version: Base: {},  App: {}", base_ver, app_ver);

        let data = self.ccgx_read(ControlRegisters::Firmware2Version as u16, 8);
        let data = data.unwrap();
        assert!(data.len() >= 8);
        debug_assert_eq!(data.len(), 8);
        let base_ver = BaseVersion::from(&data[..4]);
        let app_ver = AppVersion::from(&data[4..]);
        println!("  FW2 Version: Base: {},  App: {}", base_ver, app_ver);
    }
}
