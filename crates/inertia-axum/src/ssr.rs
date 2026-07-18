use axum::{
    Router,
    body::Body,
    http::{Extensions, HeaderMap, Method, Request, Uri},
    response::Response,
    routing::MethodRouter,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Read-only request information available to `ssr_when`.
#[derive(Clone, Copy)]
pub struct SsrContext<'a> {
    method: &'a Method,
    uri: &'a Uri,
    headers: &'a HeaderMap,
    extensions: &'a Extensions,
}

impl<'a> SsrContext<'a> {
    fn from_request(request: &'a Request<Body>) -> Self {
        Self {
            method: request.method(),
            uri: request.uri(),
            headers: request.headers(),
            extensions: request.extensions(),
        }
    }

    /// Returns the request method.
    pub fn method(&self) -> &Method {
        self.method
    }
    /// Returns the request URI, including its query string.
    pub fn uri(&self) -> &Uri {
        self.uri
    }
    /// Returns the request headers.
    pub fn headers(&self) -> &HeaderMap {
        self.headers
    }
    /// Returns the request extensions.
    pub fn extensions(&self) -> &Extensions {
        self.extensions
    }
    /// Returns a request extension of type `T`, when present.
    pub fn extension<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.extensions.get::<T>()
    }
}

pub use inertia_core::ssr::SsrOverride;

fn apply_override(decision: SsrOverride, response: &mut Response) {
    if response.extensions().get::<SsrOverride>().is_none() {
        response.extensions_mut().insert(decision);
    }
}

type SharedCondition = Arc<dyn for<'a> Fn(SsrContext<'a>) -> bool + Send + Sync>;

#[derive(Clone)]
enum SsrPolicy {
    Fixed(SsrOverride),
    Conditional(SharedCondition),
}

impl SsrPolicy {
    fn evaluate(&self, request: &Request<Body>) -> SsrOverride {
        match self {
            Self::Fixed(decision) => *decision,
            Self::Conditional(condition) => {
                if condition(SsrContext::from_request(request)) {
                    SsrOverride::Enabled
                } else {
                    SsrOverride::Disabled
                }
            }
        }
    }
}

#[derive(Clone)]
struct SsrPolicyLayer {
    policy: SsrPolicy,
}

impl SsrPolicyLayer {
    fn fixed(decision: SsrOverride) -> Self {
        Self {
            policy: SsrPolicy::Fixed(decision),
        }
    }
    fn conditional<F>(condition: F) -> Self
    where
        F: for<'a> Fn(SsrContext<'a>) -> bool + Send + Sync + 'static,
    {
        Self {
            policy: SsrPolicy::Conditional(Arc::new(condition)),
        }
    }
}

impl<S> Layer<S> for SsrPolicyLayer {
    type Service = SsrPolicyService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        SsrPolicyService {
            inner,
            policy: self.policy.clone(),
        }
    }
}

#[derive(Clone)]
struct SsrPolicyService<S> {
    inner: S,
    policy: SsrPolicy,
}

impl<S> Service<Request<Body>> for SsrPolicyService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response, S::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let decision = self.policy.evaluate(&request);
        let future = self.inner.call(request);
        Box::pin(async move {
            let mut response = future.await?;
            apply_override(decision, &mut response);
            Ok(response)
        })
    }
}

/// Axum route and router SSR policy methods.
pub trait SsrRouteExt: Sized {
    /// Explicitly enables SSR.
    fn with_ssr(self) -> Self;
    /// Explicitly disables SSR.
    fn without_ssr(self) -> Self;
    /// Enables SSR when `condition` returns true and disables it otherwise.
    fn ssr_when<F>(self, condition: F) -> Self
    where
        F: for<'a> Fn(SsrContext<'a>) -> bool + Send + Sync + 'static;
}

impl<S, E> SsrRouteExt for MethodRouter<S, E>
where
    S: Clone + Send + Sync + 'static,
    E: 'static,
{
    fn with_ssr(self) -> Self {
        self.layer(SsrPolicyLayer::fixed(SsrOverride::Enabled))
    }
    fn without_ssr(self) -> Self {
        self.layer(SsrPolicyLayer::fixed(SsrOverride::Disabled))
    }
    fn ssr_when<F>(self, condition: F) -> Self
    where
        F: for<'a> Fn(SsrContext<'a>) -> bool + Send + Sync + 'static,
    {
        self.layer(SsrPolicyLayer::conditional(condition))
    }
}

impl<S> SsrRouteExt for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_ssr(self) -> Self {
        self.layer(SsrPolicyLayer::fixed(SsrOverride::Enabled))
    }
    fn without_ssr(self) -> Self {
        self.layer(SsrPolicyLayer::fixed(SsrOverride::Disabled))
    }
    fn ssr_when<F>(self, condition: F) -> Self
    where
        F: for<'a> Fn(SsrContext<'a>) -> bool + Send + Sync + 'static,
    {
        self.layer(SsrPolicyLayer::conditional(condition))
    }
}
