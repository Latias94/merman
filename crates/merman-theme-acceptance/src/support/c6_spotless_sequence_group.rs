use std::collections::BTreeSet;

use merman::svg::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec, FontCatalogSpec,
    FontEmbeddingRequirement, FontStack, Specified, ThemeAssets, ThemeCapability, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman::{DiagramFamilyId, RenderedDocument};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget, ReferenceFontBinding,
    ReferenceThemeInput, ReferenceThemeMechanism, ThemeFixtureCatalog,
};

use crate::observation::{C6RenderIdentity, C6RuntimeError};

use super::c6_sequence_proof::{
    SequenceLineProofStyle, SequenceProofContract, SequenceRectProofStyle, SequenceSvgProof,
    prove_sequence_png, prove_sequence_svg,
};
use super::{
    C6CompletedRenderGroup, C6ProofError, C6ProofResult, C6RenderGroupAdapter, C6RenderGroupWork,
    FamilyEvidenceRequirements, bind_document_svg_artifact, brutalist_solid as fixture_solid,
    c6_renderer, complete_c6_svg_png_render_group, load_render_group_fixture, portable_svg_request,
    prove_portable_family_evidence, prove_render_group,
    render_brutalist_document as render_c6_document,
};

const FIXTURE_ID: &str = "fixture-c6-spotless-sequence";
const FONT_FAMILY: &str = "Excalifont";
const FONT_ASSET_ID: &str = "font-excalifont-latin";
const BACKGROUND: &str = "#ede8dc";
const SURFACE: &str = "#f5f1e8";
const PRIMARY: &str = "#2c2416";
const TEXT: &str = "#1a1a1a";
const LIFELINE_WIDTH: u16 = 2;
const CELL_MECHANISMS: [ReferenceThemeMechanism; 3] = [
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];
const FAMILY_EVIDENCE_REQUIREMENTS: FamilyEvidenceRequirements = FamilyEvidenceRequirements {
    require_prepared_text: true,
    require_root_theme: false,
};

pub(super) const SPOTLESS_SEQUENCE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Spotless,
    family: C6ProofFamily::Sequence,
    source_fixture_id: FIXTURE_ID,
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_spotless_sequence_group,
};

fn execute_spotless_sequence_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_spotless_sequence_rendered_document(theme_catalog, input, &source),
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
                "spotless-sequence-png-v1",
                artifact,
                plan,
            )
        },
    )
}

struct SpotlessSequenceRenderedDocument {
    contract: SequenceProofContract<'static>,
    document: RenderedDocument,
    identity: C6RenderIdentity,
    svg_proof: SequenceSvgProof,
}

fn prove_spotless_sequence_rendered_document(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<SpotlessSequenceRenderedDocument> {
    validate_input(input)?;
    let theme = compile_theme(theme_catalog)?;
    let renderer = c6_renderer();
    let document = render_c6_document(&renderer, source, &theme, portable_svg_request())?;
    let identity = prove_portable_family_evidence(
        document.evidence(),
        &theme,
        DiagramFamilyId::SEQUENCE,
        FAMILY_EVIDENCE_REQUIREMENTS,
    )?;
    let contract = proof_contract();
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_sequence_svg(
        contract,
        &CELL_MECHANISMS,
        "spotless-sequence-standalone-svg-v1",
        artifact,
    )?;
    Ok(SpotlessSequenceRenderedDocument {
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
            "Spotless Sequence requires typed tokens",
        )
    })?;
    let typography = input.typography().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Spotless Sequence requires typed typography",
        )
    })?;
    let font_stack = typography.font_stack().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Spotless Sequence requires a font stack",
        )
    })?;
    let node_style = input.node_style().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Spotless Sequence requires lifeline stroke styling",
        )
    })?;
    let border = node_style.border().ok_or_else(|| {
        C6ProofError::new(
            "fixture-contract",
            "Spotless Sequence requires a lifeline border",
        )
    })?;
    let expected_mechanisms = CELL_MECHANISMS.into_iter().collect::<BTreeSet<_>>();
    c6_ensure!(
        "fixture-contract",
        input.fixture_id() == FIXTURE_ID
            && tokens.background() == BACKGROUND
            && tokens.surface() == SURFACE
            && tokens.primary() == PRIMARY
            && tokens.text() == TEXT
            && font_stack.binding() == ReferenceFontBinding::FixtureAssets
            && font_stack.families() == [FONT_FAMILY]
            && font_stack.asset_ids().len() == 1
            && font_stack.asset_ids().contains(FONT_ASSET_ID)
            && typography.letter_spacing_milli_em().is_none()
            && typography.text_transform().is_none()
            && input.canvas().is_empty()
            && border.color() == PRIMARY
            && border.width_px() == LIFELINE_WIDTH
            && node_style.dash_pattern().is_empty()
            && node_style.corner_radius_px().is_none()
            && node_style.shadow().is_none()
            && input.semantic_rules().is_empty()
            && input.mechanisms() == expected_mechanisms,
        "Spotless Sequence fixture differs from its fixed C6 recipe"
    );
    Ok(())
}

