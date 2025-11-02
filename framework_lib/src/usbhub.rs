pub const REALTEK_VID: u16 = 0x0BDA;
pub const RTL5432_PID: u16 = 0x5432;
pub const RTL5424_PID: u16 = 0x5424;

pub const GENESYS_VID: u16 = 0x05E3;
pub const GL3590_PID: u16 = 0x0625;

/// Get and print the firmware version of the usbhub
pub fn check_usbhub_version() -> Result<(), rusb::Error> {
    for dev in rusb::devices().unwrap().iter() {
        let dev_descriptor = dev.device_descriptor().unwrap();

        if dev_descriptor.vendor_id() == REALTEK_VID
            && [RTL5432_PID, RTL5424_PID].contains(&dev_descriptor.product_id())
        {
            let dev_descriptor = dev.device_descriptor()?;
            println!("USB Hub RTL{:04X}", dev_descriptor.product_id());
            println!("  Firmware Version: {}", dev_descriptor.device_version());
        }

        if dev_descriptor.vendor_id() == GENESYS_VID
            && [GL3590_PID].contains(&dev_descriptor.product_id())
        {
            let dev_descriptor = dev.device_descriptor()?;
            if GL3590_PID == dev_descriptor.product_id() {
                println!("USB Hub GL3590");
            } else {
                println!("USB Hub GL????");
            }
            println!("  Firmware Version: {}", dev_descriptor.device_version());
        }
    }
    Ok(())
}
