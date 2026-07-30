//! Read Intel thermal and performance limiting MSRs
//!
//! These tell us *why* the CPU is running slower than requested. Most
//! interesting on our systems is PROCHOT, which the EC asserts to throttle the
//! CPU, and the frequency clipping reasons, which record both the currently
//! active and the (sticky) previously seen limiting reasons.
//!
//! Only Intel processors are supported. AMD does not expose comparable
//! information through MSRs.
//!
//! References:
//! - Intel SDM Volume 4 (Model-Specific Registers)
//! - Panther Lake reference code, `Include/Register/Ptl/Msr/MsrRegs.h`
//! - Linux `arch/x86/include/asm/msr-index.h` and `tools/power/x86/turbostat`

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Per core thermal status (IA32_THERM_STATUS)
pub const MSR_IA32_THERM_STATUS: u32 = 0x0000019C;
/// Thermal monitor reference temperature and offsets (IA32_TEMPERATURE_TARGET)
pub const MSR_IA32_TEMPERATURE_TARGET: u32 = 0x000001A2;
/// Package thermal status (IA32_PACKAGE_THERM_STATUS)
pub const MSR_IA32_PACKAGE_THERM_STATUS: u32 = 0x000001B1;
/// Power control, contains the PROCHOT configuration (MSR_POWER_CTL)
pub const MSR_POWER_CTL: u32 = 0x000001FC;
/// Indicator of frequency clipping in the processor cores
///
/// Sandy Bridge to Broadwell use 0x690 instead. All Intel based Framework
/// systems are Tiger Lake or newer, so we only implement the modern address.
pub const MSR_IA_PERF_LIMIT_REASONS: u32 = 0x0000064F;
/// Indicator of frequency clipping in the integrated graphics
pub const MSR_GT_PERF_LIMIT_REASONS: u32 = 0x000006B0;
/// Indicator of frequency clipping in the ring interconnect (a.k.a. CLR)
pub const MSR_RING_PERF_LIMIT_REASONS: u32 = 0x000006B1;

/// Status bits of the *_PERF_LIMIT_REASONS MSRs
///
/// The matching sticky log bit is always 16 bits higher.
const PLR_CORE_BITS: &[(u32, &str)] = &[
    (0, "PROCHOT"),
    (1, "Thermal"),
    (4, "ResidencyStateRegulation"),
    (5, "RunningAvgThermalLimit"),
    (6, "VR-ThermalAlert"),
    (7, "VR-ThermalDesignCurrent"),
    (8, "Other"),
    (10, "PkgPwrPL1"),
    (11, "PkgPwrPL2"),
    (12, "MaxTurboLimit"),
    (13, "TurboTransitionAttenuation"),
];
const PLR_GT_BITS: &[(u32, &str)] = &[
    (0, "PROCHOT"),
    (1, "Thermal"),
    (4, "ResidencyStateRegulation"),
    (5, "RunningAvgThermalLimit"),
    (6, "VR-ThermalAlert"),
    (7, "VR-ThermalDesignCurrent"),
    (8, "Other"),
    (10, "PkgPwrPL1"),
    (11, "PkgPwrPL2"),
    (12, "InefficientOperation"),
];
const PLR_RING_BITS: &[(u32, &str)] = &[
    (0, "PROCHOT"),
    (1, "Thermal"),
    (4, "ResidencyStateRegulation"),
    (5, "RunningAvgThermalLimit"),
    (6, "VR-ThermalAlert"),
    (7, "VR-ThermalDesignCurrent"),
    (8, "Other"),
    (10, "PkgPwrPL1"),
    (11, "PkgPwrPL2"),
];

/// Status/log bit pairs shared by IA32_THERM_STATUS and
/// IA32_PACKAGE_THERM_STATUS. The log bit is always one bit higher.
const THERM_STATUS_BITS: &[(u32, &str)] = &[
    (0, "Thermal Monitor"),
    (2, "PROCHOT/FORCEPR"),
    (4, "Critical Temp"),
    (6, "Threshold #1"),
    (8, "Threshold #2"),
    (10, "Power Limit"),
];
/// Bits above 11 that only exist in the per core IA32_THERM_STATUS
const THERM_STATUS_CORE_BITS: &[(u32, &str)] = &[(12, "Current Limit"), (14, "Cross Domain Limit")];
/// Bits above 11 that only exist in IA32_PACKAGE_THERM_STATUS
const THERM_STATUS_PKG_BITS: &[(u32, &str)] = &[(12, "Pmax Limit")];

