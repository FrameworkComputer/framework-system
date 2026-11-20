//! Fuzz parsing of Insyde H2O BIOS capsule content (--h2o-capsule)
#![no_main]

use framework_lib::capsule_content::*;
use framework_lib::ccgx::{binary, SiliconFamily};
use framework_lib::commandline::analyze_ec_fw;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = find_bios_version(data);
    let _ = find_retimer_version(data);
    if let Some(ec_bin) = find_ec_in_bios_cap(data) {
        analyze_ec_fw(ec_bin);
    }
    if let Some(pd_bin) = find_pd_in_bios_cap(data) {
        for family in [
            SiliconFamily::Ccg5,
            SiliconFamily::Ccg6Adl,
            SiliconFamily::Ccg6,
            SiliconFamily::Ccg8D,
            SiliconFamily::Ccg8S,
        ] {
            let _ = binary::read_versions(pd_bin, family);
        }
    }
});
