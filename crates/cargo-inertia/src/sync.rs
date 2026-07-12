//! Compiled Rust-to-TypeScript synchronization.

pub mod args;

use args::{DynamicPagePolicy, LargeIntegerPolicy, OutputLayout, SyncArgs};
use cargo_metadata::{Metadata, MetadataCommand, Package, Target, TargetKind};
use fs2::FileExt;
use inertia_axum_typegen::{ExportBundle, RootKind, TYPEGEN_SCHEMA_VERSION, TypeDefinition};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub fn run_args(args: SyncArgs) -> Result<(), String> {
    if args.array_tuple_limit == 0 {
        return Err("--array-tuple-limit must be greater than zero".into());
    }
    let manifest = if args.path.is_file() {
        args.path.clone()
    } else {
        args.path.join("Cargo.toml")
    };
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .exec()
        .map_err(|error| format!("could not read Cargo metadata: {error}"))?;
    let packages = select_packages(&metadata, &args)?;
    if packages.is_empty() {
        return Err("no selected package depends on inertia-axum".into());
    }
    if packages.len() > 1 && !args.workspace {
        return Err("multiple eligible packages; pass --package or --workspace".into());
    }
    for package in packages {
        sync_package(&metadata, package, &args)?;
    }
    Ok(())
}

fn select_packages<'a>(
    metadata: &'a Metadata,
    args: &SyncArgs,
) -> Result<Vec<&'a Package>, String> {
    let mut packages = if let Some(name) = &args.package {
        vec![
            metadata
                .packages
                .iter()
                .find(|package| &package.name == name)
                .ok_or_else(|| format!("Cargo package {name} was not found"))?,
        ]
    } else if args.workspace {
        metadata.workspace_packages()
    } else if let Some(root) = metadata.root_package() {
        vec![root]
    } else {
        metadata.workspace_default_packages()
    };
    packages.retain(|package| {
        package
            .dependencies
            .iter()
            .any(|dependency| dependency.name == "inertia-axum")
    });
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(packages)
}

fn sync_package(metadata: &Metadata, package: &Package, args: &SyncArgs) -> Result<(), String> {
    let package_root = package
        .manifest_path
        .parent()
        .ok_or("package manifest has no parent")?
        .as_std_path();
    let output = resolve_output(package_root, package, args)?;
    let layout = resolve_layout(&output, args.layout);
    let output = if output.is_absolute() {
        output
    } else {
        package_root.join(output)
    };
    if args.clean {
        return clean_output(&output, layout);
    }
    let staging = tempfile::Builder::new()
        .prefix(".inertia-typegen-")
        .tempdir_in(package_root)
        .map_err(|error| error.to_string())?;
    for target in select_targets(package, args)? {
        run_exporters(metadata, package, target, args, staging.path())?;
    }
    let bundles = read_bundles(staging.path())?;
    if bundles.is_empty() && has_typed_declaration(package_root) {
        return Err("error[INERTIA-TYPEGEN-018]: no type exporters were compiled; enable the typegen feature".into());
    }
    let dynamic_warnings = dynamic_page_diagnostics(package_root, args.dynamic_pages)?;
    if layout == OutputLayout::Modules {
        let (files, warnings) = render_modules(
            &bundles,
            args.large_integers,
            args.import_extension.as_deref(),
        )?;
        let warnings = warnings
            .into_iter()
            .chain(dynamic_warnings)
            .collect::<Vec<_>>();
        for warning in &warnings {
            eprintln!("{warning}");
        }
        if args.deny_warnings && !warnings.is_empty() {
            return Err("type-generation warnings were denied".into());
        }
        return reconcile_modules(
            &output,
            files,
            package.name.as_str(),
            args.large_integers,
            args.check,
        );
    }
    let (content, mut warnings) = reconcile_and_render(&bundles, args.large_integers)?;
    warnings.extend(dynamic_warnings);
    for warning in &warnings {
        eprintln!("{warning}");
    }
    if args.deny_warnings && !warnings.is_empty() {
        return Err("type-generation warnings were denied".into());
    }
    compare_or_write(&output, content.as_bytes(), args.check)?;
    if args.verbose {
        eprintln!(
            "cargo inertia sync: {} bundles -> {}",
            bundles.len(),
            output.display()
        );
    }
    Ok(())
}

