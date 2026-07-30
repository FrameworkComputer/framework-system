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
/// PL4, the instantaneous peak limit (MSR_VR_CURRENT_CONFIG)
pub const MSR_VR_CURRENT_CONFIG: u32 = 0x00000601;
/// Unit multipliers for the RAPL registers (MSR_PACKAGE_POWER_SKU_UNIT)
pub const MSR_PACKAGE_POWER_SKU_UNIT: u32 = 0x00000606;
/// PL1 and PL2 package power limits (MSR_PACKAGE_RAPL_LIMIT)
pub const MSR_PACKAGE_RAPL_LIMIT: u32 = 0x00000610;
/// TDP and the power range of this SKU (MSR_PACKAGE_POWER_SKU)
pub const MSR_PACKAGE_POWER_SKU: u32 = 0x00000614;
/// PSys PL1 and PL2, limiting the whole platform instead of the package
pub const MSR_PLATFORM_POWER_LIMIT: u32 = 0x0000065C;

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
    /// Intel Skylake (model 0x4E) or newer
    ///
    /// Most of the MSRs we read are not architectural, so we check this before
    /// touching them. Reading a reserved MSR raises a general protection
    /// fault, which is fatal in UEFI.
    pub fn skylake_or_newer(&self) -> bool {
        self.is_intel && self.family == 6 && self.model >= 0x4E
    }

    /// Whether the *_PERF_LIMIT_REASONS MSRs at their modern addresses exist
    pub fn has_perf_limit_reasons(&self) -> bool {
        self.skylake_or_newer()
    }

    /// Whether the RAPL power limit MSRs exist
    ///
    /// RAPL was introduced with Sandy Bridge (model 0x2A).
    pub fn has_rapl(&self) -> bool {
        self.is_intel && self.family == 6 && self.model >= 0x2A
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

    /// Enumerate the directory instead of probing sequentially, so that an
    /// offline CPU in the middle doesn't cut the enumeration short
    pub fn cpu_count() -> u32 {
        let Ok(dir) = std::fs::read_dir("/dev/cpu") else {
            return 0;
        };
        dir.flatten()
            .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
            .max()
            .map_or(0, |max| max + 1)
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

    /// Enumerate the devices instead of probing sequentially, so that an
    /// offline CPU in the middle doesn't cut the enumeration short
    pub fn cpu_count() -> u32 {
        let Ok(dir) = std::fs::read_dir("/dev") else {
            return 0;
        };
        dir.flatten()
            .filter_map(|entry| {
                entry
                    .file_name()
                    .to_str()?
                    .strip_prefix("cpuctl")?
                    .parse::<u32>()
                    .ok()
            })
            .max()
            .map_or(0, |max| max + 1)
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

/// Unit multipliers from MSR_PACKAGE_POWER_SKU_UNIT (0x606)
#[derive(Debug, Clone, Copy)]
pub struct RaplUnits {
    /// Watts per LSB of the power fields, 1/8 W by default
    pub power: f32,
    /// Seconds per LSB of the time window fields, 1/1024 s by default
    pub time: f32,
}

impl From<u64> for RaplUnits {
    fn from(msr: u64) -> Self {
        Self {
            power: 1.0 / (1u32 << (msr & 0xF)) as f32,
            time: 1.0 / (1u32 << ((msr >> 16) & 0xF)) as f32,
        }
    }
}

/// One power limit out of a RAPL limit register
#[derive(Debug, Clone, Copy)]
pub struct PowerLimit {
    pub watts: f32,
    pub enabled: bool,
    /// Processor may go below the OS requested P-State to hold the limit
    pub clamping: bool,
    /// Averaging window, None if this limit has no time window field
    pub time_window: Option<f32>,
}

/// Decode the 7 bit time window field of a RAPL limit register
///
/// Time Window = (1 + X/4) * 2^Y in units of `RaplUnits::time`, where Y is
/// bits 4:0 and X is bits 6:5 of the field.
fn time_window(field: u64, units: &RaplUnits) -> f32 {
    let y = field & 0x1F;
    let x = (field >> 5) & 0x3;
    (1.0 + x as f32 / 4.0) * (1u64 << y) as f32 * units.time
}

/// Decode one PL1/PL2 style limit out of a RAPL limit register
///
/// The fields repeat every 32 bits, so `shift` is 0 for PL1 and 32 for PL2.
fn decode_limit(raw: u64, shift: u32, has_time: bool, units: &RaplUnits) -> PowerLimit {
    PowerLimit {
        watts: ((raw >> shift) & 0x7FFF) as f32 * units.power,
        enabled: bit(raw, shift + 15),
        clamping: bit(raw, shift + 16),
        time_window: if has_time {
            Some(time_window((raw >> (shift + 17)) & 0x7F, units))
        } else {
            None
        },
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

    print_power_limits(&cpuid);

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

/// Print one PL1/PL2 style limit
fn print_limit(name: &str, limit: &PowerLimit) {
    let mut notes = Vec::new();
    if !limit.enabled {
        notes.push("Disabled".to_string());
    }
    if limit.clamping {
        notes.push("Clamping".to_string());
    }
    if let Some(window) = limit.time_window {
        notes.push(format!("{:.3} s window", window));
    }
    println!(
        "      {:<20} {:>6.1} W  {}",
        format!("{}:", name),
        limit.watts,
        notes.join(", ")
    );
}

/// Print the RAPL power limits, which is what the PkgPwr limiting reasons refer to
fn print_power_limits(cpuid: &CpuId) {
    if !cpuid.has_rapl() {
        debug!(
            "No RAPL MSRs on family {:#X} model {:#X}",
            cpuid.family, cpuid.model
        );
        return;
    }
    let Some(units) = read_msr(0, MSR_PACKAGE_POWER_SKU_UNIT).map(RaplUnits::from) else {
        return;
    };

    println!("    Power Limits");
    if let Some(sku) = read_msr(0, MSR_PACKAGE_POWER_SKU) {
        println!(
            "      {:<20} {:>6.1} W",
            "TDP (base power):",
            (sku & 0x7FFF) as f32 * units.power
        );
        // Not every SKU reports the power range
        let min = ((sku >> 16) & 0x7FFF) as f32 * units.power;
        let max = ((sku >> 32) & 0x7FFF) as f32 * units.power;
        if max > 0.0 {
            println!(
                "      {:<20} {:>6.1} W  (Min {:.1} W)",
                "SKU Max Power:", max, min
            );
        }
    }

    if let Some(raw) = read_msr(0, MSR_PACKAGE_RAPL_LIMIT) {
        print_limit("PL1 (sustained)", &decode_limit(raw, 0, true, &units));
        print_limit("PL2 (burst)", &decode_limit(raw, 32, true, &units));
        if bit(raw, 63) {
            println!("      {:<20} {:>6}", "PL1/PL2 Locked:", "Yes");
        }
    }

    // These two are not on pre-Skylake processors
    if !cpuid.skylake_or_newer() {
        return;
    }

    if let Some(raw) = read_msr(0, MSR_VR_CURRENT_CONFIG) {
        // The reference code describes the field in 0.125 A increments, but
        // coreboot programs it as Watts scaled by the RAPL power unit (which
        // is the same 1/8 by default) and so does the register description.
        println!(
            "      {:<20} {:>6.1} W  {}",
            "PL4 (peak):",
            (raw & 0xFFFF) as f32 * units.power,
            if bit(raw, 31) { "Locked" } else { "" }
        );
    }

    if let Some(raw) = read_msr(0, MSR_PLATFORM_POWER_LIMIT) {
        // Whether PSys is enabled matters: it is the only limit that accounts
        // for total platform power instead of just the package. With it off,
        // nothing keeps the system inside the adapter budget proactively and
        // the charger has to assert PROCHOT instead. So always report it, even
        // (especially) when it's disabled.
        print_limit("PSys PL1", &decode_limit(raw, 0, true, &units));
        // Bits 62:49 are reserved, PSys PL2 has no time window
        print_limit("PSys PL2", &decode_limit(raw, 32, false, &units));
        if bit(raw, 63) {
            println!("      {:<20} {:>6}", "PSys Locked:", "Yes");
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // The default units from the reference code are 1/8 W and 1/1024 s
    fn rapl_units() {
        let units = RaplUnits::from(0x000A_0E03);
        assert_eq!(units.power, 0.125);
        assert_eq!(units.time, 1.0 / 1024.0);
        // 1/2^0 for both
        let units = RaplUnits::from(0);
        assert_eq!(units.power, 1.0);
        assert_eq!(units.time, 1.0);
    }

    #[test]
    // Time Window = (1 + X/4) * 2^Y, Y is bits 4:0 and X is bits 6:5
    fn rapl_time_window() {
        let units = RaplUnits::from(0x000A_0E03);
        // The reset default of 0xA is Y=10, X=0, so 1024 * 1/1024 s
        assert_eq!(time_window(0x0A, &units), 1.0);
        // Y=10, X=2 gives 1.5 * 1024 * 1/1024 s
        assert_eq!(time_window(0x4A, &units), 1.5);
        assert_eq!(time_window(0, &units), 1.0 / 1024.0);
    }

    #[test]
    // PL1 28 W with a 1 s window and clamping, PL2 64 W with a 2.44 ms window
    fn rapl_limit() {
        let units = RaplUnits::from(0x000A_0E03);
        let raw = 0x8042_8200_0015_80E0;
        let pl1 = decode_limit(raw, 0, true, &units);
        assert_eq!(pl1.watts, 28.0);
        assert!(pl1.enabled);
        assert!(pl1.clamping);
        assert_eq!(pl1.time_window, Some(1.0));

        let pl2 = decode_limit(raw, 32, true, &units);
        assert_eq!(pl2.watts, 64.0);
        assert!(pl2.enabled);
        assert!(!pl2.clamping);
        // Y=1, X=1 gives 1.25 * 2 * 1/1024 s
        assert_eq!(pl2.time_window, Some(1.25 * 2.0 / 1024.0));

        // Bit 63 is the lock
        assert!(bit(raw, 63));
        // A limit without a time window field
        assert_eq!(decode_limit(raw, 32, false, &units).time_window, None);
    }

    #[test]
    // PL4 is scaled by the RAPL power unit, like PL1 and PL2. 0x02E8 is the
    // 93 W a Framework 13 reports.
    fn rapl_pl4() {
        let units = RaplUnits::from(0x000A_0E03);
        assert_eq!((0x02E8 & 0xFFFF) as f32 * units.power, 93.0);
    }

    #[test]
    // TjMax 100 C with an 8 C TCC offset, as seen on a Framework 13
    fn temperature_target() {
        let target = TemperatureTarget::from(0x0864_0000);
        assert_eq!(target.ref_temp, 100);
        assert_eq!(target.tcc_offset, 8);
        assert_eq!(target.fan_temp_offset, 0);
        assert!(!target.locked);
    }

    #[test]
    // A real package status read: 46 C, only the power limit log bit set
    fn therm_status() {
        let status = ThermStatus::from(0x8836_0800);
        assert!(status.valid);
        assert_eq!(status.readout, 54);
        assert_eq!(status.resolution, 1);
        assert_eq!(status.temp(100), Some(46));
        assert!(!status.throttling());
        // Bit 11 is the power limitation log
        assert!(bit(status.raw, 11));

        // Without the valid bit there is no temperature
        assert_eq!(ThermStatus::from(0x0836_0800).temp(100), None);
        // Bit 0 thermal monitor and bit 2 PROCHOT both mean throttling
        assert!(ThermStatus::from(0x8000_0001).throttling());
        assert!(ThermStatus::from(0x8000_0004).throttling());
        // Log bits alone are not current throttling
        assert!(!ThermStatus::from(0x8000_000A).throttling());
    }

    #[test]
    // The values from a Framework 13 with PROCHOT logged on all three domains
    fn perf_limit_reasons() {
        assert_eq!(decode_reasons(0x1803_0000, PLR_CORE_BITS, 0), "None");
        assert_eq!(
            decode_reasons(0x1803_0000, PLR_CORE_BITS, 16),
            "PROCHOT, Thermal, PkgPwrPL2, MaxTurboLimit"
        );
        assert_eq!(
            decode_reasons(0x1001_0000, PLR_GT_BITS, 16),
            "PROCHOT, InefficientOperation"
        );
        assert_eq!(decode_reasons(0x0001_0000, PLR_RING_BITS, 16), "PROCHOT");
        // The ring domain has no bit 12/13, so those must not be decoded
        assert_eq!(decode_reasons(0x3000_0000, PLR_RING_BITS, 16), "None");
    }
}
