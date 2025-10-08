#![no_main]
#![no_std]

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::prelude::*;
#[allow(unused_imports)]
use uefi_services::{print, println};

extern crate alloc;

use framework_lib::commandline;

#[used]
#[link_section = ".sbat"]
pub static SBAT: [u8; 191] = *b"sbat,1,SBAT Version,sbat,1,https://github.com/rhboot/shim/blob/main/SBAT.md\nframework_tool,1,Framework Computer Inc,framework_tool,0.4.5,https://github.com/FrameworkComputer/framework-system\0";

#[entry]
fn main(image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&mut system_table).unwrap();
    let bs = system_table.boot_services();

    let args = commandline::uefi::get_args(bs, image_handle);
    let args = commandline::parse(&args);
    if commandline::run_with_args(&args, false) == 0 {
        return Status::SUCCESS;
    }

    // Force it go into UEFI shell
    Status::LOAD_ERROR
}
