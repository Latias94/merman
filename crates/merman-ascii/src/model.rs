//! Borrowed inputs for the terminal backend, independent of graphical preparation.

use crate::Result;
use crate::operation::AsciiExecution;
use merman_model::resources::ModelComplexity;
use merman_model::{MermaidConfig, SemanticDiagramModel, SemanticMetadata};
use serde::{Serialize, Serializer};

/// Declared identity and configuration used by terminal interpretation and fallback.
///
/// Independent model consumers own these values. Source workflows pass their admitted metadata;
/// constructing this view does not certify that source parsing or configuration admission ran.
#[derive(Debug, Clone, Copy)]
pub struct AsciiSourceContext<'a> {
    pub diagram_type: &'a str,
    /// Caller-declared authored configuration retained by complete fallback.
    pub config: &'a MermaidConfig,
    /// Caller-declared effective semantic policy, before graphical theme materialization.
    pub semantic_config: &'a MermaidConfig,
    pub title: Option<&'a str>,
}

impl<'a> From<&'a SemanticMetadata> for AsciiSourceContext<'a> {
    fn from(metadata: &'a SemanticMetadata) -> Self {
        Self {
            diagram_type: &metadata.diagram_type,
            config: &metadata.config,
            semantic_config: &metadata.semantic_config,
            title: metadata.title.as_deref(),
        }
    }
}

