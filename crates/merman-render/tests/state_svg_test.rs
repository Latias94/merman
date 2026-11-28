mod common;

use merman_core::{MermaidEngine, ParseOptions};

use merman_render::LayoutOptions;

use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
};

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_state_native_edge_styles_and_neo_open_marker_are_visible() {
    use kurbo::{BezPath, ParamCurve, PathSeg};
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    let source = "---\nconfig:\n  layout: shoal\n  look: neo\n---\nstateDiagram-v2\nA --> B\nA --> C\nB --> D\nC --> D\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    for style in ["orthogonal", "straight", "sketch"] {
        let theme = ThemeCompiler::new().compile_json(
            format!(r##"{{"schema_version":2,"extends":"blueprint","roles":{{"stroke.connector":"#123456"}},"form":{{"edge_style":"{style}","marker_style":"open"}}}}"##).as_bytes(),
            None,
        ).unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let rendered = family::prepare(
            parsed.presentation(),
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let css = doc
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap();
        assert!(css.contains(r#"[id$="-barbEnd-margin"]{stroke:#123456;"#));
        for edge in doc
            .descendants()
            .filter(|node| node.attribute("data-et") == Some("edge"))
        {
            assert_eq!(
                edge.attribute("marker-end"),
                Some("url(#merman_stateDiagram-barbEnd-margin)")
            );
            let path = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
            assert_eq!(
                path.segments()
                    .any(|segment| matches!(segment, PathSeg::Cubic(_))),
                style == "sketch"
            );
            if style == "orthogonal" {
                for segment in path.segments() {
                    let delta = segment.end() - segment.start();
                    assert!(delta.x == 0.0 || delta.y == 0.0);
                }
            }
        }
    }
}

#[test]
fn state_digit_prefixed_classes_are_css_identifiers() {
    let svg = render_state_svg_from_text(
        "stateDiagram-v2\nclassDef 2done fill:#123456\nA --> B\nclass B 2done\n",
    );
    assert!(svg.contains(r#".\32 done"#));
    assert!(svg.contains(" 2done"));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_state_centers_plain_leaf_text_in_classic_and_hand_drawn_looks() {
    for look in ["classic", "handDrawn"] {
        let svg = render_state_svg_from_text(&format!(
            "---\nconfig:\n  layout: shoal\n  look: {look}\n  htmlLabels: false\n---\nstateDiagram-v2\nIdle --> Ready\n"
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let idle = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some("Idle"))
            .unwrap();
        let label = idle
            .children()
            .find(|node| node.attribute("class") == Some("label"))
            .unwrap();
        assert!(
            label
                .attribute("transform")
                .unwrap()
                .starts_with("translate(0,")
        );
        let text = label
            .children()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("text-anchor"), Some("middle"));
        assert_eq!(
            text.first_element_child().unwrap().attribute("x"),
            Some("0")
        );
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_state_cross_boundary_route_clears_the_painted_title() {
    use kurbo::{Affine, BezPath, ParamCurve, Rect, Shape, Vec2};

    let svg = render_state_svg_from_text(
        "---\nconfig:\n  layout: shoal\n  htmlLabels: true\n---\nstateDiagram-v2\n\
         [*] --> Boot\nstate Platform {\n  state Pipeline {\n    [*] --> Ingest\n\
         Ingest --> Drain\n  }\n}\nBoot --> Ingest: cold entry\n",
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let translation = |node: roxmltree::Node<'_, '_>| {
        node.ancestors()
            .filter_map(|item| item.attribute("transform"))
            .map(|value| {
                let t = value
                    .strip_prefix("translate(")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                let (x, y) = t.split_once(',').unwrap();
                Vec2::new(x.trim().parse().unwrap(), y.trim().parse().unwrap())
            })
            .fold(Vec2::ZERO, |sum, part| sum + part)
    };
    let title = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("foreignObject")
                && node
                    .parent()
                    .is_some_and(|parent| parent.attribute("class") == Some("cluster-label"))
                && node
                    .descendants()
                    .any(|text| text.text() == Some("Pipeline"))
        })
        .unwrap();
    assert!(title.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("text-align: center;"))
    }));
    let dimension = |name| title.attribute(name).unwrap().parse::<f64>().unwrap();
    let protected = Rect::from_origin_size((0.0, 0.0), (dimension("width"), dimension("height")))
        .inflate(8.0, 0.0)
        + translation(title);
    let edge_label = doc
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.attribute("data-id").is_some()
                && node
                    .descendants()
                    .any(|text| text.text() == Some("cold entry"))
        })
        .unwrap();
    let edge = doc
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("edge")
                && node.attribute("data-id") == edge_label.attribute("data-id")
        })
        .unwrap();
    let path = Affine::translate(translation(edge))
        * BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
    assert!(path.bounding_box().y0 < protected.y0 && path.bounding_box().y1 > protected.y1);
    assert!(
        path.segments().all(|segment| {
            (0..=128).all(|step| !protected.contains(segment.eval(f64::from(step) / 128.0)))
        }),
        "transition crosses the Pipeline title band: {protected:?}"
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_state_html_labels_allow_ink_overhang_and_composites_share_a_bottom_edge() {
    let svg = render_state_svg_from_text(
        "---\nconfig:\n  layout: shoal\n  htmlLabels: true\n---\nstateDiagram-v2\n\
         state Active {\n  [*] --> Waiting\n  Waiting --> Done: timeout_exceeded\n}\n\
         Idle --> Active: start_session()\nnote right of Active: database_connection\n",
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let labels: Vec<_> = doc
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .collect();
    let dimension =
        |node: roxmltree::Node<'_, '_>, name| node.attribute(name).unwrap().parse::<f64>().unwrap();
    assert!(labels.iter().any(|node| {
        node.descendants()
            .any(|child| child.text() == Some("timeout_exceeded"))
    }));
    for label in labels {
        let style = label.attribute("style").unwrap_or_default();
        assert_eq!(
            style.contains("overflow: visible;"),
            dimension(label, "width") > 0.0 && dimension(label, "height") > 0.0,
            "{label:?}"
        );
    }

    let cluster = doc
        .descendants()
        .find(|node| node.attribute("data-id") == Some("Active"))
        .unwrap();
    let bottom = |class| {
        let rect = cluster
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some(class))
            .unwrap();
        dimension(rect, "y") + dimension(rect, "height")
    };
    assert!((bottom("outer") - bottom("inner")).abs() < 1e-6);
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_horizontal_state_transitions_arrive_on_opposite_faces_of_one_end() {
    for direction in ["LR", "RL"] {
        let svg = render_state_svg_from_text(&format!(
            "---\nconfig:\n  layout: shoal\n---\nstateDiagram-v2\ndirection {direction}\n\
             [*] --> A\nA --> B\nA --> C\nB --> [*]\nC --> [*]\n"
        ));
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let end = doc
            .descendants()
            .find(|node| node.attribute("data-id") == Some("root_end"))
            .unwrap();
        let center: svgtypes::Transform = end.attribute("transform").unwrap().parse().unwrap();
        let mut contacts = ["edge3", "edge4"].map(|id| {
            let points = state_edge_data_points(&svg, id);
            let [.., lead, contact] = points.as_slice() else {
                panic!("expected a terminal lead for {id}");
            };
            assert!((contact.x - center.e).abs() < 1e-6, "{direction} {id}");
            assert!((lead.x - contact.x).abs() < 1e-6, "{direction} {id}");
            assert!((lead.y - center.f).abs() > (contact.y - center.f).abs());
            contact.y - center.f
        });
        contacts.sort_by(f64::total_cmp);
        assert!((contacts[0] + 7.0).abs() < 1e-6, "{direction} {contacts:?}");
        assert!((contacts[1] - 7.0).abs() < 1e-6, "{direction} {contacts:?}");
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_state_neo_mask_matches_the_emitted_centerline_length() {
    use kurbo::{BezPath, PathEl, Shape};
    use merman_render_core::route_types::SVG_LENGTH_ACCURACY;

    let svg = render_state_svg_from_text(
        "---\nconfig:\n  layout: shoal\n  look: neo\n---\nstateDiagram-v2\n\
         direction LR\n[*] --> A\nA --> B\nA --> C\nB --> [*]\nC --> [*]\n",
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let mut curved = false;
    for edge in doc
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-et") == Some("edge"))
    {
        let path = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
        curved |= path
            .elements()
            .iter()
            .any(|element| matches!(element, PathEl::CurveTo(..)));
        let length = f64::from(path.perimeter(SVG_LENGTH_ACCURACY) as f32);
        let mask_length: f64 = edge
            .attribute("style")
            .unwrap()
            .strip_prefix("stroke-dasharray: 0 0 ")
            .unwrap()
            .split_once(" 0;")
            .unwrap()
            .0
            .parse()
            .unwrap();
        assert_eq!(mask_length, length);
    }
    assert!(curved);
}

// These fixtures exercise the classic terminal contract; Mermaid 12 defaults are tested separately.
fn classic_engine() -> MermaidEngine {
    MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "theme": "default",
            "look": "classic",
            "layout": "dagre"
        })),
    )
}

fn state_edge_data_points(svg: &str, edge_id: &str) -> Vec<merman_render::model::LayoutPoint> {
    use base64::Engine as _;

    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let encoded = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some(edge_id))
        .and_then(|node| node.attribute("data-points"))
        .unwrap_or_else(|| panic!("missing data-points for State edge {edge_id}"));
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .expect("base64 State edge points");
    serde_json::from_slice(&decoded).expect("JSON State edge points")
}

fn state_edge_label_position(svg: &str, edge_id: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid State SVG XML");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some(edge_id)
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .unwrap_or_else(|| panic!("missing State edge label {edge_id}"));
    let transform = label
        .parent()
        .and_then(|node| node.attribute("transform"))
        .unwrap_or_else(|| panic!("missing outer transform for State edge label {edge_id}"));
    let components = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .and_then(|value| value.split_once(','))
        .unwrap_or_else(|| panic!("invalid State edge label transform: {transform}"));
    (
        components.0.trim().parse().expect("numeric label x"),
        components.1.trim().parse().expect("numeric label y"),
    )
}

fn assert_state_rough_path_pair(
    container: roxmltree::Node<'_, '_>,
    pair_class: Option<&str>,
    expected_fill: &str,
    expected_stroke: &str,
    expected_stroke_width: &str,
) {
    let pair = pair_class.map_or_else(
        || {
            if container
                .children()
                .filter(|node| node.has_tag_name("path"))
                .count()
                == 2
            {
                container
            } else {
                container
                    .children()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node
                                .children()
                                .filter(|child| child.has_tag_name("path"))
                                .count()
                                == 2
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "missing direct State rough path pair under id={:?} class={:?}",
                            container.attribute("id"),
                            container.attribute("class")
                        )
                    })
            }
        },
        |class| {
            container
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|candidate| candidate == class)
                        })
                })
                .unwrap_or_else(|| {
                    panic!(
                        "missing State rough path pair {class} under id={:?} class={:?}",
                        container.attribute("id"),
                        container.attribute("class")
                    )
                })
        },
    );
    let paths = pair
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    let [fill_path, stroke_path] = paths.as_slice() else {
        panic!("expected exactly two direct State rough paths under {pair_class:?}");
    };

    assert_eq!(fill_path.attribute("fill"), Some(expected_fill));
    assert_eq!(fill_path.attribute("stroke"), Some("none"));
    assert_eq!(stroke_path.attribute("fill"), Some("none"));
    assert_eq!(stroke_path.attribute("stroke"), Some(expected_stroke));
    assert_eq!(
        stroke_path.attribute("stroke-width"),
        Some(expected_stroke_width)
    );
}

