//! Explicit source-to-destination template catalog.

use include_dir::{Dir, include_dir};

use crate::framework::Framework;

/// All embedded template files, rooted at the crate manifest directory.
pub static TEMPLATES: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/templates");

/// A condition controlling whether a catalogued template is emitted.
#[derive(Clone, Copy)]
pub enum TemplateCondition {
    /// Always render this template.
    Always,
}

/// One explicit embedded template and its relative output path.
#[derive(Clone, Copy)]
pub struct TemplateSpec {
    /// Embedded source path.
    pub source: &'static str,
    /// Relative generated destination.
    pub destination: &'static str,
    /// Inclusion condition.
    pub condition: TemplateCondition,
}

impl TemplateSpec {
    /// Creates an unconditional template entry.
    pub const fn always(source: &'static str, destination: &'static str) -> Self {
        Self {
            source,
            destination,
            condition: TemplateCondition::Always,
        }
    }
}

/// React scaffold files.
pub const REACT_TEMPLATES: &[TemplateSpec] = &[
    TemplateSpec::always("common/gitignore.j2", ".gitignore"),
    TemplateSpec::always("react/package.json.j2", "package.json"),
    TemplateSpec::always("react/tsconfig.json.j2", "tsconfig.json"),
    TemplateSpec::always("react/vite.config.ts.j2", "vite.config.ts"),
    TemplateSpec::always("react/src/main.ts.j2", "src/main.ts"),
    TemplateSpec::always("react/src/Pages/Home.tsx.j2", "src/Pages/Home.tsx"),
];
/// Svelte scaffold files.
pub const SVELTE_TEMPLATES: &[TemplateSpec] = &[
    TemplateSpec::always("common/gitignore.j2", ".gitignore"),
    TemplateSpec::always("svelte/package.json.j2", "package.json"),
    TemplateSpec::always("svelte/svelte.config.js.j2", "svelte.config.js"),
    TemplateSpec::always("svelte/tsconfig.json.j2", "tsconfig.json"),
    TemplateSpec::always("svelte/vite.config.ts.j2", "vite.config.ts"),
    TemplateSpec::always("svelte/src/main.ts.j2", "src/main.ts"),
    TemplateSpec::always("svelte/src/Pages/Home.svelte.j2", "src/Pages/Home.svelte"),
];
/// Vue scaffold files.
pub const VUE_TEMPLATES: &[TemplateSpec] = &[
    TemplateSpec::always("common/gitignore.j2", ".gitignore"),
    TemplateSpec::always("vue/package.json.j2", "package.json"),
    TemplateSpec::always("vue/tsconfig.json.j2", "tsconfig.json"),
    TemplateSpec::always("vue/vite.config.ts.j2", "vite.config.ts"),
    TemplateSpec::always("vue/src/main.ts.j2", "src/main.ts"),
    TemplateSpec::always("vue/src/Pages/Home.vue.j2", "src/Pages/Home.vue"),
];

/// Returns only the template specifications explicitly supported by a framework.
pub const fn for_framework(framework: Framework) -> &'static [TemplateSpec] {
    match framework {
        Framework::React => REACT_TEMPLATES,
        Framework::Svelte => SVELTE_TEMPLATES,
        Framework::Vue => VUE_TEMPLATES,
    }
}
