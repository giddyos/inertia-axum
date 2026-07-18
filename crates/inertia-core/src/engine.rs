//! Framework-neutral request preparation and response finalization.

use crate::{
    CoreBody, CoreResponse, InertiaApp, PendingPage, PendingResponse, RequestParts, Visit,
    X_INERTIA, X_INERTIA_HEADER, X_INERTIA_LOCATION_HEADER, X_INERTIA_REDIRECT_HEADER,
    html::html_response_context,
    page::{Page, PageDraft, PageMetadata},
    request::{EffectiveRequest, SelectionPlan},
    root::{HeadMarkup, MountMarkup, RootContext},
    shared::{ensure_errors_prop, insert_shared_prop_path, prop_root},
};
use bytes::Bytes;
use fluent_uri::{ParseError, UriRef};
use futures_util::{StreamExt, stream};
use http::{
    Extensions, HeaderValue, Method, StatusCode,
    header::InvalidHeaderValue,
    header::{CONTENT_TYPE, LOCATION, VARY},
};
use serde::Serialize;
use serde_json::{Map, Value};
use std::{error::Error, fmt};
#[cfg(feature = "ssr")]
use tracing::Instrument as _;

/// Framework-neutral rendering and application error.
#[derive(Debug)]
pub enum CoreError {
    /// The page object could not be serialized.
    Serialization(serde_json::Error),
    /// A response header value could not be constructed.
    InvalidHeader(InvalidHeaderValue),
    /// A redirect or location URL was not a valid URI reference.
    InvalidUri(ParseError),
    /// The application root view could not be rendered.
    Root(Box<dyn Error + Send + Sync>),
    /// An asynchronous prop resolver failed.
    Prop(crate::PropError),
    /// Typed shared-data preparation failed.
    Shared(Box<dyn Error + Send + Sync>),
    /// Flash or redirected errors were used without a transient store.
    MissingTransientStore,
    /// Transient storage failed.
    Transient(Box<dyn Error + Send + Sync>),
    /// Server-side rendering failed in strict mode.
    #[cfg(feature = "ssr")]
    Ssr(crate::ssr::SsrFailure),
}

impl CoreError {
    fn invalid_header(error: InvalidHeaderValue) -> Self {
        Self::InvalidHeader(error)
    }

    fn invalid_uri(error: ParseError) -> Self {
        Self::InvalidUri(error)
    }

    fn root(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::Root(error)
    }

    fn prop(error: crate::PropError) -> Self {
        Self::Prop(error)
    }

    fn shared(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::Shared(error)
    }

    fn transient(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::Transient(error)
    }

    #[cfg(feature = "ssr")]
    fn ssr(error: crate::ssr::SsrFailure) -> Self {
        Self::Ssr(error)
    }

    /// Converts the error to the stable protocol-facing 500 response.
    pub fn into_response(self) -> CoreResponse {
        tracing::error!(error = %self, "failed to build Inertia response");
        let body = if matches!(self, Self::MissingTransientStore) {
            self.to_string()
        } else {
            "failed to build Inertia response".to_owned()
        };
        let mut response = CoreResponse::bytes(StatusCode::INTERNAL_SERVER_ERROR, body);
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => write!(formatter, "failed to serialize Inertia page: {error}"),
            Self::InvalidHeader(error) => write!(formatter, "invalid Inertia response header: {error}"),
            Self::InvalidUri(error) => write!(formatter, "invalid Inertia URI reference: {error}"),
            Self::Root(error) => write!(formatter, "failed to render Inertia root view: {error}"),
            Self::Prop(error) => write!(formatter, "failed to resolve Inertia prop: {error}"),
            Self::Shared(error) => write!(formatter, "failed to prepare Inertia shared data: {error}"),
            Self::MissingTransientStore => formatter.write_str(
                "Inertia flash or redirected error state requires a transient store; configure InertiaAppBuilder::transient(...)",
            ),
            Self::Transient(error) => {
                write!(formatter, "failed to load or commit Inertia transient state: {error}")
            }
            #[cfg(feature = "ssr")]
            Self::Ssr(error) => write!(formatter, "server-side rendering failed: {error}"),
        }
    }
}

impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            Self::InvalidHeader(error) => Some(error),
            Self::InvalidUri(error) => Some(error),
            Self::Root(error) | Self::Shared(error) | Self::Transient(error) => {
                Some(error.as_ref())
            }
            Self::Prop(error) => Some(error),
            Self::MissingTransientStore => None,
            #[cfg(feature = "ssr")]
            Self::Ssr(error) => Some(error),
        }
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

