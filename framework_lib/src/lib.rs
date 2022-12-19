//! A library to interact with [Framework Computer](https://frame.work) hardware and building tools to do so.

#![cfg_attr(feature = "uefi", no_std)]
#![feature(prelude_import)]

#[cfg(feature = "uefi")]
#[macro_use]
extern crate uefi_std as std;

#[cfg(feature = "uefi")]
#[allow(unused_imports)]
#[prelude_import]
use std::prelude::*;

#[cfg(feature = "uefi")]
extern crate alloc;

#[macro_use]
extern crate lazy_static;

pub mod capsule;
pub mod ccgx;
pub mod chromium_ec;
pub mod commandline;
pub mod csme;
pub mod ec_binary;
pub mod esrt;
mod os_specific;
pub mod power;
pub mod smbios;
#[cfg(feature = "uefi")]
pub mod uefi;
mod util;
