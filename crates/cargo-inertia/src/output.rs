//! Human-readable initialization output.

use crate::{
    framework::Framework,
    package_manager::PackageManager,
    ssr::{SsrBackend, SsrOptions},
};
use std::path::Path;

/// Formats concise post-generation next steps.
pub fn completion(
    framework: Framework,
    frontend: &Path,
    manager: PackageManager,
    installed: bool,
    ssr: &SsrOptions,
) -> String {
    let name = match framework {
        Framework::React => "React",
        Framework::Svelte => "Svelte",
        Framework::Vue => "Vue",
    };
    let mut message = if installed {
        format!(
            "Created {name} frontend in {}\nInstalled dependencies with {}.\n\nNext:\n  {} --dir {} run check\n  cargo inertia dev",
            frontend.display(),
            manager.executable(),
            manager.executable(),
            frontend.display()
        )
    } else {
        format!(
            "Created {name} frontend in {}\n\nNext:\n  cd {}\n  {} install\n  {} run check\n  {} run build\n  cd ..\n  cargo inertia dev",
            frontend.display(),
            frontend.display(),
            manager.executable(),
            manager.executable(),
            manager.executable()
        )
    };
    if matches!(ssr.backend, SsrBackend::ManagedNode) {
        message.push_str("\n\nEnable the inertia-axum SSR feature and configure Ssr::node(\"dist/ssr/main.js\").");
    }
    if matches!(ssr.backend, SsrBackend::ManagedNode) && matches!(manager, PackageManager::Bun) {
        message.push_str("\nBun will install dependencies and run frontend scripts. Managed SSR still requires Node 22.12 or newer at runtime.");
    }
    message
}
