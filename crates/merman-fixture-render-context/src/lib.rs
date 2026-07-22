//! Validated test-harness inputs and source-backed fixture capability facts.
//!
//! The catalog binds a fixture's bytes to a narrowly structured host `siteConfig`. It does not
//! encode expected render answers and does not claim the identity of an upstream source case.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const MANIFEST_RELATIVE_PATH: &str = "_config/render_contexts.json";
const SCHEMA_VERSION: u32 = 1;
const FRONTMATTER_SECURITY_PATH: [&str; 2] = ["config", "securityLevel"];
const DIRECTIVE_SECURITY_PATH: [&str; 1] = ["securityLevel"];
const NESTED_DIRECTIVE_SECURITY_PATH: [&str; 2] = ["config", "securityLevel"];

/// Returns the pinned-upstream reason that an exact fixture is semantic-only.
///
/// The family is part of the identity. File-name conventions deliberately carry no capability:
/// adding `parser_only` to a new fixture name must never exempt it from render gates.
pub fn parser_only_fixture_reason(
    diagram: &str,
    fixture_name_or_stem: &str,
) -> Option<&'static str> {
    let stem = Path::new(fixture_name_or_stem)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(fixture_name_or_stem);
    match (diagram, stem) {
        ("flowchart", "upstream_flow_text_ellipse_vertex_parser_only_spec") => {
            Some("pinned Mermaid 11.16 cannot render this parser-only ellipse vertex fixture")
        }
        (
            "sankey",
            "upstream_sankey_allows_proto_id_parser_only_spec"
            | "upstream_sankey_allows_proto_id_sankey_header_parser_only_spec",
        ) => {
            Some("pinned Mermaid 11.16 rejects this parser-only Sankey fixture as a circular link")
        }
        (
            "xychart",
            "upstream_xychart_header_only_jison_spec_parser_only_"
            | "upstream_xychart_title_variants_jison_spec_parser_only_",
        ) => Some(
            "pinned Mermaid 11.16 XYChart renderer crashes because the parser-only fixture has no plot data",
        ),
        _ => None,
    }
}

/// Strongest DOM evidence that is meaningful for an exact fixture.
///
/// RoughJS and `roughr` intentionally use different path generators. Their generated path
/// coordinates are visible residuals, so these fixtures prove SVG ownership and element order in
/// structure mode instead of weakening parity comparison for the rest of the family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureDomEvidence {
    StructureOnly,
    BrowserTextWrapping,
}

impl FixtureDomEvidence {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::StructureOnly => {
                "JavaScript RoughJS and Rust roughr path geometry differ; compare exact DOM structure without hiding path coordinates"
            }
            Self::BrowserTextWrapping => {
                "browser font measurement changes only Mermaid's generated text-row segmentation; preserve all other parity-visible DOM evidence"
            }
        }
    }
}

/// DOM evidence boundary that applies to every fixture in one diagram family.
///
/// These policies describe behavior derived directly from the pinned Mermaid implementation.
/// They must remain family-scoped so browser-dependent values are not discarded from unrelated
/// diagrams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagramDomEvidence {
    BrowserMeasuredTextLength,
}

impl DiagramDomEvidence {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::BrowserMeasuredTextLength => {
                "pinned Mermaid derives C4 textLength from browser text measurement; compare attribute presence and all surrounding DOM while normalizing only the measured numeric value"
            }
        }
    }
}

/// Returns a source-backed DOM evidence boundary for an entire diagram family.
pub fn diagram_dom_evidence(diagram: &str) -> Option<DiagramDomEvidence> {
    match diagram {
        "c4" => Some(DiagramDomEvidence::BrowserMeasuredTextLength),
        _ => None,
    }
}

