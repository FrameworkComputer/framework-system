//! Service the STM32 keyboard controller of the Framework Laptop 12 2nd gen
//! input deck: read its identity and firmware version, flash firmware, erase it.
//!
//! The STM32 sits on the EC's I2C bus, not the host's, so everything goes
//! through two EC host commands:
//!
//! - `EC_CMD_I2C_PASSTHRU` carries the vendor protocol on the deck's I2C-HID
//!   interface (0x15), and the STM32 ROM bootloader protocol (ST AN4221) at
//!   its own address (0x6A). Flashing runs entirely on the host.
//! - `EC_CMD_KBD_FLASH_STM32` only resets the controller into the ROM
//!   bootloader and back, because BOOT0 and the power rail are EC GPIOs and
//!   the EC's I2C-HID driver has to stay off the bus meanwhile.
//!
//! Nothing here checks the platform. Only call it on a Framework Laptop 12,
//! on other systems port 5 may well have something else on it.
//!
//! The library never prints. Long operations report progress through a
//! callback, and errors carry an [`ErrorKind`] so the caller can decide what
//! to do about them.
//!
//! ```no_run
//! use framework_lib::chromium_ec::CrosEc;
//! use framework_lib::stm32_kbd::{self, FlashOptions, HexImage};
//!
//! let ec = CrosEc::new();
//! let img = HexImage::parse(&std::fs::read_to_string("kbd.hex").unwrap()).unwrap();
//! let report = stm32_kbd::flash(&ec, &img, &FlashOptions::default(), &mut |stage, done, total| {
//!     println!("{:?} {}/{}", stage, done, total);
//!     true // false cancels
//! });
//! ```

mod bootloader;
mod deck;
mod flash;
mod hex;

pub use deck::*;
pub use flash::*;
pub use hex::*;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::chromium_ec::i2c_passthrough::i2c_transfer;
use crate::chromium_ec::{CrosEc, EcError};

/// dahlia/i2c.dtsi: i2c5_0 = I2C_PORT_KB_DISCRETE
pub const I2C_PORT: u8 = 5;
/// The STM32's I2C-HID interface, while the application runs
pub const HID_ADDR: u16 = 0x15;
/// The STM32 ROM bootloader. Only one of HID_ADDR and BL_ADDR answers at a time.
pub const BL_ADDR: u16 = 0x6A;

/// What kind of thing went wrong
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The deck did not answer
    Comms,
    /// It answered, and the read back disagreed
    Verify,
    /// It answered, and said no
    Refused,
    /// The firmware file was rejected
    BadImage,
    /// The progress callback asked to stop
    Cancelled,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stm32KbdError {
    pub kind: ErrorKind,
    pub message: String,
}

impl Stm32KbdError {
    pub(crate) fn new(kind: ErrorKind, message: String) -> Self {
        Self { kind, message }
    }
}

impl core::fmt::Display for Stm32KbdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<EcError> for Stm32KbdError {
    fn from(e: EcError) -> Self {
        Self::new(ErrorKind::Comms, format!("EC error: {:?}", e))
    }
}

pub type Stm32KbdResult<T> = Result<T, Stm32KbdError>;

fn sleep_ms(ms: u32) {
    crate::os_specific::sleep(ms as u64 * 1000);
}

/// One I2C transaction on the deck's bus, with a bus error turned into an error
fn i2c_xfer(ec: &CrosEc, addr: u16, write: &[u8], read_len: usize) -> Stm32KbdResult<Vec<u8>> {
    let res = i2c_transfer(ec, I2C_PORT, addr, write, read_len as u16)?;
    if res.i2c_status & 0x01 != 0 {
        return Err(Stm32KbdError::new(
            ErrorKind::Comms,
            format!("I2C 0x{:02X} not acknowledged - is the deck powered?", addr),
        ));
    }
    if res.i2c_status & 0x02 != 0 {
        return Err(Stm32KbdError::new(
            ErrorKind::Comms,
            format!("I2C timeout talking to 0x{:02X}", addr),
        ));
    }
    if res.i2c_status != 0 {
        return Err(Stm32KbdError::new(
            ErrorKind::Comms,
            format!("I2C error 0x{:02X}", res.i2c_status),
        ));
    }
    Ok(res.data)
}
