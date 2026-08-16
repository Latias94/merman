use merman::svg::{
    CanvasLayer, CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontAssetSpec,
    FontCatalogSpec, FontEmbeddingRequirement, FontStack, GradientStop, LinearGradient,
    RadialGradient, Specified, TextTransform, ThemeAssets, ThemeColorValue, ThemeLength, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman::{DiagramFamilyId, RenderedDocument};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget, ReferenceCanvasLayer,
    ReferenceFontBinding, ReferenceGradientRepetition, ReferenceGradientStop,
    ReferenceTextTransform, ReferenceThemeInput, ThemeFixtureCatalog,
};

use crate::observation::C6RuntimeError;

use super::c6_cyberpunk_state_proof::{
    CYBERPUNK_STATE_EFFECT_ID, CyberpunkStateProofContract, CyberpunkStateSvgProof,
    prove_cyberpunk_state_png, prove_cyberpunk_state_svg,
};
use super::c6_spotless_flowchart_proof::{
    SpotlessFlowchartProofContract, SpotlessFlowchartSvgProof, prove_spotless_flowchart_png,
    prove_spotless_flowchart_svg,
};
use super::c6_spotless_state_proof::{
    SpotlessStateProofContract, SpotlessStateSvgProof, prove_spotless_state_png,
    prove_spotless_state_svg,
};
use super::{
    C6CompletedRenderGroup, C6ProofError, C6ProofResult, C6RenderGroupAdapter, C6RenderGroupWork,
    FamilyEvidenceRequirements, bind_document_svg_artifact, brutalist_solid as fixture_solid,
    c6_renderer, complete_c6_svg_png_render_group, load_render_group_fixture, portable_svg_request,
    prove_portable_family_evidence, prove_render_group,
    render_brutalist_document as render_c6_document,
};

const SPOTLESS_FLOWCHART_FIXTURE_ID: &str = "fixture-c6-spotless-flowchart";
const SPOTLESS_STATE_FIXTURE_ID: &str = "fixture-c6-spotless-state";
const CYBERPUNK_STATE_FIXTURE_ID: &str = "fixture-c6-cyberpunk-state";

const RECIPE_FONT_FAMILY: &str = "Excalifont";
const RECIPE_FONT_ASSET_ID: &str = "font-excalifont-latin";

const SPOTLESS_FLOWCHART_STRIPE_STOPS: [(u8, &str); 4] = [
    (0, "#e7e2d6"),
    (50, "#e7e2d6"),
    (51, "#d2ccc0"),
    (100, "#d2ccc0"),
];
const SPOTLESS_FLOWCHART_STROKE: &str = "#2c2416";
const SPOTLESS_FLOWCHART_STROKE_WIDTH: u16 = 2;
const SPOTLESS_FLOWCHART_DASH_PATTERN: [u16; 2] = [6, 4];
const SPOTLESS_FLOWCHART_RADIUS: u16 = 4;
const SPOTLESS_FLOWCHART_PATTERN_PERIOD: u16 = 20;

const SPOTLESS_STATE_CANVAS: &str = "#ede8dc";
const SPOTLESS_STATE_SURFACE: &str = "#f5f1e8";
const SPOTLESS_STATE_PRIMARY: &str = "#2c2416";
const SPOTLESS_STATE_TEXT: &str = "#1a1a1a";
const SPOTLESS_STATE_PATTERN_PERIOD: u16 = 20;
const SPOTLESS_STATE_LETTER_SPACING_MILLI_EM: i16 = 40;
const SPOTLESS_STATE_LINEAR_STOPS: [(u8, &str); 4] = [
    (0, "#2c241608"),
    (50, "#2c241608"),
    (51, "#2c24160d"),
    (100, "#2c24160d"),
];
const SPOTLESS_STATE_RADIAL_STOPS: [(u8, &str); 4] = [
    (0, "#2c24162e"),
    (10, "#2c24162e"),
    (11, "#2c241600"),
    (100, "#2c241600"),
];

const CYBERPUNK_STATE_CANVAS: &str = "#020617";
const CYBERPUNK_STATE_SURFACE: &str = "#0f172a";
const CYBERPUNK_STATE_PRIMARY: &str = "#22d3ee";
const CYBERPUNK_STATE_TEXT: &str = "#e0f2fe";
const CYBERPUNK_STATE_BORDER_WIDTH: u16 = 2;
const CYBERPUNK_STATE_RADIUS: u16 = 10;
const CYBERPUNK_STATE_GLOW_BLUR: u16 = 8;

