#![cfg(feature = "svg")]

use merman::{
    DrawingListRequest, Engine, MermaidConfig, OperationControl, RenderError, RenderOutput,
    RenderRequest, Renderer,
};
use merman_display_list::{
    DrawingCommand, DrawingListLimits, DrawingListPolicy, LineCap, LineJoin, Paint, TextPaintOrder,
};

#[test]
fn drawing_list_limit_failure_is_the_sticky_operation_terminal() {
    for (id, limits) in [
        (
            "max_commands",
            DrawingListLimits {
                max_commands: 0,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_serialized_bytes",
            DrawingListLimits {
                max_serialized_bytes: 0,
                ..DrawingListLimits::default()
            },
        ),
    ] {
        let control = OperationControl::new();
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                "info",
                control.clone(),
                DrawingListRequest {
                    limits,
                    ..DrawingListRequest::default()
                },
            ))
            .unwrap_err();
        let RenderError::ResourceLimitExceeded(resource) = error else {
            panic!("expected quota rejection");
        };
        assert_eq!(resource.id, id);
        let terminal = control
            .terminal_checkpoint_at(merman_core::OperationPhase::Emit)
            .expect_err("a rejected document must terminate the shared operation");
        let merman_core::OperationLedgerError::ResourceLimitExceeded(ref recorded) = terminal
        else {
            panic!("expected resource terminal");
        };
        assert_eq!(recorded.id, resource.id);
        assert_eq!(recorded.requested, resource.actual);
        assert_eq!(recorded.limit, resource.maximum);
        control.cancel();
        assert_eq!(
            control.terminal_checkpoint_at(merman_core::OperationPhase::Emit),
            Err(terminal)
        );
    }
}

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
    let text_runs = output
        .document()
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawingCommand::DrawText { run } => Some(run),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(text_runs.len(), 2);
    for run in text_runs {
        let stroke = run
            .style
            .stroke
            .as_ref()
            .expect("Error text must retain Mermaid's text stroke");
        assert_eq!(stroke.paint, run.style.fill);
        assert_eq!(stroke.width, 1.0);
        assert!(stroke.dash_array.is_empty());
        assert_eq!(stroke.dash_offset, 0.0);
        assert_eq!(stroke.line_cap, LineCap::Butt);
        assert_eq!(stroke.line_join, LineJoin::Miter);
        assert_eq!(stroke.miter_limit, 4.0);
        assert_eq!(run.style.paint_order, TextPaintOrder::FillThenStroke);
        assert!(matches!(run.style.fill, Paint::Solid { .. }));
    }
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

    assert!(output.document().viewport.bounds.width > 0.0);
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
fn flowchart_frontmatter_title_is_visible_and_inside_the_viewport() {
    let source = r##"---
title: Example
config:
  flowchart:
    titleTopMargin: 12
---
flowchart TD
  accTitle: Accessible chart
  A --> B
"##;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Flowchart frontmatter titles should render as visible text");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };
    let document = output.document();
    let title = document
        .commands
        .iter()
        .find_map(|command| match command {
            DrawingCommand::DrawText { run } if run.text == "Example" => Some(run),
            _ => None,
        })
        .expect("frontmatter title must be a DrawText command, not only semantic metadata");
    assert_eq!(title.origin.y, -12.0);
    assert_eq!(title.anchor, merman_display_list::TextAnchor::Middle);
    assert_eq!(
        title.baseline,
        merman_display_list::TextBaseline::Alphabetic
    );
    assert_eq!(title.style.font_size, 18.0);
    let viewport = document.viewport.bounds;
    assert!(viewport.x <= title.bounds.x);
    assert!(viewport.y <= title.bounds.y);
    assert!(viewport.x + viewport.width >= title.bounds.x + title.bounds.width);
    assert!(viewport.y + viewport.height >= title.bounds.y + title.bounds.height);
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Document
            && semantic.title.as_deref() == Some("Accessible chart")
    }));
    assert!(!document.commands.iter().any(|command| matches!(
        command,
        DrawingCommand::DrawText { run } if run.text == "Accessible chart"
    )));
}

#[test]
fn flowchart_neo_effects_fail_closed_in_the_public_target() {
    for (source, effect) in [
        (
            r##"%%{init: {"look":"neo","theme":"neutral","themeVariables":{"useGradient":true,"dropShadow":"none"}}}%%
flowchart TD
  A --> B
"##,
            "gradient",
        ),
        (
            r##"%%{init: {"look":"neo","themeVariables":{"dropShadow":"drop-shadow(1px 2px 2px #000)"}}}%%
flowchart TD
  A --> B
"##,
            "drop-shadow",
        ),
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect_err("unrepresented neo effects must not be silently dropped");

        assert!(
            matches!(
                error,
                RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable {
                    ref family,
                    ref reason,
                }) if family == "flowchart" && reason.contains(effect)
            ),
            "expected a structured Flowchart capability error for {effect}, got {error}"
        );
    }
}

#[test]
fn flowchart_and_swimlane_limits_stop_at_the_first_over_budget_item() {
    for (source, first_text_bytes, first_path_segments) in [
        ("flowchart TD\nA --> B\n", 1, 2),
        ("swimlane-beta LR\nsubgraph Lane\nA --> B\nend\n", 4, 5),
    ] {
        for (id, actual, maximum, limits) in [
            (
                "max_commands",
                1,
                0,
                DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
            ),
            (
                "max_resources",
                2,
                1,
                DrawingListLimits {
                    max_resources: 1,
                    ..DrawingListLimits::default()
                },
            ),
            (
                "max_text_bytes",
                first_text_bytes,
                0,
                DrawingListLimits {
                    max_text_bytes: 0,
                    ..DrawingListLimits::default()
                },
            ),
            (
                "max_path_segments",
                first_path_segments,
                1,
                DrawingListLimits {
                    max_path_segments: 1,
                    ..DrawingListLimits::default()
                },
            ),
        ] {
            let error = Renderer::new()
                .render(RenderRequest::drawing_list(
                    source,
                    OperationControl::new(),
                    DrawingListRequest {
                        limits,
                        ..DrawingListRequest::default()
                    },
                ))
                .expect_err("the builder must reject the first over-budget item without output");

            assert!(
                matches!(
                    &error,
                    RenderError::ResourceLimitExceeded(limit)
                    if limit.id == id
                        && limit.phase == "drawing-list-validation"
                        && limit.actual == actual
                        && limit.maximum == maximum
                ),
                "{source}: {id}: {error}"
            );
        }
    }
}

#[test]
fn er_command_limit_stops_at_the_first_over_budget_item() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("the ER builder must reject its first command before returning output");

    assert!(matches!(
        &error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_commands"
                && limit.phase == "drawing-list-validation"
                && limit.actual == 1
                && limit.maximum == 0
    ));
}

#[test]
fn bounded_families_fit_their_exact_document_budgets() {
    for source in [
        "flowchart TD\nA[Parse] -->|next| B{Layout}\nB --> C([Done])\n",
        "swimlane-beta LR\nsubgraph Lane\nA -->|next| B\nend\n",
        "pie showData title Releases\n\"Stable\" : 3\n\"Alpha\" : 1\n",
        "journey\nsection Checkout\nSign Up: 5: Alice\nPay: 3: Bob\n",
        "timeline\ntitle Releases\nsection Planning\nPlan : Build\nShip : Done\n",
        "timeline TD\nsection Planning\nPlan : Build\nShip : Done\n",
        "cynefin-beta\n",
        "radar-beta\n  title Release health\n  axis Speed, Quality, Reach\n  curve Current{80, 60, 90}\n  curve Target{90, 90, 90}\n  graticule circle\n  ticks 3\n",
        "railroad-beta\nexpr = terminal(\"A\") ;\n",
        "sankey\nSource,Target,10\nTarget,Done,4\n",
        "xychart-beta\n  x-axis [Jan, Feb]\n  line [1, 2]\n",
        "block-beta\n  A\n",
        "ishikawa-beta\n  Effect\n  Cause\n",
        "wardley-beta\ncomponent A [0.8, 0.2]\n",
        "zenuml\n@Starter(Client)\nClient->Service: call\n",
        "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
        "eventmodeling\ntf 01 ui Shop.Cart\ntf 02 cmd Ordering.AddItem ->> 01 { sku: \"SKU-1\" }\n",
        "quadrantChart\n  title Portfolio\n  x-axis Low --> High\n  y-axis Bottom --> Top\n  quadrant-1 Leaders\n  Point: [0.7, 0.8]\n",
        "treeView-beta\nsrc/ :::highlight icon(folder) ## source directory\n    main.rs icon(file)\n",
        "kanban\n  Todo\n    task[Task]\n",
        concat!(
            "C4Context\ntitle Context\n",
            "Person(user, \"User\")\n",
            "System_Boundary(service, \"Service\") {\n",
            "System(api, \"API\", \"Service\")\n}\n",
            "Rel(user, api, \"uses\")\nRel(api, user, \"responds\")\n",
        ),
    ] {
        let renderer = Renderer::new();
        let RenderOutput::DrawingList(Some(output)) = renderer
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect("portable diagram renders")
        else {
            panic!("expected DrawingList");
        };
        let footprint = output.document().footprint().unwrap();
        let limits = DrawingListLimits {
            max_serialized_bytes: output.json().len(),
            max_commands: footprint.commands,
            max_resources: footprint.resources,
            max_path_segments: footprint.path_segments,
            max_stroke_dash_entries: footprint.stroke_dash_entries,
            max_image_bytes: footprint.image_bytes,
            max_image_pixels: footprint.image_pixels,
            max_fallback_pixels: footprint.fallback_pixels,
            max_font_bytes: footprint.font_bytes,
            max_nesting_depth: footprint.max_nesting_depth,
            max_fallbacks: footprint.fallbacks,
            max_text_bytes: footprint.text_bytes,
            max_glyphs: footprint.glyphs,
        };
        let RenderOutput::DrawingList(Some(bounded)) = renderer
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest {
                    limits,
                    ..DrawingListRequest::default()
                },
            ))
            .expect("exact budgets must admit the complete document")
        else {
            panic!("expected DrawingList");
        };
        assert_eq!(bounded.json(), output.json(), "{source}");
    }
}

#[test]
fn info_command_limit_reaches_the_bounded_document_builder() {
    let request = DrawingListRequest {
        limits: DrawingListLimits {
            max_commands: 0,
            ..DrawingListLimits::default()
        },
        ..DrawingListRequest::default()
    };
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "info",
            OperationControl::new(),
            request,
        ))
        .expect_err("the Info builder must reject its first command before returning output");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_commands"
                && limit.phase == "drawing-list-validation"
                && limit.actual == 1
                && limit.maximum == 0
    ));
}

#[test]
fn kanban_command_limit_reaches_the_bounded_document_builder() {
    let request = DrawingListRequest {
        limits: DrawingListLimits {
            max_commands: 0,
            ..DrawingListLimits::default()
        },
        ..DrawingListRequest::default()
    };
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "kanban\n  Todo\n    task[Task]\n",
            OperationControl::new(),
            request,
        ))
        .expect_err("the Kanban builder must reject its first command before returning output");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_commands"
                && limit.phase == "drawing-list-validation"
                && limit.actual == 1
                && limit.maximum == 0
    ));
}

