// Smart Battery System (SBS) protocol support
// Reference: https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf
// Based on driver/battery/smart.c and include/battery_smart.h from EC codebase

use alloc::string::String;
use alloc::vec::Vec;

use std::io::{self, Write};

use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcError, EcResult};

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

/// ManufacturerAccess block registers
/// Needs unseal for access
/// On EC Console can read these with battmfgacc command if CONFIG_CMD_BATT_MFG_ACCESS
#[repr(u16)]
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

/// Decode OperationStatus register bits (BQ40z50)
/// Based on TI bq40z50 device definitions
fn decode_operation_status(value: u32) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if value & (1 << 0) != 0 {
        flags.push("PRES (Cell Voltages OK)");
    }
    if value & (1 << 1) != 0 {
        flags.push("DSG (Discharge FET On)");
    }
    if value & (1 << 2) != 0 {
        flags.push("CHG (Charge FET On)");
    }
    if value & (1 << 3) != 0 {
        flags.push("PCHG (Precharge FET On)");
    }
    if value & (1 << 5) != 0 {
        flags.push("FUSE (Fuse Active)");
    }
    if value & (1 << 7) != 0 {
        flags.push("BTP_INT (Battery Trip Point)");
    }
    // Bits 8-9: Security mode (00=Reserved, 01=FullAccess, 10=Unsealed, 11=Sealed)
    let sec_mode = (value >> 8) & 0x03;
    match sec_mode {
        1 => flags.push("SEC=FullAccess"),
        2 => flags.push("SEC=Unsealed"),
        3 => flags.push("SEC=Sealed"),
        _ => {}
    }
    if value & (1 << 10) != 0 {
        flags.push("SDV (Shutdown Low Voltage)");
    }
    if value & (1 << 11) != 0 {
        flags.push("SS (Safety Status)");
    }
    if value & (1 << 12) != 0 {
        flags.push("PF (Permanent Failure)");
    }
    if value & (1 << 13) != 0 {
        flags.push("XDSG (Discharge Disabled)");
    }
    if value & (1 << 14) != 0 {
        flags.push("XCHG (Charge Disabled)");
    }
    if value & (1 << 15) != 0 {
        flags.push("SLEEP (Sleep Mode)");
    }
    if value & (1 << 16) != 0 {
        flags.push("SDM (Shutdown Command)");
    }
    if value & (1 << 17) != 0 {
        flags.push("LED (LED Display)");
    }
    if value & (1 << 18) != 0 {
        flags.push("AUTH (Authentication)");
    }
    if value & (1 << 19) != 0 {
        flags.push("AUTOCALM (Auto CC Offset)");
    }
    if value & (1 << 20) != 0 {
        flags.push("CAL (Calibration)");
    }
    if value & (1 << 21) != 0 {
        flags.push("CAL_OFFSET (Cal Offset)");
    }
    if value & (1 << 22) != 0 {
        flags.push("XL (400kHz Mode)");
    }
    if value & (1 << 23) != 0 {
        flags.push("SLEEPM (Sleep Cmd Active)");
    }
    if value & (1 << 24) != 0 {
        flags.push("INIT (Initialization)");
    }
    if value & (1 << 25) != 0 {
        flags.push("SMBLCAL (SMBus Cal)");
    }
    if value & (1 << 26) != 0 {
        flags.push("SLPAD (Sleep via Adapter)");
    }
    if value & (1 << 27) != 0 {
        flags.push("SLPCC (Sleep via CC)");
    }
    if value & (1 << 28) != 0 {
        flags.push("CB (Cell Balancing)");
    }
    if value & (1 << 29) != 0 {
        flags.push("EMSHUT (Emergency Shutdown)");
    }
    flags
}

