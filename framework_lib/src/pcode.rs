//! Read platform power delivery configuration out of MCHBAR and pcode
//!
//! Some power management settings live neither in an MSR nor in a normal
//! register, but only inside pcode, which firmware configures through the BIOS
//! to pcode mailbox in MCHBAR. The interesting one for us is PMON PMAX, the
//! platform power that a full scale reading of the SoC's PSYS input stands for.
//!
//! The charger drives PSYS proportionally to total platform power (adapter plus
//! battery) and pcode maps that signal linearly onto PMAX. So PMAX is not the
//! AC adapter rating, even though the two are easily confused: it follows from
//! the charger's PSYS gain and the board's PSYS resistor. Get it wrong and
//! every platform power reading and every PSys power limit is off by the same
//! factor, in the direction that lets the platform draw more than the adapter
//! can supply until the charger asserts PROCHOT.
//!
//! coreboot programs it from the `psys_pmax_watts` devicetree register via the
//! FSP-M `PsysPmax` UPD. Left at 0, the FSP falls back to the value in the
//! SKU's PPM profile, which is 350 W for every Panther Lake profile and 0
//! (keep whatever pcode defaults to) for Wildcat Lake. Reading it back is the
//! only way to tell which of those actually took effect.
//!
//! The PSys power limits themselves are in MSR_PLATFORM_POWER_LIMIT, see
//! [crate::msr], and so is PL4. What's left over and only reachable here is the
//! rest of the PSYS calibration (offset and slope), the maximum system voltage,
//! and the Isys battery current limits.
//!
//! Only Intel processors have any of this.
//!
//! References:
//! - Panther Lake reference code, `Library/PeiDxeSmmCpuMailboxLib`,
//!   `Include/Register/B2pMailbox.h`, `PeiVrLib.c` and `PowerLimits.c`
//! - coreboot `src/soc/intel/pantherlake/systemagent.c`

use crate::os_specific;

/// MCHBAR, the base address register of the host bridge's MMIO range
const HOST_BRIDGE_MCHBAR: u32 = 0x48;
/// Bit 0 of MCHBAR, set while the range decodes
const MCHBAR_ENABLE: u64 = 1;
/// Address bits of MCHBAR
///
/// The range is 32 KB on Skylake and 128 KB since Tiger Lake, so the low 15
/// bits are reserved (and read as zero) on every processor we support.
const MCHBAR_ADDRESS_MASK: u64 = 0x0000_007F_FFFF_8000;

/// The MCHBAR page we map, holding every register below
const WINDOW_BASE: u64 = 0x5000;
const WINDOW_LEN: usize = 0x1000;

/// Data register of the BIOS to pcode mailbox
const PCODE_MAILBOX_DATA: u64 = 0x5DA0;
/// Interface register of the BIOS to pcode mailbox
const PCODE_MAILBOX_INTERFACE: u64 = 0x5DA4;
/// Isys (battery) current limits, the ThETA Ibatt feature. 64 bit.
const ISYS_CONTROL: u64 = 0x5E90;

/// Set by the caller to hand a command over, cleared by pcode when it's done
const MAILBOX_RUN_BUSY: u32 = 1 << 31;
/// How long to wait for pcode, in microseconds
///
/// The same timeout that coreboot and the reference code use.
const MAILBOX_TIMEOUT_US: u32 = 1000;

/// Read and write the SVID voltage regulator configuration
const MAILBOX_CMD_SVID_VR_HANDLER: u32 = 0x18;
/// Read the PSYS offset and slope correction
const MAILBOX_SUBCMD_GET_PMON_CONFIG: u32 = 0x19;
/// Read the PSYS full scale power
const MAILBOX_SUBCMD_GET_PMON_PMAX: u32 = 0x0A;
/// Read the maximum system voltage
const MAILBOX_SUBCMD_GET_VSYS_MAX: u32 = 0x28;

/// What pcode leaves in the command field of the interface register when done
fn completion_code(code: u32) -> &'static str {
    match code {
        0x0 => "Success",
        0x1 => "Illegal command",
        0x2 => "Timeout",
        0x3 => "Illegal data",
        0x5 => "Illegal VR ID",
        0x6 => "Locked",
        0x7 => "VR error",
        0x8 => "Illegal subcommand",
        _ => "Unknown",
    }
}

