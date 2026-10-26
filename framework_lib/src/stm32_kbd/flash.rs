//! Bootloader sessions: flash, erase, identify and reset the controller
//!
//! Every step is either repeatable as it stands (polls, reads, ENTER, EXIT)
//! or checkable afterwards (writes). When a write's outcome cannot be
//! confirmed, the page it belongs to is done again: ENTER - which power-cycles
//! the part into a fresh ROM bootloader whatever state it was in - then erase
//! that one page, rewrite it and check it.

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use super::bootloader::{self, Bootloader};
use super::deck::{fw_version, FwVersion};
use super::hex::*;
use super::{sleep_ms, ErrorKind, Stm32KbdError, Stm32KbdResult};
use crate::chromium_ec::command::EcRequestRaw;
use crate::chromium_ec::commands::{EcRequestKbdFlashStm32, KbdFlashStm32Subcmd};
use crate::chromium_ec::{CrosEc, EcError, EcResponseStatus};

/// Locally a bootloader that does not answer after a second try is not there
const ENTER_ATTEMPTS: u32 = 2;
/// Writes of one page before giving up
const PAGE_ATTEMPTS: u32 = 5;
/// Rewrites of a page that read back wrong
const HEAL_ATTEMPTS: u32 = 2;
/// Bootloader restarts in one run
const RUN_RESTARTS: u32 = 300;
/// Tries at one chunk's write or read
const CHUNK_ATTEMPTS: u32 = 8;

/// How much one bootloader command can move through a host command.
/// The EC takes 248 byte requests and responses (a 256-byte LPC packet less
/// its header). A write is one I2C frame - N, the data, a checksum - behind
/// the 6 byte passthrough header; a read comes back behind a 2 byte one.
/// Kept a multiple of 8, because STM32U0 flash programs in ECC'd doublewords.
const WRITE_CHUNK: usize = 240;
const READ_CHUNK: usize = 240;

const SLOT_ADDR: u32 = MISC_FLAGS_BASE + MISC_SLOT_NO_AUTO_UPDATE * MISC_SLOT_SIZE;
const SLOT_ZEROS: [u8; MISC_SLOT_SIZE as usize] = [0; MISC_SLOT_SIZE as usize];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    EnteringBootloader,
    Erasing,
    Writing,
    Verifying,
    ExitingBootloader,
}

impl core::fmt::Display for Stage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let s = match self {
            Stage::EnteringBootloader => "Entering bootloader",
            Stage::Erasing => "Erasing",
            Stage::Writing => "Writing",
            Stage::Verifying => "Verifying",
            Stage::ExitingBootloader => "Exiting bootloader",
        };
        write!(f, "{}", s)
    }
}

/// Called during long operations with the stage and how many bytes of how
/// many are done. Both are 0 for stages without progress.
/// Return false to cancel.
pub type Progress<'a> = &'a mut dyn FnMut(Stage, usize, usize) -> bool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashOptions {
    /// Read the whole image back and compare
    pub verify: bool,
    /// Set the update protect flag (misc-flags slot 0) after writing.
    /// Automatic updaters are expected to leave the deck alone then.
    pub set_no_update: bool,
}

impl Default for FlashOptions {
    fn default() -> Self {
        Self {
            verify: true,
            set_no_update: false,
        }
    }
}

/// What the ROM bootloader reported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootloaderInfo {
    pub pid: u16,
    /// BCD
    pub bl_version: u8,
}

impl BootloaderInfo {
    pub fn part_name(&self) -> Option<&'static str> {
        part_name(self.pid)
    }
}