/// Decode SafetyAlert/SafetyStatus register bits (BQ40z50)
/// Based on TI BQ40z50 Technical Reference Manual
fn decode_safety_status(value: u32) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if value & (1 << 0) != 0 {
        flags.push("CUV (Cell Under-Voltage)");
    }
    if value & (1 << 1) != 0 {
        flags.push("COV (Cell Over-Voltage)");
    }
    if value & (1 << 2) != 0 {
        flags.push("OCC1 (Over-Current Charge Tier1)");
    }
    if value & (1 << 3) != 0 {
        flags.push("OCC2 (Over-Current Charge Tier2)");
    }
    if value & (1 << 4) != 0 {
        flags.push("OCD1 (Over-Current Discharge Tier1)");
    }
    if value & (1 << 5) != 0 {
        flags.push("OCD2 (Over-Current Discharge Tier2)");
    }
    if value & (1 << 7) != 0 {
        flags.push("OCDL (Over-Current Discharge Latch)");
    }
    if value & (1 << 9) != 0 {
        flags.push("SCCL (Short-Circuit Charge Latch)");
    }
    if value & (1 << 11) != 0 {
        flags.push("SCDL (Short-Circuit Discharge Latch)");
    }
    if value & (1 << 12) != 0 {
        flags.push("OTC (Over-Temp Charge)");
    }
    if value & (1 << 13) != 0 {
        flags.push("OTD (Over-Temp Discharge)");
    }
    if value & (1 << 14) != 0 {
        flags.push("CUVC (Cell Under-Voltage Compensated)");
    }
    if value & (1 << 16) != 0 {
        flags.push("OTF (Over-Temp FET)");
    }
    if value & (1 << 18) != 0 {
        flags.push("PTO (Precharge Timeout)");
    }
    if value & (1 << 19) != 0 {
        flags.push("PTOS (Precharge Timeout Suspend)");
    }
    if value & (1 << 20) != 0 {
        flags.push("CTO (Charge Timeout)");
    }
    if value & (1 << 21) != 0 {
        flags.push("CTOS (Charge Timeout Suspend)");
    }
    if value & (1 << 22) != 0 {
        flags.push("OC (Overcharge)");
    }
    if value & (1 << 23) != 0 {
        flags.push("CHGC (Overcharge Current)");
    }
    if value & (1 << 24) != 0 {
        flags.push("CHGV (Overcharge Voltage)");
    }
    if value & (1 << 25) != 0 {
        flags.push("PCHGC (Over Precharge Current)");
    }
    if value & (1 << 26) != 0 {
        flags.push("UTC (Under-Temp Charge)");
    }
    if value & (1 << 27) != 0 {
        flags.push("UTD (Under-Temp Discharge)");
    }
    flags
}

/// Decode PFAlert/PFStatus register bits (BQ40z50)
/// Based on TI BQ40z50 Technical Reference Manual
fn decode_pf_status(value: u32) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if value & (1 << 0) != 0 {
        flags.push("SUV (Safety Cell Under-Voltage)");
    }
    if value & (1 << 1) != 0 {
        flags.push("SOV (Safety Cell Over-Voltage)");
    }
    if value & (1 << 2) != 0 {
        flags.push("SOCC (Safety Over-Current Charge)");
    }
    if value & (1 << 3) != 0 {
        flags.push("SOCD (Safety Over-Current Discharge)");
    }
    if value & (1 << 4) != 0 {
        flags.push("SOT (Safety Over-Temp Cell)");
    }
    if value & (1 << 5) != 0 {
        flags.push("SOTF (Safety Over-Temp FET)");
    }
    if value & (1 << 6) != 0 {
        flags.push("VIMR (Voltage Imbalance at Rest)");
    }
    if value & (1 << 7) != 0 {
        flags.push("VIMA (Voltage Imbalance Active)");
    }
    if value & (1 << 8) != 0 {
        flags.push("QIM (QMax Imbalance)");
    }
    if value & (1 << 9) != 0 {
        flags.push("CB (Cell Balancing Failure)");
    }
    if value & (1 << 10) != 0 {
        flags.push("IMP (Impedance Failure)");
    }
    if value & (1 << 11) != 0 {
        flags.push("CD (Coulomb Counter Failure)");
    }
    if value & (1 << 12) != 0 {
        flags.push("FUSE (Chemical Fuse Failure)");
    }
    if value & (1 << 13) != 0 {
        flags.push("AFER (AFE Register Failure)");
    }
    if value & (1 << 14) != 0 {
        flags.push("AFEC (AFE Communication Failure)");
    }
    if value & (1 << 15) != 0 {
        flags.push("2LVL (Second Level Safety)");
    }
    if value & (1 << 16) != 0 {
        flags.push("PTC (Open PTC Failure)");
    }
    if value & (1 << 17) != 0 {
        flags.push("CFETF (Charge FET Failure)");
    }
    if value & (1 << 18) != 0 {
        flags.push("DFETF (Discharge FET Failure)");
    }
    if value & (1 << 19) != 0 {
        flags.push("TS1 (Open Thermistor TS1)");
    }
    if value & (1 << 20) != 0 {
        flags.push("TS2 (Open Thermistor TS2)");
    }
    if value & (1 << 21) != 0 {
        flags.push("TS3 (Open Thermistor TS3)");
    }
    if value & (1 << 22) != 0 {
        flags.push("TS4 (Open Thermistor TS4)");
    }
    if value & (1 << 23) != 0 {
        flags.push("DFW (Data Flash Wearout)");
    }
    if value & (1 << 24) != 0 {
        flags.push("HWMX (HW Max Cell Voltage)");
    }
    flags
}