// -------------------------------------------------------------------------
// Decoding
// -------------------------------------------------------------------------

/// Decode unsigned fixed point with `fraction_bits` fractional bits
///
/// The mailbox describes its fields as U16.x.y, so U10.6 is 10 integer and 6
/// fractional bits out of the 16 bit field.
fn unsigned_fixed_point(field: u32, fraction_bits: u32) -> f32 {
    (field & 0xFFFF) as f32 / (1u32 << fraction_bits) as f32
}

/// Decode signed (2's complement) fixed point with `fraction_bits` fractional bits
fn signed_fixed_point(field: u32, fraction_bits: u32) -> f32 {
    ((field & 0xFFFF) as u16) as i16 as f32 / (1u32 << fraction_bits) as f32
}

/// The PSYS calibration that pcode applies to the charger's PSYS signal
#[derive(Debug, Clone, Copy)]
pub struct PsysConfig {
    /// Platform power at full scale PSYS, in Watts. 0 if pcode has none.
    ///
    /// PMON PMAX, U10.6 fixed point, so up to 1024 W in 1/64 W steps.
    pub pmax: f32,
    /// Offset and slope correction of the PSYS reading
    ///
    /// `None` if pcode wouldn't tell us. Note that these read back as pcode's
    /// own defaults (no offset, unity slope) rather than as unprogrammed when
    /// firmware leaves both at Auto, in which case the FSP skips the write
    /// altogether.
    pub correction: Option<PsysCorrection>,
}

/// The offset and slope pcode applies to the PSYS reading before scaling it
#[derive(Debug, Clone, Copy)]
pub struct PsysCorrection {
    /// Offset correction in Watts, S7.8 fixed point
    pub offset: f32,
    /// Slope correction, 1.0 being none, U1.15 fixed point
    pub slope: f32,
}

/// The Isys (battery) current limits of the ThETA Ibatt feature
///
/// Two levels, each with a current limit in Amps, only the first one with a
/// time window. Firmware only programs these while on battery, so they read
/// back disabled on AC.
#[derive(Debug, Clone, Copy)]
pub struct IsysControl {
    /// Level 1 limit in Amps, the field is in 1/8 A
    pub l1_amps: f32,
    pub l1_enabled: bool,
    /// Raw 7 bit time window field of the level 1 limit
    ///
    /// Encoded like a RAPL time window, so decode it with the time unit from
    /// MSR_PACKAGE_POWER_SKU_UNIT.
    pub l1_tau: u64,
    /// Level 2 limit in Amps, the field is in 1/8 A
    pub l2_amps: f32,
    pub l2_enabled: bool,
}

/// Platform power delivery configuration, as firmware left it
#[derive(Debug, Clone, Copy)]
pub struct PlatformPower {
    pub psys: Option<PsysConfig>,
    /// Maximum system voltage in Volts, U10.6 fixed point
    ///
    /// Only programmed together with [IsysControl].
    pub vsys_max: Option<f32>,
    pub isys: Option<IsysControl>,
}

// -------------------------------------------------------------------------
// MCHBAR and the pcode mailbox
// -------------------------------------------------------------------------

/// A mapping of the MCHBAR page holding the registers we're after
struct Mchbar {
    map: imp::Mapping,
}

impl Mchbar {
    fn open() -> Option<Self> {
        let mchbar = imp::host_bridge_read64(HOST_BRIDGE_MCHBAR)?;
        if mchbar & MCHBAR_ENABLE == 0 {
            info!("MCHBAR is not enabled ({:#X}), cannot reach pcode", mchbar);
            return None;
        }
        let base = (mchbar & MCHBAR_ADDRESS_MASK) + WINDOW_BASE;
        debug!("MCHBAR {:#X}, mapping {:#X}", mchbar, base);
        Some(Self {
            map: imp::Mapping::new(base, WINDOW_LEN)?,
        })
    }

    fn read32(&self, offset: u64) -> u32 {
        self.map.read32((offset - WINDOW_BASE) as usize)
    }

    fn write32(&self, offset: u64, value: u32) {
        self.map.write32((offset - WINDOW_BASE) as usize, value)
    }

