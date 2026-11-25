use super::*;
use crate::conversion::EmbeddedDataPlan;
use crate::test_fixtures::{fixture_pdf_options, fixture_raster_options};
use base64::Engine as _;
use merman_model::{
    OperationCancelled, OperationLedgerError, OperationPhase, OperationResourceDomain,
    OperationResourceProvenance,
};

#[test]
fn backend_tree_rechecks_preserve_the_render_owned_resource_phase() {
    for limit_name in [
        merman_render::resources::SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
        merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
    ] {
        let details = ExportError::SvgConversionLimit {
            limit_name,
            actual: 2,
            max: 1,
        }
        .resource_limit_details()
        .expect("backend hard cap details");

        assert_eq!(details.limit_id, limit_name);
        assert_eq!(details.phase, "svg_postprocess");
        assert_eq!(details.actual, 2);
        assert_eq!(details.max, 1);
        assert_eq!(
            ExportError::SvgConversionLimit {
                limit_name,
                actual: 2,
                max: 1,
            }
            .resource_limit_provenance()
            .expect("backend hard cap provenance")
            .domain,
            OperationResourceDomain::Render
        );
        assert_eq!(export_resource_limit_output_ids(limit_name), None);
        assert_eq!(
            export_resource_profile_value(
                merman_render::resources::RenderResourceProfile::Interactive,
                limit_name,
            ),
            None
        );
    }
}

fn compatible_svg(svg: &str) -> merman_render::svg::ResvgCompatibleSvg {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    merman_render::svg::finalize_resvg_svg(svg, &session).unwrap()
}

fn plan_embedded_data_resources(svg: &str, limit: EmbeddedImageLimit) -> Result<EmbeddedDataPlan> {
    let svg = compatible_svg(svg);
    plan_embedded_data_resource_bytes(
        svg.inline_raster_sources().map(|(href, _)| (href, 1)),
        limit,
        &OperationControl::new(),
    )
}

#[test]
fn backend_failure_observes_cancellation_before_returning_the_backend_error() {
    let control = OperationControl::new();
    let job_control = control.clone();
    let error = run_recursive_svg_backend(&control, move |_| -> Result<()> {
        job_control.cancel();
        Err(ExportError::SvgParse)
    })
    .expect_err("the post-backend checkpoint must observe cancellation");

    assert!(matches!(
        error,
        ExportError::Cancelled(OperationCancelled {
            phase: OperationPhase::Export,
            ..
        })
    ));
}

#[test]
fn controlled_export_replays_the_first_resource_terminal_after_later_cancellation() {
    let href = png_data_uri_with_declared_size(1, 1);
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
    );
    let svg = compatible_svg(&source);
    let options = RasterOptions {
        embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
        ..fixture_raster_options()
    };
    let control = OperationControl::new();
    let first = match super::prepare_raster_controlled(&svg, &options, control.clone()) {
        Ok(_) => panic!("the embedded-image byte limit must reject preparation"),
        Err(error) => error,
    };
    let first_details = first
        .resource_limit_details()
        .expect("the first rejection must expose resource metadata");
    let first_terminal = control
        .terminal_checkpoint_at(OperationPhase::Export)
        .expect_err("the first export resource rejection must latch the operation terminal");
    assert!(matches!(
        &first_terminal,
        OperationLedgerError::ResourceLimitExceeded(error)
            if error.id == MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
                && error.resource_phase == "embedded_image_decode"
                && error.limit == first_details.max
                && error.consumed.saturating_add(error.requested) == first_details.actual
                && error.provenance.domain == OperationResourceDomain::Export
                && error.provenance.profile.is_none()
                && error.provenance.explicit_overrides.is_empty()
    ));

    control.cancel();
    let replay = match super::prepare_raster_controlled(
        &compatible_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1"/></svg>"#,
        ),
        &fixture_raster_options(),
        control,
    ) {
        Ok(_) => panic!("the existing terminal must prevent a second prepared artifact"),
        Err(error) => error,
    };

    assert_eq!(replay.resource_limit_details(), Some(first_details));
    assert!(matches!(
        replay,
        ExportError::OperationResourceTerminal(error) if error == first_terminal
    ));
    assert_eq!(
        first_details.limit_id,
        MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
    );
    assert_eq!(first_details.phase, "embedded_image_decode");
    assert_eq!(first_details.cause, ExportResourceLimitCause::Ceiling);
}

#[test]
fn export_checkpoint_preserves_foreign_resource_terminal_provenance() {
    let control = OperationControl::new();
    let provenance = OperationResourceProvenance::new(
        OperationResourceDomain::Render,
        Some(merman_model::resources::ResourceProfile::Constrained),
        [merman_model::OperationResourceOverride {
            id: "max_svg_bytes",
            value: 17,
        }],
    );
    let terminal = control.terminate_resource_overflow(
        "max_svg_bytes",
        OperationPhase::Postprocess,
        "svg_postprocess",
        u64::MAX,
        17,
        provenance,
    );

    let error = export_checkpoint(&control)
        .expect_err("the export adapter must replay the foreign terminal");

    assert!(matches!(
        error,
        ExportError::OperationResourceTerminal(actual) if actual == terminal
    ));
}

#[test]
fn exporter_backend_ignores_local_image_even_before_svg_sanitization() {
    let local_image = TempFile::new("png", encode_rgba_png(1, 1, &[255, 0, 0, 255]));
    let href = escape_xml_attr(&local_image.href_path());
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><image href="{href}" width="1" height="1"/></svg>"#
    );
    let options = usvg::Options::default();
    assert!(
        !usvg::Tree::from_str(&source, &options)
            .unwrap()
            .root()
            .children()
            .is_empty()
    );

    let svg = compatible_svg(&source);
    let parsed = parse_svg_tree(
        &source,
        options,
        SvgExportFonts::Retained(&svg),
        &OperationControl::new(),
    )
    .unwrap();
    assert!(parsed.tree.root().children().is_empty());
}

#[test]
fn export_environment_contract_matches_font_and_image_owners() {
    for output_id in ["jpeg", "pdf", "png"] {
        let contract = output_environment_contract(output_id)
            .expect("each compiled export must disclose its environment");
        assert_eq!(
            contract.system_fonts.is_some(),
            system_font_discovery_available()
        );
        if let Some(system_fonts) = contract.system_fonts {
            assert_eq!(system_fonts.source_id, "host-system");
            assert_eq!(system_fonts.discovery, "first-use");
            assert_eq!(system_fonts.cache_scope, "process-global");
            assert!(system_fonts.host_dependent);
            assert!(!system_fonts.resource_bounded);
        }
        assert_eq!(contract.embedded_images.source_ids, ["data-url"]);
        assert!(!contract.embedded_images.filesystem_access);
        assert!(!contract.embedded_images.network_access);
        assert_eq!(
            contract.embedded_images.default_limits,
            EmbeddedImageLimit::default()
        );
    }
    assert!(output_environment_contract("svg").is_none());
    assert!(output_environment_contract("ascii").is_none());
}

fn trusted_compatible_svg(svg: &str) -> merman_render::svg::ResvgCompatibleSvg {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(merman_render::resources::RenderResourcePolicy::trusted_native())
        .begin_session()
        .unwrap();
    merman_render::svg::finalize_resvg_svg(svg, &session).unwrap()
}

fn svg_to_png(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
    super::svg_to_png(&compatible_svg(svg), options)
}

fn svg_to_png_with_report(
    svg: &str,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterExportReport)> {
    super::svg_to_png_with_report(&compatible_svg(svg), options)
}

fn svg_to_jpeg(svg: &str, options: &RasterOptions) -> Result<Vec<u8>> {
    super::svg_to_jpeg(&compatible_svg(svg), options)
}

fn svg_to_jpeg_with_report(
    svg: &str,
    options: &RasterOptions,
) -> Result<(Vec<u8>, RasterExportReport)> {
    super::svg_to_jpeg_with_report(&compatible_svg(svg), options)
}

fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    super::svg_to_pdf_with_options(&compatible_svg(svg), &fixture_pdf_options())
}

