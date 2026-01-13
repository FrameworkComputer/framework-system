use crate::util::Platform;
use serde::Deserialize;
use std::collections::HashMap;
use wmi::*;

/// Driver configuration loaded from TOML
#[derive(Debug, Deserialize)]
struct DriversConfig {
    pnp_drivers: HashMap<String, String>,
    products: HashMap<String, String>,
    system_drivers: HashMap<String, String>,
}

/// Platform-specific baseline configuration
#[derive(Debug, Deserialize, Default)]
struct BaselineConfig {
    versions: HashMap<String, String>,
}

/// Load driver configuration from embedded TOML
fn load_drivers_config() -> DriversConfig {
    const CONFIG_STR: &str = include_str!("drivers.toml");
    toml::from_str(CONFIG_STR).expect("Failed to parse drivers.toml")
}

/// Load baseline configuration for a specific platform
fn load_baseline_for_platform(platform: &Platform) -> BaselineConfig {
    let config_str = match platform {
        Platform::Framework12IntelGen13 => {
            include_str!("baselines/framework12_intel_gen13.toml")
        }
        Platform::IntelGen11 => include_str!("baselines/intel_gen11.toml"),
        Platform::IntelGen12 => include_str!("baselines/intel_gen12.toml"),
        Platform::IntelGen13 => include_str!("baselines/intel_gen13.toml"),
        Platform::IntelCoreUltra1 => include_str!("baselines/intel_core_ultra1.toml"),
        Platform::Framework13Amd7080 => include_str!("baselines/framework13_amd_7080.toml"),
        Platform::Framework13AmdAi300 => include_str!("baselines/framework13_amd_ai300.toml"),
        Platform::Framework16Amd7080 => include_str!("baselines/framework16_amd_7080.toml"),
        Platform::Framework16AmdAi300 => include_str!("baselines/framework16_amd_ai300.toml"),
        Platform::FrameworkDesktopAmdAiMax300 => {
            include_str!("baselines/framework_desktop_amd_ai_max300.toml")
        }
        Platform::GenericFramework(..) | Platform::UnknownSystem => {
            return BaselineConfig::default();
        }
    };
    toml::from_str(config_str).unwrap_or_default()
}

/// Collected driver information for baseline updates
#[derive(Debug, Default)]
pub struct DetectedDrivers {
    pub drivers: HashMap<String, String>,
}

impl DetectedDrivers {
    /// Generate TOML baseline content from detected drivers
    pub fn to_toml(&self) -> String {
        let mut output = String::new();
        output.push_str("# Driver baseline - auto-generated\n");
        output.push_str("# Last updated: ");
        output.push_str(&chrono::Local::now().format("%Y-%m-%d").to_string());
        output.push_str("\n\n[versions]\n");

        let mut sorted: Vec<_> = self.drivers.iter().collect();
        sorted.sort_by_key(|(k, _)| k.as_str());

        for (name, version) in sorted {
            output.push_str(&format!("\"{}\" = \"{}\"\n", name, version));
        }
        output
    }
}

// TODO:
// - [ ] Critical
//   - [ ] Figure out why the CSME version is old
//     - Looks like that depends on the CPU/CSME type
//   - [x] Figure out how "Intel Chipset" shows up, I can't find any drivers
//     - It's present in Win32_Product
//   - [ ] Intel WiFi driver in Win32_SystemDriver is old, but in Win32_PnpSignedDriver is correct. Which one is used?
// - [x] Provide nice alias for driver names
// - [ ] Display drivers even when they're missing to make sure we find those that didn't install
// - [ ] Make more efficient by querying only the drivers we care about
// - [ ] Figure out why IGCC versios shows same as graphics driver
// - [ ] Figure out how to read HSA "Realtek Audio Console" version

