use hidapi::{HidApi, HidDevice, HidError};
use log::Level;

pub const PIX_VID: u16 = 0x093A;
pub const P274_REPORT_ID: u8 = 0x43;
pub const P239_REPORT_ID: u8 = 0x42;

// Standard HID Precision Touchpad (PTP) interface — every PTP-compliant touchpad
// reports on this usage. Only haptic touchpads expose the feature reports below.
const TOUCHPAD_USAGE_PAGE: u16 = 0x000D; // Digitizers
const TOUCHPAD_USAGE: u16 = 0x0005; // Touch Pad

// Haptic feedback intensity (HID Haptic page 0x0E, Usage 0x23 Intensity).
// Descriptor says logical range 0..100, but the Boreas haptic firmware
// only implements five steps: 0%, 25%, 50%, 75%, 100%.
const HAPTIC_INTENSITY_REPORT_ID: u8 = 0x09;
pub const HAPTIC_INTENSITY_LEVELS: [u8; 5] = [0, 25, 50, 75, 100];

// Button press threshold / click force (HID Digitizer page 0x0D, Usage 0xB0).
// 2-bit field, firmware accepts 1=Low, 2=Medium, 3=High.
const CLICK_FORCE_REPORT_ID: u8 = 0x08;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum ClickForce {
    Low = 1,
    Medium = 2,
    High = 3,
}

/// Open the PTP HID interface of the touchpad. Note: every modern touchpad
/// exposes this interface; only haptic touchpads respond to the feature
/// reports used by `set_haptic_intensity` / `set_click_force`.
fn open_haptic_touchpad() -> Option<HidDevice> {
    let api = HidApi::new().ok()?;
    for dev_info in api.device_list() {
        if dev_info.usage_page() != TOUCHPAD_USAGE_PAGE || dev_info.usage() != TOUCHPAD_USAGE {
            continue;
        }
        debug!(
            "  Touchpad candidate {:04X}:{:04X} (Usage Page {:04X}, Usage {:04X})",
            dev_info.vendor_id(),
            dev_info.product_id(),
            dev_info.usage_page(),
            dev_info.usage()
        );
        if let Ok(device) = dev_info.open_device(&api) {
            return Some(device);
        }
    }
    None
}

// The firmware accepts SET_FEATURE for these reports but doesn't reply
// to GET_FEATURE, so both controls are write-only.

fn hid_err(message: impl Into<String>) -> HidError {
    HidError::HidApiError {
        message: message.into(),
    }
}

pub fn set_haptic_intensity(value: u8) -> Result<(), HidError> {
    if !HAPTIC_INTENSITY_LEVELS.contains(&value) {
        return Err(hid_err(format!(
            "Haptic intensity must be one of: {:?}",
            HAPTIC_INTENSITY_LEVELS
        )));
    }
    let device =
        open_haptic_touchpad().ok_or_else(|| hid_err("Could not find a haptic touchpad"))?;
    let buf = [HAPTIC_INTENSITY_REPORT_ID, value];
    debug!("  send_feature_report (haptic intensity) {:X?}", buf);
    device.send_feature_report(&buf)
}

pub fn set_click_force(force: ClickForce) -> Result<(), HidError> {
    let device =
        open_haptic_touchpad().ok_or_else(|| hid_err("Could not find a haptic touchpad"))?;
    // Field is 2 bits at the bottom of the report payload
    let buf = [CLICK_FORCE_REPORT_ID, force as u8];
    debug!("  send_feature_report (click force) {:X?}", buf);
    device.send_feature_report(&buf)
}

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

fn read_360_ver(device: &HidDevice) -> Result<u16, HidError> {
    Ok(u16::from_le_bytes([
        read_byte(device, P274_REPORT_ID, 0x7e)?,
        read_byte(device, P274_REPORT_ID, 0x7f)?,
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
                if vid != PIX_VID
                    || (pid != 0x0274 && pid != 0x0239 && pid != 0x0360 && pid != 0x0343)
                {
                    debug!(
                        "  Skipping VID:PID. Expected {:04X}:{:04X}/{:04X}/{:04X}",
                        PIX_VID, 0x0274, 0x0239, 0x0343
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
                    0x0343 => format!("{:04X}", read_274_ver(&device)?),
                    0x0360 => format!("{:04X}", read_360_ver(&device)?),
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
