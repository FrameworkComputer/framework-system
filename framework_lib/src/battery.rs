// https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf?ts=1763375446472
// driver/battery/smart.c
// include/battery_smart.h
use alloc::vec::Vec;

use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcResult};
//use crate::os_specific;

#[repr(u16)]
enum SmartBatReg {
    Mode = 0x03,
    Temp = 0x08,
    ManufactureDate = 0x1B,
    SerialNum = 0x1C,
    CycleCount = 0x17,
    /// String
    ManufacturerName = 0x20,
    DeviceName = 0x21,
    Soh = 0x4F,
}

#[repr(u16)]
/// ManufacturerAccess block
/// Needs unseal
/// On EC Console can read these with
/// If CONFIG_CMD_BATT_MFG_ACCESS
/// > mattmfgacc 0xBEEF 0x50 2
enum ManufReg {
    SafetyAlert = 0x50,
    SafetyStatus = 0x51,
    PFAlert = 0x52,
    LifeTimeDataBlock1 = 0x60,
    LifeTimeDataBlock2 = 0x61,
    LifeTimeDataBlock3 = 0x62,
}

pub struct SmartBattery {
    i2c_port: u8,
    i2c_addr: u16,
}

impl SmartBattery {
    pub fn new() -> Self {
        SmartBattery {
            // Same on all our Nuvoton ECs
            i2c_port: 3,
            // 0x0B 7-bit, 0x016 8-bit address
            // Same for all our batteries, they use the same IC
            i2c_addr: 0x16,
        }
    }

    fn read_bytes(&self, ec: &CrosEc, addr: u16, len: u16) -> EcResult<Vec<u8>> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, len)?;
        i2c_response.is_successful()?;
        Ok(i2c_response.data)
    }
    fn read_i16(&self, ec: &CrosEc, addr: u16) -> EcResult<i16> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 0x02)?;
        i2c_response.is_successful()?;
        Ok(i16::from_le_bytes([
            i2c_response.data[1],
            i2c_response.data[1],
        ]))
    }
    fn read_string(&self, ec: &CrosEc, addr: u16) -> EcResult<String> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 16)?;
        i2c_response.is_successful()?;

        // First byte is the returned string length
        let str_bytes = &i2c_response.data[1..=(i2c_response.data[0] as usize)];
        Ok(String::from_utf8_lossy(str_bytes).to_string())
    }

    pub fn dump_data(&self, ec: &CrosEc) -> EcResult<()> {
        // Check mode
        println!(
            "Mode:          {:?}",
            self.read_i16(ec, SmartBatReg::Mode as u16)?
        );
        println!(
            "Serial Num:    {:04X?}",
            self.read_i16(ec, SmartBatReg::SerialNum as u16)?
        );
        println!(
            "Manuf Date:    {:04X?}",
            self.read_i16(ec, SmartBatReg::ManufactureDate as u16)?
        );
        println!(
            "Temperature:   {:?}",
            self.read_i16(ec, SmartBatReg::Temp as u16)?
        );
        println!(
            "Cycle Count:   {:?}",
            self.read_i16(ec, SmartBatReg::CycleCount as u16)?
        );
        println!(
            "Device Name:   {}",
            self.read_string(ec, SmartBatReg::DeviceName as u16)?
        );
        println!(
            "Manuf Name:    {}",
            self.read_string(ec, SmartBatReg::ManufacturerName as u16)?
        );
        println!(
            "StateOfHealth: {:?}",
            self.read_i16(ec, SmartBatReg::Soh as u16)?
        );

        // Need to unseal for access
        // SE [US] [FA]
        println!(
            "Safety Alert:  {:?}",
            self.read_i16(&ec, ManufReg::SafetyAlert as u16)?
        );
        println!(
            "Safety Status: {:?}",
            self.read_i16(&ec, ManufReg::SafetyStatus as u16)?
        );
        println!(
            "PFAlert:       {:?}",
            self.read_i16(&ec, ManufReg::PFAlert as u16)?
        );
        println!(
            "PFStatus:      {:?}",
            self.read_i16(&ec, ManufReg::SafetyAlert as u16)?
        );
        println!(
            "LifeTime1      {:X?}",
            self.read_bytes(ec, ManufReg::LifeTimeDataBlock1 as u16, 4)?
        );
        println!(
            "LifeTime2      {:X?}",
            self.read_bytes(ec, ManufReg::LifeTimeDataBlock2 as u16, 4)?
        );
        println!(
            "LifeTime3      {:X?}",
            self.read_bytes(ec, ManufReg::LifeTimeDataBlock3 as u16, 4)?
        );

        // i2c_write(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, &[0x00])?;
        // os_specific::sleep(50_000);

        Ok(())
    }
}
