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
fn radar_emits_grids_series_axes_legends_and_title_with_separate_fill_opacity() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"radar-beta
  title Release health
  axis Speed, Quality, Reach
  curve Current{80, 60, 90}
  curve Target{90, 90, 90}
  graticule circle
  ticks 3
  showLegend true
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Radar should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert_eq!(document.viewport.bounds.width, 700.0);
    assert_eq!(document.viewport.bounds.height, 700.0);

    let background_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "radar.background"
            )
        })
        .expect("Radar background should be drawn");
    let first_grid_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "radar.graticule.0.shape"
            )
        })
        .expect("Radar grid should be drawn");
    assert!(background_command < first_grid_command);

    let curve_path = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str() == "radar.curve.0.shape" =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("Radar series should own typed path geometry");
    assert!(
        curve_path
            .segments
            .iter()
            .any(|segment| matches!(segment, merman_display_list::PathSegment::CubicTo { .. }))
    );
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "radar.graticule.0.shape"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::ArcTo { .. }
                ))
    )));
    assert!(
        curve_path
            .segments
            .iter()
            .any(|segment| matches!(segment, merman_display_list::PathSegment::Close))
    );
    assert_eq!(
        document
            .commands
            .iter()
            .filter(|command| matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "radar.curve.0.shape"
            ))
            .count(),
        2,
        "series fill and stroke must remain independently painted"
    );
    for opacity in [0.3, 0.5] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::SetOpacity { opacity: actual }
                if (*actual - opacity).abs() <= f64::EPSILON
        )));
    }

    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Speed"
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Alphabetic
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Current"
                && run.anchor == merman_display_list::TextAnchor::Start
                && run.baseline == merman_display_list::TextBaseline::Hanging
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Release health"
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Hanging
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.id == "radar.axis.0"
            && semantic.title.as_deref() == Some("Speed")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.id == "radar.curve.0"
            && semantic.title.as_deref() == Some("Current")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Label
            && semantic.title.as_deref() == Some("Release health")
    }));
}

#[test]
fn ishikawa_emits_classic_fishbone_geometry_markers_labels_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r##"---
config:
  fontSize: 18px
  themeVariables:
    lineColor: '#008800'
    mainBkg: '#ffffff'
    textColor: '#111111'
---
ishikawa-beta
    Blurry Photo
    Process
        Out of focus
        Shutter speed too slow
    User
        Shaky hands
"##,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("classic Ishikawa should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "ishikawa.head.shape"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::QuadTo { .. }
                ))
    )));
    for path_id in [
        "ishikawa.branch.0.upper.line.marker.start",
        "ishikawa.branch.0.upper.cause.0.line.marker.start",
        "ishikawa.branch.0.upper.label-box",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }

    let background_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "ishikawa.background"
            )
        })
        .expect("Ishikawa root background should be drawn");
    let spine_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "ishikawa.spine.path"
            )
        })
        .expect("Ishikawa spine should be drawn");
    assert!(background_command < spine_command);

    let branch_line_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "ishikawa.branch.0.upper.line.path"
            )
        })
        .expect("Ishikawa branch should be drawn");
    let branch_marker_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "ishikawa.branch.0.upper.line.marker.start"
            )
        })
        .expect("Ishikawa branch marker should be drawn");
    assert!(branch_line_command < branch_marker_command);

    let branch_stroke_width = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "ishikawa.branch.0.upper.line.path" =>
            {
                style.stroke.as_ref().map(|stroke| stroke.width)
            }
            _ => None,
        })
        .expect("Ishikawa branch should retain its stroke");
    let sub_branch_stroke_width = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "ishikawa.branch.0.upper.cause.0.line.path" =>
            {
                style.stroke.as_ref().map(|stroke| stroke.width)
            }
            _ => None,
        })
        .expect("Ishikawa sub-branch should retain its stroke");
    assert_eq!(branch_stroke_width, 2.0);
    assert_eq!(sub_branch_stroke_width, 1.0);

    for text in ["Blurry", "Photo"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run }
                if run.text == text
                    && run.style.font.weight == 600
                    && run.style.font_size == 14.0
                    && run.anchor == merman_display_list::TextAnchor::Middle
                    && run.baseline == merman_display_list::TextBaseline::Middle
        )));
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Process"
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Middle
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Out of focus"
                && run.anchor == merman_display_list::TextAnchor::End
                && run.baseline == merman_display_list::TextBaseline::Middle
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Process")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Out of focus")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id == "ishikawa.branch.0.upper.line"
    }));
}

