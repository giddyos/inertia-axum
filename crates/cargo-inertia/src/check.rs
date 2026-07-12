pub mod args;

use cargo_metadata::MetadataCommand;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};
use syn::{Attribute, Expr, LitStr, Token, visit::Visit};

pub fn run_args(args: args::CheckArgs) -> Result<(), String> {
    let frontend = args.path.join(&args.frontend);
    let package = frontend.join("package.json");
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(&package)
            .map_err(|_| format!("package.json {} does not exist", package.display()))?,
    )
    .map_err(|error| format!("invalid package.json {}: {error}", package.display()))?;
    if value.get("private").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(format!(
            "package.json {} must set private to true",
            package.display()
        ));
    }
    let groups = [value.get("dependencies"), value.get("devDependencies")];
    let present = |name: &str| {
        groups
            .iter()
            .any(|group| group.and_then(|v| v.get(name)).is_some())
    };
    let adapters = [
        (
            crate::framework::Framework::React,
            "@inertiajs/react",
            "@vitejs/plugin-react",
        ),
        (
            crate::framework::Framework::Svelte,
            "@inertiajs/svelte",
            "@sveltejs/vite-plugin-svelte",
        ),
        (
            crate::framework::Framework::Vue,
            "@inertiajs/vue3",
            "@vitejs/plugin-vue",
        ),
    ]
    .into_iter()
    .filter(|(_, adapter, _)| present(adapter))
    .collect::<Vec<_>>();
    if adapters.len() != 1 {
        return Err(
            "package.json must contain exactly one supported Inertia framework adapter".into(),
        );
    }
    if let Some(framework) = args.framework.explicit() {
        if framework != adapters[0].0 {
            return Err("--framework conflicts with package.json dependencies".into());
        }
    }
    if !present("@inertiajs/vite") || !present(adapters[0].2) {
        return Err("package.json is missing @inertiajs/vite or its framework Vite plugin".into());
    }
    run(&args.path, &args.frontend)?;
    if args.built {
        validate_built(&frontend, args.ssr)?;
    }
    Ok(())
}

fn validate_built(frontend: &Path, ssr: bool) -> Result<(), String> {
    let manifest = frontend.join("dist/.vite/manifest.json");
    let value: serde_json::Value = serde_json::from_slice(
        &fs::read(&manifest)
            .map_err(|_| format!("missing Vite manifest {}", manifest.display()))?,
    )
    .map_err(|error| format!("invalid Vite manifest {}: {error}", manifest.display()))?;
    let entry = value.get("src/main.ts").ok_or_else(|| {
        format!(
            "Vite manifest {} is missing entry src/main.ts",
            manifest.display()
        )
    })?;
    let file = entry
        .get("file")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "Vite manifest entry src/main.ts has no file".to_owned())?;
    if !frontend.join("dist").join(file).is_file() {
        return Err(format!(
            "Vite manifest references missing output {}",
            frontend.join("dist").join(file).display()
        ));
    }
    if ssr && !frontend.join("dist/ssr/main.js").is_file() {
        return Err(format!(
            "missing SSR bundle {}",
            frontend.join("dist/ssr/main.js").display()
        ));
    }
    Ok(())
}

pub fn run(root: &Path, frontend_arg: &Path) -> Result<(), String> {
    let frontend = root.join(frontend_arg);
    if !frontend.is_dir() {
        return Err(format!(
            "frontend directory {} does not exist",
            frontend.display()
        ));
    }
    let entry = frontend.join("src/main.ts");
    if !entry.is_file() {
        return Err(format!("Vite entry {} does not exist", entry.display()));
    }
    let config = frontend.join("vite.config.ts");
    let config_text = fs::read_to_string(&config)
        .map_err(|_| format!("Vite config {} does not exist", config.display()))?;
    if !config_text.contains("src/main.ts") {
        return Err("vite.config.ts input does not match src/main.ts".into());
    }
    validate_manifest(&frontend)?;

    let sources = rust_sources(root)?;
    let mut declarations: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for source in sources {
        let text = fs::read_to_string(&source).map_err(|error| error.to_string())?;
        for component in literal_components(&text) {
            validate_component(&component)?;
            declarations
                .entry(component)
                .or_default()
                .push(source.clone());
        }
    }
    let pages = frontend.join("src/Pages");
    let frontend_components = page_components(&pages)?;
    for (component, locations) in &declarations {
        if locations.len() > 1 {
            return Err(format!(
                "duplicate component declaration `{component}` in {} files",
                locations.len()
            ));
        }
        if !frontend_components.contains(component) {
            return Err(format!(
                "component `{component}` has no matching page below {}",
                pages.display()
            ));
        }
    }
    println!(
        "cargo inertia check: {} component declarations are valid",
        declarations.len()
    );
    Ok(())
}

fn rust_sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    let manifest = root.join("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(&manifest)
        .no_deps()
        .exec()
        .map_err(|error| format!("could not read {}: {error}", manifest.display()))?;
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let _ = metadata;
    let mut files = Vec::new();
    collect(&root, "rs", &mut files)?;
    files.sort();
    files.dedup();
    Ok(files)
}

fn collect(directory: &Path, extension: &str, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path
            .file_name()
            .is_some_and(|name| name == "target" || name == "node_modules")
        {
            continue;
        }
        if path.is_dir() {
            collect(&path, extension, output)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            output.push(path);
        }
    }
    Ok(())
}

fn literal_components(source: &str) -> Vec<String> {
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    let mut visitor = ComponentVisitor::default();
    visitor.visit_file(&file);
    visitor.components
}

