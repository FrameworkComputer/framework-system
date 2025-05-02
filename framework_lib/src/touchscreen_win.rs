use hidapi::HidApi;
use std::path::Path;

use crate::touchscreen::{TouchScreen, ILI_PID, ILI_VID};
#[allow(unused_imports)]
use windows::{
    core::*,
    Win32::{
        Devices::HumanInterfaceDevice::*,
        Devices::Properties::*,
        Foundation::*,
        Storage::FileSystem::*,
        System::Threading::ResetEvent,
        System::IO::{CancelIoEx, DeviceIoControl},
        System::{Ioctl::*, IO::*},
    },
};

const REPORT_ID_FIRMWARE: u8 = 0x27;
const REPORT_ID_USI_VER: u8 = 0x28;

pub struct NativeWinTouchScreen {
    handle: HANDLE,
}

impl TouchScreen for NativeWinTouchScreen {
    fn open_device(target_up: u16, skip: u8) -> Option<Self> {
        debug!(
            "Looking for touchscreen HID device {:X} {}",
            target_up, skip
        );
        let mut skip = skip;
        match HidApi::new() {
            Ok(api) => {
                for dev_info in api.device_list() {
                    let vid = dev_info.vendor_id();
                    let pid = dev_info.product_id();
                    let usage_page = dev_info.usage_page();
                    if vid != ILI_VID {
                        trace!("    Skipping VID:PID. Expected {:04X}:*", ILI_VID);
                        continue;
                    }
                    debug!(
                        "  Found {:04X}:{:04X} (Usage Page {:04X})",
                        vid, pid, usage_page
                    );
                    if usage_page != target_up {
                        debug!("    Skipping usage page. Expected {:04X}", 0xFF00);
                        continue;
                    }
                    if pid != ILI_PID {
                        debug!("  Warning: PID is {:04X}, expected {:04X}", pid, ILI_PID);
                    }

                    debug!("  Found matching touchscreen HID device");
                    debug!("  Path:             {:?}", dev_info.path());
                    debug!("  IC Type:          {:04X}", pid);
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }

                    // TODO: Enumerate with windows
                    // Should enumerate and find the right one
                    // See: https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/finding-and-opening-a-hid-collection
                    let path = dev_info.path().to_str().unwrap();

                    let res = unsafe {
                        CreateFileW(
                            &HSTRING::from(Path::new(path)),
                            FILE_GENERIC_WRITE.0 | FILE_GENERIC_READ.0,
                            FILE_SHARE_READ | FILE_SHARE_WRITE,
                            None,
                            OPEN_EXISTING,
                            // hidapi-rs is using FILE_FLAG_OVERLAPPED but it doesn't look like we need that
                            FILE_FLAGS_AND_ATTRIBUTES(0),
                            None,
                        )
                    };
                    let handle = match res {
                        Ok(h) => h,
                        Err(err) => {
                            error!("Failed to open device {:?}", err);
                            return None;
                        }
                    };

                    debug!("Opened {:?}", path);

                    return Some(NativeWinTouchScreen { handle });
                }
            }
            Err(e) => {
                error!("Failed to open hidapi. Error: {e}");
            }
        };

        None
    }

    fn send_message(&self, message_id: u8, read_len: usize, data: Vec<u8>) -> Option<Vec<u8>> {
        let report_id = 0x03;
        let data_len = data.len();
        let mut msg = [0u8; 0x40];
        let msg_len = 3 + data_len;
        msg[0] = report_id;
        msg[1] = 0xA3;
        msg[2] = data_len as u8;
        msg[3] = read_len as u8;
        msg[4] = message_id;
        for (i, b) in data.into_iter().enumerate() {
            msg[5 + i] = b;
        }

        let mut buf = [0u8; 0x40];
        buf[0] = report_id;

        unsafe {
            debug!("  HidD_SetOutputReport {:X?}", msg);
            let success = HidD_SetOutputReport(
                self.handle,
                // Microsoft docs says that the first byte of the message has to be the report ID.
                // This is normal with HID implementations.
                // But it seems on Windows (at least for this device's firmware) we have to set the
                // length as one more than the buffer is long.
                // Otherwise no data is returned in the read call later.
                msg.as_mut_ptr() as _,
                msg.len() as u32 + 1,
            );
            debug!("    Success: {}", success);

            if read_len == 0 {
                return Some(vec![]);
            }

            let mut bytes_read = 0;
            debug!("  ReadFile");
            // HidD_GetFeature doesn't work, have to use ReadFile
            // Microsoft does recommend that
            // https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/obtaining-hid-reports
            let res = ReadFile(self.handle, Some(&mut buf), Some(&mut bytes_read), None);
            debug!("    Success: {:?}, Bytes: {}", res, bytes_read);
            debug!("    Read buf: {:X?}", buf);
            debug!("    Read msg: {:X?}", msg);
        }

        Some(buf[msg_len..msg_len + read_len].to_vec())
    }

    fn get_stylus_fw(&self) -> Option<()> {
        let mut msg = [0u8; 0x40];
        msg[0] = REPORT_ID_FIRMWARE;
        unsafe {
            let success = HidD_GetFeature(self.handle, msg.as_mut_ptr() as _, msg.len() as u32);
            debug!("    Success: {}", success);
        }
        println!("Stylus firmware: {:X?}", msg);

        let mut msg = [0u8; 0x40];
        msg[0] = REPORT_ID_USI_VER;
        unsafe {
            let success = HidD_GetFeature(self.handle, msg.as_mut_ptr() as _, msg.len() as u32);
            debug!("    Success: {}", success);
        }
        println!("USI Version:     {:X?}", msg);

        None
    }
    fn get_battery_status(&self) -> Option<u8> {
        error!("Get stylus battery status not supported on Windows");
        None
    }
}
