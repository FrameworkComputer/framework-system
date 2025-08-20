pub const REALTEK_VID: u16 = 0x0BDA;
pub const RTL5432_PID: u16 = 0x5432;
pub const RTL5424_PID: u16 = 0x5424;

/// Get and print the firmware version of the usbhub
pub fn check_usbhub_version() -> Result<(), rusb::Error> {
    for dev in rusb::devices().unwrap().iter() {
        let dev_descriptor = dev.device_descriptor().unwrap();
        if dev_descriptor.vendor_id() != REALTEK_VID
            || (dev_descriptor.product_id() != RTL5432_PID
                && dev_descriptor.product_id() != RTL5424_PID)
        {
            debug!(
                "Skipping {:04X}:{:04X}",
                dev_descriptor.vendor_id(),
                dev_descriptor.product_id()
            );
            continue;
        }

        let dev_descriptor = dev.device_descriptor()?;
        println!("USB Hub RTL{:04X}", dev_descriptor.product_id());
        println!("  Firmware Version: {}", dev_descriptor.device_version());
    }
    Ok(())
}
