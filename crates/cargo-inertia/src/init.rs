//! Deterministic initialization orchestration.

pub mod answers;
pub mod args;
#[cfg(feature = "interactive")]
pub mod interactive;
pub mod options;
pub mod plan;
pub mod render;
pub mod validate;
pub mod write;

use std::{
    io::{self, IsTerminal, Write},
    path::Path,
};

use crate::{
    error::CliError,
    framework::Framework,
    package_manager::PackageManager,
    ssr::{SsrBackend, SsrMode, SsrOptions},
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

/// Resolves CLI arguments, optionally prompts, then generates and optionally installs.
pub fn run_args(args: args::InitArgs, output: &mut impl Write) -> Result<(), CliError> {
    let can_prompt = !args.yes && io::stdin().is_terminal() && io::stderr().is_terminal();
    #[cfg(feature = "interactive")]
    let answers = if can_prompt {
        Some(interactive::collect()?)
    } else {
        None
    };
    #[cfg(not(feature = "interactive"))]
    let answers: Option<answers::InitAnswers> = {
        let _ = can_prompt;
        None
    };
    let framework = args
        .framework
        .or_else(|| answers.as_ref().map(|answer| answer.framework))
        .ok_or_else(|| {
            CliError::InvalidOptions(
                "--framework is required when prompts are unavailable".to_owned(),
            )
        })?;
    let root = args.path;
    let destination = if args.frontend_dir.is_absolute() {
        args.frontend_dir
    } else {
        root.join(args.frontend_dir)
    };
    let package_manager = if let Some(answer) = answers.as_ref() {
        answer.package_manager
    } else {
        crate::package_manager::detect(args.package_manager, &destination, &root, None)
            .map_err(|error| CliError::InvalidOptions(error.to_string()))?
    };
    let ssr = if let Some(answer) = answers.as_ref() {
        answer.ssr.clone()
    } else {
        ssr_from_args(
            args.ssr,
            args.ssr_policy,
            args.ssr_failure,
            args.ssr_endpoint,
            args.ssr_check_bundle,
        )?
    };
    let install = if args.install {
        true
    } else if args.no_install {
        false
    } else {
        answers.as_ref().is_some_and(|answer| answer.install)
    };
    let options = options::InitOptions {
        root,
        frontend_dir: destination,
        framework,
        package_manager,
        install,
        ssr,
    };
    let plan = render::render(&options)?;
    validate::validate(&plan, options.framework)?;
    if args.dry_run {
        writeln!(
            output,
            "Dry run\nDestination: {}\nFiles:\n{}",
            plan.destination.display(),
            plan.files
                .iter()
                .map(|file| format!("  {}", file.relative_path.display()))
                .collect::<Vec<_>>()
                .join("\n")
        )?;
        return Ok(());
    }
    write::commit(&plan)?;
    if options.install {
        let spec = options
            .package_manager
            .install_command(options.frontend_dir.clone());
        let status = std::process::Command::new(&spec.program)
            .args(&spec.args)
            .current_dir(&spec.current_dir)
            .status()?;
        if !status.success() {
            return Err(CliError::Message(format!(
                "Frontend files were created, but {} install failed.\n\nRetry:\n  cd {}\n  {} install",
                options.package_manager.executable(),
                options.frontend_dir.display(),
                options.package_manager.executable()
            )));
        }
    }
    writeln!(
        output,
        "{}",
        crate::output::completion(
            options.framework,
            &options.frontend_dir,
            options.package_manager,
            options.install,
            &options.ssr
        )
    )?;
    Ok(())
}

fn ssr_from_args(
    mode: SsrMode,
    policy: crate::ssr::SsrPolicy,
    failure_mode: crate::ssr::SsrFailureMode,
    endpoint: Option<String>,
    check_bundle: bool,
) -> Result<SsrOptions, CliError> {
    let mut ssr = SsrOptions::disabled();
    ssr.policy = policy;
    ssr.failure_mode = failure_mode;
    ssr.backend = match mode {
        SsrMode::None => SsrBackend::None,
        SsrMode::ManagedNode => SsrBackend::ManagedNode,
        SsrMode::External => SsrBackend::External {
            endpoint: endpoint.ok_or_else(|| {
                CliError::InvalidOptions("--ssr-endpoint is required for external SSR".to_owned())
            })?,
            check_bundle,
        },
    };
    Ok(ssr)
}
