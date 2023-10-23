use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::prelude::BootServices;
use uefi::proto::shell_params::*;
use uefi::table::boot::{OpenProtocolAttributes, OpenProtocolParams, SearchType};
use uefi::Identify;

use crate::chromium_ec::CrosEcDriverType;
use crate::commandline::Cli;

use super::{ConsoleArg, InputDeckModeArg};

/// Get commandline arguments from UEFI environment
pub fn get_args(boot_services: &BootServices) -> Vec<String> {
    // TODO: I think i should open this from the ImageHandle?
    let shell_params_h =
        boot_services.locate_handle_buffer(SearchType::ByProtocol(&ShellParameters::GUID));
    let shell_params_h = if let Ok(shell_params_h) = shell_params_h {
        shell_params_h
    } else {
        error!("ShellParameters protocol not found");
        return vec![];
    };

    for handle in &*shell_params_h {
        let params_handle = unsafe {
            boot_services
                .open_protocol::<ShellParameters>(
                    OpenProtocolParams {
                        handle: *handle,
                        agent: boot_services.image_handle(),
                        controller: None,
                    },
                    OpenProtocolAttributes::GetProtocol,
                )
                .expect("Failed to open ShellParameters handle")
        };

        // Ehm why are there two and one has no args?
        // Maybe one is the shell itself?
        if params_handle.argc == 0 {
            continue;
        }

        return params_handle.get_args();
    }
    vec![]
}

pub fn parse(args: &[String]) -> Cli {
    let mut cli = Cli {
        verbosity: log::LevelFilter::Error,
        paginate: false,
        versions: false,
        version: false,
        esrt: false,
        power: false,
        pdports: false,
        pd_info: false,
        dp_hdmi_info: false,
        dp_hdmi_update: None,
        audio_card_info: false,
        privacy: false,
        pd_bin: None,
        ec_bin: None,
        capsule: None,
        dump: None,
        ho2_capsule: None,
        intrusion: false,
        inputmodules: false,
        input_deck_mode: None,
        charge_limit: None,
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

    if args.len() == 0 {
        cli.help = true;
    }

    let mut found_an_option = false;

    for (i, arg) in args.iter().enumerate() {
        if arg == "-q" {
            cli.verbosity = log::LevelFilter::Off;
        } else if arg == "-v" {
            cli.verbosity = log::LevelFilter::Warn;
        } else if arg == "-vv" {
            cli.verbosity = log::LevelFilter::Info;
        } else if arg == "-vvv" {
            cli.verbosity = log::LevelFilter::Debug;
        } else if arg == "-vvvv" {
            cli.verbosity = log::LevelFilter::Trace;
        } else if arg == "--versions" {
            cli.versions = true;
            found_an_option = true;
        } else if arg == "--version" {
            cli.version = true;
            found_an_option = true;
        } else if arg == "-b" {
            cli.paginate = true;
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
        } else if arg == "--inputmodules" {
            cli.inputmodules = true;
            found_an_option = true;
        } else if arg == "--input-deck-mode" {
            cli.input_deck_mode = if args.len() > i + 1 {
                let input_deck_mode = &args[i + 1];
                if input_deck_mode == "auto" {
                    Some(InputDeckModeArg::Auto)
                } else if input_deck_mode == "off" {
                    Some(InputDeckModeArg::Off)
                } else if input_deck_mode == "on" {
                    Some(InputDeckModeArg::On)
                } else {
                    println!("Invalid value for --input-deck-mode: {}", input_deck_mode);
                    None
                }
            } else {
                println!(
                    "Need to provide a value for --input-deck-mode. Either `auto`, `off`, or `on`"
                );
                None
            };
            found_an_option = true;
        } else if arg == "--charge-limit" {
            cli.charge_limit = if args.len() > i + 1 {
                if let Ok(percent) = args[i + 1].parse::<u8>() {
                    Some(Some(percent))
                } else {
                    println!(
                        "Invalid value for --charge_limit: '{}'. Must be integer < 100.",
                        args[i + 1]
                    );
                    None
                }
            } else {
                Some(None)
            };
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

    if args.len() == 1 && cli.paginate {
        cli.help = true;
        found_an_option = true;
    }

    if !found_an_option {
        println!(
            "Failed to parse any commandline options. Commandline was: {:?}",
            args
        );
    }

    cli
}
