#![no_main]
#![no_std]

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::prelude::*;
#[allow(unused_imports)]
use uefi::{print, println};

extern crate alloc;

use framework_lib::commandline;

/// Since Rust 1.99 (LLVM 23), loops that search for a NUL u16, like in
/// `CStr16::from_ptr`, are turned into calls to wcslen.
/// There's no libc on UEFI and compiler_builtins doesn't provide it.
///
/// # Safety
/// `s` must point to a NUL-terminated UCS-2 string
#[no_mangle]
pub unsafe extern "C" fn wcslen(s: *const u16) -> usize {
    let mut len = 0;
    // Volatile read, so LLVM can't turn this loop into a call to itself
    while core::ptr::read_volatile(s.add(len)) != 0 {
        len += 1;
    }
    len
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    let args = commandline::uefi::get_args();
    let args = commandline::parse(&args);
    if commandline::run_with_args(&args, false) == 0 {
        return Status::SUCCESS;
    }

    // Force it go into UEFI shell
    Status::LOAD_ERROR
}
