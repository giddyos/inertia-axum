use super::runtime::SsrRuntime;
use super::{Ssr, SsrBackendKind, SsrClient, SsrEndpoints, SsrHealth, SsrStartError};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    io::{AsyncBufReadExt as _, BufReader},
    process::{Child, Command},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NodeVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

#[derive(Debug)]
pub(crate) struct ManagedNodeLaunchPaths {
    pub(crate) bundle: PathBuf,
    pub(crate) working_directory: PathBuf,
    pub(crate) runtime: PathBuf,
}

fn absolute_from(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

pub(crate) fn resolve_managed_paths(
    bundle: &Path,
    runtime: &Path,
    vite_root: Option<&Path>,
) -> Result<ManagedNodeLaunchPaths, SsrStartError> {
    let process_directory =
        std::env::current_dir().map_err(SsrStartError::CurrentDirectoryUnavailable)?;
    let vite_root = vite_root.map(|root| absolute_from(&process_directory, root));
    let unresolved_bundle = match &vite_root {
        Some(root) => absolute_from(root, bundle),
        None => absolute_from(&process_directory, bundle),
    };

    verify_bundle(&unresolved_bundle)?;
    let bundle =
        unresolved_bundle
            .canonicalize()
            .map_err(|source| SsrStartError::BundleUnavailable {
                bundle: unresolved_bundle,
                source,
            })?;
    let working_directory = match vite_root {
        Some(root) => {
            root.canonicalize()
                .map_err(|source| SsrStartError::WorkingDirectoryUnavailable {
                    directory: root,
                    source,
                })?
        }
        None => bundle.parent().unwrap_or(&process_directory).to_owned(),
    };
    let runtime = if runtime.components().count() > 1 && !runtime.is_absolute() {
        process_directory.join(runtime)
    } else {
        runtime.to_owned()
    };

    Ok(ManagedNodeLaunchPaths {
        bundle,
        working_directory,
        runtime,
    })
}

fn parse_node_version(value: &str) -> Result<NodeVersion, SsrStartError> {
    let value = value.trim().strip_prefix('v').unwrap_or(value.trim());
    let invalid = || SsrStartError::InvalidNodeVersion(value.to_owned());
    let mut parts = value.split('.');
    let major = parts
        .next()
        .ok_or_else(invalid)?
        .parse()
        .map_err(|_| invalid())?;
    let minor = parts.next().unwrap_or("0").parse().map_err(|_| invalid())?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .split('-')
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| invalid())?;
    Ok(NodeVersion {
        major,
        minor,
        patch,
    })
}

async fn verify_node(runtime: &Path) -> Result<NodeVersion, SsrStartError> {
    let output = Command::new(runtime)
        .arg("--version")
        .output()
        .await
        .map_err(|source| SsrStartError::NodeUnavailable {
            runtime: runtime.to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(SsrStartError::NodeVersionCommandFailed(output.status));
    }
    let value = String::from_utf8_lossy(&output.stdout);
    let version = parse_node_version(&value)?;
    if version.major < 22 {
        return Err(SsrStartError::UnsupportedNode {
            found: value.trim().to_owned(),
            required: 22,
        });
    }
    Ok(version)
}

pub(crate) fn verify_bundle(bundle: &Path) -> Result<(), SsrStartError> {
    let metadata =
        std::fs::metadata(bundle).map_err(|source| SsrStartError::BundleUnavailable {
            bundle: bundle.to_owned(),
            source,
        })?;
    if !metadata.is_file() {
        return Err(SsrStartError::BundleIsNotFile(bundle.to_owned()));
    }
    Ok(())
}

fn spawn_node(
    runtime: &Path,
    bundle: &Path,
    working_directory: &Path,
) -> Result<Child, SsrStartError> {
    let mut command = Command::new(runtime);
    command
        .arg(bundle)
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    command.spawn().map_err(|source| SsrStartError::NodeSpawn {
        runtime: runtime.to_owned(),
        bundle: bundle.to_owned(),
        source,
    })
}

fn forward_output(
    pid: Option<u32>,
    stdout: impl tokio::io::AsyncRead + Unpin + Send + 'static,
    stderr: impl tokio::io::AsyncRead + Unpin + Send + 'static,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::info!(target: "inertia_core::ssr::node", pid, stream = "stdout", message = %line);
        }
    });
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::warn!(target: "inertia_core::ssr::node", pid, stream = "stderr", message = %line);
        }
    });
}

