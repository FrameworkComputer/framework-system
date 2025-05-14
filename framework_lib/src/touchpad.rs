use hidapi::{HidApi, HidDevice, HidError};
use log::Level;

pub const PIX_VID: u16 = 0x093A;
pub const P274_REPORT_ID: u8 = 0x43;
pub const P239_REPORT_ID: u8 = 0x42;

fn read_byte(device: &HidDevice, report_id: u8, addr: u8) -> Result<u8, HidError> {
    device.send_feature_report(&[report_id, addr, 0x10, 0])?;

    let mut buf = [0u8; 4];
    buf[0] = report_id;

    device.get_feature_report(&mut buf)?;
    Ok(buf[3])
}

fn read_239_ver(device: &HidDevice) -> Result<u16, HidError> {
    Ok(u16::from_le_bytes([
        read_byte(device, P239_REPORT_ID, 0x16)?,
        read_byte(device, P239_REPORT_ID, 0x18)?,
    ]))
}

fn read_274_ver(device: &HidDevice) -> Result<u16, HidError> {
    Ok(u16::from_le_bytes([
        read_byte(device, P274_REPORT_ID, 0xb2)?,
        read_byte(device, P274_REPORT_ID, 0xb3)?,
    ]))
}

pub fn print_touchpad_fw_ver() -> Result<(), HidError> {
    debug!("Looking for touchpad HID device");
    match HidApi::new() {
        Ok(api) => {
            for dev_info in api.device_list() {
                let vid = dev_info.vendor_id();
                let pid = dev_info.product_id();
                let usage_page = dev_info.usage_page();
                let hid_ver = dev_info.release_number();

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
                info!("  IC Type:           {:04X}", pid);

                let ver = match pid {
                    0x0239 => format!("{:04X}", read_239_ver(&device)?),
                    0x0274 => format!("{:04X}", read_274_ver(&device)?),
                    _ => "Unsupported".to_string(),
                };
                println!("  Firmware Version: v{}", ver);

                if log_enabled!(Level::Debug) {
                    println!("  Config space 1");
                    print!("   ");
                    for x in 0..16 {
                        print!("0{:X} ", x);
                    }
                    println!();
                    for y in 0..16 {
                        print!("{:X}0 ", y);
                        for x in 0..16 {
                            print!("{:02X} ", read_byte(&device, 0x42, x + 16 * y)?);
                        }
                        println!();
                    }
                    println!("  Config space 2");
                    print!("   ");
                    for x in 0..16 {
                        print!("0{:X} ", x);
                    }
                    println!();
                    for y in 0..16 {
                        print!("{:X}0 ", y);
                        for x in 0..16 {
                            print!("{:02X} ", read_byte(&device, 0x43, x + 16 * y)?);
                        }
                        println!();
                    }
                }

                // Linux does not expose a useful version number for I2C HID devices
                #[cfg(target_os = "linux")]
                debug!("  HID Version        {:04X}", hid_ver);
                #[cfg(not(target_os = "linux"))]
                if ver != format!("{:04X}", hid_ver) || log_enabled!(Level::Debug) {
                    println!("  HID Version       v{:04X}", hid_ver);
                }

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
