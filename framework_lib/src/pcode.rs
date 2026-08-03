//! Read platform power configuration out of the pcode mailbox
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
//! Only Intel processors have this mailbox.
//!
//! References:
//! - Panther Lake reference code, `Library/PeiDxeSmmCpuMailboxLib`,
//!   `Include/Register/B2pMailbox.h` and `PeiVrLib.c`
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

/// Data register of the BIOS to pcode mailbox, an MCHBAR offset
const PCODE_MAILBOX_DATA: u64 = 0x5DA0;
/// Interface register of the BIOS to pcode mailbox, relative to the data one
const PCODE_MAILBOX_INTERFACE: usize = 4;
/// Set by the caller to hand a command over, cleared by pcode when it's done
const MAILBOX_RUN_BUSY: u32 = 1 << 31;
/// How long to wait for pcode, in microseconds
///
/// The same timeout that coreboot and the reference code use.
const MAILBOX_TIMEOUT_US: u32 = 1000;

/// Read and write the SVID voltage regulator configuration
const MAILBOX_CMD_SVID_VR_HANDLER: u32 = 0x18;
/// Subcommand of [MAILBOX_CMD_SVID_VR_HANDLER] to read the PSYS full scale power
const MAILBOX_SUBCMD_GET_PMON_PMAX: u32 = 0x0A;

/// Completion code that pcode leaves in the interface register when it's done
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

/// The two mailbox registers, mapped for 32 bit access
struct Mailbox {
    map: imp::Mapping,
}

impl Mailbox {
    fn open() -> Option<Self> {
        let mchbar = imp::host_bridge_read64(HOST_BRIDGE_MCHBAR)?;
        if mchbar & MCHBAR_ENABLE == 0 {
            info!("MCHBAR is not enabled ({:#X}), cannot reach pcode", mchbar);
            return None;
        }
        let base = (mchbar & MCHBAR_ADDRESS_MASK) + PCODE_MAILBOX_DATA;
        debug!("MCHBAR {:#X}, pcode mailbox at {:#X}", mchbar, base);
        Some(Self {
            map: imp::Mapping::new(base, 8)?,
        })
    }

    /// Wait for pcode to release the mailbox
    fn poll_ready(&self) -> bool {
        for _ in 0..MAILBOX_TIMEOUT_US {
            if self.map.read32(PCODE_MAILBOX_INTERFACE) & MAILBOX_RUN_BUSY == 0 {
                return true;
            }
            os_specific::sleep(1);
        }
        false
    }
}

/// Run one pcode mailbox read command and return the data register
///
/// Read commands only need the interface register written, pcode puts the
/// result in the data register. That write is a doorbell though, so this is not
/// a passive read: it hands a command to pcode the same way firmware does.
/// Nothing in the OS uses this mailbox, but we still follow the firmware
/// protocol and wait for it to go idle before claiming it.
///
/// The reference code only runs mailbox commands on the bootstrap processor.
/// MCHBAR is package scoped, so it doesn't matter which core we happen to be
/// on. That rule is about serializing the users of the single mailbox.
fn mailbox_read(command: u32, param1: u32, param2: u32) -> Option<u32> {
    let mailbox = Mailbox::open()?;

    if !mailbox.poll_ready() {
        error!("pcode mailbox is busy");
        return None;
    }

    let interface = (command & 0xFF)
        | ((param1 & 0xFF) << 8)
        | ((param2 & 0x1FFF) << 16)
        | MAILBOX_RUN_BUSY;
    debug!("pcode mailbox command {:#010X}", interface);
    mailbox.map.write32(PCODE_MAILBOX_INTERFACE, interface);

    if !mailbox.poll_ready() {
        error!("pcode mailbox command {:#010X} did not complete", interface);
        return None;
    }

    // pcode replaces the command field with the completion code
    let code = mailbox.map.read32(PCODE_MAILBOX_INTERFACE) & 0xFF;
    if code != 0 {
        info!(
            "pcode rejected mailbox command {:#010X}: {} ({:#X})",
            interface,
            completion_code(code),
            code
        );
        return None;
    }

    let data = mailbox.map.read32(0);
    debug!("pcode mailbox data {:#010X}", data);
    Some(data)
}

/// Decode the U10.6 fixed point Watts of the PMON PMAX mailbox data
///
/// 10 integer and 6 fractional bits, so up to 1024 W in 1/64 W steps.
fn pmon_pmax_watts(data: u32) -> f32 {
    (data & 0xFFFF) as f32 / 64.0
}

/// PSYS full scale power in Watts, as programmed into pcode by firmware
///
/// This is the platform power that pcode takes a full scale PSYS reading to
/// mean, the top of the scale for every PSys measurement and limit. Not the AC
/// adapter rating, see the module documentation.
///
/// `None` if we can't read it, `Some(0.0)` if pcode has none programmed.
pub fn psys_pmax() -> Option<f32> {
    mailbox_read(
        MAILBOX_CMD_SVID_VR_HANDLER,
        MAILBOX_SUBCMD_GET_PMON_PMAX,
        0,
    )
    .map(pmon_pmax_watts)
}

/// Why we can't reach the pcode mailbox on this platform, for logging
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
            let page_len = ((offset + len) as u64).div_ceil(page_size) * page_size;

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
                    page_len as usize,
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
                page_len: page_len as usize,
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
        "Must be root to read the pcode mailbox"
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
        "Cannot reach the pcode mailbox"
    }
}

// Windows needs a kernel driver for MMIO and we don't ship one. FreeBSD has no
// sysfs to get MCHBAR from. Also covers non-x86, which has no pcode at all.
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
        "Reading the pcode mailbox is not supported on this platform"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pmon_pmax_watts() {
        // What coreboot's psys_pmax_watts = 218 ends up as: 218 W in 1/8 W
        // units through the FSP, then U10.6 fixed point in the mailbox
        assert_eq!(pmon_pmax_watts(218 * 64), 218.0);
        // The 350 W that every Panther Lake PPM profile defaults to
        assert_eq!(pmon_pmax_watts(2800 * 8), 350.0);
        // Fractional and unprogrammed
        assert_eq!(pmon_pmax_watts(0x0020), 0.5);
        assert_eq!(pmon_pmax_watts(0), 0.0);
        // Reserved upper bits are ignored, the field is 16 bits
        assert_eq!(pmon_pmax_watts(0xFFFF_0000 | (100 * 64)), 100.0);
    }
}
