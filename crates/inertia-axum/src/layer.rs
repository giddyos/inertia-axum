//! Axum middleware for core request preparation and pending-response finalization.

use crate::{
    axum::InertiaVersion,
    response::{AxumResponse, PendingResponseHandle},
};
use axum::{
    extract::OriginalUri,
    http::Request,
    response::{IntoResponse, Response},
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Installs an [`inertia_core::InertiaApp`] as Axum middleware.
#[derive(Clone)]
pub struct InertiaLayer {
    app: inertia_core::InertiaApp,
}

impl InertiaLayer {
    /// Creates a layer for `app`.
    pub fn new(app: inertia_core::InertiaApp) -> Self {
        Self { app }
    }
}

impl<S> Layer<S> for InertiaLayer {
    type Service = InertiaService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InertiaService {
            inner,
            app: self.app.clone(),
        }
    }
}

/// Service produced by [`InertiaLayer`].
#[derive(Clone)]
pub struct InertiaService<S> {
    inner: S,
    app: inertia_core::InertiaApp,
}

type ResponseFuture<E> = Pin<Box<dyn Future<Output = Result<Response, E>> + Send + 'static>>;

impl<S, B> Service<Request<B>> for InertiaService<S>
where
    S: Service<Request<B>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = ResponseFuture<S::Error>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let mut inner = self.inner.clone();
        let app = self.app.clone();
        Box::pin(async move {
            let uri = request
                .extensions()
                .get::<OriginalUri>()
                .map(|original| original.0.clone())
                .unwrap_or_else(|| request.uri().clone());
            let request_parts = inertia_core::RequestParts::new(
                request.method().clone(),
                uri,
                request.headers().clone(),
            );
            #[cfg(feature = "tower-sessions")]
            let session = request
                .extensions()
                .get::<tower_sessions::Session>()
                .cloned();
            let prepared = app
                .prepare_request(
                    request_parts,
                    Some(request.extensions()),
                    #[cfg(feature = "tower-sessions")]
                    session,
                )
                .await;
            let prepared = match prepared {
                Ok(inertia_core::VersionCheck::Proceed(prepared)) => *prepared,
                Ok(inertia_core::VersionCheck::Mismatch(response)) => {
                    return Ok(AxumResponse(response).into_response());
                }
                Err(error) => return Ok(AxumResponse(error.into_response()).into_response()),
            };

            request.extensions_mut().insert(prepared.visit().clone());
            if let Some(version) = prepared.asset_version() {
                request
                    .extensions_mut()
                    .insert(InertiaVersion::new(version));
            }

            let mut response = inner.call(request).await?;
            #[cfg(feature = "ssr")]
            let ssr_override = response.extensions_mut().remove::<crate::SsrOverride>();
            let Some(handle) = response.extensions_mut().remove::<PendingResponseHandle>() else {
                return Ok(response);
            };
            let Some(pending) = handle.take() else {
                return Ok(response);
            };
            #[cfg(feature = "ssr")]
            let response = prepared.finalize_with_ssr(pending, ssr_override).await;
            #[cfg(not(feature = "ssr"))]
            let response = prepared.finalize(pending).await;
            Ok(AxumResponse(response).into_response())
        })
    }
}