#[derive(Clone, Copy)]
enum ReferenceRecipeId {
    SpotlessFlowchart,
    SpotlessState,
    CyberpunkState,
}

impl ReferenceRecipeId {
    const fn fixture_id(self) -> &'static str {
        match self {
            Self::SpotlessFlowchart => SPOTLESS_FLOWCHART_FIXTURE_ID,
            Self::SpotlessState => SPOTLESS_STATE_FIXTURE_ID,
            Self::CyberpunkState => CYBERPUNK_STATE_FIXTURE_ID,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::SpotlessFlowchart => "Spotless Flowchart",
            Self::SpotlessState => "Spotless State",
            Self::CyberpunkState => "Cyberpunk State",
        }
    }
}

enum FixedC6Recipe {
    SpotlessFlowchart {
        theme: DiagramTheme,
        expected: SpotlessFlowchartProofContract<'static>,
    },
    SpotlessState {
        theme: DiagramTheme,
        expected: SpotlessStateProofContract<'static>,
    },
    CyberpunkState {
        theme: DiagramTheme,
        expected: CyberpunkStateProofContract<'static>,
    },
}

struct FixedRecipeFont {
    stack: FontStack,
    assets: ThemeAssets,
}

pub(super) const SPOTLESS_FLOWCHART_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Spotless,
    family: C6ProofFamily::Flowchart,
    source_fixture_id: SPOTLESS_FLOWCHART_FIXTURE_ID,
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_spotless_flowchart_group,
};

pub(super) const SPOTLESS_STATE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Spotless,
    family: C6ProofFamily::State,
    source_fixture_id: SPOTLESS_STATE_FIXTURE_ID,
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_spotless_state_group,
};

pub(super) const CYBERPUNK_STATE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Cyberpunk,
    family: C6ProofFamily::State,
    source_fixture_id: CYBERPUNK_STATE_FIXTURE_ID,
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_cyberpunk_state_group,
};

fn execute_spotless_flowchart_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_spotless_flowchart_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_spotless_flowchart_png(rendered.expected, &rendered.svg_proof, artifact, plan)
        },
    )
}

fn execute_spotless_state_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_spotless_state_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_spotless_state_png(rendered.expected, &rendered.svg_proof, artifact, plan)
        },
    )
}

fn execute_cyberpunk_state_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_cyberpunk_state_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_cyberpunk_state_png(rendered.expected, &rendered.svg_proof, artifact, plan)
        },
    )
}

struct SpotlessFlowchartRenderedDocument {
    expected: SpotlessFlowchartProofContract<'static>,
    document: RenderedDocument,
    identity: crate::observation::C6RenderIdentity,
    svg_proof: SpotlessFlowchartSvgProof,
}

struct SpotlessStateRenderedDocument {
    expected: SpotlessStateProofContract<'static>,
    document: RenderedDocument,
    identity: crate::observation::C6RenderIdentity,
    svg_proof: SpotlessStateSvgProof,
}

struct CyberpunkStateRenderedDocument {
    expected: CyberpunkStateProofContract<'static>,
    document: RenderedDocument,
    identity: crate::observation::C6RenderIdentity,
    svg_proof: CyberpunkStateSvgProof,
}

