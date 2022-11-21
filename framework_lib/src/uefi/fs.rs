use std::uefi::status::Status;

use std::fs::{File, FileSystem};
use std::proto::Protocol;
use std::uefi::fs::FILE_MODE_READ;
use std::uefi::guid::{Guid, SHELL_GUID};
use std::uefi::status::{Error, Result};
use std::uefi::Handle;
use uefi_std::ffi::{nstr, wstr};

// TODO: Is actually void pointer. Opaque to the caller, so it doesn't matter
type ShellFileHandle = *mut u8;

#[repr(C)]
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

    pub GetFileInfo: extern "win64" fn() -> (),
    pub SetFileInfo: extern "win64" fn() -> (),
    pub OpenFileByName: extern "win64" fn(
        FileName: *const u16,
        FileHandle: *mut ShellFileHandle,
        OpenMode: u64,
    ) -> Status,
    pub CloseFile: extern "win64" fn() -> (),
    pub CreateFile: extern "win64" fn() -> (),
    pub ReadFile: extern "win64" fn(
        FileHandle: ShellFileHandle,
        ReadSize: *mut usize,
        Buffer: *mut u8,
    ) -> Status,
    pub WriteFile: extern "win64" fn() -> (),
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

    unsafe {
        buffer.set_len(buffer_size);
    }

    Some(buffer)
}
