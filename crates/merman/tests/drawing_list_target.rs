#![cfg(feature = "svg")]

use merman::{
    DrawingListRequest, Engine, MermaidConfig, OperationControl, RenderOutput, RenderRequest,
    Renderer,
};
use merman_display_list::DrawingListPolicy;

#[test]
fn error_diagram_has_a_complete_renderer_neutral_output() {
    let request = DrawingListRequest {
        policy: DrawingListPolicy::VectorOnly,
        ..DrawingListRequest::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::drawing_list("flowchart TD\nA -->", OperationControl::new(), request)
                .with_parse_options(merman::ParseOptions::lenient()),
        )
        .expect("lenient error diagram should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    assert_eq!(
        output.media_type(),
        merman_display_list::DRAWING_LIST_MEDIA_TYPE
    );
    assert_eq!(
        output.document().version,
        merman_display_list::DRAWING_LIST_VERSION
    );
    assert!(!output.document().commands.is_empty());
    assert_eq!(
        output.json(),
        output.document().canonical_json_bytes().unwrap()
    );
}

#[test]
fn flowchart_emits_typed_routes_shapes_and_semantics() {
    let source = r#"flowchart TD
        classDef hot fill:#ffe4e6,stroke:#be123c,stroke-width:2px,color:#881337
        A[Parse]:::hot -->|next| B{Layout}
        B --> C([Done])
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Flowchart should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    assert_eq!(output.document().viewport.bounds.width > 0.0, true);
    assert!(
        output
            .document()
            .resources
            .iter()
            .any(|resource| matches!(resource, merman_display_list::DrawingResource::Path(_)))
    );
    assert!(output.document().commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { .. }
    )));
    assert!(
        output
            .document()
            .semantics
            .iter()
            .any(|semantic| semantic.role == merman_display_list::SemanticRole::Node)
    );
    assert!(
        output
            .document()
            .semantics
            .iter()
            .any(|semantic| semantic.role == merman_display_list::SemanticRole::Edge)
    );
}

#[test]
fn mindmap_emits_typed_shapes_routes_text_and_semantics() {
    let source = r#"mindmap
        root((Merman))
            Parser
            Renderer[Render]
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Mindmap should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("mindmap.node")
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { .. }
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.id.starts_with("mindmap.node.")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id.starts_with("mindmap.edge.")
    }));
}

#[test]
fn mindmap_rejects_styled_markdown_instead_of_flattening_it_silently() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "mindmap\n  root[**bold root**]\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("styled Mindmap labels must not be flattened silently");
    assert!(error.to_string().contains("cannot preserve"));
}

#[test]
fn info_emits_typed_version_text_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "info",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Info should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    assert_eq!(output.document().viewport.bounds.width, 400.0);
    assert_eq!(output.document().viewport.bounds.height, 100.0);
    assert!(output.document().commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text.starts_with('v') && run.origin.x == 100.0 && run.origin.y == 40.0
    )));
    assert!(output.document().semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Label && semantic.id == "info.version"
    }));
}

#[test]
fn state_emits_typed_nodes_transitions_markers_and_labels() {
    let source = r#"stateDiagram-v2
        [*] --> Idle: start
        Idle --> Done: finish
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable State subset should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("state.edge.0.marker.end")
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Idle")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("finish")
    }));
}

#[test]
fn state_rejects_roughjs_shapes_instead_of_substituting_regular_geometry() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "stateDiagram-v2\n  A --> [*]\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("State end markers must remain explicit until RoughJS is canonical");
    assert!(error.to_string().contains("RoughJS path generation"));
}

#[test]
fn pie_emits_typed_donut_slices_static_highlight_and_legend() {
    let source = r#"%%{init: {"pie": {"donutHole": 0.4, "highlightSlice": "Alpha"}}}%%
pie showData title Releases
    "Stable" : 3
    "Alpha" : 1
"#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Pie should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "pie.slice.1.shape"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::ArcTo { sweep_clockwise: false, .. }
                ))
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ConcatTransform { transform }
            if (transform.a - 1.05).abs() <= f64::EPSILON
                && (transform.d - 1.05).abs() <= f64::EPSILON
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Alpha [1]"
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Alpha")
    }));
}

