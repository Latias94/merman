use std::collections::{BTreeMap, BTreeSet};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use merman::__theme_acceptance::TargetArtifactView;
use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontAssetSpec,
    FontCatalogSpec, FontEmbeddingRequirement, FontSource, FontStack, OrdinalPalette, Specified,
    SvgPipeline, TextMeasurementSource, ThemeAssets, ThemeCapability, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, TypographySpec,
};
use merman::{
    DiagramFamilyId, Engine, MermaidConfig, OperationControl, RenderEvidence, RenderOutput,
    RenderRequest, RenderedDocument, Renderer, SvgEnvironment, SvgRequest, TargetAdmissionReceipt,
    ThemeEvidenceStatus,
};
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman::{RenderArtifactKind, TargetAdmissionStatus, TargetFontSource};
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_export::PdfOptions;
use merman_export::{RasterOptions, RasterPlan};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6EnforcedCell, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget,
    ReferenceBorderInput, ReferenceCanvasLayer, ReferenceDiagramFamily, ReferenceFontBinding,
    ReferenceFontStack, ReferenceSemanticRule, ReferenceSemanticTarget, ReferenceShadowInput,
    ReferenceThemeInput, ReferenceThemeMechanism, ReferenceThemeTokens, ThemeFixtureCatalog,
};

use crate::eligibility::{C6aEligibilityReceipt, issue_c6a_eligibility};
use crate::observation::{
    C6BoundTargetProof, C6CellReceipt, C6EvaluatedLedger, C6ExecutionReport,
    C6ObservedMechanismDisposition, C6ReceiptBook, C6RenderGroupKey, C6RenderGroupReceipt,
    C6RuntimeError, C6TargetArtifact, RouteCutoverRuntimeError, seal_cell_from_evidence,
};

#[derive(Debug, thiserror::Error)]
#[error("{stage}: {detail}")]
pub struct NativeExportSmokeError {
    stage: &'static str,
    detail: String,
}

impl NativeExportSmokeError {
    pub(crate) fn new(stage: &'static str, detail: impl Into<String>) -> Self {
        Self {
            stage,
            detail: detail.into(),
        }
    }

    fn into_runtime(self, key: &C6RenderGroupKey) -> C6RuntimeError {
        C6RuntimeError::RenderGroupProofFailed {
            group: key.label(),
            stage: self.stage,
            detail: self.detail,
        }
    }

    fn into_target_runtime(
        self,
        key: &C6RenderGroupKey,
        artifact_target: ExpectedOutputTarget,
        required_by: &[ExpectedOutputTarget],
    ) -> C6RuntimeError {
        C6RuntimeError::RenderGroupTargetProofFailed {
            group: key.label(),
            artifact_target,
            required_by: required_by.to_vec(),
            stage: self.stage,
            detail: self.detail,
        }
    }

    pub(crate) fn into_route_runtime(self, witness: impl Into<String>) -> RouteCutoverRuntimeError {
        RouteCutoverRuntimeError::ProofFailed {
            witness: witness.into(),
            stage: self.stage,
            detail: self.detail,
        }
    }
}

pub(crate) type C6ProofError = NativeExportSmokeError;
pub(crate) type C6ProofResult<T> = Result<T, C6ProofError>;

pub(super) fn bind_document_svg_artifact(document: &RenderedDocument) -> C6TargetArtifact<'_> {
    C6TargetArtifact::new(TargetArtifactView::from_rendered_document(document))
}

fn bind_raster_artifact(output: &merman::RasterOutput) -> C6TargetArtifact<'_> {
    C6TargetArtifact::new(TargetArtifactView::from_raster_output(output))
}

/// Coarse result for the representative PNG/JPEG/PDF native export smoke.
///
/// The smoke returns only a count after all production target receipts and bounded artifact
/// proofs pass. It intentionally exposes no receipt, artifact, document, or eligibility identity.
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeExportSmokeSummary {
    verified_projection_count: usize,
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
impl NativeExportSmokeSummary {
    /// Returns the number of native projections whose target receipt and artifact proof passed.
    pub const fn verified_projection_count(self) -> usize {
        self.verified_projection_count
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FamilyEvidenceRequirements {
    require_prepared_text: bool,
    require_root_theme: bool,
}

impl FamilyEvidenceRequirements {
    const BRUTALIST_FLOWCHART: Self = Self {
        require_prepared_text: true,
        require_root_theme: true,
    };

    const BRUTALIST_STATE: Self = Self {
        require_prepared_text: true,
        require_root_theme: true,
    };

    const BRUTALIST_SEQUENCE: Self = Self {
        require_prepared_text: true,
        require_root_theme: true,
    };

    const SPOTLESS_FLOWCHART: Self = Self {
        require_prepared_text: true,
        require_root_theme: true,
    };

    const SPOTLESS_STATE: Self = Self {
        require_prepared_text: true,
        require_root_theme: true,
    };

    const CYBERPUNK_STATE: Self = Self {
        require_prepared_text: true,
        require_root_theme: true,
    };

    pub(crate) const ROUTE_CUTOVER: Self = Self {
        require_prepared_text: false,
        require_root_theme: false,
    };
}

fn prove_render_group<T>(
    key: &C6RenderGroupKey,
    result: C6ProofResult<T>,
) -> Result<T, C6RuntimeError> {
    result.map_err(|error| error.into_runtime(key))
}

fn prove_target<T>(
    key: &C6RenderGroupKey,
    artifact_target: ExpectedOutputTarget,
    required_by: &[ExpectedOutputTarget],
    result: C6ProofResult<T>,
) -> Result<T, C6RuntimeError> {
    result.map_err(|error| error.into_target_runtime(key, artifact_target, required_by))
}

#[path = "support/c6_raster_proof.rs"]
mod c6_raster_proof;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[path = "support/c6_pdf_proof.rs"]
mod c6_pdf_proof;

#[path = "support/c6_sequence_proof.rs"]
mod c6_sequence_proof;

#[path = "support/c6_spotless_sequence_group.rs"]
mod c6_spotless_sequence_group;

#[path = "support/c6_flowchart_proof.rs"]
mod c6_flowchart_proof;

#[path = "support/c6_spotless_flowchart_proof.rs"]
mod c6_spotless_flowchart_proof;

#[path = "support/c6_spotless_state_proof.rs"]
mod c6_spotless_state_proof;

#[path = "support/c6_cyberpunk_state_proof.rs"]
mod c6_cyberpunk_state_proof;

#[path = "support/c6_cyberpunk_flowchart_proof.rs"]
mod c6_cyberpunk_flowchart_proof;

#[path = "support/c6_cyberpunk_flowchart_group.rs"]
mod c6_cyberpunk_flowchart_group;

#[path = "support/c6_cyberpunk_sequence_group.rs"]
mod c6_cyberpunk_sequence_group;

#[path = "support/c6_reference_theme_groups.rs"]
mod c6_reference_theme_groups;

use c6_cyberpunk_flowchart_group::CYBERPUNK_FLOWCHART_ADAPTER;
use c6_cyberpunk_sequence_group::CYBERPUNK_SEQUENCE_ADAPTER;
use c6_flowchart_proof::{
    BrutalistFlowchartSvgProof, prove_brutalist_flowchart_png, prove_brutalist_flowchart_svg,
};
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use c6_pdf_proof::prove_brutalist_state_pdf;
use c6_raster_proof::prove_brutalist_state_png;
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use c6_raster_proof::{PngArtifactProof, prove_brutalist_state_jpeg};
pub(crate) use c6_raster_proof::{
    RasterImage as C6RasterImage, decode_bounded_png_artifact,
    decode_bounded_png_artifact_allow_transparent, parse_c6_hex_rgb, parse_c6_svg_view_box,
    transformed_c6_svg_rect,
};
use c6_reference_theme_groups::{
    CYBERPUNK_STATE_ADAPTER, SPOTLESS_FLOWCHART_ADAPTER, SPOTLESS_STATE_ADAPTER,
};
use c6_sequence_proof::{
    BrutalistSequenceSvgProof, prove_brutalist_sequence_png, prove_brutalist_sequence_svg,
};
use c6_spotless_sequence_group::SPOTLESS_SEQUENCE_ADAPTER;

const SHADOW_EFFECT_ID: &str = "c6-brutalist-state-shadow";
const BRUTALIST_FLOWCHART_FIXTURE_ID: &str = "fixture-ordinal-palette";
const BRUTALIST_STATE_FIXTURE_ID: &str = "fixture-c6-brutalist-state";
const BRUTALIST_SEQUENCE_FIXTURE_ID: &str = "fixture-sequence-proof";
const COMMON_BRUTALIST_MECHANISMS: [ReferenceThemeMechanism; 3] = [
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::ThemeVariables,
];
const COMPLETE_BRUTALIST_MECHANISMS: [ReferenceThemeMechanism; 7] = [
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::CssFilter,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];
const BRUTALIST_FLOWCHART_MECHANISMS: [ReferenceThemeMechanism; 6] = [
    ReferenceThemeMechanism::CanvasSolid,
    ReferenceThemeMechanism::FontStack,
    ReferenceThemeMechanism::NthChildSelector,
    ReferenceThemeMechanism::RoundedCorners,
    ReferenceThemeMechanism::StrokeStyling,
    ReferenceThemeMechanism::ThemeVariables,
];

struct BrutalistCommonFixtureContract<'a> {
    tokens: &'a ReferenceThemeTokens,
    font_stack: &'a ReferenceFontStack,
    font_asset_id: &'a str,
    canvas_color: &'a str,
    mechanisms: BTreeSet<ReferenceThemeMechanism>,
}

struct BrutalistFlowchartFixtureContract<'a> {
    common: BrutalistCommonFixtureContract<'a>,
    border: &'a ReferenceBorderInput,
    radius_px: u16,
    palette_colors: &'a [String],
}

struct BrutalistStateFixtureContract<'a> {
    common: BrutalistCommonFixtureContract<'a>,
    border: &'a ReferenceBorderInput,
    radius_px: u16,
    palette_colors: &'a [String],
    shadow: &'a ReferenceShadowInput,
}

