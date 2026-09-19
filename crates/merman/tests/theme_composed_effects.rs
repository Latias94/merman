//! Production theme effects and native paint witnesses, including marker terminals.

use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectColorSpace, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue,
    ThemeTarget,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, TargetAdmissionReason};
use std::io::Cursor;

fn render(
    source: &str,
    target: ThemeTarget,
    input: EffectInput,
    color_space: EffectColorSpace,
) -> merman::RenderedDocument {
    render_with_offsets(
        source,
        target,
        input,
        color_space,
        [[-12.0, 0.0], [12.0, 0.0]],
    )
}

fn render_with_offsets(
    source: &str,
    target: ThemeTarget,
    input: EffectInput,
    color_space: EffectColorSpace,
    offsets: [[f32; 2]; 2],
) -> merman::RenderedDocument {
    let graph = EffectGraph::new(
        "composed",
        [
            EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: offsets[0][0],
                offset_y: offsets[0][1],
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("rgba(128,48,32,0.5)").unwrap(),
            },
            EffectPrimitive::DropShadow {
                input,
                offset_x: offsets[1][0],
                offset_y: offsets[1][1],
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("rgba(32,64,128,0.6)").unwrap(),
            },
        ],
    )
    .unwrap()
    .with_color_space(color_space);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_canvas(
                    CanvasSpec::default().with_base(CanvasPaint::solid("#000000").unwrap()),
                )
                .with_effects(
                    DiagramEffectSet::default()
                        .with_graph(graph)
                        .unwrap()
                        .with_binding(EffectBinding::new(target, "composed").unwrap())
                        .unwrap(),
                ),
        )
        .unwrap();
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(source, OperationControl::new(), Default::default())
                .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("rendered document required")
    };
    document
}

fn color_counts(bytes: &[u8]) -> (usize, usize) {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let mut red = 0;
    let mut blue = 0;
    for pixel in bytes[..info.buffer_size()].chunks_exact(4) {
        let [r, g, b] = [
            u16::from(pixel[0]),
            u16::from(pixel[1]),
            u16::from(pixel[2]),
        ];
        red += usize::from(r > g + 10 && r > b + 10);
        blue += usize::from(b > r + 10 && b > g + 10);
    }
    (red, blue)
}

#[test]
fn composed_state_shadows_reach_png_and_localized_pdf() {
    assert_composed_shadows("stateDiagram-v2\nReady --> Done", ThemeTarget::State, 2);
}

#[test]
fn composed_flowchart_node_shadows_reach_png_and_localized_pdf() {
    assert_composed_shadows("flowchart LR\nReady --> Done", ThemeTarget::Node, 2);
}

#[test]
fn composed_xychart_series_shadows_reach_png_and_localized_pdf() {
    assert_composed_shadows(
        "xychart\nx-axis [A, B]\ny-axis 0 --> 10\nbar [4, 6]\nline [7, 7]",
        ThemeTarget::ChartSeries,
        3,
    );
}

