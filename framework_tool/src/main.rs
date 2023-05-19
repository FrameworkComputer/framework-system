use framework_lib::commandline;

fn main() {
    if let Some(binfile) = option_env!("FWK_DP_HDMI_BIN") {
        let bin = match binfile {
            "dp-flash-008" => include_bytes!("../../framework_lib/embed_bins/dp-flash-008.bin"),
            "dp-flash-100" => include_bytes!("../../framework_lib/embed_bins/dp-flash-008.bin"),
            "dp-flash-101" => include_bytes!("../../framework_lib/embed_bins/dp-flash-008.bin"),
            "hdmi-flash-006" => include_bytes!("../../framework_lib/embed_bins/hdmi-flash-006.bin"),
            "hdmi-flash-105" => include_bytes!("../../framework_lib/embed_bins/hdmi-flash-105.bin"),
            _ => unreachable!(),
        };
        framework_lib::ccgx::hid::flash_firmware(bin.as_slice());
    } else {
        commandline::print_dp_hdmi_details();
    }
}