#[test]
fn newly_bounded_families_reject_before_the_first_output_command() {
    for source in [
        "cynefin-beta\n",
        "radar-beta\n  title Release health\n  axis Speed, Quality, Reach\n  curve Current{80, 60, 90}\n  curve Target{90, 90, 90}\n  graticule circle\n  ticks 3\n",
        "railroad-beta\nexpr = terminal(\"A\") ;\n",
        "sankey\nSource,Target,10\n",
        "xychart-beta\n  x-axis [Jan, Feb]\n  line [1, 2]\n",
        "block-beta\n  A\n",
        "ishikawa-beta\n  Effect\n  Cause\n",
        "wardley-beta\ncomponent A [0.8, 0.2]\n",
        "zenuml\n@Starter(Client)\nClient->Service: call\n",
        "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
        "eventmodeling\ntf 01 ui Shop.Cart\ntf 02 cmd Ordering.AddItem ->> 01 { sku: \"SKU-1\" }\n",
        "quadrantChart\n  title Portfolio\n  x-axis Low --> High\n  y-axis Bottom --> Top\n  quadrant-1 Leaders\n  Point: [0.7, 0.8]\n",
        "treeView-beta\nsrc/ :::highlight icon(folder) ## source directory\n    main.rs icon(file)\n",
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest {
                    limits: DrawingListLimits {
                        max_commands: 0,
                        ..DrawingListLimits::default()
                    },
                    ..DrawingListRequest::default()
                },
            ))
            .expect_err("the bounded family must reject its first command");

        assert!(
            matches!(
                &error,
                RenderError::ResourceLimitExceeded(limit)
                    if limit.id == "max_commands"
                        && limit.phase == "drawing-list-validation"
                        && limit.actual == 1
                        && limit.maximum == 0
            ),
            "{source}: {error}"
        );
    }
}

#[test]
fn block_degenerate_marker_is_a_structured_unavailable_result() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "block-beta\n  A --> A\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("a zero-length Block marker must not produce a partial document");

    let RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable {
        family,
        reason,
    }) = error
    else {
        panic!("expected a structured DrawingList-unavailable error, got {error}");
    };
    assert_eq!(family, "block");
    assert!(reason.contains("non-zero tangent"));
}

#[test]
fn wardley_degenerate_marker_is_a_structured_unavailable_result() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "wardley-beta\ncomponent A [0.8, 0.2]\nevolve A 0.2\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("a zero-length Wardley marker must not produce a partial document");

    let RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable {
        family,
        reason,
    }) = error
    else {
        panic!("expected a structured DrawingList-unavailable error, got {error}");
    };
    assert_eq!(family, "wardley");
    assert!(reason.contains("non-zero tangent"));
}

#[test]
fn zenuml_emits_typed_participants_lifelines_messages_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "zenuml\n@Starter(Client)\nClient->Service: call\nService-->Client: done\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable ZenUML should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("zenuml.participant")
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("zenuml.lifeline")
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("zenuml.message")
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run } if run.text == "call"
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("call")
    }));
}

#[test]
fn class_namespace_preserves_theme_background_alpha() {
    use merman_display_list::Color;

    // Mermaid's .cluster rect uses clusterBkg directly, without a separate opacity.
    for (background, expected) in [
        ("transparent", Color::rgba(0, 0, 0, 0)),
        ("#123456", Color::rgba(18, 52, 86, 255)),
        ("#12345680", Color::rgba(18, 52, 86, 128)),
    ] {
        let config = MermaidConfig::from_value(serde_json::json!({
            "theme": "base", "themeVariables": { "clusterBkg": background }
        }));
        let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config));
        let RenderOutput::DrawingList(Some(output)) = renderer
            .render(RenderRequest::drawing_list(
                "classDiagram\nnamespace Core {\nclass Animal\n}\n",
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .unwrap()
        else {
            panic!("expected DrawingList");
        };
        let style = output
            .document()
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "class.namespace.0.box" =>
                {
                    Some(style)
                }
                _ => None,
            })
            .expect("namespace background is drawn");
        assert_eq!(style.fill, Some(Paint::solid(expected)), "{background}");
    }
}

#[test]
fn class_emits_typed_compartments_relations_namespaces_and_notes() {
    let source = r#"classDiagram
        namespace Core {
            class Animal {
                +name: String
                +speak()
            }
        }
        class Dog
        Animal <|-- Dog
        note for Dog "Companion"
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Class diagram should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("class.node")
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("Animal")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.description.as_deref().is_some_and(|description| {
                description.contains("Animal") && description.contains("Dog")
            })
    }));

    let footprint = document
        .footprint()
        .expect("the Class document should have a valid footprint");
    let request = DrawingListRequest {
        limits: DrawingListLimits {
            max_serialized_bytes: output.json().len(),
            max_commands: footprint.commands,
            max_resources: footprint.resources,
            max_path_segments: footprint.path_segments,
            max_stroke_dash_entries: footprint.stroke_dash_entries,
            max_image_bytes: footprint.image_bytes,
            max_image_pixels: footprint.image_pixels,
            max_fallback_pixels: footprint.fallback_pixels,
            max_font_bytes: footprint.font_bytes,
            max_nesting_depth: footprint.max_nesting_depth,
            max_fallbacks: footprint.fallbacks,
            max_text_bytes: footprint.text_bytes,
            max_glyphs: footprint.glyphs,
        },
        ..DrawingListRequest::default()
    };
    let bounded = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            request,
        ))
        .expect("the Class document must fit its exact footprint and serialized size");
    let RenderOutput::DrawingList(Some(bounded)) = bounded else {
        panic!("expected a DrawingList output");
    };
    assert_eq!(bounded.json(), output.json());
}

#[test]
fn class_relation_markers_preserve_aggregation_and_composition_diamonds() {
    let source = r#"classDiagram
        class A
        class B
        class C
        A o-- B
        A *-- C
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("class relation markers should render");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let marker = |id: &str| {
        output
            .document()
            .resources
            .iter()
            .find_map(|resource| match resource {
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == id => {
                    Some(path)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing Class marker resource {id}"))
    };
    let marker_style = |id: &str| {
        output
            .document()
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { path, style } if path.as_str() == id => Some(style),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing Class marker command {id}"))
    };

    for (id, filled) in [
        ("class.edge.0.marker.start", false),
        ("class.edge.1.marker.start", true),
    ] {
        let path = marker(id);
        assert_eq!(
            path.segments.len(),
            5,
            "Class aggregation/composition markers must use closed diamond geometry: {id}"
        );
        assert_eq!(
            marker_style(id).fill.is_some(),
            filled,
            "Class relation marker fill must preserve hollow aggregation vs filled composition: {id}"
        );
    }
}

#[test]
fn class_member_classifiers_preserve_abstract_italic_and_reject_static_decoration() {
    let abstract_source = r#"classDiagram
        class Parser {
            +internalHook() bool*
        }
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            abstract_source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Class abstract member styles should be representable");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let abstract_run = output
        .document()
        .commands
        .iter()
        .find_map(|command| match command {
            DrawingCommand::DrawText { run } if run.text.contains("internalHook()") => Some(run),
            _ => None,
        })
        .expect("abstract member text should be emitted");
    assert_eq!(
        abstract_run.style.font.style,
        merman_display_list::FontStyle::Italic
    );

    let static_error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"classDiagram
                class Parser {
                    +from(v: T) Result~T~$
                }
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("DrawingList must not silently drop static-member underlines");
    assert!(matches!(
        static_error,
        RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable {
            family,
            reason,
        }) if family == "class" && reason.contains("text decoration")
    ));
}

#[test]
fn class_limits_reject_the_first_over_budget_builder_item() {
    let source = r#"classDiagram
        class A {
            +value: String
            +run()
        }
        class B
        A <|-- B
    "#;
    for (id, actual, maximum, limits) in [
        (
            "max_commands",
            1,
            0,
            DrawingListLimits {
                max_commands: 0,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_resources",
            2,
            1,
            DrawingListLimits {
                max_resources: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_text_bytes",
            1,
            0,
            DrawingListLimits {
                max_text_bytes: 0,
                ..DrawingListLimits::default()
            },
        ),
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest {
                    limits,
                    ..DrawingListRequest::default()
                },
            ))
            .expect_err("the Class builder must stop at its first over-budget item");
        let RenderError::ResourceLimitExceeded(limit) = error else {
            panic!("expected a resource-limit error for {id}, got {error}");
        };
        assert_eq!(limit.id, id);
        assert_eq!(limit.phase, "drawing-list-validation");
        assert_eq!(limit.actual, actual, "first over-budget item for {id}");
        assert_eq!(limit.maximum, maximum);
    }
}

#[test]
fn er_emits_typed_entities_attribute_rows_relationships_and_cardinality() {
    let source = r#"erDiagram
        CUSTOMER ||--o{ ORDER : places
        CUSTOMER {
            string id PK
            string name
        }
        ORDER {
            int id PK
        }
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable ER diagram should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("er.entity")
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("er.edge.0.marker")
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "er.entity.0.row.0"
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "er.entity.0.divider.column.0"
    )));
    for text in ["string", "id", "PK", "name"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == text
        )));
    }
    assert!(!document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text.contains("string id")
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("CUSTOMER")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("places")
    }));
}

#[test]
fn er_transparent_edge_label_background_keeps_zero_alpha() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "edgeLabelBackground": "transparent"
        }
    })));
    let output = Renderer::new()
        .with_engine(engine)
        .render(RenderRequest::drawing_list(
            "erDiagram\n  CUSTOMER ||--|| ORDER : places\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("transparent ER relationship labels should render");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let alpha = output.document().commands.iter().find_map(|command| {
        let DrawingCommand::DrawPath { path, style } = command else {
            return None;
        };
        if path.as_str() != "er.edge.0.label.background" {
            return None;
        }
        match style.fill.as_ref() {
            Some(Paint::Solid { color }) => Some(color.alpha),
            _ => None,
        }
    });
    assert_eq!(
        alpha,
        Some(0),
        "transparent edgeLabelBackground must not be replaced by label-box opacity"
    );
}

#[test]
fn er_rejects_svg_only_effects_instead_of_silently_flattening_them() {
    for (source, expected) in [
        (
            "%%{init: {\"look\": \"neo\"}}%%\nerDiagram\n  A ||--|| B : owns\n",
            "drop-shadow filters",
        ),
        (
            "---\nconfig:\n  theme: redux-color\n---\nerDiagram\n  A ||--|| B : owns\n",
            "per-entity palette semantics",
        ),
        (
            "%%{init: {\"handDrawnSeed\": 7}}%%\nerDiagram\n  A ||--|| B : owns\n",
            "explicit handDrawnSeed",
        ),
        (
            "erDiagram\n  \"This **is** _Markdown_\"\n",
            "styled Markdown",
        ),
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect_err("SVG-only ER effects must remain explicit");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?} in {error}"
        );
    }
}

