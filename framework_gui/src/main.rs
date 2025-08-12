use framework_lib::commandline;

/// Get commandline arguments
fn get_args() -> Vec<String> {
    std::env::args().collect()
}

fn main() -> Result<(), &'static str> {
    let args = get_args();

    // If the user double-clicks (opens from explorer/desktop),
    // then we want to have the default behavior of showing a report of
    // all firmware versions.
    #[cfg(windows)]
    let (args, double_clicked) = {
        let double_clicked = unsafe {
            // See https://devblogs.microsoft.com/oldnewthing/20160125-00/?p=92922
            let mut plist: winapi::shared::minwindef::DWORD = 0;
            let processes = winapi::um::wincon::GetConsoleProcessList(&mut plist, 1);

            // If we're the only process that means we're in a fresh terminal
            // without CMD or powershell. This happens in some cases, for example
            // if the user double-clicks the app from Explorer.
            processes == 1
        };
        // But it also happens if launched from the commandline and a UAC prompt is necessary,
        // for example with sudo set to open "In a new windows", therefore we also have to
        // check that no commandline arguments were provided.
        if double_clicked && args.len() == 1 {
            (
                vec![args[0].clone(), "--versions".to_string()],
                double_clicked,
            )
        } else {
            (args, double_clicked)
        }
    };

    let args = commandline::parse(&args);
    if (commandline::run_with_args(&args, false)) != 0 {
        return Err("Fail");
    }

    // Prevent command prompt from auto closing
    #[cfg(windows)]
    if double_clicked {
        println!();
        println!("Press ENTER to exit...");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line).unwrap();
    }

    Ok(())
}
