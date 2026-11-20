//! Fuzz EC firmware flashing (--flash-ec, --flash-ro-ec, --flash-rw-ec, --flash-full-ec)
//!
//! Unlike the other targets, the EC firmware file is not taken from the fuzzer input
//! directly. The interesting parts (version structs at fixed offsets ~256K into a
//! 512K file) would practically never be found. Instead the fuzzer input selects
//! the flash type, platforms, version struct placement, file length, what the EC
//! reports and where faults are injected. The harness builds a matching file and
//! runs `CrosEc::reflash` against a model of the EC flash.
//!
//! Besides crashes and hangs, it checks that reflashing:
//! - never erases or writes outside the flash size reported by the EC
//! - only erases and writes the regions of the selected flash type
//! - only writes erased flash, with the data from the file at the same offset
//! - doesn't touch flash at all, if the file is invalid, too short or for another platform
//! - doesn't touch flash at all in dry-run mode
//! - only touches flash between FirmwareStart and FirmwareDone
//! - always sends AccessSpiDone and FirmwareDone after FirmwareStart, even on errors
//! - leaves the file contents in flash, if it reports success
#![no_main]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::LazyLock;

use framework_lib::chromium_ec::command::EcCommands;
use framework_lib::chromium_ec::{fuzz_driver, EcError, EcFlashType, EcResponseStatus, EcResult};
use libfuzzer_sys::fuzz_target;

const FLASH_SIZE: usize = 0x80000;
const RW_BASE: usize = 0x40000;
const MEC_RO_SIZE: usize = 0x3A000;
const NPC_RO_SIZE: usize = 0x3F000;
const RW_SIZE: usize = 0x3F000;

// Offsets of the version struct (struct image_data) in the EC binary
const RO_VER_LEGACY: usize = 0x2430;
const RW_VER_LEGACY: usize = 0x402f0;
const RO_VER_ZEPHYR: usize = 0x00180;
const RW_VER_ZEPHYR: usize = 0x40140;
const IMAGE_DATA_LEN: usize = 48;

// MecFlashNotify
const ACCESS_SPI: u8 = 0x00;
const FIRMWARE_START: u8 = 0x01;
const FIRMWARE_DONE: u8 = 0x02;
const ACCESS_SPI_DONE: u8 = 0x03;

/// Legacy MEC EC platforms first, then NPC/Zephyr
const PLATFORMS: &[&str] = &["hx20", "hx30", "azalea", "lotus", "marigold"];

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

#[derive(Clone, Copy)]
enum VersionPlacement {
    None,
    Legacy,
    Zephyr,
}

/// Fields are only read for Debug output of failures
#[allow(dead_code)]
#[derive(Debug)]
enum Op {
    Notify(u8),
    Erase(usize, usize),
    Write(usize, usize),
    Read(usize, usize),
    Other(u16),
}

#[derive(Clone, Copy)]
enum Fault {
    Error,
    ShortReply,
    LongReply,
    Corrupt,
}

struct Model {
    flash: Vec<u8>,
    /// Bytes erased and not written since
    erased: Vec<bool>,
    image: Rc<Vec<u8>>,
    build_info: Vec<u8>,
    flash_info: [u32; 6],
    faults: Vec<(usize, Fault)>,
    cmd_no: usize,
    log: Vec<Op>,
    violations: Vec<String>,
}

/// Physically larger than the default flash size, to notice anything beyond the reported size
const PHYS_SIZE: usize = 0x100000;
/// Different from the image pattern, so a skipped write is noticed
static INITIAL_FLASH: LazyLock<Vec<u8>> =
    LazyLock::new(|| (0..PHYS_SIZE).map(|i| !image_pattern(i)).collect());

/// Position dependent, so a write to the wrong offset is noticed
fn image_pattern(i: usize) -> u8 {
    (i as u8) ^ ((i >> 8) as u8).rotate_left(3) ^ ((i >> 16) as u8).rotate_left(5)
}

fn le_u32(data: &[u8], offset: usize) -> Option<usize> {
    Some(u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as usize)
}

impl Model {
    fn reported_size(&self) -> usize {
        (self.flash_info[0] as usize).min(self.flash.len())
    }

    /// Check range against reported flash size, the EC would reject it
    fn check_range(&mut self, what: &str, offset: usize, size: usize) -> EcResult<()> {
        let end = offset.checked_add(size);
        if end.is_none_or(|end| end > self.reported_size()) {
            self.violations.push(format!(
                "{} 0x{:X}+0x{:X} beyond reported flash size 0x{:X}",
                what, offset, size, self.flash_info[0]
            ));
            return Err(EcError::Response(EcResponseStatus::InvalidParameter));
        }
        Ok(())
    }

