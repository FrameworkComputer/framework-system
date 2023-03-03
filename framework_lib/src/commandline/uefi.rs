use std::uefi;

use core::convert::TryInto;
use uefi::guid::{Guid, SHELL_PARAMETERS_GUID};
use uefi::shell::ShellParameters as UefiShellParameters;
use uefi_std::ffi;
use uefi_std::proto::Protocol;

use crate::chromium_ec::CrosEcDriverType;
use crate::commandline::Cli;

use super::ConsoleArg;

pub struct ShellParameters(pub &'static mut UefiShellParameters);

impl Protocol<UefiShellParameters> for ShellParameters {
    fn guid() -> Guid {
        SHELL_PARAMETERS_GUID
    }

    fn new(inner: &'static mut UefiShellParameters) -> Self {
        ShellParameters(inner)
    }
}

// Convert C array of UCS-2 strings to vector of UTF-8 strings
// Basically UEFI to Rust
fn array_nstr(wstr_array: *const *const u16, size: usize) -> Vec<String> {
    let mut strings = vec![];

    for i in 0..size {
        let str = unsafe { ffi::nstr(*wstr_array.offset(i.try_into().unwrap())) };
        strings.push(str);
    }

    strings
}

/// Get commandline arguments from UEFI environment
pub fn get_args() -> Vec<String> {
    let image_handle = std::handle();

    if let Ok(parameters) = ShellParameters::handle_protocol(image_handle) {
        let ptr: *const *const u16 = parameters.0.Argv;
        let size: usize = parameters.0.Argc;

        array_nstr(ptr, size)
    } else {
        // TODO: Maybe handle errors properly
        vec![]
    }
}

pub fn parse(args: &[String]) -> Cli {
    let mut cli = Cli {
        versions: false,
        esrt: false,
        power: false,
        pdports: false,
        pd_info: false,
        privacy: false,
        pd_bin: None,
        ec_bin: None,
        capsule: None,
        dump: None,
        ho2_capsule: None,
        intrusion: false,
        kblight: None,
        console: None,
        // This is the only driver that works on UEFI
        driver: Some(CrosEcDriverType::Portio),
        test: false,
        help: false,
        allupdate: false,
        info: false,
        raw_command: vec![],
    };

    if args.len() == 1 {
        cli.help = true;
    }

    let mut found_an_option = false;

    for (i, arg) in args.iter().enumerate() {
        if arg == "-v" || arg == "--versions" {
            cli.versions = true;
            found_an_option = true;
        } else if arg == "--esrt" {
            cli.esrt = true;
            found_an_option = true;
        } else if arg == "--power" {
            cli.power = true;
            found_an_option = true;
        } else if arg == "--pdports" {
            cli.pdports = true;
            found_an_option = true;
        } else if arg == "--allupdate" {
            cli.allupdate = true;
            found_an_option = true;
        } else if arg == "--info" {
            cli.info = true;
            found_an_option = true;
        } else if arg == "--intrusion" {
            cli.intrusion = true;
            found_an_option = true;
        } else if arg == "--kblight" {
            cli.kblight = if args.len() > i + 1 {
                if let Ok(percent) = args[i + 1].parse::<u8>() {
                    Some(Some(percent))
                } else {
                    println!(
                        "Invalid value for --kblight: '{}'. Must be integer < 100.",
                        args[i + 1]
                    );
                    None
                }
            } else {
                Some(None)
            };
            found_an_option = true;
        } else if arg == "--console" {
            cli.console = if args.len() > i + 1 {
                let console_arg = &args[i + 1];
                if console_arg == "recent" {
                    Some(ConsoleArg::Recent)
                } else if console_arg == "follow" {
                    Some(ConsoleArg::Follow)
                } else {
                    println!("Invalid value for --console: {}", console_arg);
                    None
                }
            } else {
                println!("Need to provide a value for --console. Either `follow` or `recent`");
                None
            };
            found_an_option = true;
        } else if arg == "-t" || arg == "--test" {
            cli.test = true;
            found_an_option = true;
        } else if arg == "-h" || arg == "--help" {
            cli.help = true;
            found_an_option = true;
        } else if arg == "--pd-info" {
            cli.pd_info = true;
            found_an_option = true;
        } else if arg == "--privacy" {
            cli.privacy = true;
            found_an_option = true;
        } else if arg == "--pd-bin" {
            cli.pd_bin = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--pd-bin requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--ec-bin" {
            cli.ec_bin = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--ec-bin requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--capsule" {
            cli.capsule = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--capsule requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--dump" {
            cli.dump = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--dump requires extra argument to denote output file");
                None
            };
            found_an_option = true;
        } else if arg == "--ho2-capsule" {
            cli.ho2_capsule = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--ho2-capsule requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--raw-command" {
            cli.raw_command = args[1..].to_vec();
        }
    }

    if !found_an_option {
        println!(
            "Failed to parse any commandline options. Commandline was: {:?}",
            args
        );
    }

    cli
}
