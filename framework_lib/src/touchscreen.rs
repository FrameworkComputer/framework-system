use hidapi::{HidApi, HidDevice};

#[cfg(windows)]
use crate::touchscreen_win;

pub const ILI_VID: u16 = 0x222A;
pub const ILI_PID: u16 = 0x5539;
const VENDOR_USAGE_PAGE: u16 = 0xFF00;
pub const USI_BITMAP: u8 = 1 << 1;
pub const MPP_BITMAP: u8 = 1 << 2;

const REPORT_ID_FIRMWARE: u8 = 0x27;
const REPORT_ID_USI_VER: u8 = 0x28;

struct HidapiTouchScreen {
    device: HidDevice,
}

impl TouchScreen for HidapiTouchScreen {
    fn open_device(target_up: u16, skip: u8) -> Option<HidapiTouchScreen> {
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

    fn get_battery_status(&self) -> Option<u8> {
        let mut msg = [0u8; 0x40];
        msg[0] = 0x0D;
        self.device.read(&mut msg).ok()?;
        // println!("  Tip Switch        {}%", msg[12]);
        // println!("  Barrell Switch:   {}%", msg[12]);
        // println!("  Eraser:           {}%", msg[12]);
        // println!("  Invert:           {}%", msg[12]);
        // println!("  In Range:         {}%", msg[12]);
        // println!("  2nd Barrel Switch:{}%", msg[12]);
        // println!("  X                 {}%", msg[12]);
        // println!("  Y                 {}%", msg[12]);
        // println!("  Tip Pressure:     {}%", msg[12]);
        // println!("  X Tilt:           {}%", msg[12]);
        // println!("  Y Tilt:           {}%", msg[12]);
        debug!("  Battery Strength: {}%", msg[12]);
        debug!(
            "  Barrel Pressure:  {}",
            u16::from_le_bytes([msg[13], msg[14]])
        );
        debug!("  Transducer Index: {}", msg[15]);

        if msg[12] == 0 {
            None
        } else {
            Some(msg[12])
        }
    }

    fn get_stylus_fw(&self) -> Option<()> {
        let mut msg = [0u8; 0x40];
        msg[0] = REPORT_ID_USI_VER;
        self.device.get_feature_report(&mut msg).ok()?;
        let usi_major = msg[2];
        let usi_minor = msg[3];
        debug!("USI version (Major.Minor): {}.{}", usi_major, usi_minor);

        if usi_major != 2 || usi_minor != 0 {
            // Probably not USI mode
            return None;
        }

        let mut msg = [0u8; 0x40];
        msg[0] = REPORT_ID_FIRMWARE;
        self.device.get_feature_report(&mut msg).ok()?;
        let sn_low = u32::from_le_bytes([msg[2], msg[3], msg[4], msg[5]]);
        let sn_high = u32::from_le_bytes([msg[6], msg[7], msg[8], msg[9]]);
        let vid = u16::from_le_bytes([msg[14], msg[15]]);
        let vendor = if vid == 0x32AC {
            " (Framework Computer)"
        } else {
            ""
        };
        let pid = u16::from_le_bytes([msg[16], msg[17]]);
        let product = if pid == 0x002B {
            " (Framework Stylus)"
        } else {
            ""
        };
        println!("Stylus");
        println!("  Serial Number:    {:X}-{:X}", sn_high, sn_low);
        debug!("  Redundant SN      {:X?}", &msg[10..14]);
        println!("  Vendor ID:        {:04X}{}", vid, vendor);
        println!("  Product ID:       {:04X}{}", pid, product);
        println!("  Firmware Version: {:02X}.{:02X}", &msg[18], msg[19]);

        Some(())
    }
}

pub trait TouchScreen {
    fn open_device(usage_page: u16, skip: u8) -> Option<Self>
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
        let mut protocols = vec![];
        if (res[15] & USI_BITMAP) > 0 {
            protocols.push("USI");
        }
        if (res[15] & MPP_BITMAP) > 0 {
            protocols.push("MPP");
        }
        println!("  Protocols:        {}", protocols.join(", "));

        Some(())
    }

    fn enable_touch(&self, enable: bool) -> Option<()> {
        self.send_message(0x38, 0, vec![!enable as u8, 0x00])?;
        Some(())
    }

    fn get_stylus_fw(&self) -> Option<()>;
    fn get_battery_status(&self) -> Option<u8>;
}

pub fn get_battery_level() -> Option<u8> {
    for skip in 0..5 {
        if let Some(device) = HidapiTouchScreen::open_device(0x000D, skip) {
            if let Some(level) = device.get_battery_status() {
                return Some(level);
            }
        }
    }
    None
}

pub fn print_fw_ver() -> Option<()> {
    for skip in 0..5 {
        if let Some(device) = HidapiTouchScreen::open_device(0x000D, skip) {
            if device.get_stylus_fw().is_some() {
                break;
            }
        }
    }

    #[cfg(target_os = "windows")]
    let device = touchscreen_win::NativeWinTouchScreen::open_device(VENDOR_USAGE_PAGE, 0)?;
    #[cfg(not(target_os = "windows"))]
    let device = HidapiTouchScreen::open_device(VENDOR_USAGE_PAGE, 0)?;

    device.check_fw_version()
}

pub fn enable_touch(enable: bool) -> Option<()> {
    #[cfg(target_os = "windows")]
    let device = touchscreen_win::NativeWinTouchScreen::open_device(VENDOR_USAGE_PAGE, 0)?;
    #[cfg(not(target_os = "windows"))]
    let device = HidapiTouchScreen::open_device(VENDOR_USAGE_PAGE, 0)?;

    device.enable_touch(enable)
}