fn compile_theme(theme_catalog: &ThemeFixtureCatalog) -> C6ProofResult<DiagramTheme> {
    let (font_stack, assets) = resolve_font(theme_catalog)?;
    let actor = ThemeStylePatch::default()
        .with_fill(fixture_solid(SURFACE, "spotless-sequence-actor-fill")?)
        .with_stroke(fixture_solid(PRIMARY, "spotless-sequence-actor-stroke")?);
    let lifeline = ThemeStylePatch::default()
        .with_stroke(fixture_solid(PRIMARY, "spotless-sequence-lifeline-stroke")?)
        .with_stroke_width(f32::from(LIFELINE_WIDTH))
        .map_err(|error| {
            C6ProofError::new("spotless-sequence-lifeline-width", error.to_string())
        })?;
    let message = ThemeStylePatch::default()
        .with_stroke(fixture_solid(PRIMARY, "spotless-sequence-message-stroke")?);
    let styles = ThemeRuleSet::default()
        .with_rule(sequence_rule(ThemeTarget::Actor, actor))
        .with_rule(sequence_rule(ThemeTarget::Lifeline, lifeline))
        .with_rule(sequence_rule(ThemeTarget::Message, message))
        .with_rule(sequence_rule(
            ThemeTarget::ActorLabel,
            label_patch(&font_stack),
        ))
        .with_rule(sequence_rule(
            ThemeTarget::MessageLabel,
            label_patch(&font_stack),
        ))
        .with_rule(sequence_rule(
            ThemeTarget::NoteLabel,
            label_patch(&font_stack),
        ))
        .with_rule(sequence_rule(
            ThemeTarget::LoopLabel,
            label_patch(&font_stack),
        ));
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_assets(assets)
                .with_styles(styles),
        )
        .map_err(|error| C6ProofError::new("spotless-sequence-theme", error.to_string()))?;
    prove_recipe_capabilities(&theme)?;
    Ok(theme)
}

fn prove_recipe_capabilities(theme: &DiagramTheme) -> C6ProofResult<()> {
    let expected = BTreeSet::from([
        ThemeCapability::SemanticTokens,
        ThemeCapability::Typography,
        ThemeCapability::SemanticRules,
        ThemeCapability::SolidPaint,
        ThemeCapability::BorderStyling,
    ]);
    let actual = theme
        .report()
        .required_capabilities()
        .collect::<BTreeSet<_>>();
    c6_ensure!(
        "spotless-sequence-theme-capabilities",
        actual == expected,
        "Spotless Sequence recipe capabilities differ: expected={expected:?}, actual={actual:?}"
    );
    Ok(())
}

fn label_patch(font_stack: &FontStack) -> ThemeStylePatch {
    let mut patch = ThemeStylePatch::default();
    patch.typography.font_stack = Specified::Value(font_stack.clone());
    patch
}

fn sequence_rule(target: ThemeTarget, patch: ThemeStylePatch) -> ThemeRule {
    ThemeRule::new(target, patch).for_family(DiagramFamilyId::SEQUENCE)
}

fn resolve_font(theme_catalog: &ThemeFixtureCatalog) -> C6ProofResult<(FontStack, ThemeAssets)> {
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
    Ok((stack, ThemeAssets::default().with_font_catalog(catalog)))
}

const fn proof_contract() -> SequenceProofContract<'static> {
    SequenceProofContract {
        actor: SequenceRectProofStyle {
            fill: SURFACE,
            stroke: PRIMARY,
        },
        actor_rect_count: 2,
        require_actor_man: true,
        lifeline: Some(SequenceLineProofStyle {
            stroke: PRIMARY,
            width_px: Some(LIFELINE_WIDTH as f64),
        }),
        message_stroke: PRIMARY,
        message_line_counts: [1, 2],
        message_text_count: 3,
        note: None,
        activation: None,
        font_family: Some(FONT_FAMILY),
    }
}
