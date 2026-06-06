use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};
use merman_core::{Engine, MAX_DIAGRAM_NESTING_DEPTH, ParseOptions};
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
    };
    for depth in (0..=MAX_DIAGRAM_NESTING_DEPTH).rev() {
        child = TreeViewNodeRenderModel {
            id: depth as i64,
            level: depth as i64,
            name: format!("n{depth}"),
            children: vec![child],
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