fn assert_composed_shadows(source: &str, target: ThemeTarget, applications: u32) {
    for color_space in [EffectColorSpace::LinearRgb, EffectColorSpace::Srgb] {
        let previous = render(source, target, EffectInput::Previous, color_space);
        let reset = render(source, target, EffectInput::SourceGraphic, color_space);
        let png = previous
            .export_png(
                &merman::svg::export::RasterOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        let reset_png = reset
            .export_png(
                &merman::svg::export::RasterOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        let pdf = previous
            .export_pdf(
                &merman::svg::export::PdfOptions::default(),
                OperationControl::new(),
            )
            .unwrap();
        for admission in [png.admission(), reset_png.admission(), pdf.admission()] {
            for reason in [
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::NativeFilterReceiptMismatch,
                TargetAdmissionReason::PdfNativeFilterNotLocalized,
            ] {
                assert!(
                    !admission.reasons().contains(&reason),
                    "{color_space:?}: {admission:?}"
                );
            }
        }
        let receipt = png
            .export_report()
            .native_filter_receipt()
            .expect("actual native filters observed");
        assert_eq!(
            (
                receipt.filter_count(),
                receipt.drop_shadow_count(),
                receipt.reference_count()
            ),
            (applications, applications * 2, applications)
        );
        assert_eq!(pdf.export_report().native_filter_receipt(), Some(receipt));
        let (red, blue) = color_counts(png.bytes());
        let (reset_red, reset_blue) = color_counts(reset_png.bytes());
        assert!(
            red > 30 && blue > 30,
            "{color_space:?}: visible soft red/blue shadows {red}/{blue}"
        );
        assert_eq!(
            reset_red, 0,
            "SourceGraphic restarts the chain, discarding the first red shadow"
        );
        assert!(reset_blue > 30);
        assert!(pdf.bytes().starts_with(b"%PDF-"));
        if color_space == EffectColorSpace::Srgb {
            let limits = merman::svg::export::SvgConversionLimits {
                max_filter_primitives_per_filter: Some(8),
                ..Default::default()
            };
            let error = previous
                .export_png(
                    &merman::svg::export::RasterOptions::default().with_conversion_limits(limits),
                    OperationControl::new(),
                )
                .expect_err("an explicit lower primitive budget remains authoritative");
            assert!(
                error
                    .to_string()
                    .contains("max_svg_conversion_filter_primitives_per_filter"),
                "{error}"
            );
        }
    }
}

#[test]
fn shadow_writer_elides_only_srgb_identity_translations() {
    for offsets in [
        [[0.0, -0.0], [-0.0, 0.0]],
        [[0.0, 6.0], [0.0, 0.0]],
        [[0.0, 0.0], [-6.0, 0.0]],
    ] {
        for color_space in [EffectColorSpace::Srgb, EffectColorSpace::LinearRgb] {
            let document = render_with_offsets(
                "stateDiagram-v2\nReady --> Done",
                ThemeTarget::State,
                EffectInput::Previous,
                color_space,
                offsets,
            );
            let xml = roxmltree::Document::parse(document.svg()).unwrap();
            let translated = offsets
                .iter()
                .filter(|[x, y]| *x != 0.0 || *y != 0.0)
                .count();
            assert_eq!(
                xml.descendants()
                    .filter(|node| node.has_tag_name("feOffset"))
                    .count(),
                if color_space == EffectColorSpace::Srgb {
                    translated * 2
                } else {
                    0
                }
            );
            let png = document
                .export_png(&Default::default(), OperationControl::new())
                .unwrap();
            let pdf = document
                .export_pdf(&Default::default(), OperationControl::new())
                .unwrap();
            let receipt = png.export_report().native_filter_receipt().unwrap();
            assert_eq!(receipt.drop_shadow_count(), 4);
            assert_eq!(pdf.export_report().native_filter_receipt(), Some(receipt));
            assert_eq!(
                png.export_report().conversion().filter_primitives,
                if color_space == EffectColorSpace::Srgb {
                    (8 + translated) * 2
                } else {
                    4
                }
            );
            for admission in [png.admission(), pdf.admission()] {
                assert!(
                    !admission
                        .reasons()
                        .contains(&TargetAdmissionReason::NativeFilterReceiptMismatch)
                );
            }
        }
    }
}

#[test]
fn composed_sequence_actor_shadows_reach_png_and_localized_pdf() {
    assert_composed_shadows("sequenceDiagram\nA->>B: Hello", ThemeTarget::Actor, 4);
}

#[test]
fn public_cyberpunk_sequence_shape_and_message_glow_survives_native_export() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(merman::svg::ThemePreset::Cyberpunk)
        .unwrap();
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(
                include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"),
                OperationControl::new(),
                Default::default(),
            )
            .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("document required")
    };
    let png = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .unwrap();
    let pdf = document
        .export_pdf(
            &merman::svg::export::PdfOptions::default(),
            OperationControl::new(),
        )
        .unwrap();
    for admission in [png.admission(), pdf.admission()] {
        assert!(
            matches!(
                admission.status(),
                merman::TargetAdmissionStatus::Portable
                    | merman::TargetAdmissionStatus::HostDependent
            ),
            "unexpected native admission: {admission:?}"
        );
        for reason in [
            TargetAdmissionReason::ThemeEvidenceIncomplete,
            TargetAdmissionReason::NativeFilterReceiptMismatch,
            TargetAdmissionReason::PdfNativeFilterNotLocalized,
        ] {
            assert!(!admission.reasons().contains(&reason), "{admission:?}");
        }
    }
    let receipt = png
        .export_report()
        .native_filter_receipt()
        .expect("actual actor filters");
    assert_eq!(
        (
            receipt.filter_count(),
            receipt.reference_count(),
            receipt.drop_shadow_count()
        ),
        (8, 8, 12)
    );
    assert_eq!(pdf.export_report().native_filter_receipt(), Some(receipt));
}

