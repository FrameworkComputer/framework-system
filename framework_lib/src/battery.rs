// https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf?ts=1763375446472
// driver/battery/smart.c
// include/battery_smart.h
use alloc::vec::Vec;
use num_traits::FromPrimitive;

use crate::chromium_ec::command::EcRequestRaw;
use crate::chromium_ec::commands::{EcRequestGetGpuPcie, GpuVendor};
use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcResult};
use crate::os_specific;



pub fn dump_data(ec: &CrosEc) -> EcResult<Option<Vec<u8>>> {
    // I2C Port on the EC
    let i2c_port = 3;
    // 8-bit I2C address of the battery
    // EC passthrough needs 7-bit, so shift one over before sending to EC
    let i2c_addr = 0x0b << 1;

    // Check mode
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x03, 0x01)?;
    println!("Mode:     {:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x1c, 0x02)?;
    println!("Serial:   {:X?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x08, 0x02)?;
    println!("Temp:     {:X?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x17, 0x02)?;
    println!("Cycle Ct: {:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x21, 0x08)?;
    // 0A 46 52 41
    println!("Dev Name: {:X?}", i2c_response.data);

    // i2c_write(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, &[0x00])?;
    // os_specific::sleep(50_000);

    Ok(Some(i2c_response.data))
}
