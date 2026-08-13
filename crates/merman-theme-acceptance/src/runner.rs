use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, DocumentRenderReport, EffectBinding, EffectGraph, EffectInput,
    EffectPrimitive, FilterRegion, FontAssetSpec, FontCatalogSpec, FontEmbeddingRequirement,
    FontSource, FontStack, OrdinalPalette, RenderEnvironment, RenderTargetKind, RenderedDocument,
    Specified, TargetAdmissionReport, TargetAdmissionStatus, ThemeAssets, ThemeCapability,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRecipeFingerprint, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman::{DiagramFamilyId, Engine, MermaidConfig};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6EnforcedCell, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget,
    ReferenceBorderInput, ReferenceCanvasLayer, ReferenceDiagramFamily, ReferenceFontBinding,
    ReferenceFontStack, ReferenceSemanticRule, ReferenceSemanticTarget, ReferenceShadowInput,
    ReferenceThemeInput, ReferenceThemeMechanism, ReferenceThemeTokens, ThemeFixtureCatalog,
};

use crate::observation::{
    C6CellReceipt, C6ExecutionReport, C6ObservedMechanismDisposition, C6ReceiptBook,
    C6RenderGroupKey, C6RenderGroupReceipt, C6RenderLane, C6RuntimeError, C6TargetProof,
    seal_cell_from_reports,
};

#[derive(Debug, thiserror::Error)]
#[error("{stage}: {detail}")]
pub(super) struct C6ProofError {
    stage: &'static str,
    detail: String,
}

impl C6ProofError {
    pub(super) fn new(stage: &'static str, detail: impl Into<String>) -> Self {
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
}

pub(super) type C6ProofResult<T> = Result<T, C6ProofError>;

macro_rules! c6_ensure {
    ($stage:expr, $condition:expr, $($arg:tt)+) => {
        if !$condition {
            return Err(C6ProofError::new($stage, format!($($arg)+)));
        }
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

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[path = "support/c6_raster_proof.rs"]
mod c6_raster_proof;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[path = "support/c6_pdf_proof.rs"]
mod c6_pdf_proof;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use c6_pdf_proof::prove_brutalist_state_pdf;
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use c6_raster_proof::{PngArtifactProof, prove_brutalist_state_jpeg, prove_brutalist_state_png};

const SHADOW_EFFECT_ID: &str = "c6-brutalist-state-shadow";
const BRUTALIST_STATE_FIXTURE_ID: &str = "fixture-c6-brutalist-state";

struct BrutalistStateFixtureContract<'a> {
    tokens: &'a ReferenceThemeTokens,
    font_stack: &'a ReferenceFontStack,
    font_family: &'a str,
    font_asset_id: &'a str,
    canvas_color: &'a str,
    border: &'a ReferenceBorderInput,
    radius_px: u16,
    palette_colors: &'a [String],
    shadow: &'a ReferenceShadowInput,
    mechanisms: BTreeSet<ReferenceThemeMechanism>,
}

impl<'a> BrutalistStateFixtureContract<'a> {
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
        let font_family = font_stack.families().first().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist font family set is empty")
        })?;
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

        let node_style = input.node_style().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires a State node style")
        })?;
        c6_ensure!(
            "fixture-contract",
            node_style.dash_pattern().is_empty(),
            "Brutalist State node style must not use a dash pattern"
        );
        let border = node_style.border().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires a node border")
        })?;
        let radius_px = node_style.corner_radius_px().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires rounded State nodes")
        })?;
        let shadow = node_style.shadow().ok_or_else(|| {
            C6ProofError::new("fixture-contract", "Brutalist requires a hard shadow")
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
                    "ordinal palette targets {target:?}, not State nodes"
                );
                colors.as_slice()
            }
            _ => {
                return Err(C6ProofError::new(
                    "fixture-contract",
                    "Brutalist proof requires exactly one State node ordinal palette",
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
            palette_colors.first().map(String::as_str) == Some(tokens.primary()),
            "first ordinal color does not match the primary token"
        );

        Ok(Self {
            tokens,
            font_stack,
            font_family,
            font_asset_id,
            canvas_color,
            border,
            radius_px,
            palette_colors,
            shadow,
            mechanisms: input.mechanisms(),
        })
    }
}

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

pub fn run_enforced_c6_runtime() -> Result<C6ExecutionReport, C6RuntimeError> {
    run_enforced_c6_runtime_from_root(themes_root())
}

fn run_enforced_c6_runtime_from_root(
    root: impl AsRef<Path>,
) -> Result<C6ExecutionReport, C6RuntimeError> {
    let theme_catalog = ThemeFixtureCatalog::load(root)?;
    let acceptance = C6AcceptanceCatalog::load(&theme_catalog)?;
    run_catalog(&theme_catalog, &acceptance)
}