/// Print status flags with hex value (for Safety/PF registers)
fn print_status_flags(label: &str, value: u32, flags: Vec<&str>) {
    if value == 0 {
        println!("{}: (OK)", label);
    } else {
        println!("{}: 0x{:08X}", label, value);
        for flag in flags {
            println!("  - {}", flag);
        }
    }
}

/// Print operation status flags on multiple lines
fn print_operation_status(value: u32) {
    let flags = decode_operation_status(value);
    println!("Operation Status: 0x{:08X}", value);
    for flag in flags {
        println!("  - {}", flag);
    }
}

/// Reads a line from stdin without echoing (for sensitive input like keys)
#[cfg(unix)]
fn read_password() -> io::Result<String> {
    use nix::sys::termios::{self, LocalFlags, SetArg};
    use std::io::BufRead;
    use std::os::fd::AsFd;

    let stdin = io::stdin();
    let fd = stdin.as_fd();

    // Save original terminal settings
    let original = termios::tcgetattr(fd)?;

    // Disable echo
    let mut noecho = original.clone();
    noecho.local_flags.remove(LocalFlags::ECHO);
    termios::tcsetattr(fd, SetArg::TCSANOW, &noecho)?;

    // Read the line
    let mut input = String::new();
    let result = stdin.lock().read_line(&mut input);

    // Restore original settings
    termios::tcsetattr(fd, SetArg::TCSANOW, &original)?;

    // Print newline since echo was disabled
    println!();

    result?;
    Ok(input)
}

