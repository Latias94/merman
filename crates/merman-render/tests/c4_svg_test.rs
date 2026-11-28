use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::C4DiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, TextStyle};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use std::sync::{Arc, Mutex};

#[test]
fn native_theme_v2_c4_uses_categorical_fills_and_preserves_authored_paint() {
    let source = r##"C4Component
Person(user, "User")
System(system, "System")
System_Ext(external, "External")
Container_Boundary(boundary, "Group") {
    Container(container, "Container", "Rust")
    Component(component, "Component", "Rust")
}
UpdateElementStyle(system, $bgColor="#AABBCC")
Rel(user, system, "Uses")
"##;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.node":"#DDEEFF","surface.container":"#E0E8F0",
                         "stroke.connector":"#405060"},
                "typography":{"sans":["C4 Body","sans-serif"],"base_size_px":19},
                "form":{"radius":7,"container_radius":11,"connector_width":3,
                        "edge_style":"orthogonal","marker_style":"open"}
            }"##,
            None,
        )
        .unwrap();
    let data = &theme
        .family(merman_render::DiagramFamilyId::C4.as_str())
        .data;
    let fills = [
        data.fill(0).to_string(),
        data.fill(2).to_string(),
        data.fill(3).to_string(),
    ];
    let person_ink = data.fill_ink(0).to_string();
    let prepared = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap(),
    )
    .unwrap();
    let rendered = prepared
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let shape = |kind| {
        document
            .descendants()
            .find(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_ascii_whitespace().any(|class| class == kind))
            })
            .unwrap()
    };
    for (kind, color) in [
        ("c4-person", fills[0].as_str()),
        ("c4-system", "#AABBCC"),
        ("c4-external_system", "#DDEEFF"),
        ("c4-container", fills[1].as_str()),
        ("c4-component", fills[2].as_str()),
    ] {
        assert!(
            shape(kind).descendants().any(|node| node
                .attribute("style")
                .is_some_and(|style| style.contains(&format!("fill:{color} !important")))),
            "{kind}"
        );
    }
    assert!(shape("c4-person").descendants().any(|node| {
        node.has_tag_name("text")
            && node
                .attribute("style")
                .is_some_and(|style| style.contains(&person_ink))
    }));
    let boundary = shape("c4-boundary-background");
    assert_eq!(boundary.attribute("fill"), Some("#E0E8F0"));
    assert_eq!(boundary.attribute("rx"), Some("11"));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("stroke-width") == Some("3")
            && node.attribute("stroke") == Some("#405060")
            && node
                .attribute("d")
                .is_some_and(|path| path.contains('H') && path.contains('V'))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("fill") == Some("none")
            && node.attribute("stroke") == Some("#405060")
            && node
                .parent_element()
                .is_some_and(|parent| parent.has_tag_name("marker"))
    }));
}

fn render_c4_svg_with_environment(source: &str, environment: &RenderEnvironment) -> String {
    try_render_c4_svg_with_environment(source, environment).expect("render C4 SVG")
}

fn try_render_c4_svg_with_environment(
    source: &str,
    environment: &RenderEnvironment,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = environment.begin_session().expect("begin render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

#[test]
fn c4_filled_nested_boundaries_paint_backgrounds_before_their_children() {
    let source = r##"C4Context
Boundary(outer, "Outer") {
  System(sibling, "Sibling")
  Boundary(inner, "Inner") {
    System(leaf, "Leaf")
  }
}
UpdateElementStyle(outer, $bgColor="#112233")
UpdateElementStyle(inner, $bgColor="#445566")
"##;
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    let filled_boundary = |fill| {
        document
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("fill") == Some(fill))
            .expect("filled boundary background")
    };
    let outer = filled_boundary("#112233");
    let inner = filled_boundary("#445566");
    let sibling = c4_rect_labeled(&document, "Sibling");
    let leaf = c4_rect_labeled(&document, "Leaf");
    assert!(
        outer.range().start < sibling.range().start,
        "outer fill must not cover its child"
    );
    assert!(outer.range().start < inner.range().start);
    assert!(
        inner.range().start < leaf.range().start,
        "inner fill must not cover its child"
    );
    assert!(
        leaf.range().start < c4_rect_labeled(&document, "Inner").range().start,
        "the inner outline and label remain above the child"
    );
}

#[test]
fn c4_transparent_canvas_softens_theme_boundaries_and_keeps_titles_readable() {
    use kurbo::{Point, Rect};
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer};
    use merman_render::theme::{Appearance, BuiltinPreset, Color};

    const DESCRIPTION: &str = "A long boundary description reaches past the compatibility viewport while having its own text size";
    let source = |layout| {
        format!(
            r##"---
config:
{layout}  c4:
    diagramMarginX: 0
    diagramMarginY: 0
    c4ShapeMargin: 0
---
C4Context
Deployment_Node(outer, "Theme fill", "Boundary subtitle<br/>Second row", "{DESCRIPTION}") {{
  Boundary(inner, "Authored fill") {{
    System(leaf, "Leaf")
  }}
}}
UpdateElementStyle(inner, $bgColor="#445566")
"##
        )
    };
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let environment = RenderEnvironment::deterministic();
    let measurer = DeterministicTextMeasurer::default();
    for layout in ["", "  layout: shoal\n"] {
        if !layout.is_empty() && !cfg!(feature = "layout-shoal") {
            continue;
        }
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source(layout), ParseOptions::strict())
            .unwrap()
            .unwrap();
        for (canvas, expected_opacity) in [
            (Color::TRANSPARENT, Some("0.16")),
            (Color::rgb(255, 255, 255), None),
        ] {
            let session = environment
                .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                .unwrap()
                .with_canvas_background(canvas);
            let artifact = family::prepare(
                parsed.presentation(),
                &LayoutOptions::headless_svg_defaults(),
                session,
            )
            .unwrap();
            let svg = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let document = roxmltree::Document::parse(svg.svg()).unwrap();
            let backgrounds: Vec<_> = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("c4-boundary-background"))
                .collect();
            assert_eq!(backgrounds.len(), 2);
            assert_eq!(backgrounds[0].attribute("fill-opacity"), expected_opacity);
            assert_eq!(backgrounds[1].attribute("fill"), Some("#445566"));
            assert_eq!(backgrounds[1].attribute("fill-opacity"), None);

            let mut title_backgrounds = document.descendants().filter(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class") == Some("c4-boundary-title-background")
            });
            let Some(background) = title_backgrounds.next() else {
                assert!(
                    expected_opacity.is_none(),
                    "missing transparent-canvas title backing"
                );
                continue;
            };
            assert!(expected_opacity.is_some());
            assert!(
                title_backgrounds.next().is_none(),
                "authored fill owns its title backdrop"
            );

            let number = |node: roxmltree::Node<'_, '_>, name| {
                node.attribute(name).unwrap().parse::<f64>().unwrap()
            };
            let rect = Rect::from_origin_size(
                (number(background, "x"), number(background, "y")),
                (number(background, "width"), number(background, "height")),
            );
            let [x, y, width, height]: [f64; 4] = document
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let viewport = Rect::from_origin_size((x, y), (width, height));
            assert!(viewport.inflate(1e-7, 1e-7).contains_rect(rect));
            if layout.is_empty() {
                assert!(rect.width() > number(backgrounds[0], "width"));
                assert!(rect.x0 < number(backgrounds[0], "x"));
            } else {
                let boundary_center =
                    number(backgrounds[0], "x") + number(backgrounds[0], "width") / 2.0;
                assert!((rect.center().x - boundary_center).abs() < 1e-7);
            }

            let fill = Color::parse(background.attribute("fill").unwrap()).unwrap();
            let opacity = background
                .attribute("fill-opacity")
                .map_or(1.0, |value| value.parse().unwrap());
            let layer = Color::parse(backgrounds[0].attribute("fill").unwrap()).unwrap();
            let layer_opacity = backgrounds[0]
                .attribute("fill-opacity")
                .unwrap()
                .parse()
                .unwrap();
            let title = background.parent_element().unwrap();
            for content in [
                "Theme fill",
                "[Boundary subtitle",
                "Second row]",
                DESCRIPTION,
            ] {
                let row = title
                    .descendants()
                    .find(|node| node.has_tag_name("text") && svg_text_content(*node) == content)
                    .unwrap_or_else(|| panic!("missing boundary text row {content}"));
                let x = number(row, "x");
                let y = number(row, "y") + number(row.first_element_child().unwrap(), "dy");
                let style = TextStyle {
                    font_size: c4_effective_property(row, "font-size")
                        .unwrap()
                        .trim_end_matches("px")
                        .parse()
                        .unwrap(),
                    font_family: c4_effective_property(row, "font-family"),
                    font_weight: c4_effective_property(row, "font-weight"),
                    ..Default::default()
                };
                let half_width =
                    measurer.measure_svg_tspan_text_bbox_width_px(content, &style) / 2.0;
                assert!(
                    rect.contains(Point::new(x - half_width, y))
                        && rect.contains(Point::new(x + half_width, y)),
                    "backing misses {content}"
                );
                assert!(background.range().start < row.range().start);
                let ink = Color::parse(row.attribute("fill").unwrap()).unwrap();
                for host in [Color::rgb(0, 0, 0), Color::rgb(255, 255, 255)] {
                    let backdrop = fill.composite(layer.composite(host, layer_opacity), opacity);
                    assert!(ink.contrast(backdrop) >= 4.5, "{content} on {host}");
                }
            }
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn c4_transparent_shoal_relations_keep_contrast_and_authored_paint() {
    use kurbo::{Point, Rect, Vec2};
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer};
    use merman_render::theme::{Appearance, BuiltinPreset, Color};

    let source = r##"---
config:
  layout: shoal
  c4:
    diagramMarginX: 0
    diagramMarginY: 0
