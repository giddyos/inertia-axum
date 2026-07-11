use super::{
    FailureMode, ProductionBackend, Ssr, SsrBackendKind, SsrClient, SsrDefault, SsrEndpoints,
    SsrFailure, SsrHealth, SsrOverride, SsrResponse, SsrStartError,
};
use crate::assets::AssetRuntime;

#[derive(Clone)]
pub(crate) struct SsrRuntime {
    pub(crate) client: SsrClient,
    pub(crate) default: SsrDefault,
    pub(crate) failure_mode: FailureMode,
    pub(crate) backend: SsrBackendKind,
    pub(crate) health: tokio::sync::watch::Receiver<SsrHealth>,
    pub(crate) health_tx: tokio::sync::watch::Sender<SsrHealth>,
    pub(crate) lifecycle: Option<tokio::sync::watch::Sender<()>>,
}

impl SsrRuntime {
    fn ready(client: SsrClient, config: Ssr, backend: SsrBackendKind) -> Self {
        let (health_tx, health) = tokio::sync::watch::channel(SsrHealth::Ready { backend });
        Self {
            client,
            default: config.default,
            failure_mode: config.failure_mode,
            backend,
            health,
            health_tx,
            lifecycle: None,
        }
    }

    pub(crate) fn is_enabled(&self, route: Option<SsrOverride>) -> bool {
        match route {
            Some(SsrOverride::Enabled) => true,
            Some(SsrOverride::Disabled) => false,
            None => matches!(self.default, SsrDefault::Enabled),
        }
    }
    pub(crate) fn failure_mode(&self) -> FailureMode {
        self.failure_mode
    }
    pub(crate) fn backend(&self) -> SsrBackendKind {
        self.backend
    }
    pub(crate) fn health(&self) -> SsrHealth {
        self.health.borrow().clone()
    }
    pub(crate) async fn render(
        &self,
        page: bytes::Bytes,
    ) -> Result<Option<SsrResponse>, SsrFailure> {
        match self.health() {
            SsrHealth::Ready { .. } | SsrHealth::Degraded { .. } => self.client.render(page).await,
            _ => Err(SsrFailure::unavailable()),
        }
    }
    pub(crate) fn record_success(&self) {
        let _ = self.health_tx.send(SsrHealth::Ready {
            backend: self.backend,
        });
    }
    pub(crate) fn record_failure(&self, last_failure: crate::SsrFailureKind) {
        let _ = self.health_tx.send(SsrHealth::Degraded {
            backend: self.backend,
            last_failure,
        });
    }
}

pub(crate) async fn start_runtime(
    config: Ssr,
    assets: &AssetRuntime,
    vite_root: Option<&std::path::Path>,
) -> Result<SsrRuntime, SsrStartError> {
    #[cfg(feature = "vite")]
    if let Some(dev_server) = &assets.vite_dev_server {
        let client = SsrClient::new(
            SsrEndpoints::vite(dev_server)?,
            config.timeout,
            config.max_concurrency,
            config.max_response_bytes,
        )?;
        return Ok(SsrRuntime::ready(client, config, SsrBackendKind::Vite));
    }

    match config.production.clone() {
        ProductionBackend::External {
            endpoint,
            bundle,
            check_bundle,
        } => {
            if check_bundle {
                let resolved = Ssr::resolve_bundle(
                    bundle.as_deref().expect("checked bundle is configured"),
                    vite_root,
                );
                super::verify_bundle(&resolved)?;
            }
            let client = SsrClient::new(
                SsrEndpoints::node(&endpoint)?,
                config.timeout,
                config.max_concurrency,
                config.max_response_bytes,
            )?;
            super::check_health_until_ready(&client, config.startup_timeout).await?;
            Ok(SsrRuntime::ready(client, config, SsrBackendKind::External))
        }
        ProductionBackend::ManagedNode {
            bundle,
            runtime,
            endpoint,
        } => {
            let bundle = Ssr::resolve_bundle(&bundle, vite_root);
            let working_directory = vite_root
                .map(std::path::Path::to_owned)
                .or_else(|| bundle.parent().map(std::path::Path::to_owned))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            super::start_managed_node(config, bundle, runtime, endpoint, working_directory).await
        }
    }
}
