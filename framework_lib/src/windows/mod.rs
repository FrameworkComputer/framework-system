use std::{error::Error, ffi::OsString, os::windows::prelude::*, sync::RwLock};
use std::convert::TryFrom;

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

use enigo::{
    Direction::{Press, Release},
    Enigo, Key, Keyboard, Settings,
};

#[derive(Default, Debug)]
struct Globals {
    device_path: String,
}

static GLOBAL_DATA: Lazy<RwLock<Globals>> = Lazy::new(|| RwLock::new(Globals::default()));
static GUID_GPIOBUTTONS_LAPTOPSLATE_INTERFACE: Uuid = uuid!("317fc439-3f77-41c8-b09e-08ad63272aa3");

pub fn check_tablet_mode() -> bool {
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

pub fn toggle_touchpad() {
    let mut enigo = Enigo::new(&Settings::default()).unwrap();

    enigo.key(Key::Control, Press).unwrap();
    enigo.key(Key::Meta, Press).unwrap();
    enigo.key(Key::F24, Press).unwrap();

    enigo.key(Key::Control, Release).unwrap();
    enigo.key(Key::Meta, Release).unwrap();
    enigo.key(Key::F24, Release).unwrap();
}

pub fn sync_touchpad() -> Result<(), Box<dyn Error>>  {
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
pub fn check_touchpad_enable() -> Result<bool, Box<dyn Error>>  {
    use winreg::enums::*;
    use winreg::RegKey;
    let hklm = RegKey::predef(HKEY_CURRENT_USER);
    let cur_ver = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\PrecisionTouchPad\\Status")?;
    let enabled: u32 = cur_ver.get_value("Enabled")?;
    Ok(enabled == 1)
}

pub fn toggle_tabletmode() -> Result<(), Box<dyn Error>> {
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

    println!("Opened device successfully");

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