fn render_state_svg_from_text(text: &str) -> String {
    render_state_svg_from_text_with_engine(classic_engine(), text)
}

#[test]
fn state_svg_composite_token_ids_preserve_groups_and_nested_starts() {
    for html_labels in [false, true] {
        for (declaration, id, label) in [
            ("state A-B", "A-B", "A-B"),
            ("state A:B", "A:B", "A:B"),
            ("state 外部-状態:組", "外部-状態:組", "外部-状態:組"),
            ("state \"Container\" as Box:x:y", "Box:x:y", "Container"),
        ] {
            let source = format!(
                "stateDiagram-v2\nstate Outer {{\n{declaration} {{\n[*] --> Child\n}}\n}}\n"
            );
            let engine = classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "htmlLabels": html_labels
                    }),
                ),
            );
            let svg = render_state_svg_from_text_with_engine(engine, &source);
            let document = roxmltree::Document::parse(&svg).expect("valid State SVG");
            let group = document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(id))
                .unwrap_or_else(|| panic!("missing composite {id}: {svg}"));
            let title = group
                .children()
                .find(|node| node.attribute("class") == Some("cluster-label"))
                .expect("composite title");
            assert!(
                title.descendants().any(|node| {
                    node.is_text() && node.text().is_some_and(|text| text.trim() == label)
                }),
                "missing composite label {label}: {svg}"
            );
            let start_dom_id = format!("-state-{id}_start-");
            let start = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("id")
                            .is_some_and(|value| value.contains(&start_dom_id))
                })
                .unwrap_or_else(|| panic!("missing nested start for {id}: {svg}"));
            assert!(start.descendants().any(|node| {
                node.has_tag_name("circle") && node.attribute("class") == Some("state-start")
            }));
            assert!(document.descendants().any(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|value| value.contains("-state-Child-"))
            }));
            assert!(document.descendants().any(|node| {
                node.has_tag_name("path") && node.attribute("data-id") == Some("edge0")
            }));
        }
    }
}

#[test]
fn state_svg_plain_alias_discards_later_colon_segments() {
    for html_labels in [false, true] {
        for (tail, expected) in [
            ("A:x:y", &["L", "x"][..]),
            ("A:x:y:z", &["L", "x"][..]),
            ("A", &["L"][..]),
        ] {
            let source = format!("stateDiagram-v2\nstate \"L\" as {tail}\n");
            let engine = classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "htmlLabels": html_labels
                    }),
                ),
            );
            let svg = render_state_svg_from_text_with_engine(engine, &source);
            let document = roxmltree::Document::parse(&svg).expect("valid State SVG");
            let state = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("id")
                            .is_some_and(|value| value.contains("-state-A-"))
                })
                .unwrap_or_else(|| panic!("missing plain alias A: {svg}"));
            let text: Vec<_> = state
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect();
            assert_eq!(
                text.as_slice(),
                expected,
                "{tail}, htmlLabels={html_labels}: {svg}"
            );
        }
    }
}

#[test]
fn state_invalid_later_source_width_preserves_the_emitted_class_winner() {
    let svg = render_state_svg_from_text(
        "stateDiagram-v2\nstate \"Ready\" as Ready\nclassDef wide stroke-width:20px\nclass Ready wide\nstyle Ready stroke-width:99px junk\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid State SVG");
    let state = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some("merman-state-Ready-0"))
        .unwrap_or_else(|| panic!("Ready state terminal: {svg}"));
    let shape = state
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("basic label-container")
        })
        .expect("Ready state shape");
    let style = shape.attribute("style").expect("source shape style");
    assert!(
        style
            .split(';')
            .any(|declaration| declaration.trim() == "stroke-width:20px !important"),
        "the valid class width must reach the shape: {style}"
    );
    assert!(
        !style.contains("99px"),
        "invalid inline width must not reach the shape: {style}"
    );
}

fn render_state_layout_and_svg_from_text_in_environment(
    text: &str,
    environment: &RenderEnvironment,
) -> (merman_render::model::StateDiagramLayout, String) {
    let session = environment.begin_session().expect("begin State session");
    let parsed = classic_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse State source")
        .expect("detect State diagram")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare State artifact");
    let projection = artifact.layout_json().expect("State layout projection");
    let layout = serde_json::from_value(projection["layout"]["StateDiagramV2"].clone())
        .expect("typed State layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State artifact")
        .svg()
        .to_string();
    (layout, svg)
}

fn render_state_svg_from_text_with_engine(engine: MermaidEngine, text: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare State artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State artifact")
        .svg()
        .to_string()
}

fn try_render_state_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin State resource-bound session");
    let parsed = classic_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse State resource-bound fixture")
        .expect("detect State resource-bound fixture")
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn state_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> Done: advance\nDone --> [*]\n";
    let baseline = try_render_state_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded State baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "State fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact State SVG byte ceiling");
    let exact = try_render_state_svg_with_resource_policy(source, exact_policy)
        .expect("the exact State family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact State SVG byte ceiling");
    let error = try_render_state_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the State family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected State MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn native_state_svg_mixed_emphasis_retains_node_and_edge_runs() {
    for (label, expected_rows) in [
        ("plain **bold** *italic*", 1),
        ("plain **bold** *italic*<br/>plain **bold** *italic*", 2),
    ] {
        let svg = render_state_svg_from_text(&format!(
            "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nstate \"{label}\" as A\nA --> B: {label}\n"
        ));
        let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
        for (owner, attribute, value) in [
            ("node", "id", "merman-state-A-0"),
            ("edge", "data-id", "edge0"),
        ] {
            let terminal = document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute(attribute) == Some(value))
                .unwrap_or_else(|| panic!("missing {owner} terminal: {svg}"));
            let rows = terminal
                .descendants()
                .filter(|node| node.attribute("class") == Some("text-outer-tspan row"))
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), expected_rows, "{owner}: {svg}");
            for row in rows {
                let runs = row
                    .children()
                    .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
                    .map(|node| {
                        (
                            node.text().unwrap_or_default().trim(),
                            node.attribute("font-weight"),
                            node.attribute("font-style"),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    runs,
                    [
                        ("plain", Some("normal"), Some("normal")),
                        ("bold", Some("bold"), Some("normal")),
                        ("italic", Some("normal"), Some("italic")),
                    ],
                    "{owner}: {svg}"
                );
            }
        }
        assert!(!svg.contains("<foreignObject"), "{svg}");
    }

    let svg = render_state_svg_from_text(
        "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nstate \"**bold** *italic*\" as Parent {\nA\n}\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid State SVG");
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("cluster-label"))
        .expect("plain cluster title");
    let word = title
        .descendants()
        .find(|node| node.attribute("class") == Some("text-inner-tspan"))
        .expect("literal cluster title text");
    assert_eq!(word.text(), Some("**bold** *italic*"), "{svg}");
    assert_eq!(word.attribute("font-weight"), Some("normal"), "{svg}");
    assert_eq!(word.attribute("font-style"), Some("normal"), "{svg}");
}

struct StateEmphasisTextMeasurer {
    fallback: std::sync::Arc<dyn TextMeasurer + Send + Sync>,
}

impl Default for StateEmphasisTextMeasurer {
    fn default() -> Self {
        Self {
            fallback: DeterministicTextMeasurer::default().with_width_callback(Self::width),
        }
    }
}

impl StateEmphasisTextMeasurer {
    fn width(text: &str, style: &TextStyle) -> f64 {
        let bold = matches!(style.font_weight.as_deref(), Some("700" | "bold"));
        let italic = style.font_style.as_deref() == Some("italic");
        text.chars().count() as f64
            * (4.0 + if bold { 4.0 } else { 0.0 } + if italic { 2.0 } else { 0.0 })
    }
}

impl TextMeasurer for StateEmphasisTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.fallback.measure(text, style)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.fallback
            .measure_wrapped(text, style, max_width, wrap_mode)
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        Self::width(text, style)
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        let half = Self::width(text, style) / 2.0;
        match (text.trim_start(), style.font_style.as_deref()) {
            ("Tilt", Some("italic")) => (half, half + 10.0),
            ("Lean", Some("italic")) => (half + 7.0, half),
            _ => (half, half),
        }
    }
}

fn state_emphasis_environment() -> RenderEnvironment {
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.state-emphasis").expect("valid profile id"),
        "test",
    )
    .expect("valid profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(TextMeasurementPolicy::uniform(
        TextMeasurementProfile::new(
            identity,
            std::sync::Arc::new(StateEmphasisTextMeasurer::default()),
        ),
    ))
}