    /// Read a 64 bit register as its two halves
    ///
    /// Everything we read this way is static configuration, so it doesn't
    /// matter that the two halves aren't sampled at the same instant.
    fn read64(&self, offset: u64) -> u64 {
        ((self.read32(offset + 4) as u64) << 32) | (self.read32(offset) as u64)
    }

    /// Wait for pcode to release the mailbox
    fn poll_mailbox_ready(&self) -> bool {
        for _ in 0..MAILBOX_TIMEOUT_US {
            if self.read32(PCODE_MAILBOX_INTERFACE) & MAILBOX_RUN_BUSY == 0 {
                return true;
            }
            os_specific::sleep(1);
        }
        false
    }

    /// Run one pcode mailbox read command and return the data register
    ///
    /// Read commands only need the interface register written, pcode puts the
    /// result in the data register. That write is a doorbell though, so this is
    /// not a passive read: it hands a command to pcode the same way firmware
    /// does. Nothing in the OS uses this mailbox, but we still follow the
    /// firmware protocol and wait for it to go idle before claiming it.
    ///
    /// The reference code only runs mailbox commands on the bootstrap
    /// processor. MCHBAR is package scoped, so it doesn't matter which core we
    /// happen to be on. That rule is about serializing users of the one mailbox.
    fn mailbox_read(&self, command: u32, param1: u32, param2: u32) -> Option<u32> {
        if !self.poll_mailbox_ready() {
            error!("pcode mailbox is busy");
            return None;
        }

        let interface = (command & 0xFF)
            | ((param1 & 0xFF) << 8)
            | ((param2 & 0x1FFF) << 16)
            | MAILBOX_RUN_BUSY;
        debug!("pcode mailbox command {:#010X}", interface);
        self.write32(PCODE_MAILBOX_INTERFACE, interface);

        if !self.poll_mailbox_ready() {
            error!("pcode mailbox command {:#010X} did not complete", interface);
            return None;
        }

        // pcode replaces the command field with the completion code
        let code = self.read32(PCODE_MAILBOX_INTERFACE) & 0xFF;
        if code != 0 {
            info!(
                "pcode rejected mailbox command {:#010X}: {} ({:#X})",
                interface,
                completion_code(code),
                code
            );
            return None;
        }

        let data = self.read32(PCODE_MAILBOX_DATA);
        debug!("pcode mailbox data {:#010X}", data);
        Some(data)
    }

    fn vr(&self, subcommand: u32) -> Option<u32> {
        self.mailbox_read(MAILBOX_CMD_SVID_VR_HANDLER, subcommand, 0)
    }
}

/// Read back the platform power delivery configuration firmware programmed
///
/// `None` if we can't reach MCHBAR at all, see [unavailable_hint]. The
/// individual values are `None` when pcode refused the command, which is how a
/// processor without that particular knob answers.
pub fn platform_power() -> Option<PlatformPower> {
    let mchbar = Mchbar::open()?;

    let psys = mchbar
        .vr(MAILBOX_SUBCMD_GET_PMON_PMAX)
        .map(|pmax| PsysConfig {
            pmax: unsigned_fixed_point(pmax, 6),
            // A separate command, and only interesting alongside a full scale, so
            // don't fail the whole thing over it
            correction: mchbar
                .vr(MAILBOX_SUBCMD_GET_PMON_CONFIG)
                .map(|config| PsysCorrection {
                    offset: signed_fixed_point(config, 8),
                    slope: unsigned_fixed_point(config >> 16, 15),
                }),
        });

    let vsys_max = mchbar
        .vr(MAILBOX_SUBCMD_GET_VSYS_MAX)
        .map(|data| unsigned_fixed_point(data, 6));

    let raw = mchbar.read64(ISYS_CONTROL);
    debug!("ISYS_CONTROL ({:#X}): {:#018X}", ISYS_CONTROL, raw);
    let isys = Some(IsysControl {
        l1_amps: (raw & 0x7FFF) as f32 / 8.0,
        l1_enabled: raw & (1 << 15) != 0,
        l1_tau: (raw >> 16) & 0x7F,
        l2_amps: ((raw >> 32) & 0x7FFF) as f32 / 8.0,
        l2_enabled: raw & (1 << 47) != 0,
    });

    Some(PlatformPower {
        psys,
        vsys_max,
        isys,
    })
}

