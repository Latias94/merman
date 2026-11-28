use crate::RenderCapabilityPolicy;
use merman_model::presentation::{PresentationConfig, PresentationDiagram, RenderSemanticModel};
use merman_model::{OperationControl, OperationControlResult, OperationPhase};

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-mindmap",
    feature = "diagram-state",
    feature = "diagram-block",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-usecase",
    feature = "diagram-treemap"
))]
use merman_render_core::mermaid_style::native_typography as declaration_scan;

/// Inspects the paired canonical declarations used by the selected family renderer.
pub fn unsupported_native_text_style(
    parsed: &PresentationDiagram,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    unsupported_native_text_style_with_config(parsed, &parsed.metadata().effective_config, control)
}

pub fn unsupported_native_text_style_with_config(
    parsed: &PresentationDiagram,
    #[allow(
        unused_variables,
        reason = "Only some diagram families consult render configuration."
    )]
    config: &PresentationConfig,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let control = control.for_phase(OperationPhase::Semantic);
    control.checkpoint()?;
    #[cfg(feature = "diagram-gantt")]
    if let RenderSemanticModel::Gantt(model) = parsed.model() {
        return crate::gantt::unsupported_native_font_size(model, config, &control);
    }
    #[cfg(feature = "diagram-git-graph")]
    if let RenderSemanticModel::GitGraph(model) = parsed.model() {
        return crate::gitgraph::unsupported_native_font_size(model, config, &control);
    }
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    if let RenderSemanticModel::Flowchart(model) = parsed.model() {
        let default_context = Default::default();
        let context = parsed
            .render_context()
            .flowchart_render_context()
            .unwrap_or(&default_context);
        #[cfg(feature = "diagram-flowchart")]
        if parsed.metadata().diagram_type != "swimlane"
            && config.get_str("layout") != Some("swimlane")
        {
            return merman_render_flow::typography::unsupported_source_style(
                model, context, config, &control,
            );
        }
        for (ordinal, subgraph) in model.subgraphs.iter().enumerate() {
            let (_, styles) = context.effective_subgraph_css(ordinal, subgraph);
            if let Some(property) = declaration_scan::style_lists([styles], &control)? {
                return Ok(Some(property));
            }
        }
    }
    unsupported_native_model_text_style(parsed.model(), &control)
}

