use framework_lib::commandline;

/// Get commandline arguments
fn get_args() -> Vec<String> {
    std::env::args().collect()
}

fn main() {
    let args = commandline::parse(&get_args());
    commandline::run_with_args(&args, false);
}