fn nested_group_svg(depth: usize) -> String {
    let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">"#);
    svg.push_str(&r#"<g opacity="0.999">"#.repeat(depth - 1));
    svg.push_str(r#"<rect width="10" height="10" fill="black"/>"#);
    svg.push_str(&"</g>".repeat(depth - 1));
    svg.push_str("</svg>");
    svg
}

fn expanded_use_chain_svg(depth: usize) -> String {
    let mut svg =
        String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs>"#);
    for index in 0..depth {
        if index + 1 == depth {
            svg.push_str(&format!(
                r#"<g id="use-{index}" opacity="0.999"><rect width="10" height="10"/></g>"#
            ));
        } else {
            svg.push_str(&format!(
                r##"<g id="use-{index}" opacity="0.999"><use href="#use-{}"/></g>"##,
                index + 1
            ));
        }
    }
    svg.push_str(r##"</defs><use href="#use-0"/></svg>"##);
    svg
}

fn branching_data_image_use_svg(levels: usize, href: &str) -> String {
    let mut svg =
        String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs>"#);
    svg.push_str(&format!(
        r#"<g id="leaf"><image href="{href}" width="1" height="1"/></g>"#
    ));
    for index in 0..levels {
        let target = if index + 1 == levels {
            "leaf".to_owned()
        } else {
            format!("use-{}", index + 1)
        };
        svg.push_str(&format!(
            r##"<g id="use-{index}"><use href="#{target}"/><use href="#{target}"/></g>"##
        ));
    }
    svg.push_str(r##"</defs><use href="#use-0"/></svg>"##);
    svg
}

fn svg_to_pdf_with_options(svg: &str, options: &PdfOptions) -> Result<Vec<u8>> {
    super::svg_to_pdf_with_options(&compatible_svg(svg), options)
}

fn svg_to_pdf_with_report(svg: &str, options: &PdfOptions) -> Result<(Vec<u8>, PdfExportReport)> {
    super::svg_to_pdf_with_report(&compatible_svg(svg), options)
}

fn prepare_pdf(svg: &str, options: &PdfOptions) -> Result<PreparedPdf> {
    super::prepare_pdf(&compatible_svg(svg), options)
}

fn prepare_raster(svg: &str, options: &RasterOptions) -> Result<PreparedRaster> {
    super::prepare_raster(&compatible_svg(svg), options)
}

fn svg_raster_plan(svg: &str, options: &RasterOptions) -> Result<RasterPlan> {
    super::svg_raster_plan(&compatible_svg(svg), options)
}

#[test]
fn svg_to_png_produces_png_signature() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
    let bytes = svg_to_png(svg, &fixture_raster_options()).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn invalid_output_compositing_colors_fail_during_preparation() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;

    let raster_error = prepare_raster(
        svg,
        &fixture_raster_options().with_matte("not-a-solid-color"),
    )
    .err()
    .expect("invalid raster matte should fail during preparation");
    let pdf_error = prepare_pdf(
        svg,
        &fixture_pdf_options().with_page_paint("not-a-solid-color"),
    )
    .err()
    .expect("invalid PDF page paint should fail during preparation");

    assert!(matches!(raster_error, ExportError::InvalidRasterMatte));
    assert!(matches!(pdf_error, ExportError::InvalidPdfPagePaint));
}

#[test]
fn png_report_retains_dimensions_and_requested_matte() {
    let source = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
    let (bytes, report) =
        svg_to_png_with_report(source, &fixture_raster_options().with_matte("#12345678")).unwrap();

    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(report.output(), RasterOutputKind::Png);
    assert_eq!(
        report.matte(),
        Some(ExportRgbaColor {
            red: 0x12,
            green: 0x34,
            blue: 0x56,
            alpha: 0x78,
        })
    );
    assert!(!report.matte_defaulted());
    assert_eq!(
        (report.raster().width_px, report.raster().height_px),
        (10, 10)
    );
}

#[test]
fn jpeg_report_exposes_its_effective_default_or_requested_matte() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"/>"#;
    let (_, default_report) = svg_to_jpeg_with_report(svg, &fixture_raster_options()).unwrap();
    let (_, requested_report) =
        svg_to_jpeg_with_report(svg, &fixture_raster_options().with_matte("#010203")).unwrap();

    assert_eq!(default_report.output(), RasterOutputKind::Jpeg);
    assert_eq!(default_report.matte(), Some(ExportRgbaColor::WHITE));
    assert!(default_report.matte_defaulted());
    assert_eq!(
        requested_report.matte(),
        Some(ExportRgbaColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 255,
        })
    );
    assert!(!requested_report.matte_defaulted());
}

#[test]
fn pdf_report_retains_page_filter_plan_and_page_paint() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"><rect width="10" height="20" fill="black"/></svg>"#;
    let options = fixture_pdf_options()
        .with_page_policy(PdfPagePolicy::Fixed {
            width_pt: 612.0,
            height_pt: 792.0,
        })
        .with_page_paint("#01020380");
    let (bytes, report) = svg_to_pdf_with_report(svg, &options).unwrap();

    assert!(bytes.starts_with(b"%PDF-"));
    assert_eq!(report.page().page_width_pt(), 612.0);
    assert_eq!(report.page().page_height_pt(), 792.0);
    assert_eq!(report.filters().filtered_groups, 0);
    assert_eq!(
        report.page_paint(),
        Some(ExportRgbaColor {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 128,
        })
    );
}

#[test]
fn rotated_foreign_object_text_and_background_export_at_the_transformed_position() {
    // This is the same translate/rotate/translate shape used by LR Swimlane titles.
    // A long colored label background provides font-independent pixel evidence.
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200"><g transform="translate(60 100) rotate(-90) translate(-40 -10)"><foreignObject width="80" height="20"><div xmlns="http://www.w3.org/1999/xhtml" style="background-color:#ff0000;color:#ffffff"><p>Support</p></div></foreignObject></g></svg>"##;
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let compatible = merman_render::svg::SvgPipeline::resvg_safe()
        .process_resvg_compatible(source, &session)
        .unwrap();
    let bytes = super::svg_to_png(&compatible, &fixture_raster_options()).unwrap();
    assert_eq!(png_size(&bytes), (200, 200));
    for (x, y) in [(51, 70), (51, 100), (51, 130)] {
        assert_eq!(
            rgba_pixel(&bytes, x, y),
            [255, 0, 0, 255],
            "rotated label strip at ({x},{y})"
        );
    }
    assert_eq!(
        rgba_pixel(&bytes, 100, 100)[3],
        0,
        "label must not extend horizontally into nodes"
    );
    assert_eq!(
        rgba_pixel(&bytes, 51, 50)[3],
        0,
        "label stays inside its transformed header strip"
    );
}

#[test]
fn foreign_object_export_preserves_ancestor_clip_opacity_and_local_background() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram" width="100" height="100" viewBox="0 0 100 100"><style>#diagram .node rect { fill:blue!important;stroke:green!important;stroke-width:10!important; }</style><defs><clipPath id="clip"><rect width="25" height="100"/></clipPath></defs><g class="node" opacity=".5" clip-path="url(#clip)"><foreignObject x="10" y="10" width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml" style="background-color:red;color:white"><p>A</p></div></foreignObject></g></svg>"##;
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let compatible = merman_render::svg::SvgPipeline::resvg_safe()
        .process_resvg_compatible(source, &session)
        .unwrap();
    let bytes = super::svg_to_png(&compatible, &fixture_raster_options()).unwrap();
    assert_eq!(rgba_pixel(&bytes, 12, 12), [255, 0, 0, 128]);
    assert_eq!(
        rgba_pixel(&bytes, 35, 12)[3],
        0,
        "ancestor clipping must apply to the generated label"
    );
    assert_eq!(
        rgba_pixel(&bytes, 8, 12)[3],
        0,
        "node shape CSS must not add a label border"
    );
}

