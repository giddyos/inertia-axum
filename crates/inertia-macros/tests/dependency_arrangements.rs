#![allow(missing_docs)]

use std::{path::Path, process::Command};

fn check_fixture(name: &str) {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("Cargo.toml");
    let status = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(&manifest)
        .status()
        .expect("fixture cargo check must start");
    assert!(status.success(), "{name} dependency arrangement failed");
}

#[test]
fn derives_compile_with_direct_core_only() {
    check_fixture("core-only");
}

#[test]
fn derives_compile_with_axum_adapter_only() {
    check_fixture("axum-only");
}
