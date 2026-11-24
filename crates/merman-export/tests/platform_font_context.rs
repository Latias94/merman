#![cfg(feature = "experimental-system-font-query")]

use merman_export::{NativeFontContext, NativeFontError, NativeFontLimits, NativeFontOptions};
use merman_model::{OperationControl, OperationLedgerError, OperationPhase};
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
    paint_text(context, family, "AVA")
}

fn paint_text(context: &NativeFontContext, family: &str, text: &str) -> usvg::Tree {
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="100"><g font-family="{family}" font-size="22"><text id="left" x="200" y="50" text-anchor="start">{text}</text><text id="right" x="200" y="50" text-anchor="end">{text}</text></g></svg>"#,
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

fn painted_faces<'a>(tree: &'a usvg::Tree, id: &str) -> Vec<&'a str> {
    painted_text(tree, id)
        .layouted()
        .iter()
        .flat_map(|span| &span.positioned_glyphs)
        .map(|glyph| {
            assert_ne!(glyph.id.0, 0);
            tree.fontdb()
                .face(glyph.font)
                .unwrap()
                .post_script_name
                .as_str()
        })
        .collect()
}

fn assert_same_advance_and_faces(context: &NativeFontContext, family: &str) {
    assert_text_advance_and_faces(context, family, "AVA");
}

fn assert_text_advance_and_faces(context: &NativeFontContext, family: &str, text: &str) {
    let actual = advance(context, text, &style(family));
    let tree = paint_text(context, family, text);
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
            assert_ne!(glyph.id.0, 0, "missing painted glyph for {text:?}");
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
    assert_eq!(context.resident_font_bytes(), 0);
    assert_eq!(context.face_count(), 0);
    assert_text_advance_and_faces(&context, "", "AVA");
    assert_same_advance_and_faces(&context, "sans-serif");
    assert!(context.resident_font_bytes() > 0);
    context
        .check()
        .expect("platform host smoke remains healthy");
}

#[test]
#[ignore = "requires a host-managed font catalog; run explicitly in CI"]
fn platform_fallback_prefers_authored_faces_without_replacing_the_primary_ligature() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
    let context = NativeFontOptions::default()
        .with_font_file(root.join("MermanProbeBase.ttf"))
        .with_font_file(root.join("MermanProbeFull.ttf"))
        .with_font_file(root.join("MermanProbePartial.ttf"))
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .unwrap();
    let family = "MermanProbeBase, MermanProbePartial, MermanProbeFull";
    assert_text_advance_and_faces(&context, family, "fi✓中");
    let tree = paint_text(&context, family, "fi✓中");
    assert_eq!(
        painted_faces(&tree, "left"),
        ["MermanProbeBase", "MermanProbePartial", "MermanProbeFull"]
    );
}

#[test]
fn fallback_scale_reaches_metrics_outlines_and_pdf_spans() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
    let context = NativeFontOptions::default()
        .with_font_file(root.join("MermanProbeBase.ttf"))
        .with_font_file(root.join("MermanProbeFull.ttf"))
        .load(&OperationControl::new())
        .unwrap();
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let fallback = options
        .fontdb
        .faces()
        .find(|face| face.post_script_name == "MermanProbeFull")
        .unwrap()
        .id;
    options.set_text_fallback(
        None,
        Some(Box::new(move |_, text, _, _, _, _| {
            assert_eq!(text, "中");
            Some((fallback, 1.5))
        })),
    );
    let tree = usvg::Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60">
            <text id="probe" x="5" y="35" font-family="MermanProbeBase" font-size="20" text-decoration="underline">fi中</text>
        </svg>"#,
        &options,
    ).unwrap();
    let text = painted_text(&tree, "probe");
    assert_eq!(
        painted_faces(&tree, "probe"),
        ["MermanProbeBase", "MermanProbeFull"]
    );
    let spans = text.layouted();
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].font_size.get(), 20.0);
    assert_eq!(spans[1].font_size.get(), 30.0);
    assert_eq!(spans[1].positioned_glyphs[0].font_size(), 30.0);
    assert!(spans[0].underline.is_some() && spans[1].underline.is_none());
    assert_eq!(text.bounding_box().width(), 25.0);
}

