use std::collections::{BTreeMap, BTreeSet};

use crate::cutover_manifest::authorize_cutover_routes;
use crate::observation::{C6RuntimeError, append_len_prefixed, sha256};
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
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest, Renderer,
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
        _ => Err(C6ProofError::new(
            "route-source",
            format!("no route-cutover witness source for {}", route_label(route)),
        )),
    }
}

#[derive(Clone, Copy)]
struct CutoverCase {
    route: ThemeRouteCutoverDescriptor,
    source: &'static str,
}

struct RenderedCutoverCase {
    routes: Vec<ThemeRouteCutoverDescriptor>,
    source_digest: [u8; 32],
    recipe_digest: [u8; 32],
    operation_digest: [u8; 32],
    document_digest: [u8; 32],
    resource_fingerprint: [u8; 32],
    svg_artifact_digest: [u8; 32],
    png_artifact_digest: [u8; 32],
    svg_assertions: BTreeMap<ThemeRouteCutoverDescriptor, [u8; 32]>,
    svg_view_box: [f64; 4],
    target_regions: Vec<[f64; 4]>,
    target_underlay_colors: Vec<Option<[u8; 3]>>,
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
    target_underlay_colors: Vec<Option<[u8; 3]>>,
}

struct C6RouteCutoverReceipt {
    digest: [u8; 32],
}

impl C6RouteCutoverReceipt {
    fn seal(
        route: ThemeRouteCutoverDescriptor,
        rendered: &RenderedCutoverCase,
        png_assertion_digest: [u8; 32],
    ) -> C6ProofResult<Self> {
        let svg_assertion_digest = rendered.svg_assertions.get(&route).ok_or_else(|| {
            C6ProofError::new(
                "route-svg-receipt",
                format!("missing SVG assertion for {}", route_label(route)),
            )
        })?;
        let mut value = b"merman.c6-route-cutover-receipt.v1\0".to_vec();
        append_route(&mut value, route);
        value.extend_from_slice(&rendered.source_digest);
        value.extend_from_slice(&rendered.recipe_digest);
        value.extend_from_slice(&rendered.operation_digest);
        value.extend_from_slice(&rendered.document_digest);
        value.extend_from_slice(&rendered.resource_fingerprint);
        value.extend_from_slice(&rendered.svg_artifact_digest);
        value.extend_from_slice(&rendered.png_artifact_digest);
        value.extend_from_slice(svg_assertion_digest);
        value.extend_from_slice(&png_assertion_digest);
        Ok(Self {
            digest: sha256(value),
        })
    }
}

pub(crate) fn run_route_cutover_witnesses() -> Result<(), C6RuntimeError> {
    prove_route_cutover_witnesses().map(drop)
}

fn prove_route_cutover_witnesses() -> Result<[u8; 32], C6RuntimeError> {
    let inventory = legacy_replacing_typed_theme_routes().map_err(|error| {
        C6ProofError::new("route-inventory", error.to_string()).into_route_runtime("inventory")
    })?;
    let authorized = prove_route("manifest", authorize_cutover_routes(inventory))?;
    if authorized.is_empty() {
        return Err(C6ProofError::new(
            "route-inventory",
            "typed legacy-replacing route inventory is empty",
        )
        .into_route_runtime("inventory"));
    }

    let mut rendered = BTreeMap::new();
    for &route in &authorized {
        let case = CutoverCase {
            route,
            source: source_for_route(route)
                .map_err(|error| error.into_route_runtime(route_label(route)))?,
        };
        let witness = case_label(case);
        let completed = prove_route(&witness, render_cutover_case(case, vec![route]))?;
        if rendered.insert(route, completed).is_some() {
            return Err(C6RuntimeError::DuplicateRouteCutoverReceipt { route: witness });
        }
    }

    let mut receipts = BTreeMap::new();
    let shapes = rendered
        .keys()
        .copied()
        .map(route_shape)
        .collect::<BTreeSet<_>>();
    for shape in shapes {
        let pair_label = route_shape_label(shape);
        let solid_route = rendered
            .keys()
            .copied()
            .find(|route| {
                route_shape(*route) == shape && route.value() == ThemeRouteCutoverValue::Solid
            })
            .ok_or_else(|| C6RuntimeError::RouteCutoverCoverageMismatch {
                missing: vec![format!("{pair_label}/solid")],
                unexpected: Vec::new(),
            })?;
        let transparent_route = rendered
            .keys()
            .copied()
            .find(|route| {
                route_shape(*route) == shape && route.value() == ThemeRouteCutoverValue::Transparent
            })
            .ok_or_else(|| C6RuntimeError::RouteCutoverCoverageMismatch {
                missing: vec![format!("{pair_label}/transparent")],
                unexpected: Vec::new(),
            })?;
        let solid = rendered.get(&solid_route).ok_or_else(|| {
            C6RuntimeError::RouteCutoverCoverageMismatch {
                missing: vec![route_label(solid_route)],
                unexpected: Vec::new(),
            }
        })?;
        let transparent = rendered.get(&transparent_route).ok_or_else(|| {
            C6RuntimeError::RouteCutoverCoverageMismatch {
                missing: vec![route_label(transparent_route)],
                unexpected: Vec::new(),
            }
        })?;
        let png_assertions = prove_route(&pair_label, prove_terminal_png_pair(solid, transparent))?;
        for rendered_case in [solid, transparent] {
            for &route in &rendered_case.routes {
                let png_assertion = png_assertions.get(&route).copied().ok_or_else(|| {
                    C6RuntimeError::RouteCutoverCoverageMismatch {
                        missing: vec![route_label(route)],
                        unexpected: Vec::new(),
                    }
                })?;
                let receipt = prove_route(
                    &route_label(route),
                    C6RouteCutoverReceipt::seal(route, rendered_case, png_assertion),
                )?;
                if receipts.insert(route, receipt).is_some() {
                    return Err(C6RuntimeError::DuplicateRouteCutoverReceipt {
                        route: route_label(route),
                    });
                }
            }
        }
    }

    evaluate_route_receipts(authorized, receipts)
}

