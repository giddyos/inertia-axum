//! Compiled Rust-to-TypeScript synchronization.

pub mod args;

use args::{DynamicPagePolicy, LargeIntegerPolicy, OutputLayout, SyncArgs};
use cargo_metadata::{Metadata, MetadataCommand, Package, Target, TargetKind};
use fs2::FileExt;
use inertia_axum_typegen::{ExportBundle, RootKind, TYPEGEN_SCHEMA_VERSION, TypeDefinition};
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
    if args.clean {
        return Err("--clean requires module output from Source Phase 5".into());
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
    if resolve_layout(&output, args.layout) == OutputLayout::Modules {
        return Err(
            "module output is implemented in Source Phase 5; select a TypeScript file".into(),
        );
    }
    let output = if output.is_absolute() {
        output
    } else {
        package_root.join(output)
    };
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
    let (content, mut warnings) = reconcile_and_render(&bundles, args.large_integers)?;
    if has_dynamic_page(package_root) {
        match args.dynamic_pages {
            DynamicPagePolicy::Ignore => {}
            DynamicPagePolicy::Warn => warnings.push(
                "warning[INERTIA-TYPEGEN-019]: dynamic Inertia page has no static prop contract"
                    .into(),
            ),
            DynamicPagePolicy::Error => {
                return Err(
                    "error[INERTIA-TYPEGEN-019]: dynamic Inertia page has no static prop contract"
                        .into(),
                );
            }
        }
    }
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

fn has_dynamic_page(root: &Path) -> bool {
    source_tree_contains(root, "page!(")
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
}
