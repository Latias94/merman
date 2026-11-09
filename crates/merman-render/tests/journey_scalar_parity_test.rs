#![cfg(feature = "diagram-journey")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{DiagramThemeCompiler, DiagramThemeSpec};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

mod common;

fn render_journey_scalar(token: &str) -> String {
    let source = format!("journey\nsection Checkout\nPay: {token}: Customer\n");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile the default Journey theme");
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse Journey scalar source")
        .expect("detect Journey scalar source")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin a Journey render session");
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Journey scalar layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Journey scalar SVG")
        .svg()
        .to_owned()
}

#[test]
fn journey_infinite_scores_keep_signed_face_coordinates_and_mouth_shapes() {
    // Mermaid 12.3.0 svgDraw.js computes cy from the Number score and compares it with 3.
    for (token, expected_y, expected_path_prefix) in [
        ("Infinity", "-Infinity", "M7.5,0"),
        ("+Infinity", "-Infinity", "M7.5,0"),
        ("-Infinity", "Infinity", "M-7.5,0"),
        ("1e309", "-Infinity", "M7.5,0"),
        ("-1e309", "Infinity", "M-7.5,0"),
        // Finite scores can also overflow when the face's y coordinate is calculated.
        ("1e308", "-Infinity", "M7.5,0"),
        ("-1e308", "Infinity", "M-7.5,0"),
    ] {
        let svg = render_journey_scalar(token);
        let document = roxmltree::Document::parse(&svg).expect("valid Journey SVG XML");
        let face = document
            .descendants()
            .find(|node| node.attribute("class") == Some("face"))
            .expect("Journey face circle");
        assert_eq!(face.attribute("cy"), Some(expected_y), "{token}");
        let group = face.parent().expect("Journey task group");
        let eyes: Vec<_> = group
            .descendants()
            .filter(|node| node.has_tag_name("circle") && node.attribute("fill") == Some("#666"))
            .collect();
        assert_eq!(eyes.len(), 2, "{token}");
        for eye in eyes {
            assert_eq!(eye.attribute("cy"), Some(expected_y), "{token}");
        }
        let mouth = group
            .descendants()
            .find(|node| node.attribute("class") == Some("mouth"))
            .expect("Journey mouth");
        assert!(mouth.has_tag_name("path"), "{token}");
        assert!(
            mouth
                .attribute("d")
                .expect("Journey mouth arc")
                .starts_with(expected_path_prefix),
            "{token}",
        );
        assert!(
            mouth
                .attribute("transform")
                .expect("Journey mouth translation")
                .ends_with(&format!(",{expected_y})")),
            "{token}",
        );
    }
}

#[test]
fn journey_invalid_number_scores_keep_nan_faces_and_neutral_mouths() {
    for token in ["NaN", "inf", "-inf", "infinity", "invalid"] {
        let svg = render_journey_scalar(token);
        let document = roxmltree::Document::parse(&svg).expect("valid Journey SVG XML");
        let face = document
            .descendants()
            .find(|node| node.attribute("class") == Some("face"))
            .expect("Journey face circle");
        assert_eq!(face.attribute("cy"), Some("NaN"), "{token}");
        let mouth = face
            .parent()
            .expect("Journey task group")
            .descendants()
            .find(|node| node.attribute("class") == Some("mouth"))
            .expect("Journey mouth");
        assert!(mouth.has_tag_name("line"), "{token}");
        assert_eq!(mouth.attribute("y1"), Some("NaN"), "{token}");
        assert_eq!(mouth.attribute("y2"), Some("NaN"), "{token}");
    }
}

#[test]
fn journey_canonical_score_roundtrips_preserve_svg_faces_and_nan_overrides() {
    use merman_model::OperationControl;
    use merman_model::families::journey::JourneyDiagramModel;
    use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
    use merman_model::resources::{InputResourcePolicy, ResourceProfile};

    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .expect("compile the default Journey theme");
    for (token, explicit_nan, expected_y, expected_path) in [
        (
            "Infinity",
            false,
            "-Infinity",
            Some(("M7.5,0", "-Infinity")),
        ),
        (
            "-Infinity",
            false,
            "Infinity",
            Some(("M-7.5,0", "Infinity")),
        ),
        ("Infinity", true, "NaN", None),
        ("-Infinity", true, "NaN", None),
        ("NaN", false, "NaN", None),
        ("3.5", false, "345", Some(("M7.5,0", "347"))),
    ] {
        let source = format!("journey\nsection Checkout\nPay: {token}: Customer\n");
        let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .expect("parse Journey scalar source")
            .expect("detect Journey scalar source")
            .into_presentation();
        let (metadata, model, context) = parsed.into_parts();
        let RenderSemanticModel::Journey(mut model) = model else {
            panic!("expected a Journey model");
        };
        model.tasks[0].score_is_nan |= explicit_nan;
        let wire = serde_json::to_vec(&model).expect("serialize canonical Journey model");
        let restored: JourneyDiagramModel =
            serde_json::from_slice(&wire).expect("restore canonical Journey model");
        let presentation = PresentationDiagram::try_from_frozen(
            metadata,
            RenderSemanticModel::Journey(restored),
            context,
            &InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput),
            &OperationControl::new(),
        )
        .expect("private presentation construction cannot be cancelled")
        .expect("construct restored Journey presentation");
        let session = RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin a Journey render session");
        let artifact = family::prepare(
            &presentation,
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .expect("prepare restored Journey layout");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render restored Journey SVG");
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid Journey SVG XML");
        let face = document
            .descendants()
            .find(|node| node.attribute("class") == Some("face"))
            .expect("Journey face circle");
        assert_eq!(
            face.attribute("cy"),
            Some(expected_y),
            "{token}, flag={explicit_nan}"
        );
        let mouth = face
            .parent()
            .expect("Journey task group")
            .descendants()
            .find(|node| node.attribute("class") == Some("mouth"))
            .expect("Journey mouth");
        if let Some((prefix, mouth_y)) = expected_path {
            assert!(mouth.has_tag_name("path"));
            assert!(mouth.attribute("d").expect("mouth arc").starts_with(prefix));
            assert!(
                mouth
                    .attribute("transform")
                    .expect("mouth translation")
                    .ends_with(&format!(",{mouth_y})")),
            );
        } else {
            assert!(mouth.has_tag_name("line"));
            assert_eq!(mouth.attribute("y1"), Some("NaN"));
            assert_eq!(mouth.attribute("y2"), Some("NaN"));
        }
    }
}