impl<'a> BrutalistFlowchartFixtureContract<'a> {
    fn from_input(input: &'a ReferenceThemeInput) -> C6ProofResult<Self> {
        let common = BrutalistCommonFixtureContract::from_input(input)?;
        let node_style = input.node_style().ok_or_else(|| {
            C6ProofError::new(
                "fixture-contract",
                "Brutalist Flowchart requires a node style",
            )
        })?;
        c6_ensure!(
            "fixture-contract",
            node_style.dash_pattern().is_empty(),
            "Brutalist Flowchart node style must not use a dash pattern"
        );
        let border = node_style.border().ok_or_else(|| {
            C6ProofError::new(
                "fixture-contract",
                "Brutalist Flowchart requires a node border",
            )
        })?;
        let radius_px = node_style.corner_radius_px().ok_or_else(|| {
            C6ProofError::new(
                "fixture-contract",
                "Brutalist Flowchart requires rounded nodes",
            )
        })?;
        c6_ensure!(
            "fixture-contract",
            node_style.shadow().is_none(),
            "Brutalist Flowchart does not admit a node shadow"
        );

        let palette_colors = match input.semantic_rules() {
            [
                ReferenceSemanticRule::OrdinalPalette {
                    family,
                    target,
                    colors,
                },
            ] => {
                c6_ensure!(
                    "fixture-contract",
                    *family == ReferenceDiagramFamily::Flowchart,
                    "ordinal palette belongs to {family:?}, not Flowchart"
                );
                c6_ensure!(
                    "fixture-contract",
                    *target == ReferenceSemanticTarget::Node,
                    "ordinal palette targets {target:?}, not nodes"
                );
                colors.as_slice()
            }
            _ => {
                return Err(C6ProofError::new(
                    "fixture-contract",
                    "Brutalist Flowchart requires exactly one node ordinal palette",
                ));
            }
        };
        c6_ensure!(
            "fixture-contract",
            palette_colors.len() == 3,
            "Brutalist Flowchart requires three ordinal colors, got {}",
            palette_colors.len()
        );
        c6_ensure!(
            "fixture-contract",
            palette_colors.first().map(String::as_str) == Some(common.tokens.primary()),
            "first Flowchart ordinal color does not match the primary token"
        );

        let expected_mechanisms = BRUTALIST_FLOWCHART_MECHANISMS.into_iter().collect();
        c6_ensure!(
            "fixture-contract",
            common.mechanisms == expected_mechanisms,
            "Brutalist Flowchart mechanisms differ: expected={expected_mechanisms:?}, actual={:?}",
            common.mechanisms
        );
        Ok(Self {
            common,
            border,
            radius_px,
            palette_colors,
        })
    }
}

type BrutalistSequenceFixtureContract<'a> = BrutalistCommonFixtureContract<'a>;

impl<'a> BrutalistCommonFixtureContract<'a> {
    fn from_input(input: &'a ReferenceThemeInput) -> C6ProofResult<Self> {
        let tokens = input.tokens().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires typed tokens")
        })?;
        let typography = input.typography().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires typed typography")
        })?;
        c6_ensure!(
            "fixture-contract",
            typography.letter_spacing_milli_em().is_none(),
            "Brutalist proof does not admit letter spacing"
        );
        c6_ensure!(
            "fixture-contract",
            typography.text_transform().is_none(),
            "Brutalist proof does not admit text transforms"
        );
        let font_stack = typography.font_stack().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires a font stack")
        })?;
        c6_ensure!(
            "fixture-contract",
            font_stack.binding() == ReferenceFontBinding::FixtureAssets,
            "Brutalist font stack must bind fixture assets"
        );
        c6_ensure!(
            "fixture-contract",
            font_stack.families().len() == 1,
            "Brutalist proof requires exactly one font family, got {}",
            font_stack.families().len()
        );
        c6_ensure!(
            "fixture-contract",
            font_stack.asset_ids().len() == 1,
            "Brutalist proof requires exactly one font asset, got {}",
            font_stack.asset_ids().len()
        );
        let font_asset_id = font_stack.asset_ids().iter().next().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist font asset set is empty")
        })?;

        let canvas_color = match input.canvas() {
            [ReferenceCanvasLayer::Solid { color }] => color.as_str(),
            _ => {
                return Err(C6ProofError::new(
                    "fixture-contract",
                    "Brutalist proof requires exactly one solid canvas layer",
                ));
            }
        };
        c6_ensure!(
            "fixture-contract",
            canvas_color == tokens.background(),
            "canvas color does not match the background token"
        );

        Ok(Self {
            tokens,
            font_stack,
            font_asset_id,
            canvas_color,
            mechanisms: input.mechanisms(),
        })
    }

    fn from_sequence_input(input: &'a ReferenceThemeInput) -> C6ProofResult<Self> {
        let contract = Self::from_input(input)?;
        c6_ensure!(
            "fixture-contract",
            input.node_style().is_none(),
            "Brutalist Sequence does not claim the unsupported node-style contract"
        );
        c6_ensure!(
            "fixture-contract",
            input.semantic_rules().is_empty(),
            "Brutalist Sequence does not admit State ordinal rules"
        );
        let expected_mechanisms = COMMON_BRUTALIST_MECHANISMS.into_iter().collect();
        c6_ensure!(
            "fixture-contract",
            contract.mechanisms == expected_mechanisms,
            "Brutalist Sequence mechanisms differ: expected={expected_mechanisms:?}, actual={:?}",
            contract.mechanisms
        );
        Ok(contract)
    }

    fn actor_fill(&self) -> &str {
        self.tokens.primary()
    }

    fn note_fill(&self) -> &str {
        self.tokens.surface()
    }

    fn stroke(&self) -> &str {
        self.tokens.text()
    }

    fn font_family(&self) -> &str {
        &self.font_stack.families()[0]
    }
}

impl<'a> BrutalistStateFixtureContract<'a> {
    fn from_input(input: &'a ReferenceThemeInput) -> C6ProofResult<Self> {
        let common = BrutalistCommonFixtureContract::from_input(input)?;
        let node_style = input.node_style().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist State requires a node style")
        })?;
        c6_ensure!(
            "fixture-contract",
            node_style.dash_pattern().is_empty(),
            "Brutalist State node style must not use a dash pattern"
        );
        let border = node_style.border().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist State requires a node border")
        })?;
        let radius_px = node_style.corner_radius_px().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist State requires rounded nodes")
        })?;
        let shadow = node_style.shadow().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist State requires a hard shadow")
        })?;
        c6_ensure!(
            "fixture-contract",
            shadow.blur_px() == 0 && shadow.spread_px() == 0,
            "Brutalist proof requires a hard shadow with zero blur and spread"
        );
        c6_ensure!(
            "fixture-contract",
            shadow.offset_x_px() > 0 && shadow.offset_y_px() > 0,
            "Brutalist proof requires positive hard-shadow offsets"
        );

        let palette_colors = match input.semantic_rules() {
            [
                ReferenceSemanticRule::OrdinalPalette {
                    family,
                    target,
                    colors,
                },
            ] => {
                c6_ensure!(
                    "fixture-contract",
                    *family == ReferenceDiagramFamily::StateDiagram,
                    "ordinal palette belongs to {family:?}, not State"
                );
                c6_ensure!(
                    "fixture-contract",
                    *target == ReferenceSemanticTarget::Node,
                    "ordinal palette targets {target:?}, not nodes"
                );
                colors.as_slice()
            }
            _ => {
                return Err(C6ProofError::new(
                    "fixture-contract",
                    "Brutalist proof requires exactly one family node ordinal palette",
                ));
            }
        };
        c6_ensure!(
            "fixture-contract",
            palette_colors.len() == 3,
            "Brutalist proof requires three ordinal colors, got {}",
            palette_colors.len()
        );
        c6_ensure!(
            "fixture-contract",
            palette_colors.first().map(String::as_str) == Some(common.tokens.primary()),
            "first ordinal color does not match the primary token"
        );

        let mechanisms = input.mechanisms();
        let expected_mechanisms = COMPLETE_BRUTALIST_MECHANISMS.into_iter().collect();
        c6_ensure!(
            "fixture-contract",
            mechanisms == expected_mechanisms,
            "Brutalist source mechanisms differ: expected={expected_mechanisms:?}, actual={mechanisms:?}"
        );

        Ok(Self {
            common,
            border,
            radius_px,
            palette_colors,
            shadow,
        })
    }
}

