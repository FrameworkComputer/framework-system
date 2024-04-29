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
#[cfg(target_os = "linux")]
pub mod nvme;
#[cfg(feature = "hidapi")]
pub mod touchpad;
#[cfg(feature = "hidapi")]
pub mod touchscreen;
#[cfg(all(feature = "hidapi", windows))]
pub mod touchscreen_win;
#[cfg(feature = "rusb")]
pub mod usbhub;

#[cfg(feature = "uefi")]
extern crate uefi;

// Override uefi crate's print!/println! macros with non-panicking versions.
// The uefi crate's versions call .expect() on write results, which crashes
// when the UEFI shell returns an error after 'q' is pressed during -b pagination.
#[cfg(feature = "uefi")]
macro_rules! print {
    ($($arg:tt)*) => ($crate::fw_uefi::_print_safe(core::format_args!($($arg)*)));
}
#[cfg(feature = "uefi")]
macro_rules! println {
    () => ($crate::fw_uefi::_print_safe(core::format_args!("\n")));
    ($($arg:tt)*) => ($crate::fw_uefi::_print_safe(
        core::format_args!("{}\n", core::format_args!($($arg)*))
    ));
}

pub mod capsule;
pub mod capsule_content;
pub mod ccgx;
pub mod chromium_ec;
pub mod commandline;
pub mod csme;
pub mod ec_binary;
pub mod esrt;
#[cfg(feature = "uefi")]
pub mod fw_uefi;
mod os_specific;
pub mod parade_retimer;
pub mod power;
pub mod serialnum;
pub mod smbios;
mod util;

pub mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}
