//! Miscellaneous utility functions to use across modules

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use spin::{Mutex, MutexGuard};
#[cfg(feature = "std")]
use std::sync::{Arc, Mutex, MutexGuard};

use crate::smbios;

// TODO: Allow to dynamically change this. For example with a --verbose flag
#[cfg(debug_assertions)]
const DBG: bool = false; // Usually it's too verbose even for debugging
#[cfg(not(debug_assertions))]
const DBG: bool = false;

/// Whether debug mode is enabled. Is mostly used for extremly verbose debug prints
pub fn is_debug() -> bool {
    DBG
}

#[derive(Debug, PartialEq)]
pub enum Platform {
    /// Intel 11th Gen, Codenamed TigerLake
    IntelGen11,
    /// Intel 11th Gen, Codenamed AlderLake
    IntelGen12,
    /// Intel 13th Gen, Codenamed RaptorLake
    IntelGen13,
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