impl<'a> Deref for BrutalistStateFixtureContract<'a> {
    type Target = BrutalistCommonFixtureContract<'a>;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

impl<'a> Deref for BrutalistFlowchartFixtureContract<'a> {
    type Target = BrutalistCommonFixtureContract<'a>;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

/// Runs only the enforced C6 catalog cells and returns their execution summary.
pub fn run_enforced_c6_runtime() -> Result<C6ExecutionReport, C6RuntimeError> {
    run_enforced_c6_runtime_from_root(themes_root())
}

/// Runs the exact 18-cell native ledger and issues the private-harness C6a eligibility seal.
pub fn run_c6a_eligibility() -> Result<C6aEligibilityReceipt, C6RuntimeError> {
    run_c6a_eligibility_from_root(themes_root())
}

/// Runs the exact route-cutover authorization gate independently of the C6 cell gate.
pub fn run_route_cutover_authorization()
-> Result<crate::cutover::RouteCutoverAuthorizationReport, RouteCutoverRuntimeError> {
    crate::cutover::run_route_cutover_witnesses()
}

/// Runs the representative Brutalist State PNG/JPEG/PDF native export smoke.
///
/// All three exports are projected from one completed [`RenderedDocument`]. The smoke consumes the
/// production target-admission receipts directly, requires portable non-zero receipts bound to that
/// same document, and then runs the bounded PNG, JPEG, and PDF artifact proofs. It does not construct
/// C6 cells, route-cutover authorization, or an eligibility receipt.
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub fn run_representative_native_export_smoke()
-> Result<NativeExportSmokeSummary, NativeExportSmokeError> {
    let theme_catalog = ThemeFixtureCatalog::load(themes_root())
        .map_err(|error| NativeExportSmokeError::new("fixture-catalog", error.to_string()))?;
    let fixture = theme_catalog
        .fixture(BRUTALIST_STATE_FIXTURE_ID)
        .ok_or_else(|| {
            NativeExportSmokeError::new(
                "source-fixture",
                format!("missing fixture `{BRUTALIST_STATE_FIXTURE_ID}`"),
            )
        })?;
    let input = fixture.theme_input().ok_or_else(|| {
        NativeExportSmokeError::new(
            "theme-input",
            format!("fixture `{BRUTALIST_STATE_FIXTURE_ID}` has no typed theme input"),
        )
    })?;
    let source = theme_catalog
        .source_text(fixture.id())
        .map_err(|error| NativeExportSmokeError::new("source-fixture-read", error.to_string()))?;
    let rendered = prove_brutalist_state_rendered_document(&theme_catalog, input, &source)?;
    let png = prove_brutalist_state_png_projection(&rendered.contract, &rendered.document)?;
    let jpeg = prove_brutalist_state_jpeg_projection(&rendered.document, &png.artifact_proof)?;
    let pdf = prove_brutalist_state_pdf_projection(&rendered.document)?;
    let projections = [
        (RenderArtifactKind::Png, &png.target_receipt),
        (RenderArtifactKind::Jpeg, &jpeg.target_receipt),
        (RenderArtifactKind::Pdf, &pdf.target_receipt),
    ];
    prove_representative_native_export_receipts(&rendered.document, &projections)?;

    Ok(NativeExportSmokeSummary {
        verified_projection_count: projections.len(),
    })
}

fn run_enforced_c6_runtime_from_root(
    root: impl AsRef<Path>,
) -> Result<C6ExecutionReport, C6RuntimeError> {
    let theme_catalog = ThemeFixtureCatalog::load(root)?;
    let acceptance = C6AcceptanceCatalog::load(&theme_catalog)?;
    run_catalog(&theme_catalog, &acceptance)
}

fn run_c6a_eligibility_from_root(
    root: impl AsRef<Path>,
) -> Result<C6aEligibilityReceipt, C6RuntimeError> {
    let theme_catalog = ThemeFixtureCatalog::load(root)?;
    let acceptance = C6AcceptanceCatalog::load(&theme_catalog)?;
    let ledger = run_catalog_evaluated(&theme_catalog, &acceptance)?;
    issue_c6a_eligibility(ledger, &acceptance)
}

fn run_catalog(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
) -> Result<C6ExecutionReport, C6RuntimeError> {
    Ok(run_catalog_evaluated(theme_catalog, acceptance)?.execution_report())
}

pub(crate) fn run_catalog_evaluated(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
) -> Result<C6EvaluatedLedger, C6RuntimeError> {
    let plan = C6ExecutionPlan::from_catalog(acceptance)?;
    let expected_keys = acceptance
        .enforced_tranche()
        .cells()
        .map(C6EnforcedCell::key)
        .collect::<Vec<_>>();
    let mut group_receipts = Vec::with_capacity(plan.groups.len());
    let mut cell_receipts = Vec::with_capacity(expected_keys.len());
    for group in plan.groups {
        let completed = execute_render_group(theme_catalog, acceptance, group)?;
        group_receipts.push(completed.receipt);
        cell_receipts.extend(completed.cells);
    }
    C6ReceiptBook::from_receipts(expected_keys, group_receipts, cell_receipts)?
        .evaluate(acceptance, theme_catalog)
}

type C6RenderGroupExecutor = fn(
    &ThemeFixtureCatalog,
    &C6AcceptanceCatalog,
    C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError>;

#[derive(Clone, Copy, Debug)]
struct C6RenderGroupAdapter {
    theme: C6ProofTheme,
    family: C6ProofFamily,
    source_fixture_id: &'static str,
    proof_recipe_revision: &'static str,
    supported_targets: &'static [ExpectedOutputTarget],
    execute: C6RenderGroupExecutor,
}

impl C6RenderGroupAdapter {
    fn supports_target(&self, target: ExpectedOutputTarget) -> bool {
        self.supported_targets.contains(&target)
    }

    fn matches_group(&self, key: &C6RenderGroupKey, proof_recipe_revision: &str) -> bool {
        self.theme == key.theme()
            && self.family == key.family()
            && self.source_fixture_id == key.source_fixture_id()
            && self.proof_recipe_revision == proof_recipe_revision
    }
}

const BRUTALIST_FLOWCHART_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Brutalist,
    family: C6ProofFamily::Flowchart,
    source_fixture_id: BRUTALIST_FLOWCHART_FIXTURE_ID,
    proof_recipe_revision: "brutalist-flowchart-v1",
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_brutalist_flowchart_group,
};

const BRUTALIST_STATE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Brutalist,
    family: C6ProofFamily::State,
    source_fixture_id: BRUTALIST_STATE_FIXTURE_ID,
    proof_recipe_revision: "brutalist-state-v1",
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_brutalist_state_group,
};

const BRUTALIST_SEQUENCE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Brutalist,
    family: C6ProofFamily::Sequence,
    source_fixture_id: BRUTALIST_SEQUENCE_FIXTURE_ID,
    proof_recipe_revision: "brutalist-sequence-v1",
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
    ],
    execute: execute_brutalist_sequence_group,
};

static C6_RENDER_GROUP_ADAPTERS: &[C6RenderGroupAdapter] = &[
    BRUTALIST_FLOWCHART_ADAPTER,
    BRUTALIST_SEQUENCE_ADAPTER,
    BRUTALIST_STATE_ADAPTER,
    CYBERPUNK_FLOWCHART_ADAPTER,
    CYBERPUNK_SEQUENCE_ADAPTER,
    CYBERPUNK_STATE_ADAPTER,
    SPOTLESS_FLOWCHART_ADAPTER,
    SPOTLESS_SEQUENCE_ADAPTER,
    SPOTLESS_STATE_ADAPTER,
];

#[derive(Debug)]
struct C6RenderGroupWork {
    key: C6RenderGroupKey,
    proof_recipe_revision: &'static str,
    cells: Vec<C6EnforcedCell>,
}

#[derive(Debug)]
struct C6RenderGroupPlan {
    adapter: &'static C6RenderGroupAdapter,
    work: C6RenderGroupWork,
}

#[derive(Debug)]
struct C6ExecutionPlan {
    groups: Vec<C6RenderGroupPlan>,
}

impl C6ExecutionPlan {
    fn from_catalog(acceptance: &C6AcceptanceCatalog) -> Result<Self, C6RuntimeError> {
        let enforced = acceptance.enforced_tranche().cells().collect::<Vec<_>>();
        if enforced.is_empty() {
            return Err(C6RuntimeError::EmptyTranche);
        }

        let mut grouped = BTreeMap::<
            C6RenderGroupKey,
            (&'static C6RenderGroupAdapter, Vec<C6EnforcedCell>),
        >::new();
        for cell in enforced {
            let key = C6RenderGroupKey::for_cell(cell);
            let proof_recipe = acceptance
                .proof_recipe(merman_theme_fixtures::C6ProofRecipeKey::new(
                    key.theme(),
                    key.family(),
                ))
                .ok_or(C6RuntimeError::UnsupportedEnforcedCell { key: cell.key() })?;
            if proof_recipe.source_fixture_id() != key.source_fixture_id() {
                return Err(C6RuntimeError::UnsupportedEnforcedCell { key: cell.key() });
            }
            let adapter = adapter_for_group(
                C6_RENDER_GROUP_ADAPTERS,
                &key,
                proof_recipe.proof_recipe_revision(),
            )?
            .ok_or_else(|| C6RuntimeError::UnsupportedEnforcedCell { key: cell.key() })?;
            if !adapter.supports_target(cell.key().target()) {
                return Err(C6RuntimeError::UnsupportedEnforcedCell { key: cell.key() });
            }
            let entry = grouped.entry(key).or_insert_with(|| (adapter, Vec::new()));
            entry.1.push(cell.clone());
        }

        Ok(Self {
            groups: grouped
                .into_iter()
                .map(|(key, (adapter, cells))| C6RenderGroupPlan {
                    adapter,
                    work: C6RenderGroupWork {
                        key,
                        proof_recipe_revision: adapter.proof_recipe_revision,
                        cells,
                    },
                })
                .collect(),
        })
    }
}

fn adapter_for_group<'a>(
    adapters: &'a [C6RenderGroupAdapter],
    key: &C6RenderGroupKey,
    proof_recipe_revision: &str,
) -> Result<Option<&'a C6RenderGroupAdapter>, C6RuntimeError> {
    let mut matching = adapters
        .iter()
        .filter(|adapter| adapter.matches_group(key, proof_recipe_revision));
    let Some(adapter) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(C6RuntimeError::AmbiguousRenderGroupAdapter { group: key.label() });
    }
    Ok(Some(adapter))
}

struct C6CompletedRenderGroup {
    receipt: C6RenderGroupReceipt,
    cells: Vec<C6CellReceipt>,
}

fn execute_render_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupPlan,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    (group.adapter.execute)(theme_catalog, acceptance, group.work)
}

fn load_render_group_fixture<'a>(
    theme_catalog: &'a ThemeFixtureCatalog,
    key: &C6RenderGroupKey,
) -> Result<(&'a ReferenceThemeInput, String), C6RuntimeError> {
    let fixture = theme_catalog
        .fixture(key.source_fixture_id())
        .ok_or_else(|| render_group_error(key, "source-fixture"))?;
    let input = fixture
        .theme_input()
        .ok_or_else(|| render_group_error(key, "theme-input"))?;
    let source = theme_catalog.source_text(fixture.id()).map_err(|error| {
        C6RuntimeError::RenderGroupProofFailed {
            group: key.label(),
            stage: "source-fixture-read",
            detail: error.to_string(),
        }
    })?;
    Ok((input, source))
}

fn complete_c6_svg_png_render_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
    document: &RenderedDocument,
    identity: &crate::observation::C6RenderIdentity,
    standalone_svg_proof: C6BoundTargetProof,
    prove_png_artifact: impl FnOnce(
        &str,
        C6TargetArtifact<'_>,
        RasterPlan,
    ) -> C6ProofResult<C6BoundTargetProof>,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let png_dependents = group
        .cells
        .iter()
        .map(|cell| cell.key().target())
        .filter(|target| *target == ExpectedOutputTarget::Png)
        .collect::<Vec<_>>();
    let png_observation = if png_dependents.is_empty() {
        None
    } else {
        let output = prove_target(
            &group.key,
            ExpectedOutputTarget::Png,
            &png_dependents,
            document
                .export_png(
                    &RasterOptions::default().with_scale(2.0),
                    OperationControl::new(),
                )
                .map_err(|error| C6ProofError::new("png-render", error.to_string())),
        )?;
        let target_artifact = bind_raster_artifact(&output);
        let target_proof = prove_target(
            &group.key,
            ExpectedOutputTarget::Png,
            &png_dependents,
            prove_png_artifact(document.svg(), target_artifact, output.plan()),
        )?;
        Some(C6TargetObservation::new(identity.clone(), target_proof))
    };

    prove_shared_document_identity(
        &group.key,
        document.standalone_svg_admission(),
        png_observation
            .as_ref()
            .map(C6TargetObservation::target_receipt),
    )?;
    let group_receipt = C6RenderGroupReceipt::seal(
        group.key.clone(),
        group.proof_recipe_revision,
        theme_catalog,
        identity,
        document.standalone_svg_admission(),
    )?;
    let svg_observation = C6TargetObservation::new(identity.clone(), standalone_svg_proof);

    let mut cells = Vec::with_capacity(group.cells.len());
    for enforced in &group.cells {
        let observation = match enforced.key().target() {
            ExpectedOutputTarget::StandaloneSvg => svg_observation.clone(),
            ExpectedOutputTarget::Png => png_observation
                .as_ref()
                .ok_or_else(|| render_group_error(&group.key, "png-support-plan"))?
                .clone(),
            ExpectedOutputTarget::BrowserSvg
            | ExpectedOutputTarget::Jpeg
            | ExpectedOutputTarget::Pdf => {
                return Err(C6RuntimeError::UnsupportedEnforcedCell {
                    key: enforced.key(),
                });
            }
        };
        cells.push(observation.seal(acceptance, enforced, &group_receipt)?);
    }

    Ok(C6CompletedRenderGroup {
        receipt: group_receipt,
        cells,
    })
}

fn execute_brutalist_flowchart_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_brutalist_flowchart_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_brutalist_flowchart_png(&rendered.contract, &rendered.svg_proof, artifact, plan)
        },
    )
}

