use std::collections::BTreeSet;

use merman::svg::{
    BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, FontAssetSpec, FontCatalogSpec, FontEmbeddingRequirement, FontStack,
    GradientStop, LinearGradient, RadialGradient, Specified, ThemeAssets, ThemeColorValue,
    ThemeLength, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle,
    TypographySpec,
};
use merman::{DiagramFamilyId, RenderedDocument};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget, ReferenceBlendMode,
    ReferenceCanvasLayer, ReferenceFontBinding, ReferenceGradientKind, ReferenceGradientRepetition,
    ReferenceGradientStop, ReferenceThemeFacet, ReferenceThemeInput, ReferenceThemeMechanism,
    ThemeFixtureCatalog,
};

use crate::observation::{C6RenderIdentity, C6RuntimeError};

use super::c6_cyberpunk_flowchart_proof::{
    CyberpunkFlowchartProofContract, CyberpunkFlowchartSvgProof, prove_cyberpunk_flowchart_png,
    prove_cyberpunk_flowchart_svg,
};
use super::{
    C6CompletedRenderGroup, C6ProofError, C6ProofResult, C6RenderGroupAdapter, C6RenderGroupWork,
    FamilyEvidenceRequirements, bind_target_artifact, brutalist_solid as fixture_solid,
    c6_renderer, complete_c6_svg_png_render_group, load_render_group_fixture, portable_svg_request,
    prove_portable_family_evidence, prove_render_group,
    render_brutalist_document as render_c6_document,
};

const FIXTURE_ID: &str = "fixture-c6-cyberpunk-flowchart";
const FONT_FAMILY: &str = "Excalifont";
const FONT_ASSET_ID: &str = "font-excalifont-latin";
const CANVAS: &str = "#020617";
const NODE_STROKE: &str = "#22d3ee";
const NODE_STROKE_WIDTH: u16 = 2;
const TILE_SIZE_PX: u16 = 24;
const GRID_STOPS: [(u8, &str); 4] = [
    (0, "#22d3ee33"),
    (50, "#22d3ee33"),
    (51, "#22d3ee00"),
    (100, "#22d3ee00"),
];
const WASH_STOPS: [(u8, &str); 2] = [(0, "#0f172a"), (100, "#164e63")];
const RADIAL_STOPS: [(u8, &str); 2] = [(0, "#67e8f955"), (100, "#67e8f900")];
const CELL_INPUT_MECHANISMS: [ReferenceThemeMechanism; 7] = [
    ReferenceThemeMechanism::CanvasBlend,
    ReferenceThemeMechanism::CanvasGradient,
    ReferenceThemeMechanism::CanvasLayering,
    ReferenceThemeMechanism::CanvasPattern,
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::StrokeStyling,
];

pub(super) const CYBERPUNK_FLOWCHART_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Cyberpunk,
    family: C6ProofFamily::Flowchart,
    source_fixture_id: FIXTURE_ID,
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_cyberpunk_flowchart_group,
};

fn execute_cyberpunk_flowchart_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_cyberpunk_flowchart_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_cyberpunk_flowchart_png(rendered.contract, &rendered.svg_proof, artifact, plan)
        },
    )
}

struct CyberpunkFlowchartRenderedDocument {
    contract: CyberpunkFlowchartProofContract<'static>,
    document: RenderedDocument,
    identity: C6RenderIdentity,
    svg_proof: CyberpunkFlowchartSvgProof,
}

