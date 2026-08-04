//! Label-level SVG metric reporting helpers for compare commands.

use crate::XtaskError;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SemanticLabelAdapter {
    C4,
    FlowchartElk,
    DagreDataId,
    Architecture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SemanticLabelFixtureContract {
    diagram: &'static str,
    fixture: &'static str,
    input_sha256: &'static str,
    upstream_svg_sha256: &'static str,
    adapter: SemanticLabelAdapter,
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
    descendant_geometry: Vec<SemanticDescendantLabelGeometry>,
    pub(crate) presentation: SemanticLabelPresentation,
    pub(crate) associated_edge: Option<SemanticRelationEdgeEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticDescendantLabelGeometry {
    scope: String,
    width: Option<String>,
    height: Option<String>,
    transform: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticLabelPresentation {
    pub(crate) element_structure: Vec<String>,
    pub(crate) attributes: BTreeMap<String, String>,
    pub(crate) inline_style: Vec<(String, String)>,
    pub(crate) class_tokens: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SemanticRelationEdgeEvidence {
    pub(crate) presentation: SemanticLabelPresentation,
    pub(crate) geometry: SemanticRelationEdgeGeometry,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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
    #[error("registered semantic label fixture contains no stylesheet")]
    MissingStylesheet,
    #[error("registered semantic label fixture contains no relevant stylesheet rules")]
    MissingRelevantStylesheet,
    #[error("invalid stylesheet rule `{rule}`: {message}")]
    InvalidStylesheet { rule: String, message: String },
    #[error("{diagram} semantic edge identity is empty")]
    EmptyEdgeIdentity { diagram: &'static str },
    #[error("duplicate {diagram} semantic edge identity `{identity}`")]
    DuplicateEdgeIdentity {
        diagram: &'static str,
        identity: String,
    },
    #[error("{diagram} semantic label `{identity}` has no owning edge")]
    OrphanEdgeLabel {
        diagram: &'static str,
        identity: String,
    },
    #[error("{diagram} semantic edge `{identity}` has no owning label")]
    MissingEdgeLabel {
        diagram: &'static str,
        identity: String,
    },
    #[error("{diagram} semantic edge group has {edge_count} edge paths and {label_count} labels")]
    AmbiguousEdgeGroup {
        diagram: &'static str,
        edge_count: usize,
        label_count: usize,
    },
    #[error("{diagram} semantic edge `{identity}` has an empty label")]
    EmptyEdgeLabel {
        diagram: &'static str,
        identity: String,
    },
    #[error("invalid `{attribute}` geometry for semantic edge `{identity}`: {message}")]
    InvalidEdgeGeometry {
        identity: String,
        attribute: String,
        message: String,
    },
    #[error("invalid inline style declaration `{declaration}` for semantic label `{text}`")]
    InvalidInlineStyle { text: String, declaration: String },
    #[error(
        "C4 semantic label identity sets differ: missing from local={missing_from_local:?}, missing from upstream={missing_from_upstream:?}"
    )]
    IdentitySetMismatch {
        missing_from_local: Vec<C4RelationLabelKey>,
        missing_from_upstream: Vec<C4RelationLabelKey>,
    },
    #[error(
        "{diagram} semantic edge identity sets differ: missing from local={missing_from_local:?}, missing from upstream={missing_from_upstream:?}"
    )]
    StableIdentitySetMismatch {
        diagram: &'static str,
        missing_from_local: Vec<String>,
        missing_from_upstream: Vec<String>,
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
const FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE: &str = "upstream_cypress_flowchart_elk_spec_74_elk_handle_labels_for_multiple_edges_from_and_to_the_same_cou_034";
const FLOWCHART_SEMANTIC_LABEL_FIXTURES: &[&str] = &[FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE];
const CLASSIC_DAGRE_INACTIVE_NEO_SELECTORS: &[&str] = &[
    ".node .neo-node",
    r#"[data-look="neo"].node rect,[data-look="neo"].cluster rect,[data-look="neo"].node polygon"#,
    r#"[data-look="neo"].swimlane.cluster rect"#,
    r#"[data-look="neo"].node path"#,
    r#"[data-look="neo"].node .outer-path"#,
    r#"[data-look="neo"].node .neo-line path"#,
    r#"[data-look="neo"].node circle"#,
    r#"[data-look="neo"].node circle .state-start"#,
    r#"[data-look="neo"].icon-shape .icon"#,
    r#"[data-look="neo"].icon-shape .icon-neo path"#,
    "[data-look=neo].labelBkg",
];
const ARCHITECTURE_PARALLEL_LABEL_FIXTURE: &str =
    "stress_architecture_batch3_parallel_edges_and_labels_057";
const ARCHITECTURE_SEMANTIC_LABEL_FIXTURES: &[&str] = &[ARCHITECTURE_PARALLEL_LABEL_FIXTURE];
const REQUIREMENT_TRACES_LABEL_FIXTURE: &str =
    "upstream_cypress_requirementdiagram_unified_spec_example_003";
const REQUIREMENT_SEMANTIC_LABEL_FIXTURES: &[&str] = &[REQUIREMENT_TRACES_LABEL_FIXTURE];
const STATE_PARALLEL_LABEL_FIXTURE: &str = "stress_state_batch5_parallel_edges_labels_styles_067";
const STATE_SEMANTIC_LABEL_FIXTURES: &[&str] = &[STATE_PARALLEL_LABEL_FIXTURE];
const CLASS_MANY_RELATION_LABEL_FIXTURE: &str = "stress_class_many_relations_labels_020";
const CLASS_SEMANTIC_LABEL_FIXTURES: &[&str] = &[CLASS_MANY_RELATION_LABEL_FIXTURE];
const ER_PARALLEL_RELATION_LABEL_FIXTURE: &str = "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_multiple_relationships_between_003";
const ER_SEMANTIC_LABEL_FIXTURES: &[&str] = &[ER_PARALLEL_RELATION_LABEL_FIXTURE];
const SEMANTIC_LABEL_FIXTURE_CONTRACTS: &[SemanticLabelFixtureContract] = &[
    SemanticLabelFixtureContract {
        diagram: "c4",
        fixture: C4_DYNAMIC_LABEL_FIXTURE,
        input_sha256: "78a9531bbd743e92f73152dffaa28a9dd63c07dfa8da36f7e8c727800c53a284",
        upstream_svg_sha256: "1589e262048f1a463c42f1a98e84325b9eb311c1a50665334dd03f98f51cbc01",
        adapter: SemanticLabelAdapter::C4,
    },
    SemanticLabelFixtureContract {
        diagram: "flowchart",
        fixture: FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
        input_sha256: "05195f0247422c1af0299243082a2b0dc35a7293ddae62b0c57ddab0b0a6cec0",
        upstream_svg_sha256: "2683169760f6d16a7d06df1e4b8fe14e69fec133c662c5196967ea79e0b0cc58",
        adapter: SemanticLabelAdapter::FlowchartElk,
    },
    SemanticLabelFixtureContract {
        diagram: "architecture",
        fixture: ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
        input_sha256: "855b615e05d77a3fdebf0eb28561ba977ce1bee3cec16876c3aa85ab51f9788b",
        upstream_svg_sha256: "af2a3dcbecef491117c06b16ec3c95580606ea0d38a5966dc1fb25125882fa93",
        adapter: SemanticLabelAdapter::Architecture,
    },
    SemanticLabelFixtureContract {
        diagram: "requirement",
        fixture: REQUIREMENT_TRACES_LABEL_FIXTURE,
        input_sha256: "90985768cd5ffa56131287abbe99ec8ca4fbdd0ae5002dda8572d1fa094de57c",
        upstream_svg_sha256: "0060266072dcb2f816892411d510e44d31e5c64ab41da0d82a4185ed69433501",
        adapter: SemanticLabelAdapter::DagreDataId,
    },
    SemanticLabelFixtureContract {
        diagram: "state",
        fixture: STATE_PARALLEL_LABEL_FIXTURE,
        input_sha256: "1e7eace9ccbdbcdc6fdad8e905cefc62cbeb96dfa47ea84bed79af20c53d251d",
        upstream_svg_sha256: "beb766e95c7ddeb0f5dbf50affa51a7aa72c85f107286440ad8c7e14aec84885",
        adapter: SemanticLabelAdapter::DagreDataId,
    },
    SemanticLabelFixtureContract {
        diagram: "class",
        fixture: CLASS_MANY_RELATION_LABEL_FIXTURE,
        input_sha256: "6134c7861579118e7e5849aff872d7f0585ac1bad5ac067bf29a256d2a3cc92c",
        upstream_svg_sha256: "a234af4726d6e0c9ae3162e9161ed93098143364314219638c1e73682c99a3e9",
        adapter: SemanticLabelAdapter::DagreDataId,
    },
    SemanticLabelFixtureContract {
        diagram: "er",
        fixture: ER_PARALLEL_RELATION_LABEL_FIXTURE,
        input_sha256: "02bdf0d49b4f771161740ca46046e142f041dbc39d72fdc4209bb1f9f75e51d0",
        upstream_svg_sha256: "a4f684e992a882a0870145aac338a5e12153762e4ed687db3f8ea40c5f41edf9",
        adapter: SemanticLabelAdapter::DagreDataId,
    },
];
const LABEL_RESIDUAL_SCHEMA_VERSION: u32 = 3;
const LABEL_COMPARATOR_REVISION: &str = "semantic-label-v3";
const LABEL_GEOMETRY_DECIMALS: u32 = 3;
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
    semantic_key: LabelResidualSemanticKey,
    text: String,
    input_sha256: String,
    upstream_svg_sha256: String,
    evidence_kind: LabelResidualEvidenceKind,
    reason: String,
    upstream: LabelGeometrySignature,
    local: LabelGeometrySignature,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum LabelResidualSemanticKey {
    C4Relation {
        relation_index: u64,
        role: C4RelationLabelRole,
    },
    StableEdge {
        edge_key: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum LabelResidualEvidenceKind {
    BrowserMeasurement,
    SourceBackedLayoutApproximation,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoundedWorldTextGeometry {
    anchor_x: f64,
    anchor_y: f64,
    x_axis_x: f64,
    x_axis_y: f64,
    y_axis_x: f64,
    y_axis_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoundedAffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RoundedDescendantLabelGeometry {
    scope: String,
    #[serde(default)]
    width: Option<f64>,
    #[serde(default)]
    height: Option<f64>,
    #[serde(default)]
    transform: Option<RoundedAffineTransform>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LabelGeometrySignature {
    world: RoundedWorldTextGeometry,
    descendants: Vec<RoundedDescendantLabelGeometry>,
    edge: Option<SemanticRelationEdgeGeometry>,
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

impl RoundedAffineTransform {
    fn from_transform(transform: &str, decimals: u32) -> Result<Self, SemanticLabelError> {
        let parsed = transform.parse::<svgtypes::Transform>().map_err(|error| {
            SemanticLabelError::InvalidTransform {
                value: transform.to_string(),
                message: error.to_string(),
            }
        })?;
        let values = [parsed.a, parsed.b, parsed.c, parsed.d, parsed.e, parsed.f];
        if !values.into_iter().all(f64::is_finite) {
            return Err(SemanticLabelError::InvalidTransform {
                value: transform.to_string(),
                message: "transform contains a non-finite coordinate".to_string(),
            });
        }
        Ok(Self {
            a: round_for_comparison(parsed.a, decimals),
            b: round_for_comparison(parsed.b, decimals),
            c: round_for_comparison(parsed.c, decimals),
            d: round_for_comparison(parsed.d, decimals),
            e: round_for_comparison(parsed.e, decimals),
            f: round_for_comparison(parsed.f, decimals),
        })
    }

    fn values(self) -> [f64; 6] {
        [self.a, self.b, self.c, self.d, self.e, self.f]
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

fn rounded_label_dimension(
    value: Option<&str>,
    attribute: &'static str,
    scope: &str,
    decimals: u32,
) -> Result<Option<f64>, SemanticLabelError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let parsed = value
        .parse::<f64>()
        .map_err(|_| SemanticLabelError::InvalidCoordinate {
            text: scope.to_string(),
            attribute,
            value: value.to_string(),
        })?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(SemanticLabelError::InvalidCoordinate {
            text: scope.to_string(),
            attribute,
            value: value.to_string(),
        });
    }
    Ok(Some(round_for_comparison(parsed, decimals)))
}

fn label_geometry_signature(
    identity: &str,
    evidence: &SemanticLabelEvidence,
    decimals: u32,
) -> Result<LabelGeometrySignature, SemanticLabelError> {
    let descendants = evidence
        .descendant_geometry
        .iter()
        .map(|geometry| {
            Ok(RoundedDescendantLabelGeometry {
                scope: geometry.scope.clone(),
                width: rounded_label_dimension(
                    geometry.width.as_deref(),
                    "width",
                    &geometry.scope,
                    decimals,
                )?,
                height: rounded_label_dimension(
                    geometry.height.as_deref(),
                    "height",
                    &geometry.scope,
                    decimals,
                )?,
                transform: geometry
                    .transform
                    .as_deref()
                    .map(|transform| RoundedAffineTransform::from_transform(transform, decimals))
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, SemanticLabelError>>()?;
    let edge = evidence
        .associated_edge
        .as_ref()
        .map(|edge| normalized_edge_geometry(identity, edge, decimals))
        .transpose()?;
    Ok(LabelGeometrySignature {
        world: RoundedWorldTextGeometry::from_geometry(evidence.geometry, decimals),
        descendants,
        edge,
    })
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

fn pair_flowchart_elk_edge_labels(
    upstream_svg: &str,
    local_svg: &str,
) -> Result<Vec<SemanticLabelPair<String>>, SemanticLabelError> {
    pair_stable_edge_label_maps(
        "flowchart",
        extract_flowchart_elk_edge_labels(upstream_svg)?,
        extract_flowchart_elk_edge_labels(local_svg)?,
    )
}

#[derive(Debug, Clone, Copy)]
struct DagreDataIdAdapterConfig {
    diagram: &'static str,
    edge_class: &'static str,
    allow_empty_labels: bool,
}

fn dagre_data_id_adapter_config(diagram: &str) -> Option<DagreDataIdAdapterConfig> {
    match diagram {
        "requirement" => Some(DagreDataIdAdapterConfig {
            diagram: "requirement",
            edge_class: "relationshipLine",
            allow_empty_labels: false,
        }),
        "state" => Some(DagreDataIdAdapterConfig {
            diagram: "state",
            edge_class: "transition",
            allow_empty_labels: true,
        }),
        "class" => Some(DagreDataIdAdapterConfig {
            diagram: "class",
            edge_class: "relation",
            allow_empty_labels: false,
        }),
        "er" => Some(DagreDataIdAdapterConfig {
            diagram: "er",
            edge_class: "relationshipLine",
            allow_empty_labels: false,
        }),
        _ => None,
    }
}

fn pair_dagre_data_id_edge_labels(
    config: DagreDataIdAdapterConfig,
    upstream_svg: &str,
    local_svg: &str,
) -> Result<Vec<SemanticLabelPair<String>>, SemanticLabelError> {
    pair_stable_edge_label_maps(
        config.diagram,
        extract_dagre_data_id_edge_labels(upstream_svg, config)?,
        extract_dagre_data_id_edge_labels(local_svg, config)?,
    )
}

fn pair_architecture_edge_labels(
    upstream_svg: &str,
    local_svg: &str,
) -> Result<Vec<SemanticLabelPair<String>>, SemanticLabelError> {
    pair_stable_edge_label_maps(
        "architecture",
        extract_architecture_edge_labels(upstream_svg)?,
        extract_architecture_edge_labels(local_svg)?,
    )
}

fn pair_stable_edge_label_maps(
    diagram: &'static str,
    upstream: BTreeMap<String, SemanticLabelEvidence>,
    local: BTreeMap<String, SemanticLabelEvidence>,
) -> Result<Vec<SemanticLabelPair<String>>, SemanticLabelError> {
    pair_complete_semantic_label_maps(upstream, local).map_err(|mismatch| {
        SemanticLabelError::StableIdentitySetMismatch {
            diagram,
            missing_from_local: mismatch.missing_from_local,
            missing_from_upstream: mismatch.missing_from_upstream,
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StylesheetScope {
    Full,
    ClassicDagre,
}

#[derive(Debug, Clone, PartialEq)]
struct ResidualAwareSemanticLabelPair {
    key: LabelResidualSemanticKey,
    upstream: SemanticLabelEvidence,
    local: SemanticLabelEvidence,
}

fn stable_residual_pairs(
    pairs: Vec<SemanticLabelPair<String>>,
) -> Vec<ResidualAwareSemanticLabelPair> {
    pairs
        .into_iter()
        .map(|pair| ResidualAwareSemanticLabelPair {
            key: LabelResidualSemanticKey::StableEdge { edge_key: pair.key },
            upstream: pair.upstream,
            local: pair.local,
        })
        .collect()
}

fn c4_residual_pairs(
    pairs: Vec<SemanticLabelPair<C4RelationLabelKey>>,
) -> Vec<ResidualAwareSemanticLabelPair> {
    pairs
        .into_iter()
        .map(|pair| ResidualAwareSemanticLabelPair {
            key: LabelResidualSemanticKey::C4Relation {
                relation_index: pair.key.relation_index,
                role: pair.key.role,
            },
            upstream: pair.upstream,
            local: pair.local,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn compare_semantic_edge_labels(
    diagram: &str,
    stem: &str,
    input_text: &str,
    pairs: Vec<ResidualAwareSemanticLabelPair>,
    upstream_svg: &str,
    local_svg: &str,
    decimals: u32,
    stylesheet_scope: StylesheetScope,
) -> Result<SemanticLabelGateOutcome, String> {
    if decimals > MAX_LABEL_GEOMETRY_DECIMALS {
        return Err(format!(
            "semantic label comparison decimals {decimals} exceed the supported maximum {MAX_LABEL_GEOMETRY_DECIMALS}"
        ));
    }
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
    let input_sha256 = crate::util::sha256_hex(input_text.as_bytes());
    let upstream_svg_sha256 = crate::util::sha256_hex(upstream_svg.as_bytes());
    validate_fixture_residual_digests(
        diagram,
        stem,
        &fixture_entries,
        &input_sha256,
        &upstream_svg_sha256,
    )?;

    let upstream_stylesheet = extract_stylesheet_signature(upstream_svg, stylesheet_scope)
        .map_err(|error| {
            format!("semantic stylesheet extraction failed for {diagram}/{stem}: {error}")
        })?;
    let local_stylesheet =
        extract_stylesheet_signature(local_svg, stylesheet_scope).map_err(|error| {
            format!("semantic stylesheet extraction failed for {diagram}/{stem}: {error}")
        })?;
    let mut failures = Vec::new();
    if upstream_stylesheet != local_stylesheet {
        failures.push(format!(
            "{diagram}/{stem}: semantic stylesheet source differs"
        ));
    }
    let mut accepted_entries = BTreeSet::new();

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
                    "{diagram}/{stem} {:?}: associated edge presence differs",
                    pair.key
                ));
                continue;
            }
        };
        if let (Some(upstream_edge), Some(local_edge)) = (upstream_edge, local_edge)
            && upstream_edge.presentation != local_edge.presentation
        {
            failures.push(format!(
                "{diagram}/{stem} {:?}: associated edge presentation differs: upstream={:?}, local={:?}",
                pair.key, upstream_edge.presentation, local_edge.presentation
            ));
            continue;
        }

        let identity = format!("{:?}", pair.key);
        let upstream =
            label_geometry_signature(&identity, &pair.upstream, decimals).map_err(|error| {
                format!("semantic geometry extraction failed for {diagram}/{stem}: {error}")
            })?;
        let local =
            label_geometry_signature(&identity, &pair.local, decimals).map_err(|error| {
                format!("semantic geometry extraction failed for {diagram}/{stem}: {error}")
            })?;
        if upstream == local {
            continue;
        }

        let matching_entries = fixture_entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.semantic_key == pair.key
                    && entry.text == pair.upstream.text
                    && entry.upstream == upstream
                    && entry.local == local
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        match matching_entries.as_slice() {
            [index] => {
                accepted_entries.insert(*index);
            }
            [] => {
                let candidate = serde_json::json!({
                    "diagram": diagram,
                    "fixture": stem,
                    "semantic_key": &pair.key,
                    "text": pair.upstream.text.as_str(),
                    "input_sha256": input_sha256.as_str(),
                    "upstream_svg_sha256": upstream_svg_sha256.as_str(),
                    "classification": "review_required",
                    "reason": "REVIEW REQUIRED",
                    "upstream": upstream,
                    "local": local,
                });
                let candidate_json = serde_json::to_string(&candidate)
                    .expect("serializing residual candidate cannot fail");
                if std::env::var_os("MERMAN_EMIT_LABEL_RESIDUAL_CANDIDATES").is_some() {
                    eprintln!("LABEL_RESIDUAL_CANDIDATE {candidate_json}");
                }
                failures.push(format!(
                    "{diagram}/{stem} {:?}: label or associated edge geometry differs without an exact residual contract",
                    pair.key,
                ));
            }
            _ => {
                failures.push(format!(
                    "{diagram}/{stem} {:?}: multiple residual contracts matched one semantic label",
                    pair.key,
                ));
            }
        }
    }

    let stale_entries = fixture_entries
        .iter()
        .enumerate()
        .filter(|(index, _)| !accepted_entries.contains(index))
        .map(|(_, entry)| entry.semantic_key.clone())
        .collect::<Vec<_>>();
    if !stale_entries.is_empty() {
        failures.push(format!(
            "{diagram}/{stem}: stale semantic label residual contracts were not exercised: {stale_entries:?}"
        ));
    }

    Ok(SemanticLabelGateOutcome {
        evidence: SemanticLabelGateEvidence {
            compared_samples: pairs.len(),
            accepted_residuals: accepted_entries.len(),
        },
        issues: failures,
    })
}

pub(crate) fn compare_registered_semantic_labels(
    diagram: &str,
    stem: &str,
    input_text: &str,
    upstream_svg: &str,
    local_svg: &str,
    _dom_decimals: u32,
) -> Result<Option<SemanticLabelGateOutcome>, String> {
    let Some(contract) = semantic_label_fixture_contract(diagram, stem) else {
        return Ok(None);
    };
    validate_registered_fixture_digests(contract, input_text, upstream_svg)?;

    let (pairs, stylesheet_scope) = match contract.adapter {
        SemanticLabelAdapter::C4 => {
            let pairs = pair_c4_relation_labels(upstream_svg, local_svg).map_err(|error| {
                format!("semantic label extraction failed for {diagram}/{stem}: {error}")
            })?;
            (c4_residual_pairs(pairs), StylesheetScope::Full)
        }
        SemanticLabelAdapter::FlowchartElk => {
            let pairs =
                pair_flowchart_elk_edge_labels(upstream_svg, local_svg).map_err(|error| {
                    format!("semantic label extraction failed for {diagram}/{stem}: {error}")
                })?;
            (stable_residual_pairs(pairs), StylesheetScope::ClassicDagre)
        }
        SemanticLabelAdapter::DagreDataId => {
            let config = dagre_data_id_adapter_config(diagram).ok_or_else(|| {
                format!("registered semantic label fixture has no Dagre data-id adapter: {diagram}/{stem}")
            })?;
            let pairs = pair_dagre_data_id_edge_labels(config, upstream_svg, local_svg).map_err(
                |error| format!("semantic label extraction failed for {diagram}/{stem}: {error}"),
            )?;
            (stable_residual_pairs(pairs), StylesheetScope::ClassicDagre)
        }
        SemanticLabelAdapter::Architecture => {
            let pairs =
                pair_architecture_edge_labels(upstream_svg, local_svg).map_err(|error| {
                    format!("semantic label extraction failed for {diagram}/{stem}: {error}")
                })?;
            (stable_residual_pairs(pairs), StylesheetScope::Full)
        }
    };
    let outcome = compare_semantic_edge_labels(
        diagram,
        stem,
        input_text,
        pairs,
        upstream_svg,
        local_svg,
        LABEL_GEOMETRY_DECIMALS,
        stylesheet_scope,
    )?;
    Ok(Some(outcome))
}

pub(crate) fn registered_semantic_label_fixtures(diagram: &str) -> &'static [&'static str] {
    match diagram {
        "c4" => C4_SEMANTIC_LABEL_FIXTURES,
        "flowchart" => FLOWCHART_SEMANTIC_LABEL_FIXTURES,
        "architecture" => ARCHITECTURE_SEMANTIC_LABEL_FIXTURES,
        "requirement" => REQUIREMENT_SEMANTIC_LABEL_FIXTURES,
        "state" => STATE_SEMANTIC_LABEL_FIXTURES,
        "class" => CLASS_SEMANTIC_LABEL_FIXTURES,
        "er" => ER_SEMANTIC_LABEL_FIXTURES,
        _ => &[],
    }
}

fn is_registered_semantic_label_fixture(diagram: &str, fixture: &str) -> bool {
    registered_semantic_label_fixtures(diagram).contains(&fixture)
}

fn semantic_label_fixture_contract(
    diagram: &str,
    fixture: &str,
) -> Option<&'static SemanticLabelFixtureContract> {
    SEMANTIC_LABEL_FIXTURE_CONTRACTS
        .iter()
        .find(|contract| contract.diagram == diagram && contract.fixture == fixture)
}

fn validate_registered_fixture_digests(
    contract: &SemanticLabelFixtureContract,
    input_text: &str,
    upstream_svg: &str,
) -> Result<(), String> {
    let actual_input = crate::util::sha256_hex(input_text.as_bytes());
    let actual_upstream = crate::util::sha256_hex(upstream_svg.as_bytes());
    if actual_input != contract.input_sha256 || actual_upstream != contract.upstream_svg_sha256 {
        return Err(format!(
            "semantic label fixture {}/{} is not bound to the compared artifacts: input expected={} actual={actual_input}, upstream SVG expected={} actual={actual_upstream}",
            contract.diagram, contract.fixture, contract.input_sha256, contract.upstream_svg_sha256,
        ));
    }
    Ok(())
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
        let Some(contract) = semantic_label_fixture_contract(&entry.diagram, &entry.fixture) else {
            return Err(format!(
                "semantic label residual {}/{} has no signed fixture contract",
                entry.diagram, entry.fixture
            ));
        };
        if entry.input_sha256 != contract.input_sha256
            || entry.upstream_svg_sha256 != contract.upstream_svg_sha256
        {
            return Err(format!(
                "semantic label residual {}/{} digests do not match its signed fixture contract",
                entry.diagram, entry.fixture
            ));
        }
        match (contract.adapter, &entry.semantic_key) {
            (
                SemanticLabelAdapter::C4,
                LabelResidualSemanticKey::C4Relation { relation_index, .. },
            ) if *relation_index > 0 && !entry.text.trim().is_empty() => {}
            (
                SemanticLabelAdapter::FlowchartElk
                | SemanticLabelAdapter::DagreDataId
                | SemanticLabelAdapter::Architecture,
                LabelResidualSemanticKey::StableEdge { edge_key },
            ) if !edge_key.trim().is_empty() => {}
            _ => {
                return Err(format!(
                    "semantic label residual {}/{} key {:?} is invalid for adapter {:?}",
                    entry.diagram, entry.fixture, entry.semantic_key, contract.adapter
                ));
            }
        }
        if !keys.insert((
            entry.diagram.clone(),
            entry.fixture.clone(),
            entry.semantic_key.clone(),
        )) {
            return Err(format!(
                "duplicate semantic label residual key for {}/{} {:?}",
                entry.diagram, entry.fixture, entry.semantic_key
            ));
        }
        for (role, digest) in [
            ("input", entry.input_sha256.as_str()),
            ("upstream SVG", entry.upstream_svg_sha256.as_str()),
        ] {
            if !crate::util::is_canonical_sha256(digest) {
                return Err(format!(
                    "semantic label residual {}/{} {:?} {role} SHA-256 must be 64 lowercase hexadecimal characters",
                    entry.diagram, entry.fixture, entry.semantic_key
                ));
            }
        }
        if entry.reason.trim().is_empty() {
            return Err(format!(
                "semantic label residual {}/{} {:?} has an empty reason",
                entry.diagram, entry.fixture, entry.semantic_key
            ));
        }
        match entry.evidence_kind {
            LabelResidualEvidenceKind::BrowserMeasurement
            | LabelResidualEvidenceKind::SourceBackedLayoutApproximation => {}
        }
        for (side, geometry) in [("upstream", &entry.upstream), ("local", &entry.local)] {
            validate_catalog_geometry_signature(entry, side, geometry, catalog.decimals)?;
        }
        if entry.upstream == entry.local {
            return Err(format!(
                "semantic label residual {}/{} {:?} has identical upstream and local geometry",
                entry.diagram, entry.fixture, entry.semantic_key
            ));
        }
    }
    Ok(())
}

fn validate_catalog_geometry_signature(
    entry: &LabelResidualEntry,
    side: &str,
    geometry: &LabelGeometrySignature,
    decimals: u32,
) -> Result<(), String> {
    let context = format!(
        "semantic label residual {}/{} {:?} {side}",
        entry.diagram, entry.fixture, entry.semantic_key
    );
    if !geometry.world.is_finite() || !geometry.world.is_quantized(decimals) {
        return Err(format!(
            "{context} world geometry is non-finite or exceeds declared precision {decimals}"
        ));
    }

    let mut descendant_scopes = BTreeSet::new();
    for descendant in &geometry.descendants {
        if descendant.scope.trim().is_empty()
            || !descendant_scopes.insert(descendant.scope.as_str())
            || (descendant.width.is_none()
                && descendant.height.is_none()
                && descendant.transform.is_none())
        {
            return Err(format!("{context} has invalid descendant geometry"));
        }
        for (attribute, value) in [("width", descendant.width), ("height", descendant.height)] {
            if value.is_some_and(|value| {
                !value.is_finite() || value < 0.0 || round_for_comparison(value, decimals) != value
            }) {
                return Err(format!(
                    "{context} descendant {} has invalid {attribute}",
                    descendant.scope
                ));
            }
        }
        if descendant
            .transform
            .is_some_and(|transform| !transform.is_finite() || !transform.is_quantized(decimals))
        {
            return Err(format!(
                "{context} descendant {} has invalid transform",
                descendant.scope
            ));
        }
    }

    let Some(edge) = geometry.edge.as_ref() else {
        return Err(format!("{context} has no associated edge geometry"));
    };
    if edge.tag.trim().is_empty() || edge.attributes.is_empty() {
        return Err(format!("{context} has an invalid associated edge geometry"));
    }
    for (attribute, value) in &edge.attributes {
        if value.trim().is_empty()
            || !matches!(
                attribute.as_str(),
                "d" | "data-points" | "x1" | "y1" | "x2" | "y2" | "transform"
            )
        {
            return Err(format!(
                "{context} has invalid associated edge attribute `{attribute}`"
            ));
        }
        if matches!(attribute.as_str(), "x1" | "y1" | "x2" | "y2") {
            let coordinate = value
                .parse::<f64>()
                .map_err(|_| format!("{context} has invalid `{attribute}` coordinate"))?;
            if !coordinate.is_finite() || round_for_comparison(coordinate, decimals) != coordinate {
                return Err(format!(
                    "{context} `{attribute}` coordinate exceeds declared precision {decimals}"
                ));
            }
        }
    }
    Ok(())
}

fn extract_upstream_residual_evidence(
    contract: &SemanticLabelFixtureContract,
    upstream_svg: &str,
) -> Result<BTreeMap<LabelResidualSemanticKey, SemanticLabelEvidence>, String> {
    let extraction_error = |error: SemanticLabelError| {
        format!(
            "extract semantic label residual evidence for {}/{}: {error}",
            contract.diagram, contract.fixture
        )
    };
    match contract.adapter {
        SemanticLabelAdapter::C4 => extract_c4_relation_labels(upstream_svg)
            .map(|labels| {
                labels
                    .into_iter()
                    .map(|(key, evidence)| {
                        (
                            LabelResidualSemanticKey::C4Relation {
                                relation_index: key.relation_index,
                                role: key.role,
                            },
                            evidence,
                        )
                    })
                    .collect()
            })
            .map_err(extraction_error),
        SemanticLabelAdapter::FlowchartElk => extract_flowchart_elk_edge_labels(upstream_svg)
            .map(stable_residual_evidence)
            .map_err(extraction_error),
        SemanticLabelAdapter::DagreDataId => {
            let config = dagre_data_id_adapter_config(contract.diagram).ok_or_else(|| {
                format!(
                    "registered semantic label fixture has no Dagre data-id adapter: {}/{}",
                    contract.diagram, contract.fixture
                )
            })?;
            extract_dagre_data_id_edge_labels(upstream_svg, config)
                .map(stable_residual_evidence)
                .map_err(extraction_error)
        }
        SemanticLabelAdapter::Architecture => extract_architecture_edge_labels(upstream_svg)
            .map(stable_residual_evidence)
            .map_err(extraction_error),
    }
}

fn stable_residual_evidence(
    labels: BTreeMap<String, SemanticLabelEvidence>,
) -> BTreeMap<LabelResidualSemanticKey, SemanticLabelEvidence> {
    labels
        .into_iter()
        .map(|(edge_key, evidence)| (LabelResidualSemanticKey::StableEdge { edge_key }, evidence))
        .collect()
}

fn validate_label_residual_artifacts(catalog: &LabelResidualCatalog) -> Result<(), String> {
    let mut fixtures = BTreeMap::<(&str, &str), Vec<&LabelResidualEntry>>::new();
    for entry in &catalog.entries {
        fixtures
            .entry((&entry.diagram, &entry.fixture))
            .or_default()
            .push(entry);
    }

    for ((diagram, fixture), entries) in fixtures {
        let first = entries[0];
        let input_path = crate::cmd::fixtures_root()
            .join(diagram)
            .join(format!("{fixture}.mmd"));
        let upstream_path = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join(diagram)
            .join(format!("{fixture}.svg"));
        let input = std::fs::read(&input_path).map_err(|error| {
            format!(
                "read semantic label residual input {}: {error}",
                input_path.display()
            )
        })?;
        let upstream = std::fs::read_to_string(&upstream_path).map_err(|error| {
            format!(
                "read semantic label residual upstream SVG {}: {error}",
                upstream_path.display()
            )
        })?;
        for (role, actual, expected) in [
            (
                "input",
                crate::util::sha256_hex(&input),
                first.input_sha256.as_str(),
            ),
            (
                "upstream SVG",
                crate::util::sha256_hex(upstream.as_bytes()),
                first.upstream_svg_sha256.as_str(),
            ),
        ] {
            if actual != expected {
                return Err(format!(
                    "semantic label residual {diagram}/{fixture} {role} SHA-256 drifted: expected {expected}, found {actual}"
                ));
            }
        }

        let contract = semantic_label_fixture_contract(diagram, fixture).ok_or_else(|| {
            format!("semantic label residual {diagram}/{fixture} has no signed fixture contract")
        })?;
        let evidence = extract_upstream_residual_evidence(contract, &upstream)?;
        for entry in entries {
            let sample = evidence.get(&entry.semantic_key).ok_or_else(|| {
                format!(
                    "semantic label residual {diagram}/{fixture} key {:?} is absent from the signed upstream SVG",
                    entry.semantic_key
                )
            })?;
            if sample.text != entry.text {
                return Err(format!(
                    "semantic label residual {diagram}/{fixture} {:?} text does not match the signed upstream SVG",
                    entry.semantic_key
                ));
            }
            let identity = format!("{:?}", entry.semantic_key);
            let signature =
                label_geometry_signature(&identity, sample, catalog.decimals).map_err(|error| {
                    format!(
                        "extract semantic label residual geometry for {diagram}/{fixture}: {error}"
                    )
                })?;
            if signature != entry.upstream {
                return Err(format!(
                    "semantic label residual {diagram}/{fixture} {:?} upstream signature does not match the signed SVG",
                    entry.semantic_key
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

fn extract_dagre_data_id_edge_labels(
    svg: &str,
    config: DagreDataIdAdapterConfig,
) -> Result<BTreeMap<String, SemanticLabelEvidence>, SemanticLabelError> {
    let normalized = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(normalized.as_ref())
        .map_err(|error| SemanticLabelError::InvalidSvg(error.to_string()))?;
    let mut edges = BTreeMap::new();

    for path in document.descendants().filter(|node| {
        node.is_element()
            && node.has_tag_name("path")
            && has_class_token(*node, config.edge_class)
            && self_or_ancestor_has_class(*node, "edgePaths")
    }) {
        let identity = required_edge_identity(path, config.diagram, "data-id")?;
        let edge = semantic_world_relation_edge_evidence(path)?;
        if edges.insert(identity.clone(), edge).is_some() {
            return Err(SemanticLabelError::DuplicateEdgeIdentity {
                diagram: config.diagram,
                identity,
            });
        }
    }

    let mut evidence = BTreeMap::new();
    for label_root in document.descendants().filter(|node| {
        node.is_element()
            && node.has_tag_name("g")
            && has_class_token(*node, "edgeLabel")
            && node
                .parent_element()
                .is_some_and(|parent| has_class_token(parent, "edgeLabels"))
    }) {
        let identity_nodes = label_root
            .descendants()
            .filter(|node| {
                node.is_element()
                    && node.has_tag_name("g")
                    && has_class_token(*node, "label")
                    && node.attribute("data-id").is_some()
            })
            .collect::<Vec<_>>();
        if identity_nodes.len() != 1 {
            return Err(SemanticLabelError::AmbiguousEdgeGroup {
                diagram: config.diagram,
                edge_count: 1,
                label_count: identity_nodes.len(),
            });
        }
        let identity = required_edge_identity(identity_nodes[0], config.diagram, "data-id")?;
        if evidence.contains_key(&identity) {
            return Err(SemanticLabelError::DuplicateEdgeIdentity {
                diagram: config.diagram,
                identity,
            });
        }
        let edge = edges
            .remove(&identity)
            .ok_or_else(|| SemanticLabelError::OrphanEdgeLabel {
                diagram: config.diagram,
                identity: identity.clone(),
            })?;
        let text = semantic_edge_label_text(label_root);
        if text.is_empty() && !config.allow_empty_labels {
            return Err(SemanticLabelError::EmptyEdgeLabel {
                diagram: config.diagram,
                identity,
            });
        }
        let geometry_context = if text.is_empty() {
            identity.as_str()
        } else {
            text.as_str()
        };
        let geometry = world_element_geometry(label_root, 0.0, 0.0, geometry_context)?;
        let presentation = semantic_flowchart_label_presentation(label_root, geometry_context)?;
        evidence.insert(
            identity,
            SemanticLabelEvidence {
                geometry,
                descendant_geometry: collect_descendant_label_geometry(label_root)?,
                presentation,
                associated_edge: Some(edge),
                text,
            },
        );
    }

    if let Some(identity) = edges.into_keys().next() {
        return Err(SemanticLabelError::MissingEdgeLabel {
            diagram: config.diagram,
            identity,
        });
    }
    Ok(evidence)
}

fn extract_flowchart_elk_edge_labels(
    svg: &str,
) -> Result<BTreeMap<String, SemanticLabelEvidence>, SemanticLabelError> {
    const DIAGRAM: &str = "flowchart";
    let normalized = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(normalized.as_ref())
        .map_err(|error| SemanticLabelError::InvalidSvg(error.to_string()))?;
    let mut edges = BTreeMap::new();

    for path in document.descendants().filter(|node| {
        node.is_element()
            && node.has_tag_name("path")
            && has_class_token(*node, "flowchart-link")
            && self_or_ancestor_has_class(*node, "edgePaths")
    }) {
        let identity = required_edge_identity(path, DIAGRAM, "data-id")?;
        let edge = semantic_world_relation_edge_evidence(path)?;
        if edges.insert(identity.clone(), edge).is_some() {
            return Err(SemanticLabelError::DuplicateEdgeIdentity {
                diagram: DIAGRAM,
                identity,
            });
        }
    }

    let mut evidence = BTreeMap::new();
    for label in document.descendants().filter(|node| {
        node.is_element()
            && node.has_tag_name("g")
            && has_class_token(*node, "label")
            && self_or_ancestor_has_class(*node, "edgeLabels")
    }) {
        let identity = required_edge_identity(label, DIAGRAM, "data-id")?;
        if evidence.contains_key(&identity) {
            return Err(SemanticLabelError::DuplicateEdgeIdentity {
                diagram: DIAGRAM,
                identity,
            });
        }
        let edge = edges
            .remove(&identity)
            .ok_or_else(|| SemanticLabelError::OrphanEdgeLabel {
                diagram: DIAGRAM,
                identity: identity.clone(),
            })?;
        let text = semantic_edge_label_text(label);
        if text.is_empty() {
            return Err(SemanticLabelError::EmptyEdgeLabel {
                diagram: DIAGRAM,
                identity,
            });
        }
        evidence.insert(
            identity,
            SemanticLabelEvidence {
                geometry: world_element_geometry(label, 0.0, 0.0, &text)?,
                descendant_geometry: collect_descendant_label_geometry(label)?,
                presentation: semantic_flowchart_label_presentation(label, &text)?,
                associated_edge: Some(edge),
                text,
            },
        );
    }

    if let Some(identity) = edges.into_keys().next() {
        return Err(SemanticLabelError::MissingEdgeLabel {
            diagram: DIAGRAM,
            identity,
        });
    }
    Ok(evidence)
}

fn extract_architecture_edge_labels(
    svg: &str,
) -> Result<BTreeMap<String, SemanticLabelEvidence>, SemanticLabelError> {
    const DIAGRAM: &str = "architecture";
    let normalized = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(normalized.as_ref())
        .map_err(|error| SemanticLabelError::InvalidSvg(error.to_string()))?;
    let mut evidence = BTreeMap::new();

    for edge_root in document
        .descendants()
        .filter(|node| node.is_element() && has_class_token(*node, "architecture-edges"))
    {
        for edge_group in edge_root.children().filter(roxmltree::Node::is_element) {
            if edge_group.has_tag_name("path") && has_class_token(edge_group, "edge") {
                return Err(SemanticLabelError::MissingEdgeLabel {
                    diagram: DIAGRAM,
                    identity: required_edge_identity(edge_group, DIAGRAM, "id")?,
                });
            }
            let edge_paths = edge_group
                .children()
                .filter(|node| {
                    node.is_element() && node.has_tag_name("path") && has_class_token(*node, "edge")
                })
                .collect::<Vec<_>>();
            let descendant_edge_count = edge_group
                .descendants()
                .filter(|node| {
                    node.is_element() && node.has_tag_name("path") && has_class_token(*node, "edge")
                })
                .count();
            let label_groups = edge_group
                .children()
                .filter(|node| {
                    node.is_element()
                        && node.has_tag_name("g")
                        && node.descendants().any(|child| child.has_tag_name("text"))
                })
                .collect::<Vec<_>>();
            if descendant_edge_count != edge_paths.len() {
                return Err(SemanticLabelError::AmbiguousEdgeGroup {
                    diagram: DIAGRAM,
                    edge_count: descendant_edge_count,
                    label_count: label_groups.len(),
                });
            }
            if edge_paths.is_empty() && label_groups.is_empty() {
                continue;
            }
            if label_groups.is_empty() && edge_paths.len() == 1 {
                return Err(SemanticLabelError::MissingEdgeLabel {
                    diagram: DIAGRAM,
                    identity: required_edge_identity(edge_paths[0], DIAGRAM, "id")?,
                });
            }
            if edge_paths.len() != 1 || label_groups.len() != 1 {
                return Err(SemanticLabelError::AmbiguousEdgeGroup {
                    diagram: DIAGRAM,
                    edge_count: edge_paths.len(),
                    label_count: label_groups.len(),
                });
            }

            let path = edge_paths[0];
            let label = label_groups[0];
            let text_nodes = label
                .descendants()
                .filter(|node| node.is_element() && node.has_tag_name("text"))
                .collect::<Vec<_>>();
            if text_nodes.len() != 1 {
                return Err(SemanticLabelError::AmbiguousEdgeGroup {
                    diagram: DIAGRAM,
                    edge_count: 1,
                    label_count: text_nodes.len(),
                });
            }

            let identity = required_edge_identity(path, DIAGRAM, "id")?;
            let text = svg_text_content(text_nodes[0]);
            if text.is_empty() {
                return Err(SemanticLabelError::EmptyEdgeLabel {
                    diagram: DIAGRAM,
                    identity,
                });
            }
            let sample = SemanticLabelEvidence {
                geometry: world_element_geometry(label, 0.0, 0.0, &text)?,
                descendant_geometry: collect_descendant_label_geometry(label)?,
                presentation: semantic_label_presentation(label, &text)?,
                associated_edge: Some(semantic_world_relation_edge_evidence(path)?),
                text,
            };
            if evidence.insert(identity.clone(), sample).is_some() {
                return Err(SemanticLabelError::DuplicateEdgeIdentity {
                    diagram: DIAGRAM,
                    identity,
                });
            }
        }
    }
    Ok(evidence)
}

fn required_edge_identity(
    node: roxmltree::Node<'_, '_>,
    diagram: &'static str,
    attribute: &str,
) -> Result<String, SemanticLabelError> {
    let identity = node.attribute(attribute).unwrap_or_default();
    if identity.trim().is_empty() {
        Err(SemanticLabelError::EmptyEdgeIdentity { diagram })
    } else {
        Ok(identity.to_string())
    }
}

fn semantic_edge_label_text(label: roxmltree::Node<'_, '_>) -> String {
    if let Some(foreign_object) = label
        .descendants()
        .find(|node| node.is_element() && node.has_tag_name("foreignObject"))
    {
        return foreignobject_text(foreign_object);
    }
    label
        .descendants()
        .find(|node| node.is_element() && node.has_tag_name("text"))
        .map(svg_text_content)
        .unwrap_or_default()
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
        descendant_geometry: collect_descendant_label_geometry(node)?,
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
    semantic_element_presentation(
        node,
        text,
        &["x", "y", "transform"],
        &["width", "height", "transform"],
    )
}

fn collect_descendant_label_geometry(
    node: roxmltree::Node<'_, '_>,
) -> Result<Vec<SemanticDescendantLabelGeometry>, SemanticLabelError> {
    let descendants = node
        .descendants()
        .filter(roxmltree::Node::is_element)
        .filter(|descendant| *descendant != node);
    let mut geometry = Vec::new();
    for (element_index, element) in descendants.enumerate() {
        let width = element
            .attribute("width")
            .map(str::trim)
            .map(str::to_string);
        let height = element
            .attribute("height")
            .map(str::trim)
            .map(str::to_string);
        let transform = element
            .attribute("transform")
            .map(str::trim)
            .map(str::to_string);
        if width.is_none() && height.is_none() && transform.is_none() {
            continue;
        }
        geometry.push(SemanticDescendantLabelGeometry {
            scope: format!("{}[{}]", element.tag_name().name(), element_index + 1),
            width,
            height,
            transform,
        });
    }
    Ok(geometry)
}

fn semantic_flowchart_label_presentation(
    node: roxmltree::Node<'_, '_>,
    text: &str,
) -> Result<SemanticLabelPresentation, SemanticLabelError> {
    let mut presentation = semantic_label_presentation(node, text)?;
    presentation.inline_style.retain(|(property, value)| {
        let Some((scope, property)) = property.rsplit_once('@') else {
            return true;
        };
        !(scope.starts_with("foreignObject[")
            && property.eq_ignore_ascii_case("overflow")
            && value.eq_ignore_ascii_case("visible"))
    });
    Ok(presentation)
}

fn semantic_relation_edge_evidence(
    node: roxmltree::Node<'_, '_>,
) -> Result<SemanticRelationEdgeEvidence, SemanticLabelError> {
    const GEOMETRY_ATTRIBUTES: &[&str] = &["d", "data-points", "x1", "y1", "x2", "y2", "transform"];
    let attributes = GEOMETRY_ATTRIBUTES
        .iter()
        .filter_map(|attribute| {
            node.attribute(*attribute)
                .map(|value| ((*attribute).to_string(), value.trim().to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    Ok(SemanticRelationEdgeEvidence {
        presentation: semantic_element_presentation(
            node,
            "semantic relation edge",
            GEOMETRY_ATTRIBUTES,
            &[],
        )?,
        geometry: SemanticRelationEdgeGeometry {
            tag: node.tag_name().name().to_string(),
            attributes,
        },
    })
}

fn semantic_world_relation_edge_evidence(
    node: roxmltree::Node<'_, '_>,
) -> Result<SemanticRelationEdgeEvidence, SemanticLabelError> {
    let mut evidence = semantic_relation_edge_evidence(node)?;
    let world = composed_element_transform(node, "semantic relation edge")?;
    evidence.geometry.attributes.insert(
        "transform".to_string(),
        format!(
            "matrix({} {} {} {} {} {})",
            world.a, world.b, world.c, world.d, world.e, world.f
        ),
    );
    Ok(evidence)
}

fn normalized_edge_geometry(
    identity: &str,
    edge: &SemanticRelationEdgeEvidence,
    decimals: u32,
) -> Result<SemanticRelationEdgeGeometry, SemanticLabelError> {
    let mut attributes = BTreeMap::new();
    for (attribute, value) in &edge.geometry.attributes {
        let normalized = match attribute.as_str() {
            "d" => normalized_path_signature(value, decimals).map_err(|message| {
                SemanticLabelError::InvalidEdgeGeometry {
                    identity: identity.to_string(),
                    attribute: attribute.clone(),
                    message,
                }
            })?,
            "data-points" => {
                normalized_data_points_signature(value, decimals).map_err(|message| {
                    SemanticLabelError::InvalidEdgeGeometry {
                        identity: identity.to_string(),
                        attribute: attribute.clone(),
                        message,
                    }
                })?
            }
            "transform" => normalized_transform_signature(value, decimals).map_err(|message| {
                SemanticLabelError::InvalidEdgeGeometry {
                    identity: identity.to_string(),
                    attribute: attribute.clone(),
                    message,
                }
            })?,
            _ => {
                let coordinate = value.parse::<f64>().map_err(|error| {
                    SemanticLabelError::InvalidEdgeGeometry {
                        identity: identity.to_string(),
                        attribute: attribute.clone(),
                        message: error.to_string(),
                    }
                })?;
                if !coordinate.is_finite() {
                    return Err(SemanticLabelError::InvalidEdgeGeometry {
                        identity: identity.to_string(),
                        attribute: attribute.clone(),
                        message: "coordinate is not finite".to_string(),
                    });
                }
                format!("{:?}", round_for_comparison(coordinate, decimals))
            }
        };
        attributes.insert(attribute.clone(), normalized);
    }
    Ok(SemanticRelationEdgeGeometry {
        tag: edge.geometry.tag.clone(),
        attributes,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedEdgePoint {
    x: f64,
    y: f64,
}

fn normalized_data_points_signature(value: &str, decimals: u32) -> Result<String, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| format!("invalid Base64: {error}"))?;
    let points = serde_json::from_slice::<Vec<EncodedEdgePoint>>(&bytes)
        .map_err(|error| format!("invalid point JSON: {error}"))?;
    let mut signature = String::from("[");
    for (index, point) in points.into_iter().enumerate() {
        if !point.x.is_finite() || !point.y.is_finite() {
            return Err(format!("point {index} contains a non-finite coordinate"));
        }
        if index > 0 {
            signature.push(',');
        }
        write!(
            signature,
            "({:?},{:?})",
            round_for_comparison(point.x, decimals),
            round_for_comparison(point.y, decimals)
        )
        .expect("writing to a String cannot fail");
    }
    signature.push(']');
    Ok(signature)
}

fn normalized_path_signature(path: &str, decimals: u32) -> Result<String, String> {
    let segments = svgtypes::PathParser::from(path)
        .map(|segment| {
            segment
                .map(|segment| round_path_segment(segment, decimals))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if segments.is_empty() {
        return Err("path contains no segments".to_string());
    }
    if !path_segments_are_finite(&segments) {
        return Err("path contains a non-finite coordinate".to_string());
    }
    Ok(format!("{segments:?}"))
}

fn round_path_segment(segment: svgtypes::PathSegment, decimals: u32) -> svgtypes::PathSegment {
    use svgtypes::PathSegment;
    let round = |value| round_for_comparison(value, decimals);
    match segment {
        PathSegment::MoveTo { abs, x, y } => PathSegment::MoveTo {
            abs,
            x: round(x),
            y: round(y),
        },
        PathSegment::LineTo { abs, x, y } => PathSegment::LineTo {
            abs,
            x: round(x),
            y: round(y),
        },
        PathSegment::HorizontalLineTo { abs, x } => {
            PathSegment::HorizontalLineTo { abs, x: round(x) }
        }
        PathSegment::VerticalLineTo { abs, y } => PathSegment::VerticalLineTo { abs, y: round(y) },
        PathSegment::CurveTo {
            abs,
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => PathSegment::CurveTo {
            abs,
            x1: round(x1),
            y1: round(y1),
            x2: round(x2),
            y2: round(y2),
            x: round(x),
            y: round(y),
        },
        PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => PathSegment::SmoothCurveTo {
            abs,
            x2: round(x2),
            y2: round(y2),
            x: round(x),
            y: round(y),
        },
        PathSegment::Quadratic { abs, x1, y1, x, y } => PathSegment::Quadratic {
            abs,
            x1: round(x1),
            y1: round(y1),
            x: round(x),
            y: round(y),
        },
        PathSegment::SmoothQuadratic { abs, x, y } => PathSegment::SmoothQuadratic {
            abs,
            x: round(x),
            y: round(y),
        },
        PathSegment::EllipticalArc {
            abs,
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            x,
            y,
        } => PathSegment::EllipticalArc {
            abs,
            rx: round(rx),
            ry: round(ry),
            x_axis_rotation: round(x_axis_rotation),
            large_arc,
            sweep,
            x: round(x),
            y: round(y),
        },
        PathSegment::ClosePath { abs } => PathSegment::ClosePath { abs },
    }
}

fn path_segments_are_finite(segments: &[svgtypes::PathSegment]) -> bool {
    use svgtypes::PathSegment;
    segments.iter().all(|segment| match *segment {
        PathSegment::MoveTo { x, y, .. }
        | PathSegment::LineTo { x, y, .. }
        | PathSegment::SmoothQuadratic { x, y, .. } => [x, y].into_iter().all(f64::is_finite),
        PathSegment::HorizontalLineTo { x, .. } => x.is_finite(),
        PathSegment::VerticalLineTo { y, .. } => y.is_finite(),
        PathSegment::CurveTo {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
            ..
        } => [x1, y1, x2, y2, x, y].into_iter().all(f64::is_finite),
        PathSegment::SmoothCurveTo { x2, y2, x, y, .. } => {
            [x2, y2, x, y].into_iter().all(f64::is_finite)
        }
        PathSegment::Quadratic { x1, y1, x, y, .. } => {
            [x1, y1, x, y].into_iter().all(f64::is_finite)
        }
        PathSegment::EllipticalArc {
            rx,
            ry,
            x_axis_rotation,
            x,
            y,
            ..
        } => [rx, ry, x_axis_rotation, x, y]
            .into_iter()
            .all(f64::is_finite),
        PathSegment::ClosePath { .. } => true,
    })
}

fn normalized_transform_signature(transform: &str, decimals: u32) -> Result<String, String> {
    let parsed = transform
        .parse::<svgtypes::Transform>()
        .map_err(|error| error.to_string())?;
    let values = [parsed.a, parsed.b, parsed.c, parsed.d, parsed.e, parsed.f];
    if !values.into_iter().all(f64::is_finite) {
        return Err("transform contains a non-finite coordinate".to_string());
    }
    Ok(format!(
        "{:?}",
        values.map(|value| round_for_comparison(value, decimals))
    ))
}

fn semantic_element_presentation(
    node: roxmltree::Node<'_, '_>,
    context: &str,
    root_geometry_attributes: &[&str],
    descendant_geometry_attributes: &[&str],
) -> Result<SemanticLabelPresentation, SemanticLabelError> {
    let mut element_structure = Vec::new();
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
        element_structure.push(format!(
            "ancestor[{ancestor_index}]={}",
            semantic_element_name(element)
        ));
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
        element_structure.push(format!("{scope}={}", semantic_element_name(element)));
        collect_element_presentation(
            element,
            &scope,
            context,
            if element_index == 0 {
                root_geometry_attributes
            } else {
                descendant_geometry_attributes
            },
            false,
            &mut attributes,
            &mut inline_style,
            &mut class_tokens,
        )?;
    }

    Ok(SemanticLabelPresentation {
        element_structure,
        attributes,
        inline_style,
        class_tokens,
    })
}

fn semantic_element_name(element: roxmltree::Node<'_, '_>) -> String {
    let name = element.tag_name();
    match name.namespace() {
        Some(namespace) => format!("{{{namespace}}}{}", name.name()),
        None => name.name().to_string(),
    }
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
        let is_unqualified = attribute.namespace().is_none();
        if (is_unqualified && matches!(attribute.name(), "style" | "class"))
            || (is_unqualified && excluded_attributes.contains(&attribute.name()))
            || (presentation_attributes_only
                && !is_inherited_presentation_attribute(attribute.name()))
        {
            continue;
        }
        let attribute_name = match attribute.namespace() {
            Some(namespace) => format!("{{{namespace}}}{}", attribute.name()),
            None => attribute.name().to_string(),
        };
        attributes.insert(
            format!("{scope}@{attribute_name}"),
            attribute.value().to_string(),
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
    // Mermaid 11.16 serializes this exact sentinel on ER relationship paths when both style
    // fragments are absent. Browsers ignore it as invalid CSS; retain the source bytes in the
    // signature without broadening acceptance to any other invalid declaration.
    if style.trim() == "undefined;;;undefined" {
        return Ok(vec![(
            "@mermaid-invalid-style-sentinel".to_string(),
            "undefined;;;undefined".to_string(),
        )]);
    }

    let mut input = cssparser::ParserInput::new(style);
    let mut parser = cssparser::Parser::new(&mut input);
    let mut declarations = Vec::new();

    loop {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }
        if parser.try_parse(|input| input.expect_semicolon()).is_ok() {
            continue;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct StylesheetRule {
    selector: String,
    body: String,
    kind: StylesheetRuleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StylesheetRuleKind {
    Qualified,
    AtRuleWithBlock,
    AtRuleWithoutBlock,
}

struct SemanticStylesheetParser;

impl<'i> cssparser::QualifiedRuleParser<'i> for SemanticStylesheetParser {
    type Prelude = String;
    type QualifiedRule = Option<StylesheetRule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        let selector = input.slice_from(start).trim().to_string();
        if selector.is_empty() {
            Err(input.new_custom_error(()))
        } else {
            Ok(selector)
        }
    }

    fn parse_block<'t>(
        &mut self,
        selector: Self::Prelude,
        _start: &cssparser::ParserState,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        Ok(Some(StylesheetRule {
            selector,
            body: input.slice_from(start).trim().to_string(),
            kind: StylesheetRuleKind::Qualified,
        }))
    }
}

impl<'i> cssparser::AtRuleParser<'i> for SemanticStylesheetParser {
    type Prelude = String;
    type AtRule = Option<StylesheetRule>;
    type Error = ();

    fn parse_prelude<'t>(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self::Prelude, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        let prelude = input.slice_from(start).trim();
        if prelude.is_empty() {
            Ok(format!("@{name}"))
        } else {
            Ok(format!("@{name} {prelude}"))
        }
    }

    fn rule_without_block(
        &mut self,
        selector: Self::Prelude,
        _start: &cssparser::ParserState,
    ) -> Result<Self::AtRule, ()> {
        Ok(Some(StylesheetRule {
            selector,
            body: String::new(),
            kind: StylesheetRuleKind::AtRuleWithoutBlock,
        }))
    }

    fn parse_block<'t>(
        &mut self,
        selector: Self::Prelude,
        _start: &cssparser::ParserState,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self::AtRule, cssparser::ParseError<'i, Self::Error>> {
        let start = input.position();
        while !input.is_exhausted() {
            input.next_including_whitespace_and_comments()?;
        }
        Ok(Some(StylesheetRule {
            selector,
            body: input.slice_from(start).trim().to_string(),
            kind: StylesheetRuleKind::AtRuleWithBlock,
        }))
    }
}

fn extract_stylesheet_signature(
    svg: &str,
    scope: StylesheetScope,
) -> Result<Vec<String>, SemanticLabelError> {
    let normalized = crate::svgdom::normalize_xml_entities(svg);
    let document = roxmltree::Document::parse(normalized.as_ref())
        .map_err(|error| SemanticLabelError::InvalidSvg(error.to_string()))?;
    let stylesheets = document
        .descendants()
        .filter(|node| node.is_element() && node.has_tag_name("style"))
        .map(|node| {
            node.children()
                .filter(roxmltree::Node::is_text)
                .filter_map(|child| child.text())
                .collect::<String>()
                .trim()
                .to_string()
        })
        .collect::<Vec<_>>();
    if stylesheets.is_empty() {
        return Err(SemanticLabelError::MissingStylesheet);
    }

    if scope == StylesheetScope::Full {
        return Ok(stylesheets);
    }

    let has_neo_surface = document.descendants().any(|node| {
        node.is_element()
            && (node
                .attribute("data-look")
                .is_some_and(|look| look.eq_ignore_ascii_case("neo"))
                || node
                    .attribute("class")
                    .unwrap_or_default()
                    .split_whitespace()
                    .any(|class| class.starts_with("neo-") || class == "icon-neo"))
    });
    let root_id = document
        .root_element()
        .attribute("id")
        .unwrap_or_default()
        .trim();
    let mut signature = Vec::new();
    for stylesheet in stylesheets {
        let mut input = cssparser::ParserInput::new(&stylesheet);
        let mut input = cssparser::Parser::new(&mut input);
        let mut rule_parser = SemanticStylesheetParser;
        for parsed in cssparser::StyleSheetParser::new(&mut input, &mut rule_parser) {
            let rule = parsed.map_err(|(error, rule)| SemanticLabelError::InvalidStylesheet {
                rule: rule.trim().to_string(),
                message: format!("{error:?}"),
            })?;
            let Some(rule) = rule else {
                continue;
            };
            if rule.kind != StylesheetRuleKind::Qualified
                || classic_dagre_stylesheet_rule_is_relevant(
                    &rule.selector,
                    root_id,
                    has_neo_surface,
                )
            {
                signature.push(match rule.kind {
                    StylesheetRuleKind::Qualified => {
                        let declarations =
                            parse_inline_style_declarations(&rule.body, &rule.selector)?;
                        let declarations = serde_json::to_string(&declarations)
                            .expect("serializing stylesheet declarations cannot fail");
                        format!("{}{declarations}", rule.selector)
                    }
                    StylesheetRuleKind::AtRuleWithBlock => {
                        format!("{}{{{}}}", rule.selector, rule.body)
                    }
                    StylesheetRuleKind::AtRuleWithoutBlock => format!("{};", rule.selector),
                });
            }
        }
    }
    if signature.is_empty() {
        Err(SemanticLabelError::MissingRelevantStylesheet)
    } else {
        Ok(signature)
    }
}

fn classic_dagre_stylesheet_rule_is_relevant(
    selector: &str,
    root_id: &str,
    has_neo_surface: bool,
) -> bool {
    if has_neo_surface {
        return true;
    }

    // The pinned baseline has ten extra Neo-only rules that cannot match this classic DOM.
    let root_prefix = format!("#{root_id} ");
    let Some(unscoped_selector) = selector
        .split(',')
        .map(str::trim)
        .map(|component| component.strip_prefix(&root_prefix))
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join(","))
    else {
        return true;
    };
    !CLASSIC_DAGRE_INACTIVE_NEO_SELECTORS.contains(&unscoped_selector.as_str())
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
    world_element_geometry(node, x, y, text)
}

fn world_element_geometry(
    node: roxmltree::Node<'_, '_>,
    x: f64,
    y: f64,
    text: &str,
) -> Result<WorldTextGeometry, SemanticLabelError> {
    let world = composed_element_transform(node, text)?;
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

fn composed_element_transform(
    node: roxmltree::Node<'_, '_>,
    context: &str,
) -> Result<AffineTransform, SemanticLabelError> {
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
                context: format!("transform `{value}` for `{context}`"),
            });
        }
        world = world.multiply(local);
        if !world.is_finite() {
            return Err(SemanticLabelError::NonFiniteGeometry {
                context: format!("composed transform for `{context}`"),
            });
        }
    }
    Ok(world)
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

    fn signed_semantic_source(diagram: &str, fixture: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(diagram)
            .join(format!("{fixture}.mmd"));
        std::fs::read_to_string(path).unwrap()
    }

    fn signed_semantic_svg(diagram: &str, fixture: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/upstream-svgs")
            .join(diagram)
            .join(format!("{fixture}.svg"));
        std::fs::read_to_string(path).unwrap()
    }

    fn swap_all(input: &str, left: &str, right: &str) -> String {
        assert!(input.contains(left));
        assert!(input.contains(right));
        let sentinel = "__MERMAN_SEMANTIC_LABEL_SWAP_SENTINEL__";
        assert!(!input.contains(sentinel));
        input
            .replace(left, sentinel)
            .replace(right, left)
            .replace(sentinel, right)
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
    fn semantic_label_fixture_contracts_match_all_registered_artifacts() {
        assert_eq!(
            registered_semantic_label_fixtures("flowchart"),
            &[FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE]
        );
        assert_eq!(
            registered_semantic_label_fixtures("architecture"),
            &[ARCHITECTURE_PARALLEL_LABEL_FIXTURE]
        );
        assert_eq!(
            registered_semantic_label_fixtures("requirement"),
            &[REQUIREMENT_TRACES_LABEL_FIXTURE]
        );
        assert_eq!(
            registered_semantic_label_fixtures("state"),
            &[STATE_PARALLEL_LABEL_FIXTURE]
        );
        assert_eq!(
            registered_semantic_label_fixtures("class"),
            &[CLASS_MANY_RELATION_LABEL_FIXTURE]
        );
        assert_eq!(
            registered_semantic_label_fixtures("er"),
            &[ER_PARALLEL_RELATION_LABEL_FIXTURE]
        );
        assert_eq!(
            SEMANTIC_LABEL_FIXTURE_CONTRACTS.len(),
            C4_SEMANTIC_LABEL_FIXTURES.len()
                + FLOWCHART_SEMANTIC_LABEL_FIXTURES.len()
                + ARCHITECTURE_SEMANTIC_LABEL_FIXTURES.len()
                + REQUIREMENT_SEMANTIC_LABEL_FIXTURES.len()
                + STATE_SEMANTIC_LABEL_FIXTURES.len()
                + CLASS_SEMANTIC_LABEL_FIXTURES.len()
                + ER_SEMANTIC_LABEL_FIXTURES.len()
        );

        for contract in SEMANTIC_LABEL_FIXTURE_CONTRACTS {
            assert!(crate::util::is_canonical_sha256(contract.input_sha256));
            assert!(crate::util::is_canonical_sha256(
                contract.upstream_svg_sha256
            ));
            let source = signed_semantic_source(contract.diagram, contract.fixture);
            let upstream = signed_semantic_svg(contract.diagram, contract.fixture);
            assert_eq!(
                crate::util::sha256_hex(source.as_bytes()),
                contract.input_sha256
            );
            assert_eq!(
                crate::util::sha256_hex(upstream.as_bytes()),
                contract.upstream_svg_sha256
            );
            validate_registered_fixture_digests(contract, &source, &upstream).unwrap();
        }
    }

    #[test]
    fn flowchart_elk_labels_bind_shared_data_ids_to_their_paths() {
        let svg = signed_semantic_svg("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let labels = extract_flowchart_elk_edge_labels(&svg).unwrap();

        assert_eq!(labels.len(), 2);
        let lower = labels.get("L_a1_a2_0").unwrap();
        let upper = labels.get("L_a1_a2_2").unwrap();
        assert_eq!(lower.text, "l1");
        assert_eq!(upper.text, "l2");
        assert_eq!(lower.geometry.anchor_x, 130.796875);
        assert_eq!(lower.geometry.anchor_y, 96.5);
        assert_eq!(upper.geometry.anchor_x, 130.796875);
        assert_eq!(upper.geometry.anchor_y, 56.5);
        assert!(
            lower.associated_edge.as_ref().unwrap().geometry.attributes["d"]
                .contains("101.92893218813452")
        );
        assert_eq!(
            upper.associated_edge.as_ref().unwrap().geometry.attributes["d"],
            "M173.90625,69L104.796875,69"
        );
    }

    #[test]
    fn dagre_family_labels_bind_shared_data_ids_to_their_paths() {
        for (diagram, fixture, expected_samples) in [
            ("requirement", REQUIREMENT_TRACES_LABEL_FIXTURE, 8),
            ("state", STATE_PARALLEL_LABEL_FIXTURE, 5),
            ("class", CLASS_MANY_RELATION_LABEL_FIXTURE, 8),
            ("er", ER_PARALLEL_RELATION_LABEL_FIXTURE, 2),
        ] {
            let svg = signed_semantic_svg(diagram, fixture);
            let config = dagre_data_id_adapter_config(diagram).unwrap();
            let labels = extract_dagre_data_id_edge_labels(&svg, config).unwrap();

            assert_eq!(labels.len(), expected_samples, "{diagram}/{fixture}");
            assert!(labels.values().all(|label| label.associated_edge.is_some()));
        }

        let state = extract_dagre_data_id_edge_labels(
            &signed_semantic_svg("state", STATE_PARALLEL_LABEL_FIXTURE),
            dagre_data_id_adapter_config("state").unwrap(),
        )
        .unwrap();
        assert_eq!(state["edge1"].text, "fast");
        assert_eq!(state["edge2"].text, "slow");
        assert_eq!(state["edge3"].text, "retry");
        assert!(state["edge0"].text.is_empty());
        assert!(state["edge4"].text.is_empty());

        let requirement = extract_dagre_data_id_edge_labels(
            &signed_semantic_svg("requirement", REQUIREMENT_TRACES_LABEL_FIXTURE),
            dagre_data_id_adapter_config("requirement").unwrap(),
        )
        .unwrap();
        let traces = &requirement["test_req-test_req2-0"];
        assert_eq!(traces.text, "<<traces>>");
        assert_eq!(traces.geometry.anchor_x, 242.40006);
        assert_eq!(traces.geometry.anchor_y, 426.14623);
    }

    #[test]
    fn dagre_family_semantic_gates_accept_exact_geometry_without_catalog() {
        for (diagram, fixture, expected_samples) in [
            ("requirement", REQUIREMENT_TRACES_LABEL_FIXTURE, 8),
            ("state", STATE_PARALLEL_LABEL_FIXTURE, 5),
            ("class", CLASS_MANY_RELATION_LABEL_FIXTURE, 8),
            ("er", ER_PARALLEL_RELATION_LABEL_FIXTURE, 2),
        ] {
            let source = signed_semantic_source(diagram, fixture);
            let upstream = signed_semantic_svg(diagram, fixture);
            let config = dagre_data_id_adapter_config(diagram).unwrap();
            let pairs = pair_dagre_data_id_edge_labels(config, &upstream, &upstream).unwrap();
            let outcome = compare_semantic_edge_labels(
                diagram,
                "unregistered-exact-geometry-probe",
                &source,
                stable_residual_pairs(pairs),
                &upstream,
                &upstream,
                3,
                StylesheetScope::ClassicDagre,
            )
            .unwrap();

            assert!(outcome.issues.is_empty(), "{diagram}: {:?}", outcome.issues);
            assert_eq!(outcome.evidence.compared_samples, expected_samples);
            assert_eq!(outcome.evidence.accepted_residuals, 0);
        }
    }

    #[test]
    fn dagre_family_semantic_gate_rejects_identity_geometry_and_style_mutations() {
        let diagram = "er";
        let fixture = ER_PARALLEL_RELATION_LABEL_FIXTURE;
        let source = signed_semantic_source(diagram, fixture);
        let upstream = signed_semantic_svg(diagram, fixture);
        let first_id = "id_entity-CUSTOMER-0_entity-ADDRESS-1_0";
        let second_id = "id_entity-CUSTOMER-0_entity-ADDRESS-1_1";
        let swapped_label_identities = swap_all(
            &upstream,
            &format!(r#"<g class="label" data-id="{first_id}""#),
            &format!(r#"<g class="label" data-id="{second_id}""#),
        );
        let changed_path = upstream.replacen("M72.528,92", "M82.528,92", 1);
        let changed_data_points = upstream.replacen(
            "W3sieCI6NzIuNTI3NTEyNjY4OTE4OTIsInkiOjkyfSx7IngiOjQyLjc1LCJ5IjoxNDIuNX0seyJ4Ijo3Mi41Mjc1MTI2Njg5MTg5MiwieSI6MTkzfV0=",
            "W3sieCI6ODIuNTI3NTEyNjY4OTE4OTIsInkiOjkyfSx7IngiOjQyLjc1LCJ5IjoxNDIuNX0seyJ4Ijo3Mi41Mjc1MTI2Njg5MTg5MiwieSI6MTkzfV0=",
            1,
        );
        assert_ne!(changed_data_points, upstream);
        let changed_style = upstream.replacen(
            r#"class="edge-thickness-normal edge-pattern-solid relationshipLine""#,
            r#"class="edge-thickness-normal edge-pattern-dashed relationshipLine""#,
            1,
        );
        let changed_stylesheet = upstream.replacen(
            ".edgeLabel{background-color:",
            ".edgeLabel{outline-color:",
            1,
        );
        assert_ne!(changed_stylesheet, upstream);
        let changed_descendant_width = upstream.replacen(
            r#"<foreignObject width="69.5" height="21">"#,
            r#"<foreignObject width="79.5" height="21">"#,
            1,
        );
        assert_ne!(changed_descendant_width, upstream);
        let changed_descendant_transform = upstream.replacen(
            r#"<g class="label" data-id="id_entity-CUSTOMER-0_entity-ADDRESS-1_0" transform="translate(-34.75, -10.5)">"#,
            r#"<g class="label" data-id="id_entity-CUSTOMER-0_entity-ADDRESS-1_0" transform="translate(-24.75, -10.5)">"#,
            1,
        );
        assert_ne!(changed_descendant_transform, upstream);

        for local in [
            swapped_label_identities,
            changed_path,
            changed_data_points,
            changed_style,
            changed_stylesheet,
        ] {
            let outcome =
                compare_registered_semantic_labels(diagram, fixture, &source, &upstream, &local, 3)
                    .unwrap()
                    .unwrap();
            assert!(!outcome.issues.is_empty());
        }

        for local in [changed_descendant_width, changed_descendant_transform] {
            let outcome =
                compare_registered_semantic_labels(diagram, fixture, &source, &upstream, &local, 3)
                    .unwrap()
                    .unwrap();
            assert!(outcome.issues.iter().any(|issue| {
                issue.contains("geometry differs without an exact residual contract")
            }));
        }

        let missing_label_identity = upstream.replacen(&format!(r#" data-id="{first_id}""#), "", 2);
        assert!(
            compare_registered_semantic_labels(
                diagram,
                fixture,
                &source,
                &upstream,
                &missing_label_identity,
                3,
            )
            .is_err()
        );
    }

    #[test]
    fn registered_residuals_become_stale_when_local_geometry_is_exact() {
        for (diagram, fixture) in [
            ("requirement", REQUIREMENT_TRACES_LABEL_FIXTURE),
            ("er", ER_PARALLEL_RELATION_LABEL_FIXTURE),
        ] {
            let source = signed_semantic_source(diagram, fixture);
            let upstream = signed_semantic_svg(diagram, fixture);
            let outcome = compare_registered_semantic_labels(
                diagram, fixture, &source, &upstream, &upstream, 3,
            )
            .unwrap()
            .unwrap();

            assert_eq!(outcome.evidence.accepted_residuals, 0);
            assert!(outcome.issues.iter().any(|issue| {
                issue.contains("stale semantic label residual contracts were not exercised")
            }));
        }
    }

    #[test]
    fn architecture_labels_bind_to_the_stable_path_in_their_direct_group() {
        let svg = signed_semantic_svg("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let labels = extract_architecture_edge_labels(&svg).unwrap();

        assert_eq!(labels.len(), 3);
        let reads = labels
            .get(&format!(
                "{ARCHITECTURE_PARALLEL_LABEL_FIXTURE}-L_api_db_read_0"
            ))
            .unwrap();
        assert_eq!(reads.text, "reads");
        assert_eq!(reads.geometry.anchor_x, 60.75);
        assert_eq!(reads.geometry.anchor_y, 69.75000000000003);
        assert!(
            reads.associated_edge.as_ref().unwrap().geometry.attributes["d"]
                .contains("121.33184381200331")
        );
    }

    #[test]
    fn flowchart_elk_semantic_gate_accepts_the_exact_signed_fixture() {
        let source = signed_semantic_source("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let upstream = signed_semantic_svg("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);

        let outcome = compare_registered_semantic_labels(
            "flowchart",
            FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &upstream,
            3,
        )
        .unwrap()
        .unwrap();

        assert!(outcome.issues.is_empty());
        assert_eq!(outcome.evidence.compared_samples, 2);
        assert_eq!(outcome.evidence.accepted_residuals, 0);
    }

    #[test]
    fn architecture_semantic_gate_accepts_the_exact_signed_fixture() {
        let source = signed_semantic_source("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let upstream = signed_semantic_svg("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);

        let outcome = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &upstream,
            3,
        )
        .unwrap()
        .unwrap();

        assert!(outcome.issues.is_empty());
        assert_eq!(outcome.evidence.compared_samples, 3);
        assert_eq!(outcome.evidence.accepted_residuals, 0);
    }

    #[test]
    fn flowchart_elk_semantic_gate_rejects_transform_identity_and_owner_mutations() {
        let source = signed_semantic_source("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let upstream = signed_semantic_svg("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let swapped_transforms = swap_all(
            &upstream,
            "translate(137.3515625, 108.5)",
            "translate(137.3515625, 68.5)",
        );
        let transformed_edge_root = upstream.replacen(
            r#"class="edges edgePaths""#,
            r#"class="edges edgePaths" transform="translate(10 0)""#,
            1,
        );
        let swapped_identities = swap_all(&upstream, "L_a1_a2_0", "L_a1_a2_2");
        let missing_path_identity = upstream.replacen(r#" data-id="L_a1_a2_0""#, "", 1);
        let missing_label_identity = upstream.replacen(
            r#"<g class="label" data-id="L_a1_a2_0""#,
            r#"<g class="label""#,
            1,
        );

        let transform_outcome = compare_registered_semantic_labels(
            "flowchart",
            FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &swapped_transforms,
            3,
        )
        .unwrap()
        .unwrap();
        assert!(
            transform_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("label or associated edge geometry differs"))
        );

        let ancestor_transform_outcome = compare_registered_semantic_labels(
            "flowchart",
            FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &transformed_edge_root,
            3,
        )
        .unwrap()
        .unwrap();
        assert!(
            ancestor_transform_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("label or associated edge geometry differs"))
        );

        let identity_outcome = compare_registered_semantic_labels(
            "flowchart",
            FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &swapped_identities,
            3,
        )
        .unwrap()
        .unwrap();
        assert!(
            identity_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("label text differs"))
        );

        let path_identity_error = compare_registered_semantic_labels(
            "flowchart",
            FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &missing_path_identity,
            3,
        )
        .unwrap_err();
        assert!(path_identity_error.contains("semantic edge identity is empty"));

        let label_identity_error = compare_registered_semantic_labels(
            "flowchart",
            FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &missing_label_identity,
            3,
        )
        .unwrap_err();
        assert!(label_identity_error.contains("semantic edge identity is empty"));
    }

    #[test]
    fn architecture_semantic_gate_rejects_transform_identity_and_owner_mutations() {
        let source = signed_semantic_source("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let upstream = signed_semantic_svg("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let swapped_transforms = swap_all(
            &upstream,
            "translate(60.75, 69.75000000000003)",
            "translate(-39.831843812003314, 170.2123991263755) rotate(-90)",
        );
        let read_id = format!("{ARCHITECTURE_PARALLEL_LABEL_FIXTURE}-L_api_db_read_0");
        let write_id = format!("{ARCHITECTURE_PARALLEL_LABEL_FIXTURE}-L_api_db_write_0");
        let swapped_identities = swap_all(&upstream, &read_id, &write_id);
        let missing_owner = upstream.replacen(&format!(r#" id="{read_id}""#), "", 1);

        let transform_outcome = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &swapped_transforms,
            3,
        )
        .unwrap()
        .unwrap();
        assert!(
            transform_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("label or associated edge geometry differs"))
        );

        let identity_outcome = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &swapped_identities,
            3,
        )
        .unwrap()
        .unwrap();
        assert!(
            identity_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("label text differs"))
        );

        let owner_error = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &missing_owner,
            3,
        )
        .unwrap_err();
        assert!(owner_error.contains("semantic edge identity is empty"));

        let path_without_label = upstream.replacen(
            r#"<g class="architecture-edges">"#,
            r#"<g class="architecture-edges"><g><path class="edge" id="new-edge" d="M0,0L1,1"/></g>"#,
            1,
        );
        let path_without_label_error = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &path_without_label,
            3,
        )
        .unwrap_err();
        assert!(path_without_label_error.contains("semantic edge `new-edge` has no owning label"));

        let direct_path_without_label = upstream.replacen(
            r#"<g class="architecture-edges">"#,
            r#"<g class="architecture-edges"><path class="edge" id="new-direct-edge" d="M0,0L1,1"/>"#,
            1,
        );
        let direct_path_error = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &direct_path_without_label,
            3,
        )
        .unwrap_err();
        assert!(direct_path_error.contains("semantic edge `new-direct-edge` has no owning label"));

        let nested_path_without_label = upstream.replacen(
            r#"<g class="architecture-edges">"#,
            r#"<g class="architecture-edges"><g><g><path class="edge" id="new-nested-edge" d="M0,0L1,1"/></g></g>"#,
            1,
        );
        let nested_path_error = compare_registered_semantic_labels(
            "architecture",
            ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
            &source,
            &upstream,
            &nested_path_without_label,
            3,
        )
        .unwrap_err();
        assert!(nested_path_error.contains("has 1 edge paths and 0 labels"));
    }

    #[test]
    fn flowchart_elk_semantic_gate_rejects_path_presentation_and_css_mutations() {
        let source = signed_semantic_source("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let upstream = signed_semantic_svg("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let compare = |local: &str| {
            compare_registered_semantic_labels(
                "flowchart",
                FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE,
                &source,
                &upstream,
                local,
                3,
            )
            .unwrap()
            .unwrap()
        };

        let changed_path = upstream.replacen("M173.90625,87", "M183.90625,87", 1);
        assert!(
            compare(&changed_path)
                .issues
                .iter()
                .any(|issue| { issue.contains("without an exact residual contract") })
        );

        let changed_edge_presentation = upstream.replacen(
            r#"data-edge="true" data-et="edge""#,
            r#"data-edge="true" data-et="other""#,
            1,
        );
        assert!(
            compare(&changed_edge_presentation)
                .issues
                .iter()
                .any(|issue| issue.contains("associated edge presentation differs"))
        );

        let changed_label_presentation = upstream.replacen(
            r#"<foreignObject width="13.109375" height="24">"#,
            r#"<foreignObject width="13.109375" height="24" style="overflow: hidden;">"#,
            1,
        );
        assert!(
            compare(&changed_label_presentation)
                .issues
                .iter()
                .any(|issue| issue.contains("explicit label presentation differs"))
        );

        let changed_label_structure = upstream.replacen(
            r#"<p>l1</p></span></div></foreignObject></g>"#,
            r#"<p>l1</p></span></div></foreignObject><text>evil</text></g>"#,
            1,
        );
        assert_ne!(changed_label_structure, upstream);
        assert!(
            compare(&changed_label_structure)
                .issues
                .iter()
                .any(|issue| issue.contains("explicit label presentation differs"))
        );

        let changed_css = upstream.replacen(
            ".flowchart-link{stroke:#333333;fill:none;}",
            ".flowchart-link{stroke:#abcdef;fill:none;}",
            1,
        );
        assert!(
            compare(&changed_css)
                .issues
                .iter()
                .any(|issue| issue.contains("semantic stylesheet source differs"))
        );

        let root_paragraph_rule = format!("#{FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE} p{{margin:0;}}");
        let changed_root_paragraph_css = upstream.replacen(
            &root_paragraph_rule,
            &format!("#{FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE} p{{margin:12px;}}"),
            1,
        );
        assert_ne!(changed_root_paragraph_css, upstream);
        assert!(
            compare(&changed_root_paragraph_css)
                .issues
                .iter()
                .any(|issue| issue.contains("semantic stylesheet source differs"))
        );

        let changed_generic_css = upstream.replacen("</style>", "p{margin:12px;}</style>", 1);
        assert_ne!(changed_generic_css, upstream);
        assert!(
            compare(&changed_generic_css)
                .issues
                .iter()
                .any(|issue| issue.contains("semantic stylesheet source differs"))
        );

        let changed_negated_neo_css = upstream.replacen(
            "</style>",
            r#"p:not([data-look="neo"]){margin:12px;}</style>"#,
            1,
        );
        assert_ne!(changed_negated_neo_css, upstream);
        assert!(
            compare(&changed_negated_neo_css)
                .issues
                .iter()
                .any(|issue| issue.contains("semantic stylesheet source differs"))
        );

        let changed_at_rule_css = upstream.replacen(
            "</style>",
            &format!(
                "@media all {{ #{FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE} p {{ margin:12px; }} }}</style>"
            ),
            1,
        );
        assert_ne!(changed_at_rule_css, upstream);
        assert!(
            compare(&changed_at_rule_css)
                .issues
                .iter()
                .any(|issue| issue.contains("semantic stylesheet source differs"))
        );
    }

    #[test]
    fn architecture_semantic_gate_rejects_text_path_and_presentation_mutations() {
        let source = signed_semantic_source("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let upstream = signed_semantic_svg("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let compare = |local: &str| {
            compare_registered_semantic_labels(
                "architecture",
                ARCHITECTURE_PARALLEL_LABEL_FIXTURE,
                &source,
                &upstream,
                local,
                3,
            )
            .unwrap()
            .unwrap()
        };

        let changed_text = upstream.replacen(">reads<", ">fetches<", 1);
        assert!(
            compare(&changed_text)
                .issues
                .iter()
                .any(|issue| issue.contains("label text differs"))
        );

        let changed_path = upstream.replacen(
            "M 0.1681561879966864,69.75000000000003",
            "M 10.1681561879966864,69.75000000000003",
            1,
        );
        assert!(
            compare(&changed_path)
                .issues
                .iter()
                .any(|issue| { issue.contains("without an exact residual contract") })
        );

        let changed_presentation = upstream.replacen(
            r#"class="edge" id="stress_architecture"#,
            r#"class="edge highlighted" id="stress_architecture"#,
            1,
        );
        assert!(
            compare(&changed_presentation)
                .issues
                .iter()
                .any(|issue| issue.contains("associated edge presentation differs"))
        );

        let changed_css = upstream.replacen(
            ".edge{stroke-width:3;stroke:#333333;fill:none;}",
            ".edge{stroke-width:4;stroke:#333333;fill:none;}",
            1,
        );
        assert!(
            compare(&changed_css)
                .issues
                .iter()
                .any(|issue| issue.contains("semantic stylesheet source differs"))
        );
    }

    #[test]
    fn stable_edge_extractors_reject_duplicate_and_unpaired_identities() {
        let flowchart = signed_semantic_svg("flowchart", FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE);
        let duplicate_flowchart = flowchart.replace("L_a1_a2_2", "L_a1_a2_0");
        assert!(matches!(
            extract_flowchart_elk_edge_labels(&duplicate_flowchart),
            Err(SemanticLabelError::DuplicateEdgeIdentity {
                diagram: "flowchart",
                ..
            })
        ));

        let architecture = signed_semantic_svg("architecture", ARCHITECTURE_PARALLEL_LABEL_FIXTURE);
        let duplicate_architecture = architecture.replace(
            &format!("{ARCHITECTURE_PARALLEL_LABEL_FIXTURE}-L_api_db_write_0"),
            &format!("{ARCHITECTURE_PARALLEL_LABEL_FIXTURE}-L_api_db_read_0"),
        );
        assert!(matches!(
            extract_architecture_edge_labels(&duplicate_architecture),
            Err(SemanticLabelError::DuplicateEdgeIdentity {
                diagram: "architecture",
                ..
            })
        ));

        let state = signed_semantic_svg("state", STATE_PARALLEL_LABEL_FIXTURE);
        let padded_identity = state.replace(r#"data-id="edge1""#, r#"data-id=" edge1 ""#);
        assert_eq!(padded_identity.matches(r#"data-id=" edge1 ""#).count(), 2);
        assert!(matches!(
            pair_dagre_data_id_edge_labels(
                dagre_data_id_adapter_config("state").unwrap(),
                &state,
                &padded_identity,
            ),
            Err(SemanticLabelError::StableIdentitySetMismatch {
                diagram: "state",
                ..
            })
        ));
    }

    #[test]
    fn c4_semantic_label_gate_accepts_all_exact_reviewed_residuals() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let local = c4_dynamic_svg_with_reviewed_residuals();

        for dom_decimals in [3, 6] {
            let outcome = compare_registered_semantic_labels(
                "c4",
                C4_DYNAMIC_LABEL_FIXTURE,
                &source,
                &upstream,
                &local,
                dom_decimals,
            )
            .unwrap()
            .unwrap();

            assert!(outcome.issues.is_empty());
            assert_eq!(outcome.evidence.compared_samples, 5);
            assert_eq!(outcome.evidence.accepted_residuals, 5);
        }
    }

    #[test]
    fn c4_semantic_label_gate_rejects_zero_sample_selector_drift() {
        let source = signed_c4_dynamic_source();
        let upstream = signed_c4_dynamic_svg();
        let error = compare_registered_semantic_labels(
            "c4",
            C4_DYNAMIC_LABEL_FIXTURE,
            &source,
            &upstream,
            "<svg/>",
            3,
        )
        .unwrap_err();

        assert!(error.contains("missing from local"));
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

        assert!(
            path_outcome
                .issues
                .iter()
                .any(|issue| { issue.contains("without an exact residual contract") })
        );
        assert!(
            stroke_outcome
                .issues
                .iter()
                .any(|issue| issue.contains("associated edge presentation differs"))
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
                .any(|issue| issue.contains("semantic stylesheet source differs"))
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
            r#";;fill:red!important;fill:blue;font-family:"Open  Sans";--Theme: A;;"#,
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
        assert_eq!(
            parse_inline_style_declarations("undefined;;;undefined", "ER edge").unwrap(),
            vec![(
                "@mermaid-invalid-style-sentinel".to_string(),
                "undefined;;;undefined".to_string(),
            )]
        );
        assert!(
            parse_inline_style_declarations("undefined;;;other", "ER edge").is_err(),
            "only the exact pinned Mermaid sentinel may bypass CSS declaration parsing"
        );
    }

    #[test]
    fn classic_dagre_stylesheet_signature_ignores_only_insignificant_semicolons() {
        let without_trailing_semicolon =
            r#"<svg id="fixture"><style>#fixture svg{font-size:16px}</style></svg>"#;
        let with_trailing_semicolon =
            r#"<svg id="fixture"><style>#fixture svg{font-size:16px;}</style></svg>"#;
        let changed_value =
            r#"<svg id="fixture"><style>#fixture svg{font-size:17px;}</style></svg>"#;

        let expected =
            extract_stylesheet_signature(without_trailing_semicolon, StylesheetScope::ClassicDagre)
                .unwrap();
        assert_eq!(
            expected,
            extract_stylesheet_signature(with_trailing_semicolon, StylesheetScope::ClassicDagre)
                .unwrap()
        );
        assert_ne!(
            expected,
            extract_stylesheet_signature(changed_value, StylesheetScope::ClassicDagre).unwrap()
        );

        let comment_split_injection = r#"<svg id="fixture"><style>#fixture svg{font-size:16px;}<!--split-->#fixture .edgeLabel{display:none;}</style></svg>"#;
        assert_ne!(
            expected,
            extract_stylesheet_signature(comment_split_injection, StylesheetScope::ClassicDagre,)
                .unwrap()
        );
    }

    #[test]
    fn semantic_presentation_preserves_attribute_namespaces() {
        let unqualified = roxmltree::Document::parse(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><text fill="red">label</text></svg>"#,
        )
        .unwrap();
        let namespaced = roxmltree::Document::parse(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:n="urn:test"><text n:fill="red">label</text></svg>"#,
        )
        .unwrap();
        let presentation = |document: &roxmltree::Document<'_>| {
            semantic_label_presentation(
                document
                    .descendants()
                    .find(|node| node.has_tag_name("text"))
                    .unwrap(),
                "label",
            )
            .unwrap()
        };

        let unqualified = presentation(&unqualified);
        let namespaced = presentation(&namespaced);
        assert_ne!(unqualified, namespaced);
        assert_eq!(unqualified.attributes["root@fill"], "red");
        assert_eq!(namespaced.attributes["root@{urn:test}fill"], "red");
        assert!(!namespaced.attributes.contains_key("root@fill"));
    }

    #[test]
    fn classic_dagre_neo_rule_filter_is_an_exact_allowlist() {
        assert!(!classic_dagre_stylesheet_rule_is_relevant(
            "#fixture [data-look=neo].labelBkg",
            "fixture",
            false,
        ));
        assert!(classic_dagre_stylesheet_rule_is_relevant(
            r#"#fixture p:not([data-look="neo"])"#,
            "fixture",
            false,
        ));
        assert!(classic_dagre_stylesheet_rule_is_relevant(
            "#fixture [data-look=neo].labelBkg:hover",
            "fixture",
            false,
        ));
    }

    #[test]
    fn data_points_signature_decodes_and_validates_structured_geometry() {
        let upstream =
            base64::engine::general_purpose::STANDARD.encode(br#"[{"x":1.23449,"y":-2.34549}]"#);
        let same_at_three_decimals =
            base64::engine::general_purpose::STANDARD.encode(br#"[{"x":1.23448,"y":-2.34548}]"#);
        let changed_at_three_decimals =
            base64::engine::general_purpose::STANDARD.encode(br#"[{"x":1.2351,"y":-2.34548}]"#);

        assert_eq!(
            normalized_data_points_signature(&upstream, 3).unwrap(),
            normalized_data_points_signature(&same_at_three_decimals, 3).unwrap()
        );
        assert_ne!(
            normalized_data_points_signature(&upstream, 3).unwrap(),
            normalized_data_points_signature(&changed_at_three_decimals, 3).unwrap()
        );
        assert!(normalized_data_points_signature("not-base64", 3).is_err());

        let non_finite =
            base64::engine::general_purpose::STANDARD.encode(br#"[{"x":1e999,"y":0}]"#);
        assert!(normalized_data_points_signature(&non_finite, 3).is_err());
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

        let mut missing_key = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        missing_key.entries[0].semantic_key = LabelResidualSemanticKey::C4Relation {
            relation_index: 999,
            role: C4RelationLabelRole::Message,
        };
        let missing_key_error = validate_label_residual_artifacts(&missing_key).unwrap_err();
        assert!(missing_key_error.contains("is absent from the signed upstream SVG"));

        let mut changed_upstream = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        changed_upstream.entries[0].upstream.world.anchor_x += 1.0;
        let signature_error = validate_label_residual_artifacts(&changed_upstream).unwrap_err();
        assert!(signature_error.contains("upstream signature does not match the signed SVG"));
    }

    #[test]
    fn state_empty_label_residuals_keep_stable_edge_identity() {
        let catalog = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        let mut empty_state_edges = catalog
            .entries
            .iter()
            .filter(|entry| entry.diagram == "state" && entry.text.is_empty())
            .map(|entry| match &entry.semantic_key {
                LabelResidualSemanticKey::StableEdge { edge_key } => edge_key.as_str(),
                LabelResidualSemanticKey::C4Relation { .. } => {
                    panic!("State residuals must use stable edge identities")
                }
            })
            .collect::<Vec<_>>();
        empty_state_edges.sort_unstable();

        assert_eq!(empty_state_edges, ["edge0", "edge4"]);
    }

    #[test]
    fn label_residual_catalog_requires_keys_that_match_the_registered_adapter() {
        let mut json = serde_json::from_str::<serde_json::Value>(LABEL_RESIDUAL_CATALOG).unwrap();
        {
            let entry = &mut json["entries"][0];
            entry["diagram"] = serde_json::Value::String("flowchart".to_string());
            entry["fixture"] =
                serde_json::Value::String(FLOWCHART_ELK_PARALLEL_LABEL_FIXTURE.to_string());
            entry["input_sha256"] = serde_json::Value::String(
                "05195f0247422c1af0299243082a2b0dc35a7293ddae62b0c57ddab0b0a6cec0".to_string(),
            );
            entry["upstream_svg_sha256"] = serde_json::Value::String(
                "2683169760f6d16a7d06df1e4b8fe14e69fec133c662c5196967ea79e0b0cc58".to_string(),
            );
        }

        let mismatch = parse_label_residual_catalog(&json.to_string(), 3).unwrap_err();
        assert!(mismatch.contains("is invalid for adapter FlowchartElk"));

        json["entries"][0]["semantic_key"] = serde_json::json!({
            "kind": "stable_edge",
            "edge_key": "L_a1_a2_0"
        });
        parse_label_residual_catalog(&json.to_string(), 3).unwrap();
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
        geometry.entries[0].local.world.anchor_x = f64::INFINITY;
        assert!(validate_label_residual_contract(&geometry, 3).is_err());

        let mut identical = parse_label_residual_catalog(LABEL_RESIDUAL_CATALOG, 3).unwrap();
        identical.entries[0].local = identical.entries[0].upstream.clone();
        assert!(
            validate_label_residual_contract(&identical, 3)
                .unwrap_err()
                .contains("identical upstream and local geometry")
        );

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
        assert!(!valid.entries.is_empty());
    }
}
