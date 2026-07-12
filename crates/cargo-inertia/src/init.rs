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
    let (name, extension, main, home) = match framework {
        Frontend::Svelte => ("svelte", "svelte", SVELTE_MAIN, SVELTE_HOME),
        Frontend::React => ("react", "tsx", REACT_MAIN, REACT_HOME),
        Frontend::Vue => ("vue", "vue", VUE_MAIN, VUE_HOME),
    };
    write(&frontend.join("package.json"), package_json(framework))?;
    if matches!(framework, Frontend::Svelte) {
        write(&frontend.join("svelte.config.js"), SVELTE_CONFIG)?;
        write(&frontend.join("tsconfig.json"), SVELTE_TSCONFIG)?;
    }
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
        "\nNext steps:\n  pnpm --dir frontend install\n  pnpm --dir frontend build\n  cargo run"
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn vite_config(framework: Frontend) -> String {
    let (import, plugin) = match framework {
        Frontend::Svelte => (
            "import { svelte } from '@sveltejs/vite-plugin-svelte'",
            "svelte({ prebundleSvelteLibraries: true })",
        ),
        Frontend::React => ("import react from '@vitejs/plugin-react'", "react()"),
        Frontend::Vue => ("import vue from '@vitejs/plugin-vue'", "vue()"),
    };
    format!(
        "{import}\nimport {{ defineConfig }} from 'vite'\n\nexport default defineConfig({{\n  plugins: [{plugin}],\n  build: {{\n    manifest: true,\n    rollupOptions: {{\n      input: 'src/main.ts',\n    }},\n  }},\n}})\n"
    )
}

fn package_json(framework: Frontend) -> &'static str {
    match framework {
        Frontend::Svelte => SVELTE_PACKAGE,
        Frontend::React => REACT_PACKAGE,
        Frontend::Vue => VUE_PACKAGE,
    }
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|error| error.to_string())
}

const SVELTE_PACKAGE: &str = r#"{
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build"
  },
  "dependencies": {
    "@inertiajs/svelte": "3.6.1"
  },
  "devDependencies": {
    "@sveltejs/vite-plugin-svelte": "7.1.2",
    "svelte": "5.55.7",
    "svelte-preprocess": "6.0.5",
    "typescript": "5.9.3",
    "vite": "8.1.4"
  }
}
"#;

const REACT_PACKAGE: &str = r#"{
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build"
  },
  "dependencies": {
    "@inertiajs/react": "3.6.1",
    "react": "19.2.4",
    "react-dom": "19.2.4"
  },
  "devDependencies": {
    "@vitejs/plugin-react": "6.0.0",
    "typescript": "5.9.3",
    "vite": "8.1.4"
  }
}
"#;

const VUE_PACKAGE: &str = r#"{
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build"
  },
  "dependencies": {
    "@inertiajs/vue3": "3.6.1",
    "vue": "3.5.29"
  },
  "devDependencies": {
    "@vitejs/plugin-vue": "6.0.5",
    "typescript": "5.9.3",
    "vite": "8.1.4"
  }
}
"#;

const SVELTE_MAIN: &str = r#"import { createInertiaApp } from '@inertiajs/svelte'
import { mount } from 'svelte'

createInertiaApp({
  resolve: (name) => import(`./Pages/${name}.svelte`),
  setup: ({ el, App, props }) => {
    mount(App, { target: el, props })
  },
})
"#;

const SVELTE_CONFIG: &str = r#"import preprocess from 'svelte-preprocess'

export default {
  preprocess: preprocess({ typescript: true }),
}
"#;

const SVELTE_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ESNext",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "verbatimModuleSyntax": true
  }
}
"#;

const SVELTE_HOME: &str = r#"<script lang="ts">
  import { Deferred } from '@inertiajs/svelte'

  type Stats = {
    projects: number
    tasks: number
  }

  let {
    greeting = 'Hello',
    stats,
  }: {
    greeting?: string
    stats?: Stats
  } = $props()
</script>

