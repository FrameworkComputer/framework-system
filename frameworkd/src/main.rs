// In debug builds don't disable terminal
// Otherwise we can't see println and can't exit with CTRL+C
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::mem::MaybeUninit;
use std::collections::HashMap;
use std::process::Command;
use std::thread;
use std::time::Duration;

use brightness::blocking::Brightness;
use qmk_hid::via;
use trayicon::*;
use winapi::um::winuser;
use winrt_notification::Toast;

#[repr(u16)]
enum FrameworkPid {
    Macropad = 0x0012,
    IsoKeyboard = 0x0018,
}

const LOGO_32_ICO: &[u8] = include_bytes!("../res/logo_cropped_transparent_32x32.ico");
const VIA_URL: &str = "https://usevia.app";
const FWK_MARKETPLACE: &str = "https://frame.work/marketplace";
const FWK_COMMUNITY: &str = "https://community.frame.work";
const FWK_KB: &str = "https://knowledgebase.frame.work/";
const FWK_GUIDES: &str = "https://guides.frame.work/c/Root";

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Events {
    ClickTrayIcon,
    DoubleClickTrayIcon,
    Exit,
    LaunchVia,
    LaunchQmkGui,
    LaunchLedmatrixControl,
    LaunchMarketplace,
    LaunchCommunity,
    LaunchKb,
    LaunchGuides,
    // SyncKeyboards,
    SyncKeyboardScreen,
    NumLockOn,
    NumLockOff,
    NumLockToggle,
}

enum Tool {
    QmkGui,
    LedmatrixControl,
}

struct PrevValues {
    brightness: HashMap<std::ffi::CString, (u8, u8)>,
    numlock: bool,
}

impl Default for PrevValues {
    fn default() -> Self {
        PrevValues {
            brightness: HashMap::new(),
            numlock: true,
        }
    }
}

