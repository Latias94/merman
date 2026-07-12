mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};
use merman_core::{Engine, MAX_DIAGRAM_NESTING_DEPTH, ParseOptions};
use merman_render::model::LayoutDiagram;
use merman_render::svg::{SvgRenderOptions, render_layout_svg_parts_for_render_model_with_config};
use merman_render::tree_view::layout_tree_view_diagram_typed;
use merman_render::{LayoutOptions, layout_parsed_render_layout_only};

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
