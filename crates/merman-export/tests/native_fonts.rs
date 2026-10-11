#![cfg(feature = "native-fonts")]

use merman_core::{OperationControl, OperationLedgerError, OperationPhase};
use merman_export::{NativeFontContext, NativeFontError, NativeFontLimits, NativeFontOptions};
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    TextMeasurementOperation, TextMeasurementPhase,
};
use merman_render::text::{TextStyle, WrapMode};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn font(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fonts")
        .join(name)
}

fn load(limits: NativeFontLimits, control: &OperationControl) -> NativeFontContext {
    NativeFontOptions::default()
        .with_font_file(font("Cousine-Regular.ttf"))
        .with_font_file(font("Cousine-Bold.ttf"))
        .with_font_file(font("Cousine-Italic.ttf"))
        .with_limits(limits)
        .load(control)
        .expect("controlled font fixtures load")
}

fn style(weight: &str, face_style: &str) -> TextStyle {
    TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 22.0,
        font_weight: Some(weight.to_owned()),
        font_style: Some(face_style.to_owned()),
    }
}

fn measure(context: &NativeFontContext, label: &str, style: &TextStyle) -> HostMeasurementResult {
    context.measure(HostTextMeasurementRequest {
        operation: TextMeasurementOperation::TitleBBoxX,
        phase: TextMeasurementPhase::SvgBBox,
        text: label,
        style,
        max_width: None,
        wrap_mode: WrapMode::SvgLike,
    })
}

fn text(group: &usvg::Group) -> Option<&usvg::Text> {
    group.children().iter().find_map(|node| match node {
        usvg::Node::Text(text) => Some(text.as_ref()),
        usvg::Node::Group(group) => text(group),
        _ => None,
    })
}

fn probe(context: &NativeFontContext, label: &str, style: &TextStyle) -> usvg::Tree {
    let escaped = label
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2048" height="512"><g><text x="0" y="0" text-anchor="middle" xml:space="preserve" font-family="Cousine" font-size="22" font-weight="{}" font-style="{}">{}</text></g></svg>"#,
        style.font_weight.as_deref().expect("test weight"),
        style.font_style.as_deref().expect("test style"),
        escaped,
    );
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    usvg::Tree::from_str(&source, &options).expect("independent SVG probe parses")
}

#[test]
fn explicit_file_and_directory_order_produce_stable_content_identities() {
    let directory = TestDirectory::new();
    for name in [
        "Cousine-Regular.ttf",
        "Cousine-Bold.ttf",
        "Cousine-Italic.ttf",
    ] {
        std::fs::copy(font(name), directory.0.join(name)).unwrap();
    }
    let root = &directory.0;
    let first = NativeFontOptions::default()
        .with_font_dir(root)
        .load(&OperationControl::new())
        .unwrap();
    let second = NativeFontOptions::default()
        .with_font_dir(root)
        .load(&OperationControl::new())
        .unwrap();
    let sorted_files = NativeFontOptions::default()
        .with_font_file(font("Cousine-Bold.ttf"))
        .with_font_file(font("Cousine-Italic.ttf"))
        .with_font_file(font("Cousine-Regular.ttf"))
        .load(&OperationControl::new())
        .unwrap();
    assert_eq!(first.face_count(), 3);
    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.identity(), sorted_files.identity());
    assert_eq!(first, first.clone());
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "merman-font-test-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create test directory: {error}"),
            }
        }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn earlier_equal_face_descriptor_wins_even_when_source_bytes_differ() {
    let temporary = TestDirectory::new();
    let alternate = temporary.0.join("alternate.ttf");
    let mut bytes = std::fs::read(font("Cousine-Regular.ttf")).unwrap();
    bytes.extend_from_slice(b"non-table trailing bytes");
    std::fs::write(&alternate, bytes).unwrap();
    let original = NativeFontOptions::default()
        .with_font_file(font("Cousine-Regular.ttf"))
        .load(&OperationControl::new())
        .unwrap();
    let original_first = NativeFontOptions::default()
        .with_font_file(font("Cousine-Regular.ttf"))
        .with_font_file(&alternate)
        .load(&OperationControl::new())
        .unwrap();
    let alternate_first = NativeFontOptions::default()
        .with_font_file(&alternate)
        .with_font_file(font("Cousine-Regular.ttf"))
        .load(&OperationControl::new())
        .unwrap();
    assert_eq!(original_first.face_count(), 1);
    assert_eq!(alternate_first.face_count(), 1);
    assert_eq!(original.identity(), original_first.identity());
    assert_ne!(original.identity(), alternate_first.identity());
}

