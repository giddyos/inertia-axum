use crate::Frontend;
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub fn run(root: &Path, framework: Frontend) -> Result<(), String> {
    run_with_writer(root, framework, &mut io::stdout().lock())
}

fn run_with_writer(
    root: &Path,
    framework: Frontend,
    output: &mut impl Write,
) -> Result<(), String> {
    let frontend = root.join("frontend");
    if frontend.exists() {
        return Err(format!("{} already exists", frontend.display()));
    }
    fs::create_dir_all(frontend.join("src/Pages")).map_err(|error| error.to_string())?;
    let (plugin_dependency, name, dependencies, extension, main, home) = match framework {
        Frontend::Svelte => (
            "@sveltejs/vite-plugin-svelte",
            "svelte",
            r#""@inertiajs/svelte": "^3.0.0", "svelte": "latest""#,
            "svelte",
            SVELTE_MAIN,
            SVELTE_HOME,
        ),
        Frontend::React => (
            "@vitejs/plugin-react",
            "react",
            r#""@inertiajs/react": "^3.0.0", "react": "latest", "react-dom": "latest""#,
            "tsx",
            REACT_MAIN,
            REACT_HOME,
        ),
        Frontend::Vue => (
            "@vitejs/plugin-vue",
            "vue",
            r#""@inertiajs/vue3": "^3.0.0", "vue": "latest""#,
            "vue",
            VUE_MAIN,
            VUE_HOME,
        ),
    };
    let package = format!(
        r#"{{
  "private": true,
  "type": "module",
  "scripts": {{ "dev": "vite", "build": "vite build" }},
  "dependencies": {{ {dependencies} }},
  "devDependencies": {{ "vite": "latest", "{plugin_dependency}": "latest", "typescript": "latest" }}
}}
"#
    );
    write(&frontend.join("package.json"), &package)?;
    write(&frontend.join("vite.config.ts"), &vite_config(framework))?;
    write(&frontend.join("src/main.ts"), main)?;
    write(&frontend.join(format!("src/Pages/Home.{extension}")), home)?;
    writeln!(
        output,
        "Created {} frontend in {}",
        name,
        frontend.display()
    )
    .map_err(|error| error.to_string())?;
    writeln!(
        output,
        "\nRust setup:\n\n.inertia(InertiaApp::vite(\"frontend\").build()?)"
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn vite_config(framework: Frontend) -> String {
    let (import, plugin) = match framework {
        Frontend::Svelte => (
            "import { svelte } from '@sveltejs/vite-plugin-svelte';",
            "svelte()",
        ),
        Frontend::React => ("import react from '@vitejs/plugin-react';", "react()"),
        Frontend::Vue => ("import vue from '@vitejs/plugin-vue';", "vue()"),
    };
    format!(
        "import {{ defineConfig }} from 'vite';\n{import}\n\nexport default defineConfig({{ plugins: [{plugin}], build: {{ manifest: true, rollupOptions: {{ input: 'src/main.ts' }} }} }});\n"
    )
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|error| error.to_string())
}

const SVELTE_MAIN: &str = "import { createInertiaApp } from '@inertiajs/svelte';\nimport { mount } from 'svelte';\ncreateInertiaApp({ resolve: name => import(`./Pages/${name}.svelte`), setup: ({ el, App, props }) => mount(App, { target: el, props }) });\n";
const SVELTE_HOME: &str = "<script lang=\"ts\">let { greeting = 'Hello' } = $props();</script>\n<h1>{greeting} from inertia-axum</h1>\n";
const REACT_MAIN: &str = "import { createElement } from 'react';\nimport { createInertiaApp } from '@inertiajs/react';\nimport { createRoot } from 'react-dom/client';\ncreateInertiaApp({ resolve: name => import(`./Pages/${name}.tsx`), setup: ({ el, App, props }) => createRoot(el).render(createElement(App, props)) });\n";
const REACT_HOME: &str = "export default function Home({ greeting = 'Hello' }) { return <h1>{greeting} from inertia-axum</h1>; }\n";
const VUE_MAIN: &str = "import { createApp, h } from 'vue';\nimport { createInertiaApp } from '@inertiajs/vue3';\ncreateInertiaApp({ resolve: name => import(`./Pages/${name}.vue`), setup: ({ el, App, props, plugin }) => createApp({ render: () => h(App, props) }).use(plugin).mount(el) });\n";
const VUE_HOME: &str = "<script setup lang=\"ts\">withDefaults(defineProps<{ greeting?: string }>(), { greeting: 'Hello' });</script>\n<template><h1>{{ greeting }} from inertia-axum</h1></template>\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_each_supported_framework_skeleton() {
        for (name, framework, extension, adapter) in [
            ("svelte", Frontend::Svelte, "svelte", "@inertiajs/svelte"),
            ("react", Frontend::React, "tsx", "@inertiajs/react"),
            ("vue", Frontend::Vue, "vue", "@inertiajs/vue3"),
        ] {
            let root = std::env::temp_dir()
                .join(format!("cargo-inertia-init-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).unwrap();
            let mut output = Vec::new();
            run_with_writer(&root, framework, &mut output).unwrap();
            assert!(root.join("frontend/package.json").is_file());
            assert!(root.join("frontend/vite.config.ts").is_file());
            assert!(root.join("frontend/src/main.ts").is_file());
            let package = fs::read_to_string(root.join("frontend/package.json")).unwrap();
            assert!(package.contains(adapter));
            let home_path = root.join(format!("frontend/src/Pages/Home.{extension}"));
            assert!(home_path.is_file());

            insta::assert_snapshot!(format!("{name}_package_json"), package);
            insta::assert_snapshot!(
                format!("{name}_vite_config_ts"),
                fs::read_to_string(root.join("frontend/vite.config.ts")).unwrap()
            );
            insta::assert_snapshot!(
                format!("{name}_main_ts"),
                fs::read_to_string(root.join("frontend/src/main.ts")).unwrap()
            );
            insta::assert_snapshot!(
                format!("{name}_home_{extension}"),
                fs::read_to_string(home_path).unwrap()
            );
            insta::assert_snapshot!(
                format!("{name}_completion_output"),
                String::from_utf8(output)
                    .unwrap()
                    .replace(root.to_str().unwrap(), "[ROOT]")
            );

            if matches!(framework, Frontend::React) {
                assert!(root.join("frontend/src/Pages/Home.tsx").is_file());
                assert!(!root.join("frontend/src/Pages/Home.jsx").exists());
            }
            assert!(
                run_with_writer(&root, framework, &mut Vec::new())
                    .unwrap_err()
                    .contains("already exists")
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}