/// "STM32U073/U083" for a known chip ID
pub fn part_name(pid: u16) -> Option<&'static str> {
    match pid {
        0x0489 => Some("STM32U073/U083"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlashReport {
    pub bootloader: BootloaderInfo,
    /// Bootloader restarts after an operation failed
    pub restarts: u32,
    /// Pages rewritten because they read back wrong
    pub healed: u32,
}

fn cancelled() -> Stm32KbdError {
    Stm32KbdError::new(ErrorKind::Cancelled, "cancelled".to_string())
}

/// ENTER or EXIT. On an EC from before the flashing code moved to the host,
/// ENTER returns 5 bytes with the bootloader identity. We don't need them.
fn kbd_flash_stm32(ec: &CrosEc, subcmd: KbdFlashStm32Subcmd) -> Stm32KbdResult<()> {
    EcRequestKbdFlashStm32 {
        subcmd: subcmd as u8,
    }
    .send_command_vec(ec)
    .map(|_| ())
    .map_err(|e| match e {
        EcError::Response(EcResponseStatus::InvalidCommand) => Stm32KbdError::new(
            ErrorKind::Other,
            "the EC firmware does not support EC_CMD_KBD_FLASH_STM32".to_string(),
        ),
        e => e.into(),
    })
}

struct Session<'a> {
    ec: &'a CrosEc,
    bl: Bootloader<'a>,
    restarts: u32,
    healed: u32,
}

impl<'a> Session<'a> {
    /// ENTER: the EC drives BOOT0 and the rails and resets the STM32 into ROM.
    /// From there on the bootloader is ours.
    fn enter(ec: &'a CrosEc) -> Stm32KbdResult<Self> {
        let mut last_err = None;
        for attempt in 0..ENTER_ATTEMPTS {
            // Refused outright: retrying will not help
            kbd_flash_stm32(ec, KbdFlashStm32Subcmd::Enter)?;
            match Bootloader::init(ec) {
                Ok(bl) => {
                    return Ok(Self {
                        ec,
                        bl,
                        restarts: attempt,
                        healed: 0,
                    })
                }
                Err(e) => last_err = Some(e),
            }
        }
        let mut e = last_err.unwrap();
        e.kind = ErrorKind::Comms;
        Err(e)
    }

    fn info(&self) -> BootloaderInfo {
        BootloaderInfo {
            pid: self.bl.pid,
            bl_version: self.bl.bl_version,
        }
    }

    /// Start the bootloader over, from whatever state it was left in. ENTER
    /// power-cycles the part into ROM, so this is certain where nothing else
    /// is. The handshake is not repeated, what it reported cannot have changed.
    fn restart(&mut self) -> Stm32KbdResult<()> {
        let mut last_err = None;
        loop {
            self.restarts += 1;
            if self.restarts > RUN_RESTARTS {
                break;
            }
            if let Err(e) = kbd_flash_stm32(self.ec, KbdFlashStm32Subcmd::Enter) {
                last_err = Some(e);
                break;
            }
            match bootloader::wait_ready(self.ec) {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
        Err(Stm32KbdError::new(
            ErrorKind::Comms,
            format!(
                "Gave up after {} restarts: {}",
                self.restarts,
                last_err.map(|e| e.message).unwrap_or_default()
            ),
        ))
    }
}

struct Range<'a> {
    addr: u32,
    bytes: &'a [u8],
    /// The update protect slot rather than part of the image
    is_slot: bool,
}

/// What the flash should hold once written, as up to two ranges per page:
/// the image's part of it, and the update protect slot when it is being set.
/// The slot is SET by programming its doubleword to zero. The erase left it
/// all ones, so not writing it is how it gets cleared.
struct Plan<'a> {
    img: &'a HexImage,
    protect: bool,
    first_page: u32,
    last_page: u32,
    /// Bytes to write, for progress
    total: usize,
}

fn page_of(addr: u32) -> u32 {
    (addr - FLASH_BASE) / PAGE_SIZE
}

impl<'a> Plan<'a> {
    fn new(img: &'a HexImage, protect: bool) -> Self {
        let end = img.end();
        let mut first_page = page_of(img.base);
        let last_page = page_of(if protect && SLOT_ADDR > end {
            SLOT_ADDR
        } else {
            end
        });
        if protect && page_of(SLOT_ADDR) < first_page {
            first_page = page_of(SLOT_ADDR);
        }
        Self {
            img,
            protect,
            first_page,
            last_page,
            total: img.bytes.len() + if protect { SLOT_ZEROS.len() } else { 0 },
        }
    }

    fn page_ranges(&self, page: u32) -> Vec<Range<'a>> {
        let lo = FLASH_BASE + page * PAGE_SIZE;
        let hi = lo + PAGE_SIZE;
        let ib = self.img.base;
        let ie = ib + self.img.bytes.len() as u32;
        let mut ranges = Vec::with_capacity(2);

        if ib < hi && ie > lo {
            let a = core::cmp::max(ib, lo);
            let b = core::cmp::min(ie, hi);
            ranges.push(Range {
                addr: a,
                bytes: &self.img.bytes[(a - ib) as usize..(b - ib) as usize],
                is_slot: false,
            });
        }
        if self.protect && (lo..hi).contains(&SLOT_ADDR) {
            ranges.push(Range {
                addr: SLOT_ADDR,
                bytes: &SLOT_ZEROS,
                is_slot: true,
            });
        }
        ranges
    }
}

