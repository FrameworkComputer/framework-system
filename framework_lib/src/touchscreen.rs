use hidapi::{HidApi, HidDevice};

#[cfg(target_os = "windows")]
use crate::touchscreen_win;

pub const ILI_VID: u16 = 0x222A;
pub const ILI_PID: u16 = 0x5539;
pub const USI_BITMAP: u8 = 1 << 1;
pub const MPP_BITMAP: u8 = 1 << 2;

struct HidapiTouchScreen {
    device: HidDevice,
}

impl TouchScreen for HidapiTouchScreen {
    fn open_device() -> Option<HidapiTouchScreen> {
        debug!("Looking for touchscreen HID device");
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
                    if usage_page != 0xFF00 {
                        debug!("    Skipping usage page. Expected {:04X}", 0xFF00);
                        continue;
                    }
                    if pid != ILI_PID {
                        debug!("  Warning: PID is {:04X}, expected {:04X}", pid, ILI_PID);
                    }

                    debug!("  Found matching touchscreen HID device");
                    debug!("  Path:             {:?}", dev_info.path());
                    debug!("  IC Type:          {:04X}", pid);

                    // Unwrapping because if we can enumerate it, we should be able to open it
                    let device = dev_info.open_device(&api).unwrap();
                    debug!("  Opened device.");

                    return Some(HidapiTouchScreen { device });
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
        msg[0] = report_id;
        msg[1] = 0xA3;
        msg[2] = data_len as u8;
        msg[3] = read_len as u8;
        msg[4] = message_id;
        for (i, b) in data.into_iter().enumerate() {
            msg[5 + i] = b;
        }

        // Not sure why, but on Windows we just have to write an output report
        // HidApiError { message: "HidD_SetFeature: (0x00000057) The parameter is incorrect." }
        // Still doesn't work on Windows. Need to write a byte more than the buffer is long
        #[cfg(target_os = "windows")]
        let send_feature_report = false;
        #[cfg(not(target_os = "windows"))]
        let send_feature_report = true;

        if send_feature_report {
            debug!("  send_feature_report {:X?}", msg);
            self.device.send_feature_report(&msg).ok()?;
        } else {
            debug!("  Writing {:X?}", msg);
            self.device.write(&msg).ok()?;
        };

        if read_len == 0 {
            return Some(vec![]);
        }

        let msg_len = 3 + data_len;
        let mut buf: [u8; 0x40] = [0; 0x40];
        debug!("  Reading");
        let res = self.device.read(&mut buf);
        debug!("  res: {:?}", res);
        debug!("  Read buf: {:X?}", buf);
        Some(buf[msg_len..msg_len + read_len].to_vec())
    }
}

pub trait TouchScreen {
    fn open_device() -> Option<Self>
    where
        Self: std::marker::Sized;
    fn send_message(&self, message_id: u8, read_len: usize, data: Vec<u8>) -> Option<Vec<u8>>;

    fn check_fw_version(&self) -> Option<()> {
        println!("Touchscreen");
        let res = self.send_message(0x42, 3, vec![0])?;
        let ver = res
            .iter()
            .skip(1)
            .fold(format!("{:02X}", res[0]), |acc, &x| {
                acc + "." + &format!("{:02X}", x)
            });
        // Expecting 06.00.0A
        debug!("  Protocol Version: v{}", ver);

        let res = self.send_message(0x40, 8, vec![0])?;
        let ver = res
            .iter()
            .skip(1)
            .fold(res[0].to_string(), |acc, &x| acc + "." + &x.to_string());
        println!("  Firmware Version: v{}", ver);

        let res = self.send_message(0x20, 16, vec![0])?;
        println!("  USI Protocol:     {:?}", (res[15] & USI_BITMAP) > 0);
        println!("  MPP Protocol:     {:?}", (res[15] & MPP_BITMAP) > 0);

        Some(())
    }

    fn enable_touch(&self, enable: bool) -> Option<()> {
        self.send_message(0x38, 0, vec![!enable as u8, 0x00])?;
        Some(())
    }
}

pub fn print_fw_ver() -> Option<()> {
    #[cfg(target_os = "windows")]
    let device = touchscreen_win::NativeWinTouchScreen::open_device()?;
    #[cfg(not(target_os = "windows"))]
    let device = HidapiTouchScreen::open_device()?;

    device.check_fw_version()
}

pub fn enable_touch(enable: bool) -> Option<()> {
    #[cfg(target_os = "windows")]
    let device = touchscreen_win::NativeWinTouchScreen::open_device()?;
    #[cfg(not(target_os = "windows"))]
    let device = HidapiTouchScreen::open_device()?;

    device.enable_touch(enable)
}
