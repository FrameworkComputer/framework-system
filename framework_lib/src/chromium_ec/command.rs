#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::util;

use super::{CrosEc, CrosEcDriver, EcError, EcResult};

// u16
#[non_exhaustive]
#[derive(Debug)]
pub enum EcCommands {
    GetVersion = 0x02,
    GetBuildInfo = 0x04,
    /// Command to read data from EC memory map
    ReadMemMap = 0x07,
    PwmGetKeyboardBacklight = 0x0022,
    PwmSetKeyboardBacklight = 0x0023,
    I2cPassthrough = 0x9e,
    ConsoleSnapshot = 0x97,
    ConsoleRead = 0x98,
    /// Get information about PD controller power
    UsbPdPowerInfo = 0x103,

    // Framework specific commands
    /// Configure the behavior of the flash notify
    FlashNotified = 0x3E01,
    /// Get information about the current chassis open/close status
    ChassisOpenCheck = 0x3E0F,
    /// Get information about historical chassis open/close (intrusion) information
    ChassisIntrusion = 0x3E09,
    /// Get information about PD controller version
    ReadPdVersion = 0x3E11,
    /// Get information about current state of privacy switches
    PriavcySwitchesCheckMode = 0x3E14,
}

pub trait EcRequest<R> {
    fn command_id() -> EcCommands;
    // Can optionally override this
    fn command_version() -> u8 {
        0
    }

    fn format_request(&self) -> &[u8]
    where
        Self: Sized,
    {
        unsafe { util::any_as_u8_slice(self) }
    }

    fn send_command_vec(&self, ec: &CrosEc) -> EcResult<Vec<u8>>
    where
        Self: Sized,
    {
        self.send_command_vec_extra(ec, &[])
    }

    fn send_command_vec_extra(&self, ec: &CrosEc, extra_data: &[u8]) -> EcResult<Vec<u8>>
    where
        Self: Sized,
    {
        let params = self.format_request();
        let request = if extra_data.is_empty() {
            params.to_vec()
        } else {
            let mut buffer: Vec<u8> = vec![0; params.len() + extra_data.len()];
            buffer[..params.len()].copy_from_slice(params);
            buffer[params.len()..].copy_from_slice(extra_data);
            buffer
        };
        let response =
            ec.send_command(Self::command_id() as u16, Self::command_version(), &request)?;
        if util::is_debug() {
            println!("send_command<{:?}>", Self::command_id());
            println!("  Request:  {:?}", request);
            println!("  Response: {:?}", response);
        }
        Ok(response)
    }

    fn send_command(&self, ec: &CrosEc) -> EcResult<R>
    where
        Self: Sized,
    {
        self.send_command_extra(ec, &[])
    }

    // Same as send_command but with extra data packed after the defined struct
    fn send_command_extra(&self, ec: &CrosEc, extra_data: &[u8]) -> EcResult<R>
    where
        Self: Sized,
    {
        let response = self.send_command_vec_extra(ec, extra_data)?;
        if response.len() != std::mem::size_of::<R>() {
            return Err(EcError::DeviceError(format!(
                "Returned data is not the expected ({}) size: {}",
                response.len(),
                std::mem::size_of::<R>()
            )));
        }
        let val: R = unsafe { std::ptr::read(response.as_ptr() as *const _) };
        Ok(val)
    }
}
