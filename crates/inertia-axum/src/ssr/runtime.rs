use super::{
    FailureMode, ProductionBackend, Ssr, SsrBackendKind, SsrClient, SsrDefault, SsrEndpoints,
    SsrFailure, SsrHealth, SsrOverride, SsrResponse, SsrStartError,
};
use crate::assets::AssetRuntime;

#[derive(Clone)]
pub(crate) struct SsrRuntime {
    client: SsrClient,
    default: SsrDefault,
    failure_mode: FailureMode,
    backend: SsrBackendKind,
    health: tokio::sync::watch::Receiver<SsrHealth>,
}

impl SsrRuntime {
    fn ready(client: SsrClient, config: Ssr, backend: SsrBackendKind) -> Self {
        let (_, health) = tokio::sync::watch::channel(SsrHealth::Ready { backend });
        Self {
            client,
            default: config.default,
            failure_mode: config.failure_mode,
            backend,
            health,
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
        self.client.render(page).await
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
                std::fs::metadata(&resolved).map_err(|source| {
                    SsrStartError::BundleUnavailable {
                        path: resolved,
                        source,
                    }
                })?;
            }
            let client = SsrClient::new(
                SsrEndpoints::node(&endpoint)?,
                config.timeout,
                config.max_concurrency,
                config.max_response_bytes,
            )?;
            check_health_until_ready(&client, config.startup_timeout).await?;
            Ok(SsrRuntime::ready(client, config, SsrBackendKind::External))
        }
        ProductionBackend::ManagedNode { .. } => Err(SsrStartError::ManagedNodeNotImplemented),
    }
}

async fn check_health_until_ready(
    client: &SsrClient,
    timeout: std::time::Duration,
) -> Result<(), SsrStartError> {
    tokio::time::timeout(timeout, async {
        loop {
            if client.health().await.is_ok() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| SsrStartError::HealthTimeout { timeout })
}
