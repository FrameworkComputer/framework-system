//! The STM32 ROM bootloader's I2C protocol (ST AN4221), run from the host
//! over I2C passthrough.
//!
//! A port of stm32bl.c from stm32kbd_tool, which in turn is what the EC used to
//! do in its own stm32_bl.c: the same transactions, ACK polling and timeouts.

use alloc::format;
use alloc::vec::Vec;

use super::{i2c_xfer, sleep_ms, ErrorKind, Stm32KbdError, Stm32KbdResult, BL_ADDR};
use crate::chromium_ec::CrosEc;

const ACK: u8 = 0x79;
const NACK: u8 = 0x1F;

const CMD_GET: u8 = 0x00;
const CMD_GVR: u8 = 0x01;
const CMD_GID: u8 = 0x02;
const CMD_RM: u8 = 0x11;
const CMD_WM: u8 = 0x31;
/// No-stretch: answers BUSY while programming
const CMD_WM_NS: u8 = 0x32;
const CMD_ER: u8 = 0x43;
const CMD_EE: u8 = 0x44;
const CMD_EE_NS: u8 = 0x45;
const CMD_NONE: u8 = 0xFF;

/// Largest Write/Read Memory the bootloader takes in one command
pub const MAX_DATA: usize = 256;

const ACK_TIMEOUT_MS: u32 = 2000;
/// Mass erase
const ERASE_TIMEOUT_MS: u32 = 35000;
const WRITE_TIMEOUT_MS: u32 = 2000;
/// The part booting into ROM after ENTER
const ENTER_TIMEOUT_MS: u32 = 1500;
const POLL_INTERVAL_MS: u32 = 2;

/// GET's reply length depends on the bootloader version, and it has to be
/// read in one transaction. The value is the number of command codes; the
/// frame is N, the version, then those.
const GET_REPLY_LENS: [(u8, usize); 3] = [(0x10, 11), (0x11, 17), (0x12, 18)];

fn comms(msg: alloc::string::String) -> Stm32KbdError {
    Stm32KbdError::new(ErrorKind::Comms, msg)
}

fn xfer(ec: &CrosEc, write: &[u8], read_len: usize) -> Stm32KbdResult<Vec<u8>> {
    i2c_xfer(ec, BL_ADDR, write, read_len)
}

/// Poll for the bootloader's verdict. Anything but ACK or NACK - BUSY, 0x00
/// or 0xFF while it is still working, or the address not being acknowledged
/// at all - means ask again.
fn get_ack(ec: &CrosEc, timeout_ms: u32, what: &str) -> Stm32KbdResult<()> {
    let mut waited = 0;
    loop {
        if let Ok(b) = xfer(ec, &[], 1) {
            match b[0] {
                ACK => return Ok(()),
                NACK => {
                    return Err(Stm32KbdError::new(
                        ErrorKind::Refused,
                        format!("the bootloader refused {}", what),
                    ))
                }
                _ => {}
            }
        }
        if waited >= timeout_ms {
            return Err(comms(format!(
                "no answer from the bootloader to {} within {} s",
                what,
                timeout_ms / 1000
            )));
        }
        sleep_ms(POLL_INTERVAL_MS);
        waited += POLL_INTERVAL_MS;
    }
}

fn write_then_ack(ec: &CrosEc, b: &[u8], timeout_ms: u32, what: &str) -> Stm32KbdResult<()> {
    xfer(ec, b, 0)?;
    get_ack(ec, timeout_ms, what)
}

/// {cmd, ~cmd}, then its ACK
fn send_cmd(ec: &CrosEc, cmd: u8, what: &str) -> Stm32KbdResult<()> {
    write_then_ack(ec, &[cmd, cmd ^ 0xFF], ACK_TIMEOUT_MS, what)
}

/// Big-endian address and its XOR, then the ACK
fn send_addr(ec: &CrosEc, addr: u32) -> Stm32KbdResult<()> {
    let a = addr.to_be_bytes();
    let b = [a[0], a[1], a[2], a[3], a[0] ^ a[1] ^ a[2] ^ a[3]];
    write_then_ack(ec, &b, ACK_TIMEOUT_MS, "the address")
}

fn newer(prev: u8, c: u8) -> u8 {
    if prev == CMD_NONE || c > prev {
        c
    } else {
        prev
    }
}

/// Wait for a freshly reset bootloader to answer at its address, without
/// asking it anything. For restarts: what [`Bootloader::init`] learned still holds.
pub fn wait_ready(ec: &CrosEc) -> Stm32KbdResult<()> {
    let mut waited = 0;
    loop {
        // Any read the address is acknowledged for will do
        if xfer(ec, &[], 1).is_ok() {
            return Ok(());
        }
        if waited >= ENTER_TIMEOUT_MS {
            return Err(comms(format!(
                "the STM32 bootloader does not answer at 0x{:02X}",
                BL_ADDR
            )));
        }
        sleep_ms(20);
        waited += 20;
    }
}

/// A bootloader that has been through the handshake
pub struct Bootloader<'a> {
    ec: &'a CrosEc,
    /// From Get Version
    pub version: u8,
    /// From Get, BCD
    pub bl_version: u8,
    /// From Get ID
    pub pid: u16,
    /// Newest Write Memory it offers
    cmd_wm: u8,
    /// Newest erase it offers
    cmd_er: u8,
    cmd_rm: u8,
}

