use std::collections::{BTreeMap, BTreeSet};

use crate::cutover_manifest::authorize_cutover_routes;
use crate::observation::{RouteCutoverRuntimeError, append_len_prefixed, sha256};
use crate::runner::{
    C6ProofError, C6ProofResult, C6RasterImage, FamilyEvidenceRequirements, class_contains,
    decode_bounded_png_artifact, portable_svg_request, prove_portable_family_evidence, style_value,
};
use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec,
    FontCatalogSpec, FontEmbeddingRequirement, FontSource, GenericFontFamily, ThemeAssets,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman::{
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderArtifactKind, RenderOutput,
    RenderRequest, Renderer, TargetAdmissionReason, TargetAdmissionReceipt, TargetAdmissionStatus,
    TargetFontSource,
};
use merman_export::{
    ExportFontPlan, RasterExportReport, RasterOptions, RasterOutputKind, RasterPlan,
};
use merman_render::__private::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverProjection,
    ThemeRouteCutoverProjectionSet, ThemeRouteCutoverSelector, ThemeRouteCutoverValue,
    legacy_replacing_typed_theme_routes,
};

mod png_proof;
mod svg_proof;

use png_proof::prove_terminal_png_pair;
#[cfg(test)]
use png_proof::require_marker_pixel_counts;
#[cfg(test)]
use svg_proof::transformed_path_terminals;
use svg_proof::{prove_flowchart_markers_svg, prove_svg_routes};

#[derive(Clone, Copy)]
struct ControlColor {
    css: &'static str,
    rgb: [u8; 3],
}

const SOLID_FILL: ControlColor = ControlColor {
    css: "#dc2626",
    rgb: [0xdc, 0x26, 0x26],
};
const SOLID_STROKE: ControlColor = ControlColor {
    css: "#2563eb",
    rgb: [0x25, 0x63, 0xeb],
};
const SOLID_EDGE: ControlColor = ControlColor {
    css: "#16a34a",
    rgb: [0x16, 0xa3, 0x4a],
};
const DEFAULT_MARKER: ControlColor = ControlColor {
    css: "#333333",
    rgb: [0x33, 0x33, 0x33],
};
const COLOR_TOLERANCE: u8 = 6;
const MAX_TRANSPARENT_CONTROL_PIXELS: usize = 2;
const MAX_NON_TRANSPARENT_MASK_PERCENT: usize = 5;
const MIN_NON_TRANSPARENT_MASK_ALLOWANCE: usize = 8;
const MIN_UNDERLAY_MASK_PERCENT: usize = 80;
const TRANSPARENT_ALPHA_TOLERANCE: u8 = 2;
const MARKER_PROBE_RADIUS: f64 = 9.0;
const MINIMUM_MARKER_PIXELS: usize = 6;

const FLOWCHART_NODE_SOURCE: &str = "flowchart LR\nA[Alpha]\nB[Beta]\n";
const FLOWCHART_EDGE_SOURCE: &str =
    "flowchart LR\nA[Alpha] o--o B[Beta]\nB x--x C[Gamma]\nC <--> D[Delta]\n";
const FLOWCHART_ANIMATED_EDGE_SOURCE: &str = "flowchart LR\nA[Alpha] circles@o--o B[Beta]\nB crosses@x--x C[Gamma]\nC points@<--> D[Delta]\ncircles@{ animate: true }\ncrosses@{ animate: true }\npoints@{ animate: true }\n";
const SWIMLANE_NODE_SOURCE: &str = r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha]
B[Beta]
"#;
const SWIMLANE_EDGE_SOURCE: &str = r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha] o--o B[Beta]
B x--x C[Gamma]
C <--> D[Delta]
"#;
const SWIMLANE_ANIMATED_EDGE_SOURCE: &str = r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha] circles@o--o B[Beta]
B crosses@x--x C[Gamma]
C points@<--> D[Delta]
circles@{ animate: true }
crosses@{ animate: true }
points@{ animate: true }
"#;
const SEQUENCE_FILL_SOURCE: &str = r#"sequenceDiagram
participant Plain
participant Stick@{"type":"actor"}
participant Boundary@{"type":"boundary"}
participant Entity@{"type":"entity"}
participant Collection@{"type":"collections"}
participant Queue@{"type":"queue"}
participant Database@{"type":"database"}
Plain->>Database: Hello
"#;
const SEQUENCE_STROKE_SOURCE: &str = r#"sequenceDiagram
participant Plain
participant Stick@{"type":"actor"}
participant Boundary@{"type":"boundary"}
participant Entity@{"type":"entity"}
participant Collection@{"type":"collections"}
participant Queue@{"type":"queue"}
Plain->>Queue: Hello
"#;
const SEQUENCE_NOTE_SOURCE: &str = r#"sequenceDiagram
participant Alice
participant Bob
Note left of Alice: Left note
Note over Alice,Bob: Shared note
Note right of Bob: Right note
Alice->>Bob: Hello
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum CutoverWitnessProfile {
    ClassicStatic,
    NeoStatic,
    NeoAnimated,
}

impl CutoverWitnessProfile {
    const CLASSIC: [Self; 1] = [Self::ClassicStatic];
    const EDGE: [Self; 3] = [Self::ClassicStatic, Self::NeoStatic, Self::NeoAnimated];

