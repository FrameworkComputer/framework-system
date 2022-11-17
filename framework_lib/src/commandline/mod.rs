#[cfg(not(feature = "uefi"))]
pub mod clap;
#[cfg(feature = "uefi")]
pub mod uefi;

#[cfg(not(feature = "uefi"))]
use std::fs;

#[cfg(not(feature = "uefi"))]
use crate::capsule;
use crate::chromium_ec;
#[cfg(not(feature = "uefi"))]
use crate::ec_binary;
use crate::esrt;
#[cfg(not(feature = "uefi"))]
use crate::pd_binary;
use crate::power;
use smbioslib::*;

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

// Shadows clap::ClapCli with extras for UEFI
#[derive(Debug)]
pub struct Cli {
    pub versions: bool,
    pub esrt: bool,
    pub power: bool,
    pub pdports: bool,
    pub privacy: bool,
    pub pd_bin: Option<String>,
    pub ec_bin: Option<String>,
    pub capsule: Option<String>,
    pub test: bool,
    pub help: bool,
    // UEFI only
    pub allupdate: bool,
    pub info: bool,
    pub raw_command: Vec<String>,
}

pub fn parse(args: &[String]) -> Cli {
    #[cfg(feature = "uefi")]
    return uefi::parse(args);
    #[cfg(not(feature = "uefi"))]
    return clap::parse(args);
}

fn print_versions() {
    println!("UEFI BIOS");
    if let Some(smbios) = get_smbios() {
        let bios_entries = smbios.collect::<SMBiosInformation>();
        let bios = bios_entries.get(0).unwrap();
        println!("  Version:        {}", bios.version());
        println!("  Release Date:   {}", bios.release_date());
    }

    println!("EC Firmware");
    let ver = chromium_ec::version_info().unwrap_or_else(|| "UNKNOWN".to_string());
    println!("  Build version:  {:?}", ver);

    if let Some((ro, rw, curr)) = chromium_ec::flash_version() {
        println!("  RO Version:     {:?}", ro);
        println!("  RW Version:     {:?}", rw);
        print!("  Current image:  ");
        if curr == chromium_ec::EcCurrentImage::RO {
            println!("RO");
        } else if curr == chromium_ec::EcCurrentImage::RW {
            println!("RW");
        } else {
            println!("Unknown");
        }
    } else {
        println!("  RO Version:     Unknown");
        println!("  RW Version:     Unknown");
        print!("  Current image:  Unknown");
    }

    println!("PD Controllers");
    if let Some(pd_versions) = power::read_pd_version() {
        println!(
            "  Left:           {}",
            power::print_pd_app_ver(&pd_versions.controller01)
        );
        println!(
            "  Right:          {}",
            power::print_pd_app_ver(&pd_versions.controller23)
        );
    }

    #[cfg(feature = "uefi")]
    {
        let mut found_retimer = false;
        if let Some(esrt) = esrt::get_esrt() {
            for entry in &esrt.entries {
                match entry.fw_class {
                    esrt::RETIMER01_GUID | esrt::RETIMER23_GUID => {
                        if !found_retimer {
                            println!("Retimers");
                            found_retimer = true;
                        }
                    }
                    _ => {}
                }
                match entry.fw_class {
                    esrt::RETIMER01_GUID => {
                        println!(
                            "  Left:           0x{:X} ({})",
                            entry.fw_version, entry.fw_version
                        );
                    }
                    esrt::RETIMER23_GUID => {
                        println!(
                            "  Right:          0x{:X} ({})",
                            entry.fw_version, entry.fw_version
                        );
                    }
                    _ => {}
                }
            }
        }
    }
}

fn print_esrt() {
    if let Some(esrt) = esrt::get_esrt() {
        esrt::print_esrt(&esrt);
    } else {
        println!("Could not find and parse ESRT table.");
    }
}