#[derive(Default)]
struct ComponentVisitor {
    components: Vec<String>,
}
impl<'ast> Visit<'ast> for ComponentVisitor {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        if let Ok(Some(component)) = component_from_attribute(attribute) {
            self.components.push(component.value());
        }
        syn::visit::visit_attribute(self, attribute);
    }
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.is_ident("page") {
            if let Ok(literal) = syn::parse2::<LitStr>(mac.tokens.clone()) {
                self.components.push(literal.value());
            }
        }
        syn::visit::visit_macro(self, mac);
    }
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "new" && call.args.len() == 1 {
            if let Some(syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            })) = call.args.first()
            {
                self.components.push(value.value());
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}
fn component_from_attribute(attribute: &Attribute) -> syn::Result<Option<LitStr>> {
    if !attribute.path().is_ident("inertia") {
        return Ok(None);
    }
    let mut component = None;
    attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("component") {
            component = Some(meta.value()?.parse()?);
        } else if meta.input.peek(Token![=]) {
            let _: Expr = meta.value()?.parse()?;
        }
        Ok(())
    })?;
    Ok(component)
}

fn validate_component(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.split('/').any(str::is_empty)
        || value.contains('\\')
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("invalid component path `{value}`"));
    }
    Ok(())
}

fn page_components(pages: &Path) -> Result<BTreeSet<String>, String> {
    if !pages.is_dir() {
        return Err(format!(
            "pages directory {} does not exist",
            pages.display()
        ));
    }
    let mut files = Vec::new();
    for extension in ["svelte", "tsx", "jsx", "vue"] {
        collect(pages, extension, &mut files)?;
    }
    let mut components = BTreeSet::new();
    for file in files {
        let relative = file.strip_prefix(pages).unwrap().with_extension("");
        let component = relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if !components.insert(component.clone()) {
            return Err(format!("duplicate frontend page `{component}`"));
        }
    }
    Ok(components)
}

fn validate_manifest(frontend: &Path) -> Result<(), String> {
    for path in [
        frontend.join("dist/.vite/manifest.json"),
        frontend.join("dist/manifest.json"),
    ] {
        if path.is_file() {
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            let _: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|error| format!("invalid Vite manifest {}: {error}", path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_multiline_literal_components() {
        assert_eq!(
            literal_components(
                "#[inertia(\n component = \"Todos/Index\", rename_all = \"camelCase\"\n)] struct Page;"
            ),
            ["Todos/Index"]
        );
    }
    #[test]
    fn rejects_parent_paths() {
        assert!(validate_component("../Secret").is_err());
        assert!(validate_component("Todos//Index").is_err());
    }

    #[test]
    fn validates_a_complete_project_and_reports_missing_pages() {
        let root = std::env::temp_dir().join(format!("cargo-inertia-check-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("frontend/src/Pages/Todos")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='check-fixture'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "#[inertia(component = \"Todos/Index\")]\nstruct Page;\n",
        )
        .unwrap();
        fs::write(root.join("frontend/src/main.ts"), "").unwrap();
        fs::write(root.join("frontend/vite.config.ts"), "input: 'src/main.ts'").unwrap();
        fs::write(
            root.join("frontend/src/Pages/Todos/Index.svelte"),
            "<h1>Todos</h1>",
        )
        .unwrap();
        run(&root, Path::new("frontend")).unwrap();
        fs::remove_file(root.join("frontend/src/Pages/Todos/Index.svelte")).unwrap();
        assert!(
            run(&root, Path::new("frontend"))
                .unwrap_err()
                .contains("no matching page")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_manifest_entry_path_and_duplicate_failures() {
        let root =
            std::env::temp_dir().join(format!("cargo-inertia-check-errors-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("frontend/src/Pages/Todos")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='check-errors'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "#[inertia(component = \"Todos/Index\")]\nstruct Page;\n",
        )
        .unwrap();
        fs::write(root.join("frontend/src/main.ts"), "").unwrap();
        fs::write(root.join("frontend/vite.config.ts"), "input: 'wrong.ts'").unwrap();
        fs::write(root.join("frontend/src/Pages/Todos/Index.svelte"), "").unwrap();
        assert!(
            run(&root, Path::new("frontend"))
                .unwrap_err()
                .contains("does not match")
        );

        fs::write(root.join("frontend/vite.config.ts"), "input: 'src/main.ts'").unwrap();
        fs::create_dir_all(root.join("frontend/dist/.vite")).unwrap();
        fs::write(root.join("frontend/dist/.vite/manifest.json"), "not json").unwrap();
        assert!(
            run(&root, Path::new("frontend"))
                .unwrap_err()
                .contains("invalid Vite manifest")
        );
        fs::write(root.join("frontend/dist/.vite/manifest.json"), "{}").unwrap();

        fs::write(
            root.join("src/duplicate.rs"),
            "#[inertia(component = \"Todos/Index\")]\nstruct Duplicate;\n",
        )
        .unwrap();
        assert!(
            run(&root, Path::new("frontend"))
                .unwrap_err()
                .contains("duplicate component")
        );
        fs::remove_file(root.join("src/duplicate.rs")).unwrap();

        fs::write(
            root.join("src/invalid.rs"),
            "#[inertia(component = \"../Escape\")]\nstruct Invalid;\n",
        )
        .unwrap();
        assert!(
            run(&root, Path::new("frontend"))
                .unwrap_err()
                .contains("invalid component path")
        );
        fs::remove_file(root.join("src/invalid.rs")).unwrap();

        fs::write(root.join("frontend/src/Pages/Todos/Index.vue"), "").unwrap();
        assert!(
            run(&root, Path::new("frontend"))
                .unwrap_err()
                .contains("duplicate frontend page")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