fn dynamic_page_diagnostics(root: &Path, policy: DynamicPagePolicy) -> Result<Vec<String>, String> {
    dynamic_pages(root)
        .into_iter()
        .filter_map(|component| {
            let message = format!("dynamic Inertia page `{component}` has no static prop contract\n\nexact TypeScript generation requires #[derive(InertiaPage)] on a typed page\n\nuse --dynamic-pages error to reject untyped dynamic pages in CI");
            match policy {
                DynamicPagePolicy::Ignore => None,
                DynamicPagePolicy::Warn => Some(Ok(format!("warning[INERTIA-TYPEGEN-019]: {message}"))),
                DynamicPagePolicy::Error => Some(Err(format!("error[INERTIA-TYPEGEN-019]: {message}"))),
            }
        })
        .collect()
}

fn dynamic_pages(root: &Path) -> Vec<String> {
    let mut pages = Vec::new();
    collect_dynamic_pages(root, &mut pages);
    pages.sort();
    pages.dedup();
    pages
}

fn collect_dynamic_pages(root: &Path, pages: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "target") {
            continue;
        }
        if path.is_dir() {
            collect_dynamic_pages(&path, pages);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            if let Ok(source) = fs::read_to_string(path) {
                pages.extend(dynamic_pages_in_source(&source));
            }
        }
    }
}

fn dynamic_pages_in_source(source: &str) -> Vec<String> {
    use syn::{parse::Parser, visit::Visit};
    #[derive(Default)]
    struct Visitor(Vec<String>);
    impl<'ast> Visit<'ast> for Visitor {
        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            if mac
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "page")
            {
                let first_literal = |input: syn::parse::ParseStream<'_>| {
                    let literal: syn::LitStr = input.parse()?;
                    let _: proc_macro2::TokenStream = input.parse()?;
                    Ok(literal.value())
                };
                if let Ok(component) = first_literal.parse2(mac.tokens.clone()) {
                    self.0.push(component);
                }
            }
            syn::visit::visit_macro(self, mac);
        }
    }
    let Ok(file) = syn::parse_file(source) else {
        return Vec::new();
    };
    let mut visitor = Visitor::default();
    visitor.visit_file(&file);
    visitor.0
}

fn has_typed_declaration(root: &Path) -> bool {
    source_tree_contains(root, "InertiaPage") || source_tree_contains(root, "InertiaProps")
}

fn source_tree_contains(root: &Path, needle: &str) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == "target") {
            continue;
        }
        if path.is_dir() && source_tree_contains(&path, needle) {
            return true;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("rs")
            && fs::read_to_string(&path).is_ok_and(|source| source.contains(needle))
        {
            return true;
        }
    }
    false
}

fn resolve_output(
    package_root: &Path,
    package: &Package,
    args: &SyncArgs,
) -> Result<PathBuf, String> {
    if let Some(output) = args.explicit_output() {
        return Ok(output.clone());
    }
    if let Some(output) = package
        .metadata
        .pointer("/inertia/types/output")
        .and_then(serde_json::Value::as_str)
    {
        return Ok(output.into());
    }
    if package_root.join("frontend/src").is_dir() {
        return Ok("frontend/src/types/inertia.ts".into());
    }
    Err("could not determine the TypeScript output path; pass a destination or configure package metadata".into())
}

fn resolve_layout(output: &Path, explicit: OutputLayout) -> OutputLayout {
    if explicit != OutputLayout::Auto {
        return explicit;
    }
    let value = output.to_string_lossy();
    if value.ends_with(".ts") || value.ends_with(".tsx") || value.ends_with(".d.ts") {
        OutputLayout::Single
    } else {
        OutputLayout::Modules
    }
}

