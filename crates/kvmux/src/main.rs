#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use kvmux::cli;
use kvmux::update::UPDATES;
use kvmux_core::{DdcDisplays, NusbUsb};

fn main() -> ExitCode {
    // First, before anything else: when started as the update helper this installs the update
    // and exits. Otherwise it only strips the update flags from the command line.
    let launch = fastframe_update::intercept(&UPDATES);
    if let Some(message) = &launch.error {
        eprintln!("kvmux: {message}");
    }
    let args: Vec<String> = launch
        .arguments
        .iter()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();

    // The updater's helper probes new binaries with `--version`, expecting "<name> <version>".
    if args.iter().any(|arg| arg == "--version") {
        println!("kvmux {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    let _logging_guard = kvmux::logging::init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        command = args.first().map_or("app", String::as_str),
        "Starting kvmux"
    );
    let exit = run(args, launch.receipt);
    tracing::info!(?exit, "kvmux stopped");
    exit
}

fn run(args: Vec<String>, receipt: Option<fastframe_update::Receipt>) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("monitors") => {
            println!("{}", cli::monitors_report(&mut DdcDisplays::new()));
            ExitCode::SUCCESS
        }
        Some("usb") => usb_command(args.get(1).map(String::as_str) == Some("--watch")),
        Some("set-input") => match (args.get(1), args.get(2)) {
            (Some(monitor_id), Some(input)) => {
                match cli::set_input_report(&mut DdcDisplays::new(), monitor_id, input) {
                    Ok(line) => {
                        println!("{line}");
                        ExitCode::SUCCESS
                    }
                    Err(message) => {
                        tracing::error!(%message, "Input switch failed");
                        eprintln!("kvmux: {message}");
                        ExitCode::FAILURE
                    }
                }
            }
            _ => {
                tracing::warn!("Missing set-input arguments");
                eprintln!("usage: kvmux set-input <monitor-id> <input>");
                ExitCode::from(2)
            }
        },
        _ => {
            let background = args.iter().any(|arg| arg == "--background");
            kvmux::app::run(kvmux::app::Options {
                show_settings: !background,
                relaunch_arguments: args,
                receipt,
            })
        }
    }
}

fn usb_command(watch: bool) -> ExitCode {
    let mut usb = match NusbUsb::new() {
        Ok(usb) => usb,
        Err(error) => {
            tracing::error!(%error, "Could not initialize USB monitoring");
            eprintln!("kvmux: {error}");
            return ExitCode::FAILURE;
        }
    };
    match cli::usb_report(&mut usb) {
        Ok(report) => println!("{report}"),
        Err(message) => {
            tracing::error!(%message, "Could not enumerate USB devices");
            eprintln!("kvmux: {message}");
            return ExitCode::FAILURE;
        }
    }
    if watch {
        println!("\nWatching for devices, press Ctrl-C to stop.");
        let ended = cli::watch_usb(&mut usb, |line| println!("{line}"));
        eprintln!("kvmux: {ended}");
        tracing::error!(error = %ended, "USB watch ended");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
