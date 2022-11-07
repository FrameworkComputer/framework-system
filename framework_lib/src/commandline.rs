//! Module to factor out commandline interaction
//! This way we can use it in the regular OS commandline tool on Linux and Windows,
//! as well as on the UEFI shell tool.

use std::fs;

use clap::Parser;

use crate::chromium_ec;
use crate::ec_binary;
use crate::pd_binary;
use crate::power;

/// Swiss army knife for Framework laptops
#[derive(Parser)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// List current firmware versions version
    #[arg(short, long)]
    versions: bool,

    /// Show current power status (battery and AC)
    #[arg(long)]
    power: bool,

    /// Show information about USB-C PD prots
    #[arg(long)]
    pdports: bool,

    /// Show info from SMBIOS (Only on UEFI)
    //#[arg(long)]
    //info: bool,

    /// Show privacy switch statuses (camera and microphone)
    #[arg(long)]
    privacy: bool,

    /// Parse versions from PD firmware binary file
    #[arg(long)]
    pd_bin: Option<std::path::PathBuf>,

    /// Parse versions from EC firmware binary file
    #[arg(long)]
    ec_bin: Option<std::path::PathBuf>,

    /// test
    #[arg(long, short)]
    test: bool,
}

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

pub fn analyze_ec_fw(data: &[u8]) {
    if let Some(ver) = ec_binary::read_ec_version(data) {
        ec_binary::print_ec_version(&ver);
    } else {
        println!("Failed to read version")
    }
}

//fn device_info() {
//    // TODO: Implement on non UEFI
//    let bios = crate::app::BiosComponent::new();
//    // Prints:
//    // Vendor:         "INSYDE Corp."
//    // BIOS Version:   "03.05"
//    // System Version: "A4"
//    bios.print();
//}

const EC_CMD_PRIVACY_SWITCHES_CHECK_MODE: u16 = 0x3E14; /* Get information about current state of privacy switches */
#[repr(C, packed)]
struct EcResponsePrivacySwitches {
    microphone: u8,
    camera: u8,
}

fn privacy_info() -> Option<(bool, bool)> {
    let data = chromium_ec::send_command(EC_CMD_PRIVACY_SWITCHES_CHECK_MODE, 0, &[])?;
    // TODO: Rust complains that when accessing this struct, we're reading
    // from unaligned pointers. How can I fix this? Maybe create another struct to shadow it,
    // which isn't packed. And copy the data to there.
    let status: EcResponsePrivacySwitches = unsafe { std::ptr::read(data.as_ptr() as *const _) };

    println!(
        "Microphone privacy switch: {}",
        if status.microphone == 1 {
            "Open"
        } else {
            "Closed"
        }
    );
    println!(
        "Camera privacy switch:     {}",
        if status.camera == 1 { "Open" } else { "Closed" }
    );

    Some((status.microphone == 1, status.camera == 1))
}

//fn str_to_u16(string: &str) -> u16 {
//    0
//}
//
//fn args_to_bytes(args: &[String]) -> Vec<u8> {
//    args.into_iter().flat_map(|x| parse_raw_buffer(x)).collect()
//    // TODO: Why can't I do?
//    // args.into_iter().flat_map(parse_raw_buffer).collect()
//}
//
//fn parse_raw_buffer(arg: &str) -> Vec<u8> {
//    vec![]
//}
//
//fn raw_command(args: &[String]) {
//    let command = str_to_u16(&args[0]);
//    let body = args_to_bytes(&args[1..]);
//    // TODO: Check args.len() smaller than maximum size
//    let data = chromium_ec::send_command_lpc_v3(command, 0, &body);
//}

const EC_MEMMAP_ID: u16 = 0x20; /* 0x20 == 'E', 0x21 == 'C' */

pub fn run_with_args(args: &[String]) {
    let args = Cli::parse_from(args);

    if args.versions {
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
        println!("Test");
        match chromium_ec::read_memory(EC_MEMMAP_ID, 2) {
            Some(ec_id) => {
                if ec_id.len() != 2 {
                    println!("Unexpected length returned: {:?}", ec_id.len());
                }
                if ec_id[0] != b'E' || ec_id[1] != b'C' {
                    println!("This machine doesn't look like it has a Framework EC");
                } else {
                    println!("Verified that Framework EC is present!")
                }
            }
            None => println!("Failed to read EC ID from memory map"),
        }
    } else if args.power {
        power::get_and_print_power_info();
    } else if args.pdports {
        power::get_and_print_pd_info();
    //} else if args.info {
    //    device_info();
    } else if args.privacy {
        privacy_info();
    // TODO:
    //} else if arg == "-raw-command" {
    //    raw_command(&args[1..]);
    } else if let Some(pd_bin_path) = args.pd_bin {
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
    } else if let Some(ec_bin_path) = args.ec_bin {
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
}
