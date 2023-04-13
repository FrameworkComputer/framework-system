use alloc::vec;
use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::shell::FileOpenMode;
use uefi::Result;

use super::find_shell_handle;

pub fn wstr(string: &str) -> Vec<u16> {
    let mut wstring = vec![];

    for c in string.chars() {
        wstring.push(c as u16);
    }
    wstring.push(0);

    wstring
}

pub fn shell_read_file(path: &str) -> Option<Vec<u8>> {
    let shell = if let Some(shell) = find_shell_handle() {
        shell
    } else {
        println!("Failed to open Shell Protocol");
        return None;
    };

    debug_assert_eq!(shell.major_version, 2);
    debug_assert_eq!(shell.minor_version, 2);

    let c_path = wstr(path);
    let handle = shell.open_file_by_name(c_path.as_slice(), FileOpenMode::Read as u64);

    let handle = if let Ok(handle) = handle {
        handle
    } else {
        println!("Failed to open file: {:?}", handle);
        return None;
    };

    let handle = if let Some(handle) = handle {
        handle
    } else {
        println!("Failed to open file: {:?}", handle);
        return None;
    };
    let file_handle = handle;

    let res = shell.get_file_size(file_handle);
    let file_size = res.unwrap();

    let mut buffer: Vec<u8> = vec![0; file_size as usize];
    let res = shell.read_file(file_handle, &mut buffer);
    res.unwrap();

    //  TODO: Make it auto-close using Rust destructors
    shell.close_file(file_handle).unwrap();

    Some(buffer)
}

pub fn shell_write_file(path: &str, data: &[u8]) -> Result {
    let shell = if let Some(shell) = find_shell_handle() {
        shell
    } else {
        println!("Failed to open Shell Protocol");
        return Status::LOAD_ERROR.into();
    };

    debug_assert_eq!(shell.major_version, 2);
    debug_assert_eq!(shell.minor_version, 2);

    let mode = FileOpenMode::Read as u64 + FileOpenMode::Write as u64 + FileOpenMode::Create as u64;
    let c_path = wstr(path);
    let handle = shell.open_file_by_name(c_path.as_slice(), mode);
    let handle = if let Ok(handle) = handle {
        handle
    } else {
        println!("Failed to open file: {:?}", handle);
        return Status::LOAD_ERROR.into();
    };
    let handle = if let Some(handle) = handle {
        handle
    } else {
        println!("Failed to open file: {:?}", handle);
        return Status::LOAD_ERROR.into();
    };
    let file_handle = handle;

    //// TODO: Free file_info buffer
    //let file_info = (shell.0.GetFileInfo)(file_handle);
    //if file_info.is_null() {
    //    println!("Failed to get file info");
    //    return ret;
    //}

    //// Not sure if it's useful to set FileInfo
    ////let mut file_info = unsafe {
    ////    &mut *(file_info as *mut FileInfo)
    ////};
    ////println!("file_info.Size: {}", file_info.Size);

    ////if file_info.Size != 0 {
    ////    file_info.Size = 0;
    ////    let ret = (shell.0.SetFileInfo)(file_handle, file_info);
    ////    if ret.0 != 0 {
    ////        println!("Failed to set file info");
    ////        return ret;
    ////    }
    ////}

    //let mut buffer_size = data.len() as usize;
    //let ret = (shell.0.WriteFile)(file_handle, &mut buffer_size, data.as_ptr());
    //if ret.0 != 0 {
    //    println!("Failed to write file");
    //    return ret;
    //}
    //if buffer_size != data.len() {
    //    println!(
    //        "Failed to write whole buffer. Instead of {} wrote {} bytes.",
    //        data.len(),
    //        buffer_size
    //    );
    //    return Status(1);
    //}

    shell.write_file(file_handle, data).unwrap();

    shell.close_file(file_handle).unwrap();

    Status::SUCCESS.into()
}
