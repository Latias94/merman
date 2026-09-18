//! Production State writer and native-export witnesses for the bounded shadow sequence.

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
fn public_cyberpunk_sequence_actor_glow_survives_native_export() {
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
        (4, 4, 8)
    );
    assert_eq!(pdf.export_report().native_filter_receipt(), Some(receipt));
}
