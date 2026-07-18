use crate::{TestApp, TestResponse};
use axum::{
    body::{Body, to_bytes},
    http::{
        HeaderMap, Method, Request,
        header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
    },
    response::Response,
};
use inertia_axum::{
    PropKey, X_INERTIA, X_INERTIA_ERROR_BAG, X_INERTIA_EXCEPT_ONCE_PROPS,
    X_INERTIA_INFINITE_SCROLL_MERGE_INTENT, X_INERTIA_PARTIAL_COMPONENT, X_INERTIA_PARTIAL_DATA,
    X_INERTIA_PARTIAL_EXCEPT, X_INERTIA_RESET, X_INERTIA_VERSION,
};
use serde::Serialize;
use std::str::FromStr;
use tower::ServiceExt;

/// Fluent in-process request builder.
pub struct TestRequest<'a> {
    pub(crate) app: &'a TestApp,
    method: Method,
    uri: String,
    inertia: bool,
    headers: HeaderMap,
    body: Body,
    partial_component: Option<&'static str>,
    only: Vec<&'static str>,
    except: Vec<&'static str>,
    reset: Vec<&'static str>,
}

impl<'a> TestRequest<'a> {
    pub(crate) fn new(app: &'a TestApp, method: &str, uri: String, inertia: bool) -> Self {
        Self {
            app,
            method: Method::from_str(method).unwrap(),
            uri,
            inertia,
            headers: HeaderMap::new(),
            body: Body::empty(),
            partial_component: None,
            only: Vec::new(),
            except: Vec::new(),
            reset: Vec::new(),
        }
    }
    /// Requests one typed prop on a matching partial reload.
    pub fn only<T>(mut self, key: PropKey<T>) -> Self {
        self.set_component(key);
        self.only.push(key.name());
        self
    }
    /// Excludes one typed prop on a matching partial reload.
    pub fn except<T>(mut self, key: PropKey<T>) -> Self {
        self.set_component(key);
        self.except.push(key.name());
        self
    }
    /// Resets one typed merge prop.
    pub fn reset<T>(mut self, key: PropKey<T>) -> Self {
        self.set_component(key);
        self.reset.push(key.name());
        self
    }
    /// Excludes a client-cached once prop by its stable cache key.
    pub fn except_once(mut self, key: &str) -> Self {
        self.headers
            .insert(X_INERTIA_EXCEPT_ONCE_PROPS, key.parse().unwrap());
        self
    }
    /// Selects append or prepend infinite-scroll merge intent.
    pub fn scroll_intent(mut self, intent: &str) -> Self {
        self.headers.insert(
            X_INERTIA_INFINITE_SCROLL_MERGE_INTENT,
            intent.parse().unwrap(),
        );
        self
    }
    fn set_component<T>(&mut self, key: PropKey<T>) {
        let component = key.component().as_str();
        if let Some(existing) = self.partial_component {
            assert_eq!(
                existing, component,
                "typed partial keys must belong to one component"
            );
        }
        self.partial_component = Some(component);
    }
    /// Sets the validation error bag header.
    pub fn error_bag(mut self, bag: &str) -> Self {
        self.headers
            .insert(X_INERTIA_ERROR_BAG, bag.parse().unwrap());
        self
    }
    /// Overrides the asset version header.
    pub fn version(mut self, version: impl ToString) -> Self {
        self.headers
            .insert(X_INERTIA_VERSION, version.to_string().parse().unwrap());
        self
    }
    /// Adds an arbitrary request header.
    pub fn header(mut self, name: &'static str, value: &str) -> Self {
        self.headers.insert(name, value.parse().unwrap());
        self
    }
    /// Serializes a JSON request body.
    pub fn json(mut self, value: &impl Serialize) -> Self {
        self.body = Body::from(serde_json::to_vec(value).unwrap());
        self.headers
            .insert(CONTENT_TYPE, "application/json".parse().unwrap());
        self
    }
    /// Serializes an application/x-www-form-urlencoded body from key/value pairs.
    pub fn form(mut self, fields: &[(&str, &str)]) -> Self {
        let body = fields
            .iter()
            .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        self.body = Body::from(body);
        self.headers.insert(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        self
    }
    /// Sends this request through Tower.
    pub async fn send(mut self) -> TestResponse<'a> {
        if self.inertia {
            self.headers.insert(X_INERTIA, "true".parse().unwrap());
        }
        if self.inertia && !self.headers.contains_key(X_INERTIA_VERSION) {
            if let Some(version) = &self.app.default_version {
                self.headers
                    .insert(X_INERTIA_VERSION, version.parse().unwrap());
            }
        }
        if let Some(component) = self.partial_component {
            self.headers
                .insert(X_INERTIA_PARTIAL_COMPONENT, component.parse().unwrap());
        }
        if !self.only.is_empty() {
            self.headers
                .insert(X_INERTIA_PARTIAL_DATA, self.only.join(",").parse().unwrap());
        }
        if !self.except.is_empty() {
            self.headers.insert(
                X_INERTIA_PARTIAL_EXCEPT,
                self.except.join(",").parse().unwrap(),
            );
        }
        if !self.reset.is_empty() {
            self.headers
                .insert(X_INERTIA_RESET, self.reset.join(",").parse().unwrap());
        }
        let cookie_header = {
            let cookies = self.app.cookies.lock().unwrap();
            (!cookies.is_empty()).then(|| cookies.values().cloned().collect::<Vec<_>>().join("; "))
        };
        if let Some(cookie_header) = cookie_header {
            self.headers.insert(COOKIE, cookie_header.parse().unwrap());
        }
        let request = Request::builder()
            .method(self.method)
            .uri(&self.uri)
            .body(self.body)
            .unwrap();
        let (mut parts, body) = request.into_parts();
        parts.headers = self.headers;
        let response: Response = self
            .app
            .router
            .clone()
            .oneshot(Request::from_parts(parts, body))
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        persist_cookies(self.app, &headers);
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        TestResponse {
            app: self.app,
            status,
            headers,
            body: bytes.to_vec(),
            inertia: self.inertia,
        }
    }
}

fn persist_cookies(app: &TestApp, headers: &HeaderMap) {
    let mut cookies = app.cookies.lock().unwrap();
    for value in headers.get_all(SET_COOKIE) {
        if let Ok(value) = value.to_str() {
            if let Some(pair) = value.split(';').next() {
                if let Some((name, _)) = pair.split_once('=') {
                    let deleted = value.split(';').skip(1).map(str::trim).any(|attribute| {
                        attribute.eq_ignore_ascii_case("max-age=0")
                            || attribute
                                .to_ascii_lowercase()
                                .starts_with("expires=thu, 01 jan 1970")
                    });
                    if deleted {
                        cookies.remove(name);
                    } else {
                        cookies.insert(name.to_owned(), pair.to_owned());
                    }
                }
            }
        }
    }
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "+".to_owned(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