pub fn run_with_args(args: &Cli, _allupdate: bool) -> i32 {
    if args.help {
        // Only print with uefi feature here because without clap will already
        // have printed the help by itself.
        #[cfg(feature = "uefi")]
        print_help(_allupdate);
        return 2;
    } else if args.versions {
        print_versions();
    } else if args.esrt {
        print_esrt();
    } else if args.test {
        println!("Self-Test");
        let result = selftest();
        if result.is_none() {
            return 1;
        }
    } else if args.power {
        power::get_and_print_power_info();
    } else if args.pdports {
        power::get_and_print_pd_info();
    } else if args.info {
        smbios_info();
    } else if args.privacy {
        chromium_ec::privacy_info();
    // TODO:
    //} else if arg == "-raw-command" {
    //    raw_command(&args[1..]);
    } else if let Some(pd_bin_path) = &args.pd_bin {
        #[cfg(feature = "uefi")]
        {
            println!("Parsing PD binary not supported on UEFI: {}", pd_bin_path);
        }
        #[cfg(not(feature = "uefi"))]
        match fs::read(pd_bin_path) {
            Ok(data) => {
                println!("File");
                println!("  Size:       {:>20} B", data.len());
                println!("  Size:       {:>20} KB", data.len() / 1024);
                analyze_ccg6_pd_fw(&data);
            }
            // TODO: Perhaps a more user-friendly error
            Err(e) => println!("Error {:?}", e),
        }
    } else if let Some(ec_bin_path) = &args.ec_bin {
        #[cfg(feature = "uefi")]
        {
            println!("Parsing EC binary not supported on UEFI: {}", ec_bin_path);
        }
        #[cfg(not(feature = "uefi"))]
        match fs::read(ec_bin_path) {
            Ok(data) => {
                println!("File");
                println!("  Size:       {:>20} B", data.len());
                println!("  Size:       {:>20} KB", data.len() / 1024);
                analyze_ec_fw(&data);
            }
            // TODO: Perhaps a more user-friendly error
            Err(e) => println!("Error {:?}", e),
        }
    } else if let Some(capsule_path) = &args.capsule {
        #[cfg(feature = "uefi")]
        {
            println!(
                "Parsing Capsule binary not supported on UEFI: {}",
                capsule_path
            );
        }
        #[cfg(not(feature = "uefi"))]
        match fs::read(capsule_path) {
            Ok(data) => {
                println!("File");
                println!("  Size:       {:>20} B", data.len());
                println!("  Size:       {:>20} KB", data.len() / 1024);
                analyze_capsule(&data);
            }
            // TODO: Perhaps a more user-friendly error
            Err(e) => println!("Error {:?}", e),
        }
    }

    0
}

// Only on UEFI. Clap prints this by itself
#[cfg(feature = "uefi")]
fn print_help(updater: bool) {
    println!(
        r#"
    Framework Laptop Firmware Update Utility

    FWUPDATE [-h]

        -h            - Display this help text
        --versions    - Display the current firmware versions of the system
        --esrt        - Display the UEFI ESRT table
        --power       - Display the current power status (battery and AC)
        --pdports     - Display information about USB-C PD ports
        --privacy     - Display status of the privacy switches
        --test        - Run self-test to check if interaction with EC is possible
        --info        - Display information about the system
    "#
    );
    if updater {
        println!(
            r#"
        --allupdate   - Run procedure to update everything (Involves some manual steps)
    "#
        );
    }
    // TODO: Not supported yet
    //println!(
    //    r#"
    //    --raw-command - Send a raw command to the EC
    //                    Example: raw-command 0x3E14
    //"#
    //);
}

fn selftest() -> Option<()> {
    println!("  Checking EC memory mapped magic bytes");
    chromium_ec::check_mem_magic()?;

    println!("  Reading EC Build Version");
    chromium_ec::version_info()?;

    println!("  Reading EC Flash");
    chromium_ec::flash_version()?;

    println!("  Getting power info from EC");
    power::power_info()?;

    println!("  Getting AC info from EC");
    if power::get_pd_info().iter().any(|x| x.is_none()) {
        return None;
    }

    Some(())
}

pub fn dmidecode_string_val(s: &SMBiosString) -> Option<String> {
    match s.as_ref() {
        Ok(val) if val.is_empty() => Some("Not Specified".to_owned()),
        Ok(val) => Some(val.to_owned()),
        Err(SMBiosStringError::FieldOutOfBounds) => None,
        Err(SMBiosStringError::InvalidStringNumber(_)) => Some("<BAD INDEX>".to_owned()),
        Err(SMBiosStringError::Utf8(val)) => {
            Some(String::from_utf8_lossy(&val.clone().into_bytes()).to_string())
        }
    }
}