/// Returns an exact, family-scoped DOM evidence boundary.
///
/// Names and config contents are not interpreted heuristically. Adding another hand-drawn fixture
/// therefore does not silently weaken its comparison mode.
pub fn fixture_dom_evidence(
    diagram: &str,
    fixture_name_or_stem: &str,
) -> Option<FixtureDomEvidence> {
    let stem = Path::new(fixture_name_or_stem)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(fixture_name_or_stem);
    match (diagram, stem) {
        ("ishikawa", "upstream_cypress_ishikawa_spec_6_should_render_with_handdrawn_look_006")
        | (
            "venn",
            "upstream_cypress_venn_handdrawn_two_set_014"
            | "upstream_cypress_venn_handdrawn_three_set_title_015"
            | "upstream_cypress_venn_handdrawn_custom_styles_018",
        ) => Some(FixtureDomEvidence::StructureOnly),
        ("class", "stress_class_svg_font_size_precedence_025") => {
            Some(FixtureDomEvidence::BrowserTextWrapping)
        }
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureRenderContext {
    fixture: String,
    fixture_sha256: String,
    provenance: Provenance,
    security_level: SecurityLevel,
}

impl FixtureRenderContext {
    pub fn derive(fixture: &str, source: &[u8]) -> Result<Option<Self>, CatalogError> {
        validate_relative_fixture_path(fixture)?;
        let source_text =
            std::str::from_utf8(source).map_err(|source| CatalogError::InvalidFixtureEncoding {
                fixture: fixture.to_string(),
                source,
            })?;

        let mut declarations = Vec::new();
        if let Some(frontmatter) = parse_frontmatter(source_text, fixture)?
            && let Some(value) = value_at_path(&frontmatter, &FRONTMATTER_SECURITY_PATH)
        {
            declarations.push((
                Provenance::Frontmatter {
                    config_path: path_strings(&FRONTMATTER_SECURITY_PATH),
                },
                value.clone(),
            ));
        }

        for directive in parse_directives(source_text) {
            let Ok(value) = directive.value else {
                continue;
            };
            for path in [
                DIRECTIVE_SECURITY_PATH.as_slice(),
                NESTED_DIRECTIVE_SECURITY_PATH.as_slice(),
            ] {
                let Some(level) = value_at_path(&value, path) else {
                    continue;
                };
                declarations.push((
                    Provenance::Directive {
                        directive: directive.name.clone(),
                        occurrence: directive.occurrence,
                        config_path: path_strings(path),
                    },
                    level.clone(),
                ));
            }
        }

        if declarations.len() > 1 {
            return Err(CatalogError::AmbiguousSecurityDeclarations {
                fixture: fixture.to_string(),
                count: declarations.len(),
            });
        }
        match declarations.pop() {
            Some((provenance, value)) => {
                context_for_declared_level(fixture, source, provenance, &value)
            }
            None => Ok(None),
        }
    }

    pub fn fixture(&self) -> &str {
        &self.fixture
    }

    pub fn fixture_sha256(&self) -> &str {
        &self.fixture_sha256
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }

    pub fn security_level(&self) -> SecurityLevel {
        self.security_level
    }

    pub fn site_config_value(&self) -> Value {
        serde_json::json!({ "securityLevel": self.security_level.as_str() })
    }
}

fn context_for_declared_level(
    fixture: &str,
    source: &[u8],
    provenance: Provenance,
    value: &Value,
) -> Result<Option<FixtureRenderContext>, CatalogError> {
    match parse_declared_security_level(fixture, value)? {
        Some(security_level) => Ok(Some(FixtureRenderContext {
            fixture: fixture.to_string(),
            fixture_sha256: sha256_hex(source),
            provenance,
            security_level,
        })),
        None => Ok(None),
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SecurityLevel {
    Loose,
    Sandbox,
}

impl SecurityLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Loose => "loose",
            Self::Sandbox => "sandbox",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Provenance {
    Frontmatter {
        config_path: Vec<String>,
    },
    Directive {
        directive: String,
        occurrence: usize,
        config_path: Vec<String>,
    },
}

#[derive(Clone, Debug)]
pub struct RenderContextCatalog {
    fixtures_root: PathBuf,
    contexts: BTreeMap<String, FixtureRenderContext>,
}

impl RenderContextCatalog {
    pub fn rebuild(fixtures_root: impl AsRef<Path>) -> Result<Self, CatalogError> {
        Ok(Self {
            fixtures_root: canonical_fixtures_root(fixtures_root.as_ref())?,
            contexts: BTreeMap::new(),
        })
    }

    pub fn load(fixtures_root: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let fixtures_root = canonical_fixtures_root(fixtures_root.as_ref())?;
        let manifest_path = fixtures_root.join(MANIFEST_RELATIVE_PATH);
        let manifest =
            fs::read_to_string(&manifest_path).map_err(|source| CatalogError::ReadManifest {
                path: manifest_path.clone(),
                source,
            })?;
        Self::from_json(fixtures_root, &manifest)
    }

    pub fn load_for_update(fixtures_root: impl AsRef<Path>) -> Result<Self, CatalogError> {
        let fixtures_root = canonical_fixtures_root(fixtures_root.as_ref())?;
        let manifest_path = fixtures_root.join(MANIFEST_RELATIVE_PATH);
        match fs::read_to_string(&manifest_path) {
            Ok(manifest) => Self::from_json(fixtures_root, &manifest),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                fixtures_root,
                contexts: BTreeMap::new(),
            }),
            Err(source) => Err(CatalogError::ReadManifest {
                path: manifest_path,
                source,
            }),
        }
    }

    pub fn load_for_fixture_update(
        fixtures_root: impl AsRef<Path>,
        relative_fixture: &str,
    ) -> Result<Self, CatalogError> {
        validate_relative_fixture_path(relative_fixture)?;
        let fixtures_root = canonical_fixtures_root(fixtures_root.as_ref())?;
        let manifest_path = fixtures_root.join(MANIFEST_RELATIVE_PATH);
        match fs::read_to_string(&manifest_path) {
            Ok(manifest) => Self::from_json_with_pending_fixture(
                fixtures_root,
                &manifest,
                Some(relative_fixture),
            ),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                fixtures_root,
                contexts: BTreeMap::new(),
            }),
            Err(source) => Err(CatalogError::ReadManifest {
                path: manifest_path,
                source,
            }),
        }
    }

    pub fn from_json(
        fixtures_root: impl AsRef<Path>,
        manifest: &str,
    ) -> Result<Self, CatalogError> {
        Self::from_json_with_pending_fixture(fixtures_root, manifest, None)
    }

    fn from_json_with_pending_fixture(
        fixtures_root: impl AsRef<Path>,
        manifest: &str,
        pending_fixture: Option<&str>,
    ) -> Result<Self, CatalogError> {
        let fixtures_root = canonical_fixtures_root(fixtures_root.as_ref())?;
        let manifest: ManifestWire =
            serde_json::from_str(manifest).map_err(CatalogError::ParseManifest)?;
        if manifest.schema_version != SCHEMA_VERSION {
            return Err(CatalogError::UnsupportedSchemaVersion(
                manifest.schema_version,
            ));
        }

        let mut contexts = BTreeMap::new();
        for wire in manifest.contexts {
            validate_relative_fixture_path(&wire.fixture)?;
            validate_sha256(&wire.fixture, &wire.fixture_sha256)?;
            let provenance = Provenance::from_wire(wire.provenance)?;
            if pending_fixture != Some(wire.fixture.as_str()) {
                let fixture_path = resolve_manifest_fixture(&fixtures_root, &wire.fixture)?;
                let source =
                    fs::read(&fixture_path).map_err(|source| CatalogError::ReadFixture {
                        path: fixture_path.clone(),
                        source,
                    })?;
                let actual_hash = sha256_hex(&source);
                if actual_hash != wire.fixture_sha256 {
                    return Err(CatalogError::FixtureHashMismatch {
                        fixture: wire.fixture,
                        expected: wire.fixture_sha256,
                        actual: actual_hash,
                    });
                }

                let source_level = provenance.security_level(&wire.fixture, &source)?;
                if source_level != wire.site_config.security_level {
                    return Err(CatalogError::SourceValueMismatch {
                        fixture: wire.fixture,
                        source_level,
                        site_config_level: wire.site_config.security_level,
                    });
                }
            }

            let fixture = wire.fixture;
            let context = FixtureRenderContext {
                fixture: fixture.clone(),
                fixture_sha256: wire.fixture_sha256,
                provenance,
                security_level: wire.site_config.security_level,
            };
            if contexts.insert(fixture.clone(), context).is_some() {
                return Err(CatalogError::DuplicateFixture(fixture));
            }
        }

        Ok(Self {
            fixtures_root,
            contexts,
        })
    }

    pub fn context_for_fixture(
        &self,
        fixture_path: impl AsRef<Path>,
    ) -> Result<Option<&FixtureRenderContext>, CatalogError> {
        let relative = resolve_lookup_fixture(&self.fixtures_root, fixture_path.as_ref())?;
        let relative = relative_path_string(&relative)?;
        let Some(context) = self.contexts.get(&relative) else {
            return Ok(None);
        };

        let source_path = self.fixtures_root.join(&relative);
        let source = fs::read(&source_path).map_err(|source| CatalogError::ReadFixture {
            path: source_path,
            source,
        })?;
        let actual_hash = sha256_hex(&source);
        if actual_hash != context.fixture_sha256 {
            return Err(CatalogError::FixtureHashMismatch {
                fixture: relative,
                expected: context.fixture_sha256.clone(),
                actual: actual_hash,
            });
        }
        Ok(Some(context))
    }

    pub fn context_for_relative_fixture(
        &self,
        relative_fixture: &str,
    ) -> Result<Option<&FixtureRenderContext>, CatalogError> {
        validate_relative_fixture_path(relative_fixture)?;
        Ok(self.contexts.get(relative_fixture))
    }

    pub fn contexts(&self) -> impl Iterator<Item = &FixtureRenderContext> {
        self.contexts.values()
    }

    pub fn upsert_from_source(
        &mut self,
        relative_fixture: &str,
        source: &[u8],
    ) -> Result<bool, CatalogError> {
        validate_relative_fixture_path(relative_fixture)?;
        let fixture_path = resolve_manifest_fixture(&self.fixtures_root, relative_fixture)?;
        let committed_source =
            fs::read(&fixture_path).map_err(|source| CatalogError::ReadFixture {
                path: fixture_path,
                source,
            })?;
        if committed_source != source {
            return Err(CatalogError::SourceBytesMismatch(
                relative_fixture.to_string(),
            ));
        }

        match FixtureRenderContext::derive(relative_fixture, source)? {
            Some(context) => {
                if self.contexts.get(relative_fixture) == Some(&context) {
                    Ok(false)
                } else {
                    self.contexts.insert(relative_fixture.to_string(), context);
                    Ok(true)
                }
            }
            None => Ok(self.contexts.remove(relative_fixture).is_some()),
        }
    }

    pub fn remove(&mut self, relative_fixture: &str) -> Result<bool, CatalogError> {
        validate_relative_fixture_path(relative_fixture)?;
        Ok(self.contexts.remove(relative_fixture).is_some())
    }

    pub fn to_json(&self) -> Result<String, CatalogError> {
        let manifest = ManifestWire {
            schema_version: SCHEMA_VERSION,
            contexts: self.contexts.values().map(ContextWire::from).collect(),
        };
        let mut rendered =
            serde_json::to_string_pretty(&manifest).map_err(CatalogError::SerializeManifest)?;
        rendered.push('\n');
        Ok(rendered)
    }

    pub fn fixtures_root(&self) -> &Path {
        &self.fixtures_root
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ManifestWire {
    schema_version: u32,
    contexts: Vec<ContextWire>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ContextWire {
    fixture: String,
    fixture_sha256: String,
    provenance: ProvenanceWire,
    site_config: SiteConfigWire,
}

impl From<&FixtureRenderContext> for ContextWire {
    fn from(context: &FixtureRenderContext) -> Self {
        Self {
            fixture: context.fixture.clone(),
            fixture_sha256: context.fixture_sha256.clone(),
            provenance: ProvenanceWire::from(&context.provenance),
            site_config: SiteConfigWire {
                security_level: context.security_level,
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SiteConfigWire {
    security_level: SecurityLevel,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, tag = "kind")]
enum ProvenanceWire {
    #[serde(rename = "frontmatter")]
    Frontmatter {
        #[serde(rename = "configPath")]
        config_path: Vec<String>,
    },
    #[serde(rename = "directive")]
    Directive {
        directive: String,
        occurrence: usize,
        #[serde(rename = "configPath")]
        config_path: Vec<String>,
    },
}

impl Provenance {
    fn from_wire(wire: ProvenanceWire) -> Result<Self, CatalogError> {
        match wire {
            ProvenanceWire::Frontmatter { config_path } => {
                if !path_matches(&config_path, &FRONTMATTER_SECURITY_PATH) {
                    return Err(CatalogError::UnsupportedProvenancePath(config_path));
                }
                Ok(Self::Frontmatter { config_path })
            }
            ProvenanceWire::Directive {
                directive,
                occurrence,
                config_path,
            } => {
                if directive != "init" && directive != "initialize" {
                    return Err(CatalogError::UnsupportedDirective(directive));
                }
                if !path_matches(&config_path, &DIRECTIVE_SECURITY_PATH)
                    && !path_matches(&config_path, &NESTED_DIRECTIVE_SECURITY_PATH)
                {
                    return Err(CatalogError::UnsupportedProvenancePath(config_path));
                }
                Ok(Self::Directive {
                    directive,
                    occurrence,
                    config_path,
                })
            }
        }
    }

    fn security_level(&self, fixture: &str, source: &[u8]) -> Result<SecurityLevel, CatalogError> {
        let source =
            std::str::from_utf8(source).map_err(|source| CatalogError::InvalidFixtureEncoding {
                fixture: fixture.to_string(),
                source,
            })?;
        let source_level = match self {
            Self::Frontmatter { config_path } => {
                let frontmatter = parse_frontmatter(source, fixture)?.ok_or_else(|| {
                    CatalogError::MissingProvenanceSource {
                        fixture: fixture.to_string(),
                        provenance: "frontmatter".to_string(),
                    }
                })?;
                let value = value_at_owned_path(&frontmatter, config_path).ok_or_else(|| {
                    CatalogError::MissingProvenanceSource {
                        fixture: fixture.to_string(),
                        provenance: format!("{} at {}", self.kind(), self.config_path().join(".")),
                    }
                })?;
                parse_declared_security_level(fixture, value)?
            }
            Self::Directive {
                directive,
                occurrence,
                config_path,
            } => {
                let record = parse_directives(source)
                    .into_iter()
                    .find(|record| record.name == *directive && record.occurrence == *occurrence)
                    .ok_or_else(|| CatalogError::MissingProvenanceSource {
                        fixture: fixture.to_string(),
                        provenance: format!("{directive} directive occurrence {occurrence}"),
                    })?;
                let directive_value =
                    record
                        .value
                        .map_err(|error| CatalogError::InvalidDirective {
                            fixture: fixture.to_string(),
                            directive: directive.clone(),
                            occurrence: *occurrence,
                            error,
                        })?;
                let value =
                    value_at_owned_path(&directive_value, config_path).ok_or_else(|| {
                        CatalogError::MissingProvenanceSource {
                            fixture: fixture.to_string(),
                            provenance: format!(
                                "{} at {}",
                                self.kind(),
                                self.config_path().join(".")
                            ),
                        }
                    })?;
                parse_declared_security_level(fixture, value)?
            }
        };

        source_level.ok_or_else(|| CatalogError::DefaultSecurityLevelInContext {
            fixture: fixture.to_string(),
        })
    }

    fn kind(&self) -> &'static str {
        match self {
            Self::Frontmatter { .. } => "frontmatter",
            Self::Directive { .. } => "directive",
        }
    }

    fn config_path(&self) -> &[String] {
        match self {
            Self::Frontmatter { config_path } | Self::Directive { config_path, .. } => config_path,
        }
    }
}

impl From<&Provenance> for ProvenanceWire {
    fn from(provenance: &Provenance) -> Self {
        match provenance {
            Provenance::Frontmatter { config_path } => Self::Frontmatter {
                config_path: config_path.clone(),
            },
            Provenance::Directive {
                directive,
                occurrence,
                config_path,
            } => Self::Directive {
                directive: directive.clone(),
                occurrence: *occurrence,
                config_path: config_path.clone(),
            },
        }
    }
}

struct DirectiveRecord {
    name: String,
    occurrence: usize,
    value: Result<Value, String>,
}

fn parse_directives(source: &str) -> Vec<DirectiveRecord> {
    let mut records = Vec::new();
    let mut occurrences = HashMap::<String, usize>::new();
    let mut start = 0usize;

    while let Some(relative_start) = source[start..].find("%%{") {
        let content_start = start + relative_start + 3;
        let Some(relative_end) = source[content_start..].find("}%%") else {
            break;
        };
        let content_end = content_start + relative_end;
        let raw = source[content_start..content_end].trim();
        start = content_end + 3;

        let Some((name, arguments)) = raw.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name != "init" && name != "initialize" {
            continue;
        }

        let occurrence = occurrences.entry(name.to_string()).or_default();
        records.push(DirectiveRecord {
            name: name.to_string(),
            occurrence: *occurrence,
            value: json5::from_str(arguments.trim()).map_err(|error| error.to_string()),
        });
        *occurrence += 1;
    }

    records
}

fn parse_frontmatter(source: &str, fixture: &str) -> Result<Option<Value>, CatalogError> {
    let Some(after_open) = source.strip_prefix("---") else {
        return Ok(None);
    };
    let Some(open_line_end) = after_open.find('\n') else {
        return Ok(None);
    };
    if !after_open[..open_line_end].trim().is_empty() {
        return Ok(None);
    }

    let body_start = 3 + open_line_end + 1;
    let rest = &source[body_start..];
    let mut offset = 0usize;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n']).trim() == "---" {
            let yaml = &rest[..offset];
            let value =
                serde_saphyr::from_str(yaml).map_err(|error| CatalogError::InvalidFrontmatter {
                    fixture: fixture.to_string(),
                    error: error.to_string(),
                })?;
            return Ok(Some(value));
        }
        offset += line.len();
    }
    Ok(None)
}

fn parse_declared_security_level(
    fixture: &str,
    value: &Value,
) -> Result<Option<SecurityLevel>, CatalogError> {
    match value.as_str() {
        Some("loose") => Ok(Some(SecurityLevel::Loose)),
        Some("sandbox") => Ok(Some(SecurityLevel::Sandbox)),
        Some("strict") => Ok(None),
        Some(value) => Err(CatalogError::UnsupportedSecurityLevel {
            fixture: fixture.to_string(),
            value: value.to_string(),
        }),
        None => Err(CatalogError::NonStringSecurityLevel {
            fixture: fixture.to_string(),
        }),
    }
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    path.iter().try_fold(value, |value, key| value.get(key))
}

fn value_at_owned_path<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter()
        .try_fold(value, |value, key| value.get(key.as_str()))
}

fn path_strings(path: &[&str]) -> Vec<String> {
    path.iter().map(|segment| (*segment).to_string()).collect()
}

fn path_matches(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn sha256_hex(source: &[u8]) -> String {
    format!("{:x}", Sha256::digest(source))
}

fn validate_sha256(fixture: &str, value: &str) -> Result<(), CatalogError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(CatalogError::InvalidSha256 {
            fixture: fixture.to_string(),
            value: value.to_string(),
        })
    }
}

fn canonical_fixtures_root(root: &Path) -> Result<PathBuf, CatalogError> {
    let root = fs::canonicalize(root).map_err(|source| CatalogError::InvalidFixturesRoot {
        path: root.to_path_buf(),
        source,
    })?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err(CatalogError::FixturesRootNotDirectory(root))
    }
}

