use std::fs;

use clap::{Parser, ValueEnum};

use framework_lib::ec_binary;
use framework_lib::pd_binary;

#[derive(ValueEnum, Debug, Clone)] // ArgEnum here
enum FwTypes {
    PD,
    EC,
}

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser)]
struct Cli {
    /// The pattern to look for
    #[clap(value_enum)]
    fw_type: FwTypes,
    /// The path to the file to read
    path: std::path::PathBuf,
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
        ec_binary::print_ec_version(ver);
    } else {
        println!("Failed to read version")
    }
}

fn main() {
    let args = Cli::parse();

    match fs::read(args.path) {
        Ok(data) => {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);

            match args.fw_type {
                FwTypes::PD => analyze_ccg6_pd_fw(&data),
                FwTypes::EC => analyze_ec_fw(&data),
            }
        }
        // TODO: Perhaps a more user-friendly error
        Err(e) => println!("Error {:?}", e),
    }
}
