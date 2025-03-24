pub const FRAMEWORK_VID: u16 = 0x32AC;
pub const FRAMEWORK13_16_2ND_GEN_PID: u16 = 0x001C;
pub const FRAMEWORK12_PID: u16 = 0x001D;

/// Get and print the firmware version of the camera
pub fn check_camera_version() -> Result<(), rusb::Error> {
    for dev in rusb::devices().unwrap().iter() {
        let dev_descriptor = dev.device_descriptor().unwrap();
        if dev_descriptor.vendor_id() != FRAMEWORK_VID
            || (dev_descriptor.product_id() != FRAMEWORK13_16_2ND_GEN_PID
                && dev_descriptor.product_id() != FRAMEWORK12_PID)
        {
            debug!(
                "Skipping {:04X}:{:04X}",
                dev_descriptor.vendor_id(),
                dev_descriptor.product_id()
            );
            continue;
        }
        let handle = dev.open().unwrap();

        let dev_descriptor = dev.device_descriptor()?;
        let i_product = dev_descriptor
            .product_string_index()
            .and_then(|x| handle.read_string_descriptor_ascii(x).ok());
        println!("{}", i_product.unwrap_or_default());
        println!("  Firmware Version: {}", dev_descriptor.device_version());
    }
    Ok(())
}