/// Result of the pre-handler version check.
pub enum VersionCheck {
    /// Continue into the framework handler with this prepared request.
    Proceed(Box<PreparedRequest>),
    /// Return this response without invoking the framework handler.
    Mismatch(CoreResponse),
}

/// Core-owned request state retained by an adapter until response finalization.
pub struct PreparedRequest {
    engine: Engine,
    visit: Visit,
    shared: Option<crate::Props>,
    transient_seed: Option<crate::transient::TransientSeed>,
}

impl PreparedRequest {
    /// Returns the parsed visit exposed to framework extractors and guards.
    pub fn visit(&self) -> &Visit {
        &self.visit
    }

    /// Returns the configured deployment version for compatibility extractors.
    #[doc(hidden)]
    pub fn asset_version(&self) -> Option<&str> {
        self.engine.app.inner.assets.header_version.as_deref()
    }

    /// Finalizes a pending handler response.
    pub async fn finalize(self, pending: PendingResponse) -> CoreResponse {
        self.engine
            .finalize(
                &self.visit,
                pending,
                self.shared,
                self.transient_seed,
                #[cfg(feature = "ssr")]
                None,
            )
            .await
    }

    /// Finalizes a pending handler response with a route-level SSR override.
    #[cfg(feature = "ssr")]
    pub async fn finalize_with_ssr(
        self,
        pending: PendingResponse,
        ssr_override: Option<crate::ssr::SsrOverride>,
    ) -> CoreResponse {
        self.engine
            .finalize(
                &self.visit,
                pending,
                self.shared,
                self.transient_seed,
                ssr_override,
            )
            .await
    }
}

impl InertiaApp {
    /// Parses and prepares a framework-neutral request before its handler runs.
    pub async fn prepare_request(
        &self,
        request: RequestParts,
        extensions: Option<&Extensions>,
        #[cfg(feature = "tower-sessions")] session: Option<tower_sessions::Session>,
    ) -> Result<VersionCheck, CoreError> {
        let visit = Visit::from_request(&request);
        let transient_seed = self.inner.transient.as_ref().map(|_| {
            let seed = crate::transient::TransientSeed::capture(&request);
            #[cfg(feature = "tower-sessions")]
            let seed = seed.with_tower_session(session);
            seed
        });

        if request.method() == Method::GET && visit.is_inertia() {
            if let Some(version) = self.inner.assets.header_version.as_deref() {
                if visit.version() != Some(version) {
                    let mut response = conflict_response(&visit.uri)?;
                    if let (Some(store), Some(seed)) =
                        (&self.inner.transient, transient_seed.as_ref())
                    {
                        let mut data = store
                            .load(seed.request())
                            .await
                            .map_err(CoreError::transient)?;
                        data.reflash();
                        store
                            .commit(&mut response, data)
                            .await
                            .map_err(CoreError::transient)?;
                    }
                    return Ok(VersionCheck::Mismatch(response));
                }
            }
        }

        let shared = if let Some(provider) = &self.inner.shared {
            let empty_extensions = Extensions::new();
            let extensions = extensions.unwrap_or(&empty_extensions);
            Some(
                provider
                    .prepare(crate::ShareContext::new(
                        request.method(),
                        request.uri(),
                        request.headers(),
                        extensions,
                        &visit,
                    ))
                    .map_err(CoreError::shared)?,
            )
        } else {
            None
        };

        Ok(VersionCheck::Proceed(Box::new(PreparedRequest {
            engine: Engine::new(self.clone()),
            visit,
            shared,
            transient_seed,
        })))
    }

    /// Returns the filesystem mount retained for the Axum compatibility phase.
    #[cfg(feature = "vite")]
    #[doc(hidden)]
    pub fn __filesystem_mount(&self) -> Option<(String, std::path::PathBuf)> {
        self.inner.assets.filesystem_mount.clone()
    }
}

#[derive(Clone)]
struct Engine {
    app: InertiaApp,
}

impl Engine {
    fn new(app: InertiaApp) -> Self {
        Self { app }
    }