fn execute_brutalist_state_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_brutalist_state_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |svg, artifact, plan| {
            let (_, target_proof) = artifact.check_with(
                "brutalist-state-png-v1",
                brutalist_state_applied_mechanisms(),
                |bytes| prove_brutalist_state_png(&rendered.contract, svg, bytes, plan),
            )?;
            Ok(target_proof)
        },
    )
}

fn execute_brutalist_sequence_group(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let (input, source) = load_render_group_fixture(theme_catalog, &group.key)?;
    let rendered = prove_render_group(
        &group.key,
        prove_brutalist_sequence_rendered_document(theme_catalog, input, &source),
    )?;
    complete_c6_svg_png_render_group(
        theme_catalog,
        acceptance,
        group,
        &rendered.document,
        &rendered.identity,
        rendered.svg_proof.target_proof(),
        |_, artifact, plan| {
            prove_brutalist_sequence_png(&rendered.contract, &rendered.svg_proof, artifact, plan)
        },
    )
}

struct BrutalistFlowchartRenderedDocument<'a> {
    contract: BrutalistFlowchartFixtureContract<'a>,
    document: RenderedDocument,
    identity: crate::observation::C6RenderIdentity,
    svg_proof: BrutalistFlowchartSvgProof,
}

fn prove_brutalist_flowchart_rendered_document<'a>(
    theme_catalog: &ThemeFixtureCatalog,
    input: &'a ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<BrutalistFlowchartRenderedDocument<'a>> {
    let contract = BrutalistFlowchartFixtureContract::from_input(input)?;
    let theme = compile_brutalist_flowchart_theme(theme_catalog, &contract)?;
    let renderer = brutalist_native_text_renderer(contract.font_family());
    let document = render_brutalist_document(&renderer, source, &theme, portable_svg_request())?;
    let identity = prove_portable_family_evidence(
        document.evidence(),
        &theme,
        DiagramFamilyId::FLOWCHART,
        FamilyEvidenceRequirements::BRUTALIST_FLOWCHART,
    )?;
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_brutalist_flowchart_svg(&contract, artifact)?;

    Ok(BrutalistFlowchartRenderedDocument {
        contract,
        document,
        identity,
        svg_proof,
    })
}

struct BrutalistStateRenderedDocument<'a> {
    contract: BrutalistStateFixtureContract<'a>,
    document: RenderedDocument,
    identity: crate::observation::C6RenderIdentity,
    svg_proof: BrutalistStateSvgProof,
}

fn prove_brutalist_state_rendered_document<'a>(
    theme_catalog: &ThemeFixtureCatalog,
    input: &'a ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<BrutalistStateRenderedDocument<'a>> {
    let contract = BrutalistStateFixtureContract::from_input(input)?;
    let theme = compile_brutalist_state_theme(theme_catalog, &contract)?;
    prove_brutalist_state_capabilities(&theme)?;

    let renderer = c6_renderer();
    let document = render_brutalist_document(&renderer, source, &theme, portable_svg_request())?;
    let identity = prove_brutalist_state_evidence(document.evidence(), &theme)?;
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_brutalist_state_svg(&contract, artifact)?;
    prove_mechanism_coverage(&contract.common.mechanisms, &svg_proof.mechanisms)?;

    Ok(BrutalistStateRenderedDocument {
        contract,
        document,
        identity,
        svg_proof,
    })
}

struct BrutalistSequenceRenderedDocument<'a> {
    contract: BrutalistSequenceFixtureContract<'a>,
    document: RenderedDocument,
    identity: crate::observation::C6RenderIdentity,
    svg_proof: BrutalistSequenceSvgProof,
}

fn prove_brutalist_sequence_rendered_document<'a>(
    theme_catalog: &ThemeFixtureCatalog,
    input: &'a ReferenceThemeInput,
    source: &str,
) -> C6ProofResult<BrutalistSequenceRenderedDocument<'a>> {
    let contract = BrutalistSequenceFixtureContract::from_sequence_input(input)?;
    let theme = compile_brutalist_sequence_theme(theme_catalog, &contract)?;
    let renderer = brutalist_native_text_renderer(contract.font_family());
    let document = render_brutalist_document(&renderer, source, &theme, portable_svg_request())?;
    let identity = prove_portable_family_evidence(
        document.evidence(),
        &theme,
        DiagramFamilyId::SEQUENCE,
        FamilyEvidenceRequirements::BRUTALIST_SEQUENCE,
    )?;
    let artifact = bind_document_svg_artifact(&document);
    let svg_proof = prove_brutalist_sequence_svg(&contract, artifact)?;

    Ok(BrutalistSequenceRenderedDocument {
        contract,
        document,
        identity,
        svg_proof,
    })
}

fn render_group_error(key: &C6RenderGroupKey, field: &'static str) -> C6RuntimeError {
    C6RuntimeError::RenderGroupEvidenceMismatch {
        group: key.label(),
        field,
    }
}

#[derive(Clone)]
struct C6TargetObservation {
    identity: crate::observation::C6RenderIdentity,
    residual_ids: BTreeSet<String>,
    proof: C6BoundTargetProof,
}

impl C6TargetObservation {
    fn new(identity: crate::observation::C6RenderIdentity, proof: C6BoundTargetProof) -> Self {
        Self {
            identity,
            residual_ids: BTreeSet::new(),
            proof,
        }
    }

    fn target_receipt(&self) -> &TargetAdmissionReceipt {
        self.proof.target_receipt()
    }

    fn seal(
        self,
        acceptance: &C6AcceptanceCatalog,
        enforced: &C6EnforcedCell,
        group: &C6RenderGroupReceipt,
    ) -> Result<C6CellReceipt, C6RuntimeError> {
        let expectation = acceptance
            .cell(enforced.key())
            .ok_or(C6RuntimeError::EvidenceMismatch {
                key: enforced.key(),
                field: "cell-expectation",
            })?
            .expectation();
        seal_cell_from_evidence(
            expectation,
            enforced,
            group,
            self.identity,
            self.residual_ids,
            self.proof,
        )
    }
}

fn prove_shared_document_identity(
    key: &C6RenderGroupKey,
    standalone: &TargetAdmissionReceipt,
    png: Option<&TargetAdmissionReceipt>,
) -> Result<(), C6RuntimeError> {
    let standalone_resource = *standalone.resource_fingerprint().as_bytes();
    let standalone_fonts = *standalone.font_catalog_fingerprint().as_bytes();
    let mismatched = [png].into_iter().flatten().any(|receipt| {
        receipt.document_digest() != standalone.document_digest()
            || *receipt.resource_fingerprint().as_bytes() != standalone_resource
            || *receipt.font_catalog_fingerprint().as_bytes() != standalone_fonts
    });
    if standalone.document_digest() == [0; 32]
        || standalone_resource == [0; 32]
        || standalone_fonts == [0; 32]
        || mismatched
    {
        return Err(render_group_error(key, "native-document-identity"));
    }
    Ok(())
}

fn prove_brutalist_state_capabilities(theme: &DiagramTheme) -> C6ProofResult<()> {
    let actual = theme
        .report()
        .required_capabilities()
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::from([
        ThemeCapability::SemanticTokens,
        ThemeCapability::Typography,
        ThemeCapability::SemanticRules,
        ThemeCapability::OrdinalPalette,
        ThemeCapability::SolidPaint,
        ThemeCapability::BorderStyling,
        ThemeCapability::RoundedGeometry,
        ThemeCapability::Shadow,
        ThemeCapability::SvgFilter,
    ]);
    c6_ensure!(
        "theme-capabilities",
        actual == expected,
        "required capabilities differ: expected={expected:?}, actual={actual:?}"
    );
    Ok(())
}

fn prove_mechanism_coverage(
    expected: &BTreeSet<ReferenceThemeMechanism>,
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ProofResult<()> {
    let actual = mechanisms.keys().copied().collect::<BTreeSet<_>>();
    c6_ensure!(
        "mechanism-coverage",
        actual == *expected,
        "observed mechanisms differ: expected={expected:?}, actual={actual:?}"
    );
    Ok(())
}

fn brutalist_state_applied_mechanisms()
-> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    BTreeMap::from([
        (
            ReferenceThemeMechanism::CanvasSolid,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::CssFilter,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::FontStack,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::NthChildSelector,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::RoundedCorners,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::StrokeStyling,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::ThemeVariables,
            C6ObservedMechanismDisposition::Applied,
        ),
    ])
}

fn prove_brutalist_state_evidence(
    evidence: &RenderEvidence,
    theme: &DiagramTheme,
) -> C6ProofResult<crate::observation::C6RenderIdentity> {
    prove_portable_family_evidence(
        evidence,
        theme,
        DiagramFamilyId::STATE,
        FamilyEvidenceRequirements::BRUTALIST_STATE,
    )
}

