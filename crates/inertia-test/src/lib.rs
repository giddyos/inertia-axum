//! Shared adapter conformance plus optional framework testing utilities.

mod adapter;
#[cfg(feature = "axum")]
mod app;
#[cfg(feature = "axum")]
mod page;
#[cfg(feature = "axum")]
mod request;
#[cfg(feature = "axum")]
mod response;
#[cfg(feature = "axum")]
mod ssr;

#[cfg(feature = "actix")]
pub use adapter::ActixHarness;
#[cfg(feature = "axum")]
pub use adapter::AxumHarness;
#[cfg(feature = "rocket")]
pub use adapter::RocketHarness;
pub use adapter::{AdapterHarness, AdapterRequest, AdapterResponse, run_adapter_conformance};
#[cfg(feature = "axum")]
pub use app::TestApp;
#[cfg(feature = "axum")]
pub use page::TestPage;
#[cfg(feature = "axum")]
pub use request::TestRequest;
#[cfg(feature = "axum")]
pub use response::TestResponse;
#[cfg(feature = "axum")]
pub use ssr::{TestSsr, TestSsrBuilder, TestSsrCall, TestSsrDocument};
