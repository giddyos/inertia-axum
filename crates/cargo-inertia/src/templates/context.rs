//! Render context created exclusively from resolved initialization options.

use serde::Serialize;

use crate::{
    framework::Framework, init::options::InitOptions, templates::versions::TemplateVersions,
};

/// Values available to every embedded source template.
#[derive(Serialize)]
pub struct TemplateContext<'a> {
    pub package_name: &'a str,
    pub framework: FrameworkContext,
    pub ssr: SsrContext<'a>,
    pub versions: &'a TemplateVersions,
}
/// Framework-specific source details.
#[derive(Serialize)]
pub struct FrameworkContext {
    pub name: &'static str,
    pub page_extension: &'static str,
    pub check_script: &'static str,
}
/// SSR values exposed to templates without any CLI parsing state.
#[derive(Serialize)]
pub struct SsrContext<'a> {
    pub enabled: bool,
    pub host: &'a str,
    pub port: u16,
    pub bundle: &'a str,
}

impl<'a> TemplateContext<'a> {
    /// Builds a render context from resolved options.
    pub fn new(
        options: &'a InitOptions,
        package_name: &'a str,
        versions: &'a TemplateVersions,
    ) -> Self {
        let framework = match options.framework {
            Framework::React => FrameworkContext {
                name: "react",
                page_extension: "tsx",
                check_script: "tsc --noEmit",
            },
            Framework::Svelte => FrameworkContext {
                name: "svelte",
                page_extension: "svelte",
                check_script: "svelte-check",
            },
            Framework::Vue => FrameworkContext {
                name: "vue",
                page_extension: "vue",
                check_script: "vue-tsc --noEmit",
            },
        };
        Self {
            package_name,
            framework,
            ssr: SsrContext {
                enabled: options.ssr.is_enabled(),
                host: &options.ssr.host,
                port: options.ssr.port,
                bundle: options.ssr.bundle.to_str().unwrap_or("dist/ssr/main.js"),
            },
            versions,
        }
    }
}
