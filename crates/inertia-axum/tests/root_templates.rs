#![allow(missing_docs)]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    routing::get,
};
#[cfg(feature = "askama")]
use inertia_axum::{
    AskamaRoot, AskamaRootContext,
    askama::{self, Template},
};
use inertia_axum::{DynamicPage, InertiaApp, RootContext, RootView, RouterInertiaExt as _};
use std::{convert::Infallible, fs};
use tower::ServiceExt as _;

const TEMPLATE: &str = "<!doctype html><html><head><!-- inertia:assets --><!-- inertia:head --></head><body data-shell=\"template\"><!-- inertia:mount --></body></html>";

async fn page() -> DynamicPage {
    DynamicPage::new("Home").prop("unsafe", "</script><script>alert(1)</script>")
}

async fn response_body(app: Router, inertia: bool) -> String {
    let mut builder = Request::get("/");
    if inertia {
        builder = builder.header("x-inertia", "true");
    }
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn router(inertia: inertia_axum::InertiaApp) -> Router {
    Router::new().route("/", get(page)).inertia(inertia)
}

#[tokio::test]
async fn inline_template_changes_initial_html_but_not_json_or_script_safety() {
    let app = router(
        InertiaApp::default_root()
            .root_template_source(TEMPLATE)
            .build()
            .unwrap(),
    );
    let html = response_body(app.clone(), false).await;
    assert!(html.contains("data-shell=\"template\""));
    assert!(html.contains(r"\u003C/script\u003E\u003Cscript\u003Ealert(1)\u003C/script\u003E"));
    let json = response_body(app, true).await;
    assert!(!json.contains("data-shell"));
    assert!(json.starts_with('{'));
}

#[tokio::test]
async fn file_is_compiled_at_build_and_never_read_by_requests() {
    let path = std::env::temp_dir().join(format!(
        "inertia-axum-root-{}-{}.html",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::write(&path, TEMPLATE).unwrap();
    let inertia = InertiaApp::default_root()
        .root_template(&path)
        .build()
        .unwrap();
    fs::remove_file(&path).unwrap();
    let app = router(inertia);
    for _ in 0..2 {
        assert!(
            response_body(app.clone(), false)
                .await
                .contains("data-shell=\"template\"")
        );
    }
}

#[test]
fn missing_and_invalid_files_fail_during_build() {
    let missing =
        std::env::temp_dir().join(format!("inertia-axum-missing-{}.html", std::process::id()));
    let error = InertiaApp::default_root()
        .root_template(&missing)
        .build()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("Could not read template"));
    assert!(error.contains(&missing.display().to_string()));

    let path =
        std::env::temp_dir().join(format!("inertia-axum-invalid-{}.html", std::process::id()));
    fs::write(&path, "<!-- inertia:assets -->").unwrap();
    let error = InertiaApp::default_root()
        .root_template(&path)
        .build()
        .err()
        .unwrap()
        .to_string();
    fs::remove_file(path).unwrap();
    assert!(error.contains("missing the required marker"));
}

#[derive(Clone)]
struct Custom;
impl RootView for Custom {
    type Error = Infallible;
    fn render(&self, _: RootContext<'_>) -> Result<String, Self::Error> {
        Ok("CUSTOM".to_owned())
    }
}

#[cfg(feature = "askama")]
#[derive(Template)]
#[template(
    source = "<html><head>{{ inertia.assets|safe }}{{ inertia.head|safe }}</head><body data-shell=\"{{ shell }}\">{{ inertia.mount|safe }}</body></html>",
    ext = "html",
    askama = askama
)]
struct AskamaTemplate<'a> {
    inertia: AskamaRootContext<'a>,
    shell: &'a str,
}

#[cfg(feature = "askama")]
#[derive(Clone)]
struct AskamaShell(&'static str);

#[cfg(feature = "askama")]
impl AskamaRoot for AskamaShell {
    type Template<'a> = AskamaTemplate<'a>;

    fn template<'a>(&'a self, inertia: AskamaRootContext<'a>) -> Self::Template<'a> {
        AskamaTemplate {
            inertia,
            shell: self.0,
        }
    }
}

#[tokio::test]
async fn root_selection_is_last_call_wins() {
    let path =
        std::env::temp_dir().join(format!("inertia-axum-ordering-{}.html", std::process::id()));
    fs::write(&path, TEMPLATE).unwrap();
    let custom = InertiaApp::default_root()
        .root_template(&path)
        .root(Custom)
        .build()
        .unwrap();
    assert_eq!(response_body(router(custom), false).await, "CUSTOM");
    let template = InertiaApp::builder(Custom)
        .root_template(&path)
        .build()
        .unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        response_body(router(template), false)
            .await
            .contains("data-shell=\"template\"")
    );
}

#[cfg(feature = "askama")]
#[tokio::test]
async fn askama_root_selection_is_last_call_wins() {
    let askama = InertiaApp::builder(Custom)
        .root_template_source(TEMPLATE)
        .askama_root(AskamaShell("askama"))
        .build()
        .unwrap();
    assert!(
        response_body(router(askama), false)
            .await
            .contains("data-shell=\"askama\"")
    );

    let custom = InertiaApp::default_root()
        .askama_root(AskamaShell("askama"))
        .root(Custom)
        .build()
        .unwrap();
    assert_eq!(response_body(router(custom), false).await, "CUSTOM");

    let marker = InertiaApp::default_root()
        .askama_root(AskamaShell("askama"))
        .root_template_source(TEMPLATE)
        .build()
        .unwrap();
    assert!(
        response_body(router(marker), false)
            .await
            .contains("data-shell=\"template\"")
    );
}

#[tokio::test]
async fn unconfigured_default_root_is_byte_compatible() {
    let html = response_body(router(InertiaApp::default_root().build().unwrap()), false).await;
    assert!(html.starts_with("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"));
    assert!(html.contains("</head><body><script data-page=\"app\" type=\"application/json\">"));
    assert!(html.ends_with("</script><div id=\"app\"></div></body></html>"));
    assert!(!html.contains("data-shell"));
}