/// Borrows one family's canonical or explicit full data for terminal output.
///
/// Canonical parsing and explicit full hosts feed the same family adapters. The
/// Class variant owns only its derived topology when constructed from canonical
/// data; its authored model remains borrowed. Other variants never own a model.
#[derive(Debug)]
#[non_exhaustive]
pub enum AsciiDiagram<'a> {
    #[cfg(feature = "diagram-class")]
    Class(Box<merman_model::families::class::ClassTopologyView<'a>>),
    #[cfg(feature = "diagram-er")]
    Er(&'a merman_model::families::er::ErDiagramModel),
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    Flowchart(&'a merman_model::families::flowchart::FlowchartModel),
    #[cfg(feature = "diagram-gantt")]
    Gantt(merman_model::families::gantt::GanttTerminalView<'a>),
    #[cfg(feature = "diagram-git-graph")]
    GitGraph(&'a merman_model::families::git_graph::GitGraphDiagramModel),
    #[cfg(feature = "diagram-journey")]
    Journey(&'a merman_model::families::journey::JourneyDiagramModel),
    #[cfg(feature = "diagram-kanban")]
    Kanban(merman_model::families::kanban::KanbanTerminalView<'a>),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap(merman_model::families::mindmap::MindmapTerminalView<'a>),
    #[cfg(feature = "diagram-packet")]
    Packet(&'a merman_model::families::packet::PacketDiagramModel),
    #[cfg(feature = "diagram-sequence")]
    Sequence(&'a merman_model::families::sequence::SequenceDiagramModel),
    #[cfg(feature = "diagram-state")]
    State(merman_model::families::state::StateTerminalView<'a>),
    #[cfg(feature = "diagram-timeline")]
    Timeline(&'a merman_model::families::timeline::TimelineDiagramModel),
    #[cfg(feature = "diagram-tree-view")]
    TreeView(&'a merman_model::families::tree_view::TreeViewDiagramModel),
    #[cfg(feature = "diagram-xychart")]
    XyChart {
        model: &'a merman_model::families::xy_chart::XyChartDiagramModel,
        display: merman_model::families::xy_chart::XyChartDisplayPolicy,
    },
    /// Retains an unsupported host family's identity for normal capability admission.
    Unsupported(&'a str),
}

impl<'a> AsciiDiagram<'a> {
    pub(crate) fn from_semantic(
        model: &'a SemanticDiagramModel,
        source: AsciiSourceContext<'a>,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        execution.checkpoint(merman_model::OperationPhase::Semantic)?;
        Ok(match model {
            #[cfg(feature = "diagram-class")]
            SemanticDiagramModel::Class(model) => {
                crate::validate_model_identity("class", source.diagram_type)?;
                let resources =
                    execution.new_resource_context(merman_model::OperationPhase::Semantic);
                // Admit the authored graph before deriving topology maps and copied ids. The
                // later full preflight also accounts for the derived view, but cannot protect
                // this allocation boundary retroactively.
                let authored = ModelComplexity::from_serializable(model);
                resources.check_nesting_depth(authored.nesting_depth)?;
                resources.check(
                    crate::resource::AsciiResourceLimitId::MaxOutputBytes,
                    authored.text_bytes,
                )?;
                Self::Class(Box::new(model.topology_with_admission(
                    source.semantic_config,
                    &execution.cloned_control(),
                    |units| resources.charge_layout_work(units),
                )?))
            }
            #[cfg(feature = "diagram-er")]
            SemanticDiagramModel::Er(model) => Self::Er(model),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            SemanticDiagramModel::Flowchart(model) => Self::Flowchart(model),
            #[cfg(feature = "diagram-gantt")]
            SemanticDiagramModel::Gantt(model) => Self::Gantt(
                merman_model::families::gantt::GanttTerminalView::from_model(
                    model,
                    source.semantic_config,
                ),
            ),
            #[cfg(feature = "diagram-git-graph")]
            SemanticDiagramModel::GitGraph(model) => Self::GitGraph(model),
            #[cfg(feature = "diagram-journey")]
            SemanticDiagramModel::Journey(model) => Self::Journey(model),
            #[cfg(feature = "diagram-kanban")]
            SemanticDiagramModel::Kanban(model) => {
                Self::Kanban(merman_model::families::kanban::KanbanTerminalView::from_model(model))
            }
            #[cfg(feature = "diagram-mindmap")]
            SemanticDiagramModel::Mindmap(model) => Self::Mindmap(
                merman_model::families::mindmap::MindmapTerminalView::from_semantic(
                    model,
                    source.semantic_config,
                ),
            ),
            #[cfg(feature = "diagram-packet")]
            SemanticDiagramModel::Packet(model) => Self::Packet(model),
            #[cfg(feature = "diagram-sequence")]
            SemanticDiagramModel::Sequence(model) => Self::Sequence(model),
            #[cfg(feature = "diagram-state")]
            SemanticDiagramModel::State(model) => {
                Self::State(merman_model::families::state::StateTerminalView::from_semantic(model))
            }
            #[cfg(feature = "diagram-timeline")]
            SemanticDiagramModel::Timeline(model) => Self::Timeline(model),
            #[cfg(feature = "diagram-tree-view")]
            SemanticDiagramModel::TreeView(model) => Self::TreeView(model),
            #[cfg(feature = "diagram-xychart")]
            SemanticDiagramModel::XyChart(model) => Self::XyChart {
                model,
                display: merman_model::families::xy_chart::XyChartDisplayPolicy::from_config(
                    source.semantic_config,
                ),
            },
            _ => Self::Unsupported(source.diagram_type),
        })
    }

    pub fn kind(&self) -> &str {
        match self {
            #[cfg(feature = "diagram-class")]
            Self::Class(_) => "class",
            #[cfg(feature = "diagram-er")]
            Self::Er(_) => "er",
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(_) => "flowchart",
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(_) => "gantt",
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(_) => "gitGraph",
            #[cfg(feature = "diagram-journey")]
            Self::Journey(_) => "journey",
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(_) => "kanban",
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(_) => "mindmap",
            #[cfg(feature = "diagram-packet")]
            Self::Packet(_) => "packet",
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(_) => "sequence",
            #[cfg(feature = "diagram-state")]
            Self::State(_) => "state",
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(_) => "timeline",
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(_) => "treeView",
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart { .. } => "xychart",
            Self::Unsupported(kind) => kind,
        }
    }

    pub(crate) fn complexity(&self) -> ModelComplexity {
        match self {
            #[cfg(feature = "diagram-class")]
            Self::Class(model) => merman_model::families::class::class_topology_complexity(model),
            #[cfg(feature = "diagram-er")]
            Self::Er(model) => ModelComplexity::from_serializable(model),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(model) => {
                let mut complexity =
                    merman_model::resources::FlowchartComplexity::from_model(model)
                        .as_model_complexity();
                // Subgraph metadata is opaque authored JSON and is omitted from the legacy
                // flowchart counters. The ASCII fallback still serializes it, so admit its
                // exact borrowed shape before invoking generic serde.
                for metadata in model
                    .subgraphs
                    .iter()
                    .filter_map(|subgraph| subgraph.metadata.as_ref())
                {
                    let metadata = ModelComplexity::from_json(metadata);
                    complexity.items = complexity.items.saturating_add(metadata.items);
                    complexity.text_bytes =
                        complexity.text_bytes.saturating_add(metadata.text_bytes);
                    complexity.nesting_depth = complexity.nesting_depth.max(metadata.nesting_depth);
                }
                let vertex_styles =
                    ModelComplexity::from_serializable(&model.subgraph_vertex_styles);
                complexity.items = complexity.items.saturating_add(vertex_styles.items);
                complexity.text_bytes = complexity
                    .text_bytes
                    .saturating_add(vertex_styles.text_bytes);
                complexity.nesting_depth =
                    complexity.nesting_depth.max(vertex_styles.nesting_depth);
                complexity
            }
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(model) => model.complexity(),
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(model) => ModelComplexity::from_serializable(model),
            #[cfg(feature = "diagram-journey")]
            Self::Journey(model) => ModelComplexity::from_serializable(model),
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(model) => model.complexity(),
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(model) => model.complexity(),
            #[cfg(feature = "diagram-packet")]
            Self::Packet(model) => ModelComplexity::from_serializable(model),
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(model) => {
                merman_model::resources::SequenceComplexity::from_model(model).as_model_complexity()
            }
            #[cfg(feature = "diagram-state")]
            Self::State(model) => model.complexity(),
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(model) => ModelComplexity::from_serializable(model),
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(model) => ModelComplexity::from_serializable(model),
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart { model, .. } => ModelComplexity::from_serializable(model),
            Self::Unsupported(_) => ModelComplexity::default(),
        }
    }
}

impl Serialize for AsciiDiagram<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        match self {
            #[cfg(feature = "diagram-class")]
            Self::Class(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-er")]
            Self::Er(model) => model.serialize(serializer),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(model) => {
                #[derive(Serialize)]
                struct Projection<'a> {
                    #[serde(flatten)]
                    model: &'a merman_model::families::flowchart::FlowchartModel,
                    #[serde(rename = "subgraphVertexStyles")]
                    subgraph_vertex_styles:
                        &'a merman_model::families::flowchart::FlowchartSubgraphStyles,
                    #[serde(rename = "subgraphExplicitDirections")]
                    subgraph_explicit_directions: SubgraphExplicitDirections<'a>,
                }
                Projection {
                    model,
                    subgraph_vertex_styles: &model.subgraph_vertex_styles,
                    subgraph_explicit_directions: SubgraphExplicitDirections(&model.subgraphs),
                }
                .serialize(serializer)
            }
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-journey")]
            Self::Journey(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-packet")]
            Self::Packet(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(model) => {
                #[derive(Serialize)]
                struct Projection<'a> {
                    #[serde(flatten)]
                    model: merman_model::families::sequence::SequenceCompatibilityProjection<'a>,
                    #[serde(rename = "actorLifecycles")]
                    actor_lifecycles:
                        &'a Option<Vec<merman_model::families::sequence::SequenceActorLifecycle>>,
                }
                Projection {
                    model: model.compatibility_projection("sequence"),
                    actor_lifecycles: &model.actor_lifecycles,
                }
                .serialize(serializer)
            }
            #[cfg(feature = "diagram-state")]
            Self::State(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(model) => model.serialize(serializer),
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart { model, display } => {
                #[derive(Serialize)]
                struct Projection<'a> {
                    #[serde(flatten)]
                    model: &'a merman_model::families::xy_chart::XyChartDiagramModel,
                    display: &'a merman_model::families::xy_chart::XyChartDisplayPolicy,
                }
                Projection { model, display }.serialize(serializer)
            }
            Self::Unsupported(kind) => {
                let _ = serializer;
                Err(serde::ser::Error::custom(format_args!(
                    "unsupported terminal family: {kind}"
                )))
            }
        }
    }
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
struct SubgraphExplicitDirections<'a>(&'a [merman_model::families::flowchart::FlowSubgraph]);

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl Serialize for SubgraphExplicitDirections<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for subgraph in self.0 {
            sequence.serialize_element(&subgraph.has_explicit_dir)?;
        }
        sequence.end()
    }
}
