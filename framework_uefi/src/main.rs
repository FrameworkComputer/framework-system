#![no_main]
#![no_std]

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::prelude::*;
#[allow(unused_imports)]
use uefi::{print, println};

extern crate alloc;

use framework_lib::commandline;

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
