//! Fuzz handling of EC host command responses
#![no_main]

use framework_lib::chromium_ec::fuzz_driver;
use framework_lib::{ccgx, parade_retimer, power};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(ec) = fuzz_driver::setup(data) else {
        return;
    };

    // PD / power
    power::get_and_print_pd_info(&ec);
    power::get_and_print_cypd_pd_info(&ec);
    let _ = power::read_pd_version(&ec);
    let _ = power::is_charging(&ec);
    let _ = power::get_cutoff_status(&ec);
    let _ = power::get_thermal_thresholds(&ec);
    power::print_thermal_thresholds(&ec);
    let _ = ccgx::get_pd_controller_versions(&ec);
    let _ = parade_retimer::get_version(&ec);

    // EC info
    let _ = ec.version_info();
    let _ = ec.get_protocol_info();
    let _ = ec.get_features();
    let _ = ec.get_sysinfo();
    let _ = ec.get_uptime_info();
    let _ = ec.get_ap_throttle_status();
    let _ = ec.get_panic_info();
    let _ = ec.get_s0ix_counter();
    let _ = ec.get_ec_hib_delay();
    if let Some(info) = power::power_info(&ec) {
        let _ = ec.get_charge_state(&info);
    }
    let _ = ec.console_read();
    let _ = ec.motionsense_sensor_list();

    // Platform features
    let _ = ec.get_privacy_info();
    let _ = ec.get_charge_limit();
    let _ = ec.get_fp_led_level();
    let _ = ec.get_intrusion_status();
    let _ = ec.get_input_deck_status();
    let _ = ec.print_fw12_inputdeck_status();
    let _ = ec.print_fw13_inputdeck_status();
    let _ = ec.print_fw16_inputdeck_status();
    let _ = ec.get_keyboard_backlight();
    let _ = ec.check_bay_status();
    let _ = ec.get_gpu_serial();
    let _ = ec.read_gpu_desc_header();
    let _ = ec.get_all_gpios();
});
