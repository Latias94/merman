use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use regex::Regex;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn edge_labels_group(svg: &str) -> &str {
    let start = svg
        .find(r#"<g class="edgeLabels">"#)
        .expect("expected edgeLabels group");
    let end = svg[start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| start + idx)
        .expect("expected nodes group after edgeLabels");
    &svg[start..end]
}

fn render_er_svg_from_text(text: &str, options: &SvgRenderOptions) -> String {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render svg")
        .svg()
        .to_owned()
}

fn entity_transform(svg: &str, entity_id: &str) -> (f64, f64) {
    let re = Regex::new(&format!(
        r#"id="merman-{}"[^>]*transform="translate\(([^,]+), ([^)]+)\)""#,
        regex::escape(entity_id)
    ))
    .expect("entity transform regex");
    let captures = re
        .captures(svg)
        .unwrap_or_else(|| panic!("missing transform for {entity_id}: {svg}"));
    (
        captures[1].parse().expect("entity x"),
        captures[2].parse().expect("entity y"),
    )
}

fn root_view_box(svg: &str) -> [f64; 4] {
    let re = Regex::new(r#"\bviewBox="([^\"]+)""#).expect("viewBox regex");
    let captures = re.captures(svg).expect("root viewBox");
    let values = captures[1]
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    values.try_into().expect("four viewBox numbers")
}

#[test]
fn er_svg_renders_entities_and_relationships() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join("upstream_attributes_styles_classes.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"id="merman-entity-BOOK-0""#));
    assert!(svg.contains(r#"data-look="neo""#));
    assert!(svg.contains(r#"id="merman-id_entity-BOOK-0_entity-PAGE-1_0""#));
    assert!(svg.contains(r#"id="merman-drop-shadow""#));
    assert!(svg.contains("relationshipLine"));
    let edge_style_pattern = if cfg!(feature = "layout-elk") {
        r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="stroke-dasharray: [^"]*; stroke-dashoffset: 0;fill:none;;;fill:none"[^>]*>"#
    } else {
        r#"<path[^>]*class="[^"]*relationshipLine[^"]*" style="stroke-dasharray: [^"]*; stroke-dashoffset: 0;undefined;;;undefined"[^>]*>"#
    };
    assert!(
        Regex::new(edge_style_pattern)
            .expect("relationship path regex")
            .is_match(&svg),
        "relationship paths should preserve the selected renderer's pathStyle serialization"
    );
    assert!(svg.contains("relationshipLabelBox"));
    assert!(
        svg.contains("marker") && svg.contains("merman_er-zeroOrMoreStart"),
        "expected Mermaid-like marker ids"
    );
    assert!(
        {
            let path_re = Regex::new(r#"<path[^>]*relationshipLine[^>]*>"#).expect("regex");
            let d_re = Regex::new(r#"\bd="([^"]*)"#).expect("regex");
            path_re.find_iter(&svg).any(|m| {
                let d = &d_re.captures(m.as_str()).expect("path data")[1];
                if cfg!(feature = "layout-elk") {
                    d.contains('L') && !d.contains('C')
                } else {
                    d.contains('C')
                }
            })
        },
        "expected ELK rounded segments or Dagre basis curves in relationship paths"
    );
    assert!(
        svg.contains("color: rgb(255, 255, 255) !important;"),
        "expected classDef text color to use the ER HTML label CSSOM path"
    );
}

#[test]
fn er_svg_recursive_relationship_is_one_logical_edge_and_one_label() {
    let path = workspace_root()
        .join("fixtures")
        .join("er")
        .join(
            "upstream_cypress_erdiagram_spec_should_render_an_er_diagram_with_a_recursive_relationship_002.mmd",
        );
    let text = std::fs::read_to_string(path).expect("fixture");

    let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"data-id="id_entity-CUSTOMER-0_entity-CUSTOMER-0_0""#));
    assert!(!svg.contains("cyclic-special"));
    assert_eq!(
        svg.matches(">refers<").count(),
        1,
        "the logical recursive relationship should own one relationship label: {svg}"
    );
}

#[test]
fn er_svg_uses_configured_look_in_dom_attributes() {
    let text = r#"%%{init: {"look": "neo"}}%%
erDiagram
  CUSTOMER ||--o{ ORDER : places
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(
        svg.contains(r#"data-look="neo""#),
        "expected ER SVG to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured ER look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn er_svg_renders_diagram_title_and_viewbox_includes_it() {
    let text = r#"---
title: Diagram Title
---
erDiagram
  A ||--o{ B : has
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    assert!(svg.contains(r#"class="erDiagramTitleText""#));
    assert!(svg.contains(">Diagram Title<"));
    assert!(svg.contains("viewBox="));
}

#[test]
fn er_svg_title_expands_negative_viewbox_without_rebasing_graph_content() {
    let untitled = r#"erDiagram
  A ||--o{ B : has
"#;
    let titled = r#"---
title: A deliberately wide diagram title
---
erDiagram
  A ||--o{ B : has
"#;

    let untitled_svg = render_er_svg_from_text(untitled, &SvgRenderOptions::default());
    let titled_svg = render_er_svg_from_text(titled, &SvgRenderOptions::default());

    assert_eq!(
        entity_transform(&untitled_svg, "entity-A-0"),
        entity_transform(&titled_svg, "entity-A-0")
    );
    assert!(root_view_box(&titled_svg)[1] < 0.0);
}

#[test]
fn er_svg_forest_theme_renders_root_gradient() {
    let text = r#"---
config:
  theme: forest
---
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(
        text,
        &SvgRenderOptions {
            diagram_id: Some("er_theme_gradient".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"<linearGradient id="er_theme_gradient-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
        "expected Mermaid 11.15 ER root gradient element: {svg}"
    );
}

#[test]
fn er_svg_relationship_labels_follow_root_htmllabels_not_flowchart_htmllabels() {
    let text = r#"%%{init: {"htmlLabels": true, "flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(svg.contains(r#"class="nodeLabel markdown-node-label""#));
    assert!(
        edge_labels.contains(r#"class="labelBkg""#)
            && edge_labels.contains(r#"<foreignObject width=""#),
        "expected ER relationship labels to keep HTML foreignObject output when root htmlLabels=true"
    );
}

#[test]
fn er_svg_relationship_labels_follow_flowchart_htmllabels_when_root_unset() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": false}}}%%
erDiagram
  A ||--|| B : owns
"#;

    let svg = render_er_svg_from_text(text, &SvgRenderOptions::default());

    let edge_labels = edge_labels_group(&svg);
    assert!(
        edge_labels.contains(r#"<rect class="background""#)
            && edge_labels.contains(">owns</tspan>")
            && edge_labels.contains(r#"text-anchor="middle""#)
            && !edge_labels.contains("<foreignObject"),
        "expected ER relationship labels to switch to SVG text when flowchart htmlLabels=false and root htmlLabels is unset"
    );
}

#[test]
fn er_svg_distinguishes_empty_and_whitespace_relationship_labels() {
    let svg = render_er_svg_from_text(
        r#"erDiagram
BOOK }|..|{ AUTHOR : ""
BOOK }|..|{ GENRE : " "
AUTHOR }|..|{ GENRE : "  "
"#,
        &SvgRenderOptions::default(),
    );
    let labels = edge_labels_group(&svg);
    assert_eq!(labels.matches(r#"<g class="edgeLabel""#).count(), 2);
    assert!(!labels.contains("undefined"));
    assert!(!labels.contains("NaN"));
    assert_eq!(labels.matches(r#"width="0" height="0""#).count(), 2);
}

#[test]
fn er_svg_row_fills_follow_optional_theme_colors() {
    let row_re = Regex::new(r#"<g[^>]*class="row-rect-(?:odd|even)"[^>]*>(.*?)</g>"#).unwrap();
    for (variables, expected_fills) in [
        (serde_json::json!({}), vec![]),
        (
            serde_json::json!({"rowOdd": "#123456", "rowEven": "#abcdef"}),
            vec!["#123456", "#abcdef"],
        ),
        (serde_json::json!({"rowOdd": "none", "rowEven": ""}), vec![]),
    ] {
        let config = serde_json::json!({"theme": "redux", "themeVariables": variables});
        let text = format!(
            "%%{{init: {config}}}%%\nerDiagram\n BOOK {{\n string title\n int pages\n }}\n"
        );
        let svg = render_er_svg_from_text(&text, &SvgRenderOptions::default());
        let rows = row_re.captures_iter(&svg).collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row[1].matches("<path ").count(),
                if expected_fills.is_empty() { 1 } else { 2 }
            );
            if let Some(fill) = expected_fills.get(index) {
                assert!(row[1].contains(&format!(r#"fill="{fill}""#)));
            }
        }
    }
}

#[test]
fn er_redux_explicit_styles_override_palette_on_every_table_path() {
    for theme in ["redux", "redux-dark", "redux-color", "redux-dark-color"] {
        for look in ["classic", "neo"] {
            for html_labels in [false, true] {
                let source = format!(
                    r#"---
config:
  theme: {theme}
  look: {look}
  layout: dagre
  htmlLabels: {html_labels}
---
erDiagram
  BOOK["Book"]:::core {{
    string title PK
    string author FK
  }}
  classDef core fill:#f96,stroke:#456,stroke-width:3px,color:#fff
  class BOOK core
  style BOOK fill:#f9f,stroke:#333,stroke-width:2px
"#
                );
                let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
                let document = roxmltree::Document::parse(&svg).unwrap();
                let entity = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some("merman-entity-BOOK-0"))
                    .expect("BOOK entity");
                for path in entity
                    .descendants()
                    .filter(|node| node.has_tag_name("path"))
                {
                    let style = path.attribute("style").unwrap_or_default();
                    assert!(
                        style.contains("fill:#f9f !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        style.contains("stroke:#333 !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        style.contains("stroke-width:2px !important"),
                        "{theme}/{look}: {style}"
                    );
                    assert!(
                        !style.contains("#f96"),
                        "class fill must lose to explicit style"
                    );
                }
            }
        }
    }
}

#[test]
fn er_non_redux_preserves_row_colors_and_styles_both_divider_paths() {
    let source = r#"---
config:
  theme: default
  look: neo
  layout: dagre
---
erDiagram
  BOOK {
    string title PK
    string author FK
  }
  style BOOK fill:#f9f,stroke:#333,stroke-width:2px,stroke-dasharray:4 2
"#;
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).unwrap();
    for group in document.descendants().filter(|node| {
        matches!(
            node.attribute("class"),
            Some("outer-path" | "row-rect-odd" | "row-rect-even" | "divider")
        )
    }) {
        for path in group.children().filter(|node| node.has_tag_name("path")) {
            let style = path.attribute("style").unwrap_or_default();
            assert!(
                style.contains("stroke:#333 !important"),
                "{}: {style}",
                group.attribute("class").unwrap()
            );
            // Mermaid erDiagram.jison skips whitespace in style blocks and joins tokens.
            assert!(style.contains("stroke-dasharray:42 !important"), "{style}");
            assert_eq!(
                style.contains("fill:#f9f !important"),
                group.attribute("class") == Some("row-rect-even")
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_paints_ancestors_before_descendants_in_stable_model_order() {
    let source = r#"---
config:
  layout: elk
---
erDiagram
subgraph ZOuter [Outer]
  subgraph AInner [Inner]
    subgraph Deepest [Deep]
      A
    end
    B
  end
  subgraph ZSibling [Sibling]
    C
  end
end
subgraph BPeer [Peer]
  D
end
A ||--|| C : crosses_siblings
B ||--|| D : crosses_roots
"#;
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let clusters = document
        .descendants()
        .find(|node| node.attribute("class") == Some("clusters"))
        .expect("clusters group");
    let ids: Vec<_> = clusters
        .children()
        .filter_map(|node| node.attribute("id"))
        .collect();
    // ER builds its model from reverse subgraph completion order. Stable depth ordering
    // preserves that sibling order while moving every parent before its descendants.
    assert_eq!(
        ids,
        [
            "merman-BPeer",
            "merman-ZOuter",
            "merman-ZSibling",
            "merman-AInner",
            "merman-Deepest"
        ]
    );
    let last_cluster = svg.find(r#"id="merman-Deepest""#).unwrap();
    let leaf_nodes = svg.find(r#"<g class="nodes">"#).unwrap();
    assert!(last_cluster < leaf_nodes);
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
        2
    );
    assert!(svg.contains("crosses_siblings") && svg.contains("crosses_roots"));
}

#[test]
fn er_svg_label_mode_covers_containers_entities_and_attribute_cells() {
    let body = r#"erDiagram
subgraph ZOuter [Outer Namespace]
  subgraph AInner [Inner Namespace]
    BOOK["Book **Title**"] {
      string name PK "visible comment"
      int edition FK
      type~T~ generic UK "generic value"
    }
    AUTHOR["Author Name"]
  end
end
BOOK ||--|| AUTHOR : writes
style ZOuter color:#123456
"#;
    let layouts = if cfg!(feature = "layout-elk") {
        vec!["dagre", "elk"]
    } else {
        vec!["dagre"]
    };
    for layout in layouts {
        for html_labels in [false, true] {
            let source = format!(
                "---\nconfig:\n  layout: {layout}\n  htmlLabels: {html_labels}\n---\n{body}"
            );
            let svg = render_er_svg_from_text(&source, &SvgRenderOptions::default());
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let foreign_objects = document
                .descendants()
                .filter(|node| node.has_tag_name("foreignObject"))
                .count();
            assert_eq!(
                foreign_objects > 0,
                html_labels,
                "{layout}/{html_labels}: {svg}"
            );
            for id in [
                "merman-ZOuter",
                "merman-AInner",
                "merman-entity-BOOK-0",
                "merman-entity-AUTHOR-1",
            ] {
                let owner = document
                    .descendants()
                    .find(|node| node.attribute("id") == Some(id))
                    .expect("label owner");
                assert_eq!(
                    owner
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    html_labels,
                    "{id}: {svg}"
                );
                assert_eq!(
                    owner.descendants().any(|node| node.has_tag_name("text")),
                    !html_labels,
                    "{id}: {svg}"
                );
            }
            let visible: String = document
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect();
            for text in [
                "Outer Namespace",
                "Inner Namespace",
                "Book Title",
                "Author Name",
                "string",
                "name",
                "PK",
                "visible comment",
                "int",
                "edition",
                "FK",
                "type<T>",
                "generic value",
                "writes",
            ] {
                assert!(
                    visible.contains(text),
                    "missing {text:?} in {layout}/{html_labels}: {visible}"
                );
            }
            let outer = document
                .descendants()
                .find(|node| node.attribute("id") == Some("merman-ZOuter"))
                .expect("outer group");
            let title = outer
                .descendants()
                .find(|node| node.has_tag_name(if html_labels { "span" } else { "text" }))
                .expect("styled title");
            assert!(
                title
                    .attribute("style")
                    .unwrap_or_default()
                    .contains(if html_labels {
                        "color:#123456"
                    } else {
                        "fill:#123456"
                    })
            );
            for class in [
                "label name",
                "label attribute-type",
                "label attribute-name",
                "label attribute-keys",
                "label attribute-comment",
            ] {
                let label = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some(class))
                    .expect("attribute label");
                assert!(
                    label
                        .descendants()
                        .any(|node| node.has_tag_name(if html_labels {
                            "foreignObject"
                        } else {
                            "text"
                        })),
                    "{class}: {svg}"
                );
            }
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_line_hops_change_only_crossing_paths() {
    use std::collections::BTreeMap;
    let body = r#"erDiagram
A ||--|| X : links
A ||--|| Y : links
A ||--|| Z : links
B ||--|| X : links
B ||--|| Y : links
B ||--|| Z : links
C ||--|| X : links
C ||--|| Y : links
C ||--|| Z : links
"#;
    let render = |mode: &str| {
        render_er_svg_from_text(
            &format!("---\nconfig:\n  layout: elk\n  elk:\n    lineHops: {mode}\n---\n{body}"),
            &SvgRenderOptions::default(),
        )
    };
    let without = render("false");
    let arcs = render("true");
    let gaps = render("gap");
    let paths = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid SVG");
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| {
                (
                    node.attribute("id").unwrap().to_string(),
                    (
                        node.attribute("d").unwrap().to_string(),
                        node.attribute("marker-start").unwrap().to_string(),
                        node.attribute("marker-end").unwrap().to_string(),
                        node.attribute("data-points").unwrap().to_string(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    let plain_paths = paths(&without);
    let arc_paths = paths(&arcs);
    let gap_paths = paths(&gaps);
    assert_eq!(plain_paths.len(), 9);
    let mut changed = 0;
    let mut unchanged = 0;
    for (id, plain) in &plain_paths {
        let arc = &arc_paths[id];
        let gap = &gap_paths[id];
        assert_eq!((&plain.1, &plain.2, &plain.3), (&arc.1, &arc.2, &arc.3));
        assert_eq!((&plain.1, &plain.2, &plain.3), (&gap.1, &gap.2, &gap.3));
        if plain.0 != arc.0 {
            changed += 1;
            assert_ne!(plain.0, gap.0);
            assert_ne!(arc.0, gap.0);
            assert!(arc.0.contains('A'), "expected jump arc for {id}: {}", arc.0);
            assert!(
                gap.0.matches('M').count() > 1,
                "expected a discontinuity for {id}: {}",
                gap.0
            );
        } else {
            unchanged += 1;
            assert_eq!(plain.0, gap.0);
        }
    }
    assert!(changed > 0 && unchanged > 0);
    assert_eq!(edge_labels_group(&without), edge_labels_group(&arcs));
    assert_eq!(edge_labels_group(&without), edge_labels_group(&gaps));
}

#[cfg(feature = "layout-elk")]
#[test]
fn er_elk_neo_hops_update_solid_and_dashed_masks_from_final_path_length() {
    use kurbo::Shape;
    use std::collections::BTreeMap;

    for dashed in [false, true] {
        let mut body = String::from("erDiagram\n");
        let relationship = if dashed { "||..||" } else { "||--||" };
        for source in ["A", "B", "C"] {
            for target in ["X", "Y", "Z"] {
                body.push_str(&format!("{source} {relationship} {target} : links\n"));
            }
        }
        let render = |mode: &str| {
            render_er_svg_from_text(
                &format!(
                    "---\nconfig:\n  layout: elk\n  look: neo\n  elk:\n    lineHops: {mode}\n---\n{body}"
                ),
                &SvgRenderOptions::default(),
            )
        };
        let without = render("false");
        let original = roxmltree::Document::parse(&without).expect("original SVG");
        let original_edges: BTreeMap<_, _> = original
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| (node.attribute("id").unwrap(), node))
            .collect();

        for mode in ["true", "gap"] {
            let svg = render(mode);
            let document = roxmltree::Document::parse(&svg).expect("hopped SVG");
            let mut changed = 0;
            let mut unchanged = 0;
            for edge in document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
            {
                let id = edge.attribute("id").unwrap();
                let before = original_edges[id];
                let d = edge.attribute("d").unwrap();
                let style = edge.attribute("style").unwrap();
                for attribute in ["marker-start", "marker-end", "data-points"] {
                    assert_eq!(
                        before.attribute(attribute),
                        edge.attribute(attribute),
                        "{id}/{mode}/{attribute}"
                    );
                }
                if d == before.attribute("d").unwrap() {
                    unchanged += 1;
                    assert_eq!(style, before.attribute("style").unwrap(), "{id}/{mode}");
                    continue;
                }
                changed += 1;
                // getTotalLength() returns an SVG DOM float before marker-offset arithmetic.
                let length =
                    f64::from(kurbo::BezPath::from_svg(d).unwrap().perimeter(1.0e-6) as f32);
                let dasharray: Vec<f64> = style
                    .split(';')
                    .find_map(|declaration| declaration.trim().strip_prefix("stroke-dasharray:"))
                    .expect("Neo mask")
                    .split_whitespace()
                    .map(|value| value.parse().expect("dash length"))
                    .collect();
                // lineJump.ts reads offsets from the first four original dash values.
                // For a repeated Neo 2/2 pattern the fourth value is 2, not its final 0.
                let end_offset = if dashed { 2.0 } else { 0.0 };
                assert_eq!(dasharray.len(), 4, "{id}/{mode}: {style}");
                assert_eq!(
                    (dasharray[0], dasharray[1], dasharray[3]),
                    (0.0, 0.0, end_offset)
                );
                assert!(
                    (dasharray[2] - (length - end_offset).max(0.0)).abs() < 0.001,
                    "{id}/{mode}: length={length}, style={style}"
                );
                assert!(style.contains("stroke-dashoffset: 0;"));
                assert!(
                    !style.contains(";;"),
                    "upstream collapses empty declarations: {style}"
                );
            }
            assert!(changed > 0 && unchanged > 0, "dashed={dashed}/{mode}");
            assert_eq!(edge_labels_group(&without), edge_labels_group(&svg));
        }
    }
}

#[test]
fn er_svg_nested_relationships_and_empty_groups_keep_dom_order() {
    let source = concat!(
        "erDiagram\n",
        "subgraph Outer [Outer Domain]\n",
        "PRE\n",
        "subgraph Inner [Inner Domain]\n",
        "A ||--|| B : inner_relation\n",
        "end\n",
        "PRE ||--|| C : outer_relation\n",
        "subgraph Empty [Empty Domain]\nend\n",
        "end\n",
        "B ||--|| D : cross_relation\n",
    );
    let svg = render_er_svg_from_text(source, &SvgRenderOptions::default());
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    for id in ["merman-Outer", "merman-Inner", "merman-Empty"] {
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("id") == Some(id)),
            "missing {id}"
        );
    }
    let nodes = document
        .descendants()
        .find(|node| node.attribute("class") == Some("nodes"))
        .expect("entity nodes");
    let ids = nodes
        .children()
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "merman-entity-PRE-0",
            "merman-entity-A-1",
            "merman-entity-B-2",
            "merman-entity-C-3",
            "merman-entity-D-4",
        ]
    );
    let labels = edge_labels_group(&svg);
    let inner = labels.find("inner_relation").unwrap();
    let outer = labels.find("outer_relation").unwrap();
    let cross = labels.find("cross_relation").unwrap();
    assert!(inner < outer && outer < cross);
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .count(),
        3
    );
}