pub(crate) fn prove_portable_family_evidence(
    evidence: &RenderEvidence,
    theme: &DiagramTheme,
    expected_family: DiagramFamilyId,
    requirements: FamilyEvidenceRequirements,
) -> C6ProofResult<crate::observation::C6RenderIdentity> {
    let summary = merman::__theme_acceptance::theme_acceptance_evidence(evidence);
    let recipe = theme.recipe_fingerprint();
    c6_ensure!(
        "render-family",
        evidence.family_id() == expected_family,
        "expected {expected_family} family, got {}",
        evidence.family_id()
    );
    c6_ensure!(
        "render-recipe",
        evidence.theme_recipe_fingerprint() == Some(recipe),
        "render evidence recipe fingerprint does not match the compiled theme"
    );
    c6_ensure!(
        "render-recipe-report",
        summary
            .recipe_report()
            .map(|report| report.theme_recipe_fingerprint())
            == Some(recipe),
        "render evidence recipe report does not match the compiled theme"
    );
    let root_theme = summary.root();
    if requirements.require_root_theme {
        c6_ensure!(
            "root-theme-report",
            root_theme.is_verified(),
            "root theme report is not verified"
        );
    } else {
        c6_ensure!(
            "root-theme-report",
            root_theme.is_satisfied(),
            "root theme report is {:?}",
            root_theme.status()
        );
    }
    c6_ensure!(
        "theme-portability-requirement",
        summary.portability_requirement() == Some(ThemePortabilityRequirement::RequirePortable),
        "render session did not retain the strict portable theme requirement"
    );
    let family = summary.family();
    c6_ensure!(
        "family-evidence-status",
        family.status() == ThemeEvidenceStatus::Verified,
        "family evidence is {:?}",
        family.status()
    );
    c6_ensure!(
        "family-evidence-coverage",
        is_positive_family_evidence(
            family.required_count(),
            family.accounted_count(),
            family.applied_count(),
            family.not_applicable_count(),
            family.incomplete_count(),
        ),
        "family evidence is not wholly applied: required={}, accounted={}, applied={}, not_applicable={}, incomplete={}",
        family.required_count(),
        family.accounted_count(),
        family.applied_count(),
        family.not_applicable_count(),
        family.incomplete_count()
    );
    c6_ensure!(
        "family-evidence-residuals",
        family.residual_count() == 0
            && summary.source_residual_count() == 0
            && summary.compatibility_residual_count() == 0
            && summary.mermaid_compatibility_residual_count() == 0,
        "family evidence retained residual counts: theme={}, source={}, compatibility={}, mermaid={}",
        family.residual_count(),
        summary.source_residual_count(),
        summary.compatibility_residual_count(),
        summary.mermaid_compatibility_residual_count()
    );
    c6_ensure!(
        "family-evidence-output",
        !family.output_mutated(),
        "family evidence was invalidated by an output mutation"
    );
    match summary.prepared_text_layout() {
        Some(prepared_text) if requirements.require_prepared_text => c6_ensure!(
            "render-font-source",
            prepared_text.used_font_sources() == [FontSource::Embedded],
            "prepared text did not exclusively use embedded fonts: {:?}",
            prepared_text.used_font_sources()
        ),
        Some(prepared_text) => c6_ensure!(
            "render-font-source",
            prepared_text
                .used_font_sources()
                .iter()
                .all(|source| *source == FontSource::Embedded),
            "route witness prepared text consulted a non-embedded font source: {:?}",
            prepared_text.used_font_sources()
        ),
        None => c6_ensure!(
            "render-font-source",
            !requirements.require_prepared_text,
            "render evidence did not retain prepared text layout evidence"
        ),
    }
    let host_measurement_count = evidence
        .measurement()
        .entries()
        .iter()
        .filter(|entry| entry.provenance().source == TextMeasurementSource::Host)
        .map(|entry| entry.count())
        .sum::<u64>();
    c6_ensure!(
        "render-text-measurement",
        host_measurement_count == 0,
        "render used host text measurement {host_measurement_count} times"
    );
    c6_ensure!(
        "render-text-layout",
        summary.text_layout_failure().is_none(),
        "render retained a text layout failure: {:?}",
        summary.text_layout_failure()
    );
    c6_ensure!(
        "render-runtime",
        evidence.operation_context().clock_source() == merman::runtime::RuntimeValueSource::Fixed
            && evidence.operation_context().random_source()
                == merman::runtime::RuntimeValueSource::Fixed
            && evidence.operation_context().timing().is_none()
            && evidence.local_time_zone().source()
                == merman::time::LocalTimeZoneSource::FixedOffset,
        "render operation consulted host runtime state"
    );
    Ok(crate::observation::C6RenderIdentity::from_evidence(
        evidence,
    ))
}

fn is_positive_family_evidence(
    required_count: usize,
    accounted_count: usize,
    applied_count: usize,
    not_applicable_count: usize,
    incomplete_count: usize,
) -> bool {
    required_count == accounted_count
        && required_count == applied_count
        && not_applicable_count == 0
        && incomplete_count == 0
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
struct ProvenPngProjection {
    target_receipt: TargetAdmissionReceipt,
    artifact_proof: PngArtifactProof,
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
struct ProvenNativeProjection {
    target_receipt: TargetAdmissionReceipt,
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_representative_native_export_receipts(
    document: &RenderedDocument,
    projections: &[(RenderArtifactKind, &TargetAdmissionReceipt)],
) -> C6ProofResult<()> {
    let document_digest = document.document_digest();
    let resource_fingerprint = document.resource_fingerprint();
    let font_catalog_fingerprint = document
        .standalone_svg_admission()
        .font_catalog_fingerprint();
    c6_ensure!(
        "native-export-document",
        document_digest != [0; 32]
            && resource_fingerprint.as_bytes() != &[0; 32]
            && font_catalog_fingerprint.as_bytes() != &[0; 32],
        "representative native export document retained a zero production identity"
    );

    for (expected_kind, receipt) in projections {
        c6_ensure!(
            "native-export-admission",
            receipt.artifact_kind() == *expected_kind,
            "expected {} target receipt, got {}",
            expected_kind.id(),
            receipt.artifact_kind().id()
        );
        c6_ensure!(
            "native-export-admission",
            receipt.status() == TargetAdmissionStatus::Portable
                && receipt.reasons().is_empty()
                && receipt.font_source() == TargetFontSource::Embedded,
            "{} target is not portable with embedded fonts: status={} reasons={:?} font_source={}",
            expected_kind.id(),
            receipt.status().id(),
            receipt.reasons(),
            receipt.font_source().id()
        );
        c6_ensure!(
            "native-export-document",
            receipt.document_digest() == document_digest
                && receipt.resource_fingerprint() == resource_fingerprint
                && receipt.font_catalog_fingerprint() == font_catalog_fingerprint,
            "{} target receipt is not bound to the shared RenderedDocument",
            expected_kind.id()
        );
        c6_ensure!(
            "native-export-admission",
            receipt.target_evidence_digest() != [0; 32]
                && receipt.artifact_digest() != [0; 32]
                && receipt.receipt_digest() != [0; 32],
            "{} target receipt retained a zero production digest",
            expected_kind.id()
        );
    }
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_brutalist_state_png_projection(
    contract: &BrutalistStateFixtureContract<'_>,
    document: &RenderedDocument,
) -> C6ProofResult<ProvenPngProjection> {
    let output = document
        .export_png(
            &RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
        )
        .map_err(|error| C6ProofError::new("png-render", error.to_string()))?;
    let artifact_proof =
        prove_brutalist_state_png(contract, document.svg(), output.bytes(), output.plan())?;
    Ok(ProvenPngProjection {
        target_receipt: output.admission().clone(),
        artifact_proof,
    })
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_brutalist_state_jpeg_projection(
    document: &RenderedDocument,
    png_control: &PngArtifactProof,
) -> C6ProofResult<ProvenNativeProjection> {
    let output = document
        .export_jpeg(
            &RasterOptions::default().with_scale(2.0),
            OperationControl::new(),
        )
        .map_err(|error| C6ProofError::new("jpeg-render", error.to_string()))?;
    prove_brutalist_state_jpeg(output.bytes(), output.plan(), png_control)?;
    Ok(ProvenNativeProjection {
        target_receipt: output.admission().clone(),
    })
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_brutalist_state_pdf_projection(
    document: &RenderedDocument,
) -> C6ProofResult<ProvenNativeProjection> {
    let output = document
        .export_pdf(&PdfOptions::default(), OperationControl::new())
        .map_err(|error| C6ProofError::new("pdf-render", error.to_string()))?;
    prove_brutalist_state_pdf(output.bytes())?;
    Ok(ProvenNativeProjection {
        target_receipt: output.admission().clone(),
    })
}

fn c6_renderer() -> Renderer {
    Renderer::new().with_engine(Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({
            "htmlLabels": false
        }),
    )))
}

fn brutalist_native_text_renderer(font_family: &str) -> Renderer {
    Renderer::new().with_engine(Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({
            "htmlLabels": false,
            "fontFamily": font_family,
            "themeVariables": {
                "fontFamily": font_family
            }
        }),
    )))
}

pub(crate) fn portable_svg_request() -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable),
        pipeline: Some(SvgPipeline::resvg_safe()),
        ..SvgRequest::default()
    }
}

fn render_brutalist_document(
    renderer: &Renderer,
    source: &str,
    theme: &DiagramTheme,
    request: SvgRequest,
) -> C6ProofResult<RenderedDocument> {
    let output = renderer
        .render(
            RenderRequest::document(source, OperationControl::new(), request)
                .with_theme(theme.clone()),
        )
        .map_err(|error| C6ProofError::new("render-svg", error.to_string()))?;
    let RenderOutput::Document(Some(output)) = output else {
        return Err(C6ProofError::new(
            "render-svg",
            "enforced source did not produce a completed document",
        ));
    };
    Ok(output)
}

fn compile_brutalist_flowchart_theme(
    theme_catalog: &ThemeFixtureCatalog,
    contract: &BrutalistFlowchartFixtureContract<'_>,
) -> C6ProofResult<DiagramTheme> {
    let mut node_patch = ThemeStylePatch::default()
        .with_stroke(brutalist_solid(
            contract.border.color(),
            "flowchart-node-stroke",
        )?)
        .with_stroke_width(f32::from(contract.border.width_px()))
        .map_err(|error| C6ProofError::new("flowchart-node-stroke", error.to_string()))?;
    node_patch.geometry.radius = Specified::Value(f32::from(contract.radius_px));

    let palette_colors = contract
        .palette_colors
        .iter()
        .map(|color| {
            ThemeColorValue::parse(color)
                .map_err(|error| C6ProofError::new("flowchart-node-palette", error.to_string()))
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let palette = OrdinalPalette::new(palette_colors)
        .map_err(|error| C6ProofError::new("flowchart-node-palette", error.to_string()))?;
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(ThemeTarget::Node, node_patch).for_family(DiagramFamilyId::FLOWCHART),
        )
        .with_ordinal_palette(ThemeTarget::Node, palette);

    DiagramThemeCompiler::new()
        .compile(materialize_brutalist_spec(
            theme_catalog,
            contract,
            styles,
            false,
        )?)
        .map_err(|error| C6ProofError::new("flowchart-theme-compile", error.to_string()))
}

fn compile_brutalist_state_theme(
    theme_catalog: &ThemeFixtureCatalog,
    contract: &BrutalistStateFixtureContract<'_>,
) -> C6ProofResult<DiagramTheme> {
    let mut state_patch = ThemeStylePatch::default()
        .with_stroke(
            CanvasPaint::solid(contract.border.color())
                .map_err(|error| C6ProofError::new("fixture-border", error.to_string()))?,
        )
        .with_stroke_width(f32::from(contract.border.width_px()))
        .map_err(|error| C6ProofError::new("fixture-border", error.to_string()))?;
    state_patch.geometry.radius = Specified::Value(f32::from(contract.radius_px));

    let palette_colors = contract
        .palette_colors
        .iter()
        .map(|color| {
            ThemeColorValue::parse(color)
                .map_err(|error| C6ProofError::new("fixture-palette", error.to_string()))
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let palette = OrdinalPalette::new(palette_colors)
        .map_err(|error| C6ProofError::new("fixture-palette", error.to_string()))?;
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(ThemeTarget::State, state_patch).for_family(DiagramFamilyId::STATE),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::StateLabel,
                ThemeStylePatch::default().with_fill(
                    CanvasPaint::solid(contract.tokens.text()).map_err(|error| {
                        C6ProofError::new("fixture-text-color", error.to_string())
                    })?,
                ),
            )
            .for_family(DiagramFamilyId::STATE),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::TransitionLabelBackground,
                ThemeStylePatch::default().with_fill(
                    CanvasPaint::solid(contract.tokens.surface()).map_err(|error| {
                        C6ProofError::new("fixture-surface-color", error.to_string())
                    })?,
                ),
            )
            .for_family(DiagramFamilyId::STATE),
        )
        .with_ordinal_palette(ThemeTarget::State, palette);

    let graph = EffectGraph::new(
        SHADOW_EFFECT_ID,
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: f32::from(contract.shadow.offset_x_px()),
            offset_y: f32::from(contract.shadow.offset_y_px()),
            blur_radius: f32::from(contract.shadow.blur_px()),
            spread: f32::from(contract.shadow.spread_px()),
            color: ThemeColorValue::parse(contract.shadow.color())
                .map_err(|error| C6ProofError::new("fixture-shadow", error.to_string()))?,
        }],
    )
    .map_err(|error| C6ProofError::new("fixture-shadow-graph", error.to_string()))?;
    let effects = DiagramEffectSet::default()
        .with_graph(graph)
        .map_err(|error| C6ProofError::new("fixture-shadow-graph", error.to_string()))?
        .with_binding(
            EffectBinding::new(ThemeTarget::State, SHADOW_EFFECT_ID)
                .map_err(|error| C6ProofError::new("fixture-effect-binding", error.to_string()))?,
        )
        .map_err(|error| C6ProofError::new("fixture-effect-binding", error.to_string()))?;

    DiagramThemeCompiler::new()
        .compile(
            materialize_brutalist_spec(theme_catalog, contract, styles, true)?
                .with_effects(effects),
        )
        .map_err(|error| C6ProofError::new("theme-compile", error.to_string()))
}

