#![allow(dead_code, missing_docs)]

use actix_web::{
    App,
    body::MessageBody,
    dev::ServiceResponse,
    test as actix_test,
    web::{self, ServiceConfig},
};
use axum::{
    Router,
    routing::{get, post},
};
use bytes::Bytes;
use inertia_embed::{EmbeddedAsset, EmbeddedFrontend, EmbeddedStorage};
use inertia_test::{
    ActixHarness, AdapterRequest, AdapterResponse, AxumHarness, RocketHarness, TestSsr,
    TestSsrDocument, run_adapter_conformance,
};
use serde::Deserialize;
use std::{convert::Infallible, io};

static ASSETS: &[EmbeddedAsset] = &[
    EmbeddedAsset {
        path: "assets/app.css",
        bytes: b"body{color:#123}",
        storage: EmbeddedStorage::Identity,
        content_type: "text/css; charset=utf-8",
        etag: "\"css-adapter\"",
        immutable: false,
        encoding: None,
    },
    EmbeddedAsset {
        path: "assets/app.js",
        bytes: b"console.log('adapter')",
        storage: EmbeddedStorage::Identity,
        content_type: "text/javascript; charset=utf-8",
        etag: "\"js-adapter\"",
        immutable: false,
        encoding: None,
    },
];

static FRONTEND: EmbeddedFrontend = EmbeddedFrontend::new(
    "/build",
    "src/main.ts",
    "contract-v1",
    "<link rel=\"stylesheet\" href=\"/build/assets/app.css\"><script type=\"module\" src=\"/build/assets/app.js\"></script>",
    ASSETS,
);

#[derive(inertia_axum::InertiaProps)]
struct SharedProps {
    shared: &'static str,
}

#[derive(Clone)]
struct Shared;

impl inertia_axum::Share for Shared {
    type Props = SharedProps;
    type Error = Infallible;

    fn share(&self, _context: inertia_axum::ShareContext<'_>) -> Result<Self::Props, Self::Error> {
        Ok(SharedProps { shared: "adapter" })
    }
}

#[derive(Deserialize)]
struct FormInput {
    title: String,
}

async fn app_and_ssr() -> (inertia_axum::InertiaApp, TestSsr) {
    let ssr = TestSsr::builder()
        .render(
            "Ssr",
            TestSsrDocument::new(
                ["<title>Adapter SSR</title>".to_owned()],
                r#"<script data-page="app" type="application/json">{"component":"Ssr"}</script><div data-server-rendered="true" id="app">SSR</div>"#,
            ),
        )
        .start()
        .await;
    let inertia = inertia_axum::InertiaApp::embedded(&FRONTEND)
        .share(Shared)
        .transient(inertia_axum::MemoryTransient::new())
        .ssr(ssr.config())
        .start()
        .await
        .expect("shared conformance app must start");
    (inertia, ssr)
}

async fn axum_page() -> inertia_axum::DynamicPage {
    inertia_axum::DynamicPage::new("Conformance")
        .prop("ordinary", "route")
        .async_prop(
            "deferred",
            inertia_axum::defer(|| async { Ok::<_, io::Error>(1_u32) }),
        )
        .async_prop(
            "optional",
            inertia_axum::optional(|| async { Ok::<_, io::Error>(2_u32) }),
        )
        .async_prop("merged", inertia_axum::merge(vec![1_u32]).append())
        .async_prop(
            "once",
            inertia_axum::once(|| async { Ok::<_, io::Error>("cached") }),
        )
        .async_prop(
            "scroll",
            inertia_axum::scroll(inertia_axum::ScrollPage::new(vec![1_u32, 2], 1).next(2)),
        )
}

async fn axum_form(
    form: inertia_axum::Form<FormInput>,
) -> Result<inertia_axum::Redirect, inertia_axum::FormError> {
    form.validate_with(|input| {
        if input.title.trim().is_empty() {
            Err(inertia_axum::Errors::field("title", "required"))
        } else {
            Ok(())
        }
    })?;
    Ok(inertia_axum::Redirect::to("/page"))
}

fn axum_harness(inertia: inertia_axum::InertiaApp) -> AxumHarness {
    use inertia_axum::RouterInertiaExt as _;

    let installed = Router::new()
        .route("/page", get(axum_page))
        .route(
            "/redirect",
            post(|| async { inertia_axum::Redirect::to("/page") }),
        )
        .route(
            "/external",
            get(|| async { inertia_axum::Location::external("https://example.com/outside") }),
        )
        .route("/form", post(axum_form))
        .route(
            "/flash",
            post(|| async { inertia_axum::Redirect::to("/page").flash("notice", "saved") }),
        )
        .route(
            "/ssr",
            get(|| async { inertia_axum::DynamicPage::new("Ssr") }),
        )
        .route(
            "/ssr-fallback",
            get(|| async { inertia_axum::DynamicPage::new("SsrFallback") }),
        )
        .route("/health", get(|| async { "healthy" }))
        .inertia(inertia);
    let uninstalled = Router::new().route("/missing", get(axum_page));
    AxumHarness::new(installed, uninstalled)
}

