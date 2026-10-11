#![cfg(feature = "native-fonts")]

use merman_core::{OperationControl, OperationLedgerError, OperationPhase};
use merman_export::{NativeFontContext, NativeFontError, NativeFontLimits, NativeFontOptions};
use merman_render::environment::{
    HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer, TextMeasurementOperation,
    TextMeasurementPhase,
};
use merman_render::text::{TextStyle, WrapMode};
use std::path::{Path, PathBuf};

fn font(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fonts")
        .join(name)
}

fn style(family: &str) -> TextStyle {
    TextStyle {
        font_family: Some(family.to_owned()),
        font_size: 31.0,
        font_weight: Some("400".to_owned()),
        font_style: Some("normal".to_owned()),
    }
}

fn context(files: &[&str]) -> NativeFontContext {
    files
        .iter()
        .fold(NativeFontOptions::default(), |options, file| {
            options.with_font_file(font(file))
        })
        .load(&OperationControl::new())
        .expect("explicit fixtures load")
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

fn advance(context: &NativeFontContext, label: &str, style: &TextStyle) -> f64 {
    match context
        .measure(request(label, style))
        .unwrap()
        .expect("covered label")
    {
        HostTextMeasurement::Length(width) => width,
        result => panic!("unexpected advance result: {result:?}"),
    }
}

fn probe(context: &NativeFontContext, label: &str, style: &TextStyle) -> usvg::Tree {
    let escaped = label
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2048" height="512"><g font-family="{}" font-size="{}" font-style="{}" xml:space="preserve"><text id="left" x="100" y="100" text-anchor="start">{escaped}</text><text id="right" x="100" y="100" text-anchor="end">{escaped}</text></g></svg>"#,
        style.font_family.as_deref().unwrap(),
        style.font_size,
        style.font_style.as_deref().unwrap(),
    );
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    usvg::Tree::from_str(&source, &options).expect("independent export text")
}

fn svg_advance(tree: &usvg::Tree) -> f64 {
    let (Some(usvg::Node::Text(left)), Some(usvg::Node::Text(right))) =
        (tree.node_by_id("left"), tree.node_by_id("right"))
    else {
        panic!("text probes are retained");
    };
    f64::from(left.bounding_box().x() - right.bounding_box().x()).abs()
}

#[test]
fn advance_matches_independent_export_for_kerning_nbsp_rtl_and_fallback_runs() {
    let context = context(&[
        "NotoSans-Latin-subset.ttf",
        "Cousine-Regular.ttf",
        "NotoSansSC-subset.ttf",
    ]);
    for (family, label) in [
        ("Noto Sans", "AVA"),
        ("Noto Sans", "fi"),
        ("Noto Sans", "AV\u{a0}"),
        ("Cousine", "אבגד"),
        ("Cousine", "A中V"),
        ("Cousine", "中中"),
    ] {
        let style = style(family);
        let tree = probe(&context, label, &style);
        let expected = svg_advance(&tree);
        let actual = advance(&context, label, &style);
        assert!(expected > 0.0, "nonzero reference advance: {label:?}");
        assert!(
            (actual - expected).abs() < 0.001,
            "{family}: {label:?}, native={actual}, export={expected}"
        );
    }
    assert!(
        context
            .selected_faces()
            .iter()
            .any(|face| face.contains("NotoSansSC"))
    );
    let style = style("Noto Sans");
    assert!(advance(&context, "AV\u{a0}", &style) > advance(&context, "AV", &style));
}

