use framework_lib::commandline;

#[macro_use]
extern crate log;

use env_logger::Builder;

/// Get commandline arguments
fn get_args() -> Vec<String> {
    std::env::args().collect()
}

fn main() {
    // TOOD: Should probably have a custom env variable?
    // let env = Env::default()
    //     .filter("FRAMEWORK_COMPUTER_LOG")
    //     .write_style("FRAMEWORK_COMPUTER_LOG_STYLE");

    Builder::from_default_env()
        .format_target(false)
        .format_timestamp(None)
        .init();


    let args = commandline::parse(&get_args());
    trace!("a trace example");
    debug!("deboogging");
    info!("such information");
    warn!("o_O");
    error!("boom");
    commandline::run_with_args(&args, false);
}