#[test]
fn export_reports_retain_sealed_fallback_css_residual() {
    for (css, expected) in [
        ("", false),
        ("<style>tspan{font-size:64px!important}</style>", true),
    ] {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">{css}<foreignObject x="10" y="10" width="180" height="60"><div xmlns="http://www.w3.org/1999/xhtml"><p>Alpha <b>Beta</b></p></div></foreignObject></svg>"#
        );
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let svg = merman_render::svg::SvgPipeline::resvg_safe()
            .process_resvg_compatible(&source, &session)
            .unwrap();
        assert_eq!(svg.finalization_report().fallback_css_residual(), expected);

        let raster = super::prepare_raster(&svg, &fixture_raster_options()).unwrap();
        assert_eq!(
            raster
                .report_for_output(RasterOutputKind::Png)
                .fallback_css_residual(),
            expected
        );
        assert_eq!(
            raster
                .report_for_output(RasterOutputKind::Jpeg)
                .fallback_css_residual(),
            expected
        );
        let (png, report) = raster.encode_png_with_report().unwrap();
        assert_eq!(report.fallback_css_residual(), expected);
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));

        let (jpeg, report) =
            super::svg_to_jpeg_with_report(&svg, &fixture_raster_options()).unwrap();
        assert_eq!(report.fallback_css_residual(), expected);
        assert!(jpeg.starts_with(&[0xff, 0xd8]));

        let pdf = super::prepare_pdf(&svg, &fixture_pdf_options()).unwrap();
        assert_eq!(pdf.report().fallback_css_residual(), expected);
        let (pdf, report) = pdf.encode_with_report().unwrap();
        assert_eq!(report.fallback_css_residual(), expected);
        assert!(pdf.starts_with(b"%PDF-"));
    }
}

#[test]
fn root_viewport_dimensions_drive_raster_planning_and_pixels() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
    let options = fixture_raster_options();
    let plan = svg_raster_plan(svg, &options).unwrap();

    assert_eq!(plan.requested_width_px, 200.0);
    assert_eq!(plan.requested_height_px, 100.0);
    assert_eq!((plan.width_px, plan.height_px), (200, 100));

    let bytes = svg_to_png(svg, &options).unwrap();
    assert_eq!(png_size(&bytes), (200, 100));
    let center = rgba_pixel(&bytes, 100, 50);
    let side = rgba_pixel(&bytes, 10, 50);
    assert!(
        center[0] < 8 && center[1] < 8 && center[2] < 8 && center[3] > 247,
        "expected the viewBox content at the viewport center, got {center:?}"
    );
    assert_eq!(
        side,
        [0, 0, 0, 0],
        "preserveAspectRatio should leave transparent side padding"
    );
}

#[test]
fn explicit_root_size_without_viewbox_preserves_canvas() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="20" y="30" width="10" height="10" fill="red"/></svg>"#;
    let options = fixture_raster_options();
    let png = svg_to_png(svg, &options).unwrap();

    assert_eq!(png_size(&png), (100, 100));
    assert_eq!(rgba_pixel(&png, 25, 35), [255, 0, 0, 255]);
    assert_eq!(rgba_pixel(&png, 0, 0), [0, 0, 0, 0]);

    let jpeg = svg_to_jpeg(svg, &options).unwrap();
    let jpeg = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
    assert_eq!((jpeg.width(), jpeg.height()), (100, 100));
}

#[test]
fn explicit_root_size_without_viewbox_preserves_filter_pixels() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><defs><filter id="shadow" x="-100%" y="-100%" width="400%" height="300%"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><rect x="20" y="30" width="10" height="10" fill="red" filter="url(#shadow)"/></svg>"##;
    let png = svg_to_png(svg, &fixture_raster_options()).unwrap();

    assert_eq!(png_size(&png), (100, 100));
    assert_eq!(rgba_pixel(&png, 25, 35), [255, 0, 0, 255]);
    assert_eq!(rgba_pixel(&png, 40, 35), [0, 0, 255, 255]);
}

#[test]
fn empty_svg_keeps_raster_and_pdf_canvas_sizes() {
    for (attributes, body, raster_size, pdf_size) in [
        ("", "", (1, 1), (100.0, 100.0)),
        (
            r#" width="100%" style="max-width:400px""#,
            r#"<g transform="translate(50 80)" opacity=".5"><g/></g>"#,
            (1, 1),
            (100.0, 100.0),
        ),
        (r#" width="20" height="12""#, "<g/>", (20, 12), (20.0, 12.0)),
        (r#" viewBox="-2 -2 20 12""#, "<g/>", (20, 12), (20.0, 12.0)),
    ] {
        let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"{attributes}>{body}</svg>"#);
        let raster_options = fixture_raster_options().with_size_limit(RasterSizeLimit::new(
            None,
            None,
            Some(u64::from(raster_size.0) * u64::from(raster_size.1)),
        ));
        let prepared = prepare_raster(&svg, &raster_options).unwrap();
        assert!(!prepared.plan().limited);
        let png = prepared.encode_png().unwrap();
        assert_eq!(png_size(&png), raster_size);
        assert_eq!(rgba_pixel(&png, 0, 0), [0; 4]);
        let jpeg = svg_to_jpeg(&svg, &raster_options).unwrap();
        let jpeg = image::load_from_memory_with_format(&jpeg, image::ImageFormat::Jpeg).unwrap();
        assert_eq!((jpeg.width(), jpeg.height()), raster_size);

        let prepared = prepare_pdf(&svg, &fixture_pdf_options()).unwrap();
        let report = prepared.report();
        assert_eq!(
            (
                report.page().page_width_pt(),
                report.page().page_height_pt()
            ),
            pdf_size
        );
        assert_eq!(report.filters().filtered_groups, 0);
        assert_eq!(report.filters().effective_image_pixels, 0);
        assert!(!report.implicit_viewport_clipping_residual());
        assert!(prepared.encode().unwrap().starts_with(b"%PDF-"));
    }
}

#[test]
fn dimensionless_root_keeps_content_fit() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><rect x="20" y="30" width="10" height="10" fill="red"/></svg>"#;
    let png = svg_to_png(svg, &fixture_raster_options()).unwrap();

    assert_eq!(png_size(&png), (10, 10));
    assert_eq!(rgba_pixel(&png, 5, 5), [255, 0, 0, 255]);
}

#[test]
fn dimensionless_root_content_fit_preserves_filter_pixels() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><defs><filter id="shadow" filterUnits="userSpaceOnUse" x="10" y="20" width="40" height="30"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><rect x="20" y="30" width="10" height="10" fill="red" filter="url(#shadow)"/></svg>"##;
    let png = svg_to_png(svg, &fixture_raster_options()).unwrap();

    assert_eq!(png_size(&png), (40, 30));
    assert_eq!(rgba_pixel(&png, 15, 15), [255, 0, 0, 255]);
    assert_eq!(rgba_pixel(&png, 30, 15), [0, 0, 255, 255]);
    assert_eq!(rgba_pixel(&png, 0, 0), [0, 0, 0, 0]);
}

#[test]
fn dimensionless_pdf_reports_content_outside_derived_viewport() {
    for body in [
        r#"<rect x="-10" y="10" width="20" height="20" fill="red"/>"#,
        r##"<defs><filter id="shadow" filterUnits="userSpaceOnUse" x="10" y="20" width="40" height="30"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><rect x="20" y="30" width="10" height="10" fill="red" filter="url(#shadow)"/>"##,
    ] {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px">{body}</svg>"#
        );
        let prepared = prepare_pdf(&source, &fixture_pdf_options())
            .expect("best-effort PDF reports a derived-viewport limitation and still exports");
        assert!(prepared.report().implicit_viewport_clipping_residual());
        assert!(prepared.encode().unwrap().starts_with(b"%PDF-"));
        let explicit = source.replace(
            r#"width="100%" style="max-width:400px""#,
            r#"width="100" height="100""#,
        );
        let prepared = prepare_pdf(&explicit, &fixture_pdf_options())
            .expect("a caller's explicit viewport keeps intentional source clipping");
        assert!(!prepared.report().implicit_viewport_clipping_residual());
    }
}

#[test]
fn dimensionless_pdf_preserves_visible_info_style_text() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width:400px"><text x="100" y="40" font-size="32" text-anchor="middle" fill="#333">v0.9.0</text></svg>"##;
    let prepared = prepare_pdf(source, &fixture_pdf_options())
        .expect("ordinary dimensionless Info text stays inside its derived viewport");
    let page = prepared.report().page();
    assert!(page.page_width_pt > 0.0 && page.page_height_pt > 0.0);
    assert!(!prepared.report().implicit_viewport_clipping_residual());
    let pdf = prepared.encode().unwrap();
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("/Type/Font"),
        "Info must contain actual native text"
    );
}