fn prove_spotless_flowchart_rendered_document(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<SpotlessFlowchartRenderedDocument> {
    let (document, identity, expected) = {
        let FixedC6Recipe::SpotlessFlowchart { theme, expected } =
            load_fixed_c6_recipe(ReferenceRecipeId::SpotlessFlowchart, theme_catalog, input)?
        else {
            unreachable!("fixed recipe loader returned the wrong variant")
        };
        let renderer = c6_renderer();
        let document = render_c6_document(&renderer, source, &theme, portable_svg_request())?;
        let identity = prove_portable_family_evidence(
            document.evidence(),
            &theme,
            DiagramFamilyId::FLOWCHART,
            FamilyEvidenceRequirements::SPOTLESS_FLOWCHART,
        )?;
        (document, identity, expected)
    };
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_spotless_flowchart_svg(expected, artifact)?;
    Ok(SpotlessFlowchartRenderedDocument {
        expected,
        document,
        identity,
        svg_proof,
    })
}

fn prove_spotless_state_rendered_document(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<SpotlessStateRenderedDocument> {
    let (document, identity, expected) = {
        let FixedC6Recipe::SpotlessState { theme, expected } =
            load_fixed_c6_recipe(ReferenceRecipeId::SpotlessState, theme_catalog, input)?
        else {
            unreachable!("fixed recipe loader returned the wrong variant")
        };
        let renderer = c6_renderer();
        let document = render_c6_document(&renderer, source, &theme, portable_svg_request())?;
        let identity = prove_portable_family_evidence(
            document.evidence(),
            &theme,
            DiagramFamilyId::STATE,
            FamilyEvidenceRequirements::SPOTLESS_STATE,
        )?;
        (document, identity, expected)
    };
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_spotless_state_svg(expected, artifact)?;
    Ok(SpotlessStateRenderedDocument {
        expected,
        document,
        identity,
        svg_proof,
    })
}

fn prove_cyberpunk_state_rendered_document(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<CyberpunkStateRenderedDocument> {
    let (document, identity, expected) = {
        let FixedC6Recipe::CyberpunkState { theme, expected } =
            load_fixed_c6_recipe(ReferenceRecipeId::CyberpunkState, theme_catalog, input)?
        else {
            unreachable!("fixed recipe loader returned the wrong variant")
        };
        let renderer = c6_renderer();
        let document = render_c6_document(&renderer, source, &theme, portable_svg_request())?;
        let identity = prove_portable_family_evidence(
            document.evidence(),
            &theme,
            DiagramFamilyId::STATE,
            FamilyEvidenceRequirements::CYBERPUNK_STATE,
        )?;
        (document, identity, expected)
    };
    let native_filter = merman::__theme_acceptance::native_filter_receipt(document.evidence())
        .ok_or_else(|| {
            C6ProofError::new(
                "cyberpunk-state-native-filter",
                "production State evidence lacks a native-filter receipt",
            )
        })?;
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_cyberpunk_state_svg(expected, artifact, native_filter)?;
    Ok(CyberpunkStateRenderedDocument {
        expected,
        document,
        identity,
        svg_proof,
    })
}

fn load_fixed_c6_recipe(
    id: ReferenceRecipeId,
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
) -> C6ProofResult<FixedC6Recipe> {
    c6_ensure!(
        "fixture-contract",
        input.fixture_id() == id.fixture_id(),
        "{} recipe received fixture `{}`",
        id.label(),
        input.fixture_id()
    );
    let font = resolve_fixed_recipe_font(theme_catalog, input, id.label())?;
    match id {
        ReferenceRecipeId::SpotlessFlowchart => {
            validate_spotless_flowchart_input(input)?;
            build_spotless_flowchart_recipe(font)
        }
        ReferenceRecipeId::SpotlessState => {
            validate_spotless_state_input(input)?;
            build_spotless_state_recipe(font)
        }
        ReferenceRecipeId::CyberpunkState => {
            validate_cyberpunk_state_input(input)?;
            build_cyberpunk_state_recipe(font)
        }
    }
}

fn resolve_fixed_recipe_font(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    label: &'static str,
) -> C6ProofResult<FixedRecipeFont> {
    let typography = input.typography().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            format!("{label} requires typed typography"),
        )
    })?;
    let fixture_stack = typography.font_stack().ok_or_else(|| {
        C6ProofError::new("fixture-contract", format!("{label} requires a font stack"))
    })?;
    c6_ensure!(
        "fixture-contract",
        fixture_stack.binding() == ReferenceFontBinding::FixtureAssets
            && fixture_stack.families().len() == 1
            && fixture_stack.families()[0] == RECIPE_FONT_FAMILY
            && fixture_stack.asset_ids().len() == 1
            && fixture_stack.asset_ids().contains(RECIPE_FONT_ASSET_ID),
        "{label} must use the fixed embedded `{RECIPE_FONT_FAMILY}` recipe"
    );
    let asset = theme_catalog
        .font_asset(RECIPE_FONT_ASSET_ID)
        .ok_or_else(|| {
            C6ProofError::new(
                "fixture-font-asset",
                format!("font asset `{RECIPE_FONT_ASSET_ID}` does not exist"),
            )
        })?;
    c6_ensure!(
        "fixture-font-asset",
        asset.family() == RECIPE_FONT_FAMILY,
        "font asset family `{}` differs from fixed recipe family `{RECIPE_FONT_FAMILY}`",
        asset.family()
    );
    let bytes = theme_catalog
        .asset_bytes(RECIPE_FONT_ASSET_ID)
        .map_err(|error| C6ProofError::new("fixture-font-bytes", error.to_string()))?;
    let stack = FontStack::single(RECIPE_FONT_FAMILY)
        .map_err(|error| C6ProofError::new("fixture-font-stack", error.to_string()))?;
    let font_catalog = FontCatalogSpec::new([FontAssetSpec::new(asset.id(), bytes)])
        .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
    Ok(FixedRecipeFont {
        stack,
        assets: ThemeAssets::default().with_font_catalog(font_catalog),
    })
}

