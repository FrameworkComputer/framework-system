use framework_lib::commandline;

// Get commandline arguments
fn get_args() -> Vec<String> {
    // TODO: Port to UEFI
    std::env::args().collect()
}

fn main() {
    commandline::run_with_args(&get_args());
}
