/// Implementation to talk to DHowett's Windows Chrome EC driver
///
/// Does NOT work yet! Not sure why... I think I'm doing everything right.
#[allow(unused_imports)]
use windows::{
    core::*,
    w,
    Win32::Foundation::*,
    Win32::{
        Storage::FileSystem::*,
        System::{Ioctl::*, IO::*},
    },
};

// TODO: Wrap to make sure it's thread-safe
static mut DEVICE: Option<HANDLE> = None;

fn init() {
    unsafe {
        if DEVICE.is_none() {
            println!("Windows: Initializing device");
            let path = w!(r"\\.\GLOBALROOT\Device\CrosEC");
            //static wil::unique_hfile device;
            //device.reset(CreateFileW(r"\\.\GLOBALROOT\Device\CrosEC", GENERIC_READ | GENERIC_WRITE,
            //                            FILE_SHARE_READ | FILE_SHARE_WRITE, nullptr, OPEN_EXISTING, 0, nullptr));
            DEVICE = Some(
                CreateFileW(
                    path,
                    FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                )
                .unwrap(),
            );
        }
    }
}

#[cfg(target_family = "unix")]
pub fn read_memory(_offset: u16, _length: u16) -> Option<Vec<u8>> {
    None
}
#[cfg(target_os = "windows")]
pub fn read_memory(offset: u16, length: u16) -> Option<Vec<u8>> {
    println!("Windows read_memory_lpc implementation");
    init();
    let mut rm = CrosEcReadMem {
        offset: offset as u32,
        bytes: length as u32,
        buffer: [0_u8; CROSEC_MEMMAP_SIZE],
    };
    println!("Offset: {}", { rm.offset });
    println!("Bytes: {}", { rm.bytes });

    unsafe {
        //let const_ptr = &mut rm as *const ::core::ffi::c_void;
        let const_ptr = &mut rm as *const _ as *const ::core::ffi::c_void;
        let mut_ptr = &mut rm as *mut _ as *mut ::core::ffi::c_void;
        let ptr_size = std::mem::size_of::<CrosEcReadMem>() as u32;
        println!("ptr_size: {}", ptr_size);
        let retb: u32 = 0;
        DeviceIoControl(
            DEVICE,
            IOCTL_CROSEC_RDMEM,
            Some(const_ptr),
            ptr_size,
            Some(mut_ptr), // TODO: Not sure if this works
            ptr_size,
            Some(retb as *mut u32),
            None,
        )
        .unwrap();
        println!("retb: {}", retb);
        let output = &rm.buffer[..(length as usize)];
        return Some(output.to_vec());
    }
    // TODO
    None
}

#[cfg(target_family = "unix")]
pub fn send_command(_command: u16, _command_version: u8, _data: &[u8]) -> Option<Vec<u8>> {
    Some(vec![])
}
#[cfg(target_os = "windows")]
pub fn send_command(command: u16, command_version: u8, data: &[u8]) -> Option<Vec<u8>> {
    println!("Windows send_command_lpc_v3 implementation");
    init();

    //// Otherwise, run test mode
    let mut cmd = CrosEcCommand {
        command: command as u32,
        insize: data.len() as u32,
        outsize: 0,
        result: 0xFF,
        version: command_version as u32,
        buffer: [0_u8; CROSEC_CMD_MAX_REQUEST],
    };
    cmd.buffer[0..data.len()].clone_from_slice(data);
    println!("Sent bytes: {:?}", cmd.buffer);
    let size = std::mem::size_of::<CrosEcCommand>() + CROSEC_CMD_MAX_REQUEST;
    let const_ptr = &mut cmd as *const _ as *const ::core::ffi::c_void;
    let mut_ptr = &mut cmd as *mut _ as *mut ::core::ffi::c_void;
    let _ptr_size = std::mem::size_of::<CrosEcCommand>() as u32;

    let mut returned: u32 = 0;

    unsafe {
        DeviceIoControl(
            DEVICE,
            IOCTL_CROSEC_XCMD,
            Some(const_ptr),
            //Some(::core::mem::transmute_copy(&cmd)),
            size.try_into().unwrap(),
            Some(mut_ptr),
            size.try_into().unwrap(),
            Some(&mut returned as *mut u32),
            None,
        );
    }

    println!("OUT: {} | {}", {cmd.command}, {cmd.result});
    //DWORD retb{};
    //DeviceIoControl(device.get(), IOCTL_CROSEC_XCMD, cmd, (DWORD)size, cmd,
    //                                        (DWORD)size, &retb, nullptr);
    println!("IN: {} | {}", {cmd.command}, {cmd.result});
    ////std::cout << "IN_: " << (char*)(CROSEC_COMMAND_DATA(cmd)) << std::endl;

    println!("Returned bytes: {}", returned);
    Some(cmd.buffer.to_vec())
}

const CROSEC_CMD_MAX_REQUEST: usize = 0x100;
const CROSEC_CMD_MAX_RESPONSE: usize = 0x100;
const CROSEC_MEMMAP_SIZE: usize = 0xFF;

const FILE_DEVICE_CROS_EMBEDDED_CONTROLLER: u32 = 0x80EC;

const IOCTL_CROSEC_XCMD: u32 = (-2131959804 as i32) as u32;
const IOCTL_CROSEC_RDMEM: u32 = (-2131992568 as i32) as u32;
//const IOCTL_CROSEC_XCMD: u32 = ctl_code(
//    FILE_DEVICE_CROS_EMBEDDED_CONTROLLER,
//    0x801,
//    METHOD_BUFFERED,
//    FILE_READ_DATA.0 | FILE_WRITE_DATA.0,
//);
//const IOCTL_CROSEC_RDMEM: u32 = ctl_code(
//    FILE_DEVICE_CROS_EMBEDDED_CONTROLLER,
//    0x802,
//    METHOD_BUFFERED,
//    FILE_READ_ACCESS,
//);

//#define IOCTL_CROSEC_XCMD \
//	CTL_CODE(FILE_DEVICE_CROS_EMBEDDED_CONTROLLER, 0x801, METHOD_BUFFERED, FILE_READ_DATA | FILE_WRITE_DATA)
//#define IOCTL_CROSEC_RDMEM CTL_CODE(FILE_DEVICE_CROS_EMBEDDED_CONTROLLER, 0x802, METHOD_BUFFERED, FILE_READ_DATA)

/// Shadows CTL_CODE from microsoft headers
const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    ((device_type) << 16) + ((function) << 14) + ((method) << 2) + access
}

//const CROSEC_STATUS_IN_PROGRESS: NTSTATUS = NTSTATUS(0xE0EC0001);  // EC Command in progress
//const CROSEC_STATUS_UNAVAILABLE: NTSTATUS = NTSTATUS(0xE0EC0002);  // EC not available

#[repr(C)]
struct CrosEcReadMem {
    /// Offset in memory mapped region
    offset: u32,
    /// How many bytes to read
    bytes: u32,
    /// Buffer to receive requested bytes
    buffer: [u8; CROSEC_MEMMAP_SIZE],
}
#[repr(C)]
struct CrosEcCommand {
    /// Command version. Almost always 0
    version: u32,
    /// Command type
    command: u32,
    /// Size of request in bytes
    outsize: u32,
    /// Maximum response size in bytes
    insize: u32,
    /// Response status code
    result: u32,
    /// Request and response data buffer
    buffer: [u8; CROSEC_CMD_MAX_REQUEST],
}