fn select_targets<'a>(package: &'a Package, args: &SyncArgs) -> Result<Vec<&'a Target>, String> {
    let is_lib = |target: &&Target| target.kind.contains(&TargetKind::Lib);
    let is_bin = |target: &&Target| target.kind.contains(&TargetKind::Bin);
    let mut targets: Vec<_> = if args.lib {
        package.targets.iter().filter(is_lib).collect()
    } else if !args.bin.is_empty() {
        package
            .targets
            .iter()
            .filter(|target| is_bin(target) && args.bin.contains(&target.name))
            .collect()
    } else if args.all_bins {
        package.targets.iter().filter(is_bin).collect()
    } else {
        let libraries: Vec<_> = package.targets.iter().filter(is_lib).collect();
        if libraries.is_empty() {
            package.targets.iter().filter(is_bin).collect()
        } else {
            libraries
        }
    };
    if args.examples {
        targets.extend(
            package
                .targets
                .iter()
                .filter(|target| target.kind.contains(&TargetKind::Example)),
        );
    }
    targets.sort_by(|left, right| left.name.cmp(&right.name));
    targets.dedup_by(|left, right| left.name == right.name);
    if targets.is_empty() {
        return Err(format!("package {} has no selected target", package.name));
    }
    Ok(targets)
}

fn run_exporters(
    metadata: &Metadata,
    package: &Package,
    target: &Target,
    args: &SyncArgs,
    staging: &Path,
) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command
        .current_dir(metadata.workspace_root.as_std_path())
        .arg("test")
        .arg("-p")
        .arg(package.name.as_str());
    if target.kind.contains(&TargetKind::Lib) {
        command.arg("--lib");
    } else if target.kind.contains(&TargetKind::Example) {
        command.arg("--example").arg(target.name.as_str());
    } else {
        command.arg("--bin").arg(target.name.as_str());
    }
    if !args.features.is_empty() {
        command.arg("--features").arg(args.features.join(","));
    }
    if args.all_features {
        command.arg("--all-features");
    }
    if args.no_default_features {
        command.arg("--no-default-features");
    }
    command
        .arg("__inertia_typegen_")
        .arg("--")
        .arg("--test-threads=1")
        .env("INERTIA_TYPEGEN_STAGING", staging)
        .env("INERTIA_TYPEGEN_PACKAGE", package.name.as_str())
        .env("INERTIA_TYPEGEN_TARGET", target.name.as_str())
        .env(
            "INERTIA_TYPEGEN_LARGE_INT",
            inertia_axum_typegen::LARGE_INTEGER_SENTINEL,
        )
        .env(
            "INERTIA_TYPEGEN_ARRAY_TUPLE_LIMIT",
            args.array_tuple_limit.to_string(),
        );
    let status = command
        .status()
        .map_err(|error| format!("could not run Cargo exporter tests: {error}"))?;
    if !status.success() {
        return Err(format!(
            "type exporter tests failed for {} target {}",
            package.name, target.name
        ));
    }
    Ok(())
}

fn read_bundles(root: &Path) -> Result<Vec<ExportBundle>, String> {
    fn visit(path: &Path, output: &mut Vec<ExportBundle>) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_dir() {
                visit(&path, output)?;
            } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
                let bundle: ExportBundle =
                    serde_json::from_slice(&fs::read(&path).map_err(|error| error.to_string())?)
                        .map_err(|error| {
                            format!("invalid typegen IR {}: {error}", path.display())
                        })?;
                if bundle.schema_version != TYPEGEN_SCHEMA_VERSION {
                    return Err(format!(
                        "unsupported typegen IR schema {}",
                        bundle.schema_version
                    ));
                }
                output.push(bundle);
            }
        }
        Ok(())
    }
    let mut output = Vec::new();
    visit(root, &mut output)?;
    output.sort_by(|left, right| left.root.rust_name.cmp(&right.root.rust_name));
    Ok(output)
}

