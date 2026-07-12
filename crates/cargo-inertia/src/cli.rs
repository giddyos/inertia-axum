//! Command-line parsing and Cargo-subcommand normalization.

use clap::{Parser, Subcommand};

use crate::error::CliError;

/// Parses and dispatches the optional project-tooling commands.
#[derive(Parser)]
#[command(
    name = "cargo inertia",
    version,
    about = "Optional inertia-axum project tooling"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Creates a minimal Vite frontend.
    #[cfg(feature = "init")]
    Init(crate::init::args::InitArgs),
    /// Runs Vite and cargo run together.
    #[cfg(feature = "dev")]
    Dev(crate::dev::args::DevArgs),
    /// Validates Rust component declarations and Vite files.
    #[cfg(feature = "check")]
    Check(crate::check::args::CheckArgs),
    /// Generates TypeScript contracts for typed Inertia props.
    #[cfg(feature = "sync")]
    Sync(crate::sync::args::SyncArgs),
}

/// Normalizes both `cargo inertia …` and `cargo-inertia …` invocations, then runs the command.
pub fn run() -> Result<(), CliError> {
    let mut arguments = std::env::args().collect::<Vec<_>>();
    if arguments
        .get(1)
        .is_some_and(|argument| argument == "inertia")
    {
        arguments.remove(1);
    }
    match Cli::parse_from(arguments).command {
        #[cfg(feature = "init")]
        Command::Init(args) => crate::init::run_args(args, &mut std::io::stdout().lock()),
        #[cfg(feature = "dev")]
        Command::Dev(args) => crate::dev::run_args(args),
        #[cfg(feature = "check")]
        Command::Check(args) => crate::check::run_args(args).map_err(Into::into),
        #[cfg(feature = "sync")]
        Command::Sync(args) => crate::sync::run_args(args).map_err(Into::into),
    }
}