#[test]
fn acquisition_enforces_files_bytes_faces_and_directory_entries() {
    for (limits, resource) in [
        (
            NativeFontLimits {
                max_files: 0,
                ..Default::default()
            },
            "files",
        ),
        (
            NativeFontLimits {
                max_bytes: 1,
                ..Default::default()
            },
            "font-bytes",
        ),
        (
            NativeFontLimits {
                max_faces: 0,
                ..Default::default()
            },
            "font-faces",
        ),
    ] {
        let result = NativeFontOptions::default()
            .with_font_file(font("Cousine-Regular.ttf"))
            .with_limits(limits)
            .load(&OperationControl::new());
        assert!(
            matches!(result, Err(NativeFontError::Limit { resource: actual, .. }) if actual == resource)
        );
    }
    let result = NativeFontOptions::default()
        .with_font_dir(font("Cousine-Regular.ttf").parent().unwrap())
        .with_limits(NativeFontLimits {
            max_directory_entries: 0,
            ..Default::default()
        })
        .load(&OperationControl::new());
    assert!(matches!(
        result,
        Err(NativeFontError::Limit {
            resource: "directory-entries",
            ..
        })
    ));
    let temporary = TestDirectory::new();
    std::fs::create_dir(temporary.0.join("nested")).unwrap();
    let result = NativeFontOptions::default()
        .with_font_dir(&temporary.0)
        .with_limits(NativeFontLimits {
            max_directory_depth: 0,
            ..Default::default()
        })
        .load(&OperationControl::new());
    assert!(matches!(
        result,
        Err(NativeFontError::Limit {
            resource: "directory-depth",
            ..
        })
    ));
}

