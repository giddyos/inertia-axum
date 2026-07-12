//! Arguments for `cargo inertia init`.

use std::path::PathBuf;

use clap::Args;

use crate::{
    framework::Framework,
    package_manager::PackageManagerChoice,
    ssr::{SsrFailureMode, SsrMode, SsrPolicy},
};

/// Creates a Vite frontend for an `inertia-axum` application.
#[derive(Args, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct InitArgs {
    /// Frontend framework; `--frontend` remains a compatibility alias.
    #[arg(long, visible_alias = "frontend")]
    pub framework: Option<Framework>,
    /// Package manager to use or automatically detect.
    #[arg(long, default_value = "auto")]
    pub package_manager: PackageManagerChoice,
    /// SSR mode to configure.
    #[arg(long, default_value = "none")]
    pub ssr: SsrMode,
    /// SSR route policy.
    #[arg(long, default_value = "enabled")]
    pub ssr_policy: SsrPolicy,
    /// SSR request failure policy.
    #[arg(long, default_value = "fallback")]
    pub ssr_failure: SsrFailureMode,
    /// External SSR endpoint.
    #[arg(long)]
    pub ssr_endpoint: Option<String>,
    /// Verify an externally managed SSR bundle.
    #[arg(long)]
    pub ssr_check_bundle: bool,
    /// Application root.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    /// Destination directory relative to `path`.
    #[arg(long, default_value = "frontend")]
    pub frontend_dir: PathBuf,
    /// Install frontend dependencies after generation.
    #[arg(long, conflicts_with = "no_install")]
    pub install: bool,
    /// Do not install frontend dependencies.
    #[arg(long)]
    pub no_install: bool,
    /// Accept non-interactive defaults.
    #[arg(long)]
    pub yes: bool,
    /// Render and validate without writing files.
    #[arg(long)]
    pub dry_run: bool,
}
