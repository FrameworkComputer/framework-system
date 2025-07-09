//! Helper functions that need OS/platform specific implementations

#[cfg(not(feature = "uefi"))]
use std::{thread, time};

#[cfg(windows)]
use windows::{core::*, Win32::Foundation::*, Win32::System::WindowsProgramming::*};

/// Sleep a number of microseconds
pub fn sleep(micros: u64) {
    #[cfg(not(feature = "uefi"))]
    {
        let duration = time::Duration::from_micros(micros);
        thread::sleep(duration);
    }
    #[cfg(feature = "uefi")]
    {
        // TODO: It's not recommended to use this for sleep more than 10ms
        // Should use a one-shot timer event
        let st = unsafe { uefi_services::system_table().as_ref() };
        let bs = st.boot_services();
        bs.stall(micros as usize);
    }
}

pub const EFI_VARIABLE_NON_VOLATILE: u32 = 0x00000001;
pub const EFI_VARIABLE_BOOTSERVICE_ACCESS: u32 = 0x00000002;
pub const EFI_VARIABLE_RUNTIME_ACCESS: u32 = 0x00000004;
//pub const EFI_VARIABLE_HARDWARE_ERROR_RECORD: u32 = 0x00000008;
pub const EFI_VARIABLE_AUTHENTICATED_WRITE_ACCESS: u32 = 0x00000010;
//pub const EFI_VARIABLE_TIME_BASED_AUTHENTICATED_WRITE_ACCESS: u32 = 0x00000020;
pub const EFI_VARIABLE_APPEND_WRITE: u32 = 0x00000040;

#[cfg(windows)]
pub fn set_dbx(data: &[u8]) -> Option<()> {
    let attrs = EFI_VARIABLE_NON_VOLATILE
        | EFI_VARIABLE_BOOTSERVICE_ACCESS
        | EFI_VARIABLE_RUNTIME_ACCESS
        | EFI_VARIABLE_AUTHENTICATED_WRITE_ACCESS
        | EFI_VARIABLE_APPEND_WRITE;
    set_uefi_var("dbx", "d719b2cb-3d3a-4596-a3bc-dad00e67656f", data, attrs)
}

#[cfg(windows)]
pub fn get_dbx() -> Option<Vec<u8>> {
    get_uefi_var(w!("dbx"), w!("d719b2cb-3d3a-4596-a3bc-dad00e67656f"))
}

#[cfg(windows)]
pub fn set_uefi_var(name: &str, guid: &str, value: &[u8], attributes: u32) -> Option<()> {
    let res = unsafe {
        SetFirmwareEnvironmentVariableExW(
            // PCWSTR
            &HSTRING::from(name),
            // PCWSTR
            &HSTRING::from(guid),
            Some(value.as_ptr() as *const core::ffi::c_void),
            value.len() as u32,
            attributes,
        )
    };
    println!("{:?}", res);
    res.ok()
}

#[cfg(not(windows))]
pub fn set_uefi_var(name: &str, guid: &str, value: &[u8], attributes: u32) -> Option<()> {
    error!("Setting UEFI variable not supported on this OS");
    None
}

#[cfg(windows)]
pub fn get_uefi_var(name: PCWSTR, guid: PCWSTR) -> Option<Vec<u8>> {
    let mut data = [0; 1024];
    let mut attributes: u32 = 0;
    let (res, error) = unsafe {
        let res = GetFirmwareEnvironmentVariableExW(
            // PCWSTR
            //&HSTRING::from(name),
            name,
            // PCWSTR
            //&HSTRING::from(guid),
            guid,
            Some(data.as_mut_ptr() as *mut core::ffi::c_void),
            data.len() as u32,
            Some(&mut attributes),
        );
        let error = GetLastError();
        (res, error)
    };

    //let data = std::slice::from_raw_parts::<u8>(credentials_ptr as _, count as usize);

    println!("Res:       {:?}", res);
    println!("LastError: {:?}", error);
    println!("Data:      {:X?}", data);
    Some(vec![])
}

#[cfg(not(windows))]
pub fn get_uefi_var(name: &str, guid: &str, value: &[u8], attributes: u32) -> Option<Vec<u8>> {
    error!("Getting UEFI variable not supported on this OS");
    None
}