fn bit(value: u64, bit: u32) -> bool {
    (value >> bit) & 1 == 1
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}

/// Decode the set bits of a *_PERF_LIMIT_REASONS MSR
///
/// `shift` is 0 for the currently active reasons and 16 for the sticky log.
fn decode_reasons(value: u64, bits: &[(u32, &str)], shift: u32) -> String {
    let set: Vec<&str> = bits
        .iter()
        .filter(|(b, _)| bit(value, b + shift))
        .map(|(_, name)| *name)
        .collect();
    if set.is_empty() {
        "None".to_string()
    } else {
        set.join(", ")
    }
}

// -------------------------------------------------------------------------
// CPUID
// -------------------------------------------------------------------------

/// Vendor, family and model from CPUID
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuId {
    pub is_intel: bool,
    pub family: u32,
    pub model: u32,
    /// CPUID.06H:EAX[0] Digital Thermal Sensor
    pub has_dts: bool,
    /// CPUID.06H:EAX[6] Package Thermal Management
    pub has_ptm: bool,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
// __cpuid is safe since Rust 1.87, but our MSRV is 1.81
#[allow(unused_unsafe)]
pub fn cpuid() -> Option<CpuId> {
    #[cfg(target_arch = "x86")]
    use core::arch::x86::__cpuid;
    #[cfg(target_arch = "x86_64")]
    use core::arch::x86_64::__cpuid;

    // SAFETY: CPUID leaf 0 is available on every CPU we can be running on
    let leaf_0 = unsafe { __cpuid(0) };
    // "GenuineIntel" in EBX, EDX, ECX
    let is_intel = leaf_0.ebx == 0x756E6547 && leaf_0.edx == 0x49656E69 && leaf_0.ecx == 0x6C65746E;

    // SAFETY: Leaf 1 is available whenever leaf 0 reports at least 1
    let leaf_1 = unsafe { __cpuid(1) };
    let base_family = (leaf_1.eax >> 8) & 0xF;
    let base_model = (leaf_1.eax >> 4) & 0xF;
    let family = if base_family == 0xF {
        base_family + ((leaf_1.eax >> 20) & 0xFF)
    } else {
        base_family
    };
    let model = if base_family == 0x6 || base_family == 0xF {
        base_model + (((leaf_1.eax >> 16) & 0xF) << 4)
    } else {
        base_model
    };

    // Leaf 6 (Thermal and Power Management) tells us whether the thermal
    // status MSRs exist. Only query it if the CPU supports that leaf.
    let (has_dts, has_ptm) = if leaf_0.eax >= 6 {
        // SAFETY: Guarded by the maximum leaf reported above
        let leaf_6 = unsafe { __cpuid(6) };
        (leaf_6.eax & 1 == 1, (leaf_6.eax >> 6) & 1 == 1)
    } else {
        (false, false)
    };

    Some(CpuId {
        is_intel,
        family,
        model,
        has_dts,
        has_ptm,
    })
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub fn cpuid() -> Option<CpuId> {
    None
}

impl CpuId {
    /// Whether the *_PERF_LIMIT_REASONS MSRs at their modern addresses exist
    ///
    /// They are not architectural, so we only read them on Skylake (model
    /// 0x4E) and newer client processors. Reading a reserved MSR raises a
    /// general protection fault, which is fatal in UEFI.
    pub fn has_perf_limit_reasons(&self) -> bool {
        self.is_intel && self.family == 6 && self.model >= 0x4E
    }
}

// -------------------------------------------------------------------------
// MSR access, one implementation per OS
// -------------------------------------------------------------------------

#[cfg(all(target_os = "linux", any(target_arch = "x86", target_arch = "x86_64")))]
mod imp {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    pub fn read_msr(cpu: u32, msr: u32) -> Option<u64> {
        let path = format!("/dev/cpu/{}/msr", cpu);
        let mut file = File::open(&path)
            .map_err(|err| debug!("Failed to open {}: {:?}", path, err))
            .ok()?;
        file.seek(SeekFrom::Start(msr as u64))
            .map_err(|err| debug!("Failed to seek {} to {:#X}: {:?}", path, msr, err))
            .ok()?;
        let mut buf = [0u8; 8];
        file.read_exact(&mut buf)
            .map_err(|err| debug!("Failed to read MSR {:#X}: {:?}", msr, err))
            .ok()?;
        Some(u64::from_le_bytes(buf))
    }

    pub fn cpu_count() -> u32 {
        (0..256)
            .take_while(|cpu| File::open(format!("/dev/cpu/{}/msr", cpu)).is_ok())
            .count() as u32
    }

    pub fn unavailable_hint() -> &'static str {
        "Cannot access /dev/cpu/0/msr. Run as root and load the msr module (modprobe msr)"
    }
}

#[cfg(all(
    target_os = "freebsd",
    any(target_arch = "x86", target_arch = "x86_64")
))]
mod imp {
    use nix::ioctl_readwrite;
    use std::fs::File;
    use std::os::fd::AsRawFd;