#[cfg(feature = "pdf")]
mod variable_pdf {
    use super::painted_text;
    use krilla::SerializeSettings;
    use krilla::geom::Size;
    use krilla_svg::{SurfaceExt, SvgSettings};

    const FIXTURE: &[u8] = include_bytes!("fonts/MermanVariableWeight.ttf");
    const INSTANCES: [(&str, f32); 2] = [("en", 700.0), ("fr", 900.0)];
    const WEIGHT: [u8; 4] = *b"wght";

    fn export_instances() -> Vec<u8> {
        let mut options = usvg::Options::default();
        options.fontdb_mut().load_font_data(FIXTURE.to_vec());
        let font_id = options.fontdb.faces().next().expect("fixture face").id;
        options.set_font_instances(
            Some(Box::new(move |font, _| {
                let language = font.selection_language()?;
                let (_, weight) = INSTANCES.iter().find(|(tag, _)| *tag == language)?;
                Some((
                    font_id,
                    1.0,
                    vec![usvg::FontVariation::new(WEIGHT, *weight)],
                ))
            })),
            None,
        );
        let tree = usvg::Tree::from_str(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="100">
                <g font-family="Merman Variable Weight" font-size="32">
                    <text id="en" xml:lang="en" x="10" y="35">AV</text>
                    <text id="fr" xml:lang="fr" x="10" y="75">AV</text>
                </g>
            </svg>"#,
            &options,
        )
        .expect("shape both variable instances");
        let fixture = ttf_parser::Face::parse(FIXTURE, 0).expect("fixture face");
        let expected_glyphs = ['A', 'V'].map(|ch| (font_id, fixture.glyph_index(ch).unwrap().0));
        for (id, weight) in INSTANCES {
            let [span] = painted_text(&tree, id).layouted() else {
                panic!("expected one span for {id}");
            };
            let glyphs: Vec<_> = span
                .positioned_glyphs
                .iter()
                .map(|glyph| (glyph.font, glyph.id.0))
                .collect();
            assert_eq!(glyphs, expected_glyphs);
            assert_eq!(span.variations, [usvg::FontVariation::new(WEIGHT, weight)]);
        }

        let size = Size::from_wh(tree.size().width(), tree.size().height()).unwrap();
        let mut document = krilla::Document::new_with(SerializeSettings {
            ascii_compatible: true,
            ..SerializeSettings::default()
        });
        let mut page = document.start_page_with(krilla::page::PageSettings::new(size));
        let mut surface = page.surface();
        surface
            .draw_svg(&tree, size, SvgSettings::default())
            .expect("draw SVG with searchable text");
        surface.finish();
        page.finish();
        document
            .finish()
            .expect("serialize PDF with both instances")
    }

    fn embedded_font_descriptors(pdf: &str) -> impl Iterator<Item = &str> {
        pdf.split("\nendobj")
            .filter(|object| object.contains("/FontFile2"))
    }

