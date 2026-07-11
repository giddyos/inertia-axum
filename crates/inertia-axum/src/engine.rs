//! Request-aware finalization without extractor or Tower concerns.

use crate::{
    X_INERTIA_HEADER,
    app::InertiaApp,
    axum::response_headers::{
        add_vary_header, conflict_response, is_write_method, redirect_response,
    },
    html::html_response_context,
    response::{PendingPage, PendingResponse},
    root::{HeadMarkup, MountMarkup, RootContext},
    visit::Visit,
};
use crate::{
    page::{Page, PageDraft, PageMetadata},
    request::{EffectiveRequest, SelectionPlan},
    shared::{ensure_errors_prop, insert_shared_prop_path, prop_root},
};
use axum::{
    Json,
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use futures_util::{StreamExt, stream};
use serde::Serialize;
use serde_json::{Map, Value};
#[cfg(feature = "ssr")]
use tracing::Instrument as _;

#[derive(Clone)]
pub(crate) struct Engine {
    app: InertiaApp,
}

impl Engine {
    pub(crate) fn new(app: InertiaApp) -> Self {
        Self { app }
    }

    pub(crate) async fn finalize(
        &self,
        visit: &Visit,
        pending: PendingResponse,
        shared: Option<crate::Props>,
        transient_seed: Option<crate::transient::TransientSeed>,
        #[cfg(feature = "ssr")] ssr_override: Option<crate::SsrOverride>,
    ) -> Response {
        if pending.requires_transient() && self.app.inner.transient.is_none() {
            return crate::axum::InertiaError::MissingTransientStore.into_response();
        }
        let mut transient = if pending.uses_transient() {
            if let (Some(store), Some(seed)) = (&self.app.inner.transient, transient_seed.as_ref())
            {
                match store.load(seed.request()).await {
                    Ok(data) => Some(data),
                    Err(error) => {
                        return crate::axum::InertiaError::transient(error).into_response();
                    }
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
        let mut response = result.unwrap_or_else(crate::axum::error::internal_error_response);
        if let (Some(store), Some(data)) = (&self.app.inner.transient, transient) {
            if let Err(error) = store.commit(&mut response, data).await {
                return crate::axum::InertiaError::transient(error).into_response();
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
        #[cfg(feature = "ssr")] _ssr_override: Option<crate::SsrOverride>,
    ) -> Result<Response, crate::axum::InertiaError> {
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
        let partial_enabled = visit.method == axum::http::Method::GET;
        let selected = {
            let plan = SelectionPlan::new(
                EffectiveRequest::new(&visit.context, partial_enabled),
                &component,
                &metadata,
            );
            let mut selected = Vec::new();
            for (prop, shared) in candidates {
                if plan.includes(&prop.key, prop.mode()) {
                    selected.push((prop, shared));
                }
            }
            selected
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
                    resolved[index] = Some((result, shared))
                }
                crate::props::PendingResolution::Async(future) => {
                    asynchronous.push(Box::pin(async move { (index, future.await, shared) }))
                }
            }
        }
        let asynchronous = stream::iter(asynchronous)
            .buffered(16)
            .collect::<Vec<_>>()
            .await;
        for (index, result, shared) in asynchronous {
            resolved[index] = Some((result, shared));
        }
        let mut values = Map::new();
        let mut shared_values = Vec::new();
        for ((key, result, rescue), is_shared) in resolved
            .into_iter()
            .map(|result| result.expect("selected prop resolved exactly once"))
        {
            match result {
                Ok(value) => {
                    if is_shared {
                        shared_values.push((key, value));
                    } else {
                        values.insert(key, value);
                    }
                }
                Err(error) if rescue => {
                    if let Some(handler) = &self.app.inner.error_handler {
                        handler.handle(&key, &error);
                    } else {
                        tracing::error!(prop = %key, error = %error, "rescued Inertia prop resolver failure");
                    }
                    metadata = metadata.rescue(key);
                }
                Err(error) => return Err(crate::axum::InertiaError::prop(error)),
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
            return Ok(finalize_json_page(page, status));
        }
        self.finalize_initial_page(
            visit,
            page,
            status,
            #[cfg(feature = "ssr")]
            _ssr_override,
        )
        .await
    }

    async fn finalize_initial_page(
        &self,
        visit: &Visit,
        page: Page<Value>,
        status: StatusCode,
        #[cfg(feature = "ssr")] route: Option<crate::SsrOverride>,
    ) -> Result<Response, crate::axum::InertiaError> {
        let serialized = html_response_context(&page)?;
        let assets = self.app.inner.assets.tags.clone();

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
                    span.record("duration_ms", started.elapsed().as_millis() as u64);
                    let head = HeadMarkup::from_fragments(rendered.head);
                    let mount = MountMarkup::ssr(rendered.body);
                    return render_root(&self.app, &assets, &head, &mount, status);
                }
                Ok(None) => {
                    span.record("outcome", "vite_warmup");
                    span.record("duration_ms", started.elapsed().as_millis() as u64);
                }
                Err(error) if matches!(runtime.failure_mode(), crate::ssr::FailureMode::Strict) => {
                    runtime.record_failure(error.kind());
                    span.record("outcome", "strict_error");
                    span.record("duration_ms", started.elapsed().as_millis() as u64);
                    return Err(crate::axum::InertiaError::ssr(error));
                }
                Err(error) => {
                    runtime.record_failure(error.kind());
                    let outcome = match error.kind() {
                        crate::SsrFailureKind::Unavailable => "fallback_unavailable",
                        crate::SsrFailureKind::Overloaded => "fallback_overloaded",
                        crate::SsrFailureKind::Timeout => "fallback_timeout",
                        crate::SsrFailureKind::Transport => "fallback_transport",
                        crate::SsrFailureKind::InvalidResponse
                        | crate::SsrFailureKind::ResponseTooLarge => "fallback_invalid_response",
                        _ => "fallback_render",
                    };
                    span.record("outcome", outcome);
                    span.record("duration_ms", started.elapsed().as_millis() as u64);
                    tracing::warn!(error = %error, kind = ?error.kind(), "SSR failed; falling back to CSR")
                }
            }
        }

        let head = HeadMarkup::empty();
        let mount = MountMarkup::csr(serialized.data_page());
        render_root(&self.app, &assets, &head, &mount, status)
    }
}

#[cfg(feature = "ssr")]
fn should_render_ssr(app: &InertiaApp, visit: &Visit, route: Option<crate::SsrOverride>) -> bool {
    let Some(runtime) = app.inner.ssr.as_ref() else {
        return false;
    };
    !visit.is_inertia() && visit.method == axum::http::Method::GET && runtime.is_enabled(route)
}

fn finalize_json_page<T: Serialize>(page: T, status: StatusCode) -> Response {
    let mut response = Json(page).into_response();
    response
        .headers_mut()
        .insert(X_INERTIA_HEADER, HeaderValue::from_static("true"));
    *response.status_mut() = status;
    add_vary_header(&mut response);
    response
}

fn render_root(
    app: &InertiaApp,
    assets: &crate::AssetTags,
    head: &HeadMarkup,
    mount: &MountMarkup,
    status: StatusCode,
) -> Result<Response, crate::axum::InertiaError> {
    let html = app
        .inner
        .root
        .render(RootContext::new(assets, head, mount))
        .map_err(crate::axum::InertiaError::root)?;
    let mut response = Html(html).into_response();
    *response.status_mut() = status;
    add_vary_header(&mut response);
    Ok(response)
}

pub(crate) fn finalize_page_object<T, F>(
    page: T,
    is_inertia: bool,
    status: StatusCode,
    html: F,
) -> Result<Response, crate::axum::InertiaError>
where
    T: Serialize,
    F: FnOnce(crate::HtmlResponseContext) -> Result<Response, crate::axum::InertiaError>,
{
    let mut response = if is_inertia {
        let mut response = Json(page).into_response();
        response
            .headers_mut()
            .insert(X_INERTIA_HEADER, HeaderValue::from_static("true"));
        response
    } else {
        html(html_response_context(&page)?)?
    };
    *response.status_mut() = status;
    add_vary_header(&mut response);
    Ok(response)
}
