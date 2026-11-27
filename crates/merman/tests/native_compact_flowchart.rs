#![cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    feature = "native-compact",
    feature = "svg"
))]

use merman::svg::{RenderResourcePolicy, ResourceLimitId, SvgOutputProfile};
use merman::theme::{BuiltinPreset, Color, ThemeCompiler, ThemeSelection};
use merman::{OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, SvgRequest};

#[path = "native_compact/paint.rs"]
mod paint;

const FLOWCHART_CONFIG: &str = r#"---
config:
  layout: dagre
  look: classic
  htmlLabels: false
  flowchart:
    htmlLabels: false
    curve: linear
    diagramPadding: 0
---
"#;

fn render(body: &str, mut request: SvgRequest) -> Result<String, RenderError> {
    let source = format!("{FLOWCHART_CONFIG}{body}");
    request.options.diagram_id = Some("compact-flow".into());
    let output = Renderer::new().render(RenderRequest::svg(
        &source,
        OperationControl::new(),
        request,
    ))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        panic!("expected standalone SVG");
    };
    Ok(svg.svg().to_owned())
}

fn compact_request() -> SvgRequest {
    SvgRequest::default().with_output_profile(SvgOutputProfile::NativeCompact)
}

fn shafts(root: roxmltree::Node<'_, '_>) -> Vec<Vec<f64>> {
    root.descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && !node.ancestors().any(|parent| parent.has_tag_name("marker"))
        })
        .map(|node| {
            let path = node.attribute("d").expect("expected a shaft path");
            assert!(path.starts_with('M') && path.contains('L'), "{path}");
            path.split(['M', 'L', ',', ' '])
                .filter(|part| !part.is_empty())
                .map(|part| part.parse().expect("expected a shaft coordinate"))
                .collect()
        })
        .collect()
}

#[test]
fn compact_flowchart_keeps_the_prepared_dagre_shafts() {
    let source = "flowchart TB\nA[Left lane] --> Z[Join here]\nB[Middle] --> Z\nC[Right] --> Z";
    let parity = render(source, SvgRequest::default()).unwrap();
    let compact = render(source, compact_request()).unwrap();
    let parity = roxmltree::Document::parse(&parity).unwrap();
    let compact = roxmltree::Document::parse(&compact).unwrap();
    let root = compact.root_element();
    assert_eq!(root.attribute("id"), Some("compact-flow"));
    assert!(!root.descendants().any(|node| {
        node.has_tag_name("foreignObject")
            || node.has_tag_name("style")
            || node.attribute("data-points").is_some()
    }));
    for label in ["Left lane", "Middle", "Right", "Join here"] {
        assert!(
            root.descendants()
                .any(|node| node.has_tag_name("text") && node.text() == Some(label))
        );
    }

    let parity_shafts = shafts(parity.root_element());
    let compact_shafts = shafts(root);
    assert_eq!(parity_shafts.len(), 3, "expected all three parity shafts");
    assert_eq!(compact_shafts.len(), parity_shafts.len());
    assert!(parity_shafts.iter().any(|shaft| shaft.len() > 4));
    for shaft in parity_shafts {
        assert!(compact_shafts.contains(&shaft), "missing shaft {shaft:?}");
    }
}

#[test]
fn compact_flowchart_keeps_the_native_label_weight() {
    let source = "flowchart LR\nA[Plan]";
    let theme = ThemeSelection::preset(BuiltinPreset::Monograph, None);
    let parity = render(
        source,
        SvgRequest::default().with_native_theme(theme.clone()),
    )
    .unwrap();
    let compact = render(source, compact_request().with_native_theme(theme)).unwrap();
    let parity = roxmltree::Document::parse(&parity).unwrap();
    let compact = roxmltree::Document::parse(&compact).unwrap();

    assert!(parity.descendants().any(|node| {
        node.has_tag_name("style")
            && node
                .text()
                .is_some_and(|css| css.contains("#compact-flow{font-weight:600;}"))
    }));
    let label = compact
        .descendants()
        .find(|node| node.has_tag_name("text") && node.text() == Some("Plan"))
        .expect("expected the compact label");
    assert_eq!(label.attribute("font-weight"), Some("600"));
}

fn assert_compact_unsupported(result: Result<String, RenderError>, expected: &str) {
    let error = result.unwrap_err();
    let RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile { profile, reason }) =
        error
    else {
        panic!("expected an unsupported output profile, got {error:?}");
    };
    assert_eq!(profile, SvgOutputProfile::NativeCompact);
    assert_eq!(reason, expected);
}