#[test]
fn proportional_advance_preserves_pair_kerning_instead_of_summing_glyph_widths() {
    let context = context(&["NotoSans-Latin-subset.ttf"]);
    let style = style("Noto Sans");
    let bytes = std::fs::read(font("NotoSans-Latin-subset.ttf")).unwrap();
    let face = rustybuzz::Face::from_slice(&bytes, 0).unwrap();
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str("AVA");
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(&face, &[], buffer);
    let expected = shaped
        .glyph_positions()
        .iter()
        .map(|glyph| f64::from(glyph.x_advance))
        .sum::<f64>()
        * style.font_size
        / f64::from(face.units_per_em());
    let actual = advance(&context, "AVA", &style);
    let unkerned = advance(&context, "A", &style) * 2.0 + advance(&context, "V", &style);
    assert!(
        (actual - expected).abs() < 0.001,
        "native={actual}, shaped={expected}"
    );
    assert!(
        actual + 0.5 < unkerned,
        "fixture must exercise real pair kerning"
    );
}

#[test]
fn collection_index_is_used_for_measurement_and_export_face_selection() {
    let context = context(&["Cousine-test-collection.ttc"]);
    assert_eq!(context.face_count(), 2);
    let mut style = style("Cousine");
    style.font_style = Some("italic".to_owned());
    let tree = probe(&context, "AV", &style);
    let Some(usvg::Node::Text(text)) = tree.node_by_id("left") else {
        panic!("collection text survives export shaping");
    };
    for glyph in text
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
    {
        let face = tree.fontdb().face(glyph.font).unwrap();
        assert_eq!(face.index, 1);
        assert_eq!(face.style, usvg::fontdb::Style::Italic);
    }
    assert!((advance(&context, "AV", &style) - svg_advance(&tree)).abs() < 0.001);
    assert!(
        context
            .selected_faces()
            .iter()
            .any(|face| face == "Cousine-Italic#1")
    );
    assert!(matches!(
        NativeFontOptions::default()
            .with_font_file(font("Cousine-test-collection.ttc"))
            .with_limits(NativeFontLimits {
                max_faces: 1,
                ..Default::default()
            })
            .load(&OperationControl::new()),
        Err(NativeFontError::Limit {
            resource: "font-faces",
            actual: 2,
            limit: 1
        })
    ));
}

#[test]
fn active_measurement_control_observes_cancellation_after_successful_acquisition() {
    let acquisition = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(font("Cousine-Regular.ttf"))
        .load(&acquisition)
        .unwrap();
    let active = OperationControl::new();
    active.cancel();
    assert!(
        context
            .measure_with_control(request("AV", &style("Cousine")), &active)
            .is_err()
    );
    assert!(matches!(
        active.terminal_checkpoint_at(OperationPhase::Layout),
        Err(OperationLedgerError::Cancelled(_))
    ));
    assert!(
        acquisition
            .terminal_checkpoint_at(OperationPhase::Layout)
            .is_ok()
    );
}

#[test]
fn exhausted_native_work_budget_is_latched_on_the_active_measurement_control() {
    let acquisition = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_font_file(font("Cousine-Regular.ttf"))
        .with_limits(NativeFontLimits {
            max_measurements: 0,
            ..Default::default()
        })
        .load(&acquisition)
        .unwrap();
    let active = OperationControl::new();
    assert!(
        context
            .measure_with_control(request("AV", &style("Cousine")), &active)
            .is_err()
    );
    assert!(
        matches!(active.terminal_checkpoint_at(OperationPhase::Layout),
        Err(OperationLedgerError::ResourceLimitExceeded(error))
            if error.id == "native_font_measurements" && error.limit == 0 && error.requested == 1)
    );
    assert!(
        acquisition
            .terminal_checkpoint_at(OperationPhase::Layout)
            .is_ok()
    );
}

#[test]
fn malformed_collection_directory_is_rejected_before_face_loading() {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "merman-malformed-collection-{}-{}.ttc",
        std::process::id(),
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    // The collection claims two faces, but its required offset directory is truncated.
    std::io::Write::write_all(&mut file, b"ttcf\0\x01\0\0\0\0\0\x02").unwrap();
    drop(file);
    let result = NativeFontOptions::default()
        .with_font_file(&path)
        .load(&OperationControl::new());
    std::fs::remove_file(&path).unwrap();
    assert!(matches!(
        result,
        Err(NativeFontError::Source { .. }) | Err(NativeFontError::EmptyFontSet)
    ));
}
