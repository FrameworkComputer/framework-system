#[cfg(not(feature = "uefi"))]
pub mod clap;
#[cfg(feature = "uefi")]
pub mod uefi;

#[cfg(not(feature = "uefi"))]
use std::fs;

use crate::capsule;
use crate::chromium_ec;
use crate::csme;
use crate::ec_binary;
use crate::esrt;
use crate::pd_binary::{self, CcgX::*};
use crate::power;
use crate::smbios::{dmidecode_string_val, get_smbios};
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
    pub dump: Option<String>,
    pub test: bool,
    pub intrusion: bool,
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
        println!("  Current image:  Unknown");
    }

    println!("PD Controllers");
    if let Some(pd_versions) = power::read_pd_version() {
        println!(
            "  Left:           {}",
            power::format_pd_app_ver(&pd_versions.controller01)
        );
        println!(
            "  Right:          {}",
            power::format_pd_app_ver(&pd_versions.controller23)
        );
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
            println!("  Version:        {}", csme::format_csme_ver(&csme));
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
    } else if args.intrusion {
        println!("Chassis status:");
        if let Some(status) = chromium_ec::get_intrusion_status() {
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
  -v, --versions           List current firmware versions version
      --esrt               Display the UEFI ESRT table
      --power              Show current power status (battery and AC)
      --pdports            Show information about USB-C PD prots
      --info               Show info from SMBIOS (Only on UEFI)
      --privacy            Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>    Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>    Parse versions from EC firmware binary file
      --capsule <CAPSULE>  Parse UEFI Capsule information from binary file
  -t, --test               Run self-test to check if interaction with EC is possible
  -h, --help               Print help information
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

    if let Some(versions) = pd_binary::read_versions(data, Ccg5) {
        succeeded = true;
        println!("Detected CCG5 firmware");
        println!("FW 1");
        pd_binary::print_fw(&versions.first);

        println!("FW 2");
        pd_binary::print_fw(&versions.second);
    }

    if let Some(versions) = pd_binary::read_versions(data, Ccg6) {
        succeeded = true;
        println!("Detected CCG6 firmware");
        println!("FW 1");
        pd_binary::print_fw(&versions.first);

        println!("FW 2");
        pd_binary::print_fw(&versions.second);
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