---
C4Context
Boundary(outer, "Outer") {
  Boundary(middle, "Middle") {
    Boundary(inner, "Inner") {
      System(a, "Source")
      System(b, "Target")
    }
  }
}
Boundary(authored, "Authored") {
  System(c, "Outside")
}
BiRel(a, b, "First row<br/>Second row", "HTTP<br/>TLS")
Rel(b, c, "Authored color")
UpdateElementStyle(authored, $bgColor="#334455")
UpdateRelStyle(b, c, $textColor="#112233", $lineColor="#2468AC")
"##;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let compiler = ThemeCompiler::new();
    let theme = compiler
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let render = |theme, canvas| {
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap()
            .with_canvas_background(canvas);
        family::prepare(
            parsed.presentation(),
            &LayoutOptions::headless_svg_defaults(),
            session,
        )
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
    };
    let transparent = render(theme.clone(), Color::TRANSPARENT);
    let opaque = render(theme, Color::rgb(15, 20, 28));
    let document = roxmltree::Document::parse(transparent.svg()).unwrap();
    let opaque_document = roxmltree::Document::parse(opaque.svg()).unwrap();
    let find = |id| {
        document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == Some(id) && node.attribute("data-et") == Some("edge")
            })
            .unwrap()
    };
    let shaft = find("c4-rel-0");
    let authored = find("c4-rel-1");
    let outline = shaft.prev_sibling().unwrap();
    assert_eq!(outline.attribute("class"), Some("c4-relation-outline"));
    assert_eq!(shaft.attribute("stroke-width"), Some("1.5"));
    let outline_width: f64 = outline.attribute("stroke-width").unwrap().parse().unwrap();
    assert!(outline_width > 1.5);
    for attr in ["d", "clip-path"] {
        assert_eq!(outline.attribute(attr), shaft.attribute(attr));
    }
    assert!(outline.attribute("marker-end").is_none());
    assert!(outline.attribute("marker-start").is_none());
    assert_eq!(authored.attribute("stroke"), Some("#2468AC"));
    assert_ne!(
        authored.prev_sibling().unwrap().attribute("class"),
        Some("c4-relation-outline")
    );
    assert!(document.descendants().any(|node| node.attribute("class")
        == Some("c4-boundary-background")
        && node.attribute("fill") == Some("#334455")
        && node.attribute("fill-opacity").is_none()));

    let marker = |edge: roxmltree::Node<'_, '_>, attr| {
        let id = edge
            .attribute(attr)
            .unwrap()
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap()
    };
    let shaft_ink = Color::parse(shaft.attribute("stroke").unwrap()).unwrap();
    let outline_ink = Color::parse(outline.attribute("stroke").unwrap()).unwrap();
    let surface = Color::parse("#141B25").unwrap();
    for side in ["marker-start", "marker-end"] {
        let tip_paint = marker(shaft, side).first_element_child().unwrap();
        assert_eq!(tip_paint.attribute("fill"), shaft.attribute("stroke"));
        assert_eq!(tip_paint.attribute("stroke"), outline.attribute("stroke"));
        assert_eq!(tip_paint.attribute("stroke-width"), Some("1"));
    }
    let authored_tip = marker(authored, "marker-end")
        .first_element_child()
        .unwrap();
    assert_eq!(authored_tip.attribute("fill"), Some("#2468AC"));
    assert_eq!(authored_tip.attribute("stroke"), Some("#2468AC"));

    for mut host in [Color::rgb(248, 250, 252), Color::rgb(15, 20, 28)] {
        for _ in 0..=3 {
            assert!(
                shaft_ink.contrast(host).max(outline_ink.contrast(host)) >= 3.0,
                "shaft and markers on {host}"
            );
            host = surface.composite(host, 0.16);
        }
    }

    let view_box: svgtypes::ViewBox = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .parse()
        .unwrap();
    let viewport = Rect::from_origin_size((view_box.x, view_box.y), (view_box.w, view_box.h));
    let number =
        |node: roxmltree::Node<'_, '_>, attr| node.attribute(attr).unwrap().parse::<f64>().unwrap();
    for (edge, fill) in [(shaft, "#141B25"), (authored, "#FFFFFF")] {
        let label = document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == edge.attribute("data-id")
                    && node.attribute("class") == Some("label")
            })
            .unwrap();
        let plate = label.first_element_child().unwrap();
        assert_eq!(
            plate.attribute("class"),
            Some("c4-relation-label-background")
        );
        assert_eq!(plate.attribute("fill"), Some(fill));
        assert_eq!(label.attribute("stroke"), Some(fill));
        assert_eq!(label.attribute("paint-order"), Some("stroke"));
        let origin: svgtypes::Transform = label.attribute("transform").unwrap().parse().unwrap();
        let bounds = Rect::from_origin_size(
            (number(plate, "x"), number(plate, "y")),
            (number(plate, "width"), number(plate, "height")),
        );
        assert!(
            viewport
                .inflate(1e-7, 1e-7)
                .contains_rect(bounds + Vec2::new(origin.e, origin.f))
        );
        let rows: Vec<_> = label
            .children()
            .filter(|node| node.has_tag_name("text"))
            .collect();
        assert_eq!(rows.len(), if edge == shaft { 4 } else { 1 });
        for row in rows {
            let ink = Color::parse(row.attribute("fill").unwrap()).unwrap();
            assert!(ink.contrast(Color::parse(fill).unwrap()) >= 4.5);
            let y = number(row, "y") + number(row.first_element_child().unwrap(), "dy");
            let content = svg_text_content(row);
            let style = TextStyle {
                font_size: c4_effective_property(row, "font-size")
                    .unwrap()
                    .trim_end_matches("px")
                    .parse()
                    .unwrap(),
                font_family: c4_effective_property(row, "font-family"),
                font_weight: c4_effective_property(row, "font-weight"),
                font_style: c4_effective_property(row, "font-style"),
            };
            let half_width = DeterministicTextMeasurer::default()
                .measure_svg_tspan_text_bbox_width_px(&content, &style)
                / 2.0;
            let x = number(row, "x");
            assert!(
                bounds
                    .inflate(1e-7, 1e-7)
                    .contains(Point::new(x - half_width - 6.0, y))
                    && bounds
                        .inflate(1e-7, 1e-7)
                        .contains(Point::new(x + half_width + 6.0, y)),
                "missing horizontal padding around {content}"
            );
        }
        let opaque_shaft = opaque_document
            .descendants()
            .find(|node| {
                node.attribute("data-id") == edge.attribute("data-id")
                    && node.attribute("data-et") == Some("edge")
            })
            .unwrap();
        for attr in [
            "d",
            "data-points",
            "marker-start",
            "marker-end",
            "clip-path",
        ] {
            assert_eq!(opaque_shaft.attribute(attr), edge.attribute(attr));
        }
    }
    assert!(!opaque.svg().contains("c4-relation-label-background"));
    assert!(!opaque.svg().contains("c4-relation-outline"));

    let light = compiler
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Light))
        .unwrap();
    let light = render(light, Color::TRANSPARENT);
    assert!(!light.svg().contains("c4-relation-label-background"));

    let explicit_background = compiler
        .compile_json(
            br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"transparent","marker_style":"open"}}"#,
            Some(Appearance::Dark),
        )
        .unwrap();
    let bare = render(explicit_background, Color::TRANSPARENT);
    assert!(!bare.svg().contains("c4-relation-label-background"));
    let bare_document = roxmltree::Document::parse(bare.svg()).unwrap();
    let open_marker = bare_document
        .descendants()
        .find(|node| {
            node.has_tag_name("marker")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.ends_with("-arrowhead-0"))
        })
        .unwrap();
    let open_paths: Vec<_> = open_marker
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect();
    assert_eq!(open_paths.len(), 2);
    for (path, stroke, width) in [
        (open_paths[0], outline.attribute("stroke"), "1"),
        (open_paths[1], shaft.attribute("stroke"), "0.5"),
    ] {
        assert_eq!(path.attribute("fill"), Some("none"));
        assert_eq!(path.attribute("stroke"), stroke);
        assert_eq!(path.attribute("stroke-width"), Some(width));
    }
    assert_eq!(open_paths[0].attribute("d"), open_paths[1].attribute("d"));
}

