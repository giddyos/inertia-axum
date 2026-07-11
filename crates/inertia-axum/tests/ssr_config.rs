#![cfg(feature = "ssr")]

use inertia_axum::InertiaApp;

#[test]
fn synchronous_build_rejects_configured_ssr() {
    let Err(error) = InertiaApp::default_root().ssr("dist/ssr/ssr.js").build() else {
        panic!("SSR must require asynchronous startup");
    };

    assert!(error.to_string().contains(".start()"));
    assert!(error.to_string().contains(".await"));
}
