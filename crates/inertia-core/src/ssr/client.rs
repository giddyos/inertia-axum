use super::{SsrEndpoints, SsrFailure, SsrResponse, SsrStartError, WireSsrResponse};
use bytes::Bytes;
use http_body_util::{BodyExt as _, Full, Limited};
use hyper::body::Incoming;
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use std::sync::{Arc, Mutex};
use tower::{ServiceBuilder, ServiceExt as _, util::BoxCloneService};

type RequestBody = Full<Bytes>;
type HttpClient = Client<HttpConnector, RequestBody>;
type RenderService =
    BoxCloneService<http::Request<RequestBody>, http::Response<Incoming>, tower::BoxError>;

#[derive(Clone)]
pub(crate) struct SsrClient {
    endpoints: SsrEndpoints,
    render: Arc<Mutex<RenderService>>,
    raw: HttpClient,
    control_timeout: std::time::Duration,
    max_response_bytes: usize,
}

impl SsrClient {
    pub(crate) fn new(
        endpoints: SsrEndpoints,
        timeout: std::time::Duration,
        control_timeout: std::time::Duration,
        max_concurrency: usize,
        max_response_bytes: usize,
    ) -> Result<Self, SsrStartError> {
        if timeout.is_zero() {
            return Err(SsrStartError::InvalidTimeout);
        }
        if control_timeout.is_zero() {
            return Err(SsrStartError::InvalidControlTimeout);
        }
        if max_concurrency == 0 {
            return Err(SsrStartError::InvalidConcurrency);
        }
        if max_response_bytes == 0 {
            return Err(SsrStartError::InvalidResponseLimit);
        }

        let mut connector = HttpConnector::new();
        connector.enforce_http(true);
        connector.set_nodelay(true);
        connector.set_keepalive(Some(std::time::Duration::from_secs(60)));
        let raw = Client::builder(TokioExecutor::new())
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(max_concurrency)
            .build(connector);
        let render = ServiceBuilder::new()
            .load_shed()
            .concurrency_limit(max_concurrency)
            .timeout(timeout)
            .service(raw.clone());
        Ok(Self {
            endpoints,
            render: Arc::new(Mutex::new(BoxCloneService::new(render))),
            raw,
            control_timeout,
            max_response_bytes,
        })
    }

    pub(crate) async fn render(&self, page: Bytes) -> Result<Option<SsrResponse>, SsrFailure> {
        let render = self
            .render
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let endpoint = self.endpoints.render.clone();
        let max_response_bytes = self.max_response_bytes;
        let request = http::Request::post(endpoint)
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(http::header::CONTENT_LENGTH, page.len())
            .body(Full::new(page))
            .map_err(SsrFailure::request)?;
        let response = render.oneshot(request).await.map_err(SsrFailure::service)?;
        let status = response.status();
        let body = Limited::new(response.into_body(), max_response_bytes)
            .collect()
            .await
            .map_err(SsrFailure::response_body)?
            .to_bytes();
        if !status.is_success() {
            return Err(SsrFailure::render_response(status, body));
        }
        let response: Option<WireSsrResponse> =
            serde_json::from_slice(&body).map_err(SsrFailure::invalid_response)?;
        Ok(response.map(Into::into))
    }

