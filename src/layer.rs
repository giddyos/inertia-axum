//! Thin Tower layer for request setup and pending-response finalization.

use crate::axum::InertiaVersion;
use crate::{
    app::InertiaApp,
    axum::{
        error::internal_error_response,
        response_headers::{
            conflict_response, header, local_uri, original_local_uri, request_context,
        },
    },
    engine::Engine,
    response::PendingResponseHandle,
    visit::Visit,
    X_INERTIA_VERSION,
};
use axum::{
    extract::OriginalUri,
    http::{header::REFERER, Method, Request},
    response::Response,
};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Installs an [`InertiaApp`] as request middleware.
#[derive(Clone)]
pub struct InertiaLayer {
    app: InertiaApp,
}

impl InertiaLayer {
    /// Creates a layer for `app`.
    pub fn new(app: InertiaApp) -> Self {
        Self { app }
    }
}

impl<S> Layer<S> for InertiaLayer {
    type Service = InertiaService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        InertiaService {
            inner,
            engine: Engine::new(self.app.clone()),
            app: self.app.clone(),
        }
    }
}

/// Service produced by [`InertiaLayer`].
#[derive(Clone)]
pub struct InertiaService<S> {
    inner: S,
    engine: Engine,
    app: InertiaApp,
}

pin_project! {
    /// Concrete future used by [`InertiaService`].
    #[project = InertiaFutureProj]
    pub enum InertiaFuture<F, E> {
        Inner { #[pin] future: F, visit: Option<Visit>, engine: Engine },
        Ready { response: Option<Result<Response, E>> },
    }
}

impl<F, E> Future for InertiaFuture<F, E>
where
    F: Future<Output = Result<Response, E>>,
{
    type Output = Result<Response, E>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            InertiaFutureProj::Ready { response } => Poll::Ready(
                response
                    .take()
                    .expect("ready Inertia future polled after completion"),
            ),
            InertiaFutureProj::Inner {
                future,
                visit,
                engine,
            } => match future.poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
                Poll::Ready(Ok(mut response)) => {
                    let Some(handle) = response.extensions_mut().remove::<PendingResponseHandle>()
                    else {
                        return Poll::Ready(Ok(response));
                    };
                    let Some(pending) = handle.take() else {
                        return Poll::Ready(Ok(response));
                    };
                    Poll::Ready(Ok(engine.finalize(
                        visit.as_ref().expect("visit available while finalizing"),
                        pending,
                    )))
                }
            },
        }
    }
}

impl<S, B> Service<Request<B>> for InertiaService<S>
where
    S: Service<Request<B>, Response = Response>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = InertiaFuture<S::Future, S::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
        let context = request_context(request.headers());
        let configured_version = self.app.inner.assets.header_version.clone();
        if request.method() == Method::GET && context.is_inertia() {
            if let Some(version) = configured_version.as_deref() {
                if header(request.headers(), X_INERTIA_VERSION) != Some(version) {
                    let response = conflict_response(original_local_uri(&request))
                        .unwrap_or_else(internal_error_response);
                    return InertiaFuture::Ready {
                        response: Some(Ok(response)),
                    };
                }
            }
        }
        let uri = request
            .extensions()
            .get::<OriginalUri>()
            .map(|original| local_uri(&original.0))
            .unwrap_or_else(|| local_uri(request.uri()));
        let visit = Visit {
            context,
            method: request.method().clone(),
            uri,
            referer: request
                .headers()
                .get(REFERER)
                .and_then(|value| value.to_str().ok())
                .map(Into::into),
        };
        request.extensions_mut().insert(visit.clone());
        if let Some(version) = configured_version {
            request
                .extensions_mut()
                .insert(InertiaVersion::new(version));
        }
        InertiaFuture::Inner {
            future: self.inner.call(request),
            visit: Some(visit),
            engine: self.engine.clone(),
        }
    }
}
