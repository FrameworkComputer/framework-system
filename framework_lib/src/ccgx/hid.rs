use hidapi::HidDevice;

use crate::ccgx::BaseVersion;

pub const CCG_USAGE_PAGE: u16 = 0xFFEE;

pub const FRAMEWORK_VID: u16 = 0x32AC;
pub const HDMI_CARD_PID: u16 = 0x0002;
pub const DP_CARD_PID: u16 = 0x0003;

pub fn check_ccg_fw_version(device: &HidDevice) {
    device.set_blocking_mode(true).unwrap();
    device
        .send_feature_report(&[0xE4, 0x42, 0x43, 0x59, 0x00, 0x00, 0x00, 0x0B])
        .unwrap(); // Report ID 228

    // Returns Err but seems to work anyway
    let _ = device.write(&[0xE1, 0x06, 0x42, 0x00, 0xCC, 0xCC, 0xCC, 0xCC]); //.unwrap(); // Report ID 225
                                                                             // Get 0x40 bytes from 0xE0 ()
    let mut buf = [0u8; 0x40];
    buf[0] = 0xE0; // 224
    device.get_feature_report(&mut buf).unwrap();

    let signature = &buf[2..4];
    let sig_valid = signature == [b'C', b'Y'];
    if !sig_valid {
        println!("  Signature Valid: {} ({:X?})", sig_valid, &buf[2..4]);
    }
    println!("  Operating Mode:  0x{:X?}", &buf[4]);
    println!("  Silicon ID:      {:X?}", &buf[8..12]);
    println!("  BL Version:      {}", BaseVersion::from(&buf[12..]));
    println!("  Image 1 Version: {}", BaseVersion::from(&buf[20..]));
    println!("  Image 2 Version: {}", BaseVersion::from(&buf[28..]));
}