#[test]
fn native_state_markdown_rows_keep_fixed_stride_with_face_dependent_bboxes() {
    struct FaceBboxMeasurer;

    impl TextMeasurer for FaceBboxMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: self.measure_svg_text_computed_length_px(text, style),
                height: self.measure_svg_tspan_text_bbox_height_px(text, style),
                line_count: 1,
            }
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
            text.chars().count() as f64 * 5.0
        }

        fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
            let half = self.measure_svg_text_computed_length_px(text, style) / 2.0;
            (half, half)
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            if text.trim().is_empty() {
                0.0
            } else if style.font_style.as_deref() == Some("italic") {
                18.0
            } else if matches!(style.font_weight.as_deref(), Some("bold" | "700")) {
                14.0
            } else {
                10.0
            }
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_style.as_deref() == Some("italic") {
                -2.0
            } else if matches!(style.font_weight.as_deref(), Some("bold" | "700")) {
                -6.0
            } else {
                -4.0
            }
        }
    }

    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.state-face-bbox-stride").expect("valid profile id"),
        "test",
    )
    .expect("valid profile identity");
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            std::sync::Arc::new(FaceBboxMeasurer),
        )),
    );
    for (label, source_style, expected_rows, node_ink_height, edge_ink_height) in [
        ("A<br/>*ii*", "", vec!["A", "ii"], 37.6, 37.6),
        ("*ii*<br/>A", "", vec!["ii", "A"], 25.6, 25.6),
        ("A **B**<br/>*ii*", "", vec!["A B", "ii"], 39.6, 39.6),
        ("**B**<br/>*ii*<br/>A", "", vec!["B", "ii", "A"], 47.2, 47.2),
        ("*ii*", "", vec!["ii"], 18.0, 18.0),
        ("A **B** *ii*", "", vec!["A B ii"], 22.0, 22.0),
        ("A<br/>B", "", vec!["A", "B"], 27.6, 27.6),
        (
            "A<br/>*ii*",
            "style A font-weight:700,font-style:italic\n",
            vec!["A", "ii"],
            35.6,
            37.6,
        ),
    ] {
        let (layout, svg) = render_state_layout_and_svg_from_text_in_environment(
            &format!(
                "%%{{init: {{\"htmlLabels\": false, \"themeVariables\": {{\"fontSize\": \"16px\"}}, \"state\": {{\"minNodeWidth\": 0}}}}}}%%\nstateDiagram-v2\nstate \"{label}\" as A\nA --> B: {label}\n{source_style}"
            ),
            &environment,
        );
        let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
        let layout_node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let layout_edge = layout.edges[0].label.as_ref().unwrap();
        for (attribute, value, shape_class, padding, ink_height, layout_height) in [
            (
                "id",
                "merman-state-A-0",
                "basic label-container",
                16.0,
                node_ink_height,
                layout_node.height,
            ),
            (
                "data-id",
                "edge0",
                "background",
                4.0,
                edge_ink_height,
                layout_edge.height,
            ),
        ] {
            let terminal = document
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute(attribute) == Some(value))
                .expect("State label terminal");
            let rows = terminal
                .descendants()
                .filter(|node| node.attribute("class") == Some("text-outer-tspan row"))
                .collect::<Vec<_>>();
            let visible_rows = rows
                .iter()
                .map(|row| {
                    row.descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>()
                })
                .collect::<Vec<_>>();
            assert_eq!(visible_rows, expected_rows, "{value}/{label}: {svg}");
            let baselines = rows
                .iter()
                .map(|row| row.attribute("y").unwrap().parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            for pair in baselines.windows(2) {
                assert!(
                    (pair[1] - pair[0] - 17.6).abs() <= 1e-9,
                    "{value}/{label}: face bboxes must not alter the 1.1em stride: {svg}"
                );
            }
            let shape = terminal
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some(shape_class)
                })
                .expect("State label shape");
            let shape_height = shape.attribute("height").unwrap().parse::<f64>().unwrap();
            let expected_height = ink_height + padding;
            assert!(
                (layout_height - expected_height).abs() <= 1e-9,
                "{value}/{label}: layout height must enclose the face union once: {svg}"
            );
            assert!(
                (shape_height - expected_height).abs() <= 1e-9,
                "{value}/{label}: SVG height must match the layout: {svg}"
            );
        }
    }
}

#[test]
fn native_state_svg_code_spans_use_the_svg_projection_even_when_empty() {
    for label in ["ASCII ``omitted``", "``omitted``"] {
        let svg = render_state_svg_from_text(&format!(
            "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nstate \"{label}\" as A\n"
        ));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-state-A-0"))
            .unwrap();
        let text = node
            .descendants()
            .filter(|node| node.attribute("class") == Some("text-inner-tspan"))
            .flat_map(|node| node.text())
            .collect::<String>();
        assert_eq!(
            text,
            if label.starts_with("ASCII") {
                "ASCII"
            } else {
                ""
            }
        );
    }
}

#[test]
fn native_state_node_styled_start_anchor_uses_run_ink_union() {
    for (label, expected_rows, ink_left, ink_right) in [
        ("**Skew** *Tilt*", 1, 0.0, 72.0),
        ("**WiderWord**<br/>**Skew** *Tilt*", 2, 0.0, 72.0),
        ("*Lean* **Skew**", 1, -7.0, 64.0),
        ("*Lean* **Skew**<br/>**WiderWord**", 2, -7.0, 72.0),
    ] {
        let (layout, svg) = render_state_layout_and_svg_from_text_in_environment(
            &format!(
                "%%{{init: {{\"htmlLabels\": false, \"state\": {{\"minNodeWidth\": 0}}}}}}%%\nstateDiagram-v2\nstate \"{label}\" as A\n"
            ),
            &state_emphasis_environment(),
        );
        let layout_node = layout
            .nodes
            .iter()
            .find(|node| node.id == "A")
            .expect("State node layout");
        let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-state-A-0"))
            .expect("State SVG node");
        let shape = node
            .descendants()
            .find(|node| node.attribute("class") == Some("basic label-container"))
            .expect("State node shape");
        let text = node
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .expect("native State node text");
        // Start-anchored rows share their origin, not their individual advance centers.
        // Keep the existing 8px shape padding on each side of the measured ink union.
        let ink_width = ink_right - ink_left;
        let shape_width = shape
            .attribute("width")
            .expect("State node width")
            .parse::<f64>()
            .expect("numeric State node width");
        assert_eq!(layout_node.width, ink_width + 16.0, "{label}: {svg}");
        assert_eq!(shape_width, layout_node.width, "{label}: {svg}");
        assert_eq!(text.attribute("text-anchor"), None, "{svg}");
        let label_group = text.parent().expect("State text label group");
        let label_x = label_group
            .attribute("transform")
            .and_then(|value| value.strip_prefix("translate("))
            .and_then(|value| value.split_once(','))
            .expect("translated State node text")
            .0
            .parse::<f64>()
            .expect("numeric State node text x");
        assert_eq!(label_x, -ink_width / 2.0, "{label}: {svg}");
        let rows = text
            .children()
            .filter(|node| node.has_tag_name("tspan"))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), expected_rows, "{label}: {svg}");
        for row in rows {
            let baseline_x = row
                .attribute("x")
                .expect("State row baseline x")
                .parse::<f64>()
                .expect("numeric State row baseline x");
            assert_eq!(baseline_x, -ink_left, "{label}: {svg}");
            assert_eq!(
                label_x + baseline_x + ink_left,
                -shape_width / 2.0 + 8.0,
                "{label}: {svg}"
            );
            assert_eq!(
                label_x + baseline_x + ink_right,
                shape_width / 2.0 - 8.0,
                "{label}: {svg}"
            );
        }
    }
}

#[test]
fn native_state_transition_styled_bbox_preserves_symmetric_padding() {
    let (layout, svg) = render_state_layout_and_svg_from_text_in_environment(
        "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> B: **Skew** *Tilt*",
        &state_emphasis_environment(),
    );
    let layout_label = layout.edges[0].label.as_ref().expect("edge layout label");
    let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
    let label = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some("edge0") && node.has_tag_name("g"))
        .expect("edge SVG label");
    let background = label
        .descendants()
        .find(|node| node.attribute("class") == Some("background"))
        .expect("edge label background");
    // The italic tspan owns its leading space: advances are 32 + 30, with 10px right overhang.
    // Centering the 62px row requires a 41px half-width, plus the existing 2px padding.
    assert_eq!(layout_label.width, 86.0, "{svg}");
    assert_eq!(background.attribute("width"), Some("86"), "{svg}");
    let label_x = label
        .attribute("transform")
        .and_then(|value| value.strip_prefix("translate("))
        .and_then(|value| value.split_once(','))
        .expect("translated edge label")
        .0
        .parse::<f64>()
        .expect("numeric edge label x");
    assert_eq!(label_x, -43.0, "{svg}");
}

#[test]
fn native_state_transition_asymmetric_bbox_shares_text_background_and_layout_width() {
    #[derive(Default)]
    struct AsymmetricStateTextMeasurer {
        fallback: DeterministicTextMeasurer,
    }

    impl TextMeasurer for AsymmetricStateTextMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            if text == "Skew" {
                let mut metrics = self.fallback.measure(text, style);
                metrics.width = 10.0;
                metrics
            } else {
                self.fallback.measure(text, style)
            }
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            if text == "Skew" {
                self.measure(text, style)
            } else {
                self.fallback
                    .measure_wrapped(text, style, max_width, wrap_mode)
            }
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
            if text == "Skew" {
                10.0
            } else {
                self.fallback
                    .measure_svg_text_computed_length_px(text, style)
            }
        }

        fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
            if text == "Skew" {
                (0.0, 20.0)
            } else {
                self.fallback.measure_svg_text_bbox_x(text, style)
            }
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.fallback
                .measure_svg_create_text_bbox_y_offset_px(text, style)
        }
    }

    fn parse_translate(value: &str) -> (f64, f64) {
        value
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric translate x"),
                    y.trim().parse::<f64>().expect("numeric translate y"),
                )
            })
            .unwrap_or_else(|| panic!("two-component translate: {value}"))
    }

    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.state-asymmetric-bbox").expect("valid profile id"),
        "test",
    )
    .expect("valid profile identity");
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            identity,
            std::sync::Arc::new(AsymmetricStateTextMeasurer::default()),
        )),
    );
    let (layout, svg) = render_state_layout_and_svg_from_text_in_environment(
        "%%{init: {\"htmlLabels\": false}}%%\nstateDiagram-v2\nA --> B: Skew",
        &environment,
    );
    let layout_label = layout
        .edges
        .iter()
        .find(|edge| edge.id == "edge0")
        .and_then(|edge| edge.label.as_ref())
        .expect("State edge layout label");
    let document = roxmltree::Document::parse(&svg).expect("valid State SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("edge0")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|class| class == "label"))
        })
        .expect("State edge SVG label");
    let background = label
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .expect("State edge label background");
    let text = label
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .expect("State edge label text");
    let width = background
        .attribute("width")
        .expect("background width")
        .parse::<f64>()
        .expect("numeric background width");
    let (label_x, _) = parse_translate(
        label
            .attribute("transform")
            .expect("layout-sized label transform"),
    );
    let (text_x, _) = parse_translate(
        text.parent()
            .and_then(|node| node.attribute("transform"))
            .expect("centered text anchor transform"),
    );

    assert_eq!(
        text.descendants()
            .find(|node| node.has_tag_name("tspan") && node.text() == Some("Skew"))
            .and_then(|node| node.text()),
        Some("Skew"),
        "{svg}"
    );
    assert_eq!(width, 44.0, "{svg}");
    assert_eq!(layout_label.width, width, "{svg}");
    assert_eq!(label_x, -width / 2.0, "{svg}");
    assert_eq!(text_x, width / 2.0, "{svg}");
    assert!(text_x >= 2.0, "{svg}");
    assert_eq!(text_x + 20.0, width - 2.0, "{svg}");
}

