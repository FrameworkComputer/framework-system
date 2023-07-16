// In debug builds don't disable terminal
// Otherwise we can't see println and can't exit with CTRL+C
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use core::mem::MaybeUninit;
use std::process::Command;
use std::thread;
use std::time::Duration;

use brightness::blocking::Brightness;
use qmk_hid::via;
use trayicon::*;
use winapi::um::winuser;

#[repr(u16)]
enum FrameworkPid {
    Macropad = 0x0012,
    IsoKeyboard = 0x0018,
}

const LOGO_32_ICO: &[u8] = include_bytes!("../res/logo_cropped_transparent_32x32.ico");

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Events {
    ClickTrayIcon,
    DoubleClickTrayIcon,
    Exit,
    Item1,
    Item2,
    Item3,
    Item4,
    DisabledItem1,
    CheckItem1,
    SubItem1,
    SubItem2,
    SubItem3,
    LaunchQmkGui,
    LaunchLedmatrixControl,
    SyncKeyboards,
    SyncKeyboardScreen,
    NumLockOn,
    NumLockOff,
    NumLockToggle,
}

enum Tool {
    QmkGui,
    LedmatrixControl,
}

fn launch_tool(t: Tool) {
    let path = match t {
        Tool::QmkGui => r"C:\Program Files\Framework Computer\qmk_gui.exe",
        Tool::LedmatrixControl => r"C:\Program Files\Framework Computer\ledmatrix_control.exe",
    };
    Command::new(path).spawn().unwrap();
}

fn sync_keyboards() {}

fn sync_keyboard_screen() {
    println!("Sync");
    match qmk_hid::new_hidapi() {
        Ok(api) => {
            let found = qmk_hid::find_devices(&api, true, false, Some("32ac"), None);

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

fn add_menu(menu: MenuBuilder<Events>, icon: &'static [u8], nm: Events) -> MenuBuilder<Events> {
    let nm_str = match nm {
        Events::NumLockOff => "Numlock Off (Arrow Keys)",
        Events::NumLockOn => "Numlock On (Number Keys)",
        _ => "???",
    };
    menu.submenu(
        "Launch Inputmodule Apps",
        MenuBuilder::new()
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
    //.item("Item 4 Set Tooltip", Events::Item4)
    //.item("Item 3 Replace Menu 👍", Events::Item3)
    //.item("Item 2 Change Icon Green", Events::Item2)
    //.item("Item 1 Change Icon Red", Events::Item1)
    //.separator()
    //.checkable("This is checkable", true, Events::CheckItem1)
    //.submenu(
    //    "Sub Menu",
    //    MenuBuilder::new()
    //        .item("Sub item 1", Events::SubItem1)
    //        .item("Sub Item 2", Events::SubItem2)
    //        .item("Sub Item 3", Events::SubItem3),
    //)
    //.checkable("This checkbox toggles disable", true, Events::CheckItem1)
    .with(MenuItem::Item {
        name: nm_str.into(),
        disabled: false,
        id: Events::NumLockToggle,
        icon: Result::ok(Icon::from_buffer(icon, None, None)),
    })
    //.with(MenuItem::Item {
    //    name: "Item Disabled".into(),
    //    disabled: true, // Disabled entry example
    //    id: Events::DisabledItem1,
    //    icon: Result::ok(Icon::from_buffer(icon, None, None)),
    //})
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
    let icon2 = include_bytes!("icon2.ico");

    let second_icon = Icon::from_buffer(icon2, None, None).unwrap();
    let first_icon = Icon::from_buffer(icon, None, None).unwrap();

    let periodic_s = s.clone();

    // Needlessly complicated tray icon with all the whistles and bells
    let mut tray_icon = TrayIconBuilder::new()
        .sender(move |e: &Events| {
            let _ = s.send(*e);
        })
        .icon_from_buffer(icon)
        .tooltip("Cool Tray 👀 Icon")
        .on_click(Events::ClickTrayIcon)
        .on_double_click(Events::DoubleClickTrayIcon)
        .menu(add_menu(MenuBuilder::new(), icon, Events::NumLockOn))
        .build()
        .unwrap();

    std::thread::spawn(move || loop {
        // TODO: Check if it changed and if not, don't send events
        if numlock_enabled() {
            periodic_s.send(Events::NumLockOn).unwrap();
        } else {
            periodic_s.send(Events::NumLockOff).unwrap();
        }

        thread::sleep(Duration::from_secs(1));
    });

    std::thread::spawn(move || {
        r.iter().for_each(|m| match m {
            // About Popup
            // FontAwesome Icon
            // Events::About => {},
            Events::LaunchQmkGui => launch_tool(Tool::QmkGui),
            Events::LaunchLedmatrixControl => launch_tool(Tool::LedmatrixControl),
            Events::SyncKeyboards => sync_keyboards(),
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
            Events::Item1 => {
                tray_icon.set_icon(&second_icon).unwrap();
            }
            Events::Item2 => {
                tray_icon.set_icon(&first_icon).unwrap();
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
            }
            Events::Item3 => {
                tray_icon
                    .set_menu(
                        &MenuBuilder::new()
                            .item("New menu item", Events::Item1)
                            .item("Exit", Events::Exit),
                    )
                    .unwrap();
            }
            Events::CheckItem1 => {
                // You can mutate single checked, disabled value followingly.
                //
                // However, I think better way is to use reactively
                // `set_menu` by building the menu based on application
                // state.
                if let Some(old_value) = tray_icon.get_menu_item_checkable(Events::CheckItem1) {
                    // Set checkable example
                    let _ = tray_icon.set_menu_item_checkable(Events::CheckItem1, !old_value);

                    // Set disabled example
                    let _ = tray_icon.set_menu_item_disabled(Events::DisabledItem1, !old_value);
                }
            }
            e => {
                println!("{:?}", e);
            }
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
