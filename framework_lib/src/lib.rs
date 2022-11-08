#![cfg_attr(feature = "uefi", no_std)]
#![feature(prelude_import)]

#[cfg(feature = "uefi")]
#[macro_use]
extern crate uefi_std as std;

#[cfg(feature = "uefi")]
#[allow(unused_imports)]
#[prelude_import]
use std::prelude::*;

pub mod chromium_ec;
#[cfg(not(feature = "uefi"))]
pub mod commandline;
pub mod ec_binary;
mod os_specific;
pub mod pd_binary;
pub mod power;
mod util;

//pub fn standalone_mode() -> bool {
//    // TODO: Figure out how to get that information
//    // For now just say we're in standalone mode when the battery is disconnected
//    let info = crate::power::power_info();
//    if let Some(i) = info {
//        i.battery.is_none()
//    } else {
//        // Default to true, when we can't find battery status, assume it's not there. Safe default.
//        true
//    }
//}