#[test]
fn native_state_transition_auto_wrap_shares_tspans_background_and_layout_bounds() {
    const EDGE_WRAP_WIDTH_PX: f64 = 200.0;

    fn parse_translate(value: &str) -> (f64, f64) {
        value
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .and_then(|value| value.split_once(','))
            .map(|(x, y)| {
                (
                    x.trim().parse::<f64>().expect("numeric translate x"),
                    y.trim().parse::<f64>().expect("numeric translate y"),
                )
            })
            .unwrap_or_else(|| panic!("two-component translate: {value}"))
    }

    let long_word = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";
    let spaced_words = "MMMMMMMMMMMM MMMMMMMMMMMM";
    let cases = [
        (
            "ordinary-long-word",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> B: {long_word}"
            ),
            long_word,
            false,
            3,
        ),
        (
            "ordinary-spaced-words",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> B: {spaced_words}"
            ),
            spaced_words,
            true,
            2,
        ),
        (
            "self-loop-long-word",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> A: {long_word}"
            ),
            long_word,
            false,
            3,
        ),
        (
            "self-loop-spaced-words",
            format!(
                "%%{{init: {{\"htmlLabels\": false}}}}%%\nstateDiagram-v2\nA --> A: {spaced_words}"
            ),
            spaced_words,
            true,
            2,
        ),
    ];
    let measurer = DeterministicTextMeasurer::default();
    let text_style = TextStyle::default();

    for (case, source, label_text, contains_space, expected_lines) in cases {
        let (layout, svg) = render_state_layout_and_svg_from_text_in_environment(
            &source,
            &RenderEnvironment::deterministic(),
        );
        let layout_label = layout
            .edges
            .iter()
            .find(|edge| edge.id == "edge0")
            .and_then(|edge| edge.label.as_ref())
            .expect("State edge layout label");
        let document = roxmltree::Document::parse(&svg).expect("valid native State SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("edge0")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "label")
                    })
            })
            .unwrap_or_else(|| panic!("native State edge label for {case}: {svg}"));
        let background = label
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
            .unwrap_or_else(|| panic!("native State edge background for {case}: {svg}"));
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap_or_else(|| panic!("native State edge text for {case}: {svg}"));
        let rows = text
            .children()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "row")
                    })
            })
            .collect::<Vec<_>>();
        let emitted_lines = rows
            .iter()
            .map(|row| {
                row.descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        let width = background
            .attribute("width")
            .expect("background width")
            .parse::<f64>()
            .expect("numeric background width");
        let height = background
            .attribute("height")
            .expect("background height")
            .parse::<f64>()
            .expect("numeric background height");
        let (label_x, label_y) = parse_translate(
            label
                .attribute("transform")
                .expect("layout-sized label transform"),
        );
        let text_parent = text.parent().expect("native edge text parent");
        let (text_x, text_y) = parse_translate(
            text_parent
                .attribute("transform")
                .expect("measured text origin"),
        );
        assert!(rows.len() > 1, "{case} must auto-wrap: {svg}");
        assert_eq!(rows.len(), expected_lines, "{case}: {svg}");
        if contains_space {
            assert_eq!(emitted_lines.join(" "), label_text, "{case}: {svg}");
        } else {
            assert_eq!(emitted_lines.concat(), label_text, "{case}: {svg}");
        }
        let mut previous_baseline: Option<f64> = None;
        let mut maximum_advance = 0.0_f64;
        let mut ink_top = f64::INFINITY;
        let mut ink_bottom = f64::NEG_INFINITY;
        for (row, line) in rows.iter().zip(&emitted_lines) {
            let line_width = measurer.measure_svg_text_computed_length_px(line, &text_style);
            maximum_advance = maximum_advance.max(line_width);
            assert!(line_width <= EDGE_WRAP_WIDTH_PX, "{case}: {line}: {svg}");
            let baseline_y = row
                .attribute("y")
                .expect("native row has an explicit user-space baseline")
                .parse::<f64>()
                .expect("numeric native row baseline");
            assert!(baseline_y > 0.0 && baseline_y < height, "{case}: {svg}");
            if let Some(previous) = previous_baseline {
                assert!(
                    (baseline_y - previous - text_style.font_size * 1.1).abs() <= 1e-9,
                    "{case}: row stride must be 1.1em: {svg}"
                );
            }
            previous_baseline = Some(baseline_y);
            let top = baseline_y - text_style.font_size
                + measurer.measure_svg_create_text_bbox_y_offset_px(line, &text_style);
            ink_top = ink_top.min(top);
            ink_bottom = ink_bottom
                .max(top + measurer.measure_svg_tspan_text_bbox_height_px(line, &text_style));
            assert_eq!(row.attribute("dy"), None, "{case}: {svg}");
        }

        assert_eq!(text.attribute("y"), None, "{case}: {svg}");
        assert_eq!(text.attribute("id"), None, "{case} must stay non-prepared");
        assert!(!svg.contains("@font-face"), "{case} must stay non-prepared");
        let (ink_left, ink_right) =
            emitted_lines
                .iter()
                .fold((0.0_f64, 0.0_f64), |(maximum_left, maximum_right), line| {
                    let (left, right) = measurer.measure_svg_text_bbox_x(line, &text_style);
                    (maximum_left.max(left), maximum_right.max(right))
                });
        let expected_width = 2.0 * (ink_left.max(ink_right).max(maximum_advance / 2.0) + 2.0);
        assert!((width - expected_width).abs() <= 1e-9, "{case}: {svg}");
        assert!((ink_top - 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((ink_bottom - (height - 2.0)).abs() <= 1e-9, "{case}: {svg}");
        assert!((layout_label.width - width).abs() <= 1e-9, "{case}: {svg}");
        assert!(
            (layout_label.height - height).abs() <= 1e-9,
            "{case}: {svg}"
        );
        assert!((label_x + width / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((label_y + height / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert!((text_x - width / 2.0).abs() <= 1e-9, "{case}: {svg}");
        assert_eq!(text_y, 0.0, "{case}: {svg}");
    }
}

fn render_state_svg_with_hand_drawn_seed(seed: u64) -> String {
    let site_config = common::presentation_config(serde_json::json!({
        "look": "handDrawn",
        "handDrawnSeed": seed,
        "themeVariables": {
            "stateBkg": "#101827",
            "stateBorder": "#38bdf8",
            "nodeBorder": "#a855f7",
            "mainBkg": "#0f172a",
            "lineColor": "#e11d48",
            "strokeWidth": 4,
            "specialStateColor": "#f97316",
            "innerEndBackground": "#22c55e",
            "background": "#020617",
            "noteBkgColor": "#fef3c7",
            "noteBorderColor": "#92400e"
        }
    }));
    let source = r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
Idle --> Decide
Decide --> Fork
state Fork <<fork>>
Fork --> Join
state Join <<join>>
Join --> [*]
note right of Idle : seeded note"#;

    render_state_svg_from_text_with_engine(
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_config(
                site_config.as_config().clone(),
            ),
        ),
        source,
    )
}

#[test]
fn state_svg_invalid_default_look_falls_back_to_the_family_neo_default() {
    let source = "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n";
    let baseline = render_state_svg_from_text_with_engine(MermaidEngine::new(), source);
    let svg = render_state_svg_from_text_with_engine(
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "look": "default"
            })),
        ),
        source,
    );

    assert_eq!(svg, baseline);
    assert!(svg.contains(r#"data-look="neo""#));
    assert!(!svg.contains(r#"data-look="default""#));
}

#[test]
fn state_svg_default_min_node_width_reaches_leaf_foreign_object() {
    let svg = render_state_svg_from_text("stateDiagram-v2\nA\n");
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    let width = document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .find(|node| node.descendants().any(|child| child.text() == Some("A")))
        .and_then(|node| node.attribute("width"))
        .expect("leaf label foreignObject")
        .parse::<f64>()
        .expect("numeric leaf label width");
    assert_eq!(
        width, 120.0,
        "default state.minNodeWidth must size the label box"
    );
}

#[test]
fn state_svg_explicit_zero_min_node_width_keeps_natural_leaf_width() {
    let svg = render_state_svg_from_text(
        "%%{init: {\"state\": {\"minNodeWidth\": 0}}}%%\nstateDiagram-v2\nA\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    let width = document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .find(|node| node.descendants().any(|child| child.text() == Some("A")))
        .and_then(|node| node.attribute("width"))
        .expect("leaf label foreignObject")
        .parse::<f64>()
        .expect("numeric leaf label width");
    assert!(
        width > 0.0 && width < 120.0,
        "explicit zero must disable the default minimum: {width}"
    );
}

#[test]
fn state_svg_cross_composite_default_min_width_reaches_all_leaf_labels() {
    let source =
        include_str!("../../../fixtures/state/stress_state_cross_composite_transitions_007.mmd");
    let svg = render_state_svg_from_text(source);
    let document = roxmltree::Document::parse(&svg).expect("State SVG");

    for label in ["InnerA", "Deep", "After"] {
        let width = document
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .find(|node| node.descendants().any(|child| child.text() == Some(label)))
            .and_then(|node| node.attribute("width"))
            .expect("leaf label foreignObject")
            .parse::<f64>()
            .expect("numeric leaf label width");
        assert_eq!(width, 120.0, "default state.minNodeWidth must size {label}");
    }
}

#[test]
fn state_svg_uses_label_presence_and_source_owned_end_state_paints() {
    for look in ["classic", "neo"] {
        let engine = classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "look": look,
                "layout": "elk",
                "themeVariables": {
                    "mainBkg": "#112233",
                    "lineColor": "#223344",
                    "stateBorder": "#334455",
                    "specialStateColor": "#445566",
                    "innerEndBackground": "#556677",
                    "background": "#667788",
                    "nodeBorder": "#778899"
                }
            })),
        );
        let svg = render_state_svg_from_text_with_engine(
            engine,
            "stateDiagram-v2\n[*] --> Ready\nReady --> Done: finish\nDone --> [*]\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
            .collect();
        assert_eq!(
            labels.len(),
            if cfg!(feature = "layout-elk") { 1 } else { 3 },
            "{look}: label groups follow the selected provider's insertion contract"
        );
        let paths = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "edgePaths")
                    })
            })
            .expect("edge path group");
        if cfg!(feature = "layout-elk") {
            assert!(
                paths
                    .attribute("class")
                    .unwrap()
                    .split_whitespace()
                    .any(|class| class == "edges")
            );
        }
        for edge in paths.children().filter(|node| node.has_tag_name("path")) {
            let marker = edge.attribute("marker-end").expect("transition marker");
            assert_eq!(
                marker.ends_with("-margin)"),
                look == "neo",
                "{look}: {marker}"
            );
        }
        let end_outer = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("outer-path")
                    && node.children().any(|child| child.has_tag_name("g"))
            })
            .expect("end-state double circle");
        let outer_paints: Vec<_> = end_outer
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(outer_paints[0].attribute("fill"), Some("#112233"));
        assert_eq!(outer_paints[1].attribute("stroke"), Some("#223344"));
        let inner = end_outer
            .children()
            .find(|node| node.has_tag_name("g"))
            .unwrap();
        let inner_paints: Vec<_> = inner
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(inner_paints[0].attribute("fill"), Some("#334455"));
        assert_eq!(inner_paints[1].attribute("stroke"), Some("#334455"));
        for path in outer_paints.iter().chain(&inner_paints) {
            assert_eq!(path.attribute("style"), Some(""));
        }
        if look == "neo" {
            assert!(svg.contains(r##"[data-look="neo"].node path{stroke:#778899;"##));
        }
    }
}

