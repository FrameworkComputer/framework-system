//! Fuzz parsing of EC firmware binaries (--ec-bin)
#![no_main]

use framework_lib::ec_binary;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for ro in [true, false] {
        if let Some(ver) = ec_binary::read_ec_version(data, ro) {
            ec_binary::print_ec_version(&ver, ro);
        }
    }

    // Version string parsing, also used on the version reported by the EC
    if let Ok(s) = std::str::from_utf8(&data[..data.len().min(64)]) {
        let _ = ec_binary::parse_ec_version_str(s);
    }
});
