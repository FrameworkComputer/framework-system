use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use uefi::prelude::BootServices;
use uefi::proto::shell_params::*;
use uefi::Handle;

use crate::chromium_ec::commands::SetGpuSerialMagic;
use crate::chromium_ec::{CrosEcDriverType, HardwareDeviceType};
use crate::commandline::{Cli, LogLevel};

use super::{ConsoleArg, FpBrightnessArg, InputDeckModeArg, RebootEcArg, TabletModeArg};

/// Get commandline arguments from UEFI environment
pub fn get_args(bs: &BootServices, image_handle: Handle) -> Vec<String> {
    if let Ok(shell_params) = bs.open_protocol_exclusive::<ShellParameters>(image_handle) {
        shell_params.get_args()
    } else {
        // No protocol found if the application wasn't executed by the shell
        vec![]
    }
}

pub fn parse(args: &[String]) -> Cli {
    let mut cli = Cli {
        verbosity: LogLevel(log::LevelFilter::Error),
        paginate: false,
        versions: false,
        version: false,
        features: false,
        esrt: false,
        device: None,
        compare_version: None,
        power: false,
        thermal: false,
        sensors: false,
        fansetduty: None,
        fansetrpm: None,
        autofanctrl: false,
        pdports: false,
        pd_info: false,
        pd_reset: None,
        pd_disable: None,
        pd_enable: None,
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
        h2o_capsule: None,
        intrusion: false,
        inputdeck: false,
        inputdeck_mode: None,
        expansion_bay: false,
        charge_limit: None,
        charge_current_limit: None,
        charge_rate_limit: None,
        get_gpio: None,
        fp_led_level: None,
        fp_brightness: None,
        kblight: None,
        remap_key: None,
        rgbkbd: vec![],
        ps2_enable: None,
        tablet_mode: None,
        touchscreen_enable: None,
        stylus_battery: false,
        console: None,
        reboot_ec: None,
        ec_hib_delay: None,
        hash: None,
        // This is the only driver that works on UEFI
        driver: Some(CrosEcDriverType::Portio),
        pd_addrs: None,
        pd_ports: None,
        test: false,
        dry_run: false,
        force: false,
        help: false,
        flash_gpu_descriptor: None,
        flash_gpu_descriptor_file: None,
        dump_gpu_descriptor_file: None,
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
            cli.verbosity = LogLevel(log::LevelFilter::Off);
        } else if arg == "-v" {
            cli.verbosity = LogLevel(log::LevelFilter::Warn);
        } else if arg == "-vv" {
            cli.verbosity = LogLevel(log::LevelFilter::Info);
        } else if arg == "-vvv" {
            cli.verbosity = LogLevel(log::LevelFilter::Debug);
        } else if arg == "-vvvv" {
            cli.verbosity = LogLevel(log::LevelFilter::Trace);
        } else if arg == "--versions" {
            cli.versions = true;
            found_an_option = true;
        } else if arg == "--version" {
            cli.version = true;
            found_an_option = true;
        } else if arg == "--features" {
            cli.features = true;
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
        } else if arg == "--fansetduty" {
            cli.fansetduty = if args.len() > i + 2 {
                let fan_idx = args[i + 1].parse::<u32>();
                let duty = args[i + 2].parse::<u32>();
                if let (Ok(fan_idx), Ok(duty)) = (fan_idx, duty) {
                    Some((Some(fan_idx), duty))
                } else {
                    println!(
                        "Invalid values for --fansetduty: '{} {}'. Must be u32 integers.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else if args.len() > i + 1 {
                if let Ok(duty) = args[i + 1].parse::<u32>() {
                    Some((None, duty))
                } else {
                    println!(
                        "Invalid values for --fansetduty: '{}'. Must be 0-100.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--fansetduty requires one or two. [fan id] [duty] or [duty]");
                None
            };
            found_an_option = true;
        } else if arg == "--fansetrpm" {
            cli.fansetrpm = if args.len() > i + 2 {
                let fan_idx = args[i + 1].parse::<u32>();
                let rpm = args[i + 2].parse::<u32>();
                if let (Ok(fan_idx), Ok(rpm)) = (fan_idx, rpm) {
                    Some((Some(fan_idx), rpm))
                } else {
                    println!(
                        "Invalid values for --fansetrpm: '{} {}'. Must be u32 integers.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else if args.len() > i + 1 {
                if let Ok(rpm) = args[i + 1].parse::<u32>() {
                    Some((None, rpm))
                } else {
                    println!(
                        "Invalid values for --fansetrpm: '{}'. Must be an integer.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--fansetrpm requires one or two. [fan id] [rpm] or [rpm]");
                None
            };
            found_an_option = true;
        } else if arg == "--autofanctrol" {
            cli.autofanctrl = true;
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
        } else if arg == "--inputdeck" {
            cli.inputdeck = true;
            found_an_option = true;
        } else if arg == "--inputdeck-mode" {
            cli.inputdeck_mode = if args.len() > i + 1 {
                let inputdeck_mode = &args[i + 1];
                if inputdeck_mode == "auto" {
                    Some(InputDeckModeArg::Auto)
                } else if inputdeck_mode == "off" {
                    Some(InputDeckModeArg::Off)
                } else if inputdeck_mode == "on" {
                    Some(InputDeckModeArg::On)
                } else {
                    println!("Invalid value for --inputdeck-mode: {}", inputdeck_mode);
                    None
                }
            } else {
                println!(
                    "Need to provide a value for --inputdeck-mode. Either `auto`, `off`, or `on`"
                );
                None
            };
            found_an_option = true;
        } else if arg == "--expansion-bay" {
            cli.expansion_bay = true;
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
        } else if arg == "--charge-current-limit" {
            cli.charge_current_limit = if args.len() > i + 2 {
                let limit = args[i + 1].parse::<u32>();
                let soc = args[i + 2].parse::<u32>();
                if let (Ok(limit), Ok(soc)) = (limit, soc) {
                    Some((limit, Some(soc)))
                } else {
                    println!(
                        "Invalid values for --charge-current-limit: '{} {}'. Must be u32 integers.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else if args.len() > i + 1 {
                if let Ok(limit) = args[i + 1].parse::<u32>() {
                    Some((limit, None))
                } else {
                    println!(
                        "Invalid values for --charge-current-limit: '{}'. Must be an integer.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--charge-current-limit requires one or two. [limit] [soc] or [limit]");
                None
            };
            found_an_option = true;
        } else if arg == "--charge-rate-limit" {
            cli.charge_rate_limit = if args.len() > i + 2 {
                let limit = args[i + 1].parse::<f32>();
                let soc = args[i + 2].parse::<f32>();
                if let (Ok(limit), Ok(soc)) = (limit, soc) {
                    Some((limit, Some(soc)))
                } else {
                    println!(
                        "Invalid values for --charge-rate-limit: '{} {}'. Must be u32 integers.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else if args.len() > i + 1 {
                if let Ok(limit) = args[i + 1].parse::<f32>() {
                    Some((limit, None))
                } else {
                    println!(
                        "Invalid values for --charge-rate-limit: '{}'. Must be an integer.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--charge-rate-limit requires one or two. [limit] [soc] or [limit]");
                None
            };
            found_an_option = true;
        } else if arg == "--get-gpio" {
            cli.get_gpio = if args.len() > i + 1 {
                Some(Some(args[i + 1].clone()))
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
        } else if arg == "--rgbkbd" {
            cli.rgbkbd = if args.len() > i + 2 {
                let mut colors = Vec::<u64>::new();
                for color_i in i + 1..args.len() {
                    // TODO: Fail parsing instead of unwrap()
                    colors.push(args[color_i].parse::<u64>().unwrap());
                }
                colors
            } else {
                println!("--rgbkbd requires at least 2 arguments, the start key and an RGB value");
                vec![]
            }
        } else if arg == "--ps2-enable" {
            cli.ps2_enable = if args.len() > i + 1 {
                let enable_arg = &args[i + 1];
                if enable_arg == "true" {
                    Some(true)
                } else if enable_arg == "false" {
                    Some(false)
                } else {
                    println!(
                        "Need to provide a value for --ps2-enable: '{}'. {}",
                        args[i + 1],
                        "Must be `true` or `false`",
                    );
                    None
                }
            } else {
                println!("Need to provide a value for --tablet-mode. One of: `auto`, `tablet` or `laptop`");
                None
            };
            found_an_option = true;
        } else if arg == "--tablet-mode" {
            cli.tablet_mode = if args.len() > i + 1 {
                let tablet_mode_arg = &args[i + 1];
                if tablet_mode_arg == "auto" {
                    Some(TabletModeArg::Auto)
                } else if tablet_mode_arg == "tablet" {
                    Some(TabletModeArg::Tablet)
                } else if tablet_mode_arg == "laptop" {
                    Some(TabletModeArg::Laptop)
                } else {
                    println!(
                        "Need to provide a value for --tablet-mode: '{}'. {}",
                        args[i + 1],
                        "Must be one of: `auto`, `tablet` or `laptop`",
                    );
                    None
                }
            } else {
                println!("Need to provide a value for --tablet-mode. One of: `auto`, `tablet` or `laptop`");
                None
            };
            found_an_option = true;
        } else if arg == "--fp-led-level" {
            cli.fp_led_level = if args.len() > i + 1 {
                let fp_led_level_arg = &args[i + 1];
                if fp_led_level_arg == "high" {
                    Some(Some(FpBrightnessArg::High))
                } else if fp_led_level_arg == "medium" {
                    Some(Some(FpBrightnessArg::Medium))
                } else if fp_led_level_arg == "low" {
                    Some(Some(FpBrightnessArg::Low))
                } else if fp_led_level_arg == "ultra-low" {
                    Some(Some(FpBrightnessArg::UltraLow))
                } else if fp_led_level_arg == "auto" {
                    Some(Some(FpBrightnessArg::Auto))
                } else {
                    println!("Invalid value for --fp-led-level: {}", fp_led_level_arg);
                    None
                }
            } else {
                Some(None)
            };
            found_an_option = true;
        } else if arg == "--fp-brightness" {
            cli.fp_brightness = if args.len() > i + 1 {
                if let Ok(fp_brightness_arg) = args[i + 1].parse::<u8>() {
                    if fp_brightness_arg == 0 || fp_brightness_arg > 100 {
                        println!(
                            "Invalid value for --fp-brightness: {}. Must be in the range of 1-100",
                            fp_brightness_arg
                        );
                        None
                    } else {
                        Some(Some(fp_brightness_arg))
                    }
                } else {
                    println!("Invalid value for --fp-brightness. Must be in the range of 1-100");
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
        } else if arg == "--reboot-ec" {
            cli.ec_hib_delay = if args.len() > i + 1 {
                if let Ok(delay) = args[i + 1].parse::<u32>() {
                    if delay == 0 {
                        println!("Invalid value for --ec-hib-delay: {}. Must be >0", delay);
                        None
                    } else {
                        Some(Some(delay))
                    }
                } else {
                    println!("Invalid value for --fp-brightness. Must be amount in seconds >0");
                    None
                }
            } else {
                Some(None)
            };
            found_an_option = true;
        } else if arg == "-t" || arg == "--test" {
            cli.test = true;
            found_an_option = true;
        } else if arg == "-f" || arg == "--force" {
            cli.force = true;
            found_an_option = true;
        } else if arg == "--dry-run" {
            cli.dry_run = true;
            found_an_option = true;
        } else if arg == "-h" || arg == "--help" {
            cli.help = true;
            found_an_option = true;
        } else if arg == "--pd-info" {
            cli.pd_info = true;
            found_an_option = true;
        } else if arg == "--pd-reset" {
            cli.pd_reset = if args.len() > i + 1 {
                if let Ok(pd) = args[i + 1].parse::<u8>() {
                    Some(pd)
                } else {
                    println!(
                        "Invalid value for --pd-reset: '{}'. Must be 0 or 1.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--pd-reset requires specifying the PD controller");
                None
            };
            found_an_option = true;
        } else if arg == "--pd-disable" {
            cli.pd_reset = if args.len() > i + 1 {
                if let Ok(pd) = args[i + 1].parse::<u8>() {
                    Some(pd)
                } else {
                    println!(
                        "Invalid value for --pd-disable: '{}'. Must be 0 or 1.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--pd-disable requires specifying the PD controller");
                None
            };
            found_an_option = true;
        } else if arg == "--pd-enable" {
            cli.pd_enable = if args.len() > i + 1 {
                if let Ok(pd) = args[i + 1].parse::<u8>() {
                    Some(pd)
                } else {
                    println!(
                        "Invalid value for --pd-enable: '{}'. Must be 0 or 1.",
                        args[i + 1],
                    );
                    None
                }
            } else {
                println!("--pd-enable requires specifying the PD controller");
                None
            };
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
        } else if arg == "--h2o-capsule" {
            cli.h2o_capsule = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--h2o-capsule requires extra argument to denote input file");
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
            cli.pd_addrs = if args.len() > i + 3 {
                let left = args[i + 1].parse::<u16>();
                let right = args[i + 2].parse::<u16>();
                let back = args[i + 3].parse::<u16>();
                if left.is_ok() && right.is_ok() && back.is_ok() {
                    Some((left.unwrap(), right.unwrap(), back.unwrap()))
                } else {
                    println!(
                        "Invalid values for --pd-addrs: '{} {} {}'. Must be u16 integers.",
                        args[i + 1],
                        args[i + 2],
                        args[i + 3]
                    );
                    None
                }
            } else {
                println!("--pd-addrs requires three arguments, one for each address");
                None
            };
            found_an_option = true;
        } else if arg == "--pd-ports" {
            cli.pd_ports = if args.len() > i + 3 {
                let left = args[i + 1].parse::<u8>();
                let right = args[i + 2].parse::<u8>();
                let back = args[i + 3].parse::<u8>();
                if left.is_ok() && right.is_ok() && back.is_ok() {
                    Some((left.unwrap(), right.unwrap(), back.unwrap()))
                } else {
                    println!(
                        "Invalid values for --pd-ports: '{} {} {}'. Must be u16 integers.",
                        args[i + 1],
                        args[i + 2],
                        args[i + 3]
                    );
                    None
                }
            } else {
                println!("--pd-ports requires two arguments, one for each port");
                None
            };
            found_an_option = true;
        } else if arg == "--raw-command" {
            cli.raw_command = args[1..].to_vec();
        } else if arg == "--compare-version" {
            cli.compare_version = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("--compare-version requires extra argument to denote version");
                None
            };
            found_an_option = true;
        } else if arg == "--device" {
            cli.device = if args.len() > i + 1 {
                let console_arg = &args[i + 1];
                if console_arg == "bios" {
                    Some(HardwareDeviceType::BIOS)
                } else if console_arg == "ec" {
                    Some(HardwareDeviceType::EC)
                } else if console_arg == "pd0" {
                    Some(HardwareDeviceType::PD0)
                } else if console_arg == "pd1" {
                    Some(HardwareDeviceType::PD1)
                } else if console_arg == "rtm01" {
                    Some(HardwareDeviceType::RTM01)
                } else if console_arg == "rtm23" {
                    Some(HardwareDeviceType::RTM23)
                } else if console_arg == "ac-left" {
                    Some(HardwareDeviceType::AcLeft)
                } else if console_arg == "ac-right" {
                    Some(HardwareDeviceType::AcRight)
                } else {
                    println!("Invalid value for --device: {}", console_arg);
                    None
                }
            } else {
                println!("Need to provide a value for --console. Possible values: bios, ec, pd0, pd1, rtm01, rtm23, ac-left, ac-right");
                None
            };
        } else if arg == "--flash-gpu-descriptor" {
            cli.flash_gpu_descriptor = if args.len() > i + 2 {
                let sn = args[i + 2].to_string();
                let magic = &args[i + 1];

                let hex_magic = if let Some(hex_magic) = magic.strip_prefix("0x") {
                    u8::from_str_radix(hex_magic, 16)
                } else {
                    // Force parse error
                    u8::from_str_radix("", 16)
                };

                if let Ok(magic) = magic.parse::<u8>() {
                    Some((magic, sn))
                } else if let Ok(hex_magic) = hex_magic {
                    Some((hex_magic, sn))
                } else if magic.to_uppercase() == "GPU" {
                    Some((SetGpuSerialMagic::WriteGPUConfig as u8, sn))
                } else if magic.to_uppercase() == "SSD" {
                    Some((SetGpuSerialMagic::WriteSSDConfig as u8, sn))
                } else {
                    println!(
                        "Invalid values for --flash_gpu_descriptor: '{} {}'. Must be u8, 18 character string.",
                        args[i + 1],
                        args[i + 2]
                    );
                    None
                }
            } else {
                println!("Need to provide a value for --flash_gpu_descriptor. TYPE_MAGIC SERIAL");
                None
            };
            found_an_option = true;
        } else if arg == "--flash-gpu-descriptor-file" {
            cli.flash_gpu_descriptor_file = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("Need to provide a value for --flash_gpu_descriptor_file. PATH");
                None
            };
            found_an_option = true;
        } else if arg == "--dump-gpu-descriptor-file" {
            cli.dump_gpu_descriptor_file = if args.len() > i + 1 {
                Some(args[i + 1].clone())
            } else {
                println!("Need to provide a value for --dump_gpu_descriptor_file. PATH");
                None
            };
            found_an_option = true;
        }
    }

    let custom_platform = cli.pd_addrs.is_some() && cli.pd_ports.is_some();
    let no_customization = cli.pd_addrs.is_none() && cli.pd_ports.is_none();
    if !(custom_platform || no_customization) {
        println!("To customize the platform you need to provide all of --pd-addrs, and --pd-ports");
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