#[test]
fn sequence_native_message_markers_use_the_requested_paint() {
    use merman::svg::{ThemeRule, ThemeRuleSet, ThemeStylePatch};

    let compiler = DiagramThemeCompiler::new();
    for (paint, visible) in [
        (CanvasPaint::solid("#00f2ff").unwrap(), true),
        (CanvasPaint::Transparent, false),
    ] {
        let theme = compiler
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default().with_stroke(paint),
                    ),
                )),
            )
            .unwrap();
        for arrow in [
            "->>", "-->>", "-x", "-)", r"-|\", "-|/", r"-\\", "-//", "<<->>",
        ] {
            let source = format!("sequenceDiagram\nautonumber\nA{arrow}B: Message");
            let RenderOutput::Document(Some(document)) = Renderer::new()
                .render(
                    RenderRequest::document(&source, OperationControl::new(), Default::default())
                        .with_theme(theme.clone()),
                )
                .unwrap()
            else {
                panic!("document required")
            };
            assert_sequence_marker_pixels(&document, visible, &source);
        }
    }
}

#[test]
fn sequence_native_message_width_and_glow_preserve_markers_at_zero_margin() {
    use merman::svg::{ThemeRule, ThemeRuleSet, ThemeStylePatch};
    for glow in [false, true] {
        for width in [2.0, 12.0] {
            let effects = if glow {
                DiagramEffectSet::default()
                    .with_graph(
                        EffectGraph::new(
                            "glow",
                            [EffectPrimitive::DropShadow {
                                input: EffectInput::SourceGraphic,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                blur_radius: 6.0,
                                spread: 0.0,
                                color: ThemeColorValue::parse("rgba(0, 242, 255, 0.6)").unwrap(),
                            }],
                        )
                        .unwrap(),
                    )
                    .unwrap()
                    .with_binding(EffectBinding::new(ThemeTarget::Message, "glow").unwrap())
                    .unwrap()
            } else {
                DiagramEffectSet::default()
            };
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_effects(effects).with_styles(
                        ThemeRuleSet::default().with_rule(ThemeRule::new(
                            ThemeTarget::Message,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#00f2ff").unwrap())
                                .with_stroke_width(width)
                                .unwrap(),
                        )),
                    ),
                )
                .unwrap();
            for arrow in ["->>", "-x", "-)"] {
                for (target, right_angles) in [("B", false), ("A", false), ("A", true)] {
                    let source = format!(
                        "---\nconfig:\n  sequence:\n    mirrorActors: false\n    diagramMarginX: 0\n    diagramMarginY: 0\n    rightAngles: {right_angles}\n---\nsequenceDiagram\nA{arrow}{target}: Message"
                    );
                    let RenderOutput::Document(Some(document)) = Renderer::new()
                        .render(
                            RenderRequest::document(
                                &source,
                                OperationControl::new(),
                                Default::default(),
                            )
                            .with_theme(theme.clone()),
                        )
                        .unwrap()
                    else {
                        panic!("document required")
                    };
                    if !glow && target == "B" && arrow == "->>" {
                        let xml = roxmltree::Document::parse(document.svg()).unwrap();
                        let line = xml
                            .descendants()
                            .find(|n| n.attribute("class") == Some("messageLine0"))
                            .unwrap();
                        let view_box: Vec<f64> = xml
                            .root_element()
                            .attribute("viewBox")
                            .unwrap()
                            .split_whitespace()
                            .map(|s| s.parse().unwrap())
                            .collect();
                        let midpoint = (line.attribute("x1").unwrap().parse::<f64>().unwrap()
                            + line.attribute("x2").unwrap().parse::<f64>().unwrap())
                            / 2.0;
                        let png = document
                            .export_png(&Default::default(), OperationControl::new())
                            .unwrap();
                        let mut reader = png::Decoder::new(Cursor::new(png.bytes()))
                            .read_info()
                            .unwrap();
                        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
                        let info = reader.next_frame(&mut pixels).unwrap();
                        assert_eq!(info.color_type, png::ColorType::Rgba);
                        let scale = f64::from(info.width) / view_box[2];
                        let x = ((midpoint - view_box[0]) * scale).round() as usize;
                        let cyan_rows = (0..info.height as usize)
                            .filter(|y| {
                                let pixel = &pixels[(y * info.width as usize + x) * 4..][..4];
                                u16::from(pixel[1]) > u16::from(pixel[0]) + 50
                                    && u16::from(pixel[2]) > u16::from(pixel[0]) + 50
                                    && pixel[3] > 128
                            })
                            .count();
                        assert!(
                            (cyan_rows as f64 - f64::from(width) * scale).abs() <= 1.5,
                            "actual native message width: {cyan_rows} pixels, requested {width} at scale {scale}"
                        );
                    }
                    assert_sequence_marker_pixels(
                        &document,
                        true,
                        &format!("width {width}, glow {glow}: {source}"),
                    );
                }
            }
        }
    }
}