fn run_catalog(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
) -> Result<C6ExecutionReport, C6RuntimeError> {
    let plan = C6ExecutionPlan::from_catalog(&acceptance)?;
    let expected_keys = acceptance
        .enforced_tranche()
        .cells()
        .map(C6EnforcedCell::key)
        .collect::<Vec<_>>();
    let mut group_receipts = Vec::with_capacity(plan.groups.len());
    let mut cell_receipts = Vec::with_capacity(expected_keys.len());
    for group in plan.groups {
        let completed = execute_render_group(theme_catalog, group)?;
        group_receipts.push(completed.receipt);
        cell_receipts.extend(completed.cells);
    }
    C6ReceiptBook::from_receipts(expected_keys, group_receipts, cell_receipts)?
        .evaluate(acceptance, theme_catalog)
}

type C6RenderGroupExecutor =
    fn(&ThemeFixtureCatalog, C6RenderGroupWork) -> Result<C6CompletedRenderGroup, C6RuntimeError>;

#[derive(Clone, Copy, Debug)]
struct C6RenderGroupAdapter {
    theme: C6ProofTheme,
    family: C6ProofFamily,
    source_fixture_id: &'static str,
    lane: C6RenderLane,
    supported_targets: &'static [ExpectedOutputTarget],
    execute: C6RenderGroupExecutor,
}

impl C6RenderGroupAdapter {
    fn supports_target(&self, target: ExpectedOutputTarget) -> bool {
        self.supported_targets.contains(&target)
    }

    fn matches_group(&self, key: &C6RenderGroupKey) -> bool {
        self.theme == key.theme()
            && self.family == key.family()
            && self.source_fixture_id == key.source_fixture_id()
            && self.lane == key.lane()
    }
}

const BRUTALIST_STATE_ADAPTER: C6RenderGroupAdapter = C6RenderGroupAdapter {
    theme: C6ProofTheme::Brutalist,
    family: C6ProofFamily::State,
    source_fixture_id: BRUTALIST_STATE_FIXTURE_ID,
    lane: C6RenderLane::Native,
    supported_targets: &[
        ExpectedOutputTarget::StandaloneSvg,
        ExpectedOutputTarget::Png,
        ExpectedOutputTarget::Jpeg,
        ExpectedOutputTarget::Pdf,
    ],
    execute: execute_brutalist_state_group,
};

static C6_RENDER_GROUP_ADAPTERS: &[C6RenderGroupAdapter] = &[BRUTALIST_STATE_ADAPTER];

#[derive(Debug)]
struct C6RenderGroupWork {
    key: C6RenderGroupKey,
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
            let adapter = adapter_for_group(C6_RENDER_GROUP_ADAPTERS, &key)?
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
                    work: C6RenderGroupWork { key, cells },
                })
                .collect(),
        })
    }
}

fn adapter_for_group<'a>(
    adapters: &'a [C6RenderGroupAdapter],
    key: &C6RenderGroupKey,
) -> Result<Option<&'a C6RenderGroupAdapter>, C6RuntimeError> {
    let mut matching = adapters.iter().filter(|adapter| adapter.matches_group(key));
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
    group: C6RenderGroupPlan,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    (group.adapter.execute)(theme_catalog, group.work)
}

