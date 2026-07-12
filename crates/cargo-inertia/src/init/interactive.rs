//! Cliclack prompts, compiled only with the `interactive` feature.

use crate::{
    error::CliError,
    framework::Framework,
    init::answers::InitAnswers,
    package_manager::PackageManager,
    ssr::{SsrBackend, SsrFailureMode, SsrOptions, SsrPolicy},
};
use std::io;

/// Collects every answer before any filesystem action is taken.
pub fn collect() -> Result<InitAnswers, CliError> {
    let framework = match cliclack::select("Which frontend framework?")
        .item("react", "React", "")
        .item("svelte", "Svelte", "")
        .item("vue", "Vue", "")
        .initial_value("react")
        .interact()
        .map_err(prompt_error)?
    {
        "react" => Framework::React,
        "svelte" => Framework::Svelte,
        _ => Framework::Vue,
    };
    let package_manager = match cliclack::select("Which package manager?")
        .item("npm", "npm", "")
        .item("pnpm", "pnpm", "")
        .item("bun", "Bun", "")
        .initial_value("npm")
        .interact()
        .map_err(prompt_error)?
    {
        "pnpm" => PackageManager::Pnpm,
        "bun" => PackageManager::Bun,
        _ => PackageManager::Npm,
    };
    let mode = cliclack::select("How should initial page requests render?")
        .item("none", "Client-side rendering", "")
        .item("managed", "Managed Node SSR", "")
        .item("external", "External SSR service", "")
        .initial_value("none")
        .interact()
        .map_err(prompt_error)?;
    let mut ssr = SsrOptions::disabled();
    if mode != "none" {
        ssr.policy = match cliclack::select("Where should SSR apply?")
            .item("enabled", "Enabled by default", "")
            .item("opt-in", "Route-level opt-in", "")
            .initial_value("enabled")
            .interact()
            .map_err(prompt_error)?
        {
            "opt-in" => SsrPolicy::OptIn,
            _ => SsrPolicy::Enabled,
        };
        ssr.failure_mode = match cliclack::select("What should happen when SSR fails?")
            .item("fallback", "Fall back to client rendering", "")
            .item("strict", "Return an SSR error", "")
            .initial_value("fallback")
            .interact()
            .map_err(prompt_error)?
        {
            "strict" => SsrFailureMode::Strict,
            _ => SsrFailureMode::Fallback,
        };
        ssr.backend = if mode == "managed" {
            SsrBackend::ManagedNode
        } else {
            SsrBackend::External {
                endpoint: "http://127.0.0.1:13714".to_owned(),
                check_bundle: cliclack::confirm("Verify that the SSR bundle exists on this host?")
                    .initial_value(false)
                    .interact()
                    .map_err(prompt_error)?,
            }
        };
    }
    let install = cliclack::confirm("Install frontend dependencies now?")
        .initial_value(false)
        .interact()
        .map_err(prompt_error)?;
    if !cliclack::confirm("Create this frontend?")
        .initial_value(true)
        .interact()
        .map_err(prompt_error)?
    {
        return Err(CliError::Cancelled);
    }
    Ok(InitAnswers {
        framework,
        package_manager,
        ssr,
        install,
    })
}
fn prompt_error(error: io::Error) -> CliError {
    if error.kind() == io::ErrorKind::Interrupted {
        CliError::Cancelled
    } else {
        CliError::Io(error)
    }
}
