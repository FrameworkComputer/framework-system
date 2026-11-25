//! Fuzz reading and writing the Framework 16 expansion bay (GPU) EEPROM
//! (--flash-gpu-descriptor-file, --dump-gpu-descriptor-file, --validate-gpu-descriptor-file,
//! --expansion-bay, --flash-gpu-descriptor)
//!
//! The EEPROM is behind the EC, accessed with EC_CMD_I2C_PASSTHRU. Like `ec_reflash`,
//! the fuzzer input doesn't contain the EEPROM image or file directly. It selects
//! the operation, the EEPROM size, the descriptor header fields, file length, the
//! expansion bay board, power state and where faults are injected. The harness
//! builds a matching EEPROM image and file and runs the operation against a model
//! of the EEPROM.
//!
//! The model decodes the I2C passthrough requests itself, so the bytes on the wire
//! are checked, not just the round-trip through the library's helpers.
//!
//! Besides crashes and hangs, it checks that:
//! - a dry run doesn't write to the EEPROM or change GPIOs
//! - every write carries the data from the file at the offset it's written to
//! - no write crosses an EEPROM page boundary (it would wrap around within the page)
//! - expansion bay power is restored, if it was forced on
//! - after a successful write, the EEPROM contains the file
//! - successful reads return what's in the EEPROM, nothing more or less
#![no_main]

use std::cell::RefCell;
use std::rc::Rc;

use framework_lib::chromium_ec::command::EcCommands;
use framework_lib::chromium_ec::commands::{
    SetGpuSerialMagic, BOARD_VERSION_11, BOARD_VERSION_12, BOARD_VERSION_13, BOARD_VERSION_15,
};
use framework_lib::chromium_ec::{fuzz_driver, EcError, EcResponseStatus, EcResult};
use libfuzzer_sys::fuzz_target;

const EEPROM_PORT: u8 = 5;
const EEPROM_ADDR: u16 = 0x50;
const PAGE_SIZE: usize = 32;
const HEADER_LEN: usize = 48;
const MAGIC: [u8; 4] = [0x32, 0xAC, 0x00, 0x00];
const I2C_READ_FLAG: u16 = 1 << 15;
const POWER_GPIO: &str = "gpu_3v_5v_en";