    /// `cpuctl_msr_args_t` from `sys/sys/cpuctl.h`
    #[repr(C)]
    struct CpuctlMsrArgs {
        msr: i32,
        data: u64,
    }
    ioctl_readwrite!(cpuctl_rdmsr, b'c', 1, CpuctlMsrArgs);

    pub fn read_msr(cpu: u32, msr: u32) -> Option<u64> {
        let path = format!("/dev/cpuctl{}", cpu);
        let file = File::open(&path)
            .map_err(|err| debug!("Failed to open {}: {:?}", path, err))
            .ok()?;
        let mut args = CpuctlMsrArgs {
            msr: msr as i32,
            data: 0,
        };
        // SAFETY: args matches the struct the ioctl expects
        unsafe { cpuctl_rdmsr(file.as_raw_fd(), &mut args) }
            .map_err(|err| debug!("Failed to read MSR {:#X}: {:?}", msr, err))
            .ok()?;
        Some(args.data)
    }

    pub fn cpu_count() -> u32 {
        (0..256)
            .take_while(|cpu| File::open(format!("/dev/cpuctl{}", cpu)).is_ok())
            .count() as u32
    }

    pub fn unavailable_hint() -> &'static str {
        "Cannot access /dev/cpuctl0. Run as root and load the cpuctl module (kldload cpuctl)"
    }
}

#[cfg(all(target_os = "uefi", any(target_arch = "x86", target_arch = "x86_64")))]
mod imp {
    use core::arch::asm;

    /// Read an MSR on the processor we're currently executing on
    ///
    /// UEFI applications run single threaded on the bootstrap processor, so we
    /// can only ever read the BSP. Package scoped MSRs are unaffected by that.
    pub fn read_msr(_cpu: u32, msr: u32) -> Option<u64> {
        let lo: u32;
        let hi: u32;
        // SAFETY: Callers must make sure the MSR exists on this processor,
        // otherwise this raises a general protection fault
        unsafe {
            asm!(
                "rdmsr",
                in("ecx") msr,
                out("eax") lo,
                out("edx") hi,
                options(nomem, nostack, preserves_flags),
            );
        }
        Some(((hi as u64) << 32) | (lo as u64))
    }

    pub fn cpu_count() -> u32 {
        1
    }

    pub fn unavailable_hint() -> &'static str {
        "Cannot read MSRs"
    }
}

// Windows needs a kernel driver to access MSRs and we don't ship one.
// Also covers non-x86, where these MSRs don't exist at all.
#[cfg(not(all(
    any(target_os = "linux", target_os = "freebsd", target_os = "uefi"),
    any(target_arch = "x86", target_arch = "x86_64")
)))]
mod imp {
    pub fn read_msr(_cpu: u32, _msr: u32) -> Option<u64> {
        None
    }

    pub fn cpu_count() -> u32 {
        0
    }

    pub fn unavailable_hint() -> &'static str {
        "Reading MSRs is not supported on this platform"
    }
}

/// Read a 64bit MSR of a logical processor
pub fn read_msr(cpu: u32, msr: u32) -> Option<u64> {
    let value = imp::read_msr(cpu, msr);
    debug!("CPU {} MSR {:#X}: {:#018X?}", cpu, msr, value);
    value
}

/// Number of logical processors we can read MSRs of
pub fn cpu_count() -> u32 {
    imp::cpu_count()
}

// -------------------------------------------------------------------------
// Decoded structures
// -------------------------------------------------------------------------