    const fn id(self) -> &'static str {
        match self {
            Self::ClassicStatic => "classic-static",
            Self::NeoStatic => "neo-static",
            Self::NeoAnimated => "neo-animated",
        }
    }

    const fn is_neo(self) -> bool {
        matches!(self, Self::NeoStatic | Self::NeoAnimated)
    }

    const fn is_animated(self) -> bool {
        matches!(self, Self::NeoAnimated)
    }

    const fn expects_marker_margin(self) -> bool {
        matches!(self, Self::NeoStatic)
    }

    const fn expects_neo_mask(self) -> bool {
        matches!(self, Self::NeoStatic)
    }

    fn for_route(route: ThemeRouteCutoverDescriptor) -> &'static [Self] {
        if route.target() == ThemeTarget::Edge {
            &Self::EDGE
        } else {
            &Self::CLASSIC
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CutoverWitnessId {
    route: ThemeRouteCutoverDescriptor,
    profile: CutoverWitnessProfile,
}

impl CutoverWitnessId {
    const fn new(route: ThemeRouteCutoverDescriptor, profile: CutoverWitnessProfile) -> Self {
        Self { route, profile }
    }

    const fn route(self) -> ThemeRouteCutoverDescriptor {
        self.route
    }

    const fn profile(self) -> CutoverWitnessProfile {
        self.profile
    }
}

fn expected_cutover_witnesses(routes: &[ThemeRouteCutoverDescriptor]) -> Vec<CutoverWitnessId> {
    let mut witnesses = routes
        .iter()
        .copied()
        .flat_map(|route| {
            CutoverWitnessProfile::for_route(route)
                .iter()
                .copied()
                .map(move |profile| CutoverWitnessId::new(route, profile))
        })
        .collect::<Vec<_>>();
    witnesses.sort_unstable();
    witnesses
}

fn source_for_route(route: ThemeRouteCutoverDescriptor) -> C6ProofResult<&'static str> {
    match (route.family_id(), route.target(), route.facet()) {
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node, _) => Ok(FLOWCHART_NODE_SOURCE),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
            Ok(FLOWCHART_EDGE_SOURCE)
        }
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Node, _) => Ok(SWIMLANE_NODE_SOURCE),
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
            Ok(SWIMLANE_EDGE_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Fill) => {
            Ok(SEQUENCE_FILL_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Stroke) => {
            Ok(SEQUENCE_STROKE_SOURCE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, _) => Ok(SEQUENCE_NOTE_SOURCE),
        _ => Err(C6ProofError::new(
            "route-source",
            format!("no route-cutover witness source for {}", route_label(route)),
        )),
    }
}

fn source_for_witness(witness: CutoverWitnessId) -> C6ProofResult<&'static str> {
    if witness.profile() != CutoverWitnessProfile::NeoAnimated {
        return source_for_route(witness.route());
    }
    match (
        witness.route().family_id(),
        witness.route().target(),
        witness.route().facet(),
    ) {
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
            Ok(FLOWCHART_ANIMATED_EDGE_SOURCE)
        }
        (DiagramFamilyId::SWIMLANE, ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => {
            Ok(SWIMLANE_ANIMATED_EDGE_SOURCE)
        }
        _ => Err(C6ProofError::new(
            "route-source",
            format!(
                "no animated route-cutover witness source for {}",
                witness_label(witness)
            ),
        )),
    }
}

#[derive(Clone, Copy)]
struct CutoverCase {
    id: CutoverWitnessId,
    source: &'static str,
}

struct RenderedCutoverCase {
    routes: Vec<ThemeRouteCutoverDescriptor>,
    source_digest: [u8; 32],
    recipe_digest: [u8; 32],
    operation_digest: [u8; 32],
    admission_digest: [u8; 32],
    svg_target_receipt_digest: [u8; 32],
    png_target_receipt_digest: [u8; 32],
    document_digest: [u8; 32],
    resource_fingerprint: [u8; 32],
    font_catalog_fingerprint: [u8; 32],
    svg_artifact_digest: [u8; 32],
    png_artifact_digest: [u8; 32],
    svg_assertions: BTreeMap<ThemeRouteCutoverDescriptor, [u8; 32]>,
    svg_view_box: [f64; 4],
    target_regions: Vec<[f64; 4]>,
    target_underlay_colors: Vec<Vec<[u8; 3]>>,
    markers: Vec<FlowchartMarkerObservation>,
    raster: C6RasterImage,
}

#[derive(Clone)]
struct FlowchartMarkerObservation {
    digest: [u8; 32],
    geometry_digest: [u8; 32],
    view_box: [f64; 4],
    probe_rect: [f64; 4],
    fill: String,
    stroke: String,
}

struct SequenceRouteObservation {
    value: String,
    terminal_digest: [u8; 32],
    target_regions: Vec<[f64; 4]>,
}

struct SvgCutoverProof {
    assertions: BTreeMap<ThemeRouteCutoverDescriptor, [u8; 32]>,
    view_box: [f64; 4],
    target_regions: Vec<[f64; 4]>,
    target_underlay_colors: Vec<Vec<[u8; 3]>>,
}

/// Coarse evidence that the exact manifest-declared bridge routes passed their cutover gate.
///
/// The sealed route receipts and aggregate route-report digest remain private to this crate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteCutoverAuthorizationReport {
    manifest_digest: [u8; 32],
    authorization_digest: [u8; 32],
}

impl RouteCutoverAuthorizationReport {
    /// Returns the digest of the exact route-cutover manifest authorized by this run.
    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    /// Returns the sealed authorization digest for this run.
    pub const fn authorization_digest(&self) -> &[u8; 32] {
        &self.authorization_digest
    }
}

#[derive(Debug)]
struct RouteCutoverAuthorizationReceipt {
    manifest_digest: [u8; 32],
    route_report_digest: [u8; 32],
    digest: [u8; 32],
}

impl RouteCutoverAuthorizationReceipt {
    fn seal(manifest_digest: [u8; 32], route_report_digest: [u8; 32]) -> C6ProofResult<Self> {
        c6_ensure!(
            "route-authorization",
            manifest_digest != [0; 32],
            "cutover manifest digest is zero"
        );
        c6_ensure!(
            "route-authorization",
            route_report_digest != [0; 32],
            "aggregate route report digest is zero"
        );
        let digest = Self::canonical_digest(manifest_digest, route_report_digest);
        c6_ensure!(
            "route-authorization",
            digest != [0; 32],
            "route authorization digest is zero"
        );
        Ok(Self {
            manifest_digest,
            route_report_digest,
            digest,
        })
    }