#[test]
fn cancellation_is_observed_before_font_reads_and_before_native_measurement() {
    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert!(matches!(
        NativeFontOptions::default()
            .with_font_file("missing.ttf")
            .load(&cancelled),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
    let control = OperationControl::new();
    let context = load(Default::default(), &control);
    control.cancel();
    assert!(measure(&context, "title", &style("400", "normal")).is_err());
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
}

#[test]
fn title_extents_contain_independently_parsed_metric_and_ink_bounds() {
    let context = load(Default::default(), &OperationControl::new());
    for (label, weight, face_style) in [
        ("Fetch machine details", "400", "normal"),
        ("Fetch machine details", "700", "normal"),
        ("ffffj", "400", "italic"),
        ("Cafe\u{301}", "400", "normal"),
        ("\u{5e9}\u{5dc}\u{5d5}\u{5dd}", "400", "normal"),
        ("left & right < title >", "400", "normal"),
    ] {
        let style = style(weight, face_style);
        let Some(HostTextMeasurement::HorizontalExtents { left, right }) =
            measure(&context, label, &style).unwrap()
        else {
            panic!("supported fixture title must use native extents: {label:?}");
        };
        let tree = probe(&context, label, &style);
        let text = text(tree.root()).expect("probe contains laid-out text");
        for bound in [text.bounding_box(), text.flattened().bounding_box()] {
            assert!(
                -left <= f64::from(bound.x()) + 0.0001,
                "left bound for {label:?}"
            );
            assert!(
                right + 0.0001 >= f64::from(bound.right()),
                "right bound for {label:?}"
            );
        }
    }
}

#[test]
fn shared_export_selection_honors_bold_and_italic_faces() {
    let context = load(Default::default(), &OperationControl::new());
    for (weight, face_style, expected) in [
        ("400", "normal", "Cousine-Regular"),
        ("700", "normal", "Cousine-Bold"),
        ("400", "italic", "Cousine-Italic"),
    ] {
        let tree = probe(&context, "Font selection", &style(weight, face_style));
        let selected: BTreeSet<_> = text(tree.root())
            .unwrap()
            .layouted()
            .iter()
            .flat_map(|span| &span.positioned_glyphs)
            .map(|glyph| {
                tree.fontdb()
                    .face(glyph.font)
                    .unwrap()
                    .post_script_name
                    .as_str()
            })
            .collect();
        assert_eq!(selected, BTreeSet::from([expected]));
        assert!(
            measure(&context, "Font selection", &style(weight, face_style))
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(
        context.selected_title_faces(),
        ["Cousine-Bold#0", "Cousine-Italic#0", "Cousine-Regular#0"]
    );
}

#[test]
fn missing_glyph_declines_and_all_shared_operations_use_native_metrics() {
    let context = load(Default::default(), &OperationControl::new());
    let style = style("400", "normal");
    {
        let label = "\u{10ffff}";
        assert!(
            measure(&context, label, &style).unwrap().is_none(),
            "{label:?}"
        );
    }
    for operation in TextMeasurementOperation::ALL {
        let request = HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::SvgBBox,
            text: "Font measurement",
            style: &style,
            max_width: Some(20.0),
            wrap_mode: WrapMode::HtmlLike,
        };
        let result = context.measure(request).unwrap().expect("native operation");
        merman_render::environment::validate_host_text_measurement(&request, &result)
            .unwrap_or_else(|error| panic!("invalid {operation:?}: {error:?}"));
    }
    context.check().unwrap();
}

#[test]
fn a_full_cache_does_not_grow_and_uncached_work_remains_bounded() {
    let control = OperationControl::new();
    let context = load(
        NativeFontLimits {
            max_cache_entries: 1,
            max_measurements: 2,
            ..Default::default()
        },
        &control,
    );
    let style = style("400", "normal");
    assert!(measure(&context, "cached", &style).unwrap().is_some());
    for _ in 0..10 {
        assert!(measure(&context, "cached", &style).unwrap().is_some());
    }
    assert!(measure(&context, "not cached", &style).unwrap().is_some());
    assert!(measure(&context, "not cached", &style).is_err());
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Limit {
            resource: "measurements",
            actual: 3,
            limit: 2
        })
    ));
    assert!(
        matches!(control.terminal_checkpoint_at(OperationPhase::Emit),
        Err(OperationLedgerError::ResourceLimitExceeded(error))
        if error.id == "native_font_measurements" && error.requested == 3 && error.limit == 2)
    );
    assert!(measure(&context, "cached", &style).is_err());
}

#[test]
fn zero_cache_bytes_cannot_retain_a_successful_title_probe() {
    let control = OperationControl::new();
    let context = load(
        NativeFontLimits {
            max_cache_bytes: 0,
            max_measurements: 1,
            ..Default::default()
        },
        &control,
    );
    let style = style("400", "normal");
    assert!(measure(&context, "repeat", &style).unwrap().is_some());
    assert!(measure(&context, "repeat", &style).is_err());
    assert!(context.check().is_err());
}

#[test]
fn oversized_labels_fail_before_svg_parsing_and_latch_the_operation_ledger() {
    let control = OperationControl::new();
    let context = load(
        NativeFontLimits {
            max_label_bytes: 16,
            ..Default::default()
        },
        &control,
    );
    assert!(measure(&context, &"x".repeat(32), &style("400", "normal")).is_err());
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Limit {
            resource: "label-bytes",
            ..
        })
    ));
    assert!(
        matches!(control.terminal_checkpoint_at(OperationPhase::Export),
        Err(OperationLedgerError::ResourceLimitExceeded(error)) if error.id == "native_font_label_bytes")
    );
}
