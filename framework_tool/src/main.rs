use framework_lib::commandline;

fn main() {
    if let Some(binfile) = option_env!("FWK_DP_HDMI_BIN") {
        let (bin, pid) = match binfile {
            "dp-flash-008" => {
                (include_bytes!("../../framework_lib/embed_bins/dp-flash-008.bin").as_slice(), framework_lib::ccgx::hid::DP_CARD_PID)
            }
            "dp-flash-100" => {
                (include_bytes!("../../framework_lib/embed_bins/dp-flash-100.bin").as_slice(), framework_lib::ccgx::hid::DP_CARD_PID)
            }
            "dp-flash-101" => {
                (include_bytes!("../../framework_lib/embed_bins/dp-flash-101.bin").as_slice(), framework_lib::ccgx::hid::DP_CARD_PID)
            }
            "hdmi-flash-006" => {
                (include_bytes!("../../framework_lib/embed_bins/hdmi-flash-006.bin").as_slice(), framework_lib::ccgx::hid::HDMI_CARD_PID)
            }
            "hdmi-flash-105" => {
                (include_bytes!("../../framework_lib/embed_bins/hdmi-flash-105.bin").as_slice(), framework_lib::ccgx::hid::HDMI_CARD_PID)
            }
            _ => unreachable!(),
        };
        framework_lib::ccgx::hid::flash_firmware(bin, &[pid]);
    } else {
        commandline::print_dp_hdmi_details();
    }

    // Prevent command prompt from auto closing
    if cfg!(windows) {
        println!("");
        println!("Press ENTER to exit...");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line).unwrap();
    }
}
