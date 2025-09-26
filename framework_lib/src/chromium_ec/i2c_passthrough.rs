use crate::chromium_ec::command::EcCommands;
use crate::chromium_ec::{CrosEc, CrosEcDriver, EcError, EcResult};
use crate::util;
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use std::mem::size_of;

/// Maximum transfer size for one I2C transaction supported by the chip
pub const MAX_I2C_CHUNK: usize = 128;

#[repr(C, packed)]
pub struct EcParamsI2cPassthruMsg {
    /// Slave address and flags
    addr_and_flags: u16,
    transfer_len: u16,
}

#[repr(C, packed)]
pub struct EcParamsI2cPassthru {
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

#[derive(Debug)]
pub struct EcI2cPassthruResponse {
    pub i2c_status: u8, // TODO: Can probably use enum
    pub data: Vec<u8>,
}

impl EcI2cPassthruResponse {
    pub fn is_successful(&self) -> EcResult<()> {
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

pub fn i2c_read(
    ec: &CrosEc,
    i2c_port: u8,
    i2c_addr: u16,
    addr: u16,
    len: u16,
) -> EcResult<EcI2cPassthruResponse> {
    trace!(
        "i2c_read(i2c_port: 0x{:X}, i2c_addr: 0x{:X}, addr: 0x{:X}, len: 0x{:X})",
        i2c_port,
        i2c_addr,
        addr,
        len
    );
    if usize::from(len) > MAX_I2C_CHUNK {
        return EcResult::Err(EcError::DeviceError(format!(
            "i2c_read too long. Must be <128, is: {}",
            len
        )));
    }
    let addr_bytes = if addr < 0xFF {
        vec![addr as u8]
    } else {
        u16::to_le_bytes(addr).to_vec()
    };
    let messages = vec![
        EcParamsI2cPassthruMsg {
            addr_and_flags: i2c_addr,
            transfer_len: addr_bytes.len() as u16,
        },
        EcParamsI2cPassthruMsg {
            addr_and_flags: i2c_addr + I2C_READ_FLAG,
            transfer_len: len, // How much to read
        },
    ];
    let msgs_len = size_of::<EcParamsI2cPassthruMsg>() * messages.len();
    let msgs_buffer: &[u8] = unsafe { util::any_vec_as_u8_slice(&messages) };

    let params = EcParamsI2cPassthru {
        port: i2c_port,
        messages: messages.len() as u8,
        msg: [], // Messages are copied right after this struct
    };
    let params_len = size_of::<EcParamsI2cPassthru>();
    let params_buffer: &[u8] = unsafe { util::any_as_u8_slice(&params) };

    let mut buffer: Vec<u8> = vec![0; params_len + msgs_len + addr_bytes.len()];
    buffer[0..params_len].copy_from_slice(params_buffer);
    buffer[params_len..params_len + msgs_len].copy_from_slice(msgs_buffer);
    buffer[params_len + msgs_len..].copy_from_slice(&addr_bytes);

    let data = ec.send_command(EcCommands::I2cPassthrough as u16, 0, &buffer)?;
    let res: _EcI2cPassthruResponse = unsafe { std::ptr::read(data.as_ptr() as *const _) };
    let res_data = &data[size_of::<_EcI2cPassthruResponse>()..];
    // TODO: Seems to be either one, non-deterministically
    debug_assert!(res.messages as usize == messages.len() || res.messages == 0);
    Ok(EcI2cPassthruResponse {
        i2c_status: res.i2c_status,
        data: res_data.to_vec(),
    })
}

pub fn i2c_write(
    ec: &CrosEc,
    i2c_port: u8,
    i2c_addr: u16,
    addr: u16,
    data: &[u8],
) -> EcResult<EcI2cPassthruResponse> {
    trace!(
        "  i2c_write(addr: {}, len: {}, data: {:?})",
        addr,
        data.len(),
        data
    );
    let addr_bytes = [addr as u8, (addr >> 8) as u8];
    let messages = vec![EcParamsI2cPassthruMsg {
        addr_and_flags: i2c_addr,
        transfer_len: (addr_bytes.len() + data.len()) as u16,
    }];
    let msgs_len = size_of::<EcParamsI2cPassthruMsg>() * messages.len();
    let msgs_buffer: &[u8] = unsafe { util::any_vec_as_u8_slice(&messages) };

    let params = EcParamsI2cPassthru {
        port: i2c_port,
        messages: messages.len() as u8,
        msg: [], // Messages are copied right after this struct
    };
    let params_len = size_of::<EcParamsI2cPassthru>();
    let params_buffer: &[u8] = unsafe { util::any_as_u8_slice(&params) };

    let mut buffer: Vec<u8> = vec![0; params_len + msgs_len + addr_bytes.len() + data.len()];
    buffer[0..params_len].copy_from_slice(params_buffer);
    buffer[params_len..params_len + msgs_len].copy_from_slice(msgs_buffer);
    buffer[params_len + msgs_len..params_len + msgs_len + addr_bytes.len()]
        .copy_from_slice(&addr_bytes);
    buffer[params_len + msgs_len + addr_bytes.len()..].copy_from_slice(data);

    let data = ec.send_command(EcCommands::I2cPassthrough as u16, 0, &buffer)?;
    let res: _EcI2cPassthruResponse = unsafe { std::ptr::read(data.as_ptr() as *const _) };
    util::assert_win_len(data.len(), size_of::<_EcI2cPassthruResponse>()); // No extra data other than the header
    debug_assert_eq!(res.messages as usize, messages.len());
    Ok(EcI2cPassthruResponse {
        i2c_status: res.i2c_status,
        data: vec![], // Writing doesn't return any data
    })
}