fn unsupported_native_model_text_style(
    model: &RenderSemanticModel,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let control = control.for_phase(OperationPhase::Semantic);
    control.checkpoint()?;
    match model {
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => declaration_scan::style_lists(
            model
                .class_defs
                .values()
                .map(Vec::as_slice)
                .chain(model.nodes.iter().map(|node| node.styles.as_slice()))
                .chain(model.edges.iter().map(|edge| edge.style.as_slice()))
                .chain(model.edge_defaults.iter().map(|edge| edge.style.as_slice())),
            &control,
        ),
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => declaration_scan::style_lists(
            model.nodes.iter().map(|node| node.css_styles.as_slice()),
            &control,
        ),
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => {
            for node in &model.nodes {
                if let Some(property) =
                    declaration_scan::declarations([node.label_style.as_str()], &control)?
                {
                    return Ok(Some(property));
                }
            }
            declaration_scan::style_lists(
                model
                    .style_classes
                    .values()
                    .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
                    .chain(
                        model.states.values().flat_map(|state| {
                            [state.styles.as_slice(), state.text_styles.as_slice()]
                        }),
                    )
                    .chain(model.nodes.iter().flat_map(|node| {
                        [
                            node.css_styles.as_slice(),
                            node.css_compiled_styles.as_slice(),
                        ]
                    })),
                &control,
            )
        }
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => declaration_scan::style_lists(
            model
                .class_defs
                .values()
                .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
                .chain(
                    model
                        .blocks_flat
                        .iter()
                        .map(|block| block.styles.as_slice()),
                ),
            &control,
        ),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => {
            for (_, class) in model.classes() {
                control.checkpoint()?;
                for member in class.members.iter().chain(&class.methods) {
                    if let Some(property) =
                        declaration_scan::declarations([member.css_style.as_str()], &control)?
                    {
                        return Ok(Some(property));
                    }
                }
            }
            declaration_scan::style_lists(
                model
                    .model()
                    .style_classes
                    .values()
                    .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
                    .chain(
                        model
                            .classes()
                            .map(|(_, class)| class.node.styles.as_slice()),
                    ),
                &control,
            )
        }
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => declaration_scan::style_lists(
            model
                .classes
                .values()
                .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
                .chain(
                    model
                        .entities
                        .values()
                        .map(|entity| entity.css_styles.as_slice()),
                )
                .chain(
                    model
                        .subgraphs
                        .iter()
                        .map(|group| group.css_styles.as_slice()),
                ),
            &control,
        ),
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => {
            merman_render_graph::requirement::unsupported_text_style(model, &control)
        }
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => declaration_scan::style_lists(
            model
                .class_defs
                .iter()
                .map(|class| class.styles.as_slice())
                .chain(model.nodes.iter().map(|node| node.styles.as_slice()))
                .chain(model.boundaries.iter().map(|node| node.styles.as_slice()))
                .chain(
                    model
                        .relationships
                        .iter()
                        .map(|edge| edge.styles.as_slice()),
                )
                .chain(model.json_nodes.iter().map(|node| node.styles.as_slice())),
            &control,
        ),
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(model) => {
            let mut pending = vec![(None, &model.root)];
            let mut seen = vec![false; model.nodes.len()];
            while let Some((record_id, node)) = pending.pop() {
                control.checkpoint()?;
                let children = node.children.as_deref().unwrap_or_default();
                for &index in children.iter().rev() {
                    control.checkpoint()?;
                    if let Some(child) = model.nodes.get(index)
                        && !std::mem::replace(&mut seen[index], true)
                    {
                        pending.push((Some(index), child));
                    }
                }
                // The root branch's label and value are hidden by the Treemap writer.
                if record_id.is_none() && !children.is_empty() {
                    continue;
                }
                // A retained override, including an empty one, replaces the selected class.
                let styles = model.compiled_styles(record_id).or_else(|| {
                    node.class_selector
                        .as_deref()
                        .and_then(|selector| model.classes.get(selector))
                        .map(|class| class.styles.as_slice())
                });
                if let Some(styles) = styles
                    && let Some(property) = declaration_scan::style_lists([styles], &control)?
                {
                    return Ok(Some(property));
                }
            }
            Ok(None)
        }
        RenderSemanticModel::Error(_) | RenderSemanticModel::CustomJson(_) => Ok(None),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(_) => Ok(None),
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(_) => Ok(None),
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(_) => Ok(None),
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(_) => Ok(None),
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(_) => Ok(None),
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(_) => Ok(None),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(_) => Ok(None),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(_) => Ok(None),
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(_) => Ok(None),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(_) => Ok(None),
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(_) => Ok(None),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(_) => Ok(None),
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(_) => Ok(None),
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(_) => Ok(None),
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(_) => Ok(None),
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(_) => Ok(None),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(_) => Ok(None),
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(_) => Ok(None),
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(_) => Ok(None),
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(_) => Ok(None),
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(_) => Ok(None),
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(_) => Ok(None),
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(_) => Ok(None),
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => {
            declaration_scan::style_lists(model.authored.declaration_styles(), &control)
        }
        // Another consumer can widen model features without enabling this renderer's handler.
        // Family planning retains responsibility for rejecting an unavailable renderer.
        #[allow(unreachable_patterns)]
        _ => Ok(None),
    }
}

/// Inspects only the inline HTML labels that the selected family actually renders as HTML.
pub fn unsupported_native_html_style(
    parsed: &PresentationDiagram,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    unsupported_native_html_style_with_config(
        parsed,
        &parsed.metadata().effective_config,
        RenderCapabilityPolicy::unrestricted(),
        control,
    )
}