    fn canonical_digest(manifest_digest: [u8; 32], route_report_digest: [u8; 32]) -> [u8; 32] {
        let mut value = b"merman.c6-route-cutover-authorization.v1\0".to_vec();
        append_len_prefixed(&mut value, b"manifest");
        value.extend_from_slice(&manifest_digest);
        append_len_prefixed(&mut value, b"aggregate-route-report");
        value.extend_from_slice(&route_report_digest);
        sha256(value)
    }

    #[cfg(test)]
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    fn into_report(self) -> RouteCutoverAuthorizationReport {
        debug_assert_eq!(
            self.digest,
            Self::canonical_digest(self.manifest_digest, self.route_report_digest)
        );
        RouteCutoverAuthorizationReport {
            manifest_digest: self.manifest_digest,
            authorization_digest: self.digest,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(manifest_digest: [u8; 32], route_report_digest: [u8; 32]) -> Self {
        Self::seal(manifest_digest, route_report_digest).expect("seal test route authorization")
    }
}

struct C6RouteCutoverReceipt {
    digest: [u8; 32],
}

impl C6RouteCutoverReceipt {
    fn seal(
        witness: CutoverWitnessId,
        rendered: &RenderedCutoverCase,
        png_assertion_digest: [u8; 32],
    ) -> C6ProofResult<Self> {
        let route = witness.route();
        let svg_assertion_digest = rendered.svg_assertions.get(&route).ok_or_else(|| {
            C6ProofError::new(
                "route-svg-receipt",
                format!("missing SVG assertion for {}", witness_label(witness)),
            )
        })?;
        c6_ensure!(
            "route-admission-receipt",
            rendered.admission_digest != [0; 32],
            "route admission digest is zero for {}",
            witness_label(witness)
        );
        c6_ensure!(
            "route-admission-receipt",
            rendered.svg_target_receipt_digest != [0; 32]
                && rendered.png_target_receipt_digest != [0; 32],
            "target-owned route admission digest is zero for {}",
            witness_label(witness)
        );
        let mut value = b"merman.c6-route-cutover-receipt.v4\0".to_vec();
        append_witness(&mut value, witness);
        value.extend_from_slice(&rendered.source_digest);
        value.extend_from_slice(&rendered.recipe_digest);
        value.extend_from_slice(&rendered.operation_digest);
        value.extend_from_slice(&rendered.admission_digest);
        value.extend_from_slice(&rendered.svg_target_receipt_digest);
        value.extend_from_slice(&rendered.png_target_receipt_digest);
        value.extend_from_slice(&rendered.document_digest);
        value.extend_from_slice(&rendered.resource_fingerprint);
        value.extend_from_slice(&rendered.font_catalog_fingerprint);
        value.extend_from_slice(&rendered.svg_artifact_digest);
        value.extend_from_slice(&rendered.png_artifact_digest);
        value.extend_from_slice(svg_assertion_digest);
        value.extend_from_slice(&png_assertion_digest);
        Ok(Self {
            digest: sha256(value),
        })
    }
}

pub(crate) fn run_route_cutover_witnesses()
-> Result<RouteCutoverAuthorizationReport, RouteCutoverRuntimeError> {
    let inventory = legacy_replacing_typed_theme_routes().map_err(|error| {
        C6ProofError::new("route-inventory", error.to_string()).into_route_runtime("inventory")
    })?;
    let authorized_manifest = prove_route("manifest", authorize_cutover_routes(inventory))?;
    if authorized_manifest.routes().is_empty() {
        return Err(C6ProofError::new(
            "route-inventory",
            "typed legacy-replacing route inventory is empty",
        )
        .into_route_runtime("inventory"));
    }
    let manifest_digest = authorized_manifest_digest(
        authorized_manifest.manifest_version(),
        authorized_manifest.routes(),
    );
    let authorized = authorized_manifest.into_routes();

    let mut rendered = BTreeMap::new();
    for witness_id in expected_cutover_witnesses(&authorized) {
        let route = witness_id.route();
        let case = CutoverCase {
            id: witness_id,
            source: source_for_witness(witness_id)
                .map_err(|error| error.into_route_runtime(witness_label(witness_id)))?,
        };
        let witness = case_label(case);
        let completed = prove_route(&witness, render_cutover_case(case, vec![route]))?;
        if rendered.insert(witness_id, completed).is_some() {
            return Err(RouteCutoverRuntimeError::DuplicateReceipt { route: witness });
        }
    }

    let mut receipts = BTreeMap::new();
    let shapes = rendered
        .keys()
        .copied()
        .map(cutover_witness_shape)
        .collect::<BTreeSet<_>>();
    for shape in shapes {
        let pair_label = cutover_witness_shape_label(shape);
        let solid_witness = rendered
            .keys()
            .copied()
            .find(|witness| {
                cutover_witness_shape(*witness) == shape
                    && witness.route().value() == ThemeRouteCutoverValue::Solid
            })
            .ok_or_else(|| RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![format!("{pair_label}/solid")],
                unexpected: Vec::new(),
            })?;
        let transparent_witness = rendered
            .keys()
            .copied()
            .find(|witness| {
                cutover_witness_shape(*witness) == shape
                    && witness.route().value() == ThemeRouteCutoverValue::Transparent
            })
            .ok_or_else(|| RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![format!("{pair_label}/transparent")],
                unexpected: Vec::new(),
            })?;
        let solid = rendered.get(&solid_witness).ok_or_else(|| {
            RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![witness_label(solid_witness)],
                unexpected: Vec::new(),
            }
        })?;
        let transparent = rendered.get(&transparent_witness).ok_or_else(|| {
            RouteCutoverRuntimeError::CoverageMismatch {
                missing: vec![witness_label(transparent_witness)],
                unexpected: Vec::new(),
            }
        })?;
        let png_assertions = prove_route(&pair_label, prove_terminal_png_pair(solid, transparent))?;
        for (witness_id, rendered_case) in
            [(solid_witness, solid), (transparent_witness, transparent)]
        {
            let route = witness_id.route();
            let png_assertion = png_assertions.get(&route).copied().ok_or_else(|| {
                RouteCutoverRuntimeError::CoverageMismatch {
                    missing: vec![witness_label(witness_id)],
                    unexpected: Vec::new(),
                }
            })?;
            let receipt = prove_route(
                &witness_label(witness_id),
                C6RouteCutoverReceipt::seal(witness_id, rendered_case, png_assertion),
            )?;
            if receipts.insert(witness_id, receipt).is_some() {
                return Err(RouteCutoverRuntimeError::DuplicateReceipt {
                    route: witness_label(witness_id),
                });
            }
        }
    }

    let route_report_digest = evaluate_route_receipts(authorized, receipts)?;
    prove_route(
        "authorization",
        RouteCutoverAuthorizationReceipt::seal(manifest_digest, route_report_digest),
    )
    .map(RouteCutoverAuthorizationReceipt::into_report)
}

