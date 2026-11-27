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
    let casings = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-casings"))
        .unwrap();
    let paths: Vec<_> = casings.children().filter(Node::is_element).collect();
    let [path] = paths.as_slice() else {
        panic!("expected a casing only for the visible solid crossing");
    };
    assert_eq!(path.attribute("d"), cross.attribute("d"));
    assert_eq!(path.attribute("stroke"), Some("#FAF7EC"));
    assert_eq!(path.attribute("stroke-width"), Some("5"));
    assert_eq!(path.attribute("stroke-linecap"), Some("round"));
    assert_eq!(path.attribute("stroke-linejoin"), Some("round"));
    assert_ne!(path.attribute("marker-end"), cross.attribute("marker-end"));
    let casing_marker = marker_shape(&doc, *path, "marker-end");
    assert_eq!(casing_marker.attribute("fill"), Some("#FAF7EC"));
    assert_eq!(casing_marker.attribute("stroke"), Some("#FAF7EC"));
    assert_eq!(casing_marker.attribute("stroke-linecap"), Some("round"));
    assert_eq!(casing_marker.attribute("stroke-linejoin"), Some("round"));
    assert_eq!(path.attribute("data-edge"), None);
    assert!(casings.range().end < inside.range().start);
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
    assert!(
        internal
            .attribute("style")
            .unwrap()
            .contains("stroke:#FFFFFF;")
    );
    for marker in ["marker-start", "marker-end"] {
        assert_eq!(
            marker_shape(&doc, internal, marker).attribute("stroke"),
            Some("#FFFFFF")
        );
    }
    let crossing = edge(&doc, "crossing");
    let casing = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("d") == crossing.attribute("d")
                && node.parent().and_then(|parent| parent.attribute("class"))
                    == Some("merman-edge-casings")
        })
        .expect("solid colored cluster has a casing on the actual rough crossing path");
    assert_eq!(casing.attribute("stroke-dasharray"), Some("2"));
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
    }
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
    let casings = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-casings"))
        .unwrap();
    let casing = casings.first_element_child().unwrap();
    assert_eq!(casing.attribute("d"), edge(&doc, "cross").attribute("d"));
    assert_eq!(casing.attribute("stroke"), Some("#DAE9F7"));
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
    assert!(!svg.contains("merman-edge-casings"));
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
        assert!(!svg.contains("merman-edge-casings"));
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
        let casings = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("merman-edge-casings"))
            .unwrap_or_else(|| {
                panic!(
                    "expected cross-boundary casing under default theme ({theme_line:?}) without explicit canvas flag"
                )
            });
        let casing = casings.first_element_child().unwrap();
        assert_eq!(casing.attribute("stroke"), Some("#FFFFFF"));
        assert_eq!(casing.attribute("d"), edge(&doc, "cross").attribute("d"));
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
fn shoal_casing_color_avoids_same_color_casing_and_similarly_dark_crossings_adapt_ink() {
    // Dark outer (#0B1120) -> light inner (#F8FAFC) under light theme (#333333 ink):
    // casing must pick high-contrast #F8FAFC, NOT dark #0B1120!
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph Outer\nA[Dark]\nsubgraph Inner\nB[Light]\nend\nA cross@--> B\nend\n\
         style Outer fill:#0B1120\nstyle Inner fill:#F8FAFC\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let casings = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-casings"))
        .expect("expected casing for dark-to-light nested crossing");
    let casing = casings.first_element_child().unwrap();
    assert_eq!(casing.attribute("stroke"), Some("#F8FAFC"));

    // Authored white stroke crossing from light canvas (#FAF7EC) into dark cluster (#0B1120):
    // casing must pick dark #0B1120, NOT low-contrast #FAF7EC!
    let svg = render(
        "flowchart",
        "shoal",
        "classic",
        "subgraph Dark\nA[Inside]\nend\nX cross@--> A\nstyle Dark fill:#0B1120\nlinkStyle 0 stroke:#FFFFFF\n",
        false,
    );
    let doc = Document::parse(&svg).unwrap();
    let casings = doc
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-edge-casings"))
        .expect("expected casing for authored white crossing into dark cluster");
    let casing = casings.first_element_child().unwrap();
    assert_eq!(casing.attribute("stroke"), Some("#0B1120"));

    // Crossing between similarly dark nested clusters (#0B1120 -> #0F172A) has no significant
    // boundary contrast for casing, so it must fall back to adaptive #FFFFFF ink!
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
