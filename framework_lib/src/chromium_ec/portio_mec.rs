use crate::util;
use alloc::vec;
use alloc::vec::Vec;

use log::Level;

use hwio::{Io, Pio};
#[cfg(target_os = "linux")]
use libc::ioperm;

// I/O addresses for host command
#[cfg(target_os = "linux")]
const EC_LPC_ADDR_HOST_DATA: u16 = 0x200;

const MEC_EC_BYTE_ACCESS: u16 = 0x00;
const MEC_EC_LONG_ACCESS_AUTOINCREMENT: u16 = 0x03;

const MEC_LPC_ADDRESS_REGISTER0: u16 = 0x0802;
const _MEC_LPC_ADDRESS_REGISTER1: u16 = 0x0803;
const MEC_LPC_DATA_REGISTER0: u16 = 0x0804;
const _MEC_LPC_DATA_REGISTER1: u16 = 0x0805;
const MEC_LPC_DATA_REGISTER2: u16 = 0x0806;
const _MEC_LPC_DATA_REGISTER3: u16 = 0x0807;

pub fn init() -> bool {
    #[cfg(target_os = "linux")]
    unsafe {
        let res = ioperm(EC_LPC_ADDR_HOST_DATA as u64, 8, 1);
        if res != 0 {
            error!("ioperm failed. portio driver is likely block by Linux kernel lockdown mode");
            return false;
        }
        let res = ioperm(MEC_LPC_ADDRESS_REGISTER0 as u64, 10, 1);
        assert_eq!(res, 0);
    }

    true
}

// TODO: Create a wrapper
// TODO: Deduplicate this with transfer_read_mec
/// Transfer write function for MEC (Microchip) based embedded controllers
pub fn transfer_write(buffer: &[u8]) {
    let size: u16 = buffer.len().try_into().unwrap();
    let mut pos: u16 = 0;
    let mut offset = 0;

    if log_enabled!(Level::Trace) {
        println!("transfer_write_mec(size={:#X}, buffer=)", size);
        util::print_multiline_buffer(buffer, 0);
    }

    // Unaligned start address
    // Read up two three bytes one-by-one
    if offset % 4 > 0 {
        if log_enabled!(Level::Trace) {
            trace!("  Writing single byte to start at {:#X}", offset);
        }
        Pio::<u16>::new(MEC_LPC_ADDRESS_REGISTER0).write((offset & 0xFFFC) | MEC_EC_BYTE_ACCESS);
        if log_enabled!(Level::Trace) {
            trace!(
                "Writing {:#X} to port {:#X}",
                (offset & 0xFFFC) | MEC_EC_BYTE_ACCESS,
                MEC_LPC_ADDRESS_REGISTER0
            );
        }

        for _byte in (offset % 4)..4 {
            Pio::<u8>::new(MEC_LPC_DATA_REGISTER0).write(buffer[usize::from(pos)]);
            pos += 1;
        }
        offset = (offset + 4) & 0xFFFC; // Closest 4 byte alignment
    }

    // Reading in 4 byte chunks
    if size - pos >= 4 {
        if log_enabled!(Level::Trace) {
            trace!("  Writing 4 bytes to {:#X}", offset);
        }
        Pio::<u16>::new(MEC_LPC_ADDRESS_REGISTER0)
            .write((offset & 0xFFFC) | MEC_EC_LONG_ACCESS_AUTOINCREMENT);
        if log_enabled!(Level::Trace) {
            trace!(
                "Writing {:#X} to port {:#X}",
                (offset & 0xFFFC) | MEC_EC_LONG_ACCESS_AUTOINCREMENT,
                MEC_LPC_ADDRESS_REGISTER0
            );
        }
        let mut temp: [u16; 2] = [0; 2];
        while size - pos >= 4 {
            unsafe {
                temp.copy_from_slice(
                    buffer[usize::from(pos)..usize::from(pos + 4)]
                        .align_to::<u16>()
                        .1,
                )
            }
            if log_enabled!(Level::Trace) {
                trace!("  Sending: {:#X} {:#X}", temp[0], temp[1]);
            }
            Pio::<u16>::new(MEC_LPC_DATA_REGISTER0).write(temp[0]);
            Pio::<u16>::new(MEC_LPC_DATA_REGISTER2).write(temp[1]);

            pos += 4;
            offset += 4;
        }
    }

    // Read last remaining bytes individually
    if size - pos > 0 {
        if log_enabled!(Level::Trace) {
            trace!("  Writing single byte to end at {:#X}", offset);
        }
        Pio::<u16>::new(MEC_LPC_ADDRESS_REGISTER0).write((offset & 0xFFFC) | MEC_EC_BYTE_ACCESS);

        for byte in 0..(size - pos) {
            Pio::<u8>::new(MEC_LPC_DATA_REGISTER0 + byte).write(buffer[usize::from(pos + byte)]);
        }
    }
}