#[test]
fn raster_directional_filters_report_transform_residual_and_encode() {
    for transform in ["rotate(30)", "skewX(30)"] {
        for primitive in [
            r#"<feDropShadow dx="7" dy="4" stdDeviation="0" flood-color="blue"/>"#,
            r#"<feOffset dx="7" dy="4"/>"#,
        ] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" x="-100%" y="-100%" width="400%" height="400%">{primitive}</filter></defs><g transform="translate(50 50) {transform}"><rect width="10" height="20" fill="red" filter="url(#effect)"/></g></svg>"##
            );
            let prepared = prepare_raster(&svg, &fixture_raster_options())
                .expect("best-effort raster export still renders directional filter transforms");
            assert!(
                prepared
                    .report_for_output(RasterOutputKind::Png)
                    .directional_filter_transform_residual()
            );
            assert!(
                prepared
                    .report_for_output(RasterOutputKind::Jpeg)
                    .directional_filter_transform_residual()
            );
            let (png, report) = prepared.encode_png_with_report().unwrap();
            assert!(report.directional_filter_transform_residual());
            assert_png_has_visible_non_background_ink(&png);
            let (jpeg, report) = svg_to_jpeg_with_report(&svg, &fixture_raster_options()).unwrap();
            assert!(report.directional_filter_transform_residual());
            assert!(jpeg.starts_with(&[0xff, 0xd8]));
        }
    }
}

#[test]
fn raster_filter_offsets_follow_rotated_and_reflected_axes() {
    for (transform, x, y, width, height) in [
        ("rotate(90)", 65, 35, 10, 20),
        ("scale(-1 1)", 40, 20, 20, 10),
    ] {
        let filtered = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="110"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="100" height="100"><feOffset dx="15" dy="0"/></filter></defs><g transform="translate(75 20) {transform}" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##
        );
        let reference = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="110"><rect x="{x}" y="{y}" width="{width}" height="{height}" fill="red"/></svg>"#
        );
        let options = fixture_raster_options();
        let (png, report) = svg_to_png_with_report(&filtered, &options).unwrap();
        assert!(
            !report.directional_filter_transform_residual(),
            "{transform}"
        );
        let reference = svg_to_png(&reference, &options).unwrap();
        assert!(png == reference, "{transform} must paint at the SVG offset");
        assert_eq!(rgba_pixel(&png, x, y), [255, 0, 0, 255]);
        let (_, report) = svg_to_jpeg_with_report(&filtered, &options).unwrap();
        assert!(
            !report.directional_filter_transform_residual(),
            "{transform}"
        );
    }
}

#[test]
fn raster_nondirectional_filters_keep_transformed_content() {
    for primitive in [
        r#"<feGaussianBlur stdDeviation="1"/>"#,
        r#"<feDropShadow dx="0" dy="0" stdDeviation="1" flood-color="blue"/>"#,
        r#"<feOffset dx="0" dy="0"/>"#,
    ] {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" x="-100%" y="-100%" width="400%" height="400%">{primitive}</filter></defs><g transform="translate(50 20) rotate(90)"><rect width="10" height="20" fill="red" filter="url(#effect)"/></g></svg>"##
        );
        let (png, report) = svg_to_png_with_report(&svg, &fixture_raster_options()).unwrap();
        assert!(!report.directional_filter_transform_residual());
        assert!(rgba_pixel(&png, 40, 25)[3] > 200, "{primitive}");
    }
}

#[test]
fn directional_filters_keep_axis_aligned_raster_and_localized_pdf_support() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" primitiveUnits="objectBoundingBox" x="-100%" y="-100%" width="400%" height="400%"><feDropShadow dx=".5" dy=".5" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(20 20) scale(2)"><rect width="10" height="10" fill="red" filter="url(#effect)"/></g></svg>"##;
    let (png, report) = svg_to_png_with_report(svg, &fixture_raster_options()).unwrap();
    assert!(!report.directional_filter_transform_residual());
    assert_eq!(rgba_pixel(&png, 45, 45), [0, 0, 255, 255]);

    let rotated = svg.replace("translate(20 20) scale(2)", "translate(50 20) rotate(90)");
    let pdf = svg_to_pdf(&rotated).unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
}

#[test]
fn pdf_directional_filters_report_residuals_for_transforms_that_mix_axes() {
    for (transform, expects_residual) in [
        ("rotate(90)", false),
        ("scale(-1 1)", false),
        ("rotate(30)", true),
        ("skewX(30)", true),
    ] {
        for primitive in [
            r#"<feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/>"#,
            r#"<feOffset dx="15" dy="0"/>"#,
        ] {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80">{primitive}</filter></defs><g transform="translate(60 20) {transform}" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##
            );
            let prepared = prepare_pdf(&svg, &fixture_pdf_options())
                .expect("best-effort PDF still exports localized directional filter transforms");
            assert_eq!(
                prepared.report().directional_filter_transform_residual(),
                expects_residual,
                "{transform} {primitive}"
            );
            let (pdf, report) = prepared.encode_with_report().unwrap();
            assert_eq!(
                report.directional_filter_transform_residual(),
                expects_residual
            );
            assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
        }
    }
}

#[test]
fn pdf_directional_filters_allow_ancestor_transforms() {
    for transform in ["rotate(90)", "skewX(30)", "scale(-1 1)"] {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g transform="translate(60 20) {transform}"><g filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></g></svg>"##
        );
        let prepared = prepare_pdf(&svg, &fixture_pdf_options())
            .expect("vector ancestor transforms remain supported outside the filter image");
        let plan = prepared.filter_plan();
        assert_eq!(plan.filtered_groups, 1);
        assert!(plan.effective_image_pixels > 0, "{transform}: {plan:?}");
        assert!(!plan.limited, "{transform}: {plan:?}");
        assert!(!prepared.report().directional_filter_transform_residual());
        let pdf = prepared.encode().unwrap();
        assert!(pdf.starts_with(b"%PDF-"));
    }
}

#[test]
fn pdf_nondirectional_filters_keep_local_transforms() {
    for primitive in [
        r#"<feGaussianBlur stdDeviation="1"/>"#,
        r#"<feDropShadow dx="0" dy="0" stdDeviation="1" flood-color="blue"/>"#,
        r#"<feOffset dx="0" dy="0"/>"#,
    ] {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="effect" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80">{primitive}</filter></defs><g transform="translate(60 20) rotate(90)" filter="url(#effect)"><rect width="20" height="10" fill="red"/></g></svg>"##
        );
        let prepared = prepare_pdf(&svg, &fixture_pdf_options()).unwrap();
        assert_eq!(prepared.filter_plan().filtered_groups, 1);
        assert!(prepared.filter_plan().effective_image_pixels > 0);
        assert!(!prepared.report().directional_filter_transform_residual());
        assert!(prepared.encode().unwrap().starts_with(b"%PDF-"));
    }
}

#[test]
fn pdf_directional_filters_report_nested_local_transform_residual() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="outer"><feGaussianBlur stdDeviation="0"/></filter><filter id="shadow" filterUnits="userSpaceOnUse" x="-20" y="-20" width="80" height="80"><feDropShadow dx="15" dy="0" stdDeviation="0" flood-color="blue"/></filter></defs><g filter="url(#outer)"><g transform="translate(60 20) skewX(30)" filter="url(#shadow)"><rect width="20" height="10" fill="red"/></g></g></svg>"##;
    let prepared = prepare_pdf(svg, &fixture_pdf_options())
        .expect("descendant transform limitations must not disable best-effort PDF output");
    assert!(prepared.report().directional_filter_transform_residual());
    let (pdf, report) = prepared.encode_with_report().unwrap();
    assert!(report.directional_filter_transform_residual());
    assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
}

#[test]
fn ignored_and_later_viewbox_aliases_do_not_change_usvg_geometry() {
    for svg in [
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="urn:ignored" i:viewBox="0 0 9000 9000"><rect width="12" height="8" fill="black"/></svg>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" s:viewBox="invalid" viewBox="0 0 9000 9000"><rect width="12" height="8" fill="black"/></svg>"#,
    ] {
        let plan = svg_raster_plan(svg, &fixture_raster_options()).unwrap();
        assert_eq!(
            (plan.width_px, plan.height_px),
            (12, 8),
            "metadata and usvg must agree on the effective viewBox: {svg}"
        );
    }
}

