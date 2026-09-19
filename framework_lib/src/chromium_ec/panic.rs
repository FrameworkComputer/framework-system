//! Decode and print EC panic data (struct panic_data)
//!
//! Port of the EC's panic printing from core/cortex-m/panic.c.
//! See the EC's include/panic_defs.h for the struct layout.

use alloc::vec::Vec;
use core::convert::TryInto;

use crate::util;

/// "Pnc!" if valid
pub const PANIC_DATA_MAGIC: u32 = 0x21636e50;

// Flags for panic_data.flags
/// panic_data.frame is valid
const PANIC_DATA_FLAG_FRAME_VALID: u8 = 0x01;
/// Already printed at console
const PANIC_DATA_FLAG_OLD_CONSOLE: u8 = 0x02;
/// Already returned via host command
const PANIC_DATA_FLAG_OLD_HOSTCMD: u8 = 0x04;
/// Already reported via host event
const PANIC_DATA_FLAG_OLD_HOSTEVENT: u8 = 0x08;
/// The data was truncated to fit panic info host cmd
const PANIC_DATA_FLAG_TRUNCATED: u8 = 0x10;

const FLAG_NAMES: [(u8, &str); 5] = [
    (PANIC_DATA_FLAG_FRAME_VALID, "FRAME_VALID"),
    (PANIC_DATA_FLAG_OLD_CONSOLE, "OLD_CONSOLE"),
    (PANIC_DATA_FLAG_OLD_HOSTCMD, "OLD_HOSTCMD"),
    (PANIC_DATA_FLAG_OLD_HOSTEVENT, "OLD_HOSTEVENT"),
    (PANIC_DATA_FLAG_TRUNCATED, "TRUNCATED"),
];

const PANIC_ARCH_CORTEX_M: u8 = 1;

// Fault status register bits, see the EC's core/cortex-m/cpu.h
const CPU_NVIC_CFSR_BFARVALID: u32 = 1 << 15;
const CPU_NVIC_CFSR_MFARVALID: u32 = 1 << 7;
const CPU_NVIC_HFSR_DEBUGEVT: u32 = 1 << 31;
const CPU_NVIC_HFSR_FORCED: u32 = 1 << 30;
const CPU_NVIC_HFSR_VECTTBL: u32 = 1 << 1;

/// Names for each of the bits in the CFSR register, starting at bit 0
const CFSR_NAME: [(u32, &str); 15] = [
    // MMFSR
    (0, "Instruction access violation"),
    (1, "Data access violation"),
    (3, "Unstack from exception violation"),
    (4, "Stack from exception violation"),
    // BFSR
    (8, "Instruction bus error"),
    (9, "Precise data bus error"),
    (10, "Imprecise data bus error"),
    (11, "Unstack from exception bus fault"),
    (12, "Stack from exception bus fault"),
    // UFSR
    (16, "Undefined instructions"),
    (17, "Invalid state"),
    (18, "Invalid PC"),
    (19, "No coprocessor"),
    (24, "Unaligned"),
    (25, "Divide by 0"),
];

/// Names for the first 5 bits in the DFSR
const DFSR_NAME: [&str; 5] = [
    "Halt request",
    "Breakpoint",
    "Data watchpoint/trace",
    "Vector catch",
    "External debug request",
];

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

/// The least significant 4 bits of the exception LR (EXC_RETURN) determine
/// the exception stack and context. See B1.5.8 of ARM DDI 0403D.
fn is_frame_in_handler_stack(exc_return: u32) -> bool {
    matches!(
        exc_return,
        0xfffffff1 | 0xfffffff9 | 0xffffffe1 | 0xffffffe9
    )
}

fn is_exception_from_handler_mode(exc_return: u32) -> bool {
    matches!(exc_return, 0xfffffff1 | 0xffffffe1)
}

/// Print a single register, unavailable registers print as blank
fn print_reg(regnum: usize, value: Option<u32>) {
    const NAMES: [&str; 6] = ["r10", "r11", "r12", "sp ", "lr ", "pc "];
    if regnum < 10 {
        print!("r{:<2}:", regnum);
    } else {
        print!("{}:", NAMES[regnum - 10]);
    }
    if let Some(value) = value {
        print!("{:08x}", value);
    } else {
        print!("        ");
    }
    if regnum & 3 == 3 {
        println!();
    } else {
        print!(" ");
    }
}

