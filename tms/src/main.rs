// Copyright (c) Microsoft Corporation
// Copyright (c) Framework Computer Inc
// License: MIT OR Apache-2.0

//! Laptop/Slate mode switcher
//! Based on: https://learn.microsoft.com/en-us/windows-hardware/drivers/gpiobtn/laptop-slate-mode-toggling-between-states?redirectedfrom=MSDN
//! Depends on this ACPI code: https://learn.microsoft.com/en-us/windows-hardware/drivers/gpiobtn/acpi-descriptor-samples#acpi-description-for-laptopslate-mode-indicator
//! In order to load the Microsoft "GPIO Laptop or Slate Indicator Driver"
//! Code based off of https://github.com/microsoft/Windows-rust-driver-samples/blob/main/general/echo/kmdf/exe/src/main.rs
//!
//! Changing tablet mode requires admin
//! Changing touchpad enable does not require admin
//! Checking either, does not requireadmin
//!
//! Usage:
//! tms.exe           - Toggle mode
//! tms.exe on        - Enable tablet mode
//! tms.exe off       - Disable tablet mode
//! tms.exe tp-toggle - Toggle touchpad enable
//! tms.exe tp-sync   - Sync touchpad with tablet mode
//! tms.exe tp-on     - Toggle touchpad on
//! tms.exe tp-off    - Toggle touchpad off
//! tms.exe tp-check  - Check current touchpad enable

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::nursery)]
#![deny(clippy::cargo)]
#![deny(clippy::multiple_unsafe_ops_per_block)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::unnecessary_safety_doc)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(rustdoc::missing_crate_level_docs)]
#![deny(rustdoc::invalid_codeblock_attributes)]
#![deny(rustdoc::invalid_html_tags)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::bare_urls)]
#![deny(rustdoc::unescaped_backticks)]
#![deny(rustdoc::redundant_explicit_links)]

use std::{env, error::Error, ffi::OsString, os::windows::prelude::*, sync::RwLock};

use once_cell::sync::Lazy;
use uuid::{uuid, Uuid};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CONVERTIBLESLATEMODE};
use windows_sys::Win32::{
    Devices::DeviceAndDriverInstallation,
    Foundation::{GetLastError, FALSE, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, WriteFile, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    },
};

use framework_lib::windows::*;
use framework_lib::smbios::{self, Platform};
use framework_lib::chromium_ec::{CrosEc, EcResult};

fn print_framework12_gpios() -> EcResult<()> {
    if let Some(Platform::Framework12) = smbios::get_platform() {
        let gpios = [
            "chassis_open_l",
            "lid_sw_l",
            "tablet_mode_l",
        ];

        let ec = CrosEc::new();
        println!("GPIO State");
        for gpio in gpios {
            println!("  {:<25} {}", gpio, ec.get_gpio(gpio)?);
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let tablet_mode = check_tablet_mode();
    println!("Currently in tablet mode:   {:?}", tablet_mode);
    let touchpad_enable = check_touchpad_enable()?;
    println!("Touchpad currently enabled: {:?}", touchpad_enable);

    print_framework12_gpios().unwrap();

    let args: Vec<_> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "on" => {
                if tablet_mode {
                    return Ok(());
                }
            }
            "off" => {
                if !tablet_mode {
                    return Ok(());
                }
            }
            "check" => {
                // Just checking, not switching
                return Ok(());
            }
            "tp-on" => {
                if !touchpad_enable {
                    toggle_touchpad();
                }
                return Ok(());
            }
            "tp-off" => {
                if touchpad_enable {
                    toggle_touchpad();
                }
                return Ok(());
            }
            "tp-toggle" => {
                toggle_touchpad();
                return Ok(());
            }
            "tp-sync" => {
                sync_touchpad()?;
                return Ok(());
            }
            "tp-check" => {
                // Just checking, not switching
                return Ok(());
            }
            "watch" => {
                watch()?;
                return Ok(());
            }
            _ => {
                println!("Usage:");
                println!("  {}           - Toggle mode", args[0]);
                println!("  {} on        - Enable tablet mode", args[0]);
                println!("  {} off       - Disable tablet mode", args[0]);
                println!("  {} check     - Check current mode", args[0]);
                println!("  {} tp-toggle - Toggle touchpad enable", args[0]);
                println!("  {} tp-sync   - Sync touchpad with tablet mode", args[0]);
                println!("  {} tp-on     - Toggle touchpad on", args[0]);
                println!("  {} tp-off    - Toggle touchpad off", args[0]);
                println!("  {} tp-check  - Check current touchpad enable", args[0]);
                return Ok(());
            }
        }
    }

    toggle_tabletmode()?;

    Ok(())
}

use windows::{
    core::s, Win32::Foundation::*, Win32::Graphics::Gdi::ValidateRect,
    Win32::System::LibraryLoader::GetModuleHandleA, Win32::UI::WindowsAndMessaging::*,
};

fn watch() -> Result<(), Box<dyn Error>> {
    unsafe {
        let instance = GetModuleHandleA(None)?;
        let window_class = s!("window");

        let wc = WNDCLASSA {
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hInstance: instance.into(),
            lpszClassName: window_class,

            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            ..Default::default()
        };

        let atom = RegisterClassA(&wc);
        debug_assert!(atom != 0);

        let window = CreateWindowExA(
            WINDOW_EX_STYLE::default(),
            window_class,
            s!("This is a sample window"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            instance,
            None,
        )?;

        let mut message = MSG::default();

        while GetMessageA(&mut message, None, 0, 0).into() {
            DispatchMessageA(&message);
        }

        Ok(())
    }
}

use std::ffi::CStr;
extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_SETTINGCHANGE => {
                // lparam contains a pointer to a string
                if lparam.0 != 0 {
                    let c_str: &CStr = CStr::from_ptr(lparam.0 as *const i8);
                    let lparam_str: &str = c_str.to_str().unwrap();

                    if lparam_str == "ConvertibleSlateMode" {
                        // Ignore error
                        let _ = sync_touchpad();

                        // Not necessary, but need to run as admin
                        // SetForegroundWindow(window);
                    }
                }
                LRESULT(0)
            }
            _ => DefWindowProcA(window, message, wparam, lparam),
        }
    }
}