#[test]
fn ishikawa_rejects_roughjs_output_instead_of_substituting_classic_geometry() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"---
config:
  look: handDrawn
---
ishikawa-beta
    Effect
    Cause
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("hand-drawn Ishikawa must not become classic geometry silently");
    assert!(error.to_string().contains("RoughJS paths"));
}

#[test]
fn railroad_emits_ordered_grammar_shapes_arcs_markers_text_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r##"---
config:
  railroad:
    terminalFill: '#123456'
    terminalStroke: '#abcdef'
    specialFill: '#fedcba'
    specialStroke: '#654321'
    lineColor: '#112233'
    markerFill: '#445566'
    strokeWidth: 3
    fontSize: 18
---
railroad-beta
expr = sequence(nonterminal("term"), terminal("+"), zeroOrMore(special("guard"))) ;
"##,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Railroad should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "railroad.rule.0.start.shape",
        "railroad.rule.0.end.shape",
        "railroad.rule.0.element.0.shape",
        "railroad.rule.0.element.1.shape",
        "railroad.rule.0.element.2.shape",
        "railroad.rule.0.connector.start.path",
        "railroad.rule.0.connector.end.path",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }

    let terminal_path = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str() == "railroad.rule.0.element.1.shape" =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("terminal should own a rounded rectangle path");
    assert!(
        terminal_path
            .segments
            .iter()
            .any(|segment| matches!(segment, merman_display_list::PathSegment::ArcTo { .. }))
    );
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().starts_with("railroad.rule.0.path.")
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::ArcTo { .. }
                ))
    )));

    let terminal_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "railroad.rule.0.element.1.shape" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("terminal should be painted");
    assert_eq!(
        terminal_style.fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0x12, 0x34, 0x56, 0xff)
        ))
    );
    assert_eq!(
        terminal_style
            .stroke
            .as_ref()
            .expect("terminal should retain its stroke")
            .width,
        3.0
    );
    let special_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "railroad.rule.0.element.2.shape" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("special element should be painted");
    assert_eq!(
        special_style
            .stroke
            .as_ref()
            .expect("special element should retain its stroke")
            .dash_array,
        vec![5.0, 3.0]
    );

    for text in ["term", "+", "? guard ?"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run }
                if run.text == text
                    && run.anchor == merman_display_list::TextAnchor::Middle
                    && run.baseline == merman_display_list::TextBaseline::Middle
        )));
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "expr ="
                && run.style.font.weight == 700
                && run.anchor == merman_display_list::TextAnchor::Start
                && run.baseline == merman_display_list::TextBaseline::Alphabetic
    )));

    let background_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "railroad.background"
            )
        })
        .expect("Railroad background should be drawn");
    let first_element_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "railroad.rule.0.element.0.shape"
            )
        })
        .expect("Railroad grammar element should be drawn");
    assert!(background_command < first_element_command);
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.title.as_deref() == Some("expr")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("? guard ?")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id.starts_with("railroad.rule.0.path.")
    }));
}

#[test]
fn venn_emits_classic_paths_independent_opacity_labels_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r##"venn-beta
title Product Surface
set A["Core"]:20
set B["Editor"]:14
union A,B["Shared"]:4
style A fill:#ff6b6b, color:#101010, stroke:#202020, stroke-width:7, fill-opacity:0.42
style A,B fill:#00ffcc, color:#003333
"##,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("classic Venn should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert_eq!(document.viewport.bounds.width, 800.0);
    assert_eq!(document.viewport.bounds.height, 450.0);
    for path_id in [
        "venn.background",
        "venn.area.0.shape",
        "venn.area.1.shape",
        "venn.area.2.shape",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }
    assert_eq!(
        document
            .commands
            .iter()
            .filter(|command| matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "venn.area.0.shape"
            ))
            .count(),
        2,
        "Venn fill and stroke opacity must remain independently painted"
    );
    for opacity in [0.42, 0.95] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::SetOpacity { opacity: actual }
                if (*actual - opacity).abs() <= f64::EPSILON
        )));
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ConcatTransform { transform }
            if transform.e == 0.0 && transform.f == 24.0
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Product Surface"
                && run.style.font_size == 32.0
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Middle
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Core"
                && run.style.font_size == 24.0
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Alphabetic
    )));
    for title in ["Core", "Editor", "Shared"] {
        assert!(document.semantics.iter().any(|semantic| {
            semantic.role == merman_display_list::SemanticRole::Node
                && semantic.title.as_deref() == Some(title)
        }));
    }
}