#[test]
fn state_neo_rough_base_attributes_do_not_override_the_theme_stylesheet() {
    for gradient in [false, true] {
        let engine = classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "look": "neo",
                "themeVariables": {
                    "mainBkg": "#110011",
                    "nodeBorder": "#220022",
                    "stateBorder": "#440044",
                    "lineColor": "#550055",
                    "noteBkgColor": "#770077",
                    "noteBorderColor": "#880088",
                    "strokeWidth": 9,
                    "useGradient": gradient
                }
            })),
        );
        let svg = render_state_svg_from_text_with_engine(
            engine,
            r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
state Fork <<fork>>
Idle --> Decide
Decide --> Fork
Fork --> [*]
note right of Idle : terminal note
"#,
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Neo State SVG XML");
        for (id, fill, stroke, width) in [
            ("-state-Decide-", "#110011", "#220022", "1.3"),
            ("-state-Fork-", "#550055", "#550055", "1.3"),
            ("-state-root_end-", "#110011", "#550055", "2"),
            ("-state-Idle----note-", "#770077", "#880088", "1.3"),
        ] {
            let group = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("id").is_some_and(|value| value.contains(id))
                })
                .unwrap_or_else(|| panic!("missing {id}: {svg}"));
            assert_state_rough_path_pair(group, None, fill, stroke, width);
            for path in group.descendants().filter(|node| node.has_tag_name("path")) {
                assert_eq!(path.attribute("style"), Some(""), "{id}: {svg}");
            }
        }
        let stroke = if gradient {
            "url(#merman-gradient)"
        } else {
            "#220022"
        };
        assert!(
            svg.contains(&format!(
                r#"[data-look="neo"].node path{{stroke:{stroke};stroke-width:9px;}}"#
            )),
            "{svg}"
        );
    }
}

#[test]
fn state_svg_classic_look_honors_theme_css_options() {
    let engine = classic_engine().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "look": "classic",
            "themeVariables": {
                "transitionColor": "#202020",
                "lineColor": "#303030",
                "nodeBorder": "#404040",
                "stateLabelColor": "#505050",
                "mainBkg": "#606060",
                "background": "#707070",
                "altBackground": "#808080",
                "strokeWidth": 4,
                "noteBorderColor": "#909090",
                "noteBkgColor": "#a0a0a0",
                "noteTextColor": "#b0b0b0",
                "labelBackgroundColor": "#c0c0c0",
                "edgeLabelBackground": "#d0d0d0",
                "transitionLabelColor": "#e0e0e0",
                "specialStateColor": "#f0f0f0",
                "innerEndBackground": "#010101",
                "compositeBackground": "#020202",
                "stateBkg": "#030303",
                "stateBorder": "#040404",
                "compositeTitleBackground": "#050505"
            }
        })),
    );
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Active: start
Active --> [*]: done"#,
    );

    assert!(
        svg.contains(r#".marker{fill:#303030;stroke:#303030;}"#),
        "expected State base marker CSS to follow lineColor: {svg}"
    );
    assert!(
        svg.contains(r#"defs [id$="-barbEnd"]{fill:#202020;stroke:#202020;}"#),
        "expected State barbEnd marker CSS to follow transitionColor and the prefixed marker id: {svg}"
    );
    assert!(
        svg.contains(r##"#merman [id$="-dependencyStart"],#merman [id$="-dependencyEnd"]{fill:#303030;stroke:#303030;stroke-width:4;}"##),
        "expected State dependency marker CSS to use Mermaid suffix selectors: {svg}"
    );
    assert!(
        svg.contains(r#".transition{stroke:#202020;stroke-width:4;fill:none;}"#),
        "expected State transition CSS to follow transitionColor/strokeWidth: {svg}"
    );
    assert!(
        svg.contains(r#".edgeLabel .label text{fill:#e0e0e0;}"#),
        "expected State edge label CSS to follow transitionLabelColor: {svg}"
    );
    assert!(
        svg.contains(r#".node circle.state-start{fill:#f0f0f0;stroke:#f0f0f0;}"#),
        "expected State start/fork CSS to follow specialStateColor: {svg}"
    );
    assert!(
        svg.contains(r#".node rect{fill:#030303;stroke:#040404;stroke-width:4px;}"#),
        "expected State node CSS to follow stateBkg/stateBorder/strokeWidth: {svg}"
    );
    assert!(
        !svg.contains(r#"id="merman-gradient""#) && svg.contains(r#"id="merman-drop-shadow""#),
        "classic state SVG should emit classic drop-shadow defs but not gradient defs unless useGradient is set: {svg}"
    );
    assert!(
        !svg.contains(r#"markerUnits="strokeWidth""#),
        "classic state SVG should keep Mermaid's classic barb marker units: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-edge0""#)
            && svg.contains(r#"data-look="classic""#)
            && svg.contains(r#"id="merman-state-Active-1""#),
        "classic state DOM should use Mermaid scoped ids and explicit data-look: {svg}"
    );
}

#[test]
fn state_svg_neo_look_emits_neo_marker_and_cluster_theme_resources() {
    let engine = classic_engine().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "look": "neo",
            "themeVariables": {
                "transitionColor": "#202020",
                "mainBkg": "#606060",
                "stateBorder": "#040404",
                "strokeWidth": 4,
                "useGradient": true,
                "gradientStart": "#112233",
                "gradientStop": "#445566",
                "dropShadow": "url(#drop-shadow)",
                "radius": 3
            }
        })),
    );
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Active: start
state Active {
  Idle --> Busy
}"#,
    );

    assert!(
        svg.contains(r#"<defs><linearGradient id="merman-gradient""#),
        "expected neo state SVG to emit the shared gradient resource: {svg}"
    );
    assert!(
        svg.contains(r#"<filter id="merman-drop-shadow""#),
        "expected neo state SVG to emit the shared drop-shadow resource: {svg}"
    );
    assert!(
        svg.contains(r#"markerUnits="strokeWidth""#)
            && svg.contains(r#"d="M 19,7 L11,14 L13,7 L11,0 Z""#),
        "expected neo state SVG to use Mermaid's neo barb marker geometry: {svg}"
    );
    assert!(
        svg.contains(r#"marker-end="url(#merman_stateDiagram-barbEnd-margin)""#),
        "expected neo state transitions to use the margin arrowhead marker"
    );
    assert!(
        svg.contains(
            r##"[data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:url(#merman-gradient);stroke-width:4;}"##
        ),
        "expected neo state cluster CSS to reference the scoped gradient: {svg}"
    );
    assert!(
        svg.contains(
            r##"[data-look="neo"].statediagram-cluster rect.outer{rx:3px;ry:3px;filter:url(#merman-drop-shadow);}"##
        ),
        "expected neo state cluster outer rect CSS to reference the scoped drop-shadow and radius: {svg}"
    );
}

#[test]
fn state_svg_hand_drawn_seed_controls_visible_rough_paths() {
    let seed_7 = render_state_svg_with_hand_drawn_seed(7);
    let seed_7_again = render_state_svg_with_hand_drawn_seed(7);
    let seed_8 = render_state_svg_with_hand_drawn_seed(8);

    assert_eq!(
        seed_7, seed_7_again,
        "same handDrawnSeed should keep State rough SVG deterministic"
    );
    assert_ne!(
        seed_7, seed_8,
        "different handDrawnSeed should change visible State rough paths"
    );
    assert!(
        seed_7.contains(r##"fill="#0f172a""##)
            && seed_7.contains(r##"stroke="#a855f7" stroke-width="1.3""##),
        "seed test should exercise ordinary visible rough paths with Mermaid's hand-drawn mainBkg/nodeBorder/1.3 fallback: {seed_7}"
    );
    assert!(
        seed_7.contains(r##"fill="#fef3c7""##)
            && seed_7.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "seed test should exercise note rough paths as a second visible consumer: {seed_7}"
    );
}

#[test]
fn state_hand_drawn_terminals_follow_shape_specific_mermaid_fallbacks() {
    let engine = classic_engine().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "look": "handDrawn",
            "handDrawnSeed": 7,
            "htmlLabels": false,
            "themeVariables": {
                "mainBkg": "#110011",
                "nodeBorder": "#220022",
                "stateBkg": "#330033",
                "stateBorder": "#440044",
                "strokeWidth": 9,
                "lineColor": "#550055",
                "specialStateColor": "#660066",
                "noteBkgColor": "#770077",
                "noteBorderColor": "#880088",
                "compositeTitleBackground": "#990099",
                "compositeBackground": "#aa00aa",
                "altBackground": "#bb00bb",
                "background": "#cc00cc"
            }
        })),
    );
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
state Fork <<fork>>
state Join <<join>>
Idle --> Decide
Decide --> Fork
Fork --> Join
Join --> [*]
note right of Idle : terminal note
state Parent {
  [*] --> Child
}
state Outer {
  state Inner {
    [*] --> Nested
  }
}
state Parallel {
  [*] --> First
  --
  [*] --> Second
}
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid HandDrawn State SVG XML");
    let group = |id_fragment: &str| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") != Some("note-cluster")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(id_fragment))
            })
            .unwrap_or_else(|| panic!("missing State group {id_fragment}: {svg}"))
    };

    assert_state_rough_path_pair(
        group("-state-Idle-"),
        Some("outer-path"),
        "#110011",
        "#220022",
        "1.3",
    );
    assert_state_rough_path_pair(
        group("-state-root_start-"),
        Some("outer-path"),
        "#550055",
        "#550055",
        "1",
    );
    assert_state_rough_path_pair(group("-state-Decide-"), None, "#110011", "#220022", "1.3");
    assert_state_rough_path_pair(group("-state-Fork-"), None, "#550055", "#550055", "1.3");
    assert_state_rough_path_pair(group("-state-Join-"), None, "#550055", "#550055", "1.3");

    let end = group("-state-root_end-");
    assert_state_rough_path_pair(end, Some("outer-path"), "#110011", "#550055", "2");
    let end_inner = end
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.parent().is_some_and(|parent| {
                    parent.has_tag_name("g") && parent.attribute("class") == Some("outer-path")
                })
        })
        .expect("HandDrawn State end inner path pair");
    assert_state_rough_path_pair(end_inner, None, "#440044", "#440044", "2");

    let note = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "statediagram-note")
                })
        })
        .expect("HandDrawn State note group");
    assert_state_rough_path_pair(note, Some("outer-path"), "#770077", "#880088", "1.3");

    for (cluster_id, body_fill) in [("Parent", "#aa00aa"), ("Inner", "#bb00bb")] {
        let cluster = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(cluster_id))
            .unwrap_or_else(|| panic!("missing HandDrawn State cluster {cluster_id}: {svg}"));
        assert_state_rough_path_pair(cluster, Some("outer"), "#990099", "#220022", "1");
        assert_state_rough_path_pair(cluster, Some("inner"), body_fill, "#220022", "1");
    }

    let divider = group("-state-divider-id-1-");
    assert_state_rough_path_pair(divider, Some("divider"), "lightgrey", "#220022", "1");
    let divider_stroke = divider
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("divider"))
        .and_then(|pair| {
            pair.children()
                .filter(|node| node.has_tag_name("path"))
                .nth(1)
        })
        .expect("HandDrawn State divider stroke path");
    assert_eq!(divider_stroke.attribute("stroke-dasharray"), Some("5"));
}