fn validate_relative_fixture_path(fixture: &str) -> Result<PathBuf, CatalogError> {
    if fixture.is_empty() || fixture.contains('\\') {
        return Err(CatalogError::InvalidFixturePath {
            fixture: fixture.to_string(),
            reason: "path must be a non-empty slash-separated relative path".to_string(),
        });
    }

    let path = Path::new(fixture);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(CatalogError::InvalidFixturePath {
            fixture: fixture.to_string(),
            reason: "absolute, current-directory, and parent-directory components are forbidden"
                .to_string(),
        });
    }
    if path.extension().and_then(|extension| extension.to_str()) != Some("mmd") {
        return Err(CatalogError::InvalidFixturePath {
            fixture: fixture.to_string(),
            reason: "fixture path must end in .mmd".to_string(),
        });
    }

    let normalized = relative_path_string(path)?;
    if normalized != fixture {
        return Err(CatalogError::InvalidFixturePath {
            fixture: fixture.to_string(),
            reason: "path is not in canonical slash-separated form".to_string(),
        });
    }
    Ok(path.to_path_buf())
}

fn resolve_manifest_fixture(root: &Path, fixture: &str) -> Result<PathBuf, CatalogError> {
    let relative = validate_relative_fixture_path(fixture)?;
    let candidate = root.join(relative);
    let canonical = fs::canonicalize(&candidate).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            CatalogError::MissingFixture(fixture.to_string())
        } else {
            CatalogError::ReadFixture {
                path: candidate.clone(),
                source,
            }
        }
    })?;
    if !canonical.starts_with(root) {
        return Err(CatalogError::EscapingFixturePath(fixture.to_string()));
    }
    if !canonical.is_file() {
        return Err(CatalogError::MissingFixture(fixture.to_string()));
    }
    Ok(canonical)
}

