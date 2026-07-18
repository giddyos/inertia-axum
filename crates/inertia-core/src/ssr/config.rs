use std::{
    path::{Path, PathBuf},
    time::Duration,
};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:13714";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_CONTROL_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_MAX_CONCURRENCY: usize = 16;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SsrDefault {
    Enabled,
    OptIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FailureMode {
    Fallback,
    Strict,
}

#[derive(Clone, Debug)]
pub(crate) enum ProductionBackend {
    ManagedNode {
        bundle: PathBuf,
        runtime: PathBuf,
        endpoint: String,
    },
    External {
        endpoint: String,
        bundle: Option<PathBuf>,
        check_bundle: bool,
    },
}

/// Application-wide server-side rendering configuration.
#[derive(Clone, Debug)]
pub struct Ssr {
    pub(crate) production: ProductionBackend,
    pub(crate) default: SsrDefault,
    pub(crate) failure_mode: FailureMode,
    pub(crate) timeout: Duration,
    pub(crate) startup_timeout: Duration,
    pub(crate) control_timeout: Duration,
    pub(crate) max_concurrency: usize,
    pub(crate) max_response_bytes: usize,
}

impl Ssr {
    /// Launches and supervises the official Inertia Node SSR bundle.
    pub fn node(bundle: impl Into<PathBuf>) -> Self {
        Self {
            production: ProductionBackend::ManagedNode {
                bundle: bundle.into(),
                runtime: PathBuf::from("node"),
                endpoint: DEFAULT_ENDPOINT.to_owned(),
            },
            default: SsrDefault::Enabled,
            failure_mode: FailureMode::Fallback,
            timeout: DEFAULT_TIMEOUT,
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
            control_timeout: DEFAULT_CONTROL_TIMEOUT,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    /// Connects to an externally supervised Inertia SSR server.
    pub fn external(endpoint: impl Into<String>) -> Self {
        Self {
            production: ProductionBackend::External {
                endpoint: endpoint.into(),
                bundle: None,
                check_bundle: false,
            },
            default: SsrDefault::Enabled,
            failure_mode: FailureMode::Fallback,
            timeout: DEFAULT_TIMEOUT,
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
            control_timeout: DEFAULT_CONTROL_TIMEOUT,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
        }
    }

    /// Changes the application to route-level SSR opt-in.
    pub fn opt_in(mut self) -> Self {
        self.default = SsrDefault::OptIn;
        self
    }
    /// Returns an error response instead of falling back to CSR.
    pub fn strict(mut self) -> Self {
        self.failure_mode = FailureMode::Strict;
        self
    }
    /// Overrides the Node executable used by managed mode.
    pub fn runtime(mut self, runtime: impl Into<PathBuf>) -> Self {
        if let ProductionBackend::ManagedNode {
            runtime: configured,
            ..
        } = &mut self.production
        {
            *configured = runtime.into();
        }
        self
    }
    /// Overrides the standard SSR server endpoint.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        if let ProductionBackend::ManagedNode {
            endpoint: configured,
            ..
        } = &mut self.production
        {
            *configured = endpoint.into();
        }
        self
    }
    /// Verifies that an external server's bundle exists locally.
    pub fn bundle(mut self, bundle: impl Into<PathBuf>) -> Self {
        if let ProductionBackend::External {
            bundle: configured,
            check_bundle,
            ..
        } = &mut self.production
        {
            *configured = Some(bundle.into());
            *check_bundle = true;
        }
        self
    }
    /// Disables local bundle verification for an external SSR server.
    pub fn skip_bundle_check(mut self) -> Self {
        if let ProductionBackend::External { check_bundle, .. } = &mut self.production {
            *check_bundle = false;
        }
        self
    }
    /// Overrides the per-render timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    /// Overrides the startup timeout.
    pub fn startup_timeout(mut self, timeout: Duration) -> Self {
        self.startup_timeout = timeout;
        self
    }
    /// Overrides the timeout for health and shutdown requests.
    pub fn control_timeout(mut self, timeout: Duration) -> Self {
        self.control_timeout = timeout;
        self
    }
    /// Overrides the maximum number of concurrent renders.
    pub fn max_concurrency(mut self, maximum: usize) -> Self {
        self.max_concurrency = maximum;
        self
    }
    /// Overrides the maximum response size.
    pub fn max_response_bytes(mut self, maximum: usize) -> Self {
        self.max_response_bytes = maximum;
        self
    }

    pub(crate) fn resolve_bundle(bundle: &Path, vite_root: Option<&Path>) -> PathBuf {
        if bundle.is_absolute() {
            return bundle.to_owned();
        }
        vite_root
            .map(|root| root.join(bundle))
            .unwrap_or_else(|| bundle.to_owned())
    }
}

impl From<PathBuf> for Ssr {
    fn from(bundle: PathBuf) -> Self {
        Self::node(bundle)
    }
}
impl From<String> for Ssr {
    fn from(bundle: String) -> Self {
        Self::node(bundle)
    }
}
impl From<&str> for Ssr {
    fn from(bundle: &str) -> Self {
        Self::node(bundle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_ssr_is_enabled_by_default() {
        let config = Ssr::from("dist/ssr/ssr.js");
        assert_eq!(config.default, SsrDefault::Enabled);
        assert_eq!(config.failure_mode, FailureMode::Fallback);
    }

    #[test]
    fn opt_in_mode_must_be_explicit() {
        assert_eq!(
            Ssr::node("dist/ssr/ssr.js").opt_in().default,
            SsrDefault::OptIn
        );
    }
}
