#![cfg(all(feature = "ssr", feature = "live-ssr"))]
#![allow(missing_docs)]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    routing::get,
};
use inertia_axum::{DynamicPage, InertiaApp, RouterInertiaExt as _, Ssr};
use std::time::Duration;
use tower::ServiceExt as _;

#[tokio::test]
#[ignore = "requires Node 22 or newer"]
async fn managed_node_launches_relative_bundle_from_relative_vite_root() {
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);

    let workspace = std::env::current_dir().unwrap();
    let frontend = std::path::PathBuf::from("target/ssr-fixtures").join(format!(
        "relative-vite-{}-{port}/frontend",
        std::process::id()
    ));
    let absolute_frontend = workspace.join(&frontend);
    let build = absolute_frontend.join("dist");
    std::fs::create_dir_all(build.join(".vite")).unwrap();
    std::fs::create_dir_all(build.join("ssr")).unwrap();
    std::fs::write(
        build.join(".vite/manifest.json"),
        r#"{"src/main.js":{"file":"client.js","isEntry":true}}"#,
    )
    .unwrap();
    std::fs::write(build.join("client.js"), "export {};").unwrap();
    std::fs::write(
        build.join("ssr/server.mjs"),
        format!(
            r#"
import http from 'node:http';
const server = http.createServer((req, res) => {{
  if (req.url === '/health') return res.end('ok');
  if (req.url === '/shutdown') {{ res.end('ok'); return server.close(); }}
  if (req.url === '/render') {{
    res.setHeader('content-type', 'application/json');
    return res.end(JSON.stringify({{head:[],body:'<div id="app" data-server-rendered="true">relative</div>'}}));
  }}
  res.statusCode = 404; res.end();
}});
server.listen({port}, '127.0.0.1');
"#
        ),
    )
    .unwrap();

    let inertia = InertiaApp::vite(&frontend)
        .entry("src/main.js")
        .ssr(Ssr::node("dist/ssr/server.mjs").endpoint(format!("http://127.0.0.1:{port}")))
        .start()
        .await
        .unwrap();
    let app = Router::new()
        .route("/", get(|| async { DynamicPage::new("Home") }))
        .inertia(inertia);
    let response = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("data-server-rendered=\"true\">relative"));

    let _ = std::fs::remove_dir_all(workspace.join(frontend).parent().unwrap());
}

#[tokio::test]
async fn managed_node_starts_renders_and_shuts_down() {
    let version = tokio::process::Command::new("node")
        .arg("--version")
        .output()
        .await;
    let Ok(version) = version else { return };
    let major = String::from_utf8_lossy(&version.stdout)
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    if major < 22 {
        return;
    }

    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let root = std::env::temp_dir().join(format!(
        "inertia-managed-node-{}-{port}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let bundle = root.join("server.mjs");
    let stopped = root.join("stopped");
    let source = format!(
        r#"
import http from 'node:http';
import fs from 'node:fs';
const server = http.createServer((req, res) => {{
  if (req.url === '/health') {{ res.end('ok'); return; }}
  if (req.url === '/shutdown') {{
    fs.writeFileSync({stopped:?}, 'stopped');
    res.end('ok');
    server.close();
    return;
  }}
  if (req.url === '/render') {{
    let body = '';
    req.on('data', chunk => body += chunk);
    req.on('end', () => {{
      res.setHeader('content-type', 'application/json');
      res.end(JSON.stringify({{ head: ['<title>Managed</title>'], body: '<div id="app" data-server-rendered="true">managed</div>' }}));
    }});
    return;
  }}
  res.statusCode = 404; res.end();
}});
server.listen({port}, '127.0.0.1');
"#,
        stopped = stopped.to_string_lossy()
    );
    std::fs::write(&bundle, source).unwrap();

    let inertia = InertiaApp::default_root()
        .ssr(Ssr::node(&bundle).endpoint(format!("http://127.0.0.1:{port}")))
        .start()
        .await
        .unwrap();
    let app = Router::new()
        .route("/", get(|| async { DynamicPage::new("Home") }))
        .inertia(inertia);
    let response = app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("data-server-rendered"));
    drop(app);

    tokio::time::timeout(Duration::from_secs(5), async {
        while !stopped.exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn supervisor_restarts_managed_process_once_and_restores_health() {
    use inertia_axum::{SsrBackendKind, SsrHealth};
    let Ok(version) = tokio::process::Command::new("node")
        .arg("--version")
        .output()
        .await
    else {
        return;
    };
    if String::from_utf8_lossy(&version.stdout)
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap()
        < 22
    {
        return;
    }
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let root = std::env::temp_dir().join(format!(
        "inertia-restart-node-{}-{port}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let bundle = root.join("server.mjs");
    let launches = root.join("launches");
    let source = format!(
        r#"
import http from 'node:http'; import fs from 'node:fs';
const file = {launches:?}; const count = fs.existsSync(file) ? Number(fs.readFileSync(file)) + 1 : 1; fs.writeFileSync(file, String(count));
const server = http.createServer((req,res) => {{
 if(req.url==='/health') return res.end('ok');
 if(req.url==='/shutdown') {{ res.end('ok'); return server.close(); }}
 if(req.url==='/render') return res.end(JSON.stringify({{head:[],body:'<div id="app">restarted</div>'}}));
 res.statusCode=404; res.end();
}}); server.listen({port}, '127.0.0.1', () => {{ if(count===1) setTimeout(() => process.exit(17), 150); }});
"#,
        launches = launches.to_string_lossy()
    );
    std::fs::write(&bundle, source).unwrap();
    let inertia = InertiaApp::default_root()
        .ssr(Ssr::node(&bundle).endpoint(format!("http://127.0.0.1:{port}")))
        .start()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(&launches).ok().as_deref() == Some("2")
                && inertia.ssr_health()
                    == (SsrHealth::Ready {
                        backend: SsrBackendKind::ManagedNode,
                    })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&launches).unwrap(), "2");
    drop(inertia);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn official_vite_plugin_bundle_starts_and_renders() {
    let bundle = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/axum-svelte/svelte-app/dist/ssr/app.js");
    assert!(bundle.is_file());
    let inertia = InertiaApp::default_root()
        .ssr(Ssr::node(&bundle))
        .start()
        .await
        .unwrap();
    let app = Router::new()
        .route(
            "/todos",
            get(|| async {
                DynamicPage::new("Todos/Index").prop("todos", Vec::<serde_json::Value>::new())
            }),
        )
        .inertia(inertia);
    let response = app
        .oneshot(Request::get("/todos").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let html = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("data-server-rendered=\"true\""));
    assert!(html.contains("<h1>Todos</h1>"));
}
