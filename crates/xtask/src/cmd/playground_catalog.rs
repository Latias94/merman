use crate::XtaskError;
use merman::svg::HeadlessRenderer;
use merman_core::baseline::PINNED_MERMAID_BASELINE_TAG;
use merman_core::{Engine, ParseOptions};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

const MANIFEST_SCHEMA_VERSION: u32 = 2;
const DEFAULT_MANIFEST: &str = "playground/examples/manifest.json";
const DEFAULT_OUTPUT: &str = "playground/src/generated/examples.ts";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogPaths {
    manifest: PathBuf,
    output: PathBuf,
}

impl Default for CatalogPaths {
    fn default() -> Self {
        Self {
            manifest: PathBuf::from(DEFAULT_MANIFEST),
            output: PathBuf::from(DEFAULT_OUTPUT),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlaygroundManifest {
    schema_version: u32,
    mermaid_baseline: String,
    examples: Vec<ManifestExample>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ManifestExample {
    diagram_type: String,
    id: String,
    title: String,
    category: String,
    order: u32,
    aliases: Vec<String>,
    fixture: String,
    evidence: ManifestEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "kebab-case", deny_unknown_fields)]
enum ManifestEvidence {
    FamilyBaseline {
        claim: String,
    },
    Variant {
        kind: VariantEvidenceKind,
        claim: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum VariantEvidenceKind {
    Syntax,
    Behavior,
    Workflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CatalogExample {
    diagram_type: String,
    syntax_id: String,
    id: String,
    title: String,
    category: String,
    order: u32,
    aliases: Vec<String>,
    fixture: String,
    evidence: ManifestEvidence,
    source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlaygroundCatalog {
    mermaid_baseline: String,
    examples: Vec<CatalogExample>,
}

fn catalog_error(message: impl Into<String>) -> XtaskError {
    XtaskError::VerifyFailed(format!("playground example catalog: {}", message.into()))
}

fn parse_paths(args: Vec<String>) -> Result<CatalogPaths, XtaskError> {
    let mut paths = CatalogPaths::default();
    let mut manifest_set = false;
    let mut output_set = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                let value = args.get(index).ok_or(XtaskError::Usage)?;
                if manifest_set {
                    return Err(XtaskError::Usage);
                }
                paths.manifest = PathBuf::from(value);
                manifest_set = true;
            }
            "--out" => {
                index += 1;
                let value = args.get(index).ok_or(XtaskError::Usage)?;
                if output_set {
                    return Err(XtaskError::Usage);
                }
                paths.output = PathBuf::from(value);
                output_set = true;
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        index += 1;
    }
    Ok(paths)
}

fn workspace_path(workspace_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    }
}

fn read_utf8(path: &Path, kind: &str) -> Result<String, XtaskError> {
    let bytes = fs::read(path).map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    String::from_utf8(bytes).map_err(|error| {
        catalog_error(format!("{kind} `{}` is not UTF-8: {error}", path.display()))
    })
}

fn read_manifest(path: &Path) -> Result<PlaygroundManifest, XtaskError> {
    let source = read_utf8(path, "manifest")?;
    serde_json::from_str(&source).map_err(|error| {
        catalog_error(format!(
            "manifest `{}` is invalid JSON: {error}",
            path.display()
        ))
    })
}

fn validate_fixture_path(
    workspace_root: &Path,
    canonical_fixtures_root: &Path,
    entry: &ManifestExample,
) -> Result<PathBuf, XtaskError> {
    let relative = Path::new(&entry.fixture);
    let mut components = relative.components();
    if relative.is_absolute()
        || components.next() != Some(Component::Normal(OsStr::new("fixtures")))
        || components.any(|component| !matches!(component, Component::Normal(_)))
        || relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("mmd")
    {
        return Err(catalog_error(format!(
            "example `{}` fixture must be a normalized repository-relative `fixtures/**/*.mmd` path, got `{}`",
            entry.id, entry.fixture
        )));
    }

    let path = workspace_root.join(relative);
    if !path.is_file() {
        return Err(catalog_error(format!(
            "example `{}` fixture does not exist: `{}`",
            entry.id,
            path.display()
        )));
    }

    let canonical = path.canonicalize().map_err(|source| XtaskError::ReadFile {
        path: path.display().to_string(),
        source,
    })?;
    if !canonical.starts_with(canonical_fixtures_root) {
        return Err(catalog_error(format!(
            "example `{}` fixture escapes the repository fixture root: `{}`",
            entry.id, entry.fixture
        )));
    }

    for provenance in [
        path.with_extension("golden.json"),
        path.with_extension("layout.golden.json"),
    ] {
        if !provenance.is_file() {
            return Err(catalog_error(format!(
                "example `{}` fixture is not an admitted fixture: missing `{}`",
                entry.id,
                provenance.display()
            )));
        }
    }

    Ok(path)
}

fn public_diagram_type_for_syntax(syntax_id: &str) -> Option<&'static str> {
    merman_core::diagram_family_capabilities()
        .iter()
        .find(|capability| capability.diagram_type == syntax_id)
        .and_then(|capability| capability.metadata_id)
}

fn validate_entry_text(entry: &ManifestExample) -> Result<(), XtaskError> {
    if entry.id.is_empty()
        || entry.id.starts_with('-')
        || entry.id.ends_with('-')
        || !entry
            .id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(catalog_error(format!(
            "example id must be lowercase kebab-case, got `{}`",
            entry.id
        )));
    }
    for (label, value) in [("title", &entry.title), ("category", &entry.category)] {
        if value.trim().is_empty() || value.trim() != value {
            return Err(catalog_error(format!(
                "example `{}` has an empty or untrimmed {label}",
                entry.id
            )));
        }
    }
    if entry.order == 0 {
        return Err(catalog_error(format!(
            "example `{}` order must be greater than zero",
            entry.id
        )));
    }
    if entry.aliases.is_empty() {
        return Err(catalog_error(format!(
            "example `{}` must declare at least one search alias",
            entry.id
        )));
    }
    let mut aliases = HashSet::new();
    for alias in &entry.aliases {
        let normalized = alias.trim().to_lowercase();
        if normalized.is_empty() || alias.trim() != alias {
            return Err(catalog_error(format!(
                "example `{}` has an empty or untrimmed alias",
                entry.id
            )));
        }
        if !aliases.insert(normalized) {
            return Err(catalog_error(format!(
                "example `{}` repeats alias `{alias}`",
                entry.id
            )));
        }
    }
    let claim = match &entry.evidence {
        ManifestEvidence::FamilyBaseline { claim } | ManifestEvidence::Variant { claim, .. } => {
            claim
        }
    };
    if claim.trim().is_empty() || claim.trim() != claim || claim.contains(['\r', '\n']) {
        return Err(catalog_error(format!(
            "example `{}` has an empty, untrimmed, or multiline evidence claim",
            entry.id
        )));
    }
    Ok(())
}

fn validate_manifest(
    workspace_root: &Path,
    manifest_path: &Path,
    expected_diagrams: &[&str],
    render_smoke: bool,
) -> Result<PlaygroundCatalog, XtaskError> {
    let manifest = read_manifest(manifest_path)?;
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(catalog_error(format!(
            "manifest `{}` has schemaVersion {}, expected {}",
            manifest_path.display(),
            manifest.schema_version,
            MANIFEST_SCHEMA_VERSION
        )));
    }
    if manifest.mermaid_baseline != PINNED_MERMAID_BASELINE_TAG {
        return Err(catalog_error(format!(
            "manifest `{}` targets `{}`, expected `{}`",
            manifest_path.display(),
            manifest.mermaid_baseline,
            PINNED_MERMAID_BASELINE_TAG
        )));
    }

    let expected: BTreeSet<_> = expected_diagrams.iter().copied().collect();
    let mut diagram_types = HashSet::new();
    let mut ids = HashSet::new();
    let mut orders = HashSet::new();
    let mut fixtures = HashSet::new();
    let mut evidence_by_family = BTreeMap::<String, Vec<(u32, ManifestEvidence)>>::new();
    let mut evidence_claims = HashSet::new();
    let fixtures_path = workspace_root.join("fixtures");
    let canonical_fixtures_root =
        fixtures_path
            .canonicalize()
            .map_err(|source| XtaskError::ReadFile {
                path: fixtures_path.display().to_string(),
                source,
            })?;
    let engine = Engine::new();
    let renderer = HeadlessRenderer::new().with_strict_parsing();
    let mut examples = Vec::with_capacity(manifest.examples.len());

    for entry in manifest.examples {
        validate_entry_text(&entry)?;
        diagram_types.insert(entry.diagram_type.clone());
        if !ids.insert(entry.id.clone()) {
            return Err(catalog_error(format!(
                "manifest duplicates example id `{}`",
                entry.id
            )));
        }
        if !orders.insert(entry.order) {
            return Err(catalog_error(format!(
                "manifest duplicates example order {}",
                entry.order
            )));
        }
        if !fixtures.insert(entry.fixture.clone()) {
            return Err(catalog_error(format!(
                "manifest reuses fixture `{}`",
                entry.fixture
            )));
        }
        let claim = match &entry.evidence {
            ManifestEvidence::FamilyBaseline { claim }
            | ManifestEvidence::Variant { claim, .. } => claim,
        };
        if !evidence_claims.insert((entry.diagram_type.clone(), claim.to_lowercase())) {
            return Err(catalog_error(format!(
                "family `{}` repeats evidence claim `{claim}`",
                entry.diagram_type
            )));
        }
        evidence_by_family
            .entry(entry.diagram_type.clone())
            .or_default()
            .push((entry.order, entry.evidence.clone()));

        let fixture_path = validate_fixture_path(workspace_root, &canonical_fixtures_root, &entry)?;
        let source = read_utf8(&fixture_path, "fixture")?;
        let metadata = engine.parse_metadata_sync(&source).map_err(|error| {
            catalog_error(format!(
                "example `{}` fixture `{}` failed canonical detection: {error}",
                entry.id,
                fixture_path.display()
            ))
        })?;
        let detected_diagram_type = public_diagram_type_for_syntax(&metadata.diagram_type)
            .ok_or_else(|| {
                catalog_error(format!(
                    "example `{}` fixture `{}` detected syntax `{}` without a canonical public diagram type",
                    entry.id,
                    fixture_path.display(),
                    metadata.diagram_type
                ))
            })?;
        if detected_diagram_type != entry.diagram_type {
            return Err(catalog_error(format!(
                "example `{}` declares diagramType `{}` but fixture `{}` canonically detects `{}` via syntax `{}`",
                entry.id,
                entry.diagram_type,
                fixture_path.display(),
                detected_diagram_type,
                metadata.diagram_type
            )));
        }

        engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .map_err(|error| {
                catalog_error(format!(
                    "example `{}` fixture `{}` is not a valid render model: {error}",
                    entry.id,
                    fixture_path.display()
                ))
            })?
            .ok_or_else(|| {
                catalog_error(format!(
                    "example `{}` fixture `{}` produced no render model",
                    entry.id,
                    fixture_path.display()
                ))
            })?;

        if render_smoke {
            let diagram_id = format!("playground-example-{}", entry.id);
            let svg = renderer
                .clone()
                .with_diagram_id(&diagram_id)
                .render_svg_sync(&source)
                .map_err(|error| {
                    catalog_error(format!(
                        "example `{}` fixture `{}` failed the Merman render smoke: {error}",
                        entry.id,
                        fixture_path.display()
                    ))
                })?
                .ok_or_else(|| {
                    catalog_error(format!(
                        "example `{}` fixture `{}` produced no SVG during the Merman render smoke",
                        entry.id,
                        fixture_path.display()
                    ))
                })?;
            if !svg.contains("<svg") {
                return Err(catalog_error(format!(
                    "example `{}` fixture `{}` produced an invalid SVG smoke artifact",
                    entry.id,
                    fixture_path.display()
                )));
            }
        }

        examples.push(CatalogExample {
            diagram_type: entry.diagram_type,
            syntax_id: metadata.diagram_type,
            id: entry.id,
            title: entry.title,
            category: entry.category,
            order: entry.order,
            aliases: entry.aliases,
            fixture: entry.fixture,
            evidence: entry.evidence,
            source,
        });
    }

    let actual: BTreeSet<_> = diagram_types.iter().map(String::as_str).collect();
    if actual != expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
        return Err(catalog_error(format!(
            "manifest diagram set does not match the canonical diagram catalog; missing={missing:?}, unexpected={unexpected:?}"
        )));
    }
    for diagram_type in &expected {
        let family_evidence = evidence_by_family
            .get(*diagram_type)
            .expect("the exact family-set check guarantees evidence entries");
        let baseline_orders = family_evidence
            .iter()
            .filter_map(|(order, evidence)| {
                matches!(evidence, ManifestEvidence::FamilyBaseline { .. }).then_some(*order)
            })
            .collect::<Vec<_>>();
        if baseline_orders.len() != 1 {
            return Err(catalog_error(format!(
                "family `{diagram_type}` must declare exactly one family-baseline example, found {}",
                baseline_orders.len()
            )));
        }
        let first_order = family_evidence
            .iter()
            .map(|(order, _)| *order)
            .min()
            .expect("a present family has at least one example");
        if baseline_orders[0] != first_order {
            return Err(catalog_error(format!(
                "family `{diagram_type}` baseline must be its first ordered example"
            )));
        }
        if !family_evidence
            .iter()
            .any(|(_, evidence)| matches!(evidence, ManifestEvidence::Variant { .. }))
        {
            return Err(catalog_error(format!(
                "family `{diagram_type}` must retain at least one admitted syntax, behavior, or workflow variant"
            )));
        }
    }
    examples.sort_by_key(|example| example.order);
    Ok(PlaygroundCatalog {
        mermaid_baseline: manifest.mermaid_baseline,
        examples,
    })
}

fn render_typescript(catalog: &PlaygroundCatalog) -> Result<String, XtaskError> {
    let mut output = String::from(
        r#"// This file is @generated by `cargo run -p xtask -- gen-playground-example-catalog`.
// Do not edit it directly; edit `playground/examples/manifest.json` or its fixtures.

import type { DiagramType } from "@mermanjs/web";

export interface GeneratedExample {
  readonly id: string;
  readonly title: string;
  readonly category: string;
  readonly order: number;
  readonly diagramType: DiagramType;
  readonly syntaxId: string;
  readonly aliases: readonly string[];
  readonly fixture: string;
  readonly evidence: ExampleEvidence;
  readonly mermaidBaseline: string;
  readonly source: string;
}

export type ExampleEvidence =
  | { readonly role: "family-baseline"; readonly claim: string }
  | {
      readonly role: "variant";
      readonly kind: "syntax" | "behavior" | "workflow";
      readonly claim: string;
    };

"#,
    );
    writeln!(
        &mut output,
        "export const PLAYGROUND_EXAMPLE_BASELINE = {} as const;\n",
        serde_json::to_string(&catalog.mermaid_baseline)?
    )
    .expect("writing to a String cannot fail");
    output.push_str("export const GENERATED_EXAMPLES = [\n");
    for example in &catalog.examples {
        output.push_str("  {\n");
        for (field, value) in [
            ("id", &example.id),
            ("title", &example.title),
            ("category", &example.category),
        ] {
            writeln!(
                &mut output,
                "    {field}: {},",
                serde_json::to_string(value)?
            )
            .expect("writing to a String cannot fail");
        }
        writeln!(&mut output, "    order: {},", example.order)
            .expect("writing to a String cannot fail");
        for (field, value) in [
            ("diagramType", &example.diagram_type),
            ("syntaxId", &example.syntax_id),
        ] {
            writeln!(
                &mut output,
                "    {field}: {},",
                serde_json::to_string(value)?
            )
            .expect("writing to a String cannot fail");
        }
        let aliases = example
            .aliases
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        writeln!(&mut output, "    aliases: [{aliases}],")
            .expect("writing to a String cannot fail");
        writeln!(
            &mut output,
            "    fixture: {},",
            serde_json::to_string(&example.fixture)?
        )
        .expect("writing to a String cannot fail");
        writeln!(
            &mut output,
            "    evidence: {},",
            serde_json::to_string(&example.evidence)?
        )
        .expect("writing to a String cannot fail");
        output.push_str("    mermaidBaseline: PLAYGROUND_EXAMPLE_BASELINE,\n");
        writeln!(
            &mut output,
            "    source: {},",
            serde_json::to_string(&example.source)?
        )
        .expect("writing to a String cannot fail");
        output.push_str("  },\n");
    }
    output.push_str("] as const satisfies readonly GeneratedExample[];\n");
    Ok(output)
}

fn write_output(path: &Path, contents: &str) -> Result<(), XtaskError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| XtaskError::WriteFile {
        path: path.display().to_string(),
        source,
    })
}

