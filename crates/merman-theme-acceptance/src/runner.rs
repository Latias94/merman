use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FilterRegion,
    FontAssetSpec, FontCatalogSpec, FontEmbeddingRequirement, FontSource, FontStack,
    OrdinalPalette, RenderEnvironment, RenderFamilyKind, RenderTargetKind, RenderedDocument,
    Specified, TargetAdmissionStatus, ThemeAssets, ThemeCapability, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, TypographySpec,
};
use merman::{Engine, MermaidConfig};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6ArtifactAssertion, C6CellKey, C6EnforcedCell, C6ExpectedFontSource,
    C6ProofFamily, C6ProofTheme, C6RequiredAdmission, ExpectedOutputTarget, ReferenceCanvasLayer,
    ReferenceDiagramFamily, ReferenceFontBinding, ReferenceSemanticRule, ReferenceSemanticTarget,
    ReferenceThemeInput, ReferenceThemeMechanism, ThemeFixtureCatalog,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum C6ObservedMechanismDisposition {
    Applied,
    NotApplicable,
    Rejected,
    Residual,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct C6ObservedCell {
    key: C6CellKey,
    source_fixture_id: String,
    source_sha256: String,
    theme_input_sha256: String,
    recipe_fingerprint: [u8; 32],
    document_digest: [u8; 32],
    resource_fingerprint: [u8; 32],
    target: RenderTargetKind,
    admission_digest: [u8; 32],
    artifact_digest: [u8; 32],
    mechanism_digest: [u8; 32],
    proof_stages: [C6ProofStage; 5],
    mechanism_dispositions: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    residual_ids: BTreeSet<String>,
    font_source: C6ExpectedFontSource,
    admission: C6RequiredAdmission,
    artifact_assertion: C6ArtifactAssertion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum C6ProofStage {
    SourceValidated,
    ThemeCompiled,
    DocumentSealed,
    TargetAdmitted,
    ArtifactVerified,
}

const C6_COMPLETE_PROOF_STAGES: [C6ProofStage; 5] = [
    C6ProofStage::SourceValidated,
    C6ProofStage::ThemeCompiled,
    C6ProofStage::DocumentSealed,
    C6ProofStage::TargetAdmitted,
    C6ProofStage::ArtifactVerified,
];

#[derive(Clone, Debug, PartialEq, Eq)]
struct C6ObservedReport {
    cells: BTreeMap<C6CellKey, C6ObservedCell>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct C6ExecutionReport {
    verified_cell_count: usize,
    enforced_cell_count: usize,
    execution_digest: [u8; 32],
}

impl C6ExecutionReport {
    pub const fn verified_cell_count(&self) -> usize {
        self.verified_cell_count
    }

    pub const fn enforced_cell_count(&self) -> usize {
        self.enforced_cell_count
    }

    pub const fn execution_digest(&self) -> &[u8; 32] {
        &self.execution_digest
    }
}

fn sha256(bytes: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(bytes.as_ref()).into()
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn admission_digest(admission: &merman::svg::TargetAdmissionReport) -> [u8; 32] {
    let status = match admission.status() {
        TargetAdmissionStatus::Portable => "portable",
        TargetAdmissionStatus::HostDependent => "host-dependent",
        TargetAdmissionStatus::Rejected => "rejected",
        status => panic!("C6 has no stable admission id for {status:?}"),
    };
    let mut value = format!("{}|{status}", admission.target().id());
    for reason in admission.reasons() {
        value.push('|');
        value.push_str(reason.id());
    }
    sha256(value)
}

fn mechanism_digest(
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> [u8; 32] {
    let mut value = String::new();
    for (mechanism, disposition) in mechanisms {
        value.push_str(reference_mechanism_id(*mechanism));
        value.push('=');
        value.push_str(match disposition {
            C6ObservedMechanismDisposition::Applied => "applied",
            C6ObservedMechanismDisposition::NotApplicable => "not-applicable",
            C6ObservedMechanismDisposition::Rejected => "rejected",
            C6ObservedMechanismDisposition::Residual => "residual",
        });
        value.push('|');
    }
    sha256(value)
}

fn reference_mechanism_id(mechanism: ReferenceThemeMechanism) -> &'static str {
    match mechanism {
        ReferenceThemeMechanism::BackdropFilter => "backdrop-filter",
        ReferenceThemeMechanism::CanvasBlend => "canvas-blend",
        ReferenceThemeMechanism::CanvasGradient => "canvas-gradient",
        ReferenceThemeMechanism::CanvasLayering => "canvas-layering",
        ReferenceThemeMechanism::CanvasPattern => "canvas-pattern",
        ReferenceThemeMechanism::CanvasSolid => "canvas-solid",
        ReferenceThemeMechanism::CssFilter => "css-filter",
        ReferenceThemeMechanism::CssLetterSpacing => "css-letter-spacing",
        ReferenceThemeMechanism::CssTextTransform => "css-text-transform",
        ReferenceThemeMechanism::DashArray => "dash-array",
        ReferenceThemeMechanism::ExternalSvgFilterReference => "external-svg-filter-reference",
        ReferenceThemeMechanism::FontStack => "font-stack",
        ReferenceThemeMechanism::HasSelector => "has-selector",
        ReferenceThemeMechanism::NotSelector => "not-selector",
        ReferenceThemeMechanism::NthChildSelector => "nth-child-selector",
        ReferenceThemeMechanism::RoundedCorners => "rounded-corners",
        ReferenceThemeMechanism::StrokeStyling => "stroke-styling",
        ReferenceThemeMechanism::ThemeVariables => "theme-variables",
    }
}

fn expected_observed_disposition(
    expected: merman_theme_fixtures::C6ExpectedMechanismDisposition,
) -> C6ObservedMechanismDisposition {
    match expected {
        merman_theme_fixtures::C6ExpectedMechanismDisposition::MustApply => {
            C6ObservedMechanismDisposition::Applied
        }
        merman_theme_fixtures::C6ExpectedMechanismDisposition::MustRemainResidual => {
            C6ObservedMechanismDisposition::Residual
        }
        merman_theme_fixtures::C6ExpectedMechanismDisposition::MustReject => {
            C6ObservedMechanismDisposition::Rejected
        }
        merman_theme_fixtures::C6ExpectedMechanismDisposition::NotApplicable => {
            C6ObservedMechanismDisposition::NotApplicable
        }
    }
}

fn expected_artifact_assertion(target: ExpectedOutputTarget) -> C6ArtifactAssertion {
    match target {
        ExpectedOutputTarget::BrowserSvg => C6ArtifactAssertion::BrowserSvgDom,
        ExpectedOutputTarget::StandaloneSvg => C6ArtifactAssertion::StandaloneSvgDocument,
        ExpectedOutputTarget::Png => C6ArtifactAssertion::PngImage,
        ExpectedOutputTarget::Jpeg => C6ArtifactAssertion::JpegImage,
        ExpectedOutputTarget::Pdf => C6ArtifactAssertion::PdfDocument,
    }
}

fn render_target_for_artifact(assertion: C6ArtifactAssertion) -> RenderTargetKind {
    match assertion {
        C6ArtifactAssertion::BrowserSvgDom | C6ArtifactAssertion::StandaloneSvgDocument => {
            RenderTargetKind::Svg
        }
        C6ArtifactAssertion::PngImage => RenderTargetKind::Png,
        C6ArtifactAssertion::JpegImage => RenderTargetKind::Jpeg,
        C6ArtifactAssertion::PdfDocument => RenderTargetKind::Pdf,
    }
}

fn observe_cell_from_reports(
    enforced: &C6EnforcedCell,
    catalog: &ThemeFixtureCatalog,
    recipe_fingerprint: Option<merman::svg::ThemeRecipeFingerprint>,
    sealed_svg: &str,
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
    document: &merman::svg::DocumentRenderReport,
    admission: &merman::svg::TargetAdmissionReport,
    artifact_assertion: C6ArtifactAssertion,
    artifact_bytes: &[u8],
) -> C6ObservedCell {
    let fixture = catalog
        .fixture(enforced.source_fixture_id())
        .expect("the acceptance catalog validated the enforced fixture");
    assert_eq!(
        artifact_assertion,
        expected_artifact_assertion(enforced.key().target())
    );
    assert_eq!(
        artifact_assertion,
        expected_artifact_assertion(match admission.target() {
            RenderTargetKind::Svg => ExpectedOutputTarget::StandaloneSvg,
            RenderTargetKind::Png => ExpectedOutputTarget::Png,
            RenderTargetKind::Jpeg => ExpectedOutputTarget::Jpeg,
            RenderTargetKind::Pdf => ExpectedOutputTarget::Pdf,
            target => panic!("C6 has no artifact assertion for render target {target:?}"),
        })
    );
    assert!(
        document.residuals().is_empty(),
        "an enforced portable C6 artifact cannot carry document residuals"
    );

    let source_digest = sha256(
        catalog
            .source_text(fixture.id())
            .expect("read hash-validated source")
            .as_bytes(),
    );
    assert_eq!(hex_digest(&source_digest), fixture.source_sha256());
    let theme_input_sha256 = fixture
        .theme_input_sha256()
        .expect("enforced typed fixture must declare a theme input hash")
        .to_owned();

    let font_source = match document.prepared_text_used_font_sources() {
        [FontSource::Embedded] => C6ExpectedFontSource::Embedded,
        [FontSource::System] => C6ExpectedFontSource::System,
        [] => C6ExpectedFontSource::None,
        sources => panic!("C6 observed an ambiguous prepared font source set: {sources:?}"),
    };
    let observed_admission = match admission.status() {
        TargetAdmissionStatus::Portable => C6RequiredAdmission::Portable,
        TargetAdmissionStatus::HostDependent => C6RequiredAdmission::HostDependent,
        TargetAdmissionStatus::Rejected => C6RequiredAdmission::Rejected,
        status => panic!("C6 has no observation mapping for target admission {status:?}"),
    };
    if observed_admission == C6RequiredAdmission::Portable {
        assert!(admission.reasons().is_empty());
    }

    C6ObservedCell {
        key: enforced.key(),
        source_fixture_id: fixture.id().to_owned(),
        source_sha256: fixture.source_sha256().to_owned(),
        theme_input_sha256,
        recipe_fingerprint: *recipe_fingerprint
            .expect("a C6 artifact must retain the renderer-owned recipe identity")
            .as_bytes(),
        document_digest: sha256(sealed_svg),
        resource_fingerprint: *document.resource_fingerprint().as_bytes(),
        target: admission.target(),
        admission_digest: admission_digest(admission),
        artifact_digest: sha256(artifact_bytes),
        mechanism_digest: mechanism_digest(&mechanisms),
        proof_stages: C6_COMPLETE_PROOF_STAGES,
        mechanism_dispositions: mechanisms,
        residual_ids: BTreeSet::new(),
        font_source,
        admission: observed_admission,
        artifact_assertion,
    }
}

impl C6ObservedReport {
    fn from_enforced_cells(
        tranche: &merman_theme_fixtures::C6EnforcedTranche,
        cells: impl IntoIterator<Item = C6ObservedCell>,
    ) -> Self {
        let mut indexed = BTreeMap::new();
        for cell in cells {
            assert!(
                indexed.insert(cell.key, cell).is_none(),
                "duplicate C6 cell"
            );
        }
        let expected = tranche
            .cells()
            .map(C6EnforcedCell::key)
            .collect::<BTreeSet<_>>();
        let actual = indexed.keys().copied().collect::<BTreeSet<_>>();
        assert_eq!(
            actual, expected,
            "runtime observations must cover the enforced tranche"
        );
        Self { cells: indexed }
    }

    fn evaluate(
        &self,
        acceptance: &C6AcceptanceCatalog,
        theme_catalog: &ThemeFixtureCatalog,
        theme: &DiagramTheme,
        document: &RenderedDocument,
        sealed_svg: &str,
    ) -> C6ExecutionReport {
        assert!(!self.cells.is_empty(), "an empty C6 tranche cannot pass");
        let expected_recipe = *theme.recipe_fingerprint().as_bytes();
        let expected_document = sha256(sealed_svg);
        let expected_resource = *document.resource_fingerprint().as_bytes();
        for enforced in acceptance.enforced_tranche().cells() {
            let observed = self
                .cells
                .get(&enforced.key())
                .expect("enforced observation");
            let expectation = acceptance
                .specification()
                .cell(enforced.key())
                .expect("enforced expectation")
                .expectation();
            let fixture = theme_catalog
                .fixture(enforced.source_fixture_id())
                .expect("enforced fixture identity");
            assert_eq!(observed.source_fixture_id, enforced.source_fixture_id());
            assert_eq!(observed.source_sha256, fixture.source_sha256());
            assert_eq!(
                observed.theme_input_sha256,
                fixture
                    .theme_input_sha256()
                    .expect("enforced typed fixture input identity")
            );
            assert_eq!(observed.recipe_fingerprint, expected_recipe);
            assert_eq!(observed.document_digest, expected_document);
            assert_eq!(observed.resource_fingerprint, expected_resource);
            assert_eq!(
                observed.target,
                render_target_for_artifact(observed.artifact_assertion)
            );
            assert_eq!(
                observed.mechanism_dispositions.len(),
                expectation.mechanism_requirements().len()
            );
            for (mechanism, disposition) in expectation.mechanism_requirements() {
                assert_eq!(
                    observed.mechanism_dispositions.get(mechanism),
                    Some(&expected_observed_disposition(*disposition))
                );
            }
            assert_eq!(observed.residual_ids, *expectation.expected_residual_ids());
            assert_eq!(observed.font_source, expectation.required_font_source());
            assert_eq!(observed.admission, expectation.required_admission());
            assert_eq!(
                observed.artifact_assertion,
                expectation.required_artifact_assertion()
            );
            assert_ne!(observed.artifact_digest, [0; 32]);
            assert_ne!(observed.admission_digest, [0; 32]);
            assert_ne!(observed.mechanism_digest, [0; 32]);
            assert_eq!(observed.proof_stages, C6_COMPLETE_PROOF_STAGES);
        }
        let mut digest_input = b"merman.c6-execution-report.v1".to_vec();
        for cell in self.cells.values() {
            append_len_prefixed(
                &mut digest_input,
                cell.key.theme().reference_name().as_bytes(),
            );
            append_len_prefixed(&mut digest_input, cell.source_fixture_id.as_bytes());
            digest_input.extend_from_slice(cell.source_sha256.as_bytes());
            digest_input.extend_from_slice(cell.theme_input_sha256.as_bytes());
            digest_input.extend_from_slice(&cell.recipe_fingerprint);
            digest_input.extend_from_slice(&cell.document_digest);
            digest_input.extend_from_slice(&cell.resource_fingerprint);
            append_len_prefixed(&mut digest_input, cell.target.id().as_bytes());
            digest_input.extend_from_slice(&cell.admission_digest);
            digest_input.extend_from_slice(&cell.artifact_digest);
            digest_input.extend_from_slice(&cell.mechanism_digest);
            for stage in cell.proof_stages {
                digest_input.push(stage as u8);
            }
        }
        C6ExecutionReport {
            verified_cell_count: self.cells.len(),
            enforced_cell_count: acceptance.enforced_tranche().cells().len(),
            execution_digest: sha256(digest_input),
        }
    }
}

fn append_len_prefixed(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
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

fn themes_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("themes")
}

pub fn run_enforced_c6_runtime() -> C6ExecutionReport {
    let theme_catalog =
        ThemeFixtureCatalog::load(themes_root()).expect("load the source-backed theme corpus");
    let acceptance = C6AcceptanceCatalog::load(&theme_catalog)
        .expect("load the committed C6 acceptance contract");
    let enforced_cells = acceptance.enforced_tranche().cells().collect::<Vec<_>>();
    for cell in &enforced_cells {
        assert_eq!(
            cell.key().theme(),
            C6ProofTheme::Brutalist,
            "unsupported enforced C6 theme {:?}",
            cell.key()
        );
        assert_eq!(
            cell.key().family(),
            C6ProofFamily::State,
            "unsupported enforced C6 family {:?}",
            cell.key()
        );
        assert!(
            matches!(
                cell.key().target(),
                ExpectedOutputTarget::StandaloneSvg
                    | ExpectedOutputTarget::Png
                    | ExpectedOutputTarget::Jpeg
                    | ExpectedOutputTarget::Pdf
            ),
            "unsupported enforced C6 target {:?}",
            cell.key()
        );
    }

    let fixture_id = enforced_cells
        .first()
        .expect("the enforced C6 tranche must not be empty")
        .source_fixture_id();
    assert!(
        enforced_cells
            .iter()
            .all(|cell| cell.source_fixture_id() == fixture_id),
        "the current C6 vertical slice requires one shared source fixture"
    );
    let fixture = theme_catalog
        .fixture(fixture_id)
        .expect("the enforced C6 fixture must exist");
    let input = fixture
        .theme_input()
        .expect("the first enforced C6 fixture must own a typed theme input");
    assert_eq!(input.fixture_id(), fixture.id());
    let source = theme_catalog
        .source_text(fixture.id())
        .expect("read the hash-validated C6 Mermaid source");
    let theme = compile_brutalist_state_theme(&theme_catalog, input);
    assert_eq!(
        theme
            .report()
            .required_capabilities()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            ThemeCapability::SemanticTokens,
            ThemeCapability::Typography,
            ThemeCapability::SemanticRules,
            ThemeCapability::OrdinalPalette,
            ThemeCapability::SolidPaint,
            ThemeCapability::BorderStyling,
            ThemeCapability::RoundedGeometry,
            ThemeCapability::Shadow,
            ThemeCapability::SvgFilter,
        ])
    );

    let document = render_brutalist_state_document(&source, &theme);
    assert_brutalist_state_document(&document, &theme);
    let admitted = document
        .clone()
        .admit_svg()
        .expect("the shared C6 sealed document must pass strict SVG admission");
    let mechanisms = assert_brutalist_state_svg(input, admitted.as_str());
    assert_eq!(
        mechanisms.keys().copied().collect::<BTreeSet<_>>(),
        input.mechanisms()
    );

    for cell in &enforced_cells {
        assert_enforced_brutalist_state_contract(&theme_catalog, &acceptance, cell);
    }

    let png_cell = enforced_cells
        .iter()
        .find(|cell| cell.key().target() == ExpectedOutputTarget::Png)
        .expect("the enforced tranche must include a PNG target");
    let (png, png_proof) = observe_brutalist_state_png(
        png_cell,
        &theme_catalog,
        input,
        admitted.as_str(),
        &theme,
        &document,
    );
    let enforced_count = enforced_cells.len();
    let mut observed = Vec::with_capacity(enforced_count);
    for cell in enforced_cells {
        let observation = match cell.key().target() {
            ExpectedOutputTarget::StandaloneSvg => observe_brutalist_state_standalone(
                cell,
                &theme_catalog,
                input,
                &document,
                &mechanisms,
            ),
            ExpectedOutputTarget::Png => png.clone(),
            ExpectedOutputTarget::Jpeg => observe_brutalist_state_jpeg(
                cell,
                &theme_catalog,
                input,
                admitted.as_str(),
                &theme,
                &document,
                &png_proof,
            ),
            ExpectedOutputTarget::Pdf => observe_brutalist_state_pdf(
                cell,
                &theme_catalog,
                input,
                admitted.as_str(),
                &theme,
                &document,
            ),
            ExpectedOutputTarget::BrowserSvg => {
                panic!("Browser SVG requires a dedicated DOM adapter")
            }
        };
        observed.push(observation);
    }
    let report = C6ObservedReport::from_enforced_cells(acceptance.enforced_tranche(), observed);
    let evaluation = report.evaluate(
        &acceptance,
        &theme_catalog,
        &theme,
        &document,
        admitted.as_str(),
    );

    assert_eq!(evaluation.verified_cell_count(), enforced_count);
    assert_eq!(evaluation.enforced_cell_count(), enforced_count);
    evaluation
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_enforced_brutalist_state_contract(
    theme_catalog: &ThemeFixtureCatalog,
    acceptance: &C6AcceptanceCatalog,
    enforced: &C6EnforcedCell,
) {
    let fixture = theme_catalog
        .fixture(enforced.source_fixture_id())
        .expect("the enforced fixture was validated while loading the acceptance catalog");
    assert_eq!(
        fixture.source_family(),
        ReferenceDiagramFamily::StateDiagram
    );
    let input = fixture
        .theme_input()
        .expect("the C6 fixture must own a typed theme input");
    let expectation = acceptance
        .specification()
        .cell(enforced.key())
        .expect("the enforced cell must have a final expectation")
        .expectation();
    let expected_mechanisms = expectation
        .mechanism_requirements()
        .keys()
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(input.mechanisms(), expected_mechanisms);
    assert_eq!(expectation.expected_residual_ids(), &BTreeSet::new());
    assert_eq!(
        expectation.required_font_source(),
        C6ExpectedFontSource::Embedded
    );
    assert_eq!(
        expectation.required_admission(),
        C6RequiredAdmission::Portable
    );
    assert_eq!(
        expectation.required_artifact_assertion(),
        artifact_assertion_for_target(enforced.key().target())
    );
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_brutalist_state_document(document: &RenderedDocument, theme: &DiagramTheme) {
    assert_eq!(document.family_kind(), RenderFamilyKind::State);
    assert_eq!(
        document.theme_recipe_fingerprint(),
        Some(theme.recipe_fingerprint())
    );
    assert_eq!(
        document
            .theme_recipe_report()
            .map(|report| report.theme_recipe_fingerprint()),
        Some(theme.recipe_fingerprint())
    );
    assert!(document.root_theme_report().is_verified());
    assert!(document.document_report().residuals().is_empty());
    assert_eq!(
        document.document_report().prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert_eq!(document.document_report().host_text_measurement_count(), 0);
    assert!(document.resource_closure().is_closed());
    assert_eq!(
        document.svg_target_admission().status(),
        TargetAdmissionStatus::Portable
    );
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_standalone(
    enforced: &C6EnforcedCell,
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    document: &RenderedDocument,
    mechanisms: &BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
) -> C6ObservedCell {
    let admitted = document
        .clone()
        .admit_svg()
        .expect("the C6 standalone SVG must pass strict terminal admission");
    let observed_mechanisms = assert_brutalist_state_svg(input, admitted.as_str());
    assert_eq!(&observed_mechanisms, mechanisms);
    observe_cell_from_reports(
        enforced,
        theme_catalog,
        document.theme_recipe_fingerprint(),
        admitted.as_str(),
        mechanisms.clone(),
        admitted.document_report(),
        admitted.target_admission(),
        C6ArtifactAssertion::StandaloneSvgDocument,
        admitted.as_str().as_bytes(),
    )
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_png(
    enforced: &C6EnforcedCell,
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
) -> (C6ObservedCell, PngArtifactProof) {
    let resource_fingerprint = document.resource_fingerprint();
    let prepared = document
        .clone()
        .prepare_png_export(&merman::svg::export::RasterOptions::default().with_scale(2.0))
        .expect("prepare the enforced C6 PNG export");
    let prepared_export = prepared.export_report();
    assert_eq!(prepared.target_admission().target(), RenderTargetKind::Png);
    assert_eq!(prepared_export.resource_fingerprint(), resource_fingerprint);
    assert_native_filter_conversion(prepared_export.conversion());
    assert_export_font_plan(prepared_export.fonts());
    let native_filter_receipt = prepared_export
        .native_filter_receipt()
        .expect("portable typed hard-shadow PNG requires an exact native filter receipt");

    let (png, report) = prepared
        .encode()
        .expect("encode the enforced C6 PNG artifact");
    assert_png_artifact(&png);
    let proof = prove_brutalist_state_png(input, sealed_svg, &png);
    assert_eq!(
        report.export_report().native_filter_receipt(),
        Some(native_filter_receipt)
    );
    assert_native_report(
        report.document_report(),
        report.operation_report().theme_recipe_fingerprint(),
        report.target_admission().status(),
        report.export_report().resource_fingerprint(),
        resource_fingerprint,
        theme.recipe_fingerprint(),
    );
    (
        observe_cell_from_reports(
            enforced,
            theme_catalog,
            report.operation_report().theme_recipe_fingerprint(),
            sealed_svg,
            proof.mechanisms().clone(),
            report.document_report(),
            report.target_admission(),
            C6ArtifactAssertion::PngImage,
            &png,
        ),
        proof,
    )
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_jpeg(
    enforced: &C6EnforcedCell,
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
    png_proof: &PngArtifactProof,
) -> C6ObservedCell {
    let resource_fingerprint = document.resource_fingerprint();
    let prepared = document
        .clone()
        .prepare_jpeg_export(&merman::svg::export::RasterOptions::default().with_scale(2.0))
        .expect("prepare the enforced C6 JPEG export");
    let prepared_export = prepared.export_report();
    assert_eq!(prepared.target_admission().target(), RenderTargetKind::Jpeg);
    assert_eq!(prepared_export.resource_fingerprint(), resource_fingerprint);
    assert_native_filter_conversion(prepared_export.conversion());
    assert_export_font_plan(prepared_export.fonts());
    let native_filter_receipt = prepared_export
        .native_filter_receipt()
        .expect("portable typed hard-shadow JPEG requires an exact native filter receipt");

    let (jpeg, report) = prepared
        .encode()
        .expect("encode the enforced C6 JPEG artifact");
    assert_jpeg_artifact(&jpeg);
    let mechanisms = prove_brutalist_state_jpeg(input, sealed_svg, &jpeg, png_proof);
    assert_eq!(
        report.export_report().native_filter_receipt(),
        Some(native_filter_receipt)
    );
    assert_native_report(
        report.document_report(),
        report.operation_report().theme_recipe_fingerprint(),
        report.target_admission().status(),
        report.export_report().resource_fingerprint(),
        resource_fingerprint,
        theme.recipe_fingerprint(),
    );
    observe_cell_from_reports(
        enforced,
        theme_catalog,
        report.operation_report().theme_recipe_fingerprint(),
        sealed_svg,
        mechanisms,
        report.document_report(),
        report.target_admission(),
        C6ArtifactAssertion::JpegImage,
        &jpeg,
    )
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_pdf(
    enforced: &C6EnforcedCell,
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
) -> C6ObservedCell {
    let resource_fingerprint = document.resource_fingerprint();
    let prepared = document
        .clone()
        .prepare_pdf_export(&merman::svg::export::PdfOptions::default())
        .expect("prepare the enforced C6 PDF export");
    let prepared_export = prepared.export_report();
    assert_eq!(prepared.target_admission().target(), RenderTargetKind::Pdf);
    assert_eq!(prepared_export.resource_fingerprint(), resource_fingerprint);
    assert_native_filter_conversion(prepared_export.conversion());
    let filter_plan = prepared_export.filters();
    assert!(filter_plan.filtered_groups > 0);
    assert!(filter_plan.effective_image_pixels > 0);
    assert_export_font_plan(prepared_export.fonts());
    assert!(prepared_export.native_filter_fully_localized());
    let native_filter_receipt = prepared_export
        .native_filter_receipt()
        .expect("portable typed hard-shadow PDF requires an exact native filter receipt");

    let (pdf, report) = prepared
        .encode()
        .expect("encode the enforced C6 PDF artifact");
    assert_pdf_artifact(&pdf);
    let mechanisms =
        prove_brutalist_state_pdf(input, sealed_svg, &pdf, filter_plan.effective_scale);
    assert_eq!(
        report.export_report().native_filter_receipt(),
        Some(native_filter_receipt)
    );
    assert!(report.export_report().native_filter_fully_localized());
    assert_native_report(
        report.document_report(),
        report.operation_report().theme_recipe_fingerprint(),
        report.target_admission().status(),
        report.export_report().resource_fingerprint(),
        resource_fingerprint,
        theme.recipe_fingerprint(),
    );
    observe_cell_from_reports(
        enforced,
        theme_catalog,
        report.operation_report().theme_recipe_fingerprint(),
        sealed_svg,
        mechanisms,
        report.document_report(),
        report.target_admission(),
        C6ArtifactAssertion::PdfDocument,
        &pdf,
    )
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn artifact_assertion_for_target(target: ExpectedOutputTarget) -> C6ArtifactAssertion {
    match target {
        ExpectedOutputTarget::BrowserSvg => C6ArtifactAssertion::BrowserSvgDom,
        ExpectedOutputTarget::StandaloneSvg => C6ArtifactAssertion::StandaloneSvgDocument,
        ExpectedOutputTarget::Png => C6ArtifactAssertion::PngImage,
        ExpectedOutputTarget::Jpeg => C6ArtifactAssertion::JpegImage,
        ExpectedOutputTarget::Pdf => C6ArtifactAssertion::PdfDocument,
    }
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_png_artifact(bytes: &[u8]) {
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .expect("decode the enforced C6 PNG header");
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .expect("the enforced C6 PNG output size must fit in memory")
    ];
    let frame = reader
        .next_frame(&mut pixels)
        .expect("decode the enforced C6 PNG frame");
    assert!(frame.width > 0 && frame.height > 0);
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    assert_eq!(frame.bit_depth, png::BitDepth::Eight);
    let pixels = &pixels[..frame.buffer_size()];
    let first = pixels
        .chunks_exact(4)
        .next()
        .expect("the enforced C6 PNG must contain pixels");
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    assert!(
        pixels.chunks_exact(4).any(|pixel| pixel[..3] != first[..3]),
        "the enforced C6 PNG must contain rendered diagram detail"
    );
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_jpeg_artifact(bytes: &[u8]) {
    assert!(bytes.starts_with(&[0xff, 0xd8, 0xff]));
    assert!(bytes.ends_with(&[0xff, 0xd9]));
    let (width, height) = jpeg_dimensions(bytes).expect("read the enforced C6 JPEG dimensions");
    assert!(width > 0 && height > 0);
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u16, u16)> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }

    let mut offset = 2usize;
    while offset < bytes.len() {
        while offset < bytes.len() && bytes[offset] != 0xff {
            offset += 1;
        }
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if matches!(marker, 0xd9 | 0xda) {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            continue;
        }

        let segment_length = usize::from(u16::from_be_bytes([
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
        ]));
        if segment_length < 2 || offset.checked_add(segment_length)? > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if segment_length < 7 {
                return None;
            }
            let height = u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]);
            let width = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]);
            return Some((width, height));
        }
        offset += segment_length;
    }
    None
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_pdf_artifact(bytes: &[u8]) {
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 1024);
    assert!(
        bytes
            .windows(b"startxref".len())
            .any(|window| window == b"startxref")
    );
    let trimmed = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    assert!(trimmed.ends_with(b"%%EOF"));
}

fn render_brutalist_state_document(source: &str, theme: &DiagramTheme) -> RenderedDocument {
    let environment = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable);
    merman::svg::HeadlessRenderer::from_engine_and_environment(Engine::new(), environment)
        .with_site_config(MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": false
        })))
        .with_theme(theme.clone())
        .render_document_sync(source)
        .expect("the enforced C6 State fixture must render")
        .expect("the enforced C6 source must detect as State")
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_export_font_plan(fonts: merman::svg::export::ExportFontPlan) {
    assert!(fonts.used_embedded_fonts());
    assert!(!fonts.used_system_fonts());
    assert!(fonts.prepared_label_expected_count() > 0);
    assert_eq!(
        fonts.prepared_label_verified_count(),
        fonts.prepared_label_expected_count()
    );
    assert_eq!(fonts.prepared_label_mismatch_count(), 0);
    assert!(fonts.prepared_text_evidence_matches());
    assert_eq!(fonts.prepared_label_terminal_incomplete_count(), 0);
    assert!(fonts.prepared_text_terminal_proof_complete());
    assert!(!fonts.is_host_dependent());
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_native_filter_conversion(plan: merman::svg::export::SvgConversionPlan) {
    assert!(plan.filtered_groups > 0);
    assert!(plan.filter_primitives > 0);
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn assert_native_report(
    document: &merman::svg::DocumentRenderReport,
    recipe_fingerprint: Option<merman::svg::ThemeRecipeFingerprint>,
    admission: TargetAdmissionStatus,
    export_fingerprint: merman::svg::SvgResourceFingerprint,
    expected_resource_fingerprint: merman::svg::SvgResourceFingerprint,
    expected_recipe_fingerprint: merman::svg::ThemeRecipeFingerprint,
) {
    assert!(document.residuals().is_empty());
    assert_eq!(
        document.prepared_text_used_font_sources(),
        &[FontSource::Embedded]
    );
    assert_eq!(document.host_text_measurement_count(), 0);
    assert_eq!(
        document.resource_fingerprint(),
        expected_resource_fingerprint
    );
    assert_eq!(export_fingerprint, expected_resource_fingerprint);
    assert_eq!(recipe_fingerprint, Some(expected_recipe_fingerprint));
    assert_eq!(admission, TargetAdmissionStatus::Portable);
}

fn compile_brutalist_state_theme(
    theme_catalog: &ThemeFixtureCatalog,
    input: &ReferenceThemeInput,
) -> DiagramTheme {
    let tokens = input.tokens().expect("Brutalist requires typed tokens");
    let typography = input
        .typography()
        .expect("Brutalist requires typed typography");
    assert_eq!(typography.letter_spacing_milli_em(), None);
    assert_eq!(typography.text_transform(), None);
    let font_stack = typography
        .font_stack()
        .expect("Brutalist requires a font stack");
    assert_eq!(font_stack.binding(), ReferenceFontBinding::FixtureAssets);
    assert_eq!(font_stack.asset_ids().len(), 1);
    let family_stack = FontStack::new(font_stack.families().iter().cloned())
        .expect("fixture font families are validated");
    let assets = font_stack.asset_ids().iter().map(|asset_id| {
        let asset = theme_catalog
            .font_asset(asset_id)
            .expect("fixture font asset must exist");
        assert_eq!(asset.family(), font_stack.families()[0]);
        let bytes = theme_catalog
            .asset_bytes(asset_id)
            .expect("read the hash-validated fixture font");
        FontAssetSpec::new(asset.id(), bytes)
    });
    let font_catalog = FontCatalogSpec::new(assets)
        .with_available_sources([FontSource::Embedded])
        .with_embedding_requirement(FontEmbeddingRequirement::FullFont);
    let default_text = ThemeTextStyle::default().with_font_stack(family_stack);

    let canvas_color = match input.canvas() {
        [ReferenceCanvasLayer::Solid { color }] => color,
        _ => panic!("the first C6 cell requires exactly one solid canvas layer"),
    };
    assert_eq!(canvas_color, tokens.background());
    let canvas = CanvasSpec::solid(canvas_color).expect("fixture canvas color is validated");

    let node_style = input
        .node_style()
        .expect("Brutalist requires a typed State node style");
    assert!(node_style.dash_pattern().is_empty());
    let border = node_style
        .border()
        .expect("Brutalist requires a node border");
    let radius = node_style
        .corner_radius_px()
        .expect("Brutalist requires rounded State nodes");
    let mut state_patch = ThemeStylePatch::default()
        .with_stroke(CanvasPaint::solid(border.color()).expect("valid border color"))
        .with_stroke_width(f32::from(border.width_px()))
        .expect("valid border width");
    state_patch.geometry.radius = Specified::Value(f32::from(radius));

    let palette_colors = match input.semantic_rules() {
        [
            ReferenceSemanticRule::OrdinalPalette {
                family,
                target,
                colors,
            },
        ] => {
            assert_eq!(*family, ReferenceDiagramFamily::StateDiagram);
            assert_eq!(*target, ReferenceSemanticTarget::Node);
            colors
        }
        _ => panic!("the first C6 cell requires one State node ordinal palette"),
    };
    assert_eq!(
        palette_colors.first().map(String::as_str),
        Some(tokens.primary())
    );
    let palette = OrdinalPalette::new(
        palette_colors
            .iter()
            .map(|color| ThemeColorValue::parse(color).expect("valid fixture palette color")),
    )
    .expect("the fixture palette is bounded and non-empty");
    let styles = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(ThemeTarget::State, state_patch).for_family(RenderFamilyKind::State),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::StateLabel,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid(tokens.text()).expect("valid text color")),
            )
            .for_family(RenderFamilyKind::State),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::TransitionLabelBackground,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid(tokens.surface()).expect("valid surface color")),
            )
            .for_family(RenderFamilyKind::State),
        )
        .with_ordinal_palette(ThemeTarget::State, palette);

    let shadow = node_style
        .shadow()
        .expect("Brutalist requires a hard shadow");
    assert_eq!(shadow.blur_px(), 0);
    assert_eq!(shadow.spread_px(), 0);
    let graph = EffectGraph::new(
        SHADOW_EFFECT_ID,
        FilterRegion::bounded(-0.2, -0.2, 1.4, 1.4),
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: f32::from(shadow.offset_x_px()),
            offset_y: f32::from(shadow.offset_y_px()),
            blur_radius: f32::from(shadow.blur_px()),
            spread: f32::from(shadow.spread_px()),
            color: ThemeColorValue::parse(shadow.color()).expect("valid shadow color"),
        }],
    )
    .expect("the fixture hard-shadow graph is valid");
    let effects = DiagramEffectSet::default()
        .with_graph(graph)
        .expect("the C6 effect id is unique")
        .with_binding(
            EffectBinding::new(ThemeTarget::State, SHADOW_EFFECT_ID)
                .expect("valid State effect binding"),
        )
        .expect("the State effect binding is unique");

    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_default(default_text))
                .with_assets(ThemeAssets::default().with_font_catalog(font_catalog))
                .with_canvas(canvas)
                .with_styles(styles)
                .with_effects(effects),
        )
        .expect("compile the Rust-owned Brutalist State proof recipe")
}

