//! Talking to the running keyboard firmware: the I2C-HID descriptor and the
//! vendor command protocol on top of it

use alloc::format;
use alloc::string::ToString;
use alloc::vec;

use super::{
    i2c_xfer, sleep_ms, ErrorKind, Stm32KbdError, Stm32KbdResult, BL_ADDR, HID_ADDR, I2C_PORT,
};
use crate::chromium_ec::i2c_passthrough::i2c_transfer;
use crate::chromium_ec::CrosEc;

const I2C_HID_DESC_REGISTER: u16 = 0x0001;
const I2C_HID_CMD_GET_REPORT: u8 = 0x02;
const I2C_HID_CMD_SET_REPORT: u8 = 0x03;

const HID_VENDOR_REPORT_ID: u8 = 0x20;
/// The SET_REPORT write is 2 + 2 + 1 + 31 = 36 bytes, exactly the deck's
/// max-output-length. Growing it would overrun the target's write buffer.
const HID_VENDOR_PAYLOAD_SIZE: usize = 31;
/// Feature report, ID >= 15 follows
const HID_VENDOR_CMD_FEATURE: u8 = 0x3F;

const VENDOR_OP_GET_FW_VERSION: u8 = 0x01;
const VENDOR_OP_GET_HW_INFO: u8 = 0x02;
const VENDOR_OP_SET_FLAGS: u8 = 0x12;
const VENDOR_OP_GET_FLAGS: u8 = 0x13;
/// Status of an opcode the firmware doesn't know
const VENDOR_STATUS_UNKNOWN_OP: u8 = 0xFF;

/// Bit 0 of the descriptor's reserved word is a "do not update" hint.
/// It is advice aimed at automatic updaters, not a lock.
const DESC_FLAG_NO_UPDATE: u32 = 0x0000_0001;

/// Fn Lock, R/W, persisted in NVS
pub const FLAG_FN_LOCK: u32 = 0x0000_0001;
/// Fn/Ctrl swap, R/W, RAM only
pub const FLAG_FN_CTRL_SWAP: u32 = 0x0000_0002;
/// Update protect, R/O, misc-flags slot 0
pub const FLAG_UPDATE_PROTECT: u32 = 0x0000_0004;
/// The host's Caps Lock indicator, R/O
pub const FLAG_CAPS_LOCK: u32 = 0x0000_0008;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HidDescriptor {
    pub command_register: u16,
    pub data_register: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    /// Firmware version, nibble-encoded, see [`super::format_version`]
    pub version_id: u16,
    pub reserved: u32,
}

impl HidDescriptor {
    /// Firmware asks automatic updaters to leave it alone
    pub fn no_update(&self) -> bool {
        self.reserved & DESC_FLAG_NO_UPDATE != 0
    }
}

