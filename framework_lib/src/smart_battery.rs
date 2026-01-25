// Smart Battery System (SBS) protocol support
// Reference: https://www.ti.com/lit/ug/sluua43a/sluua43a.pdf
// Based on driver/battery/smart.c and include/battery_smart.h from EC codebase

use alloc::string::String;
use alloc::vec::Vec;

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcError, EcResult};

/// Raw battery data that can be dumped to a file and loaded later
#[derive(Default)]
pub struct BatteryData {
    pub mode: u16,
    pub serial_num: u16,
    pub manufacture_date: u16,
    pub temperature: u16,
    pub voltage: u16,
    pub cell_voltage1: u16,
    pub cell_voltage2: u16,
    pub cell_voltage3: u16,
    pub cell_voltage4: u16,
    pub cycle_count: u16,
    pub device_name: String,
    pub manufacturer_name: String,
    // Unsealed data (may be empty if not unsealed)
    pub state_of_health: Vec<u8>,
    pub operation_status: u32,
    pub safety_alert: u32,
    pub safety_status: u32,
    pub pf_alert: u32,
    pub pf_status: u32,
    pub lifetime1: Vec<u8>,
    pub lifetime2: Vec<u8>,
    pub lifetime3: Vec<u8>,
    pub lifetime4: Vec<u8>,
    pub lifetime5: Vec<u8>,
}

impl BatteryData {
    /// Write raw data to a file in a simple text format
    pub fn write_to_file(&self, path: &Path) -> io::Result<()> {
        let mut file = File::create(path)?;
        writeln!(file, "# Smart Battery Raw Data Dump")?;
        writeln!(file, "# Format: key=hex_value or key=string")?;
        writeln!(file)?;
        writeln!(file, "mode={:04X}", self.mode)?;
        writeln!(file, "serial_num={:04X}", self.serial_num)?;
        writeln!(file, "manufacture_date={:04X}", self.manufacture_date)?;
        writeln!(file, "temperature={:04X}", self.temperature)?;
        writeln!(file, "voltage={:04X}", self.voltage)?;
        writeln!(file, "cell_voltage1={:04X}", self.cell_voltage1)?;
        writeln!(file, "cell_voltage2={:04X}", self.cell_voltage2)?;
        writeln!(file, "cell_voltage3={:04X}", self.cell_voltage3)?;
        writeln!(file, "cell_voltage4={:04X}", self.cell_voltage4)?;
        writeln!(file, "cycle_count={:04X}", self.cycle_count)?;
        writeln!(file, "device_name={}", self.device_name)?;
        writeln!(file, "manufacturer_name={}", self.manufacturer_name)?;
        writeln!(
            file,
            "state_of_health={}",
            hex_encode(&self.state_of_health)
        )?;
        writeln!(file, "operation_status={:08X}", self.operation_status)?;
        writeln!(file, "safety_alert={:08X}", self.safety_alert)?;
        writeln!(file, "safety_status={:08X}", self.safety_status)?;
        writeln!(file, "pf_alert={:08X}", self.pf_alert)?;
        writeln!(file, "pf_status={:08X}", self.pf_status)?;
        writeln!(file, "lifetime1={}", hex_encode(&self.lifetime1))?;
        writeln!(file, "lifetime2={}", hex_encode(&self.lifetime2))?;
        writeln!(file, "lifetime3={}", hex_encode(&self.lifetime3))?;
        writeln!(file, "lifetime4={}", hex_encode(&self.lifetime4))?;
        writeln!(file, "lifetime5={}", hex_encode(&self.lifetime5))?;
        Ok(())
    }

