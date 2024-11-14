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

#[derive(Default, Debug)]
struct Globals {
    device_path: String,
}

static GLOBAL_DATA: Lazy<RwLock<Globals>> = Lazy::new(|| RwLock::new(Globals::default()));
static GUID_GPIOBUTTONS_LAPTOPSLATE_INTERFACE: Uuid = uuid!("317fc439-3f77-41c8-b09e-08ad63272aa3");

fn check_tablet_mode() -> bool {
    // Switch
    // 1.
    // HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\ImmersiveShell\TabletMode
    //
    // 2.
    // https://stackoverflow.com/questions/31865120/enable-tablet-mode-on-windows-10-through-code
    //

    // Detect
    // 1.
    // https://devblogs.microsoft.com/oldnewthing/20160706-00/?p=93815
    // #[cfg(feature = "UI_ViewManagement")]
    // UIViewSettings
    // UserInteractionMode
    //
    // 2. Notification
    // WM_SETTINGCHANGE  with "ConvertibleSlateMode" or "UserInteractionMode"
    unsafe {
        // Either 0 or 1
        let res = GetSystemMetrics(SM_CONVERTIBLESLATEMODE);
        res == 0
    }
}

fn toggle_touchpad() {
    use enigo::{
        Direction::{Press, Release},
        Enigo, Key, Keyboard, Settings,
    };
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    enigo.key(Key::Control, Press).unwrap();
    enigo.key(Key::Meta, Press).unwrap();
    enigo.key(Key::F24, Press).unwrap();

    enigo.key(Key::Control, Release).unwrap();
    enigo.key(Key::Meta, Release).unwrap();
    enigo.key(Key::F24, Release).unwrap();
}

fn sync_touchpad() -> Result<(), Box<dyn Error>> {
    let tablet_mode = check_tablet_mode();
    let touchpad_enable = check_touchpad_enable()?;
    let touchpad_disable = !touchpad_enable;

    // In tablet mode, touchpad should be disabled
    // In laptop mode, touchpad should be enabled
    // If that's not the case, toggle touchpad enable
    if tablet_mode != touchpad_disable {
        toggle_touchpad();
    }

    Ok(())
}

// See https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/touchpad-enable-or-disable-toggle-button
// HKEY_LOCAL_MACHINE does not seem to exist
// reg query HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Status
// HKEY_CURRENT_USER exists and reflects the correct value
// reg query HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Status
fn check_touchpad_enable() -> Result<bool, Box<dyn Error>> {
    use winreg::enums::*;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_CURRENT_USER);
    let cur_ver = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad\\Status")?;
    let enabled: u32 = cur_ver.get_value("Enabled")?;
    Ok(enabled == 1)
}

fn main() -> Result<(), Box<dyn Error>> {
    let tablet_mode = check_tablet_mode();
    println!("Currently in tablet mode:   {:?}", tablet_mode);
    let touchpad_enable = check_touchpad_enable()?;
    println!("Touchpad currently enabled: {:?}", touchpad_enable);

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

fn toggle_tabletmode() -> Result<(), Box<dyn Error>> {
    get_device_path(&GUID_GPIOBUTTONS_LAPTOPSLATE_INTERFACE)?;

    let globals = GLOBAL_DATA.read()?;
    println!("DevicePath: {}", globals.device_path);
    let mut path_vec = globals.device_path.encode_utf16().collect::<Vec<_>>();
    drop(globals);

    let h_device: HANDLE;
    path_vec.push(0);
    let path = path_vec.as_ptr();

    // SAFETY:
    // Call Win32 API FFI CreateFileW to access driver
    unsafe {
        h_device = CreateFileW(
            path,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            0,
        );
    }

    // SAFETY:
    // Call Win32 API FFI GetLastError() to check for any errors
    unsafe {
        if h_device == INVALID_HANDLE_VALUE {
            return Err(format!("Failed to open device. Error {}", GetLastError()).into());
        }
    }

    // println!("Opened device successfully");

    write_tablet_mode(h_device)?;

    Ok(())
}

fn write_tablet_mode(h_device: HANDLE) -> Result<(), Box<dyn Error>> {
    let write_len: u32 = 1;
    let write_buffer: Vec<u8> = vec![0; write_len as usize];

    let mut bytes_returned: u32 = 0;

    // SAFETY:
    // Call Win32 API FFI WriteFile to write buffer to the driver
    let r = unsafe {
        WriteFile(
            h_device,
            write_buffer.as_ptr().cast(),
            u32::try_from(write_buffer.len()).unwrap(),
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };

    // SAFETY:
    // Call Win32 API FFI GetLastError() to check for any errors from WriteFile
    unsafe {
        if r == FALSE {
            return Err(format!(
                "PerformWriteReadTest: WriteFile failed: Error {}",
                GetLastError()
            )
            .into());
        }
    }

    if bytes_returned != write_len {
        return Err(format!(
            "bytes written is not test length! Written {bytes_returned}, SB {write_len}"
        )
        .into());
    }

    // println!("{bytes_returned} Pattern Bytes Written successfully");

    Ok(())
}

fn get_device_path(interface_guid: &Uuid) -> Result<(), Box<dyn Error>> {
    // println!("Looking for GUID: {interface_guid:?}");

    let mut guid = windows_sys::core::GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0, 0, 0, 0, 0, 0, 0, 0],
    };
    let guid_data4: &[u8; 8];
    let mut device_interface_list_length: u32 = 0;
    let mut config_ret;

    (guid.data1, guid.data2, guid.data3, guid_data4) = interface_guid.as_fields();
    guid.data4 = *guid_data4;

    // SAFETY:
    // Call Win32 API FFI CM_Get_Device_Interface_List_SizeW to determine size of
    // space needed for a subsequent request
    unsafe {
        config_ret = DeviceAndDriverInstallation::CM_Get_Device_Interface_List_SizeW(
            &mut device_interface_list_length,
            &guid,
            std::ptr::null(),
            DeviceAndDriverInstallation::CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        );
    }

    if config_ret != DeviceAndDriverInstallation::CR_SUCCESS {
        return Err(
            format!("Error 0x{config_ret:08X} retrieving device interface list size.",).into(),
        );
    }

    if device_interface_list_length <= 1 {
        return Err("Error: No active device interfaces found.  Is the driver loaded?".into());
    }

    let mut buffer: Vec<u16> = vec![0; usize::try_from(device_interface_list_length).unwrap()];
    let buffer_ptr = buffer.as_mut_ptr();

    // SAFETY:
    // Call Win32 API FFI CM_Get_Device_Interface_ListW to get the list of Device
    // Interfaces that match the Interface GUID for the echo driver
    unsafe {
        config_ret = DeviceAndDriverInstallation::CM_Get_Device_Interface_ListW(
            &guid,
            std::ptr::null(),
            buffer_ptr,
            device_interface_list_length,
            DeviceAndDriverInstallation::CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        );
    }

    if config_ret != DeviceAndDriverInstallation::CR_SUCCESS {
        return Err(format!("Error 0x{config_ret:08X} retrieving device interface list.").into());
    }

    let path = OsString::from_wide(buffer.as_slice());

    GLOBAL_DATA.write()?.device_path = path
        .into_string()
        .expect("Unable to convert Device Path to String");

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