#[test]
fn c4_emits_typed_boundaries_shapes_relations_and_arrows() {
    let source = r#"C4Context
        Person(user, "User")
        System(api, "API", "Service")
        System(web, "Web", "Frontend")
        Rel(user, api, "uses")
        Rel(api, web, "serves")
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable C4 diagram should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("c4.shape")
    )));
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str().contains("c4.relation.0.marker.end")
    )));
    let second_relation = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str() == "c4.relation.1.route" =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("second C4 relationship has a route resource");
    assert!(matches!(
        second_relation.segments.as_slice(),
        [
            merman_display_list::PathSegment::MoveTo { .. },
            merman_display_list::PathSegment::QuadTo { control, .. }
        ] if control.x.is_finite() && control.y.is_finite()
    ));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("User")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("uses")
    }));
}

#[test]
fn c4_limits_stop_at_the_first_over_budget_command() {
    let source = r#"C4Context
        Person(user, "User")
        System(api, "API", "Service")
        Rel(user, api, "uses")
    "#;
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("C4 must reject its first command before constructing the document");
    assert!(
        matches!(
            &error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == "max_commands"
                    && limit.phase == "drawing-list-validation"
                    && limit.actual == 1
                    && limit.maximum == 0
        ),
        "expected first C4 command to hit max_commands, got {error}"
    );
}

#[test]
fn c4_links_use_the_shared_strict_and_loose_navigation_boundary() {
    let source = r#"C4Context
        Person(user, "User", "Description", "", "", "javascript:alert(1)")
        System(api, "API", "Service", "", "", "https://example.test/api")
        Rel(user, api, "uses", "", "", "", "", "javascript:alert(2)")
        Boundary(boundary, "Boundary", "enterprise", "", "javascript:alert(3)") {
            System(inner, "Inner", "Service")
        }
    "#;

    let strict = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("strict C4 links should be represented safely");
    let RenderOutput::DrawingList(Some(strict)) = strict else {
        panic!("expected a DrawingList output");
    };
    let strict_links = strict
        .document()
        .semantics
        .iter()
        .filter_map(|semantic| semantic.link.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(strict_links, vec!["https://example.test/api"]);

    let loose_config = MermaidConfig::from_value(serde_json::json!({
        "securityLevel": "loose"
    }));
    let loose = Renderer::new()
        .with_engine(Engine::new().with_site_config(loose_config))
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("loose C4 links should preserve authored navigation");
    let RenderOutput::DrawingList(Some(loose)) = loose else {
        panic!("expected a DrawingList output");
    };
    let loose_links = loose
        .document()
        .semantics
        .iter()
        .filter_map(|semantic| semantic.link.as_deref())
        .collect::<Vec<_>>();
    assert!(loose_links.contains(&"javascript:alert(1)"));
    assert!(loose_links.contains(&"javascript:alert(2)"));
    assert!(loose_links.contains(&"javascript:alert(3)"));
    assert!(loose_links.contains(&"https://example.test/api"));
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn architecture_emits_typed_icons_groups_edges_and_semantics() {
    let source = r#"%%{init: {"architecture": {"numIter": 1, "randomize": false}}}%%
architecture-beta
    group core(cloud)[Core]
    service api(server)[API] in core
    service db(database)[Database] in core
    api:R --> L:db
"#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Architecture should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    for path_prefix in [
        "architecture.node.0.icon",
        "architecture.group.0.outline",
        "architecture.edge.0.route",
        "architecture.edge.0.arrow",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str().starts_with(path_prefix)
        )));
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.title.as_deref() == Some("Core")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("API")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.description.as_deref() == Some("api → db")
    }));
}

#[cfg(feature = "layout-cytoscape")]
#[test]
fn architecture_limits_reject_before_first_emitted_command() {
    let source = "architecture-beta\n    service api(server)[API]\n";
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("Architecture must reject its first builder command");
    assert!(
        matches!(
            &error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == "max_commands"
                    && limit.phase == "drawing-list-validation"
                    && limit.actual == 1
                    && limit.maximum == 0
        ),
        "{error}"
    );
}

#[cfg(feature = "layout-cytoscape")]
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

#[cfg(feature = "layout-cytoscape")]
#[test]
fn mindmap_limits_stop_at_the_first_over_budget_command() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "mindmap\n  root((Merman))\n    Parser\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("Mindmap must reject its first builder command");
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_commands"
                && limit.phase == "drawing-list-validation"
                && limit.actual == 1
                && limit.maximum == 0
    ));
}

#[cfg(feature = "layout-cytoscape")]
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
fn drawing_list_rejects_unresolved_theme_css_at_the_shared_family_boundary() {
    for (source, family, lenient) in [
        ("flowchart TD\nA -->", "error", true),
        ("flowchart TD\nA --> B\n", "flowchart", false),
        (
            "swimlane-beta LR\n  subgraph Customer\n    request[Request]\n  end\n",
            "swimlane",
            false,
        ),
    ] {
        let site_config = MermaidConfig::from_value(serde_json::json!({
            "themeCSS": ".node { opacity: 0.5; }",
        }));
        let mut request = RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        );
        if lenient {
            request = request.with_parse_options(merman::ParseOptions::lenient());
        }
        let error = Renderer::new()
            .with_engine(Engine::new().with_site_config(site_config))
            .render(request)
            .unwrap_err();

        let RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable {
            family: actual_family,
            reason,
        }) = error
        else {
            panic!("expected a structured DrawingList-unavailable error, got {error}");
        };
        assert_eq!(actual_family, family);
        assert!(reason.contains("effect `themeCSS`"), "{reason}");
    }
}

#[test]
fn drawing_list_allows_empty_theme_css_at_the_shared_family_boundary() {
    for (source, lenient) in [
        ("flowchart TD\nA -->", true),
        ("flowchart TD\nA --> B\n", false),
        (
            "swimlane-beta LR\n  subgraph Customer\n    request[Request]\n  end\n",
            false,
        ),
    ] {
        let site_config = MermaidConfig::from_value(serde_json::json!({ "themeCSS": "  \n\t" }));
        let mut request = RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        );
        if lenient {
            request = request.with_parse_options(merman::ParseOptions::lenient());
        }
        Renderer::new()
            .with_engine(Engine::new().with_site_config(site_config))
            .render(request)
            .expect("empty themeCSS must not make DrawingList unavailable");
    }
}

#[test]
fn drawing_list_footprint_is_admitted_to_operation_work_accounting() {
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

    let footprint = output
        .document()
        .footprint()
        .expect("the returned document is balanced");
    let work_units = footprint
        .work_units()
        .expect("the returned footprint is representable");
    assert!(
        output.evidence().layout_work_units() >= work_units,
        "document footprint must be charged before output is committed"
    );
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
fn state_command_limit_stops_before_the_first_output_command() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "stateDiagram-v2\n  A --> B\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("State must reject its first command before returning output");

    assert!(
        matches!(
            &error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == "max_commands"
                    && limit.phase == "drawing-list-validation"
                    && limit.actual == 1
                    && limit.maximum == 0
        ),
        "{error}"
    );
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
fn requirement_emits_typed_nodes_relationships_markers_labels_and_semantics() {
    let source = r#"requirementDiagram
        direction LR
        requirement req1 {
            id: REQ-1
            text: Login
            risk: high
            verifymethod: test
        }
        element system {
            type: service
            docref: docs
        }
        system - satisfies -> req1
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Requirement should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "requirement.edge.0.route"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::CubicTo { .. }
                ))
    )));
    for path_id in [
        "requirement.edge.0.marker.end",
        "requirement.node.0.shape.fill",
        "requirement.node.0.shape.stroke",
        "requirement.node.1.shape.fill",
        "requirement.node.1.shape.stroke",
    ] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }
    let divider_origin = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str() == "requirement.node.0.divider" =>
            {
                match path.segments.first() {
                    Some(merman_display_list::PathSegment::MoveTo { to }) => Some(*to),
                    _ => None,
                }
            }
            _ => None,
        })
        .expect("Requirement divider origin");
    assert!(divider_origin.y >= document.viewport.bounds.y);
    for text in ["req1", "system", "<<satisfies>>"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == text
        )));
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("req1")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("<<satisfies>>")
    }));
}

#[test]
fn requirement_limits_reject_before_first_output_command() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "requirementDiagram\n  requirement req1 {\n    text: Login\n  }\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("Requirement must reject its first builder command");

    assert!(
        matches!(
            &error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == "max_commands"
                    && limit.phase == "drawing-list-validation"
                    && limit.actual == 1
                    && limit.maximum == 0
        ),
        "{error}"
    );
}

