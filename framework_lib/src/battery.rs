// https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf?ts=1763375446472
// driver/battery/smart.c
// include/battery_smart.h
use alloc::vec::Vec;

use sha1::{Sha1, Digest};
use std::thread;
use std::time::Duration;

use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcResult, EcError};

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
    CellVoltage1 = 0x3C,
    CellVoltage2 = 0x3D,
    CellVoltage3 = 0x3E,
    CellVoltage4 = 0x3F,
    Authenticate = 0x2F,
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
    PFStatus = 0x53,
    OperationStatus = 0x54,
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

/// Calculates the HMAC using TI's specific nested SHA-1 method.
/// Formula: SHA1( Key || SHA1( Key || Challenge ) )
fn calculate_ti_hmac(key: &[u8; 16], challenge: &[u8; 20]) -> [u8; 20] {
    // 1. Inner Hash: SHA1( Key + Challenge )
    let mut inner_hasher = Sha1::new();
    inner_hasher.update(key);
    inner_hasher.update(challenge);
    let inner_digest = inner_hasher.finalize();

    // 2. Outer Hash: SHA1( Key + Inner_Digest )
    let mut outer_hasher = Sha1::new();
    outer_hasher.update(key);
    outer_hasher.update(inner_digest);
    let outer_digest = outer_hasher.finalize();

    // Convert GenericArray to standard [u8; 20]
    let mut result = [0u8; 20];
    result.copy_from_slice(&outer_digest);
    result
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


    fn i2c_write(&self, ec: &CrosEc, addr: u16, data: &[u8]) -> EcResult<()> {
        i2c_write(ec, self.i2c_port, self.i2c_addr >> 1, addr, data)?;
        Ok(())
    }
    fn i2c_read(&self, ec: &CrosEc, addr: u16, len: u16) -> EcResult<Vec<u8>> {
        self.read_bytes(ec, addr, len)
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

    pub fn authenticate_battery(&self, ec: &CrosEc, auth_key: &[u8; 16]) -> EcResult<bool> {
        // 1. Generate a random 20-byte challenge
        // In production, use `rand::random()` to generate this.
        let challenge: [u8; 20] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
            0x11, 0x12, 0x13, 0x14
        ];

        println!("Step 1: Sending Challenge...");

        // SMBus Block Write format: [Command, Byte_Count, Data...]
        let mut write_buf = Vec::new();
        write_buf.push(20);               // Byte Count (0x14)
        write_buf.extend_from_slice(&challenge);

        self.i2c_write(ec, SmartBatReg::Authenticate as u16, &write_buf)?;

        // 2. Wait for the gauge to calculate (Datasheet says 250ms)
        println!("Step 2: Waiting 250ms...");
        thread::sleep(Duration::from_millis(250));

        // 3. Calculate expected result locally while waiting
        let expected_response = calculate_ti_hmac(&auth_key, &challenge);

        // 4. Read Response
        // SMBus Block Read: Write Command -> Repeated Start -> Read [Len] + [Data]
        println!("Step 3: Reading Response...");

        // For block read, we usually write the command register first
        self.i2c_write(ec, SmartBatReg::Authenticate as u16, &[])?;

        // Read 21 bytes (1 byte length + 20 bytes signature)
        // TODO: Read without writing register first
        let raw_response = self.i2c_read(ec, 0x00, 20)?;

        // 5. Parse and Compare
        if raw_response.len() < 20 {
            return Err(EcError::DeviceError("Response too short".to_string()));
        }

        let device_response = &raw_response[1..20];

        println!("Expected: {:02X?}", expected_response);
        println!("Received: {:02X?}", device_response);

        if device_response == expected_response {
            println!("SUCCESS: Battery is genuine.");
            Ok(true)
        } else {
            println!("FAILURE: Signature mismatch.");
            Ok(false)
        }
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
        let cell1_v = self.read_i16(ec, SmartBatReg::CellVoltage1 as u16)?;
        println!(
            "  Cell 1:      {}.{}V",
            cell1_v / 1000,
            cell1_v % 1000
        );
        let cell2_v = self.read_i16(ec, SmartBatReg::CellVoltage2 as u16)?;
        println!(
            "  Cell 2:      {}.{}V",
            cell2_v / 1000,
            cell2_v % 1000
        );
        let cell3_v = self.read_i16(ec, SmartBatReg::CellVoltage3 as u16)?;
        println!(
            "  Cell 3:      {}.{}V",
            cell3_v / 1000,
            cell3_v % 1000
        );
        let cell4_v = self.read_i16(ec, SmartBatReg::CellVoltage4 as u16)?;
        println!(
            "  Cell 4:      {}.{}V",
            cell4_v / 1000,
            cell4_v % 1000
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

        // Dummy code. Do not push real authentication key!
        // self.authenticate_battery(ec, &[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

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
            "OperationStatus{:?}",
            self.read_i16(&ec, ManufReg::OperationStatus as u16)?
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
            self.read_i16(&ec, ManufReg::PFStatus as u16)?
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
