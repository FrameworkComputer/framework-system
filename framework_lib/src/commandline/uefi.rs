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

use super::{ConsoleArg, FpBrightnessArg, InputDeckModeArg, RebootEcArg};

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
        thermal: false,
        sensors: false,
        pdports: false,
        pd_info: false,
        dp_hdmi_info: false,
        dp_hdmi_update: None,
        audio_card_info: false,
        privacy: false,
        pd_bin: None,
        ec_bin: None,
        dump_ec_flash: None,
        flash_ec: None,
        flash_ro_ec: None,
        flash_rw_ec: None,
        capsule: None,
        dump: None,
        ho2_capsule: None,
        intrusion: false,
        inputmodules: false,
        input_deck_mode: None,
        charge_limit: None,
        fp_brightness: None,
        kblight: None,
        console: None,
        reboot_ec: None,
        hash: None,
        // This is the only driver that works on UEFI
        driver: Some(CrosEcDriverType::Portio),
        pd_addrs: None,
        pd_ports: None,
        has_mec: None,
        test: false,
        help: false,
        allupdate: false,
        info: false,
        serialnums: false,
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
        } else if arg == "--thermal" {
            cli.thermal = true;
            found_an_option = true;
        } else if arg == "--sensors" {
            cli.sensors = true;
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
        } else if arg == "--serialnums" {
            cli.serialnums = true;
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
                        "Invalid value for --charge-limit: '{}'. Must be integer < 100.",
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
        } else if arg == "--fp-brightness" {
            cli.fp_brightness = if args.len() > i + 1 {
                let fp_brightness_arg = &args[i + 1];
                if fp_brightness_arg == "high" {
                    Some(Some(FpBrightnessArg::High))
                } else if fp_brightness_arg == "medium" {
                    Some(Some(FpBrightnessArg::Medium))
                } else if fp_brightness_arg == "low" {
                    Some(Some(FpBrightnessArg::Low))
                } else {
                    println!("Invalid value for --fp-brightness: {}", fp_brightness_arg);
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
        } else if arg == "--reboot-ec" {
            cli.reboot_ec = if args.len() > i + 1 {
                let reboot_ec_arg = &args[i + 1];
                if reboot_ec_arg == "reboot" {
                    Some(RebootEcArg::Reboot)
                } else if reboot_ec_arg == "jump-ro" {
                    Some(RebootEcArg::JumpRo)
                } else if reboot_ec_arg == "jump-rw" {
                    Some(RebootEcArg::JumpRw)
                } else if reboot_ec_arg == "cancel-jump" {
                    Some(RebootEcArg::CancelJump)
                } else if reboot_ec_arg == "disable-jump" {
                    Some(RebootEcArg::DisableJump)
                } else {
                    println!("Invalid value for --reboot-ec: {}", reboot_ec_arg);
                    None
                }
            } else {
                println!("Need to provide a value for --reboot-ec. Either `reboot`, `jump-ro`, `jump-rw`, `cancel-jump` or `disable-jump`");
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
        } else if arg == "--dump-ec-flash" {
            cli.dump_ec_flash = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--dump-ec-flash requires extra argument to denote output file");
                None
            };
            found_an_option = true;
        } else if arg == "--flash-ec" {
            cli.flash_ec = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--flash-ec requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--flash-ro-ec" {
            cli.flash_ro_ec = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--flash-ro-ec requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--flash-rw-ec" {
            cli.flash_rw_ec = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--flash-rw-ec requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--hash" {
            cli.hash = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--hash requires extra argument to denote input file");
                None
            };
            found_an_option = true;
        } else if arg == "--pd-addrs" {
            cli.pd_addrs = if args.len() > i + 2 {
                let left = args[i + 1].parse::<u16>();
                let right = args[i + 2].parse::<u16>();
                if left.is_ok() && right.is_ok() {
                    Some((left.unwrap(), right.unwrap()))
                } else {
                    println!(
                        "Invalid values for --pd-addrs: '{} {}'. Must be u16 integers.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else {
                println!("--pd-addrs requires two arguments, one for each address");
                None
            };
            found_an_option = true;
        } else if arg == "--pd-ports" {
            cli.pd_ports = if args.len() > i + 2 {
                let left = args[i + 1].parse::<u8>();
                let right = args[i + 2].parse::<u8>();
                if left.is_ok() && right.is_ok() {
                    Some((left.unwrap(), right.unwrap()))
                } else {
                    println!(
                        "Invalid values for --pd-ports: '{} {}'. Must be u16 integers.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else {
                println!("--pd-ports requires two arguments, one for each port");
                None
            };
            found_an_option = true;
        } else if arg == "--has-mec" {
            cli.has_mec = if args.len() > i + 1 {
                if let Ok(b) = args[i + 1].parse::<bool>() {
                    Some(b)
                } else {
                    println!(
                        "Invalid value for --has-mec: '{}'. Must be 'true' or 'false'.",
                        args[i + 1]
                    );
                    None
                }
            } else {
                println!("--has-mec requires extra boolean argument.");
                None
            };
            found_an_option = true;
        } else if arg == "--raw-command" {
            cli.raw_command = args[1..].to_vec();
        }
    }

    let custom_platform = cli.pd_addrs.is_some() && cli.pd_ports.is_some() && cli.has_mec.is_some();
    let no_customization =
        cli.pd_addrs.is_none() && cli.pd_ports.is_none() && cli.has_mec.is_none();
    if !(custom_platform || no_customization) {
        println!("To customize the platform you need to provide all of --pd-addrs, --pd-ports and --has-mec");
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