fn reconcile_and_render(
    bundles: &[ExportBundle],
    policy: LargeIntegerPolicy,
) -> Result<(String, Vec<String>), String> {
    let mut definitions: BTreeMap<String, TypeDefinition> = BTreeMap::new();
    let mut pages = BTreeMap::new();
    let mut shared = None;
    for bundle in bundles
        .iter()
        .filter(|bundle| bundle.root.kind != RootKind::SupportingType)
    {
        if bundle.root.kind == RootKind::Page {
            let component = bundle
                .root
                .component
                .clone()
                .ok_or("page IR is missing component")?;
            if pages
                .insert(component.clone(), bundle.root.ts_name.clone())
                .is_some()
            {
                return Err(format!(
                    "error[INERTIA-TYPEGEN-015]: duplicate Inertia component {component}"
                ));
            }
        }
        if bundle.root.shared && shared.replace(bundle.root.ts_name.clone()).is_some() {
            return Err("error[INERTIA-TYPEGEN-016]: duplicate shared prop root".into());
        }
        for definition in &bundle.definitions {
            match definitions.get(&definition.name) {
                Some(existing) if existing != definition => {
                    return Err(format!(
                        "error[INERTIA-TYPEGEN-013]: conflicting TypeScript name {}",
                        definition.name
                    ));
                }
                _ => {
                    definitions.insert(definition.name.clone(), definition.clone());
                }
            }
        }
    }
    let contains_large = definitions.values().any(|definition| {
        definition
            .declaration
            .contains(inertia_axum_typegen::LARGE_INTEGER_SENTINEL)
    });
    if contains_large && policy == LargeIntegerPolicy::Error {
        return Err("error[INERTIA-TYPEGEN-002]: large integer is not losslessly representable; serialize as string and declare ts(type = string)".into());
    }
    let replacement = match policy {
        LargeIntegerPolicy::Bigint => "bigint",
        LargeIntegerPolicy::Number | LargeIntegerPolicy::Error => "number",
    };
    let mut warnings = Vec::new();
    if contains_large {
        warnings.push(match policy {
            LargeIntegerPolicy::Number => "warning[INERTIA-TYPEGEN-001]: large integers are emitted as JavaScript numbers and may lose precision".into(),
            LargeIntegerPolicy::Bigint => "warning[INERTIA-TYPEGEN-001]: bigint requires a frontend transport transform".into(),
            LargeIntegerPolicy::Error => unreachable!(),
        });
    }
    let mut output = String::from(
        "// Generated by cargo inertia sync.\n// Do not edit manually.\n\nimport type { ErrorBag, Errors, Page, SharedPageProps } from \"@inertiajs/core\";\n\n",
    );
    for definition in definitions.values() {
        output.push_str("export ");
        output.push_str(
            &definition
                .declaration
                .replace(inertia_axum_typegen::LARGE_INTEGER_SENTINEL, replacement),
        );
        output.push_str("\n\n");
    }
    output.push_str("export interface InertiaPageMap {\n");
    for (component, props) in pages {
        output.push_str(&format!("  {component:?}: {props};\n"));
    }
    output.push_str("}\n\nexport type InertiaComponent = keyof InertiaPageMap;\n\nexport type PagePropsFor<Component extends InertiaComponent> = InertiaPageMap[Component];\n\nexport type ResolvedPagePropsFor<Component extends InertiaComponent> = PagePropsFor<Component> & SharedPageProps & { errors: Errors & ErrorBag };\n\nexport type InertiaPageFor<Component extends InertiaComponent> = Omit<Page<PagePropsFor<Component> & SharedPageProps>, \"component\"> & { component: Component };\n");
    if let Some(shared) = shared {
        output.push_str(&format!(
            "\ndeclare module \"@inertiajs/core\" {{\n  interface InertiaConfig {{\n    sharedPageProps: {shared};\n  }}\n}}\n"
        ));
    }
    let config = dprint_plugin_typescript::configuration::ConfigurationBuilder::new()
        .line_width(100)
        .build();
    let formatted =
        dprint_plugin_typescript::format_text(dprint_plugin_typescript::FormatTextOptions {
            path: Path::new("inertia.ts"),
            extension: None,
            text: output.clone(),
            config: &config,
            external_formatter: None,
        })
        .map_err(|error| format!("could not format generated TypeScript: {error}"))?
        .unwrap_or(output);
    Ok((normalize(formatted), warnings))
}