fn execute_brutalist_state_group(
    theme_catalog: &ThemeFixtureCatalog,
    group: C6RenderGroupWork,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let fixture = theme_catalog
        .fixture(group.key.source_fixture_id())
        .ok_or_else(|| render_group_error(&group.key, "source-fixture"))?;
    let input = fixture
        .theme_input()
        .ok_or_else(|| render_group_error(&group.key, "theme-input"))?;
    let source = theme_catalog.source_text(fixture.id()).map_err(|error| {
        C6RuntimeError::RenderGroupProofFailed {
            group: group.key.label(),
            stage: "source-fixture-read",
            detail: error.to_string(),
        }
    })?;
    let contract =
        prove_render_group(&group.key, BrutalistStateFixtureContract::from_input(input))?;
    let theme = prove_render_group(
        &group.key,
        compile_brutalist_state_theme(theme_catalog, &contract),
    )?;
    prove_render_group(&group.key, prove_brutalist_state_capabilities(&theme))?;

    let document =
        prove_render_group(&group.key, render_brutalist_state_document(&source, &theme))?;
    prove_render_group(
        &group.key,
        prove_brutalist_state_document(&document, &theme),
    )?;
    let admitted =
        document
            .clone()
            .admit_svg()
            .map_err(|report| C6RuntimeError::RenderGroupProofFailed {
                group: group.key.label(),
                stage: "standalone-svg-admission",
                detail: report.to_string(),
            })?;
    let sealed_svg = admitted.as_str();
    let mechanisms =
        prove_render_group(&group.key, prove_brutalist_state_svg(&contract, sealed_svg))?;
    prove_render_group(
        &group.key,
        prove_mechanism_coverage(&contract.mechanisms, &mechanisms),
    )?;
    let group_receipt = C6RenderGroupReceipt::seal(
        group.key.clone(),
        theme_catalog,
        document.family_id(),
        document.theme_recipe_fingerprint(),
        sealed_svg,
        admitted.document_report(),
    )?;

    let mut png_dependents = group
        .cells
        .iter()
        .map(|cell| cell.key().target())
        .filter(|target| {
            matches!(
                target,
                ExpectedOutputTarget::Png | ExpectedOutputTarget::Jpeg
            )
        })
        .collect::<Vec<_>>();
    png_dependents.sort();
    png_dependents.dedup();
    let png_support = (!png_dependents.is_empty())
        .then(|| {
            observe_brutalist_state_png(
                &group.key,
                &png_dependents,
                &contract,
                sealed_svg,
                &theme,
                &document,
            )
        })
        .transpose()?;

    let mut cells = Vec::with_capacity(group.cells.len());
    for enforced in &group.cells {
        let observation = match enforced.key().target() {
            ExpectedOutputTarget::StandaloneSvg => C6TargetObservation {
                recipe_fingerprint: document.theme_recipe_fingerprint(),
                document: admitted.document_report().clone(),
                admission: admitted.target_admission().clone(),
                proof: C6TargetProof::brutalist_state_standalone_svg(
                    sealed_svg.as_bytes(),
                    mechanisms.clone(),
                ),
            },
            ExpectedOutputTarget::Png => png_support
                .as_ref()
                .ok_or_else(|| render_group_error(&group.key, "png-support-plan"))?
                .0
                .clone(),
            ExpectedOutputTarget::Jpeg => observe_brutalist_state_jpeg(
                &group.key,
                &theme,
                &document,
                &png_support
                    .as_ref()
                    .ok_or_else(|| render_group_error(&group.key, "jpeg-png-control-plan"))?
                    .1,
            )?,
            ExpectedOutputTarget::Pdf => {
                observe_brutalist_state_pdf(&group.key, &contract, sealed_svg, &theme, &document)?
            }
            ExpectedOutputTarget::BrowserSvg => {
                return Err(C6RuntimeError::UnsupportedEnforcedCell {
                    key: enforced.key(),
                });
            }
        };
        cells.push(observation.seal(enforced, &group_receipt, sealed_svg)?);
    }

    Ok(C6CompletedRenderGroup {
        receipt: group_receipt,
        cells,
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
    recipe_fingerprint: Option<ThemeRecipeFingerprint>,
    document: DocumentRenderReport,
    admission: TargetAdmissionReport,
    proof: C6TargetProof,
}

impl C6TargetObservation {
    fn seal(
        self,
        enforced: &C6EnforcedCell,
        group: &C6RenderGroupReceipt,
        sealed_svg: &str,
    ) -> Result<C6CellReceipt, C6RuntimeError> {
        seal_cell_from_reports(
            enforced,
            group,
            self.recipe_fingerprint,
            sealed_svg,
            &self.document,
            &self.admission,
            self.proof,
        )
    }
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
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

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
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

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_brutalist_state_document(
    document: &RenderedDocument,
    theme: &DiagramTheme,
) -> C6ProofResult<()> {
    let recipe = theme.recipe_fingerprint();
    c6_ensure!(
        "document-family",
        document.family_id() == DiagramFamilyId::STATE,
        "expected State family, got {}",
        document.family_id()
    );
    c6_ensure!(
        "document-recipe",
        document.theme_recipe_fingerprint() == Some(recipe),
        "document recipe fingerprint does not match the compiled theme"
    );
    c6_ensure!(
        "document-recipe-report",
        document
            .theme_recipe_report()
            .map(|report| report.theme_recipe_fingerprint())
            == Some(recipe),
        "document recipe report does not match the compiled theme"
    );
    c6_ensure!(
        "root-theme-report",
        document.root_theme_report().is_verified(),
        "root theme report is not verified"
    );
    c6_ensure!(
        "document-residuals",
        document.document_report().residuals().is_empty(),
        "document retained residuals: {:?}",
        document.document_report().residuals()
    );
    c6_ensure!(
        "document-font-source",
        document.document_report().prepared_text_used_font_sources() == [FontSource::Embedded],
        "prepared text did not exclusively use embedded fonts: {:?}",
        document.document_report().prepared_text_used_font_sources()
    );
    c6_ensure!(
        "document-text-measurement",
        document.document_report().host_text_measurement_count() == 0,
        "document used host text measurement"
    );
    c6_ensure!(
        "document-resource-closure",
        document.resource_closure().is_closed(),
        "document resource closure is open"
    );
    c6_ensure!(
        "document-svg-admission",
        document.svg_target_admission().status() == TargetAdmissionStatus::Portable,
        "SVG admission is {:?}",
        document.svg_target_admission().status()
    );
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_png(
    key: &C6RenderGroupKey,
    required_by: &[ExpectedOutputTarget],
    contract: &BrutalistStateFixtureContract<'_>,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
) -> Result<(C6TargetObservation, PngArtifactProof), C6RuntimeError> {
    let resource_fingerprint = document.resource_fingerprint();
    let prepared = document
        .clone()
        .prepare_png_export(&merman::svg::export::RasterOptions::default().with_scale(2.0))
        .map_err(|error| C6ProofError::new("png-prepare", error.to_string()))
        .map_err(|error| error.into_target_runtime(key, ExpectedOutputTarget::Png, required_by))?;
    let prepared_export = prepared.export_report();
    prove_target(
        key,
        ExpectedOutputTarget::Png,
        required_by,
        prove_native_export_preparation(
            RenderTargetKind::Png,
            prepared.target_admission(),
            prepared_export.resource_fingerprint(),
            prepared_export.conversion(),
            prepared_export.fonts(),
            resource_fingerprint,
        ),
    )?;
    let native_filter_receipt = prove_target(
        key,
        ExpectedOutputTarget::Png,
        required_by,
        prepared_export.native_filter_receipt().ok_or_else(|| {
            C6ProofError::new(
                "png-native-filter-receipt",
                "portable typed hard-shadow export did not retain an exact receipt",
            )
        }),
    )?;

    let (png, report) = prepared
        .encode()
        .map_err(|error| C6ProofError::new("png-encode", error.to_string()))
        .map_err(|error| error.into_target_runtime(key, ExpectedOutputTarget::Png, required_by))?;
    let proof = prove_target(
        key,
        ExpectedOutputTarget::Png,
        required_by,
        prove_brutalist_state_png(contract, sealed_svg, &png, prepared_export.raster()),
    )?;
    let target_proof = proof.target_proof(&png);
    prove_target(
        key,
        ExpectedOutputTarget::Png,
        required_by,
        prove_native_export_report(
            report.document_report(),
            report.operation_report().theme_recipe_fingerprint(),
            report.target_admission().status(),
            report.export_report().resource_fingerprint(),
            report.export_report().native_filter_receipt() == Some(native_filter_receipt),
            resource_fingerprint,
            theme.recipe_fingerprint(),
        ),
    )?;
    Ok((
        C6TargetObservation {
            recipe_fingerprint: report.operation_report().theme_recipe_fingerprint(),
            document: report.document_report().clone(),
            admission: report.target_admission().clone(),
            proof: target_proof,
        },
        proof,
    ))
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_jpeg(
    key: &C6RenderGroupKey,
    theme: &DiagramTheme,
    document: &RenderedDocument,
    png_proof: &PngArtifactProof,
) -> Result<C6TargetObservation, C6RuntimeError> {
    const REQUIRED_BY: [ExpectedOutputTarget; 1] = [ExpectedOutputTarget::Jpeg];
    let resource_fingerprint = document.resource_fingerprint();
    let prepared = document
        .clone()
        .prepare_jpeg_export(&merman::svg::export::RasterOptions::default().with_scale(2.0))
        .map_err(|error| C6ProofError::new("jpeg-prepare", error.to_string()))
        .map_err(|error| {
            error.into_target_runtime(key, ExpectedOutputTarget::Jpeg, &REQUIRED_BY)
        })?;
    let prepared_export = prepared.export_report();
    prove_target(
        key,
        ExpectedOutputTarget::Jpeg,
        &REQUIRED_BY,
        prove_native_export_preparation(
            RenderTargetKind::Jpeg,
            prepared.target_admission(),
            prepared_export.resource_fingerprint(),
            prepared_export.conversion(),
            prepared_export.fonts(),
            resource_fingerprint,
        ),
    )?;
    let native_filter_receipt = prove_target(
        key,
        ExpectedOutputTarget::Jpeg,
        &REQUIRED_BY,
        prepared_export.native_filter_receipt().ok_or_else(|| {
            C6ProofError::new(
                "jpeg-native-filter-receipt",
                "portable typed hard-shadow export did not retain an exact receipt",
            )
        }),
    )?;

    let (jpeg, report) = prepared
        .encode()
        .map_err(|error| C6ProofError::new("jpeg-encode", error.to_string()))
        .map_err(|error| {
            error.into_target_runtime(key, ExpectedOutputTarget::Jpeg, &REQUIRED_BY)
        })?;
    let proof = prove_target(
        key,
        ExpectedOutputTarget::Jpeg,
        &REQUIRED_BY,
        prove_brutalist_state_jpeg(&jpeg, prepared_export.raster(), png_proof),
    )?;
    prove_target(
        key,
        ExpectedOutputTarget::Jpeg,
        &REQUIRED_BY,
        prove_native_export_report(
            report.document_report(),
            report.operation_report().theme_recipe_fingerprint(),
            report.target_admission().status(),
            report.export_report().resource_fingerprint(),
            report.export_report().native_filter_receipt() == Some(native_filter_receipt),
            resource_fingerprint,
            theme.recipe_fingerprint(),
        ),
    )?;
    Ok(C6TargetObservation {
        recipe_fingerprint: report.operation_report().theme_recipe_fingerprint(),
        document: report.document_report().clone(),
        admission: report.target_admission().clone(),
        proof,
    })
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_pdf(
    key: &C6RenderGroupKey,
    contract: &BrutalistStateFixtureContract<'_>,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
) -> Result<C6TargetObservation, C6RuntimeError> {
    const REQUIRED_BY: [ExpectedOutputTarget; 1] = [ExpectedOutputTarget::Pdf];
    let resource_fingerprint = document.resource_fingerprint();
    let prepared = document
        .clone()
        .prepare_pdf_export(&merman::svg::export::PdfOptions::default())
        .map_err(|error| C6ProofError::new("pdf-prepare", error.to_string()))
        .map_err(|error| error.into_target_runtime(key, ExpectedOutputTarget::Pdf, &REQUIRED_BY))?;
    let prepared_export = prepared.export_report();
    prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prove_native_export_preparation(
            RenderTargetKind::Pdf,
            prepared.target_admission(),
            prepared_export.resource_fingerprint(),
            prepared_export.conversion(),
            prepared_export.fonts(),
            resource_fingerprint,
        ),
    )?;
    let filter_plan = prepared_export.filters();
    prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prove_pdf_filter_plan(filter_plan, prepared_export.native_filter_fully_localized()),
    )?;
    let native_filter_receipt = prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prepared_export.native_filter_receipt().ok_or_else(|| {
            C6ProofError::new(
                "pdf-native-filter-receipt",
                "portable typed hard-shadow export did not retain an exact receipt",
            )
        }),
    )?;

    let (pdf, report) = prepared
        .encode()
        .map_err(|error| C6ProofError::new("pdf-encode", error.to_string()))
        .map_err(|error| error.into_target_runtime(key, ExpectedOutputTarget::Pdf, &REQUIRED_BY))?;
    prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prove_pdf_artifact(&pdf),
    )?;
    let proof = prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prove_brutalist_state_pdf(contract, sealed_svg, &pdf, filter_plan.effective_scale),
    )?;
    prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prove_native_export_report(
            report.document_report(),
            report.operation_report().theme_recipe_fingerprint(),
            report.target_admission().status(),
            report.export_report().resource_fingerprint(),
            report.export_report().native_filter_receipt() == Some(native_filter_receipt),
            resource_fingerprint,
            theme.recipe_fingerprint(),
        ),
    )?;
    prove_target(
        key,
        ExpectedOutputTarget::Pdf,
        &REQUIRED_BY,
        prove_pdf_filter_localization(report.export_report().native_filter_fully_localized()),
    )?;
    Ok(C6TargetObservation {
        recipe_fingerprint: report.operation_report().theme_recipe_fingerprint(),
        document: report.document_report().clone(),
        admission: report.target_admission().clone(),
        proof,
    })
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_native_export_preparation(
    target: RenderTargetKind,
    admission: &TargetAdmissionReport,
    export_fingerprint: merman::svg::SvgResourceFingerprint,
    conversion: merman::svg::export::SvgConversionPlan,
    fonts: merman::svg::export::ExportFontPlan,
    expected_resource_fingerprint: merman::svg::SvgResourceFingerprint,
) -> C6ProofResult<()> {
    c6_ensure!(
        "native-export-target",
        admission.target() == target,
        "expected target {target:?}, got {:?}",
        admission.target()
    );
    c6_ensure!(
        "native-export-resource",
        export_fingerprint == expected_resource_fingerprint,
        "prepared export resource fingerprint differs from the rendered document"
    );
    prove_native_filter_conversion(conversion)?;
    prove_export_font_plan(fonts)
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_native_export_report(
    document: &merman::svg::DocumentRenderReport,
    recipe_fingerprint: Option<merman::svg::ThemeRecipeFingerprint>,
    admission: TargetAdmissionStatus,
    export_fingerprint: merman::svg::SvgResourceFingerprint,
    native_filter_receipt_matches: bool,
    expected_resource_fingerprint: merman::svg::SvgResourceFingerprint,
    expected_recipe_fingerprint: merman::svg::ThemeRecipeFingerprint,
) -> C6ProofResult<()> {
    c6_ensure!(
        "native-report-residuals",
        document.residuals().is_empty(),
        "native document retained residuals: {:?}",
        document.residuals()
    );
    c6_ensure!(
        "native-report-font-source",
        document.prepared_text_used_font_sources() == [FontSource::Embedded],
        "native document did not exclusively use embedded fonts: {:?}",
        document.prepared_text_used_font_sources()
    );
    c6_ensure!(
        "native-report-text-measurement",
        document.host_text_measurement_count() == 0,
        "native document used host text measurement"
    );
    c6_ensure!(
        "native-report-document-resource",
        document.resource_fingerprint() == expected_resource_fingerprint,
        "native document resource fingerprint differs from the rendered document"
    );
    c6_ensure!(
        "native-report-export-resource",
        export_fingerprint == expected_resource_fingerprint,
        "encoded export resource fingerprint differs from the rendered document"
    );
    c6_ensure!(
        "native-report-recipe",
        recipe_fingerprint == Some(expected_recipe_fingerprint),
        "encoded export recipe fingerprint differs from the compiled theme"
    );
    c6_ensure!(
        "native-report-admission",
        admission == TargetAdmissionStatus::Portable,
        "encoded export admission is {admission:?}"
    );
    c6_ensure!(
        "native-report-filter-receipt",
        native_filter_receipt_matches,
        "encoded export native filter receipt differs from preflight"
    );
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_pdf_filter_plan(
    plan: merman::svg::export::PdfFilterImagePlan,
    fully_localized: bool,
) -> C6ProofResult<()> {
    c6_ensure!(
        "pdf-filter-plan",
        plan.filtered_groups > 0,
        "PDF export did not retain filtered groups"
    );
    c6_ensure!(
        "pdf-filter-plan",
        plan.effective_image_pixels > 0,
        "PDF export planned no filter image pixels"
    );
    prove_pdf_filter_localization(fully_localized)
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_pdf_filter_localization(fully_localized: bool) -> C6ProofResult<()> {
    c6_ensure!(
        "pdf-filter-localization",
        fully_localized,
        "PDF export did not fully localize native filters"
    );
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_pdf_artifact(bytes: &[u8]) -> C6ProofResult<()> {
    c6_ensure!(
        "pdf-envelope",
        bytes.starts_with(b"%PDF-"),
        "artifact is missing the PDF header"
    );
    c6_ensure!(
        "pdf-envelope",
        bytes.len() > 1024,
        "artifact is implausibly small: {} bytes",
        bytes.len()
    );
    c6_ensure!(
        "pdf-envelope",
        bytes
            .windows(b"startxref".len())
            .any(|window| window == b"startxref"),
        "artifact is missing startxref"
    );
    let trimmed = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    c6_ensure!(
        "pdf-envelope",
        trimmed.ends_with(b"%%EOF"),
        "artifact is missing the PDF EOF marker"
    );
    Ok(())
}

fn render_brutalist_state_document(
    source: &str,
    theme: &DiagramTheme,
) -> C6ProofResult<RenderedDocument> {
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable);
    merman::svg::HeadlessRenderer::from_engine_and_environment(Engine::new(), environment)
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false
        })))
        .with_theme(theme.clone())
        .render_document_sync(source)
        .map_err(|error| C6ProofError::new("render-document", error.to_string()))?
        .ok_or_else(|| {
            C6ProofError::new(
                "render-document",
                "enforced source did not contain a rendered Mermaid document",
            )
        })
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_export_font_plan(fonts: merman::svg::export::ExportFontPlan) -> C6ProofResult<()> {
    c6_ensure!(
        "export-fonts",
        fonts.used_embedded_fonts(),
        "no embedded font was used"
    );
    c6_ensure!(
        "export-fonts",
        !fonts.used_system_fonts(),
        "a system font was used"
    );
    c6_ensure!(
        "export-fonts",
        fonts.prepared_label_expected_count() > 0,
        "no prepared labels were expected"
    );
    c6_ensure!(
        "export-fonts",
        fonts.prepared_label_verified_count() == fonts.prepared_label_expected_count(),
        "verified prepared labels differ from expected: verified={}, expected={}",
        fonts.prepared_label_verified_count(),
        fonts.prepared_label_expected_count()
    );
    c6_ensure!(
        "export-fonts",
        fonts.prepared_label_mismatch_count() == 0,
        "prepared-label mismatches={}",
        fonts.prepared_label_mismatch_count()
    );
    c6_ensure!(
        "export-fonts",
        fonts.prepared_label_terminal_incomplete_count() == 0,
        "terminal-incomplete prepared labels={}",
        fonts.prepared_label_terminal_incomplete_count()
    );
    c6_ensure!(
        "export-fonts",
        !fonts.is_host_dependent(),
        "font plan is host-dependent"
    );
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_native_filter_conversion(
    plan: merman::svg::export::SvgConversionPlan,
) -> C6ProofResult<()> {
    c6_ensure!(
        "native-filter-conversion",
        plan.filtered_groups > 0,
        "conversion retained no filtered groups"
    );
    c6_ensure!(
        "native-filter-conversion",
        plan.filter_primitives > 0,
        "conversion retained no filter primitives"
    );
    Ok(())
}

fn compile_brutalist_state_theme(
    theme_catalog: &ThemeFixtureCatalog,
    contract: &BrutalistStateFixtureContract<'_>,
) -> C6ProofResult<DiagramTheme> {
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
        asset.family() == contract.font_stack.families()[0],
        "font asset family `{}` differs from requested family `{}`",
        asset.family(),
        contract.font_stack.families()[0]
    );
    let bytes = theme_catalog
        .asset_bytes(contract.font_asset_id)
        .map_err(|error| C6ProofError::new("fixture-font-bytes", error.to_string()))?;
    let font_catalog = FontCatalogSpec::new([FontAssetSpec::new(asset.id(), bytes)])
        .with_available_sources([FontSource::Embedded])
        .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
    let default_text = ThemeTextStyle::default().with_font_stack(family_stack);

    let canvas = CanvasSpec::solid(contract.canvas_color)
        .map_err(|error| C6ProofError::new("fixture-canvas", error.to_string()))?;
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
        FilterRegion::bounded(-0.2, -0.2, 1.4, 1.4),
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
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(default_text))
                .with_assets(ThemeAssets::default().with_font_catalog(font_catalog))
                .with_canvas(canvas)
                .with_styles(styles)
                .with_effects(effects),
        )
        .map_err(|error| C6ProofError::new("theme-compile", error.to_string()))
}

