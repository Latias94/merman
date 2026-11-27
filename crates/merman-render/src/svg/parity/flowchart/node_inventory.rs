//! Project facade layout contracts into the Flow node inventory.

use merman_render_flow::node_inventory::FlowchartNodeLayoutView;
#[cfg(test)]
pub(super) use merman_render_flow::node_inventory::FlowchartPreparedNodes;

use crate::model::FlowchartLayout;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use crate::model::SwimlaneLayout;

impl FlowchartLayout {
    pub(crate) fn node_layout_view(&self) -> FlowchartNodeLayoutView<'_> {
        FlowchartNodeLayoutView::Flowchart {
            nodes: &self.nodes,
            clusters: &self.clusters,
            dom_node_order_by_root: &self.dom_node_order_by_root,
            edge_owners: &self.edge_owners,
            solved_parents: self.route_contract.group_parents(),
            dom: super::svg_layout(self).route_contract.dom(),
            uses_projected_edges: self.route_contract.uses_projected_edges(),
            canonical: super::svg_layout(self).route_contract.is_canonical_axis(),
        }
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl SwimlaneLayout {
    pub(crate) fn node_layout_view(&self) -> FlowchartNodeLayoutView<'_> {
        FlowchartNodeLayoutView::Swimlane {
            nodes: &self.nodes,
            lanes: &self.lanes,
            edge_owners: &self.edge_owners,
            direction: self.direction,
            canonical: self.has_canonical_routes(),
        }
    }
}

#[cfg(all(test, feature = "diagram-flowchart", feature = "layout-dagre"))]
mod source_typography_tests {
    use super::FlowchartPreparedNodes;

    #[test]
    fn cluster_terminal_uses_first_source_owner_and_releases_retained_paint() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n---\nflowchart LR\nclassDef first fill:#112233,font-size:19px\nclassDef last fill:#aabbcc,font-size:27px\nsubgraph Shared[First]\nA\nend\nclass Shared first\nsubgraph Shared[Last]\nB\nend\n",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap().into_presentation();
        let render_context = parsed
            .render_context()
            .flowchart_render_context()
            .unwrap()
            .clone();
        let (metadata, semantic, _) = parsed.into_parts();
        let merman_model::presentation::RenderSemanticModel::Flowchart(model) = semantic else {
            panic!("Flowchart model");
        };
        assert_eq!(model.subgraphs.len(), 1);
        assert_eq!(model.subgraphs[0].title, "First");
        assert_eq!(
            render_context
                .effective_subgraph_css(0, &model.subgraphs[0])
                .0,
            ["first"]
        );
        let work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let edge_style = crate::test_support::prepare_flowchart_edge_style_plan(
            &model,
            &metadata.effective_config,
            false,
            &work,
        )
        .unwrap();
        let layout = crate::flowchart::dagre::layout(
            &model,
            &render_context,
            &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
            &metadata.effective_config,
            &crate::text::DeterministicTextMeasurer::default(),
            None,
            None,
            &edge_style,
            work.clone(),
        )
        .unwrap();
        let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            &metadata.effective_config,
            None,
            None,
        );
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let render_config = super::super::render_config::prepare_flowchart_render_config(
            &model,
            &metadata.effective_config,
            &theme.compatibility,
            layout.route_contract.is_elk_parity(),
            sidecar.base_typography(),
        );
        let before = work.prepared_text_retained_bytes();
        let prepared = FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
            layout.node_layout_view(),
            &sidecar,
            &metadata.effective_config,
            &theme,
            &render_config,
            &work,
            true,
        )
        .unwrap();
        assert_eq!(prepared.cluster_ids().collect::<Vec<_>>(), ["Shared"]);
        let terminal = prepared.cluster("Shared").unwrap();
        assert!(terminal.compiled.node_style.contains("#112233"));
        assert!(!terminal.compiled.node_style.contains("#aabbcc"));
        assert_eq!(terminal.title_text_style.font_size, 19.0);
        assert!(work.prepared_text_retained_bytes() > before);
        drop(prepared);
        assert_eq!(work.prepared_text_retained_bytes(), before);

        #[cfg(feature = "layout-elk")]
        {
            let mut layout = layout;
            layout.route_contract = crate::graph_layout::RouteContract::ElkParity;
            layout
                .dom_node_order_by_root
                .insert(String::new(), vec!["Shared".into(), "Shared".into()]);
            let error = FlowchartPreparedNodes::prepare(
                &model,
                &render_context,
                &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
                layout.node_layout_view(),
                &sidecar,
                &metadata.effective_config,
                &theme,
                &render_config,
                &work,
                true,
            )
            .unwrap_err();
            assert!(
                matches!(error, crate::Error::InvalidModel { message } if message.contains("cluster `Shared` more than once"))
            );
        }
    }

    #[test]
    fn disabled_svg_label_preparation_still_enforces_retained_font_limit() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[Label]\nstyle A font-family:CallerFont,font-size:23px",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let render_context = parsed
            .render_context()
            .flowchart_render_context()
            .unwrap()
            .clone();
        let (metadata, semantic, _) = parsed.into_parts();
        let merman_model::presentation::RenderSemanticModel::Flowchart(model) = semantic else {
            panic!("Flowchart model");
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let layout_work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let edge_style = crate::test_support::prepare_flowchart_edge_style_plan(
            &model,
            &metadata.effective_config,
            false,
            &layout_work,
        )
        .unwrap();
        let layout = crate::flowchart::dagre::layout(
            &model,
            &render_context,
            &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
            &metadata.effective_config,
            &measurer,
            None,
            None,
            &edge_style,
            layout_work,
        )
        .unwrap();
        let work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                .unwrap(),
        ));
        let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            &metadata.effective_config,
            None,
            None,
        );
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let render_config = super::super::render_config::prepare_flowchart_render_config(
            &model,
            &metadata.effective_config,
            &theme.compatibility,
            layout.route_contract.is_elk_parity(),
            sidecar.base_typography(),
        );
        let result = FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            &crate::flowchart::FlowchartClassDefinitions::new(&model.class_defs),
            layout.node_layout_view(),
            &sidecar,
            &metadata.effective_config,
            &theme,
            &render_config,
            &work,
            true,
        );
        assert!(
            matches!(result, Err(crate::Error::ResourceLimitExceeded(ref error))
            if error.limit == "max_prepared_text_retained_bytes")
        );
        assert_eq!(work.prepared_text_retained_bytes(), 0);
    }
}
