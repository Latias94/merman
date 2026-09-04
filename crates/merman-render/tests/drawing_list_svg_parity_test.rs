use merman_core::{
    Engine, OperationControl, ParseOptions, baseline::PINNED_MERMAID_BASELINE_VERSION,
};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render_svg(source: &str, diagram_id: &str) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::lenient())
        .expect("source parses")
        .expect("source detects a diagram");
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(OperationControl::new())
        .expect("render session starts");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("family layout succeeds")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("canonical SVG succeeds")
        .svg()
        .to_owned()
}

#[test]
fn info_canonical_svg_keeps_document_root_and_version_structure() {
    let svg = render_svg("info", "info-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Info SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("info"));
    assert_eq!(root.attribute("viewBox"), None);
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 400px; background-color: white;")
    );
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("text") && node.attribute("class") == Some("version"))
    );
    assert!(svg.contains(&format!(">v{PINNED_MERMAID_BASELINE_VERSION}</text>")));
}

#[test]
fn error_canonical_svg_keeps_source_backed_icon_and_text_classes() {
    let svg = render_svg("flowchart TD\nA -->\n", "error-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Error SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("error"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 2412 512"));
    assert_eq!(root.attribute("style"), Some("max-width: 512px;"));
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("error-icon"))
            .count(),
        6
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("error-text"))
            .count(),
        2
    );
    assert!(svg.contains("Syntax error in text"));
    assert!(svg.contains(&format!(
        "mermaid version {PINNED_MERMAID_BASELINE_VERSION}"
    )));
}

#[test]
fn packet_canonical_svg_keeps_root_profile_and_packet_dom_roles() {
    let svg = render_svg(
        "packet\ntitle Header\n0-7: \"Version\"\n8-15: \"Length\"\n",
        "packet-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Packet SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), None);
    assert_eq!(root.attribute("viewBox"), Some("0 0 1026 94"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 1026px; background-color: white;")
    );
    assert_eq!(
        document
            .descendants()
            .filter(
                |node| node.has_tag_name("rect") && node.attribute("class") == Some("packetBlock")
            )
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetLabel"))
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetByte start"))
            .count(),
        2
    );
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("packetByte end"))
            .count(),
        2
    );
    assert!(svg.contains("chart-title-packet-parity"));
}

#[test]
fn radar_canonical_svg_keeps_root_profile_and_family_roles() {
    let svg = render_svg(
        "radar-beta\ntitle Radar parity\naxis A,B,C\ncurve score{1,2,3}\n",
        "radar-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Radar SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("overflow"), Some("visible"));
    assert_eq!(root.attribute("viewBox"), Some("0 0 700 700"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 700px; background-color: white;")
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarTitle"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarAxisLabel"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarCurve-0"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarLegendBox-0"))
    );
}

#[test]
fn xychart_canonical_svg_keeps_root_profile_theme_and_group_roles() {
    let svg = render_svg(
        "xychart\n  title Sales\n  x-axis [A, B]\n  y-axis 0 --> 100\n  bar [40, 60]\n  line [30, 70]\n",
        "xychart-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical XYChart SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 700 500"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 700px; background-color: white;")
    );
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|token| token == "main"))
    }));
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|token| token == "bar-plot-0"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("xychart.text.0.0") })
    );
}

#[test]
fn quadrantchart_canonical_svg_keeps_root_profile_and_dom_roles() {
    let svg = render_svg(
        "quadrantChart\n  accTitle: Quadrant parity\n  title Portfolio\n  x-axis Low --> High\n  y-axis Bottom --> Top\n  quadrant-1 Invest\n  quadrant-2 Explore\n  quadrant-3 Retire\n  quadrant-4 Maintain\n  Feature: [0.7, 0.8]\n",
        "quadrantchart-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical QuadrantChart SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 500 500"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 500px; background-color: white;")
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-quadrantchart-parity")
    );
    for class in [
        "main",
        "quadrants",
        "border",
        "data-points",
        "labels",
        "title",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical QuadrantChart class {class:?}"
        );
    }
    assert!(document.descendants().any(|node| node.has_tag_name("rect")));
    assert!(document.descendants().any(|node| node.has_tag_name("line")));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("circle"))
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("quadrantchart.point.0")
    }));
}

