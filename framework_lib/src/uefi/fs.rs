use std::uefi::status::Status;

use std::proto::Protocol;
use std::uefi::fs::{FILE_MODE_CREATE, FILE_MODE_READ, FILE_MODE_WRITE};
use std::uefi::guid::{Guid, SHELL_GUID};
use std::uefi::Handle;
use uefi_std::ffi::wstr;

// TODO: Is actually void pointer. Opaque to the caller, so it doesn't matter
type ShellFileHandle = *mut u8;

///
/// EFI Time Abstraction:
///  Year:       1900 - 9999
///  Month:      1 - 12
///  Day:        1 - 31
///  Hour:       0 - 23
///  Minute:     0 - 59
///  Second:     0 - 59
///  Nanosecond: 0 - 999,999,999
///  TimeZone:   -1440 to 1440 or 2047
///
#[repr(C)]
#[allow(non_snake_case)] // To mirror the UEFI definition
pub struct EfiTime {
    Year: u16,
    Month: u8,
    Day: u8,
    Hour: u8,
    Minute: u8,
    Second: u8,
    Pad1: u8,
    Nanosecond: u32,
    TimeZone: u16,
    Daylight: u8,
    Pad2: u8,
}

#[repr(C)]
#[allow(non_snake_case)] // To mirror the UEFI definition
pub struct FileInfo {
    ///
    /// The size of the EFI_FILE_INFO structure, including the Null-terminated FileName string.
    ///
    Size: u64,
    ///
    /// The size of the file in bytes.
    ///
    FileSize: u64,
    ///
    /// PhysicalSize The amount of physical space the file consumes on the file system volume.
    ///
    PhysicalSize: u64,
    ///
    /// The time the file was created.
    ///
    CreateTime: EfiTime,
    ///
    /// The time when the file was last accessed.
    ///
    LastAccessTime: EfiTime,
    ///
    /// The time when the file's contents were last modified.
    ///
    ModificationTime: EfiTime,
    ///
    /// The attribute bits for the file.
    ///
    Attribute: u64,
    ///
    /// The Null-terminated name of the file.
    ///
    FileName: [u16; 0],
}

#[repr(C)]
#[allow(non_snake_case)] // To mirror the UEFI definition
pub struct UefiShell {
    pub Execute: extern "win64" fn(
        ImageHandle: &Handle,
        CommandLine: *const u16,
        Environment: *const *const u16,
        Status: *mut Status,
    ) -> Status,
    pub GetEnv: extern "win64" fn(FileSystemMapping: *const u16) -> (),
    // TODO: Specify correct function prototypes
    pub SetEnv: extern "win64" fn() -> (),
    pub GetAlias: extern "win64" fn() -> (),
    pub SetAlias: extern "win64" fn() -> (),
    pub GetHelpText: extern "win64" fn() -> (),
    pub GetDevicePathFromMap: extern "win64" fn() -> (),
    pub GetMapFromDevicePath: extern "win64" fn() -> (),
    pub GetDevicePathFromFilePath: extern "win64" fn() -> (),
    pub GetFilePathFromDevicePath: extern "win64" fn() -> (),
    pub SetMap: extern "win64" fn() -> (),

    pub GetCurDir: extern "win64" fn(FileSystemMapping: *const u16) -> Status,
    pub SetCurDir: extern "win64" fn(FileSystemMapping: *const u16) -> Status,

    pub OpenFileList: extern "win64" fn() -> (),
    pub FreeFileList: extern "win64" fn() -> (),
    pub RemoveDupInFileList: extern "win64" fn() -> (),
    pub BatchIsActive: extern "win64" fn() -> (),
    pub IsRootShell: extern "win64" fn() -> (),
    pub EnablePageBreak: extern "win64" fn() -> (),
    pub DisablePageBreak: extern "win64" fn() -> (),
    pub GetPageBreak: extern "win64" fn() -> (),
    pub GetDeviceName: extern "win64" fn() -> (),

    /// Caller needs to free the buffer!
    pub GetFileInfo: extern "win64" fn(FileHandle: ShellFileHandle) -> *const FileInfo,
    pub SetFileInfo: extern "win64" fn(FileHandle: ShellFileHandle, *const FileInfo) -> Status,
    pub OpenFileByName: extern "win64" fn(
        FileName: *const u16,
        FileHandle: *mut ShellFileHandle,
        OpenMode: u64,
    ) -> Status,
    pub CloseFile: extern "win64" fn(FileHandle: ShellFileHandle) -> (),
    pub CreateFile: extern "win64" fn() -> (),
    pub ReadFile: extern "win64" fn(
        FileHandle: ShellFileHandle,
        ReadSize: *mut usize,
        Buffer: *mut u8,
    ) -> Status,
    pub WriteFile: extern "win64" fn(
        FileHandle: ShellFileHandle,
        BufferSize: *mut usize,
        Buffer: *const u8,
    ) -> Status,
    pub DeleteFile: extern "win64" fn() -> (),
    pub DeleteFileByName: extern "win64" fn() -> (),
    pub GetFilePosition: extern "win64" fn() -> (),
    pub SetFilePosition: extern "win64" fn() -> (),
    pub FlushFile: extern "win64" fn() -> (),
    pub FindFiles: extern "win64" fn() -> (),
    pub FindFilesInDir: extern "win64" fn() -> (),
    pub GetFileSize: extern "win64" fn(FileHandle: ShellFileHandle, Size: *mut u64) -> Status,

