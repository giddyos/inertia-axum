//! Stateful in-process assertions for inertia-axum applications.

mod app;
mod page;
mod request;
mod response;
mod ssr;

pub use app::TestApp;
pub use page::TestPage;
pub use request::TestRequest;
pub use response::TestResponse;
pub use ssr::{TestSsr, TestSsrBuilder, TestSsrCall, TestSsrDocument};