fn assert_brutalist_state_svg(
    input: &ReferenceThemeInput,
    svg: &str,
) -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    let document = roxmltree::Document::parse(svg).expect("parse the terminal standalone SVG");
    let tokens = input.tokens().expect("Brutalist requires typed tokens");
    let node_style = input
        .node_style()
        .expect("Brutalist requires a typed State node style");
    let border = node_style
        .border()
        .expect("Brutalist requires a node border");
    let radius = f32::from(
        node_style
            .corner_radius_px()
            .expect("Brutalist requires rounded State nodes"),
    );
    let palette = match input.semantic_rules() {
        [ReferenceSemanticRule::OrdinalPalette { colors, .. }] => colors,
        _ => panic!("the first C6 cell requires one ordinal palette"),
    };

    let canvas = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .expect("the typed canvas base must be emitted");
    assert_eq!(canvas.attribute("fill"), Some(tokens.background()));

    let state_rects = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "basic")
                && class_contains(*node, "label-container")
        })
        .collect::<Vec<_>>();
    let expected_ordinal_fills = [
        ("Ready", palette[0].as_str()),
        ("Review", palette[1].as_str()),
        ("Done", palette[2].as_str()),
        ("Archive", palette[0].as_str()),
    ];
    assert_eq!(state_rects.len(), expected_ordinal_fills.len());
    for (state_id, expected_fill) in expected_ordinal_fills {
        let rect = state_rects
            .iter()
            .copied()
            .find(|node| state_rect_belongs_to(*node, state_id))
            .unwrap_or_else(|| panic!("missing rendered State node for {state_id}"));
        assert_eq!(
            style_value(rect, "fill"),
            Some(expected_fill),
            "State ordinal palette mapping is wrong for {state_id}"
        );
    }
    assert!(state_rects.iter().all(|node| {
        style_value(*node, "stroke") == Some(border.color())
            && style_number(*node, "stroke-width") == Some(f32::from(border.width_px()))
            && number_attribute(*node, "rx") == Some(radius)
            && number_attribute(*node, "ry") == Some(radius)
    }));

    let effect_filters = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("filter")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains(SHADOW_EFFECT_ID))
        })
        .collect::<Vec<_>>();
    assert_eq!(effect_filters.len(), state_rects.len());
    let shadow = node_style.shadow().expect("Brutalist hard shadow");
    let effect_ids = effect_filters
        .iter()
        .map(|filter| {
            assert_eq!(filter.attribute("filterUnits"), Some("objectBoundingBox"));
            assert_eq!(number_attribute(*filter, "x"), Some(-0.2));
            assert_eq!(number_attribute(*filter, "y"), Some(-0.2));
            assert_eq!(number_attribute(*filter, "width"), Some(1.4));
            assert_eq!(number_attribute(*filter, "height"), Some(1.4));
            assert_eq!(
                filter.attribute("color-interpolation-filters"),
                Some("linearRGB")
            );
            let primitives = filter
                .children()
                .filter(|node| node.is_element())
                .collect::<Vec<_>>();
            assert_eq!(primitives.len(), 1);
            let drop_shadow = primitives[0];
            assert!(drop_shadow.has_tag_name("feDropShadow"));
            assert_eq!(drop_shadow.attribute("in"), Some("SourceGraphic"));
            assert_eq!(
                number_attribute(drop_shadow, "dx"),
                Some(f32::from(shadow.offset_x_px()))
            );
            assert_eq!(
                number_attribute(drop_shadow, "dy"),
                Some(f32::from(shadow.offset_y_px()))
            );
            assert_eq!(number_attribute(drop_shadow, "stdDeviation"), Some(0.0));
            assert_eq!(drop_shadow.attribute("flood-color"), Some(shadow.color()));
            filter.attribute("id").expect("filter id")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(effect_ids.len(), state_rects.len());
    let referenced_effect_ids = state_rects
        .iter()
        .map(|node| {
            node.attribute("filter")
                .and_then(|value| value.strip_prefix("url(#"))
                .and_then(|value| value.strip_suffix(')'))
                .expect("State rect must reference one hard-shadow filter")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(referenced_effect_ids, effect_ids);

    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect") && style_value(node, "fill") == Some(tokens.surface())
    }));
    assert!(document.descendants().any(|node| {
        matches!(node.tag_name().name(), "text" | "tspan")
            && style_value(node, "fill") == Some(tokens.text())
    }));

    let root = document.root_element();
    let font_style = root
        .children()
        .find(|node| {
            node.has_tag_name("style") && node.attribute("data-merman-typed-fonts") == Some("v1")
        })
        .expect("the standalone SVG must carry a terminally validated typed font stylesheet");
    let font_css = font_style.text().unwrap_or_default();
    assert!(font_css.contains("@font-face"));
    assert!(font_css.contains("data:font/ttf;base64,"));
    assert!(font_css.contains(r#"font-family:"Excalifont""#));
    assert!(document.descendants().any(|node| node.has_tag_name("text")));
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    assert!(!svg.contains("merman-prepared-"));

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