#[test]
fn state_svg_root_html_labels_override_deprecated_flowchart_label_dom() {
    let root_false = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );
    let root_true = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
stateDiagram-v2
A --> B: owns
"#,
    );

    assert!(
        root_false.contains(r#"class="text-outer-tspan row""#)
            && root_false.contains(r#"class="text-inner-tspan""#),
        "root htmlLabels=false should render State labels as SVG text: {root_false}"
    );
    assert!(
        !root_false.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for simple State label DOM: {root_false}"
    );
    assert!(
        root_true.contains("<foreignObject")
            && root_true.contains(r#"class="nodeLabel markdown-node-label""#)
            && root_true.contains(r#"class="edgeLabel""#),
        "root htmlLabels=true should override deprecated flowchart.htmlLabels=false and keep HTML label DOM: {root_true}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_cluster_titles() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
state Parent {
  A
}
"#,
    );

    assert!(
        svg.contains(r#"class="cluster-label""#) && svg.contains(r#"class="text-outer-tspan row""#),
        "root htmlLabels=false should render State cluster titles as SVG text: {svg}"
    );
    assert!(
        !svg.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for simple State cluster DOM: {svg}"
    );
}

#[test]
fn state_composite_paint_uses_large_and_multiline_title_measurements() {
    for layout in ["dagre", "elk"] {
        if layout == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for html_labels in [true, false] {
            for title in ["Large title", "First<br/>Second"] {
                let engine = classic_engine().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({
                            "layout": layout,
                            "look": "classic",
                            "htmlLabels": html_labels,
                            "themeVariables": { "fontSize": "32px" }
                        }),
                    ),
                );
                let source = format!("stateDiagram-v2\nstate \"{title}\" as Parent {{\n A\n}}\n");
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                    .expect("parse composite")
                    .expect("State diagram")
                    .into_presentation();
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)
                    .expect("prepare composite");
                let projection = artifact.layout_json().expect("State layout projection");
                let clusters = projection["layout"]["StateDiagramV2"]["clusters"]
                    .as_array()
                    .expect("State clusters");
                let cluster = clusters
                    .iter()
                    .find(|cluster| cluster["id"] == "Parent")
                    .expect("Parent cluster");
                let title_height = cluster["title_label"]["height"].as_f64().unwrap();
                let rendered = artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .expect("render composite");
                let document = roxmltree::Document::parse(rendered.svg()).expect("State SVG");
                let group = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("data-id") == Some("Parent")
                    })
                    .expect("Parent SVG group");
                let rect = |class| {
                    group
                        .descendants()
                        .find(|node| {
                            node.has_tag_name("rect") && node.attribute("class") == Some(class)
                        })
                        .expect("composite rectangle")
                };
                let number = |node: roxmltree::Node<'_, '_>, name| {
                    node.attribute(name).unwrap().parse::<f64>().unwrap()
                };
                let outer = rect("outer");
                let inner = rect("inner");
                // clusters.js reserves the full painted bbox plus the fixed border gap.
                assert!(
                    (number(inner, "y") - number(outer, "y") - title_height - 2.0).abs() < 0.001,
                    "{layout}, html={html_labels}, title={title}"
                );
                assert!(
                    (number(outer, "height") - number(inner, "height") - title_height - 6.0).abs()
                        < 0.001,
                    "{layout}, html={html_labels}, title={title}"
                );
                let label = group
                    .children()
                    .find(|node| node.attribute("class") == Some("cluster-label"))
                    .expect("composite title");
                let (label_x, label_y) = label
                    .attribute("transform")
                    .and_then(|value| value.strip_prefix("translate("))
                    .and_then(|value| value.strip_suffix(')'))
                    .and_then(|value| value.split_once(','))
                    .map(|(x, y)| {
                        (
                            x.trim().parse::<f64>().unwrap(),
                            y.trim().parse::<f64>().unwrap(),
                        )
                    })
                    .expect("composite title origin");
                let title_width = cluster["title_label"]["width"].as_f64().unwrap();
                let header_center_x = number(outer, "x") + number(outer, "width") / 2.0;
                let header_center_y = (number(outer, "y") + number(inner, "y")) / 2.0;
                assert!(
                    (label_x + title_width / 2.0 - header_center_x).abs() < 0.001
                        && (label_y + title_height / 2.0 - header_center_y).abs() < 0.001,
                    "normalized title box must be centered in its header: {layout}, html={html_labels}, title={title}"
                );
                assert!(
                    (cluster["title_label"]["y"].as_f64().unwrap() - header_center_y).abs() < 0.001,
                    "layout and painted title boxes must agree: {layout}, html={html_labels}, title={title}"
                );
                if html_labels {
                    let foreign = label
                        .descendants()
                        .find(|node| node.has_tag_name("foreignObject"))
                        .expect("HTML title");
                    assert_eq!(number(foreign, "height"), title_height);
                    assert_eq!(
                        title_height,
                        if title.contains("<br/>") { 96.0 } else { 48.0 }
                    );
                    assert_eq!(label.descendants().filter(|node| node.has_tag_name(("http://www.w3.org/1999/xhtml", "br"))).count(),
                        usize::from(title.contains("<br/>")));
                } else {
                    assert!(
                        title_height > 24.0,
                        "32px title must exceed the old fixed height"
                    );
                    assert_eq!(
                        label
                            .descendants()
                            .filter(|node| node.attribute("class") == Some("text-outer-tspan row"))
                            .count(),
                        if title.contains("<br/>") { 2 } else { 1 }
                    );
                }
            }
        }
    }
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_notes() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A
note right of A : Note text
"#,
    );

    assert!(
        svg.contains("statediagram-note")
            && svg.contains(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">Note text</tspan>"#
            ),
        "root htmlLabels=false should render State notes as SVG text: {svg}"
    );
    assert!(
        !svg.contains(r#"<span class="nodeLabel"><p>Note text</p></span>"#),
        "root htmlLabels=false should not render State note text through HTML node labels: {svg}"
    );
}

#[test]
fn state_svg_serializes_sanitized_note_images_as_valid_xhtml() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
A
note right of A
  <a href='https://mermaid.js.org/' target='_blank'><code>note about mermaid</code></a><br/>
  <img src=x onerror=alert(1)>