fn validate_spotless_flowchart_input(input: &ReferenceThemeInput) -> C6ProofResult<()> {
    let typography = input
        .typography()
        .expect("fixed recipe font validation requires typography");
    let canvas_matches = matches!(
        input.canvas(),
        [ReferenceCanvasLayer::LinearGradient {
            angle_degrees: 45,
            repetition: ReferenceGradientRepetition::Repeating,
            tile_width_px: Some(SPOTLESS_FLOWCHART_PATTERN_PERIOD),
            tile_height_px: Some(SPOTLESS_FLOWCHART_PATTERN_PERIOD),
            stops,
        }] if stops_match(stops, &SPOTLESS_FLOWCHART_STRIPE_STOPS)
    );
    let node_style = input.node_style().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Spotless Flowchart requires node styling",
        )
    })?;
    let border = node_style.border().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Spotless Flowchart requires a node border",
        )
    })?;
    c6_ensure!(
        "fixture-contract",
        input.tokens().is_none()
            && typography.letter_spacing_milli_em().is_none()
            && typography.text_transform().is_none()
            && canvas_matches
            && border.color() == SPOTLESS_FLOWCHART_STROKE
            && border.width_px() == SPOTLESS_FLOWCHART_STROKE_WIDTH
            && node_style.dash_pattern() == SPOTLESS_FLOWCHART_DASH_PATTERN
            && node_style.corner_radius_px() == Some(SPOTLESS_FLOWCHART_RADIUS)
            && node_style.shadow().is_none()
            && input.semantic_rules().is_empty(),
        "Spotless Flowchart fixture differs from its fixed C6 recipe"
    );
    Ok(())
}

fn validate_spotless_state_input(input: &ReferenceThemeInput) -> C6ProofResult<()> {
    let tokens = input.tokens().ok_or_else(|| {
        C6ProofError::new("fixture-contract", "Spotless State requires typed tokens")
    })?;
    let typography = input
        .typography()
        .expect("fixed recipe font validation requires typography");
    let canvas_matches = matches!(
        input.canvas(),
        [
            ReferenceCanvasLayer::Solid { color },
            ReferenceCanvasLayer::LinearGradient {
                angle_degrees: 45,
                repetition: ReferenceGradientRepetition::Repeating,
                tile_width_px: Some(SPOTLESS_STATE_PATTERN_PERIOD),
                tile_height_px: Some(SPOTLESS_STATE_PATTERN_PERIOD),
                stops: linear_stops,
            },
            ReferenceCanvasLayer::RadialGradient {
                center_x_percent: 50,
                center_y_percent: 50,
                repetition: ReferenceGradientRepetition::Tiled,
                tile_width_px: Some(SPOTLESS_STATE_PATTERN_PERIOD),
                tile_height_px: Some(SPOTLESS_STATE_PATTERN_PERIOD),
                stops: radial_stops,
            },
        ] if color.as_str() == SPOTLESS_STATE_CANVAS
            && stops_match(linear_stops, &SPOTLESS_STATE_LINEAR_STOPS)
            && stops_match(radial_stops, &SPOTLESS_STATE_RADIAL_STOPS)
    );
    c6_ensure!(
        "fixture-contract",
        tokens.background() == SPOTLESS_STATE_CANVAS
            && tokens.surface() == SPOTLESS_STATE_SURFACE
            && tokens.primary() == SPOTLESS_STATE_PRIMARY
            && tokens.text() == SPOTLESS_STATE_TEXT
            && typography.letter_spacing_milli_em() == Some(SPOTLESS_STATE_LETTER_SPACING_MILLI_EM)
            && typography.text_transform() == Some(ReferenceTextTransform::Uppercase)
            && canvas_matches
            && input.node_style().is_none()
            && input.semantic_rules().is_empty(),
        "Spotless State fixture differs from its fixed C6 recipe"
    );
    Ok(())
}