    async fn finalize(
        &self,
        visit: &Visit,
        pending: PendingResponse,
        shared: Option<crate::Props>,
        transient_seed: Option<crate::transient::TransientSeed>,
        #[cfg(feature = "ssr")] ssr_override: Option<crate::ssr::SsrOverride>,
    ) -> CoreResponse {
        if pending.requires_transient() && self.app.inner.transient.is_none() {
            return CoreError::MissingTransientStore.into_response();
        }
        let mut transient = if pending.uses_transient() {
            if let (Some(store), Some(seed)) = (&self.app.inner.transient, transient_seed.as_ref())
            {
                match store.load(seed.request()).await {
                    Ok(data) => Some(data),
                    Err(error) => return CoreError::transient(error).into_response(),
                }
            } else {
                None
            }
        } else {
            None
        };

        let result = match pending {
            PendingResponse::Page(page) => {
                self.finalize_page(
                    visit,
                    *page,
                    shared,
                    transient.as_mut(),
                    #[cfg(feature = "ssr")]
                    ssr_override,
                )
                .await
            }
            PendingResponse::Redirect(redirect) => {
                let destination = redirect.resolve(visit.referer.as_deref()).to_owned();
                if let Some(data) = transient.as_mut() {
                    data.reflash();
                    for (key, value) in redirect.flash {
                        data.flash_next_value(key, value);
                    }
                }
                let status = if is_write_method(&visit.method) {
                    StatusCode::SEE_OTHER
                } else {
                    StatusCode::FOUND
                };
                redirect_response(status, &destination)
            }
            PendingResponse::Location(location) => {
                if visit.is_inertia() {
                    conflict_response(location.url())
                } else {
                    let status = if is_write_method(&visit.method) {
                        StatusCode::SEE_OTHER
                    } else {
                        StatusCode::FOUND
                    };
                    redirect_response(status, location.url())
                }
            }
            PendingResponse::InvalidForm(validation) => {
                let data = transient
                    .as_mut()
                    .expect("validation requires configured transient state");
                data.reflash();
                data.store_errors(validation.errors);
                if let Some(old_input) = validation.old_input {
                    data.store_old_input(old_input);
                }
                redirect_response(StatusCode::SEE_OTHER, &validation.back)
            }
        };

        let mut response = result.unwrap_or_else(CoreError::into_response);
        if let (Some(store), Some(data)) = (&self.app.inner.transient, transient) {
            if let Err(error) = store.commit(&mut response, data).await {
                return CoreError::transient(error).into_response();
            }
        }
        response
    }

