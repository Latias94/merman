//! Actual marker pixels, with marker references removed as an independent negative control.

#![cfg(all(feature = "svg", feature = "png"))]

use merman::svg::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemePreset, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

fn rgba(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

#[test]
fn native_marker_pixels_follow_the_public_edge_paint() {
    let compiler = DiagramThemeCompiler::new();
    let ordinary = compiler
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#00f2ff").unwrap()),
            ))),
        )
        .unwrap();
    let preset = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let transparent = compiler
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
            ))),
        )
        .unwrap();
    for (source, theme, visible) in [
        (
            "flowchart LR\nA[Alpha] <--> B[Beta]",
            ordinary.clone(),
            true,
        ),
        (
            "---\nconfig:\n  look: neo\n---\nflowchart LR\nA[Alpha] o--o B[Beta]",
            ordinary,
            true,
        ),
        (
            "---\nconfig:\n  look: neo\n---\nflowchart LR\nA[Alpha] o--o B[Beta]",
            transparent,
            false,
        ),
        (
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd"),
            preset,
            true,
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
        let options = merman::svg::export::RasterOptions::default();
        let original = document
            .export_png(&options, OperationControl::new())
            .unwrap();
        let xml = roxmltree::Document::parse(document.svg()).unwrap();
        let mut removals = Vec::new();
        for edge in xml
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
        {
            for attr in edge
                .attributes()
                .filter(|attr| matches!(attr.name(), "marker-start" | "marker-end"))
            {
                removals.push(attr.range());
            }
        }
        assert!(removals.len() >= 2);
        removals.sort_by_key(|range| range.start);
        let mut without_markers = document.svg().to_owned();
        for range in removals.into_iter().rev() {
            without_markers.replace_range(range, "");
        }
        let session = merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let mutated = merman_render::svg::finalize_resvg_svg(&without_markers, &session).unwrap();
        let control = merman::svg::export::svg_to_png(&mutated, &options).unwrap();
        let (width, height, pixels) = rgba(original.bytes());
        let (control_width, control_height, control_pixels) = rgba(&control);
        assert_eq!((width, height), (control_width, control_height));
        if !visible {
            assert_eq!(
                pixels, control_pixels,
                "transparent markers must not leave visible fill"
            );
            continue;
        }
        let mut marker_pixels = 0;
        let mut cyan_marker_pixels = 0;
        for (original, control) in pixels.chunks_exact(4).zip(control_pixels.chunks_exact(4)) {
            if original == control {
                continue;
            }
            marker_pixels += 1;
            let [r, g, b] = [
                u16::from(original[0]),
                u16::from(original[1]),
                u16::from(original[2]),
            ];
            cyan_marker_pixels += usize::from(g > r + 50 && b > r + 50 && original[3] > 200);
        }
        assert!(
            marker_pixels > 20,
            "missing visible markers in {source}: {marker_pixels}"
        );
        assert!(
            cyan_marker_pixels > 20 && cyan_marker_pixels * 2 > marker_pixels,
            "marker pixels must use the requested cyan in {source}: {cyan_marker_pixels}/{marker_pixels}"
        );
    }
}