#[test]
fn requirement_preserves_seeded_rough_geometry_in_the_public_drawing_list() {
    let render_stroke = |seed: u64| {
        let source = format!(
            r#"%%{{init: {{"handDrawnSeed": {seed}}}}}%%
requirementDiagram
  requirement req1 {{
    id: REQ-1
    text: Seeded requirement
    risk: high
    verifymethod: test
  }}
"#
        );
        let output = Renderer::new()
            .render(RenderRequest::drawing_list(
                &source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect("seeded Requirement should render as vector DrawingList geometry");
        let RenderOutput::DrawingList(Some(output)) = output else {
            panic!("expected a DrawingList output");
        };
        output
            .document()
            .resources
            .iter()
            .find_map(|resource| match resource {
                merman_display_list::DrawingResource::Path(path)
                    if path.id.as_str() == "requirement.node.0.shape.stroke" =>
                {
                    Some(path.segments.clone())
                }
                _ => None,
            })
            .expect("Requirement rough stroke path")
    };

    let seed_7 = render_stroke(7);
    assert_eq!(seed_7, render_stroke(7));
    assert_ne!(seed_7, render_stroke(8));
    assert!(
        seed_7
            .iter()
            .any(|segment| matches!(segment, merman_display_list::PathSegment::CubicTo { .. }))
    );
}

#[test]
fn requirement_rejects_rich_markdown_instead_of_flattening_it_silently() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"requirementDiagram
        requirement req1 {
            text: **bold** requirement
        }
    "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("rich Requirement labels must remain an explicit capability boundary");
    assert!(error.to_string().contains("cannot preserve"));
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
fn pie_limits_stop_at_the_first_over_budget_item() {
    let source = "pie showData title Releases\n\"Stable\" : 3\n\"Alpha\" : 1\n";
    for (id, actual, maximum, limits) in [
        (
            "max_commands",
            6,
            5,
            DrawingListLimits {
                max_commands: 5,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_resources",
            2,
            1,
            DrawingListLimits {
                max_resources: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_path_segments",
            5,
            1,
            DrawingListLimits {
                max_path_segments: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_text_bytes",
            6,
            3,
            DrawingListLimits {
                max_text_bytes: 3,
                ..DrawingListLimits::default()
            },
        ),
        (
            "max_nesting_depth",
            3,
            2,
            DrawingListLimits {
                max_nesting_depth: 2,
                ..DrawingListLimits::default()
            },
        ),
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest {
                    limits,
                    ..DrawingListRequest::default()
                },
            ))
            .expect_err("the Pie builder must reject the first over-budget item without output");
        let RenderError::ResourceLimitExceeded(limit) = error else {
            panic!("expected a resource-limit error for {id}, got {error}");
        };
        assert_eq!(limit.id, id);
        assert_eq!(limit.phase, "drawing-list-validation");
        assert_eq!(limit.actual, actual, "first over-budget item for {id}");
        assert_eq!(limit.maximum, maximum);
    }
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
fn packet_resource_limit_reaches_the_bounded_document_builder() {
    let site_config = MermaidConfig::from_value(serde_json::json!({
        "packet": {
            "blockFillColor": "#123456"
        }
    }));
    let request = DrawingListRequest {
        limits: DrawingListLimits {
            max_resources: 1,
            ..DrawingListLimits::default()
        },
        ..DrawingListRequest::default()
    };
    let error = Renderer::new()
        .with_engine(Engine::new().with_site_config(site_config))
        .render(RenderRequest::drawing_list(
            "packet\n0-7: \"Version\"\n8-15: \"Length\"\n",
            OperationControl::new(),
            request,
        ))
        .expect_err("the Packet builder must reject its second path resource");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_resources"
                && limit.phase == "drawing-list-validation"
                && limit.actual == 2
                && limit.maximum == 1
    ));
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
fn sankey_outlined_labels_preserve_two_source_ordered_text_layers() {
    let config = MermaidConfig::from_value(serde_json::json!({
        "theme": "base",
        "themeVariables": {"mainBkg": "#12345680", "textColor": "#abcdef"},
        "sankey": {"labelStyle": "outlined"}
    }));
    let output = Renderer::new()
        .with_engine(Engine::new().with_site_config(config))
        .render(RenderRequest::drawing_list(
            "sankey\nA,B,10\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("outlined Sankey labels have portable text stroke semantics");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected DrawingList");
    };
    let runs = output
        .document()
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawingCommand::DrawText { run } => Some(run),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(runs.len(), 4);
    for (background, foreground) in runs[..2].iter().zip(&runs[2..]) {
        assert_eq!(background.text, foreground.text);
        assert_eq!(background.origin, foreground.origin);
        let stroke = background
            .style
            .stroke
            .as_ref()
            .expect("background outline");
        assert_eq!(stroke.width, 4.0);
        assert_eq!(stroke.line_join, LineJoin::Round);
        assert_eq!(
            stroke.paint,
            Paint::solid(merman_display_list::Color::rgba(18, 52, 86, 128))
        );
        assert_eq!(background.style.paint_order, TextPaintOrder::StrokeThenFill);
        assert!(foreground.style.stroke.is_none());
        assert_eq!(
            foreground.style.fill,
            Paint::solid(merman_display_list::Color::rgba(171, 205, 239, 255))
        );
    }
}

#[test]
fn radar_text_inherits_fill_independently_of_css_color() {
    let config = MermaidConfig::from_value(serde_json::json!({
        "theme": "base",
        "themeVariables": {
            "textColor": "#123456", "titleColor": "#00ff00",
            "radar": {"axisColor": "#ff0000"}
        }
    }));
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config));
    let RenderOutput::DrawingList(Some(output)) = renderer
        .render(RenderRequest::drawing_list(
            "radar-beta\n title Health\n axis Speed, Quality, Reach\n curve Current{80, 60, 90}\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .unwrap()
    else {
        panic!("expected DrawingList");
    };
    let commands = &output.document().commands;
    for text in ["Health", "Speed", "Quality", "Reach"] {
        let run = commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawText { run } if run.text == text => Some(run),
                _ => None,
            })
            .expect("Radar text is drawn");
        assert_eq!(
            run.style.fill,
            Paint::solid(merman_display_list::Color::rgba(18, 52, 86, 255)),
            "{text}: CSS color does not replace SVG text fill"
        );
    }
    let line = commands
        .iter()
        .find_map(|command| match command {
            DrawingCommand::DrawPath { path, style } if path.as_str() == "radar.axis.0.line" => {
                Some(style)
            }
            _ => None,
        })
        .expect("axis is drawn");
    assert_eq!(
        line.stroke.as_ref().unwrap().paint,
        Paint::solid(merman_display_list::Color::rgba(255, 0, 0, 255))
    );
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
    let fill_alpha = |path_id: &str| {
        document.commands.iter().find_map(|command| match command {
            merman_display_list::DrawingCommand::DrawPath { path, style }
                if path.as_str() == path_id =>
            {
                match style.fill.as_ref() {
                    Some(merman_display_list::Paint::Solid { color }) => Some(color.alpha),
                    _ => None,
                }
            }
            _ => None,
        })
    };
    assert_eq!(fill_alpha("radar.graticule.0.shape"), Some(77));
    assert_eq!(fill_alpha("radar.curve.0.shape"), Some(128));

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
    let area_draws = document
        .commands
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            if let merman_display_list::DrawingCommand::DrawPath { path, style } = command
                && path.as_str() == "venn.area.0.shape"
            {
                Some((index, style))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        area_draws.len(),
        2,
        "Venn fill and stroke opacity must remain independently painted"
    );
    for ((index, style), opacity) in area_draws.iter().zip([0.42, 0.95]) {
        assert!(matches!(document.commands[index - 1],
            merman_display_list::DrawingCommand::SetOpacity { opacity: actual } if actual == opacity));
        assert!(matches!(
            document.commands[index + 1],
            merman_display_list::DrawingCommand::Restore
        ));
        let paint = style
            .fill
            .as_ref()
            .or_else(|| style.stroke.as_ref().map(|stroke| &stroke.paint))
            .unwrap();
        assert!(matches!(paint, merman_display_list::Paint::Solid { color } if color.alpha == 255));
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
fn venn_area_labels_use_detached_js_whitespace_without_width_wrapping() {
    let label =
        "A\u{a0}very\u{2003}long\u{feff}label with many words that cannot fit a tiny circle";
    let source = format!(
        "---\nconfig:\n  venn:\n    width: 160\n    height: 100\n---\nvenn-beta\ntitle Heading\u{a0}Title\nset A[\"{label}\"]\n"
    );
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            &source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .unwrap();
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("DrawingList output")
    };
    let runs = output
        .document()
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawingCommand::DrawText { run } => Some(run),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        runs.len(),
        2,
        "detached Venn area labels stay single-line even when wider than their circle"
    );
    assert_eq!(
        runs[0].text, "Heading\u{a0}Title",
        "visible titles use SVG whitespace, not the Venn.js token loop"
    );
    assert_eq!(
        runs[1].text,
        "A very long label with many words that cannot fit a tiny circle"
    );
    assert!(
        output
            .document()
            .semantics
            .iter()
            .any(|semantic| semantic.id == "venn.area.0"
                && semantic.title.as_deref() == Some(runs[1].text.as_str()))
    );
}

#[test]
fn venn_emits_normal_plain_text_nodes_without_losing_literal_content() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"venn-beta
set A["Frontend"]:20
  text A1["React   <br>   A B"]
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Venn normal plain text must have resolved DrawingList lines");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected DrawingList");
    };
    let mut in_text_node = false;
    let mut lines = Vec::new();
    for command in &output.document().commands {
        match command {
            DrawingCommand::BeginSemanticGroup { semantic_id } if semantic_id == "venn.text.0" => {
                in_text_node = true
            }
            DrawingCommand::EndSemanticGroup => in_text_node = false,
            DrawingCommand::DrawText { run } if in_text_node => {
                assert_eq!(run.baseline, merman_display_list::TextBaseline::Alphabetic);
                assert!(run.style.line_height > 0.0);
                lines.push(run.text.as_str());
            }
            _ => {}
        }
    }
    assert_eq!(lines.join(" "), "React <br> A B");
}

#[test]
fn venn_text_nodes_keep_theme_color_overrides_and_interleaved_debug_cells() {
    let output = Renderer::new()
        .with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "venn": {"useDebugLayout": true},
                "themeVariables": {"vennSetTextColor": "#123456"}
            }))),
        )
        .render(RenderRequest::drawing_list(
            r##"venn-beta
set A["Alpha"]:20
  text A1["First"]
  text A2["Second"]
style A1 color:red
"##,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .unwrap();
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected DrawingList");
    };
    let mut order = Vec::new();
    for command in &output.document().commands {
        match command {
            DrawingCommand::DrawPath { path, style } if path.as_str().contains(".debug-") => {
                assert!(!style.stroke.as_ref().unwrap().dash_array.is_empty());
                order.push(path.as_str().to_owned());
            }
            DrawingCommand::DrawText { run } if matches!(run.text.as_str(), "First" | "Second") => {
                let expected = if run.text == "First" {
                    merman_display_list::Color::rgba(255, 0, 0, 255)
                } else {
                    merman_display_list::Color::rgba(18, 52, 86, 255)
                };
                assert_eq!(run.style.fill, Paint::solid(expected));
                order.push(run.text.clone());
            }
            _ => {}
        }
    }
    assert_eq!(
        order,
        [
            "venn.text-area.0.debug-circle",
            "venn.text.0.debug-cell",
            "First",
            "venn.text.1.debug-cell",
            "Second"
        ]
    );
}

#[test]
fn venn_emits_bounded_rough_paths_instead_of_substituting_classic_geometry() {
    let output = Renderer::new()
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
        .expect("hand-drawn Venn should emit bounded vector geometry");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected DrawingList");
    };
    let document = output.document();
    for suffix in ["rough-fill", "rough-outline"] {
        let id = format!("venn.area.0.{suffix}");
        let resource = document
            .resources
            .iter()
            .find_map(|resource| match resource {
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == id => {
                    Some(path)
                }
                _ => None,
            })
            .expect("rough paths must be public resources");
        assert!(
            resource.segments.len() > 8,
            "rough geometry must not be a classic ellipse"
        );
        assert!(document.commands.iter().any(|command| matches!(
            command, DrawingCommand::DrawPath { path, style }
                if path.as_str() == id && style.fill.is_none() && style.stroke.is_some()
        )));
    }
}

#[test]
fn venn_rough_path_limit_rejects_the_first_unadmitted_segment() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "---\nconfig:\n  look: handDrawn\n---\nvenn-beta\nset A\nset B\nunion A,B\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_path_segments: 8,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect_err("rough production must stop before retaining a complete path");
    let RenderError::ResourceLimitExceeded(limit) = error else {
        panic!("expected a path resource limit, got {error}");
    };
    assert_eq!(limit.id, "max_path_segments");
    assert_eq!(limit.actual, 9);
    assert_eq!(limit.maximum, 8);
}

