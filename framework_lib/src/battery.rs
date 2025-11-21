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

#[repr(u8)]
enum SmartBatReg {
    Mode = 0x03,
    Temp = 0x08,
    ManufactureDate = 0x1B,
    SerialNum = 0x1C,
    CycleCount = 0x17,
    DeviceName = 0x21,
}

#[repr(u8)]
/// ManufacturerAccess block
/// Needs unseal
enum ManufReg {
    SafetyAlert = 0x50,
    SafetyStatus = 0x51,
    PFAlert = 0x52,
    LifeTimeDataBlock1 = 0x60,
    LifeTimeDataBlock2 = 0x61,
    LifeTimeDataBlock3 = 0x62,
}

// fn get_i16(ec: &CrosEC) ->

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
    // 0A 46 52 41 "FRAN...
    println!("Dev Name: {:X?}", i2c_response.data);

    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x4F, 0x02)?;
    println!("SOH:      {:?}", i2c_response.data);

    // Need to unseal for access
    // SE [US] [FA]
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x51, 0x02)?;
    println!("SafetyAlrt{:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x53, 0x02)?;
    println!("SafetySts:{:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x51, 0x02)?;
    println!("PFAlert:  {:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x53, 0x02)?;
    println!("PFStatus: {:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x60, 0x02)?;
    println!("LifeTime1 {:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x61, 0x02)?;
    println!("LifeTime2 {:?}", i2c_response.data);
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x61, 0x02)?;
    println!("LifeTime3 {:?}", i2c_response.data);

    // i2c_write(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, &[0x00])?;
    // os_specific::sleep(50_000);

    Ok(Some(i2c_response.data))
}
