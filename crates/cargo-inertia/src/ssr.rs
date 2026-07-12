//! Shared server-side rendering configuration types.

use std::path::PathBuf;

/// Default Vite frontend entry.
pub const DEFAULT_ENTRY: &str = "src/main.ts";
/// Default client build directory.
pub const DEFAULT_CLIENT_OUT_DIR: &str = "dist";
/// Default Vite client manifest.
pub const DEFAULT_MANIFEST: &str = "dist/.vite/manifest.json";
/// Default Node SSR bundle.
pub const DEFAULT_SSR_BUNDLE: &str = "dist/ssr/main.js";

/// The rendering mode generated for a frontend project.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SsrMode {
    /// Do not configure SSR; render only in the browser.
    #[default]
    None,
    /// Run the generated Node SSR bundle through `inertia-axum`.
    ManagedNode,
    /// Connect `inertia-axum` to an independently managed SSR endpoint.
    External,
}

/// The selected production SSR backend.
#[cfg_attr(feature = "templates", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SsrBackend {
    /// Do not configure SSR.
    None,
    /// Run the generated SSR bundle with Node through `inertia-axum`.
    ManagedNode,
    /// Connect to a separately managed SSR HTTP endpoint.
    External {
        endpoint: String,
        check_bundle: bool,
    },
}

/// Whether all routes use SSR or individual routes opt in.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "templates", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SsrPolicy {
    Enabled,
    OptIn,
}

/// How SSR errors affect a request.
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[cfg_attr(feature = "templates", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SsrFailureMode {
    Fallback,
    Strict,
}

/// Fully resolved SSR settings used by source generation.
#[cfg_attr(feature = "templates", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsrOptions {
    /// Selected backend.
    pub backend: SsrBackend,
    /// Route policy.
    pub policy: SsrPolicy,
    /// Request failure behavior.
    pub failure_mode: SsrFailureMode,
    /// SSR bundle path relative to the frontend directory.
    pub bundle: PathBuf,
    /// Loopback bind host for an external endpoint.
    pub host: String,
    /// Loopback bind port for an external endpoint.
    pub port: u16,
}

impl SsrOptions {
    /// Creates explicitly disabled CSR settings.
    pub fn disabled() -> Self {
        Self {
            backend: SsrBackend::None,
            policy: SsrPolicy::Enabled,
            failure_mode: SsrFailureMode::Fallback,
            bundle: PathBuf::from(DEFAULT_SSR_BUNDLE),
            host: "127.0.0.1".to_owned(),
            port: 13_714,
        }
    }
    /// Whether an SSR backend is configured.
    pub const fn is_enabled(&self) -> bool {
        !matches!(self.backend, SsrBackend::None)
    }
}

/// Runs the managed-SSR Node preflight and reports a warning-worthy version.
#[cfg(feature = "package-managers")]
pub fn node_preflight() -> Result<NodePreflight, std::io::Error> {
    use std::process::Command;

    let executable = which::which("node").map_err(std::io::Error::other)?;
    let output = Command::new(&executable).arg("--version").output()?;
    Ok(NodePreflight {
        executable,
        version: NodeVersion::parse(&String::from_utf8_lossy(&output.stdout)),
    })
}

/// The result of checking Node for managed SSR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodePreflight {
    pub executable: PathBuf,
    pub version: Option<NodeVersion>,
}

impl NodePreflight {
    /// True when Node satisfies the Vite 8 and managed-SSR minimum of 22.12.0.
    pub const fn meets_managed_ssr_minimum(&self) -> bool {
        matches!(
            self.version,
            Some(NodeVersion {
                major: 22..,
                minor: 12..,
                ..
            })
        ) || matches!(self.version, Some(NodeVersion { major: 23.., .. }))
    }
}

/// A parsed Node semantic version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl NodeVersion {
    /// Parses `vMAJOR.MINOR.PATCH` output from `node --version`.
    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.trim().strip_prefix('v')?.split('.');
        Some(Self {
            major: parts.next()?.parse().ok()?,
            minor: parts.next()?.parse().ok()?,
            patch: parts.next()?.parse().ok()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_versions_and_enforces_the_managed_ssr_floor() {
        assert_eq!(
            NodeVersion::parse("v22.12.0\n"),
            Some(NodeVersion {
                major: 22,
                minor: 12,
                patch: 0
            })
        );
        let too_old = NodePreflight {
            executable: PathBuf::from("node"),
            version: NodeVersion::parse("v22.11.0"),
        };
        assert!(!too_old.meets_managed_ssr_minimum());
        let supported = NodePreflight {
            executable: PathBuf::from("node"),
            version: NodeVersion::parse("v22.12.0"),
        };
        assert!(supported.meets_managed_ssr_minimum());
    }
}