// Helpful commands
// Win32_SystemDriver
//   get-wmiobject Win32_SystemDriver | select DisplayName,Name,@{n="version";e={(gi $_.pathname).VersionInfo.FileVersion}},PathName
//   Get-WmiObject -query "SELECT * FROM CIM_Datafile WHERE Name = 'C:\Windows\system32\drivers\xinputhid.sys'"
//   Get-WmiObject -query "SELECT * FROM Win32_PnPEntity WHERE ConfigManagerErrorCode <> 0"
//
// Win32_PnpSignedDriver
//   get-wmiobject Win32_PnpSignedDriver |  select Manufacturer,DeviceName,HardwareID,DriverVersion
//   get-wmiobject Win32_PnpSignedDriver |  where manufacturer -like 'Goodix' | select Manufacturer,DeviceName,HardwareID,DriverVersion
//   get-wmiobject Win32_PnpSignedDriver |  where manufacturer -like 'Intel' | select Manufacturer,DeviceName,HardwareID,DriverVersion
//
// Win32_Product
//   Get-WmiObject -Class Win32_Product | select IdentifyingNumber, Name, Version
//
// Get-WmiObject -Class Win32_Product | select IdentifyingNumber, Name, Version | Format-Table | Out-String -width 9999 > product.txt
// get-wmiobject Win32_PnpSignedDriver |  select Manufacturer,DeviceName,HardwareID,DriverVersion | Format-Table | Out-String -width 9999 > pnp.txt
// get-wmiobject Win32_SystemDriver | select DisplayName,Name,@{n="version";e={(gi $_.pathname).VersionInfo.FileVersion}},PathName | Format-Table | Out-String -width 9999 > system.txt

pub fn print_yellow_bangs() {
    println!("Devices with Yellow Bangs");
    debug!("Opening WMI");
    let wmi_con = WMIConnection::new(COMLibrary::new().unwrap()).unwrap();
    debug!("Querying WMI");
    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query("SELECT * FROM Win32_PnPEntity WHERE ConfigManagerErrorCode <> 0")
        .unwrap();

    if results.is_empty() {
        println!("  None");
        return;
    }

    for bang in results.iter() {
        // println!("  {:#?}", results);
        // TODO: Unpack the Variant types
        // TODO: Use serde
        let description = if let Variant::String(s) = &bang["Description"] {
            s.clone()
        } else {
            "".to_string()
        };
        println!("  {}", description);
        println!("    Compatible IDs:        {:?}", &bang["CompatibleID"]);
        println!("    Hardware IDs:          {:?}", &bang["HardwareID"]);
        println!("    DeviceID:              {:?}", &bang["DeviceID"]);
        println!("    PNPDeviceID:           {:?}", &bang["PNPDeviceID"]);
        println!(
            "    ConfigManagerErrorCode {:?}",
            &bang["ConfigManagerErrorCode"]
        );
        println!("    Status                 {:?}", &bang["Status"]);
        // Other values that don't seem to have useful information
        // ConfigManagerUserConfig: Bool (false)
        // ErrorCleared: ? (Null)
        // CreationClassName: String ("Win32_PnPEntity")
        // Present: Bool (true)
        // InstallDate: ? (Null)
        // ErrorDescription: ? (Null)
        // Caption: String ("Multimedia Audio Controller")
        // LastErrorCode: ? (Null)
        // Availability: ? (Null)
        // PowerManagementCapabilities: Array ([])
        // PowerManagementSupported: ? (Null)
        // PNPClass: ? (Null)
        // StatusInfo: ? (Null)
    }
}

/// Print drivers with optional baseline comparison
pub fn print_drivers_with_baseline(platform: Option<&Platform>) {
    print_yellow_bangs();

    let config = load_drivers_config();
    let baseline = platform.map(load_baseline_for_platform).unwrap_or_default();

    println!("Drivers");
    let wmi_con = WMIConnection::new(COMLibrary::new().unwrap()).unwrap();

    // PNP Drivers
    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query(
            "SELECT Manufacturer,DeviceName,HardwareID,DriverVersion FROM Win32_PnpSignedDriver",
        )
        .unwrap();
    for val in results.iter() {
        let device_name = if let Variant::String(s) = &val["DeviceName"] {
            s.clone()
        } else {
            "".to_string()
        };
        let version = if let Variant::String(s) = &val["DriverVersion"] {
            s.clone()
        } else {
            "".to_string()
        };

        // Find matching driver entry
        if let Some(alias) = config.pnp_drivers.get(&device_name) {
            println!("  {}", alias);
            print_version_with_baseline(&version, alias, &baseline);
        }
    }

    // Products
    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query("SELECT IdentifyingNumber, Name, Version FROM Win32_Product")
        .unwrap();
    for val in results.iter() {
        let name = if let Variant::String(s) = &val["Name"] {
            s.clone()
        } else {
            "".to_string()
        };
        let version = if let Variant::String(s) = &val["Version"] {
            s.clone()
        } else {
            "".to_string()
        };

        // Find matching product entry
        if let Some(alias) = config.products.get(&name) {
            println!("  {}", alias);
            print_version_with_baseline(&version, alias, &baseline);
        }
    }

    // System Drivers
    let system_drivers = &config.system_drivers;

    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query("SELECT DisplayName,Name,PathName FROM Win32_SystemDriver")
        .unwrap();
    for val in results.iter() {
        let display_name = if let Variant::String(s) = &val["DisplayName"] {
            s.clone()
        } else {
            "".to_string()
        };
        let name = if let Variant::String(s) = &val["Name"] {
            s.clone()
        } else {
            "".to_string()
        };
        let path_name = if let Variant::String(s) = &val["PathName"] {
            s.clone()
        } else {
            "".to_string()
        };

        if !path_name.starts_with("C:") {
            debug!("Skipping path_name: {:?}", path_name);
            continue;
        }
        let query = format!(
            "SELECT Version FROM CIM_Datafile WHERE Name = '{}'",
            path_name.replace("\\", "\\\\")
        );
        let results: Vec<HashMap<String, Variant>> = wmi_con.raw_query(query).unwrap();
        let version = if let Variant::String(s) = &results[0]["Version"] {
            s.clone()
        } else {
            "".to_string()
        };

        if let Some(alias) = system_drivers.get(&name) {
            println!("  {}", alias);
            debug!("    Display: {}", display_name);
            debug!("    Name:    {}", name);
            debug!("    Path:    {}", path_name);
            print_version_with_baseline(&version, alias, &baseline);
        }
    }
}

