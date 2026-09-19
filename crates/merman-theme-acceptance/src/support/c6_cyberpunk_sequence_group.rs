use std::collections::BTreeSet;

use merman::svg::{
    CanvasSpec, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec,
    FontCatalogSpec, FontEmbeddingRequirement, FontStack, Specified, ThemeAssets, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman::{DiagramFamilyId, RenderedDocument};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget, ReferenceCanvasLayer,
    ReferenceFontBinding, ReferenceThemeInput, ReferenceThemeMechanism, ThemeFixtureCatalog,
};

use crate::observation::{C6RenderIdentity, C6RuntimeError};

use super::c6_sequence_proof::{
    SequenceLineProofStyle, SequenceProofContract, SequenceRectProofStyle,
    SequenceRoleTextProofContract, SequenceSvgProof, prove_sequence_png,
    prove_sequence_svg_with_role_text,
};
use super::{
    C6CompletedRenderGroup, C6ProofError, C6ProofResult, C6RenderGroupAdapter, C6RenderGroupWork,
    FamilyEvidenceRequirements, bind_document_svg_artifact, brutalist_solid as fixture_solid,
    c6_renderer, complete_c6_svg_png_render_group, load_render_group_fixture, portable_svg_request,
    prove_portable_family_evidence, prove_render_group,
    render_brutalist_document as render_c6_document,
};

const FIXTURE_ID: &str = "fixture-c6-cyberpunk-sequence";
const FONT_FAMILY: &str = "Excalifont";
const FONT_ASSET_ID: &str = "font-excalifont-latin";
const CANVAS: &str = "#020617";
const SURFACE: &str = "#0f172a";
const PRIMARY: &str = "#22d3ee";
const TEXT: &str = "#e0f2fe";
const LIFELINE_WIDTH: u16 = 2;
const CELL_MECHANISMS: [ReferenceThemeMechanism; 4] = [
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];

pub(super) const CYBERPUNK_SEQUENCE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Cyberpunk,
    family: C6ProofFamily::Sequence,
    source_fixture_id: FIXTURE_ID,
    proof_recipe_revision: "cyberpunk-sequence-v1",
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_cyberpunk_sequence_group,
};

fn execute_cyberpunk_sequence_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_cyberpunk_sequence_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_sequence_png(
                rendered.contract,
                &rendered.svg_proof,
                "cyberpunk-sequence-png-v1",
                artifact,
                plan,
            )
        },
    )
}

struct CyberpunkSequenceRenderedDocument {
    contract: SequenceProofContract<'static>,
    document: RenderedDocument,
    identity: C6RenderIdentity,
    svg_proof: SequenceSvgProof,
}

fn prove_cyberpunk_sequence_rendered_document(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<CyberpunkSequenceRenderedDocument> {
    validate_input(input)?;
    let theme = compile_theme(theme_catalog, input)?;
    let renderer = c6_renderer();
    let document = render_c6_document(&renderer, source, &theme, portable_svg_request())?;
    let identity = prove_portable_family_evidence(
        document.evidence(),
        &theme,
        DiagramFamilyId::SEQUENCE,
        FamilyEvidenceRequirements::BRUTALIST_SEQUENCE,
    )?;
    let contract = proof_contract();
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_sequence_svg_with_role_text(
        contract,
        role_text_contract(),
        &CELL_MECHANISMS,
        "cyberpunk-sequence-standalone-svg-v1",
        artifact,
    )?;
    Ok(CyberpunkSequenceRenderedDocument {
        contract,
        document,
        identity,
        svg_proof,
    })
}

fn validate_input(input: &ReferenceThemeInput) -> C6ProofResult<()> {
    let tokens = input.tokens().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Sequence requires typed tokens",
        )
    })?;
    let typography = input.typography().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Sequence requires typed typography",
        )
    })?;
    let font_stack = typography.font_stack().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Sequence requires a font stack",
        )
    })?;
    let node_style = input.node_style().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Sequence requires lifeline stroke styling",
        )
    })?;
    let border = node_style.border().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Cyberpunk Sequence requires a lifeline border",
        )
    })?;
    let expected_mechanisms = CELL_MECHANISMS.into_iter().collect::<BTreeSet<_>>();
    let canvas_matches = matches!(
        input.canvas(),
        [ReferenceCanvasLayer::Solid { color }] if color.as_str() == CANVAS
    );
    c6_ensure!(
        "fixture-contract",
        input.fixture_id() == FIXTURE_ID
            && tokens.background() == CANVAS
            && tokens.surface() == SURFACE
            && tokens.primary() == PRIMARY
            && tokens.text() == TEXT
            && font_stack.binding() == ReferenceFontBinding::FixtureAssets
            && font_stack.families() == [FONT_FAMILY]
            && font_stack.asset_ids().len() == 1
            && font_stack.asset_ids().contains(FONT_ASSET_ID)
            && typography.letter_spacing_milli_em().is_none()
            && typography.text_transform().is_none()
            && canvas_matches
            && border.color() == PRIMARY
            && border.width_px() == LIFELINE_WIDTH
            && node_style.dash_pattern().is_empty()
            && node_style.corner_radius_px().is_none()
            && node_style.shadow().is_none()
            && input.semantic_rules().is_empty()
            && input.mechanisms() == expected_mechanisms,
        "Cyberpunk Sequence fixture differs from its fixed C6 recipe"
    );
    Ok(())
}