#[test]
fn c4_nodes_render_each_layout_occurrence_after_alias_updates() {
    let source =
        "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nUpdateElementStyle(a, $alias=\"b\")\n";
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| { node.has_tag_name("text") && svg_text_content(*node) == "B" })
            .count(),
        2
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn c4_shoal_unseatable_caption_is_exported_below_its_graph() {
    use kurbo::{BezPath, Shape};
    use merman_render::theme::{Appearance, BuiltinPreset};

    let source = r#"---
config:
  layout: shoal
  htmlLabels: false
---
C4Context
UpdateLayoutConfig($c4ShapeInRow="1")
System(n0, "Service 0")
System(n1, "Service 1")
System(n2, "Service 2")
Rel(n2, n0, "Caption 0 for the relationship", "HTTPS")
Rel(n1, n0, "Caption 1 for the relationship", "HTTPS")
Rel(n0, n2, "Caption 2 for the relationship", "HTTPS")
Rel(n0, n0, "Caption 3 for the relationship", "HTTPS")
Rel(n1, n1, "Caption 4 for the relationship", "HTTPS")
"#;
    // A caption taller than the loop's entire shelf forces the exterior placement.
    let line = "Caption 4 word group that must remain legible";
    let caption = [line; 11].join("<br/>");
    let source = source.replace("Caption 4 for the relationship", &caption);
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::constrained())
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let prepared = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap();
    let projection = prepared.layout_json().unwrap();
    let relation = &projection["layout"]["C4Diagram"]["rels"][4];
    let prepared_caption = relation["label"]["text"]
        .as_str()
        .unwrap()
        .replace("<br/>", " ");
    assert_eq!(
        prepared_caption.split_whitespace().collect::<Vec<_>>(),
        caption
            .replace("<br/>", " ")
            .split_whitespace()
            .collect::<Vec<_>>(),
    );
    assert_eq!(relation["techn"]["text"], "HTTPS");

    let svg = prepared
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("edge"))
        .collect();
    assert_eq!(paths.len(), 5);
    let owner = paths
        .iter()
        .find(|path| path.attribute("data-id") == Some("c4-rel-4"))
        .unwrap();
    assert!(owner.attribute("marker-end").is_some());
    assert!(owner.attribute("clip-path").is_none());
    let shaft_bottom = paths
        .iter()
        .map(|path| {
            let shaft = BezPath::from_svg(path.attribute("d").unwrap()).unwrap();
            let outset = path
                .attribute("stroke-width")
                .unwrap()
                .parse::<f64>()
                .unwrap()
                / 2.0;
            shaft.bounding_box().y1 + outset
        })
        .fold(f64::NEG_INFINITY, f64::max);
    let caption = document
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.attribute("data-id") == owner.attribute("data-id")
        })
        .unwrap();
    let blocks: Vec<_> = caption
        .children()
        .filter(|node| node.has_tag_name("text"))
        .map(svg_text_content)
        .collect();
    let [reference, message @ .., technology] = blocks.as_slice() else {
        panic!("expected the exterior relationship caption: {blocks:?}");
    };
    assert_eq!(reference, "n1 → n1 (5)");
    assert_eq!(message.join(" "), [line; 11].join(" "));
    assert_eq!(technology, "[HTTPS]");
    let transform = caption
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let y: f64 = transform.split_once(',').unwrap().1.trim().parse().unwrap();
    let viewport = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .parse::<svgtypes::ViewBox>()
        .unwrap();
    for line in caption
        .descendants()
        .filter(|node| node.has_tag_name("tspan"))
    {
        let text_y = line.parent_element().unwrap().attribute("y").unwrap();
        let baseline = y
            + text_y.parse::<f64>().unwrap()
            + line.attribute("dy").unwrap().parse::<f64>().unwrap();
        assert!(
            shaft_bottom < baseline && baseline < viewport.y + viewport.h,
            "{baseline}"
        );
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn c4_shoal_fit_width_wraps_only_the_exterior_reference_for_a_long_alias() {
    let alias = "W".repeat(36);
    let message = "Retry one request";
    let caption = [message; 8].join("<br/>");
    let source = format!(
        r#"---
config:
  layout: shoal
  c4:
    wrap: false
    width: 120
    diagramMarginX: 30
---
C4Context
System({alias}, "A")
Rel({alias}, {alias}, "{caption}", "HTTPS")
"#,
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let mut options = LayoutOptions::headless_svg_defaults()
        .with_adaptation(merman_render::LayoutAdaptation::FitWidth);
    options.container_width = 300.0;
    let svg = family::prepare(parsed.presentation(), &options, session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    let viewport = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .parse::<svgtypes::ViewBox>()
        .unwrap();
    assert!(viewport.w <= 300.0 + 1e-7, "viewport width {}", viewport.w);
    let relation = |node: &roxmltree::Node<'_, '_>| node.attribute("data-id") == Some("c4-rel-0");
    let shaft = document
        .descendants()
        .find(|node| relation(node) && node.attribute("data-et") == Some("edge"))
        .unwrap();
    assert!(shaft.attribute("marker-end").is_some());
    assert!(shaft.attribute("clip-path").is_none());
    let rows: Vec<_> = document
        .descendants()
        .find(|node| relation(node) && node.attribute("class") == Some("label"))
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("text"))
        .map(svg_text_content)
        .collect();
    let message_start = rows.iter().position(|row| row == message).unwrap();
    assert!(message_start > 1, "expected a wrapped reference: {rows:?}");
    assert_eq!(
        rows[..message_start]
            .join("")
            .replace(char::is_whitespace, ""),
        format!("{alias}→{alias}(1)")
    );
    assert_eq!(&rows[message_start..rows.len() - 1], &[message; 8]);
    assert_eq!(rows.last().unwrap(), "[HTTPS]");
}

#[cfg(feature = "layout-shoal")]
#[test]
fn c4_shoal_fits_the_painted_mesh_without_losing_relationship_captions() {
    use kurbo::{BezPath, ParamCurve, PathSeg, Rect, Shape, Vec2};
    use merman_render::theme::{Appearance, BuiltinPreset, Color};

    let source = r#"---
config:
  layout: shoal
---
C4Container
  title Fit-Width 480px C4 Mesh
  UpdateLayoutConfig($c4ShapeInRow="4")
  System_Boundary(b1, "Frontend & Edge") {
    Container(web, "Web App", "React", "Browser SPA")
    Container(mob, "Mobile App", "Swift", "iOS client")
    Container(bff, "API Gateway", "Envoy", "Routing and TLS")
  }
  System_Boundary(b2, "Core Services") {
    Container(ord, "Order Engine", "Rust", "State machine")
    Container(pay, "Payment Hub", "Go", "PSP routing")
    Container(inv, "Inventory", "Rust", "Stock reservation")
  }
  System_Boundary(b3, "Data Tier") {
    ContainerDb(pg, "Ledger DB", "Postgres", "ACID ledger")
    ContainerDb(rd, "Cache", "Redis", "Hot sessions")
    ContainerQueue(kf, "Kafka", "Kafka", "Domain events")
  }
  Rel(web, bff, "Calls GraphQL API with JWT bearer auth", "HTTPS")
  Rel(mob, bff, "Streams telemetry and order updates", "gRPC")
  Rel(bff, ord, "Dispatches verified checkout commands", "gRPC/mTLS")
  Rel(ord, pay, "Authorizes and captures card funds", "gRPC")
  Rel(ord, inv, "Reserves SKU allocation atomically", "gRPC")
  Rel(ord, pg, "Persists immutable double-entry rows", "SQL/TLS")
  Rel(pay, kf, "Publishes settlement completion events", "Kafka")
  Rel(bff, rd, "Checks sliding-window rate limits", "RESP3")
  Rel(kf, ord, "Triggers async reconciliation callbacks", "Kafka")
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap()
        .with_canvas_background(Color::TRANSPARENT);
    let mut options = LayoutOptions::headless_svg_defaults()
        .with_adaptation(merman_render::LayoutAdaptation::FitWidth);
    options.container_width = 480.0;
    let svg = family::prepare(parsed.presentation(), &options, session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    let viewport = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .parse::<svgtypes::ViewBox>()
        .unwrap();
    assert!(viewport.w <= 480.0, "viewport width {}", viewport.w);
    let viewport = Rect::from_origin_size((viewport.x, viewport.y), (viewport.w, viewport.h))
        .inflate(1e-7, 1e-7);
    let number =
        |node: roxmltree::Node<'_, '_>, attr| node.attribute(attr).unwrap().parse::<f64>().unwrap();
    let rect = |node| {
        Rect::from_origin_size(
            (number(node, "x"), number(node, "y")),
            (number(node, "width"), number(node, "height")),
        )
    };
    let frames: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("cluster"))
        .map(|cluster| {
            let frame = cluster
                .children()
                .find(|node| {
                    node.has_tag_name("rect") && node.attribute("stroke-dasharray").is_some()
                })
                .unwrap();
            let stroke = number(frame, "stroke-width") / 2.0;
            let (outer, inner) = (
                rect(frame).inflate(stroke, stroke),
                rect(frame).inflate(-stroke, -stroke),
            );
            let id = cluster.attribute("data-id").unwrap();
            assert!(
                frame.attribute("stroke").is_some_and(|ink| ink != "none"),
                "{id}"
            );
            assert!(stroke > 0.0 && viewport.contains_rect(outer), "{id}");
            (id, outer, inner)
        })
        .collect();
    assert_eq!(frames.len(), 3);
    let shafts: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("edge"))
        .map(|node| {
            let outer = node
                .prev_sibling()
                .filter(|sibling| sibling.attribute("class") == Some("c4-relation-outline"))
                .unwrap_or(node);
            (
                node.attribute("data-id").unwrap(),
                BezPath::from_svg(node.attribute("d").unwrap()).unwrap(),
                number(outer, "stroke-width") / 2.0,
            )
        })
        .collect();
    assert_eq!(shafts.len(), 9);
    let mut captions: Vec<(String, Rect)> = Vec::new();
    for (occurrence, text) in [
        "Calls GraphQL API with JWT bearer auth",
        "Streams telemetry and order updates",
        "Dispatches verified checkout commands",
        "Authorizes and captures card funds",
        "Reserves SKU allocation atomically",
        "Persists immutable double-entry rows",
        "Publishes settlement completion events",
        "Checks sliding-window rate limits",
        "Triggers async reconciliation callbacks",
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("c4-rel-{occurrence}");
        let matches_id = |node: &roxmltree::Node<'_, '_>| node.attribute("data-id") == Some(&id);
        let (_, owner, _) = shafts.iter().find(|(edge, _, _)| *edge == id).unwrap();
        let caption = document
            .descendants()
            .find(|node| matches_id(node) && node.attribute("class") == Some("label"))
            .unwrap();
        let origin: svgtypes::Transform = caption.attribute("transform").unwrap().parse().unwrap();
        let plate = caption
            .children()
            .find(|node| node.attribute("class") == Some("c4-relation-label-background"))
            .unwrap();
        let ink = rect(plate) + Vec2::new(origin.e, origin.f);
        assert!(viewport.contains_rect(ink), "{id} leaves the viewport");
        for &(boundary, outer, inner) in &frames {
            assert!(
                ink.intersect(outer).area() - ink.intersect(inner).area() <= 1e-7,
                "{id} crosses the {boundary} boundary stroke"
            );
        }
        for (edge, path, stroke) in shafts.iter().filter(|(edge, _, _)| *edge != id) {
            let guard = ink.inflate(*stroke, *stroke);
            assert!(
                !path.segments().any(|segment| {
                    guard.contains(segment.start())
                        || guard.contains(segment.end())
                        || guard.path_segments(0.0).any(|side| {
                            let PathSeg::Line(side) = side else {
                                unreachable!()
                            };
                            !segment.intersect_line(side).is_empty()
                        })
                }),
                "{id} is crossed by foreign shaft {edge}"
            );
        }
        for (other, other_ink) in &captions {
            assert!(
                ink.intersect(*other_ink).area() <= 1e-7,
                "{id} overlaps {other}"
            );
        }
        if occurrence == 1 {
            assert!(
                ink.y1 < owner.bounding_box().y1,
                "{id} should stay in the open channel on its own shaft"
            );
        }
        assert!(
            caption
                .children()
                .filter(|node| node.has_tag_name("text"))
                .map(svg_text_content)
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .contains(text),
            "lost caption for {id}",
        );
        captions.push((id, ink));
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn c4_shoal_definitions_keep_unique_ids_after_rebasing() {
    let source = r##"---
config:
  layout: shoal
  look: neo
  themeVariables:
    useGradient: true
---
C4Context
System(computer, "Computer")
System(gradient, "Gradient")
System("drop-shadow", "Shadow")
System("c4-rel-0-label-cutout", "Cutout")
System("arrowhead-0", "Arrow")
Rel(computer, gradient, "Own caption")
UpdateRelStyle(computer, gradient, $lineColor="#1144AA")
"##;
    let env = RenderEnvironment::deterministic();
    let svg = render_c4_svg_with_environment(source, &env);
    let rebased =
        merman_render::svg::rebase_svg_ids(&svg, "preview", &env.begin_session().unwrap()).unwrap();
    for document in [&svg, &rebased] {
        let doc = roxmltree::Document::parse(document).unwrap();
        let mut ids = std::collections::HashMap::new();
        for node in doc.descendants().filter(roxmltree::Node::is_element) {
            if let Some(id) = node.attribute("id") {
                assert!(ids.insert(id, node).is_none(), "duplicate ID {id}");
            }
        }
        for tag in ["linearGradient", "filter", "marker", "symbol"] {
            assert!(
                ids.values().any(|node| node.has_tag_name(tag)),
                "missing {tag}"
            );
        }
        for url in document.split("url(#").skip(1) {
            let id = url.split_once(')').unwrap().0;
            assert!(ids.contains_key(id), "unresolved fragment {id}");
        }
        assert_eq!(
            doc.descendants()
                .filter(|node| node.attribute("data-et") == Some("node"))
                .count(),
            5
        );
    }
}

#[test]
fn c4_boundary_labels_render_each_layout_occurrence_after_alias_updates() {
    let source = "C4Context\nBoundary(a, \"A\") {\nUpdateLayoutConfig(4)\n}\nBoundary(b, \"B\") {\nUpdateLayoutConfig(4)\n}\nSystem(outside, \"Outside\")\nUpdateElementStyle(a, $alias=\"b\")\n";
    let baseline = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&baseline).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| { node.has_tag_name("text") && svg_text_content(*node) == "B" })
            .count(),
        2
    );
}

