use std::collections::HashMap;
use wmi::*;

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

const PNP_DRIVERS: &[&str] = &[
    // Manufacturer              DeviceName                                                 HardWareID                                              DriverVersion
    // ------------              ----------                                                 ----------                                              -------------
    // Goodix                    Framework Fingerprint Reader                               USB\VID_27C6&PID_609C&REV_0100                          3.12804.0.240
    "Framework Fingerprint Reader",
    // TODO: Wrong version
    // Intel Corporation         Intel(R) Graphics Command Center                           SWC\101.5522_VEN8086_IGCC
    // "Intel(R) Graphics Command Center",

    // Intel Corporation         Intel(R) Wi-Fi 6E AX210 160MHz                             PCI\VEN_8086&DEV_2725&SUBSYS_00248086&REV_1A            23.60.0.10
    "Intel(R) Wi-Fi 6E AX210 160MHz",
    // Don't need, already in system_drivers
    // Intel Corporation         Intel(R) Wireless Bluetooth(R)                             USB\VID_8087&PID_0032&REV_0000                          23.60.0.1
    // "Intel(R) Wireless Bluetooth(R)",

    // Intel(R) Platform Monito… Intel(R) Platform Monitoring Technology (PMT) Driver       PCI\VEN_8086&DEV_7D0D&SUBSYS_0009F111&REV_01            3.1.2.2
    "Intel(R) Platform Monitoring Technology (PMT) Driver",
    // Also in system_drivers
    // Not using the one here, because it doesn't show up when the card isn't plugged in
    // Genesys Logic             Framework SD Expansion Card                                USB\VID_32AC&PID_0009&REV_0003                          4.5.10.201

    // MediaTek, Inc.               RZ616 Wi-Fi 6E 160MHz                           PCI\VEN_14C3&DEV_0616&SUBSYS_E61614C3&REV_00            3.3.0.908      
    // Mediatek Inc.                RZ616 Bluetooth(R) Adapter                      USB\VID_0E8D&PID_E616&REV_0100&MI_00                    1.1037.0.395   
    "RZ616 Bluetooth(R) Adapter",

    // MediaTek, Inc.               RZ717 WiFi 7 160MHz                             PCI\VEN_14C3&DEV_0717&SUBSYS_071714C3&REV_00                 5.4.0.1920
    // Mediatek Inc.                RZ717 Bluetooth(R) Adapter                      USB\VID_0E8D&PID_0717&REV_0100&MI_00                         1.1037.0.433
    "RZ717 Bluetooth(R) Adapter",

    // For both of these WMI shows 31.0.24018.2001 instead of 23.40.18.02. But it's actually the same version
    // 31.0.22024.17002 instead of 23.20.24.17
    // Advanced Micro Devices, Inc. AMD Radeon(TM) 780M                             PCI\VEN_1002&DEV_15BF&SUBSYS_0005F111&REV_C1            31.0.24018.2001
    "AMD Radeon(TM) 780M",
    // Advanced Micro Devices, Inc. AMD Radeon(TM) RX 7700S                         PCI\VEN_1002&DEV_7480&SUBSYS_0007F111&REV_C1            31.0.24018.2001
    "AMD Radeon(TM) RX 7700S",

    // Framework                    Framework NE160QDM-NZ6                          MONITOR\BOE0BC9                                         1.0.0.0
    "Framework NE160QDM-NZ6",

    // Advanced Micro Devices, Inc  AMD DRTM Boot Driver                            ACPI\VEN_DRTM&DEV_0001                                       1.0.18.4
    "AMD DRTM Boot Driver",
];

const PRODUCTS: &[&str] = &[
    // TODO: Can I rely on the IdentifyingNumber GUID?
    // IdentifyingNumber                      Name                                                                      Version
    // -----------------                      ----                                                                      -------
    // {BAB97289-552B-49D5-B1E7-95DB4E4D2DEF} Intel(R) Chipset Device Software                                          10.1.19627.84…
    "Intel(R) Chipset Device Software",
    // {00000060-0230-1033-84C8-B8D95FA3C8C3} Intel(R) Wireless Bluetooth(R)                                            23.60.0.1
    // {1C1EBF97-5EC2-4C01-BCFC-037D140796B4} Intel(R) Serial IO                                                        30.100.2405.44

    // {35143df0-ba1c-4148-8744-137275e83211} AMD_Chipset_Drivers                                                       5.06.29.310    
    "AMD_Chipset_Drivers",
];