/// Reads fuzzer input, returns 0 once it runs out
struct Input<'a>(&'a [u8]);
impl Input<'_> {
    fn u8(&mut self) -> u8 {
        let Some((&b, rest)) = self.0.split_first() else {
            return 0;
        };
        self.0 = rest;
        b
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes([self.u8(), self.u8()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    /// Pick from interesting values most of the time, otherwise an arbitrary one
    fn pick(&mut self, interesting: &[u32]) -> u32 {
        let sel = self.u8() as usize;
        match interesting.get(sel) {
            Some(&v) => v,
            None if sel < 0xC0 => interesting[sel % interesting.len()],
            None => self.u32(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Fault {
    /// Host command fails, I2C transfer not executed
    Error,
    /// I2C NAK on the first message, nothing executed
    Nak,
    /// I2C timeout on the first message, nothing executed
    Timeout,
    /// Executed, but reply truncated to this many bytes
    Truncate(usize),
    /// Executed, but garbage appended to the reply
    LongReply,
    /// Executed, but reply reports a different number of messages
    WrongMsgCount,
    /// Executed, but read returns fewer bytes with success status
    PartialRead,
}

/// Fields are only read for Debug output of failures
#[allow(dead_code)]
#[derive(Debug)]
enum Op {
    Write(usize, usize),
    Read(usize, usize),
    GpioSet(bool),
    Other(u16),
}

struct Model {
    eeprom: Vec<u8>,
    /// EEPROM only responds when powered (on the dual interposer power is switchable)
    powered: bool,
    dual_interposer: bool,
    bay_status: [u8; 3],
    gpu_serial: [u8; 22],
    faults: Vec<(usize, Fault)>,
    cmd_no: usize,
    /// Faults that actually hit an I2C command
    hit_faults: Vec<Fault>,
    log: Vec<Op>,
    violations: Vec<String>,
    file: Rc<Vec<u8>>,
    /// EEPROM's current address, set by a write, advanced by reads
    addr_ptr: usize,
}

fn le_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

impl Model {
    fn handle(&mut self, command: u16, data: &[u8]) -> EcResult<Vec<u8>> {
        match command {
            c if c == EcCommands::I2cPassthrough as u16 => self.i2c(data),
            c if c == EcCommands::ExpansionBayStatus as u16 => {
                self.log.push(Op::Other(c));
                Ok(self.bay_status.to_vec())
            }
            c if c == EcCommands::GpioGet as u16 => {
                self.log.push(Op::Other(c));
                if !data.starts_with(POWER_GPIO.as_bytes()) {
                    return Err(EcError::Response(EcResponseStatus::InvalidParameter));
                }
                Ok(vec![self.powered as u8])
            }
            c if c == EcCommands::GpioSet as u16 => {
                if !data.starts_with(POWER_GPIO.as_bytes()) || data.len() != 33 {
                    self.log.push(Op::Other(c));
                    return Err(EcError::Response(EcResponseStatus::InvalidParameter));
                }
                let value = data[32] != 0;
                self.log.push(Op::GpioSet(value));
                if self.dual_interposer {
                    self.powered = value;
                }
                Ok(vec![])
            }
            c if c == EcCommands::GetGpuSerial as u16 => {
                self.log.push(Op::Other(c));
                Ok(self.gpu_serial.to_vec())
            }
            c if c == EcCommands::ProgramGpuEeprom as u16 => {
                self.log.push(Op::Other(c));
                Ok(vec![1])
            }
            c => {
                self.log.push(Op::Other(c));
                Err(EcError::Response(EcResponseStatus::InvalidCommand))
            }
        }
    }

    fn i2c(&mut self, data: &[u8]) -> EcResult<Vec<u8>> {
        let invalid = || EcError::Response(EcResponseStatus::InvalidParameter);
        let fault = self
            .faults
            .iter()
            .find(|(n, _)| *n == self.cmd_no)
            .map(|(_, f)| *f);
        self.cmd_no += 1;
        if let Some(f) = fault {
            self.hit_faults.push(f);
        }

        // Decode the request like the EC does
        let (&port, rest) = data.split_first().ok_or_else(invalid)?;
        let (&count, rest) = rest.split_first().ok_or_else(invalid)?;
        let mut msgs = vec![];
        for i in 0..count as usize {
            let addr_flags = le_u16(rest, i * 4).ok_or_else(invalid)?;
            let len = le_u16(rest, i * 4 + 2).ok_or_else(invalid)? as usize;
            msgs.push((addr_flags, len));
        }
        let mut write_data = &rest[count as usize * 4..];
        let write_total: usize = msgs
            .iter()
            .filter(|(f, _)| f & I2C_READ_FLAG == 0)
            .map(|(_, l)| l)
            .sum();
        if write_total != write_data.len() {
            self.violations.push(format!(
                "I2C request has {} bytes of write data, messages say {}",
                write_data.len(),
                write_total
            ));
            return Err(invalid());
        }

        match fault {
            Some(Fault::Error) => return Err(EcError::Response(EcResponseStatus::Error)),
            Some(Fault::Nak) => return Ok(vec![1, 0]),
            Some(Fault::Timeout) => return Ok(vec![2, 0]),
            _ => {}
        }

        let mut resp = vec![0u8, 0];
        for (i, &(addr_flags, len)) in msgs.iter().enumerate() {
            let addr = addr_flags & !I2C_READ_FLAG;
            if port != EEPROM_PORT || addr != EEPROM_ADDR || !self.powered {
                // Not acknowledged, EC stops at the first failing message
                resp[0] = 1;
                resp[1] = i as u8;
                return Ok(resp);
            }
            if addr_flags & I2C_READ_FLAG != 0 {
                // Sequential read from the address pointer, wraps at the end of the EEPROM
                let ptr = self.addr_ptr;
                let mut len = len;
                if let Some(Fault::PartialRead) = fault {
                    len /= 2;
                }
                self.log.push(Op::Read(ptr, len));
                for j in 0..len {
                    resp.push(self.eeprom[(ptr + j) % self.eeprom.len()]);
                }
                self.addr_ptr = (ptr + len) % self.eeprom.len();
            } else {
                let (msg, rest) = write_data.split_at(len);
                write_data = rest;
                if msg.len() < 2 {
                    self.violations
                        .push(format!("I2C write of {} bytes, no address", msg.len()));
                    return Ok(vec![1, i as u8]);
                }
                // 16-bit address, MSB first
                let ptr = u16::from_be_bytes([msg[0], msg[1]]) as usize;
                let payload = &msg[2..];
                self.addr_ptr = ptr % self.eeprom.len();
                if !payload.is_empty() {
                    self.write(ptr, payload);
                }
            }
            resp[1] = i as u8 + 1;
        }

        match fault {
            Some(Fault::Truncate(n)) => resp.truncate(n),
            Some(Fault::LongReply) => resp.extend_from_slice(&[0xAA; 16]),
            Some(Fault::WrongMsgCount) => resp[1] = resp[1].wrapping_add(1),
            _ => {}
        }
        Ok(resp)
    }

    fn write(&mut self, ptr: usize, payload: &[u8]) {
        self.log.push(Op::Write(ptr, payload.len()));
        if ptr / PAGE_SIZE != (ptr + payload.len() - 1) / PAGE_SIZE {
            self.violations.push(format!(
                "Write 0x{:X}+0x{:X} crosses EEPROM page boundary",
                ptr,
                payload.len()
            ));
        }
        if self.file.get(ptr..ptr + payload.len()) != Some(payload) {
            self.violations.push(format!(
                "Write 0x{:X}+0x{:X} doesn't match file",
                ptr,
                payload.len()
            ));
        }
        // Like a real EEPROM, wrap within the page
        let page = ptr - ptr % PAGE_SIZE;
        for (j, b) in payload.iter().enumerate() {
            let a = page + (ptr % PAGE_SIZE + j) % PAGE_SIZE;
            let len = self.eeprom.len();
            self.eeprom[a % len] = *b;
        }
    }
}

/// Build a descriptor: header with the given fields, then position dependent data
fn descriptor(total_len: usize, length: u32, desc_len: u32, magic_ok: bool) -> Vec<u8> {
    let mut d: Vec<u8> = (0..total_len)
        .map(|i| (i as u8) ^ ((i >> 8) as u8).rotate_left(3) ^ 0x5A)
        .collect();
    let mut hdr = [0u8; HEADER_LEN];
    hdr[0..4].copy_from_slice(if magic_ok {
        &MAGIC
    } else {
        &[0x32, 0xAC, 0, 1]
    });
    hdr[4..8].copy_from_slice(&length.to_le_bytes());
    hdr[36..40].copy_from_slice(&desc_len.to_le_bytes());
    let n = total_len.min(HEADER_LEN);
    d[..n].copy_from_slice(&hdr[..n]);
    d
}

fuzz_target!(|data: &[u8]| {
    let mut input = Input(data);

    let op = input.u8() % 6;
    let dry_run = input.u8() & 1 != 0;
    let eeprom_size = [0x1000, 0x2000, 0x8000, 0x10000][input.u8() as usize % 4];

    let board = input.u8();
    let (b0, b1) = match board % 5 {
        0 => (BOARD_VERSION_12, BOARD_VERSION_12),
        1 => (BOARD_VERSION_13, BOARD_VERSION_15),
        2 => (BOARD_VERSION_11, BOARD_VERSION_15),
        3 => (BOARD_VERSION_15, BOARD_VERSION_15),
        _ => (input.u8(), input.u8()),
    };
    let dual_interposer = (b0, b1) == (BOARD_VERSION_12, BOARD_VERSION_12);
    // Only the dual interposer can switch power, others are always powered
    let initially_powered = !dual_interposer || board & 0x80 != 0;

    // Header in the EEPROM
    let lengths = [
        HEADER_LEN as u32,
        0,
        1,
        31,
        32,
        33,
        0x100,
        0x1000 - HEADER_LEN as u32,
        0xFFFF - HEADER_LEN as u32,
        0x10000 - HEADER_LEN as u32,
        0x10000,
        0xFFFF_FFFF,
        0x8000_0000,
    ];
    let ee_length = input.pick(&lengths);
    let ee_desc_len = input.pick(&lengths);
    let ee_magic_ok = input.u8() < 0xE0;
    let mut eeprom = descriptor(eeprom_size, ee_length, ee_desc_len, ee_magic_ok);
    // Distinguish EEPROM from file contents, except for the header
    for b in eeprom.iter_mut().skip(HEADER_LEN) {
        *b = !*b;
    }

    // File to write or validate against
    let file_len = input.pick(&[
        0,
        1,
        4,
        31,
        32,
        33,
        HEADER_LEN as u32,
        0x200,
        0x1000,
        0x2000 + 7,
        0xFFFF,
        0x10000,
        0x10000 + 32,
        0x10021,
    ]) as usize
        % 0x10100;
    let file_length = input.pick(&lengths);
    let file_desc_len = input.pick(&lengths);
    let file = Rc::new(descriptor(file_len, file_length, file_desc_len, true));

    let mut gpu_serial = [0u8; 22];
    gpu_serial[1] = input.u8() & 1;
    for b in gpu_serial[2..].iter_mut() {
        *b = input.u8();
    }

    // Up to two faults, at a passthrough command number and of a kind
    let mut faults = vec![];
    for _ in 0..2 {
        let kind = input.u8();
        let at = input.u16() as usize % 0x900;
        let kind = match kind % 10 {
            0 => Fault::Error,
            1 => Fault::Nak,
            2 => Fault::Timeout,
            3 => Fault::Truncate(0),
            4 => Fault::Truncate(1),
            5 => Fault::LongReply,
            6 => Fault::WrongMsgCount,
            7 => Fault::PartialRead,
            _ => continue,
        };
        faults.push((at, kind));
    }

    let initial_eeprom = eeprom.clone();
    let model = Rc::new(RefCell::new(Model {
        eeprom,
        powered: initially_powered,
        dual_interposer,
        bay_status: [0x07, b0, b1],
        gpu_serial,
        faults,
        cmd_no: 0,
        hit_faults: vec![],
        log: vec![],
        violations: vec![],
        file: file.clone(),
        addr_ptr: 0,
    }));

    // Framework 16, the EEPROM is only there
    let ec = fuzz_driver::setup(&[9]).unwrap();
    let handler_model = model.clone();
    fuzz_driver::set_command_handler(Box::new(move |command, _version, data| {
        Some(handler_model.borrow_mut().handle(command, data))
    }));

    // What a correct read from offset 0 returns: what's in the EEPROM, wrapping around
    let expected = |len: usize| -> Vec<u8> {
        (0..len)
            .map(|i| initial_eeprom[i % initial_eeprom.len()])
            .collect()
    };

    match op {
        0 => {
            let res = ec.set_gpu_descriptor(&file, dry_run);
            let m = model.borrow();
            if dry_run {
                assert!(
                    !m.log
                        .iter()
                        .any(|op| matches!(op, Op::Write(..) | Op::GpioSet(_))),
                    "Dry run modified EEPROM or GPIO: {:?}",
                    m.log
                );
            }
            assert_eq!(
                m.powered, initially_powered,
                "Expansion bay power not restored, result {:?}",
                res
            );
            if res.is_ok() && !dry_run {
                // Can't hold more than it has room for
                if file_len <= eeprom_size {
                    assert!(
                        m.eeprom[..file_len] == file[..],
                        "Write reported success, but EEPROM doesn't contain the file"
                    );
                } else {
                    // Reporting success here would be wrong, the EEPROM wraps around.
                    // But the tool doesn't know the EEPROM size, the write offsets
                    // and verification after writing take care of it.
                }
                drop(m);
                // Like the commandline does after writing
                let valid = ec.validate_gpu_descriptor(&file);
                let m = model.borrow();
                if file_len > eeprom_size && m.hit_faults.is_empty() {
                    assert!(
                        !matches!(valid, Ok(true)),
                        "File larger than EEPROM validated successfully"
                    );
                }
            }
        }
        1 => {
            let res = ec.read_gpu_descriptor();
            if let Ok(desc) = res {
                let total = ee_length as u64 + ee_desc_len as u64;
                assert!(ee_magic_ok, "Read descriptor with invalid magic");
                assert_eq!(desc.len() as u64, total, "Descriptor length wrong");
                assert!(desc == expected(desc.len()), "Descriptor content wrong");
            }
        }
        2 => {
            if let Ok(hdr) = ec.read_gpu_desc_header() {
                assert_eq!({ hdr.length }, ee_length);
                assert_eq!({ hdr.descriptor_length }, ee_desc_len);
            }
        }
        3 => {
            let addr = input.u16();
            let len = input.pick(&[256, 48, 1, 0, 128, 129, 0xFFFF]) as u16;
            if let Ok(chunk) = ec.read_ec_gpu_chunk(addr, len) {
                assert_eq!(chunk.len(), len as usize);
                let want: Vec<u8> = (0..len as usize)
                    .map(|i| initial_eeprom[(addr as usize + i) % initial_eeprom.len()])
                    .collect();
                assert!(chunk == want, "Read chunk content wrong");
            }
        }
        4 => {
            let _ = ec.get_gpu_serial();
            let len = input.u8() as usize % 24;
            let serial: String = (0..len).map(|_| (b'A' + input.u8() % 26) as char).collect();
            let _ = ec.set_gpu_serial(SetGpuSerialMagic::WriteGPUConfig as u8, serial);
        }
        _ => {
            // Validate against the EEPROM contents, optionally with one byte changed
            let len = (ee_length as u64 + ee_desc_len as u64).min(0x10000) as usize;
            let mut reference = expected(len);
            let flip = input.u16() as usize;
            let flipped = input.u8() & 1 != 0 && !reference.is_empty();
            if flipped {
                let i = flip % reference.len();
                reference[i] ^= 0x01;
            }
            if let Ok(valid) = ec.validate_gpu_descriptor(&reference) {
                assert!(ee_magic_ok);
                assert_eq!(valid, !flipped, "Validation result wrong");
            }
        }
    }

    let m = model.borrow();
    assert!(m.violations.is_empty(), "Violations: {:#?}", m.violations);
});