    async fn finalize_page(
        &self,
        visit: &Visit,
        pending: PendingPage,
        shared: Option<crate::Props>,
        transient: Option<&mut crate::TransientData>,
        #[cfg(feature = "ssr")] ssr_override: Option<crate::ssr::SsrOverride>,
    ) -> Result<CoreResponse, CoreError> {
        let PendingPage {
            component,
            props,
            encrypt_history,
            clear_history,
            preserve_fragment,
            flash,
            status,
        } = pending;
        let mut metadata = PageMetadata::new();
        if encrypt_history {
            metadata = metadata.encrypt_history();
        }
        if clear_history {
            metadata = metadata.clear_history();
        }
        if preserve_fragment {
            metadata = metadata.preserve_fragment();
        }
        let mut route_roots = Vec::new();
        for prop in &props {
            let root = prop_root(&prop.key).to_owned();
            if !route_roots.contains(&root) {
                route_roots.push(root);
            }
            prop.apply_metadata(&mut metadata, !prop.is_fresh_once());
        }
        let mut candidates = props
            .into_iter()
            .map(|prop| (prop, false))
            .collect::<Vec<_>>();
        if let Some(shared) = shared {
            for prop in shared.into_inner() {
                if route_roots.iter().any(|root| root == prop_root(&prop.key)) {
                    continue;
                }
                prop.apply_shared_metadata(&mut metadata);
                candidates.push((prop, true));
            }
        }
        let partial_enabled = visit.method == Method::GET;
        let selected = {
            let plan = SelectionPlan::new(
                EffectiveRequest::new(&visit.context, partial_enabled),
                &component,
                &metadata,
            );
            candidates
                .into_iter()
                .filter(|(prop, _)| plan.includes(&prop.key, prop.mode()))
                .collect::<Vec<_>>()
        };
        for (prop, _) in &selected {
            if prop.is_fresh_once() {
                prop.apply_metadata(&mut metadata, true);
            }
        }

        let mut resolved = Vec::with_capacity(selected.len());
        type IndexedResolution = std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = (usize, (String, crate::InertiaResult<Value>, bool), bool),
                    > + Send
                    + 'static,
            >,
        >;
        let mut asynchronous: Vec<IndexedResolution> = Vec::new();
        for (index, (prop, shared)) in selected.into_iter().enumerate() {
            resolved.push(None);
            match prop.into_resolution() {
                crate::props::PendingResolution::Ready(result) => {
                    resolved[index] = Some((result, shared));
                }
                crate::props::PendingResolution::Async(future) => {
                    asynchronous.push(Box::pin(async move { (index, future.await, shared) }));
                }
            }
        }
        for (index, result, shared) in stream::iter(asynchronous)
            .buffered(16)
            .collect::<Vec<_>>()
            .await
        {
            resolved[index] = Some((result, shared));
        }

        let mut values = Map::new();
        let mut shared_values = Vec::new();
        for ((key, result, rescue), is_shared) in resolved
            .into_iter()
            .map(|result| result.expect("selected prop resolved exactly once"))
        {
            match result {
                Ok(value) if is_shared => shared_values.push((key, value)),
                Ok(value) => {
                    values.insert(key, value);
                }
                Err(error) if rescue => {
                    if let Some(handler) = &self.app.inner.error_handler {
                        handler.handle(&key, &error);
                    } else {
                        tracing::error!(prop = %key, error = %error, "rescued Inertia prop resolver failure");
                    }
                    metadata = metadata.rescue(key);
                }
                Err(error) => return Err(CoreError::prop(error)),
            }
        }
        ensure_errors_prop(&mut values);
        for (key, value) in shared_values {
            if insert_shared_prop_path(&mut values, &key, value) {
                metadata = metadata.share(prop_root(&key));
            }
        }
        if let Some(data) = transient.as_ref() {
            if let Some(errors) = data.errors() {
                values.insert("errors".to_owned(), errors.clone());
            }
            if let Some(old_input) = data.old_input() {
                values.insert("oldInput".to_owned(), old_input.clone());
            }
        }
        metadata = metadata.into_response_metadata(&visit.context, &component, Some(&values));
        let mut page = Page::from_parts_version(
            component,
            Value::Object(values),
            visit.uri.to_string(),
            self.app.inner.assets.version.clone(),
            metadata,
        );
        let mut page_flash = transient
            .as_ref()
            .map_or_else(Map::new, |data| data.incoming_flash().clone());
        page_flash.extend(flash);
        page.set_flash(page_flash);
        let page = PageDraft::new(page, route_roots).finish();
        if visit.is_inertia() {
            return finalize_json_page(&page, status);
        }
        self.finalize_initial_page(
            visit,
            &page,
            status,
            #[cfg(feature = "ssr")]
            ssr_override,
        )
        .await
    }

