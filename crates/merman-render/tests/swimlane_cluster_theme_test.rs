#![cfg(feature = "diagram-swimlane")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use serde_json::{Value, json};

const SOURCE: &str = "swimlane-beta TD\nsubgraph Lane[Lane]\nA\nend\n";

fn render(source: &str, config: Value, theme: Option<NativeTheme>) -> String {
    with_artifact(source, config, theme, None, |artifact| {
        artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
            .svg()
            .to_owned()
    })
}

fn with_artifact<T>(
    source: &str,
    config: Value,
    theme: Option<NativeTheme>,
    canvas: Option<Color>,
    use_artifact: impl FnOnce(family::FamilyRenderArtifact<'_>) -> T,
) -> T {
    let parsed = MermaidEngine::new()
        .with_site_config(SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    let session = match canvas {
        Some(canvas) => session.with_canvas_background(canvas),
        None => session,
    };
    use_artifact(
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap(),
    )
}

fn lane<'a, 'input>(document: &'a Document<'input>) -> Node<'a, 'input> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some("Lane")
                && node.attribute("data-et") == Some("cluster")
        })
        .expect("Lane terminal group")
}

fn lane_rect<'a, 'input>(lane: Node<'a, 'input>, class: &str) -> Node<'a, 'input> {
    lane.children()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some(class))
        .expect("Lane terminal rectangle")
}

fn stylesheet<'a>(document: &'a Document<'_>) -> &'a str {
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Swimlane stylesheet")
}

#[test]
fn swimlane_neo_non_palette_lane_stroke_has_the_matching_cluster_css_owner() {
    for (variables, expected) in [
        (json!({}), "#aaaa33"),
        (
            json!({"clusterBorder": "#112233", "nodeBorder": "#445566"}),
            "#112233",
        ),
    ] {
        let svg = render(
            SOURCE,
            json!({"theme":"default", "look":"neo", "themeVariables":variables}),
            None,
        );
        let document = Document::parse(&svg).unwrap();
        let lane = lane(&document);
        assert_eq!(lane.attribute("data-look"), Some("neo"));
        assert_eq!(lane.attribute("data-color-id"), None);
        assert!(stylesheet(&document).contains(&format!(
            ".swimlane.cluster:not([data-color-id]) rect{{stroke:{expected}!important;}}"
        )));
        for class in ["swimlane-body", "swimlane-title"] {
            assert_eq!(lane_rect(lane, class).attribute("stroke"), Some(expected));
        }
    }
}

#[test]
fn swimlane_neo_palette_rules_and_authored_strokes_target_both_lane_rectangles() {
    for authored in [
        "style Lane stroke:#22c55e\n",
        "classDef source stroke:#22c55e\nclass Lane source\n",
    ] {
        for selected in ["default", "redux-color"] {
            let svg = render(
                &format!("{SOURCE}{authored}"),
                json!({"theme":selected, "look":"neo"}),
                None,
            );
            let document = Document::parse(&svg).unwrap();
            let lane = lane(&document);
            let color_slot = (selected == "redux-color").then_some("color-0");
            assert_eq!(lane.attribute("data-color-id"), color_slot);
            for class in ["swimlane-body", "swimlane-title"] {
                if let Some(slot) = color_slot {
                    assert!(stylesheet(&document).contains(&format!(
                        r#"[data-look="neo"][data-color-id="{slot}"].swimlane.cluster rect.{class}"#
                    )));
                }
                let style = lane_rect(lane, class).attribute("style").unwrap();
                assert!(
                    style.contains("stroke:#22c55e !important"),
                    "{selected}/{class}: {style}"
                );
            }
        }
    }
}