    pub OpenRoot: extern "win64" fn() -> (),
    pub OpenRootByHandle: extern "win64" fn() -> (),

    // TODO: Is actually EFI_EVENT, not a function
    pub ExecutionBreak: extern "win64" fn() -> (),

    MajorVersion: u32,
    MinorVersion: u32,
    pub RegisterGuidName: extern "win64" fn() -> (),
    pub GetGuidName: extern "win64" fn() -> (),
    pub GetGuidFromName: extern "win64" fn() -> (),
    pub GetEnvEx: extern "win64" fn() -> (),
}

pub struct Shell(pub &'static mut UefiShell);

impl Protocol<UefiShell> for Shell {
    fn guid() -> Guid {
        SHELL_GUID
    }

    fn new(inner: &'static mut UefiShell) -> Self {
        Shell(inner)
    }
}

pub fn shell_read_file(path: &str) -> Option<Vec<u8>> {
    let shell = if let Ok(shell) = Shell::locate_protocol() {
        shell
    } else {
        println!("Failed to open Shell Protocol");
        return None;
    };

    debug_assert_eq!(shell.0.MajorVersion, 2);
    debug_assert_eq!(shell.0.MinorVersion, 2);

    let mut file_handle: ShellFileHandle = ::core::ptr::null_mut();
    let ret = (shell.0.OpenFileByName)(wstr(path).as_ptr(), &mut file_handle, FILE_MODE_READ);
    if ret.0 != 0 {
        println!("Failed to open file {}", path);
        return None;
    }

    let mut file_size: u64 = 0;
    let ret = (shell.0.GetFileSize)(file_handle, &mut file_size);
    if ret.0 != 0 {
        println!("Failed to get file size");
        return None;
    }

    let mut buffer: Vec<u8> = Vec::with_capacity(file_size as usize);
    let mut buffer_size = file_size as usize;
    let ret = (shell.0.ReadFile)(file_handle, &mut buffer_size, buffer.as_mut_ptr());
    if ret.0 != 0 {
        println!("Failed to read file");
        return None;
    }

    //  TODO: Make it auto-close using Rust destructors
    (shell.0.CloseFile)(file_handle);

    unsafe {
        buffer.set_len(buffer_size);
    }

    Some(buffer)
}

pub fn shell_write_file(path: &str, data: &[u8]) -> Status {
    let shell = if let Ok(shell) = Shell::locate_protocol() {
        shell
    } else {
        println!("Failed to open Shell Protocol");
        return Status(1);
    };

    debug_assert_eq!(shell.0.MajorVersion, 2);
    debug_assert_eq!(shell.0.MinorVersion, 2);

    let mut file_handle: ShellFileHandle = ::core::ptr::null_mut();
    let mode = FILE_MODE_READ | FILE_MODE_CREATE | FILE_MODE_WRITE;
    let ret = (shell.0.OpenFileByName)(wstr(path).as_ptr(), &mut file_handle, mode);
    if ret.0 != 0 {
        println!("Failed to open file {}", path);
        return ret;
    }

    // TODO: Free file_info buffer
    let file_info = (shell.0.GetFileInfo)(file_handle);
    if file_info.is_null() {
        println!("Failed to get file info");
        return ret;
    }

    // Not sure if it's useful to set FileInfo
    //let mut file_info = unsafe {
    //    &mut *(file_info as *mut FileInfo)
    //};
    //println!("file_info.Size: {}", file_info.Size);

    //if file_info.Size != 0 {
    //    file_info.Size = 0;
    //    let ret = (shell.0.SetFileInfo)(file_handle, file_info);
    //    if ret.0 != 0 {
    //        println!("Failed to set file info");
    //        return ret;
    //    }
    //}

    let mut buffer_size = data.len() as usize;
    let ret = (shell.0.WriteFile)(file_handle, &mut buffer_size, data.as_ptr());
    if ret.0 != 0 {
        println!("Failed to write file");
        return ret;
    }
    if buffer_size != data.len() {
        println!(
            "Failed to write whole buffer. Instead of {} wrote {} bytes.",
            data.len(),
            buffer_size
        );
        return Status(1);
    }

    (shell.0.CloseFile)(file_handle);

    Status(0)
}