    pub(crate) async fn health(&self) -> Result<(), SsrFailure> {
        let Some(uri) = self.endpoints.health.clone() else {
            return Ok(());
        };
        let request = http::Request::get(uri)
            .body(Full::new(Bytes::new()))
            .map_err(SsrFailure::request)?;
        let response = self.control_request(request).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(SsrFailure::health(response.status()))
        }
    }

    pub(crate) async fn shutdown(&self) -> Result<(), SsrFailure> {
        let Some(uri) = self.endpoints.shutdown.clone() else {
            return Ok(());
        };
        let request = http::Request::get(uri)
            .body(Full::new(Bytes::new()))
            .map_err(SsrFailure::request)?;
        let response = self.control_request(request).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(SsrFailure::shutdown(response.status()))
        }
    }

    async fn control_request(
        &self,
        request: http::Request<RequestBody>,
    ) -> Result<http::Response<Incoming>, SsrFailure> {
        tokio::time::timeout(self.control_timeout, self.raw.clone().oneshot(request))
            .await
            .map_err(|_| SsrFailure::Timeout)?
            .map_err(SsrFailure::transport)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_server::{Request, Response, server};
    use super::*;
    use http::StatusCode;
    use std::{
        collections::HashSet,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    use tokio::sync::Notify;

    fn client(base: &str, timeout: Duration, concurrency: usize, limit: usize) -> SsrClient {
        SsrClient::new(
            SsrEndpoints::node(base).unwrap(),
            timeout,
            timeout,
            concurrency,
            limit,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn render_sends_existing_page_bytes_and_parses_typed_head_and_body() {
        let received = Arc::new(Mutex::new(None));
        let capture = received.clone();
        let base = server(move |request: Request| {
            let capture = capture.clone();
            async move {
                assert_eq!(request.path, "/render");
                *capture.lock().unwrap() = Some(request.body);
                Response::ok(Bytes::from_static(
                    br#"{"head":["<title>SSR</title>"],"body":"<div>rendered</div>"}"#,
                ))
            }
        })
        .await;
        let page = Bytes::from_static(br#"{"component":"Home"}"#);
        let response = client(&base, Duration::from_secs(1), 2, 1024)
            .render(page.clone())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(received.lock().unwrap().as_ref(), Some(&page));
        assert_eq!(response.head, ["<title>SSR</title>"]);
        assert_eq!(response.body, "<div>rendered</div>");
    }

    #[tokio::test]
    async fn vite_null_response_returns_none() {
        let base = server(|request| async move {
            assert_eq!(request.path, "/__inertia_ssr");
            Response::ok(Bytes::from_static(b"null"))
        })
        .await;
        let client = SsrClient::new(
            SsrEndpoints::vite(&base).unwrap(),
            Duration::from_secs(1),
            Duration::from_secs(1),
            1,
            100,
        )
        .unwrap();
        assert!(
            client
                .render(Bytes::from_static(b"{}"))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn non_success_status_becomes_render_failure_and_is_not_retried() {
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let base = server(move |request| {
            let count = count.clone();
            async move {
                assert_eq!(request.path, "/render");
                count.fetch_add(1, Ordering::SeqCst);
                Response::status(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Bytes::from_static(b"broken"),
                )
            }
        })
        .await;
        let error = client(&base, Duration::from_secs(1), 1, 1024)
            .render(Bytes::new())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SsrFailure::Render {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                ..
            }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn response_body_limit_is_enforced() {
        let base = server(|_| async { Response::ok(Bytes::from(vec![b'x'; 128])) }).await;
        let error = client(&base, Duration::from_secs(1), 1, 16)
            .render(Bytes::new())
            .await
            .unwrap_err();
        assert!(matches!(error, SsrFailure::ResponseTooLarge));
    }

    #[tokio::test]
    async fn render_timeout_is_enforced() {
        let base = server(|_| async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Response::ok(Bytes::from_static(b"null"))
        })
        .await;
        let error = client(&base, Duration::from_millis(10), 1, 100)
            .render(Bytes::new())
            .await
            .unwrap_err();
        assert!(matches!(error, SsrFailure::Timeout));
    }

    #[tokio::test]
    async fn saturated_concurrency_limit_load_sheds() {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let base = server({
            let entered = entered.clone();
            let release = release.clone();
            move |_| {
                let entered = entered.clone();
                let release = release.clone();
                async move {
                    entered.notify_one();
                    release.notified().await;
                    Response::ok(Bytes::from_static(b"null"))
                }
            }
        })
        .await;
        let client = client(&base, Duration::from_secs(1), 1, 100);
        let first = tokio::spawn({
            let client = client.clone();
            async move { client.render(Bytes::new()).await }
        });
        entered.notified().await;
        let second = client.render(Bytes::new()).await;
        assert!(matches!(second, Err(SsrFailure::Overloaded)));
        release.notify_one();
        assert!(first.await.unwrap().unwrap().is_none());
    }

    #[tokio::test]
    async fn sequential_requests_reuse_the_client_pool() {
        let peers = Arc::new(Mutex::new(HashSet::new()));
        let seen = peers.clone();
        let base = server(move |request| {
            seen.lock().unwrap().insert(request.peer);
            async { Response::ok(Bytes::from_static(b"null")) }
        })
        .await;
        let client = client(&base, Duration::from_secs(1), 2, 100);
        client.render(Bytes::new()).await.unwrap();
        client.render(Bytes::new()).await.unwrap();
        assert_eq!(peers.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn health_and_shutdown_use_unbounded_control_requests() {
        let counts = Arc::new(AtomicUsize::new(0));
        let count = counts.clone();
        let base = server(move |request| {
            let count = count.clone();
            async move {
                match request.path.as_str() {
                    "/health" => Response::ok(Bytes::new()),
                    "/shutdown" => {
                        count.fetch_add(1, Ordering::SeqCst);
                        Response::ok(Bytes::new())
                    }
                    _ => Response::status(StatusCode::NOT_FOUND, Bytes::new()),
                }
            }
        })
        .await;
        let client = client(&base, Duration::from_secs(1), 1, 100);
        client.health().await.unwrap();
        client.shutdown().await.unwrap();
        assert_eq!(counts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn endpoint_and_limit_validation_is_actionable() {
        assert!(matches!(
            SsrEndpoints::node("https://example.com"),
            Err(SsrStartError::UnsupportedEndpoint(_))
        ));
        assert!(SsrEndpoints::node("relative").is_err());
        let endpoints = SsrEndpoints::node("http://127.0.0.1:13714/").unwrap();
        assert_eq!(endpoints.render, "http://127.0.0.1:13714/render");
        assert!(matches!(
            SsrClient::new(
                endpoints.clone(),
                Duration::ZERO,
                Duration::from_secs(1),
                1,
                1,
            ),
            Err(SsrStartError::InvalidTimeout)
        ));
        assert!(matches!(
            SsrClient::new(
                endpoints.clone(),
                Duration::from_secs(1),
                Duration::ZERO,
                1,
                1,
            ),
            Err(SsrStartError::InvalidControlTimeout)
        ));
        assert!(matches!(
            SsrClient::new(
                endpoints.clone(),
                Duration::from_secs(1),
                Duration::from_secs(1),
                0,
                1,
            ),
            Err(SsrStartError::InvalidConcurrency)
        ));
        assert!(matches!(
            SsrClient::new(
                endpoints,
                Duration::from_secs(1),
                Duration::from_secs(1),
                1,
                0,
            ),
            Err(SsrStartError::InvalidResponseLimit)
        ));
    }
}
