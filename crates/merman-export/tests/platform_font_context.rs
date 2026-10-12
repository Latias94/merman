#![cfg(feature = "experimental-system-font-query")]

use merman_core::{OperationControl, OperationLedgerError, OperationPhase};
use merman_export::{NativeFontContext, NativeFontError, NativeFontLimits, NativeFontOptions};
use merman_render::environment::{
    HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer, TextMeasurementOperation,
    TextMeasurementPhase, validate_host_text_measurement,
};
use merman_render::text::{TextStyle, WrapMode};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn options() -> NativeFontOptions {
    NativeFontOptions::default().with_font_file(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/Cousine-Regular.ttf"),
    )
}

fn style(family: &str) -> TextStyle {
    TextStyle {
        font_family: Some(family.to_owned()),
        font_size: 22.0,
        font_weight: Some("400".to_owned()),
        font_style: Some("normal".to_owned()),
    }
}

#[cfg(target_os = "windows")]
fn styled(family: &str, weight: &str) -> TextStyle {
    TextStyle {
        font_family: Some(family.to_owned()),
        font_size: 22.0,
        font_weight: Some(weight.to_owned()),
        font_style: Some("normal".to_owned()),
    }
}

fn request<'a>(text: &'a str, style: &'a TextStyle) -> HostTextMeasurementRequest<'a> {
    HostTextMeasurementRequest {
        operation: TextMeasurementOperation::CanvasMeasureTextWidth,
        phase: TextMeasurementPhase::ComputedLength,
        text,
        style,
        max_width: None,
        wrap_mode: WrapMode::SvgLike,
    }
}

fn advance(context: &NativeFontContext, text: &str, style: &TextStyle) -> f64 {
    match context
        .measure(request(text, style))
        .expect("measurement succeeds")
        .expect("font covers the label")
    {
        HostTextMeasurement::Length(width) => width,
        other => panic!("expected computed length, got {other:?}"),
    }
}

fn paint(context: &NativeFontContext, family: &str) -> usvg::Tree {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="100"><g font-family="{family}" font-size="22"><text id="left" x="200" y="50" text-anchor="start">AVA</text><text id="right" x="200" y="50" text-anchor="end">AVA</text></g></svg>"#,
    );
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    usvg::Tree::from_str(&source, &options).expect("export shapes covered text")
}

fn painted_text<'a>(tree: &'a usvg::Tree, id: &str) -> &'a usvg::Text {
    let Some(usvg::Node::Text(text)) = tree.node_by_id(id) else {
        panic!("export retains text probe {id}");
    };
    text
}

fn assert_same_advance_and_faces(context: &NativeFontContext, family: &str) {
    let actual = advance(context, "AVA", &style(family));
    let tree = paint(&context.clone(), family);
    let left = painted_text(&tree, "left");
    let right = painted_text(&tree, "right");
    let expected = f64::from(left.bounding_box().x() - right.bounding_box().x()).abs();
    assert!(actual > 0.0);
    assert!(
        (actual - expected).abs() < 0.001,
        "measurement={actual}, paint={expected}"
    );
    let painted_faces: BTreeSet<_> = left
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| {
            let face = tree.fontdb().face(glyph.font).expect("paint font face");
            format!("{}#{}", face.post_script_name, face.index)
        })
        .collect();
    assert!(!painted_faces.is_empty());
    assert_eq!(
        painted_faces,
        context
            .selected_faces()
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    context.check().expect("paint did not latch a font failure");
}

#[test]
fn explicit_platform_entry_preserves_closed_context_and_content_identity() {
    let ordinary = options().load(&OperationControl::new()).unwrap();
    let platform = options().load_platform(&OperationControl::new()).unwrap();
    assert!(!platform.uses_system_fonts());
    assert_eq!(platform.face_count(), 1);
    assert_eq!(ordinary.identity(), platform.identity());
    assert_eq!(
        ordinary.resident_font_bytes(),
        platform.resident_font_bytes()
    );
    assert_eq!(platform, platform.clone());
    assert_same_advance_and_faces(&platform, "Cousine");
    assert_eq!(platform.selected_faces(), ["Cousine-Regular#0"]);
}

