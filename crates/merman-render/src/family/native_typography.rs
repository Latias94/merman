use merman_core::{
    OperationControl, OperationControlResult, OperationPhase, ParsedDiagramRender,
    RenderSemanticModel,
};

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-mindmap",
    feature = "diagram-state",
    feature = "diagram-block",
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-requirement",
    feature = "diagram-usecase",
    feature = "diagram-treemap",
))]
mod declaration_scan {
    use super::{OperationControl, OperationControlResult};

    pub(super) const UNSUPPORTED_NATIVE_TEXT_STYLES: &[&str] = &[
        "letter-spacing",
        "word-spacing",
        "text-transform",
        "white-space",
        "font",
        "font-variant",
        "font-stretch",
        "font-feature-settings",
        "font-variation-settings",
        "font-kerning",
        "line-height",
        "text-indent",
        "writing-mode",
        "direction",
        "unicode-bidi",
        "text-anchor",
        "dominant-baseline",
        "alignment-baseline",
        "baseline-shift",
        "kerning",
        "text-rendering",
        "font-size-adjust",
    ];

    fn declaration<'a>(
        raw: &'a str,
        control: &OperationControl,
    ) -> OperationControlResult<Option<&'a str>> {
        control.checkpoint()?;
        let mut property_start = None;
        let mut property_end = 0;
        let mut in_value = false;
        let mut next_checkpoint = 1024;
        for (index, ch) in raw.char_indices() {
            if index >= next_checkpoint {
                control.checkpoint()?;
                next_checkpoint = index.saturating_add(1024);
            }
            if ch == ';' {
                property_start = None;
                in_value = false;
            } else if !in_value && ch == ':' {
                if let Some(start) = property_start {
                    let property = &raw[start..property_end];
                    if UNSUPPORTED_NATIVE_TEXT_STYLES
                        .iter()
                        .any(|name| property.eq_ignore_ascii_case(name))
                    {
                        return Ok(Some(property));
                    }
                }
                in_value = true;
            } else if !in_value && !ch.is_whitespace() {
                property_start.get_or_insert(index);
                property_end = index + ch.len_utf8();
            }
        }
        control.checkpoint()?;
        Ok(None)
    }

    pub(super) fn declarations<'a>(
        styles: impl IntoIterator<Item = &'a str>,
        control: &OperationControl,
    ) -> OperationControlResult<Option<String>> {
        for raw in styles {
            if let Some(property) = declaration(raw, control)? {
                return Ok(Some(property.to_owned()));
            }
        }
        Ok(None)
    }

    pub(super) fn style_lists<'a>(
        lists: impl IntoIterator<Item = &'a [String]>,
        control: &OperationControl,
    ) -> OperationControlResult<Option<String>> {
        for styles in lists {
            control.checkpoint()?;
            if let Some(property) = declarations(styles.iter().map(String::as_str), control)? {
                return Ok(Some(property));
            }
        }
        Ok(None)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use merman_core::OperationPhase;

        #[test]
        fn declarations_preserve_property_case_and_ignore_values() {
            let control = OperationControl::new();
            assert_eq!(
                declaration("fill:letter-spacing:4px; stroke:red", &control).unwrap(),
                None
            );
            assert_eq!(
                declaration("fill:red; \u{2003}Word-Spacing\u{2003}:2px", &control).unwrap(),
                Some("Word-Spacing")
            );
        }

        #[test]
        fn native_admission_cancels_during_one_large_declaration() {
            let raw = format!("fill:{};word-spacing:4px", "a".repeat(64 * 1024));
            let control = OperationControl::new();
            control.cancel_after_checkpoints(8);
            let cancelled =
                declaration(&raw, &control.for_phase(OperationPhase::Semantic)).unwrap_err();
            assert_eq!(cancelled.phase, OperationPhase::Semantic);
        }
    }
}

/// Inspects the paired canonical declarations used by the selected family renderer.
pub fn unsupported_native_text_style(
    parsed: &ParsedDiagramRender,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let control = control.for_phase(OperationPhase::Semantic);
    control.checkpoint()?;
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    if let RenderSemanticModel::Flowchart(model) = parsed.model() {
        let default_context = Default::default();
        let context = parsed
            .flowchart_render_context()
            .unwrap_or(&default_context);
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
            if let Some(property) = declaration_scan::declarations(
                model.nodes.iter().map(|node| node.label_style.as_str()),
                &control,
            )? {
                return Ok(Some(property));
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
            for class in model.classes.values() {
                control.checkpoint()?;
                if let Some(property) = declaration_scan::declarations(
                    class
                        .members
                        .iter()
                        .chain(&class.methods)
                        .map(|member| member.css_style.as_str()),
                    &control,
                )? {
                    return Ok(Some(property));
                }
            }
            declaration_scan::style_lists(
                model
                    .style_classes
                    .values()
                    .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
                    .chain(model.classes.values().map(|class| class.styles.as_slice())),
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
        RenderSemanticModel::Requirement(model) => declaration_scan::style_lists(
            model
                .classes
                .values()
                .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()])
                .chain(
                    model
                        .requirements
                        .iter()
                        .map(|node| node.css_styles.as_slice()),
                )
                .chain(model.elements.iter().map(|node| node.css_styles.as_slice())),
            &control,
        ),
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
            if let Some(styles) = model.compiled_styles(None)
                && let Some(property) = declaration_scan::style_lists([styles], &control)?
            {
                return Ok(Some(property));
            }
            for index in 0..model.nodes.len() {
                control.checkpoint()?;
                if let Some(styles) = model.compiled_styles(Some(index))
                    && let Some(property) = declaration_scan::style_lists([styles], &control)?
                {
                    return Ok(Some(property));
                }
            }
            declaration_scan::style_lists(
                model
                    .classes
                    .values()
                    .flat_map(|class| [class.styles.as_slice(), class.text_styles.as_slice()]),
                &control,
            )
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
            declaration_scan::style_lists(model.presentation.declaration_styles(), &control)
        }
        // Another consumer can widen core features without enabling this renderer's handler.
        // Family planning retains responsibility for rejecting an unavailable renderer.
        #[cfg(not(feature = "all-diagrams"))]
        #[allow(unreachable_patterns)]
        _ => Ok(None),
    }
}