/// Decoded registers of a Cortex-M EC panic
///
/// Port of the register handling in panic_data_print() in the EC's
/// core/cortex-m/panic.c, with handling for the older struct version 1
/// (missing MSP, LR at another position) like the EC's util/ec_panicinfo.c.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CortexMPanic {
    /// Exception number (IPSR)
    pub exception: u32,
    /// Whether the exception happened in handler mode, otherwise process mode
    pub handler_mode: bool,
    /// r0-r12, sp, lr, pc. Registers from the exception stack frame
    /// (r0-r3, r12, lr, pc) are `None` if the frame is not valid.
    pub regs: [Option<u32>; 16],
    /// xPSR from the exception stack frame, `None` if the frame is not valid
    pub xpsr: Option<u32>,
    pub cfsr: u32,
    pub bfar: u32,
    pub mfar: u32,
    pub shcsr: u32,
    pub hfsr: u32,
    pub dfsr: u32,
}

impl CortexMPanic {
    /// Textual representation of the set bits in the fault registers
    pub fn fault_names(&self) -> Vec<&'static str> {
        let mut names = CFSR_NAME
            .iter()
            .filter(|(bit, _)| self.cfsr & (1 << bit) != 0)
            .map(|(_, name)| *name)
            .collect::<Vec<_>>();
        if self.hfsr & CPU_NVIC_HFSR_DEBUGEVT != 0 {
            names.push("Debug event");
        }
        if self.hfsr & CPU_NVIC_HFSR_FORCED != 0 {
            names.push("Forced hard fault");
        }
        if self.hfsr & CPU_NVIC_HFSR_VECTTBL != 0 {
            names.push("Vector table bus fault");
        }
        for (bit, name) in DFSR_NAME.iter().enumerate() {
            if self.dfsr & (1 << bit) != 0 {
                names.push(name);
            }
        }
        names
    }

    /// Whether the bus fault address register holds a valid address
    pub fn bfar_valid(&self) -> bool {
        self.cfsr & CPU_NVIC_CFSR_BFARVALID != 0
    }

    /// Whether the memory management fault address register holds a valid address
    pub fn mfar_valid(&self) -> bool {
        self.cfsr & CPU_NVIC_CFSR_MFARVALID != 0
    }
}

/// Parse panic data of a Cortex-M EC
///
/// Returns `None` if the data is too short to hold all registers.
fn parse_panic_info_cm(data: &[u8], struct_version: u8, flags: u8) -> Option<CortexMPanic> {
    // Register offsets into the data blob. Registers not saved on the
    // exception stack frame come first (lregs), the stack frame follows
    // (sregs). See struct cortex_panic_data(_v1) in the EC.
    //
    // lregs v2: psp, ipsr, msp, r4-r11, exc_lr (12 entries)
    // lregs v1: psp, ipsr, exc_lr, r4-r11   (11 entries)
    // sregs:    r0-r3, r12, lr, pc, xpsr    (8 entries)
    // fault:    cfsr, bfar, mfar, shcsr, hfsr, dfsr
    let (num_lregs, exc_lr_idx) = if struct_version == 1 {
        (11, 2)
    } else {
        (12, 11)
    };
    let frame_offset = 4 + 4 * num_lregs;
    let fault_offset = frame_offset + 4 * 8;
    if data.len() < fault_offset + 4 * 6 {
        return None;
    }

    let lreg = |i: usize| u32_at(data, 4 + 4 * i);
    let frame_valid = flags & PANIC_DATA_FLAG_FRAME_VALID != 0;
    let sreg = |i: usize| frame_valid.then(|| u32_at(data, frame_offset + 4 * i));

    let exc_lr = lreg(exc_lr_idx);
    // v1 does not save the MSP, fall back to the PSP
    let sp = if struct_version != 1 && is_frame_in_handler_stack(exc_lr) {
        lreg(2) // msp
    } else {
        lreg(0) // psp
    };

    let mut regs = [None; 16];
    for (i, reg) in regs.iter_mut().enumerate().take(4) {
        *reg = sreg(i);
    }
    for (i, reg) in regs.iter_mut().enumerate().take(10).skip(4) {
        *reg = Some(lreg(i - 1));
    }
    regs[10] = Some(lreg(9));
    regs[11] = Some(lreg(10));
    regs[12] = sreg(4);
    regs[13] = Some(sp);
    regs[14] = sreg(5);
    regs[15] = sreg(6);

    Some(CortexMPanic {
        exception: lreg(1) & 0xff,
        handler_mode: is_exception_from_handler_mode(exc_lr),
        regs,
        xpsr: sreg(7),
        cfsr: u32_at(data, fault_offset),
        bfar: u32_at(data, fault_offset + 4),
        mfar: u32_at(data, fault_offset + 8),
        shcsr: u32_at(data, fault_offset + 12),
        hfsr: u32_at(data, fault_offset + 16),
        dfsr: u32_at(data, fault_offset + 20),
    })
}

