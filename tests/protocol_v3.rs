#[path = "protocol_v3/support.rs"]
mod support;

#[path = "protocol_v3/basic_responses.rs"]
mod basic_responses;

#[path = "protocol_v3/request_headers.rs"]
mod request_headers;

#[path = "protocol_v3/page_objects.rs"]
mod page_objects;

#[path = "protocol_v3/versioning.rs"]
mod versioning;

#[path = "protocol_v3/partial_reloads.rs"]
mod partial_reloads;

#[path = "protocol_v3/lazy_deferred_once.rs"]
mod lazy_deferred_once;

#[path = "protocol_v3/merge_and_scroll.rs"]
mod merge_and_scroll;

#[path = "protocol_v3/shared_props.rs"]
mod shared_props;

#[path = "protocol_v3/redirects.rs"]
mod redirects;

#[path = "protocol_v3/errors.rs"]
mod errors;

#[path = "protocol_v3/security.rs"]
mod security;

#[path = "protocol_v3/compatibility.rs"]
mod compatibility;
