use core::convert::TryInto;
use core::prelude::v1::derive;

use crate::chromium_ec;
use crate::util;

// The offset address of each type of data in mapped memory.
// TODO: Move non-power values to other modules
const _EC_MEMMAP_TEMP_SENSOR: u16 = 0x00; // Temp sensors 0x00 - 0x0f
const _EC_MEMMAP_FAN: u16 = 0x10; // Fan speeds 0x10 - 0x17
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
const _EC_MEMMAP_BATT_MFGR: u16 = 0x60; // Battery Manufacturer String
const _EC_MEMMAP_BATT_MODEL: u16 = 0x68; // Battery Model Number String
const _EC_MEMMAP_BATT_SERIAL: u16 = 0x70; // Battery Serial Number String
const _EC_MEMMAP_BATT_TYPE: u16 = 0x78; // Battery Type String
const _EC_MEMMAP_ALS: u16 = 0x80; // ALS readings in lux (2 X 16 bits)
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

pub struct BatteryInformation {
    pub present_voltage: u32,
    pub present_rate: u32,
    pub remaining_capacity: u32,
    pub battery_count: u8,
    pub current_battery_index: u8,
    pub design_capacity: u32,
    pub design_voltage: u32,
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

pub struct PowerInfo {
    pub ac_present: bool,
    pub battery: Option<BatteryInformation>,
}

fn read_u32(address: u16) -> u32 {
    let bytes = chromium_ec::read_memory(address, 4).unwrap();
    if bytes.len() != 4 {
        debug_assert!(
            bytes.len() == 4,
            "Tried to read 4 bytes but got {}",
            bytes.len()
        );
        println!("Unexpected length returned: {:?} instead of 4", bytes.len());
        return 0;
    }
    u32::from_ne_bytes(bytes[0..4].try_into().unwrap())
}

pub fn print_memmap_version_info() {
    // TODO: I don't think these are very useful
    let _id_ver = chromium_ec::read_memory(EC_MEMMAP_ID_VERSION, 2).unwrap(); /* Version of data in 0x20 - 0x2f */
    let _thermal_ver = chromium_ec::read_memory(EC_MEMMAP_THERMAL_VERSION, 2).unwrap(); /* Version of data in 0x00 - 0x1f */
    let _battery_ver = chromium_ec::read_memory(EC_MEMMAP_BATTERY_VERSION, 2).unwrap(); /* Version of data in 0x40 - 0x7f */
    let _switches_ver = chromium_ec::read_memory(EC_MEMMAP_SWITCHES_VERSION, 2).unwrap(); /* Version of data in 0x30 - 0x33 */
    let _events_ver = chromium_ec::read_memory(EC_MEMMAP_EVENTS_VERSION, 2).unwrap();
}

// TODO: Use Result
pub fn power_info() -> Option<PowerInfo> {
    let battery_flag = chromium_ec::read_memory(EC_MEMMAP_BATT_FLAG, 1).unwrap()[0];
    if util::is_debug() {
        println!("AC/Battery flag: {:#X}", battery_flag);
    }
    let battery_lfcc = read_u32(EC_MEMMAP_BATT_LFCC);
    let battery_cap = read_u32(EC_MEMMAP_BATT_CAP);

    let present_voltage = read_u32(EC_MEMMAP_BATT_VOLT);
    let present_rate = read_u32(EC_MEMMAP_BATT_RATE);
    let _remaining_capacity = read_u32(EC_MEMMAP_BATT_CAP); // TODO: Why didn't I use this?
    let battery_count = chromium_ec::read_memory(EC_MEMMAP_BATT_COUNT, 1).unwrap()[0]; // 8 bit
    let current_battery_index = chromium_ec::read_memory(EC_MEMMAP_BATT_INDEX, 1).unwrap()[0]; // 8 bit
    let design_capacity = read_u32(EC_MEMMAP_BATT_DCAP);
    let design_voltage = read_u32(EC_MEMMAP_BATT_DVLT);
    let cycle_count = read_u32(EC_MEMMAP_BATT_CCNT);

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

                charge_percentage: (100 * battery_cap as u32) / battery_lfcc,

                // Strings are all 8 bytes (EC_MEMMAP_TEXT_MAX)
                manufacturer: "TODO".to_string(),
                model_number: "TODO".to_string(),
                serial_number: "TODO".to_string(),
                battery_type: "TODO".to_string(),
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
pub fn is_standalone() -> bool {
    if let Some(info) = power_info() {
        debug_assert!(
            info.battery.is_some() || info.ac_present,
            "If there's no battery, we must be running off AC"
        );
        info.battery.is_none()
    } else {
        true // Safe default
    }
}

pub fn get_and_print_power_info() {
    if let Some(power_info) = power_info() {
        print_battery_information(&power_info);

        check_update_ready(&power_info);
    }
}

fn print_battery_information(power_info: &PowerInfo) {
    print!("  AC is ");
    if power_info.ac_present {
        println!("connected.");
    } else {
        println!("not connected");
    }

    print!("  Battery is: ");
    if let Some(battery) = &power_info.battery {
        println!("connected");
        println!(
            "  Battery LFCC:     {:#?} mAh",
            battery.last_full_charge_capacity
        );
        println!("  Battery CAP:      {:#?} mAh", battery.remaining_capacity);
        println!("  Charge level:     {:?}%", battery.charge_percentage);

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

const EC_CMD_USB_PD_POWER_INFO: u16 = 0x103; /* Get information about PD controller power */

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

#[repr(C, packed)]
struct _UsbChargeMeasures {
    voltage_max: u16,
    voltage_now: u16,
    current_max: u16,
    current_lim: u16,
}

pub struct UsbChargeMeasures {
    pub voltage_max: u16,
    pub voltage_now: u16,
    pub current_max: u16,
    pub current_lim: u16,
}

// Private struct just for parsing binary
#[repr(C, packed)]
struct _EcResponseUsbPdPowerInfo {
    role: u8,          // UsbPowerRoles
    charging_type: u8, // UsbChargingType
    dualrole: u8,      // I think this is a boolean?
    reserved1: u8,
    meas: _UsbChargeMeasures,
    max_power: u32,
}

pub struct UsbPdPowerInfo {
    pub role: UsbPowerRoles,
    pub charging_type: UsbChargingType,
    pub dualrole: bool,
    pub meas: UsbChargeMeasures,
    pub max_power: u32,
}

fn check_ac(port: u8) -> Option<UsbPdPowerInfo> {
    // port=0 or port=1 to check right
    // port=2 or port=3 to check left
    // If dest returns 0x2 that means it's powered

    let data = chromium_ec::send_command(EC_CMD_USB_PD_POWER_INFO, 0, &[port])?;
    // TODO: Rust complains that when accessing this struct, we're reading
    // from unaligned pointers. How can I fix this? Maybe create another struct to shadow it,
    // which isn't packed. And copy the data to there.
    let info: _EcResponseUsbPdPowerInfo = unsafe { std::ptr::read(data.as_ptr() as *const _) };

    // TODO: Checksum

    Some(UsbPdPowerInfo {
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

pub fn get_pd_info() -> Vec<Option<UsbPdPowerInfo>> {
    // 4 ports on our current laptops
    let mut info = vec![];
    for port in 0..4 {
        info.push(check_ac(port));
    }

    info
}

pub fn get_and_print_pd_info() {
    let infos = get_pd_info();
    for (port, info) in infos.iter().enumerate().take(4) {
        println!(
            "USB-C Port {} ({}):",
            port,
            match port {
                0 => "Right Back",
                1 => "Right Front",
                2 => "Left Front",
                3 => "Left Back",
                _ => "??",
            }
        );

        if let Some(info) = &info {
            println!("  Role:          {:?}", info.role);
        } else {
            println!("  Role:          Unknown");
        }

        if let Some(info) = &info {
            println!("  Charging Type: {:?}", info.charging_type);
        } else {
            println!("  Charging Type: Unknown");
        }

        // TODO: I haven't checked the encoding/endianness of these numbers. They're likely incorrectly decoded
        if let Some(info) = &info {
            println!("  Voltage Max:   {}, Now: {}", { info.meas.voltage_max }, {
                info.meas.voltage_now
            });
            println!("  Current Max:   {}, Lim: {}", { info.meas.current_max }, {
                info.meas.current_lim
            });
            println!("  Dual Role:     {:?}", { info.dualrole });
            println!("  Max Power:     {:?}", { info.max_power });
        } else {
            println!("  Voltage Max:   Unknown, Now: Unknown");
            println!("  Current Max:   Unknown, Lim: Unknown");
            println!("  Dual Role:     Unknown");
            println!("  Max Power:     Unknown");
        }
    }
}

// TODO: Improve return type to be more obvious
pub fn is_charging() -> Option<(bool, bool)> {
    let port0 = check_ac(0)?.role == UsbPowerRoles::Sink;
    let port1 = check_ac(1)?.role == UsbPowerRoles::Sink;
    let port2 = check_ac(2)?.role == UsbPowerRoles::Sink;
    let port3 = check_ac(3)?.role == UsbPowerRoles::Sink;
    Some((port0 || port1, port2 || port3))
}

const EC_CMD_READ_PD_VERSION: u16 = 0x3E11; /* Get information about PD controller power */
#[repr(C, packed)]
struct _EcResponseReadPdVersion {
    controller01: [u8; 8],
    controller23: [u8; 8],
}

pub struct ControllerVersion {
    pub base1: u8,
    pub base2: u8,
    pub base3: u8,
    pub base4: u32,
    pub app_major: u8,
    pub app_minor: u8,
    pub app_patch: u8,
}

pub struct EcResponseReadPdVersion {
    pub controller01: ControllerVersion,
    pub controller23: ControllerVersion,
}

fn parse_pd_ver(data: &[u8; 8]) -> ControllerVersion {
    ControllerVersion {
        base1: (data[3] >> 4) & 0xF,
        base2: (data[3]) & 0xF,
        base3: data[2],
        base4: (data[0] as u32) + ((data[1] as u32) << 8),
        app_major: (data[7] >> 4) & 0xF,
        app_minor: (data[7]) & 0xF,
        app_patch: data[6],
    }
}

pub fn print_pd_base_ver(ver: &ControllerVersion) -> String {
    format!("{}.{}.{}.{}", ver.base1, ver.base2, ver.base3, ver.base4)
}

// Must be same format as pd_binary::format_pd_app_ver
pub fn format_pd_app_ver(ver: &ControllerVersion) -> String {
    format!("{}.{}.{:0>2x}", ver.app_major, ver.app_minor, ver.app_patch)
}

// NOTE: Only works on ADL!
// TODO: Handle cases when command doesn't exist.
pub fn read_pd_version() -> Option<EcResponseReadPdVersion> {
    // port=0 or port=1 to check right
    // port=2 or port=3 to check left
    // If dest returns 0x2 that means it's powered

    let data = chromium_ec::send_command(EC_CMD_READ_PD_VERSION, 0, &[])?;
    // TODO: Rust complains that when accessing this struct, we're reading
    // from unaligned pointers. How can I fix this? Maybe create another struct to shadow it,
    // which isn't packed. And copy the data to there.
    let info: _EcResponseReadPdVersion = unsafe { std::ptr::read(data.as_ptr() as *const _) };

    Some(EcResponseReadPdVersion {
        controller01: parse_pd_ver(&info.controller01),
        controller23: parse_pd_ver(&info.controller23),
    })
}

pub fn standalone_mode() -> bool {
    // TODO: Figure out how to get that information
    // For now just say we're in standalone mode when the battery is disconnected
    let info = power_info();
    if let Some(i) = info {
        i.battery.is_none()
    } else {
        // Default to true, when we can't find battery status, assume it's not there. Safe default.
        true
    }
}
