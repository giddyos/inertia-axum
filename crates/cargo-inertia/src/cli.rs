//! Command-line parsing and Cargo-subcommand normalization.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::{error::CliError, framework::Framework};

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
enum Command {
    /// Creates a minimal Vite frontend.
    #[cfg(feature = "init")]
    Init {
        #[arg(long)]
        frontend: Frontend,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Runs Vite and cargo run together.
    #[cfg(feature = "dev")]
    Dev {
        #[arg(long, default_value = "frontend")]
        frontend: PathBuf,
        #[arg(long, default_value_t = 5173)]
        port: u16,
    },
    /// Validates Rust component declarations and Vite files.
    #[cfg(feature = "check")]
    Check {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "frontend")]
        frontend: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Frontend {
    Svelte,
    React,
    Vue,
}

impl From<Frontend> for Framework {
    fn from(value: Frontend) -> Self {
        match value {
            Frontend::Svelte => Self::Svelte,
            Frontend::React => Self::React,
            Frontend::Vue => Self::Vue,
        }
    }
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
        Command::Init { frontend, path } => {
            crate::init::run(&path, frontend.into()).map_err(Into::into)
        }
        #[cfg(feature = "dev")]
        Command::Dev { frontend, port } => crate::dev::run(&frontend, port).map_err(Into::into),
        #[cfg(feature = "check")]
        Command::Check { path, frontend } => {
            crate::check::run(&path, &frontend).map_err(Into::into)
        }
    }
}
