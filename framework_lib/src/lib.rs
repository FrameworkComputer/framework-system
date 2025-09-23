//! A library to interact with [Framework Computer](https://frame.work) hardware and building tools to do so.

#![cfg_attr(feature = "uefi", no_std)]
#![allow(clippy::uninlined_format_args)]

extern crate alloc;
#[cfg(feature = "uefi")]
extern crate no_std_compat as std; // TODO: I don't this should be necessary

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[cfg(feature = "rusb")]
pub mod audio_card;
#[cfg(feature = "rusb")]
pub mod camera;
#[cfg(feature = "rusb")]
pub mod inputmodule;
#[cfg(feature = "hidapi")]
pub mod touchpad;
#[cfg(feature = "hidapi")]
pub mod touchscreen;
#[cfg(all(feature = "hidapi", windows))]
pub mod touchscreen_win;
#[cfg(feature = "rusb")]
pub mod usbhub;

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
mod os_specific;
pub mod parade_retimer;
pub mod power;
pub mod smbios;
#[cfg(feature = "uefi")]
pub mod uefi;
mod util;

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