fn render_cutover_case(
    case: CutoverCase,
    mut routes: Vec<ThemeRouteCutoverDescriptor>,
) -> C6ProofResult<RenderedCutoverCase> {
    let witness_id = case.id;
    let expected_route = witness_id.route();
    let profile = witness_id.profile();
    routes.sort_unstable();
    c6_ensure!(
        "route-inventory",
        !routes.is_empty(),
        "{} has no matching typed bridge-replacing routes",
        case_label(case)
    );
    c6_ensure!(
        "route-inventory",
        routes.iter().all(|route| {
            route.selector() == ThemeRouteCutoverSelector::StaticUnqualified
                && *route == expected_route
        }) && CutoverWitnessProfile::for_route(expected_route).contains(&profile),
        "{} contains a route outside its canonical selector/family/value",
        case_label(case)
    );

    let theme = compile_cutover_theme(case)?;
    let renderer = cutover_renderer(profile);
    let svg_request = portable_svg_request();
    let document_output = renderer
        .render(
            RenderRequest::document(case.source, OperationControl::new(), svg_request)
                .with_theme(theme.clone()),
        )
        .map_err(|error| C6ProofError::new("route-svg-render", error.to_string()))?;
    let RenderOutput::Document(Some(document)) = document_output else {
        return Err(C6ProofError::new(
            "route-svg-render",
            "route witness did not produce a completed document",
        ));
    };
    let render_evidence = document.evidence();
    let render_identity = prove_portable_family_evidence(
        render_evidence,
        &theme,
        expected_route.family_id(),
        FamilyEvidenceRequirements::ROUTE_CUTOVER,
    )?;
    let document_portability = document.portability();
    c6_ensure!(
        "route-document-portability",
        document_portability.is_evidence_valid()
            && !document_portability.is_host_dependent()
            && document_portability.reasons().is_empty(),
        "route document proof is not target-independent and verified: verified={} host_dependent={} reasons={:?}",
        document_portability.is_evidence_valid(),
        document_portability.is_host_dependent(),
        document_portability.reasons()
    );
    let family_evidence =
        merman::__theme_acceptance::theme_acceptance_evidence(render_evidence).family();
    c6_ensure!(
        "route-family-disposition",
        family_evidence.required_count() == 1
            && family_evidence.applied_count() == 1
            && family_evidence.not_applicable_count() == 0
            && family_evidence.residual_count() == 0,
        "route witness did not prove one exact Applied family mechanism: required={} applied={} not_applicable={} residual={}",
        family_evidence.required_count(),
        family_evidence.applied_count(),
        family_evidence.not_applicable_count(),
        family_evidence.residual_count()
    );
    let sealed_svg = document.sealed_svg();
    let svg = document.svg();
    let markers = match (expected_route.family_id(), expected_route.target()) {
        (DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE, ThemeTarget::Edge) => {
            prove_flowchart_markers_svg(svg, profile)?
        }
        _ => Vec::new(),
    };
    let svg_proof = prove_svg_routes(case, &routes, svg, &markers)?;
    let svg_resource = *sealed_svg.resource_fingerprint().as_bytes();
    c6_ensure!(
        "route-svg-resource",
        svg_resource != [0; 32],
        "finalized SVG retained a zero resource fingerprint"
    );

    let raster_options = RasterOptions::default().with_scale(2.0);
    let png_output = document
        .export_png(&raster_options, OperationControl::new())
        .map_err(|error| C6ProofError::new("route-png-encode", error.to_string()))?;
    let png_bytes = png_output.bytes();
    let report = png_output.export_report();
    let png_plan = report.raster();
    prove_cutover_png_report(report, png_plan, render_evidence)?;
    let raster = decode_bounded_png_artifact(png_bytes, png_plan)?;

    let png_resource = *report.resource_fingerprint().as_bytes();
    c6_ensure!(
        "route-resource-identity",
        svg_resource == png_resource,
        "SVG and PNG resource fingerprints differ"
    );

    let svg_receipt = document.standalone_svg_admission();
    let png_receipt = png_output.admission();
    c6_ensure!(
        "route-document-identity",
        svg_receipt.document_digest() == png_receipt.document_digest(),
        "SVG and PNG target receipts refer to different completed documents"
    );
    c6_ensure!(
        "route-resource-receipt-identity",
        svg_receipt.resource_fingerprint().as_bytes() == &svg_resource
            && png_receipt.resource_fingerprint().as_bytes() == &png_resource
            && svg_receipt.font_catalog_fingerprint() == png_receipt.font_catalog_fingerprint(),
        "SVG and PNG target receipts do not retain the same document resources"
    );
    c6_ensure!(
        "route-artifact-identity",
        svg_receipt.artifact_digest() == sha256(svg)
            && png_receipt.artifact_digest() == sha256(png_bytes),
        "target receipt artifact digests do not match the final bytes"
    );
    let svg_target_receipt_digest = cutover_target_receipt_digest(
        CutoverTargetAdmissionContract::PaintStandaloneSvgV1,
        svg_receipt,
    )?;
    let png_target_receipt_digest = cutover_target_receipt_digest(
        CutoverTargetAdmissionContract::PortableNativePngV1,
        png_receipt,
    )?;

    Ok(RenderedCutoverCase {
        routes,
        source_digest: sha256(case.source),
        recipe_digest: *theme.recipe_fingerprint().as_bytes(),
        operation_digest: *render_identity.operation_digest(),
        admission_digest: *render_identity.admission_digest(),
        svg_target_receipt_digest,
        png_target_receipt_digest,
        document_digest: svg_receipt.document_digest(),
        resource_fingerprint: svg_resource,
        font_catalog_fingerprint: *svg_receipt.font_catalog_fingerprint().as_bytes(),
        svg_artifact_digest: svg_receipt.artifact_digest(),
        png_artifact_digest: png_receipt.artifact_digest(),
        svg_assertions: svg_proof.assertions,
        svg_view_box: svg_proof.view_box,
        target_regions: svg_proof.target_regions,
        target_underlay_colors: svg_proof.target_underlay_colors,
        markers,
        raster,
    })
}