#[test]
fn pie_canonical_svg_keeps_root_profile_and_chart_roles() {
    let svg = render_svg(
        "pie\n  accTitle: Pie parity\n  title Releases\n  \"Stable\" : 3\n  \"Alpha\" : 1\n",
        "pie-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Pie SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 556.2 450"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 556.2px; background-color: white;")
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-pie-parity")
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("circle") && node.attribute("class") == Some("pieOuterCircle")
    }));
    assert!(
        document.descendants().any(|node| {
            node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")
        })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.attribute("class") == Some("slice") })
    );
    assert!(document.descendants().any(|node| {
        node.attribute("class")
            .is_some_and(|class| class.split_whitespace().any(|token| token == "legend"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("pieTitleText")
    }));
    assert!(svg.contains("data-merman-resource=\"pie.slice.0.shape\""));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn timeline_canonical_svg_keeps_node_connector_and_axis_roles() {
    let svg = render_svg(
        "timeline\n  accTitle: Timeline parity\n  section Release\n    2026 : Ship\n",
        "timeline-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Timeline SVG is XML");
    let root = document.root_element();

    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-timeline-parity")
    );
    for class in [
        "timeline-node",
        "taskWrapper",
        "eventWrapper",
        "lineWrapper",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Timeline class {class:?}"
        );
    }
    assert!(document.descendants().any(|node| node.has_tag_name("line")));
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-resource")
            .is_some_and(|id| id.ends_with(".arrowhead"))
    }));
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("Ship") })
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn sankey_canonical_svg_keeps_nodes_labels_links_and_gradients() {
    let svg = render_svg("sankey-beta\nA,B,10\n", "sankey-parity");
    let document = roxmltree::Document::parse(&svg).expect("canonical Sankey SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 600 400"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 600px; background-color: white;")
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("g")
            && node
                .attribute("class")
                .is_some_and(|class| class.split_whitespace().any(|token| token == "node"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect") && node.attribute("data-merman-resource").is_some()
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("linearGradient"))
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node
                .attribute("class")
                .is_some_and(|class| class.split_whitespace().any(|token| token == "link-path"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.text().is_some_and(|text| text.contains("A"))
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn venn_canonical_svg_keeps_area_roles_labels_and_theme() {
    let svg = render_svg(
        "venn-beta\n title Product Surface\n set A[\"Core\"]:20\n set B[\"Editor\"]:14\n union A,B[\"Shared\"]:4\n",
        "venn-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Venn SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("viewBox"), Some("0 0 800 450"));
    assert_eq!(
        root.attribute("style"),
        Some("max-width: 800px; background-color: white;")
    );
    assert!(svg.contains("#venn-parity .venn-title"));
    for (sets, class) in [
        ("A", "venn-circle"),
        ("B", "venn-circle"),
        ("A_B", "venn-intersection"),
    ] {
        assert!(document.descendants().any(|node| {
            node.has_tag_name("g")
                && node.attribute("data-venn-sets") == Some(sets)
                && node
                    .attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
        }));
    }
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-merman-resource") == Some("venn.area.0.shape")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.attribute("class") == Some("venn-title")
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text")
            && node.attribute("class") == Some("label")
            && node.text() == Some("Core")
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn railroad_canonical_svg_keeps_rule_roles_connectors_and_theme() {
    let svg = render_svg(
        "railroad-beta\naccTitle: Railroad parity\nexpr = sequence(nonterminal(\"term\"), terminal(\"+\"), special(\"guard\")) ;\n",
        "railroad-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Railroad SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("class"), Some("railroad-diagram"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-railroad-parity")
    );
    assert!(svg.contains("#railroad-parity .railroad-terminal"));
    for class in [
        "railroad-rule",
        "railroad-terminal",
        "railroad-nonterminal",
        "railroad-special",
        "railroad-line",
        "railroad-start",
        "railroad-end",
        "railroad-rule-name",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Railroad class {class:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("railroad.rule.0") })
    );
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-resource")
            .is_some_and(|id| id == "railroad.rule.0.connector.start.path")
    }));
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("text") && node.text() == Some("term"))
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn eventmodeling_canonical_svg_keeps_swimlanes_boxes_relations_and_text() {
    let svg = render_svg(
        "eventmodeling\ntf 01 ui Web.ShopCart\ntf 02 cmd Cart.AddItem ->> 01 { sku: \"SKU-1\" }\ntf 03 evt Cart.ItemAdded ->> 02\n",
        "eventmodeling-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical EventModeling SVG is XML");
    let root = document.root_element();

    assert_eq!(
        root.attribute("aria-roledescription"),
        Some("eventmodeling")
    );
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    for class in ["em-swimlane", "em-box", "em-relation", "em-arrowhead"] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical EventModeling class {class:?}"
        );
    }
    assert!(document.descendants().any(|node| {
        node.attribute("data-merman-semantic-id") == Some("eventmodeling.relation.0")
    }));
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("ShopCart") })
    );
    assert!(!svg.contains("<foreignObject"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn ishikawa_canonical_svg_keeps_fishbone_geometry_and_semantic_labels() {
    let svg = render_svg(
        "ishikawa-beta\n    Blurry Photo\n    Process\n        Out of focus\n        Shutter speed too slow\n    User\n        Shaky hands\n",
        "ishikawa-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Ishikawa SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("ishikawa"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "ishikawa",
        "ishikawa-spine",
        "ishikawa-branch",
        "ishikawa-sub-branch",
        "ishikawa-head-label",
        "ishikawa-label-box",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Ishikawa class {class:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("ishikawa.head") })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("Out of focus") })
    );
    assert!(!svg.contains("<marker") && !svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn cynefin_canonical_svg_keeps_domains_transitions_and_accessibility() {
    let svg = render_svg(
        "cynefin-beta\n  complex\n    \"Observe\"\n  complicated\n    \"Analyze\"\n  complex --> complicated : \"move\"\n",
        "cynefin-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Cynefin SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("cynefin"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "cynefin",
        "cynefinDomain",
        "cynefinBoundary",
        "cynefinCliff",
        "cynefinConfusion",
        "cynefinDomainLabel",
        "cynefinItem",
        "cynefinItemText",
        "cynefinArrowLine",
        "cynefinArrowHead",
        "cynefinArrowLabel",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Cynefin class {class:?}"
        );
    }
    assert!(
        document.descendants().any(|node| {
            node.attribute("data-merman-semantic-id") == Some("cynefin.transition.0")
        })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("Observe") })
    );
    assert!(!svg.contains("<marker") && !svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn tree_view_canonical_svg_keeps_lines_icons_labels_and_semantics() {
    let svg = render_svg(
        "treeView-beta\nsrc/ :::highlight icon(folder) ## source directory\n    main.rs icon(file) ## entry point\n",
        "tree-view-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical TreeView SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("treeView"));
    assert!(root.attribute("viewBox").is_some());
    for class in [
        "tree-view",
        "treeView-node-line",
        "treeView-node-icon",
        "treeView-node-label",
        "treeView-node-dir",
        "treeView-node-description",
        "treeView-highlight-bg",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical TreeView class {class:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| { node.attribute("data-merman-semantic-id") == Some("treeView.node.0") })
    );
    assert!(
        document
            .descendants()
            .any(|node| { node.has_tag_name("text") && node.text() == Some("main.rs") })
    );
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn gantt_canonical_svg_keeps_axes_tasks_states_ids_and_semantics() {
    let svg = render_svg(
        "gantt\n  title Release Plan\n  dateFormat YYYY-MM-DD\n  topAxis\n  todayMarker off\n  section Core\n  Build :a1, 2026-01-01, 4d\n  Ship :crit, milestone, 2026-01-05, 1d\n  section Follow-up\n  Docs :done, 2026-01-06, 2d\n",
        "gantt-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Gantt SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("gantt"));
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    for class in [
        "grid",
        "tick",
        "section0",
        "section1",
        "task0",
        "done1",
        "critText0",
        "doneText1",
        "milestoneText",
        "sectionTitle",
        "titleText",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Gantt class {class:?}"
        );
    }
    for semantic_id in [
        "gantt.axis.top",
        "gantt.axis.bottom",
        "gantt.section.0",
        "gantt.task.0",
        "gantt.task.1",
        "gantt.task.2",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| { node.attribute("data-merman-semantic-id") == Some(semantic_id) }),
            "expected canonical Gantt semantic id {semantic_id:?}"
        );
    }
    for dom_id in [
        "gantt-parity-a1",
        "gantt-parity-a1-text",
        "gantt-parity-task1",
        "gantt-parity-task1-text",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(dom_id)),
            "expected scoped Gantt DOM id {dom_id:?}"
        );
    }
    assert!(document.descendants().any(|node| {
        node.has_tag_name("rect")
            && node
                .attribute("class")
                .is_some_and(|value| value.split_whitespace().any(|token| token == "task"))
    }));
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.text().is_some_and(|text| text == "Release Plan")
    }));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}