fn to_wstring(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn launch_website(url: &str) {
    println!("Launch: {:?}", url);
    use std::ptr::null_mut;
    use winapi::um::shellapi::*;
    use winapi::um::winuser::*;
    let wurl = to_wstring(url).as_ptr();
    let action = to_wstring("open").as_ptr();
    let ret = unsafe {
        ShellExecuteW(
            null_mut(),
            action,
            wurl,
            null_mut(),
            null_mut(),
            SW_SHOWNORMAL,
        )
    };
    // TODO: Sometimes it fails with 0x1F, which I think is SE_ERR_NOASSOC
    // Not sure why, maybe just retry?
    println!(
        "Launch: done with {:?}, success: {:?}",
        ret,
        (ret as u8) > 32
    );
}

fn launch_tool(t: Tool) {
    let path = match t {
        Tool::QmkGui => r"C:\Program Files\Framework Computer\qmk_gui.exe",
        Tool::LedmatrixControl => r"C:\Program Files\Framework Computer\ledmatrix_control.exe",
    };
    Command::new(path).spawn().unwrap();
}

// Returns percentage if brightness changed
fn sync_keyboards(prev_brightness: &mut HashMap<std::ffi::CString, (u8, u8)>) -> Option<u32> {
    match qmk_hid::new_hidapi() {
        Ok(api) => {
            let found = qmk_hid::find_devices(&api, false, false, Some("32ac"), None);

            let dev_infos = found.raw_usages;

            if dev_infos.len() <= 1 {
                // No need to sync
                return None;
            }

            for dev_info in &dev_infos {
                let device = dev_info.open_device(&api).unwrap();
                let white_brightness =
                    via::get_backlight(&device, via::ViaBacklightValue::Brightness as u8).unwrap();
                let rgb_brightness =
                    via::get_rgb_u8(&device, via::ViaRgbMatrixValue::Brightness as u8).unwrap();
                //println!("{:?}", dev_info.product_string());
                //println!("  RGB: {}/255 White: {}/255", rgb_brightness, white_brightness);

                let mut br_changed = false;
                let mut rgb_br_changed = false;

                let path = dev_info.path();
                if let Some((prev_white, prev_rgb)) = prev_brightness.get(path) {
                    if white_brightness != *prev_white {
                        // println!("White changed from {} to {}", prev_white, white_brightness);
                        br_changed = true
                    }
                    if rgb_brightness != *prev_rgb {
                        // println!("RGB changed from {} to {}", prev_rgb, rgb_brightness);
                        rgb_br_changed = true
                    }
                }
                prev_brightness.insert(path.into(), (white_brightness, rgb_brightness));

                if br_changed || rgb_br_changed {
                    // Update other keyboards
                    let new_brightness = if br_changed {
                        white_brightness
                    } else {
                        rgb_brightness
                    };
                    // println!("Updating based on {:?}", dev_info.product_string());
                    // println!("  Updating other keyboards to {}", new_brightness);
                    for other_info in &dev_infos {
                        if path == other_info.path() {
                            continue;
                        }
                        // println!("  Updating {:?}", other_info.product_string());
                        {
                            let other_device = other_info.open_device(&api).unwrap();
                            via::set_backlight(
                                &other_device,
                                via::ViaBacklightValue::Brightness as u8,
                                new_brightness,
                            )
                            .unwrap();
                            via::set_rgb_u8(
                                &other_device,
                                via::ViaRgbMatrixValue::Brightness as u8,
                                new_brightness,
                            )
                            .unwrap();
                        }

                        // Avoid triggering an update in the other direction
                        // Need to read the value since the keyboard might only have 3 brightness levels,
                        // if that's the case, the value we set and the value the keyboard sets itself to are not the same.
                        // Seems we have to sleep a bit and also connect to the device again to make the change visible.
                        // TODO: Figure out why QMK changes the value also on the RGB which should have all 255 levels
                        thread::sleep(Duration::from_millis(100));
                        {
                            let other_device = other_info.open_device(&api).unwrap();
                            let actual_white = via::get_backlight(
                                &other_device,
                                via::ViaBacklightValue::Brightness as u8,
                            )
                            .unwrap();
                            let actual_rgb = via::get_rgb_u8(
                                &other_device,
                                via::ViaRgbMatrixValue::Brightness as u8,
                            )
                            .unwrap();
                            // println!("  Actually set to white: {}, rgb: {}", actual_white, actual_rgb);
                            prev_brightness
                                .insert(other_info.path().into(), (actual_white, actual_rgb));
                        }
                    }
                    return Some((new_brightness as u32) * 100 / 255);
                }
            }
            //println!();
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
    None
}

fn sync_keyboard_screen() {
    println!("Sync");
    match qmk_hid::new_hidapi() {
        Ok(api) => {
            let found = qmk_hid::find_devices(&api, false, false, Some("32ac"), None);

            let dev_infos = found.raw_usages;

            let dev_info = if dev_infos.is_empty() {
                println!("No device found");
                return;
            } else if dev_infos.len() == 1 {
                //println!("Found one device");
                dev_infos.get(0).unwrap()
            } else {
                println!("More than 1 device found. Select a specific device with --vid and --pid");
                dev_infos.get(0).unwrap()
            };
            //println!("Open");
            let device = dev_info.open_device(&api).unwrap();
            //println!("Opened");

            //println!("Get RGB");
            let rgb_brightness =
                via::get_rgb_u8(&device, via::ViaRgbMatrixValue::Brightness as u8).unwrap();
            //println!("Get white");
            let white_brightness =
                via::get_backlight(&device, via::ViaBacklightValue::Brightness as u8).unwrap();
            // println!("RGB: {}/255 White: {}/255", rgb_brightness, brightness);

            // TODO: In firmware it should sync both brightnesses
            let pid = dev_info.product_id();
            let brightness = if pid == FrameworkPid::IsoKeyboard as u16 {
                white_brightness
            // } else if pid == FrameworkPid::Macropad as u16 {
            //     white_brightness
            } else if pid == FrameworkPid::Macropad as u16 {
                rgb_brightness
            } else {
                white_brightness
            };

            let percent = (brightness as u32) * 100 / 255;
            println!("Brightness: {}/255, {}%", brightness, percent);

            let devs = brightness::blocking::brightness_devices();
            for dev in devs {
                let dev = dev.unwrap();
                dev.set(percent).unwrap();
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    };
}

fn add_menu(menu: MenuBuilder<Events>, _icon: &'static [u8], nm: Events) -> MenuBuilder<Events> {
    let (nm_str, nm_checked) = match nm {
        Events::NumLockOff => ("Numlock Off (Arrow Keys)", false),
        Events::NumLockOn => ("Numlock On (Number Keys)", true),
        _ => ("???", false),
    };
    menu.submenu(
        "Launch Inputmodule Apps",
        MenuBuilder::new()
            .item("Keyboard (VIA)", Events::LaunchVia)
            .item("Keyboard (QMK GUI)", Events::LaunchQmkGui)
            .item("LED Matrix", Events::LaunchLedmatrixControl),
    )
    .separator()
    // TODO
    // .item("Sync keyboard brightness", Events::SyncKeyboards)
    .item(
        "Sync keyboard brightness with screen",
        Events::SyncKeyboardScreen,
    )
    //.separator()
    .checkable(nm_str, nm_checked, Events::NumLockToggle)
    //.with(MenuItem::Item {
    //    name: "Item Disabled".into(),
    //    disabled: true, // Disabled entry example
    //    id: Events::DisabledItem1,
    //    icon: Result::ok(Icon::from_buffer(icon, None, None)),
    //})
    .separator()
    .submenu(
        "Framework Websites",
        MenuBuilder::new()
            .item("Marketplace", Events::LaunchMarketplace)
            .item("Community", Events::LaunchCommunity)
            .item("Knowledge Base", Events::LaunchKb)
            .item("Guides", Events::LaunchGuides),
    )
    .separator()
    .item("E&xit", Events::Exit)
}

/// Check if numlock is enabled
///
/// Enabled means number mode, disabled means arrow mode
fn numlock_enabled() -> bool {
    use winapi::um::winuser::*;
    unsafe { GetKeyState(VK_NUMLOCK) == 1 }
}

fn numlock_toggle() {
    use winapi::um::winuser::*;
    unsafe {
        keybd_event(VK_NUMLOCK as u8, 0x3A, 0x1, 0);
        keybd_event(VK_NUMLOCK as u8, 0x3A, 0x3, 0);
    }
}

fn main() {
    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon = LOGO_32_ICO;
    // let icon2 = include_bytes!("icon2.ico");
    // let second_icon = Icon::from_buffer(icon2, None, None).unwrap();
    // let first_icon = Icon::from_buffer(icon, None, None).unwrap();

    // Needlessly complicated tray icon with all the whistles and bells
    let mut tray_icon = TrayIconBuilder::new()
        .sender(s.clone())
        .icon_from_buffer(icon)
        .tooltip("Cool Tray 👀 Icon")
        .on_click(Events::ClickTrayIcon)
        .on_double_click(Events::DoubleClickTrayIcon)
        .menu(add_menu(MenuBuilder::new(), icon, Events::NumLockOn))
        .build()
        .unwrap();

    let periodic_s = s;
    let mut prev_values = PrevValues::default();

    std::thread::spawn(move || loop {
        let numlock_state = numlock_enabled();
        if numlock_state != prev_values.numlock {
            let text = if numlock_state {
                periodic_s.send(Events::NumLockOn).unwrap();
                "Numlock enabled"
            } else {
                periodic_s.send(Events::NumLockOff).unwrap();
                "Numlock disabled"
            };
            // TODO: Figure out our own Application User Model ID
            Toast::new(Toast::POWERSHELL_APP_ID)
                .title("Framework Keyboard")
                .text1(text)
                .sound(None)
                .duration(winrt_notification::Duration::Short)
                .show()
                .expect("unable to toast");
        }
        prev_values.numlock = numlock_state;

        //sync_keyboard_screen();
        if let Some(percentage) = sync_keyboards(&mut prev_values.brightness) {
            let devs = brightness::blocking::brightness_devices();
            for dev in devs {
                let dev = dev.unwrap();
                dev.set(percentage).unwrap();
            }
        }

        thread::sleep(Duration::from_secs(1));
    });

    std::thread::spawn(move || {
        r.iter().for_each(|m| match m {
            // About Popup
            // FontAwesome Icon
            // Events::About => {},
            Events::LaunchVia => launch_website(VIA_URL),
            Events::LaunchQmkGui => launch_tool(Tool::QmkGui),
            Events::LaunchLedmatrixControl => launch_tool(Tool::LedmatrixControl),
            Events::LaunchMarketplace => launch_website(FWK_MARKETPLACE),
            Events::LaunchCommunity => launch_website(FWK_COMMUNITY),
            Events::LaunchKb => launch_website(FWK_KB),
            Events::LaunchGuides => launch_website(FWK_GUIDES),
            //Events::SyncKeyboards => sync_keyboards(),
            Events::SyncKeyboardScreen => sync_keyboard_screen(),

            Events::DoubleClickTrayIcon => {
                println!("Double click");
                let devs = brightness::blocking::brightness_devices();
                for dev in devs {
                    let dev = dev.unwrap();
                    dev.set(50).unwrap();
                }
            }
            Events::ClickTrayIcon => {
                println!("Single click");
                let devs = brightness::blocking::brightness_devices();
                for dev in devs {
                    // TODO: Skip unsupported monitors
                    let dev = dev.unwrap();
                    println!("{:?}", dev.device_name());
                    println!("  {:?}", dev.get());
                }
                sync_keyboard_screen();
            }
            Events::Exit => {
                std::process::exit(0);
            }
            Events::NumLockToggle => {
                numlock_toggle();
            }
            Events::NumLockOn => {
                //println!("Turning numlock on");
                tray_icon
                    .set_menu(&add_menu(MenuBuilder::new(), icon, Events::NumLockOn))
                    .unwrap();
            }
            Events::NumLockOff => {
                //println!("Turning numlock off");
                tray_icon
                    .set_menu(&add_menu(MenuBuilder::new(), icon, Events::NumLockOff))
                    .unwrap();
            } // Events::Item1 => {
              //     tray_icon.set_icon(&second_icon).unwrap();
              // }
              // Events::Item2 => {
              //     tray_icon.set_icon(&first_icon).unwrap();
              // }
              // e => {
              //     println!("{:?}", e);
              // }
        })
    });

    // Your applications message loop. Because all applications require an
    // application loop, you are best served using an `winit` crate.
    loop {
        unsafe {
            let mut msg = MaybeUninit::uninit();
            let bret = winuser::GetMessageA(msg.as_mut_ptr(), 0 as _, 0, 0);
            if bret > 0 {
                winuser::TranslateMessage(msg.as_ptr());
                winuser::DispatchMessageA(msg.as_ptr());
            } else {
                break;
            }
        }
    }
}
