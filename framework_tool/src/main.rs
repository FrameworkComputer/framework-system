use framework_lib::commandline;
#[allow(unused_imports)]
use log::{debug, error, info, trace};

fn main() {
    let level = log::LevelFilter::Debug.as_str();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .format_target(false)
        .format_timestamp(None)
        .init();
    if let Some(binfile) = option_env!("FWK_DP_HDMI_BIN") {
        let bin = match binfile {
            "dp-flash-006" => {
                include_bytes!("../../framework_lib/embed_bins/dp-flash-006.bin").as_slice()
            }
            "dp-flash-008" => {
                include_bytes!("../../framework_lib/embed_bins/dp-flash-008.bin").as_slice()
            }
            "dp-flash-100" => {
                include_bytes!("../../framework_lib/embed_bins/dp-flash-100.bin").as_slice()
            }
            "dp-flash-101" => {
                include_bytes!("../../framework_lib/embed_bins/dp-flash-101.bin").as_slice()
            }
            "hdmi-flash-005" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-005.bin").as_slice()
            }
            "hdmi-flash-006" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-006.bin").as_slice()
            }
            "hdmi-flash-102" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-102.bin").as_slice()
            }
            "hdmi-flash-103" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-103.bin").as_slice()
            }
            "hdmi-flash-104" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-104.bin").as_slice()
            }
            "hdmi-flash-105" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-105.bin").as_slice()
            }
            "hdmi-flash-106" => {
                include_bytes!("../../framework_lib/embed_bins/hdmi-flash-106.bin").as_slice()
            }
            _ => unreachable!(),
        };
        framework_lib::ccgx::hid::flash_firmware(bin);
    } else {
        commandline::print_dp_hdmi_details();
    }

    // Prevent command prompt from auto closing
    if cfg!(windows) {
        println!();
        println!("Press ENTER to exit...");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line).unwrap();
    }
}
