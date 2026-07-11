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

async fn spawn_node(
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
            tracing::info!(target: "inertia_axum::ssr::node", pid, stream = "stdout", message = %line);
        }
    });
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            tracing::warn!(target: "inertia_axum::ssr::node", pid, stream = "stderr", message = %line);
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
    let mut last_error = None;
    loop {
        match client.health().await {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(SsrStartError::HealthTimeout { source: last_error });
        }
        tokio::time::sleep(DELAYS[attempt.min(DELAYS.len() - 1)]).await;
        attempt = attempt.saturating_add(1);
    }
}

async fn stop_child(child: &mut Child, client: &SsrClient) {
    let _ = client.shutdown().await;
    if tokio::time::timeout(std::time::Duration::from_secs(2), child.wait())
        .await
        .is_err()
    {
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}

pub(crate) async fn start_managed_node(
    config: Ssr,
    bundle: PathBuf,
    runtime: PathBuf,
    endpoint: String,
    working_directory: PathBuf,
) -> Result<SsrRuntime, SsrStartError> {
    verify_bundle(&bundle)?;
    let version = verify_node(&runtime).await?;
    tracing::info!(node = %format!("{}.{}.{}", version.major, version.minor, version.patch), bundle = %bundle.display(), "starting Inertia SSR server");
    let client = SsrClient::new(
        SsrEndpoints::node(&endpoint)?,
        config.timeout,
        config.max_concurrency,
        config.max_response_bytes,
    )?;
    let mut child = spawn_node(&runtime, &bundle, &working_directory).await?;
    let pid = child.id();
    forward_output(
        pid,
        child.stdout.take().expect("piped stdout exists"),
        child.stderr.take().expect("piped stderr exists"),
    );
    if let Err(error) = check_health_until_ready(&client, config.startup_timeout).await {
        let _ = child.start_kill();
        let _ = child.wait().await;
        return Err(error);
    }
    let (_, health) = tokio::sync::watch::channel(SsrHealth::Ready {
        backend: SsrBackendKind::ManagedNode,
    });
    let (lifecycle, mut lifecycle_rx) = tokio::sync::watch::channel(());
    let supervisor_client = client.clone();
    tokio::spawn(async move {
        let _ = lifecycle_rx.changed().await;
        stop_child(&mut child, &supervisor_client).await;
    });
    Ok(SsrRuntime {
        client,
        default: config.default,
        failure_mode: config.failure_mode,
        backend: SsrBackendKind::ManagedNode,
        health,
        lifecycle: Some(lifecycle),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
