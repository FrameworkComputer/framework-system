//! Fuzz parsing of the EC memory map (battery, thermal, sensors, switches, ...)
#![no_main]

use framework_lib::chromium_ec::fuzz_driver;
use framework_lib::power;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(ec) = fuzz_driver::setup(data) else {
        return;
    };

    let _ = ec.check_mem_magic();
    let _ = power::power_info(&ec);
    power::get_and_print_power_info(&ec);
    power::print_memmap_version_info(&ec);
    let _ = power::get_thermal(&ec);
    power::print_thermal(&ec);
    let _ = power::get_fan_num(&ec);
    power::print_switches(&ec);
    let _ = power::get_accel_data(&ec);
    power::print_sensors(&ec);
});