fn prove_cyberpunk_flowchart_rendered_document(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<CyberpunkFlowchartRenderedDocument> {
    validate_input(input)?;
    let theme = compile_theme(theme_catalog, input)?;
    let renderer = c6_renderer();
    let document = render_c6_document(&renderer, source, &theme, portable_svg_request())?;
    let identity = prove_portable_family_evidence(
        document.evidence(),
        &theme,
        DiagramFamilyId::FLOWCHART,
        FamilyEvidenceRequirements::SPOTLESS_FLOWCHART,
    )?;
    let contract = proof_contract();
    let artifact = bind_target_artifact(
        document.svg().as_bytes(),
        document.standalone_svg_admission(),
    )?;
    let svg_proof = prove_cyberpunk_flowchart_svg(contract, artifact)?;
    Ok(CyberpunkFlowchartRenderedDocument {
        contract,
        document,
        identity,
        svg_proof,
    })
}

fn validate_input(input: &ReferenceThemeInput) -> C6ProofResult<()> {
    let typography = input.typography().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Flowchart requires typed typography",
        )
    })?;
    let font_stack = typography.font_stack().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Flowchart requires a font stack",
        )
    })?;
    let node_style = input.node_style().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Flowchart requires node stroke styling",
        )
    })?;
    let border = node_style.border().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Flowchart requires a node border",
        )
    })?;
    let canvas_matches = matches!(
        input.canvas(),
        [
            ReferenceCanvasLayer::Solid { color },
            ReferenceCanvasLayer::LinearGradient {
                angle_degrees: 90,
                repetition: ReferenceGradientRepetition::Tiled,
                tile_width_px: Some(TILE_SIZE_PX),
                tile_height_px: Some(TILE_SIZE_PX),
                stops: grid_stops,
            },
            ReferenceCanvasLayer::LinearGradient {
                angle_degrees: 135,
                repetition: ReferenceGradientRepetition::None,
                tile_width_px: None,
                tile_height_px: None,
                stops: wash_stops,
            },
            ReferenceCanvasLayer::RadialGradient {
                center_x_percent: 50,
                center_y_percent: 50,
                repetition: ReferenceGradientRepetition::None,
                tile_width_px: None,
                tile_height_px: None,
                stops: radial_stops,
            },
        ] if color.as_str() == CANVAS
            && stops_match(grid_stops, &GRID_STOPS)
            && stops_match(wash_stops, &WASH_STOPS)
            && stops_match(radial_stops, &RADIAL_STOPS)
    );
    let expected_mechanisms = CELL_INPUT_MECHANISMS.into_iter().collect::<BTreeSet<_>>();
    let expected_facets = BTreeSet::from([
        ReferenceThemeFacet::CanvasBlend {
            mode: ReferenceBlendMode::Screen,
        },
        ReferenceThemeFacet::CanvasGradient {
            gradient: ReferenceGradientKind::Linear,
            repetition: ReferenceGradientRepetition::Tiled,
        },
        ReferenceThemeFacet::CanvasGradient {
            gradient: ReferenceGradientKind::Linear,
            repetition: ReferenceGradientRepetition::None,
        },
        ReferenceThemeFacet::CanvasGradient {
            gradient: ReferenceGradientKind::Radial,
            repetition: ReferenceGradientRepetition::None,
        },
    ]);
    c6_ensure!(
        "fixture-contract",
        input.fixture_id() == FIXTURE_ID
            && input.tokens().is_none()
            && font_stack.binding() == ReferenceFontBinding::FixtureAssets
            && font_stack.families() == [FONT_FAMILY]
            && font_stack.asset_ids().len() == 1
            && font_stack.asset_ids().contains(FONT_ASSET_ID)
            && typography.letter_spacing_milli_em().is_none()
            && typography.text_transform().is_none()
            && canvas_matches
            && border.color() == NODE_STROKE
            && border.width_px() == NODE_STROKE_WIDTH
            && node_style.dash_pattern().is_empty()
            && node_style.corner_radius_px().is_none()
            && node_style.shadow().is_none()
            && input.semantic_rules().is_empty()
            && input.mechanisms() == expected_mechanisms
            && input.facets() == expected_facets,
        "Cyberpunk Flowchart fixture differs from its fixed C6 recipe"
    );
    Ok(())
}

fn compile_theme(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
) -> C6ProofResult<DiagramTheme> {
    let (font_stack, assets) = resolve_font(theme_catalog, input)?;
    let node = ThemeStylePatch::default()
        .with_stroke(fixture_solid(
            NODE_STROKE,
            "cyberpunk-flowchart-node-stroke",
        )?)
        .with_stroke_width(f32::from(NODE_STROKE_WIDTH))
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-node-stroke", error.to_string()))?;
    let mut label = ThemeStylePatch::default();
    label.typography.font_stack = Specified::Value(font_stack);
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(ThemeTarget::Node, node).for_family(DiagramFamilyId::FLOWCHART))
        .with_rule(
            ThemeRule::new(ThemeTarget::NodeLabel, label).for_family(DiagramFamilyId::FLOWCHART),
        );
    let spec = DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(ThemeTextStyle::default()))
        .with_assets(assets)
        .with_canvas(cyberpunk_canvas()?)
        .with_styles(styles);
    DiagramThemeCompiler::new()
        .compile(spec)
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-theme", error.to_string()))
}

