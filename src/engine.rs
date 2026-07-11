//! Request-aware finalization without extractor or Tower concerns.

use crate::{
    app::InertiaApp,
    axum::response_headers::{
        add_vary_header, conflict_response, is_write_method, redirect_response,
    },
    html::html_response_context,
    response::{PendingPage, PendingResponse},
    root::{MountMarkup, RootContext},
    visit::Visit,
    X_INERTIA_HEADER,
};
use crate::{
    page::{Page, PageDraft, PageMetadata},
    request::{EffectiveRequest, SelectionPlan},
    shared::ensure_errors_prop,
};
use axum::{
    http::{HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use futures_util::{stream, StreamExt};
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Clone)]
pub(crate) struct Engine {
    app: InertiaApp,
}

impl Engine {
    pub(crate) fn new(app: InertiaApp) -> Self {
        Self { app }
    }

    pub(crate) async fn finalize(&self, visit: &Visit, pending: PendingResponse) -> Response {
        let result = match pending {
            PendingResponse::Page(page) => self.finalize_page(visit, *page).await,
            PendingResponse::Redirect(redirect) => {
                let status = if is_write_method(&visit.method) {
                    StatusCode::SEE_OTHER
                } else {
                    StatusCode::FOUND
                };
                redirect_response(status, redirect.resolve(visit.referer.as_deref()))
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
        };
        result.unwrap_or_else(crate::axum::error::internal_error_response)
    }

    async fn finalize_page(
        &self,
        visit: &Visit,
        pending: PendingPage,
    ) -> Result<Response, crate::axum::InertiaError> {
        let PendingPage {
            component,
            props,
            encrypt_history,
            clear_history,
            preserve_fragment,
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
        for prop in &props {
            prop.apply_metadata(&mut metadata, !prop.is_fresh_once());
        }
        let partial_enabled = visit.method == axum::http::Method::GET;
        let selected = {
            let plan = SelectionPlan::new(
                EffectiveRequest::new(&visit.context, partial_enabled),
                &component,
                &metadata,
            );
            let mut selected = Vec::new();
            for prop in props {
                if plan.includes(&prop.key, prop.mode()) {
                    selected.push(prop);
                }
            }
            selected
        };
        for prop in &selected {
            if prop.is_fresh_once() {
                prop.apply_metadata(&mut metadata, true);
            }
        }
        let mut resolved = Vec::with_capacity(selected.len());
        type IndexedResolution = std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = (usize, (String, crate::InertiaResult<Value>, bool)),
                    > + Send
                    + 'static,
            >,
        >;
        let mut asynchronous: Vec<IndexedResolution> = Vec::new();
        for (index, prop) in selected.into_iter().enumerate() {
            resolved.push(None);
            match prop.into_resolution() {
                crate::props::PendingResolution::Ready(result) => resolved[index] = Some(result),
                crate::props::PendingResolution::Async(future) => {
                    asynchronous.push(Box::pin(async move { (index, future.await) }))
                }
            }
        }
        let asynchronous = stream::iter(asynchronous)
            .buffered(16)
            .collect::<Vec<_>>()
            .await;
        for (index, result) in asynchronous {
            resolved[index] = Some(result);
        }
        let mut values = Map::new();
        let mut route_roots = Vec::new();
        for (key, result, rescue) in resolved
            .into_iter()
            .map(|result| result.expect("selected prop resolved exactly once"))
        {
            match result {
                Ok(value) => {
                    route_roots.push(key.clone());
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
                Err(error) => return Err(crate::axum::InertiaError::prop(error)),
            }
        }
        ensure_errors_prop(&mut values);
        metadata = metadata.into_response_metadata(&visit.context, &component, Some(&values));
        let page = Page::from_parts_version(
            component,
            Value::Object(values),
            visit.uri.to_string(),
            self.app.inner.assets.version.clone(),
            metadata,
        );
        let page = PageDraft::new(page, route_roots).finish();
        finalize_page_object(page, visit.is_inertia(), status, |serialized| {
            let assets = self.app.inner.assets.tags.clone();
            let mount = MountMarkup::new(serialized.data_page());
            let html = self
                .app
                .inner
                .root
                .render(RootContext::new(&assets, &mount))
                .map_err(crate::axum::InertiaError::root)?;
            Ok(Html(html).into_response())
        })
    }
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
