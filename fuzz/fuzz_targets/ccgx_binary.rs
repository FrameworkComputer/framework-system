//! Fuzz parsing of CCGX PD firmware binaries (--pd-bin)
#![no_main]

use framework_lib::ccgx::binary;
use framework_lib::ccgx::SiliconFamily;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let families = [
        SiliconFamily::Ccg3,
        SiliconFamily::Ccg5,
        SiliconFamily::Ccg6Adl,
        SiliconFamily::Ccg6,
        SiliconFamily::Ccg8D,
        SiliconFamily::Ccg8S,
        SiliconFamily::Ccg6Cfp,
        SiliconFamily::Ccg8Cfp,
    ];
    // Unlike the commandline, try all families, not just until the first match
    for family in families {
        if let Some(versions) = binary::read_versions(data, family) {
            binary::print_fw(&versions.backup_fw);
            binary::print_fw(&versions.main_fw);
        }
    }
});
