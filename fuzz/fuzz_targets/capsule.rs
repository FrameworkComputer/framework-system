//! Fuzz parsing of UEFI capsules (--capsule)
#![no_main]

use framework_lib::capsule;
use framework_lib::commandline::analyze_capsule;
use framework_lib::esrt;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Some(header) = analyze_capsule(data) {
        if header.capsule_guid == esrt::WINUX_GUID.into() {
            let _ = capsule::parse_ux_header(data);
        }
    }
});
