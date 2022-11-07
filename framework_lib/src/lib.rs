pub mod chromium_ec;
pub mod commandline;
pub mod ec_binary;
mod os_specific;
pub mod pd_binary;
pub mod power;
mod util;

//pub fn standalone_mode() -> bool {
//    // TODO: Figure out how to get that information
//    // For now just say we're in standalone mode when the battery is disconnected
//    let info = crate::power::power_info();
//    if let Some(i) = info {
//        i.battery.is_none()
//    } else {
//        // Default to true, when we can't find battery status, assume it's not there. Safe default.
//        true
//    }
//}
