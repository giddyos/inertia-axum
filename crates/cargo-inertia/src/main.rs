//! Binary entry point for the optional `cargo inertia` subcommand.

fn main() {
    if let Err(error) = cargo_inertia::cli::run() {
        eprintln!("cargo inertia: {error}");
        std::process::exit(1);
    }
}
