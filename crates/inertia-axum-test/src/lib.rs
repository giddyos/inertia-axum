//! Stateful in-process assertions for inertia-axum applications.

mod app;
mod page;
mod request;
mod response;

pub use app::TestApp;
pub use page::TestPage;
pub use request::TestRequest;
pub use response::TestResponse;
