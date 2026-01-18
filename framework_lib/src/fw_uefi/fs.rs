use alloc::vec;
use alloc::vec::Vec;
use uefi::prelude::*;
use uefi::proto::shell::Shell;
use uefi_raw::protocol::shell::ShellProtocol;
//use uefi::proto::shell::FileOpenMode;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use uefi::Result;

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct ShellFileHandle(NonNull<c_void>);

const FILE_MODE_READ: u64 = 0x0000000000000001;
const FILE_MODE_WRITE: u64 = 0x0000000000000002;
const FILE_MODE_CREATE: u64 = 0x8000000000000000;

pub fn wstr(string: &str) -> Vec<u16> {
    let mut wstring = vec![];

    for c in string.chars() {
        wstring.push(c as u16);
    }
    wstring.push(0);

    wstring
}

pub fn shell_read_file(path: &str) -> Option<Vec<u8>> {
    let handle = boot::get_handle_for_protocol::<Shell>().expect("No Shell handles");
    let mut shell =
        boot::open_protocol_exclusive::<Shell>(handle).expect("Failed to open Shell protocol");
    let shell = unsafe {
        let proto: &ShellProtocol = std::mem::transmute(shell.get().unwrap());
        proto
    };

    println!("Opened shell protocol");

    debug_assert_eq!(shell.major_version, 2);
    debug_assert_eq!(shell.minor_version, 2);

    println!(
        "Shell protocol ver: {}.{}",
        shell.major_version, shell.minor_version
    );

    unsafe {
        let c_path = wstr(path);
        let mut mode = FILE_MODE_READ;
        let mut handle: MaybeUninit<*const c_void> = MaybeUninit::zeroed();
        (shell.open_file_by_name)(c_path.as_ptr(), handle.as_mut_ptr().cast(), mode);

        println!("Opened file");

        let file_handle = handle.assume_init();

        let mut file_size = 0;
        println!("get_file_size");
        let res = (shell.get_file_size)(file_handle, &mut file_size);
        // let file_size = res.unwrap();

        let mut buffer: Vec<u8> = vec![0; file_size as usize];
        let mut read_size = file_size as usize;
        println!("read_file {} bytes", file_size);
        (shell.read_file)(
            file_handle,
            &mut read_size,
            buffer.as_mut_ptr() as *mut c_void,
        );

        println!("close_file");

        //  TODO: Make it auto-close using Rust destructors
        (shell.close_file)(file_handle);

        println!("Done");

        Some(buffer)
    }
}

pub fn shell_write_file(path: &str, data: &[u8]) -> Result {
    let handle = boot::get_handle_for_protocol::<Shell>().expect("No Shell handles");
    let mut shell =
        boot::open_protocol_exclusive::<Shell>(handle).expect("Failed to open Shell protocol");
    let shell = unsafe {
        let proto: &ShellProtocol = std::mem::transmute(shell.get().unwrap());
        proto
    };

    debug_assert_eq!(shell.major_version, 2);
    debug_assert_eq!(shell.minor_version, 2);

    unsafe {
        // let mode = FileOpenMode::Read as u64 + FileOpenMode::Write as u64 + FileOpenMode::Create as u64;
        let mode = FILE_MODE_READ + FILE_MODE_WRITE + FILE_MODE_CREATE;
        let c_path = wstr(path);
        let mut handle: MaybeUninit<*const c_void> = MaybeUninit::zeroed();
        (shell.open_file_by_name)(c_path.as_ptr(), handle.as_mut_ptr().cast(), mode);
        let file_handle = handle.assume_init();

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

        let mut read_size = data.len();
        (shell.write_file)(file_handle, &mut read_size, data.as_ptr() as *mut c_void);

        (shell.close_file)(file_handle);

        Status::SUCCESS.to_result()
    }
}
