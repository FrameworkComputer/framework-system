//! Get information about system power (battery, AC, PD ports)

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
use core::fmt;
use core::prelude::v1::derive;
use log::Level;

use crate::ccgx::{AppVersion, Application, BaseVersion, ControllerVersion, MainPdVersions};
use crate::chromium_ec::command::EcRequestRaw;
use crate::chromium_ec::commands::{EcRequestReadPdVersion, EcRequestUsbPdPowerInfo};
use crate::chromium_ec::{print_err_ref, CrosEc, CrosEcDriver, EcResult};
use crate::smbios;
use crate::smbios::get_platform;
use crate::util::Platform;

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
const _EC_MEMMAP_ACC_STATUS: u16 = 0x90; // Accelerometer status (8 bits )
                                         // Unused 0x91
const _EC_MEMMAP_ACC_DATA: u16 = 0x92; // Accelerometers data 0x92 - 0x9f
                                       // 0x92: u16Lid Angle if available, LID_ANGLE_UNRELIABLE otherwise
                                       // 0x94 - 0x99: u161st Accelerometer
                                       // 0x9a - 0x9f: u162nd Accelerometer
const _EC_MEMMAP_GYRO_DATA: u16 = 0xa0; // Gyroscope data 0xa0 - 0xa5
                                        // Unused 0xa6 - 0xdf

// Battery bit flags at EC_MEMMAP_BATT_FLAG.
const EC_BATT_FLAG_AC_PRESENT: u8 = 0x01;
const EC_BATT_FLAG_BATT_PRESENT: u8 = 0x02;
const EC_BATT_FLAG_DISCHARGING: u8 = 0x04;
const EC_BATT_FLAG_CHARGING: u8 = 0x08;
const EC_BATT_FLAG_LEVEL_CRITICAL: u8 = 0x10;

#[derive(Debug)]
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
pub fn get_als_reading(ec: &CrosEc) -> Option<u32> {
    let als = ec.read_memory(EC_MEMMAP_ALS, 0x04)?;
    Some(u32::from_le_bytes([als[0], als[1], als[2], als[3]]))
}

pub fn print_sensors(ec: &CrosEc) {
    let als_int = get_als_reading(ec).unwrap();
    println!("ALS: {:>4} Lux", als_int);
}

pub fn print_expansion_bay_info(ec: &CrosEc) {
    let platform = smbios::get_platform();
    if !matches!(
        platform,
        Some(Platform::Framework13Amd) | Some(Platform::Framework16)
    ) {
        println!("Only applicable to Framework 16 and Framework AMD systems");
        return;
    }

    println!("AMD");
    // TODO: This is also on Azalea?
    let power_slider = ec.read_memory(0x151, 0x02).unwrap()[0];
    let dc_ac = if power_slider <= 0b1000 { "DC" } else { "AC" };
    let mode = match power_slider {
        0b0000_0001 | 0b0001_0000 => "Best Performance",
        0b0000_0010 | 0b0010_0000 => "Balanced",
        0b0000_0100 | 0b0100_0000 => "Best Power Efficiency",
        0b0000_1000 => "Battery Saver",
        _ => "Unknown Mode",
    };
    println!(
        "  Power Slider:     {}, {} ({:#09b})",
        dc_ac, mode, power_slider
    );

    // TODO: This is also on Azalea?
    let stt_table = ec.read_memory(0x154, 0x01).unwrap()[0];
    println!("  STT Table:        {:?}", stt_table);

    // TODO: What's this? Always [0x00, 0x00] so far
    // TODO: This is also on Azalea?
    // Core Performance Boost
    let cbp = ec.read_memory(0x155, 0x02).unwrap();
    println!("  CBP:              {} ({:?})", cbp == [0x00, 0x00], cbp);

    // TODO: When is this changed?
    // TODO: This is also on Azalea?
    let dtt_temp = ec.read_memory(0x160, 0x0F).unwrap();
    println!("  DTT Temp:         {:?}", dtt_temp);

    if !matches!(platform, Some(Platform::Framework16)) {
        return;
    }

    println!("Expansion Bay");

    // TODO: This is the serial struct in the Expansion Bay?
    let serial_struct = ec.read_memory(0x140, 0x04).unwrap();
    println!("  Serial Struct:    {:?}", serial_struct);

    // TODO: Why is this in the same namespace?
    // let batt_manuf_day = ec.read_memory(0x144, 0x01).unwrap()[0];
    // let batt_manuf_month = ec.read_memory(0x145, 0x01).unwrap()[0];
    // let batt_manuf_year = ec.read_memory(0x146, 0x02).unwrap();
    // let batt_manuf_year = u16::from_le_bytes([batt_manuf_year[0], batt_manuf_year[1]]);
    // println!("  Batt Manuf        {:?}-{:?}-{:?}", batt_manuf_year, batt_manuf_month, batt_manuf_day);

    // TODO: This is the PD in the dGPU module?
    let pd_ver = ec.read_memory(0x14C, 0x04).unwrap();
    println!("  PD Version:       {:?}", pd_ver);

    let gpu_ctrl = ec.read_memory(0x150, 0x01).unwrap()[0];
    // Unused, this is for the BIOS to set
    let _set_mux_status = match gpu_ctrl & 0b11 {
        0b00 => "EC Received and Clear",
        0b01 => "BIOS Set APU",
        0b10 => "BIOS Set GPU",
        _ => "Unknown",
    };
    let mux_status = if (gpu_ctrl & 0b100) > 0 { "APU" } else { "GPU" };
    let board_status = if (gpu_ctrl & 0b1000) > 0 {
        "Present"
    } else {
        "Absent"
    };
    // Unused, set by BIOS: (gpu_ctrl & 0b10000)
    let pcie_config = match gpu_ctrl & 0b01100000 {
        0b00 => "8x1",
        0b01 => "4x1",
        0b10 => "4x2",
        0b11 => "Disabled",
        _ => "Unknown",
    };
    println!("  GPU CTRL:         {:#x}", gpu_ctrl);
    println!("    MUX Status:     {}", mux_status);
    println!("    Board Status:   {}", board_status);
    println!("    PCIe Config:    {}", pcie_config);

    // TODO: This seems like it's not correctly working? It's always false
    let display_on = ec.read_memory(0x153, 0x01).unwrap()[0];
    println!("  Display On:       {:?}", display_on == 0x01);

    let gpu_type = ec.read_memory(0x157, 0x01).unwrap()[0];
    let gpu_name = match gpu_type {
        0x00 => "Initializing",
        0x01 => "Fan Only",
        0x02 => "AMD R23M",
        0x03 => "SSD",
        0x04 => "PCIe Accessory",
        _ => "Unknown",
    };
    println!("  GPU Type:         {} ({:?})", gpu_name, gpu_type);
}

