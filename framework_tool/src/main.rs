use framework_lib::commandline;

/// Get commandline arguments
fn get_args() -> Vec<String> {
    std::env::args().collect()
}

fn main() -> Result<(), &'static str> {
    let args = commandline::parse(&get_args());
    if (commandline::run_with_args(&args, false)) != 0 {
        return Err("Fail");
    }
    Ok(())
}
