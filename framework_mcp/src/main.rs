//! MCP server for Framework Computer hardware
//!
//! Exposes the read-only information framework_tool can show (battery,
//! charger, PD ports, temperatures, fans, sensors, EC state, ...) as tools
//! for AI assistants that speak the Model Context Protocol. Talks to the
//! client over stdio, so stdout must stay clean: everything else goes to
//! the log on stderr.

mod server;

use clap::Parser;
use framework_lib::chromium_ec::{CrosEc, CrosEcDriverType};
use rmcp::transport::stdio;
use rmcp::ServiceExt;

use crate::server::FrameworkServer;

/// MCP server for Framework Computer hardware information
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Force a specific driver to talk to the EC
    #[arg(long, value_enum)]
    driver: Option<CrosEcDriverType>,

    /// Start even if not running as root or the EC does not respond
    ///
    /// By default the server refuses to start in that case, so the MCP
    /// client shows a clear error instead of every tool failing.
    #[arg(long)]
    skip_checks: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // stdout is the MCP transport, all logging goes to stderr
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_target(false)
        .format_timestamp(None)
        .init();

    let args = Args::parse();

    #[cfg(unix)]
    if !args.skip_checks && !nix::unistd::Uid::effective().is_root() {
        eprintln!("framework_mcp must run as root to access the Embedded Controller.");
        eprintln!("Configure your MCP client to start it with sudo.");
        std::process::exit(1);
    }

    let ec = match args.driver {
        Some(driver) => match CrosEc::with(driver) {
            Some(ec) => ec,
            None => {
                eprintln!("EC driver {:?} is not available on this system.", driver);
                std::process::exit(1);
            }
        },
        None => CrosEc::new(),
    };

    if !args.skip_checks {
        if let Err(err) = ec.version_info() {
            eprintln!("Cannot talk to the Embedded Controller: {:?}", err);
            eprintln!("Is this a Framework Computer system? Pass --skip-checks to start anyway.");
            std::process::exit(1);
        }
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let service = FrameworkServer::new(ec).serve(stdio()).await?;
            log::info!("framework_mcp connected");
            service.waiting().await?;
            Ok(())
        })
}