#[test]
fn venn_hand_drawn_disjoint_custom_intersection_keeps_semantics_without_empty_paths() {
    let output = Renderer::new().render(RenderRequest::drawing_list(
        "---\nconfig:\n  look: handDrawn\n---\nvenn-beta\nset A:10\nset B:10\nunion A,B:0\nstyle A,B fill:#ff0000\n",
        OperationControl::new(), DrawingListRequest::default(),
    )).expect("a zero-area styled intersection is valid");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected DrawingList");
    };
    assert!(
        output
            .document()
            .semantics
            .iter()
            .any(|semantic| semantic.id == "venn.area.2")
    );
    assert!(
        output
            .document()
            .resources
            .iter()
            .all(|resource| match resource {
                merman_display_list::DrawingResource::Path(path) => !path.segments.is_empty(),
                _ => true,
            })
    );
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
fn tree_view_missing_icon_uses_the_source_unknown_icon() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "treeView-beta\nRoot icon(logos:react)\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .unwrap();
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("DrawingList expected")
    };
    let document = output.document();
    let text = document
        .commands
        .iter()
        .find_map(|command| match command {
            DrawingCommand::DrawText { run } if run.text == "?" => Some(run),
            _ => None,
        })
        .expect("missing Iconify name uses the Mermaid fallback, not an empty icon");
    assert_eq!(text.origin, merman_display_list::Point::new(21.16, 64.67));
    assert_eq!(text.style.font_size, 67.75);
    assert_eq!(text.style.font.families, ["ArialMT", "Arial"]);
    assert!(
        document
            .commands
            .iter()
            .any(|command| matches!(command, DrawingCommand::ClipPath { .. }))
    );
    assert!(document.commands.iter().any(
        |command| matches!(command, DrawingCommand::DrawPath { style, .. }
        if style.fill == Some(Paint::solid(merman_display_list::Color::rgba(8, 126, 191, 255))))
    ));
}

#[test]
fn tree_view_registered_icon_preserves_viewport_alias_and_current_color() {
    use merman_display_list::{Color, Point, Transform};
    let pack = br#"{"prefix":"test","width":20,"height":10,"icons":{"box":{"body":"<path fill=\"currentColor\" d=\"M0 0H20V10H0Z\"/>"}},"aliases":{"turned":{"parent":"box","rotate":1}}}"#;
    let registry =
        merman::svg::IconRegistry::from_packs([merman::svg::IconPack::new(pack)]).unwrap();
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
        MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "treeView": { "iconColor": "#123456" } }
        })),
    ));
    for (name, expected) in [
        (
            "box",
            Transform {
                a: 0.7,
                d: 0.7,
                e: 0.0,
                f: 3.5,
                ..Transform::IDENTITY
            },
        ),
        (
            "turned",
            Transform {
                a: 0.0,
                b: 0.7,
                c: -0.7,
                d: 0.0,
                e: 10.5,
                f: 0.0,
            },
        ),
    ] {
        let output = renderer
            .render(RenderRequest::drawing_list(
                &format!("treeView-beta\nRoot icon(test:{name})\n"),
                OperationControl::new(),
                DrawingListRequest {
                    environment: merman::SvgEnvironment::deterministic()
                        .with_icon_registry(registry.clone()),
                    ..Default::default()
                },
            ))
            .unwrap();
        let RenderOutput::DrawingList(Some(output)) = output else {
            panic!("DrawingList expected")
        };
        let document = output.document();
        let mut matrix = Transform::IDENTITY;
        let mut saves = Vec::new();
        let mut painted = 0;
        let mut viewport_origin = None;
        for command in &document.commands {
            match command {
                DrawingCommand::Save => saves.push(matrix),
                DrawingCommand::Restore => matrix = saves.pop().unwrap(),
                DrawingCommand::ConcatTransform { transform: right } => {
                    matrix = Transform {
                        a: matrix.a * right.a + matrix.c * right.b,
                        b: matrix.b * right.a + matrix.d * right.b,
                        c: matrix.a * right.c + matrix.c * right.d,
                        d: matrix.b * right.c + matrix.d * right.d,
                        e: matrix.a * right.e + matrix.c * right.f + matrix.e,
                        f: matrix.b * right.e + matrix.d * right.f + matrix.f,
                    }
                }
                DrawingCommand::ClipPath { path, .. } => {
                    assert!(path.as_str().ends_with(".asset.clip"));
                    assert_eq!(
                        (matrix.a, matrix.b, matrix.c, matrix.d),
                        (1.0, 0.0, 0.0, 1.0)
                    );
                    viewport_origin = Some(Point::new(matrix.e, matrix.f));
                }
                DrawingCommand::DrawPath { path, style }
                    if path.as_str().contains(".asset.shape.") =>
                {
                    let origin = viewport_origin.expect("viewport clip precedes asset geometry");
                    assert_eq!(
                        matrix,
                        Transform {
                            e: origin.x + expected.e,
                            f: origin.y + expected.f,
                            ..expected
                        }
                    );
                    assert_eq!(
                        style.fill,
                        Some(Paint::solid(Color::rgba(0x12, 0x34, 0x56, 255)))
                    );
                    assert!(style.stroke.is_none());
                    let resource = document
                        .resources
                        .iter()
                        .find_map(|resource| match resource {
                            merman_display_list::DrawingResource::Path(resource)
                                if &resource.id == path =>
                            {
                                Some(resource)
                            }
                            _ => None,
                        })
                        .unwrap();
                    assert_eq!(
                        resource.segments[0],
                        merman_display_list::PathSegment::MoveTo {
                            to: Point::new(0.0, 0.0)
                        }
                    );
                    painted += 1;
                }
                _ => {}
            }
        }
        assert_eq!(painted, 1);
        assert!(
            !document.commands.iter().any(
                |command| matches!(command, DrawingCommand::DrawText { run } if run.text == "?")
            )
        );
    }
}

#[test]
fn tree_view_rejects_unsupported_registry_effects_instead_of_substituting_fallback_art() {
    let pack = br#"{"prefix":"test","icons":{"filtered":{"body":"<path d=\"M0 0L16 16\" style=\"filter:blur(2px)\"/>"}}}"#;
    let registry =
        merman::svg::IconRegistry::from_packs([merman::svg::IconPack::new(pack)]).unwrap();
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "treeView-beta\nRoot icon(test:filtered)\n",
            OperationControl::new(),
            DrawingListRequest {
                environment: merman::SvgEnvironment::deterministic().with_icon_registry(registry),
                ..Default::default()
            },
        ))
        .expect_err("an existing non-portable icon must not be replaced by the unknown icon");
    assert!(error.to_string().contains("filter"), "{error}");
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
    assert_eq!(
        leaf_paints.len(),
        1,
        "a source rect is one compositing object"
    );
    assert_eq!(
        leaf_paints[0].fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0xe7, 0x4c, 0x3c, 77)
        ))
    );
    let stroke = leaf_paints[0]
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
fn treemap_limits_stop_at_the_first_over_budget_command() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "treemap-beta\n\"Root\"\n  \"Child\": 1\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("Treemap must reject its first builder command");
    assert!(
        matches!(
            &error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == "max_commands"
                    && limit.phase == "drawing-list-validation"
                    && limit.actual == 1
                    && limit.maximum == 0
        ),
        "{error}"
    );
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
fn cynefin_emits_domains_boundaries_transitions_and_overflow_items() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"cynefin-beta
  title Team Practices
  clear
    "Runbook"
  complex
    "Retrospective"
  confusion
    "A"
    "B"
    "C"
    "D"
  clear --> complex : "Probe"
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Cynefin should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert_eq!(document.viewport.bounds.width, 880.0);
    assert_eq!(document.viewport.bounds.height, 680.0);
    // Upstream badges use top-left layout coordinates with text centered inside the rect.
    // Check both normal and overflow badges, not just the presence of path resources.
    for item in document
        .semantics
        .iter()
        .filter(|semantic| semantic.id.starts_with("cynefin.item."))
    {
        let path_id = format!("{}.shape", item.id);
        let path = document
            .resources
            .iter()
            .find_map(|resource| match resource {
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id => {
                    Some(path)
                }
                _ => None,
            })
            .expect("item shape");
        let run = document
            .commands
            .iter()
            .find_map(|command| match command {
                merman_display_list::DrawingCommand::DrawText { run }
                    if Some(run.text.as_str()) == item.title.as_deref() =>
                {
                    Some(run)
                }
                _ => None,
            })
            .expect("item label");
        let mut left = f64::INFINITY;
        let mut right = f64::NEG_INFINITY;
        let mut top = f64::INFINITY;
        let mut bottom = f64::NEG_INFINITY;
        for segment in &path.segments {
            use merman_display_list::PathSegment;
            let point = match segment {
                PathSegment::MoveTo { to }
                | PathSegment::LineTo { to }
                | PathSegment::ArcTo { to, .. } => to,
                PathSegment::Close => continue,
                _ => panic!("badge should be a rounded rectangle"),
            };
            left = left.min(point.x);
            right = right.max(point.x);
            top = top.min(point.y);
            bottom = bottom.max(point.y);
        }
        assert!(
            ((left + right) / 2.0 - run.origin.x).abs() < 1e-9,
            "{} badge and label must share their horizontal center",
            item.id
        );
        assert!(
            ((top + bottom) / 2.0 - run.origin.y).abs() < 1e-9,
            "{} badge and label must share their vertical center",
            item.id
        );
    }
    for path_id in [
        "cynefin.boundary.fold",
        "cynefin.boundary.horizontal",
        "cynefin.boundary.cliff",
        "cynefin.domain.confusion.background",
        "cynefin.transition.0.line",
        "cynefin.transition.0.arrowhead",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "cynefin.boundary.fold"
                && path.segments.iter().any(|segment| matches!(
                    segment,
                    merman_display_list::PathSegment::CubicTo { .. }
                ))
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::SetOpacity { opacity }
            if (*opacity - 0.4).abs() <= f64::EPSILON
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "+1 more"
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Probe" && run.baseline == merman_display_list::TextBaseline::Alphabetic
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Label
            && semantic.title.as_deref() == Some("Team Practices")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.description.as_deref() == Some("clear → complex")
    }));
}

#[test]
fn gantt_emits_axes_rows_task_states_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"gantt
  title Release Plan
  dateFormat YYYY-MM-DD
  topAxis
  todayMarker off
  section Core
  Build :a1, 2026-01-01, 4d
  Ship :crit, milestone, 2026-01-05, 1d
  section Follow-up
  Docs :done, 2026-01-06, 2d
"#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Gantt should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "gantt.background",
        "gantt.axis.bottom.domain",
        "gantt.axis.top.domain",
        "gantt.row.0",
        "gantt.task.0.bar",
        "gantt.task.1.bar",
        "gantt.task.2.bar",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Release Plan" && run.style.font_size == 18.0
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text.trim() == "Ship"
                && run.style.font.style == merman_display_list::FontStyle::Italic
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic
                .title
                .as_deref()
                .is_some_and(|title| title.trim() == "Docs")
    }));
    assert!(
        document.fallbacks.is_empty(),
        "Gantt's built-in vector primitives must not rasterize"
    );
}