    fn handle(&mut self, command: u16, data: &[u8]) -> EcResult<Vec<u8>> {
        let fault = self
            .faults
            .iter()
            .find(|(n, _)| *n == self.cmd_no)
            .map(|(_, f)| *f);
        self.cmd_no += 1;
        // Log even if it fails, the command was sent and the EC might have acted on it
        self.log.push(log_op(command, data));
        if let Some(Fault::Error) = fault {
            return Err(EcError::Response(EcResponseStatus::Error));
        }

        let mut resp = self.execute(command, data)?;
        match fault {
            Some(Fault::ShortReply) => {
                resp.pop();
            }
            Some(Fault::LongReply) => resp.extend_from_slice(&[0xAA; 16]),
            Some(Fault::Corrupt) => {
                if let Some(b) = resp.first_mut() {
                    *b ^= 0x01;
                }
            }
            _ => {}
        }
        Ok(resp)
    }

    fn execute(&mut self, command: u16, data: &[u8]) -> EcResult<Vec<u8>> {
        let invalid = EcError::Response(EcResponseStatus::InvalidParameter);
        match command {
            c if c == EcCommands::GetBuildInfo as u16 => Ok(self.build_info.clone()),
            c if c == EcCommands::FlashInfo as u16 => Ok(self
                .flash_info
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect()),
            c if c == EcCommands::FlashNotified as u16 => {
                data.first().ok_or(invalid)?;
                Ok(vec![])
            }
            c if c == EcCommands::FlashErase as u16 => {
                let (offset, size) = (le_u32(data, 0).unwrap(), le_u32(data, 4).unwrap());
                self.check_range("Erase", offset, size)?;
                self.flash[offset..offset + size].fill(0xFF);
                self.erased[offset..offset + size].fill(true);
                Ok(vec![])
            }
            c if c == EcCommands::FlashWrite as u16 => {
                let (offset, size) = (le_u32(data, 0).unwrap(), le_u32(data, 4).unwrap());
                let payload = &data[8..];
                if payload.len() != size {
                    self.violations.push(format!(
                        "Write 0x{:X} claims 0x{:X} bytes, sends 0x{:X}",
                        offset,
                        size,
                        payload.len()
                    ));
                    return Err(invalid);
                }
                self.check_range("Write", offset, size)?;
                if self.image.get(offset..offset + size) != Some(payload) {
                    self.violations.push(format!(
                        "Write 0x{:X}+0x{:X} doesn't match file",
                        offset, size
                    ));
                }
                if !self.erased[offset..offset + size].iter().all(|x| *x) {
                    self.violations.push(format!(
                        "Write 0x{:X}+0x{:X} to unerased flash",
                        offset, size
                    ));
                }
                self.flash[offset..offset + size].copy_from_slice(payload);
                self.erased[offset..offset + size].fill(false);
                Ok(vec![])
            }
            c if c == EcCommands::FlashRead as u16 => {
                let (offset, size) = (le_u32(data, 0).unwrap(), le_u32(data, 4).unwrap());
                self.check_range("Read", offset, size)?;
                Ok(self.flash[offset..offset + size].to_vec())
            }
            _ => Err(EcError::Response(EcResponseStatus::InvalidCommand)),
        }
    }
}

fn log_op(command: u16, data: &[u8]) -> Op {
    let range = || (le_u32(data, 0).unwrap(), le_u32(data, 4).unwrap());
    match command {
        c if c == EcCommands::FlashNotified as u16 => match data.first() {
            Some(&flags) => Op::Notify(flags),
            None => Op::Other(c),
        },
        c if c == EcCommands::FlashErase as u16 => {
            let (offset, size) = range();
            Op::Erase(offset, size)
        }
        c if c == EcCommands::FlashWrite as u16 => {
            let (offset, size) = range();
            Op::Write(offset, size)
        }
        c if c == EcCommands::FlashRead as u16 => {
            let (offset, size) = range();
            Op::Read(offset, size)
        }
        c => Op::Other(c),
    }
}

fn plant_version(image: &mut [u8], offset: usize, version: &str) {
    let Some(dst) = image.get_mut(offset..offset + IMAGE_DATA_LEN) else {
        return;
    };
    let mut v = [0u8; 32];
    v[..version.len()].copy_from_slice(version.as_bytes());
    dst[0..4].copy_from_slice(&0xce778899u32.to_le_bytes());
    dst[4..36].copy_from_slice(&v);
    dst[36..40].copy_from_slice(&0x40000u32.to_le_bytes());
    dst[40..44].copy_from_slice(&0u32.to_le_bytes());
    dst[44..48].copy_from_slice(&0xceaabbddu32.to_le_bytes());
}

