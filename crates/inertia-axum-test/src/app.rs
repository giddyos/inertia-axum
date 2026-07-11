use crate::TestRequest;
use axum::Router;
use std::{collections::BTreeMap, sync::Mutex};

/// Stateful in-process application client.
pub struct TestApp {
    pub(crate) router: Router,
    pub(crate) cookies: Mutex<BTreeMap<String, String>>,
    pub(crate) history: Mutex<Vec<String>>,
    pub(crate) default_version: Option<String>,
}

impl TestApp {
    /// Wraps an Axum router without binding a socket.
    pub fn new(router: Router) -> Self {
        Self {
            router,
            cookies: Mutex::new(BTreeMap::new()),
            history: Mutex::new(Vec::new()),
            default_version: None,
        }
    }
    /// Sets the version sent by subsequent Inertia requests.
    pub fn with_version(mut self, version: impl ToString) -> Self {
        self.default_version = Some(version.to_string());
        self
    }
    /// Starts an ordinary initial-page GET.
    pub fn get(&self, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, "GET", uri.into(), false)
    }
    /// Starts an Inertia GET.
    pub fn inertia_get(&self, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, "GET", uri.into(), true)
    }
    /// Starts an Inertia POST.
    pub fn inertia_post(&self, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, "POST", uri.into(), true)
    }
    /// Starts an Inertia PUT.
    pub fn inertia_put(&self, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, "PUT", uri.into(), true)
    }
    /// Starts an Inertia PATCH.
    pub fn inertia_patch(&self, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, "PATCH", uri.into(), true)
    }
    /// Starts an Inertia DELETE.
    pub fn inertia_delete(&self, uri: impl Into<String>) -> TestRequest<'_> {
        TestRequest::new(self, "DELETE", uri.into(), true)
    }
    /// Returns followed redirect destinations in order.
    pub fn history(&self) -> Vec<String> {
        self.history.lock().unwrap().clone()
    }
}
