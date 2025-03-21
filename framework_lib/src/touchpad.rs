use hidapi::{HidApi, HidDevice, HidError};

pub const PIX_VID: u16 = 0x093A;
pub const PIX_REPORT_ID: u8 = 0x43;

fn read_byte(device: &HidDevice, addr: u8) -> Result<u8, HidError> {
    device.send_feature_report(&[PIX_REPORT_ID, addr, 0x10, 0])?;

    let mut buf = [0u8; 4];
    buf[0] = PIX_REPORT_ID;

    device.get_feature_report(&mut buf)?;
    Ok(buf[3])
}

fn read_ver(device: &HidDevice) -> Result<u16, HidError> {
    Ok(u16::from_le_bytes([
        read_byte(device, 0xb2)?,
        read_byte(device, 0xb3)?,
    ]))
}

pub fn print_touchpad_fw_ver() -> Result<(), HidError> {
    match HidApi::new() {
        Ok(api) => {
            for dev_info in api.device_list() {
                let vid = dev_info.vendor_id();
                let pid = dev_info.product_id();
                let usage_page = dev_info.usage_page();

                debug!("Found {:X}:{:X} Usage Page: {}", vid, pid, usage_page);
                if vid != PIX_VID || (pid != 0x0274 && pid != 0x0239) {
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

                println!("Touchpad");
                println!("  IC Type:           {:04X}", pid);
                println!("  Firmware Version: v{:04X}", read_ver(&device)?);
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    };

    Ok(())
}
