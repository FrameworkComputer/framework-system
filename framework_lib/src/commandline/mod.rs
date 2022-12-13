//! Module to build a portable commandline tool
//!
//! Can be easily re-used from any OS or UEFI shell.
//! We have implemented both in the `framework_tool` and `framework_uefi` crates.

#[cfg(not(feature = "uefi"))]
pub mod clap;
#[cfg(feature = "uefi")]
pub mod uefi;

#[cfg(not(feature = "uefi"))]
use std::fs;

use crate::capsule;
use crate::ccgx::device::{PdController, PdPort};
use crate::ccgx::{self, SiliconId::*};
use crate::chromium_ec;
use crate::chromium_ec::print_err;
#[cfg(feature = "linux")]
use crate::csme;
use crate::ec_binary;
use crate::esrt;
use crate::power;
use crate::smbios::{dmidecode_string_val, get_smbios, is_framework};
use smbioslib::*;

use crate::chromium_ec::{CrosEc, CrosEcDriverType};

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

/// Shadows `clap_std::ClapCli` with extras for UEFI
///
/// The UEFI commandline currently doesn't use clap, so we need to shadow the struct.
/// Also it has extra options.
#[derive(Debug)]
pub struct Cli {
    pub versions: bool,
    pub esrt: bool,
    pub power: bool,
    pub pdports: bool,
    pub privacy: bool,
    pub pd_info: bool,
    pub pd_bin: Option<String>,
    pub ec_bin: Option<String>,
    pub capsule: Option<String>,
    pub dump: Option<String>,
    pub driver: Option<CrosEcDriverType>,
    pub test: bool,
    pub intrusion: bool,
    pub kblight: Option<Option<u8>>,
    pub help: bool,
    pub info: bool,
    // UEFI only
    pub allupdate: bool,
    // TODO: This is not actually implemented yet
    pub raw_command: Vec<String>,
}

pub fn parse(args: &[String]) -> Cli {
    #[cfg(feature = "uefi")]
    return uefi::parse(args);
    #[cfg(not(feature = "uefi"))]
    return clap::parse(args);
}

fn print_single_pd_details(pd: &PdController) {
    if let Ok(si) = pd.get_silicon_id() {
        println!("  Silicon ID:     0x{:X}", si);
    } else {
        println!("  Failed to read Silicon ID");
    }
    if let Ok((mode, frs)) = pd.get_device_info() {
        println!("  Mode:           {:?}", mode);
        println!("  Flash Row Size: {} B", frs);
    } else {
        println!("  Failed to device info");
    }
    pd.print_fw_info();
}

fn print_pd_details() {
    if !is_framework() {
        println!("Only supported on Framework systems");
        return;
    }
    let pd_01 = PdController::new(PdPort::Left01);
    let pd_23 = PdController::new(PdPort::Right23);

    println!("Left / Ports 01");
    print_single_pd_details(&pd_01);
    println!("Right / Ports 23");
    print_single_pd_details(&pd_23);
}