#[test]
fn journey_canonical_svg_keeps_faces_sections_actors_and_activity_axis() {
    let svg = render_svg(
        "journey\n  title User checkout\n  section Checkout\n    Sign Up: 5: Alice\n    Pay: 3: Bob\n    Review: 1: Alice\n",
        "journey-parity",
    );
    let document = roxmltree::Document::parse(&svg).expect("canonical Journey SVG is XML");
    let root = document.root_element();

    assert_eq!(root.attribute("aria-roledescription"), Some("journey"));
    assert_eq!(
        root.attribute("aria-labelledby"),
        Some("chart-title-journey-parity")
    );
    assert!(root.attribute("viewBox").is_some());
    assert!(
        root.attribute("style")
            .is_some_and(|style| style.contains("background-color: white;"))
    );
    assert_eq!(root.attribute("preserveAspectRatio"), Some("xMinYMin meet"));
    for class in [
        "legend",
        "journey-section",
        "section-type-0",
        "task",
        "task-type-0",
        "task-line",
        "face",
        "mouth",
        "actor-0",
        "actor-1",
    ] {
        assert!(
            document.descendants().any(|node| {
                node.attribute("class")
                    .is_some_and(|value| value.split_whitespace().any(|token| token == class))
            }),
            "expected canonical Journey class {class:?}"
        );
    }
    for semantic_id in [
        "journey.document",
        "journey.actor.0",
        "journey.section.0",
        "journey.task.0",
        "journey.task.1",
        "journey.activity",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| { node.attribute("data-merman-semantic-id") == Some(semantic_id) }),
            "expected canonical Journey semantic id {semantic_id:?}"
        );
    }
    for dom_id in [
        "journey-parity-task0",
        "journey-parity-task1",
        "journey-parity-task2",
    ] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(dom_id)),
            "expected scoped Journey task line id {dom_id:?}"
        );
    }
    assert!(
        document
            .descendants()
            .any(|node| node.has_tag_name("circle"))
    );
    assert!(document.descendants().any(|node| {
        node.has_tag_name("text") && node.text().is_some_and(|text| text == "User checkout")
    }));
    assert!(svg.contains(r#"data-merman-resource="journey.activity.arrowhead""#));
    assert!(!svg.contains("<foreignObject"));
    assert!(!svg.contains("NaN") && !svg.contains("Infinity"));
}