enum ChunkState {
    /// Holds the data
    Matches,
    /// Still erased
    Erased,
    /// Something else: a write half done, or not ours
    Other,
}

fn chunk_state(s: &Session, addr: u32, data: &[u8]) -> Stm32KbdResult<ChunkState> {
    let mut same = true;
    let mut erased = true;
    for (i, chunk) in data.chunks(READ_CHUNK).enumerate() {
        let buf = s.bl.read(addr + (i * READ_CHUNK) as u32, chunk.len())?;
        same &= buf == chunk;
        erased &= buf.iter().all(|b| *b == 0xFF);
    }
    Ok(if same {
        ChunkState::Matches
    } else if erased {
        ChunkState::Erased
    } else {
        ChunkState::Other
    })
}

/// Write one chunk and be sure of it. When the verdict did not come back,
/// the flash is the only witness, so read it: there already, done; still
/// erased, write it again; anything else, the page has to be redone.
/// Returns whether the page needs to be redone.
fn write_chunk(s: &mut Session, addr: u32, data: &[u8]) -> Stm32KbdResult<bool> {
    if s.bl.write(addr, data).is_ok() {
        return Ok(false);
    }
    for _ in 0..CHUNK_ATTEMPTS {
        match chunk_state(s, addr, data) {
            Ok(ChunkState::Matches) => return Ok(false),
            Ok(ChunkState::Erased) => {
                if s.bl.write(addr, data).is_ok() {
                    return Ok(false);
                }
            }
            Ok(ChunkState::Other) => return Ok(true),
            // Could not even read: start the bootloader over
            Err(_) => s.restart()?,
        }
    }
    Ok(true)
}

/// Write a page's ranges. Returns whether the page needs to be redone.
fn write_ranges(
    s: &mut Session,
    ranges: &[Range],
    done: &mut usize,
    total: usize,
    progress: Progress,
) -> Stm32KbdResult<bool> {
    for r in ranges {
        for (i, chunk) in r.bytes.chunks(WRITE_CHUNK).enumerate() {
            if write_chunk(s, r.addr + (i * WRITE_CHUNK) as u32, chunk)? {
                return Ok(true);
            }
            *done += chunk.len();
            if !progress(Stage::Writing, core::cmp::min(*done, total), total) {
                return Err(cancelled());
            }
        }
    }
    Ok(false)
}

/// Write one page, redoing it until it goes through: a restart, an erase of
/// just this page and a rewrite each time. fresh says the page is known to be
/// erased already, as it is after the mass erase.
fn write_page(
    s: &mut Session,
    plan: &Plan,
    page: u32,
    fresh: bool,
    done: &mut usize,
    progress: Progress,
) -> Stm32KbdResult<()> {
    let ranges = plan.page_ranges(page);
    if ranges.is_empty() {
        return Ok(());
    }
    let start = *done;
    let mut last_err = None;
    for attempt in 0..PAGE_ATTEMPTS {
        *done = start;
        if !fresh || attempt > 0 {
            if attempt > 0 {
                s.restart()?;
            }
            if let Err(e) = s.bl.erase_page(page as u16) {
                last_err = Some(e);
                continue;
            }
        }
        if !write_ranges(s, &ranges, done, plan.total, progress)? {
            return Ok(());
        }
    }
    Err(last_err.unwrap_or_else(|| {
        Stm32KbdError::new(
            ErrorKind::Comms,
            format!(
                "page at 0x{:08X} did not take after {} attempts",
                FLASH_BASE + page * PAGE_SIZE,
                PAGE_ATTEMPTS
            ),
        )
    }))
}

/// Read a page's ranges back and compare. Returns the first address that
/// does not match. A read is safe to repeat, so a failed one just goes again.
fn check_page(s: &mut Session, plan: &Plan, page: u32) -> Stm32KbdResult<Option<u32>> {
    for r in plan.page_ranges(page) {
        for (i, chunk) in r.bytes.chunks(READ_CHUNK).enumerate() {
            let a = r.addr + (i * READ_CHUNK) as u32;
            let mut attempt = 0;
            let buf = loop {
                match s.bl.read(a, chunk.len()) {
                    Ok(buf) => break buf,
                    Err(e) => {
                        attempt += 1;
                        if attempt >= CHUNK_ATTEMPTS {
                            return Err(e);
                        }
                        s.restart()?;
                    }
                }
            };
            if let Some(k) = buf.iter().zip(chunk).position(|(x, y)| x != y) {
                return Ok(Some(a + k as u32));
            }
        }
    }
    Ok(None)
}