#[test]
fn sequence_native_marker_paint_preserves_source_and_clear_ownership() {
    use merman::svg::{Specified, ThemePreset, ThemeRule, ThemeRuleSet, ThemeStylePatch};

    let compiler = DiagramThemeCompiler::new();
    let mut clear = ThemeStylePatch::default();
    clear.stroke.paint = Specified::Clear;
    let cleared = compiler
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#ff0000").unwrap()),
                    ))
                    .with_rule(ThemeRule::new(ThemeTarget::Message, clear)),
            ),
        )
        .unwrap();
    let baseline_source = "sequenceDiagram\nA->>B: Message";
    let mut baseline: Option<Vec<u8>> = None;
    for theme in [None, Some(cleared)] {
        let request =
            RenderRequest::document(baseline_source, OperationControl::new(), Default::default());
        let request = match theme {
            Some(theme) => request.with_theme(theme),
            None => request,
        };
        let RenderOutput::Document(Some(document)) = Renderer::new().render(request).unwrap()
        else {
            panic!("document required")
        };
        let png = document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap();
        if let Some(baseline) = &baseline {
            assert_eq!(
                png.bytes(),
                baseline.as_slice(),
                "Clear must restore default marker paint"
            );
        } else {
            baseline = Some(png.bytes().to_vec());
        }
    }
    let source = r##"---
config:
  themeVariables:
    signalColor: '#00f2ff'
---
sequenceDiagram
A->>B: Message
"##;
    let ordinary = compiler
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Message,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#ff0000").unwrap()),
            ))),
        )
        .unwrap();
    for (source, theme) in [
        (source, ordinary),
        (
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"),
            compiler.compile_preset(ThemePreset::Cyberpunk).unwrap(),
        ),
    ] {
        let RenderOutput::Document(Some(document)) = Renderer::new()
            .render(
                RenderRequest::document(source, OperationControl::new(), Default::default())
                    .with_theme(theme),
            )
            .unwrap()
        else {
            panic!("document required")
        };
        assert_sequence_marker_pixels(&document, true, source);
    }
}

