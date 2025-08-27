pub const FRAMEWORK_VID: u16 = 0x32AC;
pub const LEDMATRIX_PID: u16 = 0x0020;
pub const FRAMEWORK16_INPUTMODULE_PIDS: [u16; 7] = [
    0x0012, // Keyboard White Backlight ANSI
    0x0013, // Keyboard RGB Backlight Numpad
    0x0014, // Keyboard White Backlight Numpad
    0x0018, // Keyboard White Backlight ISO
    0x0019, // Keyboard White Backlight JIS
    0x0030, // Keyboard White Backlight ANSI Copilot
    LEDMATRIX_PID,
];

/// Get and print the firmware version of the camera
pub fn check_inputmodule_version() -> Result<(), rusb::Error> {
    for dev in rusb::devices().unwrap().iter() {
        let dev_descriptor = dev.device_descriptor().unwrap();
        let vid = dev_descriptor.vendor_id();
        let pid = dev_descriptor.product_id();
        if vid != FRAMEWORK_VID || !FRAMEWORK16_INPUTMODULE_PIDS.contains(&pid) {
            trace!("Skipping {:04X}:{:04X}", vid, pid);
            continue;
        }

        // I'm not sure why, but the LED Matrix can't be opened with this code
        if pid == LEDMATRIX_PID {
            println!("LED Matrix");
        } else {
            debug!("Opening {:04X}:{:04X}", vid, pid);
            let handle = dev.open().unwrap();

            let dev_descriptor = dev.device_descriptor()?;
            let i_product = dev_descriptor
                .product_string_index()
                .and_then(|x| handle.read_string_descriptor_ascii(x).ok());
            println!("{}", i_product.unwrap_or_default());
        }
        println!("  Firmware Version: {}", dev_descriptor.device_version());

        debug!("Address:      {:?}", dev.address());
        debug!("Bus  Number:  {:?}", dev.bus_number());
        debug!("Port Number:  {:?}", dev.port_number());
        debug!("Port Numbers: {:?}", dev.port_numbers());
        let port_numbers = dev.port_numbers();
        let location = if let Ok(port_numbers) = port_numbers {
            if port_numbers.len() == 2 {
                match (port_numbers[0], port_numbers[1]) {
                    (4, 2) => "[X] [ ] [ ]       [ ] [ ]",
                    (4, 3) => "[ ] [X] [ ]       [ ] [ ]",
                    (3, 1) => "[ ] [ ] [X]       [ ] [ ]",
                    (3, 2) => "[ ] [ ] [ ]       [X] [ ]",
                    (3, 3) => "[ ] [ ] [ ]       [ ] [X]",
                    _ => "Unknown",
                }
            } else {
                "Unknown"
            }
        } else {
            "Unknown"
        };
        println!("  Location: {}", location);
    }
    Ok(())
}