<main>
  <h1>{greeting} from inertia-axum</h1>

  <Deferred data="stats">
    {#snippet fallback()}
      <p>Loading stats…</p>
    {/snippet}

    {#if stats}
      <p>{stats.projects} projects · {stats.tasks} tasks</p>
    {/if}
  </Deferred>
</main>
"#;

const REACT_MAIN: &str = r#"import { createInertiaApp } from '@inertiajs/react'
import { createElement } from 'react'
import { createRoot } from 'react-dom/client'

createInertiaApp({
  resolve: (name) => import(`./Pages/${name}.tsx`),
  setup: ({ el, App, props }) => {
    createRoot(el).render(createElement(App, props))
  },
})
"#;

const REACT_HOME: &str = r#"import { Deferred } from '@inertiajs/react'

type Props = {
  greeting?: string
  stats?: {
    projects: number
    tasks: number
  }
}

export default function Home({ greeting = 'Hello', stats }: Props) {
  return (
    <main>
      <h1>{greeting} from inertia-axum</h1>

      <Deferred data="stats" fallback={<p>Loading stats…</p>}>
        {stats && (
          <p>
            {stats.projects} projects · {stats.tasks} tasks
          </p>
        )}
      </Deferred>
    </main>
  )
}
"#;

const VUE_MAIN: &str = r#"import { createInertiaApp } from '@inertiajs/vue3'
import { createApp, h } from 'vue'

createInertiaApp({
  resolve: (name) => import(`./Pages/${name}.vue`),
  setup: ({ el, App, props, plugin }) => {
    createApp({ render: () => h(App, props) })
      .use(plugin)
      .mount(el)
  },
})
"#;

const VUE_HOME: &str = r#"<script setup lang="ts">
import { Deferred } from '@inertiajs/vue3'

const props = withDefaults(
  defineProps<{
    greeting?: string
    stats?: {
      projects: number
      tasks: number
    }
  }>(),
  {
    greeting: 'Hello',
  },
)
</script>

<template>
  <main>
    <h1>{{ props.greeting }} from inertia-axum</h1>

    <Deferred data="stats">
      <template #fallback>
        <p>Loading stats…</p>
      </template>

      <p v-if="props.stats">
        {{ props.stats.projects }} projects ·
        {{ props.stats.tasks }} tasks
      </p>
    </Deferred>
  </main>
</template>
"#;

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
            let home = fs::read_to_string(&home_path).unwrap();
            assert!(home.contains("Deferred"));
            assert!(home.contains("stats"));
            assert_eq!(home, canonical_home(name));
            assert!(
                fs::read_to_string(root.join("frontend/vite.config.ts"))
                    .unwrap()
                    .contains("\n  build: {\n")
            );

            insta::assert_snapshot!(format!("{name}_package_json"), package);
            if matches!(framework, Frontend::Svelte) {
                insta::assert_snapshot!(
                    "svelte_config_js",
                    fs::read_to_string(root.join("frontend/svelte.config.js")).unwrap()
                );
                insta::assert_snapshot!(
                    "svelte_tsconfig_json",
                    fs::read_to_string(root.join("frontend/tsconfig.json")).unwrap()
                );
            }
            insta::assert_snapshot!(
                format!("{name}_vite_config_ts"),
                fs::read_to_string(root.join("frontend/vite.config.ts")).unwrap()
            );
            insta::assert_snapshot!(
                format!("{name}_main_ts"),
                fs::read_to_string(root.join("frontend/src/main.ts")).unwrap()
            );
            insta::assert_snapshot!(format!("{name}_home_{extension}"), home);
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

    fn canonical_home(framework: &str) -> String {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let relative = match framework {
            "svelte" => "docs/snippets/svelte/src/Pages/Home.svelte",
            "react" => "docs/snippets/react/src/Pages/Home.tsx",
            "vue" => "docs/snippets/vue/src/Pages/Home.vue",
            _ => unreachable!(),
        };
        fs::read_to_string(manifest.join("../..").join(relative)).unwrap()
    }
}