end note
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let image = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "img")
        .expect("sanitized note image");
    assert_eq!(image.attribute("src"), Some("x"));
    assert_eq!(
        image.attribute("style"),
        Some("display: flex; flex-direction: column; width: 100%;")
    );
    assert!(image.attribute("onerror").is_none());
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_rect_with_title() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
Display : Ready
Display : Running
"#,
    );

    assert!(
        svg.contains(r#"title-state"#)
            && svg.contains(r#"class="text-outer-tspan row""#)
            && svg.contains("Ready")
            && svg.contains("Running"),
        "root htmlLabels=false should render State rectWithTitle labels as SVG text: {svg}"
    );
    assert!(
        !svg.contains("<foreignObject"),
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for State rectWithTitle DOM: {svg}"
    );
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_empty_edge_labels() {
    for backend in ["elk", "dagre"] {
        let engine = classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "layout": backend,
                "htmlLabels": false,
                "flowchart": {"htmlLabels": true}
            })),
        );
        let svg = render_state_svg_from_text_with_engine(engine, "stateDiagram-v2\nA --> B\n");
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
            .collect();
        if backend == "elk" && cfg!(feature = "layout-elk") {
            assert!(
                labels.is_empty(),
                "ELK only inserts groups for present labels"
            );
        } else {
            assert_eq!(
                labels.len(),
                1,
                "Dagre inserts an empty wrapper for an unlabeled edge"
            );
            let label = labels[0]
                .children()
                .find(|node| node.is_element())
                .expect("Dagre label group");
            assert_eq!(label.attribute("data-id"), Some("edge0"));
            assert_eq!(label.attribute("transform"), Some("translate(0, 0)"));
            assert!(
                !label.children().any(|node| node.is_element()),
                "empty SVG labels have no HTML child"
            );
        }
        assert!(
            !document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            "{backend}: root htmlLabels=false overrides the deprecated Flowchart setting"
        );
    }
}

#[test]
fn state_svg_root_html_labels_false_uses_svg_text_for_self_loop_edge_labels() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": true}}}%%
stateDiagram-v2
A --> A: again
"#,
    );

    assert!(
        svg.contains(r#"data-id="edge0""#)
            && svg.contains(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">again</tspan>"#
            ),
        "root htmlLabels=false should render State self-loop labels on the original edge id as SVG text: {svg}"
    );
    assert!(
        !svg.contains("cyclic-special"),
        "Mermaid 11.16 keeps cyclic-special helpers out of the public self-loop edge label DOM: {svg}"
    );
    assert_eq!(
        svg.matches("<foreignObject").count(),
        0,
        "root htmlLabels=false should override deprecated flowchart.htmlLabels=true for State self-loop label DOM: {svg}"
    );
}

#[test]
fn state_svg_leaf_self_loop_keeps_dagre_label_anchor_without_an_explicit_path_update() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
A --> A: again
"#,
    );

    let points = state_edge_data_points(&svg, "edge0");
    let (_, label_y) = state_edge_label_position(&svg, "edge0");
    let path_max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);

    assert!(
        label_y > path_max_y,
        "a leaf self-loop has no cluster cut, so Mermaid keeps its outside Dagre label anchor: {svg}"
    );
}

#[test]
fn state_svg_composite_self_loop_preserves_the_layout_label_offset_after_cluster_clipping() {
    let source = r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
state Active {
  Idle
}
Inactive --> Idle: ACT
Active --> Active: LOG
"#;
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = classic_engine()
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap()
        .into_presentation();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout: merman_render::model::StateDiagramLayout =
        serde_json::from_value(projection["layout"]["StateDiagramV2"].clone()).unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let svg = rendered.svg();

    let points = state_edge_data_points(svg, "edge1");
    assert_eq!(points.len(), 4, "expected one compact logical self-loop");
    assert!(
        points[0].x > points[1].x && points[3].x < points[2].x,
        "data-points must retain the endpoint-clipped self-loop geometry: {points:?}"
    );

    let (label_x, label_y) = state_edge_label_position(svg, "edge1");
    let edge = layout.edges.iter().find(|edge| edge.id == "edge1").unwrap();
    let anchor = edge.label.as_ref().unwrap();
    // This symmetric self-loop keeps the same route midpoint after cluster clipping.
    // Mermaid 12.1 therefore preserves the layout anchor, including its label offset.
    let expected_x = anchor.x;
    let expected_y = anchor.y;
    let midpoint_y = (points[1].y + points[2].y) / 2.0;
    assert!(
        expected_y > midpoint_y,
        "the fixture must retain a nonzero label offset"
    );
    assert!(
        (label_x - expected_x).abs() <= 1e-5 && (label_y - expected_y).abs() <= 1e-5,
        "a symmetric cluster cut must preserve the layout label offset: label=({label_x}, {label_y}), expected=({expected_x}, {expected_y})"
    );
}

#[test]
fn state_svg_direct_composite_self_loop_keeps_unclipped_dagre_endpoints() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"layout": "dagre"}}%%
stateDiagram-v2
[*] --> Active
state Active {
  [*] --> Ready
  Ready --> Ready : ping
  Ready --> Working : start
  Working --> Working : progress
  Working --> Ready : done
}
Active --> Active : re-enter
Active --> [*] : stop
"#,
    );

    let points = state_edge_data_points(&svg, "edge6");
    assert_eq!(points.len(), 4, "expected one compact composite self-loop");
    assert!(
        (points[0].x - points[1].x).abs() <= 1e-9 && (points[2].x - points[3].x).abs() <= 1e-9,
        "a direct composite endpoint has no node intersect callback, so the Dagre endpoints must remain unclipped: {points:?}"
    );
}

#[test]
fn state_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
	S1
	S2
	S3
	S4
	click S1 href "javascript:alert(1)"
	click S2 href "jav&#x61;script:alert(2)"
	click S3 href ""
	click S4 href "javascript#colon;alert(4)"
"#,
    );
    assert!(
        strict.contains(r#"<a>"#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared State link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe State click href from SVG: {strict}"
    );
    assert!(
        strict.contains(r#"xlink:href="jav&amp;&amp;x61;script:alert(2)""#),
        "expected strict mode to run Mermaid cleanup and preserve only the browser-safe literal entity spelling: {strict}"
    );
    assert!(
        strict.contains(r#"xlink:href="""#),
        "expected strict mode to preserve an empty DOM href like DOMPurify: {strict}"
    );
    assert!(!strict.contains(r#"target="_blank""#), "{strict}");
    assert!(
        !strict.contains("ﬂ°")
            && !strict.contains("¶ß")
            && !strict.contains(r#"xlink:href="javascript&colon;alert(4)""#),
        "expected strict mode to apply Mermaid cleanup before DOMPurify admission: {strict}"
    );

    let loose = render_state_svg_from_text_with_engine(
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "securityLevel": "loose"
            })),
        ),
        r#"stateDiagram-v2
S1
click S1 href "javascript:alert(1)"
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="javascript:alert(1)" target="_blank""#),
        "expected loose mode to preserve State click hrefs exactly like Mermaid's link injection path: {loose}"
    );
}

#[test]
fn state_svg_normalizes_truthy_whitespace_tooltips_after_dompurify() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
click S1 "https://example.test" "   "
"#,
    );

    assert!(svg.contains(r#"title="""#), "{svg}");
    assert!(!svg.contains(r#"title="   ""#), "{svg}");
}

#[test]
fn state_svg_treats_explicit_and_omitted_empty_tooltips_like_mermaid() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
S2
click S1 "https://example.test/one" ""
click S2 href "https://example.test/two"
"#,
    );

    let explicit = svg
        .find(r#"xlink:href="https://example.test/one""#)
        .expect("explicit-tooltip anchor");
    let explicit_tag = &svg[explicit
        ..svg[explicit..]
            .find('>')
            .map_or(svg.len(), |end| explicit + end)];
    assert!(!explicit_tag.contains(" title="), "{explicit_tag}");

    let omitted = svg
        .find(r#"xlink:href="https://example.test/two""#)
        .expect("href-form anchor");
    let omitted_tag = &svg[omitted
        ..svg[omitted..]
            .find('>')
            .map_or(svg.len(), |end| omitted + end)];
    assert!(!omitted_tag.contains(" title="), "{omitted_tag}");
}

#[test]
fn state_svg_strict_repeated_unsafe_clicks_preserve_nested_wrappers() {
    let svg = render_state_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
stateDiagram-v2
S1
click S1 "javascript:alert(1)" "JavaScript"
click S1 "data:text/html,unsafe" "Data"
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert!(inner.has_tag_name("a"), "{svg}");
    assert!(outer.has_tag_name("a"), "{svg}");
    assert_eq!(outer.attribute("title"), Some("JavaScript"));
    assert_eq!(inner.attribute("title"), Some("Data"));
    assert_eq!(node.attribute("title"), Some("Data"));
    for anchor in [outer, inner] {
        assert_eq!(
            anchor.attribute(("http://www.w3.org/1999/xlink", "href")),
            None,
            "{svg}"
        );
        assert_eq!(anchor.attribute("target"), None, "{svg}");
    }
}

#[test]
fn state_svg_loose_repeated_clicks_preserve_each_href_and_target() {
    let svg = render_state_svg_from_text_with_engine(
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "securityLevel": "loose"
            })),
        ),
        r#"stateDiagram-v2
S1
click S1 "https://example.test/first" "First"
click S1 "https://example.test/last" "Last"
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert_eq!(outer.attribute("title"), Some("First"));
    assert_eq!(inner.attribute("title"), Some("Last"));
    assert_eq!(node.attribute("title"), Some("Last"));
    assert_eq!(
        outer.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.test/first")
    );
    assert_eq!(
        inner.attribute(("http://www.w3.org/1999/xlink", "href")),
        Some("https://example.test/last")
    );
    assert_eq!(outer.attribute("target"), Some("_blank"));
    assert_eq!(inner.attribute("target"), Some("_blank"));
}

#[test]
fn state_svg_empty_later_tooltip_does_not_clear_existing_node_title() {
    let svg = render_state_svg_from_text(
        r#"stateDiagram-v2
S1
click S1 "https://example.test/first" "First"
click S1 "https://example.test/last" ""
"#,
    );

    let document = roxmltree::Document::parse(&svg).expect("valid State SVG XML");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("-state-S1-"))
        })
        .expect("S1 node");
    let inner = node.parent().expect("inner link wrapper");
    let outer = inner.parent().expect("outer link wrapper");

    assert_eq!(outer.attribute("title"), Some("First"));
    assert_eq!(inner.attribute("title"), None);
    assert_eq!(node.attribute("title"), Some("First"));
}