fn version_str(platform: &str) -> String {
    format!("{}_v0.0.1-7a61a89", platform)
}

fn ranges_overlap_outside(op: (usize, usize), allowed: &[(usize, usize)]) -> bool {
    let (start, size) = op;
    let end = start + size;
    !allowed.iter().any(|&(a, len)| start >= a && end <= a + len)
}

fuzz_target!(|data: &[u8]| {
    let mut input = Input(data);

    let cfg = input.u8();
    // EcFlashType isn't Copy
    let ft = || match cfg & 0x3 {
        0 => EcFlashType::Full,
        1 => EcFlashType::Both,
        2 => EcFlashType::Ro,
        _ => EcFlashType::Rw,
    };
    let dry_run = cfg & 0x4 != 0;

    let running = PLATFORMS[input.u8() as usize % PLATFORMS.len()];
    let file_platform = |input: &mut Input| {
        let b = input.u8();
        if b < 0xC0 {
            running
        } else {
            PLATFORMS[b as usize % PLATFORMS.len()]
        }
    };
    let ro_platform = file_platform(&mut input);
    let rw_platform = file_platform(&mut input);
    let placement = |b: u8| match b % 4 {
        0 => VersionPlacement::None,
        1 | 2 => VersionPlacement::Zephyr,
        _ => VersionPlacement::Legacy,
    };
    let b = input.u8();
    let (ro_place, rw_place) = (placement(b), placement(b >> 4));

    let file_len = input.pick(&[
        FLASH_SIZE as u32,
        FLASH_SIZE as u32 + 0x100,
        (RW_BASE + RW_SIZE) as u32,
        (RW_BASE + RW_SIZE - 1) as u32,
        RW_BASE as u32,
        NPC_RO_SIZE as u32,
        MEC_RO_SIZE as u32,
        (RW_VER_ZEPHYR + IMAGE_DATA_LEN) as u32,
        (RW_VER_LEGACY + IMAGE_DATA_LEN) as u32,
    ]) as usize
        % (FLASH_SIZE + 0x101);
    let flash_size = input.pick(&[
        FLASH_SIZE as u32,
        (RW_BASE + RW_SIZE) as u32,
        (RW_BASE + RW_SIZE - 1) as u32,
        0x100000,
        0,
    ]);
    let erase_block_size = input.pick(&[0x1000, 0x10000, 0x80, 0, 0x7F000, 0xFFFF_F000, 3]);
    let write_block_size = input.u32();
    let protect_block_size = input.u32();
    let write_ideal_size = input.u32();
    let flags = input.u32();

    // Up to two faults, at a command number and of a kind
    let mut faults = vec![];
    for _ in 0..2 {
        let kind = input.u8();
        let at = u16::from_le_bytes([input.u8(), input.u8()]) as usize;
        let kind = match kind % 8 {
            0 => Fault::Error,
            1 => Fault::ShortReply,
            2 => Fault::LongReply,
            3 => Fault::Corrupt,
            _ => continue,
        };
        faults.push((at, kind));
    }

    let mut image: Vec<u8> = (0..file_len).map(image_pattern).collect();
    match ro_place {
        VersionPlacement::None => {}
        VersionPlacement::Legacy => {
            plant_version(&mut image, RO_VER_LEGACY, &version_str(ro_platform))
        }
        VersionPlacement::Zephyr => {
            plant_version(&mut image, RO_VER_ZEPHYR, &version_str(ro_platform))
        }
    }
    match rw_place {
        VersionPlacement::None => {}
        VersionPlacement::Legacy => {
            plant_version(&mut image, RW_VER_LEGACY, &version_str(rw_platform))
        }
        VersionPlacement::Zephyr => {
            plant_version(&mut image, RW_VER_ZEPHYR, &version_str(rw_platform))
        }
    }
    let image = Rc::new(image);

    let mut build_info = version_str(running).into_bytes();
    build_info.push(0);

    let model = Rc::new(RefCell::new(Model {
        flash: INITIAL_FLASH.clone(),
        erased: vec![false; PHYS_SIZE],
        image: image.clone(),
        build_info,
        flash_info: [
            flash_size,
            write_block_size,
            erase_block_size,
            protect_block_size,
            write_ideal_size,
            flags,
        ],
        faults,
        cmd_no: 0,
        log: vec![],
        violations: vec![],
    }));

    // Pick any Framework platform to pass the SMBIOS check, reflash doesn't care which.
    // Empty memory map and tape, the command handler answers everything.
    let ec = fuzz_driver::setup(&[0]).unwrap();
    let handler_model = model.clone();
    fuzz_driver::set_command_handler(Box::new(move |command, _version, data| {
        Some(handler_model.borrow_mut().handle(command, data))
    }));

    let res = ec.reflash(&image, ft(), dry_run);

    let model = model.borrow();
    assert!(
        model.violations.is_empty(),
        "Violations: {:#?}",
        model.violations
    );

    // Which regions are allowed to be touched and whether the file is good to flash at all
    let has_mec = matches!(running, "hx20" | "hx30");
    let ro_size = if has_mec { MEC_RO_SIZE } else { NPC_RO_SIZE };
    let (needs_ro, needs_rw) = match ft() {
        EcFlashType::Full | EcFlashType::Both => (true, true),
        EcFlashType::Ro => (true, false),
        EcFlashType::Rw => (false, true),
    };
    let allowed: Vec<(usize, usize)> = match ft() {
        EcFlashType::Full => vec![(0, FLASH_SIZE)],
        EcFlashType::Both => vec![(0, ro_size), (RW_BASE, RW_SIZE)],
        EcFlashType::Ro => vec![(0, ro_size)],
        EcFlashType::Rw => vec![(RW_BASE, RW_SIZE)],
    };
    let required_len = match ft() {
        EcFlashType::Full => FLASH_SIZE,
        EcFlashType::Both | EcFlashType::Rw => RW_BASE + RW_SIZE,
        EcFlashType::Ro => ro_size,
    };
    let version_ok = |place: VersionPlacement, platform: &str, legacy: usize, zephyr: usize| {
        let offset = match place {
            VersionPlacement::None => return false,
            VersionPlacement::Legacy => legacy,
            VersionPlacement::Zephyr => zephyr,
        };
        platform == running && offset + IMAGE_DATA_LEN <= file_len
    };
    let file_ok = file_len >= required_len
        && (!needs_ro || version_ok(ro_place, ro_platform, RO_VER_LEGACY, RO_VER_ZEPHYR))
        && (!needs_rw || version_ok(rw_place, rw_platform, RW_VER_LEGACY, RW_VER_ZEPHYR));

    let mut started = false;
    let mut spi_done_after_start = false;
    let mut done_after_start = false;
    let mut modified = false;
    for op in &model.log {
        match *op {
            Op::Notify(flags) => match flags {
                FIRMWARE_START => {
                    started = true;
                    spi_done_after_start = false;
                    done_after_start = false;
                }
                FIRMWARE_DONE if started => done_after_start = true,
                ACCESS_SPI_DONE if started => spi_done_after_start = true,
                ACCESS_SPI | FIRMWARE_DONE | ACCESS_SPI_DONE => {}
                other => panic!("Unexpected flash notify flags 0x{:X}", other),
            },
            Op::Erase(offset, size) | Op::Write(offset, size) => {
                modified = true;
                assert!(!dry_run, "Flash modified in dry run: {:?}", op);
                assert!(file_ok, "Flash modified with invalid file: {:?}", op);
                assert!(
                    started && !done_after_start,
                    "Flash modified outside of FirmwareStart/Done: {:?}",
                    op
                );
                assert!(
                    !ranges_overlap_outside((offset, size), &allowed),
                    "{:?} outside of allowed regions {:X?}",
                    op,
                    allowed
                );
            }
            Op::Read(..) | Op::Other(_) => {}
        }
    }
    if started {
        assert!(
            spi_done_after_start && done_after_start,
            "Flash not locked again after FirmwareStart, result: {:?}, log: {:?}",
            res,
            model
                .log
                .iter()
                .filter(|op| matches!(op, Op::Notify(_)))
                .collect::<Vec<_>>()
        );
    }

    if res.is_ok() {
        assert!(file_ok, "Reflash succeeded with invalid file");
    }
    if res.is_ok() && !dry_run {
        assert!(modified);
        for (start, len) in &allowed {
            assert!(
                model.flash[*start..start + len] == image[*start..start + len],
                "Reported success, but flash 0x{:X}+0x{:X} doesn't match file",
                start,
                len
            );
        }
    }
    // Nothing outside the allowed regions changed
    let mut gap_start = 0;
    for &(start, len) in allowed.iter().chain(&[(PHYS_SIZE, 0)]) {
        assert!(
            model.flash[gap_start..start] == INITIAL_FLASH[gap_start..start],
            "Flash changed outside of allowed regions, between 0x{:X} and 0x{:X}",
            gap_start,
            start
        );
        gap_start = start + len;
    }
});