#[cfg(feature = "uefi")]
fn get_smbios() -> Option<SMBiosData> {
    let data = crate::uefi::smbios_data().unwrap();
    let version = None; // TODO: Maybe add the version here
    let smbios = SMBiosData::from_vec_and_version(data, version);
    Some(smbios)
}
// On Linux this reads either from /dev/mem or sysfs
// On FreeBSD from /dev/mem
// On Windows from the kernel API
#[cfg(not(feature = "uefi"))]
fn get_smbios() -> Option<SMBiosData> {
    match smbioslib::table_load_from_device() {
        Ok(data) => Some(data),
        Err(err) => {
            println!("failure: {:?}", err);
            None
        }
    }
}

fn smbios_info() {
    let smbios = get_smbios();
    if smbios.is_none() {
        println!("Failed to find SMBIOS");
        return;
    }
    for undefined_struct in smbios.unwrap().iter() {
        match undefined_struct.defined_struct() {
            DefinedStruct::Information(data) => {
                println!("BIOS Information");
                if let Some(vendor) = dmidecode_string_val(&data.vendor()) {
                    println!("\tVendor:       {}", vendor);
                }
                if let Some(version) = dmidecode_string_val(&data.version()) {
                    println!("\tVersion:      {}", version);
                }
                if let Some(release_date) = dmidecode_string_val(&data.release_date()) {
                    println!("\tRelease Date: {}", release_date);
                }
            }
            DefinedStruct::SystemInformation(data) => {
                println!("BIOS Information");
                if let Some(version) = dmidecode_string_val(&data.version()) {
                    println!("\tVersion:      {}", version);
                }
                if let Some(manufacturer) = dmidecode_string_val(&data.manufacturer()) {
                    println!("\tManufacturer: {}", manufacturer);
                }
                if let Some(product_name) = dmidecode_string_val(&data.product_name()) {
                    println!("\tProduct Name: {}", product_name);
                }
                if let Some(wake_up_type) = data.wakeup_type() {
                    println!("\tWake-Up-Type: {:?}", wake_up_type.value);
                }
                if let Some(sku_number) = dmidecode_string_val(&data.sku_number()) {
                    println!("\tSKU Number:   {}", sku_number);
                }
                if let Some(family) = dmidecode_string_val(&data.family()) {
                    println!("\tFamily:       {}", family);
                }
            }
            _ => {}
        }
    }
}

#[cfg(not(feature = "uefi"))]
fn analyze_ccg6_pd_fw(data: &[u8]) {
    //let flash_row_size = 256;
    let flash_row_size = 128;
    if let Some(versions) = pd_binary::read_versions(data, flash_row_size) {
        println!("FW 1");
        pd_binary::print_fw(&versions.first);

        println!("FW 2");
        pd_binary::print_fw(&versions.second);
    } else {
        println!("Failed to read versions")
    }
}

#[cfg(not(feature = "uefi"))]
pub fn analyze_ec_fw(data: &[u8]) {
    if let Some(ver) = ec_binary::read_ec_version(data) {
        ec_binary::print_ec_version(&ver);
    } else {
        println!("Failed to read version")
    }
}

#[cfg(not(feature = "uefi"))]
pub fn analyze_capsule(data: &[u8]) {
    let header = capsule::parse_capsule_header(data);
    capsule::print_capsule_header(&header);

    match header.capsule_guid {
        esrt::BIOS_GUID => {
            println!("  Type:         Framework Insyde BIOS");
        }
        esrt::RETIMER01_GUID => {
            println!("  Type:    Framework Retimer01 (Left)");
        }
        esrt::RETIMER23_GUID => {
            println!("  Type:   Framework Retimer23 (Right)");
        }
        esrt::WINUX_GUID => {
            println!("  Type:            Windows UX capsule");
            let ux_header = capsule::parse_ux_header(data);
            capsule::print_ux_header(&ux_header);
        }
        _ => {
            println!("  Type:                      Unknown");
        }
    }
}
