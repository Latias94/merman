use merman_core::{Engine, OperationControl, ParseOptions};
use merman_display_list::{
    DrawingCommand, DrawingListDocument, DrawingListPolicy, DrawingResource, Paint, ResourceId,
};
use merman_render::environment::RenderEnvironment;
use merman_render::{LayoutOptions, family};
use serde::Deserialize;
use std::collections::BTreeSet;

const EFFECT_COVERAGE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/drawing-list/v1/effect-coverage.json"
));
const FAMILY_COVERAGE_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/drawing-list/v1/family-coverage.json"
));

#[derive(Debug, Deserialize)]
struct EffectCoverageMatrix {
    schema_version: u32,
    family_baselines: Vec<FamilyEffectBaseline>,
    effects: Vec<EffectCoverageRow>,
}

#[derive(Debug, Deserialize)]
struct FamilyEffectBaseline {
    family: String,
    source: String,
    expected_effects: Vec<String>,
    required_feature: Option<RequiredFeature>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RequiredFeature {
    LayoutCytoscape,
}

impl RequiredFeature {
    const fn enabled(self) -> bool {
        match self {
            Self::LayoutCytoscape => cfg!(feature = "layout-cytoscape"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FamilyCoverageMatrix {
    schema_version: u32,
    families: Vec<FamilyCoverageRow>,
}

#[derive(Debug, Deserialize)]
struct FamilyCoverageRow {
    id: String,
    document_adapter: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectDisposition {
    Vector,
    Raster,
    Error,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectAssertion {
    PathFillStroke,
    HostText,
    TextStroke,
    LinearGradient,
    RadialGradient,
    ImageResource,
    Pattern,
    FontResource,
    ClipPath,
    Layer,
    Image,
    RasterSubtree,
    GlyphRun,
    TextOutline,
    TextRasterFallback,
    SemanticLink,
    Opacity,
    BlendMode,
    Transform,
    ExpandedMarker,
    ErrorMessage,
}

impl EffectAssertion {
    fn protocol_kind(&self) -> &'static str {
        match self {
            Self::PathFillStroke => "path",
            Self::HostText => "host_text",
            Self::TextStroke => "text_stroke",
            Self::LinearGradient => "linear_gradient",
            Self::RadialGradient => "radial_gradient",
            Self::ImageResource => "image_resource",
            Self::Pattern => "pattern",
            Self::FontResource => "font_resource",
            Self::ClipPath => "clip_path",
            Self::Layer => "layer",
            Self::Image => "image",
            Self::RasterSubtree => "raster_subtree",
            Self::GlyphRun => "glyph_run",
            Self::TextOutline => "text_outline",
            Self::TextRasterFallback => "text_raster_fallback",
            Self::SemanticLink => "semantic_link",
            Self::Opacity => "opacity",
            Self::BlendMode => "blend_mode",
            Self::Transform => "transform",
            Self::ExpandedMarker => "expanded_marker",
            Self::ErrorMessage => "structured_error",
        }
    }
}

#[derive(Debug, Deserialize)]
struct EffectCoverageRow {
    id: String,
    disposition: EffectDisposition,
    family: String,
    source: String,
    assertion: EffectAssertion,
    expected_effects: Vec<String>,
    expected_error: Option<String>,
    evidence: String,
}

#[test]
fn every_direct_family_has_a_family_local_effect_baseline() {
    let matrix: EffectCoverageMatrix = serde_json::from_str(EFFECT_COVERAGE_JSON)
        .expect("effect coverage fixture must be valid JSON");
    let family_matrix: FamilyCoverageMatrix = serde_json::from_str(FAMILY_COVERAGE_JSON)
        .expect("family coverage fixture must be valid JSON");
    assert_eq!(matrix.schema_version, 2);
    assert_eq!(family_matrix.schema_version, 1);

    let direct_families = family_matrix
        .families
        .iter()
        .filter(|row| row.document_adapter == "direct")
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let baseline_families = matrix
        .family_baselines
        .iter()
        .map(|baseline| baseline.family.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        baseline_families.len(),
        matrix.family_baselines.len(),
        "family effect baselines must be unique"
    );
    assert_eq!(
        baseline_families, direct_families,
        "every direct DrawingList family must have exactly one effect baseline"
    );

    for baseline in &matrix.family_baselines {
        assert!(
            !baseline.family.trim().is_empty(),
            "family must not be empty"
        );
        assert!(
            !baseline.source.trim().is_empty(),
            "{} family baseline has no source",
            baseline.family
        );
        expected_effects(&baseline.family, baseline.expected_effects.as_slice());
    }

    for baseline in matrix.family_baselines {
        if baseline
            .required_feature
            .is_some_and(|feature| !feature.enabled())
        {
            continue;
        }
        let expected = expected_effects(&baseline.family, &baseline.expected_effects);
        let (actual_family, rendered) = render_drawing_list(&baseline.source);
        assert_eq!(
            actual_family, baseline.family,
            "{} family baseline drifted",
            baseline.family
        );
        let document = rendered.unwrap_or_else(|error| {
            panic!(
                "{} baseline failed instead of producing a document: {error}",
                baseline.family
            )
        });
        assert_eq!(
            observed_protocol_effects(&document),
            expected,
            "{} family baseline emitted an undeclared effect or stopped emitting a declared effect",
            baseline.family
        );
    }
}

#[test]
fn exercised_effects_have_an_explicit_vector_raster_or_error_disposition() {
    let matrix: EffectCoverageMatrix = serde_json::from_str(EFFECT_COVERAGE_JSON)
        .expect("effect coverage fixture must be valid JSON");
    assert_eq!(matrix.schema_version, 2);
    assert!(!matrix.effects.is_empty());
    let known_families = merman_render::family::RenderFamilyKind::ALL
        .into_iter()
        .map(merman_render::family::RenderFamilyKind::as_str)
        .collect::<BTreeSet<_>>();
    let mut effect_ids = BTreeSet::new();
    for row in matrix.effects {
        assert!(!row.id.trim().is_empty(), "effect ids must not be empty");
        assert!(
            effect_ids.insert(row.id.clone()),
            "duplicate effect id {}",
            row.id
        );
        assert!(!row.family.trim().is_empty(), "{} has no family", row.id);
        assert!(
            known_families.contains(row.family.as_str()),
            "{} references unknown family {}",
            row.id,
            row.family
        );
        assert!(!row.source.trim().is_empty(), "{} has no source", row.id);
        assert!(
            !row.evidence.trim().is_empty(),
            "{} has no evidence",
            row.id
        );
        let expected_effects = expected_effects(&row.id, &row.expected_effects);

        let (family, rendered) = render_drawing_list(&row.source);
        assert_eq!(family, row.family, "{} family fixture drifted", row.id);
        match row.disposition {
            EffectDisposition::Vector => {
                assert!(
                    row.expected_error.is_none(),
                    "vector effect {} must not carry an error expectation",
                    row.id
                );
                let document = rendered.unwrap_or_else(|error| {
                    panic!(
                        "{} ({}) is classified as vector but failed: {error}",
                        row.id, row.family
                    )
                });
                assert_fixture_effects(&row.id, &row.assertion, &expected_effects, &document);
                assert_effect_construct(&row.id, row.assertion, &document);
            }
            EffectDisposition::Raster => {
                assert!(
                    row.expected_error.is_none(),
                    "raster effect {} must not carry an error expectation",
                    row.id
                );
                let document = rendered.unwrap_or_else(|error| {
                    panic!(
                        "{} ({}) is classified as raster but failed: {error}",
                        row.id, row.family
                    )
                });
                assert_fixture_effects(&row.id, &row.assertion, &expected_effects, &document);
                assert_effect_construct(&row.id, row.assertion, &document);
                assert!(
                    document
                        .commands
                        .iter()
                        .any(|command| matches!(command, DrawingCommand::DrawRasterSubtree { .. })),
                    "{} must expose a raster subtree command",
                    row.id
                );
            }
            EffectDisposition::Error => {
                assert!(
                    row.expected_error.is_some(),
                    "error effect {} must carry an error expectation",
                    row.id
                );
                assert!(
                    expected_effects.is_empty(),
                    "error effect {} cannot declare emitted protocol effects",
                    row.id
                );
                let error = rendered.expect_err(&format!(
                    "{} ({}) is classified as an explicit error",
                    row.id, row.family
                ));
                assert!(matches!(row.assertion, EffectAssertion::ErrorMessage));
                let expected = row
                    .expected_error
                    .as_deref()
                    .unwrap_or_else(|| panic!("{} has no expected error text", row.id));
                assert!(
                    error.contains(expected),
                    "{} error should contain {expected:?}, got {error:?}",
                    row.id
                );
                assert!(
                    error.contains("DrawingList"),
                    "{} must remain a structured DrawingList capability error: {error}",
                    row.id
                );
            }
        }
    }
}

fn expected_effects<'a>(id: &str, effects: &'a [String]) -> BTreeSet<&'a str> {
    let expected = effects.iter().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(
        expected.len(),
        effects.len(),
        "{id} repeats an expected protocol effect"
    );
    expected
}

fn assert_fixture_effects(
    id: &str,
    assertion: &EffectAssertion,
    expected_effects: &BTreeSet<&str>,
    document: &DrawingListDocument,
) {
    let asserted_effect = assertion.protocol_kind();
    assert!(
        expected_effects.contains(asserted_effect),
        "{id} does not include its asserted effect {asserted_effect:?} in expected_effects"
    );
    assert_eq!(
        observed_protocol_effects(document),
        *expected_effects,
        "{id} must account for every protocol effect emitted by its own fixture"
    );
}

fn render_drawing_list(source: &str) -> (String, Result<DrawingListDocument, String>) {
    let parsed =
        match Engine::new().parse_diagram_for_render_model_sync(source, ParseOptions::lenient()) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => {
                return (
                    String::new(),
                    Err("source did not produce a diagram".into()),
                );
            }
            Err(error) => return (String::new(), Err(error.to_string())),
        };
    let session = match RenderEnvironment::deterministic()
        .begin_session_with_control(OperationControl::new())
    {
        Ok(session) => session,
        Err(error) => return (String::new(), Err(error.to_string())),
    };
    let artifact = match family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session) {
        Ok(artifact) => artifact,
        Err(error) => return (String::new(), Err(error.to_string())),
    };
    let family = artifact.family_kind().as_str().to_string();
    let rendered = artifact
        .render_drawing_list(DrawingListPolicy::AllowRasterSubtree, Default::default())
        .map(|output| output.document().clone())
        .map_err(|error| error.to_string());
    (family, rendered)
}

fn assert_effect_construct(id: &str, assertion: EffectAssertion, document: &DrawingListDocument) {
    match assertion {
        EffectAssertion::PathFillStroke => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawPath { style, .. }
                    if style.fill.is_some() && style.stroke.is_some()
            )),
            "{id} must retain both fill and stroke"
        ),
        EffectAssertion::HostText => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawText { run }
                    if matches!(run.obligation, merman_display_list::TextObligation::HostText { .. })
            )),
            "{id} must retain a host-text obligation"
        ),
        EffectAssertion::TextStroke => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawText { run }
                    if run.style.stroke.as_ref().is_some_and(|stroke| stroke.paint == run.style.fill)
                        && run.style.paint_order
                            == merman_display_list::TextPaintOrder::FillThenStroke
            )),
            "{id} must retain a text stroke used by DrawText"
        ),
        EffectAssertion::LinearGradient => assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                DrawingResource::LinearGradient(gradient)
                    if path_paint_references_resource(document, &gradient.id)
            )),
            "{id} must retain a linear-gradient resource referenced by a path paint"
        ),
        EffectAssertion::RadialGradient => assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                DrawingResource::RadialGradient(gradient)
                    if path_paint_references_resource(document, &gradient.id)
            )),
            "{id} must retain a radial-gradient resource referenced by a path paint"
        ),
        EffectAssertion::ImageResource => assert!(
            document
                .resources
                .iter()
                .any(|resource| matches!(resource, DrawingResource::Image(_))),
            "{id} must retain an image resource"
        ),
        EffectAssertion::Pattern => assert!(
            document
                .resources
                .iter()
                .any(|resource| matches!(resource, DrawingResource::Pattern(_))),
            "{id} must retain a pattern resource"
        ),
        EffectAssertion::FontResource => assert!(
            document
                .resources
                .iter()
                .any(|resource| matches!(resource, DrawingResource::Font(_))),
            "{id} must retain a font resource"
        ),
        EffectAssertion::ClipPath => assert!(
            document
                .commands
                .iter()
                .any(|command| matches!(command, DrawingCommand::ClipPath { .. })),
            "{id} must retain a clip-path command"
        ),
        EffectAssertion::Layer => assert!(
            document
                .commands
                .iter()
                .any(|command| matches!(command, DrawingCommand::BeginLayer { .. })),
            "{id} must retain a layer command"
        ),
        EffectAssertion::Image => assert!(
            document
                .commands
                .iter()
                .any(|command| matches!(command, DrawingCommand::DrawImage { .. })),
            "{id} must retain an image command"
        ),
        EffectAssertion::RasterSubtree => assert!(
            document
                .commands
                .iter()
                .any(|command| matches!(command, DrawingCommand::DrawRasterSubtree { .. })),
            "{id} must retain a raster-subtree command"
        ),
        EffectAssertion::GlyphRun => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawText { run }
                    if matches!(run.obligation, merman_display_list::TextObligation::GlyphRun { .. })
            )),
            "{id} must retain a glyph-run obligation"
        ),
        EffectAssertion::TextOutline => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawText { run }
                    if matches!(run.obligation, merman_display_list::TextObligation::Outline { .. })
            )),
            "{id} must retain a text-outline obligation"
        ),
        EffectAssertion::TextRasterFallback => assert!(
            document.commands.iter().any(|command| matches!(
                command,
                DrawingCommand::DrawText { run }
                    if matches!(run.obligation, merman_display_list::TextObligation::RasterFallback { .. })
            )),
            "{id} must retain a text-raster-fallback obligation"
        ),
        EffectAssertion::SemanticLink => assert!(
            document
                .semantics
                .iter()
                .any(|semantic| semantic.link.is_some()),
            "{id} must retain semantic navigation metadata"
        ),
        EffectAssertion::Opacity => assert!(
            document.commands.iter().any(|command| {
                matches!(command, DrawingCommand::SetOpacity { opacity } if *opacity < 1.0)
                    || matches!(
                        command,
                        DrawingCommand::DrawPath { style, .. }
                            if style.fill.as_ref().is_some_and(paint_has_transparency)
                                || style
                                    .stroke
                                    .as_ref()
                                    .is_some_and(|stroke| paint_has_transparency(&stroke.paint))
                    )
            }),
            "{id} must retain a non-default opacity command"
        ),
        EffectAssertion::BlendMode => assert!(
            document.commands.iter().any(|command| {
                matches!(
                    command,
                    DrawingCommand::SetBlendMode { blend_mode }
                        if *blend_mode != merman_display_list::BlendMode::Normal
                )
            }),
            "{id} must retain a non-default blend-mode command"
        ),
        EffectAssertion::Transform => assert!(
            document.commands.iter().any(|command| {
                matches!(
                    command,
                    DrawingCommand::ConcatTransform { transform }
                        if *transform != merman_display_list::Transform::IDENTITY
                )
            }),
            "{id} must retain a non-identity transform command"
        ),
        EffectAssertion::ExpandedMarker => assert!(
            has_expanded_marker(document),
            "{id} must retain expanded marker geometry"
        ),
        EffectAssertion::ErrorMessage => {
            panic!("{id} is an error assertion but was rendered as a document")
        }
    }
}