/// IA32_TEMPERATURE_TARGET (0x1A2)
#[derive(Debug, Clone, Copy)]
pub struct TemperatureTarget {
    /// Lowest temperature at which PROCHOT# is asserted, in degrees C
    pub ref_temp: u8,
    /// Offset below ref_temp at which TCC activates, in degrees C
    pub tcc_offset: u8,
    /// Offset below ref_temp at which fans should be engaged (T-Control)
    pub fan_temp_offset: u8,
    /// Allow RATL throttling below P1
    pub tcc_offset_clamping: bool,
    /// The whole MSR is read-only
    pub locked: bool,
}

impl From<u64> for TemperatureTarget {
    fn from(msr: u64) -> Self {
        Self {
            ref_temp: ((msr >> 16) & 0xFF) as u8,
            // 7 bits [30:24] on Panther Lake, 6 bits since Skylake.
            // Offsets that large don't occur, so mask conservatively.
            tcc_offset: ((msr >> 24) & 0x3F) as u8,
            fan_temp_offset: ((msr >> 8) & 0xFF) as u8,
            tcc_offset_clamping: bit(msr, 7),
            locked: bit(msr, 31),
        }
    }
}

/// IA32_THERM_STATUS (0x19C) and IA32_PACKAGE_THERM_STATUS (0x1B1)
#[derive(Debug, Clone, Copy)]
pub struct ThermStatus {
    pub raw: u64,
    /// Temperature is valid
    pub valid: bool,
    /// Degrees C below the reference temperature
    pub readout: u8,
    /// Resolution of the readout in degrees C
    pub resolution: u8,
}

impl From<u64> for ThermStatus {
    fn from(msr: u64) -> Self {
        Self {
            raw: msr,
            valid: bit(msr, 31),
            readout: ((msr >> 16) & 0x7F) as u8,
            resolution: ((msr >> 27) & 0xF) as u8,
        }
    }
}

impl ThermStatus {
    /// Temperature in degrees C, needs the reference temperature from
    /// IA32_TEMPERATURE_TARGET
    pub fn temp(&self, ref_temp: u8) -> Option<i32> {
        if !self.valid {
            return None;
        }
        Some(ref_temp as i32 - self.readout as i32)
    }

    /// Whether the core or package is currently being thermally throttled
    pub fn throttling(&self) -> bool {
        bit(self.raw, 0) || bit(self.raw, 2)
    }
}

// -------------------------------------------------------------------------
// Printing
// -------------------------------------------------------------------------

/// Print PROCHOT status, frequency limiting reasons and thermal MSR details
///
/// Silently does nothing if the platform doesn't have these MSRs or we can't
/// read them. Reasons are logged.
pub fn print_thermal_msrs() {
    let Some(cpuid) = cpuid() else {
        debug!("No CPUID, not reading thermal MSRs");
        return;
    };
    if !cpuid.is_intel {
        info!("Thermal MSRs are only implemented for Intel processors");
        return;
    }
    if !cpuid.has_dts && !cpuid.has_ptm {
        info!("Processor has no digital thermal sensor, not reading thermal MSRs");
        return;
    }

    let Some(target) = read_msr(0, MSR_IA32_TEMPERATURE_TARGET).map(TemperatureTarget::from) else {
        info!("{}", imp::unavailable_hint());
        return;
    };

    println!("  Intel Thermal MSRs");
    println!("    TjMax:              {:>4} C", target.ref_temp);
    println!(
        "    TCC Activation:     {:>4} C (Offset {} C{})",
        target.ref_temp - target.tcc_offset,
        target.tcc_offset,
        if target.tcc_offset_clamping {
            ", clamping"
        } else {
            ""
        }
    );
    // Our EC does its own fan control, so this is usually left at 0 (unused)
    if target.fan_temp_offset > 0 {
        println!(
            "    Fan Temp Target:    {:>4} C (Offset {} C)",
            target.ref_temp - target.fan_temp_offset,
            target.fan_temp_offset
        );
    } else {
        debug!("Fan temperature target offset (T-Control) not programmed");
    }
    debug!("TEMPERATURE_TARGET locked: {}", target.locked);

    if cpuid.has_ptm {
        if let Some(pkg) = read_msr(0, MSR_IA32_PACKAGE_THERM_STATUS).map(ThermStatus::from) {
            if let Some(temp) = pkg.temp(target.ref_temp) {
                println!(
                    "    Package Temp:       {:>4} C (Resolution {} C)",
                    temp, pkg.resolution
                );
            }
            println!("    Package Thermal Status ({:#010X})", pkg.raw);
            print_therm_status_table(pkg.raw, THERM_STATUS_PKG_BITS);
        }
    }

    if cpuid.has_dts {
        print_core_therm_status(target.ref_temp);
    }

    if cpuid.has_perf_limit_reasons() {
        print_perf_limit_reasons();
    } else {
        debug!(
            "No PERF_LIMIT_REASONS MSRs on family {:#X} model {:#X}",
            cpuid.family, cpuid.model
        );
    }

    print_power_ctl();
}