#[test]
fn non_positive_root_dimensions_cannot_cross_the_sealed_raster_boundary() {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();

    for svg in [
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="100" viewBox="0 0 10 10"/>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="-1" viewBox="0 0 10 10"/>"#,
    ] {
        let error = merman_render::svg::finalize_resvg_svg(svg, &session)
            .expect_err("invalid root dimensions must fail before raster preparation");
        assert!(
            error.to_string().contains("must be a positive length"),
            "{error}"
        );
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn scheduling_weights_include_the_recursive_backend_stack() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#;
    let raster = prepare_raster(svg, &fixture_raster_options()).unwrap();
    let pdf = prepare_pdf(svg, &fixture_pdf_options()).unwrap();

    assert!(raster.png_scheduling_weight_bytes() >= RECURSIVE_SVG_BACKEND_STACK_BYTES as u64);
    assert!(pdf.scheduling_weight_bytes() >= RECURSIVE_SVG_BACKEND_STACK_BYTES as u64);
}

#[test]
fn trusted_resvg_backend_handles_the_declared_tree_depth() {
    let depth = merman_render::resources::MAX_RESVG_TREE_DEPTH;
    let svg = trusted_compatible_svg(&nested_group_svg(depth));
    let options = fixture_raster_options().with_conversion_limits(SvgConversionLimits::unbounded());
    let prepared = super::prepare_raster(&svg, &options).unwrap();
    assert_eq!(prepared.report.conversion.max_tree_depth, depth - 1);

    let bytes = prepared.encode_png().unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn trusted_krilla_backend_handles_the_declared_tree_depth() {
    let depth = merman_render::resources::MAX_RESVG_TREE_DEPTH;
    let svg = trusted_compatible_svg(&nested_group_svg(depth));
    let options = fixture_pdf_options().with_conversion_limits(SvgConversionLimits::unbounded());
    let prepared = super::prepare_pdf(&svg, &options).unwrap();
    assert_eq!(prepared.report().conversion().max_tree_depth, depth - 1);

    let bytes = prepared.encode().unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn expanded_use_tree_is_rejected_before_usvg_parsing() {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(merman_render::resources::RenderResourcePolicy::trusted_native())
        .begin_session()
        .unwrap();
    let error = merman_render::svg::finalize_resvg_svg(
        &expanded_use_chain_svg(merman_render::resources::MAX_RESVG_TREE_DEPTH + 2),
        &session,
    )
    .expect_err("the reference preflight must reject the expanded usvg depth");

    assert!(
        error
            .to_string()
            .contains(merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID),
        "{error}"
    );
}

#[test]
fn expanded_use_data_urls_count_toward_the_aggregate_before_usvg_parsing() {
    let href = png_data_uri_with_declared_size(1, 1);
    let svg = compatible_svg(&branching_data_image_use_svg(4, &href));
    let one_source_resource =
        plan_embedded_data_resources(svg.as_str(), EmbeddedImageLimit::unbounded())
            .unwrap()
            .total_bytes;
    let options = RasterOptions {
        embedded_image_limit: EmbeddedImageLimit::new(
            Some(one_source_resource),
            Some(one_source_resource),
            None,
            None,
        ),
        ..fixture_raster_options()
    };

    let error = super::prepare_raster(&svg, &options)
        .err()
        .expect("expanded data URLs must exceed the aggregate preflight budget");

    assert!(error.to_string().contains("max_total_bytes"), "{error}");
}

#[test]
fn filter_and_marker_subroots_count_repeated_data_urls_before_usvg_parsing() {
    let href = png_data_uri_with_declared_size(1, 1);
    let cases = [
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><image id="source" href="{href}" width="1" height="1"/><filter id="f"><feImage href="#source"/></filter></defs><rect width="10" height="10" filter="url(#f)"/></svg>"##
        ),
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><marker id="m"><image href="{href}" width="1" height="1"/></marker></defs><path d="M0 0L1 1L2 2L3 3" marker-mid="url(#m)"/></svg>"##
        ),
    ];

    for raw in cases {
        let one_source_resource =
            plan_embedded_data_resources(&raw, EmbeddedImageLimit::unbounded())
                .unwrap()
                .total_bytes;
        let svg = compatible_svg(&raw);
        let options = RasterOptions {
            embedded_image_limit: EmbeddedImageLimit::new(
                Some(one_source_resource),
                Some(one_source_resource),
                None,
                None,
            ),
            ..fixture_raster_options()
        };

        let error = super::prepare_raster(&svg, &options)
            .err()
            .expect("repeated filter or marker data URLs must exceed the preflight budget");
        assert!(error.to_string().contains("max_total_bytes"), "{error}");
    }
}

#[test]
fn css_effect_fanout_counts_filter_and_mask_data_urls_before_usvg_parsing() {
    let href = png_data_uri_with_declared_size(1, 1);
    let one_resource = plan_embedded_data_resources(
        &format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="{href}"/></svg>"#),
        EmbeddedImageLimit::unbounded(),
    )
    .unwrap()
    .total_bytes;
    let raw = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><filter id="f"><feImage href="{href}"/></filter><mask id="m"><image href="{href}" width="1" height="1"/></mask></defs><style>.affected {{ filter: url(#f); mask: url(#m); }}</style><rect class="affected" width="1" height="1"/><rect class="affected" x="2" width="1" height="1"/><rect class="affected" x="4" width="1" height="1"/></svg>"##
    );
    let svg = compatible_svg(&raw);
    let options = RasterOptions {
        embedded_image_limit: EmbeddedImageLimit::new(
            Some(one_resource),
            Some(one_resource.saturating_mul(2)),
            None,
            None,
        ),
        ..fixture_raster_options()
    };

    let error = super::prepare_raster(&svg, &options)
        .err()
        .expect("CSS effect fanout must exceed the aggregate preflight budget");

    assert!(error.to_string().contains("max_total_bytes"), "{error}");
}

#[test]
fn svg_to_png_does_not_load_local_image_hrefs() {
    let local_image = TempFile::new("png", encode_rgba_png(1, 1, &[255, 0, 0, 255]));
    let href = escape_xml_attr(&local_image.href_path());
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="white"/><image href="{href}" width="10" height="10"/></svg>"#
    );

    let bytes = svg_to_png(&svg, &fixture_raster_options()).unwrap();
    let center = rgba_pixel(&bytes, 5, 5);

    assert!(
        center[0] > 240 && center[1] > 240 && center[2] > 240,
        "expected local image href to be ignored, got center pixel {center:?}"
    );
}

#[test]
fn embedded_image_limits_reject_large_decode_before_png_or_pdf_encoding() {
    let href = png_data_uri_with_declared_size(100_000, 100_000);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
    );

    let png_err = prepare_raster(&svg, &fixture_raster_options())
        .err()
        .expect("PNG preparation should reject the decoded image size");
    let pdf_err = prepare_pdf(&svg, &fixture_pdf_options())
        .err()
        .expect("PDF preparation should reject the decoded image size");

    assert!(png_err.to_string().contains("max_pixels_per_image"));
    assert!(pdf_err.to_string().contains("max_pixels_per_image"));
}

#[test]
fn embedded_image_plan_reports_intrinsic_pixels_from_headers() {
    let href = png_data_uri_with_declared_size(2, 3);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
    );
    let prepared = prepare_raster(&svg, &fixture_raster_options()).unwrap();

    assert_eq!(
        prepared.report.embedded_images,
        EmbeddedImagePlan {
            data_resources: 1,
            raster_images: 1,
            largest_data_bytes: 68,
            total_data_bytes: 68,
            largest_raster_pixels: 6,
            total_pixels: 6,
        }
    );
}

#[test]
fn raster_and_pdf_charge_each_effective_inline_image_source_once() {
    let href = png_data_uri_with_declared_size(1, 1);
    let image = format!(r#"<image x:href="{href}" href="{href}" width="10" height="10"/>"#);
    for (image_count, byte_limit) in [(1, 68), (2, 136)] {
        let svg = compatible_svg(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:x="http://www.w3.org/1999/xlink" viewBox="0 0 10 10">{}</svg>"#,
            image.repeat(image_count)
        ));
        let limit = EmbeddedImageLimit::new(Some(68), Some(byte_limit), None, None);
        let raster = super::prepare_raster(
            &svg,
            &RasterOptions {
                embedded_image_limit: limit,
                ..fixture_raster_options()
            },
        )
        .unwrap();
        let pdf = super::prepare_pdf(
            &svg,
            &PdfOptions {
                embedded_image_limit: limit,
                ..fixture_pdf_options()
            },
        )
        .unwrap();
        assert_eq!(
            svg.resource_closure().inline_data_resource_count(),
            image_count
        );
        for plan in [raster.report.embedded_images, pdf.report.embedded_images] {
            assert_eq!(plan.data_resources, image_count);
            assert_eq!(plan.largest_data_bytes, 68);
            assert_eq!(plan.total_data_bytes, byte_limit);
        }
    }
}

