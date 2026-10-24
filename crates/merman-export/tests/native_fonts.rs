#![cfg(all(feature = "native-fonts", not(target_family = "wasm")))]

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

fn font_buffers() -> impl Iterator<Item = Vec<u8>> {
    [
        include_bytes!("fonts/Cousine-Regular.ttf").as_slice(),
        include_bytes!("fonts/Cousine-Bold.ttf").as_slice(),
        include_bytes!("fonts/Cousine-Italic.ttf").as_slice(),
    ]
    .into_iter()
    .map(<[u8]>::to_vec)
}

fn load_memory(limits: NativeFontLimits, control: &OperationControl) -> NativeFontContext {
    NativeFontContext::from_font_bytes(font_buffers(), limits, control)
        .expect("owned font buffers load")
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

#[test]
fn bytes_and_files_share_identity_face_count_residency_and_metrics() {
    let control = OperationControl::new();
    let file = load(Default::default(), &control);
    let memory = load_memory(Default::default(), &control);
    assert_eq!(file.identity(), memory.identity());
    assert_eq!(file.face_count(), memory.face_count());
    assert_eq!(file.resident_font_bytes(), memory.resident_font_bytes());
    assert!(!memory.uses_system_fonts());
    for (weight, face_style) in [("400", "normal"), ("700", "normal"), ("400", "italic")] {
        let style = style(weight, face_style);
        for operation in TextMeasurementOperation::ALL {
            let request = HostTextMeasurementRequest {
                operation,
                phase: TextMeasurementPhase::SvgBBox,
                text: "Owned font bytes",
                style: &style,
                max_width: Some(80.0),
                wrap_mode: WrapMode::HtmlLike,
            };
            let actual = memory.measure(request).unwrap().expect("native metrics");
            let expected = file.measure(request).unwrap().expect("file metrics");
            merman_render::environment::validate_host_text_measurement(&request, &actual).unwrap();
            match (actual, expected) {
                (HostTextMeasurement::Length(actual), HostTextMeasurement::Length(expected)) => {
                    assert_eq!(actual, expected);
                }
                (
                    HostTextMeasurement::HorizontalExtents { left, right },
                    HostTextMeasurement::HorizontalExtents {
                        left: other_left,
                        right: other_right,
                    },
                ) => {
                    assert_eq!((left, right), (other_left, other_right));
                }
                (HostTextMeasurement::Metrics(actual), HostTextMeasurement::Metrics(expected)) => {
                    assert_eq!(
                        (actual.width, actual.height, actual.line_count),
                        (expected.width, expected.height, expected.line_count)
                    );
                }
                (
                    HostTextMeasurement::WrappedWithRawWidth {
                        metrics: actual,
                        raw_width,
                    },
                    HostTextMeasurement::WrappedWithRawWidth {
                        metrics: expected,
                        raw_width: other_raw,
                    },
                ) => {
                    assert_eq!(
                        (actual.width, actual.height, actual.line_count, raw_width),
                        (
                            expected.width,
                            expected.height,
                            expected.line_count,
                            other_raw
                        )
                    );
                }
                _ => panic!("mismatched native result variants for {operation:?}"),
            }
        }
    }
    assert_eq!(file.selected_faces(), memory.selected_faces());
}

#[test]
fn memory_duplicates_share_faces_but_charge_all_input_sources_and_bytes() {
    let bytes = include_bytes!("fonts/Cousine-Regular.ttf").to_vec();
    let control = OperationControl::new();
    let limits = NativeFontLimits {
        max_files: 2,
        max_bytes: bytes.len() * 2,
        max_faces: 1,
        ..Default::default()
    };
    let memory = NativeFontContext::from_font_bytes(
        [bytes.clone(), bytes.clone()],
        limits.clone(),
        &control,
    )
    .unwrap();
    let files = NativeFontOptions::default()
        .with_font_file(font("Cousine-Regular.ttf"))
        .with_font_file(font("Cousine-Regular.ttf"))
        .with_limits(limits.clone())
        .load(&control)
        .unwrap();
    let single =
        NativeFontContext::from_font_bytes([bytes.clone()], limits.clone(), &control).unwrap();
    assert_eq!(memory.face_count(), 1);
    assert_eq!(memory.resident_font_bytes(), bytes.len());
    assert_eq!(memory.identity(), single.identity());
    assert_eq!(memory.identity(), files.identity());
    for (limits, expected) in [
        (
            NativeFontLimits {
                max_files: 1,
                ..limits.clone()
            },
            "files",
        ),
        (
            NativeFontLimits {
                max_bytes: bytes.len(),
                ..limits
            },
            "font-bytes",
        ),
    ] {
        let from_bytes = NativeFontContext::from_font_bytes(
            [bytes.clone(), bytes.clone()],
            limits.clone(),
            &control,
        );
        let from_files = NativeFontOptions::default()
            .with_font_file(font("Cousine-Regular.ttf"))
            .with_font_file(font("Cousine-Regular.ttf"))
            .with_limits(limits)
            .load(&control);
        for result in [from_bytes, from_files] {
            assert!(matches!(result,
                Err(NativeFontError::Limit { resource, .. }) if resource == expected));
        }
    }
}

#[test]
fn malformed_empty_and_over_limit_memory_fonts_return_structured_errors() {
    let control = OperationControl::new();
    assert!(matches!(
        NativeFontContext::from_font_bytes([], Default::default(), &control),
        Err(NativeFontError::EmptyFontSet)
    ));
    for bytes in [Vec::new(), b"not an OpenType font".to_vec()] {
        let temporary = TestDirectory::new();
        let path = temporary.0.join("malformed.ttf");
        std::fs::write(&path, &bytes).unwrap();
        let memory = NativeFontContext::from_font_bytes([bytes], Default::default(), &control);
        let file = NativeFontOptions::default()
            .with_font_file(path)
            .load(&control);
        let Err(NativeFontError::Source { path, message }) = memory else {
            panic!("malformed memory font must report its source");
        };
        assert_eq!(path, Path::new("<memory-font-1>"));
        assert!(
            matches!(file, Err(NativeFontError::Source { message: other, .. }) if other == message)
        );
    }
    let bytes = include_bytes!("fonts/Cousine-Regular.ttf").to_vec();
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
                max_bytes: bytes.len() - 1,
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
        assert!(matches!(
            NativeFontContext::from_font_bytes([bytes.clone()], limits, &control),
            Err(NativeFontError::Limit { resource: actual, .. }) if actual == resource
        ));
    }
    // A collection's advertised cardinality is bounded before fontdb parses its faces.
    let mut collection = b"ttcf\x00\x01\x00\x00".to_vec();
    collection.extend_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        NativeFontContext::from_font_bytes([collection], Default::default(), &control),
        Err(NativeFontError::Limit {
            resource: "font-faces",
            ..
        })
    ));
    assert!(
        measure(
            &load_memory(Default::default(), &control),
            "recovered",
            &style("400", "normal")
        )
        .unwrap()
        .is_some()
    );
}

