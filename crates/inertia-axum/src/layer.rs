//! Thin Tower layer for request setup and pending-response finalization.

use crate::axum::InertiaVersion;
use crate::{
    X_INERTIA_VERSION,
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
};
use axum::{
    extract::OriginalUri,
    http::{Method, Request, header::REFERER},
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
        Inner { #[pin] future: F, visit: Option<Visit>, shared: Option<crate::Props>, transient_seed: Option<crate::transient::TransientSeed>, engine: Engine },
        Finalizing { #[pin] future: std::pin::Pin<Box<dyn Future<Output = Response> + Send>>, error: std::marker::PhantomData<E> },
        Ready { response: Option<Result<Response, E>> },
    }
}

impl<F, E> Future for InertiaFuture<F, E>
where
    F: Future<Output = Result<Response, E>>,
{
    type Output = Result<Response, E>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.as_mut().project() {
                InertiaFutureProj::Ready { response } => {
                    return Poll::Ready(
                        response
                            .take()
                            .expect("ready Inertia future polled after completion"),
                    );
                }
                InertiaFutureProj::Finalizing { future, .. } => {
                    return future.poll(cx).map(Ok);
                }
                InertiaFutureProj::Inner {
                    future,
                    visit,
                    shared,
                    transient_seed,
                    engine,
                } => match future.poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                    Poll::Ready(Ok(mut response)) => {
                        #[cfg(feature = "ssr")]
                        let ssr_override = response.extensions_mut().remove::<crate::SsrOverride>();
                        let Some(handle) =
                            response.extensions_mut().remove::<PendingResponseHandle>()
                        else {
                            return Poll::Ready(Ok(response));
                        };
                        let Some(pending) = handle.take() else {
                            return Poll::Ready(Ok(response));
                        };
                        let visit = visit.take().expect("visit available while finalizing");
                        let shared = shared.take();
                        let transient_seed = transient_seed.take();
                        let engine = engine.clone();
                        self.set(InertiaFuture::Finalizing {
                            future: Box::pin(async move {
                                engine
                                    .finalize(
                                        &visit,
                                        pending,
                                        shared,
                                        transient_seed,
                                        #[cfg(feature = "ssr")]
                                        ssr_override,
                                    )
                                    .await
                            }),
                            error: std::marker::PhantomData,
                        });
                    }
                },
            }
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
        let transient_seed = self
            .app
            .inner
            .transient
            .as_ref()
            .map(|_| crate::transient::TransientSeed::capture(&request));
        let context = request_context(request.headers());
        let configured_version = self.app.inner.assets.header_version.clone();
        if request.method() == Method::GET && context.is_inertia() {
            if let Some(version) = configured_version.as_deref() {
                if header(request.headers(), X_INERTIA_VERSION) != Some(version) {
                    let response = conflict_response(original_local_uri(&request))
                        .unwrap_or_else(internal_error_response);
                    if let (Some(store), Some(seed)) =
                        (self.app.inner.transient.clone(), transient_seed.clone())
                    {
                        return InertiaFuture::Finalizing {
                            future: Box::pin(async move {
                                let mut response = response;
                                match store.load(seed.request()).await {
                                    Ok(mut data) => {
                                        data.reflash();
                                        if let Err(error) = store.commit(&mut response, data).await
                                        {
                                            return internal_error_response(
                                                crate::axum::InertiaError::transient(error),
                                            );
                                        }
                                        response
                                    }
                                    Err(error) => internal_error_response(
                                        crate::axum::InertiaError::transient(error),
                                    ),
                                }
                            }),
                            error: std::marker::PhantomData,
                        };
                    }
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
        let shared = if let Some(provider) = &self.app.inner.shared {
            match provider.prepare(crate::ShareContext::new(
                request.method(),
                request.uri(),
                request.headers(),
                request.extensions(),
                &visit,
            )) {
                Ok(shared) => Some(shared),
                Err(error) => {
                    return InertiaFuture::Ready {
                        response: Some(Ok(internal_error_response(
                            crate::axum::InertiaError::shared(error),
                        ))),
                    };
                }
            }
        } else {
            None
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
            shared,
            transient_seed,
            engine: self.engine.clone(),
        }
    }
}