#[test]
fn gantt_command_limit_reaches_the_bounded_document_builder() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "gantt\n  section Delivery\n  Task :task, 2026-01-01, 1d\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 0,
                    ..DrawingListLimits::default()
                },
                ..DrawingListRequest::default()
            },
        ))
        .expect_err("the Gantt builder must reject its first command before returning output");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_commands"
                && limit.phase == "drawing-list-validation"
                && limit.actual == 1
                && limit.maximum == 0
    ));
}

#[test]
fn wardley_emits_axes_pipeline_markers_overlays_and_semantics() {
    let source = r#"---
config:
  wardley-beta:
    showGrid: true
---
wardley-beta
title Platform Strategy
accTitle: Platform map
accDescr: Strategic platform evolution
anchor Customer [0.90, 0.95]
component API [0.70, 0.65] (buy)
component Database [0.50, 0.45] (inertia)
pipeline Database {
  component File System [0.25]
  component SQL DB [0.50]
}
Customer +> API
API -> Database
evolve API 0.85
note "Build mobile-first" [0.85, 0.90]
annotations [0.10, 0.20]
annotation 1,[0.78, 0.82] "User touchpoints"
accelerator "Cloud Native" [0.20, 0.85]
deaccelerator "Legacy Data" [0.45, 0.35]
"#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Wardley should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "wardley.background",
        "wardley.axis.x",
        "wardley.axis.y",
        "wardley.grid.0.vertical",
        "wardley.pipeline.0.box",
        "wardley.pipeline.0.link",
        "wardley.link.0.line",
        "wardley.link.0.end",
        "wardley.trend.0.line",
        "wardley.trend.0.end",
        "wardley.node.1.shape",
        "wardley.node.1.overlay.buy",
        "wardley.annotation.0.point.0",
        "wardley.accelerator.0.shape",
        "wardley.deaccelerator.0.shape",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::ConcatTransform { .. }
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "Platform Strategy"
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.baseline == merman_display_list::TextBaseline::Central
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("API")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.description.as_deref() == Some("Customer → API")
    }));
}

#[test]
fn gitgraph_emits_branches_commits_tags_and_parent_arrows() {
    let source = r#"gitGraph
  commit id: "base"
  branch feature
  checkout feature
  commit id: "highlight" type: HIGHLIGHT tag: "v1"
  commit id: "reverse" type: REVERSE
  checkout main
  merge feature id: "release"
  cherry-pick id: "highlight" tag: "backport"
"#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("GitGraph should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "gitgraph.branch.0.line",
        "gitgraph.branch.0.label.background",
        "gitgraph.arrow.0.path",
        "gitgraph.commit.0.circle",
        "gitgraph.commit.1.highlight.outer",
        "gitgraph.commit.2.reverse.cross",
        "gitgraph.commit.3.merge.inner",
        "gitgraph.commit.4.cherry-pick.left",
        "gitgraph.commit.1.tag.0.background",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run }
            if run.text == "release"
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.title.as_deref() == Some("highlight")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.title.as_deref() == Some("base → highlight")
    }));
    assert!(document.fallbacks.is_empty());
}

#[test]
fn gitgraph_command_limit_reaches_the_bounded_document_builder() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            "gitGraph\n  commit id: \"base\"\n",
            OperationControl::new(),
            DrawingListRequest {
                limits: DrawingListLimits {
                    max_commands: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .expect_err("GitGraph must reject the first command beyond the caller limit");
    let RenderError::ResourceLimitExceeded(limit) = error else {
        panic!("expected resource limit, got {error}");
    };
    assert_eq!(limit.id, "max_commands");
    assert_eq!(limit.phase, "drawing-list-validation");
    assert_eq!(limit.actual, 3);
    assert_eq!(limit.maximum, 2);
}

#[test]
fn timeline_connectors_use_the_winning_line_wrapper_color() {
    let config = MermaidConfig::from_value(serde_json::json!({
        "theme": "base", "themeVariables": {
            "THEME_COLOR_LIMIT": 2, "cScaleLabel1": "#123456", "cScaleInv1": "#abcdef"
        }
    }));
    let renderer = Renderer::new().with_engine(Engine::new().with_site_config(config));
    let RenderOutput::DrawingList(Some(output)) = renderer
        .render(RenderRequest::drawing_list(
            "timeline\nsection Work\nPlan : Build\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .unwrap()
    else {
        panic!("expected DrawingList")
    };
    for id in ["timeline.activity.line", "timeline.task.0.connector.0.line"] {
        let style = output
            .document()
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { path, style } if path.as_str() == id => Some(style),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            style.stroke.as_ref().unwrap().paint,
            Paint::solid(merman_display_list::Color::rgba(18, 52, 86, 255)),
            "{id}"
        );
    }
}

#[test]
fn timeline_neo_effects_are_not_silently_dropped() {
    for config in [
        serde_json::json!({"look":"neo","theme":"base","themeVariables":{"useGradient":true}}),
        serde_json::json!({"look":"neo","theme":"redux","themeVariables":{"useGradient":false}}),
    ] {
        let renderer = Renderer::new()
            .with_engine(Engine::new().with_site_config(MermaidConfig::from_value(config)));
        for policy in [
            DrawingListPolicy::VectorOnly,
            DrawingListPolicy::AllowRasterSubtree,
        ] {
            let error = renderer
                .render(RenderRequest::drawing_list(
                    "timeline\nsection Work\nPlan : Build\n",
                    OperationControl::new(),
                    DrawingListRequest {
                        policy,
                        ..Default::default()
                    },
                ))
                .expect_err("unrepresented neo effects must return no partial document");
            assert!(
                matches!(error, RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable { ref family, ref reason })
                if family == "timeline" && reason.contains("neo")),
                "{error}"
            );
        }
    }
}

#[test]
fn timeline_limits_stop_at_the_first_over_budget_item() {
    for direction in ["", " TD"] {
        let source = format!(
            "timeline{direction}\ntitle Releases\nsection Planning\nPlan : Build\nShip : Done\n"
        );
        for (id, actual, maximum, limits) in [
            (
                "max_commands",
                6,
                5,
                DrawingListLimits {
                    max_commands: 5,
                    ..Default::default()
                },
            ),
            (
                "max_text_bytes",
                8,
                3,
                DrawingListLimits {
                    max_text_bytes: 3,
                    ..Default::default()
                },
            ),
            (
                "max_stroke_dash_entries",
                2,
                1,
                DrawingListLimits {
                    max_stroke_dash_entries: 1,
                    ..Default::default()
                },
            ),
        ] {
            let error = Renderer::new()
                .render(RenderRequest::drawing_list(
                    &source,
                    OperationControl::new(),
                    DrawingListRequest {
                        limits,
                        ..Default::default()
                    },
                ))
                .expect_err("Timeline must reject the first over-budget item without output");
            let RenderError::ResourceLimitExceeded(limit) = error else {
                panic!("expected resource limit {id}, got {error}");
            };
            assert_eq!(limit.id, id);
            assert_eq!(limit.phase, "drawing-list-validation");
            assert_eq!(
                limit.actual, actual,
                "{direction}: first over-budget item for {id}"
            );
            assert_eq!(limit.maximum, maximum);
        }
    }
}

#[test]
fn timeline_emits_sections_tasks_events_connectors_and_title() {
    let source = r#"timeline
        title Release history
        section Planning
            Plan : Build
            Ship : Done
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Timeline should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "timeline.section.0.node.background",
        "timeline.section.0.node.divider",
        "timeline.task.0.node.background",
        "timeline.task.0.event.0.node.background",
        "timeline.task.0.connector.0.line",
        "timeline.task.0.connector.0.arrowhead",
        "timeline.activity.line",
        "timeline.activity.arrowhead",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    for label in [
        "Release history",
        "Planning",
        "Plan",
        "Build",
        "Ship",
        "Done",
    ] {
        assert!(
            document.commands.iter().any(|command| matches!(
                command,
                merman_display_list::DrawingCommand::DrawText { run } if run.text == label
            )),
            "missing text {label}"
        );
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.id == "timeline.section.0"
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.id == "timeline.task.0.event.0"
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id == "timeline.task.0.connector.0"
    }));
    assert!(document.fallbacks.is_empty());
}

#[test]
fn vertical_timeline_emits_explicit_arrowhead_geometry() {
    let source = r#"timeline TD
        title Release history
        section Planning
            Plan : Build
            Ship : Done
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("vertical Timeline should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    for path_id in [
        "timeline.activity.arrowhead",
        "timeline.task.0.connector.0.arrowhead",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing vertical Timeline arrowhead {path_id}"
        );
    }
    assert!(document.fallbacks.is_empty());
}

#[test]
fn journey_limits_stop_at_the_first_over_budget_item() {
    let source = "journey\nsection Checkout\nSign Up: 5: Alice\nPay: 3: Bob\n";
    for (id, actual, maximum, limits) in [
        (
            "max_commands",
            6,
            5,
            DrawingListLimits {
                max_commands: 5,
                ..Default::default()
            },
        ),
        (
            "max_text_bytes",
            5,
            3,
            DrawingListLimits {
                max_text_bytes: 3,
                ..Default::default()
            },
        ),
        (
            "max_stroke_dash_entries",
            2,
            1,
            DrawingListLimits {
                max_stroke_dash_entries: 1,
                ..Default::default()
            },
        ),
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest {
                    limits,
                    ..Default::default()
                },
            ))
            .expect_err("Journey must reject the first over-budget item without output");
        let RenderError::ResourceLimitExceeded(limit) = error else {
            panic!("expected resource limit {id}, got {error}");
        };
        assert_eq!(limit.id, id);
        assert_eq!(limit.phase, "drawing-list-validation");
        assert_eq!(limit.actual, actual, "first over-budget item for {id}");
        assert_eq!(limit.maximum, maximum);
    }
}

