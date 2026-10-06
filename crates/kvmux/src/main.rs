#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use kvmux::cli;
use kvmux_core::DdcDisplays;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // The updater's helper probes new binaries with `--version`, expecting "<name> <version>".
    if args.iter().any(|arg| arg == "--version") {
        println!("kvmux {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    match args.first().map(String::as_str) {
        Some("monitors") => {
            println!("{}", cli::monitors_report(&mut DdcDisplays::new()));
            ExitCode::SUCCESS
        }
        Some("set-input") => match (args.get(1), args.get(2)) {
            (Some(monitor_id), Some(input)) => {
                match cli::set_input_report(&mut DdcDisplays::new(), monitor_id, input) {
                    Ok(line) => {
                        println!("{line}");
                        ExitCode::SUCCESS
                    }
                    Err(message) => {
                        eprintln!("kvmux: {message}");
                        ExitCode::FAILURE
                    }
                }
            }
            _ => {
                eprintln!("usage: kvmux set-input <monitor-id> <input>");
                ExitCode::from(2)
            }
        },
        _ => {
            // Until the tray exists there is no way to open settings later, so show them by default.
            let background = args.iter().any(|arg| arg == "--background");
            kvmux::app::run(!background);
            ExitCode::SUCCESS
        }
    }
}