pub(crate) async fn check_health_until_ready(
    client: &SsrClient,
    maximum: std::time::Duration,
) -> Result<(), SsrStartError> {
    const DELAYS: &[std::time::Duration] = &[
        std::time::Duration::from_millis(25),
        std::time::Duration::from_millis(50),
        std::time::Duration::from_millis(100),
        std::time::Duration::from_millis(250),
        std::time::Duration::from_millis(500),
    ];
    let deadline = tokio::time::Instant::now() + maximum;
    let mut attempt = 0usize;
    #[allow(unused_assignments)]
    let mut last_error = None;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(SsrStartError::HealthTimeout { source: last_error });
        }
        match tokio::time::timeout(remaining, client.health()).await {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => {
                return Err(SsrStartError::HealthTimeout {
                    source: Some(super::SsrFailure::Timeout),
                });
            }
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        tokio::time::sleep(DELAYS[attempt.min(DELAYS.len() - 1)].min(remaining)).await;
        attempt = attempt.saturating_add(1);
    }
}

async fn stop_child(child: &mut Child, client: &SsrClient, control_timeout: std::time::Duration) {
    let _ = tokio::time::timeout(control_timeout, client.shutdown()).await;
    if tokio::time::timeout(control_timeout, child.wait())
        .await
        .is_err()
    {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(control_timeout, child.wait()).await;
    }
}

#[derive(Clone)]
struct NodeLaunchConfig {
    runtime: PathBuf,
    bundle: PathBuf,
    working_directory: PathBuf,
    startup_timeout: std::time::Duration,
    control_timeout: std::time::Duration,
}

fn relaunch(launch: &NodeLaunchConfig) -> Result<Child, SsrStartError> {
    let mut child = spawn_node(&launch.runtime, &launch.bundle, &launch.working_directory)?;
    let pid = child.id();
    forward_output(
        pid,
        child.stdout.take().expect("piped stdout exists"),
        child.stderr.take().expect("piped stderr exists"),
    );
    Ok(child)
}

async fn supervise(
    mut child: Child,
    client: SsrClient,
    launch: NodeLaunchConfig,
    health: tokio::sync::watch::Sender<SsrHealth>,
    mut lifecycle: tokio::sync::watch::Receiver<()>,
) {
    const RESTART_DELAYS: &[std::time::Duration] = &[
        std::time::Duration::from_millis(100),
        std::time::Duration::from_millis(250),
        std::time::Duration::from_millis(500),
        std::time::Duration::from_secs(1),
        std::time::Duration::from_secs(2),
        std::time::Duration::from_secs(5),
    ];
    let mut restart_attempt = 0usize;
    loop {
        tokio::select! {
            shutdown = lifecycle.changed() => {
                if shutdown.is_err() { stop_child(&mut child, &client, launch.control_timeout).await; break; }
            }
            result = child.wait() => {
                tracing::error!(result = ?result, "Inertia SSR Node process exited");
                let _ = health.send(SsrHealth::Unavailable { backend: SsrBackendKind::ManagedNode });
                loop {
                    let delay = RESTART_DELAYS[restart_attempt.min(RESTART_DELAYS.len() - 1)];
                    restart_attempt = restart_attempt.saturating_add(1);
                    tokio::select! {
                        shutdown = lifecycle.changed() => { if shutdown.is_err() { return; } }
                        () = tokio::time::sleep(delay) => {}
                    }
                    let _ = health.send(SsrHealth::Starting { backend: SsrBackendKind::ManagedNode });
                    match relaunch(&launch) {
                        Ok(new_child) => {
                            child = new_child;
                            if check_health_until_ready(&client, launch.startup_timeout).await.is_ok() {
                                restart_attempt = 0;
                                let _ = health.send(SsrHealth::Ready { backend: SsrBackendKind::ManagedNode });
                                break;
                            }
                            let _ = child.start_kill(); let _ = child.wait().await;
                        }
                        Err(error) => tracing::error!(error = %error, "failed to restart SSR process"),
                    }
                }
            }
        }
    }
}