fn validate_cyberpunk_state_input(input: &ReferenceThemeInput) -> C6ProofResult<()> {
    let tokens = input.tokens().ok_or_else(|| {
        C6ProofError::new("fixture-contract", "Cyberpunk State requires typed tokens")
    })?;
    let typography = input
        .typography()
        .expect("fixed recipe font validation requires typography");
    let canvas_matches = matches!(
        input.canvas(),
        [ReferenceCanvasLayer::Solid { color }] if color.as_str() == CYBERPUNK_STATE_CANVAS
    );
    let node_style = input.node_style().ok_or_else(|| {
        C6ProofError::new("fixture-contract", "Cyberpunk State requires node styling")
    })?;
    let border = node_style.border().ok_or_else(|| {
        C6ProofError::new("fixture-contract", "Cyberpunk State requires a node border")
    })?;
    let shadow = node_style.shadow().ok_or_else(|| {
        C6ProofError::new("fixture-contract", "Cyberpunk State requires a soft glow")
    })?;
    c6_ensure!(
        "fixture-contract",
        tokens.background() == CYBERPUNK_STATE_CANVAS
            && tokens.surface() == CYBERPUNK_STATE_SURFACE
            && tokens.primary() == CYBERPUNK_STATE_PRIMARY
            && tokens.text() == CYBERPUNK_STATE_TEXT
            && typography.letter_spacing_milli_em().is_none()
            && typography.text_transform().is_none()
            && canvas_matches
            && border.color() == CYBERPUNK_STATE_PRIMARY
            && border.width_px() == CYBERPUNK_STATE_BORDER_WIDTH
            && node_style.dash_pattern().is_empty()
            && node_style.corner_radius_px() == Some(CYBERPUNK_STATE_RADIUS)
            && shadow.offset_x_px() == 0
            && shadow.offset_y_px() == 0
            && shadow.blur_px() == CYBERPUNK_STATE_GLOW_BLUR
            && shadow.spread_px() == 0
            && shadow.color() == CYBERPUNK_STATE_PRIMARY
            && input.semantic_rules().is_empty(),
        "Cyberpunk State fixture differs from its fixed C6 recipe"
    );
    Ok(())
}

fn stops_match(stops: &[ReferenceGradientStop], expected: &[(u8, &str); 4]) -> bool {
    stops.len() == expected.len()
        && stops.iter().zip(expected).all(|(stop, (offset, color))| {
            stop.offset_percent() == *offset && stop.color() == *color
        })
}

fn build_spotless_flowchart_recipe(font: FixedRecipeFont) -> C6ProofResult<FixedC6Recipe> {
    let mut node_patch = ThemeStylePatch::default()
        .with_stroke(fixture_solid(SPOTLESS_FLOWCHART_STROKE, "fixture-border")?)
        .with_stroke_width(f32::from(SPOTLESS_FLOWCHART_STROKE_WIDTH))
        .map_err(|error| C6ProofError::new("fixture-border", error.to_string()))?
        .with_stroke_dasharray(SPOTLESS_FLOWCHART_DASH_PATTERN.into_iter().map(f32::from))
        .map_err(|error| C6ProofError::new("fixture-dasharray", error.to_string()))?;
    node_patch.geometry.radius = Specified::Value(f32::from(SPOTLESS_FLOWCHART_RADIUS));
    let mut label_patch = ThemeStylePatch::default();
    label_patch.typography.font_stack = Specified::Value(font.stack);
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(ThemeTarget::Node, node_patch).for_family(DiagramFamilyId::FLOWCHART),
        )
        .with_rule(
            ThemeRule::new(ThemeTarget::NodeLabel, label_patch)
                .for_family(DiagramFamilyId::FLOWCHART),
        );
    let spec = DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(ThemeTextStyle::default()))
        .with_assets(font.assets)
        .with_canvas(spotless_flowchart_canvas()?)
        .with_styles(styles);
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .map_err(|error| C6ProofError::new("spotless-flowchart-theme", error.to_string()))?;
    Ok(FixedC6Recipe::SpotlessFlowchart {
        theme,
        expected: SpotlessFlowchartProofContract {
            stripe_colors: [
                SPOTLESS_FLOWCHART_STRIPE_STOPS[0].1,
                SPOTLESS_FLOWCHART_STRIPE_STOPS[2].1,
            ],
            stop_offsets: SPOTLESS_FLOWCHART_STRIPE_STOPS.map(|(offset, _)| f64::from(offset)),
            period_px: f64::from(SPOTLESS_FLOWCHART_PATTERN_PERIOD),
            stroke: SPOTLESS_FLOWCHART_STROKE,
            stroke_width: f64::from(SPOTLESS_FLOWCHART_STROKE_WIDTH),
            dash_pattern: &SPOTLESS_FLOWCHART_DASH_PATTERN,
            radius: f64::from(SPOTLESS_FLOWCHART_RADIUS),
            font_family: RECIPE_FONT_FAMILY,
        },
    })
}