#[test]
fn state_svg_honors_theme_options_on_visible_rough_paths() {
    let engine = classic_engine().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": {
                "stateBkg": "#101827",
                "stateBorder": "#38bdf8",
                "nodeBorder": "#a855f7",
                "mainBkg": "#0f172a",
                "lineColor": "#e11d48",
                "strokeWidth": 4,
                "specialStateColor": "#f97316",
                "innerEndBackground": "#22c55e",
                "background": "#020617",
                "compositeBackground": "#111827",
                "noteBkgColor": "#fef3c7",
                "noteBorderColor": "#92400e"
            }
        })),
    );
    let svg = render_state_svg_from_text_with_engine(
        engine,
        r#"stateDiagram-v2
[*] --> Idle
state Decide <<choice>>
state Fork <<fork>>
state Join <<join>>
Idle --> Decide
Decide --> Fork
Fork --> Join
Join --> [*]
note right of Idle : themed note"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed State rough SVG XML");
    let special_paths = |id: &str| {
        let group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|value| value.contains(&format!("-state-{id}-")))
            })
            .unwrap_or_else(|| panic!("missing {id} State special node: {svg}"));
        let mut paths = group.descendants().filter(|node| node.has_tag_name("path"));
        (
            paths.next().expect("special-state fill path"),
            paths.next().expect("special-state stroke path"),
        )
    };
    let (choice_fill, choice_stroke) = special_paths("Decide");
    let (fork_fill, fork_stroke) = special_paths("Fork");
    let end_outer = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("outer-path")
                && node.children().any(|child| child.has_tag_name("g"))
        })
        .expect("State end outer-path group");
    let mut end_outer_paths = end_outer
        .children()
        .filter(|node| node.has_tag_name("path"));
    let end_outer_fill = end_outer_paths.next().expect("State end outer fill path");
    let end_outer_stroke = end_outer_paths.next().expect("State end outer stroke path");
    let end_inner = end_outer
        .children()
        .find(|node| node.has_tag_name("g"))
        .expect("State end inner group");
    let mut end_inner_paths = end_inner
        .children()
        .filter(|node| node.has_tag_name("path"));
    let end_inner_fill = end_inner_paths.next().expect("State end inner fill path");
    let end_inner_stroke = end_inner_paths.next().expect("State end inner stroke path");

    assert!(
        svg.contains(r##".node rect{fill:#101827;stroke:#38bdf8;stroke-width:4px;}"##),
        "classic ordinary State rects should consume stateBkg/stateBorder/strokeWidth through CSS: {svg}"
    );
    assert_eq!(choice_fill.attribute("fill"), Some("#0f172a"), "{svg}");
    assert_eq!(choice_stroke.attribute("stroke"), Some("#a855f7"), "{svg}");
    assert_eq!(fork_fill.attribute("fill"), Some("#e11d48"), "{svg}");
    assert_eq!(fork_stroke.attribute("stroke"), Some("#e11d48"), "{svg}");
    assert!(
        svg.contains(r##".node circle.state-start{fill:#f97316;stroke:#f97316;}"##),
        "start-state styling should consume specialStateColor: {svg}"
    );
    assert_eq!(end_outer_fill.attribute("fill"), Some("#0f172a"), "{svg}");
    assert_eq!(
        end_outer_stroke.attribute("stroke"),
        Some("#e11d48"),
        "{svg}"
    );
    assert_eq!(end_inner_fill.attribute("fill"), Some("#38bdf8"), "{svg}");
    assert_eq!(
        end_inner_stroke.attribute("stroke"),
        Some("#38bdf8"),
        "{svg}"
    );
    assert_ne!(end_inner_fill.attribute("fill"), Some("#22c55e"), "{svg}");
    assert_ne!(
        end_outer_stroke.attribute("stroke"),
        Some("#020617"),
        "{svg}"
    );
    assert!(
        svg.contains(r##"fill="#fef3c7""##)
            && svg.contains(r##"stroke="#92400e" stroke-width="1.3""##),
        "note rough paths should consume noteBkgColor/noteBorderColor: {svg}"
    );
}

#[test]
fn state_svg_plain_rect_radius_uses_effective_theme_for_both_label_modes() {
    // Pinned roundedRect -> drawRect keeps State's preset radius 10 for numeric
    // zero, while the truthy string "0" explicitly draws square corners.
    // Theme Neo supplies radius 3 and Redux supplies 12, independently of look.
    for (look, theme, radius, expected) in [
        ("neo", "neo", None, 3.0),
        ("neo", "redux", None, 12.0),
        ("classic", "redux", None, 12.0),
        ("neo", "default", None, 5.0),
        ("classic", "default", None, 5.0),
        ("default", "default", None, 5.0),
        ("neo", "neo", Some(serde_json::json!(0)), 10.0),
        ("classic", "default", Some(serde_json::json!(0)), 10.0),
        ("neo", "neo", Some(serde_json::json!("0")), 0.0),
        ("classic", "default", Some(serde_json::json!(7.5)), 7.5),
        ("neo", "neo", Some(serde_json::json!(7.5)), 7.5),
    ] {
        for html_labels in [true, false] {
            let mut config = serde_json::json!({
                "look": look,
                "theme": theme,
                "htmlLabels": html_labels
            });
            if let Some(radius) = radius.clone() {
                config["themeVariables"] = serde_json::json!({"radius": radius});
            }
            let engine = classic_engine().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            );
            let svg = render_state_svg_from_text_with_engine(engine, "stateDiagram-v2\nA\n");
            let document = roxmltree::Document::parse(&svg).expect("State SVG");
            let rect = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class") == Some("basic label-container")
                })
                .expect("ordinary State rectangle");
            for attr in ["rx", "ry"] {
                let actual: f64 = rect.attribute(attr).unwrap().parse().unwrap();
                assert_eq!(
                    actual, expected,
                    "look={look}, theme={theme}, radius={radius:?}, htmlLabels={html_labels}, {attr}"
                );
            }
        }
    }
}

#[test]
fn state_svg_small_terminal_shadow_uses_effective_theme_and_look() {
    // stateStart/stateEnd choose the small filter using nodeShadow, independently
    // of look=neo and dropShadow; a hand-drawn terminal never gets this override.
    for (look, theme, shadow, expected) in [
        ("neo", "neo", None, false),
        ("neo", "redux", None, true),
        ("classic", "redux", None, true),
        ("classic", "default", None, false),
        ("neo", "neo", Some(serde_json::json!(false)), false),
        ("classic", "default", Some(serde_json::json!(true)), true),
        ("handDrawn", "neo", Some(serde_json::json!(true)), false),
        ("neo", "neo", Some(serde_json::json!(0)), false),
        ("neo", "neo", Some(serde_json::json!("false")), true),
    ] {
        let mut config = serde_json::json!({"look": look, "theme": theme});
        if let Some(shadow) = shadow.clone() {
            config["themeVariables"] = serde_json::json!({"nodeShadow": shadow});
        }
        let engine = classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        );
        let svg = render_state_svg_from_text_with_engine(
            engine,
            "stateDiagram-v2\n[*] --> A\nA --> [*]\n",
        );
        let document = roxmltree::Document::parse(&svg).expect("State SVG");
        let diagram_id = document.root_element().attribute("id").expect("diagram id");
        let expected_style = format!("filter:url(#{diagram_id}-drop-shadow-small)");
        let terminals: Vec<_> = document
            .descendants()
            .filter(|node| matches!(node.attribute("class"), Some("state-start" | "outer-path")))
            .collect();
        assert_eq!(terminals.len(), 2, "look={look}, theme={theme}: {svg}");
        for terminal in terminals {
            let class = terminal.attribute("class").unwrap();
            let filter = terminal
                .attribute("style")
                .unwrap_or_default()
                .split(';')
                .map(str::trim)
                .find(|declaration| declaration.starts_with("filter:"));
            assert_eq!(
                filter,
                expected.then_some(expected_style.as_str()),
                "look={look}, theme={theme}, nodeShadow={shadow:?}, terminal={class}"
            );
        }
    }
}

#[test]
fn state_svg_min_width_updates_html_box_without_changing_wrapping() {
    for (min_width, label, wrapping_width, display, white_space, width) in [
        (120, "A", 1000, "table", "nowrap", Some("120px")),
        (0, "A", 1000, "table-cell", "nowrap", None),
        (
            120,
            "A long state label beyond the minimum width",
            1000,
            "table-cell",
            "nowrap",
            None,
        ),
        (
            120,
            "A long state label that must wrap onto multiple lines",
            120,
            "table",
            "break-spaces",
            Some("120px"),
        ),
    ] {
        for note in [false, true] {
            let config = serde_json::json!({
                "htmlLabels": true,
                "state": {"minNodeWidth": min_width, "wrappingWidth": wrapping_width}
            });
            let source = if note {
                format!("stateDiagram-v2\nN\nnote right of N : {label}\n")
            } else {
                format!("stateDiagram-v2\nstate \"{label}\" as N\n")
            };
            let svg = render_state_svg_from_text_with_engine(
                classic_engine().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(config),
                ),
                &source,
            );
            let document = roxmltree::Document::parse(&svg).expect("State SVG");
            let div = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("div")
                        && node.descendants().any(|child| child.text() == Some(label))
                })
                .expect("leaf label div");
            let style: std::collections::BTreeMap<_, _> = div
                .attribute("style")
                .unwrap()
                .split(';')
                .filter_map(|entry| entry.split_once(':'))
                .map(|(key, value)| (key.trim(), value.trim()))
                .collect();
            assert_eq!(
                style.get("display").copied(),
                Some(display),
                "note={note}, min={min_width}, label={label}"
            );
            assert_eq!(style.get("white-space").copied(), Some(white_space));
            assert_eq!(style.get("width").copied(), width);
        }
    }
    let svg = render_state_svg_from_text_with_engine(
        classic_engine().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "htmlLabels": false, "state": {"minNodeWidth": 120}
            })),
        ),
        "stateDiagram-v2\n[*] --> A\nA --> [*]\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("State SVG");
    assert!(
        !document
            .descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("state-start"))
            .count(),
        1
    );
}

#[test]
fn state_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("\u{0085}", "\u{0085}", Some("\u{0085}"), Some("\u{0085}")),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "stateDiagram-v2\naccTitle: {title}\naccDescr {{\n{description}\n}}\n[*] --> Ready\n"
        );
        let svg = render_state_svg_from_text(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}