fn cyberpunk_canvas() -> C6ProofResult<CanvasSpec> {
    let grid = LinearGradient::new(90.0, fixed_gradient_stops(&GRID_STOPS)?)
        .and_then(|gradient| {
            gradient.with_tile_px(f32::from(TILE_SIZE_PX), f32::from(TILE_SIZE_PX))
        })
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-canvas", error.to_string()))?;
    let wash = LinearGradient::new(135.0, fixed_gradient_stops(&WASH_STOPS)?)
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-canvas", error.to_string()))?;
    let radial = RadialGradient::new(
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        fixed_gradient_stops(&RADIAL_STOPS)?,
    )
    .map_err(|error| C6ProofError::new("cyberpunk-flowchart-canvas", error.to_string()))?;
    CanvasSpec::default()
        .with_base(fixture_solid(CANVAS, "cyberpunk-flowchart-canvas")?)
        .with_layer(screen_layer(CanvasPaint::LinearGradient(grid)))
        .and_then(|canvas| canvas.with_layer(screen_layer(CanvasPaint::LinearGradient(wash))))
        .and_then(|canvas| canvas.with_layer(screen_layer(CanvasPaint::RadialGradient(radial))))
        .map_err(|error| C6ProofError::new("cyberpunk-flowchart-canvas", error.to_string()))
}

fn screen_layer(paint: CanvasPaint) -> CanvasLayer {
    CanvasLayer::new(paint).with_blend_mode(BlendMode::Screen)
}

fn resolve_font(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
) -> C6ProofResult<(FontStack, ThemeAssets)> {
    let asset = theme_catalog.font_asset(FONT_ASSET_ID).ok_or_else(|| {
        C6ProofError::new(
            "fixture-font-asset",
            format!("font asset `{FONT_ASSET_ID}` does not exist"),
        )
    })?;
    c6_ensure!(
        "fixture-font-asset",
        asset.family() == FONT_FAMILY,
        "font asset family `{}` differs from `{FONT_FAMILY}`",
        asset.family()
    );
    let bytes = theme_catalog
        .asset_bytes(FONT_ASSET_ID)
        .map_err(|error| C6ProofError::new("fixture-font-bytes", error.to_string()))?;
    let stack = FontStack::single(FONT_FAMILY)
        .map_err(|error| C6ProofError::new("fixture-font-stack", error.to_string()))?;
    let catalog = FontCatalogSpec::new([FontAssetSpec::new(asset.id(), bytes)])
        .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
    c6_ensure!(
        "fixture-font-stack",
        input
            .typography()
            .and_then(|typography| typography.font_stack())
            .is_some(),
        "Cyberpunk Flowchart fixture lost its font binding"
    );
    Ok((stack, ThemeAssets::default().with_font_catalog(catalog)))
}

fn fixed_gradient_stops<const N: usize>(
    stops: &[(u8, &str); N],
) -> C6ProofResult<Vec<GradientStop>> {
    stops
        .iter()
        .map(|(offset, color)| {
            GradientStop::new(
                f32::from(*offset) / 100.0,
                ThemeColorValue::parse(color).map_err(|error| {
                    C6ProofError::new("cyberpunk-flowchart-canvas", error.to_string())
                })?,
            )
            .map_err(|error| C6ProofError::new("cyberpunk-flowchart-canvas", error.to_string()))
        })
        .collect()
}

fn stops_match<const N: usize>(
    actual: &[ReferenceGradientStop],
    expected: &[(u8, &str); N],
) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.offset_percent() == expected.0 && actual.color() == expected.1
        })
}

fn proof_contract() -> CyberpunkFlowchartProofContract<'static> {
    CyberpunkFlowchartProofContract {
        canvas: CANVAS,
        node_stroke: NODE_STROKE,
        node_stroke_width: NODE_STROKE_WIDTH as f64,
        blend_mode: "screen",
        tile_size_px: [TILE_SIZE_PX as f64, TILE_SIZE_PX as f64],
        grid_stops: GRID_STOPS.map(|(offset, color)| (offset as f64, color)),
        wash_stops: WASH_STOPS.map(|(offset, color)| (offset as f64, color)),
        radial_stops: RADIAL_STOPS.map(|(offset, color)| (offset as f64, color)),
        font_family: FONT_FAMILY,
    }
}