fn assert_sequence_marker_pixels(document: &merman::RenderedDocument, visible: bool, case: &str) {
    let svg = document.svg();
    let xml = roxmltree::Document::parse(svg).unwrap();
    let mut markers = Vec::new();
    for node in xml.descendants() {
        let is_message = node.attribute("class").is_some_and(|classes| {
            classes
                .split_whitespace()
                .any(|c| matches!(c, "messageLine0" | "messageLine1"))
        });
        for attribute in node
            .attributes()
            .filter(|attribute| matches!(attribute.name(), "marker-start" | "marker-end"))
        {
            if is_message {
                markers.push(attribute.range());
            }
        }
    }
    assert!(!markers.is_empty(), "missing marker control for {case}");
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let options = merman::svg::export::RasterOptions::default();
    let original = document
        .export_png(&options, OperationControl::new())
        .unwrap();
    let decode = |bytes: &[u8]| {
        let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        pixels.truncate(info.buffer_size());
        ((info.width, info.height), pixels)
    };
    let (size, original) = decode(original.bytes());
    // Judge opaque marker color separately from the translucent halo. Keep the original
    // filtered render below as an independent witness that each marker still contributes paint.
    let without_message_filters = |svg: &str| {
        let xml = roxmltree::Document::parse(svg).unwrap();
        let mut ranges: Vec<_> = xml
            .descendants()
            .filter(|node| {
                matches!(
                    node.attribute("class"),
                    Some("messageLine0" | "messageLine1")
                )
            })
            .flat_map(|node| {
                node.attributes()
                    .filter(|a| a.name() == "filter")
                    .map(|a| a.range())
            })
            .collect();
        ranges.sort_by_key(|range| range.start);
        let mut result = svg.to_owned();
        for range in ranges.into_iter().rev() {
            result.replace_range(range, "");
        }
        result
    };
    let render_control = |svg: &str| {
        let control = merman_render::svg::finalize_resvg_svg(svg, &session).unwrap();
        let png = merman::svg::export::svg_to_png(&control, &options).unwrap();
        let (control_size, pixels) = decode(&png);
        assert_eq!(size, control_size);
        pixels
    };
    let unfiltered = without_message_filters(svg);
    let color_original = (unfiltered != svg).then(|| render_control(&unfiltered));
    // Remove one reference at a time so a correct arrow cannot hide a gray sibling.
    for marker in markers {
        let marker_name = &svg[marker.clone()];
        let mut without_marker = svg.to_owned();
        without_marker.replace_range(marker, "");
        let control = merman_render::svg::finalize_resvg_svg(&without_marker, &session).unwrap();
        let control = merman::svg::export::svg_to_png(&control, &options).unwrap();
        let (control_size, control) = decode(&control);
        assert_eq!(size, control_size);
        if !visible {
            assert_eq!(
                original, control,
                "transparent {marker_name} must be invisible: {case}"
            );
            continue;
        }
        assert!(
            original
                .chunks_exact(4)
                .zip(control.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count()
                > 5,
            "missing filtered {marker_name} pixels: {case}"
        );
        let color_control = color_original
            .as_ref()
            .map(|_| render_control(&without_message_filters(&without_marker)));
        let original = color_original.as_deref().unwrap_or(&original);
        let control = color_control.as_deref().unwrap_or(&control);
        let mut changed = 0;
        let mut cyan = 0;
        for (pixel, control) in original.chunks_exact(4).zip(control.chunks_exact(4)) {
            if pixel == control {
                continue;
            }
            changed += 1;
            let [r, g, b] = [
                u16::from(pixel[0]),
                u16::from(pixel[1]),
                u16::from(pixel[2]),
            ];
            cyan += usize::from(g > r + 50 && b > r + 50 && pixel[3] > 200);
        }
        assert!(changed > 5, "missing {marker_name} pixels: {case}");
        assert!(
            cyan > 5 && cyan * 2 > changed,
            "expected cyan {marker_name}: {cyan}/{changed}: {case}"
        );
    }
}

#[test]
fn composed_sequence_message_shadows_reach_png_and_localized_pdf() {
    for source in [
        "sequenceDiagram\nA->>B: Request\nB-->>A: Reply\nA->>A: Self",
        "---\nconfig:\n  sequence:\n    rightAngles: true\n    diagramMarginX: 0\n    diagramMarginY: 0\n---\nsequenceDiagram\nA->>B: Request\nB-->>A: Reply\nA->>A: Self",
    ] {
        assert_composed_shadows(source, ThemeTarget::Message, 3);
    }
}

#[test]
fn composed_sequence_note_shadows_reach_png_and_localized_pdf() {
    assert_composed_shadows(
        "sequenceDiagram\nNote left of A: Left\nNote right of B: Right\nNote over A,B: A long note spanning both participants",
        ThemeTarget::Note,
        3,
    );
}

#[test]
fn composed_sequence_note_text_shadows_reach_png_and_localized_pdf() {
    assert_composed_shadows(
        "sequenceDiagram\nNote left of A: Left label<br/><br/>Second line\nNote right of B: Right label",
        ThemeTarget::NoteLabel,
        3,
    );
}

#[test]
fn paintless_sequence_note_labels_do_not_create_empty_native_filter_groups() {
    let document = render(
        "sequenceDiagram\nNote over A,B: <br/>",
        ThemeTarget::NoteLabel,
        EffectInput::Previous,
        EffectColorSpace::Srgb,
    );
    assert!(!document.svg().contains("<filter"));
    let png = document
        .export_png(&Default::default(), OperationControl::new())
        .unwrap();
    let pdf = document
        .export_pdf(&Default::default(), OperationControl::new())
        .unwrap();
    for admission in [png.admission(), pdf.admission()] {
        for reason in [
            TargetAdmissionReason::ThemeEvidenceIncomplete,
            TargetAdmissionReason::NativeFilterReceiptMismatch,
            TargetAdmissionReason::PdfNativeFilterNotLocalized,
        ] {
            assert!(!admission.reasons().contains(&reason), "{admission:?}");
        }
    }
}

#[test]
fn nonempty_note_text_with_zero_host_width_is_not_paintless_or_certified() {
    use merman::svg::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity, TextMeasurer, TextMetrics, TextStyle,
    };
    #[derive(Debug)]
    struct ZeroWidth;
    impl TextMeasurer for ZeroWidth {
        fn measure(&self, _: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: style.font_size,
                line_count: 1,
            }
        }
    }
    let environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.zero-note-width").unwrap(),
                "1",
            )
            .unwrap(),
            std::sync::Arc::new(ZeroWidth),
        )),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_effects(
                DiagramEffectSet::default()
                    .with_graph(
                        EffectGraph::new(
                            "note",
                            [EffectPrimitive::DropShadow {
                                input: EffectInput::SourceGraphic,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                blur_radius: 4.0,
                                spread: 0.0,
                                color: ThemeColorValue::parse("#ff00ff").unwrap(),
                            }],
                        )
                        .unwrap(),
                    )
                    .unwrap()
                    .with_binding(EffectBinding::new(ThemeTarget::NoteLabel, "note").unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    let source = format!("sequenceDiagram\nNote over A,B: {}", "W".repeat(128));
    let RenderOutput::Document(Some(document)) = Renderer::new()
        .render(
            RenderRequest::document(
                &source,
                OperationControl::new(),
                merman::SvgRequest {
                    environment,
                    ..Default::default()
                },
            )
            .with_theme(theme),
        )
        .unwrap()
    else {
        panic!("document required")
    };
    assert!(document.svg().contains("-note-text-0-theme-effect-"));
    let png = document
        .export_png(&Default::default(), OperationControl::new())
        .unwrap();
    let pdf = document
        .export_pdf(&Default::default(), OperationControl::new())
        .unwrap();
    for admission in [png.admission(), pdf.admission()] {
        assert!(
            admission
                .reasons()
                .contains(&TargetAdmissionReason::NativeFilterReceiptMismatch),
            "zero host width must not certify overflowing glyph ink: {admission:?}"
        );
    }
}