fn compile_brutalist_sequence_theme(
    theme_catalog: &ThemeFixtureCatalog,
    contract: &BrutalistSequenceFixtureContract<'_>,
) -> C6ProofResult<DiagramTheme> {
    let actor = ThemeStylePatch::default()
        .with_fill(brutalist_solid(
            contract.actor_fill(),
            "sequence-actor-fill",
        )?)
        .with_stroke(brutalist_solid(contract.stroke(), "sequence-actor-stroke")?);
    let note = ThemeStylePatch::default()
        .with_fill(brutalist_solid(contract.note_fill(), "sequence-note-fill")?)
        .with_stroke(brutalist_solid(contract.stroke(), "sequence-note-stroke")?);
    let activation = ThemeStylePatch::default()
        .with_fill(brutalist_solid(
            contract.actor_fill(),
            "sequence-activation-fill",
        )?)
        .with_stroke(brutalist_solid(
            contract.stroke(),
            "sequence-activation-stroke",
        )?);
    let message = ThemeStylePatch::default().with_stroke(brutalist_solid(
        contract.stroke(),
        "sequence-message-stroke",
    )?);
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(ThemeTarget::Actor, actor).for_family(DiagramFamilyId::SEQUENCE))
        .with_rule(ThemeRule::new(ThemeTarget::Note, note).for_family(DiagramFamilyId::SEQUENCE))
        .with_rule(
            ThemeRule::new(ThemeTarget::Activation, activation)
                .for_family(DiagramFamilyId::SEQUENCE),
        )
        .with_rule(
            ThemeRule::new(ThemeTarget::Message, message).for_family(DiagramFamilyId::SEQUENCE),
        );

    DiagramThemeCompiler::new()
        .compile(materialize_brutalist_spec(
            theme_catalog,
            contract,
            styles,
            false,
        )?)
        .map_err(|error| C6ProofError::new("sequence-theme-compile", error.to_string()))
}

fn materialize_brutalist_spec(
    theme_catalog: &ThemeFixtureCatalog,
    contract: &BrutalistCommonFixtureContract<'_>,
    styles: ThemeRuleSet,
    family_typography: bool,
) -> C6ProofResult<DiagramThemeSpec> {
    let family_stack = FontStack::new(contract.font_stack.families().iter().cloned())
        .map_err(|error| C6ProofError::new("fixture-font-stack", error.to_string()))?;
    let asset = theme_catalog
        .font_asset(contract.font_asset_id)
        .ok_or_else(|| {
            C6ProofError::new(
                "fixture-font-asset",
                format!("font asset `{}` does not exist", contract.font_asset_id),
            )
        })?;
    c6_ensure!(
        "fixture-font-asset",
        asset.family() == contract.font_family(),
        "font asset family `{}` differs from requested family `{}`",
        asset.family(),
        contract.font_family()
    );
    let bytes = theme_catalog
        .asset_bytes(contract.font_asset_id)
        .map_err(|error| C6ProofError::new("fixture-font-bytes", error.to_string()))?;
    let font_catalog = FontCatalogSpec::new([FontAssetSpec::new(asset.id(), bytes)])
        .with_available_sources([FontSource::Embedded])
        .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
    let canvas = CanvasSpec::solid(contract.canvas_color)
        .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?;
    let default_text = if family_typography {
        ThemeTextStyle::default().with_font_stack(family_stack)
    } else {
        ThemeTextStyle::default()
    };
    Ok(DiagramThemeSpec::new()
        .with_typography(TypographySpec::default().with_default(default_text))
        .with_assets(ThemeAssets::default().with_font_catalog(font_catalog))
        .with_canvas(canvas)
        .with_styles(styles))
}

fn brutalist_solid(value: &str, stage: &'static str) -> C6ProofResult<CanvasPaint> {
    CanvasPaint::solid(value).map_err(|error| C6ProofError::new(stage, error.to_string()))
}

struct BrutalistStateSvgProof {
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    target_proof: C6BoundTargetProof,
}

impl BrutalistStateSvgProof {
    fn target_proof(&self) -> C6BoundTargetProof {
        self.target_proof.clone()
    }
}

fn prove_brutalist_state_svg(
    contract: &BrutalistStateFixtureContract<'_>,
    artifact: C6TargetArtifact<'_>,
) -> C6ProofResult<BrutalistStateSvgProof> {
    let mechanisms = brutalist_state_applied_mechanisms();
    let (_, target_proof) = artifact.check_with(
        "brutalist-state-standalone-svg-v1",
        mechanisms.clone(),
        |bytes| check_brutalist_state_svg(contract, bytes),
    )?;
    Ok(BrutalistStateSvgProof {
        mechanisms,
        target_proof,
    })
}

fn check_brutalist_state_svg(
    contract: &BrutalistStateFixtureContract<'_>,
    bytes: &[u8],
) -> C6ProofResult<()> {
    let svg = std::str::from_utf8(bytes)
        .map_err(|error| C6ProofError::new("standalone-svg-utf8", error.to_string()))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("standalone-svg-parse", error.to_string()))?;

    let canvas = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .ok_or_else(|| {
            C6ProofError::new("standalone-svg-canvas", "typed canvas base was not emitted")
        })?;
    c6_ensure!(
        "standalone-svg-canvas",
        canvas.attribute("fill") == Some(contract.tokens.background()),
        "canvas fill differs from the background token"
    );

    let state_rects = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "basic")
                && class_contains(*node, "label-container")
        })
        .collect::<Vec<_>>();
    let expected_ordinal_fills = [
        ("Ready", contract.palette_colors[0].as_str()),
        ("Review", contract.palette_colors[1].as_str()),
        ("Done", contract.palette_colors[2].as_str()),
        ("Archive", contract.palette_colors[0].as_str()),
    ];
    c6_ensure!(
        "standalone-svg-state-count",
        state_rects.len() == expected_ordinal_fills.len(),
        "expected {} State nodes, got {}",
        expected_ordinal_fills.len(),
        state_rects.len()
    );
    for (state_id, expected_fill) in expected_ordinal_fills {
        let rect = state_rects
            .iter()
            .copied()
            .find(|node| state_rect_belongs_to(*node, state_id))
            .ok_or_else(|| {
                C6ProofError::new(
                    "standalone-svg-state-node",
                    format!("missing rendered State node `{state_id}`"),
                )
            })?;
        c6_ensure!(
            "standalone-svg-ordinal-fill",
            style_value(rect, "fill") == Some(expected_fill),
            "State ordinal palette mapping is wrong for `{state_id}`"
        );
    }
    let radius = f32::from(contract.radius_px);
    c6_ensure!(
        "standalone-svg-node-style",
        state_rects.iter().all(|node| {
            style_value(*node, "stroke") == Some(contract.border.color())
                && style_number(*node, "stroke-width")
                    == Some(f32::from(contract.border.width_px()))
                && number_attribute(*node, "rx") == Some(radius)
                && number_attribute(*node, "ry") == Some(radius)
        }),
        "one or more State nodes omitted the typed border or radius"
    );

    let effect_filters = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("filter")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains(SHADOW_EFFECT_ID))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "standalone-svg-filter-count",
        effect_filters.len() == state_rects.len(),
        "expected one hard-shadow filter per State node"
    );
    let mut effect_ids = BTreeSet::new();
    for filter in &effect_filters {
        c6_ensure!(
            "standalone-svg-filter-region",
            filter.attribute("filterUnits") == Some("objectBoundingBox"),
            "hard-shadow filter must use objectBoundingBox units"
        );
        c6_ensure!(
            "standalone-svg-filter-color-space",
            filter.attribute("color-interpolation-filters") == Some("linearRGB"),
            "hard-shadow filter does not use linearRGB"
        );
        let primitives = filter
            .children()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        c6_ensure!(
            "standalone-svg-filter-primitives",
            primitives.len() == 1,
            "expected one filter primitive, got {}",
            primitives.len()
        );
        let drop_shadow = primitives[0];
        c6_ensure!(
            "standalone-svg-filter-primitive",
            drop_shadow.has_tag_name("feDropShadow")
                && drop_shadow.attribute("in") == Some("SourceGraphic"),
            "filter primitive is not a SourceGraphic feDropShadow"
        );
        c6_ensure!(
            "standalone-svg-filter-shadow",
            number_attribute(drop_shadow, "dx") == Some(f32::from(contract.shadow.offset_x_px()))
                && number_attribute(drop_shadow, "dy")
                    == Some(f32::from(contract.shadow.offset_y_px()))
                && number_attribute(drop_shadow, "stdDeviation") == Some(0.0)
                && drop_shadow.attribute("flood-color") == Some(contract.shadow.color()),
            "emitted hard shadow differs from the fixture contract"
        );
        let id = filter.attribute("id").ok_or_else(|| {
            C6ProofError::new("standalone-svg-filter-id", "hard-shadow filter has no id")
        })?;
        c6_ensure!(
            "standalone-svg-filter-id",
            effect_ids.insert(id),
            "duplicate hard-shadow filter id `{id}`"
        );
    }
    c6_ensure!(
        "standalone-svg-filter-id",
        effect_ids.len() == state_rects.len(),
        "hard-shadow filter ids are not one-to-one with State nodes"
    );
    let mut referenced_effect_ids = BTreeSet::new();
    for node in &state_rects {
        let id = node
            .attribute("filter")
            .and_then(|value| value.strip_prefix("url(#"))
            .and_then(|value| value.strip_suffix(')'))
            .ok_or_else(|| {
                C6ProofError::new(
                    "standalone-svg-filter-reference",
                    "State node does not reference one hard-shadow filter",
                )
            })?;
        let filter = effect_filters
            .iter()
            .copied()
            .find(|filter| filter.attribute("id") == Some(id))
            .ok_or_else(|| {
                C6ProofError::new(
                    "standalone-svg-filter-reference",
                    format!("State node references missing hard-shadow filter `{id}`"),
                )
            })?;
        prove_state_hard_shadow_filter_region(*node, filter)?;
        referenced_effect_ids.insert(id);
    }
    c6_ensure!(
        "standalone-svg-filter-reference",
        referenced_effect_ids == effect_ids,
        "State node filter references differ from emitted filter ids"
    );

    c6_ensure!(
        "standalone-svg-surface",
        document.descendants().any(|node| {
            node.has_tag_name("rect")
                && style_value(node, "fill") == Some(contract.tokens.surface())
        }),
        "no emitted rect consumes the surface token"
    );
    c6_ensure!(
        "standalone-svg-text-color",
        document.descendants().any(|node| {
            matches!(node.tag_name().name(), "text" | "tspan")
                && style_value(node, "fill") == Some(contract.tokens.text())
        }),
        "no emitted text consumes the text token"
    );

    let root = document.root_element();
    let font_style = root
        .children()
        .find(|node| {
            node.has_tag_name("style") && node.attribute("data-merman-typed-fonts") == Some("v1")
        })
        .ok_or_else(|| {
            C6ProofError::new(
                "standalone-svg-font-style",
                "terminally validated typed font stylesheet was not emitted",
            )
        })?;
    let font_css = font_style.text().unwrap_or_default();
    c6_ensure!(
        "standalone-svg-font-style",
        font_css.contains("@font-face")
            && font_css.contains("data:font/ttf;base64,")
            && font_css.contains(r#"font-family:"Excalifont""#),
        "typed font stylesheet is incomplete"
    );
    c6_ensure!(
        "standalone-svg-text",
        document.descendants().any(|node| node.has_tag_name("text")),
        "standalone SVG contains no text elements"
    );
    c6_ensure!(
        "standalone-svg-foreign-object",
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject")),
        "standalone SVG retained foreignObject text"
    );
    c6_ensure!(
        "standalone-svg-prepared-token",
        !svg.contains("merman-prepared-"),
        "standalone SVG leaked prepared-text tokens"
    );

    Ok(())
}