#[test]
fn explicit_platform_entry_does_not_discover_a_missing_glyph_from_the_host() {
    let context = options().load_platform(&OperationControl::new()).unwrap();
    assert!(
        context
            .measure(request("A\u{4e2d}V", &style("Cousine")))
            .unwrap()
            .is_none()
    );
    assert!(!context.uses_system_fonts());
    assert_eq!(context.face_count(), 1);
    assert!(context.selected_faces().is_empty());
    context.check().unwrap();
}

#[test]
fn explicit_platform_entry_retains_acquisition_byte_limit_and_cancellation() {
    let control = OperationControl::new();
    let result = options()
        .with_limits(NativeFontLimits {
            max_bytes: 1,
            ..Default::default()
        })
        .load_platform(&control);
    assert!(matches!(
        result,
        Err(NativeFontError::Limit {
            resource: "font-bytes",
            limit: 1,
            ..
        })
    ));
    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert!(matches!(
        options().load_platform(&cancelled),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
}

#[test]
fn platform_entry_serves_all_shared_operations_not_only_titles() {
    let context = options().load_platform(&OperationControl::new()).unwrap();
    let style = style("Cousine");
    for operation in TextMeasurementOperation::ALL {
        let request = HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::SvgBBox,
            text: "Shared diagram text",
            style: &style,
            max_width: Some(90.0),
            wrap_mode: WrapMode::HtmlLike,
        };
        let result = context
            .measure(request)
            .unwrap()
            .unwrap_or_else(|| panic!("native measurement declined {operation:?}"));
        validate_host_text_measurement(&request, &result)
            .unwrap_or_else(|error| panic!("invalid {operation:?}: {error:?}"));
    }
    context.check().unwrap();
}

#[test]
fn platform_entry_forks_operation_budget_without_inheriting_failure() {
    let first_control = OperationControl::new();
    let first = options()
        .with_limits(NativeFontLimits {
            max_measurements: 1,
            ..Default::default()
        })
        .load_platform(&first_control)
        .unwrap();
    assert!(advance(&first, "first", &style("Cousine")) > 0.0);
    assert!(first.measure(request("second", &style("Cousine"))).is_err());
    assert!(first.check().is_err());

    let next_control = OperationControl::new();
    let next = first.for_operation(&next_control);
    assert_ne!(first, next);
    assert_eq!(first.identity(), next.identity());
    assert!(next.selected_faces().is_empty());
    assert_same_advance_and_faces(&next, "Cousine");
    next_control
        .terminal_checkpoint_at(OperationPhase::Export)
        .unwrap();
    assert!(first.check().is_err());
}

#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_system_font_host_smoke_initializes_and_materializes() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes");
    assert!(context.uses_system_fonts());
    let initial_bytes = context.resident_font_bytes();
    assert_same_advance_and_faces(&context, "sans-serif");
    assert!(context.resident_font_bytes() >= initial_bytes);
    context
        .check()
        .expect("platform host smoke remains healthy");
}

#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_system_font_repeated_measurement_and_paint_keep_residency_stable() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes");
    assert_same_advance_and_faces(&context, "sans-serif");
    let resident_bytes = context.resident_font_bytes();
    let face_count = context.face_count();
    let selected_faces = context.selected_faces();
    assert!(resident_bytes > 0);
    assert!(face_count > 0);

    for _ in 0..3 {
        assert_same_advance_and_faces(&context, "sans-serif");
        assert_eq!(context.resident_font_bytes(), resident_bytes);
        assert_eq!(context.face_count(), face_count);
        assert_eq!(context.selected_faces(), selected_faces);
    }
}

#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_system_font_clones_share_failure_and_new_operations_reset_measurement_state() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_measurements: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes");
    let cloned = context.clone();
    assert_eq!(context, cloned);
    assert_same_advance_and_faces(&context, "sans-serif");
    assert_eq!(cloned.selected_faces(), context.selected_faces());

    assert!(
        context
            .measure(request("Uncached second label", &style("sans-serif")))
            .is_err()
    );
    assert!(matches!(
        cloned.check(),
        Err(NativeFontError::Limit {
            resource: "measurements",
            actual: 2,
            limit: 1
        })
    ));
    assert!(
        cloned
            .measure(request("AVA", &style("sans-serif")))
            .is_err()
    );

    let next_control = OperationControl::new();
    let next = context.for_operation(&next_control);
    assert_ne!(context, next);
    assert_eq!(context.identity(), next.identity());
    assert!(next.uses_system_fonts());
    assert!(next.selected_faces().is_empty());
    next.check()
        .expect("new operation does not inherit a failure");
    assert_same_advance_and_faces(&next, "sans-serif");
    next_control
        .terminal_checkpoint_at(OperationPhase::Export)
        .expect("new operation has an independent resource ledger");
    assert!(context.check().is_err());
    assert!(cloned.check().is_err());
}