async fn actix_page() -> inertia_actix::DynamicPage {
    inertia_actix::DynamicPage::new("Conformance")
        .prop("ordinary", "route")
        .async_prop(
            "deferred",
            inertia_actix::defer(|| async { Ok::<_, io::Error>(1_u32) }),
        )
        .async_prop(
            "optional",
            inertia_actix::optional(|| async { Ok::<_, io::Error>(2_u32) }),
        )
        .async_prop("merged", inertia_actix::merge(vec![1_u32]).append())
        .async_prop(
            "once",
            inertia_actix::once(|| async { Ok::<_, io::Error>("cached") }),
        )
        .async_prop(
            "scroll",
            inertia_actix::scroll(inertia_actix::ScrollPage::new(vec![1_u32, 2], 1).next(2)),
        )
}

async fn actix_form(
    form: inertia_actix::InertiaForm<FormInput>,
) -> Result<inertia_actix::Redirect, inertia_actix::FormError> {
    form.validate_with(|input| {
        if input.title.trim().is_empty() {
            Err(inertia_actix::Errors::field("title", "required"))
        } else {
            Ok(())
        }
    })?;
    Ok(inertia_actix::Redirect::to("/page"))
}

fn actix_routes(config: &mut ServiceConfig) {
    config
        .route("/page", web::get().to(actix_page))
        .route(
            "/redirect",
            web::post().to(|| async { inertia_actix::Redirect::to("/page") }),
        )
        .route(
            "/external",
            web::get()
                .to(|| async { inertia_actix::Location::external("https://example.com/outside") }),
        )
        .route("/form", web::post().to(actix_form))
        .route(
            "/flash",
            web::post()
                .to(|| async { inertia_actix::Redirect::to("/page").flash("notice", "saved") }),
        )
        .route(
            "/ssr",
            web::get().to(|| async { inertia_actix::DynamicPage::new("Ssr") }),
        )
        .route(
            "/ssr-fallback",
            web::get().to(|| async { inertia_actix::DynamicPage::new("SsrFallback") }),
        )
        .route("/health", web::get().to(|| async { "healthy" }));
}

fn actix_request(request: AdapterRequest) -> actix_test::TestRequest {
    let method = actix_web::http::Method::from_bytes(request.method.as_str().as_bytes())
        .expect("core test method must convert to Actix");
    let mut native = actix_test::TestRequest::default()
        .method(method)
        .uri(&request.uri.to_string())
        .set_payload(request.body);
    for (name, value) in &request.headers {
        let name = actix_web::http::header::HeaderName::from_bytes(name.as_str().as_bytes())
            .expect("core test header name must convert to Actix");
        let value = actix_web::http::header::HeaderValue::from_bytes(value.as_bytes())
            .expect("core test header value must convert to Actix");
        native = native.append_header((name, value));
    }
    native
}

async fn actix_response<B>(response: ServiceResponse<B>) -> AdapterResponse
where
    B: MessageBody,
{
    let status =
        http::StatusCode::from_u16(response.status().as_u16()).expect("Actix status must convert");
    let mut headers = http::HeaderMap::new();
    for (name, value) in response.headers() {
        let name = http::HeaderName::from_bytes(name.as_str().as_bytes())
            .expect("Actix response header name must convert");
        let value = http::HeaderValue::from_bytes(value.as_bytes())
            .expect("Actix response header value must convert");
        headers.append(name, value);
    }
    let Ok(body) = actix_web::body::to_bytes(response.into_body()).await else {
        panic!("Actix adapter response body must buffer");
    };
    let body = Bytes::copy_from_slice(&body);
    AdapterResponse {
        status,
        headers,
        body,
    }
}

async fn request_actix(
    inertia: inertia_actix::InertiaApp,
    request: AdapterRequest,
) -> AdapterResponse {
    if request.uri.path() == "/missing" {
        let app =
            actix_test::init_service(App::new().route("/missing", web::get().to(actix_page))).await;
        let request = actix_request(request).to_request();
        return actix_response(actix_test::call_service(&app, request).await).await;
    }

    let app = actix_test::init_service(
        App::new()
            .configure(actix_routes)
            .app_data(web::Data::new(inertia.clone()))
            .wrap(inertia_actix::InertiaMiddleware::new(inertia.clone()))
            .configure(inertia_actix::assets(inertia)),
    )
    .await;
    let request = actix_request(request).to_request();
    actix_response(actix_test::call_service(&app, request).await).await
}

#[rocket::get("/page")]
fn rocket_page() -> inertia_rocket::DynamicPage {
    inertia_rocket::DynamicPage::new("Conformance")
        .prop("ordinary", "route")
        .async_prop(
            "deferred",
            inertia_rocket::defer(|| async { Ok::<_, io::Error>(1_u32) }),
        )
        .async_prop(
            "optional",
            inertia_rocket::optional(|| async { Ok::<_, io::Error>(2_u32) }),
        )
        .async_prop("merged", inertia_rocket::merge(vec![1_u32]).append())
        .async_prop(
            "once",
            inertia_rocket::once(|| async { Ok::<_, io::Error>("cached") }),
        )
        .async_prop(
            "scroll",
            inertia_rocket::scroll(inertia_rocket::ScrollPage::new(vec![1_u32, 2], 1).next(2)),
        )
}

