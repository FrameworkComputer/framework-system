use hidapi::{HidApi, HidDevice, HidError};

pub const ILI_VID: u16 = 0x222A;
pub const ILI_PID: u16 = 0x5539;
pub const USI_BITMAP: u8 = 1 << 1;
pub const MPP_BITMAP: u8 = 1 << 2;

fn send_message(device: &HidDevice, message_id: u8, read_len: usize) -> Result<Vec<u8>, HidError> {
    let report_id = 0x03;
    let write_len = 0x01;
    let mut msg = vec![report_id, 0xA3, write_len, read_len as u8, message_id];
    device.send_feature_report(&msg)?;

    msg.pop();
    let mut buf: [u8; 255] = [0; 255];
    device.read(&mut buf[..read_len + msg.len()])?;
    Ok(buf[msg.len()..msg.len() + read_len].to_vec())
}

fn check_fw_version(device: &HidDevice) -> Result<(), HidError> {
    let res = send_message(device, 0x40, 8)?;
    let ver = res
        .iter()
        .skip(1)
        .fold(res[0].to_string(), |acc, &x| acc + "." + &x.to_string());
    println!("  Firmware Version: v{}", ver);

    let res = send_message(device, 0x20, 16)?;
    println!("  USI Protocol:     {:?}", (res[15] & USI_BITMAP) > 0);
    println!("  MPP Protocol:     {:?}", (res[15] & MPP_BITMAP) > 0);

    Ok(())
}

pub fn print_touchscreen_fw_ver() -> Result<(), HidError> {
    debug!("Looking for touchscreen HID device");
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
                if vid != ILI_VID {
                    debug!("  Skipping VID:PID. Expected {:04X}:*", ILI_VID);
                    continue;
                }
                if usage_page != 0xFF00 {
                    debug!("  Skipping usage page. Expected {:04X}", 0xFF00);
                    continue;
                }
                if pid != ILI_PID {
                    debug!("  Warning: PID is {:04X}, expected {:04X}", pid, ILI_PID);
                }

                debug!("  Found matching touchscreen HID device");
                println!("Touchscreen");
                println!("  IC Type:          {:04X}", pid);

                // Unwrapping because if we can enumerate it, we should be able to open it
                let device = dev_info.open_device(&api).unwrap();
                if let Err(e) = check_fw_version(&device) {
                    error!("Failed to read touchscreen firmware version {:?}", e);
                    continue;
                };
            }
        }
        Err(e) => {
            eprintln!("Failed to open hidapi. Error: {e}");
            return Err(e);
        }
    };

    Ok(())
}