#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_system_font_cancellation_blocks_measurement_and_residency_growth() {
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&control)
        .expect("platform font context initializes");
    let resident_bytes = context.resident_font_bytes();
    let face_count = context.face_count();
    control.cancel();

    assert!(
        context
            .measure(request("AVA", &style("sans-serif")))
            .is_err()
    );
    assert_eq!(context.resident_font_bytes(), resident_bytes);
    assert_eq!(context.face_count(), face_count);
    assert!(context.selected_faces().is_empty());
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_system_font_exports_share_snapshots_and_preserve_context_cancellation() {
    use merman_export::{
        PdfOptions, RasterOptions, svg_to_jpeg_controlled, svg_to_pdf_controlled,
        svg_to_png_controlled,
    };
    use merman_render::environment::RenderEnvironment;
    use merman_render::svg::SvgPipeline;

    let font_control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&font_control)
        .expect("platform font context initializes");
    assert!(advance(&context, "AVA", &style("sans-serif")) > 0.0);
    let resident_bytes = context.resident_font_bytes();
    let face_count = context.face_count();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let svg = SvgPipeline::resvg_safe()
        .process_resvg_compatible(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="64"><text x="10" y="40" font-family="sans-serif" font-size="22">AVA</text></svg>"#,
            &session,
        )
        .expect("platform font export SVG is sealed");
    let raster = RasterOptions::default().with_font_context(context.clone());
    let pdf = PdfOptions::default().with_font_context(context.clone());

    let outputs = [
        (
            svg_to_png_controlled(&svg, &raster, OperationControl::new())
                .expect("shared platform context exports PNG"),
            b"\x89PNG\r\n\x1a\n".as_slice(),
        ),
        (
            svg_to_jpeg_controlled(&svg, &raster, OperationControl::new())
                .expect("shared platform context exports JPEG"),
            b"\xff\xd8\xff".as_slice(),
        ),
        (
            svg_to_pdf_controlled(&svg, &pdf, OperationControl::new())
                .expect("shared platform context exports PDF"),
            b"%PDF-".as_slice(),
        ),
    ];
    for (bytes, signature) in outputs {
        assert!(bytes.starts_with(signature));
    }
    assert_eq!(context.resident_font_bytes(), resident_bytes);
    assert_eq!(context.face_count(), face_count);
    context
        .check()
        .expect("all exports leave the context healthy");

    font_control.cancel();
    for result in [
        svg_to_png_controlled(&svg, &raster, OperationControl::new()),
        svg_to_jpeg_controlled(&svg, &raster, OperationControl::new()),
        svg_to_pdf_controlled(&svg, &pdf, OperationControl::new()),
    ] {
        assert!(
            result.is_err(),
            "cancelled font context must not return output bytes"
        );
    }
    assert_eq!(context.resident_font_bytes(), resident_bytes);
    assert_eq!(context.face_count(), face_count);
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
    assert!(matches!(
        font_control.terminal_checkpoint_at(OperationPhase::Export),
        Err(OperationLedgerError::Cancelled(_))
    ));
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_system_font_exports_replay_measurement_limit_failure() {
    use merman_export::{
        ExportError, PdfOptions, RasterOptions, svg_to_jpeg_controlled, svg_to_pdf_controlled,
        svg_to_png_controlled,
    };
    use merman_render::environment::RenderEnvironment;
    use merman_render::svg::SvgPipeline;

    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_measurements: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes");
    assert!(advance(&context, "AVA", &style("sans-serif")) > 0.0);
    assert!(
        context
            .measure(request("Uncached second label", &style("sans-serif")))
            .is_err()
    );
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Limit {
            resource: "measurements",
            actual: 2,
            limit: 1
        })
    ));

    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let svg = SvgPipeline::resvg_safe()
        .process_resvg_compatible(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="64"><text x="10" y="40" font-family="sans-serif" font-size="22">AVA</text></svg>"#,
            &session,
        )
        .expect("platform font export SVG is sealed");
    let raster = RasterOptions::default().with_font_context(context.clone());
    let pdf = PdfOptions::default().with_font_context(context.clone());
    for (format, result) in [
        (
            "PNG",
            svg_to_png_controlled(&svg, &raster, OperationControl::new()),
        ),
        (
            "JPEG",
            svg_to_jpeg_controlled(&svg, &raster, OperationControl::new()),
        ),
        (
            "PDF",
            svg_to_pdf_controlled(&svg, &pdf, OperationControl::new()),
        ),
    ] {
        let error = result.expect_err("font resource failure must not return output bytes");
        assert!(
            matches!(
                &error,
                ExportError::NativeFont(NativeFontError::Limit {
                    resource: "measurements",
                    actual: 2,
                    limit: 1
                })
            ),
            "{format} must replay the native font measurement failure: {error:?}"
        );
    }
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires Windows system Arial; run explicitly on controlled system-font hosts"]
fn windows_platform_system_font_smoke_loads_on_demand_and_shares_paint_selection() {
    let context = options()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("Windows platform font context initializes");
    assert!(context.uses_system_fonts());
    assert_eq!(context.face_count(), 1);
    let initial_bytes = context.resident_font_bytes();
    assert!(initial_bytes > 0);
    assert_same_advance_and_faces(&context, "Arial");
    assert!(context.resident_font_bytes() > initial_bytes);
    assert!(
        context
            .selected_faces()
            .iter()
            .any(|face| face.starts_with("Arial")),
        "controlled Windows host must supply Arial"
    );
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires the Windows platform font catalog; run explicitly on controlled hosts"]
fn windows_platform_query_honors_weight_and_generic_family_matching() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("Windows platform font context initializes");
    assert!(context.uses_system_fonts());

    let bold = styled("Arial", "700");
    assert!(context.measure(request("AVA", &bold)).unwrap().is_some());
    let bold_faces = context.selected_faces();
    assert!(bold_faces.iter().any(|face| {
        let normalized = face.to_ascii_lowercase();
        normalized.contains("arial") && normalized.contains("bold")
    }));

    let monospace = style("monospace");
    assert!(
        context
            .measure(request("AVA", &monospace))
            .unwrap()
            .is_some()
    );
    let monospace_tree = paint(&context, "monospace");
    let text = painted_text(&monospace_tree, "left");
    let faces: Vec<_> = text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .filter_map(|glyph| monospace_tree.fontdb().face(glyph.font))
        .collect();
    assert!(!faces.is_empty());
    assert!(faces.iter().all(|face| face.monospaced));
    assert!(
        faces
            .iter()
            .all(|face| { !face.post_script_name.to_ascii_lowercase().contains("arial") })
    );
    context.check().unwrap();
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires the Windows platform font catalog; run explicitly on controlled hosts"]
fn windows_platform_query_reuses_bootstrap_face_without_duplicate_snapshot() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_files: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("platform bootstrap fits one file");
    let initial_bytes = context.resident_font_bytes();
    assert!(
        context
            .measure(request("AVA", &style("Arial")))
            .unwrap()
            .is_some()
    );
    assert_eq!(context.resident_font_bytes(), initial_bytes);
    context.check().unwrap();
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires Windows system Arial; run explicitly on controlled system-font hosts"]
fn windows_platform_selected_font_limit_is_fatal_instead_of_fallback_success() {
    let context = options()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_files: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("explicit fixture fits the acquisition budget");
    assert!(context.measure(request("AVA", &style("Arial"))).is_err());
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Limit {
            resource: "system-snapshot-files",
            actual: 2,
            limit: 1
        })
    ));
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires Windows system font discovery; run explicitly on controlled hosts"]
fn windows_platform_cancellation_after_initialization_blocks_dynamic_measurement() {
    let control = OperationControl::new();
    let context = options()
        .with_system_fonts(true)
        .load_platform(&control)
        .expect("Windows platform font context initializes");
    let initial_bytes = context.resident_font_bytes();
    control.cancel();
    assert!(context.measure(request("AVA", &style("Arial"))).is_err());
    assert_eq!(context.resident_font_bytes(), initial_bytes);
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
}