fn resolve_lookup_fixture(root: &Path, fixture: &Path) -> Result<PathBuf, CatalogError> {
    let candidate = if fixture.is_absolute() {
        fixture.to_path_buf()
    } else {
        root.join(validate_relative_fixture_path(
            fixture
                .to_str()
                .ok_or_else(|| CatalogError::NonUtf8FixturePath(fixture.to_path_buf()))?,
        )?)
    };
    let canonical = fs::canonicalize(&candidate).map_err(|source| CatalogError::ReadFixture {
        path: candidate,
        source,
    })?;
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| CatalogError::LookupOutsideFixturesRoot(canonical.clone()))?;
    validate_relative_fixture_path(&relative_path_string(relative)?)
}

fn relative_path_string(path: &Path) -> Result<String, CatalogError> {
    path.components()
        .map(|component| match component {
            Component::Normal(segment) => segment
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| CatalogError::NonUtf8FixturePath(path.to_path_buf())),
            _ => Err(CatalogError::InvalidFixturePath {
                fixture: path.display().to_string(),
                reason: "path is not relative and normalized".to_string(),
            }),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|components| components.join("/"))
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("failed to read fixture render context manifest {path}: {source}")]
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid fixture render context manifest: {0}")]
    ParseManifest(serde_json::Error),
    #[error("unsupported fixture render context schema version {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("failed to serialize fixture render context manifest: {0}")]
    SerializeManifest(serde_json::Error),
    #[error("invalid fixtures root {path}: {source}")]
    InvalidFixturesRoot {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("fixtures root is not a directory: {0}")]
    FixturesRootNotDirectory(PathBuf),
    #[error("invalid fixture path {fixture:?}: {reason}")]
    InvalidFixturePath { fixture: String, reason: String },
    #[error("fixture path is not UTF-8: {0}")]
    NonUtf8FixturePath(PathBuf),
    #[error("fixture path escapes the fixtures root: {0}")]
    EscapingFixturePath(String),
    #[error("fixture lookup is outside the fixtures root: {0}")]
    LookupOutsideFixturesRoot(PathBuf),
    #[error("fixture render context references missing fixture {0}")]
    MissingFixture(String),
    #[error("failed to read fixture {path}: {source}")]
    ReadFixture {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("fixture {fixture} is not valid UTF-8: {source}")]
    InvalidFixtureEncoding {
        fixture: String,
        source: std::str::Utf8Error,
    },
    #[error("fixture {fixture} has invalid YAML frontmatter: {error}")]
    InvalidFrontmatter { fixture: String, error: String },
    #[error("fixture {fixture} has invalid {directive} directive occurrence {occurrence}: {error}")]
    InvalidDirective {
        fixture: String,
        directive: String,
        occurrence: usize,
        error: String,
    },
    #[error("fixture {fixture} has unsupported securityLevel {value:?}")]
    UnsupportedSecurityLevel { fixture: String, value: String },
    #[error("fixture {fixture} has a non-string securityLevel")]
    NonStringSecurityLevel { fixture: String },
    #[error(
        "fixture {fixture} has multiple securityLevel declarations ({count}); provenance is ambiguous"
    )]
    AmbiguousSecurityDeclarations { fixture: String, count: usize },
    #[error(
        "fixture {fixture} uses strict/default securityLevel and must not have a render context"
    )]
    DefaultSecurityLevelInContext { fixture: String },
    #[error("fixture {fixture} has invalid SHA-256 {value:?}")]
    InvalidSha256 { fixture: String, value: String },
    #[error("fixture {fixture} SHA-256 drifted: expected {expected}, actual {actual}")]
    FixtureHashMismatch {
        fixture: String,
        expected: String,
        actual: String,
    },
    #[error("fixture {0} source bytes do not match the imported file")]
    SourceBytesMismatch(String),
    #[error("duplicate fixture render context for {0}")]
    DuplicateFixture(String),
    #[error("unsupported fixture render context provenance path {}", .0.join("."))]
    UnsupportedProvenancePath(Vec<String>),
    #[error("unsupported fixture render context directive {0:?}")]
    UnsupportedDirective(String),
    #[error("fixture {fixture} is missing provenance source {provenance}")]
    MissingProvenanceSource { fixture: String, provenance: String },
    #[error(
        "fixture {fixture} source securityLevel {source_level:?} does not match siteConfig {site_config_level:?}"
    )]
    SourceValueMismatch {
        fixture: String,
        source_level: SecurityLevel,
        site_config_level: SecurityLevel,
    },
}
