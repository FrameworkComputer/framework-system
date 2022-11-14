#[cfg(not(feature = "uefi"))]
pub mod clap;
#[cfg(feature = "uefi")]
pub mod uefi;

#[cfg(not(feature = "uefi"))]
use std::fs;

use crate::chromium_ec;
#[cfg(not(feature = "uefi"))]
use crate::ec_binary;
#[cfg(not(feature = "uefi"))]
use crate::pd_binary;
use crate::power;

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

// Shadows clap::ClapCli with extras for UEFI
#[derive(Debug)]
pub struct Cli {
    pub versions: bool,
    pub power: bool,
    pub pdports: bool,
    pub privacy: bool,
    pub pd_bin: Option<String>,
    pub ec_bin: Option<String>,
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

pub fn run_with_args(args: &Cli, _allupdate: bool) -> i32 {
    if args.help {
        #[cfg(feature = "uefi")]
        print_help(_allupdate);
        return 2;
    } else if args.versions {
        let ver = chromium_ec::version_info().unwrap_or_else(|| "UNKNOWN".to_string());
        println!("Build version:  {:?}", ver);

        if let Some((ro, rw, curr)) = chromium_ec::flash_version() {
            println!("RO Version:     {:?}", ro);
            println!("RW Version:     {:?}", rw);
            print!("Current image:  ");
            if curr == chromium_ec::EcCurrentImage::RO {
                println!("RO");
            } else if curr == chromium_ec::EcCurrentImage::RW {
                println!("RW");
            } else {
                println!("Unknown");
            }
        } else {
            println!("RO Version:     Unknown");
            println!("RW Version:     Unknown");
            print!("Current image:  Unknown");
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
    //} else if args.info {
    //    device_info();
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
        --power       - Display the current power status (battery and AC)
        --pdports     - Display information about USB-C PD ports
        --privacy     - Display status of the privacy switches
        --test        - Run self-test to check if interaction with EC is possible
    "#
    );
    if updater {
        println!(
            r#"
        --info        - Display information about the system
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