#[test]
fn pie_rejects_hover_highlighting_without_an_interaction_contract() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"%%{init: {"pie": {"highlightSlice": "hover"}}}%%
pie
    "A" : 1
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("hover highlighting must not disappear from DrawingList output");
    assert!(error.to_string().contains("interaction state machine"));
}

#[test]
fn packet_emits_typed_blocks_bit_numbers_title_and_styles() {
    let source = r#"packet
    title Header
    0-7: "Version"
    8-15: "Length"
"#;
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "packet": {
            "bitsPerRow": 16,
            "blockFillColor": "#123456",
            "blockStrokeWidth": 2
        }
    }));
    let output = Renderer::new()
        .with_engine(Engine::new().with_site_config(site_config))
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Packet should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "packet.block.0.0.shape"
    )));
    let block_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "packet.block.0.0.shape" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("first Packet block should be drawn");
    assert_eq!(
        block_style.fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0x12, 0x34, 0x56, 0xff)
        ))
    );
    assert_eq!(
        block_style
            .stroke
            .as_ref()
            .expect("Packet block should retain its stroke")
            .width,
        2.0
    );
    for text in ["Version", "0", "7", "Header"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == text
        )));
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Version")
    }));
}

#[test]
fn sankey_emits_typed_nodes_gradient_links_blending_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "sankey\nSource,Target,10\nTarget,Done,4\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Sankey should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::LinearGradient(gradient)
            if gradient.id.as_str() == "sankey.link.0.paint"
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "sankey.link.0.path"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::CubicTo { .. }
                ))
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::SetOpacity { opacity }
            if (*opacity - 0.5).abs() <= f64::EPSILON
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::SetBlendMode {
            blend_mode: merman_display_list::BlendMode::Multiply
        }
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Source 10"
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Source")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("Source → Target")
    }));
}

#[test]
fn sankey_rejects_outlined_labels_without_text_stroke_semantics() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"---
config:
  sankey:
    labelStyle: outlined
---
sankey
A,B,10
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("outlined Sankey labels must not become plain text silently");
    assert!(error.to_string().contains("text stroke and paint-order"));
}

#[test]
fn quadrantchart_emits_regions_points_rotated_axes_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"quadrantChart
  title Portfolio
  x-axis Low --> High
  y-axis Bottom --> Top
  quadrant-1 Invest
  quadrant-2 Explore
  quadrant-3 Retire
  quadrant-4 Maintain
  Styled: [0.7, 0.8] color: #ff3300, radius: 10, stroke-color: #0ea5e9, stroke-width: 3px
  Default: [0.3, 0.2]
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("QuadrantChart should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "quadrantchart.quadrant.0.shape"
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "quadrantchart.point.1.shape"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::ArcTo { .. }
                ))
    )));
    let point_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "quadrantchart.point.1.shape" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("styled point should be drawn");
    assert_eq!(
        point_style.fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0xff, 0x33, 0x00, 0xff)
        ))
    );
    assert_eq!(
        point_style
            .stroke
            .as_ref()
            .expect("styled point should retain its stroke")
            .width,
        3.0
    );
    let default_point_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "quadrantchart.point.0.shape" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("default point should use the browser-visible fallback");
    assert_eq!(
        default_point_style.fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0, 0, 0, 0xff)
        ))
    );
    assert_eq!(default_point_style.stroke, None);
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ConcatTransform { transform }
            if transform.b.abs() > 0.99 && transform.c.abs() > 0.99
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.title.as_deref() == Some("Invest")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Styled")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Label
            && semantic.title.as_deref() == Some("Portfolio")
    }));
}

#[test]
fn an_unmigrated_family_fails_without_partial_drawing_list_bytes() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "timeline\n  section Release\n    Plan : Build\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("unmigrated families must fail closed");
    assert!(
        output
            .to_string()
            .contains("has not been migrated to the canonical document seam")
    );
}
