use crate::{Error, ParseMetadata, Result};
use serde_json::Value;

pub type DiagramSemanticParser = fn(code: &str, meta: &ParseMetadata) -> Result<Value>;
pub type RenderSemanticParser = fn(code: &str, meta: &ParseMetadata) -> Result<RenderSemanticModel>;

#[derive(Debug, Clone, Default)]
pub struct DiagramRegistry {
    parsers: std::collections::HashMap<&'static str, DiagramSemanticParser>,
}

impl DiagramRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, diagram_type: &'static str, parser: DiagramSemanticParser) {
        self.parsers.insert(diagram_type, parser);
    }

    pub fn get(&self, diagram_type: &str) -> Option<DiagramSemanticParser> {
        self.parsers.get(diagram_type).copied()
    }

    pub fn for_pinned_mermaid_baseline() -> Self {
        let mut reg = Self::new();

        reg.insert("error", crate::diagrams::error_diagram::parse_error);

        reg.insert("flowchart-v2", crate::diagrams::flowchart::parse_flowchart);
        reg.insert("flowchart", crate::diagrams::flowchart::parse_flowchart);
        reg.insert("flowchart-elk", crate::diagrams::flowchart::parse_flowchart);

        reg.insert("info", crate::diagrams::info::parse_info);
        reg.insert("pie", crate::diagrams::pie::parse_pie);
        reg.insert("c4", crate::diagrams::c4::parse_c4);
        reg.insert(
            "requirement",
            crate::diagrams::requirement::parse_requirement,
        );
        reg.insert("sequence", crate::diagrams::sequence::parse_sequence);
        reg.insert("zenuml", crate::diagrams::zenuml::parse_zenuml);

        reg.insert("classDiagram", crate::diagrams::class::parse_class);
        reg.insert("class", crate::diagrams::class::parse_class);

        reg.insert("er", crate::diagrams::er::parse_er);
        reg.insert("erDiagram", crate::diagrams::er::parse_er);

        reg.insert("stateDiagram", crate::diagrams::state::parse_state);
        reg.insert("state", crate::diagrams::state::parse_state);

        reg.insert("mindmap", crate::diagrams::mindmap::parse_mindmap);
        reg.insert("gantt", crate::diagrams::gantt::parse_gantt);
        reg.insert("timeline", crate::diagrams::timeline::parse_timeline);
        reg.insert("journey", crate::diagrams::journey::parse_journey);
        reg.insert("kanban", crate::diagrams::kanban::parse_kanban);
        reg.insert(
            "architecture",
            crate::diagrams::architecture::parse_architecture,
        );
        reg.insert("block", crate::diagrams::block::parse_block);
        reg.insert("gitGraph", crate::diagrams::git_graph::parse_git_graph);
        reg.insert(
            "quadrantChart",
            crate::diagrams::quadrant_chart::parse_quadrant_chart,
        );
        reg.insert("packet", crate::diagrams::packet::parse_packet);
        reg.insert("radar", crate::diagrams::radar::parse_radar);
        reg.insert("treemap", crate::diagrams::treemap::parse_treemap);
        reg.insert("sankey", crate::diagrams::sankey::parse_sankey);
        reg.insert("xychart", crate::diagrams::xychart::parse_xychart);

        reg
    }

    #[deprecated(note = "use for_pinned_mermaid_baseline")]
    pub fn default_mermaid_11_12_2() -> Self {
        Self::for_pinned_mermaid_baseline()
    }
}

#[derive(Debug, Clone)]
pub struct ParsedDiagram {
    pub meta: ParseMetadata,
    pub model: Value,
}

#[derive(Debug, Clone)]
pub enum RenderSemanticModel {
    Json(Value),
    Mindmap(crate::diagrams::mindmap::MindmapDiagramRenderModel),
    State(crate::diagrams::state::StateDiagramRenderModel),
    Sequence(crate::diagrams::sequence::SequenceDiagramRenderModel),
    Flowchart(crate::diagrams::flowchart::FlowchartV2Model),
    Architecture(crate::diagrams::architecture::ArchitectureDiagramRenderModel),
    Class(crate::models::class_diagram::ClassDiagram),
    C4(crate::diagrams::c4::C4DiagramRenderModel),
    Kanban(crate::diagrams::kanban::KanbanDiagramRenderModel),
    Gantt(crate::diagrams::gantt::GanttDiagramRenderModel),
    Pie(crate::diagrams::pie::PieDiagramRenderModel),
    Packet(crate::diagrams::packet::PacketDiagramRenderModel),
    Timeline(crate::diagrams::timeline::TimelineDiagramRenderModel),
    Journey(crate::diagrams::journey::JourneyDiagramRenderModel),
    Requirement(crate::diagrams::requirement::RequirementDiagramRenderModel),
    Sankey(crate::diagrams::sankey::SankeyDiagramRenderModel),
    Radar(crate::diagrams::radar::RadarDiagramRenderModel),
    Info(crate::diagrams::info::InfoDiagramRenderModel),
    Treemap(crate::diagrams::treemap::TreemapDiagramRenderModel),
    Block(crate::diagrams::block::BlockDiagramRenderModel),
    Er(crate::diagrams::er::ErDiagramRenderModel),
    QuadrantChart(crate::diagrams::quadrant_chart::QuadrantChartRenderModel),
    XyChart(crate::diagrams::xychart::XyChartDiagramRenderModel),
    GitGraph(crate::diagrams::git_graph::GitGraphRenderModel),
}