fn normalize(value: String) -> String {
    format!("{}\n", value.replace("\r\n", "\n").trim_end())
}

fn compare_or_write(path: &Path, bytes: &[u8], check: bool) -> Result<(), String> {
    if check {
        return match fs::read(path) {
            Ok(existing) if existing == bytes => Ok(()),
            Ok(_) => Err(format!(
                "generated Inertia types are stale: changed {}",
                path.display()
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(format!(
                "generated Inertia types are stale: missing {}",
                path.display()
            )),
            Err(error) => Err(error.to_string()),
        };
    }
    let parent = path.parent().ok_or("output has no parent directory")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(parent.join(".inertia-types.lock"))
        .map_err(|error| error.to_string())?;
    lock.lock_exclusive().map_err(|error| error.to_string())?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    temporary
        .persist(path)
        .map_err(|error| error.error.to_string())?;
    FileExt::unlock(&lock).map_err(|error| error.to_string())?;
    Ok(())
}

const MANIFEST_NAME: &str = ".inertia-types.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputManifest {
    schema_version: u32,
    generator: String,
    package: String,
    layout: String,
    large_integers: String,
    files: Vec<PathBuf>,
}

fn render_modules(
    bundles: &[ExportBundle],
    policy: LargeIntegerPolicy,
    import_extension: Option<&str>,
) -> Result<(BTreeMap<PathBuf, Vec<u8>>, Vec<String>), String> {
    let mut definitions: BTreeMap<String, TypeDefinition> = BTreeMap::new();
    let mut pages = BTreeMap::new();
    let mut shared = None;
    for bundle in bundles
        .iter()
        .filter(|bundle| bundle.root.kind != RootKind::SupportingType)
    {
        if bundle.root.kind == RootKind::Page {
            let component = bundle
                .root
                .component
                .clone()
                .ok_or("page IR is missing component")?;
            if pages
                .insert(component.clone(), bundle.root.ts_name.clone())
                .is_some()
            {
                return Err(format!(
                    "error[INERTIA-TYPEGEN-015]: duplicate Inertia component {component}"
                ));
            }
        }
        if bundle.root.shared && shared.replace(bundle.root.ts_name.clone()).is_some() {
            return Err("error[INERTIA-TYPEGEN-016]: duplicate shared prop root".into());
        }
        for definition in &bundle.definitions {
            match definitions.get(&definition.name) {
                Some(existing) if existing != definition => {
                    return Err(format!(
                        "error[INERTIA-TYPEGEN-013]: conflicting TypeScript name {}",
                        definition.name
                    ));
                }
                _ => {
                    definitions.insert(definition.name.clone(), definition.clone());
                }
            }
        }
    }
    let contains_large = definitions.values().any(|definition| {
        definition
            .declaration
            .contains(inertia_axum_typegen::LARGE_INTEGER_SENTINEL)
    });
    if contains_large && policy == LargeIntegerPolicy::Error {
        return Err(
            "error[INERTIA-TYPEGEN-002]: large integer is not losslessly representable".into(),
        );
    }
    let replacement = if policy == LargeIntegerPolicy::Bigint {
        "bigint"
    } else {
        "number"
    };
    let warnings = if contains_large {
        vec![if policy == LargeIntegerPolicy::Bigint {
            "warning[INERTIA-TYPEGEN-001]: bigint requires a frontend transport transform".into()
        } else {
            "warning[INERTIA-TYPEGEN-001]: large integers may lose precision as JavaScript numbers"
                .into()
        }]
    } else {
        Vec::new()
    };

    let mut paths = BTreeMap::new();
    for name in definitions.keys() {
        let page_component = pages
            .iter()
            .find_map(|(component, page_name)| (page_name == name).then_some(component));
        let path = page_component.map_or_else(
            || PathBuf::from(format!("types/{name}.ts")),
            |component| PathBuf::from(format!("pages/{component}.ts")),
        );
        validate_generated_path(&path)?;
        paths.insert(name.clone(), path);
    }

    let mut files = BTreeMap::new();
    for (name, definition) in &definitions {
        let path = paths.get(name).expect("definition path");
        let mut text =
            String::from("// Generated by cargo inertia sync.\n// Do not edit manually.\n\n");
        for dependency in definitions.keys().filter(|dependency| {
            *dependency != name && contains_type_name(&definition.declaration, dependency)
        }) {
            let target = paths.get(dependency).expect("dependency path");
            let import = relative_import(path, target, import_extension)?;
            text.push_str(&format!(
                "import type {{ {dependency} }} from {import:?};\n"
            ));
        }
        if text.contains("import type") {
            text.push('\n');
        }
        text.push_str("export ");
        text.push_str(
            &definition
                .declaration
                .replace(inertia_axum_typegen::LARGE_INTEGER_SENTINEL, replacement),
        );
        text.push('\n');
        files.insert(path.clone(), format_typescript(text, path)?.into_bytes());
    }

    let mut pages_text = String::from(
        "// Generated by cargo inertia sync.\n// Do not edit manually.\n\nimport type { ErrorBag, Errors, Page, SharedPageProps } from \"@inertiajs/core\";\n",
    );
    for (component, name) in &pages {
        let path = paths.get(name).expect("page path");
        let import = module_specifier(Path::new("pages.ts"), path, import_extension)?;
        pages_text.push_str(&format!("import type {{ {name} }} from {import:?};\n"));
        let _ = component;
    }
    pages_text.push_str("\nexport interface InertiaPageMap {\n");
    for (component, name) in &pages {
        pages_text.push_str(&format!("  {component:?}: {name};\n"));
    }
    pages_text.push_str("}\n\nexport type InertiaComponent = keyof InertiaPageMap;\nexport type PagePropsFor<Component extends InertiaComponent> = InertiaPageMap[Component];\nexport type ResolvedPagePropsFor<Component extends InertiaComponent> = PagePropsFor<Component> & SharedPageProps & { errors: Errors & ErrorBag };\nexport type InertiaPageFor<Component extends InertiaComponent> = Omit<Page<PagePropsFor<Component> & SharedPageProps>, \"component\"> & { component: Component };\n");
    files.insert(
        "pages.ts".into(),
        format_typescript(pages_text, Path::new("pages.ts"))?.into_bytes(),
    );

    let mut index = String::from(
        "// Generated by cargo inertia sync.\n// Do not edit manually.\n\nexport type { InertiaComponent, InertiaPageFor, InertiaPageMap, PagePropsFor, ResolvedPagePropsFor } from \"./pages\";\n",
    );
    for (name, path) in &paths {
        if pages.values().any(|page| page == name) {
            continue;
        }
        let import = module_specifier(Path::new("index.ts"), path, import_extension)?;
        index.push_str(&format!("export type {{ {name} }} from {import:?};\n"));
    }
    files.insert(
        "index.ts".into(),
        format_typescript(index, Path::new("index.ts"))?.into_bytes(),
    );

    if let Some(shared_name) = shared {
        let shared_path = paths
            .get(&shared_name)
            .ok_or("shared definition is missing")?;
        let import = module_specifier(Path::new("shared.d.ts"), shared_path, import_extension)?;
        let text = format!(
            "// Generated by cargo inertia sync.\n// Do not edit manually.\n\nimport type {{ {shared_name} }} from {import:?};\n\ndeclare module \"@inertiajs/core\" {{ interface InertiaConfig {{ sharedPageProps: {shared_name}; }} }}\n"
        );
        files.insert(
            "shared.d.ts".into(),
            format_typescript(text, Path::new("shared.d.ts"))?.into_bytes(),
        );
    }
    Ok((files, warnings))
}

fn contains_type_name(declaration: &str, name: &str) -> bool {
    declaration.match_indices(name).any(|(index, _)| {
        let before = declaration[..index].chars().next_back();
        let after = declaration[index + name.len()..].chars().next();
        before.is_none_or(|value| !value.is_alphanumeric() && value != '_')
            && after.is_none_or(|value| !value.is_alphanumeric() && value != '_')
    })
}

fn module_specifier(from: &Path, to: &Path, extension: Option<&str>) -> Result<String, String> {
    relative_import(from, to, extension)
}

fn relative_import(from: &Path, to: &Path, extension: Option<&str>) -> Result<String, String> {
    validate_generated_path(from)?;
    validate_generated_path(to)?;
    let from_parent = from.parent().unwrap_or(Path::new(""));
    let from_parts: Vec<_> = from_parent.components().collect();
    let to_without_extension = to.with_extension("");
    let to_parts: Vec<_> = to_without_extension.components().collect();
    let mut common = 0;
    while common < from_parts.len()
        && common < to_parts.len()
        && from_parts[common] == to_parts[common]
    {
        common += 1;
    }
    let mut value = String::new();
    for _ in common..from_parts.len() {
        value.push_str("../");
    }
    if value.is_empty() {
        value.push_str("./");
    }
    for (index, component) in to_parts[common..].iter().enumerate() {
        if index > 0 {
            value.push('/');
        }
        value.push_str(&component.as_os_str().to_string_lossy());
    }
    if let Some(extension) = extension {
        value.push('.');
        value.push_str(extension.trim_start_matches('.'));
    }
    Ok(value)
}

fn format_typescript(text: String, path: &Path) -> Result<String, String> {
    let config = dprint_plugin_typescript::configuration::ConfigurationBuilder::new()
        .line_width(100)
        .build();
    let formatted =
        dprint_plugin_typescript::format_text(dprint_plugin_typescript::FormatTextOptions {
            path,
            extension: None,
            text: text.clone(),
            config: &config,
            external_formatter: None,
        })
        .map_err(|error| format!("could not format generated TypeScript: {error}"))?
        .unwrap_or(text);
    Ok(normalize(formatted))
}

fn validate_generated_path(path: &Path) -> Result<(), String> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(format!(
            "error[INERTIA-TYPEGEN-017]: unsafe generated path {}",
            path.display()
        ));
    }
    Ok(())
}

