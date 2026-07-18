//! Stateful in-process assertions for inertia-axum applications.

mod adapter;
mod app;
mod page;
mod request;
mod response;
mod ssr;

pub use adapter::{
    ActixHarness, AdapterHarness, AdapterRequest, AdapterResponse, AxumHarness, RocketHarness,
    run_adapter_conformance,
};
pub use app::TestApp;
pub use page::TestPage;
pub use request::TestRequest;
pub use response::TestResponse;
pub use ssr::{TestSsr, TestSsrBuilder, TestSsrCall, TestSsrDocument};