#[test]
fn missing_required_font_tables_are_rejected_for_memory_and_files() {
    let mut bytes = include_bytes!("../assets/fonts/Cousine-Regular.ttf").to_vec();
    rustybuzz::ttf_parser::Face::parse(&bytes, 0).expect("production Cousine is valid");
    let table_count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
    let record = bytes[12..12 + table_count * 16]
        .chunks_exact_mut(16)
        .find(|record| &record[..4] == b"hhea")
        .expect("production Cousine has the required hhea table");
    record[..4].copy_from_slice(b"bad!");
    assert!(rustybuzz::ttf_parser::Face::parse(&bytes, 0).is_err());

    let temporary = TestDirectory::new();
    let path = temporary.0.join("missing-hhea.ttf");
    std::fs::write(&path, &bytes).unwrap();
    let control = OperationControl::new();
    let memory = NativeFontContext::from_font_bytes([bytes.clone()], Default::default(), &control);
    let file = NativeFontOptions::default()
        .with_font_file(&path)
        .load(&control);
    for (result, expected_path) in [(memory, PathBuf::from("<memory-font-1>")), (file, path)] {
        let Err(NativeFontError::Source { path, message }) = result else {
            panic!("a font missing hhea must be rejected at admission");
        };
        assert_eq!(path, expected_path);
        assert!(
            message.contains("invalid OpenType font face 0"),
            "{message}"
        );
    }
    assert!(matches!(
        NativeFontContext::from_font_bytes(
            [bytes],
            NativeFontLimits {
                max_faces: 0,
                ..Default::default()
            },
            &control,
        ),
        Err(NativeFontError::Limit {
            resource: "font-faces",
            ..
        })
    ));
}