pub(crate) async fn start_managed_node(
    config: Ssr,
    paths: ManagedNodeLaunchPaths,
    endpoint: String,
) -> Result<SsrRuntime, SsrStartError> {
    let ManagedNodeLaunchPaths {
        bundle,
        runtime,
        working_directory,
    } = paths;
    let version = verify_node(&runtime).await?;
    tracing::info!(node = %format!("{}.{}.{}", version.major, version.minor, version.patch), bundle = %bundle.display(), "starting Inertia SSR server");
    let client = SsrClient::new(
        SsrEndpoints::node(&endpoint)?,
        config.timeout,
        config.control_timeout,
        config.max_concurrency,
        config.max_response_bytes,
    )?;
    let mut child = spawn_node(&runtime, &bundle, &working_directory)?;
    let pid = child.id();
    forward_output(
        pid,
        child.stdout.take().expect("piped stdout exists"),
        child.stderr.take().expect("piped stderr exists"),
    );
    tokio::select! {
        readiness = check_health_until_ready(&client, config.startup_timeout) => {
            if let Err(error) = readiness {
                let _ = child.start_kill();
                let _ = tokio::time::timeout(config.control_timeout, child.wait()).await;
                return Err(error);
            }
        }
        status = child.wait() => {
            return Err(SsrStartError::ProcessExitedDuringStartup {
                status: status.map_err(|source| SsrStartError::NodeWait { source })?,
            });
        }
    }
    let (health_tx, health) = tokio::sync::watch::channel(SsrHealth::Ready {
        backend: SsrBackendKind::ManagedNode,
    });
    let (lifecycle, lifecycle_rx) = tokio::sync::watch::channel(());
    tokio::spawn(supervise(
        child,
        client.clone(),
        NodeLaunchConfig {
            runtime,
            bundle,
            working_directory,
            startup_timeout: config.startup_timeout,
            control_timeout: config.control_timeout,
        },
        health_tx.clone(),
        lifecycle_rx,
    ));
    Ok(SsrRuntime {
        client,
        default: config.default,
        failure_mode: config.failure_mode,
        backend: SsrBackendKind::ManagedNode,
        health,
        health_tx,
        _lifecycle: Some(lifecycle),
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_server::{Request, Response, server};
    use super::*;
    use bytes::Bytes;
    use http::StatusCode;
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::current_dir()
                .unwrap()
                .join("target/ssr-path-tests")
                .join(format!(
                    "{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn bundle(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "export {};").unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn control_client<H, F>(handler: H, control_timeout: std::time::Duration) -> SsrClient
    where
        H: Fn(Request) -> F + Send + Sync + 'static,
        F: Future<Output = Response> + Send + 'static,
    {
        let base = server(handler).await;
        SsrClient::new(
            SsrEndpoints::node(&base).unwrap(),
            std::time::Duration::from_secs(1),
            control_timeout,
            1,
            1024,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn hanging_health_respects_overall_startup_timeout() {
        let client = control_client(
            |request| async move {
                assert_eq!(request.path, "/health");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                Response::ok(Bytes::new())
            },
            std::time::Duration::from_secs(1),
        )
        .await;
        let error = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            check_health_until_ready(&client, std::time::Duration::from_millis(25)),
        )
        .await
        .expect("startup deadline must remain bounded")
        .unwrap_err();
        assert!(matches!(
            error,
            SsrStartError::HealthTimeout {
                source: Some(super::super::SsrFailure::Timeout)
            }
        ));
    }

    #[tokio::test]
    async fn failed_health_status_retains_classified_source() {
        let client = control_client(
            |request| async move {
                assert_eq!(request.path, "/health");
                Response::status(StatusCode::SERVICE_UNAVAILABLE, Bytes::new())
            },
            std::time::Duration::from_millis(20),
        )
        .await;
        let error = check_health_until_ready(&client, std::time::Duration::from_millis(10))
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SsrStartError::HealthTimeout {
                source: Some(super::super::SsrFailure::Transport(message))
            } if message.contains("503")
        ));
    }

    #[test]
    fn resolves_absolute_bundle_without_vite_root() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("server.mjs");
        let paths = resolve_managed_paths(&bundle, Path::new("node"), None).unwrap();
        assert_eq!(paths.bundle, bundle.canonicalize().unwrap());
        assert_eq!(paths.working_directory, fixture.0.canonicalize().unwrap());
    }

    #[test]
    fn resolves_relative_bundle_without_vite_root() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("dist/server.mjs");
        let cwd = std::env::current_dir().unwrap();
        let relative = bundle.strip_prefix(&cwd).unwrap();
        let paths = resolve_managed_paths(relative, Path::new("node"), None).unwrap();
        assert_eq!(paths.bundle, bundle.canonicalize().unwrap());
        assert_eq!(paths.working_directory, bundle.parent().unwrap());
    }

    #[test]
    fn resolves_relative_vite_root_and_relative_bundle() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("frontend/dist/ssr/server.mjs");
        let cwd = std::env::current_dir().unwrap();
        let root = fixture.0.join("frontend");
        let paths = resolve_managed_paths(
            Path::new("dist/ssr/server.mjs"),
            Path::new("node"),
            Some(root.strip_prefix(cwd).unwrap()),
        )
        .unwrap();
        assert_eq!(paths.bundle, bundle.canonicalize().unwrap());
        assert_eq!(paths.working_directory, root.canonicalize().unwrap());
    }

    #[test]
    fn resolves_absolute_vite_root_and_relative_bundle() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("frontend/dist/ssr/server.mjs");
        let root = fixture.0.join("frontend");
        let paths = resolve_managed_paths(
            Path::new("dist/ssr/server.mjs"),
            Path::new("node"),
            Some(&root),
        )
        .unwrap();
        assert_eq!(paths.bundle, bundle.canonicalize().unwrap());
        assert_eq!(paths.working_directory, root.canonicalize().unwrap());
    }

    #[test]
    fn preserves_absolute_bundle_with_vite_root() {
        let fixture = Fixture::new();
        let root = fixture.0.join("frontend");
        std::fs::create_dir_all(&root).unwrap();
        let bundle = fixture.bundle("elsewhere/server.mjs");
        let paths = resolve_managed_paths(&bundle, Path::new("node"), Some(&root)).unwrap();
        assert_eq!(paths.bundle, bundle.canonicalize().unwrap());
        assert_eq!(paths.working_directory, root.canonicalize().unwrap());
    }

    #[test]
    fn resolves_relative_custom_runtime_against_process_directory() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("server.mjs");
        let paths = resolve_managed_paths(&bundle, Path::new("./tools/node"), None).unwrap();
        assert_eq!(
            paths.runtime,
            std::env::current_dir().unwrap().join("./tools/node")
        );
        assert!(paths.runtime.is_absolute());
    }

    #[test]
    fn leaves_bare_node_as_path_lookup() {
        let fixture = Fixture::new();
        let bundle = fixture.bundle("server.mjs");
        let paths = resolve_managed_paths(&bundle, Path::new("node"), None).unwrap();
        assert_eq!(paths.runtime, Path::new("node"));
    }

    #[test]
    fn missing_bundle_error_contains_resolved_path() {
        let fixture = Fixture::new();
        let cwd = std::env::current_dir().unwrap();
        let relative = fixture.0.strip_prefix(&cwd).unwrap().join("missing.mjs");
        let error = resolve_managed_paths(&relative, Path::new("node"), None).unwrap_err();
        assert!(matches!(
            error,
            SsrStartError::BundleUnavailable { bundle, .. }
                if bundle == cwd.join(relative)
        ));
    }

    #[test]
    fn rejects_directory_bundle() {
        let fixture = Fixture::new();
        let error = resolve_managed_paths(&fixture.0, Path::new("node"), None).unwrap_err();
        assert!(matches!(error, SsrStartError::BundleIsNotFile(bundle) if bundle == fixture.0));
    }

    #[test]
    fn parses_current_node_version() {
        assert_eq!(
            parse_node_version("v22.11.0").unwrap(),
            NodeVersion {
                major: 22,
                minor: 11,
                patch: 0
            }
        );
    }
    #[test]
    fn parses_prerelease_node_version() {
        assert_eq!(parse_node_version("v23.0.0-nightly").unwrap().patch, 0);
    }
    #[test]
    fn rejects_invalid_node_version() {
        assert!(matches!(
            parse_node_version("nope"),
            Err(SsrStartError::InvalidNodeVersion(_))
        ));
    }
    #[tokio::test]
    async fn rejects_node_before_version_22() {
        let dir = std::env::temp_dir().join(format!("inertia-node-old-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("node");
        std::fs::write(&script, "#!/bin/sh\necho v20.0.0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(matches!(
            verify_node(&script).await,
            Err(SsrStartError::UnsupportedNode { .. })
        ));
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn rejects_missing_and_directory_bundles() {
        let missing = std::env::temp_dir().join("inertia-missing-bundle.js");
        assert!(matches!(
            verify_bundle(&missing),
            Err(SsrStartError::BundleUnavailable { .. })
        ));
        assert!(matches!(
            verify_bundle(std::env::temp_dir().as_path()),
            Err(SsrStartError::BundleIsNotFile(_))
        ));
    }
}
