#[cfg(not(target_os = "freebsd"))]
use hidapi::{HidApi, HidDevice, HidError};

#[cfg(target_os = "freebsd")]
use crate::freebsd_hid::*;
#[cfg(target_os = "freebsd")]
use std::os::fd::AsRawFd;

pub const PIX_VID: u16 = 0x093A;
pub const TP_PID: u16 = 0x0274;
pub const PIX_REPORT_ID: u8 = 0x43;

#[cfg(target_os = "freebsd")]
pub fn print_touchpad_fw_ver() -> Option<()> {
    if let Some(file) = hidraw_open(PIX_VID, TP_PID) {
        println!("Touchpad");
        unsafe {
            let fd = file.as_raw_fd();

            let mut desc = HidIocGrInfo {
                bustype: 0,
                vendor: 0,
                product: 0,
            };
            if let Err(err) = hidiocgrawninfo(fd, &mut desc) {
                error!("Failed to call hidiocgrawninfo: {}", err);
                return None;
            }
            println!("  IC Type:           {:04X}", desc.product);

            let mut buf = [0u8; 255];
            if let Err(err) = hid_raw_name(fd, &mut buf) {
                error!("Failed to call hid_raw_name: {}", err);
                return None;
            }
            let name = std::str::from_utf8(&buf)
                .unwrap()
                .trim_end_matches(char::from(0));
            debug!("  Name: {}", name);

            println!("  Firmware Version: v{:04X}", read_ver(fd)?);

            read_byte(fd, 0x2b);
        }
    }

    Some(())
}

fn read_byte(fd: i32, addr: u8) -> Option<u8> {
    unsafe {
        let mut buf: [u8; 4] = [PIX_REPORT_ID, addr, 0x10, 0];
        if let Err(err) = hid_set_feature(fd, &mut buf) {
            error!("Failed to hid_set_feature: {:?}", err);
            return None;
        }
        //device.send_feature_report(&[PIX_REPORT_ID, addr, 0x10, 0])?;

        let mut buf = [0u8; 4];
        buf[0] = PIX_REPORT_ID;

        if let Err(err) = hid_get_feature(fd, &mut buf) {
            error!("Failed to hid_get_feature: {:?}", err);
            return None;
        }
        Some(buf[3])
    }
}

#[cfg(target_os = "freebsd")]
fn read_ver(device: i32) -> Option<u16> {
    Some(u16::from_le_bytes([
        read_byte(device, 0xb2)?,
        read_byte(device, 0xb3)?,
    ]))
}

#[cfg(not(target_os = "freebsd"))]
fn read_byte(device: &HidDevice, addr: u8) -> Result<u8, HidError> {
    device.send_feature_report(&[PIX_REPORT_ID, addr, 0x10, 0])?;

    let mut buf = [0u8; 4];
    buf[0] = PIX_REPORT_ID;

    device.get_feature_report(&mut buf)?;
    Ok(buf[3])
}

#[cfg(not(target_os = "freebsd"))]
fn read_ver(device: &HidDevice) -> Result<u16, HidError> {
    Ok(u16::from_le_bytes([
        read_byte(device, 0xb2)?,
        read_byte(device, 0xb3)?,
    ]))
}

#[cfg(not(target_os = "freebsd"))]
pub fn print_touchpad_fw_ver() -> Result<(), HidError> {
    debug!("Looking for touchpad HID device");
    match HidApi::new() {
        Ok(api) => {
            for dev_info in api.device_list() {
                let vid = dev_info.vendor_id();
                let pid = dev_info.product_id();
                let usage_page = dev_info.usage_page();

                debug!(
                    "  Found {:04X}:{:04X} (Usage Page {:04X})",
                    vid, pid, usage_page
                );
                if vid != PIX_VID || (pid != 0x0274 && pid != 0x0239) {
                    debug!(
                        "  Skipping VID:PID. Expected {:04X}:{:04X}/{:04X}",
                        PIX_VID, 0x0274, 0x0239
                    );
                    continue;
                }
                if usage_page != 0xFF00 {
                    debug!("  Skipping usage page. Expected {:04X}", 0xFF00);
                    continue;
                }

                debug!("  Found matching touchpad HID device");
                let device = dev_info.open_device(&api).unwrap();

                println!("Touchpad");
                println!("  IC Type:           {:04X}", pid);
                println!("  Firmware Version: v{:04X}", read_ver(&device)?);
                // If we found one, there's no need to look for more
                return Ok(());
            }
        }
        Err(e) => {
            eprintln!("Failed to open hidapi. Error: {e}");
            return Err(e);
        }
    };

    Ok(())
}
