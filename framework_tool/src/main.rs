use std::fs;

use framework_lib::ec_binary;

pub fn analyze_ec_fw(data: &Vec<u8>) {
    if let Some(ver) = ec_binary::read_ec_version(data) {
        ec_binary::print_ec_version(ver);
    } else {
        println!("Failed to read version")
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 {
        let path = &args[1];
        let data = fs::read(path).expect("Unable to read file");

        println!("File");
        println!("  Size:       {:>20} B", data.len());
        println!("  Size:       {:>20} KB", data.len() / 1024);

        analyze_ec_fw(&data);
    }
}