#[test]
fn c4_source_relationship_paints_keep_shared_markers_without_host_paint() {
    let body = "C4Context\nSystem(a, \"A\")\nSystem(b, \"B\")\nBiRel(a, b, \"Calls\", \"HTTPS\")\n";
    for source_owned in [false, true] {
        let source = if source_owned {
            format!("{body}UpdateRelStyle(a, b, $textColor=\"red\", $lineColor=\"blue\")\n")
        } else {
            body.to_owned()
        };
        let config = if source_owned {
            serde_json::json!({})
        } else {
            serde_json::json!({"c4": {
                "relationship_text_color": "red", "relationship_line_color": "blue"
            }})
        };
        let parsed = MermaidEngine::new()
            .with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            )
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_c4_shared_relationship_markers(&document);
        let line = document
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .unwrap();
        assert_eq!(line.attribute("stroke"), Some("blue"));
        for label in ["Calls", "[HTTPS]"] {
            let text = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
                .unwrap();
            assert_eq!(text.attribute("fill"), Some("red"));
            if label == "[HTTPS]" {
                assert_eq!(text.attribute("font-style"), Some("italic"));
            }
        }
    }
}

fn assert_c4_shared_relationship_markers(document: &roxmltree::Document<'_>) {
    let line = document
        .descendants()
        .find(|node| node.has_tag_name("line") && node.attribute("marker-end").is_some())
        .unwrap();
    assert!(
        line.parent()
            .unwrap()
            .children()
            .all(|node| !node.has_tag_name("defs")),
        "source relationship colors must preserve Mermaid's relationship group"
    );
    for (attribute, suffix) in [
        ("marker-start", "-arrowend)"),
        ("marker-end", "-arrowhead)"),
    ] {
        assert!(line.attribute(attribute).unwrap().ends_with(suffix));
    }
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("marker"))
            .count(),
        4
    );
}

fn try_render_c4_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse C4 resource-bound fixture")
        .expect("detect C4 resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin C4 resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn layout_c4_with_options(source: &str, options: &LayoutOptions) -> C4DiagramLayout {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, options, session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("C4 layout projection")
}

#[cfg(feature = "layout-shoal")]
#[test]
fn c4_shoal_uses_authored_columns_independently_of_mermaid_defaults() {
    use std::fmt::Write as _;

    let mut source = String::from("---\nconfig:\n  layout: shoal\n---\nC4Context\n");
    for index in 0..12 {
        writeln!(source, "System(s{index}, \"Service\")").unwrap();
    }
    let columns = |layout: &C4DiagramLayout| {
        let first_row = layout.shapes[0].y;
        layout
            .shapes
            .iter()
            .filter(|shape| shape.parent_boundary == layout.shapes[0].parent_boundary)
            .filter(|shape| (shape.y - first_row).abs() < 0.01)
            .count()
    };
    let layout = |source: &str| layout_c4_with_options(source, &LayoutOptions::default());
    let natural = columns(&layout(&source));
    assert!((2..4).contains(&natural));

    let explicit = format!("{source}UpdateLayoutConfig($c4ShapeInRow=\"4\")\n");
    let flat = layout(&explicit);
    assert_eq!(columns(&flat), 4);
    assert!(flat.shapes[4].y > flat.shapes[0].y);
    assert!((flat.shapes[11].y - flat.shapes[8].y).abs() < 0.01);

    source.push_str(
        "Boundary(one, \"Group\") {\n  System(a, \"Service\")\n}\n\
         Boundary(two, \"Group\") {\n  System(b, \"Service\")\n}\n",
    );
    let both = layout(&format!(
        "{source}UpdateLayoutConfig($c4ShapeInRow=\"4\", $c4BoundaryInRow=\"1\")\n"
    ));
    let boundary_only = layout(&format!(
        "{source}UpdateLayoutConfig($c4BoundaryInRow=\"1\")\n"
    ));
    assert_eq!(columns(&both), 4);
    assert_eq!(columns(&boundary_only), natural);
    for mixed in [both, boundary_only] {
        let boundary = |alias| {
            mixed
                .boundaries
                .iter()
                .find(|boundary| boundary.alias == alias)
                .unwrap()
        };
        let first = boundary("one");
        assert!(boundary("two").y > first.y + first.height);
    }
}

fn svg_text_content(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect()
}

fn c4_rect_labeled<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    label: &str,
) -> roxmltree::Node<'a, 'input> {
    let label_node = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == label)
        .unwrap_or_else(|| panic!("missing C4 label {label}"));

    let shape_group = label_node.ancestors().find(|node| {
        node.has_tag_name("g")
            && node
                .attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == "c4-shape"))
    });

    if let Some(group) = shape_group
        && let Some(shape) = group.descendants().find(|node| {
            ["rect", "path", "polygon", "circle", "ellipse"].contains(&node.tag_name().name())
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "label-container")
                })
        })
    {
        return shape;
    }

    label_node
        .ancestors()
        .filter(|node| node.has_tag_name("g"))
        .find_map(|group| {
            group.children().find(|child| {
                child.has_tag_name("rect")
                    && (child.attribute("fill").is_some() || child.attribute("style").is_some())
            })
        })
        .unwrap_or_else(|| panic!("missing C4 shape for label {label}"))
}

fn c4_effective_property(node: roxmltree::Node<'_, '_>, property: &str) -> Option<String> {
    let from_style = node.attribute("style").and_then(|style| {
        style.split(';').find_map(|declaration| {
            let (key, value) = declaration.split_once(':')?;
            (key.trim() == property).then_some(
                value
                    .trim()
                    .trim_end_matches("!important")
                    .trim()
                    .to_owned(),
            )
        })
    });
    from_style.or_else(|| node.attribute(property).map(str::to_owned))
}

fn deep_c4_boundary_chain(depth: usize) -> String {
    let mut input = String::from("C4Context\n");
    for level in 0..depth {
        input.push_str(&format!("Boundary(b{level}, \"B{level}\") {{\n"));
    }
    input.push_str("System(leaf, \"Leaf\")\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[derive(Default)]
struct C4FontHost {
    probes: Mutex<Vec<(String, TextMeasurementOperation, TextStyle)>>,
}

impl HostTextMeasurer for C4FontHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.probes.lock().unwrap().push((
            request.text.to_owned(),
            request.operation,
            request.style.clone(),
        ));
        let width = request.text.chars().count() as f64 * request.style.font_size * 0.5;
        Ok(Some(match request.operation {
            TextMeasurementOperation::Measure
            | TextMeasurementOperation::Wrapped
            | TextMeasurementOperation::MermaidCalculateTextDimensions => {
                HostTextMeasurement::Metrics(TextMetrics {
                    width,
                    height: request.style.font_size,
                    line_count: 1,
                })
            }
            TextMeasurementOperation::BBoxX => HostTextMeasurement::HorizontalExtents {
                left: 0.0,
                right: width,
            },
            TextMeasurementOperation::ComputedLength => HostTextMeasurement::Length(width),
            TextMeasurementOperation::RawBBoxHeight => {
                HostTextMeasurement::Length(request.style.font_size)
            }
            TextMeasurementOperation::CreateTextBBoxYOffset => {
                HostTextMeasurement::Length(-request.style.font_size * 0.8)
            }
            _ => return Ok(None),
        }))
    }
}