fn render_cutover_case(
    case: CutoverCase,
    mut routes: Vec<ThemeRouteCutoverDescriptor>,
) -> C6ProofResult<RenderedCutoverCase> {
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
            route.selector() == ThemeRouteCutoverSelector::StaticUnqualified && *route == case.route
        }),
        "{} contains a route outside its canonical selector/family/value",
        case_label(case)
    );

    let theme = compile_cutover_theme(case)?;
    let renderer = cutover_renderer();
    let svg_request = portable_svg_request();
    let svg_output = renderer
        .render(
            RenderRequest::finalized_svg(case.source, OperationControl::new(), svg_request.clone())
                .with_theme(theme.clone()),
        )
        .map_err(|error| C6ProofError::new("route-svg-render", error.to_string()))?;
    let RenderOutput::FinalizedSvg(Some(svg_output)) = svg_output else {
        return Err(C6ProofError::new(
            "route-svg-render",
            "route witness did not produce a finalized SVG",
        ));
    };
    let (sealed_svg, render_evidence) = svg_output.into_parts();
    let render_identity = prove_portable_family_evidence(
        &render_evidence,
        &theme,
        case.route.family_id(),
        FamilyEvidenceRequirements::ROUTE_CUTOVER,
    )?;
    let svg = sealed_svg.as_str();
    let markers = match (case.route.family_id(), case.route.target()) {
        (DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE, ThemeTarget::Edge) => {
            prove_flowchart_markers_svg(svg)?
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
    let prepared = merman_export::prepare_raster_controlled(
        &sealed_svg,
        &raster_options,
        OperationControl::new(),
    )
    .map_err(|error| C6ProofError::new("route-png-prepare", error.to_string()))?;
    let (png_bytes, report) = prepared
        .encode_png_with_report()
        .map_err(|error| C6ProofError::new("route-png-encode", error.to_string()))?;
    let png_plan = report.raster();
    prove_cutover_png_report(report, png_plan, &render_evidence)?;
    let raster = decode_bounded_png_artifact(&png_bytes, png_plan)?;

    let png_resource = *report.resource_fingerprint().as_bytes();
    c6_ensure!(
        "route-resource-identity",
        svg_resource == png_resource,
        "SVG and PNG resource fingerprints differ"
    );

    let svg_artifact_digest = sha256(svg);
    let mut document_identity = b"merman.c6-route-cutover-document.v1\0".to_vec();
    document_identity.extend_from_slice(theme.recipe_fingerprint().as_bytes());
    document_identity.extend_from_slice(render_identity.operation_digest());
    document_identity.extend_from_slice(&svg_resource);
    document_identity.extend_from_slice(&svg_artifact_digest);

    Ok(RenderedCutoverCase {
        routes,
        source_digest: sha256(case.source),
        recipe_digest: *theme.recipe_fingerprint().as_bytes(),
        operation_digest: *render_identity.operation_digest(),
        document_digest: sha256(document_identity),
        resource_fingerprint: svg_resource,
        svg_artifact_digest,
        png_artifact_digest: sha256(&png_bytes),
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
    let route = case.route;
    let style = match route.facet() {
        ThemeRouteCutoverFacet::Fill => {
            ThemeStylePatch::default().with_fill(cutover_paint(route.value(), SOLID_FILL.css)?)
        }
        ThemeRouteCutoverFacet::Stroke => {
            let solid = match route.target() {
                ThemeTarget::Edge => SOLID_EDGE.css,
                ThemeTarget::Node | ThemeTarget::Actor => SOLID_STROKE.css,
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
    ThemeRule::new(target, style).for_family(case.route.family_id())
}

fn cutover_paint(value: ThemeRouteCutoverValue, solid: &str) -> C6ProofResult<CanvasPaint> {
    match value {
        ThemeRouteCutoverValue::Transparent => Ok(CanvasPaint::Transparent),
        ThemeRouteCutoverValue::Solid => CanvasPaint::solid(solid)
            .map_err(|error| C6ProofError::new("route-theme-paint", error.to_string())),
    }
}

fn cutover_renderer() -> Renderer {
    Renderer::new().with_engine(Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({
            "htmlLabels": false,
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
            == merman_render::__private::family_native_filter_receipt(evidence.family_report()),
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
    prove_cutover_font_plan(report.fonts())
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
    receipts: BTreeMap<ThemeRouteCutoverDescriptor, C6RouteCutoverReceipt>,
) -> Result<[u8; 32], C6RuntimeError> {
    let expected = inventory.into_iter().collect::<BTreeSet<_>>();
    let actual = receipts.keys().copied().collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(C6RuntimeError::RouteCutoverCoverageMismatch {
            missing: expected
                .difference(&actual)
                .copied()
                .map(route_label)
                .collect(),
            unexpected: actual
                .difference(&expected)
                .copied()
                .map(route_label)
                .collect(),
        });
    }

    let mut value = b"merman.c6-route-cutover-report.v1\0".to_vec();
    for (&route, receipt) in &receipts {
        append_route(&mut value, route);
        value.extend_from_slice(&receipt.digest);
    }
    Ok(sha256(value))
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

fn route_control_color(route: ThemeRouteCutoverDescriptor) -> C6ProofResult<ControlColor> {
    match (route.target(), route.facet()) {
        (ThemeTarget::Node | ThemeTarget::Actor, ThemeRouteCutoverFacet::Fill) => Ok(SOLID_FILL),
        (ThemeTarget::Node | ThemeTarget::Actor, ThemeRouteCutoverFacet::Stroke) => {
            Ok(SOLID_STROKE)
        }
        (ThemeTarget::Edge, ThemeRouteCutoverFacet::Stroke) => Ok(SOLID_EDGE),
        _ => Err(C6ProofError::new(
            "route-png-proof",
            format!("no PNG control color for {}", route_label(route)),
        )),
    }
}

fn minimum_control_pixels_per_region(route: ThemeRouteCutoverDescriptor) -> usize {
    match route.facet() {
        ThemeRouteCutoverFacet::Fill => 8,
        ThemeRouteCutoverFacet::Stroke => 1,
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
            (ThemeTarget::Node | ThemeTarget::Actor, ThemeRouteCutoverFacet::Fill) => {
                Ok(SOLID_FILL.css)
            }
            (ThemeTarget::Node | ThemeTarget::Actor, ThemeRouteCutoverFacet::Stroke) => {
                Ok(SOLID_STROKE.css)
            }
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

fn case_label(case: CutoverCase) -> String {
    route_label(case.route)
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
    for projection in route.projections().iter() {
        append_len_prefixed(output, projection.contribution_id().as_bytes());
        append_len_prefixed(output, projection.action().id().as_bytes());
    }
}

fn append_rect(output: &mut Vec<u8>, rect: [f64; 4]) {
    for value in rect {
        output.extend_from_slice(&value.to_bits().to_be_bytes());
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn prove_route<T>(witness: &str, result: C6ProofResult<T>) -> Result<T, C6RuntimeError> {
    result.map_err(|error| error.into_route_runtime(witness))
}

#[cfg(test)]
mod tests {
    use super::{
        legacy_replacing_typed_theme_routes, prove_route_cutover_witnesses,
        require_marker_pixel_counts, transformed_path_terminals,
    };

    #[test]
    fn every_legacy_replacing_typed_route_has_terminal_svg_and_png_proof() {
        let digest = prove_route_cutover_witnesses().expect("prove typed bridge cutovers");

        let inventory = legacy_replacing_typed_theme_routes().expect("derive route inventory");
        assert!(!inventory.is_empty());
        assert_ne!(digest, [0; 32]);
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