fn verify_output(path: &Path, expected: &str) -> Result<(), XtaskError> {
    let actual = read_utf8(path, "generated output")?;
    if actual != expected {
        return Err(catalog_error(format!(
            "generated output `{}` is stale; regenerate with `cargo run -p xtask -- gen-playground-example-catalog`",
            path.display()
        )));
    }
    Ok(())
}

fn build_committed_catalog(
    workspace_root: &Path,
    manifest_path: &Path,
    render_smoke: bool,
) -> Result<PlaygroundCatalog, XtaskError> {
    validate_manifest(
        workspace_root,
        manifest_path,
        merman_core::supported_diagrams(),
        render_smoke,
    )
}

pub(crate) fn gen_playground_example_catalog(args: Vec<String>) -> Result<(), XtaskError> {
    let paths = parse_paths(args)?;
    let workspace_root = super::workspace_root();
    let manifest_path = workspace_path(&workspace_root, &paths.manifest);
    let output_path = workspace_path(&workspace_root, &paths.output);
    let catalog = build_committed_catalog(&workspace_root, &manifest_path, true)?;
    write_output(&output_path, &render_typescript(&catalog)?)
}

pub(crate) fn verify_playground_example_catalog(args: Vec<String>) -> Result<(), XtaskError> {
    let paths = parse_paths(args)?;
    let workspace_root = super::workspace_root();
    let manifest_path = workspace_path(&workspace_root, &paths.manifest);
    let output_path = workspace_path(&workspace_root, &paths.output);
    let expected = render_typescript(&build_committed_catalog(
        &workspace_root,
        &manifest_path,
        true,
    )?)?;
    verify_output(&output_path, &expected)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_MANIFEST, DEFAULT_OUTPUT, MANIFEST_SCHEMA_VERSION, ManifestEvidence,
        ManifestExample, PlaygroundManifest, VariantEvidenceKind, build_committed_catalog,
        parse_paths, read_utf8, render_typescript, validate_manifest, verify_output,
    };
    use merman_core::baseline::PINNED_MERMAID_BASELINE_TAG;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    fn test_manifest(example: ManifestExample) -> PlaygroundManifest {
        PlaygroundManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            mermaid_baseline: PINNED_MERMAID_BASELINE_TAG.to_string(),
            examples: vec![example],
        }
    }

    fn test_example(diagram_type: &str, fixture: &str) -> ManifestExample {
        ManifestExample {
            diagram_type: diagram_type.to_string(),
            id: format!("{diagram_type}-example").to_ascii_lowercase(),
            title: format!("{diagram_type} example"),
            category: "Test".to_string(),
            order: 10,
            aliases: vec!["test alias".to_string()],
            fixture: fixture.to_string(),
            evidence: ManifestEvidence::FamilyBaseline {
                claim: format!("Canonical {diagram_type} test syntax and rendering."),
            },
        }
    }

    fn write_manifest(root: &Path, manifest: &PlaygroundManifest) -> PathBuf {
        let path = root.join("manifest.json");
        fs::write(&path, serde_json::to_vec_pretty(manifest).unwrap()).unwrap();
        path
    }

    fn write_fixture(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        fs::write(path.with_extension("golden.json"), b"{}").unwrap();
        fs::write(path.with_extension("layout.golden.json"), b"{}").unwrap();
    }

    #[test]
    fn committed_manifest_is_exact_typed_and_renderable() {
        let workspace_root = crate::cmd::workspace_root();
        let manifest = workspace_root.join(DEFAULT_MANIFEST);
        let catalog = build_committed_catalog(&workspace_root, &manifest, true).unwrap();

        let mut evidence_counts = BTreeMap::<&str, (usize, usize)>::new();
        for example in &catalog.examples {
            let counts = evidence_counts
                .entry(example.diagram_type.as_str())
                .or_default();
            match &example.evidence {
                ManifestEvidence::FamilyBaseline { .. } => counts.0 += 1,
                ManifestEvidence::Variant { .. } => counts.1 += 1,
            }
        }
        assert!(
            evidence_counts
                .values()
                .all(|(baselines, variants)| *baselines == 1 && *variants >= 1)
        );
        assert_eq!(
            catalog
                .examples
                .iter()
                .map(|example| example.diagram_type.as_str())
                .collect::<BTreeSet<_>>(),
            merman_core::supported_diagrams()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
        );
        assert!(
            catalog
                .examples
                .windows(2)
                .all(|pair| pair[0].order < pair[1].order)
        );
        let typescript = render_typescript(&catalog).unwrap();
        assert!(typescript.contains("import type { DiagramType } from \"@mermanjs/web\";"));
        assert!(typescript.contains("diagramType: DiagramType;"));
        assert!(typescript.contains("readonly evidence: ExampleEvidence;"));
        assert!(!typescript.contains("export type DiagramType"));
    }

    #[test]
    fn missing_family_fails_closed_and_multiple_family_examples_are_allowed() {
        let root = TempDir::new().unwrap();
        write_fixture(
            root.path(),
            "fixtures/flowchart/example.mmd",
            b"flowchart TD\nA --> B\n",
        );
        let example = test_example("flowchart", "fixtures/flowchart/example.mmd");

        let missing_path = write_manifest(root.path(), &test_manifest(example.clone()));
        let error = validate_manifest(
            root.path(),
            &missing_path,
            &["flowchart", "sequence"],
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("missing=[\"sequence\"]"));

        let mut duplicate = test_manifest(example.clone());
        let mut repeated = example;
        repeated.id = "flowchart-second".to_string();
        repeated.order = 20;
        repeated.fixture = "fixtures/flowchart/second.mmd".to_string();
        repeated.evidence = ManifestEvidence::Variant {
            kind: VariantEvidenceKind::Behavior,
            claim: "A second flowchart fixture exercises a distinct edge layout.".to_string(),
        };
        write_fixture(
            root.path(),
            "fixtures/flowchart/second.mmd",
            b"flowchart LR\nStart --> Finish\n",
        );
        duplicate.examples.push(repeated);
        let duplicate_path = write_manifest(root.path(), &duplicate);
        let catalog =
            validate_manifest(root.path(), &duplicate_path, &["flowchart"], false).unwrap();
        assert_eq!(catalog.examples.len(), 2);
        assert!(
            catalog
                .examples
                .iter()
                .all(|catalog_example| catalog_example.diagram_type == "flowchart")
        );
    }

    #[test]
    fn family_evidence_contract_rejects_unearned_variants() {
        let root = TempDir::new().unwrap();
        write_fixture(
            root.path(),
            "fixtures/flowchart/example.mmd",
            b"flowchart TD\nA --> B\n",
        );
        write_fixture(
            root.path(),
            "fixtures/flowchart/second.mmd",
            b"flowchart LR\nStart --> Finish\n",
        );
        let baseline = test_example("flowchart", "fixtures/flowchart/example.mmd");

        let no_variant_path = write_manifest(root.path(), &test_manifest(baseline.clone()));
        assert!(
            validate_manifest(root.path(), &no_variant_path, &["flowchart"], false)
                .unwrap_err()
                .to_string()
                .contains("at least one admitted")
        );

        let mut second = test_example("flowchart", "fixtures/flowchart/second.mmd");
        second.id = "flowchart-second".to_string();
        second.order = 20;
        second.evidence = ManifestEvidence::FamilyBaseline {
            claim: "A second baseline must be rejected even with a distinct claim.".to_string(),
        };
        let duplicate_baseline = PlaygroundManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            mermaid_baseline: PINNED_MERMAID_BASELINE_TAG.to_string(),
            examples: vec![baseline.clone(), second.clone()],
        };
        let duplicate_baseline_path = write_manifest(root.path(), &duplicate_baseline);
        assert!(
            validate_manifest(root.path(), &duplicate_baseline_path, &["flowchart"], false)
                .unwrap_err()
                .to_string()
                .contains("exactly one family-baseline")
        );

        second.evidence = ManifestEvidence::Variant {
            kind: VariantEvidenceKind::Behavior,
            claim: "canonical flowchart test syntax and rendering.".to_string(),
        };
        let repeated_claim = PlaygroundManifest {
            schema_version: MANIFEST_SCHEMA_VERSION,
            mermaid_baseline: PINNED_MERMAID_BASELINE_TAG.to_string(),
            examples: vec![baseline, second],
        };
        let repeated_claim_path = write_manifest(root.path(), &repeated_claim);
        assert!(
            validate_manifest(root.path(), &repeated_claim_path, &["flowchart"], false)
                .unwrap_err()
                .to_string()
                .contains("repeats evidence claim")
        );
    }

    #[test]
    fn cross_family_fixture_reports_declared_and_detected_types() {
        let root = TempDir::new().unwrap();
        write_fixture(
            root.path(),
            "fixtures/sequence/example.mmd",
            b"sequenceDiagram\nAlice->>Bob: Hi\n",
        );
        let manifest = test_manifest(test_example("flowchart", "fixtures/sequence/example.mmd"));
        let path = write_manifest(root.path(), &manifest);

        let error = validate_manifest(root.path(), &path, &["flowchart"], false).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("declares diagramType `flowchart`"));
        assert!(message.contains("canonically detects `sequence`"));
    }

    #[test]
    fn missing_non_utf8_and_invalid_fixtures_are_actionable() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("fixtures")).unwrap();

        let missing = test_manifest(test_example("flowchart", "fixtures/flowchart/missing.mmd"));
        let missing_path = write_manifest(root.path(), &missing);
        assert!(
            validate_manifest(root.path(), &missing_path, &["flowchart"], false)
                .unwrap_err()
                .to_string()
                .contains("fixture does not exist")
        );

        write_fixture(
            root.path(),
            "fixtures/flowchart/non-utf8.mmd",
            &[0xff, 0xfe],
        );
        let non_utf8 = test_manifest(test_example("flowchart", "fixtures/flowchart/non-utf8.mmd"));
        let non_utf8_path = write_manifest(root.path(), &non_utf8);
        assert!(
            validate_manifest(root.path(), &non_utf8_path, &["flowchart"], false)
                .unwrap_err()
                .to_string()
                .contains("is not UTF-8")
        );

        write_fixture(
            root.path(),
            "fixtures/sequence/invalid.mmd",
            b"sequenceDiagram\nAlice->>\n",
        );
        let invalid = test_manifest(test_example("sequence", "fixtures/sequence/invalid.mmd"));
        let invalid_path = write_manifest(root.path(), &invalid);
        assert!(
            validate_manifest(root.path(), &invalid_path, &["sequence"], false)
                .unwrap_err()
                .to_string()
                .contains("not a valid render model")
        );
    }

    #[test]
    fn stale_output_and_non_utf8_manifest_fail_closed() {
        let root = TempDir::new().unwrap();
        let stale = root.path().join("examples.ts");
        fs::write(&stale, "stale\n").unwrap();
        let error = verify_output(&stale, "expected\n").unwrap_err();
        assert!(error.to_string().contains("is stale"));

        let manifest = root.path().join("manifest.json");
        fs::write(&manifest, [0xff, 0xfe]).unwrap();
        assert!(
            read_utf8(&manifest, "manifest")
                .unwrap_err()
                .to_string()
                .contains("manifest")
        );
    }

    #[test]
    fn command_paths_default_and_reject_duplicates() {
        let defaults = parse_paths(Vec::new()).unwrap();
        assert_eq!(defaults.manifest, PathBuf::from(DEFAULT_MANIFEST));
        assert_eq!(defaults.output, PathBuf::from(DEFAULT_OUTPUT));
        assert_eq!(
            parse_paths(vec![
                "--manifest".into(),
                "custom.json".into(),
                "--out".into(),
                "custom.ts".into(),
            ])
            .unwrap(),
            super::CatalogPaths {
                manifest: PathBuf::from("custom.json"),
                output: PathBuf::from("custom.ts"),
            }
        );
        assert!(parse_paths(vec!["--manifest".into()]).is_err());
        assert!(
            parse_paths(vec![
                "--out".into(),
                "first.ts".into(),
                "--out".into(),
                "second.ts".into(),
            ])
            .is_err()
        );
    }
}