fn compile_cutover_theme(case: CutoverCase) -> C6ProofResult<DiagramTheme> {
    let mut styles = ThemeRuleSet::default();
    let route = case.id.route();
    let style = match route.facet() {
        ThemeRouteCutoverFacet::Fill => {
            ThemeStylePatch::default().with_fill(cutover_paint(route.value(), SOLID_FILL.css)?)
        }
        ThemeRouteCutoverFacet::Stroke => {
            let solid = match route.target() {
                ThemeTarget::Edge => SOLID_EDGE.css,
                ThemeTarget::Node | ThemeTarget::Actor | ThemeTarget::Note => SOLID_STROKE.css,
                target => {
                    return Err(C6ProofError::new(
                        "route-theme",
                        format!("unsupported cutover stroke target {}", target.id()),
                    ));
                }
            };
            ThemeStylePatch::default().with_stroke(cutover_paint(route.value(), solid)?)
        }
    };
    styles = styles.with_rule(cutover_rule(case, route.target(), style));

    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let font_catalog =
        FontCatalogSpec::new([FontAssetSpec::new("route-cutover-excalifont", font_bytes)])
            .with_alias("trebuchet ms", "Excalifont")
            .with_alias("verdana", "Excalifont")
            .with_alias("arial", "Excalifont")
            .with_generic_family(GenericFontFamily::SansSerif, "Excalifont")
            .with_available_sources([FontSource::Embedded])
            .with_embedding_requirement(FontEmbeddingRequirement::FullFont);

    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_assets(ThemeAssets::default().with_font_catalog(font_catalog))
                .with_canvas(CanvasSpec::transparent())
                .with_styles(styles),
        )
        .map_err(|error| C6ProofError::new("route-theme", error.to_string()))
}

fn cutover_rule(case: CutoverCase, target: ThemeTarget, style: ThemeStylePatch) -> ThemeRule {
    ThemeRule::new(target, style).for_family(case.id.route().family_id())
}

fn cutover_paint(value: ThemeRouteCutoverValue, solid: &str) -> C6ProofResult<CanvasPaint> {
    match value {
        ThemeRouteCutoverValue::Transparent => Ok(CanvasPaint::Transparent),
        ThemeRouteCutoverValue::Solid => CanvasPaint::solid(solid)
            .map_err(|error| C6ProofError::new("route-theme-paint", error.to_string())),
    }
}

fn cutover_renderer(profile: CutoverWitnessProfile) -> Renderer {
    Renderer::new().with_engine(Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({
            "htmlLabels": false,
            "look": if profile.is_neo() { "neo" } else { "classic" },
            "flowchart": { "htmlLabels": false }
        }),
    )))
}

fn prove_cutover_png_report(
    report: RasterExportReport,
    expected_plan: RasterPlan,
    evidence: &merman::RenderEvidence,
) -> C6ProofResult<()> {
    c6_ensure!(
        "route-png-report",
        report.output() == RasterOutputKind::Png && report.raster() == expected_plan,
        "PNG output kind or raster plan differs from its frozen export report"
    );
    c6_ensure!(
        "route-png-report",
        report.resource_fingerprint().as_bytes() != &[0; 32],
        "PNG export retained a zero resource fingerprint"
    );
    c6_ensure!(
        "route-png-report",
        report.native_filter_receipt()
            == merman::__theme_acceptance::native_filter_receipt(evidence),
        "PNG filter receipt differs from family evidence"
    );
    let conversion = report.conversion();
    c6_ensure!(
        "route-png-report",
        conversion.tree_nodes > 0
            && conversion.filtered_groups == 0
            && conversion.filter_primitives == 0,
        "unexpected PNG conversion plan: {conversion:?}"
    );
    let images = report.embedded_images();
    c6_ensure!(
        "route-png-report",
        images.data_resources == 0
            && images.raster_images == 0
            && images.total_data_bytes == 0
            && images.total_pixels == 0,
        "route witness unexpectedly retained embedded images: {images:?}"
    );
    c6_ensure!(
        "route-png-report",
        report.matte().is_none() && !report.matte_defaulted(),
        "PNG route witness unexpectedly applied an output matte"
    );
    prove_cutover_font_plan(report.fonts())?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CutoverTargetAdmissionContract {
    PaintStandaloneSvgV1,
    PortableNativePngV1,
}

impl CutoverTargetAdmissionContract {
    const fn id(self) -> &'static str {
        match self {
            Self::PaintStandaloneSvgV1 => "paint-standalone-svg-v1",
            Self::PortableNativePngV1 => "portable-native-png-v1",
        }
    }

    const fn artifact_kind(self) -> RenderArtifactKind {
        match self {
            Self::PaintStandaloneSvgV1 => RenderArtifactKind::Svg,
            Self::PortableNativePngV1 => RenderArtifactKind::Png,
        }
    }

    fn accepts(
        self,
        status: TargetAdmissionStatus,
        reasons: &[TargetAdmissionReason],
        font_source: TargetFontSource,
    ) -> bool {
        match self {
            Self::PaintStandaloneSvgV1 => {
                (status == TargetAdmissionStatus::Portable
                    && reasons.is_empty()
                    && font_source == TargetFontSource::Embedded)
                    || (status == TargetAdmissionStatus::HostDependent
                        && reasons == [TargetAdmissionReason::SvgFontsNotSelfContained]
                        && matches!(
                            font_source,
                            TargetFontSource::None | TargetFontSource::Embedded
                        ))
            }
            Self::PortableNativePngV1 => {
                status == TargetAdmissionStatus::Portable
                    && reasons.is_empty()
                    && font_source == TargetFontSource::Embedded
            }
        }
    }
}

