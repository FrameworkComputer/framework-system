use std::{thread, time};

//#[cfg(target_os = "linux")]
//#[cfg(target_os = "windows")]
pub fn sleep(micros: u64) {
    let ten_millis = time::Duration::from_micros(micros);
    thread::sleep(ten_millis);
    // TODO: If UEFI
    //let uefi = std::system_table();
    //let _ = (uefi.BootServices.Stall)(1000);
}
