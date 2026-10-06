#![allow(clippy::print_stdout)]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // The updater's helper probes new binaries with `--version`, expecting "<name> <version>".
    if args.iter().any(|arg| arg == "--version") {
        println!("kvmux {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Until the tray exists there is no way to open settings later, so show them by default.
    let background = args.iter().any(|arg| arg == "--background");
    kvmux::app::run(!background);
}