fn path_paint_references_resource(
    document: &DrawingListDocument,
    resource_id: &ResourceId,
) -> bool {
    document.commands.iter().any(|command| {
        let DrawingCommand::DrawPath { style, .. } = command else {
            return false;
        };
        style
            .fill
            .as_ref()
            .is_some_and(|paint| paint_references_resource(paint, resource_id))
            || style
                .stroke
                .as_ref()
                .is_some_and(|stroke| paint_references_resource(&stroke.paint, resource_id))
    })
}

fn paint_references_resource(paint: &Paint, resource_id: &ResourceId) -> bool {
    matches!(paint, Paint::Resource { id } if id == resource_id)
}

#[test]
fn gradient_reference_check_rejects_an_orphaned_resource() {
    let (_, rendered) = render_drawing_list("sankey\nSource,Target,10\n");
    let mut document = rendered.expect("Sankey gradient fixture must render");
    let gradient_id = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            DrawingResource::LinearGradient(gradient) => Some(gradient.id.clone()),
            _ => None,
        })
        .expect("Sankey gradient fixture must contain a linear-gradient resource");
    assert!(path_paint_references_resource(&document, &gradient_id));

    let gradient_stroke = document
        .commands
        .iter_mut()
        .find_map(|command| {
            let DrawingCommand::DrawPath { style, .. } = command else {
                return None;
            };
            let stroke = style.stroke.as_mut()?;
            paint_references_resource(&stroke.paint, &gradient_id).then_some(stroke)
        })
        .expect("Sankey link path must reference its gradient resource");
    gradient_stroke.paint = Paint::solid(merman_display_list::Color::rgba(0, 0, 0, 255));

    document
        .validate()
        .expect("an orphaned gradient is currently protocol-valid");
    assert!(!path_paint_references_resource(&document, &gradient_id));
}

