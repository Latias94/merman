use crate::model::{LayoutPoint, SwimlaneDirection, SwimlaneTitleRect};
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkingNodeKind {
    Group,
    Content,
    EdgeLabel,
    Dummy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProducerLaneRole {
    Pool,
    Lane,
}

#[derive(Debug, Clone)]
pub(super) struct WorkingNode {
    pub id: String,
    pub label: String,
    pub label_type: String,
    pub shape: String,
    pub producer_lane_role: Option<ProducerLaneRole>,
    pub kind: WorkingNodeKind,
    pub parent_id: Option<String>,
    pub top_lane_id: Option<String>,
    pub requested_dir: Option<String>,
    pub padding: f64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub label_width: f64,
    pub label_height: f64,
    pub layer: usize,
    pub order: usize,
    pub content_top: Option<f64>,
    pub title_rect: Option<SwimlaneTitleRect>,
}

impl WorkingNode {
    pub fn drawn_size(&self) -> (f64, f64) {
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        if let Some(shape) = crate::flowchart::bpmn::BpmnShape::resolve(&self.shape)
            && !matches!(
                shape,
                crate::flowchart::bpmn::BpmnShape::Activity
                    | crate::flowchart::bpmn::BpmnShape::Annotation
            )
        {
            let geometry = shape.geometry(
                crate::text::TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                },
                false,
                false,
                false,
            );
            return (geometry.drawn_width, geometry.drawn_height);
        }
        (self.width, self.height)
    }

    pub fn is_group(&self) -> bool {
        self.kind == WorkingNodeKind::Group
    }

    pub fn is_layout_node(&self) -> bool {
        matches!(self.kind, WorkingNodeKind::Content | WorkingNodeKind::Dummy)
    }
}

#[derive(Debug, Clone)]
pub(super) struct WorkingEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub reference_id: String,
    pub label_node_id: Option<String>,
    pub reversed_for_layout: bool,
    pub points: Vec<LayoutPoint>,
}

impl WorkingEdge {
    pub fn layout_key(&self) -> String {
        format!("{}:{}->{}", self.id, self.from, self.to)
    }
}

#[derive(Debug, Clone)]
pub(super) struct WorkingLayout {
    pub direction: SwimlaneDirection,
    pub nodes: IndexMap<String, WorkingNode>,
    pub graph_edges: Vec<WorkingEdge>,
    pub original_edges: Vec<WorkingEdge>,
    pub top_lane_order: Vec<String>,
}

impl WorkingLayout {
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub fn refresh_top_lane_ids(&mut self) {
        let lanes = super::lanes::LaneModel::new(&self.nodes);
        for node in self.nodes.values_mut() {
            node.top_lane_id = lanes.lane_of(&node.id).map(str::to_string);
        }
    }

    pub fn top_lane_of(&self, id: &str) -> Option<&str> {
        self.nodes
            .get(id)
            .and_then(|node| node.top_lane_id.as_deref())
    }

    pub fn children_of<'a>(&'a self, parent_id: &'a str) -> impl Iterator<Item = &'a WorkingNode> {
        self.nodes
            .values()
            .filter(move |node| node.parent_id.as_deref() == Some(parent_id))
    }
}