fn flash_session(
    s: &mut Session,
    img: &HexImage,
    opts: &FlashOptions,
    progress: Progress,
) -> Stm32KbdResult<()> {
    let plan = Plan::new(img, opts.set_no_update);

    progress(Stage::Erasing, 0, 0);
    mass_erase(s)?;

    let mut done = 0;
    for page in plan.first_page..=plan.last_page {
        write_page(s, &plan, page, true, &mut done, progress)?;
    }

    // Read back - all of it with verify, and the update protect slot always:
    // a half-written slot reads as clear, so a silent failure there would
    // leave the deck unprotected while we claimed otherwise. A page that
    // reads back wrong is rewritten a couple of times before that counts
    // as a real mismatch.
    let mut checked = 0;
    for page in plan.first_page..=plan.last_page {
        let ranges = plan.page_ranges(page);
        let has_slot = ranges.iter().any(|r| r.is_slot);
        if ranges.is_empty() || (!opts.verify && !has_slot) {
            continue;
        }
        let mut heal = 0;
        while let Some(at) = check_page(s, &plan, page)? {
            // One wrong read proves nothing, only a second one agreeing does
            if check_page(s, &plan, page)?.is_none() {
                break;
            }
            if heal >= HEAL_ATTEMPTS {
                let msg = if (MISC_FLAGS_BASE..STORAGE_BASE).contains(&at) {
                    format!("update protection did not take at 0x{:08X}", at)
                } else {
                    format!("verify mismatch at 0x{:08X}", at)
                };
                return Err(Stm32KbdError::new(ErrorKind::Verify, msg));
            }
            heal += 1;
            s.healed += 1;
            s.restart()?;
            write_page(s, &plan, page, false, &mut 0, &mut |_, _, _| true)?;
        }
        if opts.verify {
            checked += ranges.iter().map(|r| r.bytes.len()).sum::<usize>();
            if !progress(
                Stage::Verifying,
                core::cmp::min(checked, plan.total),
                plan.total,
            ) {
                return Err(cancelled());
            }
        }
    }
    Ok(())
}

fn mass_erase(s: &mut Session) -> Stm32KbdResult<()> {
    let mut attempt = 0;
    loop {
        match s.bl.mass_erase() {
            Ok(()) => return Ok(()),
            Err(e) => {
                attempt += 1;
                if attempt >= PAGE_ATTEMPTS {
                    return Err(e);
                }
                s.restart()?;
            }
        }
    }
}

/// Run f in a bootloader session and always leave the bootloader afterwards,
/// a deck left in ROM has no keyboard and no touchpad.
fn with_session<T>(
    ec: &CrosEc,
    progress: Progress,
    f: impl FnOnce(&mut Session, Progress) -> Stm32KbdResult<T>,
) -> Stm32KbdResult<T> {
    progress(Stage::EnteringBootloader, 0, 0);
    let res = Session::enter(ec).and_then(|mut s| f(&mut s, progress));
    match res {
        Ok(val) => {
            progress(Stage::ExitingBootloader, 0, 0);
            kbd_flash_stm32(ec, KbdFlashStm32Subcmd::Exit)?;
            Ok(val)
        }
        Err(e) => {
            let _ = kbd_flash_stm32(ec, KbdFlashStm32Subcmd::Exit);
            Err(e)
        }
    }
}

/// Erase the controller and write the image: Enter, mass erase, write, read
/// back, Exit. The mass erase also resets the settings (Fn Lock), because
/// they live in NVS on the same flash.
///
/// The deck drops out for the duration and re-enumerates afterwards, see
/// [`fw_version_after_reset`] to check what it runs now.
pub fn flash(
    ec: &CrosEc,
    img: &HexImage,
    opts: &FlashOptions,
    progress: Progress,
) -> Stm32KbdResult<FlashReport> {
    with_session(ec, progress, |s, progress| {
        flash_session(s, img, opts, progress)?;
        Ok(FlashReport {
            bootloader: s.info(),
            restarts: s.restarts,
            healed: s.healed,
        })
    })
}