#[test]
fn compact_flowchart_admits_original_presets_on_a_transparent_canvas() {
    let source = "flowchart LR\nA[Plan] --> B[Ship]";
    for (preset, arrow_path) in [
        (BuiltinPreset::Blueprint, "M1 1 L10 5 L1 9"),
        (BuiltinPreset::Cyberpunk, "M0 1 L10 5 L0 9 L2.5 5 Z"),
    ] {
        let request = compact_request().with_native_theme(ThemeSelection::preset(preset, None));
        for request in [
            request.clone(),
            request.clone().with_background(Color::rgb(18, 52, 86)),
            request.clone().with_background(Color::rgba(18, 52, 86, 1)),
        ] {
            assert_compact_unsupported(render(source, request), "flowchart-canvas-texture");
        }

        let transparent = Color::rgba(18, 52, 86, 0);
        let theme_doc = serde_json::json!({
            "schema_version": 2,
            "extends": preset.as_str(),
            "roles": { "canvas": transparent },
        });
        let canvas_theme = ThemeCompiler::new()
            .compile_json(&serde_json::to_vec(&theme_doc).unwrap(), None)
            .unwrap();
        let flat_view_box = (preset == BuiltinPreset::Cyberpunk).then(|| {
            let mut flat_doc = theme_doc;
            flat_doc["form"] = serde_json::json!({"elevation": "flat"});
            let flat_theme = ThemeCompiler::new()
                .compile_json(&serde_json::to_vec(&flat_doc).unwrap(), None)
                .unwrap();
            let svg = render(
                source,
                compact_request().with_native_theme(ThemeSelection::compiled(flat_theme)),
            )
            .unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = document.root_element();
            assert!(
                !root
                    .children()
                    .any(|node| node.attribute("data-scene-role") == Some("depth"))
            );
            paint::view_box(root)
        });
        for request in [
            request.with_background(transparent),
            compact_request().with_native_theme(ThemeSelection::compiled(canvas_theme)),
        ] {
            let svg = render(source, request).unwrap();
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = document.root_element();
            paint::assert_transparent_scene(root);
            assert!(
                root.descendants()
                    .any(|node| node.has_tag_name("text") && node.text() == Some("Ship"))
            );
            let glyph = paint::endpoint_glyph(root);
            assert_eq!(glyph.attribute("d"), Some(arrow_path));
            let stroke = glyph
                .attribute("stroke")
                .expect("expected painted arrow outline");
            assert_ne!(stroke, "none");
            if preset == BuiltinPreset::Blueprint {
                assert_eq!(glyph.attribute("fill"), Some("none"));
                assert_eq!(glyph.attribute("stroke-linecap"), Some("round"));
                assert_eq!(glyph.attribute("stroke-linejoin"), Some("round"));
                assert!(
                    !root
                        .children()
                        .any(|node| node.attribute("data-scene-role") == Some("depth"))
                );
            } else {
                assert_eq!(glyph.attribute("fill"), Some(stroke));
                paint::assert_cyberpunk_shadows(root, 2);
                let [left, top, width, height] = paint::view_box(root);
                let [flat_left, flat_top, flat_width, flat_height] = flat_view_box.unwrap();
                assert!(left < flat_left && left + width > flat_left + flat_width);
                assert!(top + height > flat_top + flat_height);
            }
        }
    }
}

#[test]
fn compact_flowchart_transparent_canvas_rejects_blurred_elevation() {
    let source = "flowchart LR\nA[Plan] --> B[Ship]";
    let blurred_theme = ThemeCompiler::new()
        .compile_json(
            br#"{"schema_version":2,"extends":"cyberpunk","form":{"elevation":"soft"}}"#,
            None,
        )
        .unwrap();
    let request = compact_request()
        .with_native_theme(ThemeSelection::compiled(blurred_theme))
        .with_background(Color::TRANSPARENT);
    assert_compact_unsupported(render(source, request), "flowchart-native-material");
}

#[test]
fn compact_flowchart_refuses_an_edge_label_without_falling_back() {
    let source = "flowchart LR\nA[Plan] -->|Review| B[Ship]";
    let parity = render(source, SvgRequest::default()).unwrap();
    assert!(parity.contains("Review"));
    let error = render(source, compact_request()).unwrap_err();
    assert!(matches!(
        error,
        RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile {
            profile: SvgOutputProfile::NativeCompact,
            ..
        })
    ));
}

#[test]
fn compact_flowchart_refuses_interactive_nodes() {
    let source = "flowchart LR\nA[Plan] --> B[Ship]\nclick A href \"https://example.org/\"";
    let error = render(source, compact_request()).unwrap_err();
    assert!(matches!(
        error,
        RenderError::Svg(merman_render::Error::UnsupportedSvgOutputProfile {
            profile: SvgOutputProfile::NativeCompact,
            ..
        })
    ));
}

#[test]
fn compact_flowchart_checks_the_exact_svg_byte_boundary() {
    let source = "flowchart LR\nA[Plan] --> B[Ship] --- C[Done]";
    let expected = render(source, compact_request()).unwrap();
    let render_capped = |maximum| {
        let mut request = compact_request();
        request.environment = request.environment.with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                .unwrap(),
        );
        render(source, request)
    };
    assert_eq!(render_capped(expected.len()).unwrap(), expected);
    let RenderError::ResourceLimitExceeded(limit) = render_capped(expected.len() - 1).unwrap_err()
    else {
        panic!("expected the SVG byte limit");
    };
    assert_eq!(limit.id, "max_svg_bytes");
}
