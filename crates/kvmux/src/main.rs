#![allow(clippy::print_stdout)]

fn main() {
    // The updater's helper probes new binaries with `--version`, expecting "<name> <version>".
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!("kvmux {}", env!("CARGO_PKG_VERSION"));
    }
}