/// Why we can't reach MCHBAR on this platform, for logging
pub fn unavailable_hint() -> &'static str {
    imp::unavailable_hint()
}

// Linux gives us the host bridge's config space through sysfs and physical
// memory through /dev/mem. Both need root.
#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
mod imp {
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Seek, SeekFrom};
    use std::os::unix::io::AsRawFd;
    use std::ptr;

    const HOST_BRIDGE_CONFIG: &str = "/sys/bus/pci/devices/0000:00:00.0/config";

    /// Read a 64 bit register of the host bridge's PCI configuration space
    ///
    /// Anything above the standard 64 byte header needs CAP_SYS_ADMIN, so this
    /// only works as root.
    pub fn host_bridge_read64(offset: u32) -> Option<u64> {
        let mut file = match File::open(HOST_BRIDGE_CONFIG) {
            Ok(file) => file,
            Err(err) => {
                info!("Cannot open {}: {}", HOST_BRIDGE_CONFIG, err);
                return None;
            }
        };
        let mut buf = [0u8; 8];
        if let Err(err) = file
            .seek(SeekFrom::Start(offset as u64))
            .and_then(|_| file.read_exact(&mut buf))
        {
            info!("Cannot read host bridge config {:#X}: {}", offset, err);
            return None;
        }
        Some(u64::from_le_bytes(buf))
    }

    /// A mapping of physical memory, for volatile 32 bit register access
    pub struct Mapping {
        page: *mut libc::c_void,
        page_len: usize,
        /// Where the requested address ended up inside the mapping
        offset: usize,
        len: usize,
    }

    impl Mapping {
        /// Map `len` bytes of physical memory starting at `phys`
        pub fn new(phys: u64, len: usize) -> Option<Self> {
            // SAFETY: sysconf() with a valid name has no preconditions
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
            let page_base = phys & !(page_size - 1);
            let offset = (phys - page_base) as usize;
            let page_len = (((offset + len) as u64).div_ceil(page_size) * page_size) as usize;

            let file = match OpenOptions::new().read(true).write(true).open("/dev/mem") {
                Ok(file) => file,
                Err(err) => {
                    info!("Cannot open /dev/mem: {}", err);
                    return None;
                }
            };

            // SAFETY: A null hint lets the kernel pick the address and the
            // length is a multiple of the page size. The mapping is only used
            // through self.reg(), which stays inside it.
            let page = unsafe {
                libc::mmap(
                    ptr::null_mut(),
                    page_len,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_SHARED,
                    file.as_raw_fd(),
                    page_base as libc::off_t,
                )
            };
            if page == libc::MAP_FAILED {
                error!(
                    "Cannot map {:#X} from /dev/mem: {}",
                    page_base,
                    std::io::Error::last_os_error()
                );
                return None;
            }

            Some(Self {
                page,
                page_len,
                offset,
                len,
            })
        }

        fn reg(&self, offset: usize) -> *mut u32 {
            assert!(offset + 4 <= self.len);
            // SAFETY: Asserted to be inside the mapping
            unsafe { (self.page as *mut u8).add(self.offset + offset) as *mut u32 }
        }

        pub fn read32(&self, offset: usize) -> u32 {
            // SAFETY: reg() returns an aligned pointer into the mapping
            unsafe { ptr::read_volatile(self.reg(offset)) }
        }

        pub fn write32(&self, offset: usize, value: u32) {
            // SAFETY: reg() returns an aligned pointer into the mapping
            unsafe { ptr::write_volatile(self.reg(offset), value) }
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            // SAFETY: Unmapping exactly what new() mapped
            unsafe {
                libc::munmap(self.page, self.page_len);
            }
        }
    }

    pub fn unavailable_hint() -> &'static str {
        "Must be root to read MCHBAR"
    }
}

#[cfg(all(target_os = "uefi", any(target_arch = "x86", target_arch = "x86_64")))]
mod imp {
    use core::arch::asm;
    use core::ptr;

    /// Address and data port of legacy PCI configuration space access
    const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
    const PCI_CONFIG_DATA: u16 = 0xCFC;

