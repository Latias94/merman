//! Label-level SVG metric reporting helpers for compare commands.

use crate::XtaskError;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

pub(crate) const DEFAULT_LABEL_DELTA_REPORT_LIMIT: LabelDeltaReportLimit =
    LabelDeltaReportLimit::Top(80);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelDeltaReportLimit {
    Top(usize),
    All,
}

impl LabelDeltaReportLimit {
    fn take_count(self, total: usize) -> usize {
        match self {
            Self::Top(limit) => total.min(limit),
            Self::All => total,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LabelMetricDelta {
    pub(crate) stem: String,
    pub(crate) index: usize,
    pub(crate) label_class: String,
    pub(crate) text: String,
    pub(crate) markup: String,
    pub(crate) upstream_width: f64,
    pub(crate) local_width: f64,
    pub(crate) width_delta: f64,
    pub(crate) upstream_height: f64,
    pub(crate) local_height: f64,
    pub(crate) height_delta: f64,
}

#[derive(Debug, Clone)]
struct LabelMetricSample {
    label_class: String,
    text: String,
    markup: String,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum C4RelationLabelRole {
    Message,
    Technology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct C4RelationLabelKey {
    pub(crate) relation_index: u64,
    pub(crate) role: C4RelationLabelRole,
}

/// Text anchor and basis vectors in root SVG user space, without reapplying the root viewBox.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WorldTextGeometry {
    pub(crate) anchor_x: f64,
    pub(crate) anchor_y: f64,
    pub(crate) x_axis_x: f64,
    pub(crate) x_axis_y: f64,
    pub(crate) y_axis_x: f64,
    pub(crate) y_axis_y: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticLabelEvidence {
    pub(crate) text: String,
    pub(crate) geometry: WorldTextGeometry,
    pub(crate) presentation: SemanticLabelPresentation,
    pub(crate) associated_edge: Option<SemanticRelationEdgeEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticLabelPresentation {
    pub(crate) attributes: BTreeMap<String, String>,
    pub(crate) inline_style: Vec<(String, String)>,
    pub(crate) class_tokens: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticRelationEdgeEvidence {
    pub(crate) presentation: SemanticLabelPresentation,
    pub(crate) geometry: SemanticRelationEdgeGeometry,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SemanticRelationEdgeGeometry {
    tag: String,
    attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SemanticLabelPair<K> {
    pub(crate) key: K,
    pub(crate) upstream: SemanticLabelEvidence,
    pub(crate) local: SemanticLabelEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticIdentitySetMismatch<K> {
    pub(crate) missing_from_local: Vec<K>,
    pub(crate) missing_from_upstream: Vec<K>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SemanticLabelError {
    #[error("invalid SVG XML: {0}")]
    InvalidSvg(String),
    #[error("invalid `{attribute}` coordinate `{value}` for semantic label `{text}`")]
    InvalidCoordinate {
        text: String,
        attribute: &'static str,
        value: String,
    },
    #[error("invalid SVG transform `{value}`: {message}")]
    InvalidTransform { value: String, message: String },
    #[error("non-finite semantic label geometry: {context}")]
    NonFiniteGeometry { context: String },
    #[error("C4 relation number is invalid in `{text}`")]
    InvalidRelationNumber { text: String },
    #[error("duplicate C4 semantic label identity: {0:?}")]
    DuplicateIdentity(C4RelationLabelKey),
    #[error("C4 technology label has no owning relation message: `{text}`")]
    OrphanTechnology { text: String },
    #[error("C4 relation message has no immediately preceding path or line: `{text}`")]
    MissingRelationEdge { text: String },
    #[error("C4 relation parent contains consecutive unowned path or line elements")]
    AmbiguousRelationEdge,
    #[error("C4 relation parent ends with an unowned path or line element")]
    OrphanRelationEdge,
    #[error("registered C4 semantic label fixture contains no stylesheet")]
    MissingStylesheet,
    #[error("invalid inline style declaration `{declaration}` for semantic label `{text}`")]
    InvalidInlineStyle { text: String, declaration: String },
    #[error(
        "C4 semantic label identity sets differ: missing from local={missing_from_local:?}, missing from upstream={missing_from_upstream:?}"
    )]
    IdentitySetMismatch {
        missing_from_local: Vec<C4RelationLabelKey>,
        missing_from_upstream: Vec<C4RelationLabelKey>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SemanticLabelGateEvidence {
    pub(crate) compared_samples: usize,
    pub(crate) accepted_residuals: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticLabelGateOutcome {
    pub(crate) evidence: SemanticLabelGateEvidence,
    pub(crate) issues: Vec<String>,
}

const C4_DYNAMIC_LABEL_FIXTURE: &str = "upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010";
const C4_SEMANTIC_LABEL_FIXTURES: &[&str] = &[C4_DYNAMIC_LABEL_FIXTURE];
const LABEL_RESIDUAL_SCHEMA_VERSION: u32 = 2;
const LABEL_COMPARATOR_REVISION: &str = "semantic-label-v2";
const MAX_LABEL_GEOMETRY_DECIMALS: u32 = 6;
const LABEL_RESIDUAL_CATALOG: &str =
    include_str!("../../../../../fixtures/_verification/label-geometry-residuals.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelResidualCatalog {
    schema_version: u32,
    mermaid_version: String,
    mermaid_source_commit: String,
    comparator_revision: String,
    decimals: u32,
    entries: Vec<LabelResidualEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelResidualEntry {
    diagram: String,
    fixture: String,
    relation_index: u64,
    role: C4RelationLabelRole,
    text: String,
    input_sha256: String,
    upstream_svg_sha256: String,
    evidence_kind: LabelResidualEvidenceKind,
    reason: String,
    upstream: RoundedWorldTextGeometry,
    local: RoundedWorldTextGeometry,
    #[serde(default)]
    edge_upstream: Option<SemanticRelationEdgeGeometry>,
    #[serde(default)]
    edge_local: Option<SemanticRelationEdgeGeometry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LabelResidualEvidenceKind {
    BrowserMeasurement,
    SourceBackedLayoutApproximation,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoundedWorldTextGeometry {
    anchor_x: f64,
    anchor_y: f64,
    x_axis_x: f64,
    x_axis_y: f64,
    y_axis_x: f64,
    y_axis_y: f64,
}

impl RoundedWorldTextGeometry {
    fn from_geometry(geometry: WorldTextGeometry, decimals: u32) -> Self {
        Self {
            anchor_x: round_for_comparison(geometry.anchor_x, decimals),
            anchor_y: round_for_comparison(geometry.anchor_y, decimals),
            x_axis_x: round_for_comparison(geometry.x_axis_x, decimals),
            x_axis_y: round_for_comparison(geometry.x_axis_y, decimals),
            y_axis_x: round_for_comparison(geometry.y_axis_x, decimals),
            y_axis_y: round_for_comparison(geometry.y_axis_y, decimals),
        }
    }

    fn values(self) -> [f64; 6] {
        [
            self.anchor_x,
            self.anchor_y,
            self.x_axis_x,
            self.x_axis_y,
            self.y_axis_x,
            self.y_axis_y,
        ]
    }

    fn is_finite(self) -> bool {
        self.values().into_iter().all(f64::is_finite)
    }

    fn is_quantized(self, decimals: u32) -> bool {
        self.values()
            .into_iter()
            .all(|value| round_for_comparison(value, decimals) == value)
    }
}

fn round_for_comparison(value: f64, decimals: u32) -> f64 {
    debug_assert!(decimals <= MAX_LABEL_GEOMETRY_DECIMALS);
    let scale = 10_f64.powi(decimals as i32);
    (value * scale + 0.5).floor() / scale
}

#[derive(Debug, Clone, Copy)]
struct AffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl AffineTransform {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn from_svg(transform: svgtypes::Transform) -> Self {
        Self {
            a: transform.a,
            b: transform.b,
            c: transform.c,
            d: transform.d,
            e: transform.e,
            f: transform.f,
        }
    }

    fn multiply(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            e: self.a * child.e + self.c * child.f + self.e,
            f: self.b * child.e + self.d * child.f + self.f,
        }
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn is_finite(self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .into_iter()
            .all(f64::is_finite)
    }
}

pub(crate) fn pair_complete_semantic_label_maps<K>(
    upstream: BTreeMap<K, SemanticLabelEvidence>,
    mut local: BTreeMap<K, SemanticLabelEvidence>,
) -> Result<Vec<SemanticLabelPair<K>>, SemanticIdentitySetMismatch<K>>
where
    K: Clone + Ord,
{
    // A partial intersection would hide missing or ambiguously owned labels.
    let upstream_keys = upstream.keys().cloned().collect::<BTreeSet<_>>();
    let local_keys = local.keys().cloned().collect::<BTreeSet<_>>();
    if upstream_keys != local_keys {
        return Err(SemanticIdentitySetMismatch {
            missing_from_local: upstream_keys.difference(&local_keys).cloned().collect(),
            missing_from_upstream: local_keys.difference(&upstream_keys).cloned().collect(),
        });
    }

    let mut pairs = Vec::with_capacity(upstream.len());
    for (key, upstream) in upstream {
        let Some(local) = local.remove(&key) else {
            return Err(SemanticIdentitySetMismatch {
                missing_from_local: vec![key],
                missing_from_upstream: Vec::new(),
            });
        };
        pairs.push(SemanticLabelPair {
            key,
            upstream,
            local,
        });
    }
    Ok(pairs)
}

pub(crate) fn pair_c4_relation_labels(
    upstream_svg: &str,
    local_svg: &str,
) -> Result<Vec<SemanticLabelPair<C4RelationLabelKey>>, SemanticLabelError> {
    let upstream = extract_c4_relation_labels(upstream_svg)?;
    let local = extract_c4_relation_labels(local_svg)?;
    pair_complete_semantic_label_maps(upstream, local).map_err(|mismatch| {
        SemanticLabelError::IdentitySetMismatch {
            missing_from_local: mismatch.missing_from_local,
            missing_from_upstream: mismatch.missing_from_upstream,
        }
    })
}

pub(crate) fn compare_registered_semantic_labels(
    diagram: &str,
    stem: &str,
    input_text: &str,
    upstream_svg: &str,
    local_svg: &str,
    decimals: u32,
) -> Result<Option<SemanticLabelGateOutcome>, String> {
    if !is_registered_semantic_label_fixture(diagram, stem) {
        return Ok(None);
    }

    let pairs = pair_c4_relation_labels(upstream_svg, local_svg).map_err(|error| {
        format!("semantic label extraction failed for {diagram}/{stem}: {error}")
    })?;
    if pairs.is_empty() {
        return Err(format!(
            "registered semantic label fixture {diagram}/{stem} produced no label samples"
        ));
    }
    let catalog = load_label_residual_catalog(decimals)?;
    let fixture_entries = catalog
        .entries
        .iter()
        .filter(|entry| entry.diagram == diagram && entry.fixture == stem)
        .collect::<Vec<_>>();
    validate_fixture_residual_digests(
        diagram,
        stem,
        &fixture_entries,
        &crate::util::sha256_hex(input_text.as_bytes()),
        &crate::util::sha256_hex(upstream_svg.as_bytes()),
    )?;
    let mut accepted_keys = BTreeSet::new();
    let mut failures = Vec::new();

    let upstream_stylesheet = extract_c4_stylesheet_signature(upstream_svg).map_err(|error| {
        format!("semantic stylesheet extraction failed for {diagram}/{stem}: {error}")
    })?;
    let local_stylesheet = extract_c4_stylesheet_signature(local_svg).map_err(|error| {
        format!("semantic stylesheet extraction failed for {diagram}/{stem}: {error}")
    })?;
    if upstream_stylesheet != local_stylesheet {
        failures.push(format!(
            "{diagram}/{stem}: fixture stylesheet source differs"
        ));
    }

    for pair in &pairs {
        if pair.upstream.text != pair.local.text {
            failures.push(format!(
                "{diagram}/{stem} {:?}: label text differs: upstream={:?}, local={:?}",
                pair.key, pair.upstream.text, pair.local.text
            ));
            continue;
        }
        if pair.upstream.presentation != pair.local.presentation {
            failures.push(format!(
                "{diagram}/{stem} {:?}: explicit label presentation differs: upstream={:?}, local={:?}",
                pair.key, pair.upstream.presentation, pair.local.presentation
            ));
            continue;
        }

        let (upstream_edge, local_edge) = match (
            pair.upstream.associated_edge.as_ref(),
            pair.local.associated_edge.as_ref(),
        ) {
            (Some(upstream), Some(local)) => (Some(upstream), Some(local)),
            (None, None) => (None, None),
            _ => {
                failures.push(format!(
                    "{diagram}/{stem} {:?}: associated relation edge presence differs",
                    pair.key
                ));
                continue;
            }
        };
        if let (Some(upstream), Some(local)) = (upstream_edge, local_edge)
            && upstream.presentation != local.presentation
        {
            failures.push(format!(
                "{diagram}/{stem} {:?}: associated relation edge presentation differs: upstream={:?}, local={:?}",
                pair.key, upstream.presentation, local.presentation
            ));
            continue;
        }

        let upstream = RoundedWorldTextGeometry::from_geometry(pair.upstream.geometry, decimals);
        let local = RoundedWorldTextGeometry::from_geometry(pair.local.geometry, decimals);
        let edge_geometry_differs = upstream_edge
            .zip(local_edge)
            .is_some_and(|(upstream, local)| upstream.geometry != local.geometry);
        if upstream == local && !edge_geometry_differs {
            continue;
        }

        let accepted = fixture_entries.iter().any(|entry| {
            entry.relation_index == pair.key.relation_index
                && entry.role == pair.key.role
                && entry.text == pair.upstream.text
                && entry.upstream == upstream
                && entry.local == local
                && match (upstream_edge, local_edge) {
                    (Some(upstream_edge), Some(local_edge)) => {
                        entry.edge_upstream.as_ref() == Some(&upstream_edge.geometry)
                            && entry.edge_local.as_ref() == Some(&local_edge.geometry)
                    }
                    (None, None) => entry.edge_upstream.is_none() && entry.edge_local.is_none(),
                    _ => false,
                }
        });
        if accepted {
            accepted_keys.insert(pair.key);
        } else {
            failures.push(format!(
                "{diagram}/{stem} {:?}: label or associated edge geometry differs without an exact residual contract: label-upstream={upstream:?}, label-local={local:?}, edge-upstream={:?}, edge-local={:?}",
                pair.key,
                upstream_edge.map(|edge| &edge.geometry),
                local_edge.map(|edge| &edge.geometry),
            ));
        }
    }

    let catalog_keys = fixture_entries
        .iter()
        .map(|entry| C4RelationLabelKey {
            relation_index: entry.relation_index,
            role: entry.role,
        })
        .collect::<BTreeSet<_>>();
    let stale_entries = catalog_keys
        .difference(&accepted_keys)
        .copied()
        .collect::<Vec<_>>();
    if !stale_entries.is_empty() {
        failures.push(format!(
            "{diagram}/{stem}: stale semantic label residual contracts were not exercised: {stale_entries:?}"
        ));
    }

    Ok(Some(SemanticLabelGateOutcome {
        evidence: SemanticLabelGateEvidence {
            compared_samples: pairs.len(),
            accepted_residuals: accepted_keys.len(),
        },
        issues: failures,
    }))
}

pub(crate) fn registered_semantic_label_fixtures(diagram: &str) -> &'static [&'static str] {
    if diagram == "c4" {
        C4_SEMANTIC_LABEL_FIXTURES
    } else {
        &[]
    }
}

fn is_registered_semantic_label_fixture(diagram: &str, fixture: &str) -> bool {
    registered_semantic_label_fixtures(diagram).contains(&fixture)
}

fn load_label_residual_catalog(decimals: u32) -> Result<LabelResidualCatalog, String> {
    let catalog = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, decimals)?;
    validate_label_residual_artifacts(&catalog)?;
    Ok(catalog)
}

fn parse_label_residual_catalog(json: &str, decimals: u32) -> Result<LabelResidualCatalog, String> {
    let catalog = serde_json::from_str::<LabelResidualCatalog>(json)
        .map_err(|error| format!("invalid semantic label residual catalog: {error}"))?;
    validate_label_residual_contract(&catalog, decimals)?;
    Ok(catalog)
}

fn validate_label_residual_contract(
    catalog: &LabelResidualCatalog,
    decimals: u32,
) -> Result<(), String> {
    if decimals > MAX_LABEL_GEOMETRY_DECIMALS {
        return Err(format!(
            "semantic label comparison decimals {decimals} exceed the supported maximum {MAX_LABEL_GEOMETRY_DECIMALS}"
        ));
    }
    if catalog.decimals > MAX_LABEL_GEOMETRY_DECIMALS {
        return Err(format!(
            "semantic label residual catalog decimals {} exceed the supported maximum {MAX_LABEL_GEOMETRY_DECIMALS}",
            catalog.decimals
        ));
    }
    if catalog.schema_version != LABEL_RESIDUAL_SCHEMA_VERSION {
        return Err(format!(
            "semantic label residual catalog schema {} is unsupported; expected {LABEL_RESIDUAL_SCHEMA_VERSION}",
            catalog.schema_version
        ));
    }
    if catalog.mermaid_version != merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION
        || catalog.mermaid_source_commit != crate::cmd::MERMAID_SOURCE_COMMIT
        || catalog.comparator_revision != LABEL_COMPARATOR_REVISION
        || catalog.decimals != decimals
    {
        return Err(format!(
            "semantic label residual catalog provenance does not match the comparator: version={} commit={} revision={} decimals={}",
            catalog.mermaid_version,
            catalog.mermaid_source_commit,
            catalog.comparator_revision,
            catalog.decimals
        ));
    }

    let mut keys = BTreeSet::new();
    for entry in &catalog.entries {
        if !is_registered_semantic_label_fixture(&entry.diagram, &entry.fixture) {
            return Err(format!(
                "semantic label residual {}/{} is not registered by comparator revision {LABEL_COMPARATOR_REVISION}",
                entry.diagram, entry.fixture
            ));
        }
        if entry.relation_index == 0 || entry.text.trim().is_empty() {
            return Err(format!(
                "semantic label residual {}/{} relation {} {:?} has an invalid identity or empty text",
                entry.diagram, entry.fixture, entry.relation_index, entry.role
            ));
        }
        if !keys.insert((
            entry.diagram.as_str(),
            entry.fixture.as_str(),
            entry.relation_index,
            entry.role,
        )) {
            return Err(format!(
                "duplicate semantic label residual key for {}/{} relation {} {:?}",
                entry.diagram, entry.fixture, entry.relation_index, entry.role
            ));
        }
        for (role, digest) in [
            ("input", entry.input_sha256.as_str()),
            ("upstream SVG", entry.upstream_svg_sha256.as_str()),
        ] {
            if !crate::util::is_canonical_sha256(digest) {
                return Err(format!(
                    "semantic label residual {}/{} relation {} {:?} {role} SHA-256 must be 64 lowercase hexadecimal characters",
                    entry.diagram, entry.fixture, entry.relation_index, entry.role
                ));
            }
        }
        if entry.reason.trim().is_empty() {
            return Err(format!(
                "semantic label residual {}/{} relation {} {:?} has an empty reason",
                entry.diagram, entry.fixture, entry.relation_index, entry.role
            ));
        }
        match entry.evidence_kind {
            LabelResidualEvidenceKind::BrowserMeasurement
            | LabelResidualEvidenceKind::SourceBackedLayoutApproximation => {}
        }
        for (side, geometry) in [("upstream", entry.upstream), ("local", entry.local)] {
            if !geometry.is_finite() {
                return Err(format!(
                    "semantic label residual {}/{} relation {} {:?} has non-finite {side} geometry",
                    entry.diagram, entry.fixture, entry.relation_index, entry.role
                ));
            }
            if !geometry.is_quantized(catalog.decimals) {
                return Err(format!(
                    "semantic label residual {}/{} relation {} {:?} {side} geometry exceeds declared precision {}",
                    entry.diagram,
                    entry.fixture,
                    entry.relation_index,
                    entry.role,
                    catalog.decimals
                ));
            }
        }
        if entry.edge_upstream.is_none() || entry.edge_local.is_none() {
            return Err(format!(
                "semantic label residual {}/{} relation {} {:?} has an invalid associated edge contract",
                entry.diagram, entry.fixture, entry.relation_index, entry.role
            ));
        }
    }
    Ok(())
}

fn validate_label_residual_artifacts(catalog: &LabelResidualCatalog) -> Result<(), String> {
    for entry in &catalog.entries {
        let input_path = crate::cmd::fixtures_root()
            .join(&entry.diagram)
            .join(format!("{}.mmd", entry.fixture));
        let upstream_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(&entry.diagram)
            .join(format!("{}.svg", entry.fixture));
        for (role, path, expected) in [
            ("input", input_path, entry.input_sha256.as_str()),
            (
                "upstream SVG",
                upstream_path,
                entry.upstream_svg_sha256.as_str(),
            ),
        ] {
            let bytes = std::fs::read(&path).map_err(|error| {
                format!(
                    "read semantic label residual {role} {}: {error}",
                    path.display()
                )
            })?;
            let actual = crate::util::sha256_hex(&bytes);
            if actual != expected {
                return Err(format!(
                    "semantic label residual {}/{} {role} SHA-256 drifted: expected {expected}, found {actual}",
                    entry.diagram, entry.fixture
                ));
            }
        }
    }
    Ok(())
}

fn validate_fixture_residual_digests(
    diagram: &str,
    fixture: &str,
    entries: &[&LabelResidualEntry],
    actual_input_sha256: &str,
    actual_upstream_svg_sha256: &str,
) -> Result<(), String> {
    for entry in entries {
        if entry.input_sha256 != actual_input_sha256
            || entry.upstream_svg_sha256 != actual_upstream_svg_sha256
        {
            return Err(format!(
                "semantic label residual {diagram}/{fixture} is not bound to the compared artifacts: input expected={} actual={actual_input_sha256}, upstream SVG expected={} actual={actual_upstream_svg_sha256}",
                entry.input_sha256, entry.upstream_svg_sha256
            ));
        }
    }
    Ok(())
}

pub(crate) fn extract_c4_relation_labels(
    svg: &str,
) -> Result<BTreeMap<C4RelationLabelKey, SemanticLabelEvidence>, SemanticLabelError> {
    let normalized = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(normalized.as_ref())
        .map_err(|error| SemanticLabelError::InvalidSvg(error.to_string()))?;
    let mut evidence = BTreeMap::new();

    for parent in document.descendants().filter(roxmltree::Node::is_element) {
        let has_relation_message = parent
            .children()
            .filter(roxmltree::Node::is_element)
            .filter(|node| node.has_tag_name("text"))
            .map(svg_text_content)
            .any(|text| parse_c4_relation_index(&text).transpose().is_some());
        if !has_relation_message {
            continue;
        }

        extract_c4_relation_parent(parent, &mut evidence)?;
    }

    Ok(evidence)
}

fn extract_c4_relation_parent(
    parent: roxmltree::Node<'_, '_>,
    evidence: &mut BTreeMap<C4RelationLabelKey, SemanticLabelEvidence>,
) -> Result<(), SemanticLabelError> {
    let mut pending_edge = None;
    let mut current_relation = None;
    for node in parent.children().filter(roxmltree::Node::is_element) {
        if node.has_tag_name("path") || node.has_tag_name("line") {
            if pending_edge.is_some() {
                return Err(SemanticLabelError::AmbiguousRelationEdge);
            }
            pending_edge = Some(semantic_relation_edge_evidence(node)?);
            current_relation = None;
            continue;
        }
        if !node.has_tag_name("text") {
            continue;
        }

        let text = svg_text_content(node);
        if let Some(relation_index) = parse_c4_relation_index(&text)? {
            let associated_edge = pending_edge
                .take()
                .ok_or_else(|| SemanticLabelError::MissingRelationEdge { text: text.clone() })?;
            current_relation = Some((relation_index, associated_edge.clone()));
            insert_c4_relation_evidence(
                evidence,
                C4RelationLabelKey {
                    relation_index,
                    role: C4RelationLabelRole::Message,
                },
                node,
                text,
                Some(associated_edge),
            )?;
            continue;
        }

        if is_c4_technology_label(&text) {
            let (relation_index, associated_edge) = current_relation
                .as_ref()
                .ok_or_else(|| SemanticLabelError::OrphanTechnology { text: text.clone() })?;
            insert_c4_relation_evidence(
                evidence,
                C4RelationLabelKey {
                    relation_index: *relation_index,
                    role: C4RelationLabelRole::Technology,
                },
                node,
                text,
                Some(associated_edge.clone()),
            )?;
        }
    }
    if pending_edge.is_some() {
        return Err(SemanticLabelError::OrphanRelationEdge);
    }
    Ok(())
}

fn insert_c4_relation_evidence(
    evidence: &mut BTreeMap<C4RelationLabelKey, SemanticLabelEvidence>,
    key: C4RelationLabelKey,
    node: roxmltree::Node<'_, '_>,
    text: String,
    associated_edge: Option<SemanticRelationEdgeEvidence>,
) -> Result<(), SemanticLabelError> {
    let sample = SemanticLabelEvidence {
        geometry: world_text_geometry(node, &text)?,
        presentation: semantic_label_presentation(node, &text)?,
        associated_edge,
        text,
    };
    if evidence.insert(key, sample).is_some() {
        return Err(SemanticLabelError::DuplicateIdentity(key));
    }
    Ok(())
}

fn semantic_label_presentation(
    node: roxmltree::Node<'_, '_>,
    text: &str,
) -> Result<SemanticLabelPresentation, SemanticLabelError> {
    semantic_element_presentation(node, text, &["x", "y", "transform"])
}

fn semantic_relation_edge_evidence(
    node: roxmltree::Node<'_, '_>,
) -> Result<SemanticRelationEdgeEvidence, SemanticLabelError> {
    const GEOMETRY_ATTRIBUTES: &[&str] = &["d", "x1", "y1", "x2", "y2", "transform"];
    let attributes = GEOMETRY_ATTRIBUTES
        .iter()
        .filter_map(|attribute| {
            node.attribute(*attribute)
                .map(|value| ((*attribute).to_string(), value.trim().to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    Ok(SemanticRelationEdgeEvidence {
        presentation: semantic_element_presentation(node, "C4 relation edge", GEOMETRY_ATTRIBUTES)?,
        geometry: SemanticRelationEdgeGeometry {
            tag: node.tag_name().name().to_string(),
            attributes,
        },
    })
}

fn semantic_element_presentation(
    node: roxmltree::Node<'_, '_>,
    context: &str,
    root_geometry_attributes: &[&str],
) -> Result<SemanticLabelPresentation, SemanticLabelError> {
    let mut attributes = BTreeMap::new();
    let mut inline_style = Vec::new();
    let mut class_tokens = BTreeSet::new();

    let mut ancestors = node
        .ancestors()
        .filter(roxmltree::Node::is_element)
        .skip(1)
        .collect::<Vec<_>>();
    ancestors.reverse();
    for (ancestor_index, element) in ancestors.into_iter().enumerate() {
        collect_element_presentation(
            element,
            &format!("ancestor[{ancestor_index}]:{}", element.tag_name().name()),
            context,
            &[],
            true,
            &mut attributes,
            &mut inline_style,
            &mut class_tokens,
        )?;
    }

    let descendants = node
        .descendants()
        .filter(roxmltree::Node::is_element)
        .filter(|descendant| *descendant != node);
    for (element_index, element) in std::iter::once(node).chain(descendants).enumerate() {
        let scope = if element_index == 0 {
            "root".to_string()
        } else {
            format!("{}[{element_index}]", element.tag_name().name())
        };
        collect_element_presentation(
            element,
            &scope,
            context,
            if element_index == 0 {
                root_geometry_attributes
            } else {
                &[]
            },
            false,
            &mut attributes,
            &mut inline_style,
            &mut class_tokens,
        )?;
    }

    Ok(SemanticLabelPresentation {
        attributes,
        inline_style,
        class_tokens,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_element_presentation(
    element: roxmltree::Node<'_, '_>,
    scope: &str,
    context: &str,
    excluded_attributes: &[&str],
    presentation_attributes_only: bool,
    attributes: &mut BTreeMap<String, String>,
    inline_style: &mut Vec<(String, String)>,
    class_tokens: &mut BTreeSet<String>,
) -> Result<(), SemanticLabelError> {
    for attribute in element.attributes() {
        if matches!(attribute.name(), "style" | "class")
            || excluded_attributes.contains(&attribute.name())
            || (presentation_attributes_only
                && !is_inherited_presentation_attribute(attribute.name()))
        {
            continue;
        }
        attributes.insert(
            format!("{scope}@{}", attribute.name()),
            attribute.value().trim().to_string(),
        );
    }
    class_tokens.extend(
        element
            .attribute("class")
            .unwrap_or_default()
            .split_whitespace()
            .map(|token| format!("{scope}@{token}")),
    );
    inline_style.extend(
        parse_inline_style_declarations(element.attribute("style").unwrap_or_default(), context)?
            .into_iter()
            .filter(|(property, _)| {
                !presentation_attributes_only
                    || property.starts_with("--")
                    || is_inherited_presentation_attribute(&property.to_ascii_lowercase())
            })
            .map(|(property, value)| (format!("{scope}@{property}"), value)),
    );
    Ok(())
}

fn is_inherited_presentation_attribute(attribute: &str) -> bool {
    matches!(
        attribute,
        "alignment-baseline"
            | "clip-path"
            | "color"
            | "display"
            | "dominant-baseline"
            | "fill"
            | "fill-opacity"
            | "filter"
            | "font-family"
            | "font-size"
            | "font-style"
            | "font-weight"
            | "marker-end"
            | "marker-mid"
            | "marker-start"
            | "mask"
            | "opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-dashoffset"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-opacity"
            | "stroke-width"
            | "text-anchor"
            | "vector-effect"
            | "visibility"
    )
}

fn parse_inline_style_declarations(
    style: &str,
    context: &str,
) -> Result<Vec<(String, String)>, SemanticLabelError> {
    let mut input = cssparser::ParserInput::new(style);
    let mut parser = cssparser::Parser::new(&mut input);
    let mut declarations = Vec::new();

    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }
        let start = parser.position();
        let parsed = parser.parse_until_after(cssparser::Delimiter::Semicolon, |declaration| {
            declaration.skip_whitespace();
            let property = declaration.expect_ident_cloned()?.to_string();
            declaration.expect_colon()?;
            let value_start = declaration.position();
            while declaration.next_including_whitespace_and_comments().is_ok() {}
            let value = declaration.slice_from(value_start).trim().to_string();
            if value.is_empty() {
                return Err(declaration.new_custom_error(()));
            }
            Ok::<_, cssparser::ParseError<'_, ()>>((property, value))
        });
        match parsed {
            Ok(declaration) => declarations.push(declaration),
            Err(_) => {
                let end = parser.position();
                return Err(SemanticLabelError::InvalidInlineStyle {
                    text: context.to_string(),
                    declaration: parser.slice(start..end).trim().to_string(),
                });
            }
        }
    }

    Ok(declarations)
}

fn extract_c4_stylesheet_signature(svg: &str) -> Result<Vec<String>, SemanticLabelError> {
    let normalized = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(normalized.as_ref())
        .map_err(|error| SemanticLabelError::InvalidSvg(error.to_string()))?;
    let stylesheets = document
        .descendants()
        .filter(|node| node.is_element() && node.has_tag_name("style"))
        .map(|node| node.text().unwrap_or_default().trim().to_string())
        .collect::<Vec<_>>();
    if stylesheets.is_empty() {
        Err(SemanticLabelError::MissingStylesheet)
    } else {
        Ok(stylesheets)
    }
}

fn parse_c4_relation_index(text: &str) -> Result<Option<u64>, SemanticLabelError> {
    let Some((candidate, _)) = text.split_once(':') else {
        return Ok(None);
    };
    if candidate.is_empty() || !candidate.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(None);
    }

    candidate
        .parse::<u64>()
        .map(Some)
        .map_err(|_| SemanticLabelError::InvalidRelationNumber {
            text: text.to_string(),
        })
}

fn is_c4_technology_label(text: &str) -> bool {
    text.starts_with('[') && text.ends_with(']')
}

fn world_text_geometry(
    node: roxmltree::Node<'_, '_>,
    text: &str,
) -> Result<WorldTextGeometry, SemanticLabelError> {
    let x = finite_coordinate(node, "x", text)?;
    let y = finite_coordinate(node, "y", text)?;
    let mut chain = node
        .ancestors()
        .filter(roxmltree::Node::is_element)
        .collect::<Vec<_>>();
    chain.reverse();

    let mut world = AffineTransform::IDENTITY;
    for ancestor in chain {
        let Some(value) = ancestor.attribute("transform") else {
            continue;
        };
        let parsed = value.parse::<svgtypes::Transform>().map_err(|error| {
            SemanticLabelError::InvalidTransform {
                value: value.to_string(),
                message: error.to_string(),
            }
        })?;
        let local = AffineTransform::from_svg(parsed);
        if !local.is_finite() {
            return Err(SemanticLabelError::NonFiniteGeometry {
                context: format!("transform `{value}` for `{text}`"),
            });
        }
        world = world.multiply(local);
        if !world.is_finite() {
            return Err(SemanticLabelError::NonFiniteGeometry {
                context: format!("composed transform for `{text}`"),
            });
        }
    }

    let (anchor_x, anchor_y) = world.apply(x, y);
    let geometry = WorldTextGeometry {
        anchor_x,
        anchor_y,
        x_axis_x: world.a,
        x_axis_y: world.b,
        y_axis_x: world.c,
        y_axis_y: world.d,
    };
    if [
        geometry.anchor_x,
        geometry.anchor_y,
        geometry.x_axis_x,
        geometry.x_axis_y,
        geometry.y_axis_x,
        geometry.y_axis_y,
    ]
    .into_iter()
    .all(f64::is_finite)
    {
        Ok(geometry)
    } else {
        Err(SemanticLabelError::NonFiniteGeometry {
            context: format!("world geometry for `{text}`"),
        })
    }
}

fn finite_coordinate(
    node: roxmltree::Node<'_, '_>,
    attribute: &'static str,
    text: &str,
) -> Result<f64, SemanticLabelError> {
    let value = node.attribute(attribute).unwrap_or_default();
    let coordinate = value
        .parse::<f64>()
        .map_err(|_| SemanticLabelError::InvalidCoordinate {
            text: text.to_string(),
            attribute,
            value: value.to_string(),
        })?;
    if coordinate.is_finite() {
        Ok(coordinate)
    } else {
        Err(SemanticLabelError::NonFiniteGeometry {
            context: format!("coordinate `{attribute}={value}` for `{text}`"),
        })
    }
}

pub(crate) fn parse_label_delta_report_limit(
    value: Option<&str>,
) -> Result<LabelDeltaReportLimit, XtaskError> {
    let value = value.ok_or(XtaskError::Usage)?.trim();
    if value.eq_ignore_ascii_case("all") {
        return Ok(LabelDeltaReportLimit::All);
    }
    let limit = value.parse::<usize>().map_err(|_| XtaskError::Usage)?;
    if limit == 0 {
        return Err(XtaskError::Usage);
    }
    Ok(LabelDeltaReportLimit::Top(limit))
}

pub(crate) fn collect_label_metric_deltas(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
) -> Result<Vec<LabelMetricDelta>, String> {
    let upstream =
        extract_label_metric_samples(upstream_svg).map_err(|e| format!("upstream {stem}: {e}"))?;
    let local =
        extract_label_metric_samples(local_svg).map_err(|e| format!("local {stem}: {e}"))?;

    if upstream.len() != local.len() {
        return Err(format!(
            "label metric sample count mismatch for {stem}: upstream={}, local={}",
            upstream.len(),
            local.len()
        ));
    }

    let mut out = Vec::new();
    for (idx, (up, lo)) in upstream.iter().zip(&local).enumerate() {
        let width_delta = lo.width - up.width;
        let height_delta = lo.height - up.height;
        if width_delta.abs() < 0.0005 && height_delta.abs() < 0.0005 {
            continue;
        }

        out.push(LabelMetricDelta {
            stem: stem.to_string(),
            index: idx,
            label_class: if !lo.label_class.is_empty() {
                lo.label_class.clone()
            } else {
                up.label_class.clone()
            },
            text: if !lo.text.is_empty() {
                lo.text.clone()
            } else {
                up.text.clone()
            },
            markup: if !lo.markup.is_empty() {
                lo.markup.clone()
            } else {
                up.markup.clone()
            },
            upstream_width: up.width,
            local_width: lo.width,
            width_delta,
            upstream_height: up.height,
            local_height: lo.height,
            height_delta,
        });
    }

    Ok(out)
}

pub(crate) fn write_label_deltas_report(
    report: &mut String,
    label_deltas: &mut [LabelMetricDelta],
    limit: LabelDeltaReportLimit,
) {
    if label_deltas.is_empty() {
        return;
    }

    label_deltas.sort_by(|a, b| {
        let aw = a.width_delta.abs().max(a.height_delta.abs());
        let bw = b.width_delta.abs().max(b.height_delta.abs());
        aw.partial_cmp(&bw)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });

    let take = limit.take_count(label_deltas.len());
    let _ = writeln!(
        report,
        "\n## Label Metric Deltas\n\nHTML `<foreignObject>` labels and SVG `<text>` labels are paired by fixture-local DOM order. SVG text rows use emitted label-container geometry when no browser `getBBox()` dimensions are present. This report identifies shared text metric drift without changing production rendering.\n"
    );
    match limit {
        LabelDeltaReportLimit::All => {
            let _ = writeln!(
                report,
                "Showing all {} label delta rows.\n",
                label_deltas.len()
            );
        }
        LabelDeltaReportLimit::Top(_) => {
            let _ = writeln!(
                report,
                "Showing top {take} of {} label delta rows. Use `--report-label-all` or `--report-label-limit all` for a full audit table.\n",
                label_deltas.len()
            );
        }
    }

    let _ = writeln!(
        report,
        "| Fixture | # | class | upstream w×h | local w×h | Δw | Δh | text | markup |\n|---|---:|---|---:|---:|---:|---:|---|---|"
    );
    for d in label_deltas.iter().take(take) {
        let _ = writeln!(
            report,
            "| `{}` | {} | `{}` | {:.3}×{:.3} | {:.3}×{:.3} | {:+.3} | {:+.3} | {} | {} |",
            d.stem,
            d.index,
            markdown_cell(&d.label_class),
            d.upstream_width,
            d.upstream_height,
            d.local_width,
            d.local_height,
            d.width_delta,
            d.height_delta,
            markdown_cell(&d.text),
            markdown_cell(&d.markup),
        );
    }
}

fn extract_label_metric_samples(svg: &str) -> Result<Vec<LabelMetricSample>, String> {
    let svg = crate::svgdom::normalize_xml_entities(svg);
    let doc = roxmltree::Document::parse(svg.as_ref()).map_err(|e| e.to_string())?;
    let mut out = Vec::new();

    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.has_tag_name("foreignObject") {
            out.push(foreignobject_label_metric_sample(node));
        } else if let Some(sample) = svg_text_label_metric_sample(node) {
            out.push(sample);
        }
    }

    Ok(out)
}

fn foreignobject_label_metric_sample(fo: roxmltree::Node<'_, '_>) -> LabelMetricSample {
    let width = fo
        .attribute("width")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let height = fo
        .attribute("height")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let label_class = fo
        .descendants()
        .find(|n| {
            n.has_tag_name("span")
                && n.attribute("class")
                    .unwrap_or_default()
                    .split_whitespace()
                    .any(|t| t.ends_with("Label"))
        })
        .and_then(|n| n.attribute("class"))
        .unwrap_or_default()
        .to_string();

    LabelMetricSample {
        label_class,
        text: foreignobject_text(fo),
        markup: foreignobject_markup_summary(fo),
        width,
        height,
    }
}

fn svg_text_label_metric_sample(label_group: roxmltree::Node<'_, '_>) -> Option<LabelMetricSample> {
    if !label_group.has_tag_name("g")
        || !(has_class_token(label_group, "label") || has_class_token(label_group, "cluster-label"))
        || label_group
            .descendants()
            .any(|n| n.has_tag_name("foreignObject"))
    {
        return None;
    }

    let text_node = label_group.descendants().find(|n| n.has_tag_name("text"))?;
    let text = svg_text_content(text_node);
    if text.is_empty() {
        return None;
    }

    let (width, height) = svg_text_label_container_size(label_group)?;
    Some(LabelMetricSample {
        label_class: svg_text_label_class(label_group),
        text,
        markup: svg_text_markup_summary(text_node),
        width,
        height,
    })
}

fn foreignobject_text(fo: roxmltree::Node<'_, '_>) -> String {
    let mut raw = String::new();
    for n in fo.descendants() {
        if n.is_element() {
            match n.tag_name().name() {
                "br" => raw.push('\n'),
                "p" if !raw.is_empty() && !raw.ends_with('\n') => {
                    raw.push('\n');
                }
                _ => {}
            }
        }
        if n.is_text()
            && let Some(t) = n.text()
        {
            raw.push_str(t);
        }
    }

    raw.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\\n")
}

fn foreignobject_markup_summary(fo: roxmltree::Node<'_, '_>) -> String {
    let mut parts = Vec::new();
    for n in fo.descendants().filter(|n| n.is_element()) {
        let name = n.tag_name().name();
        if !matches!(
            name,
            "i" | "img" | "strong" | "b" | "em" | "code" | "br" | "math" | "svg"
        ) {
            continue;
        }

        let class = n.attribute("class").unwrap_or_default();
        if class.is_empty() {
            parts.push(name.to_string());
        } else {
            parts.push(format!(
                "{}.{}",
                name,
                class.split_whitespace().collect::<Vec<_>>().join(".")
            ));
        }
    }
    parts.join(" ")
}

fn svg_text_content(text: roxmltree::Node<'_, '_>) -> String {
    let mut lines = Vec::new();
    for outer in text
        .children()
        .filter(|n| n.has_tag_name("tspan") && has_class_token(*n, "text-outer-tspan"))
    {
        let line = normalize_text_line(&collect_text(outer));
        if !line.is_empty() {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        let line = normalize_text_line(&collect_text(text));
        if !line.is_empty() {
            lines.push(line);
        }
    }

    lines.join("\\n")
}

fn svg_text_markup_summary(text: roxmltree::Node<'_, '_>) -> String {
    let mut parts = vec![format!("svgText:{}lines", svg_text_line_count(text).max(1))];
    for node in text.descendants().filter(|n| n.has_tag_name("tspan")) {
        if let Some(weight) = node.attribute("font-weight")
            && weight != "normal"
        {
            parts.push(format!("weight:{weight}"));
        }
        if let Some(style) = node.attribute("font-style")
            && style != "normal"
        {
            parts.push(format!("style:{style}"));
        }
    }
    parts.sort();
    parts.dedup();
    parts.join(" ")
}

fn svg_text_line_count(text: roxmltree::Node<'_, '_>) -> usize {
    text.children()
        .filter(|n| n.has_tag_name("tspan") && has_class_token(*n, "text-outer-tspan"))
        .count()
}

fn svg_text_label_class(label_group: roxmltree::Node<'_, '_>) -> String {
    if self_or_ancestor_has_class(label_group, "edgeLabel") {
        "edgeLabel".to_string()
    } else if self_or_ancestor_has_class(label_group, "cluster-label") {
        "clusterLabel".to_string()
    } else if self_or_ancestor_has_class(label_group, "node") {
        "nodeLabel".to_string()
    } else {
        label_group
            .attribute("class")
            .unwrap_or_default()
            .to_string()
    }
}

fn svg_text_label_container_size(label_group: roxmltree::Node<'_, '_>) -> Option<(f64, f64)> {
    let owner = nearest_label_owner(label_group)?;
    owner
        .descendants()
        .filter(|n| n.is_element() && has_class_token(*n, "label-container"))
        .filter(|n| !is_descendant_of(*n, label_group))
        .find_map(element_bbox_size)
}

fn nearest_label_owner<'a, 'input>(
    label_group: roxmltree::Node<'a, 'input>,
) -> Option<roxmltree::Node<'a, 'input>> {
    let mut current = Some(label_group);
    while let Some(node) = current {
        if has_class_token(node, "node")
            || has_class_token(node, "edgeLabel")
            || has_class_token(node, "cluster")
        {
            return Some(node);
        }
        current = node.parent();
    }
    None
}

fn element_bbox_size(node: roxmltree::Node<'_, '_>) -> Option<(f64, f64)> {
    match node.tag_name().name() {
        "rect" | "image" | "foreignObject" => {
            Some((attr_f64(node, "width")?, attr_f64(node, "height")?))
        }
        "circle" => {
            let r = attr_f64(node, "r")?;
            Some((r * 2.0, r * 2.0))
        }
        "ellipse" => Some((attr_f64(node, "rx")? * 2.0, attr_f64(node, "ry")? * 2.0)),
        "polygon" | "polyline" => bbox_size_from_points(node.attribute("points")?),
        _ => None,
    }
}

fn bbox_size_from_points(points: &str) -> Option<(f64, f64)> {
    let nums = points
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if nums.len() < 4 || nums.len() % 2 != 0 {
        return None;
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for pair in nums.chunks_exact(2) {
        min_x = min_x.min(pair[0]);
        max_x = max_x.max(pair[0]);
        min_y = min_y.min(pair[1]);
        max_y = max_y.max(pair[1]);
    }
    Some((max_x - min_x, max_y - min_y))
}

fn attr_f64(node: roxmltree::Node<'_, '_>, name: &str) -> Option<f64> {
    node.attribute(name)?.parse().ok()
}

fn collect_text(node: roxmltree::Node<'_, '_>) -> String {
    let mut raw = String::new();
    for n in node.descendants() {
        if n.is_text()
            && let Some(text) = n.text()
        {
            raw.push_str(text);
        }
    }
    raw
}

fn normalize_text_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
    node.attribute("class")
        .unwrap_or_default()
        .split_whitespace()
        .any(|part| part == token)
}

fn self_or_ancestor_has_class(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
    let mut current = Some(node);
    while let Some(n) = current {
        if has_class_token(n, token) {
            return true;
        }
        current = n.parent();
    }
    false
}

fn is_descendant_of(node: roxmltree::Node<'_, '_>, ancestor: roxmltree::Node<'_, '_>) -> bool {
    let mut current = node.parent();
    while let Some(n) = current {
        if n == ancestor {
            return true;
        }
        current = n.parent();
    }
    false
}

fn markdown_cell(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_c4_dynamic_svg() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../fixtures/upstream-svgs/c4/upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010.svg",
        );
        std::fs::read_to_string(path).unwrap()
    }

    fn signed_c4_dynamic_source() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/c4/upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010.mmd");
        std::fs::read_to_string(path).unwrap()
    }

    fn c4_dynamic_svg_with_reviewed_residuals() -> String {
        let mut local = signed_c4_dynamic_svg();
        for (upstream, reviewed) in [
            (
                r#"x="487.198474097224" y="641.5""#,
                r#"x="487.2846949188151" y="645.5""#,
            ),
            (
                r#"x="487.198474097224" y="658.5""#,
                r#"x="487.2846949188151" y="662.5""#,
            ),
            (
                r#"x="493.6410154384848" y="897""#,
                r#"x="493.9486587427764" y="902""#,
            ),
            (
                r#"x="577.6205949910768" y="436.5""#,
                r#"x="577.4387868033255" y="439.5""#,
            ),
            (
                r#"x="577.6205949910768" y="453.5""#,
                r#"x="577.4387868033255" y="456.5""#,
            ),
            (
                r#"x1="439.64775725593665" y1="476" x2="403.74919093851133" y2="887""#,
                r#"x1="439.71853146853147" y1="479" x2="403.8508583690987" y2="892""#,
            ),
            (
                r#"d="M456.28688524590166,887 Q457.46395034219324,837 460.99514563106794,787""#,
                r#"d="M456.76797385620915 892 Q457.8583162994928 842 461.1293436293436 792""#,
            ),
            (
                r#"d="M450.8915857605178,682 Q452.75609037579727,476.5 458.3496042216359,271""#,
                r#"d="M450.9431330472103 686 Q452.6909599252679 479.5 457.93444055944053 273""#,
            ),
        ] {
            assert!(local.contains(upstream));
            local = local.replacen(upstream, reviewed, 1);
        }
        local
    }

    #[test]
    fn label_metric_deltas_extract_text_and_icon_markup() {
        let upstream = r#"<svg><foreignObject width="85.0625" height="24"><div><span class="nodeLabel"><p><i class="fa fa-twitter"></i> for peace</p></span></div></foreignObject></svg>"#;
        let local = r#"<svg><foreignObject width="89.0625" height="24"><div><span class="nodeLabel"><p><i class="fa fa-twitter"></i> for peace</p></span></div></foreignObject></svg>"#;

        let rows = collect_label_metric_deltas("fixture", upstream, local).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label_class, "nodeLabel");
        assert_eq!(rows[0].text, "for peace");
        assert_eq!(rows[0].markup, "i.fa.fa-twitter");
        assert_eq!(rows[0].width_delta, 4.0);
    }

    #[test]
    fn label_metric_deltas_extract_svg_text_container_geometry() {
        let upstream = r#"<svg><g class="node default"><polygon class="label-container" points="-10,0 110,0 100,-40 0,-40"/><g class="label"><text><tspan class="text-outer-tspan"><tspan font-weight="bold">Hello</tspan></tspan><tspan class="text-outer-tspan"><tspan>World</tspan></tspan></text></g></g></svg>"#;
        let local = r#"<svg><g class="node default"><polygon class="label-container" points="-10,0 108,0 98,-40 0,-40"/><g class="label"><text><tspan class="text-outer-tspan"><tspan font-weight="bold">Hello</tspan></tspan><tspan class="text-outer-tspan"><tspan>World</tspan></tspan></text></g></g></svg>"#;

        let rows = collect_label_metric_deltas("fixture", upstream, local).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label_class, "nodeLabel");
        assert_eq!(rows[0].text, "Hello\\nWorld");
        assert!(rows[0].markup.contains("svgText:2lines"));
        assert!(rows[0].markup.contains("weight:bold"));
        assert_eq!(rows[0].upstream_width, 120.0);
        assert_eq!(rows[0].local_width, 118.0);
        assert_eq!(rows[0].width_delta, -2.0);
        assert_eq!(rows[0].height_delta, 0.0);
    }

    #[test]
    fn label_metric_deltas_reject_mismatched_sample_counts() {
        let upstream = r#"<svg><foreignObject width="10" height="10"><span class="nodeLabel">one</span></foreignObject></svg>"#;
        let local = r#"<svg><foreignObject width="10" height="10"><span class="nodeLabel">one</span></foreignObject><foreignObject width="10" height="10"><span class="nodeLabel">two</span></foreignObject></svg>"#;

        let error = collect_label_metric_deltas("fixture", upstream, local).unwrap_err();

        assert!(error.contains("label metric sample count mismatch"));
        assert!(error.contains("upstream=1"));
        assert!(error.contains("local=2"));
    }

    #[test]
    fn c4_relation_labels_pair_by_semantic_identity_after_dom_reordering() {
        let upstream = r#"
            <svg xmlns="http://www.w3.org/2000/svg" transform="translate(10 20)">
              <g transform="scale(2)">
                <path d="M0 0L1 1"/>
                <text x="3" y="4">1: calls</text>
                <text x="3" y="5">[HTTP]</text>
                <path d="M0 0L1 1"/>
                <text x="7" y="8">2: returns</text>
              </g>
            </svg>
        "#;
        let local = r#"
            <svg xmlns="http://www.w3.org/2000/svg" transform="translate(10 20)">
              <g transform="scale(2)">
                <path d="M0 0L1 1"/>
                <text x="7" y="8">2: returns</text>
                <path d="M0 0L1 1"/>
                <text x="3" y="4">1: calls</text>
                <text x="3" y="5">[HTTP]</text>
              </g>
            </svg>
        "#;

        let pairs = pair_c4_relation_labels(upstream, local).unwrap();

        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].key.relation_index, 1);
        assert_eq!(pairs[0].key.role, C4RelationLabelRole::Message);
        assert_eq!(pairs[0].upstream.geometry.anchor_x, 16.0);
        assert_eq!(pairs[0].upstream.geometry.anchor_y, 28.0);
        assert_eq!(pairs[0].upstream.geometry.x_axis_x, 2.0);
        assert_eq!(pairs[0].upstream.geometry.x_axis_y, 0.0);
        assert_eq!(pairs[0].upstream.geometry.y_axis_x, 0.0);
        assert_eq!(pairs[0].upstream.geometry.y_axis_y, 2.0);
        assert_eq!(pairs[0].upstream, pairs[0].local);
        assert_eq!(pairs[1].key.role, C4RelationLabelRole::Technology);
        assert_eq!(pairs[2].key.relation_index, 2);
    }

    #[test]
    fn c4_relation_labels_reject_missing_identity_on_either_side() {
        let upstream = r#"<svg><g><path/><text x="1" y="2">1: calls</text><text x="1" y="3">[HTTP]</text></g></svg>"#;
        let local = r#"<svg><g><path/><text x="1" y="2">1: calls</text></g></svg>"#;

        let error = pair_c4_relation_labels(upstream, local).unwrap_err();

        assert!(matches!(
            error,
            SemanticLabelError::IdentitySetMismatch {
                missing_from_local,
                missing_from_upstream,
            } if missing_from_local == vec![C4RelationLabelKey {
                relation_index: 1,
                role: C4RelationLabelRole::Technology,
            }] && missing_from_upstream.is_empty()
        ));
    }

    #[test]
    fn c4_relation_labels_reject_duplicate_identity() {
        let svg = r#"
            <svg><g>
              <path/><text x="1" y="2">1: first</text>
              <path/><text x="3" y="4">1: duplicate</text>
            </g></svg>
        "#;

        let error = extract_c4_relation_labels(svg).unwrap_err();

        assert!(matches!(
            error,
            SemanticLabelError::DuplicateIdentity(C4RelationLabelKey {
                relation_index: 1,
                role: C4RelationLabelRole::Message,
            })
        ));
    }

    #[test]
    fn c4_relation_labels_reject_orphan_technology() {
        let svg = r#"
            <svg><g>
              <text x="1" y="2">[HTTP]</text>
              <path/><text x="3" y="4">1: calls</text>
            </g></svg>
        "#;

        let error = extract_c4_relation_labels(svg).unwrap_err();

        assert!(matches!(
            error,
            SemanticLabelError::OrphanTechnology { text } if text == "[HTTP]"
        ));
    }

    #[test]
    fn c4_relation_labels_reject_unparseable_or_non_finite_geometry() {
        let invalid_transform = r#"<svg><g transform="translate(nope)"><path/><text x="1" y="2">1: calls</text></g></svg>"#;
        let overflowing_transform = r#"<svg transform="scale(1e308)"><g transform="scale(1e308)"><path/><text x="1" y="2">1: calls</text></g></svg>"#;
        let non_finite_coordinate =
            r#"<svg><g><path/><text x="1e999" y="2">1: calls</text></g></svg>"#;

        assert!(matches!(
            extract_c4_relation_labels(invalid_transform),
            Err(SemanticLabelError::InvalidTransform { .. })
        ));
        assert!(matches!(
            extract_c4_relation_labels(overflowing_transform),
            Err(SemanticLabelError::NonFiniteGeometry { .. })
        ));
        assert!(matches!(
            extract_c4_relation_labels(non_finite_coordinate),
            Err(SemanticLabelError::NonFiniteGeometry { .. })
        ));
    }

    #[test]
    fn c4_relation_labels_extract_signed_dynamic_fixture_coordinates() {
        let svg = signed_c4_dynamic_svg();

        let labels = extract_c4_relation_labels(&svg).unwrap();
        let message = labels
            .get(&C4RelationLabelKey {
                relation_index: 2,
                role: C4RelationLabelRole::Message,
            })
            .unwrap();

        assert_eq!(labels.len(), 5);
        assert_eq!(message.text, "2: Calls isAuthenticated() on");
        assert_eq!(message.geometry.anchor_x, 493.6410154384848);
        assert_eq!(message.geometry.anchor_y, 897.0);
    }

    #[test]
    fn c4_semantic_label_gate_accepts_all_exact_reviewed_residuals() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();

        let outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &local,
            3,
        )
        .unwrap()
        .unwrap();

        assert!(outcome.issues.is_empty());
        assert_eq!(outcome.evidence.compared_samples, 5);
        assert_eq!(outcome.evidence.accepted_residuals, 5);
    }

    #[test]
    fn c4_semantic_label_gate_rejects_zero_sample_selector_drift() {
        let source = signed_c4_dynamic_source();
        let error = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            "<svg/>",
            "<svg/>",
            3,
        )
        .unwrap_err();

        assert!(error.contains("produced no label samples"));
    }

    #[test]
    fn c4_semantic_label_gate_rejects_previous_large_offset() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals().replacen(
            r#"x="493.9486587427764" y="902""#,
            r#"x="593.9486587427764" y="842""#,
            1,
        );

        let outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &local,
            3,
        )
        .unwrap()
        .unwrap();
        let error = outcome.issues.join("\n");

        assert!(
            error.contains("label or associated edge geometry differs without an exact residual")
        );
        assert!(error.contains("anchor_x: 593.949"));
        assert!(error.contains("anchor_y: 842.0"));
        assert!(error.contains("stale semantic label residual contracts were not exercised"));
        assert_eq!(outcome.evidence.compared_samples, 5);
    }

    #[test]
    fn c4_semantic_label_gate_rejects_text_and_explicit_style_mutations() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();
        let changed_text = local.replacen(
            "2: Calls isAuthenticated() on",
            "2: Calls isAuthorized() on",
            1,
        );
        let changed_style = local.replacen("fill=\"red\"", "fill=\"blue\"", 1);
        let changed_child_style = local.replace(
            "alignment-baseline=\"mathematical\"",
            "alignment-baseline=\"hanging\"",
        );

        let text_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_text,
            3,
        )
        .unwrap()
        .unwrap();
        let child_style_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_child_style,
            3,
        )
        .unwrap()
        .unwrap();
        let style_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_style,
            3,
        )
        .unwrap()
        .unwrap();

        assert!(
            text_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("label text differs"))
        );
        assert!(
            style_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("explicit label presentation differs"))
        );
        assert!(
            child_style_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("explicit label presentation differs"))
        );
    }

    #[test]
    fn c4_semantic_label_gate_rejects_edge_geometry_and_presentation_mutations() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();
        let changed_path = local.replacen(
            "M456.76797385620915 892 Q457.8583162994928 842 461.1293436293436 792",
            "M456.76797385620915 892 Q457.8583162994928 842 462.1293436293436 792",
            1,
        );
        let changed_stroke = local.replacen(
            r##"stroke="#444444" marker-end"##,
            r##"stroke="#abcdef" marker-end"##,
            1,
        );

        let path_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_path,
            3,
        )
        .unwrap()
        .unwrap();
        let stroke_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_stroke,
            3,
        )
        .unwrap()
        .unwrap();

        assert!(path_outcome.issues.iter().any(|issue| {
            issue.contains("associated edge geometry differs without an exact residual")
        }));
        assert!(
            stroke_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("associated relation edge presentation differs"))
        );
    }

    #[test]
    fn c4_semantic_label_gate_rejects_stylesheet_and_ancestor_style_mutations() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();
        let changed_stylesheet = local.replacen(
            ".marker{fill:#333333;stroke:#333333;}",
            ".marker{fill:#abcdef;stroke:#333333;}",
            1,
        );
        let changed_ancestor = local.replacen(
            "<g><line x1=\"439.71853146853147\"",
            "<g style=\"opacity: .5\"><line x1=\"439.71853146853147\"",
            1,
        );

        let stylesheet_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_stylesheet,
            3,
        )
        .unwrap()
        .unwrap();
        let ancestor_outcome = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &changed_ancestor,
            3,
        )
        .unwrap()
        .unwrap();

        assert!(
            stylesheet_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("fixture stylesheet source differs"))
        );
        assert!(
            ancestor_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("explicit label presentation differs"))
        );
    }

    #[test]
    fn c4_semantic_label_gate_rejects_missing_relation_edge_owner() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();
        let path = format!(
            r##"<path fill="none" stroke-width="1" stroke="#444444" d="M456.76797385620915 892 Q457.8583162994928 842 461.1293436293436 792" marker-end="url(#{C4_DYNAMIC_LABEL_FIXTURE}-arrowhead)"/>"##
        );
        assert!(local.contains(&path));
        let missing_edge = local.replacen(&path, "", 1);

        let error = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            &missing_edge,
            3,
        )
        .unwrap_err();

        assert!(error.contains("has no immediately preceding path or line"));
    }

    #[test]
    fn semantic_label_residuals_are_bound_to_compared_artifact_bytes() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();

        let input_error = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &format!("{source}\n"),
            &upstream,
            &local,
            3,
        )
        .unwrap_err();
        let upstream_error = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &format!("{upstream}<!-- drift -->"),
            &local,
            3,
        )
        .unwrap_err();

        assert!(input_error.contains("is not bound to the compared artifacts"));
        assert!(upstream_error.contains("is not bound to the compared artifacts"));
    }

    #[test]
    fn inline_style_parser_preserves_css_cascade_significant_source() {
        let declarations = parse_inline_style_declarations(
            r#"fill:red!important;fill:blue;font-family:"Open  Sans";--Theme: A"#,
            "test",
        )
        .unwrap();

        assert_eq!(
            declarations,
            vec![
                ("fill".to_string(), "red!important".to_string()),
                ("fill".to_string(), "blue".to_string()),
                ("font-family".to_string(), r#""Open  Sans""#.to_string()),
                ("--Theme".to_string(), "A".to_string()),
            ]
        );
        assert_ne!(
            declarations,
            parse_inline_style_declarations("fill:blue", "test").unwrap()
        );
    }

    #[test]
    fn label_residual_catalog_rejects_duplicate_and_stale_entries() {
        let mut json = serde_json::from_str::<serde_json::Value>(LABEL_RESIDUAL_CATALOG).unwrap();
        let duplicate = json["entries"][0].clone();
        json["entries"].as_array_mut().unwrap().push(duplicate);
        let duplicate_error = parse_label_residual_catalog(&json.to_string(), 3).unwrap_err();
        assert!(duplicate_error.contains("duplicate semantic label residual key"));

        let mut stale = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        stale.entries[0].input_sha256 = "0".repeat(64);
        let stale_error = validate_label_residual_artifacts(&stale).unwrap_err();
        assert!(stale_error.contains("input SHA-256 drifted"));
    }

    #[test]
    fn label_residual_catalog_rejects_invalid_contract_fields() {
        let valid = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();

        let mut schema = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        schema.schema_version += 1;
        assert!(validate_label_residual_contract(&schema, 3).is_err());

        let mut provenance = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        provenance.comparator_revision.push_str("-stale");
        assert!(validate_label_residual_contract(&provenance, 3).is_err());

        let mut hash = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        hash.entries[0].input_sha256 = "A".repeat(64);
        assert!(validate_label_residual_contract(&hash, 3).is_err());

        let mut reason = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        reason.entries[0].reason = "  ".to_string();
        assert!(validate_label_residual_contract(&reason, 3).is_err());

        let mut geometry = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        geometry.entries[0].local.anchor_x = f64::INFINITY;
        assert!(validate_label_residual_contract(&geometry, 3).is_err());

        let mut catalog_decimals = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        catalog_decimals.decimals = 7;
        assert!(
            validate_label_residual_contract(&catalog_decimals, 3)
                .unwrap_err()
                .contains("catalog decimals 7 exceed")
        );

        assert!(
            parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 7)
                .unwrap_err()
                .contains("exceed the supported maximum")
        );
        assert_eq!(valid.entries.len(), 5);
    }
}