#[test]
fn namespaced_image_and_filter_image_share_href_byte_limits() {
    let href = png_data_uri_with_declared_size(1, 1);
    let single_svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><image href="{href}"/></svg>"#);
    let bytes_per_resource =
        plan_embedded_data_resources(&single_svg, EmbeddedImageLimit::unbounded())
            .unwrap()
            .total_bytes;
    assert!(bytes_per_resource > 0);
    let total_bytes = bytes_per_resource * 2;
    let svg = format!(
        r#"<svg:svg xmlns:svg="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><svg:image xlink:href="{href}"/><svg:defs><svg:filter id="f"><svg:feImage xlink:href="{href}" href="{href}"/></svg:filter></svg:defs></svg:svg>"#
    );

    let plan = plan_embedded_data_resources(
        &svg,
        EmbeddedImageLimit::new(Some(bytes_per_resource), Some(total_bytes), None, None),
    )
    .expect("the exact per-resource and aggregate limits should be accepted");
    assert_eq!(plan.resources, 2);
    assert_eq!(plan.largest_bytes, bytes_per_resource);
    assert_eq!(plan.total_bytes, total_bytes);

    let per_resource_error = plan_embedded_data_resources(
        &svg,
        EmbeddedImageLimit::new(Some(bytes_per_resource - 1), None, None, None),
    )
    .expect_err("both image element kinds must enforce the per-resource byte limit");
    assert!(matches!(
        per_resource_error,
        ExportError::EmbeddedImageLimit {
            limit_name: "max_bytes_per_image",
            actual,
            max,
        } if actual > max
    ));

    let aggregate_error = plan_embedded_data_resources(
        &svg,
        EmbeddedImageLimit::new(Some(bytes_per_resource), Some(total_bytes - 1), None, None),
    )
    .expect_err("feImage must contribute to the aggregate byte limit");
    assert!(matches!(
        aggregate_error,
        ExportError::EmbeddedImageLimit {
            limit_name: "max_total_bytes",
            actual,
            max,
        } if actual > max
    ));
}

#[test]
fn external_image_hrefs_remain_outside_the_data_url_budget() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><image href="https://example.invalid/image.png"/><defs><filter id="f"><feImage xlink:href="file:///tmp/image.png"/></filter></defs></svg>"#;

    let plan =
        plan_embedded_data_resources(svg, EmbeddedImageLimit::new(Some(1), Some(1), None, None))
            .expect("external references are handled by the separate href resolver policy");

    assert_eq!(plan.resources, 0);
    assert_eq!(plan.largest_bytes, 0);
    assert_eq!(plan.total_bytes, 0);
}

#[test]
fn embedded_image_bytes_are_limited_before_usvg_decodes_data_urls() {
    let href = png_data_uri_with_declared_size(1, 1);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><image href="{href}" width="10" height="10"/></svg>"#
    );
    let options = RasterOptions {
        embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
        ..fixture_raster_options()
    };

    let error = prepare_raster(&svg, &options)
        .err()
        .expect("data URL should be rejected before usvg parsing");

    assert!(error.to_string().contains("max_bytes_per_image"));
}

#[test]
fn filter_image_bytes_are_limited_before_usvg_decodes_data_urls() {
    let href = png_data_uri_with_declared_size(1, 1);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><filter id="f"><feImage href="{href}"/></filter></defs><rect width="10" height="10" filter="url(#f)"/></svg>"#
    );
    let raster_options = RasterOptions {
        embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
        ..fixture_raster_options()
    };
    let pdf_options = PdfOptions {
        embedded_image_limit: EmbeddedImageLimit::new(Some(16), None, None, None),
        ..fixture_pdf_options()
    };

    let png_error = prepare_raster(&svg, &raster_options)
        .err()
        .expect("filter data URL should be rejected before raster usvg parsing");
    let pdf_error = prepare_pdf(&svg, &pdf_options)
        .err()
        .expect("filter data URL should be rejected before PDF usvg parsing");

    assert!(png_error.to_string().contains("max_bytes_per_image"));
    assert!(pdf_error.to_string().contains("max_bytes_per_image"));
}

#[test]
fn filter_image_pixels_are_limited_after_header_decode() {
    let href = png_data_uri_with_declared_size(100_000, 100_000);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10"><defs><filter id="f"><feImage xlink:href="{href}"/></filter></defs><rect width="10" height="10" filter="url(#f)"/></svg>"#
    );

    let png_error = prepare_raster(&svg, &fixture_raster_options())
        .err()
        .expect("filter image intrinsic pixels should be bounded for raster output");
    let pdf_error = prepare_pdf(&svg, &fixture_pdf_options())
        .err()
        .expect("filter image intrinsic pixels should be bounded for PDF output");

    assert!(png_error.to_string().contains("max_pixels_per_image"));
    assert!(pdf_error.to_string().contains("max_pixels_per_image"));
}

#[test]
fn embedded_image_limits_cover_pattern_subroots() {
    let href = png_data_uri_with_declared_size(100, 100);
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><defs><pattern id="p" patternUnits="userSpaceOnUse" width="1" height="1"><image href="{href}" width="1" height="1"/></pattern></defs><rect width="10" height="10" fill="url(#p)"/></svg>"#
    );

    let options = RasterOptions {
        embedded_image_limit: EmbeddedImageLimit::new(None, None, Some(10), None),
        ..fixture_raster_options()
    };
    let error = prepare_raster(&svg, &options)
        .err()
        .expect("pattern raster image should be checked");

    assert!(error.to_string().contains("max_pixels_per_image"));
}

#[test]
fn svg_to_pdf_produces_pdf_signature() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#;
    let bytes = svg_to_pdf(svg).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn fixed_pdf_page_policy_uses_requested_media_box() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 20"><rect width="10" height="20" fill="black"/></svg>"#;
    let bytes = svg_to_pdf_with_options(
        svg,
        &fixture_pdf_options()
            .with_page_paint("white")
            .with_page_policy(PdfPagePolicy::Fixed {
                width_pt: 612.0,
                height_pt: 792.0,
            }),
    )
    .unwrap();
    let pdf = String::from_utf8_lossy(&bytes);
    let media_box = pdf
        .find("/MediaBox")
        .map(|start| &pdf[start..pdf.len().min(start + 80)])
        .expect("fixed PDF media box");

    assert!(
        media_box.contains("612") && media_box.contains("792"),
        "{media_box}"
    );
}

#[test]
fn fixed_pdf_page_policy_rejects_invalid_page_dimensions() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;
    let err = svg_to_pdf_with_options(
        svg,
        &fixture_pdf_options().with_page_policy(PdfPagePolicy::Fixed {
            width_pt: f32::NAN,
            height_pt: 792.0,
        }),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("fixed PDF page dimensions"),
        "{err}"
    );
}

#[test]
fn css_width_pdf_page_policy_matches_browser_pixel_to_point_sizing() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#;
    let bytes = svg_to_pdf_with_options(
        svg,
        &fixture_pdf_options().with_page_policy(PdfPagePolicy::FitCssWidth {
            max_width_px: 800.0,
        }),
    )
    .unwrap();
    let pdf = String::from_utf8_lossy(&bytes);
    let media_box = pdf
        .find("/MediaBox")
        .map(|start| &pdf[start..pdf.len().min(start + 80)])
        .expect("CSS-sized PDF media box");

    assert!(media_box.contains("600"), "{media_box}");
}