fn render_c4_with_font_host(
    source: &str,
    config: serde_json::Value,
) -> (
    family::RenderedFamilySvg,
    Vec<(String, TextMeasurementOperation, TextStyle)>,
    C4DiagramLayout,
) {
    let parsed = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let host = Arc::new(C4FontHost::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.c4-font-host").unwrap(),
        "1",
    )
    .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity,
            host.clone(),
            TextMeasurementPhase::ALL,
        ))
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare C4 through the production host measurement route");
    // Snapshot preparation before the writer can contribute any measurement probes.
    let probes = host.probes.lock().unwrap().clone();
    let projection = artifact
        .layout_json()
        .expect("serialize host-measured C4 layout");
    let layout = serde_json::from_value(projection["layout"]["C4Diagram"].clone()).unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    (rendered, probes, layout)
}

#[test]
fn c4_host_measurement_resolves_inherited_fonts_without_rewriting_css() {
    for family in ["", "inherit", "unset", r"i\6eherit", r"u\6eset"] {
        for has_title in [false, true] {
            let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine,serif","fontSize":"29px"}, "c4": {
                "wrap": false,
                "systemFontFamily": family, "systemFontSize": 20, "systemFontWeight": 600,
                "boundaryFontFamily": family, "boundaryFontSize": 22, "boundaryFontWeight": 500,
                "messageFontFamily": family, "messageFontSize": 24, "messageFontWeight": 300
            }});
            let source = format!(
                "{}C4Context\nDeployment_Node(boundary, \"BoundaryName\", \"BoundaryType\", \"BoundaryDescription\") {{\nSystem(a, \"NodeName\", \"NodeDescription\")\n}}\nSystem(b, \"Peer\")\nRel(a, b, \"MessageLabel\", \"MessageTechnology\")\n",
                if has_title {
                    "---\ntitle: Context\n---\n"
                } else {
                    ""
                }
            );
            let (rendered, probes, _) = render_c4_with_font_host(&source, config);
            for (text, operation, size, weight) in [
                ("NodeName", TextMeasurementOperation::BBoxX, 20.0, "bold"),
                (
                    "[Software System]",
                    TextMeasurementOperation::BBoxX,
                    15.0,
                    "600",
                ),
                (
                    "NodeDescription",
                    TextMeasurementOperation::BBoxX,
                    16.4,
                    "600",
                ),
                (
                    "BoundaryName",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    24.0,
                    "bold",
                ),
                (
                    "[BoundaryType]",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    22.0,
                    "500",
                ),
                (
                    "BoundaryDescription",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    20.0,
                    "500",
                ),
                (
                    "MessageLabel",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    24.0,
                    "300",
                ),
                (
                    "MessageTechnology",
                    TextMeasurementOperation::MermaidCalculateTextDimensions,
                    24.0,
                    "300",
                ),
            ] {
                assert!(
                    probes.iter().any(|(actual_text, actual_operation, style)| {
                        actual_text == text
                            && *actual_operation == operation
                            && style.font_family.as_deref() == Some("Cousine,serif")
                            && (style.font_size - size).abs() < 1e-9
                            && style.font_weight.as_deref() == Some(weight)
                    }),
                    "{family}, title={has_title}, missing resolved probe for {text}: {probes:?}"
                );
            }
            assert!(
                probes.iter().all(|(_, _, style)| {
                    style.font_family.as_deref() == Some("Cousine,serif")
                        || style.font_family.as_deref() == Some("sans-serif")
                }),
                "legacy dual-font probes may use sans-serif, never unresolved inheritance"
            );
            let family_css = if family.is_empty() {
                String::new()
            } else {
                format!("font-family:{family};")
            };
            assert!(rendered.svg().contains(&format!(
                ".c4-shape.c4-system .label{{{family_css}font-size:20px;font-weight:600;}}"
            )));
            assert!(rendered.svg().contains(&format!("font-family: {family};")));
        }
    }
}

#[test]
fn c4_zero_and_fractional_legacy_font_sizes_follow_visible_lines() {
    let nodes = "System(a, \"A\")\nSystem(b, \"B\")\n";
    for (key, base, text, raw, used) in [
        ("messageFontSize", 0.0, "Calls", 0.0, 0.0),
        ("messageFontSize", 0.5, "Calls", 0.5, 0.5),
        ("messageFontSize", -1.0, "Calls", -1.0, 29.0),
        ("messageFontSize", -3.0, "Calls", -3.0, 29.0),
        ("messageFontSize", -5e-7, "Calls", -5e-7, 29.0),
        ("messageFontSize", 5e-7, "Calls", 5e-7, 5e-7),
        ("boundaryFontSize", 0.0, "BoundaryName", 2.0, 2.0),
        ("boundaryFontSize", 0.0, "[Type]", 0.0, 0.0),
        ("boundaryFontSize", 0.0, "Description", -2.0, 29.0),
        ("boundaryFontSize", 0.5, "Description", -1.5, 29.0),
        ("boundaryFontSize", 2.0, "Description", 0.0, 0.0),
        (
            "boundaryFontSize",
            1.9999995,
            "Description",
            1.9999995 - 2.0,
            29.0,
        ),
        (
            "boundaryFontSize",
            2.0000005,
            "Description",
            2.0000005 - 2.0,
            2.0000005 - 2.0,
        ),
        ("boundaryFontSize", 2.5, "Description", 0.5, 0.5),
        ("boundaryFontSize", -1.0, "BoundaryName", 1.0, 1.0),
        ("boundaryFontSize", -1.0, "[Type]", -1.0, 29.0),
        ("boundaryFontSize", -1.0, "Description", -3.0, 29.0),
        ("boundaryFontSize", -3.0, "BoundaryName", -1.0, 29.0),
        ("boundaryFontSize", -3.0, "[Type]", -3.0, 29.0),
        ("boundaryFontSize", -3.0, "Description", -5.0, 29.0),
    ] {
        let body = match text {
            "Calls" => format!("{nodes}Rel(a, b, \"Calls\", \"HTTPS\")\n"),
            "BoundaryName" => {
                "Boundary(b, \"BoundaryName\", \"\") {\nSystem(a, \"A\")\n}\n".to_owned()
            }
            "[Type]" => "Boundary(b, \"\", \"Type\") {\nSystem(a, \"A\")\n}\n".to_owned(),
            _ => "Deployment_Node(b, \"\", \"\", \"Description\") {\nSystem(a, \"A\")\n}\n"
                .to_owned(),
        };
        let source = format!("C4Context\n{body}");
        for family in ["inherit", "Independent"] {
            let mut config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {
                "wrap":false, "boundaryFontFamily":family, "messageFontFamily":family
            }});
            config["c4"][key] = serde_json::json!(base);
            let (rendered, probes, _) = render_c4_with_font_host(&source, config.clone());
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let label = document
                .descendants()
                .find(|node| node.has_tag_name("text") && svg_text_content(*node) == text)
                .expect("the zero-sized row is still emitted");
            let css_size = c4_effective_property(label, "font-size").unwrap();
            assert_eq!(css_size, format!("{raw}px"), "{config}, {source}");
            // The legacy writer emits only numeric px declarations. Invalid negatives are ignored
            // by CSS and inherit the root, while zero remains a valid invisible declaration.
            let computed = css_size.strip_suffix("px").unwrap().parse::<f64>().unwrap();
            let computed = if computed < 0.0 { 29.0 } else { computed };
            assert_eq!(computed, used, "{config}, {source}");
            let measured_family = if family == "inherit" {
                "Cousine"
            } else {
                family
            };
            let configured_probes = probes
                .iter()
                .filter(|(actual, operation, style)| {
                    actual == text
                        && *operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                        && style.font_family.as_deref() == Some(measured_family)
                })
                .collect::<Vec<_>>();
            assert!(
                !configured_probes.is_empty(),
                "missing host probes: {config}, {source}"
            );
            assert!(
                configured_probes
                    .iter()
                    .all(|(_, _, style)| style.font_size == computed),
                "{configured_probes:?}"
            );
            assert!(probes.iter().all(|(_, _, style)| style.font_size >= 0.0));
        }
    }
}

#[test]
fn c4_invalid_element_sizes_inherit_root_before_section_scaling() {
    let source = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (raw, used) in [(-3.0, 29.0), (-1.0, 29.0), (0.0, 29.0), (1e-7, 1e-7)] {
        for family in ["inherit", "Independent"] {
            let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {
                "wrap":false, "systemFontSize":raw, "systemFontFamily":family,
                "systemFontWeight":600
            }});
            let (rendered, probes, _) = render_c4_with_font_host(source, config.clone());
            assert!(rendered.svg().contains("font-size:29px;"));
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let stylesheet = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            let element_rule = stylesheet
                .split(".c4-shape.c4-system .label{")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            if raw <= 0.0 {
                assert!(
                    !element_rule.contains("font-size:"),
                    "CSSOM drops invalid or falsy sizes: {element_rule}"
                );
            } else {
                assert!(
                    element_rule.contains(&format!("font-size:{raw}px;")),
                    "tiny positive CSS sizes remain positive: {element_rule}"
                );
            }
            assert!(
                stylesheet.contains(".c4-type{font-size:0.75em;}")
                    && stylesheet.contains(".c4-descr{font-size:0.82em;}")
            );
            let measured_family = if family == "inherit" {
                "Cousine"
            } else {
                family
            };
            for (text, computed, weight) in [
                ("NodeName", used, "bold"),
                ("[Software System]", used * 0.75, "600"),
                ("NodeDescription", used * 0.82, "600"),
            ] {
                let measured = probes
                    .iter()
                    .filter(|(actual, operation, _)| {
                        actual == text && *operation == TextMeasurementOperation::BBoxX
                    })
                    .collect::<Vec<_>>();
                assert!(!measured.is_empty(), "missing section probe for {text}");
                assert!(
                    measured.iter().all(|(_, _, style)| {
                        style.font_family.as_deref() == Some(measured_family)
                            && (style.font_size - computed).abs() < 1e-9
                            && style.font_weight.as_deref() == Some(weight)
                    }),
                    "{config}, {text}: {measured:?}"
                );
            }
        }
    }
}