#[test]
fn journey_text_placement_preserves_literal_text_fonts_and_final_colors() {
    use merman_display_list::{Color, TextBaseline};
    for (placement, size, labels, task_color, section_color) in [
        (
            "fo",
            18.0,
            vec!["Task<br>Next"],
            Color::rgba(18, 52, 86, 255),
            Color::rgba(18, 52, 86, 255),
        ),
        (
            "old",
            18.0,
            vec!["Task<br>Next"],
            Color::rgba(18, 52, 86, 255),
            Color::rgba(236, 236, 255, 255),
        ),
        (
            "tspan",
            22.0,
            vec!["Task", "Next"],
            Color::rgba(255, 255, 255, 255),
            Color::rgba(236, 236, 255, 255),
        ),
    ] {
        let config = MermaidConfig::from_value(serde_json::json!({
            "themeVariables": {"textColor":"#123456", "fontFamily":"Arial", "fontSize":"18px"},
            "journey": {"textPlacement": placement, "taskFontFamily":"Courier", "taskFontSize":22}
        }));
        let source = "journey\nsection Heading<br>Two\nTask<br>Next: 5: Alice\n";
        let output = Renderer::new()
            .with_engine(Engine::new().with_site_config(config))
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect("Journey text placement renders");
        let RenderOutput::DrawingList(Some(output)) = output else {
            panic!("expected DrawingList")
        };
        let document = output.document();
        assert_eq!(
            document
                .commands
                .iter()
                .filter(|command| matches!(command, DrawingCommand::ClipPath { .. }))
                .count(),
            if placement == "fo" { 2 } else { 0 }
        );
        let mut scopes = Vec::new();
        let mut task_runs = Vec::new();
        let mut section_runs = Vec::new();
        for command in &document.commands {
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id } => {
                    scopes.push(semantic_id.as_str())
                }
                DrawingCommand::EndSemanticGroup => {
                    scopes.pop();
                }
                DrawingCommand::DrawText { run } if scopes.last() == Some(&"journey.task.0") => {
                    task_runs.push(run)
                }
                DrawingCommand::DrawText { run } if scopes.last() == Some(&"journey.section.0") => {
                    section_runs.push(run)
                }
                _ => {}
            }
        }
        assert_eq!(
            task_runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>(),
            labels,
            "{placement}"
        );
        for run in &task_runs {
            assert_eq!(run.style.font_size, size, "{placement}");
            assert_eq!(
                run.style.font.families,
                [if placement == "tspan" {
                    "Courier"
                } else {
                    "Arial"
                }]
            );
            assert_eq!(run.style.fill, Paint::solid(task_color));
            assert_eq!(
                run.baseline,
                if placement == "tspan" {
                    TextBaseline::Central
                } else {
                    TextBaseline::Alphabetic
                }
            );
        }
        assert!(!section_runs.is_empty());
        for run in section_runs {
            assert_eq!(run.style.fill, Paint::solid(section_color), "{placement}");
        }
        let title = &document
            .semantics
            .iter()
            .find(|item| item.id == "journey.task.0")
            .unwrap()
            .title;
        assert_eq!(
            title.as_deref(),
            Some(if placement == "tspan" {
                "Task\nNext"
            } else {
                "Task<br>Next"
            })
        );
        if placement == "tspan" {
            assert_eq!(task_runs[1].origin.y - task_runs[0].origin.y, 22.0);
        }
    }
}

#[test]
fn journey_table_wraps_plain_text_and_names_the_resolved_lines() {
    let source = r#"%%{init: {"journey":{"width":50}}}%%
journey
section Heading
Alpha     Beta     Gamma: 5: Alice
"#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Journey table text renders");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected DrawingList")
    };
    let document = output.document();
    let title = document
        .semantics
        .iter()
        .find(|item| item.id == "journey.task.0")
        .unwrap()
        .title
        .as_deref()
        .unwrap();
    assert_eq!(title, "Alpha\nBeta\nGamma");
    let lines = document
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawingCommand::DrawText { run }
                if ["Alpha", "Beta", "Gamma"].contains(&run.text.as_str()) =>
            {
                Some(run)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    for pair in lines.windows(2) {
        assert_eq!(
            pair[1].bounds.y,
            pair[0].bounds.y + pair[0].style.line_height
        );
    }
}

#[test]
fn journey_table_clips_overflow_without_truncating_text() {
    use merman_display_list::{DrawingResource, PathSegment};
    for text in [
        "UnbreakableLongWordBeyondTheBox",
        "Alpha Beta Gamma Delta Epsilon",
    ] {
        let renderer = Renderer::new().with_engine(Engine::new().with_site_config(
            MermaidConfig::from_value(serde_json::json!({
                "journey": {"width":50, "height":10}
            })),
        ));
        let source = format!("journey\nsection Heading\n{text}: 5: Alice\n");
        let RenderOutput::DrawingList(Some(output)) = renderer
            .render(RenderRequest::drawing_list(
                &source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .unwrap()
        else {
            panic!("expected DrawingList")
        };
        let document = output.document();
        let clip_index = document.commands.iter().position(|command| matches!(command,
            DrawingCommand::ClipPath { path, .. } if path.as_str() == "journey.task.0.label.clip"
        )).unwrap();
        assert!(matches!(
            document.commands[clip_index - 1],
            DrawingCommand::Save
        ));
        let path = document
            .resources
            .iter()
            .find_map(|resource| match resource {
                DrawingResource::Path(path) if path.id.as_str() == "journey.task.0.label.clip" => {
                    Some(path)
                }
                _ => None,
            })
            .unwrap();
        let [
            PathSegment::MoveTo { to: a },
            PathSegment::LineTo { to: b },
            PathSegment::LineTo { to: c },
            PathSegment::LineTo { to: d },
            PathSegment::Close,
        ] = path.segments.as_slice()
        else {
            panic!("rectangular clip")
        };
        assert_eq!((b.x - a.x, c.y - b.y), (50.0, 10.0));
        assert_eq!((d.x, d.y), (a.x, c.y));
        let mut texts = Vec::new();
        let mut restored = false;
        for command in &document.commands[clip_index + 1..] {
            match command {
                DrawingCommand::DrawText { run } => texts.push(run),
                DrawingCommand::Restore => {
                    restored = true;
                    break;
                }
                _ => panic!("only label text belongs inside the clip"),
            }
        }
        assert!(restored);
        assert_eq!(
            texts
                .iter()
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            text
        );
        assert!(
            texts
                .iter()
                .any(|run| run.bounds.width > 50.0 || run.bounds.y + run.bounds.height > c.y)
        );
    }
}

#[test]
fn journey_emits_actors_sections_tasks_faces_and_activity_axis() {
    let source = r#"journey
        title User checkout
        section Checkout
            Sign Up: 5: Alice
            Pay: 3: Bob
            Review: 1: Alice
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Journey should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "journey.actor.0.circle",
        "journey.actor.1.circle",
        "journey.section.0.background",
        "journey.task.0.line",
        "journey.task.0.face",
        "journey.task.0.face.left_eye",
        "journey.task.0.face.right_eye",
        "journey.task.0.face.mouth",
        "journey.task.0.background",
        "journey.task.0.actor.0.circle",
        "journey.activity.line",
        "journey.activity.arrowhead",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    for label in [
        "User checkout",
        "Checkout",
        "Sign Up",
        "Pay",
        "Review",
        "Alice",
        "Bob",
    ] {
        assert!(
            document.commands.iter().any(|command| matches!(
                command,
                merman_display_list::DrawingCommand::DrawText { run } if run.text == label
            )),
            "missing text {label}"
        );
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.id == "journey.section.0"
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.id == "journey.task.1"
            && semantic.title.as_deref() == Some("Pay")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id == "journey.activity"
    }));
    assert!(document.fallbacks.is_empty());
}

#[test]
fn kanban_emits_sections_cards_metadata_and_ticket_semantics() {
    let source = r##"%%{init: {"kanban": {"ticketBaseUrl": "https://example.invalid/tickets/#TICKET#"}}}%%
kanban
  todo[Todo]
    task[Task]@{ ticket: K-1, assigned: "Ada", priority: "High" }
"##;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Kanban should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in ["kanban.section.0.background", "kanban.item.0.background"] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
        )));
    }
    for label in ["Todo", "Task", "K-1", "Ada"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == label
        )));
    }
    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == "kanban.item.0.priority"
    )));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.id == "kanban.section.0"
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Node
            && semantic.id == "kanban.item.0"
            && semantic.link.as_deref() == Some("https://example.invalid/tickets/K-1")
    }));
    assert!(document.fallbacks.is_empty());
}

#[test]
fn kanban_preserves_duplicate_ids_across_sections() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            "kanban\n  Todo\n    id3[First]\n  Done\n    id3[Second]\n",
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("duplicate Kanban ids in separate sections should remain renderable");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    for label in ["First", "Second"] {
        assert!(
            document.commands.iter().any(|command| matches!(
                command,
                merman_display_list::DrawingCommand::DrawText { run } if run.text == label
            )),
            "missing Kanban card label {label}"
        );
    }
    assert_eq!(
        document
            .semantics
            .iter()
            .filter(|semantic| semantic.role == merman_display_list::SemanticRole::Node)
            .count(),
        2,
        "both cards need independent semantic nodes even when their source ids repeat"
    );
}

#[test]
fn kanban_rejects_rich_markdown_and_icons_without_silent_loss() {
    for source in [
        "kanban\n  todo[**Todo**]\n    task[Task]\n",
        "kanban\n  todo[Todo]\n    task[Task]@{ icon: star }\n",
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect_err("unsupported Kanban presentation must stay explicit");
        assert!(
            error.to_string().contains("DrawingList") || error.to_string().contains("icon"),
            "unexpected error: {error}"
        );
    }
}

#[test]
fn sequence_emits_typed_participants_lifelines_signals_markers_and_labels() {
    let source = r#"sequenceDiagram
        accTitle: Authentication flow
        accDescr: A request is sent and acknowledged
        participant Client
        participant Server
        Client->>Server: request
        Server-->>Client: response
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Sequence should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.viewport.bounds.height > 0.0);
    for path_id in [
        "sequence.actor.0.top.shape",
        "sequence.actor.0.lifeline",
        "sequence.message.0.route",
        "sequence.message.0.marker.end",
        "sequence.message.1.route",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    for text in ["Client", "Server", "request", "response"] {
        assert!(
            document.commands.iter().any(|command| matches!(
                command,
                merman_display_list::DrawingCommand::DrawText { run } if run.text == text
            )),
            "missing text {text}"
        );
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Document
            && semantic.id == "sequence.document"
            && semantic.title.as_deref() == Some("Authentication flow")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id == "sequence.message.0"
            && semantic.title.as_deref() == Some("request")
    }));
    assert!(document.fallbacks.is_empty());
}

#[test]
fn sequence_rejects_rich_labels_and_non_classic_shapes_without_silent_loss() {
    for (index, source) in [
        "sequenceDiagram\n  participant A\n  participant B\n  A->>B: **bold**\n",
        "sequenceDiagram\n  actor A\n  participant B\n  A->>B: hello\n",
        "%%{init: {\"look\": \"neo\"}}%%\nsequenceDiagram\n  participant A\n  participant B\n  A->>B: hello\n",
    ]
    .into_iter()
    .enumerate()
    {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
        .expect_err("unsupported Sequence presentation must stay explicit");
        assert!(
            error.to_string().contains("DrawingList")
                || error.to_string().contains("classic")
                || error.to_string().contains("preserve"),
            "unexpected error for case {index}: {error}"
        );
    }
}

#[test]
fn sequence_emits_control_note_and_activation_semantics() {
    let source = r#"sequenceDiagram
        participant Client
        participant Server
        loop retry
            Client->>+Server: request
            Server-->>-Client: response
        end
        Note over Client,Server: observed
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Sequence control constructs should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    for path_id in [
        "sequence.control.0.frame",
        "sequence.control.0.label_box",
        "sequence.activation.0.shape",
        "sequence.note.6.shape",
    ] {
        assert!(
            document.resources.iter().any(|resource| matches!(
                resource,
                merman_display_list::DrawingResource::Path(path) if path.id.as_str() == path_id
            )),
            "missing path {path_id}"
        );
    }
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run } if run.text == "retry"
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawText { run } if run.text == "observed"
    )));
}

