use crate::{TestApp, TestPage};
use axum::http::{
    HeaderMap, StatusCode,
    header::{CONTENT_TYPE, LOCATION},
};
use inertia_axum::{InertiaPage, X_INERTIA_LOCATION};

/// A buffered in-process response with fluent Inertia assertions.
pub struct TestResponse<'a> {
    pub(crate) app: &'a TestApp,
    pub(crate) status: StatusCode,
    pub(crate) headers: HeaderMap,
    pub(crate) body: Vec<u8>,
    pub(crate) inertia: bool,
}

impl<'a> TestResponse<'a> {
    /// Asserts that the document contains server-rendered application markup.
    pub fn assert_ssr(&self) -> &Self {
        let html = std::str::from_utf8(&self.body).expect("HTML response was not UTF-8");
        assert!(
            html.contains("data-server-rendered=\"true\""),
            "response was not server rendered"
        );
        self
    }
    /// Asserts that the document uses the client-rendered mount.
    pub fn assert_csr(&self) -> &Self {
        let html = std::str::from_utf8(&self.body).expect("HTML response was not UTF-8");
        assert!(
            !html.contains("data-server-rendered=\"true\""),
            "response was unexpectedly server rendered"
        );
        self
    }
    /// Asserts that SSR-generated head markup contains `expected`.
    pub fn assert_ssr_head_contains(&self, expected: &str) -> &Self {
        let html = std::str::from_utf8(&self.body).expect("HTML response was not UTF-8");
        assert!(
            html.contains(expected),
            "SSR head did not contain {expected:?}"
        );
        self
    }
    /// Returns the HTTP status.
    pub fn status(&self) -> StatusCode {
        self.status
    }
    /// Returns the response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
    /// Returns the buffered response body.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Asserts a `200 OK` response.
    pub fn assert_ok(self) -> Self {
        assert_eq!(self.status, StatusCode::OK);
        self
    }

    /// Asserts an HTML content type.
    pub fn assert_html(self) -> Self {
        let content_type = self
            .headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(
            content_type.starts_with("text/html"),
            "response was not HTML: {content_type}"
        );
        self
    }

    /// Parses a JSON response and verifies the generated component identity.
    pub fn assert_page<P: InertiaPage>(self) -> TestPage {
        assert_eq!(self.status, StatusCode::OK);
        let page: serde_json::Value =
            serde_json::from_slice(&self.body).expect("response body was not an Inertia JSON page");
        assert_component::<P>(&page);
        TestPage::new(page)
    }

    /// Extracts and verifies the page embedded in an initial HTML response.
    pub fn assert_html_page<P: InertiaPage>(&self) -> TestPage {
        let html = std::str::from_utf8(&self.body).expect("HTML response was not UTF-8");
        let marker = r#"<script data-page="app" type="application/json">"#;
        let start = html
            .find(marker)
            .unwrap_or_else(|| panic!("embedded Inertia page script was missing"))
            + marker.len();
        let end = html[start..]
            .find("</script>")
            .map(|offset| start + offset)
            .expect("embedded Inertia page script was not closed");
        let page = serde_json::from_str(&html[start..end])
            .expect("embedded Inertia page was invalid JSON");
        assert_component::<P>(&page);
        TestPage::new(page)
    }

    /// Asserts a `303 See Other` redirect to `expected`.
    pub fn assert_see_other(self, expected: &str) -> Self {
        assert_eq!(self.status, StatusCode::SEE_OTHER);
        assert_eq!(self.location(), Some(expected));
        self
    }

    /// Asserts an Inertia external-location or version conflict.
    pub fn assert_location_conflict(self, expected: &str) -> Self {
        assert_eq!(self.status, StatusCode::CONFLICT);
        assert_eq!(self.header(X_INERTIA_LOCATION), Some(expected));
        self
    }

    /// Asserts a page version without consuming the response.
    pub fn assert_version(&self, expected: impl serde::Serialize) -> &Self {
        self.parsed_page().assert_version(expected);
        self
    }

    /// Follows a redirect or external-location response and records its destination.
    pub async fn follow(&self) -> TestResponse<'a> {
        let destination = self
            .header(X_INERTIA_LOCATION)
            .or_else(|| self.location())
            .expect("response had no redirect location")
            .to_owned();
        self.app.history.lock().unwrap().push(destination.clone());
        if self.inertia {
            self.app.inertia_get(destination).send().await
        } else {
            self.app.get(destination).send().await
        }
    }

    fn parsed_page(&self) -> TestPage {
        TestPage::new(
            serde_json::from_slice(&self.body).expect("response body was not an Inertia JSON page"),
        )
    }
    fn location(&self) -> Option<&str> {
        self.header(LOCATION.as_str())
    }
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

fn assert_component<P: InertiaPage>(page: &serde_json::Value) {
    assert_eq!(
        page.get("component").and_then(serde_json::Value::as_str),
        Some(P::COMPONENT.as_str()),
        "page component differed"
    );
}