impl RenderSemanticModel {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Json(_) => "json",
            Self::Mindmap(_) => "mindmap",
            Self::State(_) => "state",
            Self::Sequence(_) => "sequence",
            Self::Flowchart(_) => "flowchart",
            Self::Architecture(_) => "architecture",
            Self::Class(_) => "class",
            Self::C4(_) => "c4",
            Self::Kanban(_) => "kanban",
            Self::Gantt(_) => "gantt",
            Self::Pie(_) => "pie",
            Self::Packet(_) => "packet",
            Self::Timeline(_) => "timeline",
            Self::Journey(_) => "journey",
            Self::Requirement(_) => "requirement",
            Self::Sankey(_) => "sankey",
            Self::Radar(_) => "radar",
            Self::Info(_) => "info",
            Self::Treemap(_) => "treemap",
            Self::Block(_) => "block",
            Self::Er(_) => "er",
            Self::QuadrantChart(_) => "quadrantChart",
            Self::XyChart(_) => "xychart",
            Self::GitGraph(_) => "gitGraph",
        }
    }

    pub fn supports_diagram_type(&self, diagram_type: &str) -> bool {
        match self {
            Self::Json(_) => true,
            Self::Mindmap(_) => diagram_type == "mindmap",
            Self::State(_) => matches!(diagram_type, "stateDiagram" | "state"),
            Self::Sequence(_) => matches!(diagram_type, "sequence" | "zenuml"),
            Self::Flowchart(_) => {
                matches!(diagram_type, "flowchart-v2" | "flowchart" | "flowchart-elk")
            }
            Self::Architecture(_) => diagram_type == "architecture",
            Self::Class(_) => matches!(diagram_type, "classDiagram" | "class"),
            Self::C4(_) => diagram_type == "c4",
            Self::Kanban(_) => diagram_type == "kanban",
            Self::Gantt(_) => diagram_type == "gantt",
            Self::Pie(_) => diagram_type == "pie",
            Self::Packet(_) => diagram_type == "packet",
            Self::Timeline(_) => diagram_type == "timeline",
            Self::Journey(_) => diagram_type == "journey",
            Self::Requirement(_) => diagram_type == "requirement",
            Self::Sankey(_) => diagram_type == "sankey",
            Self::Radar(_) => diagram_type == "radar",
            Self::Info(_) => diagram_type == "info",
            Self::Treemap(_) => diagram_type == "treemap",
            Self::Block(_) => diagram_type == "block",
            Self::Er(_) => matches!(diagram_type, "er" | "erDiagram"),
            Self::QuadrantChart(_) => diagram_type == "quadrantChart",
            Self::XyChart(_) => diagram_type == "xychart",
            Self::GitGraph(_) => diagram_type == "gitGraph",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RenderDiagramRegistry {
    parsers: std::collections::HashMap<&'static str, RenderSemanticParser>,
}

impl RenderDiagramRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, diagram_type: &'static str, parser: RenderSemanticParser) {
        self.parsers.insert(diagram_type, parser);
    }

    pub fn get(&self, diagram_type: &str) -> Option<RenderSemanticParser> {
        self.parsers.get(diagram_type).copied()
    }

    pub fn for_pinned_mermaid_baseline() -> Self {
        let mut reg = Self::new();

        reg.insert("mindmap", |code, meta| {
            crate::diagrams::mindmap::parse_mindmap_model_for_render(code, meta)
                .map(RenderSemanticModel::Mindmap)
        });
        reg.insert("stateDiagram", |code, meta| {
            crate::diagrams::state::parse_state_model_for_render(code, meta)
                .map(RenderSemanticModel::State)
        });
        reg.insert("state", |code, meta| {
            crate::diagrams::state::parse_state_model_for_render(code, meta)
                .map(RenderSemanticModel::State)
        });
        reg.insert("zenuml", |code, meta| {
            crate::diagrams::zenuml::parse_zenuml_model_for_render(code, meta)
                .map(RenderSemanticModel::Sequence)
        });
        reg.insert("sequence", |code, meta| {
            crate::diagrams::sequence::parse_sequence_model_for_render(code, meta)
                .map(RenderSemanticModel::Sequence)
        });
        reg.insert("flowchart-v2", |code, meta| {
            crate::diagrams::flowchart::parse_flowchart_model_for_render(code, meta)
                .map(RenderSemanticModel::Flowchart)
        });
        reg.insert("flowchart", |code, meta| {
            crate::diagrams::flowchart::parse_flowchart_model_for_render(code, meta)
                .map(RenderSemanticModel::Flowchart)
        });
        reg.insert("flowchart-elk", |code, meta| {
            crate::diagrams::flowchart::parse_flowchart_model_for_render(code, meta)
                .map(RenderSemanticModel::Flowchart)
        });
        reg.insert("classDiagram", |code, meta| {
            crate::diagrams::class::parse_class_typed(code, meta).map(RenderSemanticModel::Class)
        });
        reg.insert("class", |code, meta| {
            crate::diagrams::class::parse_class_typed(code, meta).map(RenderSemanticModel::Class)
        });
        reg.insert("c4", |code, meta| {
            crate::diagrams::c4::parse_c4_model_for_render(code, meta).map(RenderSemanticModel::C4)
        });
        reg.insert("architecture", |code, meta| {
            crate::diagrams::architecture::parse_architecture_model_for_render(code, meta)
                .map(RenderSemanticModel::Architecture)
        });
        reg.insert("kanban", |code, meta| {
            crate::diagrams::kanban::parse_kanban_model_for_render(code, meta)
                .map(RenderSemanticModel::Kanban)
        });
        reg.insert("gantt", |code, meta| {
            crate::diagrams::gantt::parse_gantt_model_for_render(code, meta)
                .map(RenderSemanticModel::Gantt)
        });
        reg.insert("pie", |code, meta| {
            crate::diagrams::pie::parse_pie_model_for_render(code, meta)
                .map(RenderSemanticModel::Pie)
        });
        reg.insert("packet", |code, meta| {
            crate::diagrams::packet::parse_packet_model_for_render(code, meta)
                .map(RenderSemanticModel::Packet)
        });
        reg.insert("timeline", |code, meta| {
            crate::diagrams::timeline::parse_timeline_model_for_render(code, meta)
                .map(RenderSemanticModel::Timeline)
        });
        reg.insert("journey", |code, meta| {
            crate::diagrams::journey::parse_journey_model_for_render(code, meta)
                .map(RenderSemanticModel::Journey)
        });
        reg.insert("requirement", |code, meta| {
            crate::diagrams::requirement::parse_requirement_model_for_render(code, meta)
                .map(RenderSemanticModel::Requirement)
        });
        reg.insert("sankey", |code, meta| {
            crate::diagrams::sankey::parse_sankey_model_for_render(code, meta)
                .map(RenderSemanticModel::Sankey)
        });
        reg.insert("radar", |code, meta| {
            crate::diagrams::radar::parse_radar_model_for_render(code, meta)
                .map(RenderSemanticModel::Radar)
        });
        reg.insert("info", |code, meta| {
            crate::diagrams::info::parse_info_model_for_render(code, meta)
                .map(RenderSemanticModel::Info)
        });
        reg.insert("treemap", |code, meta| {
            crate::diagrams::treemap::parse_treemap_model_for_render(code, meta)
                .map(RenderSemanticModel::Treemap)
        });
        reg.insert("block", |code, meta| {
            crate::diagrams::block::parse_block_model_for_render(code, meta)
                .map(RenderSemanticModel::Block)
        });
        reg.insert("er", |code, meta| {
            crate::diagrams::er::parse_er_model_for_render(code, meta).map(RenderSemanticModel::Er)
        });
        reg.insert("erDiagram", |code, meta| {
            crate::diagrams::er::parse_er_model_for_render(code, meta).map(RenderSemanticModel::Er)
        });
        reg.insert("quadrantChart", |code, meta| {
            crate::diagrams::quadrant_chart::parse_quadrant_chart_model_for_render(code, meta)
                .map(RenderSemanticModel::QuadrantChart)
        });
        reg.insert("xychart", |code, meta| {
            crate::diagrams::xychart::parse_xychart_model_for_render(code, meta)
                .map(RenderSemanticModel::XyChart)
        });
        reg.insert("gitGraph", |code, meta| {
            crate::diagrams::git_graph::parse_git_graph_model_for_render(code, meta)
                .map(RenderSemanticModel::GitGraph)
        });

        reg
    }

    #[deprecated(note = "use for_pinned_mermaid_baseline")]
    pub fn default_mermaid_11_12_2() -> Self {
        Self::for_pinned_mermaid_baseline()
    }
}

#[derive(Debug, Clone)]
pub struct ParsedDiagramRender {
    pub meta: ParseMetadata,
    pub model: RenderSemanticModel,
}

pub fn parse_or_unsupported(
    registry: &DiagramRegistry,
    diagram_type: &str,
    code: &str,
    meta: &ParseMetadata,
) -> Result<Value> {
    let Some(parser) = registry.get(diagram_type) else {
        return Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        });
    };
    parser(code, meta)
}