#[rocket::post("/redirect")]
fn rocket_redirect() -> inertia_rocket::Redirect {
    inertia_rocket::Redirect::to("/page")
}

#[rocket::get("/external")]
fn rocket_external() -> inertia_rocket::Location {
    inertia_rocket::Location::external("https://example.com/outside")
}

#[rocket::post("/form", data = "<form>")]
fn rocket_form(
    form: inertia_rocket::InertiaForm<FormInput>,
) -> Result<inertia_rocket::Redirect, inertia_rocket::FormError> {
    form.validate_with(|input| {
        if input.title.trim().is_empty() {
            Err(inertia_rocket::Errors::field("title", "required"))
        } else {
            Ok(())
        }
    })?;
    Ok(inertia_rocket::Redirect::to("/page"))
}

#[rocket::post("/flash")]
fn rocket_flash() -> inertia_rocket::Redirect {
    inertia_rocket::Redirect::to("/page").flash("notice", "saved")
}

#[rocket::get("/ssr")]
fn rocket_ssr() -> inertia_rocket::DynamicPage {
    inertia_rocket::DynamicPage::new("Ssr")
}

#[rocket::get("/ssr-fallback")]
fn rocket_ssr_fallback() -> inertia_rocket::DynamicPage {
    inertia_rocket::DynamicPage::new("SsrFallback")
}

#[rocket::get("/health")]
fn rocket_health() -> &'static str {
    "healthy"
}

#[rocket::get("/missing")]
fn rocket_missing() -> inertia_rocket::DynamicPage {
    inertia_rocket::DynamicPage::new("Missing")
}

async fn rocket_harness(inertia: inertia_rocket::InertiaApp) -> RocketHarness {
    use rocket::local::asynchronous::Client;
    use std::rc::Rc;

    let installed = Client::untracked(
        rocket::build()
            .attach(inertia_rocket::InertiaFairing::new(inertia))
            .mount(
                "/",
                rocket::routes![
                    rocket_page,
                    rocket_redirect,
                    rocket_external,
                    rocket_form,
                    rocket_flash,
                    rocket_ssr,
                    rocket_ssr_fallback,
                    rocket_health,
                ],
            ),
    )
    .await
    .expect("Rocket conformance app must ignite");
    let uninstalled =
        Client::untracked(rocket::build().mount("/", rocket::routes![rocket_missing]))
            .await
            .expect("uninstalled Rocket conformance app must ignite");
    let installed = Rc::new(installed);
    let uninstalled = Rc::new(uninstalled);
    RocketHarness::new(move |request| {
        let installed = Rc::clone(&installed);
        let uninstalled = Rc::clone(&uninstalled);
        async move {
            let client = if request.uri.path() == "/missing" {
                uninstalled
            } else {
                installed
            };
            let method = request
                .method
                .as_str()
                .parse::<rocket::http::Method>()
                .expect("core test method must convert to Rocket");
            let mut native = client.req(method, request.uri.to_string());
            for (name, value) in &request.headers {
                native.add_header(rocket::http::Header::new(
                    name.as_str().to_owned(),
                    value
                        .to_str()
                        .expect("conformance request header must be UTF-8")
                        .to_owned(),
                ));
            }
            native.set_body(request.body);
            let response = native.dispatch().await;
            let status = http::StatusCode::from_u16(response.status().code)
                .expect("Rocket status must convert");
            let mut headers = http::HeaderMap::new();
            for header in response.headers().iter() {
                let name = http::HeaderName::from_bytes(header.name().as_str().as_bytes())
                    .expect("Rocket response header name must convert");
                let value = http::HeaderValue::from_bytes(header.value().as_bytes())
                    .expect("Rocket response header value must convert");
                headers.append(name, value);
            }
            let body = response
                .into_bytes()
                .await
                .map_or_else(Bytes::new, Bytes::from);
            AdapterResponse {
                status,
                headers,
                body,
            }
        }
    })
}

#[tokio::test]
async fn axum_passes_shared_adapter_conformance() {
    let (inertia, _ssr) = app_and_ssr().await;
    run_adapter_conformance(&axum_harness(inertia)).await;
}

#[tokio::test]
async fn actix_passes_shared_adapter_conformance() {
    let (inertia, _ssr) = app_and_ssr().await;
    let harness = ActixHarness::new(move |request| request_actix(inertia.clone(), request));
    run_adapter_conformance(&harness).await;
}

#[tokio::test]
async fn rocket_passes_shared_adapter_conformance() {
    let (inertia, _ssr) = app_and_ssr().await;
    run_adapter_conformance(&rocket_harness(inertia).await).await;
}