    fn dictionary_value<'a>(object: &'a str, key: &str) -> &'a str {
        object
            .split_once(key)
            .unwrap_or_else(|| panic!("missing {key} in font object"))
            .1
            .trim_start()
    }

    fn pdf_name(value: &str) -> &str {
        value
            .trim_start()
            .strip_prefix('/')
            .expect("PDF name")
            .split(|ch: char| ch.is_ascii_whitespace() || matches!(ch, '/' | '>'))
            .next()
            .unwrap()
    }

    #[test]
    fn embeds_effective_variable_font_geometry() {
        let pdf = export_instances();
        let pdf = std::str::from_utf8(&pdf).expect("ASCII-compatible PDF");
        let mut bounds: Vec<[f32; 4]> = embedded_font_descriptors(pdf)
            .map(|descriptor| {
                let (coordinates, _) = dictionary_value(descriptor, "/FontBBox")
                    .strip_prefix('[')
                    .and_then(|value| value.split_once(']'))
                    .expect("font bounding box array");
                coordinates
                    .split_whitespace()
                    .map(|value| value.parse().expect("numeric font bound"))
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("four font bounds")
            })
            .collect();
        bounds.sort_by(|left, right| left[2].total_cmp(&right[2]));

        let expected = INSTANCES.map(|(_, weight)| {
            let mut fixture = ttf_parser::Face::parse(FIXTURE, 0).unwrap();
            assert_eq!(fixture.units_per_em(), 1000);
            fixture
                .set_variation(ttf_parser::Tag::from_bytes(&WEIGHT), weight)
                .unwrap();
            let widest = fixture
                .glyph_bounding_box(fixture.glyph_index('V').unwrap())
                .expect("outline bounds at the selected weight");
            let bounds = [widest.x_min, widest.y_min, widest.x_max, widest.y_max].map(f32::from);
            let advances = ['A', 'V'].map(|ch| {
                f32::from(
                    fixture
                        .glyph_hor_advance(fixture.glyph_index(ch).unwrap())
                        .unwrap(),
                )
            });
            let notdef = f32::from(fixture.glyph_hor_advance(ttf_parser::GlyphId(0)).unwrap());
            (
                bounds,
                [
                    0.0,
                    0.0,
                    notdef,
                    1.0,
                    1.0,
                    advances[0],
                    2.0,
                    2.0,
                    advances[1],
                ],
            )
        });
        assert_ne!(expected[0], expected[1], "fixture instances must differ");
        assert_eq!(bounds, expected.map(|(bounds, _)| bounds));
        let mut widths: Vec<[f32; 9]> = pdf
            .split("\nendobj")
            .filter(|object| object.contains("/CIDFontType2"))
            .map(|object| {
                let (numeric_widths, _) = dictionary_value(object, "/W")
                    .strip_prefix('[')
                    .and_then(|value| value.split_once(']'))
                    .expect("CID width array");
                numeric_widths
                    .split_whitespace()
                    .map(|value| value.parse().expect("numeric CID width"))
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("three CID width ranges")
            })
            .collect();
        widths.sort_by(|left, right| left[5].total_cmp(&right[5]));
        assert_eq!(widths, expected.map(|(_, widths)| widths));
        assert_eq!(pdf.matches("/ToUnicode").count(), INSTANCES.len());
    }

    #[test]
    fn gives_variable_instances_distinct_subset_tags() {
        let pdf = export_instances();
        let pdf = std::str::from_utf8(&pdf).expect("ASCII-compatible PDF");
        let names: Vec<_> = embedded_font_descriptors(pdf)
            .map(|descriptor| pdf_name(dictionary_value(descriptor, "/FontName")))
            .collect();
        let [first, second] = names.as_slice() else {
            panic!("expected two embedded font subsets, got {names:?}");
        };
        let [first_tag, second_tag] = [first, second].map(|name| {
            let (tag, _) = name.split_once('+').expect("font subset tag");
            assert!(tag.len() == 6 && tag.bytes().all(|byte| byte.is_ascii_uppercase()));
            tag
        });
        assert_ne!(
            first_tag, second_tag,
            "the two instances share a subset tag"
        );
        for name in names {
            let uses = pdf
                .split("/BaseFont")
                .skip(1)
                .map(pdf_name)
                .filter(|base_font| *base_font == name)
                .count();
            assert_eq!(uses, 2, "Type0 and CIDFont must carry the name {name}");
        }
    }
}