fn build_spotless_state_recipe(font: FixedRecipeFont) -> C6ProofResult<FixedC6Recipe> {
    let default_text = ThemeTextStyle::default().with_font_stack(font.stack);
    let letter_spacing_px =
        default_text.font_size_px() * f32::from(SPOTLESS_STATE_LETTER_SPACING_MILLI_EM) / 1000.0;
    let default_text = default_text
        .with_letter_spacing_px(letter_spacing_px)
        .map_err(|error| C6ProofError::new("fixture-typography", error.to_string()))?
        .with_transform(TextTransform::Uppercase);
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(
                ThemeTarget::State,
                ThemeStylePatch::default()
                    .with_fill(fixture_solid(SPOTLESS_STATE_SURFACE, "fixture-state-fill")?),
            )
            .for_family(DiagramFamilyId::STATE),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::StateLabel,
                ThemeStylePatch::default()
                    .with_fill(fixture_solid(SPOTLESS_STATE_TEXT, "fixture-text-color")?),
            )
            .for_family(DiagramFamilyId::STATE),
        );
    let spec = DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(default_text))
        .with_assets(font.assets)
        .with_canvas(spotless_state_canvas()?)
        .with_styles(styles);
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .map_err(|error| C6ProofError::new("spotless-state-theme", error.to_string()))?;
    Ok(FixedC6Recipe::SpotlessState {
        theme,
        expected: SpotlessStateProofContract {
            canvas: SPOTLESS_STATE_CANVAS,
            surface: SPOTLESS_STATE_SURFACE,
            text: SPOTLESS_STATE_TEXT,
            font_family: RECIPE_FONT_FAMILY,
            letter_spacing_px: f64::from(letter_spacing_px),
            overlay_period_px: f64::from(SPOTLESS_STATE_PATTERN_PERIOD),
            linear_stops: SPOTLESS_STATE_LINEAR_STOPS
                .map(|(offset, color)| (f64::from(offset), color)),
            radial_stops: SPOTLESS_STATE_RADIAL_STOPS
                .map(|(offset, color)| (f64::from(offset), color)),
        },
    })
}

