//! Get information about system power (battery, AC, PD ports)

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::fmt;
use core::prelude::v1::derive;
use log::Level;

use crate::ccgx::{AppVersion, Application, BaseVersion, ControllerVersion, MainPdVersions};
use crate::chromium_ec::command::EcRequestRaw;
use crate::chromium_ec::commands::*;
use crate::chromium_ec::*;
use crate::smbios;
use crate::util::{Platform, PlatformFamily};

/// Maximum length of strings in memmap
const EC_MEMMAP_TEXT_MAX: u16 = 8;

// The offset address of each type of data in mapped memory.
// TODO: Move non-power values to other modules
const EC_MEMMAP_TEMP_SENSOR: u16 = 0x00; // Temp sensors 0x00 - 0x0f
const EC_MEMMAP_FAN: u16 = 0x10; // Fan speeds 0x10 - 0x17
const _EC_MEMMAP_TEMP_SENSOR_B: u16 = 0x18; // More temp sensors 0x18 - 0x1f
const _EC_MEMMAP_ID: u16 = 0x2120; // 0x20 == 'E', 0x21 == 'C'
const EC_MEMMAP_ID_VERSION: u16 = 0x22; // Version of data in 0x20 - 0x2f
const EC_MEMMAP_THERMAL_VERSION: u16 = 0x23; // Version of data in 0x00 - 0x1f
const EC_MEMMAP_BATTERY_VERSION: u16 = 0x24; // Version of data in 0x40 - 0x7f
const EC_MEMMAP_SWITCHES_VERSION: u16 = 0x25; // Version of data in 0x30 - 0x33
const EC_MEMMAP_EVENTS_VERSION: u16 = 0x26; // Version of data in 0x34 - 0x3f
const _EC_MEMMAP_HOST_CMD_FLAGS: u16 = 0x27; // Host cmd interface flags (8 bits)
                                             // Unused 0x28 - 0x2f
const _EC_MEMMAP_SWITCHES: u16 = 0x30; // 8 bits
                                       // Unused 0x31 - 0x33
const _EC_MEMMAP_HOST_EVENTS: u16 = 0x34; // 64 bits
                                          // Battery values are all 32 bits, unless otherwise noted.
const EC_MEMMAP_BATT_VOLT: u16 = 0x40; // Battery Present Voltage
const EC_MEMMAP_BATT_RATE: u16 = 0x44; // Battery Present Rate
const EC_MEMMAP_BATT_CAP: u16 = 0x48; // Battery Remaining Capacity
const EC_MEMMAP_BATT_FLAG: u16 = 0x4c; // Battery State, see below (8-bit)
const EC_MEMMAP_BATT_COUNT: u16 = 0x4d; // Battery Count (8-bit)
const EC_MEMMAP_BATT_INDEX: u16 = 0x4e; // Current Battery Data Index (8-bit)
                                        // Unused 0x4f
const EC_MEMMAP_BATT_DCAP: u16 = 0x50; // Battery Design Capacity
const EC_MEMMAP_BATT_DVLT: u16 = 0x54; // Battery Design Voltage
const EC_MEMMAP_BATT_LFCC: u16 = 0x58; // Battery Last Full Charge Capacity
const EC_MEMMAP_BATT_CCNT: u16 = 0x5c; // Battery Cycle Count
                                       // Strings are all 8 bytes (EC_MEMMAP_TEXT_MAX)
const EC_MEMMAP_BATT_MFGR: u16 = 0x60; // Battery Manufacturer String
const EC_MEMMAP_BATT_MODEL: u16 = 0x68; // Battery Model Number String
const EC_MEMMAP_BATT_SERIAL: u16 = 0x70; // Battery Serial Number String
const EC_MEMMAP_BATT_TYPE: u16 = 0x78; // Battery Type String
const EC_MEMMAP_ALS: u16 = 0x80; // ALS readings in lux (2 X 16 bits)
                                 // Unused 0x84 - 0x8f
const EC_MEMMAP_ACC_STATUS: u16 = 0x90; // Accelerometer status (8 bits )
                                        // Unused 0x91
const EC_MEMMAP_ACC_DATA: u16 = 0x92; // Accelerometers data 0x92 - 0x9f
                                      // 0x92: u16Lid Angle if available, LID_ANGLE_UNRELIABLE otherwise
                                      // 0x94 - 0x99: u161st Accelerometer
                                      // 0x9a - 0x9f: u162nd Accelerometer