#[test]
fn fallback_receives_and_repairs_a_complete_combining_cluster() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
    let context = options()
        .with_font_file(root.join("MermanProbeBase.ttf"))
        .load(&OperationControl::new())
        .unwrap();
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let fallback = options
        .fontdb
        .faces()
        .find(|face| face.post_script_name == "Cousine-Regular")
        .unwrap()
        .id;
    options.set_text_fallback(
        None,
        Some(Box::new(move |_, text, language, rtl, _, _| {
            assert_eq!(text, "f\u{301}");
            assert_eq!(language, Some("en-US"));
            assert!(!rtl);
            Some((fallback, 1.0))
        })),
    );
    let tree = usvg::Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
          <text xml:lang="en-US" id="probe" y="35" font-family="MermanProbeBase" font-size="20">f&#x301;i</text>
        </svg>"#,
        &options,
    ).unwrap();
    assert_eq!(
        painted_faces(&tree, "probe"),
        ["Cousine-Regular", "Cousine-Regular", "MermanProbeBase"]
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Helvetica and its system math fallback; run explicitly in CI"]
fn macos_math_cluster_finds_a_covering_face_within_the_snapshot_budget() {
    let budget = 64 * 1024 * 1024;
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_bytes: budget,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .unwrap()
        .with_strict_glyph_coverage(true);
    assert_text_advance_and_faces(&context, "Helvetica", "ℝ\u{20d7}");
    let faces = context.selected_faces();
    assert_eq!(faces.len(), 1, "one face must shape the cluster: {faces:?}");
    assert!(!faces[0].contains("Emoji"), "{faces:?}");
    assert!(
        context.face_count() <= 2,
        "only the base and fallback may be resident"
    );
    assert!(context.resident_font_bytes() <= budget);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Apple Color Emoji; run explicitly in CI"]
fn macos_authored_emoji_face_is_skipped_before_acquiring_its_bitmap() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts");
    let context = NativeFontOptions::default()
        .with_font_file(root.join("MermanProbeBase.ttf"))
        .with_font_file(root.join("Cousine-Regular.ttf"))
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_bytes: 64 * 1024 * 1024,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .unwrap()
        .with_strict_glyph_coverage(true);
    let resident = context.resident_font_bytes();
    let family = "MermanProbeBase, Apple Color Emoji, Cousine";
    let text = "f\u{301}i";
    assert_text_advance_and_faces(&context, family, text);
    let export = context.for_export(&OperationControl::new()).unwrap();
    let tree = paint_text(&export, family, text);
    assert_eq!(
        painted_faces(&tree, "left"),
        ["Cousine-Regular", "Cousine-Regular", "MermanProbeBase"]
    );
    export.check().unwrap();
    assert_eq!(context.resident_font_bytes(), resident);
    assert_eq!(export.resident_font_bytes(), resident);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS system Han fallback; run explicitly in CI"]
fn core_text_inherits_svg_language_in_platform_fallback() {
    let context = options()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .unwrap();
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let tree = usvg::Tree::from_str(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="240" height="100">
        <g font-family="Cousine" font-size="22" xml:lang="zh-Hans">
          <text id="regions" x="10" y="50">骨<tspan xml:lang="zh-Hant">骨</tspan><tspan xml:lang="ja">骨</tspan><tspan xml:lang="ko">骨</tspan></text>
        </g></svg>"#,
        &options,
    ).unwrap();
    context.check().unwrap();
    let regional = painted_faces(&tree, "regions");
    assert_eq!(regional.len(), 4);

    // Several languages may use the same installed face. Compare each span to
    // a fresh context with that language and its own platform selections.
    for (tag, expected) in ["zh-Hans", "zh-Hant", "ja", "ko"].into_iter().zip(regional) {
        let localized = context.clone().with_language(tag);
        assert_ne!(localized.identity(), context.identity());
        assert_text_advance_and_faces(&localized, "Cousine", "骨");
        let selected = localized.selected_faces();
        let [face] = selected.as_slice() else {
            panic!("{tag}: expected one fallback face, got {selected:?}");
        };
        assert_eq!(face.rsplit_once('#').unwrap().0, expected, "{tag}");
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Arial and its system fallback fonts; run explicitly in CI"]
fn macos_character_fallback_shares_symbol_faces_with_paint_without_emoji_admission() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_bytes: 8 * 1024 * 1024,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes");
    assert_text_advance_and_faces(&context, "Arial", "Latin ✓ ∑ ∫ e\u{301}");
    let faces = context.selected_faces();
    assert!(
        faces.iter().any(|face| !face.starts_with("Arial")),
        "the check mark must exercise glyph fallback: {faces:?}"
    );
    assert!(
        faces
            .iter()
            .all(|face| !face.to_ascii_lowercase().contains("emoji"))
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Arial and its system fallback fonts; run explicitly in CI"]
fn macos_unassigned_scalar_declines_without_acquiring_unrelated_fallback_files() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes");
    assert!(advance(&context, "AVA", &style("Arial")) > 0.0);
    let resident = context.resident_font_bytes();
    let faces = context.face_count();
    assert!(
        context
            .measure(request("A\u{0378}", &style("Arial")))
            .unwrap()
            .is_none()
    );
    assert_eq!(context.resident_font_bytes(), resident);
    assert_eq!(context.face_count(), faces);
    context
        .check()
        .expect("missing coverage does not poison the context");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Arial and CJK system fonts; run explicitly in CI"]
fn macos_selected_collection_faces_fit_the_data_budget_and_survive_export() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("platform font context initializes")
        .with_strict_glyph_coverage(true);
    for text in ["Latin 中文 日本語 한글", "Latin 𠮷野家"] {
        let operation = context.for_operation(&OperationControl::new());
        assert_text_advance_and_faces(&operation, "Arial", text);
        let resident = operation.resident_font_bytes();
        assert!(resident > 0);
        assert!(resident <= NativeFontLimits::default().max_bytes);
        let export = operation.for_export(&OperationControl::new()).unwrap();
        assert_text_advance_and_faces(&export, "Arial", text);
        assert_eq!(export.resident_font_bytes(), resident);
        assert_eq!(operation.resident_font_bytes(), resident);
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Arial and its system fallback fonts; run explicitly in CI"]
fn macos_native_queries_keep_family_order_and_selected_source_limits() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_files: 1,
            max_faces: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("initialization does not acquire unrelated font files");
    assert_text_advance_and_faces(&context, "MermanMissingFamily-46da48fb, Arial", "AVA");
    assert_eq!(context.selected_faces(), ["ArialMT#0"]);
    assert_eq!(context.face_count(), 1);
    let bytes = context.resident_font_bytes();
    let export = context.for_export(&OperationControl::new()).unwrap();
    assert_text_advance_and_faces(&export, "Arial", "AVA");
    assert_eq!(export.resident_font_bytes(), bytes);
    assert_eq!(export.face_count(), 1);

    // A real fallback still consumes a source slot and cannot bypass admission.
    assert!(context.measure(request("A中文", &style("Arial"))).is_err());
    assert!(matches!(
        context.check(),
        Err(NativeFontError::Limit { .. })
    ));
    let fresh = context.for_operation(&OperationControl::new());
    assert_eq!(fresh.resident_font_bytes(), 0);
    assert_eq!(fresh.face_count(), 0);
    assert!(advance(&fresh, "AVA", &style("Arial")) > 0.0);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS system CJK fallback; run explicitly in CI"]
fn macos_unregistered_explicit_font_can_use_authorized_system_fallback() {
    let context = options()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .unwrap()
        .with_strict_glyph_coverage(true);
    let ascii = context.for_operation(&OperationControl::new());
    assert_same_advance_and_faces(&ascii, "Cousine");
    assert_eq!(ascii.selected_faces(), ["Cousine-Regular#0"]);
    assert_text_advance_and_faces(&context, "Cousine", "A中V");
    assert!(
        context
            .selected_faces()
            .iter()
            .any(|face| !face.starts_with("Cousine-"))
    );
    let closed = options().load_platform(&OperationControl::new()).unwrap();
    assert!(
        closed
            .measure(request("A中V", &style("Cousine")))
            .unwrap()
            .is_none()
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires the macOS Helvetica collection and Arial; run explicitly in CI"]
fn macos_collection_weights_share_one_file_slot_across_export() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_files: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .unwrap();
    let regular = style("Helvetica");
    assert!(advance(&context, "AVA", &regular) > 0.0);
    let export = context.for_export(&OperationControl::new()).unwrap();
    let bold = TextStyle {
        font_weight: Some("700".to_owned()),
        ..regular
    };
    assert!(advance(&export, "AVA", &bold) > 0.0);
    assert_eq!(export.face_count(), 2);
    assert_eq!(context.face_count(), 1);
    assert!(export.measure(request("AVA", &style("Arial"))).is_err());
    assert!(matches!(
        export.check(),
        Err(NativeFontError::Limit {
            resource: "system-snapshot-files",
            actual: 2,
            limit: 1
        })
    ));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires the macOS Heiti SC and TC shared-outline collection"]
fn macos_heiti_sc_and_tc_share_outline_bytes_through_export() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .unwrap();
    let mut bytes = None;
    for family in ["Heiti SC", "Heiti TC"] {
        let light = TextStyle {
            font_weight: Some("300".to_owned()),
            ..style(family)
        };
        assert!(advance(&context, "漢字", &light) > 0.0);
        let resident = context.resident_font_bytes();
        assert!(resident > 0 && resident <= NativeFontLimits::default().max_bytes);
        assert_eq!(*bytes.get_or_insert(resident), resident);
    }
    assert_eq!(context.face_count(), 2);
    let export = context.for_export(&OperationControl::new()).unwrap();
    let mut options = usvg::Options::default();
    export.apply_to_usvg_options(&mut options);
    let tree = usvg::Tree::from_str(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60" font-weight="300"><text id="sc" y="20" font-family="Heiti SC">漢字</text><text id="tc" y="50" font-family="Heiti TC">漢字</text></svg>"#, &options).unwrap();
    let mut painted = BTreeSet::new();
    for id in ["sc", "tc"] {
        for span in painted_text(&tree, id).layouted() {
            for glyph in &span.positioned_glyphs {
                assert_ne!(glyph.id.0, 0);
                let face = tree.fontdb().face(glyph.font).unwrap();
                painted.insert(format!("{}#{}", face.post_script_name, face.index));
            }
        }
    }
    assert_eq!(painted.len(), 2);
    assert_eq!(painted, context.selected_faces().into_iter().collect());
    export.check().unwrap();
    assert_eq!(export.resident_font_bytes(), context.resident_font_bytes());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_variations_share_one_primary_selection_per_css_weight() {
    let context = NativeFontOptions::default()
        .with_font_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fonts/MermanVariableWeight.ttf"),
        )
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_selection_entries: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .unwrap();
    let mut options = usvg::Options::default();
    context.apply_to_usvg_options(&mut options);
    let ink_width = |css_weight: u16, axis_weight: u16| -> Result<f32, NativeFontError> {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="60">
                <text id="sample" y="45" font-family="Merman Variable Weight" font-size="32"
                      font-weight="{css_weight}" font-variation-settings="'wght' {axis_weight}">AV</text>
            </svg>"#
        );
        let tree = usvg::Tree::from_str(&svg, &options).unwrap();
        context.check()?;
        let sample = painted_text(&tree, "sample");
        Ok(sample.flattened().bounding_box().width())
    };

    let thin = ink_width(400, 100).unwrap();
    let bold = ink_width(400, 700).unwrap();
    assert!(thin < bold);
    assert!(matches!(
        ink_width(700, 700),
        Err(NativeFontError::Limit {
            resource: "font-selections",
            actual: 2,
            limit: 1,
        })
    ));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Arial; run explicitly in CI"]
fn macos_selection_pinning_has_independent_terminal_metadata_limits() {
    for limits in [
        NativeFontLimits {
            max_selection_entries: 0,
            ..Default::default()
        },
        NativeFontLimits {
            max_selection_bytes: 1,
            ..Default::default()
        },
    ] {
        let context = NativeFontOptions::default()
            .with_system_fonts(true)
            .with_limits(limits)
            .load_platform(&OperationControl::new())
            .unwrap();
        assert!(context.measure(request("AVA", &style("Arial"))).is_err());
        assert!(matches!(
            context.check(),
            Err(NativeFontError::Limit {
                resource: "font-selections" | "font-selection-bytes",
                ..
            })
        ));
    }
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires macOS Arial; run explicitly in CI"]
fn macos_concurrent_cold_paint_queries_share_one_admitted_selection() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_files: 1,
            max_faces: 1,
            max_selection_entries: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .unwrap();
    let start = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        let threads: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    let tree = paint(&context, "Arial");
                    let text = painted_text(&tree, "left");
                    assert!(!text.layouted().is_empty());
                    context.check().unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
    });
    assert_eq!(context.face_count(), 1);
    let bytes = context.resident_font_bytes();
    let export = context.for_export(&OperationControl::new()).unwrap();
    let _ = paint(&export, "Arial");
    export.check().unwrap();
    assert_eq!(export.resident_font_bytes(), bytes);
    assert_eq!(export.face_count(), 1);
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
fn windows_platform_first_named_face_uses_the_only_file_slot() {
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .with_limits(NativeFontLimits {
            max_files: 1,
            ..Default::default()
        })
        .load_platform(&OperationControl::new())
        .expect("platform initializes without consuming its file budget");
    assert_eq!(context.resident_font_bytes(), 0);
    assert_same_advance_and_faces(&context, "Trebuchet MS");
    let resident = context.resident_font_bytes();
    assert!(resident > 0);
    assert_eq!(context.face_count(), 1);
    assert_same_advance_and_faces(&context, "Trebuchet MS");
    assert_eq!(context.resident_font_bytes(), resident);
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
#[ignore = "requires Windows Segoe UI Emoji; run explicitly on controlled system-font hosts"]
fn windows_platform_fallback_shapes_a_supplementary_character_in_layout_and_paint() {
    let context = options()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .expect("Windows platform font context initializes");
    assert_text_advance_and_faces(&context, "Cousine", "AVA \u{1f600}");
    assert!(
        context
            .selected_faces()
            .iter()
            .any(|face| face.contains("Segoe"))
    );
    context.check().unwrap();
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires Windows Trebuchet MS, Arial and Segoe UI Emoji"]
fn windows_platform_fallback_keeps_coverage_when_a_scalar_face_would_change_the_run() {
    let control = OperationControl::new();
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&control)
        .unwrap()
        .with_strict_glyph_coverage(true);
    for family in ["Trebuchet MS, Verdana, Arial, sans-serif", "Arial"] {
        let operation = context.for_operation(&OperationControl::new());
        assert_text_advance_and_faces(&operation, family, "fi✓😀");
        operation.check().unwrap();
    }
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires Windows Calibri, Trebuchet MS and system font fallback"]
fn windows_platform_fallback_covers_mixed_ligatures_symbols_and_han() {
    let fonts = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts");
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .unwrap()
        .with_strict_glyph_coverage(true);
    for (family, text, font_files) in [
        (
            "Calibri, Arial, sans-serif",
            "fi✓中",
            &["calibri.ttf", "seguiemj.ttf", "msyh.ttc"][..],
        ),
        (
            "Trebuchet MS, Verdana, Arial, sans-serif",
            "fi✚✗⚑",
            &["trebuc.ttf", "seguisym.ttf"][..],
        ),
    ] {
        if font_files.iter().any(|file| !fonts.join(file).is_file()) {
            continue;
        }
        let operation = context.for_operation(&OperationControl::new());
        assert_text_advance_and_faces(&operation, family, text);
        operation.check().unwrap();
    }
}

#[cfg(target_os = "windows")]
#[test]
#[ignore = "requires the Windows Microsoft Himalaya and Trebuchet MS fonts"]
fn windows_platform_fallback_accepts_the_physical_faces_alternate_postscript_record() {
    let fonts = PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("Fonts");
    if !fonts.join("himalaya.ttf").is_file() {
        return;
    }
    let context = NativeFontOptions::default()
        .with_system_fonts(true)
        .load_platform(&OperationControl::new())
        .unwrap()
        .with_strict_glyph_coverage(true);
    assert_text_advance_and_faces(&context, "Trebuchet MS, Verdana, Arial, sans-serif", "བོད་");
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