#[test]
fn fixed_pdf_page_scales_large_vector_source_without_pixel_allocation_limits() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#;
    let bytes = svg_to_pdf_with_options(
        svg,
        &fixture_pdf_options().with_page_policy(PdfPagePolicy::Fixed {
            width_pt: 612.0,
            height_pt: 792.0,
        }),
    )
    .unwrap();

    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn svg_to_pdf_preserves_large_intrinsic_vector_page() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 9000"><rect width="9000" height="9000" fill="black"/></svg>"#;
    let bytes = svg_to_pdf(svg).unwrap();
    let pdf = String::from_utf8_lossy(&bytes);
    let media_box = pdf
        .find("/MediaBox")
        .map(|start| &pdf[start..pdf.len().min(start + 80)])
        .expect("large PDF media box");

    assert!(
        media_box.contains("9000"),
        "large vector dimensions should survive PDF conversion: {media_box}"
    );
}

#[test]
fn svg_to_pdf_rejects_invalid_filter_scale() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"/>"#;
    let err =
        svg_to_pdf_with_options(svg, &fixture_pdf_options().with_filter_scale(0.0)).unwrap_err();

    assert!(err.to_string().contains("PDF filter_scale"), "{err}");
}

#[test]
fn pdf_filter_plan_bounds_aggregate_localized_rasterization() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10000 10000">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="black"/></g>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="white"/></g>
        </svg>"##;
    let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
    let plan = prepared.report().filters();

    assert_eq!(plan.filtered_groups, 2);
    assert_eq!(plan.requested_image_pixels, 50_000_000);
    assert!(plan.limited, "{plan:?}");
    assert!(
        plan.effective_image_pixels <= DEFAULT_MAX_PDF_FILTER_IMAGE_PIXELS,
        "{plan:?}"
    );
    assert!(plan.effective_scale < plan.requested_scale, "{plan:?}");
}

#[test]
fn pdf_filter_plan_allows_explicit_trusted_unbounded_policy() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10000 10000">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="black"/></g>
          <g filter="url(#blur)"><rect width="10000" height="10000" fill="white"/></g>
        </svg>"##;
    let prepared = prepare_pdf(svg, &fixture_pdf_options().with_unbounded_filter_images()).unwrap();
    let plan = prepared.report().filters();

    assert_eq!(plan.requested_image_pixels, 50_000_000);
    assert_eq!(plan.effective_image_pixels, 50_000_000);
    assert!(!plan.limited, "{plan:?}");
}

const TWO_FILTERED_SQUARES: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter></defs><rect x="10" y="10" width="20" height="20" fill="red" filter="url(#identity)"/><rect x="50" y="10" width="20" height="20" fill="blue" filter="url(#identity)"/></svg>"##;

#[test]
fn pdf_filter_budget_rejects_zero_dimension_images() {
    let error = prepare_pdf(
        TWO_FILTERED_SQUARES,
        &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(1))),
    )
    .err()
    .expect("two filtered groups cannot be represented by one retained pixel");

    let details = error
        .resource_limit_details()
        .expect("typed filter budget failure");
    assert_eq!(
        details.limit_id,
        MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID
    );
    assert_eq!(details.phase, "pdf_filter_rasterization");
    assert!(details.actual >= 2, "{details:?}");
    assert_eq!(details.max, 1);
}

#[test]
fn pdf_filter_cap_rejects_zero_dimension_images() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="identity" filterUnits="userSpaceOnUse" x="0" y="0" width="1000000" height="1"><feGaussianBlur stdDeviation="0"/></filter></defs><rect width="1000000" height="1" fill="red" filter="url(#identity)"/></svg>"##;
    let error = prepare_pdf(svg, &fixture_pdf_options())
        .err()
        .expect("the backend's side cap must not silently erase a thin filtered group");

    assert!(matches!(error, ExportError::InvalidSizing(_)), "{error}");
}

#[test]
fn pdf_filter_small_requested_scale_rejects_zero_dimension_images() {
    let error = prepare_pdf(
        TWO_FILTERED_SQUARES,
        &fixture_pdf_options().with_filter_scale(0.001),
    )
    .err()
    .expect("a requested scale that erases every filter image must fail preparation");

    assert!(matches!(error, ExportError::InvalidSizing(_)), "{error}");
}

#[test]
fn pdf_filter_budget_retains_nonzero_images_when_feasible() {
    let prepared = prepare_pdf(
        TWO_FILTERED_SQUARES,
        &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(2))),
    )
    .unwrap();
    let plan = prepared.filter_plan();
    assert_eq!(plan.filtered_groups, 2);
    assert_eq!(plan.effective_image_pixels, 2);

    let pdf = String::from_utf8_lossy(&prepared.encode().unwrap()).into_owned();
    assert!(
        pdf.contains("/Subtype/Image"),
        "filtered content must survive encoding"
    );
}

#[test]
fn filter_output_without_source_geometry_is_still_budgeted() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><filter id="flood" filterUnits="userSpaceOnUse" x="20" y="30" width="8" height="4"><feFlood flood-color="red"/></filter></defs><g filter="url(#flood)"><g/></g></svg>"##;
    let png = svg_to_png(svg, &fixture_raster_options()).unwrap();
    assert_eq!(png_size(&png), (8, 4));
    assert_eq!(rgba_pixel(&png, 3, 2), [255, 0, 0, 255]);

    let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
    assert_eq!(prepared.filter_plan().filtered_groups, 1);
    assert_eq!(prepared.filter_plan().requested_image_pixels, 32 * 16);
    let pdf = prepared.encode().unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
}

#[test]
fn pdf_filter_budget_matches_composed_transforms() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><defs><filter id="identity" filterUnits="userSpaceOnUse" x="0" y="0" width="10" height="10"><feGaussianBlur stdDeviation="0"/></filter></defs><g transform="translate(50 50) rotate(-45)"><g transform="rotate(45)" filter="url(#identity)"><rect width="10" height="10" fill="red"/></g></g></svg>"##;
    let prepared = prepare_pdf(
        svg,
        &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(40 * 40))),
    )
    .unwrap();
    let plan = prepared.filter_plan();
    assert_eq!(plan.filtered_groups, 1);
    assert_eq!(plan.requested_image_pixels, 40 * 40);
    assert!(!plan.limited);
    let pdf = prepared.encode().unwrap();
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(pdf.contains("/Subtype/Image"));
    assert!(pdf.contains("/Width 40") && pdf.contains("/Height 40"));
}

#[test]
fn pdf_filter_budget_includes_mask_and_pattern_subroots() {
    for body in [
        r##"<defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter><mask id="mask"><rect width="80" height="80" fill="white" filter="url(#identity)"/></mask></defs><rect width="80" height="80" fill="red" mask="url(#mask)"/>"##,
        r##"<defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter><pattern id="pattern" patternUnits="userSpaceOnUse" width="80" height="80"><rect width="80" height="80" fill="red" filter="url(#identity)"/></pattern></defs><rect width="80" height="80" fill="url(#pattern)"/>"##,
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">{body}</svg>"#
        );
        let prepared = prepare_pdf(
            &svg,
            &fixture_pdf_options().with_filter_image_limit(PdfFilterImageLimit::new(Some(1))),
        )
        .unwrap();
        let plan = prepared.filter_plan();
        assert_eq!(plan.filtered_groups, 1, "{body}");
        assert!(plan.requested_image_pixels > 1, "{plan:?}");
        assert_eq!(plan.effective_image_pixels, 1);
        assert!(plan.limited);
        let pdf = prepared.encode().unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("/Subtype/Image"));
    }
}

#[test]
fn pdf_filter_plan_matches_isolated_content_stream_scale() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><defs><filter id="identity"><feGaussianBlur stdDeviation="0"/></filter></defs><g transform="scale(5)" opacity=".5"><rect width="16" height="16" fill="red" filter="url(#identity)"/></g></svg>"##;
    let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
    let plan = prepared.filter_plan();
    // 16px * the default 1.2 filter region * 4 sampling = 76.8, rounded to 77.
    // The opacity content stream is isolated before the parent's scale is reapplied.
    assert_eq!(plan.requested_image_pixels, 77 * 77);
    let pdf = prepared.encode().unwrap();
    let pdf = String::from_utf8_lossy(&pdf);
    assert!(
        pdf.contains("/Width 77") || pdf.contains("/Width77"),
        "{pdf}"
    );
    assert!(
        pdf.contains("/Height 77") || pdf.contains("/Height77"),
        "{pdf}"
    );
}