fn reconcile_modules(
    root: &Path,
    files: BTreeMap<PathBuf, Vec<u8>>,
    package: &str,
    policy: LargeIntegerPolicy,
    check: bool,
) -> Result<(), String> {
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(format!(
            "refusing symlink module destination {}",
            root.display()
        ));
    }
    let parent = root.parent().ok_or("module output has no parent")?;
    if !check {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(parent.join(".inertia-types.lock"))
        .map_err(|error| error.to_string())?;
    lock.lock_exclusive().map_err(|error| error.to_string())?;
    let old = read_manifest(root)?;
    let old_files: std::collections::BTreeSet<_> =
        old.as_ref().map_or_else(Default::default, |manifest| {
            manifest.files.iter().cloned().collect()
        });
    let new_files: std::collections::BTreeSet<_> = files.keys().cloned().collect();
    if check {
        let mut differences = Vec::new();
        for (path, bytes) in &files {
            match fs::read(root.join(path)) {
                Ok(existing) if existing == *bytes => {}
                Ok(_) => differences.push(format!("changed  {}", path.display())),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    differences.push(format!("missing  {}", path.display()))
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        for stale in old_files.difference(&new_files) {
            differences.push(format!("stale    {}", stale.display()));
        }
        if !differences.is_empty() {
            return Err(format!(
                "generated Inertia types are stale\n\n  {}",
                differences.join("\n  ")
            ));
        }
        return Ok(());
    }
    fs::create_dir_all(root).map_err(|error| error.to_string())?;
    for path in files.keys() {
        validate_generated_path(path)?;
        let destination = root.join(path);
        if destination.exists() && !old_files.contains(path) {
            return Err(format!(
                "refusing to overwrite untracked file {}",
                destination.display()
            ));
        }
    }
    for (path, bytes) in &files {
        let destination = root.join(path);
        let parent = destination.parent().expect("generated file parent");
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        temporary
            .write_all(bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|error| error.to_string())?;
        temporary
            .persist(&destination)
            .map_err(|error| error.error.to_string())?;
    }
    for stale in old_files.difference(&new_files) {
        let path = root.join(stale);
        if path.is_file() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    let manifest = OutputManifest {
        schema_version: 1,
        generator: format!("cargo-inertia {}", env!("CARGO_PKG_VERSION")),
        package: package.into(),
        layout: "modules".into(),
        large_integers: format!("{policy:?}").to_lowercase(),
        files: new_files.into_iter().collect(),
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    let mut temporary = tempfile::NamedTempFile::new_in(root).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    temporary
        .persist(root.join(MANIFEST_NAME))
        .map_err(|error| error.error.to_string())?;
    FileExt::unlock(&lock).map_err(|error| error.to_string())?;
    Ok(())
}

fn read_manifest(root: &Path) -> Result<Option<OutputManifest>, String> {
    let path = root.join(MANIFEST_NAME);
    match fs::read(&path) {
        Ok(bytes) => {
            let manifest: OutputManifest = serde_json::from_slice(&bytes).map_err(|error| {
                format!("invalid generator manifest {}: {error}", path.display())
            })?;
            if manifest.schema_version != 1 || manifest.layout != "modules" {
                return Err("unsupported generator manifest".into());
            }
            for file in &manifest.files {
                validate_generated_path(file)?;
            }
            Ok(Some(manifest))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn clean_output(path: &Path, layout: OutputLayout) -> Result<(), String> {
    if layout == OutputLayout::Single {
        match fs::read_to_string(path) {
            Ok(content) if content.starts_with("// Generated by cargo inertia sync.") => {
                fs::remove_file(path).map_err(|error| error.to_string())
            }
            Ok(_) => Err(format!("refusing to clean unowned file {}", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    } else {
        let Some(manifest) = read_manifest(path)? else {
            return Ok(());
        };
        for file in manifest.files {
            let target = path.join(file);
            if target.is_file() {
                fs::remove_file(target).map_err(|error| error.to_string())?;
            }
        }
        fs::remove_file(path.join(MANIFEST_NAME)).map_err(|error| error.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_resolution_is_deterministic() {
        assert_eq!(
            resolve_layout(Path::new("types.ts"), OutputLayout::Auto),
            OutputLayout::Single
        );
        assert_eq!(
            resolve_layout(Path::new("types"), OutputLayout::Auto),
            OutputLayout::Modules
        );
    }

    #[test]
    fn module_manifest_owns_only_generated_files() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("inertia");
        let files = BTreeMap::from([(
            PathBuf::from("index.ts"),
            b"// Generated by cargo inertia sync.\n".to_vec(),
        )]);
        reconcile_modules(&root, files, "server", LargeIntegerPolicy::Number, false).unwrap();
        fs::write(root.join("user.ts"), "user owned").unwrap();
        clean_output(&root, OutputLayout::Modules).unwrap();
        assert!(root.join("user.ts").is_file());
        assert!(!root.join("index.ts").exists());
        assert!(!root.join(MANIFEST_NAME).exists());
    }

    #[test]
    fn generated_paths_cannot_escape() {
        assert!(validate_generated_path(Path::new("types/Todo.ts")).is_ok());
        assert!(validate_generated_path(Path::new("../Todo.ts")).is_err());
        assert!(validate_generated_path(Path::new("/Todo.ts")).is_err());
    }

    #[test]
    fn dynamic_scanner_reports_literal_page_components() {
        let source = r#"
            page!("Users/Show", { user });
            crate::page!("Home", { greeting: value() });
            unrelated!("Ignored");
        "#;
        assert_eq!(dynamic_pages_in_source(source), ["Users/Show", "Home"]);
    }
}
