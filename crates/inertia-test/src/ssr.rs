use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use inertia_axum::Ssr;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// A deterministic document returned by [`TestSsr`].
#[derive(Clone, Debug)]
pub struct TestSsrDocument {
    /// Trusted head fragments returned by the fake backend.
    pub head: Vec<String>,
    /// Trusted mount body returned by the fake backend.
    pub body: String,
}
impl TestSsrDocument {
    /// Creates a test SSR document.
    pub fn new(head: impl IntoIterator<Item = String>, body: impl Into<String>) -> Self {
        Self {
            head: head.into_iter().collect(),
            body: body.into(),
        }
    }
}

/// One recorded render call.
#[derive(Clone, Debug)]
pub struct TestSsrCall {
    component: String,
    url: String,
    page: Value,
}
impl TestSsrCall {
    /// Returns the requested Inertia component.
    pub fn component(&self) -> &str {
        &self.component
    }
    /// Returns the requested page URL.
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Returns the complete page sent to the fake backend.
    pub fn page(&self) -> &Value {
        &self.page
    }
}

#[derive(Default)]
struct Shared {
    documents: BTreeMap<String, TestSsrDocument>,
    calls: Mutex<Vec<TestSsrCall>>,
}

/// Builder for a Node-free fake Inertia SSR server.
#[derive(Default)]
pub struct TestSsrBuilder {
    documents: BTreeMap<String, TestSsrDocument>,
}
impl TestSsrBuilder {
    /// Configures a rendered document for `component`.
    pub fn render(mut self, component: impl Into<String>, document: TestSsrDocument) -> Self {
        self.documents.insert(component.into(), document);
        self
    }
    /// Starts the fake server on an ephemeral loopback port.
    pub async fn start(self) -> TestSsr {
        let shared = Arc::new(Shared {
            documents: self.documents,
            calls: Mutex::new(Vec::new()),
        });
        let app = Router::new()
            .route("/health", get(|| async { StatusCode::OK }))
            .route("/render", post(render))
            .route("/shutdown", get(|| async { StatusCode::OK }))
            .with_state(shared.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        TestSsr {
            endpoint,
            shared,
            task,
        }
    }
}

async fn render(
    State(shared): State<Arc<Shared>>,
    Json(page): Json<Value>,
) -> Result<Json<TestSsrDocumentWire>, StatusCode> {
    let component = page
        .get("component")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let url = page
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    shared.calls.lock().unwrap().push(TestSsrCall {
        component: component.clone(),
        url,
        page,
    });
    shared
        .documents
        .get(&component)
        .cloned()
        .map(|document| {
            Json(TestSsrDocumentWire {
                head: document.head,
                body: document.body,
            })
        })
        .ok_or(StatusCode::NOT_FOUND)
}

#[derive(serde::Serialize)]
struct TestSsrDocumentWire {
    head: Vec<String>,
    body: String,
}

/// Running fake SSR server and its recorded calls.
pub struct TestSsr {
    endpoint: String,
    shared: Arc<Shared>,
    task: tokio::task::JoinHandle<()>,
}
impl TestSsr {
    /// Starts configuring a fake SSR server.
    pub fn builder() -> TestSsrBuilder {
        TestSsrBuilder::default()
    }
    /// Returns an external SSR configuration targeting this server.
    pub fn config(&self) -> Ssr {
        Ssr::external(self.endpoint())
    }
    /// Returns the fake server endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    /// Returns all recorded render calls.
    pub fn calls(&self) -> Vec<TestSsrCall> {
        self.shared.calls.lock().unwrap().clone()
    }
    /// Asserts the number of render calls.
    pub fn assert_render_count(&self, expected: usize) {
        assert_eq!(self.calls().len(), expected);
    }
    /// Asserts that a component was rendered.
    pub fn assert_rendered_component(&self, expected: &str) {
        assert!(
            self.calls().iter().any(|call| call.component() == expected),
            "component {expected:?} was not rendered"
        );
    }
    /// Asserts that a component was not rendered.
    pub fn assert_not_rendered_component(&self, expected: &str) {
        assert!(
            !self.calls().iter().any(|call| call.component() == expected),
            "component {expected:?} was rendered"
        );
    }
}
impl Drop for TestSsr {
    fn drop(&mut self) {
        self.task.abort();
    }
}