fn cutover_target_receipt_digest(
    contract: CutoverTargetAdmissionContract,
    receipt: &TargetAdmissionReceipt,
) -> C6ProofResult<[u8; 32]> {
    let expected_kind = contract.artifact_kind();
    c6_ensure!(
        "route-target-admission",
        receipt.artifact_kind() == expected_kind,
        "expected {} target receipt, got {}",
        expected_kind.id(),
        receipt.artifact_kind().id()
    );
    c6_ensure!(
        "route-target-admission",
        contract.accepts(receipt.status(), receipt.reasons(), receipt.font_source(),),
        "{} target does not satisfy {}: status={} reasons={:?} font_source={}",
        expected_kind.id(),
        contract.id(),
        receipt.status().id(),
        receipt.reasons(),
        receipt.font_source().id()
    );
    c6_ensure!(
        "route-target-admission",
        receipt.resource_fingerprint().as_bytes() != &[0; 32]
            && receipt.font_catalog_fingerprint().as_bytes() != &[0; 32]
            && receipt.document_digest() != [0; 32]
            && receipt.target_evidence_digest() != [0; 32]
            && receipt.artifact_digest() != [0; 32],
        "{} target receipt retained a zero identity component",
        expected_kind.id()
    );

    let mut value = b"merman.c6-route-target-admission.v2\0".to_vec();
    append_len_prefixed(&mut value, contract.id().as_bytes());
    append_len_prefixed(&mut value, receipt.artifact_kind().id().as_bytes());
    append_len_prefixed(&mut value, receipt.status().id().as_bytes());
    append_len_prefixed(&mut value, b"admission-reasons");
    value.extend_from_slice(&usize_to_u64(receipt.reasons().len()).to_be_bytes());
    for reason in receipt.reasons() {
        append_len_prefixed(&mut value, reason.id().as_bytes());
    }
    append_len_prefixed(&mut value, receipt.font_source().id().as_bytes());
    value.extend_from_slice(receipt.resource_fingerprint().as_bytes());
    value.extend_from_slice(receipt.font_catalog_fingerprint().as_bytes());
    value.extend_from_slice(&receipt.document_digest());
    value.extend_from_slice(&receipt.target_evidence_digest());
    value.extend_from_slice(&receipt.artifact_digest());
    Ok(sha256(value))
}

fn prove_cutover_font_plan(fonts: ExportFontPlan) -> C6ProofResult<()> {
    c6_ensure!(
        "route-png-fonts",
        fonts.loaded_embedded_face_count() > 0 && fonts.used_embedded_fonts(),
        "PNG did not consume the retained embedded face"
    );
    c6_ensure!(
        "route-png-fonts",
        !fonts.used_system_fonts()
            && !fonts.family_fallback_used()
            && !fonts.glyph_fallback_used()
            && !fonts.unresolved_font_request()
            && !fonts.unresolved_glyph_fallback()
            && fonts.prepared_label_mismatch_count() == 0
            && fonts.prepared_label_host_dependent_count() == 0
            && fonts.prepared_label_terminal_incomplete_count() == 0
            && fonts.unclassified_face_count() == 0
            && fonts.notdef_glyph_count() == 0
            && fonts.prepared_label_verified_count() == fonts.prepared_label_expected_count()
            && !fonts.is_host_dependent(),
        "PNG font plan was not portable: {fonts:?}"
    );
    Ok(())
}

fn evaluate_route_receipts(
    inventory: Vec<ThemeRouteCutoverDescriptor>,
    receipts: BTreeMap<CutoverWitnessId, C6RouteCutoverReceipt>,
) -> Result<[u8; 32], RouteCutoverRuntimeError> {
    let expected = expected_cutover_witnesses(&inventory)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual = receipts.keys().copied().collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(RouteCutoverRuntimeError::CoverageMismatch {
            missing: expected
                .difference(&actual)
                .copied()
                .map(witness_label)
                .collect(),
            unexpected: actual
                .difference(&expected)
                .copied()
                .map(witness_label)
                .collect(),
        });
    }

    let mut value = b"merman.c6-route-cutover-report.v2\0".to_vec();
    value.extend_from_slice(&usize_to_u64(receipts.len()).to_be_bytes());
    for (&witness, receipt) in &receipts {
        append_witness(&mut value, witness);
        value.extend_from_slice(&receipt.digest);
    }
    Ok(sha256(value))
}

fn authorized_manifest_digest(
    manifest_version: u16,
    routes: &[ThemeRouteCutoverDescriptor],
) -> [u8; 32] {
    let mut value = b"merman.c6-route-cutover-manifest.v1\0".to_vec();
    value.extend_from_slice(&manifest_version.to_be_bytes());
    value.extend_from_slice(&usize_to_u64(routes.len()).to_be_bytes());
    for &route in routes {
        append_route(&mut value, route);
    }
    sha256(value)
}

type RouteShape = (
    DiagramFamilyId,
    ThemeTarget,
    ThemeRouteCutoverSelector,
    ThemeRouteCutoverFacet,
    ThemeRouteCutoverProjectionSet,
);

fn route_shape(route: ThemeRouteCutoverDescriptor) -> RouteShape {
    (
        route.family_id(),
        route.target(),
        route.selector(),
        route.facet(),
        route.projections(),
    )
}

fn route_shapes(routes: &[ThemeRouteCutoverDescriptor]) -> BTreeSet<RouteShape> {
    routes.iter().copied().map(route_shape).collect()
}