/// Inspects only the inline HTML labels that the selected family actually renders as HTML.
pub fn unsupported_native_html_style(
    parsed: &ParsedDiagramRender,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let control = control.for_phase(OperationPhase::Semantic);
    control.checkpoint()?;
    match parsed.model() {
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => {
            let default_context = Default::default();
            unsupported_native_flowchart_html_style(
                model,
                parsed
                    .flowchart_render_context()
                    .unwrap_or(&default_context),
                &parsed.metadata().effective_config,
                parsed.metadata().effective_config.get_str("layout") == Some("swimlane"),
                &control,
            )
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => {
            let config = &parsed.metadata().effective_config;
            let view = crate::flowchart::FlowchartConfigView::new(config.as_value());
            let labels = model.render_label_sources();
            let inspect = |label, kind: Option<&str>| {
                crate::text::unsupported_html_label_style(
                    label,
                    kind.unwrap_or("text"),
                    config,
                    declaration_scan::UNSUPPORTED_NATIVE_TEXT_STYLES,
                    &control,
                )
            };
            if view.node_html_labels() {
                for (label, kind) in labels.node_labels() {
                    control.checkpoint()?;
                    if let Some(property) = inspect(label, kind)? {
                        return Ok(Some(property));
                    }
                }
            }
            if view.effective_html_labels() {
                for label in labels
                    .edge_labels()
                    .chain(labels.subgraph_labels().map(Some))
                {
                    control.checkpoint()?;
                    if let Some((label, kind)) = label
                        && let Some(property) = inspect(label, kind)?
                    {
                        return Ok(Some(property));
                    }
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn unsupported_native_flowchart_html_style(
    model: &merman_core::diagrams::flowchart::FlowchartModel,
    context: &merman_core::diagrams::flowchart::FlowchartRenderContext,
    config: &merman_core::presentation::PresentationConfig,
    swimlane: bool,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let view = crate::flowchart::FlowchartConfigView::new(config.as_value());
    let edge_html = view.effective_html_labels();
    let inspect = |label: &str, label_type: Option<&str>| {
        crate::text::unsupported_html_label_style(
            label,
            label_type.unwrap_or("text"),
            config,
            declaration_scan::UNSUPPORTED_NATIVE_TEXT_STYLES,
            control,
        )
    };
    if view.node_html_labels() {
        for node in &model.nodes {
            control.checkpoint()?;
            if let Some(property) = inspect(
                context.node_label_for_render(node).unwrap_or(&node.id),
                node.label_type.as_deref(),
            )? {
                return Ok(Some(property));
            }
        }
    }
    if edge_html {
        for (semantic_index, edge) in model.edges.iter().enumerate() {
            control.checkpoint()?;
            if let Some(label) = context.edge_label_for_render(semantic_index, edge)
                && let Some(property) = inspect(label, edge.label_type.as_deref())?
            {
                return Ok(Some(property));
            }
        }
    }
    let swimlane_title_html = view.swimlane_title_html_labels();
    let mut subgraph_children = std::collections::HashSet::new();
    if swimlane {
        for subgraph in &model.subgraphs {
            control.checkpoint()?;
            for child in &subgraph.nodes {
                control.checkpoint()?;
                subgraph_children.insert(child.as_str());
            }
        }
    }
    for (ordinal, subgraph) in model.subgraphs.iter().enumerate() {
        control.checkpoint()?;
        let (title_html, label_type) =
            if swimlane && !subgraph_children.contains(subgraph.id.as_str()) {
                (swimlane_title_html, Some("markdown"))
            } else {
                (edge_html, subgraph.label_type.as_deref())
            };
        if title_html
            && let Some(property) = inspect(
                context.subgraph_title_for_render(ordinal, subgraph),
                label_type,
            )?
        {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

#[cfg(all(test, feature = "diagram-block"))]
mod tests {
    use super::*;
    use merman_core::{BuiltinRenderSemantic, Engine, ParseOptions};

    #[test]
    fn native_admission_cancels_among_flat_records_without_declarations() {
        let mut model = merman_core::diagrams::block::BlockDiagramRenderModel::default();
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
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("block-beta\nA\n", ParseOptions::strict())
            .unwrap()
            .unwrap();
        let (metadata, RenderSemanticModel::Block(mut model)) = parsed.into_parts() else {
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