/// Transfer read function for MEC (Microchip) based embedded controllers
pub fn transfer_read(address: u16, size: u16) -> Vec<u8> {
    trace!(
        "transfer_read_mec(address={:#X}, size={:#X})",
        address,
        size
    );

    // Allocate buffer to hold result
    let mut buffer = vec![0_u8; size.into()];
    let mut pos: u16 = 0;
    let mut offset = address;

    // Unaligned start address
    // Read up two three bytes one-by-one
    if offset % 4 > 0 {
        if log_enabled!(Level::Trace) {
            trace!("  Reading single byte from start at {:#X}", offset);
        }
        Pio::<u16>::new(MEC_LPC_ADDRESS_REGISTER0).write((offset & 0xFFFC) | MEC_EC_BYTE_ACCESS);

        for byte in (offset % 4)..std::cmp::min(4, size) {
            buffer[usize::from(pos)] = Pio::<u8>::new(MEC_LPC_DATA_REGISTER0 + byte).read();
            if log_enabled!(Level::Trace) {
                trace!("  Received: {:#X}", buffer[usize::from(pos)]);
            }
            pos += 1;
        }
        offset = (offset + 4) & 0xFFFC; // Closest 4 byte alignment
    }

    // Reading in 4 byte chunks
    if size - pos >= 4 {
        if log_enabled!(Level::Trace) {
            trace!("  Reading 4 bytes from {:#X}", offset);
        }
        Pio::<u16>::new(MEC_LPC_ADDRESS_REGISTER0)
            .write((offset & 0xFFFC) | MEC_EC_LONG_ACCESS_AUTOINCREMENT);
        let mut temp: [u16; 2] = [0; 2];
        while size - pos >= 4 {
            temp[0] = Pio::<u16>::new(MEC_LPC_DATA_REGISTER0).read();
            temp[1] = Pio::<u16>::new(MEC_LPC_DATA_REGISTER2).read();
            trace!("  Received: {:#X} {:#X}", temp[0], temp[1]);
            let aligned = unsafe { temp.align_to::<u8>() };
            assert!(aligned.0.is_empty());
            assert!(aligned.2.is_empty());
            buffer[usize::from(pos)..usize::from(pos + 4)].copy_from_slice(aligned.1);

            pos += 4;
            offset += 4;
        }
    }

    // Read last remaining bytes individually
    if size - pos > 0 {
        if log_enabled!(Level::Trace) {
            trace!("  Reading single byte from end at {:#X}", offset);
        }
        Pio::<u16>::new(MEC_LPC_ADDRESS_REGISTER0).write((offset & 0xFFFC) | MEC_EC_BYTE_ACCESS);

        for byte in 0..(size - pos) {
            buffer[usize::from(pos + byte)] = Pio::<u8>::new(MEC_LPC_DATA_REGISTER0 + byte).read();
            if log_enabled!(Level::Trace) {
                trace!("  Received: {:#X}", buffer[usize::from(pos + byte)]);
            }
        }
    }

    if log_enabled!(Level::Trace) {
        println!("Read bytes: ");
        util::print_multiline_buffer(&buffer, (address) as usize)
    }

    buffer
}
