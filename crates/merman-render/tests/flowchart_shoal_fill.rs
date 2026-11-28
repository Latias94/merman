#![cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-flowchart", feature = "diagram-agentflow")
))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::SvgPipeline;
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

const THEME: &[u8] = br##"{
    "schema_version":2,"extends":"studio",
    "roles":{"canvas":"#FAF7EC","surface.container":"#FAF7EC","stroke.connector":"#202938"},
    "form":{"container_radius":4,"elevation":"flat","marker_style":"filled"}
}"##;

const COMPOUND: &str = "subgraph G[Group]\nA inside@<--> B\nB authored@--> C\nend\n\
    X cross@--> A\nX dotted@-.-> B\n\
    classDef midnight fill:#161D2E,stroke:#657086\nclass G midnight\n\
    linkStyle 1 stroke:#D97706\nlinkStyle 2 stroke-width:2px";

fn render(keyword: &str, backend: &str, look: &str, body: &str, resvg: bool) -> String {
    let source = format!(
        "---\nconfig:\n  layout: {backend}\n  look: {look}\n  htmlLabels: false\n---\n{keyword} LR\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = ThemeCompiler::new().compile_json(THEME, None).unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    if resvg {
        rendered
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .unwrap()
            .svg()
            .as_str()
            .to_owned()
    } else {
        rendered.svg().to_owned()
    }
}

fn edge<'a, 'input>(doc: &'a Document<'input>, id: &str) -> Node<'a, 'input> {
    doc.descendants()
        .find(|node| {
            node.attribute("data-edge") == Some("true") && node.attribute("data-id") == Some(id)
        })
        .unwrap_or_else(|| panic!("missing edge {id}"))
}

fn marker_shape<'a, 'input>(
    doc: &'a Document<'input>,
    edge: Node<'a, 'input>,
    attribute: &str,
) -> Node<'a, 'input> {
    let id = edge
        .attribute(attribute)
        .and_then(|value| value.strip_prefix("url(#"))
        .and_then(|value| value.strip_suffix(')'))
        .unwrap();
    doc.descendants()
        .find(|node| node.has_tag_name("marker") && node.attribute("id") == Some(id))
        .and_then(|marker| marker.children().find(Node::is_element))
        .unwrap()
}

const SURFACE_INK: &str = "merman-edge-surface-ink";

fn paint<'a, 'input>(node: Node<'a, 'input>, property: &str) -> Option<&'a str> {
    node.attribute("style")
        .into_iter()
        .flat_map(|style| style.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim().trim_end_matches("!important").trim())
        .next_back()
        .or_else(|| node.attribute(property))
}

fn surface_layer<'a, 'input>(
    doc: &'a Document<'input>,
    base: Node<'a, 'input>,
) -> (Node<'a, 'input>, Node<'a, 'input>) {
    let layer = base
        .next_sibling_element()
        .filter(|node| node.attribute("class") == Some(SURFACE_INK))
        .unwrap_or_else(|| {
            panic!(
                "missing surface repaint for {:?}",
                base.attribute("data-id")
            )
        });
    let path = layer.first_element_child().unwrap();
    assert_eq!(path.attribute("d"), base.attribute("d"));
    assert_eq!(path.attribute("class"), base.attribute("class"));
    assert_eq!(path.attribute("data-edge"), None);
    for property in [
        "stroke-width",
        "stroke-dasharray",
        "stroke-dashoffset",
        "stroke-linecap",
        "stroke-linejoin",
        "stroke-miterlimit",
    ] {
        assert_eq!(paint(path, property), paint(base, property), "{property}");
    }
    let clip_id = layer
        .attribute("clip-path")
        .and_then(|value| value.strip_prefix("url(#"))
        .and_then(|value| value.strip_suffix(')'))
        .unwrap();
    let frame = doc
        .descendants()
        .find(|node| node.has_tag_name("clipPath") && node.attribute("id") == Some(clip_id))
        .and_then(|clip| clip.first_element_child())
        .unwrap();
    assert!(frame.has_tag_name("rect"));
    for side in ["width", "height"] {
        assert!(frame.attribute(side).unwrap().parse::<f64>().unwrap() > 0.0);
    }
    (path, frame)
}