/// Print version with baseline comparison
fn print_version_with_baseline(version: &str, alias: &str, baseline: &BaselineConfig) {
    if let Some(expected) = baseline.versions.get(alias) {
        if expected != "0.0.0.0" && version != expected {
            println!("    Version: {} (expected: {})", version, expected);
        } else {
            println!("    Version: {}", version);
        }
    } else {
        println!("    Version: {}", version);
    }
}

/// Print drivers without baseline comparison (backwards compatible)
pub fn print_drivers() {
    print_drivers_with_baseline(None);
}

/// Collect all detected drivers (for generating baselines)
pub fn collect_drivers() -> DetectedDrivers {
    let config = load_drivers_config();
    let mut detected = DetectedDrivers::default();
    let wmi_con = WMIConnection::new(COMLibrary::new().unwrap()).unwrap();

    // PNP Drivers
    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query(
            "SELECT Manufacturer,DeviceName,HardwareID,DriverVersion FROM Win32_PnpSignedDriver",
        )
        .unwrap();
    for val in results.iter() {
        let device_name = if let Variant::String(s) = &val["DeviceName"] {
            s.clone()
        } else {
            continue;
        };
        let version = if let Variant::String(s) = &val["DriverVersion"] {
            s.clone()
        } else {
            continue;
        };

        if let Some(alias) = config.pnp_drivers.get(&device_name) {
            detected.drivers.insert(alias.clone(), version);
        }
    }

    // Products
    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query("SELECT IdentifyingNumber, Name, Version FROM Win32_Product")
        .unwrap();
    for val in results.iter() {
        let name = if let Variant::String(s) = &val["Name"] {
            s.clone()
        } else {
            continue;
        };
        let version = if let Variant::String(s) = &val["Version"] {
            s.clone()
        } else {
            continue;
        };

        if let Some(alias) = config.products.get(&name) {
            detected.drivers.insert(alias.clone(), version);
        }
    }

    // System Drivers
    let system_drivers = &config.system_drivers;
    let results: Vec<HashMap<String, Variant>> = wmi_con
        .raw_query("SELECT DisplayName,Name,PathName FROM Win32_SystemDriver")
        .unwrap();
    for val in results.iter() {
        let name = if let Variant::String(s) = &val["Name"] {
            s.clone()
        } else {
            continue;
        };
        let path_name = if let Variant::String(s) = &val["PathName"] {
            s.clone()
        } else {
            continue;
        };

        if !path_name.starts_with("C:") {
            continue;
        }
        let query = format!(
            "SELECT Version FROM CIM_Datafile WHERE Name = '{}'",
            path_name.replace("\\", "\\\\")
        );
        if let Ok(results) = wmi_con.raw_query::<HashMap<String, Variant>>(&query) {
            if !results.is_empty() {
                if let Variant::String(version) = &results[0]["Version"] {
                    if let Some(alias) = system_drivers.get(&name) {
                        detected.drivers.insert(alias.to_string(), version.clone());
                    }
                }
            }
        }
    }

    detected
}