#[test]
fn memory_admission_observes_cancellation_before_and_between_iterator_items() {
    let control = OperationControl::new();
    control.cancel();
    let unread = std::iter::once_with(|| -> Vec<u8> {
        panic!("cancelled admission must not request a buffer")
    });
    assert!(matches!(
        NativeFontContext::from_font_bytes(unread, Default::default(), &control),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
    let control = OperationControl::new();
    let buffers = font_buffers().inspect(|_| control.cancel());
    assert!(matches!(
        NativeFontContext::from_font_bytes(buffers, Default::default(), &control),
        Err(NativeFontError::Operation(OperationLedgerError::Cancelled(
            _
        )))
    ));
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

#[test]
fn symbolic_unicode_fixture_covers_ascii_without_using_latin_glyphs() {
    for (fixture, symbolic, pictorial) in [
        ("MermanSymbolicTest.ttf", true, false),
        ("MermanPictorialTest.ttf", false, true),
    ] {
        let bytes = std::fs::read(font(fixture)).unwrap();
        let face = rustybuzz::ttf_parser::Face::parse(&bytes, 0).unwrap();
        assert!((b'a'..=b'z').all(|c| face.glyph_index(char::from(c)).is_some()));
        assert_eq!(
            face.glyph_name(face.glyph_index('a').unwrap()),
            Some("alpha")
        );
        assert_eq!(face.glyph_name(face.glyph_index('m').unwrap()), Some("mu"));
        assert_eq!(
            face.glyph_name(face.glyph_index('A').unwrap()),
            Some("Alpha")
        );
        let os2 = face
            .raw_face()
            .table(rustybuzz::ttf_parser::Tag::from_bytes(b"OS/2"))
            .unwrap();
        assert_eq!(os2[30] == 12, symbolic);
        assert_eq!(os2[32] == 5, pictorial);
    }
}

fn painted_families(context: &NativeFontContext, family: &str, label: &str) -> BTreeSet<String> {
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="300" height="80"><text x="0" y="40" font-family="{family}">{label}</text></svg>"#,
    );
    let tree = usvg::Tree::from_str(&source, &options).unwrap();
    text(tree.root())
        .into_iter()
        .flat_map(|text| text.layouted())
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| {
            tree.fontdb()
                .face(glyph.font)
                .unwrap()
                .post_script_name
                .clone()
        })
        .collect()
}

#[test]
fn implicit_fonts_reject_symbolic_and_pictorial_faces_in_either_source_order() {
    for fixture in ["MermanSymbolicTest.ttf", "MermanPictorialTest.ttf"] {
        for symbol_first in [false, true] {
            let paths = if symbol_first {
                [font(fixture), font("Cousine-Regular.ttf")]
            } else {
                [font("Cousine-Regular.ttf"), font(fixture)]
            };
            let context = NativeFontOptions::default()
                .with_font_file(&paths[0])
                .with_font_file(&paths[1])
                .load(&OperationControl::new())
                .unwrap();
            for family in [
                "serif",
                "sans-serif",
                "monospace",
                "cursive",
                "fantasy",
                "__missing__",
                "Helvetica,Verdana,serif",
                "",
            ] {
                assert_eq!(
                    painted_families(&context, family, "amA"),
                    BTreeSet::from(["Cousine-Regular".to_owned()]),
                    "{fixture}, symbol_first={symbol_first}, family={family}"
                );
            }
        }
    }
}

#[test]
fn glyph_fallback_rejects_symbolic_and_pictorial_faces() {
    for fixture in ["MermanSymbolicTest.ttf", "MermanPictorialTest.ttf"] {
        for include_text_fallback in [false, true] {
            let mut fonts = NativeFontOptions::default()
                .with_font_file(font("NotoSans-Latin-subset.ttf"))
                .with_font_file(font(fixture));
            if include_text_fallback {
                fonts = fonts.with_font_file(font("Cousine-Regular.ttf"));
            }
            let context = fonts.load(&OperationControl::new()).unwrap();
            for family in ["Noto Sans", "sans-serif"] {
                let expected = if include_text_fallback {
                    "Cousine-Regular"
                } else {
                    "NotoSans-Regular"
                };
                assert_eq!(
                    painted_families(&context, family, "Aam"),
                    BTreeSet::from([expected.to_owned()]),
                    "{fixture}, include_text_fallback={include_text_fallback}, family={family}"
                );
                let mut text_style = style("400", "normal");
                text_style.font_family = Some(family.to_owned());
                assert_eq!(
                    measure(&context, "Aam", &text_style).unwrap().is_some(),
                    include_text_fallback
                );
            }
            context.check().unwrap();
        }
    }
}

#[test]
fn named_symbolic_fonts_remain_available_without_an_implicit_text_face() {
    for (fixture, family) in [
        ("MermanSymbolicTest.ttf", "Merman Symbolic Test"),
        ("MermanPictorialTest.ttf", "Merman Pictorial Test"),
    ] {
        let context = NativeFontOptions::default()
            .with_font_file(font(fixture))
            .load(&OperationControl::new())
            .unwrap();
        assert_eq!(
            painted_families(&context, family, "amA"),
            BTreeSet::from([family.replace(' ', "")])
        );
        for implicit in [
            "serif",
            "sans-serif",
            "monospace",
            "cursive",
            "fantasy",
            "__missing__",
            "",
        ] {
            assert!(
                painted_families(&context, implicit, "amA").is_empty(),
                "{family} must remain unresolved for {implicit}"
            );
        }
    }
}

#[test]
fn implicit_fonts_prefer_compatible_serif_sans_and_monospace_faces() {
    let serif = font("DejaVuSerif-NativeFilter.ttf");
    for serif_first in [false, true] {
        let sans = font("NotoSans-Latin-subset.ttf");
        let ordered = if serif_first {
            [serif.clone(), sans]
        } else {
            [sans, serif.clone()]
        };
        let context = NativeFontOptions::default()
            .with_font_file(font("MermanSymbolicTest.ttf"))
            .with_font_file(&ordered[0])
            .with_font_file(font("Cousine-Regular.ttf"))
            .with_font_file(&ordered[1])
            .load(&OperationControl::new())
            .unwrap();
        // V is genuinely covered by all three text fixtures, so this checks painting
        // without invoking the independent glyph-level fallback policy.
        for (family, expected) in [
            ("serif", "DejaVuSerif"),
            ("sans-serif", "NotoSans-Regular"),
            ("monospace", "Cousine-Regular"),
        ] {
            assert_eq!(
                painted_families(&context, family, "V"),
                BTreeSet::from([expected.to_owned()]),
                "{family}, serif_first={serif_first}"
            );
        }
    }
}

#[test]
fn strict_coverage_errors_instead_of_declining_and_operation_forks_recover() {
    let source = load_memory(Default::default(), &OperationControl::new());
    let strict = source.clone().with_strict_glyph_coverage(true);
    let style = style("400", "normal");
    assert_ne!(source.identity(), strict.identity());
    assert_eq!(
        source.identity(),
        strict.clone().with_strict_glyph_coverage(false).identity()
    );
    for operation in TextMeasurementOperation::ALL {
        let operation_context = strict.for_operation(&OperationControl::new());
        assert!(
            operation_context
                .measure(HostTextMeasurementRequest {
                    operation,
                    phase: TextMeasurementPhase::SvgBBox,
                    text: "\u{10ffff}",
                    style: &style,
                    max_width: Some(80.0),
                    wrap_mode: WrapMode::SvgLike,
                })
                .is_err(),
            "strict {operation:?} must not decline"
        );
        assert!(matches!(
            operation_context.check(),
            Err(NativeFontError::MissingGlyphCoverage)
        ));
        assert!(matches!(
            operation_context.for_export(&OperationControl::new()),
            Err(NativeFontError::MissingGlyphCoverage)
        ));
        let recovered = operation_context.for_operation(&OperationControl::new());
        assert!(measure(&recovered, "Recovered", &style).unwrap().is_some());
    }
    let export = strict.for_export(&OperationControl::new()).unwrap();
    assert!(measure(&export, "\u{10ffff}", &style).is_err());
    strict.check().unwrap();
    source.check().unwrap();
    assert!(measure(&source, "\u{10ffff}", &style).unwrap().is_none());
    let next_export = strict.for_export(&OperationControl::new()).unwrap();
    assert!(
        measure(&next_export, "Recovered", &style)
            .unwrap()
            .is_some()
    );
    assert!(measure(&next_export, "\u{10ffff}", &style).is_err());
}

#[test]
fn memory_operation_and_export_forks_isolate_budgets_caches_and_cancellation() {
    let source = load_memory(
        NativeFontLimits {
            max_measurements: 1,
            ..Default::default()
        },
        &OperationControl::new(),
    );
    let first_control = OperationControl::new();
    let first = source.for_operation(&first_control);
    let second = source.for_operation(&OperationControl::new());
    let export = first.for_export(&OperationControl::new()).unwrap();
    let style = style("400", "normal");
    assert!(measure(&first, "first", &style).unwrap().is_some());
    assert!(measure(&first, "exhausted", &style).is_err());
    assert!(measure(&second, "second", &style).unwrap().is_some());
    assert!(measure(&export, "export", &style).unwrap().is_some());
    first_control.cancel();
    source.check().unwrap();
    second.check().unwrap();
    export.check().unwrap();
    assert!(source.selected_faces().is_empty());
    for context in [&first, &second, &export] {
        assert_eq!(source.identity(), context.identity());
        assert_eq!(source.resident_font_bytes(), context.resident_font_bytes());
    }
}

#[cfg(feature = "bundled-fonts")]
#[test]
fn bundled_fonts_are_opt_in_owned_buffers_with_caller_precedence() {
    let bundled: Vec<_> = merman_export::bundled_font_data().collect();
    assert_eq!(bundled.len(), 3);
    let source = NativeFontContext::from_font_bytes(
        bundled.clone(),
        Default::default(),
        &OperationControl::new(),
    )
    .unwrap();
    assert_eq!(source.face_count(), 3);
    assert!(!source.uses_system_fonts());
    let mut caller = bundled[0].clone();
    caller.extend_from_slice(b"caller priority");
    let caller_first = NativeFontContext::from_font_bytes(
        std::iter::once(caller.clone()).chain(merman_export::bundled_font_data()),
        Default::default(),
        &OperationControl::new(),
    )
    .unwrap();
    let expected = NativeFontContext::from_font_bytes(
        [caller, bundled[1].clone(), bundled[2].clone()],
        Default::default(),
        &OperationControl::new(),
    )
    .unwrap();
    assert_eq!(caller_first.face_count(), 3);
    assert_eq!(caller_first.identity(), expected.identity());
    assert_ne!(caller_first.identity(), source.identity());
    for (weight, face_style) in [("400", "normal"), ("700", "normal"), ("400", "italic")] {
        assert!(
            measure(&caller_first, "Bundled fonts", &style(weight, face_style))
                .unwrap()
                .is_some()
        );
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_svg(label: &str) -> merman_render::svg::ResvgCompatibleSvg {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="80" viewBox="0 0 400 80"><text x="8" y="32" font-family="Cousine" font-size="22">{label}</text></svg>"#
    );
    merman_render::svg::finalize_resvg_svg(&source, &session).unwrap()
}

#[cfg(feature = "png")]
fn png_pixels(bytes: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

#[cfg(feature = "png")]
#[test]
fn bytes_and_files_paint_identical_nonempty_png_pixels() {
    let control = OperationControl::new();
    let svg = export_svg(
        "Owned regular <tspan font-weight=\"700\">bold</tspan> <tspan font-style=\"italic\">italic</tspan>",
    );
    let render = |context| {
        merman_export::svg_to_png(
            &svg,
            &merman_export::RasterOptions::default().with_font_context(context),
        )
        .unwrap()
    };
    let file = render(load(Default::default(), &control));
    let memory = render(load_memory(Default::default(), &control));
    let file_pixels = png_pixels(file);
    let memory_pixels = png_pixels(memory);
    assert_eq!(file_pixels, memory_pixels);
    assert_eq!((memory_pixels.0, memory_pixels.1), (400, 80));
    assert!(
        memory_pixels.2.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "the PNG must paint actual font glyphs, not an empty canvas"
    );
}

#[cfg(feature = "jpeg")]
#[test]
fn bytes_and_files_encode_identical_jpeg() {
    let control = OperationControl::new();
    let svg = export_svg("Owned font bytes");
    let encode = |context| {
        merman_export::svg_to_jpeg(
            &svg,
            &merman_export::RasterOptions::default().with_font_context(context),
        )
        .unwrap()
    };
    let file = encode(load(Default::default(), &control));
    let memory = encode(load_memory(Default::default(), &control));
    assert_eq!(file, memory);
    assert!(memory.starts_with(&[0xff, 0xd8, 0xff]));
}

#[cfg(feature = "pdf")]
#[test]
fn bytes_and_files_encode_identical_pdf() {
    let control = OperationControl::new();
    let svg = export_svg("Owned font bytes");
    let encode = |context| {
        merman_export::svg_to_pdf_with_options(
            &svg,
            &merman_export::PdfOptions::default().with_font_context(context),
        )
        .unwrap()
    };
    let file = encode(load(Default::default(), &control));
    let memory = encode(load_memory(Default::default(), &control));
    assert_eq!(file, memory);
    assert!(memory.starts_with(b"%PDF"));
}
