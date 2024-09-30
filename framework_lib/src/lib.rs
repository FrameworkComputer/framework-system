//! A library to interact with [Framework Computer](https://frame.work) hardware and building tools to do so.

#![cfg_attr(feature = "uefi", no_std)]

extern crate alloc;
#[cfg(feature = "uefi")]
extern crate no_std_compat as std; // TODO: I don't this should be necessary

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[cfg(feature = "rusb")]
pub mod audio_card;

#[cfg(feature = "uefi")]
#[macro_use]
extern crate uefi_services;

pub mod capsule;
pub mod capsule_content;
pub mod ccgx;
pub mod chromium_ec;
pub mod commandline;
pub mod csme;
pub mod ec_binary;
pub mod esrt;
#[cfg(not(feature = "uefi"))]
pub mod guid;
mod os_specific;
pub mod power;
pub mod serialnum;
pub mod smbios;
#[cfg(feature = "uefi")]
pub mod uefi;
mod util;

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