fn route_shape_label(shape: RouteShape) -> String {
    format!(
        "{}/{}/{}/{}/{}",
        shape.0.as_str(),
        shape.1.id(),
        selector_id(shape.2),
        facet_id(shape.3),
        projection_set_label(shape.4)
    )
}

type CutoverWitnessShape = (RouteShape, CutoverWitnessProfile);

fn cutover_witness_shape(witness: CutoverWitnessId) -> CutoverWitnessShape {
    (route_shape(witness.route()), witness.profile())
}

fn cutover_witness_shape_label(shape: CutoverWitnessShape) -> String {
    format!("{}/{}", route_shape_label(shape.0), shape.1.id())
}

fn route_control_color(route: ThemeRouteCutoverDescriptor) -> C6ProofResult<ControlColor> {
    match (route.target(), route.facet()) {
        (
            ThemeTarget::Node | ThemeTarget::Actor | ThemeTarget::Note,
            ThemeRouteCutoverFacet::Fill,
        ) => Ok(SOLID_FILL),
        (
            ThemeTarget::Node | ThemeTarget::Actor | ThemeTarget::Note,
            ThemeRouteCutoverFacet::Stroke,
        ) => Ok(SOLID_STROKE),
        (ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => Ok(SOLID_EDGE),
        _ => Err(C6ProofError::new(
            "route-png-proof",
            format!("no PNG control color for {}", route_label(route)),
        )),
    }
}

fn minimum_control_pixels_per_region(route: ThemeRouteCutoverDescriptor) -> usize {
    match route.facet() {
        ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke => 8,
    }
}

fn expected_svg_value(route: ThemeRouteCutoverDescriptor) -> C6ProofResult<&'static str> {
    match route.value() {
        ThemeRouteCutoverValue::Transparent => match route.family_id() {
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE => Ok("none"),
            DiagramFamilyId::SEQUENCE => Ok("transparent"),
            family => Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported transparent cutover family {family}"),
            )),
        },
        ThemeRouteCutoverValue::Solid => match (route.target(), route.facet()) {
            (
                ThemeTarget::Node | ThemeTarget::Actor | ThemeTarget::Note,
                ThemeRouteCutoverFacet::Fill,
            ) => Ok(SOLID_FILL.css),
            (
                ThemeTarget::Node | ThemeTarget::Actor | ThemeTarget::Note,
                ThemeRouteCutoverFacet::Stroke,
            ) => Ok(SOLID_STROKE.css),
            (ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => Ok(SOLID_EDGE.css),
            _ => Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported solid cutover route {}", route_label(route)),
            )),
        },
    }
}

fn facet_property(facet: ThemeRouteCutoverFacet) -> &'static str {
    match facet {
        ThemeRouteCutoverFacet::Fill => "fill",
        ThemeRouteCutoverFacet::Stroke => "stroke",
    }
}

fn css_last_property<'a>(css: &'a str, selector: &str, property: &str) -> Option<&'a str> {
    let marker = format!("{selector}{{");
    let mut cursor = 0;
    let mut winner = None;
    while let Some(relative_start) = css.get(cursor..)?.find(&marker) {
        let body_start = cursor
            .checked_add(relative_start)?
            .checked_add(marker.len())?;
        let relative_end = css.get(body_start..)?.find('}')?;
        let body_end = body_start.checked_add(relative_end)?;
        winner = css
            .get(body_start..body_end)?
            .split(';')
            .fold(winner, |winner, declaration| {
                let Some((name, value)) = declaration.split_once(':') else {
                    return winner;
                };
                if name.trim() == property {
                    Some(value.trim())
                } else {
                    winner
                }
            });
        cursor = body_end.checked_add(1)?;
    }
    winner
}

fn route_label(route: ThemeRouteCutoverDescriptor) -> String {
    format!(
        "{}/{}/{}/{}/{}/{}",
        route.family_id().as_str(),
        route.target().id(),
        selector_id(route.selector()),
        facet_id(route.facet()),
        value_id(route.value()),
        projection_set_label(route.projections())
    )
}

fn witness_label(witness: CutoverWitnessId) -> String {
    format!(
        "{}/{}",
        route_label(witness.route()),
        witness.profile().id()
    )
}

fn case_label(case: CutoverCase) -> String {
    witness_label(case.id)
}

fn selector_id(selector: ThemeRouteCutoverSelector) -> &'static str {
    match selector {
        ThemeRouteCutoverSelector::StaticUnqualified => "static-unqualified",
    }
}

fn projection_set_label(projections: ThemeRouteCutoverProjectionSet) -> String {
    projections
        .iter()
        .map(ThemeRouteCutoverProjection::contribution_id)
        .collect::<Vec<_>>()
        .join("+")
}

fn facet_id(facet: ThemeRouteCutoverFacet) -> &'static str {
    match facet {
        ThemeRouteCutoverFacet::Fill => "fill",
        ThemeRouteCutoverFacet::Stroke => "stroke",
    }
}

fn value_id(value: ThemeRouteCutoverValue) -> &'static str {
    match value {
        ThemeRouteCutoverValue::Transparent => "transparent",
        ThemeRouteCutoverValue::Solid => "solid",
    }
}

fn append_route(output: &mut Vec<u8>, route: ThemeRouteCutoverDescriptor) {
    append_len_prefixed(output, route.family_id().as_str().as_bytes());
    append_len_prefixed(output, route.target().id().as_bytes());
    append_len_prefixed(output, selector_id(route.selector()).as_bytes());
    append_len_prefixed(output, facet_id(route.facet()).as_bytes());
    append_len_prefixed(output, value_id(route.value()).as_bytes());
    output.extend_from_slice(&usize_to_u64(route.projections().len()).to_be_bytes());
    for projection in route.projections().iter() {
        append_len_prefixed(output, projection.contribution_id().as_bytes());
        append_len_prefixed(output, projection.action().id().as_bytes());
    }
}

