#![windows_subsystem = "windows"]

use core::mem::MaybeUninit;

use brightness::blocking::Brightness;
use trayicon::*;
use winapi::um::winuser;

const LOGO_32_ICO: &[u8] = include_bytes!("../res/logo_cropped_transparent_32x32.ico");

fn main() {
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
    }

    let (s, r) = std::sync::mpsc::channel::<Events>();
    let icon = LOGO_32_ICO;
    let icon2 = include_bytes!("icon2.ico");

    let second_icon = Icon::from_buffer(icon2, None, None).unwrap();
    let first_icon = Icon::from_buffer(icon, None, None).unwrap();

    // Needlessly complicated tray icon with all the whistles and bells
    let mut tray_icon = TrayIconBuilder::new()
        .sender(move |e: &Events| {
            let _ = s.send(*e);
        })
        .icon_from_buffer(icon)
        .tooltip("Cool Tray 👀 Icon")
        .on_click(Events::ClickTrayIcon)
        .on_double_click(Events::DoubleClickTrayIcon)
        .menu(
            MenuBuilder::new()
                .item("Item 4 Set Tooltip", Events::Item4)
                .item("Item 3 Replace Menu 👍", Events::Item3)
                .item("Item 2 Change Icon Green", Events::Item2)
                .item("Item 1 Change Icon Red", Events::Item1)
                .separator()
                .checkable("This is checkable", true, Events::CheckItem1)
                .submenu(
                    "Sub Menu",
                    MenuBuilder::new()
                        .item("Sub item 1", Events::SubItem1)
                        .item("Sub Item 2", Events::SubItem2)
                        .item("Sub Item 3", Events::SubItem3),
                )
                .checkable("This checkbox toggles disable", true, Events::CheckItem1)
                .with(MenuItem::Item {
                    name: "Item Disabled".into(),
                    disabled: true, // Disabled entry example
                    id: Events::DisabledItem1,
                    icon: Result::ok(Icon::from_buffer(icon, None, None)),
                })
                .separator()
                .item("E&xit", Events::Exit),
        )
        .build()
        .unwrap();

    std::thread::spawn(move || {
        r.iter().for_each(|m| match m {
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
