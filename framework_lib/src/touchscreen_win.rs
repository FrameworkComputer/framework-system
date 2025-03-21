use crate::touchscreen::TouchScreen;
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

pub struct NativeWinTouchScreen {
    handle: HANDLE,
}

impl TouchScreen for NativeWinTouchScreen {
    fn open_device() -> Option<Self> {
        // TODO: I don't know if this might be different on other systems
        // Should enumerate and find the right one
        // See: https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/finding-and-opening-a-hid-collection
        let path =
            w!(r"\\?\HID#ILIT2901&Col03#5&357cbf85&0&0002#{4d1e55b2-f16f-11cf-88cb-001111000030}");

        let res = unsafe {
            CreateFileW(
                path,
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

        Some(NativeWinTouchScreen { handle })
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
}
