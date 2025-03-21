use hidapi::{HidApi, HidDevice, HidError};

pub const ILI_VID: u16 = 0x32AC;
pub const USI_BITMAP: u8 = 1 << 1;
pub const MPP_BITMAP: u8 = 1 << 2;

fn send_message(device: &HidDevice, message_id: u8, read_len: usize) -> Result<Vec<u8>, HidError> {
    let report_id = 0x03;
    let write_len = 0x01;
    let mut msg = vec![report_id, 0xA3, write_len, read_len as u8, message_id];
    device
        .send_feature_report(&msg)
        .expect("Failed to unlock device");

    msg.pop();
    let mut buf: [u8; 255] = [0; 255];
    device.read(&mut buf[..read_len + msg.len()])?;
    Ok(buf[msg.len()..msg.len() + read_len].to_vec())
}

fn check_fw_version(device: &HidDevice) -> Result<(), HidError> {
    println!("Touchscreen");

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
    match HidApi::new() {
        Ok(api) => {
            for dev_info in api.device_list() {
                let vid = dev_info.vendor_id();
                let pid = dev_info.product_id();
                let usage_page = dev_info.usage_page();

                debug!("Found {:X}:{:X} Usage Page: {}", vid, pid, usage_page);
                if vid != 0x222A || pid != 0x5539 {
                    continue;
                }
                if usage_page != 0xFF00 {
                    continue;
                }

                let device = dev_info.open_device(&api).unwrap();

                // On Windows this value is "Control Interface", probably hijacked by the kernel driver
                debug!(
                    "  Product String:  {}",
                    dev_info.product_string().unwrap_or("")
                );

                check_fw_version(&device)?;
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    };

    Ok(())
}