pub fn unsupported_native_html_style_with_config(
    parsed: &PresentationDiagram,
    #[allow(
        unused_variables,
        reason = "Only families with HTML labels consult render configuration."
    )]
    config: &PresentationConfig,
    #[allow(
        unused_variables,
        reason = "ER relationship labels depend on Grid capability admission."
    )]
    capability_policy: RenderCapabilityPolicy,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let control = control.for_phase(OperationPhase::Semantic);
    control.checkpoint()?;
    match parsed.model() {
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => {
            let default_context = Default::default();
            merman_render_flow::typography::unsupported_flowchart_html_style(
                model,
                parsed
                    .render_context()
                    .flowchart_render_context()
                    .unwrap_or(&default_context),
                config,
                config.get_str("layout") == Some("swimlane"),
                &control,
            )
        }
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => {
            merman_render_graph::mindmap::unsupported_html_style(model, config, &control)
        }
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => {
            let grid_available = crate::layout_grid_available()
                && capability_policy.allows(crate::RenderCapability::LayoutGrid);
            merman_render_graph::er::unsupported_html_style(model, config, grid_available, &control)
        }
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => {
            merman_render_graph::requirement::unsupported_html_style(model, config, &control)
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => {
            merman_render_flow::typography::unsupported_agentflow_html_style(
                model, config, &control,
            )
        }
        _ => Ok(None),
    }
}

#[cfg(all(test, feature = "diagram-treemap"))]
mod treemap_tests {
    use super::*;
    use merman_model::families::treemap::{
        TreemapClassDef, TreemapDiagramModel, TreemapDiagramRenderModel, TreemapNode,
    };

    #[test]
    fn treemap_native_admission_uses_selected_retained_styles() {
        let semantic = TreemapDiagramModel {
            root: TreemapNode {
                children: Some(vec![0]),
                ..Default::default()
            },
            nodes: vec![
                TreemapNode {
                    name: "Leaf".into(),
                    value: Some(serde_json::json!(42).into()),
                    class_selector: Some("spaced".into()),
                    ..Default::default()
                },
                TreemapNode {
                    name: "Unreachable".into(),
                    class_selector: Some("spaced".into()),
                    ..Default::default()
                },
            ],
            classes: [(
                "spaced".into(),
                TreemapClassDef {
                    id: "spaced".into(),
                    styles: vec!["letter-spacing:4px".into()],
                    text_styles: Vec::new(),
                },
            )]
            .into(),
            ..Default::default()
        };
        for (retained, expected) in [
            (None, Some("letter-spacing")),
            (Some(Vec::new()), None),
            (Some(vec!["font-size:24px".into()]), None),
            (Some(vec!["word-spacing:4px".into()]), Some("word-spacing")),
        ] {
            let mut model = TreemapDiagramRenderModel::from_semantic_model(semantic.clone());
            model.visuals.node_styles = vec![retained];
            assert_eq!(
                unsupported_native_model_text_style(
                    &RenderSemanticModel::Treemap(model),
                    &OperationControl::new(),
                )
                .unwrap()
                .as_deref(),
                expected,
            );
        }
    }
}

#[cfg(all(test, feature = "diagram-block"))]
mod tests {
    use super::*;
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::BuiltinRenderSemantic;

    #[test]
    fn native_admission_cancels_among_flat_records_without_declarations() {
        let mut model = merman_model::families::block::BlockDiagramRenderModel::default();
        model.blocks_flat.resize_with(4_096, Default::default);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(8);
        let cancelled =
            unsupported_native_model_text_style(&RenderSemanticModel::Block(model), &control)
                .unwrap_err();
        assert_eq!(cancelled.phase, OperationPhase::Semantic);
    }

    #[test]
    fn native_admission_does_not_expand_the_block_compatibility_tree() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync("block-beta\nA\n", ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let (metadata, RenderSemanticModel::Block(mut model), _) = parsed.into_parts() else {
            panic!("Block model");
        };
        model.blocks_flat[0].children.push(usize::MAX);
        assert!(model.compatibility_json(&metadata).is_err());
        assert_eq!(
            unsupported_native_model_text_style(
                &RenderSemanticModel::Block(model),
                &OperationControl::new()
            )
            .unwrap(),
            None
        );
    }
}