pub(crate) fn class_contains(node: roxmltree::Node<'_, '_>, class_name: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == class_name)
    })
}

fn state_rect_belongs_to(node: roxmltree::Node<'_, '_>, state_id: &str) -> bool {
    let expected_fragment = format!("-state-{state_id}-");
    node.parent()
        .and_then(|parent| parent.attribute("id"))
        .is_some_and(|id| id.contains(&expected_fragment))
}

pub(crate) fn style_value<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")?.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name.trim() == property).then(|| {
            value
                .trim()
                .strip_suffix("!important")
                .unwrap_or(value.trim())
                .trim()
        })
    })
}

fn style_number(node: roxmltree::Node<'_, '_>, property: &str) -> Option<f32> {
    style_value(node, property)?
        .strip_suffix("px")
        .unwrap_or(style_value(node, property)?)
        .parse()
        .ok()
}

fn number_attribute(node: roxmltree::Node<'_, '_>, attribute: &str) -> Option<f32> {
    node.attribute(attribute)?.parse().ok()
}

fn prove_state_hard_shadow_filter_region(
    rect: roxmltree::Node<'_, '_>,
    filter: roxmltree::Node<'_, '_>,
) -> C6ProofResult<()> {
    let width = required_positive_number_attribute(rect, "width")?;
    let height = required_positive_number_attribute(rect, "height")?;
    let stroke_width = style_number(rect, "stroke-width").ok_or_else(|| {
        C6ProofError::new(
            "standalone-svg-filter-region",
            "State node has no numeric painting stroke width",
        )
    })?;
    c6_ensure!(
        "standalone-svg-filter-region",
        stroke_width.is_finite() && stroke_width >= 0.0,
        "State node has an invalid painting stroke width `{stroke_width}`"
    );

    let drop_shadow = filter
        .children()
        .find(|node| node.is_element() && node.has_tag_name("feDropShadow"))
        .ok_or_else(|| {
            C6ProofError::new(
                "standalone-svg-filter-region",
                "hard-shadow filter has no feDropShadow primitive",
            )
        })?;
    let offset_x = required_number_attribute(drop_shadow, "dx")?;
    let offset_y = required_number_attribute(drop_shadow, "dy")?;
    let region = [
        required_number_attribute(filter, "x")?,
        required_number_attribute(filter, "y")?,
        required_positive_number_attribute(filter, "width")?,
        required_positive_number_attribute(filter, "height")?,
    ];

    let half_stroke = stroke_width / 2.0;
    let expected_outsets = [
        half_stroke + (-offset_y).max(0.0),
        half_stroke + offset_x.max(0.0),
        half_stroke + offset_y.max(0.0),
        half_stroke + (-offset_x).max(0.0),
    ];
    let actual_outsets = [
        -region[1] * height,
        (region[0] + region[2] - 1.0) * width,
        (region[1] + region[3] - 1.0) * height,
        -region[0] * width,
    ];
    for (side, actual, expected) in ["top", "right", "bottom", "left"]
        .into_iter()
        .zip(actual_outsets)
        .zip(expected_outsets)
        .map(|((side, actual), expected)| (side, actual, expected))
    {
        c6_ensure!(
            "standalone-svg-filter-region",
            actual >= expected - 1.0e-4 && actual <= expected + 1.0e-2,
            "hard-shadow {side} outset must tightly contain {expected}px, got {actual}px"
        );
    }
    Ok(())
}

fn required_number_attribute(node: roxmltree::Node<'_, '_>, attribute: &str) -> C6ProofResult<f32> {
    let value = number_attribute(node, attribute).ok_or_else(|| {
        C6ProofError::new(
            "standalone-svg-filter-region",
            format!(
                "{} has no numeric `{attribute}` attribute",
                node.tag_name().name()
            ),
        )
    })?;
    c6_ensure!(
        "standalone-svg-filter-region",
        value.is_finite(),
        "{} has a non-finite `{attribute}` attribute",
        node.tag_name().name()
    );
    Ok(value)
}