/// Print panic data of a Cortex-M EC like the EC's console does
fn print_panic_info_cm(panic: &CortexMPanic) {
    println!(
        "=== {} EXCEPTION: {:02x} ====== xPSR: {:08x} ===",
        if panic.handler_mode {
            "HANDLER"
        } else {
            "PROCESS"
        },
        panic.exception,
        panic.xpsr.unwrap_or(0xffffffff),
    );
    for (i, reg) in panic.regs.iter().enumerate() {
        print_reg(i, *reg);
    }

    print!("{}", panic.fault_names().join(", "));
    if panic.bfar_valid() {
        print!(", bfar = {:x}", panic.bfar);
    }
    if panic.mfar_valid() {
        print!(", mfar = {:x}", panic.mfar);
    }
    println!();
    println!(
        "cfsr = {:x}, shcsr = {:x}, hfsr = {:x}, dfsr = {:x}",
        panic.cfsr, panic.shcsr, panic.hfsr, panic.dfsr
    );
}

/// Decoded EC panic data (struct panic_data)
///
/// Use [`parse_panic_info`] to decode it and [`print_panic_info`] to show it
/// like the commandline tool does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicInfo {
    /// Architecture of the EC, see [`PanicInfo::cortex_m`]
    pub arch: u8,
    pub struct_version: u8,
    /// Raw flags, see [`PanicInfo::flag_names`]
    pub flags: u8,
    /// Size of the struct as recorded by the EC
    pub struct_size: u32,
    /// Should be [`PANIC_DATA_MAGIC`]
    pub magic: u32,
    /// Length of the data we actually received
    pub data_len: usize,
    /// Decoded registers, `None` if the architecture is unknown or the
    /// data is too short
    pub cortex_m: Option<CortexMPanic>,
}

impl PanicInfo {
    pub fn magic_valid(&self) -> bool {
        self.magic == PANIC_DATA_MAGIC
    }

    /// Whether the recorded struct size matches the data we received
    pub fn size_consistent(&self) -> bool {
        self.struct_size as usize == self.data_len
    }

    pub fn version_known(&self) -> bool {
        self.struct_version <= 2
    }

    /// Whether this panic was already reported via host command before
    pub fn already_reported(&self) -> bool {
        self.flags & PANIC_DATA_FLAG_OLD_HOSTCMD != 0
    }

    /// Whether the EC is a Cortex-M, the only architecture we can decode
    pub fn is_cortex_m(&self) -> bool {
        self.arch == PANIC_ARCH_CORTEX_M
    }

    /// Names of the set flags
    pub fn flag_names(&self) -> Vec<&'static str> {
        FLAG_NAMES
            .iter()
            .filter(|(bit, _)| self.flags & bit != 0)
            .map(|(_, name)| *name)
            .collect()
    }
}

/// Parse panic data as returned by EC_CMD_GET_PANIC_INFO
///
/// Returns `None` if the data is too short to hold the header and trailer.
/// Implausible data is not rejected, check the `PanicInfo` methods.
pub fn parse_panic_info(data: &[u8]) -> Option<PanicInfo> {
    // arch, struct_version, flags, reserved
    const HEADER_SIZE: usize = 4;
    // struct_size, magic - at the very end of the struct
    const TRAILER_SIZE: usize = 8;
    if data.len() < HEADER_SIZE + TRAILER_SIZE {
        return None;
    }

    let arch = data[0];
    let struct_version = data[1];
    let flags = data[2];
    let cortex_m = if arch == PANIC_ARCH_CORTEX_M {
        parse_panic_info_cm(data, struct_version, flags)
    } else {
        None
    };

    Some(PanicInfo {
        arch,
        struct_version,
        flags,
        struct_size: u32_at(data, data.len() - 8),
        magic: u32_at(data, data.len() - 4),
        data_len: data.len(),
        cortex_m,
    })
}