    /// Read raw data from a dump file
    pub fn read_from_file(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut data = BatteryData::default();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                match key {
                    "mode" => data.mode = u16::from_str_radix(value, 16).unwrap_or(0),
                    "serial_num" => data.serial_num = u16::from_str_radix(value, 16).unwrap_or(0),
                    "manufacture_date" => {
                        data.manufacture_date = u16::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "temperature" => data.temperature = u16::from_str_radix(value, 16).unwrap_or(0),
                    "voltage" => data.voltage = u16::from_str_radix(value, 16).unwrap_or(0),
                    "cell_voltage1" => {
                        data.cell_voltage1 = u16::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "cell_voltage2" => {
                        data.cell_voltage2 = u16::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "cell_voltage3" => {
                        data.cell_voltage3 = u16::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "cell_voltage4" => {
                        data.cell_voltage4 = u16::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "cycle_count" => data.cycle_count = u16::from_str_radix(value, 16).unwrap_or(0),
                    "device_name" => data.device_name = value.to_string(),
                    "manufacturer_name" => data.manufacturer_name = value.to_string(),
                    "state_of_health" => data.state_of_health = hex_decode(value),
                    "operation_status" => {
                        data.operation_status = u32::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "safety_alert" => {
                        data.safety_alert = u32::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "safety_status" => {
                        data.safety_status = u32::from_str_radix(value, 16).unwrap_or(0)
                    }
                    "pf_alert" => data.pf_alert = u32::from_str_radix(value, 16).unwrap_or(0),
                    "pf_status" => data.pf_status = u32::from_str_radix(value, 16).unwrap_or(0),
                    "lifetime1" => data.lifetime1 = hex_decode(value),
                    "lifetime2" => data.lifetime2 = hex_decode(value),
                    "lifetime3" => data.lifetime3 = hex_decode(value),
                    "lifetime4" => data.lifetime4 = hex_decode(value),
                    "lifetime5" => data.lifetime5 = hex_decode(value),
                    _ => {} // Ignore unknown keys for forward compatibility
                }
            }
        }
        Ok(data)
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02X}", b)).collect()
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

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
        // Prompt for unseal key to access ManufacturerAccess data
        print!("Enter unseal key in hex (e.g. 04143672), or press enter to skip: ");
        io::stdout()
            .flush()
            .map_err(|e| EcError::DeviceError(format!("Failed to flush stdout: {}", e)))?;
        let input_text = read_password()
            .map_err(|e| EcError::DeviceError(format!("Failed to read key: {}", e)))?;
        let input_text = input_text.trim();

        let unseal_key = if input_text.is_empty() {
            None
        } else {
            Some(
                u32::from_str_radix(input_text, 16)
                    .map_err(|e| EcError::DeviceError(format!("Invalid key: {}", e)))?,
            )
        };

        let data = self.collect_data(ec, unseal_key)?;
        display_battery_data(&data);

        Ok(())
    }

    /// Collect raw battery data into a struct (for dumping or analysis)
    #[allow(clippy::field_reassign_with_default)]
    pub fn collect_data(&self, ec: &CrosEc, unseal_key: Option<u32>) -> EcResult<BatteryData> {
        let mut data = BatteryData::default();

        // Basic registers (no unseal required)
        data.mode = self.read_i16(ec, SmartBatReg::Mode as u16)?;
        data.serial_num = self.read_i16(ec, SmartBatReg::SerialNum as u16)?;
        data.manufacture_date = self.read_i16(ec, SmartBatReg::ManufactureDate as u16)?;
        data.temperature = self.read_i16(ec, SmartBatReg::Temp as u16)?;
        data.voltage = self.read_i16(ec, SmartBatReg::Voltage as u16)?;
        data.cell_voltage1 = self.read_i16(ec, SmartBatReg::CellVoltage1 as u16)?;
        data.cell_voltage2 = self.read_i16(ec, SmartBatReg::CellVoltage2 as u16)?;
        data.cell_voltage3 = self.read_i16(ec, SmartBatReg::CellVoltage3 as u16)?;
        data.cell_voltage4 = self.read_i16(ec, SmartBatReg::CellVoltage4 as u16)?;
        data.cycle_count = self.read_i16(ec, SmartBatReg::CycleCount as u16)?;
        data.device_name = self.read_string(ec, SmartBatReg::DeviceName as u16)?;
        data.manufacturer_name = self.read_string(ec, SmartBatReg::ManufacturerName as u16)?;

        // Unsealed data
        if let Some(key) = unseal_key {
            self.unseal(ec, (key >> 16) as u16, key as u16)?;

            data.state_of_health = self.read_bytes(ec, ManufReg::Soh as u16, 4)?;
            data.operation_status = self.read_i32(ec, ManufReg::OperationStatus as u16)?;
            data.safety_alert = self.read_i32(ec, ManufReg::SafetyAlert as u16)?;
            data.safety_status = self.read_i32(ec, ManufReg::SafetyStatus as u16)?;
            data.pf_alert = self.read_i32(ec, ManufReg::PFAlert as u16)?;
            data.pf_status = self.read_i32(ec, ManufReg::PFStatus as u16)?;
            data.lifetime1 = self.read_bytes(ec, ManufReg::LifeTimeDataBlock1 as u16, 32)?;
            data.lifetime2 = self.read_bytes(ec, ManufReg::LifeTimeDataBlock2 as u16, 20)?;
            data.lifetime3 = self.read_block(ec, ManufReg::LifeTimeDataBlock3 as u16, 16)?;
            data.lifetime4 = self.read_block(ec, ManufReg::LifeTimeDataBlock4 as u16, 32)?;
            data.lifetime5 = self.read_block(ec, ManufReg::LifeTimeDataBlock5 as u16, 32)?;

            self.seal(ec)?;
        }

        Ok(data)
    }

    /// Dump battery data to a file
    pub fn dump_to_file(&self, ec: &CrosEc, path: &Path) -> EcResult<()> {
        // Prompt for unseal key
        print!("Enter unseal key in hex (e.g. 04143672), or press enter to skip: ");
        io::stdout()
            .flush()
            .map_err(|e| EcError::DeviceError(format!("Failed to flush stdout: {}", e)))?;
        let input_text = read_password()
            .map_err(|e| EcError::DeviceError(format!("Failed to read key: {}", e)))?;
        let input_text = input_text.trim();

        let unseal_key = if input_text.is_empty() {
            None
        } else {
            Some(
                u32::from_str_radix(input_text, 16)
                    .map_err(|e| EcError::DeviceError(format!("Invalid key: {}", e)))?,
            )
        };

        let data = self.collect_data(ec, unseal_key)?;
        data.write_to_file(path)
            .map_err(|e| EcError::DeviceError(format!("Failed to write file: {}", e)))?;

        println!("Battery data saved to {}", path.display());
        Ok(())
    }
}

/// Display decoded battery data from a BatteryData struct
pub fn display_battery_data(data: &BatteryData) {
    println!("Mode:          0x{:04X}", data.mode);
    println!("Serial Num:    {:04X}", data.serial_num);

    let day = data.manufacture_date & 0x1F;
    let month = (data.manufacture_date >> 5) & 0x0F;
    let year = (data.manufacture_date >> 9) + 1980;
    println!("Manuf Date:    {:04}-{:02}-{:02}", year, month, day);

    let temp_c = (data.temperature as f32 / 10.0) - 273.15;
    println!("Temperature:   {:.1}C", temp_c);

    println!(
        "Voltage:       {}.{:03}V",
        data.voltage / 1000,
        data.voltage % 1000
    );
    println!(
        "  Cell 1:      {}.{:03}V",
        data.cell_voltage1 / 1000,
        data.cell_voltage1 % 1000
    );
    println!(
        "  Cell 2:      {}.{:03}V",
        data.cell_voltage2 / 1000,
        data.cell_voltage2 % 1000
    );
    println!(
        "  Cell 3:      {}.{:03}V",
        data.cell_voltage3 / 1000,
        data.cell_voltage3 % 1000
    );
    println!(
        "  Cell 4:      {}.{:03}V",
        data.cell_voltage4 / 1000,
        data.cell_voltage4 % 1000
    );
    println!("Cycle Count:   {}", data.cycle_count);
    println!("Device Name:   {}", data.device_name);
    println!("Manuf Name:    {}", data.manufacturer_name);

    // Unsealed data
    if !data.state_of_health.is_empty() {
        let soh = &data.state_of_health;
        println!(
            "StateOfHealth: {}mAh, {}.{:02}Wh",
            u16::from_le_bytes([soh[0], soh[1]]),
            u16::from_le_bytes([soh[2], soh[3]]) / 100,
            u16::from_le_bytes([soh[2], soh[3]]) % 100,
        );
        print_operation_status(data.operation_status);
        print_status_flags(
            "Safety Alert",
            data.safety_alert,
            decode_safety_status(data.safety_alert),
        );
        print_status_flags(
            "Safety Status",
            data.safety_status,
            decode_safety_status(data.safety_status),
        );
        print_status_flags("PF Alert", data.pf_alert, decode_pf_status(data.pf_alert));
        print_status_flags(
            "PF Status",
            data.pf_status,
            decode_pf_status(data.pf_status),
        );

        if data.lifetime1.len() >= 32 {
            let lt1 = &data.lifetime1;
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
        }

        if data.lifetime2.len() >= 20 {
            let lt2 = &data.lifetime2;
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
        }

        if !data.lifetime3.is_empty() {
            println!("LifeTime3:");
            if data.lifetime3.len() >= 4 {
                println!(
                    "  Total FW Runtime:       {}h",
                    u16::from_le_bytes([data.lifetime3[0], data.lifetime3[1]])
                );
                println!(
                    "  Time in Under Temp:     {}h",
                    u16::from_le_bytes([data.lifetime3[2], data.lifetime3[3]])
                );
            }
            if data.lifetime3.len() >= 16 {
                println!(
                    "  Time in Low Temp:       {}h",
                    u16::from_le_bytes([data.lifetime3[4], data.lifetime3[5]])
                );
                println!(
                    "  Time in Std Temp Low:   {}h",
                    u16::from_le_bytes([data.lifetime3[6], data.lifetime3[7]])
                );
                println!(
                    "  Time in Std Temp:       {}h",
                    u16::from_le_bytes([data.lifetime3[8], data.lifetime3[9]])
                );
                println!(
                    "  Time in Std Temp High:  {}h",
                    u16::from_le_bytes([data.lifetime3[10], data.lifetime3[11]])
                );
                println!(
                    "  Time in High Temp:      {}h",
                    u16::from_le_bytes([data.lifetime3[12], data.lifetime3[13]])
                );
                println!(
                    "  Time in Over Temp:      {}h",
                    u16::from_le_bytes([data.lifetime3[14], data.lifetime3[15]])
                );
            }
        }

        if !data.lifetime4.is_empty() {
            println!("LifeTime4:");
            if data.lifetime4.len() >= 32 {
                let lt4 = &data.lifetime4;
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
        }

        if !data.lifetime5.is_empty() {
            println!("LifeTime5:");
            if data.lifetime5.len() >= 32 {
                let lt5 = &data.lifetime5;
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
        }
    }

    // Print health analysis at the end
    analyze_health(data);
}

/// Analyze battery health and print a summary
pub fn analyze_health(data: &BatteryData) {
    println!("\n=== Battery Health Analysis ===\n");

    let mut issues: Vec<&str> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Check Safety/PF status
    if data.safety_status != 0 {
        issues.push("Safety status flags active");
    }
    if data.pf_status != 0 {
        issues.push("Permanent failure flags active");
    }

    // Check cell voltage balance (current)
    let cells = [
        data.cell_voltage1,
        data.cell_voltage2,
        data.cell_voltage3,
        data.cell_voltage4,
    ];
    let max_cell = cells.iter().max().unwrap_or(&0);
    let min_cell = cells.iter().min().unwrap_or(&0);
    let cell_delta = max_cell - min_cell;
    if cell_delta > 100 {
        warnings.push(format!(
            "Cell imbalance: {}mV difference (>100mV)",
            cell_delta
        ));
    }

    // Analyze lifetime data if available
    if data.lifetime1.len() >= 32 {
        let lt1 = &data.lifetime1;
        let max_delta = u16::from_le_bytes([lt1[16], lt1[17]]);
        if max_delta > 200 {
            warnings.push(format!(
                "Historical cell imbalance: {}mV max delta recorded",
                max_delta
            ));
        }

        // Check temperature extremes
        let max_temp = lt1[26];
        let min_temp = lt1[27];
        if max_temp > 55 {
            warnings.push(format!("High temperature recorded: {}C max", max_temp));
        }
        if min_temp < 5 {
            warnings.push(format!("Low temperature recorded: {}C min", min_temp));
        }
    }

    // Analyze safety events
    if data.lifetime4.len() >= 32 {
        let lt4 = &data.lifetime4;
        let cov_events = u16::from_le_bytes([lt4[0], lt4[1]]);
        let cuv_events = u16::from_le_bytes([lt4[4], lt4[5]]);
        let ocd1_events = u16::from_le_bytes([lt4[8], lt4[9]]);
        let ocd2_events = u16::from_le_bytes([lt4[12], lt4[13]]);
        let occ1_events = u16::from_le_bytes([lt4[16], lt4[17]]);
        let scd_events = u16::from_le_bytes([lt4[28], lt4[29]]);

        if cov_events > 0 {
            warnings.push(format!("{} cell over-voltage events", cov_events));
        }
        if cuv_events > 5 {
            warnings.push(format!(
                "{} cell under-voltage events (deep discharge)",
                cuv_events
            ));
        }
        if ocd1_events > 0 || ocd2_events > 0 {
            warnings.push(format!(
                "{} over-current discharge events",
                ocd1_events + ocd2_events
            ));
        }
        if occ1_events > 0 {
            warnings.push(format!("{} over-current charge events", occ1_events));
        }
        if scd_events > 0 {
            issues.push("Short-circuit events detected");
        }
    }

    // Analyze gauging health
    if data.lifetime5.len() >= 32 {
        let lt5 = &data.lifetime5;
        let valid_terminations = u16::from_le_bytes([lt5[16], lt5[17]]);
        let ra_updates = u16::from_le_bytes([lt5[24], lt5[25]]);
        let ra_fails = u16::from_le_bytes([lt5[28], lt5[29]]);

        // Check charge termination ratio
        if data.cycle_count > 10 && valid_terminations < (data.cycle_count * 8 / 10) {
            warnings.push(format!(
                "Low charge termination rate: {} terminations over {} cycles",
                valid_terminations, data.cycle_count
            ));
        }

        // Check resistance update failures
        if ra_updates > 0 {
            let fail_rate = (ra_fails as f32 / ra_updates as f32) * 100.0;
            if fail_rate > 20.0 {
                warnings.push(format!(
                    "High resistance update fail rate: {:.1}%",
                    fail_rate
                ));
            }
        }
    }

    // Print results
    if issues.is_empty() && warnings.is_empty() {
        println!("Status: HEALTHY");
        println!("  No issues detected. Battery is operating normally.");
    } else if issues.is_empty() {
        println!("Status: GOOD (with notes)");
        for warning in &warnings {
            println!("  Note: {}", warning);
        }
    } else {
        println!("Status: NEEDS ATTENTION");
        for issue in &issues {
            println!("  Issue: {}", issue);
        }
        for warning in &warnings {
            println!("  Note: {}", warning);
        }
    }

    // Print summary stats
    println!("\nSummary:");
    println!("  Cycle count: {}", data.cycle_count);
    if !data.state_of_health.is_empty() {
        let soh_mah = u16::from_le_bytes([data.state_of_health[0], data.state_of_health[1]]);
        let soh_mwh = u16::from_le_bytes([data.state_of_health[2], data.state_of_health[3]]);
        println!(
            "  Remaining capacity: {}mAh ({}.{:02}Wh)",
            soh_mah,
            soh_mwh / 100,
            soh_mwh % 100
        );
    }
    println!("  Current cell balance: {}mV spread", cell_delta);
    if data.lifetime2.len() >= 20 {
        let lt2 = &data.lifetime2;
        let cb_times: Vec<f64> = [
            u32::from_le_bytes([lt2[4], lt2[5], lt2[6], lt2[7]]),
            u32::from_le_bytes([lt2[8], lt2[9], lt2[10], lt2[11]]),
            u32::from_le_bytes([lt2[12], lt2[13], lt2[14], lt2[15]]),
            u32::from_le_bytes([lt2[16], lt2[17], lt2[18], lt2[19]]),
        ]
        .iter()
        .map(|&t| t as f64 / 3600.0)
        .collect();
        println!(
            "  Cell balancing time: {:.1}h / {:.1}h / {:.1}h / {:.1}h",
            cb_times[0], cb_times[1], cb_times[2], cb_times[3]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_bins_path(filename: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test_bins");
        path.push(filename);
        path
    }

    #[test]
    fn test_parse_battery_dump() {
        let path = test_bins_path("battery_00E3.txt");
        if !path.exists() {
            // Skip test if dump file doesn't exist yet
            eprintln!("Skipping test: {:?} not found", path);
            return;
        }

        let data = BatteryData::read_from_file(&path).expect("Failed to read dump file");

        // Basic info
        assert_eq!(data.mode, 0x6001);
        assert_eq!(data.serial_num, 0x00E3);
        assert!(!data.device_name.is_empty());
        assert!(!data.manufacturer_name.is_empty());

        // Verify temperature is reasonable (0-100C range in 0.1K units)
        let temp_c = (data.temperature as f32 / 10.0) - 273.15;
        assert!(
            temp_c > 0.0 && temp_c < 100.0,
            "Temperature {} out of range",
            temp_c
        );

        // Verify voltage is reasonable (10-20V for 4-cell pack)
        let voltage_v = data.voltage as f32 / 1000.0;
        assert!(
            voltage_v > 10.0 && voltage_v < 25.0,
            "Voltage {} out of range",
            voltage_v
        );

        // Verify cell voltages add up approximately to total
        let cell_sum =
            data.cell_voltage1 + data.cell_voltage2 + data.cell_voltage3 + data.cell_voltage4;
        let diff = (cell_sum as i32 - data.voltage as i32).abs();
        assert!(
            diff < 100,
            "Cell voltages don't add up: {} vs {}",
            cell_sum,
            data.voltage
        );
    }

    #[test]
    fn test_parse_lifetime_data() {
        let path = test_bins_path("battery_00E3.txt");
        if !path.exists() {
            eprintln!("Skipping test: {:?} not found", path);
            return;
        }

        let data = BatteryData::read_from_file(&path).expect("Failed to read dump file");

        // Check lifetime1 has expected length
        if !data.lifetime1.is_empty() {
            assert_eq!(data.lifetime1.len(), 32, "LifeTime1 should be 32 bytes");

            // Check cell max voltages are reasonable (3.0-4.5V)
            for i in 0..4 {
                let max_v = u16::from_le_bytes([data.lifetime1[i * 2], data.lifetime1[i * 2 + 1]]);
                assert!(
                    max_v > 3000 && max_v < 4600,
                    "Cell {} max voltage {} out of range",
                    i + 1,
                    max_v
                );
            }

            // Check temperatures are reasonable
            let max_temp = data.lifetime1[26];
            let min_temp = data.lifetime1[27];
            assert!(max_temp < 100, "Max temp {} too high", max_temp);
            assert!(
                min_temp < max_temp || min_temp == 0,
                "Min temp {} > max temp {}",
                min_temp,
                max_temp
            );
        }

        // Check lifetime2 has expected length
        if !data.lifetime2.is_empty() {
            assert_eq!(data.lifetime2.len(), 20, "LifeTime2 should be 20 bytes");
        }
    }

    #[test]
    fn test_hex_encode_decode_roundtrip() {
        let original = vec![0x00, 0x11, 0x22, 0x33, 0xAA, 0xBB, 0xCC, 0xFF];
        let encoded = hex_encode(&original);
        let decoded = hex_decode(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_manufacture_date_decode() {
        // Test date decoding: Year*512 + Month*32 + Day
        // 2024-10-06 = (2024-1980)*512 + 10*32 + 6 = 44*512 + 320 + 6 = 22528 + 326 = 22854
        let mfg_date: u16 = 22854;
        let day = mfg_date & 0x1F;
        let month = (mfg_date >> 5) & 0x0F;
        let year = (mfg_date >> 9) + 1980;
        assert_eq!(day, 6);
        assert_eq!(month, 10);
        assert_eq!(year, 2024);
    }

    #[test]
    fn test_temperature_conversion() {
        // 35.5C in 0.1K units = (35.5 + 273.15) * 10 = 3086.5 ≈ 3087
        let temp_raw: u16 = 3087;
        let temp_c = (temp_raw as f32 / 10.0) - 273.15;
        assert!(
            (temp_c - 35.55).abs() < 0.1,
            "Temperature conversion failed: {}",
            temp_c
        );
    }
}