#[test]
fn venn_rejects_foreign_object_text_nodes_instead_of_dropping_them() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"venn-beta
set A["Frontend"]:20
  text A1["React"]
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("Venn browser-wrapped text nodes must not disappear silently");
    assert!(
        error
            .to_string()
            .contains("foreignObject and browser wrapping")
    );
}

#[test]
fn venn_rejects_roughjs_output_instead_of_substituting_classic_geometry() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"---
config:
  look: handDrawn
---
venn-beta
set A
set B
union A,B
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("hand-drawn Venn must not become classic geometry silently");
    assert!(error.to_string().contains("RoughJS paths"));
}

#[test]
fn tree_view_emits_ordered_lines_builtin_icons_highlights_and_descriptions() {
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "treeView": {
                "labelFontSize": "20px",
                "labelColor": "#112233",
                "lineColor": "#334455",
                "iconColor": "#556677",
                "descriptionColor": "#778899",
                "highlightBg": "rgba(255, 193, 7, 0.15)",
                "highlightStroke": "#ffc107"
            }
        }
    }));
    let output = Renderer::new()
        .with_engine(Engine::new().with_site_config(site_config))
        .render(RenderRequest::drawing_list(
            r#"treeView-beta
src/ :::highlight icon(folder) ## source directory
    main.rs icon(file)
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable TreeView should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert_eq!(document.viewport.bounds.x, -0.5);
    assert!(document.viewport.bounds.width > 0.0);
    for path_id in [
        "treeView.background",
        "treeView.node.1.highlight",
        "treeView.node.1.icon",
        "treeView.node.2.icon",
        "treeView.line.0.path",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }

    let highlight_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "treeView.node.1.highlight" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("highlight should be painted");
    assert_eq!(
        highlight_style.fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(255, 193, 7, 38)
        ))
    );
    assert_eq!(
        highlight_style
            .stroke
            .as_ref()
            .expect("highlight should retain its stroke")
            .paint,
        merman_display_list::Paint::solid(merman_display_list::Color::rgba(0xff, 0xc1, 0x07, 0xff))
    );

    let line_style = document
        .commands
        .iter()
        .find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "treeView.line.0.path" =>
            {
                Some(style)
            }
            _ => None,
        })
        .expect("connector should be painted");
    assert_eq!(
        line_style
            .stroke
            .as_ref()
            .expect("connector should retain its stroke")
            .paint,
        merman_display_list::Paint::solid(merman_display_list::Color::rgba(0x33, 0x44, 0x55, 0xff))
    );

    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ConcatTransform { transform }
            if (transform.a - 14.0 / 24.0).abs() <= f64::EPSILON
                && (transform.d - 14.0 / 24.0).abs() <= f64::EPSILON
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "src"
                && run.style.font_size == 20.0
                && run.style.font.weight == 700
                && run.style.fill == merman_display_list::Paint::solid(
                    merman_display_list::Color::rgba(0x11, 0x22, 0x33, 0xff)
                )
                && run.baseline == merman_display_list::TextBaseline::Middle
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "source directory"
                && run.style.font.style == merman_display_list::FontStyle::Italic
                && run.style.fill == merman_display_list::Paint::solid(
                    merman_display_list::Color::rgba(0x77, 0x88, 0x99, 0xff)
                )
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("src")
            && semantic.description.as_deref() == Some("source directory")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge && semantic.id == "treeView.line.0"
    }));
}

#[test]
fn tree_view_rejects_registry_svg_icons_instead_of_substituting_fallback_art() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "treeView-beta\nRoot icon(logos:react)\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("registry TreeView icons must remain an explicit capability boundary");
    assert!(error.to_string().contains("non-builtin icon `logos:react`"));
}

#[test]
fn treemap_emits_sections_clipped_leaves_formatted_values_and_class_styles() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r##"---
config:
  treemap:
    valueFormat: "$,.2f"