/// Parse and print panic data as returned by EC_CMD_GET_PANIC_INFO
///
/// The data must not be empty. Prints warnings if the data looks
/// implausible and falls back to a hex dump if it cannot be decoded.
pub fn print_panic_info(data: &[u8]) {
    let Some(info) = parse_panic_info(data) else {
        println!("Panic data too short ({} bytes), hex dump:", data.len());
        util::print_multiline_buffer(data, 0);
        return;
    };

    if !info.magic_valid() {
        println!(
            "WARNING: Incorrect panic magic ({:#010x}), following data may be incorrect!",
            info.magic
        );
    }
    if !info.size_consistent() {
        println!(
            "WARNING: Panic struct size inconsistent ({} vs {}), following data may be incorrect!",
            info.struct_size, info.data_len
        );
    }
    if !info.version_known() {
        println!(
            "WARNING: Unknown panic data version ({}), following data may be incorrect!",
            info.struct_version
        );
    }

    println!(
        "Saved panic data:{}",
        if info.already_reported() {
            ""
        } else {
            " (NEW)"
        }
    );
    println!(
        "Flags: {:#04x} ({})",
        info.flags,
        info.flag_names().join(" | ")
    );

    if !info.is_cortex_m() {
        println!("Unknown architecture ({})", info.arch);
    }
    if let Some(cortex_m) = &info.cortex_m {
        print_panic_info_cm(cortex_m);
    } else {
        println!("Cannot decode panic data, hex dump:");
        util::print_multiline_buffer(data, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Build a struct version 2 Cortex-M panic data blob (116 bytes)
    fn cm_v2_blob() -> Vec<u8> {
        let mut data = vec![0u8; 116];
        data[0] = PANIC_ARCH_CORTEX_M;
        data[1] = 2; // struct_version
        data[2] = PANIC_DATA_FLAG_FRAME_VALID;
        // psp (lregs[0])
        data[4..8].copy_from_slice(&0x2000_1000u32.to_le_bytes());
        // ipsr (lregs[1]): exception 3 (hard fault)
        data[8..12].copy_from_slice(&3u32.to_le_bytes());
        // exc_lr (lregs[11]): exception from process mode, PSP used
        data[4 + 4 * 11..4 + 4 * 12].copy_from_slice(&0xfffffffdu32.to_le_bytes());
        // pc (sregs[6])
        let frame_offset = 4 + 4 * 12;
        data[frame_offset + 4 * 6..frame_offset + 4 * 7]
            .copy_from_slice(&0x0800_1234u32.to_le_bytes());
        // hfsr: forced hard fault
        let fault_offset = frame_offset + 4 * 8;
        data[fault_offset + 16..fault_offset + 20]
            .copy_from_slice(&CPU_NVIC_HFSR_FORCED.to_le_bytes());
        let len = data.len();
        data[len - 8..len - 4].copy_from_slice(&116u32.to_le_bytes());
        data[len - 4..].copy_from_slice(&PANIC_DATA_MAGIC.to_le_bytes());
        data
    }

    #[test]
    fn decode_cm_v2() {
        let data = cm_v2_blob();
        let info = parse_panic_info(&data).unwrap();
        assert!(info.magic_valid());
        assert!(info.size_consistent());
        assert!(info.version_known());
        assert!(!info.already_reported());
        assert!(info.is_cortex_m());
        assert_eq!(info.flag_names(), vec!["FRAME_VALID"]);

        let cm = info.cortex_m.as_ref().unwrap();
        assert_eq!(cm.exception, 3);
        assert!(!cm.handler_mode);
        assert_eq!(cm.regs[13], Some(0x2000_1000)); // sp = psp
        assert_eq!(cm.regs[15], Some(0x0800_1234)); // pc
        assert_eq!(cm.xpsr, Some(0));
        assert_eq!(cm.fault_names(), vec!["Forced hard fault"]);
        assert!(!cm.bfar_valid());

        // Must not panic
        print_panic_info(&data);
    }

    #[test]
    fn invalid_frame_has_no_stack_registers() {
        let mut data = cm_v2_blob();
        data[2] = 0; // no FRAME_VALID
        let info = parse_panic_info(&data).unwrap();
        let cm = info.cortex_m.as_ref().unwrap();
        assert_eq!(cm.regs[0], None);
        assert_eq!(cm.regs[4], Some(0)); // r4 comes from lregs, always valid
        assert_eq!(cm.regs[15], None);
        assert_eq!(cm.xpsr, None);
    }

    #[test]
    fn unknown_arch_and_short_data() {
        let mut unknown_arch = cm_v2_blob();
        unknown_arch[0] = 42;
        let info = parse_panic_info(&unknown_arch).unwrap();
        assert!(!info.is_cortex_m());
        assert!(info.cortex_m.is_none());
        // Must not panic, falls back to hex dump
        print_panic_info(&unknown_arch);

        assert!(parse_panic_info(&[1, 2, 3]).is_none());
        print_panic_info(&[1, 2, 3]);
    }

    #[test]
    fn too_short_for_registers() {
        // Valid header/trailer but not enough space for Cortex-M registers
        let mut data = cm_v2_blob();
        data.truncate(50);
        assert!(parse_panic_info_cm(&data, 2, 0).is_none());
        let info = parse_panic_info(&data).unwrap();
        assert!(info.is_cortex_m());
        assert!(info.cortex_m.is_none());
        print_panic_info(&data);
    }
}
