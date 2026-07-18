//! Internal compiled-type export substrate for `inertia-axum`.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

#[doc(hidden)]
pub use serde;
pub use ts_rs::*;

/// Version of the JSON intermediate representation.
pub const TYPEGEN_SCHEMA_VERSION: u32 = 1;
/// Placeholder retained in IR until the CLI applies its integer policy.
pub const LARGE_INTEGER_SENTINEL: &str = "__INERTIA_LARGE_INTEGER__";

/// One root and every declaration reachable from it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub schema_version: u32,
    pub package: String,
    pub cargo_target: String,
    pub root: RootDefinition,
    pub definitions: Vec<TypeDefinition>,
    pub diagnostics: Vec<TypegenDiagnostic>,
}

/// Stable description of an exported root.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RootDefinition {
    pub kind: RootKind,
    pub rust_name: String,
    pub ts_name: String,
    pub component: Option<String>,
    pub shared: bool,
    pub source: SourceLocation,
}

/// Test-friendly root metadata emitted by derives.
#[derive(Debug, Clone)]
pub struct RootMetadata {
    pub kind: RootKind,
    pub rust_name: &'static str,
    pub ts_name: &'static str,
    pub component: Option<&'static str>,
    pub shared: bool,
    pub source: SourceLocation,
}

/// Test-friendly metadata for a supporting type.
#[derive(Debug, Clone)]
pub struct TypeMetadata {
    pub rust_name: &'static str,
    pub source: SourceLocation,
}

/// Kind of generated contract root.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RootKind {
    Page,
    Props,
    SupportingType,
}

/// One stable TypeScript declaration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TypeDefinition {
    pub name: String,
    pub declaration: String,
    pub output_path: PathBuf,
    pub docs: Option<String>,
}

/// Source location used only for actionable diagnostics and stable identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub module: String,
}

/// Diagnostic severity stored in IR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Warning,
    Error,
}

/// Stable structured diagnostic stored in IR.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TypegenDiagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub rust_type: Option<String>,
    pub field: Option<String>,
    pub source: Option<SourceLocation>,
}

/// Validated exporter-test environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportEnvironment {
    pub staging: PathBuf,
    pub package: String,
    pub cargo_target: String,
    pub large_int: String,
    pub array_tuple_limit: usize,
}

/// Typed exporter failures surfaced by generated tests.
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("missing environment variable {0}")]
    MissingEnvironment(&'static str),
    #[error("INERTIA_TYPEGEN_STAGING must be an absolute path: {0}")]
    RelativeStaging(PathBuf),
    #[error("invalid environment variable {name}: {value}")]
    InvalidEnvironment { name: &'static str, value: String },
    #[error("conflicting TypeScript declarations for {name}")]
    ConflictingType { name: String },
    #[error("unsafe generated output path: {0}")]
    UnsafeOutputPath(PathBuf),
    #[error("failed to serialize type-generation IR: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("type-generation filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
}

impl ExportEnvironment {
    /// Reads and validates the CLI-to-exporter environment contract.
    pub fn from_env() -> Result<Self, ExportError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    fn from_lookup(mut get: impl FnMut(&str) -> Option<String>) -> Result<Self, ExportError> {
        fn required(
            get: &mut impl FnMut(&str) -> Option<String>,
            name: &'static str,
        ) -> Result<String, ExportError> {
            get(name).ok_or(ExportError::MissingEnvironment(name))
        }
        let staging = PathBuf::from(required(&mut get, "INERTIA_TYPEGEN_STAGING")?);
        if !staging.is_absolute() {
            return Err(ExportError::RelativeStaging(staging));
        }
        let package = required(&mut get, "INERTIA_TYPEGEN_PACKAGE")?;
        let cargo_target = required(&mut get, "INERTIA_TYPEGEN_TARGET")?;
        let large_int = required(&mut get, "INERTIA_TYPEGEN_LARGE_INT")?;
        let raw_limit = required(&mut get, "INERTIA_TYPEGEN_ARRAY_TUPLE_LIMIT")?;
        let array_tuple_limit = raw_limit
            .parse()
            .map_err(|_| ExportError::InvalidEnvironment {
                name: "INERTIA_TYPEGEN_ARRAY_TUPLE_LIMIT",
                value: raw_limit,
            })?;
        if package.is_empty()
            || cargo_target.is_empty()
            || large_int.is_empty()
            || array_tuple_limit == 0
        {
            return Err(ExportError::InvalidEnvironment {
                name: "typegen environment",
                value: "values must be non-empty and tuple limit must be positive".into(),
            });
        }
        Ok(Self {
            staging,
            package,
            cargo_target,
            large_int,
            array_tuple_limit,
        })
    }
}

/// Recursively collects exportable declarations through `TypeVisitor`.
pub struct TypeCollector<'a> {
    config: &'a Config,
    visited: BTreeSet<TypeId>,
    definitions: BTreeMap<String, TypeDefinition>,
    error: Option<ExportError>,
    diagnostics: Vec<TypegenDiagnostic>,
}

