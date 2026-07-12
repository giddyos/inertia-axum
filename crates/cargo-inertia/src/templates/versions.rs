//! Centralized, deterministic frontend package versions.

/// Versions referenced by all embedded templates.
#[derive(serde::Serialize)]
pub struct TemplateVersions {
    pub inertia: &'static str,
    pub vite: &'static str,
    pub typescript: &'static str,
    pub react: &'static str,
    pub react_dom: &'static str,
    pub react_types: &'static str,
    pub react_dom_types: &'static str,
    pub vite_plugin_react: &'static str,
    pub svelte: &'static str,
    pub svelte_check: &'static str,
    pub vite_plugin_svelte: &'static str,
    pub vue: &'static str,
    pub vue_tsc: &'static str,
    pub vite_plugin_vue: &'static str,
}
/// The pinned versions used by generated projects.
pub const VERSIONS: TemplateVersions = TemplateVersions {
    inertia: "3.6.1",
    vite: "8.1.4",
    typescript: "5.9.3",
    react: "19.2.4",
    react_dom: "19.2.4",
    react_types: "19.2.2",
    react_dom_types: "19.2.2",
    vite_plugin_react: "6.0.0",
    svelte: "5.55.7",
    svelte_check: "4.3.4",
    vite_plugin_svelte: "7.1.2",
    vue: "3.5.29",
    vue_tsc: "3.1.5",
    vite_plugin_vue: "6.0.5",
};
