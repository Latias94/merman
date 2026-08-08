use std::fmt;

/// Stable identity for a built-in typed render family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderFamilyKind {
    Error,
    Mindmap,
    State,
    Sequence,
    Zenuml,
    Flowchart,
    Swimlane,
    Architecture,
    Class,
    C4,
    Cynefin,
    Wardley,
    Railroad,
    Kanban,
    Gantt,
    Pie,
    Packet,
    Timeline,
    Journey,
    Requirement,
    Sankey,
    Radar,
    Info,
    Treemap,
    Block,
    Er,
    QuadrantChart,
    XyChart,
    GitGraph,
    TreeView,
    Ishikawa,
    EventModeling,
    Venn,
}

impl RenderFamilyKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Mindmap => "mindmap",
            Self::State => "state",
            Self::Sequence => "sequence",
            Self::Zenuml => "zenuml",
            Self::Flowchart => "flowchart",
            Self::Swimlane => "swimlane",
            Self::Architecture => "architecture",
            Self::Class => "class",
            Self::C4 => "c4",
            Self::Cynefin => "cynefin",
            Self::Wardley => "wardley",
            Self::Railroad => "railroad",
            Self::Kanban => "kanban",
            Self::Gantt => "gantt",
            Self::Pie => "pie",
            Self::Packet => "packet",
            Self::Timeline => "timeline",
            Self::Journey => "journey",
            Self::Requirement => "requirement",
            Self::Sankey => "sankey",
            Self::Radar => "radar",
            Self::Info => "info",
            Self::Treemap => "treemap",
            Self::Block => "block",
            Self::Er => "er",
            Self::QuadrantChart => "quadrantChart",
            Self::XyChart => "xychart",
            Self::GitGraph => "gitGraph",
            Self::TreeView => "treeView",
            Self::Ishikawa => "ishikawa",
            Self::EventModeling => "eventmodeling",
            Self::Venn => "venn",
        }
    }
}

impl fmt::Display for RenderFamilyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
