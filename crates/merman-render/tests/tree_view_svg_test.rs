mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};
use merman_core::{Engine, MAX_DIAGRAM_NESTING_DEPTH, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{
    IconRegistry, IconSvg, SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config,
};
use merman_render::tree_view::layout_tree_view_diagram_typed;
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};
use std::sync::Arc;

fn render_tree_view_svg_with_options(input: &str, options: SvgRenderOptions) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .expect("TreeView diagram");
    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &options,
    )
    .unwrap()
}

#[test]
fn tree_view_typed_render_model_outputs_svg() {
    let input = r##"---
config:
  treeView:
    rowIndent: 80
    lineThickness: 3
  themeVariables:
    treeView:
      labelFontSize: '20px'
      labelColor: '#FF0000'
      lineColor: '#00FF00'
---
treeView-beta
    "packages"
        "mermaid"
            "src"
        "parser"
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.meta.diagram_type, "treeView");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(svg.contains(r#"aria-roledescription="treeView""#));
    assert!(svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"style="max-width: "#));
    assert!(svg.contains(r#"viewBox="-1.5 0 "#));
    assert!(svg.contains(r#"<g/><g class="tree-view">"#));
    assert!(svg.contains(r#"<g class="tree-view">"#));
    assert!(svg.contains(r#"<g><text dominant-baseline="middle""#));
    assert!(svg.contains(r#"class="treeView-node-label""#));
    assert!(svg.contains(r#"class="treeView-node-line""#));
    assert!(svg.contains(r#"font-size: 20px"#));
    assert!(svg.contains(r#"fill: #FF0000"#));
    assert!(svg.contains(r#"stroke: #00FF00"#));
}

#[test]
fn tree_view_typed_render_model_outputs_accessibility_nodes() {
    let input = r##"treeView-beta
title TreeView Diagram Title
accTitle: Accessible TreeView Title
accDescr: Accessible TreeView Description
"Root"
    "Child"
"##;

    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.meta.diagram_type, "treeView");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-a11y-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(svg.contains(
        r#"aria-describedby="chart-desc-tree-view-a11y-test" aria-labelledby="chart-title-tree-view-a11y-test""#
    ));
    assert!(svg.contains(
        r#"<title id="chart-title-tree-view-a11y-test">Accessible TreeView Title</title><desc id="chart-desc-tree-view-a11y-test">Accessible TreeView Description</desc><style>"#
    ));
}

#[test]
fn tree_view_mermaid_11_16_annotations_render_svg_dom() {
    let input = r##"---
config:
  treeView:
    showIcons: true
    defaultIconPack: logos
    extensionIcons:
      ".tsx": react
---
treeView-beta
src/ :::highlight icon(folder) ## source directory
    App.tsx ## main component
    package.json icon(none)
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-11-16-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(svg.contains(r#"class="treeView-node-label treeView-node-dir highlight""#));
    assert!(svg.contains(".treeView-node-dir { font-weight: bold; }"));
    assert!(svg.contains(r#"class="treeView-highlight-bg""#));
    assert!(svg.contains(r#"class="treeView-node-description""#));
    assert!(svg.contains("source directory"));
    assert!(svg.contains("main component"));
    assert!(
        svg.contains(r##"xlink:href="#tv-icon-tree-view-11-16-test-mermaid-treeview-folder""##)
    );
    assert!(svg.contains(r##"xlink:href="#tv-icon-tree-view-11-16-test-logos-react""##));
    assert!(!svg.contains("package.json icon"));
    assert!(svg.contains(".treeView-node-icon"));
    assert!(svg.contains(".treeView-node-description"));
    assert!(svg.contains(".treeView-highlight-bg"));
}

#[test]
fn tree_view_builtin_icons_render_at_fourteen_pixels_without_overlapping_labels() {
    let input = r##"treeView-beta
src/ icon(folder)
file.txt icon(file)
App.tsx icon(logos:react)
"##;

    let mut registry = IconRegistry::new();
    for icon in ["file", "folder"] {
        registry.insert(
            format!("mermaid-treeview:{icon}"),
            IconSvg::new(r#"<path data-icon="registry-override"/>"#, 16.0, 16.0),
        );
    }
    let svg = render_tree_view_svg_with_options(
        input,
        SvgRenderOptions {
            diagram_id: Some("tree-view-icon-size-test".to_string()),
            icon_registry: Some(Arc::new(registry)),
            ..Default::default()
        },
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");

    for (icon, label) in [("folder", "src"), ("file", "file.txt")] {
        let symbol_id = format!("tv-icon-tree-view-icon-size-test-mermaid-treeview-{icon}");
        let symbol = document
            .descendants()
            .find(|node| node.attribute("id") == Some(symbol_id.as_str()))
            .expect("built-in icon definition");
        let icon_svg = symbol
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "svg")
            .expect("built-in icon uses a size-constrained SVG viewport");

        assert_eq!(icon_svg.attribute("width"), Some("14"));
        assert_eq!(icon_svg.attribute("height"), Some("14"));
        assert_eq!(icon_svg.attribute("viewBox"), Some("0 0 24 24"));

        let href = format!("#{symbol_id}");
        let icon_use = document
            .descendants()
            .find(|node| {
                node.tag_name().name() == "use"
                    && node.attribute(("http://www.w3.org/1999/xlink", "href"))
                        == Some(href.as_str())
            })
            .expect("icon use node");
        let label_node = document
            .descendants()
            .find(|node| node.tag_name().name() == "text" && node.text() == Some(label))
            .expect("icon label node");
        let icon_right = icon_use
            .attribute("x")
            .expect("icon x")
            .parse::<f64>()
            .expect("numeric icon x")
            + 14.0;
        let label_x = label_node
            .attribute("x")
            .expect("label x")
            .parse::<f64>()
            .expect("numeric label x");

        assert_eq!(label_x - icon_right, 4.0);
    }
    assert!(!svg.contains("registry-override"), "{svg}");

    let third_party_symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some("tv-icon-tree-view-icon-size-test-logos-react"))
        .expect("third-party fallback icon definition");
    let fallback_svg = third_party_symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("missing icon uses the standard fallback SVG");
    assert_eq!(fallback_svg.attribute("width"), Some("14"));
    assert_eq!(fallback_svg.attribute("height"), Some("14"));
    assert_eq!(fallback_svg.attribute("viewBox"), Some("0 0 80 80"));
    assert!(
        fallback_svg
            .children()
            .any(|node| node.is_element() && node.tag_name().name() == "g")
    );
}

#[test]
fn tree_view_registry_icons_preserve_viewbox_and_empty_body_semantics() {
    let mut registry = IconRegistry::new();
    registry.insert(
        "test:rocket",
        IconSvg::new(
            r#"<path data-icon="tree-view-registry" d="M2 3H34V21H2z"/>"#,
            32.0,
            18.0,
        )
        .with_viewbox(2.0, 3.0, 32.0, 18.0),
    );
    registry.insert("test:empty", IconSvg::new("", 16.0, 16.0));
    let svg = render_tree_view_svg_with_options(
        "treeView-beta\nRoot\n    Rocket icon(test:rocket)\n    Rocket Again icon(test:rocket)\n    Missing icon(test:missing)\n    Empty icon(test:empty)\n",
        SvgRenderOptions {
            diagram_id: Some("tree-view-registry-test".to_string()),
            icon_registry: Some(Arc::new(registry)),
            ..Default::default()
        },
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");

    let rocket_symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some("tv-icon-tree-view-registry-test-test-rocket"))
        .expect("registry icon symbol");
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.attribute("id") == Some("tv-icon-tree-view-registry-test-test-rocket")
            })
            .count(),
        1
    );
    let rocket_svg = rocket_symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("registry icon SVG");
    assert_eq!(rocket_svg.attribute("width"), Some("14"));
    assert_eq!(rocket_svg.attribute("height"), Some("14"));
    assert_eq!(rocket_svg.attribute("viewBox"), Some("2 3 32 18"));
    assert!(
        rocket_svg
            .descendants()
            .any(|node| node.attribute("data-icon") == Some("tree-view-registry"))
    );

    assert_unknown_tree_view_icon(&document, "tv-icon-tree-view-registry-test-test-missing");

    let empty_symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some("tv-icon-tree-view-registry-test-test-empty"))
        .expect("empty registry icon symbol");
    let empty_svg = empty_symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("an explicitly empty registry icon still resolves");
    assert_eq!(empty_svg.attribute("viewBox"), Some("0 0 16 16"));
    assert_eq!(
        empty_svg
            .children()
            .filter(|node| node.is_element())
            .count(),
        0
    );
}

#[test]
fn tree_view_missing_icon_without_registry_uses_unknown_icon() {
    let svg = render_tree_view_svg_with_options(
        "treeView-beta\nRoot icon(test:missing)\n",
        SvgRenderOptions {
            diagram_id: Some("tree-view-no-registry-test".to_string()),
            ..Default::default()
        },
    );
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    assert_unknown_tree_view_icon(&document, "tv-icon-tree-view-no-registry-test-test-missing");
}

fn assert_unknown_tree_view_icon(document: &roxmltree::Document<'_>, symbol_id: &str) {
    let symbol = document
        .descendants()
        .find(|node| node.attribute("id") == Some(symbol_id))
        .expect("missing icon symbol");
    let icon_svg = symbol
        .children()
        .find(|node| node.is_element() && node.tag_name().name() == "svg")
        .expect("unknown icon SVG");
    assert_eq!(icon_svg.attribute("width"), Some("14"));
    assert_eq!(icon_svg.attribute("height"), Some("14"));
    assert_eq!(icon_svg.attribute("viewBox"), Some("0 0 80 80"));
    assert_eq!(
        icon_svg
            .descendants()
            .find(|node| node.tag_name().name() == "tspan")
            .and_then(|node| node.text()),
        Some("?")
    );
}

#[test]
fn tree_view_root_highlight_visual_bounds_fit_inside_viewbox() {
    let input = r##"treeView-beta
root/ :::highlight
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let LayoutDiagram::TreeViewDiagram(tree_view) = &layout else {
        panic!("expected TreeView layout");
    };
    let content_width = tree_view
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0, f64::max);

    assert_eq!(tree_view.total_width, content_width + 10.0);

    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-root-highlight-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_tree_view_highlights_fit_viewbox(&svg);
}

#[test]
fn tree_view_multiple_highlights_follow_upstream_width_growth() {
    let input = r##"treeView-beta
root/ :::highlight
    child/ :::highlight
        leaf.txt
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let LayoutDiagram::TreeViewDiagram(tree_view) = &layout else {
        panic!("expected TreeView layout");
    };
    let content_width = tree_view
        .nodes
        .iter()
        .map(|node| node.x + node.width)
        .fold(0.0, f64::max);
    let highlighted_nodes = tree_view
        .nodes
        .iter()
        .filter(|node| {
            node.css_class
                .as_deref()
                .is_some_and(|class| class.split_whitespace().any(|part| part == "highlight"))
        })
        .collect::<Vec<_>>();

    assert_eq!(highlighted_nodes.len(), 2);
    assert_eq!(tree_view.total_width, content_width + 20.0);

    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-multiple-highlights-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    let document = roxmltree::Document::parse(&svg).expect("valid TreeView SVG");
    let highlight_rects = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-highlight-bg"))
        .collect::<Vec<_>>();
    let mut width_before_highlight = content_width;
    for (node, rect) in highlighted_nodes.into_iter().zip(highlight_rects) {
        let actual_width = rect
            .attribute("width")
            .expect("highlight width")
            .parse::<f64>()
            .expect("numeric highlight width");
        let expected_width = width_before_highlight - node.x + 8.0;
        assert!((actual_width - expected_width).abs() < 1e-9);
        width_before_highlight += 10.0;
    }
    assert_eq!(width_before_highlight, tree_view.total_width);
    assert_tree_view_highlights_fit_viewbox(&svg);
}

fn assert_tree_view_highlights_fit_viewbox(svg: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid TreeView SVG");
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .expect("TreeView viewBox")
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    let view_box_right = view_box[0] + view_box[2];

    for rect in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("treeView-highlight-bg"))
    {
        let x = rect
            .attribute("x")
            .expect("highlight x")
            .parse::<f64>()
            .expect("numeric highlight x");
        let width = rect
            .attribute("width")
            .expect("highlight width")
            .parse::<f64>()
            .expect("numeric highlight width");
        let visual_right = x + width + 0.5;
        assert!(
            visual_right <= view_box_right,
            "highlight right edge {visual_right} exceeds viewBox right edge {view_box_right}"
        );
    }
}

#[test]
fn tree_view_trailing_slash_only_marks_directory_labels() {
    let input = r##"treeView-beta
src/ :::directory-probe
    main.rs :::file-probe
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-directory-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(
        svg.contains(r#"class="treeView-node-label treeView-node-dir directory-probe""#),
        "trailing-slash directory should receive the upstream directory class: {svg}"
    );
    assert!(svg.contains(r#"class="treeView-node-label file-probe""#));
    assert!(!svg.contains(r#"treeView-node-dir file-probe"#));
}

#[test]
fn tree_view_fixed_size_root_keeps_width_and_height() {
    let input = r##"---
config:
  treeView:
    useMaxWidth: false
---
treeView-beta
"Root"
    "Child"
"##;

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.meta.diagram_type, "treeView");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let svg = render_layout_svg_parts_for_render_model_with_config(
        &layout,
        &parsed.model,
        &parsed.meta.effective_config,
        parsed.meta.title.as_deref(),
        LayoutOptions::default().text_measurer.as_ref(),
        &SvgRenderOptions {
            diagram_id: Some("tree-view-fixed-test".to_string()),
            ..Default::default()
        },
    )
    .unwrap();

    assert!(!svg.contains(r#"width="100%""#));
    assert!(svg.contains(r#"<svg id="tree-view-fixed-test" width=""#));
    assert!(svg.contains(r#"" height=""#));
    assert!(svg.contains(r#"style="background-color: white;" viewBox="-0.5 0 "#));
    assert!(!svg.contains("max-width:"));
}

#[test]
fn tree_view_layout_rejects_typed_model_beyond_nesting_limit() {
    let mut child = TreeViewNodeRenderModel {
        id: (MAX_DIAGRAM_NESTING_DEPTH + 1) as i64,
        level: (MAX_DIAGRAM_NESTING_DEPTH + 1) as i64,
        name: "leaf".to_string(),
        children: Vec::new(),
        ..Default::default()
    };
    for depth in (0..=MAX_DIAGRAM_NESTING_DEPTH).rev() {
        child = TreeViewNodeRenderModel {
            id: depth as i64,
            level: depth as i64,
            name: format!("n{depth}"),
            children: vec![child],
            ..Default::default()
        };
    }

    let model = TreeViewDiagramRenderModel {
        root: TreeViewNodeRenderModel {
            children: vec![child],
            ..Default::default()
        },
        ..Default::default()
    };

    let err = layout_tree_view_diagram_typed(
        &model,
        &serde_json::json!({}),
        LayoutOptions::default().text_measurer.as_ref(),
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("treeView nesting depth exceeds"),
        "{err}"
    );
}

#[test]
fn tree_view_public_layout_accepts_max_allowed_chain() {
    let mut input = String::from("treeView-beta\n");
    for depth in 0..MAX_DIAGRAM_NESTING_DEPTH {
        input.push_str(&" ".repeat(depth));
        input.push('"');
        input.push_str(&format!("n{depth}"));
        input.push_str("\"\n");
    }

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.meta.diagram_type, "treeView");

    let layout = layout_parsed_render_layout_only(&parsed, &LayoutOptions::default()).unwrap();
    let LayoutDiagram::TreeViewDiagram(tree_view) = &layout else {
        panic!("expected treeView layout");
    };

    assert_eq!(tree_view.nodes.len(), MAX_DIAGRAM_NESTING_DEPTH + 1);
    assert_eq!(
        tree_view.nodes.first().map(|node| node.name.as_str()),
        Some("/")
    );
    let expected_last = format!("n{}", MAX_DIAGRAM_NESTING_DEPTH - 1);
    assert_eq!(
        tree_view.nodes.last().map(|node| node.name.as_str()),
        Some(expected_last.as_str())
    );
}
