//! Per-diagram SVG compare commands.
use crate::XtaskError;

mod architecture;
mod block;
mod c4;
mod class;
mod er;
mod flowchart;
mod gantt;
mod generic_stage_b;
mod gitgraph;
mod info;
mod journey;
mod kanban;
mod mindmap;
mod packet;
mod pie;
mod quadrantchart;
mod radar;
mod requirement;
mod sankey;
mod sequence;
mod state;
mod timeline;
mod treemap;
mod xychart;

pub(crate) use architecture::compare_architecture_svgs;
pub(crate) use block::compare_block_svgs;
pub(crate) use c4::compare_c4_svgs;
pub(crate) use class::compare_class_svgs;
pub(crate) use er::compare_er_svgs;
pub(crate) use flowchart::compare_flowchart_svgs;
pub(crate) use gantt::compare_gantt_svgs;
pub(crate) use generic_stage_b::{
    compare_eventmodeling_svgs, compare_ishikawa_svgs, compare_tree_view_svgs,
};
pub(crate) use gitgraph::compare_gitgraph_svgs;
pub(crate) use info::compare_info_svgs;
pub(crate) use journey::compare_journey_svgs;
pub(crate) use kanban::compare_kanban_svgs;
pub(crate) use mindmap::compare_mindmap_svgs;
pub(crate) use packet::compare_packet_svgs;
pub(crate) use pie::compare_pie_svgs;
pub(crate) use quadrantchart::compare_quadrantchart_svgs;
pub(crate) use radar::compare_radar_svgs;
pub(crate) use requirement::compare_requirement_svgs;
pub(crate) use sankey::compare_sankey_svgs;
pub(crate) use sequence::compare_sequence_svgs;
pub(crate) use state::compare_state_svgs;
pub(crate) use timeline::compare_timeline_svgs;
pub(crate) use treemap::compare_treemap_svgs;
pub(crate) use xychart::compare_xychart_svgs;

type DiagramCompareFn = fn(Vec<String>) -> Result<(), XtaskError>;

#[derive(Debug, Clone, Copy)]
struct DiagramCompareAdapter {
    diagram: &'static str,
    run: DiagramCompareFn,
}

const DIAGRAM_COMPARE_ADAPTERS: &[DiagramCompareAdapter] = &[
    DiagramCompareAdapter {
        diagram: "er",
        run: compare_er_svgs,
    },
    DiagramCompareAdapter {
        diagram: "flowchart",
        run: compare_flowchart_svgs,
    },
    DiagramCompareAdapter {
        diagram: "state",
        run: compare_state_svgs,
    },
    DiagramCompareAdapter {
        diagram: "class",
        run: compare_class_svgs,
    },
    DiagramCompareAdapter {
        diagram: "sequence",
        run: compare_sequence_svgs,
    },
    DiagramCompareAdapter {
        diagram: "info",
        run: compare_info_svgs,
    },
    DiagramCompareAdapter {
        diagram: "pie",
        run: compare_pie_svgs,
    },
    DiagramCompareAdapter {
        diagram: "sankey",
        run: compare_sankey_svgs,
    },
    DiagramCompareAdapter {
        diagram: "packet",
        run: compare_packet_svgs,
    },
    DiagramCompareAdapter {
        diagram: "timeline",
        run: compare_timeline_svgs,
    },
    DiagramCompareAdapter {
        diagram: "journey",
        run: compare_journey_svgs,
    },
    DiagramCompareAdapter {
        diagram: "kanban",
        run: compare_kanban_svgs,
    },
    DiagramCompareAdapter {
        diagram: "gitgraph",
        run: compare_gitgraph_svgs,
    },
    DiagramCompareAdapter {
        diagram: "gantt",
        run: compare_gantt_svgs,
    },
    DiagramCompareAdapter {
        diagram: "c4",
        run: compare_c4_svgs,
    },
    DiagramCompareAdapter {
        diagram: "block",
        run: compare_block_svgs,
    },
    DiagramCompareAdapter {
        diagram: "radar",
        run: compare_radar_svgs,
    },
    DiagramCompareAdapter {
        diagram: "requirement",
        run: compare_requirement_svgs,
    },
    DiagramCompareAdapter {
        diagram: "mindmap",
        run: compare_mindmap_svgs,
    },
    DiagramCompareAdapter {
        diagram: "architecture",
        run: compare_architecture_svgs,
    },
    DiagramCompareAdapter {
        diagram: "quadrantchart",
        run: compare_quadrantchart_svgs,
    },
    DiagramCompareAdapter {
        diagram: "treemap",
        run: compare_treemap_svgs,
    },
    DiagramCompareAdapter {
        diagram: "xychart",
        run: compare_xychart_svgs,
    },
    DiagramCompareAdapter {
        diagram: "treeView",
        run: compare_tree_view_svgs,
    },
    DiagramCompareAdapter {
        diagram: "ishikawa",
        run: compare_ishikawa_svgs,
    },
    DiagramCompareAdapter {
        diagram: "eventmodeling",
        run: compare_eventmodeling_svgs,
    },
];

pub(crate) fn compare_diagram_svgs(diagram: &str, args: Vec<String>) -> Result<(), XtaskError> {
    let Some(adapter) = diagram_compare_adapter(diagram) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "unexpected diagram: {diagram}"
        )));
    };

    (adapter.run)(args)
}

fn diagram_compare_adapter(diagram: &str) -> Option<&'static DiagramCompareAdapter> {
    DIAGRAM_COMPARE_ADAPTERS
        .iter()
        .find(|adapter| adapter.diagram == diagram)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_adapter_registry_covers_primary_svg_matrix() {
        for diagram in crate::cmd::primary_svg_matrix_diagrams() {
            assert!(
                diagram_compare_adapter(diagram).is_some(),
                "primary SVG matrix diagram {diagram} should have a compare adapter"
            );
        }
    }

    #[test]
    fn compare_adapter_registry_is_one_to_one() {
        let mut diagrams = std::collections::BTreeSet::new();
        for adapter in DIAGRAM_COMPARE_ADAPTERS {
            assert!(
                diagrams.insert(adapter.diagram),
                "duplicate compare adapter for {}",
                adapter.diagram
            );
        }
    }
}