fn print_versions(ec: &CrosEc) {
    println!("UEFI BIOS");
    if let Some(smbios) = get_smbios() {
        let bios_entries = smbios.collect::<SMBiosInformation>();
        let bios = bios_entries.get(0).unwrap();
        println!("  Version:        {}", bios.version());
        println!("  Release Date:   {}", bios.release_date());
    }

    println!("EC Firmware");
    let ver = print_err(ec.version_info()).unwrap_or_else(|| "UNKNOWN".to_string());
    println!("  Build version:  {:?}", ver);

    if let Some((ro, rw, curr)) = ec.flash_version() {
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
        println!("  Current image:  Unknown");
    }

    println!("PD Controllers");

    if let Ok(pd_versions) = power::read_pd_version() {
        println!("  Left:           {}", pd_versions.controller01.app);
        println!("  Right:          {}", pd_versions.controller23.app);
    } else if let Ok(pd_versions) = ccgx::get_pd_controller_versions() {
        // If EC doesn't have host command, get it directly from the PD controllers
        // TODO: Maybe print all FW versions
        println!("  Left:           {}", pd_versions.controller01.main_fw.app);
        println!("  Right:          {}", pd_versions.controller23.main_fw.app);
    } else {
        println!("  Unknown")
    }

    println!("Retimers");
    let mut found_retimer = false;
    if let Some(esrt) = esrt::get_esrt() {
        for entry in &esrt.entries {
            match entry.fw_class {
                esrt::RETIMER01_GUID | esrt::RETIMER23_GUID => {
                    if !found_retimer {
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
    } else if !found_retimer {
        println!("  Unknown");
    }

    #[cfg(feature = "linux")]
    {
        println!("CSME");
        if let Ok(csme) = csme::csme_from_sysfs() {
            println!("  Enabled:        {}", csme.enabled);
            println!("  Version:        {}", csme.version);
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
    let ec = if let Some(driver) = args.driver {
        if let Some(driver) = CrosEc::with(driver) {
            driver
        } else {
            println!("Selected driver {:?} not available.", driver);
            return 1;
        }
    } else {
        CrosEc::new()
    };
    if args.help {
        // Only print with uefi feature here because without clap will already
        // have printed the help by itself.
        #[cfg(feature = "uefi")]
        print_help(_allupdate);
        return 2;
    } else if args.versions {
        print_versions(&ec);
    } else if args.esrt {
        print_esrt();
    } else if args.intrusion {
        println!("Chassis status:");
        if let Some(status) = print_err(ec.get_intrusion_status()) {
            println!(
                "  Coin cell ever removed:   {}",
                status.coin_cell_ever_removed
            );
            println!("  Chassis currently open:   {}", status.currently_open);
            println!("  Chassis ever opened:      {}", status.ever_opened);
            println!("  Chassis opened:           {} times", status.total_opened);
            println!(
                "  Chassis opened while off: {} times",
                status.vtr_open_count
            );
        } else {
            println!("  Unable to tell");
        }
    } else if let Some(Some(kblight)) = args.kblight {
        assert!(kblight <= 100);
        ec.set_keyboard_backlight(kblight);
    } else if let Some(None) = args.kblight {
        print!("Keyboard backlight: ");
        if let Some(percentage) = print_err(ec.get_keyboard_backlight()) {
            println!("{}%", percentage);
        } else {
            println!("Unable to tell");
        }
    } else if args.test {
        println!("Self-Test");
        let result = selftest(&ec);
        if result.is_none() {
            return 1;
        }
    } else if args.power {
        power::get_and_print_power_info();
    } else if args.pdports {
        power::get_and_print_pd_info();
    } else if args.info {
        smbios_info();
    } else if args.pd_info {
        print_pd_details();
    } else if args.privacy {
        if let Some((mic, cam)) = print_err(ec.get_privacy_info()) {
            println!(
                "Microphone privacy switch: {}",
                if mic { "Open" } else { "Closed" }
            );
            println!(
                "Camera privacy switch:     {}",
                if cam { "Open" } else { "Closed" }
            );
        };
    // TODO:
    //} else if arg == "-raw-command" {
    //    raw_command(&args[1..]);
    } else if let Some(pd_bin_path) = &args.pd_bin {
        #[cfg(feature = "uefi")]
        let data = crate::uefi::fs::shell_read_file(pd_bin_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(pd_bin_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            analyze_ccgx_pd_fw(&data);
        }
    } else if let Some(ec_bin_path) = &args.ec_bin {
        #[cfg(feature = "uefi")]
        let data = crate::uefi::fs::shell_read_file(ec_bin_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(ec_bin_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            analyze_ec_fw(&data);
        }
    } else if let Some(capsule_path) = &args.capsule {
        #[cfg(feature = "uefi")]
        let data = crate::uefi::fs::shell_read_file(capsule_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(capsule_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            if let Some(_header) = analyze_capsule(&data) {
                // TODO: For now we can only read files on UEFI, not write them
                if _header.capsule_guid == esrt::WINUX_GUID {
                    let ux_header = capsule::parse_ux_header(&data);
                    if let Some(dump_path) = &args.dump {
                        // TODO: Better error handling, rather than just panicking
                        capsule::dump_winux_image(&data, &ux_header, dump_path);
                    }
                }
            } else {
                println!("Capsule is invalid.");
            }
        }
    }

    0
}

// Only on UEFI. Clap prints this by itself
#[cfg(feature = "uefi")]
fn print_help(updater: bool) {
    println!(
        r#"Swiss army knife for Framework laptops

Usage: framework_tool [OPTIONS]

Options:
  -v, --versions             List current firmware versions version
      --esrt                 Display the UEFI ESRT table
      --power                Show current power status (battery and AC)
      --pdports              Show information about USB-C PD prots
      --info                 Show info from SMBIOS (Only on UEFI)
      --pd-info              Show details about the PD controllers
      --privacy              Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>      Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>      Parse versions from EC firmware binary file
      --capsule <CAPSULE>    Parse UEFI Capsule information from binary file
      --intrusion            Show status of intrusion switch
      --kblight [<KBLIGHT>]  Set keyboard backlight percentage or get, if no value provided
  -t, --test                 Run self-test to check if interaction with EC is possible
  -h, --help                 Print help information
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

fn selftest(ec: &CrosEc) -> Option<()> {
    println!("  Checking EC memory mapped magic bytes");
    ec.check_mem_magic()?;

    println!("  Reading EC Build Version");
    print_err(ec.version_info())?;

    println!("  Reading EC Flash");
    ec.flash_version()?;

    println!("  Getting power info from EC");
    power::power_info()?;

    println!("  Getting AC info from EC");
    if power::get_pd_info().iter().any(|x| x.is_err()) {
        return None;
    }

    let pd_01 = PdController::new(PdPort::Left01);
    let pd_23 = PdController::new(PdPort::Right23);
    println!("  Getting PD01 info");
    print_err(pd_01.get_silicon_id())?;
    print_err(pd_01.get_device_info())?;
    println!("  Getting PD23 info");
    print_err(pd_23.get_silicon_id())?;
    print_err(pd_23.get_device_info())?;

    Some(())
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

fn analyze_ccgx_pd_fw(data: &[u8]) {
    let mut succeeded = false;

    if let Some(versions) = ccgx::binary::read_versions(data, Ccg5) {
        succeeded = true;
        println!("Detected CCG5 firmware");
        println!("FW 1");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2");
        ccgx::binary::print_fw(&versions.main_fw);
    }

    if let Some(versions) = ccgx::binary::read_versions(data, Ccg6) {
        succeeded = true;
        println!("Detected CCG6 firmware");
        println!("FW 1 (Backup)");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2 (Main)");
        ccgx::binary::print_fw(&versions.main_fw);
    }

    if !succeeded {
        println!("Failed to read versions")
    }
}

pub fn analyze_ec_fw(data: &[u8]) {
    if let Some(ver) = ec_binary::read_ec_version(data) {
        ec_binary::print_ec_version(&ver);
    } else {
        println!("Failed to read version")
    }
}

pub fn analyze_capsule(data: &[u8]) -> Option<capsule::EfiCapsuleHeader> {
    let header = capsule::parse_capsule_header(data)?;
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
    Some(header)
}