/// What is on the end of the bus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckState {
    /// Nothing answers on the bus
    Absent,
    /// No application; the ROM bootloader answers. Needs flashing.
    Bootloader,
    /// The application runs, but predates the vendor command protocol
    Legacy(HidDescriptor),
    /// The application runs and answers vendor commands
    Ready(HidDescriptor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HwInfo {
    /// Shipped firmware reports 0, use [`super::identify`] to read it from ROM
    pub chip_id: u16,
    pub flash_kb: u16,
    pub ram_kb: u16,
    pub uid: [u8; 12],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FwVersion {
    /// Nibble-encoded, see [`super::format_version`]
    pub version: u16,
    /// Not built from a released state; the hash identifies the build
    pub dirty: bool,
    pub hash: [u8; 8],
}

/// The flag block. Bits not listed as constants here are from newer firmware,
/// ignore them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags {
    /// Bits this firmware implements
    pub present: u32,
    /// Their values; only meaningful within present
    pub flags: u32,
    /// Subset of present that [`set_flags`] accepts
    pub writable: u32,
}

fn rd16(p: &[u8]) -> u16 {
    u16::from_le_bytes([p[0], p[1]])
}

fn rd32(p: &[u8]) -> u32 {
    u32::from_le_bytes([p[0], p[1], p[2], p[3]])
}

/// Read the I2C-HID descriptor. It's served below the vendor protocol, so
/// every firmware ever shipped answers it.
pub fn hid_descriptor(ec: &CrosEc) -> Stm32KbdResult<HidDescriptor> {
    let raw = i2c_xfer(ec, HID_ADDR, &I2C_HID_DESC_REGISTER.to_le_bytes(), 30)?;
    let len = rd16(&raw[0..]);
    let bcd = rd16(&raw[2..]);
    if bcd != 0x0100 || len != 30 {
        return Err(Stm32KbdError::new(
            ErrorKind::Comms,
            format!(
                "implausible HID descriptor (len {}, bcd 0x{:04X}) - is the deck in bootloader mode?",
                len, bcd
            ),
        ));
    }
    Ok(HidDescriptor {
        command_register: rd16(&raw[16..]),
        data_register: rd16(&raw[18..]),
        vendor_id: rd16(&raw[20..]),
        product_id: rd16(&raw[22..]),
        version_id: rd16(&raw[24..]),
        reserved: rd32(&raw[26..]),
    })
}

/// One vendor command: SET_REPORT with the request, then GET_REPORT to
/// collect the reply. Returns status and reply `[opcode][status][data...]`,
/// resp_len bytes of it.
fn vendor_cmd(
    ec: &CrosEc,
    req: &[u8],
    resp_len: usize,
) -> Stm32KbdResult<(u8, alloc::vec::Vec<u8>)> {
    if req.is_empty() || req.len() > HID_VENDOR_PAYLOAD_SIZE {
        return Err(Stm32KbdError::new(
            ErrorKind::Other,
            format!("bad vendor request length {}", req.len()),
        ));
    }
    let desc = hid_descriptor(ec)?;
    let [cmd_lo, cmd_hi] = desc.command_register.to_le_bytes();
    let [data_lo, data_hi] = desc.data_register.to_le_bytes();
    let length = (2 + 1 + HID_VENDOR_PAYLOAD_SIZE) as u16;

    // The deck keeps only the most recent response, and the EC issues vendor
    // writes of its own (fn/ctrl swap sync). If the echoed opcode does not
    // match ours, someone interleaved - retry.
    for _ in 0..3 {
        let set_report = [
            cmd_lo,
            cmd_hi,
            HID_VENDOR_CMD_FEATURE,
            I2C_HID_CMD_SET_REPORT,
            HID_VENDOR_REPORT_ID,
            data_lo,
            data_hi,
        ];
        i2c_xfer(ec, HID_ADDR, &set_report, 0)?;
        sleep_ms(2);

        let mut payload = vec![data_lo, data_hi];
        payload.extend_from_slice(&length.to_le_bytes());
        payload.push(HID_VENDOR_REPORT_ID);
        payload.extend_from_slice(req);
        payload.resize(2 + 2 + 1 + HID_VENDOR_PAYLOAD_SIZE, 0);
        i2c_xfer(ec, HID_ADDR, &payload, 0)?;
        sleep_ms(3);

        let get_report = [
            cmd_lo,
            cmd_hi,
            HID_VENDOR_CMD_FEATURE,
            I2C_HID_CMD_GET_REPORT,
            HID_VENDOR_REPORT_ID,
            data_lo,
            data_hi,
        ];
        i2c_xfer(ec, HID_ADDR, &get_report, 0)?;
        sleep_ms(2);

        // The reply is only as long as the opcode's, so ask for exactly that.
        // rx = [len:u16][report id][opcode][status][data...]
        let rx = i2c_xfer(ec, HID_ADDR, &[data_lo, data_hi], 3 + resp_len)?;
        if rx[2] == HID_VENDOR_REPORT_ID && rx[3] == req[0] {
            return Ok((rx[4], rx[3..].to_vec()));
        }
        sleep_ms(5);
    }
    Err(Stm32KbdError::new(
        ErrorKind::Comms,
        format!(
            "no matching reply for opcode 0x{:02X} after 3 attempts",
            req[0]
        ),
    ))
}

/// Like vendor_cmd, but any status other than success is an error
fn vendor_cmd_ok(ec: &CrosEc, req: &[u8], resp_len: usize) -> Stm32KbdResult<alloc::vec::Vec<u8>> {
    let (status, resp) = vendor_cmd(ec, req, resp_len)?;
    if status != 0 {
        return Err(Stm32KbdError::new(
            ErrorKind::Refused,
            format!(
                "deck rejected opcode 0x{:02X} (status 0x{:02X})",
                req[0], status
            ),
        ));
    }
    Ok(resp)
}

/// Find out what is on the bus. Costs two I2C transactions at worst and
/// power-cycles nothing, so it's safe to call any time.
///
/// Only fails if the EC itself could not be reached.
pub fn probe(ec: &CrosEc) -> Stm32KbdResult<DeckState> {
    if let Ok(desc) = hid_descriptor(ec) {
        // Answering the descriptor and not the vendor channel places the
        // firmware before the channel existed
        return Ok(if fw_version(ec).is_ok() {
            DeckState::Ready(desc)
        } else {
            DeckState::Legacy(desc)
        });
    }

    // An STM32 with empty flash starts in its ROM bootloader by itself, so a
    // blank controller answers there while the HID address stays silent.
    // A one-byte read asks the bootloader for nothing and changes nothing.
    let res = i2c_transfer(ec, I2C_PORT, BL_ADDR, &[], 1)?;
    Ok(if res.i2c_status == 0 {
        DeckState::Bootloader
    } else {
        DeckState::Absent
    })
}

pub fn hw_info(ec: &CrosEc) -> Stm32KbdResult<HwInfo> {
    // [opcode][status][chip_id:u16][flash_kb:u16][ram_kb:u16][uid:12]
    let resp = vendor_cmd_ok(ec, &[VENDOR_OP_GET_HW_INFO], 20)?;
    let mut uid = [0; 12];
    uid.copy_from_slice(&resp[8..20]);
    Ok(HwInfo {
        chip_id: rd16(&resp[2..]),
        flash_kb: rd16(&resp[4..]),
        ram_kb: rd16(&resp[6..]),
        uid,
    })
}

pub fn fw_version(ec: &CrosEc) -> Stm32KbdResult<FwVersion> {
    // [opcode][status][version:u16][dirty][hash:8]
    let resp = vendor_cmd_ok(ec, &[VENDOR_OP_GET_FW_VERSION], 13)?;
    let mut hash = [0; 8];
    hash.copy_from_slice(&resp[5..13]);
    Ok(FwVersion {
        version: rd16(&resp[2..]),
        dirty: resp[4] != 0,
        hash,
    })
}

/// Read the flag block. None if this firmware doesn't have one.
pub fn get_flags(ec: &CrosEc) -> Stm32KbdResult<Option<Flags>> {
    // [opcode][status][present:u32][flags:u32][writable:u32]
    let (status, resp) = vendor_cmd(ec, &[VENDOR_OP_GET_FLAGS], 14)?;
    match status {
        0 => Ok(Some(Flags {
            present: rd32(&resp[2..]),
            flags: rd32(&resp[6..]),
            writable: rd32(&resp[10..]),
        })),
        VENDOR_STATUS_UNKNOWN_OP => Ok(None),
        _ => Err(Stm32KbdError::new(
            ErrorKind::Other,
            format!("GET_FLAGS failed (status 0x{:02X})", status),
        )),
    }
}

/// Set the bits named in mask to value, leaving the others alone, so no
/// read-modify-write is needed. Returns the resulting flags.
pub fn set_flags(ec: &CrosEc, mask: u32, value: u32) -> Stm32KbdResult<u32> {
    let mut req = vec![VENDOR_OP_SET_FLAGS];
    req.extend_from_slice(&mask.to_le_bytes());
    req.extend_from_slice(&value.to_le_bytes());

    // Setting FN_LOCK writes NVS, and an occasional garbage collection erases
    // a page while this part executes from flash - so the device can NAK for
    // a few tens of milliseconds afterwards. Retrying is safe: setting a bit
    // to the value it already holds costs no write.
    let mut res = vendor_cmd(ec, &req, 6);
    for _ in 1..8 {
        if res.is_ok() {
            break;
        }
        sleep_ms(25);
        res = vendor_cmd(ec, &req, 6);
    }
    let (status, resp) = res?;
    match status {
        0 => Ok(rd32(&resp[2..])),
        VENDOR_STATUS_UNKNOWN_OP => Err(Stm32KbdError::new(
            ErrorKind::Refused,
            "this firmware has no flag block".to_string(),
        )),
        0x01 => Err(Stm32KbdError::new(
            ErrorKind::Refused,
            format!(
                "the deck refused the change: mask 0x{:08X} names a bit that is unknown or read-only. Nothing was changed.",
                mask
            ),
        )),
        _ => Err(Stm32KbdError::new(
            ErrorKind::Other,
            format!("SET_FLAGS failed (status 0x{:02X})", status),
        )),
    }
}
