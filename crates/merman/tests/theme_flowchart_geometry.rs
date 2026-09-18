//! Typed rectangle geometry must change actual native paint, not only SVG attributes.

#![cfg(all(feature = "svg", feature = "png"))]

use merman::svg::{
    CanvasPaint, CanvasSpec, DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget,
};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

fn painted_rect(width: f32, radius: f32, source: &str) -> (u32, u32, Vec<u8>) {
    let mut patch = ThemeStylePatch::default()
        .with_fill(CanvasPaint::solid("#ffffff").unwrap())
        .with_stroke(CanvasPaint::solid("#00f2ff").unwrap())
        .with_stroke_width(width)
        .unwrap();
    patch.geometry.radius = Specified::Value(radius);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_canvas(
                    CanvasSpec::default().with_base(CanvasPaint::solid("#000000").unwrap()),
                )
                .with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)),
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
        panic!("document required")
    };
    document_pixels(&document, 4.0)
}

fn document_pixels(document: &merman::RenderedDocument, scale: f32) -> (u32, u32, Vec<u8>) {
    let output = document
        .export_png(
            &merman::svg::export::RasterOptions::default().with_scale(scale),
            OperationControl::new(),
        )
        .unwrap();
    assert!(
        !output
            .admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::ThemeEvidenceIncomplete)
    );
    let mut reader = png::Decoder::new(std::io::Cursor::new(output.bytes()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    pixels.truncate(info.buffer_size());
    (info.width, info.height, pixels)
}

#[test]
fn rounded_rectangle_width_and_radius_change_native_pixels() {
    for look in ["classic", "neo"] {
        for shape in ["[Rectangle]", "(Rounded rectangle)"] {
            let source = format!("---\nconfig:\n  look: {look}\n---\nflowchart LR\nA{shape}");
            let (width, height, rounded) = painted_rect(3.0, 10.0, &source);
            let (thin_width, thin_height, thin) = painted_rect(1.0, 10.0, &source);
            let (square_width, square_height, square) = painted_rect(3.0, 0.0, &source);
            assert_eq!((width, height), (thin_width, thin_height));
            assert_eq!((width, height), (square_width, square_height));
            let cyan = |pixels: &[u8]| {
                pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] < 60 && p[1] > 200 && p[2] > 200 && p[3] > 200)
                    .count()
            };
            assert!(
                cyan(&rounded) > cyan(&thin) * 2,
                "3px stroke must paint more than twice the 1px stroke"
            );
            let white = |pixels: &[u8]| {
                pixels
                    .chunks_exact(4)
                    .filter(|p| p[0] > 240 && p[1] > 240 && p[2] > 240 && p[3] > 200)
                    .count()
            };
            assert!(
                white(&square) > white(&rounded) + 100,
                "{look}: 10px corners must remove white fill area on the black canvas at 4x scale: square={}, rounded={}, differing pixels={}",
                white(&square),
                white(&rounded),
                square
                    .chunks_exact(4)
                    .zip(rounded.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count()
            );
        }
    }
}

#[test]
fn neo_configured_radius_and_equivalent_typed_radius_have_identical_native_geometry() {
    for shape in ["[Rectangle]", "(Rounded rectangle)"] {
        let configured = format!(
            "---\nconfig:\n  look: neo\n  themeVariables:\n    radius: 4\n---\nflowchart LR\nA{shape}"
        );
        let typed = format!("---\nconfig:\n  look: neo\n---\nflowchart LR\nA{shape}");
        let configured = painted_rect(3.0, 10.0, &configured);
        let typed = painted_rect(3.0, 4.0, &typed);
        assert_eq!((configured.0, configured.1), (typed.0, typed.1));
        assert!(
            configured.2 == typed.2,
            "Neo config radius must reach native geometry for {shape}"
        );
    }
}

#[test]
fn source_radius_and_equivalent_typed_radius_have_identical_native_geometry() {
    for look in ["classic", "neo"] {
        for shape in ["[Rectangle]", "(Rounded rectangle)"] {
            let source = format!(
                "---\nconfig:\n  look: {look}\n---\nflowchart LR\nA{shape}\nstyle A rx:4px!important,RX:6px,ry:4px"
            );
            let typed = format!("---\nconfig:\n  look: {look}\n---\nflowchart LR\nA{shape}");
            let source = painted_rect(3.0, 10.0, &source);
            let typed = painted_rect(3.0, 4.0, &typed);
            assert_eq!((source.0, source.1), (typed.0, typed.1));
            assert!(
                source.2 == typed.2,
                "source radius must reach native geometry for {look} {shape}"
            );
        }
    }
}

#[test]
fn exported_cyberpunk_corners_change_rectangle_pixels_and_preserve_diamonds() {
    use merman::svg::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let recipe = compiler.export_preset(ThemePreset::Cyberpunk).unwrap();
    let rounded = compiler.compile_recipe(recipe.clone()).unwrap();
    let mut square_recipe = serde_json::to_value(recipe).unwrap();
    let mut changed = 0;
    for rule in square_recipe["complete_spec"]["styles"]
        .as_array_mut()
        .unwrap()
    {
        if rule["family"] == "flowchart"
            && rule["target"] == "node"
            && rule["style"]["radius"] == 10.0
        {
            rule["style"]["radius"] = serde_json::json!(0.0);
            changed += 1;
        }
    }
    assert_eq!(
        changed, 1,
        "the public recipe must expose one family-scoped corner rule"
    );
    let square = compiler
        .compile_recipe(serde_json::from_value(square_recipe).unwrap())
        .unwrap();
    for (source, has_corners) in [
        (
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd"),
            true,
        ),
        ("flowchart LR\nA{Decision}", false),
    ] {
        let render = |theme| {
            let RenderOutput::Document(Some(document)) = Renderer::new()
                .render(
                    RenderRequest::document(source, OperationControl::new(), Default::default())
                        .with_theme(theme),
                )
                .unwrap()
            else {
                panic!("document required")
            };
            document_pixels(&document, 1.0)
        };
        let rounded = render(rounded.clone());
        let square = render(square.clone());
        assert_eq!((rounded.0, rounded.1), (square.0, square.1));
        if has_corners {
            let changed_pixels = rounded
                .2
                .chunks_exact(4)
                .zip(square.2.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                changed_pixels > 100,
                "public 10px corners must affect actual native pixels"
            );
        } else {
            assert!(
                rounded.2 == square.2,
                "corner settings must preserve Diamond pixels"
            );
        }
    }
}
