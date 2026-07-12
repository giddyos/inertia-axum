#![allow(dead_code)]

use super::*;

#[derive(TS)]
#[ts(export_to = "leaf.ts")]
struct Leaf {
    value: u64,
}

#[derive(TS)]
#[ts(export_to = "branch.ts")]
struct Branch {
    leaf: Leaf,
}

#[derive(TS)]
#[ts(export_to = "root.ts")]
/// Root documentation.
struct Root {
    branch: Branch,
    duplicate: Leaf,
}

fn config() -> Config {
    Config::default().with_large_int(LARGE_INTEGER_SENTINEL)
}

#[test]
fn collector_includes_root_and_recursive_deduplicated_dependencies() {
    let definitions = TypeCollector::new(&config())
        .collect_root::<Root>()
        .unwrap();
    assert_eq!(
        definitions
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["Branch", "Leaf", "Root"]
    );
    assert!(
        definitions
            .iter()
            .any(|item| item.declaration.contains(LARGE_INTEGER_SENTINEL))
    );
    assert!(
        definitions
            .iter()
            .find(|item| item.name == "Root")
            .unwrap()
            .docs
            .as_deref()
            .unwrap()
            .contains("Root documentation.")
    );
}

struct Cycle;

impl TS for Cycle {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;
    fn name(_: &Config) -> String {
        "Cycle".into()
    }
    fn inline(_: &Config) -> String {
        "Cycle".into()
    }
    fn decl(_: &Config) -> String {
        "type Cycle = Cycle;".into()
    }
    fn output_path() -> Option<PathBuf> {
        Some("cycle.ts".into())
    }
    fn visit_dependencies(visitor: &mut impl TypeVisitor)
    where
        Self: 'static,
    {
        visitor.visit::<Self>();
    }
}

#[test]
fn recursive_graph_terminates() {
    let definitions = TypeCollector::new(&config())
        .collect_root::<Cycle>()
        .unwrap();
    assert_eq!(definitions.len(), 1);
}

struct UnsafeDependency;

impl TS for UnsafeDependency {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;
    fn name(_: &Config) -> String {
        "UnsafeDependency".into()
    }
    fn inline(_: &Config) -> String {
        "string".into()
    }
    fn decl(_: &Config) -> String {
        "type UnsafeDependency = string;".into()
    }
    fn output_path() -> Option<PathBuf> {
        Some("../unsafe.ts".into())
    }
}

#[test]
fn visitor_failure_is_deferred_and_returned() {
    let error = TypeCollector::new(&config())
        .collect_root::<UnsafeDependency>()
        .unwrap_err();
    assert!(matches!(error, ExportError::UnsafeOutputPath(_)));
}

#[test]
fn same_definition_deduplicates_and_conflict_is_typed() {
    let definition = TypeDefinition {
        name: "Same".into(),
        declaration: "type Same = string;".into(),
        output_path: "same.ts".into(),
        docs: None,
    };
    let cfg = config();
    let mut collector = TypeCollector::new(&cfg);
    collector.insert(definition.clone()).unwrap();
    collector.insert(definition).unwrap();
    let error = collector
        .insert(TypeDefinition {
            name: "Same".into(),
            declaration: "type Same = number;".into(),
            output_path: "same.ts".into(),
            docs: None,
        })
        .unwrap_err();
    assert!(matches!(error, ExportError::ConflictingType { name } if name == "Same"));
}

#[test]
fn environment_and_hash_are_stable() {
    let values = BTreeMap::from([
        ("INERTIA_TYPEGEN_STAGING", "/tmp/typegen"),
        ("INERTIA_TYPEGEN_PACKAGE", "server"),
        ("INERTIA_TYPEGEN_TARGET", "lib"),
        ("INERTIA_TYPEGEN_LARGE_INT", LARGE_INTEGER_SENTINEL),
        ("INERTIA_TYPEGEN_ARRAY_TUPLE_LIMIT", "64"),
    ]);
    let environment =
        ExportEnvironment::from_lookup(|name| values.get(name).map(ToString::to_string)).unwrap();
    let root = RootDefinition {
        kind: RootKind::Page,
        rust_name: "Root".into(),
        ts_name: "RootProps".into(),
        component: Some("Root".into()),
        shared: false,
        source: SourceLocation {
            file: "src/lib.rs".into(),
            line: 1,
            module: "app".into(),
        },
    };
    assert_eq!(
        stable_root_hash(&environment, &root),
        stable_root_hash(&environment, &root)
    );
    assert_eq!(stable_root_hash(&environment, &root).len(), 64);
}

#[test]
fn output_paths_reject_traversal_and_normalize_dots() {
    assert_eq!(
        normalize_relative_path(Path::new("types/./root.ts")).unwrap(),
        PathBuf::from("types/root.ts")
    );
    assert!(matches!(
        normalize_relative_path(Path::new("../root.ts")),
        Err(ExportError::UnsafeOutputPath(_))
    ));
}

#[test]
fn collector_warns_for_target_dependent_integers() {
    let cfg = config();
    let (_, diagnostics) = TypeCollector::new(&cfg)
        .collect_with_diagnostics::<usize>()
        .unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "INERTIA-TYPEGEN-003");
}
