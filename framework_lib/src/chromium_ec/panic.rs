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

/// Print a textual representation of the fault registers
fn print_fault(cfsr: u32, hfsr: u32, dfsr: u32) {
    let mut names = CFSR_NAME
        .iter()
        .filter(|(bit, _)| cfsr & (1 << bit) != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();
    if hfsr & CPU_NVIC_HFSR_DEBUGEVT != 0 {
        names.push("Debug event");
    }
    if hfsr & CPU_NVIC_HFSR_FORCED != 0 {
        names.push("Forced hard fault");
    }
    if hfsr & CPU_NVIC_HFSR_VECTTBL != 0 {
        names.push("Vector table bus fault");
    }
    for (bit, name) in DFSR_NAME.iter().enumerate() {
        if dfsr & (1 << bit) != 0 {
            names.push(name);
        }
    }
    print!("{}", names.join(", "));
}

/// Print panic data of a Cortex-M EC
///
/// Port of panic_data_print() in the EC's core/cortex-m/panic.c, with
/// handling for the older struct version 1 (missing MSP, LR at another
/// position) like the EC's util/ec_panicinfo.c.
fn print_panic_info_cm(data: &[u8], struct_version: u8, flags: u8) -> Option<()> {
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
    println!(
        "=== {} EXCEPTION: {:02x} ====== xPSR: {:08x} ===",
        if is_exception_from_handler_mode(exc_lr) {
            "HANDLER"
        } else {
            "PROCESS"
        },
        lreg(1) & 0xff,
        sreg(7).unwrap_or(0xffffffff),
    );
    for i in 0..4 {
        print_reg(i, sreg(i));
    }
    for i in 4..10 {
        print_reg(i, Some(lreg(i - 1)));
    }
    print_reg(10, Some(lreg(9)));
    print_reg(11, Some(lreg(10)));
    print_reg(12, sreg(4));
    // v1 does not save the MSP, fall back to the PSP
    let sp = if struct_version != 1 && is_frame_in_handler_stack(exc_lr) {
        lreg(2) // msp
    } else {
        lreg(0) // psp
    };
    print_reg(13, Some(sp));
    print_reg(14, sreg(5));
    print_reg(15, sreg(6));

    let cfsr = u32_at(data, fault_offset);
    let bfar = u32_at(data, fault_offset + 4);
    let mfar = u32_at(data, fault_offset + 8);
    let shcsr = u32_at(data, fault_offset + 12);
    let hfsr = u32_at(data, fault_offset + 16);
    let dfsr = u32_at(data, fault_offset + 20);

    print_fault(cfsr, hfsr, dfsr);
    if cfsr & CPU_NVIC_CFSR_BFARVALID != 0 {
        print!(", bfar = {:x}", bfar);
    }
    if cfsr & CPU_NVIC_CFSR_MFARVALID != 0 {
        print!(", mfar = {:x}", mfar);
    }
    println!();
    println!(
        "cfsr = {:x}, shcsr = {:x}, hfsr = {:x}, dfsr = {:x}",
        cfsr, shcsr, hfsr, dfsr
    );

    Some(())
}

/// Parse and print panic data as returned by EC_CMD_GET_PANIC_INFO
///
/// The data must not be empty. Prints warnings if the data looks
/// implausible and falls back to a hex dump if it cannot be decoded.
pub fn print_panic_info(data: &[u8]) {
    // arch, struct_version, flags, reserved
    const HEADER_SIZE: usize = 4;
    // struct_size, magic - at the very end of the struct
    const TRAILER_SIZE: usize = 8;
    if data.len() < HEADER_SIZE + TRAILER_SIZE {
        println!("Panic data too short ({} bytes), hex dump:", data.len());
        util::print_multiline_buffer(data, 0);
        return;
    }

    let arch = data[0];
    let struct_version = data[1];
    let flags = data[2];
    let struct_size = u32_at(data, data.len() - 8);
    let magic = u32_at(data, data.len() - 4);

    if magic != PANIC_DATA_MAGIC {
        println!(
            "WARNING: Incorrect panic magic ({:#010x}), following data may be incorrect!",
            magic
        );
    }
    if struct_size as usize != data.len() {
        println!(
            "WARNING: Panic struct size inconsistent ({} vs {}), following data may be incorrect!",
            struct_size,
            data.len()
        );
    }
    if struct_version > 2 {
        println!(
            "WARNING: Unknown panic data version ({}), following data may be incorrect!",
            struct_version
        );
    }

    println!(
        "Saved panic data:{}",
        if flags & PANIC_DATA_FLAG_OLD_HOSTCMD != 0 {
            ""
        } else {
            " (NEW)"
        }
    );
    let flag_names = FLAG_NAMES
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>();
    println!("Flags: {:#04x} ({})", flags, flag_names.join(" | "));

    let decoded = match arch {
        PANIC_ARCH_CORTEX_M => print_panic_info_cm(data, struct_version, flags),
        _ => {
            println!("Unknown architecture ({})", arch);
            None
        }
    };
    if decoded.is_none() {
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
        // exc_lr (lregs[11]): exception from process mode, PSP used
        data[4 + 4 * 11..4 + 4 * 12].copy_from_slice(&0xfffffffdu32.to_le_bytes());
        let len = data.len();
        data[len - 8..len - 4].copy_from_slice(&116u32.to_le_bytes());
        data[len - 4..].copy_from_slice(&PANIC_DATA_MAGIC.to_le_bytes());
        data
    }

    #[test]
    fn decode_cm_v2() {
        let data = cm_v2_blob();
        assert!(print_panic_info_cm(&data, data[1], data[2]).is_some());
        // Must not panic, falls back to hex dump on unknown arch
        print_panic_info(&data);
        let mut unknown_arch = cm_v2_blob();
        unknown_arch[0] = 42;
        print_panic_info(&unknown_arch);
        print_panic_info(&[1, 2, 3]);
    }

    #[test]
    fn too_short_for_registers() {
        // Valid header/trailer but not enough space for Cortex-M registers
        let mut data = cm_v2_blob();
        data.truncate(50);
        assert!(print_panic_info_cm(&data, 2, 0).is_none());
    }
}