#[test]
fn c4_element_css_sizes_keep_source_units_inheritance_before_measurement() {
    let body = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (raw, used, css, _inherits) in [
        (serde_json::json!(""), 29.0, None, true),
        (serde_json::json!(false), 29.0, None, true),
        (serde_json::json!("bad"), 29.0, None, true),
        (serde_json::json!("24"), 29.0, None, true),
        (serde_json::json!("24pt"), 32.0, Some("24pt"), false),
        (serde_json::json!("150%"), 43.5, Some("150%"), false),
        (serde_json::json!("0em"), 0.0, Some("0em"), false),
        (serde_json::json!("0rem"), 0.0, Some("0rem"), false),
        (serde_json::json!("inherit"), 29.0, Some("inherit"), true),
        (serde_json::json!("unset"), 29.0, Some("unset"), true),
    ] {
        for from_source in [false, true] {
            let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {
                "wrap":false, "systemFontFamily":"inherit", "systemFontSize":raw,
                "systemFontWeight":600
            }});
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config)
            };
            let (rendered, probes, _) = render_c4_with_font_host(&source, site);
            let stylesheet = roxmltree::Document::parse(rendered.svg())
                .unwrap()
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap()
                .to_owned();
            let rule = stylesheet
                .split(".c4-shape.c4-system .label{")
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            match css {
                Some(css) => assert!(rule.contains(&format!("font-size:{css};")), "{raw}: {rule}"),
                None => assert!(!rule.contains("font-size:"), "{raw}: {rule}"),
            }
            for (text, size, weight) in [
                ("NodeName", used, "bold"),
                ("[Software System]", used * 0.75, "600"),
                ("NodeDescription", used * 0.82, "600"),
            ] {
                if used > 0.0 {
                    assert!(
                        probes.iter().any(|(actual, operation, style)| {
                            actual == text
                                && *operation == TextMeasurementOperation::BBoxX
                                && (style.font_size - size).abs() < 1e-9
                                && style.font_family.as_deref() == Some("Cousine")
                                && style.font_weight.as_deref() == Some(weight)
                        }),
                        "{raw}, {text}: {probes:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn c4_element_cssom_rejects_literal_priority_before_normalizing_fonts() {
    let source = "C4Context\nSystem(a, \"NodeName\", \"NodeDescription\")\n";
    for (family, size, family_css, size_css, used, inherits_family, _inherits_size) in [
        (
            "serif !important",
            "24px !important",
            None,
            None,
            29.0,
            true,
            true,
        ),
        (
            r#""A: B", serif !important"#,
            "0em !important",
            None,
            None,
            29.0,
            true,
            true,
        ),
        ("serif;", "24pt !IMPORTANT", None, None, 29.0, true, true),
        ("inherit", "24px;", Some("inherit"), None, 29.0, true, true),
        (
            "inherit",
            "calc(24px);",
            Some("inherit"),
            None,
            29.0,
            true,
            true,
        ),
        (
            r#""A: !important", serif"#,
            "24pt",
            Some(r#""A: !important",serif"#),
            Some("24pt"),
            32.0,
            false,
            false,
        ),
        (
            "inherit !IMPORTANT",
            "unset !important",
            Some("inherit !important"),
            Some("unset !important"),
            29.0,
            true,
            true,
        ),
    ] {
        // Source presentation filtering rejects unquoted priority before the family adapter.
        // Site input exercises the actual raw CSSOM boundary retained by Mermaid's C4 stylesheet.
        let config = serde_json::json!({"themeVariables":{"fontFamily":"Cousine","fontSize":"29px"}, "c4": {"wrap":false,
            "systemFontFamily":family,"systemFontSize":size,"systemFontWeight":600}});
        let (rendered, probes, _) = render_c4_with_font_host(source, config);
        let stylesheet = roxmltree::Document::parse(rendered.svg())
            .unwrap()
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap()
            .to_owned();
        let rule = stylesheet
            .split(".c4-shape.c4-system .label{")
            .nth(1)
            .unwrap()
            .split('}')
            .next()
            .unwrap();
        for (property, value) in [("font-family", family_css), ("font-size", size_css)] {
            match value {
                Some(value) => assert!(
                    rule.contains(&format!("{property}:{value};")),
                    "{family}/{size}: {rule}"
                ),
                None => assert!(
                    !rule.contains(&format!("{property}:")),
                    "{family}/{size}: {rule}"
                ),
            }
        }
        let expected_family = if inherits_family {
            "Cousine"
        } else {
            r#""A: !important",serif"#
        };
        assert!(
            probes.iter().any(|(text, operation, style)| {
                text == "NodeName"
                    && *operation == TextMeasurementOperation::BBoxX
                    && style.font_size == used
                    && style.font_family.as_deref() == Some(expected_family)
            }),
            "{family}/{size}: {probes:?}"
        );
    }
}

#[test]
fn c4_contextual_root_sizes_preserve_css_and_keep_independent_element_sizes() {
    let body = "C4Context\ntitle Root Size\nSystem(a, \"API\")\n";
    for size in [
        "1em",
        "100%",
        "1rem",
        "calc(24px)",
        "var(--audit-size, 24px)",
        "calc(24px);",
        "1em;",
        "100%;",
    ] {
        for from_source in [false, true] {
            if from_source && (size.starts_with("var(") || size.ends_with(';')) {
                continue;
            }
            let config = serde_json::json!({"themeVariables":{"fontSize":size}});
            let (source, site) = if from_source {
                (
                    format!("---\nconfig: {config}\n---\n{body}"),
                    serde_json::json!({}),
                )
            } else {
                (body.to_owned(), config)
            };
            let (rendered, _, _) = render_c4_with_font_host(&source, site);
            assert!(rendered.svg().contains(&format!("font-size:{size};")));
            assert!(rendered.svg().contains(
                ".c4-shape.c4-system .label{font-family:\"Open Sans\",sans-serif;font-size:14px;"
            ));
        }
    }
}

#[test]
fn c4_subpixel_root_fallback_multiline_sections_use_actual_em_steps() {
    let config = serde_json::json!({
        "themeVariables": {"fontFamily":"Cousine", "fontSize":"0.5px"},
        "c4": {"wrap":false, "systemFontSize":-1, "systemFontFamily":"inherit"}
    });
    for from_source in [false, true] {
        let render = |body: &str| {
            if from_source {
                let source = format!("---\nconfig: {config}\n---\n{body}");
                render_c4_with_font_host(&source, serde_json::json!({}))
            } else {
                render_c4_with_font_host(body, config.clone())
            }
        };
        let (_, _, single) = render("C4Context\nSystem(a, \"A\", \"First\")\n");
        let (rendered, probes, multi) =
            render("C4Context\nSystem(a, \"A<br/>B\", \"First<br/>Second\")\n");
        assert!(rendered.svg().contains("font-size:0.5px;"));
        let single = &single.shapes[0];
        let multi = &multi.shapes[0];
        assert!((multi.label.height - single.label.height - 0.55).abs() < 1e-9);
        assert!((multi.type_block.height - single.type_block.height).abs() < 1e-9);
        assert!(
            (multi.descr.as_ref().unwrap().height - single.descr.as_ref().unwrap().height - 0.451)
                .abs()
                < 1e-9
        );
        assert!((multi.height - single.height - 1.001).abs() < 1e-9);
        for (text, size) in [("B", 0.5), ("Second", 0.41)] {
            assert!(probes.iter().any(|(actual, operation, style)| {
                actual == text
                    && *operation == TextMeasurementOperation::BBoxX
                    && (style.font_size - size).abs() < 1e-9
                    && style.font_family.as_deref() == Some("Cousine")
            }));
        }
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for class in ["c4-name", "c4-descr"] {
            let section = document
                .descendants()
                .find(|node| node.attribute("class") == Some(class))
                .unwrap();
            let rows = section
                .descendants()
                .filter(|node| node.attribute("class") == Some("text-outer-tspan row"))
                .collect::<Vec<_>>();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].attribute("y"), Some("-0.1em"));
            assert_eq!(rows[1].attribute("y"), Some("1em"));
            assert!(rows.iter().all(|row| row.attribute("dy") == Some("1.1em")));
        }
    }
}

#[test]
fn c4_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"C4Context
Person(user, "User")
System_Boundary(system, "Bounded system") {
  Container(api, "API", "Rust")
  ContainerDb(database, "Database", "PostgreSQL")
}
Rel(user, api, "Calls", "HTTPS")
Rel(api, database, "Reads", "SQL")
"#;
    let baseline = try_render_c4_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded C4 baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "C4 fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact C4 SVG byte ceiling");
    let exact = try_render_c4_svg_with_resource_policy(source, exact_policy)
        .expect("the exact C4 family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact C4 SVG byte ceiling");
    let error = try_render_c4_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the C4 family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected C4 MaxSvgBytes rejection, got {error}");
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
fn c4_public_layout_and_svg_render_handle_deep_boundary_chain() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    const DEPTH: usize = 1500;
    let source = deep_c4_boundary_chain(DEPTH);

    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    assert_eq!(parsed.metadata().diagram_type, "c4");

    let options = LayoutOptions::default();
    let artifact = family::prepare(&parsed, &options, session)
        .expect("layout should not depend on recursive boundary traversal");
    let projection = artifact.layout_json().expect("serialize C4 layout");
    let c4: C4DiagramLayout = serde_json::from_value(projection["layout"]["C4Diagram"].clone())
        .expect("C4 layout projection");

    assert_eq!(c4.boundaries.len(), DEPTH + 1);
    assert_eq!(c4.shapes.len(), 1);
    assert_eq!(c4.shapes[0].alias, "leaf");

    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("SVG painting should use an iterative boundary traversal")
        .svg()
        .to_owned();
    assert!(svg.contains(">Leaf</tspan>"));
    assert!(svg.contains(">B0</tspan>"));
    assert!(svg.contains(&format!(">B{}</tspan>", DEPTH - 1)));
}