/// Reads a line from stdin without echoing (for sensitive input like keys)
#[cfg(windows)]
fn read_password() -> io::Result<String> {
    use std::io::BufRead;
    use windows::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, CONSOLE_MODE, ENABLE_ECHO_INPUT,
        STD_INPUT_HANDLE,
    };

    unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to get stdin handle: {}", e),
            )
        })?;

        let mut mode = CONSOLE_MODE::default();
        GetConsoleMode(handle, &mut mode).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to get console mode: {}", e),
            )
        })?;

        // Disable echo
        let noecho = CONSOLE_MODE(mode.0 & !ENABLE_ECHO_INPUT.0);
        SetConsoleMode(handle, noecho).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to set console mode: {}", e),
            )
        })?;

        // Read the line
        let mut input = String::new();
        let result = io::stdin().lock().read_line(&mut input);

        // Restore original mode
        let _ = SetConsoleMode(handle, mode);

        // Print newline since echo was disabled
        println!();

        result?;
        Ok(input)
    }
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

    fn unseal(&self, ec: &CrosEc, key1: u16, key2: u16) -> EcResult<()> {
        i2c_write_block(
            ec,
            self.i2c_port,
            self.i2c_addr >> 1,
            0x00,
            &key1.to_le_bytes(),
        )?;
        i2c_write_block(
            ec,
            self.i2c_port,
            self.i2c_addr >> 1,
            0x00,
            &key2.to_le_bytes(),
        )?;
        Ok(())
    }

    fn seal(&self, ec: &CrosEc) -> EcResult<()> {
        i2c_write_block(ec, self.i2c_port, self.i2c_addr >> 1, 0x00, &[0x30, 0x00])?;
        Ok(())
    }

    fn read_i16(&self, ec: &CrosEc, addr: u16) -> EcResult<u16> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 0x02)?;
        i2c_response.is_successful()?;
        Ok(u16::from_le_bytes([
            i2c_response.data[0],
            i2c_response.data[1],
        ]))
    }

    /// Read a 32-bit value from a ManufacturerAccess block command (SMBus block format with length prefix)
    fn read_i32(&self, ec: &CrosEc, addr: u16) -> EcResult<u32> {
        // ManufacturerAccess block commands return data in SMBus block format:
        // Byte 0: Length, Bytes 1-4: Data
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, 0x05)?;
        i2c_response.is_successful()?;
        let len = i2c_response.data[0];
        if len != 4 {
            return Err(EcError::DeviceError(format!(
                "Expected 4 bytes but got {} from register 0x{:02X}",
                len, addr
            )));
        }
        Ok(u32::from_le_bytes([
            i2c_response.data[1],
            i2c_response.data[2],
            i2c_response.data[3],
            i2c_response.data[4],
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

    /// Read a block of bytes with expected length
    fn read_bytes(&self, ec: &CrosEc, addr: u16, len: u16) -> EcResult<Vec<u8>> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, len + 1)?;
        i2c_response.is_successful()?;
        let actual_len = i2c_response.data[0];
        if actual_len != len as u8 {
            return Err(EcError::DeviceError(format!(
                "Expected {} bytes but got {} from register 0x{:02X}",
                len, actual_len, addr
            )));
        }
        Ok(i2c_response.data[1..].to_vec())
    }

    /// Read a block of bytes, returning whatever length the device provides
    fn read_block(&self, ec: &CrosEc, addr: u16, max_len: u16) -> EcResult<Vec<u8>> {
        let i2c_response = i2c_read(ec, self.i2c_port, self.i2c_addr >> 1, addr, max_len + 1)?;
        i2c_response.is_successful()?;
        let actual_len = i2c_response.data[0] as usize;
        Ok(i2c_response.data[1..=actual_len].to_vec())
    }

    /// Print battery information interactively (prompts for unseal key)
    pub fn dump_data(&self, ec: &CrosEc) -> EcResult<()> {
        // Basic registers (no unseal required)
        println!(
            "Mode:          0x{:04X}",
            self.read_i16(ec, SmartBatReg::Mode as u16)?
        );
        let serial_num = self.read_i16(ec, SmartBatReg::SerialNum as u16)?;
        println!("Serial Num:    {:04X}", serial_num);

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

        // Prompt for unseal key to access ManufacturerAccess data
        print!("Enter unseal key in hex (e.g. 04143672), or press enter to skip: ");
        io::stdout()
            .flush()
            .map_err(|e| EcError::DeviceError(format!("Failed to flush stdout: {}", e)))?;
        let input_text = read_password()
            .map_err(|e| EcError::DeviceError(format!("Failed to read key: {}", e)))?;
        let input_text = input_text.trim();

        if !input_text.is_empty() {
            let key: u32 = u32::from_str_radix(input_text, 16)
                .map_err(|e| EcError::DeviceError(format!("Invalid key: {}", e)))?;
            self.unseal(ec, (key >> 16) as u16, key as u16)?;

            let soh = self.read_bytes(ec, ManufReg::Soh as u16, 4)?;
            println!(
                "StateOfHealth: {}mAh, {}.{:02}Wh",
                u16::from_le_bytes([soh[0], soh[1]]),
                u16::from_le_bytes([soh[2], soh[3]]) / 100,
                u16::from_le_bytes([soh[2], soh[3]]) % 100,
            );

            let operation_status = self.read_i32(ec, ManufReg::OperationStatus as u16)?;
            print_operation_status(operation_status);

            let safety_alert = self.read_i32(ec, ManufReg::SafetyAlert as u16)?;
            print_status_flags(
                "Safety Alert",
                safety_alert,
                decode_safety_status(safety_alert),
            );

            let safety_status = self.read_i32(ec, ManufReg::SafetyStatus as u16)?;
            print_status_flags(
                "Safety Status",
                safety_status,
                decode_safety_status(safety_status),
            );

            let pf_alert = self.read_i32(ec, ManufReg::PFAlert as u16)?;
            print_status_flags("PF Alert", pf_alert, decode_pf_status(pf_alert));

            let pf_status = self.read_i32(ec, ManufReg::PFStatus as u16)?;
            print_status_flags("PF Status", pf_status, decode_pf_status(pf_status));

            // LifeTime Data Blocks
            let lt1 = self.read_bytes(ec, ManufReg::LifeTimeDataBlock1 as u16, 32)?;
            println!("LifeTime1:");
            println!(
                "  Cell 1 Max Voltage: {}mV",
                u16::from_le_bytes([lt1[0], lt1[1]])
            );
            println!(
                "         Min Voltage: {}mV",
                u16::from_le_bytes([lt1[8], lt1[9]])
            );
            println!(
                "  Cell 2 Max Voltage: {}mV",
                u16::from_le_bytes([lt1[2], lt1[3]])
            );
            println!(
                "         Min Voltage: {}mV",
                u16::from_le_bytes([lt1[10], lt1[11]])
            );
            println!(
                "  Cell 3 Max Voltage: {}mV",
                u16::from_le_bytes([lt1[4], lt1[5]])
            );
            println!(
                "         Min Voltage: {}mV",
                u16::from_le_bytes([lt1[12], lt1[13]])
            );
            println!(
                "  Cell 4 Max Voltage: {}mV",
                u16::from_le_bytes([lt1[6], lt1[7]])
            );
            println!(
                "         Min Voltage: {}mV",
                u16::from_le_bytes([lt1[14], lt1[15]])
            );
            println!(
                "  Max Delta Cell Voltage: {}mV",
                u16::from_le_bytes([lt1[16], lt1[17]])
            );
            println!(
                "  Max Charge Current:     {:.2}A",
                u16::from_le_bytes([lt1[18], lt1[19]]) as f32 / 1000.0
            );
            println!(
                "  Max Discharge Current:  {:.2}A",
                i16::from_le_bytes([lt1[20], lt1[21]]).unsigned_abs() as f32 / 1000.0
            );
            println!(
                "  Max Avg Dsg Current:    {:.2}A",
                i16::from_le_bytes([lt1[22], lt1[23]]).unsigned_abs() as f32 / 1000.0
            );
            println!(
                "  Max Avg Dsg Power:      {:.1}W",
                u16::from_le_bytes([lt1[24], lt1[25]]) as f32 / 1000.0
            );
            println!("  Max Temp Cell:          {}C", lt1[26]);
            println!("  Min Temp Cell:          {}C", lt1[27]);
            println!("  Max Delta Cell Temp:    {}C", lt1[28]);
            println!("  Max Temp Int Sensor:    {}C", lt1[29]);
            println!("  Min Temp Int Sensor:    {}C", lt1[30]);
            println!("  Max Temp FET:           {}C", lt1[31]);

            let lt2 = self.read_bytes(ec, ManufReg::LifeTimeDataBlock2 as u16, 20)?;
            println!("LifeTime2:");
            println!("  No. of Shutdowns:       {}", lt2[0]);
            println!("  No. of Partial Resets:  {}", lt2[1]);
            println!("  No. of Full Resets:     {}", lt2[2]);
            println!("  No. of WDT Resets:      {}", lt2[3]);
            let cb1 = u32::from_le_bytes([lt2[4], lt2[5], lt2[6], lt2[7]]);
            let cb2 = u32::from_le_bytes([lt2[8], lt2[9], lt2[10], lt2[11]]);
            let cb3 = u32::from_le_bytes([lt2[12], lt2[13], lt2[14], lt2[15]]);
            let cb4 = u32::from_le_bytes([lt2[16], lt2[17], lt2[18], lt2[19]]);
            println!(
                "  CB Time Cell 1:         {}s ({:.1}h)",
                cb1,
                cb1 as f64 / 3600.0
            );
            println!(
                "  CB Time Cell 2:         {}s ({:.1}h)",
                cb2,
                cb2 as f64 / 3600.0
            );
            println!(
                "  CB Time Cell 3:         {}s ({:.1}h)",
                cb3,
                cb3 as f64 / 3600.0
            );
            println!(
                "  CB Time Cell 4:         {}s ({:.1}h)",
                cb4,
                cb4 as f64 / 3600.0
            );

            let lt3 = self.read_block(ec, ManufReg::LifeTimeDataBlock3 as u16, 16)?;
            if !lt3.is_empty() {
                println!("LifeTime3:");
                if lt3.len() >= 4 {
                    println!(
                        "  Total FW Runtime:       {}h",
                        u16::from_le_bytes([lt3[0], lt3[1]])
                    );
                    println!(
                        "  Time in Under Temp:     {}h",
                        u16::from_le_bytes([lt3[2], lt3[3]])
                    );
                }
                if lt3.len() >= 16 {
                    println!(
                        "  Time in Low Temp:       {}h",
                        u16::from_le_bytes([lt3[4], lt3[5]])
                    );
                    println!(
                        "  Time in Std Temp Low:   {}h",
                        u16::from_le_bytes([lt3[6], lt3[7]])
                    );
                    println!(
                        "  Time in Std Temp:       {}h",
                        u16::from_le_bytes([lt3[8], lt3[9]])
                    );
                    println!(
                        "  Time in Std Temp High:  {}h",
                        u16::from_le_bytes([lt3[10], lt3[11]])
                    );
                    println!(
                        "  Time in High Temp:      {}h",
                        u16::from_le_bytes([lt3[12], lt3[13]])
                    );
                    println!(
                        "  Time in Over Temp:      {}h",
                        u16::from_le_bytes([lt3[14], lt3[15]])
                    );
                }
            }

            let lt4 = self.read_block(ec, ManufReg::LifeTimeDataBlock4 as u16, 32)?;
            if lt4.len() >= 32 {
                println!("LifeTime4:");
                println!(
                    "  Cell Over-Voltage:        {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[0], lt4[1]]),
                    u16::from_le_bytes([lt4[2], lt4[3]])
                );
                println!(
                    "  Cell Under-Voltage:       {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[4], lt4[5]]),
                    u16::from_le_bytes([lt4[6], lt4[7]])
                );
                println!(
                    "  Over-Current Discharge 1: {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[8], lt4[9]]),
                    u16::from_le_bytes([lt4[10], lt4[11]])
                );
                println!(
                    "  Over-Current Discharge 2: {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[12], lt4[13]]),
                    u16::from_le_bytes([lt4[14], lt4[15]])
                );
                println!(
                    "  Over-Current Charge 1:    {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[16], lt4[17]]),
                    u16::from_le_bytes([lt4[18], lt4[19]])
                );
                println!(
                    "  Over-Current Charge 2:    {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[20], lt4[21]]),
                    u16::from_le_bytes([lt4[22], lt4[23]])
                );
                println!(
                    "  Open Load Detection:      {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[24], lt4[25]]),
                    u16::from_le_bytes([lt4[26], lt4[27]])
                );
                println!(
                    "  Short-Circuit Discharge:  {} (last @ cycle {})",
                    u16::from_le_bytes([lt4[28], lt4[29]]),
                    u16::from_le_bytes([lt4[30], lt4[31]])
                );
            }

            let lt5 = self.read_block(ec, ManufReg::LifeTimeDataBlock5 as u16, 32)?;
            if lt5.len() >= 32 {
                println!("LifeTime5:");
                println!(
                    "  Short-Circuit Charge:     {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[0], lt5[1]]),
                    u16::from_le_bytes([lt5[2], lt5[3]])
                );
                println!(
                    "  Over-Temp Charge:         {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[4], lt5[5]]),
                    u16::from_le_bytes([lt5[6], lt5[7]])
                );
                println!(
                    "  Over-Temp Discharge:      {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[8], lt5[9]]),
                    u16::from_le_bytes([lt5[10], lt5[11]])
                );
                println!(
                    "  Over-Temp FET:            {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[12], lt5[13]]),
                    u16::from_le_bytes([lt5[14], lt5[15]])
                );
                println!(
                    "  Valid Charge Terminations:{} (last @ cycle {})",
                    u16::from_le_bytes([lt5[16], lt5[17]]),
                    u16::from_le_bytes([lt5[18], lt5[19]])
                );
                println!(
                    "  QMax Updates:             {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[20], lt5[21]]),
                    u16::from_le_bytes([lt5[22], lt5[23]])
                );
                println!(
                    "  Resistance Updates:       {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[24], lt5[25]]),
                    u16::from_le_bytes([lt5[26], lt5[27]])
                );
                println!(
                    "  Resistance Update Fails:  {} (last @ cycle {})",
                    u16::from_le_bytes([lt5[28], lt5[29]]),
                    u16::from_le_bytes([lt5[30], lt5[31]])
                );
            }

            self.seal(ec)?;
        }

        Ok(())
    }
}
