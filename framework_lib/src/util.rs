//! Miscellaneous utility functions to use across modules

use num::{Num, NumCast};
use std::prelude::v1::*;

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use spin::{Mutex, MutexGuard};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex, MutexGuard};

use crate::smbios;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Platform {
    /// Framework 13 - Intel 11th Gen, Codenamed TigerLake
    IntelGen11,
    /// Framework 13 - Intel 11th Gen, Codenamed AlderLake
    IntelGen12,
    /// Framework 13 - Intel 13th Gen, Codenamed RaptorLake
    IntelGen13,
    /// Framework 13 - AMD Ryzen
    Framework13Amd,
    /// Framework 16
    Framework16,
}

#[derive(Debug)]
pub struct Config {
    pub verbose: bool,
    pub platform: Platform,
}

impl Config {
    fn new() -> Self {
        Config {
            verbose: false,
            platform: Platform::IntelGen11,
        }
    }

    pub fn get() -> MutexGuard<'static, Option<Self>> {
        #[cfg(feature = "std")]
        let mut config = CONFIG.lock().unwrap();
        #[cfg(not(feature = "std"))]
        let mut config = CONFIG.lock();

        if (*config).is_none() {
            let mut cfg = Config::new();
            if let Some(platform) = smbios::get_platform() {
                // TODO: Perhaps add Qemu or NonFramework as a platform
                cfg.platform = platform;
            }
            *config = Some(cfg);
        }

        // TODO: See if we can map the Option::unwrap before returning
        assert!((*config).is_some());
        config
    }
}

lazy_static! {
    static ref CONFIG: Arc<Mutex<Option<Config>>> = Arc::new(Mutex::new(None));
}

/// Convert any type to a u8 slice (Like a C byte buffer)
pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    let len = ::std::mem::size_of::<T>();
    ::std::slice::from_raw_parts((p as *const T) as *const u8, len)
}

/// Convert any type to a mut u8 slice (Like a C byte buffer)
#[cfg(not(feature = "uefi"))]
pub unsafe fn any_as_mut_u8_slice<T: Sized>(p: &mut T) -> &mut [u8] {
    let len = ::std::mem::size_of::<T>();
    ::std::slice::from_raw_parts_mut((p as *mut T) as *mut u8, len)
}

/// Convert an array/slice of any type to a u8 slice (Like a C byte buffer)
pub unsafe fn any_vec_as_u8_slice<T: Sized>(p: &[T]) -> &[u8] {
    let len = ::std::mem::size_of::<T>() * p.len();
    ::std::slice::from_raw_parts((p.as_ptr() as *const T) as *const u8, len)
}

/// Print a byte buffer as a series of hex bytes
pub fn print_buffer(buffer: &[u8]) {
    for byte in buffer {
        print!("{:#X} ", byte);
    }
    println!();
}

fn print_chunk(buffer: &[u8], newline: bool) {
    for (i, byte) in buffer.iter().enumerate() {
        if i % 2 == 0 {
            print!(" ")
        }
        print!("{:02x}", byte);
    }
    if newline {
        println!();
    }
}

// Example:
// Input: [0x00; 0x16]
// Output: ................
// Input: [a000 0036 626e 0300 c511 8035 0000 0000]
// Output: ...6bn.....5....
fn print_ascii_buffer(buffer: &[u8], newline: bool) {
    for byte in buffer {
        // If printable, print, else display a dot
        if *byte >= 32 && *byte <= 127 {
            print!("{}", *byte as char);
        } else {
            print!(".")
        }
    }
    if newline {
        println!();
    }
}

/// Print a big byte buffer
///
/// Because it's long it'll be printed in several lines, each 16 bytes
///
/// Example
///
/// print_multiline_buffer(&[0xa0, 0x00, 0x00, 0x36, 0x62, 0x6e, 0x03, 0x00, 0xc5, 0x11, 0x80, 0x35, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], 0x2000)
/// Output:
/// 00002000: a000 0036 626e 0300 c511 8035 0000 0000  ...6bn.....5....
/// 00002010: 0000 0000 0000 0000 0000 0000 0000 00    ................
pub fn print_multiline_buffer(buffer: &[u8], offset: usize) {
    let chunk_size = 16;
    for (i, chunk) in buffer.chunks(chunk_size).enumerate() {
        print!("{:08x}:", offset + i * chunk_size);
        print_chunk(chunk, false);

        // Make sure ASCII section aligns, even if less than 16 byte chunks
        if chunk.len() < 16 {
            let byte_padding = 16 - chunk.len();
            let space_padding = byte_padding / 2;
            let padding = byte_padding * 2 + space_padding;
            print!("{}", " ".repeat(padding));
        }
        print!("  ");

        print_ascii_buffer(chunk, true);
    }
}

/// Find a sequence of bytes in a long slice of bytes
pub fn find_sequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Assert length of an EC response from the windows driver
/// It's always 20 more than expected. TODO: Figure out why
pub fn assert_win_len<N: Num + std::fmt::Debug + Ord + NumCast + Copy>(left: N, right: N) {
    #[cfg(feature = "win_driver")]
    assert_eq!(left, right + NumCast::from(20).unwrap());
    #[cfg(not(feature = "win_driver"))]
    assert_eq!(left, right);
}

pub fn print_buffer_short(buffer: &[u8]) {
    for byte in buffer {
        print!("{:02x}", byte);
    }
    println!();
}