#[test]
fn c4_unified_shapes_render_canonical_labels() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Person(person, "Person")
Container(system, "System", "Rust", "A short description")
Person_Ext(external, "External")
System(framed, "Framed", "A component-shaped system", $shape="component")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");

    for class in ["c4-name", "c4-type", "c4-descr"] {
        assert!(
            document
                .descendants()
                .any(|node| { node.has_tag_name("g") && node.attribute("class") == Some(class) }),
            "missing unified C4 label section {class}: {svg}"
        );
    }
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();
    assert!(type_texts.iter().any(|text| text == "[Person]"));
    assert!(type_texts.iter().any(|text| text == "[Container: Rust]"));
    assert!(
        document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-descr"))
            .map(svg_text_content)
            .any(|text| text == "A short description")
    );
    assert_eq!(svg.matches("<circle ").count(), 2);
    assert_eq!(svg.matches("<polygon ").count(), 1);
    assert!(!svg.contains("<<person>>"));
    assert!(!svg.contains("<<system>>"));
    assert!(!svg.contains("<<external_person>>"));
    assert!(
        document
            .descendants()
            .all(|node| node.attribute("data-look").is_none()),
        "Classic C4 must retain the upstream DOM without an unused look attribute"
    );
}

#[test]
fn c4_neo_look_reaches_layout_and_svg_terminals() {
    let classic_source = r#"C4Context
ContainerDb(database, "Database", "PostgreSQL")
"#;
    let neo_source = r#"%%{init: {"look": "neo"}}%%
C4Context
ContainerDb(database, "Database", "PostgreSQL")
"#;

    let classic = layout_c4_with_options(classic_source, &LayoutOptions::default());
    let neo = layout_c4_with_options(neo_source, &LayoutOptions::default());
    let classic_shape = classic.shapes.first().expect("classic C4 shape");
    let neo_shape = neo.shapes.first().expect("neo C4 shape");

    // Mermaid's 11.17 cylinder handler increases the vertical label padding from 20px to
    // 24px for Neo. Keep the assertion relational so host text metrics do not become a pixel
    // oracle for this semantic difference.
    assert!(
        neo_shape.height > classic_shape.height + 3.0,
        "Neo cylinder padding must affect layout height: classic={:?}, neo={:?}",
        classic_shape,
        neo_shape
    );

    let svg = render_c4_svg_with_environment(neo_source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid Neo C4 SVG");
    let shape_group = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|c| c == "c4-shape"))
        })
        .expect("Neo C4 shape group");
    assert_eq!(shape_group.attribute("data-look"), Some("neo"));
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("C4 stylesheet");
    assert!(
        style.contains(r#"[data-look="neo"].node path"#),
        "the common Neo selector must be reachable from C4 terminals"
    );
}

#[test]
fn c4_neo_component_uses_framed_rectangle_geometry() {
    let source = r#"%%{init: {"look": "neo", "c4": {"width": 1}}}%%
C4Context
System(framed, "Framed", "Component description", $shape="component")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = layout.shapes.first().expect("Neo component shape");

    let mut content_width = shape.label.width.max(shape.type_block.width);
    let mut content_height = shape.label.height + shape.type_block.height;
    let mut section_count = 2_usize;
    if let Some(description) = shape.descr.as_ref() {
        content_width = content_width.max(description.width);
        content_height += description.height;
        section_count += 1;
    }
    content_height += 3.0 * section_count.saturating_sub(1) as f64;

    // Mermaid maps C4 `component` to `fr-rect`/subroutine. Its Neo geometry reserves two
    // eight-pixel frames plus 28px horizontal label padding and 12px total vertical padding.
    assert!((shape.width - (content_width + 44.0)).abs() < 1e-6);
    assert!((shape.height - (content_height + 12.0)).abs() < 1e-6);

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid Neo component SVG");
    let shape_group = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-framed"))
        .expect("Neo component group");
    let label_group = shape_group
        .children()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
        .expect("Neo component label group");
    let transform = label_group
        .attribute("transform")
        .and_then(|value| value.strip_prefix("translate("))
        .and_then(|value| value.strip_suffix(')'))
        .expect("translate transform");
    let mut values = transform.split(',').map(|value| {
        value
            .trim()
            .parse::<f64>()
            .expect("numeric translate component")
    });
    let translate_x = values.next().expect("translate x");
    let translate_y = values.next().expect("translate y");
    assert!(values.next().is_none());
    assert!((translate_x + content_width / 2.0).abs() < 1e-6);
    assert!((translate_y + content_height / 2.0).abs() < 1e-6);
}

#[test]
fn c4_hand_drawn_shapes_have_solid_fills_and_rough_outlines() {
    let source = r#"%%{init: {"look": "handDrawn"}}%%
C4Context
Person(person, "Person")
System(service, "Service")
ContainerDb(database, "Database", "PostgreSQL")
System(component, "Component", "Rust", $shape="component")
System(queue, "Queue", "SQS", $shape="queue")
"#;

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid hand-drawn C4 SVG");
    let shape_groups = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|classes| classes.split_whitespace().any(|c| c == "c4-shape"))
        })
        .collect::<Vec<_>>();
    assert_eq!(shape_groups.len(), 5);
    for group in shape_groups {
        let id = group.attribute("id").unwrap();
        assert_eq!(group.attribute("data-look"), Some("handDrawn"));
        let fills: Vec<_> = group
            .descendants()
            .filter(|node| {
                node.has_tag_name("path") && node.attribute("class") == Some("rough-fill")
            })
            .collect();
        assert_eq!(fills.len(), if id == "merman-person" { 2 } else { 1 });
        for fill in fills {
            assert!(
                fill.attribute("fill").is_some_and(|color| color != "none"),
                "{id} needs a continuous fill behind its text"
            );
            assert_eq!(fill.attribute("stroke"), Some("none"));
            let path = svgtypes::PathParser::from(fill.attribute("d").unwrap());
            assert!(matches!(
                path.last(),
                Some(Ok(svgtypes::PathSegment::ClosePath { .. }))
            ));
        }
        assert!(group.descendants().any(|node| {
            node.has_tag_name("path")
                && node.attribute("class") == Some("rough-stroke")
                && node.attribute("fill") == Some("none")
        }));
    }
}

#[test]
fn c4_hand_drawn_look_honors_configured_seed() {
    fn rough_stroke_paths(svg: &str) -> Vec<String> {
        let document = roxmltree::Document::parse(svg).expect("valid hand-drawn C4 SVG");
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| matches!(class, "rough-stroke" | "c4-boundary-outline"))
                    })
            })
            .map(|node| {
                node.attribute("d")
                    .expect("rough stroke path data")
                    .to_owned()
            })
            .collect()
    }

    let source_for_seed = |layout, seed| {
        format!(
            r#"---
config:
{layout}  look: handDrawn
  handDrawnSeed: {seed}
---
C4Context
Boundary(group, "Group") {{
  System(service, "Service")
}}
UpdateElementStyle(group, $bgColor="lightgrey")
"#
        )
    };
    let environment = RenderEnvironment::deterministic();
    for (layout, shoal) in [("", false), ("  layout: shoal\n", true)] {
        if shoal && !cfg!(feature = "layout-shoal") {
            continue;
        }
        let render =
            |seed| render_c4_svg_with_environment(&source_for_seed(layout, seed), &environment);
        let seeded = render(7);
        let paths = rough_stroke_paths(&seeded);
        assert_eq!(paths.len(), if shoal { 2 } else { 1 });
        assert_eq!(paths, rough_stroke_paths(&render(7)));
        let different_paths = rough_stroke_paths(&render(8));
        assert_eq!(paths.len(), different_paths.len());
        for (path, different_path) in paths.iter().zip(&different_paths) {
            assert_ne!(path, different_path);
        }

        let document = roxmltree::Document::parse(&seeded).unwrap();
        let database_icon = document.descendants().any(|node| {
            node.has_tag_name("symbol") && node.attribute("id") == Some("merman-database")
        });
        assert_eq!(database_icon, !shoal);
        let background = document
            .descendants()
            .find(|node| node.attribute("class") == Some("c4-boundary-background"))
            .unwrap();
        let rough_outline = document
            .descendants()
            .find(|node| node.attribute("class") == Some("c4-boundary-outline"));
        let outline = if shoal {
            let path = rough_outline.expect("Shoal boundary has a rough outline");
            assert!(path.has_tag_name("path"));
            path
        } else {
            assert!(rough_outline.is_none());
            let rect = c4_rect_labeled(&document, "Group");
            assert!(rect.has_tag_name("rect"));
            assert_eq!(rect.attribute("rx"), Some("2.5"));
            assert_eq!(rect.attribute("ry"), Some("2.5"));
            rect
        };
        let card = document
            .descendants()
            .find(|node| node.attribute("id") == Some("merman-service"))
            .unwrap();
        let silhouette = card
            .descendants()
            .find(|node| node.attribute("class") == Some("rough-fill"))
            .unwrap();
        let arc_count = svgtypes::PathParser::from(silhouette.attribute("d").unwrap())
            .map(|segment| segment.unwrap())
            .filter(|segment| matches!(segment, svgtypes::PathSegment::EllipticalArc { .. }))
            .count();
        assert_eq!(arc_count, if shoal { 4 } else { 0 });
        assert_eq!(background.attribute("fill"), Some("lightgrey"));
        assert_eq!(outline.attribute("fill"), Some("none"));
        assert_eq!(outline.attribute("stroke-dasharray"), Some("7.0,7.0"));
        assert!(background.range().start < card.range().start);
        assert!(card.range().start < outline.range().start);
    }
}

