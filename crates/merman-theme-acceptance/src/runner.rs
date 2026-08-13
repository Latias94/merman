use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler,
    DiagramThemeSpec, DocumentRenderReport, EffectBinding, EffectGraph, EffectInput,
    EffectPrimitive, FilterRegion, FontAssetSpec, FontCatalogSpec, FontEmbeddingRequirement,
    FontSource, FontStack, OrdinalPalette, RenderEnvironment, RenderFamilyKind, RenderTargetKind,
    RenderedDocument, Specified, TargetAdmissionReport, TargetAdmissionStatus, ThemeAssets,
    ThemeCapability, ThemeColorValue, ThemePortabilityRequirement, ThemeRecipeFingerprint,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman::{Engine, MermaidConfig};
use merman_theme_fixtures::{
    C6AcceptanceCatalog, C6EnforcedCell, C6ProofFamily, C6ProofTheme, ExpectedOutputTarget,
    ReferenceCanvasLayer, ReferenceDiagramFamily, ReferenceFontBinding, ReferenceSemanticRule,
    ReferenceSemanticTarget, ReferenceThemeInput, ReferenceThemeMechanism, ThemeFixtureCatalog,
};

use crate::observation::{
    C6CellReceipt, C6ExecutionReport, C6ObservedMechanismDisposition, C6ReceiptBook,
    C6RenderGroupKey, C6RenderGroupReceipt, C6RenderLane, C6RuntimeError, C6TargetProof,
    seal_cell_from_reports,
};

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

pub fn run_enforced_c6_runtime() -> Result<C6ExecutionReport, C6RuntimeError> {
    let theme_catalog = ThemeFixtureCatalog::load(themes_root())?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum C6RenderGroupAdapter {
    BrutalistState,
}

#[derive(Debug)]
struct C6RenderGroupPlan {
    key: C6RenderGroupKey,
    adapter: C6RenderGroupAdapter,
    cells: Vec<C6EnforcedCell>,
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

        let mut grouped =
            BTreeMap::<C6RenderGroupKey, (C6RenderGroupAdapter, Vec<C6EnforcedCell>)>::new();
        for cell in enforced {
            let adapter = adapter_for_cell(cell)?;
            let key = C6RenderGroupKey::for_cell(cell);
            let entry = grouped.entry(key).or_insert_with(|| (adapter, Vec::new()));
            if entry.0 != adapter {
                return Err(C6RuntimeError::UnsupportedEnforcedCell { key: cell.key() });
            }
            entry.1.push(cell.clone());
        }

        Ok(Self {
            groups: grouped
                .into_iter()
                .map(|(key, (adapter, cells))| C6RenderGroupPlan {
                    key,
                    adapter,
                    cells,
                })
                .collect(),
        })
    }
}

fn adapter_for_cell(cell: &C6EnforcedCell) -> Result<C6RenderGroupAdapter, C6RuntimeError> {
    let key = C6RenderGroupKey::for_cell(cell);
    match (key.theme(), key.family(), key.lane(), cell.key().target()) {
        (
            C6ProofTheme::Brutalist,
            C6ProofFamily::State,
            C6RenderLane::Native,
            ExpectedOutputTarget::StandaloneSvg
            | ExpectedOutputTarget::Png
            | ExpectedOutputTarget::Jpeg
            | ExpectedOutputTarget::Pdf,
        ) => Ok(C6RenderGroupAdapter::BrutalistState),
        _ => Err(C6RuntimeError::UnsupportedEnforcedCell { key: cell.key() }),
    }
}

struct C6CompletedRenderGroup {
    receipt: C6RenderGroupReceipt,
    cells: Vec<C6CellReceipt>,
}

fn execute_render_group(
    theme_catalog: &ThemeFixtureCatalog,
    group: C6RenderGroupPlan,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    match group.adapter {
        C6RenderGroupAdapter::BrutalistState => execute_brutalist_state_group(theme_catalog, group),
    }
}

fn execute_brutalist_state_group(
    theme_catalog: &ThemeFixtureCatalog,
    group: C6RenderGroupPlan,
) -> Result<C6CompletedRenderGroup, C6RuntimeError> {
    let fixture = theme_catalog
        .fixture(group.key.source_fixture_id())
        .ok_or_else(|| render_group_error(&group.key, "source-fixture"))?;
    let input = fixture
        .theme_input()
        .ok_or_else(|| render_group_error(&group.key, "theme-input"))?;
    let source = theme_catalog.source_text(fixture.id())?;
    let theme = compile_brutalist_state_theme(theme_catalog, input);
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
        .expect("the C6 render group must pass strict SVG admission");
    let sealed_svg = admitted.as_str();
    let mechanisms = assert_brutalist_state_svg(input, sealed_svg);
    assert_eq!(
        mechanisms.keys().copied().collect::<BTreeSet<_>>(),
        input.mechanisms()
    );
    let group_receipt = C6RenderGroupReceipt::seal(
        group.key.clone(),
        theme_catalog,
        document.family_kind(),
        document.theme_recipe_fingerprint(),
        sealed_svg,
        admitted.document_report(),
    )?;

    let needs_png_support = group.cells.iter().any(|cell| {
        matches!(
            cell.key().target(),
            ExpectedOutputTarget::Png | ExpectedOutputTarget::Jpeg
        )
    });
    let png_support = needs_png_support
        .then(|| observe_brutalist_state_png(input, sealed_svg, &theme, &document))
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
                .expect("PNG support was planned for the PNG cell")
                .0
                .clone(),
            ExpectedOutputTarget::Jpeg => observe_brutalist_state_jpeg(
                input,
                sealed_svg,
                &theme,
                &document,
                &png_support
                    .as_ref()
                    .expect("PNG control proof was planned for the JPEG cell")
                    .1,
            )?,
            ExpectedOutputTarget::Pdf => {
                observe_brutalist_state_pdf(input, sealed_svg, &theme, &document)?
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
fn observe_brutalist_state_png(
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
) -> Result<(C6TargetObservation, PngArtifactProof), C6RuntimeError> {
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
    let target_proof = proof.target_proof(&png);
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
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
    png_proof: &PngArtifactProof,
) -> Result<C6TargetObservation, C6RuntimeError> {
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
    let proof = prove_brutalist_state_jpeg(input, sealed_svg, &jpeg, png_proof);
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
    Ok(C6TargetObservation {
        recipe_fingerprint: report.operation_report().theme_recipe_fingerprint(),
        document: report.document_report().clone(),
        admission: report.target_admission().clone(),
        proof,
    })
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn observe_brutalist_state_pdf(
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    theme: &DiagramTheme,
    document: &RenderedDocument,
) -> Result<C6TargetObservation, C6RuntimeError> {
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
    let proof = prove_brutalist_state_pdf(input, sealed_svg, &pdf, filter_plan.effective_scale);
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
    Ok(C6TargetObservation {
        recipe_fingerprint: report.operation_report().theme_recipe_fingerprint(),
        document: report.document_report().clone(),
        admission: report.target_admission().clone(),
        proof,
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use merman_theme_fixtures::C6_ACCEPTANCE_RELATIVE_PATH;
    use serde_json::{Value, json};
    use std::fs;

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
                        "sourceFixtureId": "fixture-c6-brutalist-state"
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
}
