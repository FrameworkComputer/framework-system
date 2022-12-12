#[cfg(not(feature = "uefi"))]
use std::{thread, time};

#[cfg(not(feature = "uefi"))]
pub fn sleep(micros: u64) {
    let ten_millis = time::Duration::from_micros(micros);
    thread::sleep(ten_millis);
}

#[cfg(feature = "uefi")]
pub fn sleep(_micros: u64) {
    let uefi = std::system_table();
    let _ = (uefi.BootServices.Stall)(1000);
}