    /// Read a 32 bit register of the host bridge (bus 0, device 0, function 0)
    fn host_bridge_read32(offset: u32) -> u32 {
        // Bit 31 enables the configuration cycle, bus/device/function are 0
        let address = 0x8000_0000 | (offset & 0xFC);
        let value: u32;
        // SAFETY: Reading PCI configuration space has no side effects
        unsafe {
            asm!(
                "out dx, eax",
                in("dx") PCI_CONFIG_ADDRESS,
                in("eax") address,
                options(nomem, nostack, preserves_flags),
            );
            asm!(
                "in eax, dx",
                in("dx") PCI_CONFIG_DATA,
                out("eax") value,
                options(nomem, nostack, preserves_flags),
            );
        }
        value
    }

    pub fn host_bridge_read64(offset: u32) -> Option<u64> {
        let lo = host_bridge_read32(offset);
        let hi = host_bridge_read32(offset + 4);
        Some(((hi as u64) << 32) | (lo as u64))
    }

    /// Physical memory is identity mapped in UEFI, so there's nothing to map
    pub struct Mapping {
        base: u64,
        len: usize,
    }

    impl Mapping {
        pub fn new(phys: u64, len: usize) -> Option<Self> {
            Some(Self { base: phys, len })
        }

        fn reg(&self, offset: usize) -> *mut u32 {
            assert!(offset + 4 <= self.len);
            (self.base as usize + offset) as *mut u32
        }

        pub fn read32(&self, offset: usize) -> u32 {
            // SAFETY: MCHBAR is identity mapped MMIO, so the address is valid
            unsafe { ptr::read_volatile(self.reg(offset)) }
        }

        pub fn write32(&self, offset: usize, value: u32) {
            // SAFETY: MCHBAR is identity mapped MMIO, so the address is valid
            unsafe { ptr::write_volatile(self.reg(offset), value) }
        }
    }

    pub fn unavailable_hint() -> &'static str {
        "Cannot reach MCHBAR"
    }
}

// Windows needs a kernel driver for MMIO and we don't ship one. FreeBSD has no
// sysfs to get MCHBAR from. Also covers non-x86, which has no MCHBAR at all.
#[cfg(not(any(
    all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")),
    all(target_os = "uefi", any(target_arch = "x86", target_arch = "x86_64")),
)))]
mod imp {
    pub fn host_bridge_read64(_offset: u32) -> Option<u64> {
        None
    }

    pub struct Mapping;

    impl Mapping {
        pub fn new(_phys: u64, _len: usize) -> Option<Self> {
            None
        }

        pub fn read32(&self, _offset: usize) -> u32 {
            0
        }

        pub fn write32(&self, _offset: usize, _value: u32) {}
    }

    pub fn unavailable_hint() -> &'static str {
        "Reading MCHBAR is not supported on this platform"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pmon_pmax() {
        // What coreboot's psys_pmax_watts = 218 ends up as: 218 W in 1/8 W
        // units through the FSP, then U10.6 fixed point in the mailbox
        assert_eq!(unsigned_fixed_point(218 * 64, 6), 218.0);
        // The 350 W that every Panther Lake PPM profile defaults to
        assert_eq!(unsigned_fixed_point(2800 * 8, 6), 350.0);
        // Fractional and unprogrammed
        assert_eq!(unsigned_fixed_point(0x0020, 6), 0.5);
        assert_eq!(unsigned_fixed_point(0, 6), 0.0);
        // Reserved upper bits are ignored, the field is 16 bits
        assert_eq!(unsigned_fixed_point(0xFFFF_0000 | (100 * 64), 6), 100.0);
    }

    #[test]
    fn test_vsys_max() {
        // The 24000 mV that every Panther Lake PPM profile defaults to
        assert_eq!(unsigned_fixed_point(24000 * 64 / 1000, 6), 24.0);
    }

    #[test]
    fn test_pmon_config() {
        // U1.15 slope correction, the reference code's example of 125 (1.25)
        assert_eq!(unsigned_fixed_point(125 * 32768 / 100, 15), 1.25);
        assert_eq!(unsigned_fixed_point(0, 15), 0.0);
        // S7.8 offset, the reference code's example of 25348 (25.348 W), which
        // 8 fractional bits can only get within 1/256 of
        assert_eq!(signed_fixed_point(25348 * 256 / 1000, 8), 25.347656);
        assert_eq!(signed_fixed_point(0xFF00, 8), -1.0);
    }
}
