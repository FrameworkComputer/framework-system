#![no_main]
#![no_std]

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::prelude::*;
#[allow(unused_imports)]
use uefi_services::{print, println};

extern crate alloc;

use framework_lib::commandline;

#[entry]
fn main(_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();

    let args = commandline::uefi::get_args(bs);
    let args = commandline::parse(&args);
    if commandline::run_with_args(&args, false) == 0 {
        return Status::SUCCESS;
    }

    // Force it go into UEFI shell
    Status::LOAD_ERROR
}
