//! Eager serializable props conversion and request filtering.

use super::resolver::IntoPageProps;
use crate::page::PageMetadata;
use crate::request::{EffectiveRequest, RequestContext, SelectionMode, SelectionPlan};
use crate::shared::ensure_errors_prop;
use serde::Serialize;
use serde_json::Value;

impl<T: Serialize> IntoPageProps for T {
    fn into_page_props(
        self,
        component: &str,
        request: &RequestContext,
        partial_reload_enabled: bool,
        metadata: PageMetadata,
    ) -> Result<(Value, PageMetadata, Vec<String>), serde_json::Error> {
        let mut props = serde_json::to_value(self)?;
        let route_props = props
            .as_object()
            .map(|props| props.keys().cloned().collect())
            .unwrap_or_default();

        if let Some(object) = props.as_object_mut() {
            ensure_errors_prop(object);
            let plan = SelectionPlan::new(
                EffectiveRequest::new(request, partial_reload_enabled),
                component,
                &metadata,
            );
            object.retain(|key, _| key == "errors" || plan.includes(key, SelectionMode::Standard));
        }
        let metadata = metadata.into_response_metadata(request, component, props.as_object());

        Ok((props, metadata, route_props))
    }
}