fn prove_brutalist_state_svg(
    contract: &BrutalistStateFixtureContract<'_>,
    svg: &str,
) -> C6ProofResult<BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>> {
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
            filter.attribute("filterUnits") == Some("objectBoundingBox")
                && number_attribute(*filter, "x") == Some(-0.2)
                && number_attribute(*filter, "y") == Some(-0.2)
                && number_attribute(*filter, "width") == Some(1.4)
                && number_attribute(*filter, "height") == Some(1.4),
            "hard-shadow filter region differs from the fixture contract"
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

    Ok(brutalist_state_applied_mechanisms())
}

fn class_contains(node: roxmltree::Node<'_, '_>, class_name: &str) -> bool {
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

fn style_value<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use merman_theme_fixtures::C6_ACCEPTANCE_RELATIVE_PATH;
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
        let json = fs::read_to_string(theme_catalog.root().join(C6_ACCEPTANCE_RELATIVE_PATH))
            .expect("read C6 acceptance catalog");
        serde_json::from_str(&json).expect("parse C6 acceptance catalog")
    }

    fn retain_only_brutalist_state_target(value: &mut Value, selected: ExpectedOutputTarget) {
        for cell in value["cells"].as_array_mut().expect("C6 cells") {
            if cell["theme"] == "brutalist" && cell["family"] == "state" {
                let target = match cell["target"].as_str().expect("target id") {
                    "browser-svg" => ExpectedOutputTarget::BrowserSvg,
                    "standalone-svg" => ExpectedOutputTarget::StandaloneSvg,
                    "png" => ExpectedOutputTarget::Png,
                    "jpeg" => ExpectedOutputTarget::Jpeg,
                    "pdf" => ExpectedOutputTarget::Pdf,
                    other => panic!("unexpected target {other}"),
                };
                cell["enforcement"] = if target == selected {
                    json!({
                        "kind": "enforced",
                        "sourceFixtureId": BRUTALIST_STATE_FIXTURE_ID
                    })
                } else {
                    json!({
                        "kind": "deferred",
                        "blocker": "runtime-runner-missing"
                    })
                };
            } else if cell["enforcement"]["kind"] == "enforced" {
                cell["enforcement"] = json!({
                    "kind": "deferred",
                    "blocker": "family-adapter-incomplete"
                });
            }
        }
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
                    && group.work.key.lane() == C6RenderLane::Native
            })
            .expect("registered Brutalist State native group");
        assert_eq!(group.adapter.theme, C6ProofTheme::Brutalist);
        assert_eq!(group.adapter.family, C6ProofFamily::State);
        assert_eq!(group.adapter.source_fixture_id, BRUTALIST_STATE_FIXTURE_ID);
        assert_eq!(group.adapter.lane, C6RenderLane::Native);
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
                ExpectedOutputTarget::Jpeg,
                ExpectedOutputTarget::Pdf,
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
    fn duplicate_render_group_adapter_identity_fails_closed() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");
        let key = enforced_brutalist_state_native_key(&acceptance);
        let adapters = [BRUTALIST_STATE_ADAPTER; 2];

        let error = adapter_for_group(&adapters, &key)
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
        let required_by = [ExpectedOutputTarget::Jpeg];

        let error = prove_target::<()>(
            &key,
            ExpectedOutputTarget::Png,
            &required_by,
            Err(C6ProofError::new(
                "png-decode",
                "failed to decode the JPEG control PNG",
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
                && required_by == [ExpectedOutputTarget::Jpeg]
                && detail.contains("JPEG control PNG")
        ));
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn malformed_artifacts_fail_through_target_proof_boundaries() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let acceptance = C6AcceptanceCatalog::load(&theme_catalog).expect("load C6 acceptance");
        let key = enforced_brutalist_state_native_key(&acceptance);
        let fixture = theme_catalog
            .fixture(BRUTALIST_STATE_FIXTURE_ID)
            .expect("Brutalist State fixture");
        let contract = BrutalistStateFixtureContract::from_input(
            fixture.theme_input().expect("typed Brutalist State input"),
        )
        .expect("valid Brutalist State fixture contract");

        let svg = prove_render_group(&key, prove_brutalist_state_svg(&contract, "<svg>"))
            .expect_err("a malformed SVG must fail closed");
        assert!(matches!(
            svg,
            C6RuntimeError::RenderGroupProofFailed {
                group,
                stage: "standalone-svg-parse",
                detail,
            } if group == key.label() && !detail.is_empty()
        ));

        let png_required_by = [ExpectedOutputTarget::Png];
        let png = prove_target(
            &key,
            ExpectedOutputTarget::Png,
            &png_required_by,
            c6_raster_proof::RasterImage::decode_png(b"\x89PNG\r\n\x1a\n", raster_plan(1, 1)),
        )
        .err()
        .expect("a truncated PNG must fail closed");
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
        .err()
        .expect("a truncated JPEG must fail closed");
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
        prove_pdf_artifact(&malformed_pdf).expect("malformed PDF passes the cheap envelope check");
        let pdf_required_by = [ExpectedOutputTarget::Pdf];
        let pdf = prove_target(
            &key,
            ExpectedOutputTarget::Pdf,
            &pdf_required_by,
            c6_pdf_proof::load_pdf_artifact(&malformed_pdf),
        )
        .err()
        .expect("a malformed PDF must fail closed");
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
            adapter_for_group(&[BRUTALIST_STATE_ADAPTER], &key)
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
                "lane",
                C6RenderGroupAdapter {
                    lane: C6RenderLane::Browser,
                    ..BRUTALIST_STATE_ADAPTER
                },
            ),
        ];

        for (dimension, adapter) in mismatched_adapters {
            assert!(
                adapter_for_group(std::slice::from_ref(&adapter), &key)
                    .expect("evaluate adapter identity")
                    .is_none(),
                "{dimension} mismatch must not select the adapter"
            );
        }
    }

    #[test]
    fn enforced_browser_cell_without_registered_adapter_fails_closed() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let mut value = acceptance_value(&theme_catalog);
        retain_only_brutalist_state_target(&mut value, ExpectedOutputTarget::BrowserSvg);
        let acceptance = C6AcceptanceCatalog::from_json(
            &serde_json::to_string(&value).expect("serialize C6 acceptance catalog"),
            &theme_catalog,
        )
        .expect("load browser-only C6 acceptance catalog");

        let error = C6ExecutionPlan::from_catalog(&acceptance)
            .expect_err("browser cell must not be silently omitted");

        assert!(matches!(
            error,
            C6RuntimeError::UnsupportedEnforcedCell { key }
                if key.theme() == C6ProofTheme::Brutalist
                    && key.family() == C6ProofFamily::State
                    && key.target() == ExpectedOutputTarget::BrowserSvg
        ));
    }

    #[test]
    fn jpeg_only_tranche_uses_an_internal_png_control_without_requiring_a_png_cell() {
        let theme_catalog = ThemeFixtureCatalog::load(themes_root()).expect("load theme fixtures");
        let mut value = acceptance_value(&theme_catalog);
        retain_only_brutalist_state_target(&mut value, ExpectedOutputTarget::Jpeg);
        let acceptance = C6AcceptanceCatalog::from_json(
            &serde_json::to_string(&value).expect("serialize C6 acceptance catalog"),
            &theme_catalog,
        )
        .expect("load JPEG-only C6 acceptance catalog");

        let report = run_catalog(&theme_catalog, &acceptance).expect("prove JPEG-only tranche");

        assert_eq!(report.verified_cell_count(), 1);
        assert_eq!(report.render_group_count(), 1);
        assert_ne!(report.execution_digest(), &[0; 32]);
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