#[test]
fn native_theme_v2_swimlane_layers_container_and_title_paints_beneath_authored_stroke() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.container":"#ABCDEF","surface.emphasis":"#234567",
                         "stroke.guide":"#56789A","stroke.border":"#102030"}
            }"##,
            None,
        )
        .unwrap();
    let native = NativeTheme::new(theme);
    for (authored, has_override) in [
        ("", false),
        ("classDef source stroke:#22c55e\nclass Lane source\n", true),
    ] {
        let svg = render(
            &format!("{SOURCE}{authored}"),
            json!({"theme":"default", "look":"neo"}),
            Some(native.clone()),
        );
        let document = Document::parse(&svg).unwrap();
        let lane = lane(&document);
        let css = stylesheet(&document);
        assert!(!css.contains(".swimlane.cluster:not([data-color-id]) rect"));
        for (class, fill) in [("swimlane-body", "#ABCDEF"), ("swimlane-title", "#234567")] {
            assert!(css.contains(&format!(
                ".swimlane.cluster rect.{class}{{fill:{fill};stroke:#56789A}}"
            )));
            let style = lane_rect(lane, class).attribute("style").unwrap();
            assert_eq!(
                style.contains("stroke:#22c55e !important"),
                has_override,
                "{class}: {style}",
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_adapts_html_and_svg_ink_to_authored_fills() {
    use merman_render::theme::{Appearance, BuiltinPreset, Color};

    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let source = "swimlane-beta LR\n\
        subgraph Lane[Light custom lane]\nA[Verify Checksum] --> B[Retain authored ink]\nend\n\
        style Lane fill:#FEF9C3\nstyle A fill:#FFFBEB\nstyle B fill:#FFFBEB,color:#624D00";
    for html in [false, true] {
        let svg = render(
            source,
            json!({"layout":"shoal", "htmlLabels":html, "flowchart":{"htmlLabels":html}}),
            Some(NativeTheme::new(theme.clone())),
        );
        let document = Document::parse(&svg).unwrap();
        for (id, fill) in [("Lane", "#FEF9C3"), ("A", "#FFFBEB"), ("B", "#FFFBEB")] {
            let text = document
                .descendants()
                .find(|node| node.attribute("data-id") == Some(id))
                .unwrap()
                .descendants()
                .find(|node| {
                    if html {
                        node.has_tag_name("span") && node.attribute("class") == Some("nodeLabel")
                    } else {
                        node.has_tag_name("text")
                    }
                })
                .unwrap();
            let ink = text
                .descendants()
                .filter_map(|node| node.attribute("style"))
                .flat_map(|style| style.split(';'))
                .filter_map(|declaration| declaration.split_once(':'))
                .filter(|(property, _)| property.trim() == if html { "color" } else { "fill" })
                .filter_map(|(_, value)| {
                    Color::parse(value.trim().trim_end_matches("!important").trim()).ok()
                })
                .next_back()
                .unwrap_or_else(|| panic!("missing ink for {id} (htmlLabels={html})"));
            assert!(ink.contrast(Color::parse(fill).unwrap()) >= 4.5, "{id}");
            if id == "B" {
                assert_eq!(ink, Color::parse("#624D00").unwrap());
            }
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_translucent_lane_ink_samples_the_effective_canvas() {
    let source = format!("{SOURCE}style Lane fill:#000000,fill-opacity:0.25\n");
    let black = Color::rgb(0, 0, 0);
    let white = Color::rgb(255, 255, 255);
    for html in [false, true] {
        for (configured, canvas, adapted) in [
            ("#000000", None, true),
            ("#000000", Some(Color::TRANSPARENT), false),
            ("#000000", Some(white), false),
            ("#FFFFFF", Some(black), true),
        ] {
            let config = json!({
                "layout":"shoal", "htmlLabels":html, "flowchart":{"htmlLabels":html},
                "themeVariables":{"background":configured,"titleColor":"#333333"}
            });
            let rendered = with_artifact(&source, config, None, canvas, |artifact| {
                artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap()
            });
            let document = Document::parse(rendered.svg()).unwrap();
            let label = lane(&document)
                .descendants()
                .find(|node| {
                    if html {
                        node.has_tag_name("span") && node.attribute("class") == Some("nodeLabel")
                    } else {
                        node.has_tag_name("text")
                    }
                })
                .expect("lane title");
            let white_ink = if html {
                "color:#FFFFFF !important"
            } else {
                "fill:#FFFFFF !important"
            };
            assert_eq!(
                label.descendants().any(|node| node
                    .attribute("style")
                    .is_some_and(|style| { style.contains(white_ink) })),
                adapted,
                "htmlLabels={html}, theme canvas={configured}, host canvas={canvas:?}"
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_audit_55_keeps_all_ink_legible_on_both_lanes() {
    use kurbo::{BezPath, ParamCurve, Rect};
    use merman_render::theme::{Appearance, BuiltinPreset, Color};

    fn text<'a, 'input>(owner: Node<'a, 'input>) -> Node<'a, 'input> {
        owner
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap()
    }

    let source = "swimlane-beta LR
subgraph L1[\"Dark Custom Lane\"]
  N1[Start Job] -->|dispatch| N2[Process Batch]
end
subgraph L2[\"Light Custom Lane\"]
  N2 -->|emit| N3[Verify Checksum]
  N3 -->|reject| N1
  N3 -->|done| N4[Publish Report]
end
style L1 fill:#0F172A,stroke:#64748B
style L2 fill:#FEF9C3,stroke:#CA8A04
style N1 fill:#1E293B,stroke:#94A3B8
style N3 fill:#FFFBEB,stroke:#D97706";
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let config = json!({"layout":"shoal", "htmlLabels":false, "flowchart":{"htmlLabels":false}});
    let svg = render(
        source,
        config.clone(),
        Some(NativeTheme::new(theme.clone())),
    );
    let document = Document::parse(&svg).unwrap();
    let css = stylesheet(&document);
    let default_text = Color::parse("#E5ECF6").unwrap();
    let default_connector = Color::parse("#94A3B8").unwrap();
    assert!(css.contains("span{fill:#E5ECF6;color:#E5ECF6;"));
    assert!(css.contains(".swimlane-label text{fill:#E5ECF6}"));
    assert!(css.contains(".flowchart-link{stroke:#94A3B8;"));
    assert!(css.contains(".marker{fill:#94A3B8;stroke:#94A3B8;"));

    let paint = |node: Node<'_, '_>, property| {
        node.attribute("style")
            .into_iter()
            .flat_map(|style| style.split(';'))
            .filter_map(|declaration| declaration.split_once(':'))
            .rfind(|(name, _)| name.trim() == property)
            .map(|(_, value)| value.trim().trim_end_matches("!important").trim_end())
            .or_else(|| node.attribute(property))
            .map(|value| Color::parse(value).unwrap())
    };
    let owner = |id, kind| {
        document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == Some(id) && node.attribute("data-et") == Some(kind)
            })
            .unwrap()
    };
    let center_y = |id| {
        owner(id, "node")
            .attribute("transform")
            .and_then(|transform| transform.strip_suffix(')'))
            .and_then(|transform| transform.split_once(','))
            .unwrap()
            .1
            .trim()
            .parse::<f64>()
            .unwrap()
    };
    assert!((center_y("N1") - center_y("N2")).abs() < 1e-6);
    let mut measured = Vec::new();
    let mut record = |name: String, ink: Color, fill: Color| {
        measured.push((name, ink.contrast(fill)));
    };
    let visible_shaft = |base, point: kurbo::Point| {
        let mut path = base;
        let mut layer = Node::next_sibling_element(&base);
        while let Some(group) =
            layer.filter(|node| node.attribute("class") == Some("merman-edge-surface-ink"))
        {
            let clip_id = group
                .attribute("clip-path")
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let clip = document
                .descendants()
                .find(|node| node.has_tag_name("clipPath") && node.attribute("id") == Some(clip_id))
                .unwrap()
                .first_element_child()
                .unwrap();
            let number = |attribute| clip.attribute(attribute).unwrap().parse::<f64>().unwrap();
            let bounds = Rect::from_origin_size(
                (number("x"), number("y")),
                (number("width"), number("height")),
            );
            if bounds.contains(point) {
                path = group.first_element_child().unwrap();
            }
            layer = group.next_sibling_element();
        }
        path
    };
    let lanes = ["L1", "L2"].map(|id| {
        let group = owner(id, "cluster");
        let title_bg = paint(lane_rect(group, "swimlane-title"), "fill").unwrap();
        record(
            format!("{id} title"),
            paint(text(group), "fill").unwrap_or(default_text),
            title_bg,
        );
        let body = lane_rect(group, "swimlane-body");
        let number = |name| body.attribute(name).unwrap().parse::<f64>().unwrap();
        let bounds = Rect::from_origin_size(
            (number("x"), number("y")),
            (number("width"), number("height")),
        );
        (bounds, paint(body, "fill").unwrap())
    });
    let node = owner("N3", "node");
    let node_bg = node
        .children()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    record(
        "N3 node text".into(),
        paint(text(node), "fill").unwrap_or(default_text),
        paint(node_bg, "fill").unwrap(),
    );

    for (label, shaft_lanes, arrow_lane) in [
        ("dispatch", &[0][..], 0),
        ("emit", &[0, 1][..], 1),
        ("reject", &[0, 1][..], 0),
        ("done", &[1][..], 1),
    ] {
        let label_group = document
            .descendants()
            .find(|node| {
                node.attribute("data-et") == Some("edge-label")
                    && text(*node)
                        .descendants()
                        .any(|word| word.is_text() && word.text() == Some(label))
            })
            .unwrap();
        let plate = label_group.prev_sibling_element().unwrap();
        assert_eq!(
            plate.attribute("class"),
            Some("merman-edge-label-background")
        );
        record(
            format!("{label} label"),
            paint(text(label_group), "fill").unwrap_or(default_text),
            paint(plate, "fill").unwrap(),
        );

        let path = owner(label_group.attribute("data-id").unwrap(), "edge");
        let geometry = BezPath::from_svg(path.attribute("d").unwrap()).unwrap();
        for &lane in shaft_lanes {
            let (bounds, fill) = lanes[lane];
            let point = geometry
                .segments()
                .flat_map(|segment| [0.25, 0.5, 0.75].map(|t| segment.eval(t)))
                .find(|&point| bounds.contains(point))
                .unwrap_or_else(|| panic!("{label} must exercise lane {lane}"));
            let stroke = paint(visible_shaft(path, point), "stroke").unwrap_or(default_connector);
            record(format!("{label} shaft on lane {lane}"), stroke, fill);
        }

        let (bounds, fill) = lanes[arrow_lane];
        let end = geometry.segments().last().unwrap().end();
        assert!(bounds.contains(end));
        let path = visible_shaft(path, end);
        let stroke = paint(path, "stroke").unwrap_or(default_connector);
        let marker_id = path
            .attribute("marker-end")
            .unwrap()
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let marker = document
            .descendants()
            .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(marker_id))
            .unwrap()
            .first_element_child()
            .unwrap();
        for property in ["fill", "stroke"] {
            let ink = paint(marker, property).unwrap_or(default_connector);
            assert_eq!(
                ink, stroke,
                "{label} marker and shaft have different {property} paint"
            );
            record(format!("{label} marker {property}"), ink, fill);
        }
    }
    let failures: Vec<_> = measured.iter().filter(|(_, ratio)| *ratio < 3.0).collect();
    assert!(failures.is_empty(), "sub-3:1 contrast: {failures:?}");

    let authored = render(
        &format!("{source}\nlinkStyle 1 stroke:#CA8A04"),
        config,
        Some(NativeTheme::new(theme)),
    );
    let document = Document::parse(&authored).unwrap();
    let edge = document
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("edge")
                && node.attribute("data-id") == Some("L_N2_N3_0")
        })
        .unwrap();
    assert_eq!(
        paint(edge, "stroke"),
        Some(Color::parse("#CA8A04").unwrap())
    );
    let style = edge.attribute("style").unwrap();
    assert_eq!(style.matches("stroke:#CA8A04").count(), 1);
    assert!(!style.contains(";;;"));
    assert!(
        edge.attribute("marker-end")
            .unwrap()
            .to_ascii_lowercase()
            .contains("ca8a04")
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_rough_edge_label_reseats_clear_of_foreign_ink() {
    use kurbo::{BezPath, Line, ParamCurve, Point, Rect};
    use merman_render::model::SwimlaneLayout;

    let source = "swimlane-beta RL
subgraph One
A --> B --> C
end
subgraph Two
D --> E --> F
end
A edge0@-->|Next| C
C edge1@--> D
D edge3@-->|Pass| C
E edge4@-->|Prepare audit<br/>and archive<br/>all records| B
C edge5@-->|Next| E
C edge6@--> B
B edge7@-->|Pass| E
B[Batch event for data processor]
E[Dispatch the result before validation]";
    let sketch = ThemeCompiler::new()
        .compile_json(
            br#"{"schema_version":2,"extends":"studio","form":{"edge_style":"sketch"}}"#,
            None,
        )
        .unwrap();
    for (look, theme) in [
        ("handDrawn", None),
        ("classic", Some(NativeTheme::new(sketch))),
    ] {
        let config = json!({
            "layout":"shoal", "theme":"forest", "look":look, "handDrawnSeed":7,
            "htmlLabels":false, "flowchart":{"nodeSpacing":8,"rankSpacing":50,"htmlLabels":false}
        });
        let (svg, layout) = with_artifact(source, config, theme, None, |artifact| {
            let layout: SwimlaneLayout = serde_json::from_value(
                artifact.layout_json().unwrap()["layout"]["SwimlaneDiagram"].clone(),
            )
            .unwrap();
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap()
                .svg()
                .to_owned();
            (svg, layout)
        });
        let doc = Document::parse(&svg).unwrap();
        let element = |id, kind| {
            doc.descendants()
                .find(|node| {
                    node.attribute("data-id") == Some(id) && node.attribute("data-et") == Some(kind)
                })
                .unwrap()
        };
        let host = element("edge4", "edge");
        let d = host.attribute("d").unwrap();
        assert!(d.contains('C') && !d.contains('L'), "{look}: {d}");
        let foreign = element("edge5", "edge");
        let shaft = BezPath::from_svg(foreign.attribute("d").unwrap()).unwrap();
        let plate = element("edge4", "edge-label")
            .prev_sibling_element()
            .unwrap();
        assert_eq!(
            plate.attribute("class"),
            Some("merman-edge-label-background")
        );
        let number = |name| plate.attribute(name).unwrap().parse::<f64>().unwrap();
        let painted = Rect::from_origin_size(
            (number("x"), number("y")),
            (number("width"), number("height")),
        );
        let intersects = |rect: Rect| {
            let corners = [
                rect.origin(),
                Point::new(rect.x1, rect.y0),
                Point::new(rect.x1, rect.y1),
                Point::new(rect.x0, rect.y1),
            ];
            shaft.segments().any(|segment| {
                [segment.start(), segment.end()]
                    .iter()
                    .any(|&point| rect.contains(point))
                    || (0..4).any(|i| {
                        !segment
                            .intersect_line(Line::new(corners[i], corners[(i + 1) % 4]))
                            .is_empty()
                    })
            })
        };
        assert!(
            !intersects(painted),
            "{look}: label crosses its neighboring edge"
        );
        if look == "handDrawn" {
            let label_id = layout
                .edges
                .iter()
                .find(|edge| edge.id == "edge4")
                .and_then(|edge| edge.label_node_id.as_deref())
                .unwrap();
            let label = layout
                .nodes
                .iter()
                .find(|node| node.id == label_id)
                .unwrap();
            let unseated = Rect::from_center_size((label.x, label.y), painted.size());
            assert!(
                intersects(unseated),
                "the layout anchor would overlap painted ink"
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_forest_draws_a_measured_svg_edge_label_backplate() {
    let svg = render(
        "swimlane-beta TB\nsubgraph Lane\nA verify@-->|Verify checksum| B\nend",
        json!({"layout":"shoal", "theme":"forest", "htmlLabels":false,
            "flowchart":{"htmlLabels":false}}),
        None,
    );
    let document = Document::parse(&svg).unwrap();
    let label = document
        .descendants()
        .find(|node| {
            node.attribute("data-id") == Some("verify")
                && node.attribute("data-et") == Some("edge-label")
        })
        .unwrap();
    let plate = label.prev_sibling_element().unwrap();
    assert_eq!(
        plate.attribute("class"),
        Some("merman-edge-label-background")
    );
    assert!(plate.attribute("width").unwrap().parse::<f64>().unwrap() > 40.0);
    assert!(plate.attribute("height").unwrap().parse::<f64>().unwrap() > 12.0);
    assert_eq!(plate.attribute("fill"), Some("#E8E8E8"));
    assert_eq!(plate.attribute("fill-opacity"), None);
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_swimlane_keeps_foreign_stroke_clear_of_caption_backplate() {
    use kurbo::{BezPath, ParamCurve, ParamCurveNearest, Shape};
    use merman_render::theme::{Appearance, BuiltinPreset};

    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Light))
        .unwrap();
    for direction in ["LR", "RL"] {
        let source = format!(
            "swimlane-beta {direction}\n\
            subgraph One\nA\nB\nend\n\
            subgraph Two\nC\nD\nend\n\
            A caption@-->|Caption| D\nC passing@--> B\n\
            linkStyle 1 stroke-width:64px"
        );
        let config = json!({"layout":"shoal", "htmlLabels":false,
            "flowchart":{"htmlLabels":false, "curve":"basis"}});
        let native = NativeTheme::new(theme.clone());
        let svg = render(&source, config.clone(), Some(native.clone()));
        let geometry = with_artifact(&source, config, Some(native), None, |artifact| {
            artifact.edge_geometry_json().unwrap()
        });
        let document = Document::parse(&svg).unwrap();
        let element = |kind, id| {
            document
                .descendants()
                .find(|node| {
                    node.attribute("data-et") == Some(kind) && node.attribute("data-id") == Some(id)
                })
                .unwrap()
        };
        let label = element("edge-label", "caption");
        let plate = label.prev_sibling_element().unwrap();
        assert_eq!(
            plate.attribute("class"),
            Some("merman-edge-label-background")
        );
        let number = |attribute| plate.attribute(attribute).unwrap().parse::<f64>().unwrap();
        let bounds = kurbo::Rect::from_origin_size(
            (number("x"), number("y")),
            (number("width"), number("height")),
        );
        let center = geometry
            .iter()
            .find(|edge| edge.id == "caption")
            .unwrap()
            .label_position
            .as_ref()
            .unwrap();
        assert!((bounds.center().x - center.x).abs() < 1e-6);
        assert!((bounds.center().y - center.y).abs() < 1e-6);
        let foreign = element("edge", "passing");
        assert!(
            foreign
                .ancestors()
                .all(|node| node.attribute("mask").is_none())
        );
        assert!(foreign.attribute("clip-path").is_none());
        let path = |node: Node<'_, '_>| BezPath::from_svg(node.attribute("d").unwrap()).unwrap();
        let guard = bounds.inflate(32.0, 32.0);
        assert!(
            path(foreign).segments().all(|segment| {
                !guard.contains(segment.start())
                    && !guard.contains(segment.end())
                    && guard
                        .to_path(0.0)
                        .segments()
                        .filter_map(|side| match side {
                            kurbo::PathSeg::Line(line) => Some(line),
                            _ => None,
                        })
                        .all(|side| segment.intersect_line(side).is_empty())
            }),
            "{direction}: foreign stroke enters the caption backplate",
        );
        assert!(
            path(element("edge", "caption"))
                .segments()
                .any(|segment| { segment.nearest(bounds.center(), 1e-6).distance_sq < 1e-6 })
        );
    }
}

#[test]
fn swimlane_cluster_style_preserves_literal_entity_spelling_in_css() {
    let svg = render(
        &format!(r#"{SOURCE}style Lane --custom: "R&amp;D;fill""#),
        json!({"look":"classic"}),
        None,
    );
    let document = Document::parse(&svg).unwrap();
    let lane = lane(&document);
    for class in ["swimlane-body", "swimlane-title"] {
        assert_eq!(
            lane_rect(lane, class).attribute("style"),
            Some(r#"--custom:"R&amp;D;fill" !important"#),
        );
    }
}
