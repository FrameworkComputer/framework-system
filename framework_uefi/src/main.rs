#![no_std]
#![no_main]
#![feature(prelude_import)]
#![feature(try_trait_v2)]
#![feature(control_flow_enum)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::many_single_char_names)]
#![allow(clippy::missing_safety_doc)]
#![allow(dead_code)]
#![allow(unused_variables)]

extern crate alloc;
//#[macro_use]
extern crate uefi_std as std;

#[allow(unused_imports)]
#[prelude_import]
use std::prelude::*;

use std::uefi::status::Status;

use framework_lib::commandline;

#[no_mangle]
pub extern "C" fn main() -> Status {
    let uefi = std::system_table();

    let args = commandline::parse(&commandline::uefi::get_args());
    commandline::run_with_args(&args);

    // If I don't return 1, we crash(?). Or I think it tries other boot options and they fail.
    // But if I return 1, then we land in UEFI shell and we can run the command manually.
    Status(1)
}