#[test]
fn expanded_marker_reference_check_rejects_orphaned_resources() {
    let (_, rendered) = render_drawing_list(
        "wardley-beta\ncomponent A [0.8, 0.2]\ncomponent B [0.4, 0.8]\nA -> B\nevolve A 0.7\n",
    );
    let mut document = rendered.expect("Wardley marker fixture must render");
    let marker_ids = document
        .resources
        .iter()
        .filter_map(|resource| match resource {
            DrawingResource::Path(path) if is_expanded_marker_path_id(&path.id) => {
                Some(path.id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!marker_ids.is_empty());
    assert!(has_expanded_marker(&document));

    document.commands.retain(|command| {
        !matches!(
            command,
            DrawingCommand::DrawPath { path, .. } if marker_ids.contains(path)
        )
    });

    document
        .validate()
        .expect("orphaned marker resources are currently protocol-valid");
    assert!(!has_expanded_marker(&document));
    assert!(!observed_protocol_effects(&document).contains("expanded_marker"));
}

fn observed_protocol_effects(document: &DrawingListDocument) -> BTreeSet<&'static str> {
    let mut effects = BTreeSet::new();

    // Keep these matches exhaustive.  Adding a new protocol resource, command, or text
    // obligation must force this gate to classify it before a renderer can silently introduce it.
    for resource in &document.resources {
        match resource {
            DrawingResource::Path(_) => {
                effects.insert("path");
            }
            DrawingResource::LinearGradient(_) => {
                effects.insert("linear_gradient");
            }
            DrawingResource::RadialGradient(_) => {
                effects.insert("radial_gradient");
            }
            DrawingResource::Image(_) => {
                effects.insert("image_resource");
            }
            DrawingResource::Pattern(_) => {
                effects.insert("pattern");
            }
            DrawingResource::Font(_) => {
                effects.insert("font_resource");
            }
        }
    }

    for command in &document.commands {
        match command {
            DrawingCommand::SetOpacity { opacity } if *opacity < 1.0 => {
                effects.insert("opacity");
            }
            DrawingCommand::SetOpacity { .. } => {}
            DrawingCommand::SetBlendMode { blend_mode }
                if *blend_mode != merman_display_list::BlendMode::Normal =>
            {
                effects.insert("blend_mode");
            }
            DrawingCommand::SetBlendMode { .. } => {}
            DrawingCommand::ConcatTransform { transform }
                if *transform != merman_display_list::Transform::IDENTITY =>
            {
                effects.insert("transform");
            }
            DrawingCommand::ConcatTransform { .. } => {}
            DrawingCommand::BeginLayer { .. } => {
                effects.insert("layer");
            }
            DrawingCommand::EndLayer => {}
            DrawingCommand::DrawPath { style, .. } => {
                if style.fill.as_ref().is_some_and(paint_has_transparency)
                    || style
                        .stroke
                        .as_ref()
                        .is_some_and(|stroke| paint_has_transparency(&stroke.paint))
                {
                    effects.insert("opacity");
                }
            }
            DrawingCommand::ClipPath { .. } => {
                effects.insert("clip_path");
            }
            DrawingCommand::DrawImage { .. } => {
                effects.insert("image");
            }
            DrawingCommand::BeginSemanticGroup { .. }
            | DrawingCommand::EndSemanticGroup
            | DrawingCommand::Save
            | DrawingCommand::Restore => {}
            DrawingCommand::DrawRasterSubtree { .. } => {
                effects.insert("raster_subtree");
            }
            DrawingCommand::DrawText { run } => {
                if run.style.stroke.is_some() {
                    effects.insert("text_stroke");
                }
                match &run.obligation {
                    merman_display_list::TextObligation::HostText { .. } => {
                        effects.insert("host_text");
                    }
                    merman_display_list::TextObligation::GlyphRun { .. } => {
                        effects.insert("glyph_run");
                    }
                    merman_display_list::TextObligation::Outline { .. } => {
                        effects.insert("text_outline");
                    }
                    merman_display_list::TextObligation::RasterFallback { .. } => {
                        effects.insert("text_raster_fallback");
                    }
                }
            }
        }
    }

    if document
        .semantics
        .iter()
        .any(|semantic| semantic.link.is_some())
    {
        effects.insert("semantic_link");
    }
    if has_expanded_marker(document) {
        effects.insert("expanded_marker");
    }
    effects
}

fn paint_has_transparency(paint: &Paint) -> bool {
    matches!(paint, Paint::Solid { color } if color.alpha < u8::MAX)
}

fn has_expanded_marker(document: &DrawingListDocument) -> bool {
    document
        .resources
        .iter()
        .filter_map(|resource| match resource {
            DrawingResource::Path(path) if is_expanded_marker_path_id(&path.id) => Some(&path.id),
            _ => None,
        })
        .any(|marker_id| {
            document.commands.iter().any(|command| {
                matches!(
                    command,
                    DrawingCommand::DrawPath { path, .. } if path == marker_id
                )
            })
        })
}

fn is_expanded_marker_path_id(path_id: &ResourceId) -> bool {
    path_id.as_str().contains("marker")
        || (path_id.as_str().starts_with("wardley.link.")
            && (path_id.as_str().ends_with(".start") || path_id.as_str().ends_with(".end")))
        || (path_id.as_str().starts_with("wardley.trend.") && path_id.as_str().ends_with(".end"))
}