impl<'a> Bootloader<'a> {
    /// Get Version, Get and Get ID. Retries the first command for a while,
    /// because the part is still coming out of reset when ENTER returns.
    pub fn init(ec: &'a CrosEc) -> Stm32KbdResult<Self> {
        let mut bl = Self {
            ec,
            version: 0,
            bl_version: 0,
            pid: 0,
            cmd_wm: CMD_NONE,
            cmd_er: CMD_NONE,
            cmd_rm: CMD_NONE,
        };

        // The first write is NACKed at the address until the ROM is listening
        let mut waited = 0;
        while xfer(ec, &[CMD_GVR, CMD_GVR ^ 0xFF], 0).is_err() {
            if waited >= ENTER_TIMEOUT_MS {
                return Err(comms(format!(
                    "the STM32 bootloader does not answer at 0x{:02X}",
                    BL_ADDR
                )));
            }
            sleep_ms(20);
            waited += 20;
        }
        get_ack(ec, ACK_TIMEOUT_MS, "Get Version")?;
        bl.version = xfer(ec, &[], 1)?[0];
        get_ack(ec, ACK_TIMEOUT_MS, "Get Version")?;

        let get_len = GET_REPLY_LENS
            .iter()
            .find(|(v, _)| *v == bl.version)
            .map(|(_, len)| *len)
            .unwrap_or(17);
        send_cmd(ec, CMD_GET, "Get")?;
        let buf = xfer(ec, &[], get_len + 2)?;
        get_ack(ec, ACK_TIMEOUT_MS, "Get")?;
        let n = core::cmp::min(buf[0] as usize + 1, get_len + 1);
        bl.bl_version = buf[1];
        for &c in &buf[2..n + 1] {
            match c {
                CMD_RM => bl.cmd_rm = c,
                CMD_WM | CMD_WM_NS => bl.cmd_wm = newer(bl.cmd_wm, c),
                CMD_ER | CMD_EE | CMD_EE_NS => bl.cmd_er = newer(bl.cmd_er, c),
                _ => {}
            }
        }

        send_cmd(ec, CMD_GID, "Get ID")?;
        let buf = xfer(ec, &[], 3)?;
        get_ack(ec, ACK_TIMEOUT_MS, "Get ID")?;
        bl.pid = if buf[0] >= 1 {
            u16::from_be_bytes([buf[1], buf[2]])
        } else {
            buf[1] as u16
        };
        Ok(bl)
    }

    fn require(cmd: u8, what: &str) -> Stm32KbdResult<u8> {
        if cmd == CMD_NONE {
            return Err(Stm32KbdError::new(
                ErrorKind::Other,
                format!("the bootloader offers no {}", what),
            ));
        }
        Ok(cmd)
    }

    pub fn mass_erase(&self) -> Stm32KbdResult<()> {
        let cmd = Self::require(self.cmd_er, "erase")?;
        send_cmd(self.ec, cmd, "Erase")?;
        // Regular erase: 0xFF + complement. Extended: 0xFFFF + checksum.
        let frame: &[u8] = if cmd == CMD_ER {
            &[0xFF, 0x00]
        } else {
            &[0xFF, 0xFF, 0x00]
        };
        write_then_ack(self.ec, frame, ERASE_TIMEOUT_MS, "the mass erase")
    }

    /// Erase one PAGE_SIZE page, counted from the start of flash
    pub fn erase_page(&self, page: u16) -> Stm32KbdResult<()> {
        let cmd = Self::require(self.cmd_er, "erase")?;
        send_cmd(self.ec, cmd, "Erase")?;
        let frame: Vec<u8> = if cmd == CMD_ER {
            // Regular erase: N-1 = 0, one page number, XOR
            let p = page as u8;
            [0, p, p].to_vec()
        } else {
            // Extended: 16-bit N-1 = 0, one 16-bit page number, XOR
            let [hi, lo] = page.to_be_bytes();
            [0, 0, hi, lo, hi ^ lo].to_vec()
        };
        write_then_ack(self.ec, &frame, WRITE_TIMEOUT_MS * 5, "the page erase")
    }

    /// addr must be 4-byte aligned, data 1..MAX_DATA bytes. Padded to 4 with 0xFF.
    pub fn write(&self, addr: u32, data: &[u8]) -> Stm32KbdResult<()> {
        let cmd = Self::require(self.cmd_wm, "Write Memory")?;
        let aligned = (data.len() + 3) & !3;
        if data.is_empty() || aligned > MAX_DATA || addr & 3 != 0 {
            return Err(Stm32KbdError::new(
                ErrorKind::Other,
                format!("bad write: {} bytes at 0x{:08X}", data.len(), addr),
            ));
        }
        send_cmd(self.ec, cmd, "Write Memory")?;
        send_addr(self.ec, addr)?;

        // [N-1] [data, padded with 0xFF to 4] [XOR of all of it]
        let mut frame = Vec::with_capacity(aligned + 2);
        frame.push((aligned - 1) as u8);
        frame.extend_from_slice(data);
        frame.resize(aligned + 1, 0xFF);
        frame.push(frame.iter().fold(0, |acc, b| acc ^ b));
        write_then_ack(self.ec, &frame, WRITE_TIMEOUT_MS, "the write")
    }

    /// Read 1..MAX_DATA bytes
    pub fn read(&self, addr: u32, len: usize) -> Stm32KbdResult<Vec<u8>> {
        let cmd = Self::require(self.cmd_rm, "Read Memory")?;
        if len == 0 || len > MAX_DATA {
            return Err(Stm32KbdError::new(
                ErrorKind::Other,
                format!("bad read length {}", len),
            ));
        }
        send_cmd(self.ec, cmd, "Read Memory")?;
        send_addr(self.ec, addr)?;
        send_cmd(self.ec, (len - 1) as u8, "the read length")?;
        xfer(self.ec, &[], len)
    }
}
