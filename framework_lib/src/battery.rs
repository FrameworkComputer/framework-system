// https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf?ts=1763375446472
// driver/battery/smart.c
// include/battery_smart.h
use alloc::vec::Vec;

use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcResult};

#[repr(u16)]
enum SmartBatReg {
    Mode = 0x03,
    Temp = 0x08,
    Voltage = 0x09,
    ManufactureDate = 0x1B,
    SerialNum = 0x1C,
    CycleCount = 0x17,
    /// String
    ManufacturerName = 0x20,
    DeviceName = 0x21,
    Soh = 0x4F,
    CellVoltage1 = 0x3C,
    CellVoltage2 = 0x3D,
    CellVoltage3 = 0x3E,
    CellVoltage4 = 0x3F,
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
    LifeTimeDataBlock4 = 0x63,
    LifeTimeDataBlock5 = 0x64,
    Soh = 0x77,
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

    fn unseal(&self, ec: &CrosEc, key1: u16, key2: u16) -> EcResult<()> {
        i2c_write_block(ec, self.i2c_port, self.i2c_addr >> 1, 0x00, &key1.to_le_bytes())?;
        i2c_write_block(ec, self.i2c_port, self.i2c_addr >> 1, 0x00, &key2.to_le_bytes())?;
        Ok(())
    }

    fn seal(&self, ec: &CrosEc) -> EcResult<()> {
        i2c_write(ec, self.i2c_port, self.i2c_addr >> 1, 0x00, &[0x30, 0x00])?;
        Ok(())
    }

    fn read_bytes(&self, ec: &CrosEc, addr: u16, len: u16) -> EcResult<Vec<u8>> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, len + 1)?;
        i2c_response.is_successful()?;
        debug_assert_eq!(i2c_response.data[0], len as u8);
        Ok(i2c_response.data[1..].to_vec())
    }
    fn read_i16(&self, ec: &CrosEc, addr: u16) -> EcResult<u16> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 0x02)?;
        i2c_response.is_successful()?;
        Ok(u16::from_le_bytes([
            i2c_response.data[0],
            i2c_response.data[1],
        ]))
    }
    fn read_string(&self, ec: &CrosEc, addr: u16) -> EcResult<String> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 32)?;
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
        let temp = self.read_i16(ec, SmartBatReg::Temp as u16)?;
        println!(
            "Temperature:   {}.{}C",
            temp / 100,
            temp % 100
        );
        let voltage = self.read_i16(ec, SmartBatReg::Voltage as u16)?;
        println!(
            "Voltage:       {}.{}V",
            voltage / 1000,
            voltage % 1000
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

        // Default key - does not work on our battery, it's changed during manufacturing!
        self.unseal(ec, 0x0414, 0x3672).unwrap();

        // Need to unseal for access
        // SE [US] [FA]
        let soh = self.read_bytes(ec, ManufReg::Soh as u16, 4)?;
        println!(
            "StateOfHealth: {}mAh, {}.{}Wh",
            u16::from_le_bytes([soh[0], soh[1]]),
            u16::from_le_bytes([soh[2], soh[3]]) / 100,
            u16::from_le_bytes([soh[2], soh[3]]) % 100,
        );
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
        let lifetime1 = self.read_bytes(ec, ManufReg::LifeTimeDataBlock1 as u16, 32)?;
        println!("LifeTime1");
        println!(
            "  Cell 1 Max Voltage: {}mV",
            u16::from_le_bytes([lifetime1[0], lifetime1[1]])
        );
        println!(
            "         Min Voltage: {}mV",
            u16::from_le_bytes([lifetime1[8], lifetime1[9]])
        );
        println!(
            "  Cell 2 Max Voltage: {}mV",
            u16::from_le_bytes([lifetime1[2], lifetime1[3]])
        );
        println!(
            "         Min Voltage: {}mV",
            u16::from_le_bytes([lifetime1[10], lifetime1[11]])
        );
        println!(
            "  Cell 3 Max Voltage: {}mV",
            u16::from_le_bytes([lifetime1[4], lifetime1[5]])
        );
        println!(
            "         Min Voltage: {}mV",
            u16::from_le_bytes([lifetime1[12], lifetime1[13]])
        );
        println!(
            "  Cell 4 Max Voltage: {}mV",
            u16::from_le_bytes([lifetime1[6], lifetime1[7]])
        );
        println!(
            "         Min Voltage: {}mV",
            u16::from_le_bytes([lifetime1[14], lifetime1[15]])
        );
        println!(
            "  Max Delta Cell Voltage: {}mV",
            u16::from_le_bytes([lifetime1[16], lifetime1[17]])
        );
        println!(
            "  Max Charge Current:     {}mA",
            u16::from_le_bytes([lifetime1[18], lifetime1[19]])
        );
        println!(
            "  Max Discharge Current:  {}mA",
            u16::from_le_bytes([lifetime1[20], lifetime1[21]])
        );
        println!(
            "  Max Avg Dsg Current:    {}mA",
            u16::from_le_bytes([lifetime1[22], lifetime1[23]])
        );
        println!(
            "  Max Avg Dsg Power:      {}mW",
            u16::from_le_bytes([lifetime1[24], lifetime1[25]])
        );
        println!(
            "  Max Temp Cell:          {}C",
            lifetime1[27]
        );
        println!(
            "  Min Temp Cell:          {}C",
            lifetime1[28]
        );
        println!(
            "  Max Delta Cell temp:    {}K",
            lifetime1[29]
        );
        println!(
            "  Max Temp Int Sensor:    {}C",
            lifetime1[29]
        );
        println!(
            "  Min Temp Int Sensor:    {}C",
            lifetime1[30]
        );
        println!(
            "  Max Temp Fet:           {}C",
            lifetime1[31]
        );
        let lifetime2 = self.read_bytes(ec, ManufReg::LifeTimeDataBlock2 as u16, 20)?; // 8?
        println!("LifeTime2");
        println!("  No. of Shutdowns:       {}
  No. of Partial Resets:  {}
  No. of Full Resets:     {}
  No. of WDT resets:      {}
  CB Time Cell 1:         {}
  CB Time Cell 2:         {}
  CB Time Cell 3:         {}
  CB Time Cell 4:         {}",
            lifetime2[0],
            lifetime2[1],
            lifetime2[2],
            lifetime2[3],
            lifetime2[4],
            lifetime2[5],
            lifetime2[6],
            lifetime2[7],
        );
        println!(
            "LifeTime3      {:X?}",
            self.read_bytes(ec, ManufReg::LifeTimeDataBlock3 as u16, 4)?
        );
        println!(
            "LifeTime4      {:X?}",
            self.read_bytes(ec, ManufReg::LifeTimeDataBlock4 as u16, 32)?
        );
        println!(
            "LifeTime5      {:X?}",
            self.read_bytes(ec, ManufReg::LifeTimeDataBlock5 as u16, 32)?
        );

        // Seal back again after we're finished
        self.seal(ec).unwrap();

        Ok(())
    }
}
