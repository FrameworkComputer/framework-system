// Smart Battery System (SBS) protocol support
// Reference: https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf
// Based on driver/battery/smart.c and include/battery_smart.h from EC codebase

use alloc::string::String;

use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcResult};

#[repr(u16)]
enum SmartBatReg {
    Mode = 0x03,
    Temp = 0x08,
    Voltage = 0x09,
    CycleCount = 0x17,
    ManufactureDate = 0x1B,
    SerialNum = 0x1C,
    ManufacturerName = 0x20,
    DeviceName = 0x21,
    CellVoltage1 = 0x3C,
    CellVoltage2 = 0x3D,
    CellVoltage3 = 0x3E,
    CellVoltage4 = 0x3F,
}

pub struct SmartBattery {
    i2c_port: u8,
    i2c_addr: u16,
}

impl Default for SmartBattery {
    fn default() -> Self {
        Self::new()
    }
}

impl SmartBattery {
    pub fn new() -> Self {
        SmartBattery {
            // Same on all our Nuvoton ECs
            i2c_port: 3,
            // 0x0B 7-bit, 0x16 8-bit address
            // Same for all our batteries, they use the same IC
            i2c_addr: 0x16,
        }
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
        // SMBus strings are length-prefixed
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 0x20)?;
        i2c_response.is_successful()?;
        // First byte is the returned string length
        let str_bytes = &i2c_response.data[1..=(i2c_response.data[0] as usize)];
        Ok(String::from_utf8_lossy(str_bytes).to_string())
    }

    /// Print basic battery information (sealed data, no unseal required)
    pub fn dump_data(&self, ec: &CrosEc) -> EcResult<()> {
        println!(
            "Mode:          0x{:04X}",
            self.read_i16(ec, SmartBatReg::Mode as u16)?
        );
        println!(
            "Serial Num:    {:04X}",
            self.read_i16(ec, SmartBatReg::SerialNum as u16)?
        );

        // ManufactureDate format: Year*512 + Month*32 + Day (year offset from 1980)
        let mfg_date = self.read_i16(ec, SmartBatReg::ManufactureDate as u16)?;
        let day = mfg_date & 0x1F;
        let month = (mfg_date >> 5) & 0x0F;
        let year = (mfg_date >> 9) + 1980;
        println!("Manuf Date:    {:04}-{:02}-{:02}", year, month, day);

        // Temperature is in 0.1K units, convert to Celsius
        let temp = self.read_i16(ec, SmartBatReg::Temp as u16)?;
        let temp_c = (temp as f32 / 10.0) - 273.15;
        println!("Temperature:   {:.1}C", temp_c);

        let voltage = self.read_i16(ec, SmartBatReg::Voltage as u16)?;
        println!("Voltage:       {}.{:03}V", voltage / 1000, voltage % 1000);

        let cell1_v = self.read_i16(ec, SmartBatReg::CellVoltage1 as u16)?;
        println!("  Cell 1:      {}.{:03}V", cell1_v / 1000, cell1_v % 1000);
        let cell2_v = self.read_i16(ec, SmartBatReg::CellVoltage2 as u16)?;
        println!("  Cell 2:      {}.{:03}V", cell2_v / 1000, cell2_v % 1000);
        let cell3_v = self.read_i16(ec, SmartBatReg::CellVoltage3 as u16)?;
        println!("  Cell 3:      {}.{:03}V", cell3_v / 1000, cell3_v % 1000);
        let cell4_v = self.read_i16(ec, SmartBatReg::CellVoltage4 as u16)?;
        println!("  Cell 4:      {}.{:03}V", cell4_v / 1000, cell4_v % 1000);

        println!(
            "Cycle Count:   {}",
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

        Ok(())
    }
}