impl<'a> TypeCollector<'a> {
    pub fn new(config: &'a Config) -> Self {
        Self {
            config,
            visited: BTreeSet::new(),
            definitions: BTreeMap::new(),
            error: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn collect_root<T: TS + 'static>(mut self) -> Result<Vec<TypeDefinition>, ExportError> {
        self.collect::<T>();
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(self.definitions.into_values().collect())
    }

    fn collect_with_diagnostics<T: TS + 'static>(
        mut self,
    ) -> Result<(Vec<TypeDefinition>, Vec<TypegenDiagnostic>), ExportError> {
        self.collect::<T>();
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok((self.definitions.into_values().collect(), self.diagnostics))
    }

    fn collect<T: TS + 'static + ?Sized>(&mut self) {
        if self.error.is_some() || !self.visited.insert(TypeId::of::<T>()) {
            return;
        }
        if TypeId::of::<T>() == TypeId::of::<usize>() || TypeId::of::<T>() == TypeId::of::<isize>()
        {
            self.diagnostics.push(TypegenDiagnostic {
                severity: Severity::Warning,
                code: "INERTIA-TYPEGEN-003".into(),
                message: "usize/isize is target-dependent in a frontend contract; prefer a fixed-width wire integer".into(),
                rust_type: Some(std::any::type_name::<T>().into()),
                field: None,
                source: None,
            });
        }
        if let Some(path) = T::output_path() {
            match normalize_relative_path(&path) {
                Ok(output_path) => {
                    let definition = TypeDefinition {
                        name: T::ident(self.config),
                        declaration: T::decl(self.config),
                        output_path,
                        docs: T::docs(),
                    };
                    if let Err(error) = self.insert(definition) {
                        self.error = Some(error);
                        return;
                    }
                }
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        T::visit_dependencies(self);
    }

    fn insert(&mut self, definition: TypeDefinition) -> Result<(), ExportError> {
        match self.definitions.get(&definition.name) {
            Some(existing) if existing != &definition => Err(ExportError::ConflictingType {
                name: definition.name,
            }),
            Some(_) => Ok(()),
            None => {
                self.definitions.insert(definition.name.clone(), definition);
                Ok(())
            }
        }
    }
}

impl TypeVisitor for TypeCollector<'_> {
    fn visit<T: TS + 'static + ?Sized>(&mut self) {
        self.collect::<T>();
    }
}

/// Exports a page or reusable props root into deterministic per-root IR.
pub fn export_root<T: TS + 'static>(metadata: RootMetadata) -> Result<PathBuf, ExportError> {
    let environment = ExportEnvironment::from_env()?;
    let config = Config::default().with_large_int(&environment.large_int);
    let (definitions, diagnostics) = TypeCollector::new(&config).collect_with_diagnostics::<T>()?;
    let root = RootDefinition {
        kind: metadata.kind,
        rust_name: metadata.rust_name.into(),
        ts_name: metadata.ts_name.into(),
        component: metadata.component.map(str::to_owned),
        shared: metadata.shared,
        source: metadata.source,
    };
    write_bundle(
        &environment,
        ExportBundle {
            schema_version: TYPEGEN_SCHEMA_VERSION,
            package: environment.package.clone(),
            cargo_target: environment.cargo_target.clone(),
            root,
            definitions,
            diagnostics,
        },
    )
}

/// Exports metadata and declarations for a supporting application type.
pub fn export_supporting_type<T: TS + 'static>(
    metadata: TypeMetadata,
) -> Result<PathBuf, ExportError> {
    export_root::<T>(RootMetadata {
        kind: RootKind::SupportingType,
        rust_name: metadata.rust_name,
        ts_name: metadata.rust_name,
        component: None,
        shared: false,
        source: metadata.source,
    })
}

/// Stable root identifier used as the IR file name.
pub fn stable_root_hash(environment: &ExportEnvironment, root: &RootDefinition) -> String {
    let mut hash = Sha256::new();
    for value in [
        &environment.package,
        &environment.cargo_target,
        root_kind_name(root.kind),
        root.component.as_deref().unwrap_or(""),
        &root.rust_name,
        &root.source.module,
    ] {
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = hash.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

fn root_kind_name(kind: RootKind) -> &'static str {
    match kind {
        RootKind::Page => "page",
        RootKind::Props => "props",
        RootKind::SupportingType => "supportingType",
    }
}

fn write_bundle(
    environment: &ExportEnvironment,
    bundle: ExportBundle,
) -> Result<PathBuf, ExportError> {
    let directory = environment
        .staging
        .join(&environment.package)
        .join(&environment.cargo_target);
    fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "{}.json",
        stable_root_hash(environment, &bundle.root)
    ));
    let mut bytes = serde_json::to_vec_pretty(&bundle)?;
    bytes.push(b'\n');
    fs::write(&path, bytes)?;
    Ok(path)
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, ExportError> {
    if path.is_absolute() {
        return Err(ExportError::UnsafeOutputPath(path.to_owned()));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            _ => return Err(ExportError::UnsafeOutputPath(path.to_owned())),
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests;