#[test]
fn block_emits_source_backed_shapes_routes_styles_and_semantics() {
    let source = r#"block
        columns 3
        A("Rounded")
        B(("Circle"))
        C[("Database")]
        A -- "next" --> B
        B --> C
        classDef hot fill:#ffe4e6,stroke:#be123c,stroke-width:3px,color:#881337
        class A hot
    "#;
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            source,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Block diagram should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    let rounded_semantic = document
        .semantics
        .iter()
        .find(|semantic| {
            semantic.role == merman_display_list::SemanticRole::Node
                && semantic.title.as_deref() == Some("Rounded")
        })
        .expect("styled Block node should have node semantics");
    let rounded_shape = format!("{}.shape", rounded_semantic.id);

    assert!(document.resources.iter().any(|resource| matches!(
        resource,
        merman_display_list::DrawingResource::Path(path)
            if path.id.as_str() == rounded_shape
    )));
    assert!(document.commands.iter().any(|command| matches!(
        command,
        merman_display_list::DrawingCommand::DrawPath { path, style }
            if path.as_str() == rounded_shape
                && style.fill == Some(merman_display_list::Paint::solid(
                    merman_display_list::Color::rgba(0xff, 0xe4, 0xe6, 0xff)
                ))
                && style.stroke.as_ref().is_some_and(|stroke| {
                    stroke.width == 3.0
                        && stroke.paint == merman_display_list::Paint::solid(
                            merman_display_list::Color::rgba(0xbe, 0x12, 0x3c, 0xff)
                        )
                })
    )));
    for text in ["Rounded", "Circle", "Database", "next"] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == text
        )));
    }
    let marker = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str().starts_with("block.edge.")
                    && path.id.as_str().ends_with(".marker.end") =>
            {
                Some(path)
            }
            _ => None,
        })
        .expect("Block point marker should be expanded into typed geometry");
    let [
        merman_display_list::PathSegment::MoveTo { to: base_top },
        merman_display_list::PathSegment::LineTo { to: tip },
        ..,
    ] = marker.segments.as_slice()
    else {
        panic!("Block point marker should start with its base and tip");
    };
    assert!(
        tip.x > base_top.x,
        "a left-to-right Block edge must expand an end marker that points right"
    );
    assert_eq!(
        document
            .semantics
            .iter()
            .filter(|semantic| semantic.role == merman_display_list::SemanticRole::Node)
            .count(),
        3
    );
    assert_eq!(
        document
            .semantics
            .iter()
            .filter(|semantic| semantic.role == merman_display_list::SemanticRole::Edge)
            .count(),
        2
    );
    assert!(document.fallbacks.is_empty());
}

#[test]
fn block_scopes_box_opacity_without_fading_the_label_or_double_charging_paint_alpha() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"block
                A["Alpha"]
                style A fill:#112233,stroke:#445566,color:#778899,opacity:0.5,fill-opacity:0.4,stroke-opacity:0.2
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Block opacity should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    let semantic_id = document
        .semantics
        .iter()
        .find(|semantic| semantic.title.as_deref() == Some("Alpha"))
        .map(|semantic| semantic.id.as_str())
        .expect("Block node semantics");
    let group_start = document
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::BeginSemanticGroup { semantic_id: actual }
                    if actual == semantic_id
            )
        })
        .expect("Block node semantic group");
    let group_end = document.commands[group_start + 1..]
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::EndSemanticGroup
            )
        })
        .map(|offset| group_start + 1 + offset)
        .expect("Block node semantic group end");
    let [
        merman_display_list::DrawingCommand::Save,
        merman_display_list::DrawingCommand::SetOpacity { opacity },
        merman_display_list::DrawingCommand::DrawPath { path, style },
        merman_display_list::DrawingCommand::Restore,
        merman_display_list::DrawingCommand::DrawText { run },
    ] = &document.commands[group_start + 1..group_end]
    else {
        panic!("Block box opacity must be scoped to shape drawing");
    };

    assert_eq!(*opacity, 0.5);
    assert_eq!(path.as_str(), format!("{semantic_id}.shape"));
    assert_eq!(
        style.fill,
        Some(merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0x11, 0x22, 0x33, 102)
        ))
    );
    assert_eq!(
        style.stroke.as_ref().map(|stroke| &stroke.paint),
        Some(&merman_display_list::Paint::solid(
            merman_display_list::Color::rgba(0x44, 0x55, 0x66, 51)
        ))
    );
    assert_eq!(run.text, "Alpha");
    assert_eq!(
        run.style.fill,
        merman_display_list::Paint::solid(merman_display_list::Color::rgba(0x77, 0x88, 0x99, 0xff))
    );
}

#[test]
fn block_resolves_shape_and_html_label_class_opacity_in_their_source_orders() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"block
                A["Alpha"]
                classDef first opacity:0.2
                classDef second opacity:0.5
                class A second
                class A first
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("Block class opacity should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let semantic_id = output
        .document()
        .semantics
        .iter()
        .find(|semantic| semantic.title.as_deref() == Some("Alpha"))
        .map(|semantic| semantic.id.as_str())
        .expect("Block node semantics");
    let group_start = output
        .document()
        .commands
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::BeginSemanticGroup { semantic_id: actual }
                    if actual == semantic_id
            )
        })
        .expect("Block node semantic group");
    let group_end = output.document().commands[group_start + 1..]
        .iter()
        .position(|command| {
            matches!(
                command,
                merman_display_list::DrawingCommand::EndSemanticGroup
            )
        })
        .map(|offset| group_start + 1 + offset)
        .expect("Block node semantic group end");
    let [
        merman_display_list::DrawingCommand::Save,
        merman_display_list::DrawingCommand::SetOpacity {
            opacity: shape_opacity,
        },
        merman_display_list::DrawingCommand::DrawPath { .. },
        merman_display_list::DrawingCommand::Restore,
        merman_display_list::DrawingCommand::Save,
        merman_display_list::DrawingCommand::SetOpacity {
            opacity: label_opacity,
        },
        merman_display_list::DrawingCommand::DrawText { run },
        merman_display_list::DrawingCommand::Restore,
    ] = &output.document().commands[group_start + 1..group_end]
    else {
        panic!("Block class opacity should preserve both CSS cascade scopes");
    };

    // Mermaid compiles shape styles in the node's assigned-class order, while equal-specificity
    // class CSS for the HTML label wins in class-definition order.
    assert_eq!(*shape_opacity, 0.2);
    assert_eq!(*label_opacity, 0.25);
    assert_eq!(run.text, "Alpha");
}

#[test]
fn block_display_none_retains_a_semantic_dom_anchor() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"block
                A["Hidden"]
                classDef hidden display:none
                class A hidden
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("hidden Block node should still produce a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    let semantic = document
        .semantics
        .iter()
        .find(|semantic| semantic.title.as_deref() == Some("Hidden"))
        .expect("display:none node must retain semantic metadata");
    assert!(document.commands.iter().any(|command| {
        matches!(
            command,
            merman_display_list::DrawingCommand::BeginSemanticGroup { semantic_id }
                if semantic_id == &semantic.id
        )
    }));
    assert!(!document.commands.iter().any(|command| {
        matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == "Hidden"
        )
    }));
}

#[test]
fn block_inline_background_color_does_not_replace_svg_fill() {
    let node_fill = |source: &str| {
        let output = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect("Block should render as a DrawingList");
        let RenderOutput::DrawingList(Some(output)) = output else {
            panic!("expected a DrawingList output");
        };
        output
            .document()
            .commands
            .iter()
            .find_map(|command| match command {
                merman_display_list::DrawingCommand::DrawPath { path, style }
                    if path.as_str() == "block.node.0.shape" =>
                {
                    style.fill.clone()
                }
                _ => None,
            })
            .expect("Block node fill")
    };

    let baseline = node_fill("block\n  A[\"Alpha\"]\n");
    let with_background = node_fill("block\n  A[\"Alpha\"]\n  style A background-color:#ff0000\n");
    assert_eq!(with_background, baseline);
}

#[test]
fn block_class_background_color_is_a_structured_unavailable_effect() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"block
                A["Alpha"]
                classDef highlighted background-color:#ff0000
                class A highlighted
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("an HTML label background must not be silently dropped");

    let RenderError::DrawingList(merman::svg::RenderError::DrawingListUnavailable {
        family,
        reason,
    }) = error
    else {
        panic!("expected a structured DrawingList-unavailable error, got {error}");
    };
    assert_eq!(family, "block");
    assert!(reason.contains("HTML label background"), "{reason}");
}

#[test]
fn block_rejects_unmapped_visual_effects_instead_of_dropping_them() {
    let error = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"block
                A["Styled"]
                classDef glow fill:#123456,filter:blur(2px)
                class A glow
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect_err("Block filters must remain an explicit capability boundary");
    assert!(error.to_string().contains("style property `filter`"));

    for (source, expected) in [
        (
            "block\n  A([\"Stadium\"])\n",
            "stadium rendered through RoughJS",
        ),
        (
            "block\n  A>\"Odd\"]\n",
            "odd shape rendered through RoughJS",
        ),
        (
            "%%{init: {\"look\": \"neo\"}}%%\nblock\n  A[\"Neo\"]\n",
            "neo output uses SVG drop-shadow filters",
        ),
    ] {
        let error = Renderer::new()
            .render(RenderRequest::drawing_list(
                source,
                OperationControl::new(),
                DrawingListRequest::default(),
            ))
            .expect_err("unmapped Block effects must fail before returning a document");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn swimlane_emits_lane_bands_flowchart_shapes_routes_and_semantics() {
    let output = Renderer::new()
        .render(RenderRequest::drawing_list(
            r#"swimlane-beta LR
                subgraph Customer
                    request[Request service]
                    receive[Receive update]
                end
                subgraph Support
                    triage{Triage request}
                    answer([Send answer])
                end
                request --> triage
                triage -->|Known issue| answer
                answer --> receive
            "#,
            OperationControl::new(),
            DrawingListRequest::default(),
        ))
        .expect("portable Swimlane should render as a DrawingList");
    let RenderOutput::DrawingList(Some(output)) = output else {
        panic!("expected a DrawingList output");
    };

    let document = output.document();
    for path_id in ["swimlane.group.0.body", "swimlane.group.0.title"] {
        assert!(document.resources.iter().any(|resource| matches!(
            resource,
            merman_display_list::DrawingResource::Path(path)
                if path.id.as_str() == path_id
        )));
    }
    for text in [
        "Customer",
        "Support",
        "Request service",
        "Triage request",
        "Known issue",
    ] {
        assert!(document.commands.iter().any(|command| matches!(
            command,
            merman_display_list::DrawingCommand::DrawText { run } if run.text == text
        )));
    }
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Group
            && semantic.title.as_deref() == Some("Customer")
    }));
    assert!(document.semantics.iter().any(|semantic| {
        semantic.role == merman_display_list::SemanticRole::Edge
            && semantic.id.starts_with("swimlane.edge.")
    }));
    assert!(document.viewport.bounds.width > 0.0);
    assert!(document.fallbacks.is_empty());
}
