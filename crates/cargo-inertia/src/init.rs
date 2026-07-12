//! Deterministic initialization orchestration.

pub mod answers;
pub mod args;
pub mod options;
pub mod plan;
pub mod render;
pub mod validate;
pub mod write;

use std::{
    io::{self, Write},
    path::Path,
};

use crate::{
    error::CliError, framework::Framework, package_manager::PackageManager, ssr::SsrOptions,
};

/// Runs compatibility initialization with the historical fixed destination.
pub fn run(root: &Path, framework: Framework) -> Result<(), String> {
    let options = options::InitOptions {
        root: root.to_path_buf(),
        frontend_dir: root.join("frontend"),
        framework,
        package_manager: PackageManager::Pnpm,
        install: false,
        ssr: SsrOptions::disabled(),
    };
    run_options(&options, &mut io::stdout().lock()).map_err(|error| error.to_string())
}

/// Renders, validates, and atomically commits a resolved initialization request.
pub fn run_options(
    options: &options::InitOptions,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let plan = render::render(options)?;
    validate::validate(&plan, options.framework)?;
    write::commit(&plan)?;
    writeln!(
        output,
        "Created {} frontend in {}",
        match options.framework {
            Framework::React => "react",
            Framework::Svelte => "svelte",
            Framework::Vue => "vue",
        },
        plan.destination.display()
    )?;
    writeln!(
        output,
        "\nNext steps:\n  {} run install\n  {} run build\n  cargo run",
        options.package_manager.executable(),
        options.package_manager.executable()
    )?;
    Ok(())
}