pub fn print_thermal(ec: &CrosEc) {
    let temps = ec.read_memory(EC_MEMMAP_TEMP_SENSOR, 0x0F).unwrap();
    println!("Temps: {:?}", temps);
    let fans = ec.read_memory(EC_MEMMAP_FAN, 0x08).unwrap();

    let platform = smbios::get_platform();
    match platform {
        Some(Platform::IntelGen11) | Some(Platform::IntelGen12) | Some(Platform::IntelGen13) => {
            println!("  F75303_Local: {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_CPU:   {:>4}", TempSensor::from(temps[1]));
            println!("  F75303_DDR:   {:>4}", TempSensor::from(temps[2]));
            println!("  Battery:      {:>4}", TempSensor::from(temps[3]));
            println!("  PECI:         {:>4}", TempSensor::from(temps[4]));
            println!("  F57397_VCCGT: {:>4}", TempSensor::from(temps[5]));
        }
        Some(Platform::Framework13Amd | Platform::Framework16) => {
            println!("  F75303_Local: {:>4}", TempSensor::from(temps[0]));
            println!("  F75303_CPU:   {:>4}", TempSensor::from(temps[1]));
            println!("  F75303_DDR:   {:>4}", TempSensor::from(temps[2]));
            println!("  APU:          {:>4}", TempSensor::from(temps[3]));
            // TODO: Only display if dGPU is present
            // TODO: Sometimes  these show 0 even if the GPU is present. Why?
            if matches!(platform, Some(Platform::Framework16)) {
                println!("  dGPU VR:      {:>4}", TempSensor::from(temps[4]));
                println!("  dGPU VRAM:    {:>4}", TempSensor::from(temps[5]));
                println!("  dGPU AMB:     {:>4}", TempSensor::from(temps[6]));
                println!("  dGPU temp:    {:>4}", TempSensor::from(temps[7]));
            }
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
        }
    }

    let fan0 = u16::from_le_bytes([fans[0], fans[1]]);
    let fan1 = u16::from_le_bytes([fans[2], fans[3]]);
    if matches!(platform, Some(Platform::Framework16)) {
        println!("  Fan L Speed:  {:>4} RPM", fan0);
        println!("  Fan R Speed:  {:>4} RPM", fan1);
    } else {
        println!("  Fan Speed:    {:>4} RPM", fan0);
    }
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
        print_battery_information(&power_info);
        if let Some(_battery) = &power_info.battery {
            return 0;
        }
    }
    1
}

fn print_battery_information(power_info: &PowerInfo) {
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
    let fl16 = Some(crate::util::Platform::Framework16) == get_platform();
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

// NOTE: Only works on ADL at the moment!
// TODO: Not on TGL, need to check if RPL and later have it.
pub fn read_pd_version(ec: &CrosEc) -> EcResult<MainPdVersions> {
    let info = EcRequestReadPdVersion {}.send_command(ec)?;

    Ok(MainPdVersions {
        controller01: parse_pd_ver(&info.controller01),
        controller23: parse_pd_ver(&info.controller23),
    })
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