fn append_witness(output: &mut Vec<u8>, witness: CutoverWitnessId) {
    append_route(output, witness.route());
    append_len_prefixed(output, witness.profile().id().as_bytes());
}

fn append_rect(output: &mut Vec<u8>, rect: [f64; 4]) {
    for value in rect {
        output.extend_from_slice(&value.to_bits().to_be_bytes());
    }
}

pub(crate) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn prove_route<T>(witness: &str, result: C6ProofResult<T>) -> Result<T, RouteCutoverRuntimeError> {
    result.map_err(|error| error.into_route_runtime(witness))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use merman::svg::ThemeTarget;
    use merman::{DiagramFamilyId, TargetAdmissionReason, TargetAdmissionStatus, TargetFontSource};

    use super::{
        C6RouteCutoverReceipt, CutoverTargetAdmissionContract, CutoverWitnessId,
        CutoverWitnessProfile, RouteCutoverAuthorizationReceipt, evaluate_route_receipts,
        expected_cutover_witnesses, legacy_replacing_typed_theme_routes,
        require_marker_pixel_counts, run_route_cutover_witnesses, transformed_path_terminals,
    };

    #[test]
    fn every_legacy_replacing_typed_route_has_terminal_svg_and_png_proof() {
        let authorization = run_route_cutover_witnesses().expect("prove typed bridge cutovers");

        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        assert!(!inventory.is_empty());
        assert_ne!(authorization.manifest_digest(), &[0; 32]);
        assert_ne!(authorization.authorization_digest(), &[0; 32]);
    }

    #[test]
    fn route_authorization_binds_manifest_and_aggregate_report_digests() {
        let baseline = RouteCutoverAuthorizationReceipt::for_test([1; 32], [2; 32]);
        let changed_manifest = RouteCutoverAuthorizationReceipt::for_test([3; 32], [2; 32]);
        let changed_report = RouteCutoverAuthorizationReceipt::for_test([1; 32], [4; 32]);

        assert_ne!(baseline.digest(), changed_manifest.digest());
        assert_ne!(baseline.digest(), changed_report.digest());
    }

    #[test]
    fn paint_route_svg_admission_allows_only_the_exact_font_seal_residual() {
        let contract = CutoverTargetAdmissionContract::PaintStandaloneSvgV1;

        assert!(contract.accepts(
            TargetAdmissionStatus::Portable,
            &[],
            TargetFontSource::Embedded,
        ));
        assert!(contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::Embedded,
        ));
        assert!(contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::None,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::HostDependentTextLayout],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[
                TargetAdmissionReason::SvgFontsNotSelfContained,
                TargetAdmissionReason::HostDependentTextLayout,
            ],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::Rejected,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::System,
        ));
    }

    #[test]
    fn native_png_route_admission_remains_strictly_portable() {
        let contract = CutoverTargetAdmissionContract::PortableNativePngV1;

        assert!(contract.accepts(
            TargetAdmissionStatus::Portable,
            &[],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::HostDependent,
            &[TargetAdmissionReason::SvgFontsNotSelfContained],
            TargetFontSource::Embedded,
        ));
        assert!(!contract.accepts(
            TargetAdmissionStatus::Portable,
            &[],
            TargetFontSource::Mixed,
        ));
    }

    #[test]
    fn edge_route_receipts_require_classic_and_both_neo_profiles() {
        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        let edge_route = inventory
            .iter()
            .copied()
            .find(|route| {
                route.family_id() == DiagramFamilyId::FLOWCHART
                    && route.target() == ThemeTarget::Edge
            })
            .expect("Flowchart Edge route");
        let complete_receipts = || {
            expected_cutover_witnesses(&inventory)
                .into_iter()
                .map(|witness| (witness, C6RouteCutoverReceipt { digest: [0x5a; 32] }))
                .collect::<BTreeMap<_, _>>()
        };

        assert!(evaluate_route_receipts(inventory.clone(), complete_receipts()).is_ok());
        for profile in [
            CutoverWitnessProfile::ClassicStatic,
            CutoverWitnessProfile::NeoStatic,
            CutoverWitnessProfile::NeoAnimated,
        ] {
            let mut incomplete = complete_receipts();
            incomplete.remove(&CutoverWitnessId::new(edge_route, profile));
            assert!(
                evaluate_route_receipts(inventory.clone(), incomplete).is_err(),
                "missing {profile:?} Edge witness must reject cutover authorization"
            );
        }
    }

    #[test]
    fn marker_png_receipt_rejects_removed_or_transparent_markers() {
        assert!(require_marker_pixel_counts(6, 6).is_ok());
        assert!(require_marker_pixel_counts(0, 6).is_err());
        assert!(require_marker_pixel_counts(6, 0).is_err());
    }

    #[test]
    fn marker_path_terminals_follow_relative_segments_and_nested_transforms() {
        let document = roxmltree::Document::parse(
            r#"<svg><g transform="translate(10 20)"><g transform="scale(2)"><path id="edge" d="m 1 2 h 3 v 4 c 1 1 2 2 3 3"/></g></g></svg>"#,
        )
        .expect("parse SVG");
        let path = document
            .descendants()
            .find(|node| node.attribute("id") == Some("edge"))
            .expect("find path");

        assert_eq!(
            transformed_path_terminals(path, path.attribute("d").expect("path data"))
                .expect("resolve path terminals"),
            ((12.0, 24.0), (24.0, 38.0))
        );
    }

    #[test]
    fn marker_path_close_returns_to_the_transformed_subpath_start() {
        let document = roxmltree::Document::parse(
            r#"<svg><g transform="translate(10 20)"><path id="edge" d="M 1 2 h 3 v 4 z"/></g></svg>"#,
        )
        .expect("parse SVG");
        let path = document
            .descendants()
            .find(|node| node.attribute("id") == Some("edge"))
            .expect("find path");

        assert_eq!(
            transformed_path_terminals(path, path.attribute("d").expect("path data"))
                .expect("resolve path terminals"),
            ((11.0, 22.0), (11.0, 22.0))
        );
    }
}