const LID_ANGLE_UNRELIABLE: u16 = 500;
const _EC_MEMMAP_GYRO_DATA: u16 = 0xa0; // Gyroscope data 0xa0 - 0xa5
                                        // Unused 0xa6 - 0xdf

// Battery bit flags at EC_MEMMAP_BATT_FLAG.
const EC_BATT_FLAG_AC_PRESENT: u8 = 0x01;
const EC_BATT_FLAG_BATT_PRESENT: u8 = 0x02;
const EC_BATT_FLAG_DISCHARGING: u8 = 0x04;
const EC_BATT_FLAG_CHARGING: u8 = 0x08;
const EC_BATT_FLAG_LEVEL_CRITICAL: u8 = 0x10;

const EC_FAN_SPEED_ENTRIES: usize = 4;
/// Used on old EC firmware (before 2023)
const EC_FAN_SPEED_STALLED_DEPRECATED: u16 = 0xFFFE;
const EC_FAN_SPEED_NOT_PRESENT: u16 = 0xFFFF;

#[derive(Debug, PartialEq)]
enum TempSensor {
    Ok(u8),
    NotPresent,
    Error,
    NotPowered,
    NotCalibrated,
}
impl From<u8> for TempSensor {
    fn from(t: u8) -> Self {
        match t {
            0xFF => TempSensor::NotPresent,
            0xFE => TempSensor::Error,
            0xFD => TempSensor::NotPowered,
            0xFC => TempSensor::NotCalibrated,
            _ => TempSensor::Ok(t - 73),
        }
    }
}
impl fmt::Display for TempSensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let TempSensor::Ok(t) = self {
            write!(f, "{} C", t)
        } else {
            write!(f, "{:?}", self)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatteryInformation {
    pub present_voltage: u32,
    pub present_rate: u32,
    pub remaining_capacity: u32,
    pub battery_count: u8,
    pub current_battery_index: u8,
    pub design_capacity: u32,
    pub design_voltage: u32,
    /// LFCC in mAH
    pub last_full_charge_capacity: u32,
    pub cycle_count: u32,
    pub charge_percentage: u32, // Calculated based on Remaining Capacity / LFCC
    pub manufacturer: String,
    pub model_number: String,
    pub serial_number: String,
    pub battery_type: String,
    // TODO: Can both charging and discharging be true/falses at the same time?
    // Otherwise we can reduce them to a single flag
    pub discharging: bool,
    pub charging: bool,
    pub level_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PowerInfo {
    pub ac_present: bool,
    pub battery: Option<BatteryInformation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedBatteryInformation {
    pub cycle_count: u32,
    pub charge_percentage: u32, // Calculated based on Remaining Capacity / LFCC
    pub charging: bool,
}

/// Reduced version of PowerInfo
///
/// Usually you won't need all of the fields.
/// Some of them (e.g. present_voltage) will vary with a high frequency, so it's not good to
/// compare two PowerInfo structs to see whether the battery status has changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedPowerInfo {
    pub ac_present: bool,
    pub battery: Option<ReducedBatteryInformation>,
}
impl From<PowerInfo> for ReducedPowerInfo {
    fn from(val: PowerInfo) -> Self {
        ReducedPowerInfo {
            ac_present: val.ac_present,
            battery: if let Some(b) = val.battery {
                Some(ReducedBatteryInformation {
                    cycle_count: b.cycle_count,
                    charge_percentage: b.charge_percentage,
                    charging: b.charging,
                })
            } else {
                None
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccelData {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}
impl From<Vec<u8>> for AccelData {
    fn from(t: Vec<u8>) -> Self {
        Self {
            x: i16::from_le_bytes([t[0], t[1]]),
            y: i16::from_le_bytes([t[2], t[3]]),
            z: i16::from_le_bytes([t[4], t[5]]),
        }
    }
}
impl fmt::Display for AccelData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let quarter: f32 = 0xFFFF as f32 / 4.0;
        let x = (self.x as f32) / quarter;
        let y = (self.y as f32) / quarter;
        let z = (self.z as f32) / quarter;
        write!(f, "X={:+.2}G Y={:+.2}G, Z={:+.2}G", x, y, z)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LidAngle {
    Angle(u16),
    Unreliable,
}
impl From<u16> for LidAngle {
    fn from(a: u16) -> Self {
        match a {
            LID_ANGLE_UNRELIABLE => Self::Unreliable,
            _ => Self::Angle(a),
        }
    }
}
impl fmt::Display for LidAngle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Angle(deg) => write!(f, "{}", deg),
            Self::Unreliable => write!(f, "Unreliable"),
        }
    }
}

fn read_string(ec: &CrosEc, address: u16) -> String {
    let bytes = ec.read_memory(address, EC_MEMMAP_TEXT_MAX).unwrap();
    String::from_utf8_lossy(bytes.as_slice()).replace(['\0'], "")
}

fn read_u32(ec: &CrosEc, address: u16) -> u32 {
    let bytes = ec.read_memory(address, 4).unwrap();
    if bytes.len() != 4 {
        debug_assert!(
            bytes.len() == 4,
            "Tried to read 4 bytes but got {}",
            bytes.len()
        );
        error!("Unexpected length returned: {:?} instead of 4", bytes.len());
        return 0;
    }
    u32::from_ne_bytes(bytes[0..4].try_into().unwrap())
}

pub fn print_memmap_version_info(ec: &CrosEc) {
    // TODO: I don't think these are very useful
    let _id_ver = ec.read_memory(EC_MEMMAP_ID_VERSION, 2).unwrap(); /* Version of data in 0x20 - 0x2f */
    let _thermal_ver = ec.read_memory(EC_MEMMAP_THERMAL_VERSION, 2).unwrap(); /* Version of data in 0x00 - 0x1f */
    let _battery_ver = ec.read_memory(EC_MEMMAP_BATTERY_VERSION, 2).unwrap(); /* Version of data in 0x40 - 0x7f */
    let _switches_ver = ec.read_memory(EC_MEMMAP_SWITCHES_VERSION, 2).unwrap(); /* Version of data in 0x30 - 0x33 */
    let _events_ver = ec.read_memory(EC_MEMMAP_EVENTS_VERSION, 2).unwrap();
}

/// Not supported on TGL EC
pub fn get_als_reading(ec: &CrosEc, index: usize) -> Option<u32> {
    let als = ec.read_memory(EC_MEMMAP_ALS, 0x04)?;
    let offset = index + 4 * index;
    Some(u32::from_le_bytes([
        als[offset],
        als[1 + offset],
        als[2 + offset],
        als[3 + offset],
    ]))
}

pub fn get_accel_data(ec: &CrosEc) -> (AccelData, AccelData, LidAngle) {
    // bit 4 = busy
    // bit 7 = present
    // #define EC_MEMMAP_ACC_STATUS_SAMPLE_ID_MASK 0x0f
    let _acc_status = ec.read_memory(EC_MEMMAP_ACC_STATUS, 0x01).unwrap()[0];
    // While busy, keep reading

    let lid_angle = ec.read_memory(EC_MEMMAP_ACC_DATA, 0x02).unwrap();
    let lid_angle = u16::from_le_bytes([lid_angle[0], lid_angle[1]]);
    let accel_1 = ec.read_memory(EC_MEMMAP_ACC_DATA + 2, 0x06).unwrap();
    let accel_2 = ec.read_memory(EC_MEMMAP_ACC_DATA + 8, 0x06).unwrap();

    // TODO: Make sure we got a new sample
    // println!("  Status Bit: {} 0x{:X}", acc_status, acc_status);
    // println!("  Present:    {}", (acc_status & 0x80) > 0);
    // println!("  Busy:       {}", (acc_status & 0x8) > 0);
    (
        AccelData::from(accel_1),
        AccelData::from(accel_2),
        LidAngle::from(lid_angle),
    )
}

pub fn print_sensors(ec: &CrosEc) {
    let mut has_als = false;
    let mut accel_locations = vec![];

    match ec.motionsense_sensor_info() {
        Ok(sensors) => {
            info!("Sensors: {}", sensors.len());
            for sensor in sensors {
                info!("  Type: {:?}", sensor.sensor_type);
                info!("  Location: {:?}", sensor.location);
                info!("  Chip:     {:?}", sensor.chip);
                if sensor.sensor_type == MotionSenseType::Light {
                    has_als = true;
                }
                if sensor.sensor_type == MotionSenseType::Accel {
                    accel_locations.push(sensor.location);
                }
            }
        }
        Err(EcError::Response(EcResponseStatus::InvalidCommand)) => {
            debug!("Motionsense commands not supported")
        }
        err => _ = print_err(err),
    }

    // If we can't detect it based on motionsense, check the system family
    // If family is unknown, assume it has
    let als_family = matches!(
        smbios::get_family(),
        Some(PlatformFamily::Framework13) | Some(PlatformFamily::Framework16) | None
    );

    if has_als || als_family {
        let als_int = get_als_reading(ec, 0).unwrap();
        println!("ALS: {:>4} Lux", als_int);
    }

    // bit 4 = busy
    // bit 7 = present
    // #define EC_MEMMAP_ACC_STATUS_SAMPLE_ID_MASK 0x0f
    let acc_status = ec.read_memory(EC_MEMMAP_ACC_STATUS, 0x01).unwrap()[0];
    // While busy, keep reading

    let lid_angle = ec.read_memory(EC_MEMMAP_ACC_DATA, 0x02).unwrap();
    let lid_angle = u16::from_le_bytes([lid_angle[0], lid_angle[1]]);
    let accel_1 = ec.read_memory(EC_MEMMAP_ACC_DATA + 2, 0x06).unwrap();
    let accel_2 = ec.read_memory(EC_MEMMAP_ACC_DATA + 8, 0x06).unwrap();

    let present = (acc_status & 0x80) > 0;
    if present {
        println!("Accelerometers:");
        debug!("  Status Bit: {} 0x{:X}", acc_status, acc_status);
        debug!("  Present:    {}", present);
        debug!("  Busy:       {}", (acc_status & 0x8) > 0);
        print!("  Lid Angle:   ");
        if lid_angle == LID_ANGLE_UNRELIABLE {
            println!("Unreliable");
        } else {
            println!("{} Deg", lid_angle);
        }
        println!(
            "  {:<12} {}",
            format!("{:?} Sensor:", accel_locations[0]),
            AccelData::from(accel_1)
        );
        println!(
            "  {:<12} {}",
            format!("{:?} Sensor:", accel_locations[1]),
            AccelData::from(accel_2)
        );
    }
}

pub fn print_thermal(ec: &CrosEc) {
    let temps = ec.read_memory(EC_MEMMAP_TEMP_SENSOR, 0x0F).unwrap();
    let fans = ec.read_memory(EC_MEMMAP_FAN, 0x08).unwrap();

    let platform = smbios::get_platform();
    let family = smbios::get_family();
    let remaining_sensors = match platform {
        Some(Platform::IntelGen11) | Some(Platform::IntelGen12) | Some(Platform::IntelGen13) => {
            println!("  F75303_Local: {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_CPU:   {:>4}", TempSensor::from(temps[1]));
            println!("  F75303_DDR:   {:>4}", TempSensor::from(temps[2]));
            println!("  Battery:      {:>4}", TempSensor::from(temps[3]));
            println!("  PECI:         {:>4}", TempSensor::from(temps[4]));
            if matches!(
                platform,
                Some(Platform::IntelGen12) | Some(Platform::IntelGen13)
            ) {
                println!("  F57397_VCCGT: {:>4}", TempSensor::from(temps[5]));
            }
            2
        }

        Some(Platform::IntelCoreUltra1) => {
            println!("  F75303_Local: {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_CPU:   {:>4}", TempSensor::from(temps[1]));
            println!("  Battery:      {:>4}", TempSensor::from(temps[2]));
            println!("  F75303_DDR:   {:>4}", TempSensor::from(temps[3]));
            println!("  PECI:         {:>4}", TempSensor::from(temps[4]));
            3
        }

        Some(Platform::Framework12IntelGen13) => {
            println!("  F75303_CPU:   {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_Skin:  {:>4}", TempSensor::from(temps[1]));
            println!("  F75303_Local: {:>4}", TempSensor::from(temps[2]));
            println!("  Battery:      {:>4}", TempSensor::from(temps[3]));
            println!("  PECI:         {:>4}", TempSensor::from(temps[4]));
            println!("  Charger IC    {:>4}", TempSensor::from(temps[5]));
            2
        }

        Some(
            Platform::Framework13Amd7080
            | Platform::Framework13AmdAi300
            | Platform::Framework16Amd7080
            | Platform::Framework16AmdAi300,
        ) => {
            println!("  F75303_Local: {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_CPU:   {:>4}", TempSensor::from(temps[1]));
            println!("  F75303_DDR:   {:>4}", TempSensor::from(temps[2]));
            println!("  APU:          {:>4}", TempSensor::from(temps[3]));
            if family == Some(PlatformFamily::Framework16) {
                println!("  dGPU VR:      {:>4}", TempSensor::from(temps[4]));
                println!("  dGPU VRAM:    {:>4}", TempSensor::from(temps[5]));
                println!("  dGPU AMB:     {:>4}", TempSensor::from(temps[6]));
                println!("  dGPU temp:    {:>4}", TempSensor::from(temps[7]));
                0
            } else {
                4
            }
        }

        Some(Platform::FrameworkDesktopAmdAiMax300) => {
            println!("  F75303_APU:   {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_DDR:   {:>4}", TempSensor::from(temps[1]));
            println!("  F75303_AMB:   {:>4}", TempSensor::from(temps[2]));
            println!("  APU:          {:>4}", TempSensor::from(temps[3]));
            4
        }

        _ => {
            println!("  Temp 0:       {:>4}", TempSensor::from(temps[0]));
            println!("  Temp 1:       {:>4}", TempSensor::from(temps[1]));
            println!("  Temp 2:       {:>4}", TempSensor::from(temps[2]));
            println!("  Temp 3:       {:>4}", TempSensor::from(temps[3]));
            println!("  Temp 4:       {:>4}", TempSensor::from(temps[4]));
            println!("  Temp 5:       {:>4}", TempSensor::from(temps[5]));
            println!("  Temp 6:       {:>4}", TempSensor::from(temps[6]));
            println!("  Temp 7:       {:>4}", TempSensor::from(temps[7]));
            0
        }
    };

    // Just in case EC has more sensors than we know about, print them
    for (i, temp) in temps.iter().enumerate().take(8).skip(8 - remaining_sensors) {
        let temp = TempSensor::from(*temp);
        if temp != TempSensor::NotPresent {
            println!("  Temp {}:       {:>4}", i, temp);
        }
    }

    for i in 0..EC_FAN_SPEED_ENTRIES {
        let fan = u16::from_le_bytes([fans[i * 2], fans[1 + i * 2]]);
        if fan == EC_FAN_SPEED_STALLED_DEPRECATED {
            println!("  Fan Speed:  {:>4} RPM (Stalled)", fan);
        } else if fan == EC_FAN_SPEED_NOT_PRESENT {
            info!("  Fan Speed:    Not present");
        } else {
            println!("  Fan Speed:  {:>4} RPM", fan);
        }
    }
}

pub fn get_fan_num(ec: &CrosEc) -> EcResult<usize> {
    let fans = ec.read_memory(EC_MEMMAP_FAN, 0x08).unwrap();

    let mut count = 0;
    for i in 0..EC_FAN_SPEED_ENTRIES {
        let fan = u16::from_le_bytes([fans[i * 2], fans[1 + i * 2]]);
        if fan == EC_FAN_SPEED_NOT_PRESENT {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

// TODO: Use Result
pub fn power_info(ec: &CrosEc) -> Option<PowerInfo> {
    let battery_flag = ec.read_memory(EC_MEMMAP_BATT_FLAG, 1)?[0];
    debug!("AC/Battery flag: {:#X}", battery_flag);
    let battery_lfcc = read_u32(ec, EC_MEMMAP_BATT_LFCC);
    let battery_cap = read_u32(ec, EC_MEMMAP_BATT_CAP);

    let present_voltage = read_u32(ec, EC_MEMMAP_BATT_VOLT);
    let present_rate = read_u32(ec, EC_MEMMAP_BATT_RATE);
    let _remaining_capacity = read_u32(ec, EC_MEMMAP_BATT_CAP); // TODO: Why didn't I use this?
    let battery_count = ec.read_memory(EC_MEMMAP_BATT_COUNT, 1).unwrap()[0]; // 8 bit
    let current_battery_index = ec.read_memory(EC_MEMMAP_BATT_INDEX, 1).unwrap()[0]; // 8 bit
    let design_capacity = read_u32(ec, EC_MEMMAP_BATT_DCAP);
    let design_voltage = read_u32(ec, EC_MEMMAP_BATT_DVLT);
    let cycle_count = read_u32(ec, EC_MEMMAP_BATT_CCNT);

    let manufacturer = read_string(ec, EC_MEMMAP_BATT_MFGR);
    let model_number = read_string(ec, EC_MEMMAP_BATT_MODEL);
    let serial_number = read_string(ec, EC_MEMMAP_BATT_SERIAL);
    let battery_type = read_string(ec, EC_MEMMAP_BATT_TYPE);

    Some(PowerInfo {
        ac_present: 0 != (battery_flag & EC_BATT_FLAG_AC_PRESENT),
        battery: if 0 != (battery_flag & EC_BATT_FLAG_BATT_PRESENT) {
            Some(BatteryInformation {
                // TODO: Add some more information
                present_voltage,
                present_rate,
                remaining_capacity: battery_cap,
                battery_count,
                current_battery_index,
                design_capacity,
                design_voltage,
                last_full_charge_capacity: battery_lfcc,
                cycle_count,

                charge_percentage: (100 * battery_cap) / battery_lfcc,

                manufacturer,
                model_number,
                serial_number,
                battery_type,
                // TODO: Can both be true/falses at the same time?
                discharging: 0 != (battery_flag & EC_BATT_FLAG_DISCHARGING),
                charging: 0 != (battery_flag & EC_BATT_FLAG_CHARGING),
                level_critical: 0 != (battery_flag & EC_BATT_FLAG_LEVEL_CRITICAL),
            })
        } else {
            None
        },
    })
}

// When no battery is present and we're running on AC
pub fn is_standalone(ec: &CrosEc) -> bool {
    if let Some(info) = power_info(ec) {
        debug_assert!(
            info.battery.is_some() || info.ac_present,
            "If there's no battery, we must be running off AC"
        );
        info.battery.is_none()
    } else {
        true // Safe default
    }
}

pub fn get_and_print_power_info(ec: &CrosEc) -> i32 {
    if let Some(power_info) = power_info(ec) {
        print_err_ref(&ec.get_charge_state(&power_info));
        print_battery_information(&power_info);
        if let Some(_battery) = &power_info.battery {
            return 0;
        }
    }
    1
}

fn print_battery_information(power_info: &PowerInfo) {
    println!("Battery Status");
    print!("  AC is:            ");
    if power_info.ac_present {
        println!("connected");
    } else {
        println!("not connected");
    }

    print!("  Battery is:       ");
    if let Some(battery) = &power_info.battery {
        println!("connected");
        println!(
            "  Battery LFCC:     {:#?} mAh (Last Full Charge Capacity)",
            battery.last_full_charge_capacity
        );
        println!("  Battery Capacity: {} mAh", battery.remaining_capacity);
        let wah = battery.remaining_capacity * battery.present_voltage / 1000;
        println!("                    {}.{:2} Wh", wah / 1000, wah % 1000);
        println!("  Charge level:     {:?}%", battery.charge_percentage);

        if log_enabled!(Level::Info) {
            println!("  Manufacturer:     {}", battery.manufacturer);
            println!("  Model Number:     {}", battery.model_number);
            println!("  Serial Number:    {}", battery.serial_number);
            println!("  Battery Type:     {}", battery.battery_type);

            println!(
                "  Present Voltage:  {}.{} V",
                battery.present_voltage / 1000,
                battery.present_voltage % 1000
            );
            println!("  Present Rate:     {} mA", battery.present_rate);
            // We only have a single battery in all our systems
            // Both values are always 0
            // println!("  Battery Count:    {}", battery.battery_count);
            // println!("  Current Battery#: {}", battery.current_battery_index);

            println!("  Design Capacity:  {} mAh", battery.design_capacity);
            let design_wah = battery.design_capacity * battery.design_voltage / 1000;
            println!(
                "                    {}.{} Wh",
                design_wah / 1000,
                design_wah % 1000
            );
            println!(
                "  Design Voltage:   {}.{} V",
                battery.design_voltage / 1000,
                battery.design_voltage % 1000
            );
            println!("  Cycle Count:      {}", battery.cycle_count);
        }

        if battery.discharging {
            println!("  Battery discharging");
        }
        if battery.charging {
            println!("  Battery charging");
        }
        if battery.level_critical {
            println!("  Battery level CRITICAL!");
        }
    } else {
        println!("not connected");
    }
}

pub fn check_update_ready(power_info: &PowerInfo) -> bool {
    // Checking if battery/AC conditions are enough for FW update
    // Either standalone mode or AC+20% charge
    if power_info.battery.is_none()
        || (power_info.ac_present && power_info.battery.as_ref().unwrap().charge_percentage > 20)
    {
        true
    } else {
        println!("Please plug in AC. If the battery is connected, charge it to at least 20% before proceeding.");
        println!(
            "Current charge is: {}%",
            power_info.battery.as_ref().unwrap().charge_percentage
        );
        false
    }
}

#[derive(Debug, PartialEq)]
pub enum UsbChargingType {
    None = 0,
    PD = 1,
    TypeC = 2,
    Proprietary = 3,
    Bc12Dcp = 4,
    Bc12Cdp = 5,
    Bc12Sdp = 6,
    Other = 7,
    VBus = 8,
    Unknown = 9,
}
#[derive(Debug, PartialEq)]
pub enum UsbPowerRoles {
    Disconnected = 0,
    Source = 1,
    Sink = 2,
    SinkNotCharging = 3,
}

pub struct UsbChargeMeasures {
    pub voltage_max: u16,
    pub voltage_now: u16,
    pub current_max: u16,
    pub current_lim: u16,
}

pub struct UsbPdPowerInfo {
    pub role: UsbPowerRoles,
    pub charging_type: UsbChargingType,
    pub dualrole: bool,
    pub meas: UsbChargeMeasures,
    pub max_power: u32,
}

fn check_ac(ec: &CrosEc, port: u8) -> EcResult<UsbPdPowerInfo> {
    // port=0 or port=1 to check right
    // port=2 or port=3 to check left
    // If dest returns 0x2 that means it's powered

    let info = EcRequestUsbPdPowerInfo { port }.send_command(ec)?;

    Ok(UsbPdPowerInfo {
        role: match info.role {
            0 => UsbPowerRoles::Disconnected,
            1 => UsbPowerRoles::Source,
            2 => UsbPowerRoles::Sink,
            3 => UsbPowerRoles::SinkNotCharging,
            _ => {
                debug_assert!(false, "Unknown Role!!");
                UsbPowerRoles::Disconnected
            }
        },
        charging_type: match info.charging_type {
            0 => UsbChargingType::None,
            1 => UsbChargingType::PD,
            2 => UsbChargingType::TypeC,
            3 => UsbChargingType::Proprietary,
            4 => UsbChargingType::Bc12Dcp,
            5 => UsbChargingType::Bc12Cdp,
            6 => UsbChargingType::Bc12Sdp,
            7 => UsbChargingType::Other,
            8 => UsbChargingType::VBus,
            9 => UsbChargingType::Unknown,
            _ => {
                debug_assert!(false, "Unknown Role!!");
                UsbChargingType::Unknown
            }
        },
        dualrole: info.dualrole != 0,
        meas: UsbChargeMeasures {
            voltage_max: info.meas.voltage_max,
            voltage_now: info.meas.voltage_now,
            current_lim: info.meas.current_lim,
            current_max: info.meas.current_max,
        },
        max_power: info.max_power,
    })
}

pub fn get_pd_info(ec: &CrosEc, ports: u8) -> Vec<EcResult<UsbPdPowerInfo>> {
    // 4 ports on our current laptops
    let mut info = vec![];
    for port in 0..ports {
        info.push(check_ac(ec, port));
    }

    info
}

pub fn get_and_print_pd_info(ec: &CrosEc) {
    let fl16 = Some(PlatformFamily::Framework16) == smbios::get_family();
    let ports = 4; // All our platforms have 4 PD ports so far
    let infos = get_pd_info(ec, ports);
    for (port, info) in infos.iter().enumerate().take(ports.into()) {
        println!(
            "USB-C Port {} ({}):",
            port,
            match port {
                0 => "Right Back",
                1 => "Right Front",
                2 =>
                    if fl16 {
                        "Left Middle"
                    } else {
                        "Left Front"
                    },
                3 =>
                    if fl16 {
                        "Left Middle"
                    } else {
                        "Left Back"
                    },
                _ => "??",
            }
        );
        print_err_ref(info);

        // TODO: I haven't checked the encoding/endianness of these numbers. They're likely incorrectly decoded
        if let Ok(info) = info {
            println!("  Role:          {:?}", info.role);

            println!("  Charging Type: {:?}", info.charging_type);

            let volt_max = { info.meas.voltage_max };
            let volt_now = { info.meas.voltage_now };
            println!(
                "  Voltage Now:   {}.{} V, Max: {}.{} V",
                volt_now / 1000,
                volt_now % 1000,
                volt_max / 1000,
                volt_max % 1000,
            );

            let cur_lim = { info.meas.current_lim };
            let cur_max = { info.meas.current_max };
            println!("  Current Lim:   {} mA, Max: {} mA", cur_lim, cur_max);
            println!(
                "  Dual Role:     {}",
                if info.dualrole { "DRP" } else { "Charger" }
            );
            let max_power_mw = { info.max_power } / 1000;
            println!(
                "  Max Power:     {}.{} W",
                max_power_mw / 1000,
                max_power_mw % 1000
            );
        } else {
            println!("  Role:          Unknown");
            println!("  Charging Type: Unknown");

            println!("  Voltage Max:   Unknown, Now: Unknown");
            println!("  Current Max:   Unknown, Lim: Unknown");
            println!("  Dual Role:     Unknown");
            println!("  Max Power:     Unknown");
        }
    }
}

// TODO: Improve return type to be more obvious
// (right, left)
pub fn is_charging(ec: &CrosEc) -> EcResult<(bool, bool)> {
    let port0 = check_ac(ec, 0)?.role == UsbPowerRoles::Sink;
    let port1 = check_ac(ec, 1)?.role == UsbPowerRoles::Sink;
    let port2 = check_ac(ec, 2)?.role == UsbPowerRoles::Sink;
    let port3 = check_ac(ec, 3)?.role == UsbPowerRoles::Sink;
    Ok((port0 || port1, port2 || port3))
}

fn parse_pd_ver_slice(data: &[u8]) -> ControllerVersion {
    parse_pd_ver(&[
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ])
}
fn parse_pd_ver(data: &[u8; 8]) -> ControllerVersion {
    ControllerVersion {
        base: BaseVersion {
            major: (data[3] >> 4) & 0xF,
            minor: (data[3]) & 0xF,
            patch: data[2],
            build_number: u16::from_le_bytes([data[0], data[1]]),
        },
        app: AppVersion {
            application: Application::Notebook,
            major: (data[7] >> 4) & 0xF,
            minor: (data[7]) & 0xF,
            circuit: data[6],
        },
    }
}

// NOTE: TGL (hx20) does not have this host command
pub fn read_pd_version(ec: &CrosEc) -> EcResult<MainPdVersions> {
    let info = EcRequestReadPdVersionV1 {}.send_command_vec(ec);

    // If v1 not available, fall back
    if let Err(EcError::Response(EcResponseStatus::InvalidVersion)) = info {
        let info = EcRequestReadPdVersionV0 {}.send_command(ec)?;

        return Ok(if info.controller23 == [0, 0, 0, 0, 0, 0, 0, 0] {
            MainPdVersions::Single(parse_pd_ver(&info.controller01))
        } else {
            MainPdVersions::RightLeft((
                parse_pd_ver(&info.controller01),
                parse_pd_ver(&info.controller23),
            ))
        });
    }
    // If any other error, exit
    let info = info?;

    let mut versions = vec![];
    let pd_count = info[0] as usize;
    for i in 0..pd_count {
        // TODO: Is there a safer way to check the range?
        if info.len() < 1 + 8 * (i + 1) {
            return Err(EcError::DeviceError("Not enough data returned".to_string()));
        }
        versions.push(parse_pd_ver_slice(&info[1 + 8 * i..1 + 8 * (i + 1)]));
    }

    Ok(MainPdVersions::Many(versions))
}

pub fn standalone_mode(ec: &CrosEc) -> bool {
    // TODO: Figure out how to get that information
    // For now just say we're in standalone mode when the battery is disconnected
    let info = power_info(ec);
    if let Some(i) = info {
        i.battery.is_none()
    } else {
        // Default to true, when we can't find battery status, assume it's not there. Safe default.
        true
    }
}