pub fn print_drivers() {
    print_yellow_bangs();

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

        // Skip those that we don't care about
        if !PNP_DRIVERS.contains(&device_name.as_str()) {
            continue;
        }
        println!("  {}", device_name);
        println!("    Version: {}", version);
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

        // Skip those that we don't care about
        if !PRODUCTS.contains(&name.as_str()) {
            continue;
        }
        println!("  {}", name);
        println!("    Version: {}", version);
    }

    // System Drivers
    //const system_drivers: HashMap<&str, Option<&str>> = HashMap::from([
    let system_drivers = HashMap::from([
        // [ ] 13 Goodix
        // TODO: Can find via Win32_PnpSignedDriver, Manufacturer 'Goodix', DeviceName 'Framework Fingerprint Reader'
        // HardwareID: USB\VID_27C6&PID_609C&REV_0100

        // [ ] 12 Intel Platform Monitoring Technology
        // Can find in PNP

        // [x] 11 Intel PROSet Bluetooth
        ("ibtusb", None), // Intel(R) Wireless Bluetooth(R)
        // [ ] 10 Intel PROSet WiFi
        // TODO: The first two show old version - Is the wrong version used?
        // "Netwtw14",              // Intel® Smart Sound Technology BUS
        // "Netwtw10",             // Intel® Smart Sound Technology BUS
        // "Netwtw16",              // This one has the correct version, but it doesn't show up

        // [x] 09 Realtek Audio
        ("IntcAzAudAddService", None), // Service for Realtek HD Audio (WDM)
        // [x] 08 Intel Smart Sound Technology
        ("IntcAudioBus", None), // Intel® Smart Sound Technology BUS
        //"IntcOED",               // Intel® Smart Sound Technology OED
        //"IntcUSB",               // Intel® Smart Sound Technology for USB Audio

        // [x] 07 Intel Dynamic Tuning Technology
        ("ipf_acpi", Some("Intel Dynamic Tuning Technology")),
        // "ipf_cpu",
        // "ipf_lf",

        // [x] 06 Intel NPU
        ("npu", Some("Intel NPU")),
        // [x] 05 Intel Graphics
        ("igfxn", Some("Intel Graphics")), // igfxn
        // [x] 04 Intel Serial IO
        // Don't need to show GPIO and I2C versions, we don't need GPIO driver anyways
        // "iaLPSS2_GPIO2_MTL",     // Serial IO GPIO Driver v2
        // "iagpio",                // Intel Serial IO GPIO Controller Driver
        // "iai2c",                 // Serial IO I2C Host Controller
        // "iaLPSS2i_GPIO2"
        // "iaLPSS2i_GPIO2_BXT_P",
        // "iaLPSS2i_GPIO2_CNL",
        // "iaLPSS2i_GPIO2_GLK",
        ("iaLPSS2_I2C_MTL", Some("Intel Serial IO")), // Serial IO I2C Driver v2
        // "iaLPSS2i_I2C",
        // "iaLPSS2i_I2C_BXT_P",
        // "iaLPSS2i_I2C_CNL",
        // "iaLPSS2i_I2C_GLK",

        // [ ] 03 Intel Management Engine
        // TODO: Shows old version 2406.5.5.0 instead of 2409.5.63.0
        ("MEIx64", None), // Intel(R) Management Engine Interface
        // [x] 02 Intel GNA Scoring Accelerator
        ("IntelGNA", None), // Intel(R) GNA Scoring Accelerator service
        // [ ] 01 Intel Chipset
        // TODO: How to find?
        // Can't find it anywhere, not in Device Manager, not in Win32_SystemDriver, not in Win32_PnpSignedDriver

        // Framework provided drivers
        // Realtek USB FE/1GbE/2.5GbE NIC Family Windows 10 64-bit Driver
        (
            "rtux64w10",
            Some("Realtek/Framework Ethernet Expansion Card"),
        ),
        // Genesys Logic Storage Driver
        ("GeneStor", Some("Genesys/Framework SD Expansion Card")),
        //"IntcAzAudAddService",   // Service for Realtek HD Audio (WDM)
        //"intelpmax",             // Intel(R) Dynamic Device Peak Power Manager Driver
        //"IntelPMT",              // Intel(R) Platform Monitoring Technology Service

        // Mediatek PCI LE Extensible Wireless LAN Card Driver mtkwlex               3.3.0.0908                             C:\Windows\system32\drivers\mtkwl6ex.sys
        ("mtkwlex", Some("RZ616 WiFi Driver")),
        // Mediatek PCI LE Extensible Wireless LAN Card Driver                         mtkwecx              5.4.0.1920                             C:\Windows\system32\DriverStore\FileRepository\mtkwecx.inf_amd64_b64df836c89617f7\mtkwecx.sys
        ("mtkwecx", Some("RZ717 WiFi Driver")),

        // RZ616 and RZ717
        // MTK BT Filter Driver                                MTKBTFilterx64        1.1037.0.395 TK                        C:\Windows\system32\drivers\mtkbtfilterx.sys
        // ("MTKBTFilterx64", Some("RZ616/RZ717 Bluetooth Driver")),
    ]);

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

        // select * from CIM_Datafile" & _ " where Name = '" & Replace(strPath, "\", "\\") &
        // C:\Windows\system32\drivers\xinputhid.sys
        // Get-WmiObject -query "SELECT Version FROM CIM_Datafile WHERE Name = 'C:\\Windows\\system32\\drivers\\xinputhid.sys'"
        if !path_name.starts_with("C:") {
            debug!("Skipping path_name: {:?}", path_name);
            // TODO: Probably a UNC path, not sure how to handle it, let's skip it
            continue;
        }
        let query = format!(
            "SELECT Version FROM CIM_Datafile WHERE Name = '{}'",
            path_name.replace("\\", "\\\\")
        );
        let results: Vec<HashMap<String, Variant>> = wmi_con.raw_query(query).unwrap();
        let version = if let Variant::String(s) = &results[0]["Version"] {
            s
        } else {
            ""
        };

        // Skip those that we don't care about
        let str_name: &str = &name;
        //if let Ok(alias) = system_drivers.binary_search_by(|(k, _)| k.cmp(&str_name)).map(|x| system_drivers[x].1) {
        if let Some(alias) = system_drivers.get(&str_name) {
            let alias = if let Some(alias) = alias {
                *alias
            } else {
                &display_name
            };
            println!("  {}", alias);
            debug!("    Display: {}", display_name);
            debug!("    Name:    {}", name);
            debug!("    Path:    {}", path_name);
            println!("    Version: {}", version);
        } else {
            //println!("Not found: {}", display_name);
            //println!("    Name:    '{}'", name);
            //debug!("    Path:    {}", path_name);
            //println!("    Version: {}", version);
            continue;
        }
    }
}
