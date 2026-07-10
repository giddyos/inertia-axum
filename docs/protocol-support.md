# Protocol Support Matrix

This matrix covers the Axum adapter and the framework-neutral Inertia protocol
core. The "Verified by" column lists representative tests.

Status vocabulary:

- `Supported`: implemented and covered by representative tests.
- `Partial`: implemented with a documented limitation.
- `Not supported`: not implemented.

| Feature | Status | Verified by |
| --- | --- | --- |
| Initial HTML response | Supported | `axum::tests::html_response_includes_escaped_page_and_version` |
| JSON Inertia response | Supported | `axum::tests::inertia_json_response_includes_headers_url_and_version` |
| Asset version conflict | Supported | `axum::tests::stale_inertia_version_conflicts_before_handler_runs` |
| Dynamic asset version | Supported | `axum::tests::dynamic_version_is_resolved_for_each_page_response` |
| Query-string and local page URLs | Supported | `axum::tests::nested_routes_use_original_uri_for_page_urls` |
| Request header parsing | Supported | `tests::request_context_parses_inertia_headers`; `axum::tests::inertia_json_response_includes_headers_url_and_version` |
| Partial reloads | Supported | `axum::tests::partial_reloads_include_shared_props_but_preserve_route_owned_roots`; `axum::tests::partial_except_takes_precedence_over_partial_data` |
| Component mismatch | Supported | `axum::tests::partial_reload_component_mismatch_ignores_partial_filtering` |
| Merge and deep-merge metadata | Supported | `tests::page_serializes_v3_metadata`; `axum::tests::render_supports_advanced_builder_pages` |
| Deferred props | Partial | `tests::deferred_props_emit_metadata_and_resolve_only_when_requested`; `axum::tests::render_supports_lazy_props` |
| Lazy and optional props | Partial | `tests::lazy_props_are_only_resolved_when_included`; `axum::tests::render_supports_lazy_props` |
| Once props | Supported | `tests::once_lazy_props_are_not_resolved_when_client_already_has_them`; `axum::tests::post_response_ignores_partial_reload_but_preserves_once_exclusions` |
| Shared props | Supported | `axum::tests::shared_props_are_merged_into_json_responses`; `axum::tests::shared_props_promote_non_object_props_to_an_object` |
| History flags | Supported | `tests::page_serializes_v3_metadata`; `axum::tests::history_flags_are_preserved_through_axum_rendering` |
| Scroll and infinite-scroll metadata | Supported | `tests::infinite_scroll_merge_intent_can_prepend_scroll_props`; `axum::tests::infinite_scroll_prepend_intent_sets_prepend_metadata` |
| Reset metadata | Supported | `axum::tests::reset_omits_merge_and_scroll_metadata_for_reset_props` |
| Errors prop and error-bag headers | Partial | `tests::request_context_parses_inertia_headers`; `tests::lazy_errors_are_preserved_during_partial_reloads` |
| External location redirects | Supported | `axum::tests::external_location_uses_conflict_for_inertia_requests` |
| Write-method redirects | Supported | `axum::tests::all_write_redirect_methods_use_see_other_status` |
| Not-found passthrough | Supported | `axum::tests::matching_version_preserves_not_found_responses` |
| SSR bridge | Not supported | Not applicable |

Deferred, lazy, and optional prop support is marked `Partial` because the
current prop container supports synchronous resolvers only. Async prop
resolvers remain planned.

Errors support is marked `Partial` because the protocol header is parsed and
the `errors` prop shape is preserved, but the Axum integration does not provide
a framework-level validation error bag or flash-message integration.