#[test]
fn c4_shape_named_type_does_not_override_the_rendered_stereotype() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
Container(app, "Application", $type="Domain Service")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();

    assert_eq!(type_texts, ["[Container]"]);
}

#[test]
fn c4_shape_technology_reaches_layout_and_svg_while_named_type_is_ignored() {
    let source = r#"C4Context
Container(app, "Application", "Rust", $type="Domain Service")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = layout
        .shapes
        .iter()
        .find(|shape| shape.alias == "app")
        .expect("C4 application shape");

    assert_eq!(shape.type_block.text, "[Container: Rust]");

    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let type_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("c4-type"))
        .map(svg_text_content)
        .collect::<Vec<_>>();

    assert_eq!(type_texts, ["[Container: Rust]"]);
    assert!(!svg.contains("Domain Service"));
}

#[test]
fn c4_uses_explicit_screen_available_width_without_changing_container_geometry() {
    let source = include_str!(
        "../../../fixtures/c4/upstream_docs_c4_c4_container_diagram_c4container_006.mmd"
    );
    let default = layout_c4_with_options(source, &LayoutOptions::default());
    let wide_screen = layout_c4_with_options(
        source,
        &LayoutOptions::default().with_screen_available_width(1280.0),
    );

    assert_eq!(default.container_width, 800.0);
    assert_eq!(default.screen_available_width, None);
    assert_eq!(wide_screen.container_width, 800.0);
    assert_eq!(wide_screen.screen_available_width, Some(1280.0));
    assert!(wide_screen.width > default.width);
    assert!(wide_screen.height < default.height);
}

#[test]
fn c4_svg_paints_each_boundary_subtree_in_mermaid_order() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(root_before, "Root Before")
Boundary(outer, "Outer Boundary") {
  System(outer_before, "Outer Before")
  Boundary(inner, "Inner Boundary") {
    System(inner_shape, "Inner Shape")
  }
  System(outer_after, "Outer After")
}
Boundary(sibling, "Sibling Boundary") {
  System(sibling_shape, "Sibling Shape")
}
System(root_after, "Root After")
Rel(inner_shape, root_before, "Leaves subtree")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let labels = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(svg_text_content)
        .filter(|text| {
            matches!(
                text.as_str(),
                "Root Before"
                    | "Root After"
                    | "Outer Before"
                    | "Outer After"
                    | "Inner Shape"
                    | "Inner Boundary"
                    | "Outer Boundary"
                    | "Sibling Shape"
                    | "Sibling Boundary"
                    | "Leaves subtree"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        labels,
        [
            "Root Before",
            "Root After",
            "Outer Before",
            "Outer After",
            "Inner Shape",
            "Inner Boundary",
            "Outer Boundary",
            "Sibling Shape",
            "Sibling Boundary",
            "Leaves subtree",
        ]
    );
}

#[test]
fn c4_relation_keeps_explicit_line_and_text_styles() {
    let svg = render_c4_svg_with_environment(
        r#"C4Context
System(a, "A")
System(b, "B")
Rel(a, b, "Calls", "HTTPS")
UpdateRelStyle(a, b, $textColor="red", $lineColor="blue", $offsetX="10", $offsetY="20")
"#,
        &RenderEnvironment::deterministic(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let calls = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "Calls")
        .expect("relationship label");
    let technology = document
        .descendants()
        .find(|node| node.has_tag_name("text") && svg_text_content(*node) == "[HTTPS]")
        .expect("relationship technology label");
    let line = document
        .descendants()
        .find(|node| {
            matches!(node.tag_name().name(), "line" | "path")
                && node.attribute("stroke") == Some("blue")
                && node.attribute("marker-end").is_some()
        })
        .expect("relationship line");

    assert_eq!(calls.attribute("fill"), Some("red"));
    assert!(calls.attribute("style").is_some_and(|style| {
        style.contains("text-anchor: middle") && style.contains("font-family:")
    }));
    assert_eq!(technology.attribute("fill"), Some("red"));
    assert_eq!(technology.attribute("font-style"), Some("italic"));
    assert_eq!(line.attribute("stroke-width"), Some("1"));
    assert_eq!(line.attribute("style"), Some("fill: none;"));
}

#[test]
fn c4_relationships_resolve_boundary_endpoints_and_prefer_shapes() {
    let source = r#"C4Context
System(outside, "Outside")
Boundary(left, "Left") {
  System(inside, "Inside")
}
Boundary(right, "Right") {
  System(other, "Other")
}
Rel(outside, left, "Enters")
Rel(left, outside, "Leaves")
Rel(left, right, "Connects")
"#;
    let layout = layout_c4_with_options(source, &LayoutOptions::headless_svg_defaults());
    assert_eq!(layout.rels.len(), 3);
    for (alias, point) in [
        ("left", &layout.rels[0].end_point),
        ("left", &layout.rels[1].start_point),
        ("left", &layout.rels[2].start_point),
        ("right", &layout.rels[2].end_point),
    ] {
        let boundary = layout.boundaries.iter().find(|b| b.alias == alias).unwrap();
        let on_vertical = (point.x - boundary.x).abs() < 1e-8
            || (point.x - boundary.x - boundary.width).abs() < 1e-8;
        let on_horizontal = (point.y - boundary.y).abs() < 1e-8
            || (point.y - boundary.y - boundary.height).abs() < 1e-8;
        assert!(
            on_vertical || on_horizontal,
            "endpoint must meet {alias}'s perimeter"
        );
        assert!(point.x >= boundary.x - 1e-8 && point.x <= boundary.x + boundary.width + 1e-8);
        assert!(point.y >= boundary.y - 1e-8 && point.y <= boundary.y + boundary.height + 1e-8);
    }
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    assert!(svg.contains("Enters") && svg.contains("Leaves") && svg.contains("Connects"));

    let shared_alias = source
        .replace("System(outside,", "System(left,")
        .replace("outside, left", "left, inside")
        .replace("left, outside", "inside, left");
    let layout = layout_c4_with_options(&shared_alias, &LayoutOptions::headless_svg_defaults());
    let shape = layout
        .shapes
        .iter()
        .find(|shape| shape.alias == "left")
        .unwrap();
    let endpoint = &layout.rels[0].start_point;
    assert!(endpoint.x >= shape.x - 1e-8 && endpoint.x <= shape.x + shape.width + 1e-8);
    assert!(endpoint.y >= shape.y - 1e-8 && endpoint.y <= shape.y + shape.height + 1e-8);
}

#[test]
fn c4_hand_drawn_person_body_shares_the_classic_body_coordinate_domain() {
    let source = "%%{init: {\"look\": \"handDrawn\"}}%%\nC4Context\nPerson(person, \"Person\")\n";
    let layout = layout_c4_with_options(source, &LayoutOptions::default());
    let shape = &layout.shapes[0];
    let radius = (shape.width * 0.23).clamp(16.0, 56.0);
    let overlap = radius * 0.27;
    let body_height = shape.height - 2.0 * radius + overlap;
    let body_top = -shape.height / 2.0 + 2.0 * radius - overlap;
    let svg = render_c4_svg_with_environment(source, &RenderEnvironment::deterministic());
    let document = roxmltree::Document::parse(&svg).unwrap();
    let body = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g") && node.attribute("class") == Some("basic label-container")
        })
        .unwrap();
    let transforms = body
        .children()
        .filter(|node| node.has_tag_name("g"))
        .map(|node| node.attribute("transform").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(transforms.len(), 2);
    let translate_y = |value: &str| {
        value
            .strip_prefix("translate(0,")
            .unwrap()
            .strip_suffix(')')
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!((translate_y(transforms[0]) - (body_top + body_height / 2.0)).abs() < 1e-6);
    assert!((translate_y(transforms[1]) - (-shape.height / 2.0 + radius)).abs() < 1e-6);
    assert!(translate_y(transforms[0]) > translate_y(transforms[1]));
}

const PRESENTATION_C4: &str = r#"C4Context
title Theme coverage
Boundary(boundary, "Boundary title", "Boundary type") {
  Person(person, "Person", "Person description")
  System(system, "System", "System description")
  Deployment_Node(node, "Node title", "Node type", "Boundary description") {
    System(child, "Child")
  }
}
System_Ext(external, "External", "External description")
BiRel(person, system, "Uses", "HTTPS")
Rel(system, external, "Calls")
"#;
#[test]
fn c4_dynamic_text_colors_remain_single_xml_attributes_for_site_and_source_values() {
    let site_color = r#"red" data-probe="site&raw"#;
    let authored_color = "blue' data-probe='source&raw";
    let source = format!(
        r#"{PRESENTATION_C4}
UpdateRelStyle(system, external, $textColor="{authored_color}")
"#
    );
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({"c4": {
            "boundary_font_color": site_color,
            "relationship_text_color": site_color
        }})),
    );
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    let document =
        roxmltree::Document::parse(&svg).expect("dynamic color values must not break XML");
    for (content, expected) in [
        ("Boundary title", site_color),
        ("Boundary description", site_color),
        ("Uses", site_color),
        ("[HTTPS]", site_color),
        ("Calls", authored_color),
    ] {
        let text = document
            .descendants()
            .find(|node| node.has_tag_name("text") && svg_text_content(*node) == content)
            .unwrap();
        assert_eq!(text.attribute("fill"), Some(expected), "{content}");
    }
    assert!(
        document
            .descendants()
            .all(|node| node.attribute("data-probe").is_none())
    );
}

mod common;

#[test]
fn c4_svg_preserves_db_accessibility_text() {
    for (description, expected) in [
        ("First\n<!--a11y-->", Some("First\n")),
        ("<!--a11y-->\u{0085}<!--a11y-->", Some("\u{0085}")),
        ("<!--a11y-->", None),
    ] {
        let source = format!("C4Context\naccDescr {{\n{description}\n}}\nSystem(app, \"App\")\n");
        let svg = render_c4_svg_with_environment(&source, &RenderEnvironment::deterministic());
        common::assert_svg_accessibility(&svg, None, expected);
    }
}