/// Print the Active/Logged table of a THERM_STATUS style MSR
///
/// The bits above 11 differ between the per core and the package register, so
/// the caller passes those in.
fn print_therm_status_table(raw: u64, extra: &[(u32, &str)]) {
    println!("      {:<24} {:>6}  {:>6}", "Condition", "Active", "Logged");
    for (b, name) in THERM_STATUS_BITS.iter().chain(extra) {
        println!(
            "      {:<24} {:>6}  {:>6}",
            format!("{}:", name),
            yes_no(bit(raw, *b)),
            yes_no(bit(raw, b + 1))
        );
    }
}

/// Print the hottest core and the thermal status across all cores
fn print_core_therm_status(ref_temp: u8) {
    let cpus = cpu_count();
    let mut hottest: Option<(u32, i32)> = None;
    let mut throttling = Vec::new();
    // The status bits are per core, so OR them together to see whether any
    // core ever hit a condition. Reading every core individually is far too
    // much output on a system with dozens of them.
    let mut any = 0;
    let mut read = 0;

    for cpu in 0..cpus {
        let Some(status) = read_msr(cpu, MSR_IA32_THERM_STATUS).map(ThermStatus::from) else {
            continue;
        };
        read += 1;
        // Mask off temperature, resolution and valid, they're not status bits
        any |= status.raw & 0xFFFF;
        if let Some(temp) = status.temp(ref_temp) {
            match hottest {
                Some((_, hottest_temp)) if temp <= hottest_temp => {}
                _ => hottest = Some((cpu, temp)),
            }
        }
        if status.throttling() {
            throttling.push(format!("{}", cpu));
        }
    }

    if read == 0 {
        return;
    }
    if let Some((cpu, temp)) = hottest {
        println!("    Hottest Core Temp:  {:>4} C (CPU {})", temp, cpu);
    }
    println!(
        "    Cores Throttling:   {:>4}",
        if throttling.is_empty() {
            "None".to_string()
        } else {
            throttling.join(", ")
        }
    );
    println!(
        "    Core Thermal Status, any of {} cores ({:#06X})",
        read, any
    );
    print_therm_status_table(any, THERM_STATUS_CORE_BITS);
}

/// Print which limits are clipping the core, graphics and ring frequency
fn print_perf_limit_reasons() {
    for (msr, name, bits) in [
        (MSR_IA_PERF_LIMIT_REASONS, "Core", PLR_CORE_BITS),
        (MSR_GT_PERF_LIMIT_REASONS, "Graphics", PLR_GT_BITS),
        (MSR_RING_PERF_LIMIT_REASONS, "Ring", PLR_RING_BITS),
    ] {
        let Some(value) = read_msr(0, msr) else {
            continue;
        };
        println!(
            "    {} Frequency Limit Reasons ({:#X}: {:#010X})",
            name, msr, value
        );
        println!("      Active:           {}", decode_reasons(value, bits, 0));
        println!(
            "      Logged:           {}",
            decode_reasons(value, bits, 16)
        );
    }
}

/// Print how the processor is configured to react to PROCHOT
fn print_power_ctl() {
    let Some(value) = read_msr(0, MSR_POWER_CTL) else {
        return;
    };
    println!("    PROCHOT Config ({:#X}: {:#010X})", MSR_POWER_CTL, value);
    // Bidirectional PROCHOT lets the EC throttle the CPU by asserting PROCHOT#
    println!("      Bidirectional:    {}", yes_no(bit(value, 0)));
    println!("      Output Enabled:   {}", yes_no(!bit(value, 21)));
    // The reference code only documents this as "Prochot Configurable
    // Response Enable", so don't claim to know what the response is
    println!(
        "      Response:         {}",
        if bit(value, 22) {
            "Configurable"
        } else {
            "Throttle to minimum"
        }
    );
    println!("      VR Therm Alert:   {}", yes_no(!bit(value, 24)));
    println!("      Locked:           {}", yes_no(bit(value, 23)));
}
