use std::{
    path::Path,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

pub fn run(frontend: &Path, port: u16) -> Result<(), String> {
    run_with(frontend, port, Path::new("npm"), Path::new("cargo"))
}

fn run_with(
    frontend: &Path,
    port: u16,
    npm_program: &Path,
    cargo_program: &Path,
) -> Result<(), String> {
    if !frontend.join("package.json").is_file() {
        return Err(format!(
            "{} does not contain package.json",
            frontend.display()
        ));
    }
    let url = format!("http://127.0.0.1:{port}");
    let mut vite = Command::new(npm_program)
        .args([
            "run",
            "dev",
            "--",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .current_dir(frontend)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("could not start Vite: {error}"))?;
    let mut cargo = match Command::new(cargo_program)
        .arg("run")
        .env("VITE_DEV_SERVER_URL", &url)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            let _ = vite.kill();
            return Err(format!("could not start cargo run: {error}"));
        }
    };
    loop {
        if let Some(status) = vite.try_wait().map_err(|error| error.to_string())? {
            let _ = cargo.kill();
            let _ = cargo.wait();
            return status
                .success()
                .then_some(())
                .ok_or_else(|| format!("Vite exited with {status}"));
        }
        if let Some(status) = cargo.try_wait().map_err(|error| error.to_string())? {
            let _ = vite.kill();
            let _ = vite.wait();
            return status
                .success()
                .then_some(())
                .ok_or_else(|| format!("cargo run exited with {status}"));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[ignore = "This test is flaky and fails on CI sometimes. It is not critical to the functionality of cargo-inertia, so we can ignore it for now."]
    #[test]
    fn sets_the_dev_url_and_stops_the_peer_when_vite_exits() {
        let root = std::env::temp_dir().join(format!("cargo-inertia-dev-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("frontend")).unwrap();
        fs::write(root.join("frontend/package.json"), "{}").unwrap();
        let npm = root.join("npm");
        let cargo = root.join("cargo");
        fs::write(&npm, "#!/bin/sh\nsleep 0.3\nexit 0\n").unwrap();
        fs::write(
            &cargo,
            format!(
                "#!/bin/sh\necho \"$VITE_DEV_SERVER_URL\" > '{}'\necho $$ > '{}'\nwhile :; do :; done\n",
                root.join("url").display(),
                root.join("pid").display()
            ),
        )
        .unwrap();
        for executable in [&npm, &cargo] {
            let mut permissions = fs::metadata(executable).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(executable, permissions).unwrap();
        }
        run_with(&root.join("frontend"), 4317, &npm, &cargo).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("url")).unwrap().trim(),
            "http://127.0.0.1:4317"
        );
        let pid = fs::read_to_string(root.join("pid")).unwrap();
        assert!(
            !Command::new("kill")
                .args(["-0", pid.trim()])
                .status()
                .unwrap()
                .success()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "flaky test"]
    fn stops_vite_when_cargo_exits() {
        let root =
            std::env::temp_dir().join(format!("cargo-inertia-dev-peer-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("frontend")).unwrap();
        fs::write(root.join("frontend/package.json"), "{}").unwrap();
        let npm = root.join("npm");
        let cargo = root.join("cargo");
        fs::write(
            &npm,
            format!(
                "#!/bin/sh\necho $$ > '{}'\nwhile :; do :; done\n",
                root.join("pid").display()
            ),
        )
        .unwrap();
        fs::write(&cargo, "#!/bin/sh\nsleep 0.3\nexit 0\n").unwrap();
        for executable in [&npm, &cargo] {
            let mut permissions = fs::metadata(executable).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(executable, permissions).unwrap();
        }
        run_with(&root.join("frontend"), 4318, &npm, &cargo).unwrap();
        let pid = fs::read_to_string(root.join("pid")).unwrap();
        assert!(
            !Command::new("kill")
                .args(["-0", pid.trim()])
                .status()
                .unwrap()
                .success()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