fn required_positive_number_attribute(
    node: roxmltree::Node<'_, '_>,
    attribute: &str,
) -> C6ProofResult<f32> {
    let value = required_number_attribute(node, attribute)?;
    c6_ensure!(
        "standalone-svg-filter-region",
        value > 0.0,
        "{} must have a positive `{attribute}` attribute, got {value}",
        node.tag_name().name()
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::fs;
    use tempfile::TempDir;

    fn copied_theme_root() -> TempDir {
        let temp = tempfile::tempdir().expect("create temporary theme root");
        copy_tree(&themes_root(), temp.path());
        temp
    }

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).expect("create fixture directory");
        for entry in fs::read_dir(source).expect("read fixture directory") {
            let entry = entry.expect("read fixture entry");
            let target = destination.join(entry.file_name());
            if entry.file_type().expect("read fixture type").is_dir() {
                copy_tree(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), target).expect("copy fixture file");
            }
        }
    }

    fn write_json(root: &Path, relative: &str, value: &Value) -> String {
        let mut bytes = serde_json::to_vec_pretty(value).expect("serialize fixture JSON");
        bytes.push(b'\n');
        fs::write(root.join(relative), &bytes).expect("write fixture JSON");
        format!("{:x}", Sha256::digest(bytes))
    }

    fn fixture_mut<'a>(manifest: &'a mut Value, id: &str) -> &'a mut Value {
        manifest["fixtures"]
            .as_array_mut()
            .expect("fixture array")
            .iter_mut()
            .find(|fixture| fixture["id"] == id)
            .expect("named fixture")
    }

    fn acceptance_value(theme_catalog: &ThemeFixtureCatalog) -> Value {
        let json = fs::read_to_string(
            theme_catalog
                .root()
                .join(merman_theme_fixtures::C6_ACCEPTANCE_RELATIVE_PATH),
        )
        .expect("read C6 acceptance catalog");
        serde_json::from_str(&json).expect("parse C6 acceptance catalog")
    }

    fn enforced_brutalist_state_native_key(acceptance: &C6AcceptanceCatalog) -> C6RenderGroupKey {
        C6RenderGroupKey::for_cell(
            acceptance
                .enforced_tranche()
                .cells()
                .find(|cell| {
                    cell.key().theme() == C6ProofTheme::Brutalist
                        && cell.key().family() == C6ProofFamily::State
                        && cell.source_fixture_id() == BRUTALIST_STATE_FIXTURE_ID
                        && cell.key().target() == ExpectedOutputTarget::StandaloneSvg
                })
                .expect("enforced Brutalist State native cell"),
        )
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn raster_plan(width: u32, height: u32) -> merman_export::RasterPlan {
        merman_export::RasterPlan {
            requested_width_px: f64::from(width),
            requested_height_px: f64::from(height),
            width_px: width,
            height_px: height,
            requested_scale: 1.0,
            effective_scale: 1.0,
            limited: false,
        }
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn encode_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().expect("write PNG header");
            writer
                .write_image_data(&vec![0; width as usize * height as usize * 4])
                .expect("write PNG image");
        }
        bytes
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn encode_jpeg(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let pixels = vec![0; width as usize * height as usize * 3];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90)
            .encode(&pixels, width, height, image::ExtendedColorType::Rgb8)
            .expect("encode JPEG");
        bytes
    }

    #[test]
    fn native_brutalist_state_cells_share_one_registered_adapter() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");

        let plan = C6ExecutionPlan::from_catalog(&acceptance).expect("plan enforced C6 tranche");

        let group = plan
            .groups
            .iter()
            .find(|group| {
                group.work.key.theme() == C6ProofTheme::Brutalist
                    && group.work.key.family() == C6ProofFamily::State
                    && group.work.key.source_fixture_id() == BRUTALIST_STATE_FIXTURE_ID
            })
            .expect("registered Brutalist State native group");
        assert_eq!(group.adapter.theme, C6ProofTheme::Brutalist);
        assert_eq!(group.adapter.family, C6ProofFamily::State);
        assert_eq!(group.adapter.source_fixture_id, BRUTALIST_STATE_FIXTURE_ID);
        assert_eq!(
            group
                .work
                .cells
                .iter()
                .map(|cell| cell.key().target())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                ExpectedOutputTarget::StandaloneSvg,
                ExpectedOutputTarget::Png,
            ])
        );
        assert!(
            group
                .work
                .cells
                .iter()
                .all(|cell| group.adapter.supports_target(cell.key().target()))
        );
        assert!(
            !group
                .adapter
                .supports_target(ExpectedOutputTarget::BrowserSvg)
        );
    }

    #[test]
    fn positive_c6_family_evidence_rejects_accounted_not_applicable_routes() {
        assert!(is_positive_family_evidence(2, 2, 2, 0, 0));
        assert!(!is_positive_family_evidence(2, 2, 1, 1, 0));
    }

    #[test]
    fn runner_rejects_a_manifest_semantic_assertion_that_the_scenario_did_not_produce() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let mut value = acceptance_value(&theme_catalog);
        let cell = value["cells"]
            .as_array_mut()
            .expect("C6 cells")
            .iter_mut()
            .find(|cell| {
                cell["theme"] == "brutalist" && cell["family"] == "state" && cell["target"] == "png"
            })
            .expect("Brutalist State PNG cell");
        cell["expectation"]["semanticAssertionId"] = json!("brutalist-state-png-v2");
        let acceptance = C6AcceptanceCatalog::from_json(
            &serde_json::to_string(&value).expect("serialize mutated C6 acceptance catalog"),
            &theme_catalog,
        )
        .expect("mutated semantic assertion remains structurally valid");

        let error = run_catalog(&theme_catalog, &acceptance)
            .expect_err("runner must bind the scenario assertion to the manifest");

        assert!(matches!(
            error,
            C6RuntimeError::EvidenceMismatch {
                key,
                field: "semantic-assertion-id",
            } if key.theme() == C6ProofTheme::Brutalist
                && key.family() == C6ProofFamily::State
                && key.target() == ExpectedOutputTarget::Png
        ));
    }

    #[test]
    fn duplicate_render_group_adapter_identity_fails_closed() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");
        let key = enforced_brutalist_state_native_key(&acceptance);
        let adapters = [BRUTALIST_STATE_ADAPTER; 2];

        let error = adapter_for_group(&adapters, &key, "brutalist-state-v1")
            .expect_err("duplicate adapter identity must fail closed");

        assert!(matches!(
            error,
            C6RuntimeError::AmbiguousRenderGroupAdapter { group }
                if group == key.label()
        ));
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn target_proof_failure_retains_artifact_and_dependent_targets() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");
        let key = enforced_brutalist_state_native_key(&acceptance);
        let required_by = [ExpectedOutputTarget::Png];

        let error = prove_target::<()>(
            &key,
            ExpectedOutputTarget::Png,
            &required_by,
            Err(C6ProofError::new(
                "png-decode",
                "failed to decode the C6 PNG artifact",
            )),
        )
        .expect_err("invalid artifacts must fail with structured runtime evidence");

        assert!(matches!(
            error,
            C6RuntimeError::RenderGroupTargetProofFailed {
                group,
                artifact_target: ExpectedOutputTarget::Png,
                required_by,
                stage: "png-decode",
                detail,
            } if group == key.label()
                && required_by == [ExpectedOutputTarget::Png]
                && detail.contains("C6 PNG artifact")
        ));
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn malformed_artifacts_fail_through_target_proof_boundaries() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");
        let key = enforced_brutalist_state_native_key(&acceptance);

        let png_required_by = [ExpectedOutputTarget::Png];
        let png = prove_target(
            &key,
            ExpectedOutputTarget::Png,
            &png_required_by,
            c6_raster_proof::RasterImage::decode_png(b"\x89PNG\r\n\x1a\n", raster_plan(1, 1)),
        )
        .expect_err("a truncated PNG must fail closed");
        assert!(matches!(
            png,
            C6RuntimeError::RenderGroupTargetProofFailed {
                artifact_target: ExpectedOutputTarget::Png,
                required_by,
                stage: "png-decode",
                detail,
                ..
            } if required_by == png_required_by && !detail.is_empty()
        ));

        let jpeg_required_by = [ExpectedOutputTarget::Jpeg];
        let jpeg = prove_target(
            &key,
            ExpectedOutputTarget::Jpeg,
            &jpeg_required_by,
            c6_raster_proof::RasterImage::decode_jpeg(&[0xff, 0xd8, 0xff, 0xd9], raster_plan(1, 1)),
        )
        .expect_err("a truncated JPEG must fail closed");
        assert!(matches!(
            jpeg,
            C6RuntimeError::RenderGroupTargetProofFailed {
                artifact_target: ExpectedOutputTarget::Jpeg,
                required_by,
                stage: "jpeg-decode",
                detail,
                ..
            } if required_by == jpeg_required_by && !detail.is_empty()
        ));

        let mut malformed_pdf = b"%PDF-1.7\n".to_vec();
        malformed_pdf.resize(1030, b'x');
        malformed_pdf.extend_from_slice(b"\nstartxref\n0\n%%EOF");
        let pdf_required_by = [ExpectedOutputTarget::Pdf];
        let pdf = prove_target(
            &key,
            ExpectedOutputTarget::Pdf,
            &pdf_required_by,
            c6_pdf_proof::prove_brutalist_state_pdf(&malformed_pdf),
        )
        .expect_err("a malformed PDF must fail closed");
        assert!(matches!(
            pdf,
            C6RuntimeError::RenderGroupTargetProofFailed {
                artifact_target: ExpectedOutputTarget::Pdf,
                required_by,
                stage: "pdf-artifact",
                detail,
                ..
            } if required_by == pdf_required_by && !detail.is_empty()
        ));
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn raster_decoders_reject_plan_drift_and_truncated_jpeg_envelopes() {
        let png_error =
            c6_raster_proof::RasterImage::decode_png(&encode_png(2, 1), raster_plan(1, 1))
                .expect_err("PNG dimensions outside the frozen raster plan must fail");
        assert!(
            png_error
                .to_string()
                .contains("artifact dimensions do not match the frozen raster plan")
        );

        let jpeg = encode_jpeg(2, 1);
        let jpeg_error = c6_raster_proof::RasterImage::decode_jpeg(&jpeg, raster_plan(1, 1))
            .expect_err("JPEG dimensions outside the frozen raster plan must fail");
        assert!(
            jpeg_error
                .to_string()
                .contains("artifact dimensions do not match the frozen raster plan")
        );

        let mut truncated = encode_jpeg(1, 1);
        assert_eq!(truncated.split_off(truncated.len() - 2), [0xff, 0xd9]);
        let envelope_error =
            c6_raster_proof::RasterImage::decode_jpeg(&truncated, raster_plan(1, 1))
                .expect_err("missing JPEG EOI must fail before tolerant decoding");
        assert!(envelope_error.to_string().contains("SOI and EOI envelope"));

        let oversized_plan = raster_plan(merman_export::DEFAULT_MAX_RASTER_SIDE_LENGTH + 1, 1);
        let budget_error = c6_raster_proof::RasterImage::decode_jpeg(&jpeg, oversized_plan)
            .expect_err("an over-budget raster plan must fail before decoding");
        assert!(budget_error.to_string().contains("default side limit"));
    }

    #[test]
    fn render_group_adapter_identity_requires_every_dimension() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");
        let key = enforced_brutalist_state_native_key(&acceptance);

        assert!(
            adapter_for_group(&[BRUTALIST_STATE_ADAPTER], &key, "brutalist-state-v1",)
                .expect("match exact adapter identity")
                .is_some()
        );

        let mismatched_adapters = [
            (
                "theme",
                C6RenderGroupAdapter {
                    theme: C6ProofTheme::Spotless,
                    ..BRUTALIST_STATE_ADAPTER
                },
            ),
            (
                "family",
                C6RenderGroupAdapter {
                    family: C6ProofFamily::Flowchart,
                    ..BRUTALIST_STATE_ADAPTER
                },
            ),
            (
                "fixture",
                C6RenderGroupAdapter {
                    source_fixture_id: "fixture-c6-brutalist-state-other",
                    ..BRUTALIST_STATE_ADAPTER
                },
            ),
            (
                "recipe revision",
                C6RenderGroupAdapter {
                    proof_recipe_revision: "brutalist-state-v2",
                    ..BRUTALIST_STATE_ADAPTER
                },
            ),
        ];

        for (dimension, adapter) in mismatched_adapters {
            assert!(
                adapter_for_group(std::slice::from_ref(&adapter), &key, "brutalist-state-v1",)
                    .expect("evaluate adapter identity")
                    .is_none(),
                "{dimension} mismatch must not select the adapter"
            );
        }
    }

    #[test]
    fn runner_reports_fixture_contract_failures_without_panicking() {
        let temp = copied_theme_root();
        let input_relative = "inputs/c6-brutalist-state.json";
        let mut input: Value = serde_json::from_str(
            &fs::read_to_string(temp.path().join(input_relative)).expect("read theme input"),
        )
        .expect("parse theme input");
        input["nodeStyle"]["shadow"]["offsetXPx"] = json!(-6);
        let input_hash = write_json(temp.path(), input_relative, &input);

        let manifest_path = temp.path().join("manifest.json");
        let mut manifest: Value = serde_json::from_str(
            &fs::read_to_string(&manifest_path).expect("read fixture manifest"),
        )
        .expect("parse fixture manifest");
        fixture_mut(&mut manifest, BRUTALIST_STATE_FIXTURE_ID)["themeInputSha256"] =
            json!(input_hash);
        write_json(temp.path(), "manifest.json", &manifest);

        let theme_catalog = ThemeFixtureCatalog::load(temp.path())
            .expect("mutated catalog remains structurally valid");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog)
            .expect("mutated catalog remains acceptance-valid");
        let error = run_catalog(&theme_catalog, &acceptance)
            .expect_err("runner contract drift must fail closed");

        assert!(matches!(
            error,
            C6RuntimeError::RenderGroupProofFailed {
                stage: "fixture-contract",
                detail,
                ..
            } if detail.contains("positive hard-shadow offsets")
        ));
    }

    #[test]
    fn public_runner_reports_catalog_hash_failures_without_panicking() {
        let temp = copied_theme_root();
        let input_path = temp.path().join("inputs/c6-brutalist-state.json");
        fs::write(&input_path, b"{}\n").expect("corrupt theme input without updating its hash");

        let error = run_enforced_c6_runtime_from_root(temp.path())
            .expect_err("catalog hash drift must fail closed");

        assert!(matches!(error, C6RuntimeError::Catalog(_)));
    }

    #[test]
    fn loaded_catalog_source_drift_retains_render_group_identity() {
        let temp = copied_theme_root();
        let theme_catalog = ThemeFixtureCatalog::load(temp.path()).expect("load theme fixtures");
        let acceptance =
            C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance catalog");
        let key = enforced_brutalist_state_native_key(&acceptance);
        fs::write(
            temp.path().join("sources/c6-brutalist-state.mmd"),
            b"stateDiagram-v2\n[*] --> Drifted\n",
        )
        .expect("drift fixture source after catalog loading");

        let error = run_catalog(&theme_catalog, &acceptance)
            .expect_err("post-load source drift must fail with render-group context");

        assert!(matches!(
            error,
            C6RuntimeError::RenderGroupProofFailed {
                group,
                stage: "source-fixture-read",
                detail,
            } if group == key.label() && !detail.is_empty()
        ));
    }
}