---
treemap-beta
title Allocation
"Engineering"
  "Frontend": 1234.5:::important
  "Backend": 500
classDef important fill:#e74c3c,color:#fff,stroke:#c0392b,stroke-width:3px,stroke-dasharray:5 5;
"##,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Treemap should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    for path_id in [
        "treemap.background",
        "treemap.section.1.shape",
        "treemap.leaf.0.shape",
        "treemap.leaf.0.label.clip",
        "treemap.leaf.0.value.clip",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ClipPath { path, .. }
            if path.as_str() == "treemap.leaf.0.label.clip"
    )));

    let leaf_paints = document
        .commands
        .iter()
        .filter_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == "treemap.leaf.0.shape" =>
            {
                Some(style)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(leaf_paints.len(), 2);
    assert_eq!(
        leaf_paints[0].fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0xe7, 0x4c, 0x3c, 77)
        ))
    );
    let stroke = leaf_paints[1]
        .stroke
        .as_ref()
        .expect("styled Treemap leaf should retain its stroke");
    assert_eq!(stroke.width, 3.0);
    assert_eq!(stroke.dash_array, vec![5.0, 5.0]);
    assert_eq!(
        stroke.paint,
        merman_display_list::Paint::solid(merman_display_list::Color::rgba(0xc0, 0x39, 0x2b, 0xff))
    );

    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Frontend"
                && run.style.fill == merman_display_list::Paint::solid(
                    merman_display_list::Color::rgba(0xff, 0xff, 0xff, 0xff)
                )
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Middle
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "$1.2e+3"
                && run.baseline == merman_display_list::TextBaseline::Hanging
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.title.as_deref() == Some("Engineering")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Frontend")
    }));
}

#[test]
fn treemap_rejects_unmapped_class_effects_instead_of_dropping_them() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"treemap-beta
"Engineering"
  "Frontend": 1:::glow
classDef glow fill:#123456,filter:blur(2px);
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("Treemap filters must remain an explicit capability boundary");
    assert!(error.to_string().contains("classDef property `filter`"));
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
fn xychart_emits_bars_lines_labels_legends_and_rotated_axes_in_paint_order() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"---
config:
  xyChart:
    showDataLabel: true
---
xychart
  title "Releases"
  x-axis "Quarter" [Q1, Q2]
  y-axis "Units" 0 --> 100
  bar "Actual" [20, 40]
  line "Target" [30, 50]
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("XYChart should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    let background_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str() == "xychart.background"
            )
        })
        .expect("XYChart background should be drawn");
    let first_bar_command = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::DrawPath { path, .. }
                    if path.as_str().starts_with("xychart.rect.")
            )
        })
        .expect("XYChart bars should be drawn");
    assert!(background_command < first_bar_command);

    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().starts_with("xychart.path.")
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::LineTo { .. }
                ))
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "20"
                && run.anchor == merman_display_list::TextAnchor::Middle
                && run.baseline == merman_display_list::TextBaseline::Hanging
    )));
    for text in ["Actual", "Target"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == text
        )));
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ConcatTransform { transform }
            if transform.b.abs() > 0.99 && transform.c.abs() > 0.99
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Q1: 20")
            && semantic.description.as_deref() == Some("Actual")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.title.as_deref() == Some("Target")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Label
            && semantic.title.as_deref() == Some("Releases")
    }));
}

#[test]
fn eventmodeling_emits_swimlanes_boxes_relations_and_explicit_text_runs() {
    let source = r#"eventmodeling
tf 01 ui Shop.Cart
tf 02 cmd Ordering.AddItem ->> 01 { sku: "SKU-1" }
tf 03 evt Cart.ItemAdded ->> 02 [[ItemAddedData]]
rf 04 rmo Read.CartSummary
tf 05 evt Checkout.CheckedOut

data ItemAddedData {
  sku: "SKU-1"
  quantity: 1
}
"#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("EventModeling should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "eventmodeling.swimlane.1.shape"
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "eventmodeling.box.2.shape"
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "eventmodeling.relation.0.arrowhead"
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "AddItem" && run.style.font.weight == 700
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text.contains("quantity")
                && run.style.font.families.iter().any(|family| family == "monospace")
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.id == "eventmodeling.swimlane.101"
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("ItemAdded")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.description.as_deref() == Some("01 → 02")
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