    #[cfg_attr(not(feature = "ssr"), allow(clippy::unused_async))]
    async fn finalize_initial_page(
        &self,
        visit: &Visit,
        page: &Page<Value>,
        status: StatusCode,
        #[cfg(feature = "ssr")] route: Option<crate::ssr::SsrOverride>,
    ) -> Result<CoreResponse, CoreError> {
        #[cfg(not(feature = "ssr"))]
        let _ = visit;
        let serialized = html_response_context(page)?;
        let assets = &self.app.inner.assets.tags;

        #[cfg(feature = "ssr")]
        if should_render_ssr(&self.app, visit, route) {
            let runtime = self
                .app
                .inner
                .ssr
                .as_ref()
                .expect("eligible SSR runtime exists");
            let page_bytes = serialized.data_page_bytes();
            let started = std::time::Instant::now();
            let span = tracing::info_span!("inertia.ssr.render",
                component = page.component(), url = page.url(), backend = ?runtime.backend(),
                outcome = tracing::field::Empty, request_bytes = page_bytes.len(),
                response_bytes = tracing::field::Empty, duration_ms = tracing::field::Empty);
            match runtime.render(page_bytes).instrument(span.clone()).await {
                Ok(Some(rendered)) => {
                    runtime.record_success();
                    span.record("outcome", "rendered");
                    span.record("response_bytes", rendered.body.len());
                    span.record(
                        "duration_ms",
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                    let head = HeadMarkup::from_fragments(rendered.head);
                    let mount = MountMarkup::ssr(rendered.body);
                    return render_root(&self.app, assets, &head, &mount, status);
                }
                Ok(None) => {
                    span.record("outcome", "vite_warmup");
                    span.record(
                        "duration_ms",
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                }
                Err(error) if matches!(runtime.failure_mode(), crate::ssr::FailureMode::Strict) => {
                    runtime.record_failure(error.kind());
                    span.record("outcome", "strict_error");
                    span.record(
                        "duration_ms",
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                    return Err(CoreError::ssr(error));
                }
                Err(error) => {
                    runtime.record_failure(error.kind());
                    let outcome = match error.kind() {
                        crate::ssr::SsrFailureKind::Unavailable => "fallback_unavailable",
                        crate::ssr::SsrFailureKind::Overloaded => "fallback_overloaded",
                        crate::ssr::SsrFailureKind::Timeout => "fallback_timeout",
                        crate::ssr::SsrFailureKind::Transport => "fallback_transport",
                        crate::ssr::SsrFailureKind::InvalidResponse
                        | crate::ssr::SsrFailureKind::ResponseTooLarge => {
                            "fallback_invalid_response"
                        }
                        _ => "fallback_render",
                    };
                    span.record("outcome", outcome);
                    span.record(
                        "duration_ms",
                        u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    );
                    tracing::warn!(error = %error, kind = ?error.kind(), "SSR failed; falling back to CSR");
                }
            }
        }

        let head = HeadMarkup::empty();
        let mount = MountMarkup::csr(serialized.data_page());
        render_root(&self.app, assets, &head, &mount, status)
    }
}

#[cfg(feature = "ssr")]
fn should_render_ssr(
    app: &InertiaApp,
    visit: &Visit,
    route: Option<crate::ssr::SsrOverride>,
) -> bool {
    let Some(runtime) = app.inner.ssr.as_ref() else {
        return false;
    };
    !visit.is_inertia() && visit.method == Method::GET && runtime.is_enabled(route)
}

fn finalize_json_page<T: Serialize>(
    page: &T,
    status: StatusCode,
) -> Result<CoreResponse, CoreError> {
    let body = serde_json::to_vec(page)?;
    let mut response = CoreResponse::bytes(status, body);
    response
        .headers_mut()
        .insert(X_INERTIA_HEADER, HeaderValue::from_static("true"));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    add_vary_header(&mut response);
    Ok(response)
}

fn render_root(
    app: &InertiaApp,
    assets: &crate::RootAssetTags,
    head: &HeadMarkup,
    mount: &MountMarkup,
    status: StatusCode,
) -> Result<CoreResponse, CoreError> {
    let html = app
        .inner
        .root
        .render(RootContext::new(assets, head, mount))
        .map_err(CoreError::root)?;
    let mut response = CoreResponse::new(
        status,
        http::HeaderMap::new(),
        CoreBody::Bytes(Bytes::from(html)),
    );
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    add_vary_header(&mut response);
    Ok(response)
}

fn is_write_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn location_header_with_fragment(url: &str) -> Result<(HeaderValue, bool), CoreError> {
    let uri = UriRef::parse(url).map_err(CoreError::invalid_uri)?;
    let has_fragment = uri.fragment().is_some();
    HeaderValue::from_str(url)
        .map_err(CoreError::invalid_header)
        .map(|header| (header, has_fragment))
}

fn redirect_response(status: StatusCode, url: &str) -> Result<CoreResponse, CoreError> {
    let (location, _) = location_header_with_fragment(url)?;
    let mut response = CoreResponse::empty(status);
    response.headers_mut().insert(LOCATION, location);
    add_vary_header(&mut response);
    Ok(response)
}

fn conflict_response(url: &str) -> Result<CoreResponse, CoreError> {
    let (location, has_fragment) = location_header_with_fragment(url)?;
    let mut response = CoreResponse::empty(StatusCode::CONFLICT);
    let header = if has_fragment {
        X_INERTIA_REDIRECT_HEADER
    } else {
        X_INERTIA_LOCATION_HEADER
    };
    response.headers_mut().insert(header, location);
    add_vary_header(&mut response);
    Ok(response)
}

fn add_vary_header(response: &mut CoreResponse) {
    let has_inertia = response
        .headers()
        .get_all(VARY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|value| value.eq_ignore_ascii_case(X_INERTIA));

    if !has_inertia {
        response
            .headers_mut()
            .append(VARY, HeaderValue::from_static(X_INERTIA));
    }
}