fn compile_theme(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
) -> C6ProofResult<DiagramTheme> {
    let (font_stack, assets) = resolve_font(theme_catalog, input)?;
    let actor = ThemeStylePatch::default()
        .with_fill(fixture_solid(PRIMARY, "cyberpunk-sequence-actor-fill")?)
        .with_stroke(fixture_solid(TEXT, "cyberpunk-sequence-actor-stroke")?);
    let lifeline = ThemeStylePatch::default()
        .with_stroke(fixture_solid(
            PRIMARY,
            "cyberpunk-sequence-lifeline-stroke",
        )?)
        .with_stroke_width(f32::from(LIFELINE_WIDTH))
        .map_err(|error| {
            C6ProofError::new("cyberpunk-sequence-lifeline-width", error.to_string())
        })?;
    let message = ThemeStylePatch::default()
        .with_stroke(fixture_solid(PRIMARY, "cyberpunk-sequence-message-stroke")?);
    let note = ThemeStylePatch::default()
        .with_fill(fixture_solid(SURFACE, "cyberpunk-sequence-note-fill")?)
        .with_stroke(fixture_solid(PRIMARY, "cyberpunk-sequence-note-stroke")?);
    let activation = ThemeStylePatch::default()
        .with_fill(fixture_solid(
            SURFACE,
            "cyberpunk-sequence-activation-fill",
        )?)
        .with_stroke(fixture_solid(
            PRIMARY,
            "cyberpunk-sequence-activation-stroke",
        )?);
    let loop_surface = ThemeStylePatch::default()
        .with_fill(fixture_solid(SURFACE, "cyberpunk-sequence-loop-fill")?);
    let actor_label = label_patch(&font_stack, CANVAS, "cyberpunk-sequence-actor-label-fill")?;
    let message_label = label_patch(&font_stack, TEXT, "cyberpunk-sequence-message-label-fill")?;
    let note_label = label_patch(&font_stack, TEXT, "cyberpunk-sequence-note-label-fill")?;
    let loop_label = label_patch(&font_stack, TEXT, "cyberpunk-sequence-loop-label-fill")?;
    let styles = ThemeRuleSet::default()
        .with_rule(sequence_rule(ThemeTarget::Actor, actor))
        .with_rule(sequence_rule(ThemeTarget::Lifeline, lifeline))
        .with_rule(sequence_rule(ThemeTarget::Message, message))
        .with_rule(sequence_rule(ThemeTarget::Note, note))
        .with_rule(sequence_rule(ThemeTarget::Activation, activation))
        .with_rule(sequence_rule(
            ThemeTarget::LoopLabelBackground,
            loop_surface,
        ))
        .with_rule(sequence_rule(ThemeTarget::ActorLabel, actor_label))
        .with_rule(sequence_rule(ThemeTarget::MessageLabel, message_label))
        .with_rule(sequence_rule(ThemeTarget::NoteLabel, note_label))
        .with_rule(sequence_rule(ThemeTarget::LoopLabel, loop_label));
    let spec =
        DiagramThemeSpec::new()
            .with_typography(TypographySpec::default().with_default(ThemeTextStyle::default()))
            .with_assets(assets)
            .with_canvas(CanvasSpec::solid(CANVAS).map_err(|error| {
                C6ProofError::new("cyberpunk-sequence-canvas", error.to_string())
            })?)
            .with_styles(styles);
    DiagramThemeCompiler::new()
        .compile(spec)
        .map_err(|error| C6ProofError::new("cyberpunk-sequence-theme", error.to_string()))
}

fn label_patch(
    font_stack: &FontStack,
    fill: &str,
    context: &'static str,
) -> C6ProofResult<ThemeStylePatch> {
    let mut patch = ThemeStylePatch::default().with_fill(fixture_solid(fill, context)?);
    patch.typography.font_stack = Specified::Value(font_stack.clone());
    Ok(patch)
}

fn sequence_rule(target: ThemeTarget, patch: ThemeStylePatch) -> ThemeRule {
    ThemeRule::new(target, patch).for_family(DiagramFamilyId::SEQUENCE)
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
        "Cyberpunk Sequence fixture lost its font binding"
    );
    Ok((stack, ThemeAssets::default().with_font_catalog(catalog)))
}

const fn proof_contract() -> SequenceProofContract<'static> {
    SequenceProofContract {
        actor: SequenceRectProofStyle {
            fill: PRIMARY,
            stroke: TEXT,
        },
        actor_rect_count: 4,
        require_actor_man: false,
        lifeline: Some(SequenceLineProofStyle {
            stroke: PRIMARY,
            width_px: Some(LIFELINE_WIDTH as f64),
        }),
        message_stroke: PRIMARY,
        message_line_counts: [1, 1],
        message_text_count: 2,
        note: Some(SequenceRectProofStyle {
            fill: SURFACE,
            stroke: PRIMARY,
        }),
        activation: Some(SequenceRectProofStyle {
            fill: SURFACE,
            stroke: PRIMARY,
        }),
        font_family: Some(FONT_FAMILY),
    }
}

const fn role_text_contract() -> SequenceRoleTextProofContract<'static> {
    SequenceRoleTextProofContract {
        actor_text: CANVAS,
        message_text: TEXT,
        note_text: TEXT,
        loop_text: TEXT,
        canvas: CANVAS,
        loop_label_surface: SURFACE,
    }
}