fn assert_ink(ink: &str, surface: &str) {
    use merman_theme_contract::v2::Color;

    let ink = Color::parse(ink).unwrap();
    let surface = Color::parse(surface).unwrap();
    assert!(ink.contrast(surface) >= 3.0, "{ink} over {surface}");
}

fn assert_compound_paint(svg: &str, bidirectional: bool) {
    let doc = Document::parse(svg).unwrap();
    let inside = edge(&doc, "inside");
    let authored = edge(&doc, "authored");
    let cross = edge(&doc, "cross");
    assert!(
        inside
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF")
    );
    let expected_markers = if bidirectional {
        &["marker-start", "marker-end"][..]
    } else {
        assert!(inside.attribute("marker-start").is_none());
        &["marker-end"][..]
    };
    for end in expected_markers {
        let shape = marker_shape(&doc, inside, end);
        assert_eq!(shape.attribute("stroke"), Some("#FFFFFF"));
        assert_eq!(shape.attribute("fill"), Some("#FFFFFF"));
    }
    assert!(
        authored
            .attribute("style")
            .unwrap()
            .contains("stroke:#D97706")
    );
    assert_eq!(
        marker_shape(&doc, authored, "marker-end").attribute("stroke"),
        Some("#D97706")
    );
    let (path, _) = surface_layer(&doc, cross);
    let ink = paint(path, "stroke").unwrap();
    assert_ink(ink, "#161D2E");
    assert_eq!(paint(path, "stroke-width"), Some("2px"));
    assert_ne!(path.attribute("marker-end"), cross.attribute("marker-end"));
    let marker = marker_shape(&doc, path, "marker-end");
    assert_eq!(marker.attribute("fill"), Some(ink));
    assert_eq!(marker.attribute("stroke"), Some(ink));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_paints_fill_boundaries_and_adapts_internal_markers() {
    assert_compound_paint(&render("flowchart", "shoal", "neo", COMPOUND, false), true);
    assert_compound_paint(
        &render("flowchart", "shoal", "classic", COMPOUND, true),
        true,
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_authored_edge_hues_and_markers_survive_contrast_adjustment() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        concat!(
            "subgraph Light\nA blue@<--> B\nB orange@--> C\nend\n",
            "subgraph Dark\nD purple@--> E\nend\n",
            "style Light fill:#EFF4FA\nstyle Dark fill:#111827\n",
            "linkStyle 0 stroke:#0EA5E9\nlinkStyle 1 stroke:#D97706\nlinkStyle 2 stroke:#2D1359\n",
        ),
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    for (id, source, backdrop) in [
        ("blue", "#0EA5E9", "#EFF4FA"),
        ("orange", "#D97706", "#EFF4FA"),
        ("purple", "#2D1359", "#111827"),
    ] {
        let route = edge(&doc, id);
        let stroke = paint(route, "stroke").unwrap();
        assert_ne!(stroke, source, "{id}");
        assert_ink(stroke, backdrop);
        let source = merman_model::css_color::parse(source).unwrap().hsla;
        let ink = merman_model::css_color::parse(stroke).unwrap().hsla;
        assert!(
            (ink.hue_degrees - source.hue_degrees).abs() < 2.0,
            "{id}: {stroke}"
        );
        assert!(ink.saturation_percent > 50.0, "{id}: {stroke}");
        for attribute in ["marker-start", "marker-end"] {
            if route.attribute(attribute).is_some() {
                let marker = marker_shape(&doc, route, attribute);
                assert_eq!(marker.attribute("stroke"), Some(stroke), "{id}");
                assert_eq!(marker.attribute("fill"), Some(stroke), "{id}");
            }
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_hand_drawn_edges_adapt_over_dark_cluster_and_hatches_have_a_backing() {
    let svg = render(
        "flowchart",
        "shoal",
        "handDrawn",
        "subgraph G[Dark group]\nA([Start]) inside@<--> DB[(Database)]\nend\n\
         X crossing@-.-> A\nstyle G fill:#0B1120\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let internal = edge(&doc, "inside");
    let internal_ink = paint(internal, "stroke").unwrap();
    assert_ink(internal_ink, "#0B1120");
    for marker in ["marker-start", "marker-end"] {
        assert_eq!(
            marker_shape(&doc, internal, marker).attribute("stroke"),
            Some(internal_ink)
        );
    }
    let crossing = edge(&doc, "crossing");
    let (path, _) = surface_layer(&doc, crossing);
    assert_ink(paint(path, "stroke").unwrap(), "#0B1120");
    for id in ["A", "DB"] {
        let node = doc
            .descendants()
            .find(|node| {
                node.attribute("data-et") == Some("node") && node.attribute("data-id") == Some(id)
            })
            .unwrap();
        let backing = node
            .descendants()
            .find(|node| node.attribute("class") == Some("merman-rough-backing"))
            .expect("opaque node paint beneath rough hatches");
        assert_eq!(backing.attribute("stroke"), Some("none"));
        let hatch = backing.next_sibling_element().unwrap();
        assert!(
            hatch
                .attribute("style")
                .unwrap()
                .contains("stroke-opacity:0.15")
        );
        assert!(backing.range().end <= hatch.range().start);
        if id == "DB" {
            let clip = hatch.parent_element().unwrap();
            let clip_id = clip
                .attribute("clip-path")
                .and_then(|value| value.strip_prefix("url(#"))
                .and_then(|value| value.strip_suffix(')'))
                .expect("cylinder hatches use a silhouette clip");
            let silhouette = node
                .descendants()
                .find(|element| {
                    element.has_tag_name("clipPath") && element.attribute("id") == Some(clip_id)
                })
                .and_then(|element| element.first_element_child())
                .unwrap();
            assert_eq!(silhouette.attribute("d"), backing.attribute("d"));
        }
    }
    assert!(doc.descendants().any(|node| {
        node.attribute("class") == Some("merman-rough-container-outline")
            && paint(node, "fill") == Some("none")
            && node.attribute("d").is_some_and(|path| path.contains('C'))
    }));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_uses_the_parent_paint_at_a_nested_boundary() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        concat!(
            "subgraph Outer\nA[Outside inner]\nsubgraph Inner\nB inside@--> C\nend\n",
            "A cross@--> B\nend\n",
            "style Outer fill:#DAE9F7\nstyle Inner fill:#141C31,fill-opacity:0.8\n",
        ),
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let (path, _) = surface_layer(&doc, edge(&doc, "cross"));
    assert_ink(paint(path, "stroke").unwrap(), "#3C4559");
    assert!(
        edge(&doc, "inside")
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF")
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_skips_transparent_fills_and_authored_invisible_strokes() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        concat!(
            "subgraph G\nA inside@--> B\nend\nX transparent@--> A\nstyle G fill:#000000,fill-opacity:0\n",
            "subgraph H\nC --> D\nend\nY hidden@--> C\nstyle H fill:#101A24\n",
            "linkStyle 3 stroke:none\n",
        ),
        false,
    );
    assert!(!svg.contains(SURFACE_INK));
    let doc = Document::parse(&svg).unwrap();
    assert!(
        !edge(&doc, "inside")
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF")
    );
    assert!(
        edge(&doc, "hidden")
            .attribute("style")
            .unwrap()
            .contains("stroke:none")
    );
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn shoal_agentflow_paints_resolved_subgraph_fill() {
    let source = COMPOUND
        .replacen("subgraph ", "flow ", 1)
        .replace("<-->", "-->")
        .replace("-.->", "-.-");
    assert_compound_paint(
        &render("agentflow-beta", "shoal", "classic", &source, false),
        false,
    );
}

#[cfg(all(
    feature = "diagram-flowchart",
    any(feature = "layout-dagre", feature = "layout-elk")
))]
#[test]
fn flowchart_parity_backends_keep_the_original_edge_paint_with_the_same_theme() {
    for backend in [
        #[cfg(feature = "layout-dagre")]
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        let svg = render("flowchart", backend, "classic", COMPOUND, false);
        assert!(!svg.contains(SURFACE_INK));
        let doc = Document::parse(&svg).unwrap();
        assert!(
            !edge(&doc, "inside")
                .attribute("style")
                .unwrap()
                .contains("stroke:#FFFFFF")
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_flowchart_default_theme_cases_solid_crossing_into_dark_cluster_without_canvas_flag() {
    for theme_line in ["", "  theme: default\n"] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n{theme_line}  htmlLabels: false\n---\nflowchart LR\n\
            subgraph Dark[Dark Zone]\nA inside@--> B\nend\nX cross@--> A\n\
            style Dark fill:#0b1120,stroke:#334155\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let doc = Document::parse(svg.svg()).unwrap();
        let (path, _) = surface_layer(&doc, edge(&doc, "cross"));
        assert_ink(paint(path, "stroke").unwrap(), "#0B1120");
        assert!(
            edge(&doc, "inside")
                .attribute("style")
                .unwrap()
                .contains("stroke:#FFFFFF")
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_node_to_nested_subgraph_edges_get_adaptive_ink_in_dark_and_light_clusters() {
    // c05: dark custom outer cluster in forest theme with nested dark (and light) subgraph.
    for inner_fill in ["#1E293B", "#F8FAFC"] {
        let source = format!(
            "---\nconfig:\n  layout: shoal\n  theme: forest\n  htmlLabels: false\n---\nflowchart LR\n\
            subgraph Outer[Dark Control Plane]\n\
                Scheduler[Scheduler]\n\
                subgraph sg_inner_dark[Nested Stage]\n\
                    Worker[Worker]\n\
                end\n\
                Reporter[Reporter]\n\
                Scheduler enter@--> sg_inner_dark\n\
                sg_inner_dark exit@--> Reporter\n\
            end\n\
            style Outer fill:#0B1120,stroke:#334155,color:#F8FAFC\n\
            style sg_inner_dark fill:{inner_fill},stroke:#475569\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let doc = Document::parse(svg.svg()).unwrap();
        for id in ["enter", "exit"] {
            let e = edge(&doc, id);
            assert!(
                e.attribute("style")
                    .unwrap_or("")
                    .contains("stroke:#FFFFFF"),
                "c05 ({inner_fill}): expected adaptive white ink on {id}, got {:?}",
                e.attribute("style")
            );
            assert_eq!(
                marker_shape(&doc, e, "marker-end").attribute("stroke"),
                Some("#FFFFFF"),
                "c05 ({inner_fill}): expected white marker on {id}"
            );
        }
    }

    // c08: light custom outer cluster in dark studio theme with nested subgraph.
    let dark_theme_json = br##"{
        "schema_version":2,"extends":"studio",
        "roles":{"canvas":"#0F172A","surface.container":"#1E293B","stroke.connector":"#E2E8F0"},
        "form":{"container_radius":4,"elevation":"flat","marker_style":"filled"}
    }"##;
    let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n---\nflowchart LR\n\
        subgraph Outer[Light Control Plane]\n\
            Scheduler[Scheduler]\n\
            subgraph sg_inner[Nested Stage]\n\
                Worker[Worker]\n\
            end\n\
            Reporter[Reporter]\n\
            Scheduler enter@--> sg_inner\n\
            sg_inner exit@--> Reporter\n\
        end\n\
        style Outer fill:#F8FAFC,stroke:#94A3B8,color:#0F172A\n\
        style sg_inner fill:#E2E8F0,stroke:#64748B\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = ThemeCompiler::new()
        .compile_json(dark_theme_json, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = Document::parse(svg.svg()).unwrap();
    for id in ["enter", "exit"] {
        let e = edge(&doc, id);
        assert!(
            e.attribute("style")
                .unwrap_or("")
                .contains("stroke:#000000"),
            "c08: expected adaptive black ink on {id}, got {:?}",
            e.attribute("style")
        );
        assert_eq!(
            marker_shape(&doc, e, "marker-end").attribute("stroke"),
            Some("#000000"),
            "c08: expected black marker on {id}"
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_nested_crossings_repaint_in_surface_order() {
    for inner in ["#F8FAFC", "var(--local-fill)"] {
        let svg = render(
            "flowchart",
            "shoal",
            "classic",
            &format!(
                "subgraph Outer\nA[Dark]\nsubgraph Inner\nB[Inner]\nend\nA cross@--> B\nend\n\
                 style Outer fill:#0B1120\nstyle Inner fill:{inner}\n"
            ),
            false,
        );
        let doc = Document::parse(&svg).unwrap();
        let base = edge(&doc, "cross");
        let base_ink = paint(base, "stroke").unwrap_or("#202938");
        assert_ink(base_ink, "#0B1120");
        if inner.starts_with('#') {
            let (path, _) = surface_layer(&doc, base);
            assert_ink(paint(path, "stroke").unwrap(), inner);
        } else {
            assert!(!svg.contains(SURFACE_INK));
        }
    }

    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph Outer\nsubgraph Inner\nA inside@--> B\nend\nend\n\
         style Outer fill:#0B1120\nstyle Inner fill:var(--local-fill)\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    assert_eq!(paint(edge(&doc, "inside"), "stroke"), Some("#FFFFFF"));
    assert!(!svg.contains(SURFACE_INK));

    // The authored stroke remains legible on the destination after adapting to the canvas.
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph Dark\nA[Inside]\nend\nX cross@--> A\nstyle Dark fill:#0B1120\nlinkStyle 0 stroke:#CBD5E1\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let base = edge(&doc, "cross");
    assert_ink(paint(base, "stroke").unwrap(), "#FAF7EC");
    let (path, _) = surface_layer(&doc, base);
    use merman_theme_contract::v2::Color;
    let authored = Color::parse("#CBD5E1").unwrap();
    assert_eq!(
        Color::parse(paint(path, "stroke").unwrap()).unwrap(),
        authored
    );
    for property in ["fill", "stroke"] {
        assert_eq!(
            Color::parse(
                marker_shape(&doc, path, "marker-end")
                    .attribute(property)
                    .unwrap()
            )
            .unwrap(),
            authored
        );
    }

    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph Outer\nA[Outer]\nsubgraph Inner\nB[Inner]\nend\nA darkcross@--> B\nend\n\
         style Outer fill:#0B1120\nstyle Inner fill:#0F172A\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    assert!(
        edge(&doc, "darkcross")
            .attribute("style")
            .unwrap_or("")
            .contains("stroke:#FFFFFF"),
        "similarly dark nested crossing must emit adaptive white ink: {svg}"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_edge_ink_skips_restoring_on_disjoint_surfaces() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph Light\nB[To]\nend\nsubgraph Dark\nA[From]\nend\nA cross@--> B\n\
         style Dark fill:#0B1120\nstyle Light fill:#F8FAFC\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let base = edge(&doc, "cross");
    let (path, _) = surface_layer(&doc, base);
    assert_ink(paint(path, "stroke").unwrap(), "#0B1120");
    assert_eq!(
        base.next_siblings()
            .skip(1)
            .filter(Node::is_element)
            .take_while(|node| node.attribute("class") == Some(SURFACE_INK))
            .count(),
        1,
        "the light surface owns separate ink"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_edge_ink_keeps_authored_dash_cap_and_join() {
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        concat!(
            "subgraph Dark\nA[Inside]\nend\nX cross@--> A\nstyle Dark fill:#0B1120\n",
            "linkStyle 0 stroke:#202938,stroke-width:2px,stroke-dasharray:5 2,",
            "stroke-dashoffset:1,stroke-linecap:round,stroke-linejoin:bevel,stroke-miterlimit:4\n",
        ),
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let base = edge(&doc, "cross");
    let (path, _) = surface_layer(&doc, base);
    assert_ink(paint(path, "stroke").unwrap(), "#0B1120");
    for (property, expected) in [
        ("stroke-dasharray", "5 2"),
        ("stroke-dashoffset", "1"),
        ("stroke-linecap", "round"),
        ("stroke-linejoin", "bevel"),
    ] {
        assert_eq!(paint(path, property), Some(expected));
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_redux_dark_color_neo_dark_and_single_cluster_isolation() {
    // redux-dark-color flowchart must use cluster_base_fill, not agentflow bkgColorArray,
    // so light default stroke on dark cluster stays light (not black-on-dark!).
    let redux_source = "---\nconfig:\n  layout: shoal\n  theme: redux-dark-color\n  htmlLabels: false\n---\nflowchart LR\n\
        subgraph G[Group]\nA inside@--> B\nend\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(redux_source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = Document::parse(svg.svg()).unwrap();
    assert!(
        !edge(&doc, "inside")
            .attribute("style")
            .unwrap_or("")
            .contains("stroke:#000000"),
        "redux-dark-color must not falsely treat flowchart clusters as light palette fills: {}",
        svg.svg()
    );

    // neo-dark with custom light cluster gets adaptive #000000 ink.
    let neo_dark_source = "---\nconfig:\n  layout: shoal\n  theme: neo-dark\n  htmlLabels: false\n---\nflowchart LR\n\
        subgraph Light[Light Box]\nA inside@--> B\nend\nstyle Light fill:#F8FAFC,stroke:#94A3B8\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(neo_dark_source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = Document::parse(svg.svg()).unwrap();
    assert!(
        edge(&doc, "inside")
            .attribute("style")
            .unwrap_or("")
            .contains("stroke:#000000"),
        "neo-dark custom light cluster must get adaptive black ink: {:?}",
        edge(&doc, "inside").attribute("style")
    );

    // One cluster with non-allowlisted `rx:8` must only isolate that cluster, not disable
    // adaptive ink in a separate proven dark cluster.
    let isolated_svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph CustomRx[Rounded]\nA custom@--> B\nend\n\
         subgraph Dark[Dark]\nC inside@--> D\nend\n\
         style CustomRx fill:#0B1120,rx:8\n\
         style Dark fill:#0B1120\n",
        false,
    );
    let doc = Document::parse(&isolated_svg).unwrap();
    assert!(
        !edge(&doc, "custom")
            .attribute("style")
            .unwrap_or("")
            .contains("stroke:#FFFFFF"),
        "unproven rx:8 cluster should skip adaptive ink"
    );
    assert!(
        edge(&doc, "inside")
            .attribute("style")
            .unwrap_or("")
            .contains("stroke:#FFFFFF"),
        "sibling proven dark cluster must still receive adaptive white ink"
    );
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn shoal_compat_agentflow_dark_container_gets_adaptive_ink() {
    let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n---\nagentflow-beta LR\n\
        flow Dark[Dark Flow]\nA inside@--> B\nend\nstyle Dark fill:#0B1120,stroke:#334155\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let doc = Document::parse(svg.svg()).unwrap();
    assert!(
        edge(&doc, "inside")
            .attribute("style")
            .unwrap_or("")
            .contains("stroke:#FFFFFF"),
        "compat agentflow dark container must get adaptive white ink: {:?}",
        edge(&doc, "inside").attribute("style")
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn shoal_transparent_canvas_with_none_cluster_fill_omits_white_title_halo() {
    use merman_render::theme::Color;

    let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: true\n---\nflowchart TB\n\
        subgraph G[A long title with no room beside the entering route]\n\
            A[Short]\n\
        end\n\
        Outside trunk@--> A\n\
        style G fill:none,stroke:#60A5FA\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"tokyo"}"#, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap()
        .with_canvas_background(Color::TRANSPARENT);
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let svg = rendered.svg();
    assert!(
        !svg.contains(r##"stroke="#FFFFFF""##) && !svg.contains("-webkit-text-stroke:5px #FFFFFF"),
        "transparent canvas with fill:none cluster must never emit a blinding white halo: {svg}"
    );
}