/// Mass erase and stop there, leaving the controller blank.
///
/// The deck has no keyboard and no touchpad afterwards, until something
/// flashes it. That's recoverable: the ROM bootloader is in ROM and BOOT0 is
/// driven by the EC, so [`flash`] reaches a blank part just as a working one.
pub fn erase(ec: &CrosEc, progress: Progress) -> Stm32KbdResult<BootloaderInfo> {
    with_session(ec, progress, |s, progress| {
        progress(Stage::Erasing, 0, 0);
        mass_erase(s)?;
        Ok(s.info())
    })
}

/// Read the chip ID from the ROM bootloader.
///
/// The shipped firmware reports chip_id as 0, the ROM gets it right. But
/// entering and leaving the bootloader power-cycles the deck: keyboard and
/// touchpad go away for about a second, up to ten if the EC's HID re-init
/// misses its first attempt. Never call this routinely.
pub fn identify(ec: &CrosEc) -> Stm32KbdResult<BootloaderInfo> {
    let res = Session::enter(ec).map(|s| s.info());
    let exit = kbd_flash_stm32(ec, KbdFlashStm32Subcmd::Exit);
    match (res, exit) {
        (Ok(info), Ok(())) => Ok(info),
        (Ok(_), Err(e)) => Err(Stm32KbdError::new(
            e.kind,
            format!(
                "read the controller ID but failed to leave the bootloader: {}",
                e.message
            ),
        )),
        (Err(e), _) => Err(e),
    }
}

/// Restart the controller into its application: BOOT0 low, power off, power on.
/// Keyboard and touchpad drop out for around a second.
pub fn reset(ec: &CrosEc) -> Stm32KbdResult<()> {
    kbd_flash_stm32(ec, KbdFlashStm32Subcmd::Exit)
}

/// Wait for the application to come up after leaving the bootloader and read
/// its version. The EC re-inits HID only 500ms after the power-cycle.
pub fn fw_version_after_reset(ec: &CrosEc) -> Stm32KbdResult<FwVersion> {
    sleep_ms(1200);
    let mut res = fw_version(ec);
    for _ in 1..4 {
        if res.is_ok() {
            break;
        }
        sleep_ms(500);
        res = fw_version(ec);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(base: u32, len: usize) -> HexImage {
        HexImage {
            base,
            bytes: (0..len).map(|i| i as u8).collect(),
        }
    }

    #[test]
    fn plan_without_protect() {
        let img = image(FLASH_BASE, PAGE_SIZE as usize + 10);
        let plan = Plan::new(&img, false);
        assert_eq!((plan.first_page, plan.last_page), (0, 1));
        assert_eq!(plan.total, img.bytes.len());

        let r = plan.page_ranges(1);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].addr, FLASH_BASE + PAGE_SIZE);
        assert_eq!(r[0].bytes, &img.bytes[PAGE_SIZE as usize..]);
        assert!(plan.page_ranges(2).is_empty());
    }

    #[test]
    fn plan_with_protect_extends_to_slot_page() {
        let img = image(FLASH_BASE, 100);
        let plan = Plan::new(&img, true);
        let slot_page = page_of(SLOT_ADDR);
        assert_eq!(slot_page, 27);
        assert_eq!((plan.first_page, plan.last_page), (0, slot_page));
        assert_eq!(plan.total, 108);

        // Pages in between have nothing to write
        assert!(plan.page_ranges(1).is_empty());
        let r = plan.page_ranges(slot_page);
        assert_eq!(r.len(), 1);
        assert!(r[0].is_slot);
        assert_eq!(r[0].addr, 0x0800_DFC0);
        assert_eq!(r[0].bytes, &[0; 8]);
    }

    #[test]
    fn plan_image_and_slot_share_a_page() {
        let img = image(0x0800_D800, 0x700);
        let plan = Plan::new(&img, true);
        assert_eq!((plan.first_page, plan.last_page), (27, 27));
        let r = plan.page_ranges(27);
        assert_eq!(r.len(), 2);
        assert!(!r[0].is_slot);
        assert_eq!(r[0].bytes.len(), 0x700);
        assert!(r[1].is_slot);
    }

    #[test]
    fn plan_image_not_at_flash_base() {
        let img = image(FLASH_BASE + 3 * PAGE_SIZE + 0x10, 0x20);
        let plan = Plan::new(&img, false);
        assert_eq!((plan.first_page, plan.last_page), (3, 3));
        assert_eq!(plan.page_ranges(3)[0].addr, img.base);
        assert!(plan.page_ranges(2).is_empty());
    }
}