#[test]
fn conversion_plan_rejects_filter_primitive_fanout_before_backend_rendering() {
    let primitives = (0..=DEFAULT_MAX_FILTER_PRIMITIVES_PER_FILTER)
            .map(|index| {
                format!(
                    r#"<feColorMatrix in="SourceGraphic" result="p{index}" type="matrix" values="1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0"/>"#
                )
            })
            .collect::<String>();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><defs><filter id="f">{primitives}</filter></defs><g filter="url(#f)"><rect width="16" height="16"/></g></svg>"#
    );

    let error = prepare_pdf(&svg, &fixture_pdf_options())
        .err()
        .expect("filter primitive fanout should be rejected during preparation");

    assert!(
        error
            .to_string()
            .contains("max_filter_primitives_per_filter"),
        "{error}"
    );
}

#[test]
fn conversion_plan_rejects_deep_isolation_before_backend_rendering() {
    let depth = DEFAULT_MAX_SVG_ISOLATION_DEPTH + 1;
    let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">"#);
    svg.push_str(&r#"<g opacity="0.99">"#.repeat(depth));
    svg.push_str(r#"<rect width="16" height="16"/>"#);
    svg.push_str(&"</g>".repeat(depth));
    svg.push_str("</svg>");

    let error = prepare_raster(&svg, &fixture_raster_options())
        .err()
        .expect("deep isolation should be rejected during preparation");

    assert!(error.to_string().contains("max_isolation_depth"), "{error}");
}

#[test]
fn pdf_filter_plan_counts_only_krilla_owned_outer_filtered_groups() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
          <defs><filter id="blur"><feGaussianBlur stdDeviation="2"/></filter></defs>
          <g filter="url(#blur)"><g filter="url(#blur)"><rect width="100" height="100"/></g></g>
        </svg>"##;

    let prepared = prepare_pdf(svg, &fixture_pdf_options()).unwrap();
    let report = prepared.report();

    assert_eq!(report.filters().filtered_groups, 1);
    assert_eq!(report.conversion().filtered_groups, 2);
    assert_eq!(report.conversion().filter_primitives, 2);
}

#[test]
fn svg_to_jpeg_defaults_to_white_background() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"></svg>"#;
    let bytes = svg_to_jpeg(svg, &fixture_raster_options()).unwrap();
    let img = image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg)
        .unwrap()
        .to_rgb8();
    let px = img.get_pixel(0, 0);

    assert!(
        px[0] > 240 && px[1] > 240 && px[2] > 240,
        "expected default JPG background to be white-ish, got {px:?}"
    );
}

#[test]
fn jpeg_encoder_limit_is_checked_before_allocating_the_pixmap() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 65536 1"><rect width="65536" height="1" fill="black"/></svg>"#;
    let err = svg_to_jpeg(svg, &fixture_raster_options().with_unbounded_size()).unwrap_err();

    assert!(matches!(err, ExportError::JpegDimensionLimit));
}

#[test]
fn svg_to_png_keeps_text_visible_when_requested_font_is_missing() {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width: 400px; background-color: white;"><text x="100" y="40" fill="#333333" font-size="32" style="font-family: '__merman_missing_font__'; text-anchor: middle;">v{}</text></svg>"##,
        merman_model::baseline::PINNED_MERMAID_BASELINE_VERSION
    );
    let bytes = svg_to_png(&svg, &fixture_raster_options()).unwrap();
    assert_png_has_visible_non_background_ink(&bytes);
}

#[test]
fn default_plan_downscales_large_intrinsic_svg_without_allocating() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 14544.4375 6565.5"><rect width="14544.4375" height="6565.5" fill="white"/></svg>"#;
    let plan = svg_raster_plan(svg, &fixture_raster_options()).unwrap();

    assert_eq!(plan.requested_width_px, 14545.0);
    assert_eq!(plan.requested_height_px, 6566.0);
    assert_eq!(plan.width_px, DEFAULT_MAX_RASTER_SIDE_LENGTH);
    assert!(plan.height_px < DEFAULT_MAX_RASTER_SIDE_LENGTH);
    assert!(plan.limited);
    assert!(plan.effective_scale < plan.requested_scale);
}

#[test]
fn fit_to_models_browser_preview_container_before_scale() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><rect width="1000" height="500" fill="black"/></svg>"#;
    let options = fixture_raster_options()
        .with_fit_to(RasterFitBox::width(250))
        .with_scale(2.0);
    let plan = svg_raster_plan(svg, &options).unwrap();

    assert_eq!(plan.requested_width_px, 500.0);
    assert_eq!(plan.requested_height_px, 250.0);
    assert_eq!(plan.width_px, 500);
    assert_eq!(plan.height_px, 250);
    assert!(!plan.limited);
}

#[test]
fn size_limit_caps_actual_png_dimensions() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><rect width="1000" height="500" fill="black"/></svg>"#;
    let options = fixture_raster_options()
        .with_size_limit(RasterSizeLimit::max_side_length(128))
        .with_matte("white");
    let bytes = svg_to_png(svg, &options).unwrap();
    let (width, height) = png_size(&bytes);

    assert_eq!((width, height), (128, 64));
}

#[test]
fn size_limit_caps_by_total_pixels() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000"><rect width="1000" height="1000" fill="black"/></svg>"#;
    let options =
        fixture_raster_options().with_size_limit(RasterSizeLimit::new(None, None, Some(10_000)));
    let plan = svg_raster_plan(svg, &options).unwrap();

    assert_eq!((plan.width_px, plan.height_px), (100, 100));
    assert!(plan.limited);
}

#[test]
fn unbounded_size_keeps_requested_dimensions() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 9000 4500"><rect width="9000" height="4500" fill="black"/></svg>"#;
    let plan = svg_raster_plan(svg, &fixture_raster_options().with_unbounded_size()).unwrap();

    assert_eq!((plan.width_px, plan.height_px), (9000, 4500));
    assert!(!plan.limited);
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let reader = decoder.read_info().expect("png read_info");
    let info = reader.info();
    (info.width, info.height)
}

fn rgba_pixel(bytes: &[u8], x: u32, y: u32) -> [u8; 4] {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("png read_info");
    let size = reader
        .output_buffer_size()
        .expect("invalid png output buffer size");
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).expect("png next_frame");
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    assert!(x < info.width && y < info.height);
    let offset = ((y * info.width + x) as usize) * 4;
    [
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]
}

fn encode_rgba_png(width: u32, height: u32, data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png write_header");
        writer.write_image_data(data).expect("png write_image_data");
    }
    bytes
}

fn png_data_uri_with_declared_size(width: u32, height: u32) -> String {
    let mut png = encode_rgba_png(1, 1, &[0, 0, 0, 0]);
    png[16..20].copy_from_slice(&width.to_be_bytes());
    png[20..24].copy_from_slice(&height.to_be_bytes());
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

struct TempFile {
    path: std::path::PathBuf,
}

impl TempFile {
    fn new(extension: &str, data: Vec<u8>) -> Self {
        let path = std::env::temp_dir().join(format!(
            "merman-raster-{}-{}.{}",
            std::process::id(),
            line!(),
            extension
        ));
        std::fs::write(&path, data).expect("write temp image");
        Self { path }
    }

    fn href_path(&self) -> String {
        self.path.to_string_lossy().replace('\\', "/")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn assert_png_has_visible_non_background_ink(bytes: &[u8]) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("png read_info");
    let size = reader
        .output_buffer_size()
        .expect("invalid png output buffer size");
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).expect("png next_frame");

    assert_eq!(
        info.color_type,
        png::ColorType::Rgba,
        "expected RGBA PNG output"
    );
    assert_eq!(
        info.bit_depth,
        png::BitDepth::Eight,
        "expected 8-bit PNG output"
    );

    let pixels = &buf[..info.buffer_size()];
    let Some(background) = pixels.as_chunks::<4>().0.iter().next() else {
        panic!("expected at least one PNG pixel");
    };
    let differing_pixels = pixels
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| {
            let alpha_delta = px[3].abs_diff(background[3]) as u16;
            let rgb_delta = px[0].abs_diff(background[0]) as u16
                + px[1].abs_diff(background[1]) as u16
                + px[2].abs_diff(background[2]) as u16;
            alpha_delta > 3 || (px[3] > 0 && rgb_delta > 8)
        })
        .take(16)
        .count();
    assert!(
        differing_pixels >= 8,
        "expected visible text ink in rasterized PNG"
    );
}