fn build_cyberpunk_state_recipe(font: FixedRecipeFont) -> C6ProofResult<FixedC6Recipe> {
    let mut state_patch = ThemeStylePatch::default()
        .with_fill(fixture_solid(
            CYBERPUNK_STATE_SURFACE,
            "fixture-state-fill",
        )?)
        .with_stroke(fixture_solid(CYBERPUNK_STATE_PRIMARY, "fixture-border")?)
        .with_stroke_width(f32::from(CYBERPUNK_STATE_BORDER_WIDTH))
        .map_err(|error| C6ProofError::new("fixture-border", error.to_string()))?;
    state_patch.geometry.radius = Specified::Value(f32::from(CYBERPUNK_STATE_RADIUS));
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(ThemeTarget::State, state_patch).for_family(DiagramFamilyId::STATE),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::StateLabel,
                ThemeStylePatch::default()
                    .with_fill(fixture_solid(CYBERPUNK_STATE_TEXT, "fixture-text-color")?),
            )
            .for_family(DiagramFamilyId::STATE),
        );
    let graph = EffectGraph::new(
        CYBERPUNK_STATE_EFFECT_ID,
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: f32::from(CYBERPUNK_STATE_GLOW_BLUR),
            spread: 0.0,
            color: ThemeColorValue::parse(CYBERPUNK_STATE_PRIMARY)
                .map_err(|error| C6ProofError::new("fixture-shadow", error.to_string()))?,
        }],
    )
    .map_err(|error| C6ProofError::new("fixture-shadow-graph", error.to_string()))?;
    let effects = DiagramEffectSet::default()
        .with_graph(graph)
        .map_err(|error| C6ProofError::new("fixture-shadow-graph", error.to_string()))?
        .with_binding(
            EffectBinding::new(ThemeTarget::State, CYBERPUNK_STATE_EFFECT_ID)
                .map_err(|error| C6ProofError::new("fixture-effect-binding", error.to_string()))?,
        )
        .map_err(|error| C6ProofError::new("fixture-effect-binding", error.to_string()))?;
    let default_text = ThemeTextStyle::default().with_font_stack(font.stack);
    let spec = DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(default_text))
        .with_assets(font.assets)
        .with_canvas(
            CanvasSpec::solid(CYBERPUNK_STATE_CANVAS)
                .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?,
        )
        .with_styles(styles)
        .with_effects(effects);
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .map_err(|error| C6ProofError::new("cyberpunk-state-theme", error.to_string()))?;
    Ok(FixedC6Recipe::CyberpunkState {
        theme,
        expected: CyberpunkStateProofContract {
            canvas: CYBERPUNK_STATE_CANVAS,
            surface: CYBERPUNK_STATE_SURFACE,
            primary: CYBERPUNK_STATE_PRIMARY,
            text: CYBERPUNK_STATE_TEXT,
            border_width: f64::from(CYBERPUNK_STATE_BORDER_WIDTH),
            radius: f64::from(CYBERPUNK_STATE_RADIUS),
            glow_blur: f64::from(CYBERPUNK_STATE_GLOW_BLUR),
            font_family: RECIPE_FONT_FAMILY,
        },
    })
}

fn spotless_flowchart_canvas() -> C6ProofResult<CanvasSpec> {
    let gradient = LinearGradient::new(
        45.0,
        fixed_gradient_stops(&SPOTLESS_FLOWCHART_STRIPE_STOPS)?,
    )
    .and_then(|gradient| {
        gradient.with_repeating_period_px(f32::from(SPOTLESS_FLOWCHART_PATTERN_PERIOD))
    })
    .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?;
    Ok(CanvasSpec::default().with_base(CanvasPaint::LinearGradient(gradient)))
}

fn spotless_state_canvas() -> C6ProofResult<CanvasSpec> {
    let linear = LinearGradient::new(45.0, fixed_gradient_stops(&SPOTLESS_STATE_LINEAR_STOPS)?)
        .and_then(|gradient| {
            gradient.with_repeating_period_px(f32::from(SPOTLESS_STATE_PATTERN_PERIOD))
        })
        .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?;
    let radial = RadialGradient::new(
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        ThemeLength::percent(50.0),
        fixed_gradient_stops(&SPOTLESS_STATE_RADIAL_STOPS)?,
    )
    .and_then(|gradient| {
        gradient.with_tile_px(
            f32::from(SPOTLESS_STATE_PATTERN_PERIOD),
            f32::from(SPOTLESS_STATE_PATTERN_PERIOD),
        )
    })
    .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?;
    CanvasSpec::default()
        .with_base(fixture_solid(SPOTLESS_STATE_CANVAS, "fixture-canvas")?)
        .with_layer(CanvasLayer::new(CanvasPaint::LinearGradient(linear)))
        .and_then(|canvas| canvas.with_layer(CanvasLayer::new(CanvasPaint::RadialGradient(radial))))
        .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))
}

fn fixed_gradient_stops(stops: &[(u8, &str); 4]) -> C6ProofResult<Vec<GradientStop>> {
    stops
        .iter()
        .map(|(offset, color)| {
            GradientStop::new(
                f32::from(*offset) / 100.0,
                ThemeColorValue::parse(color)
                    .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?,
            )
            .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))
        })
        .collect()
}
