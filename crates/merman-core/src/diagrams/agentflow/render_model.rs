use super::presentation::{AgentflowEdgeAttributes, AgentflowNodeAttributes};
use super::{
    AgentflowAuthoredStyles, AgentflowConnector, AgentflowContainment, AgentflowDiagnostic,
    AgentflowDiagramModel, AgentflowEdge, AgentflowEdgeSemantic, AgentflowNode, AgentflowSubGraph,
    AgentflowVertexKind, normalized_render_shape,
};
use crate::diagrams::flowchart::FlowEdgeDefaults;
use indexmap::IndexMap;
use serde::ser::{SerializeMap, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

/// Computed identifiers needed only by the Flowchart-backed renderer.
#[derive(Debug, Clone, Default)]
pub struct AgentflowVisuals {
    pub(super) node_dom_indices: HashMap<String, usize>,
}

/// One canonical graph plus operation-requested rendering state.
#[derive(Debug, Clone)]
pub struct AgentflowDiagramRenderModel {
    pub model: AgentflowDiagramModel,
    pub visuals: AgentflowVisuals,
}

impl std::ops::Deref for AgentflowDiagramRenderModel {
    type Target = AgentflowDiagramModel;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl std::ops::DerefMut for AgentflowDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.model
    }
}

impl Serialize for AgentflowDiagramRenderModel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let fields = 8
            + usize::from(self.title.is_some())
            + usize::from(self.acc_title.is_some())
            + usize::from(self.acc_descr.is_some());
        let mut record = serializer.serialize_struct("AgentflowDiagramRenderModel", fields)?;
        record.serialize_field(
            "presentation",
            &FullPresentation {
                authored: &self.model.authored,
                visuals: &self.visuals,
            },
        )?;
        record.serialize_field("direction", &self.direction)?;
        record.serialize_field("diagnostics", &self.diagnostics)?;
        record.serialize_field("warningFacts", &self.warning_facts)?;
        record.serialize_field("vertices", &self.vertices)?;
        record.serialize_field("edges", &self.edges)?;
        record.serialize_field("subGraphs", &self.sub_graphs)?;
        record.serialize_field("connectors", &self.connectors)?;
        if let Some(title) = &self.title {
            record.serialize_field("title", title)?;
        }
        if let Some(title) = &self.acc_title {
            record.serialize_field("accTitle", title)?;
        }
        if let Some(description) = &self.acc_descr {
            record.serialize_field("accDescr", description)?;
        }
        record.end()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FullModelWire {
    #[serde(default)]
    presentation: FullPresentationWire,
    direction: String,
    #[serde(default)]
    diagnostics: Vec<AgentflowDiagnostic>,
    #[serde(default)]
    warning_facts: Vec<crate::DiagramWarningFact>,
    #[serde(default)]
    vertices: Vec<AgentflowNode>,
    #[serde(default)]
    edges: Vec<AgentflowEdge>,
    #[serde(default)]
    sub_graphs: Vec<AgentflowSubGraph>,
    #[serde(default)]
    connectors: Vec<AgentflowConnector>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    acc_descr: Option<String>,
}

impl<'de> Deserialize<'de> for AgentflowDiagramRenderModel {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FullModelWire::deserialize(deserializer)?;
        let mut visuals = AgentflowVisuals::default();
        let nodes = wire
            .presentation
            .nodes
            .into_iter()
            .map(|(id, node)| {
                if let Some(index) = node.dom_index {
                    visuals.node_dom_indices.insert(id.clone(), index);
                }
                (id, node.authored)
            })
            .collect();
        Ok(Self {
            model: AgentflowDiagramModel {
                authored: AgentflowAuthoredStyles {
                    nodes,
                    edges: wire.presentation.edges,
                    subgraph_classes: wire.presentation.subgraph_classes,
                    subgraph_label_types: wire.presentation.subgraph_label_types,
                    class_defs: wire.presentation.class_defs,
                    edge_defaults: wire.presentation.edge_defaults,
                    tooltips: wire.presentation.tooltips,
                },
                direction: wire.direction,
                diagnostics: wire.diagnostics,
                warning_facts: wire.warning_facts,
                vertices: wire.vertices,
                edges: wire.edges,
                sub_graphs: wire.sub_graphs,
                connectors: wire.connectors,
                title: wire.title,
                acc_title: wire.acc_title,
                acc_descr: wire.acc_descr,
            },
            visuals,
        })
    }
}

#[derive(Default, Deserialize)]
struct FullPresentationWire {
    #[serde(default)]
    nodes: HashMap<String, FullNodeWire>,
    #[serde(default)]
    edges: HashMap<String, AgentflowEdgeAttributes>,
    #[serde(default)]
    subgraph_classes: HashMap<String, Vec<String>>,
    #[serde(default)]
    subgraph_label_types: HashMap<String, String>,
    #[serde(default)]
    class_defs: IndexMap<String, Vec<String>>,
    #[serde(default)]
    edge_defaults: Option<FlowEdgeDefaults>,
    #[serde(default)]
    tooltips: HashMap<String, String>,
}

#[derive(Deserialize)]
struct FullNodeWire {
    #[serde(default)]
    dom_index: Option<usize>,
    #[serde(flatten)]
    authored: AgentflowNodeAttributes,
}

struct FullPresentation<'a> {
    authored: &'a AgentflowAuthoredStyles,
    visuals: &'a AgentflowVisuals,
}

impl Serialize for FullPresentation<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("AgentflowPresentation", 7)?;
        record.serialize_field(
            "nodes",
            &FullNodes {
                authored: self.authored,
                visuals: self.visuals,
            },
        )?;
        record.serialize_field("edges", &self.authored.edges)?;
        record.serialize_field("subgraph_classes", &self.authored.subgraph_classes)?;
        record.serialize_field("subgraph_label_types", &self.authored.subgraph_label_types)?;
        record.serialize_field("class_defs", &self.authored.class_defs)?;
        record.serialize_field("edge_defaults", &self.authored.edge_defaults)?;
        record.serialize_field("tooltips", &self.authored.tooltips)?;
        record.end()
    }
}

struct FullNodes<'a> {
    authored: &'a AgentflowAuthoredStyles,
    visuals: &'a AgentflowVisuals,
}

impl Serialize for FullNodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut nodes = serializer.serialize_map(Some(self.authored.nodes.len()))?;
        for (id, node) in &self.authored.nodes {
            nodes.serialize_entry(
                id,
                &FullNode {
                    authored: node,
                    dom_index: self.visuals.node_dom_indices.get(id).copied(),
                },
            )?;
        }
        nodes.end()
    }
}

struct FullNode<'a> {
    authored: &'a AgentflowNodeAttributes,
    dom_index: Option<usize>,
}

impl Serialize for FullNode<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut record = serializer.serialize_struct("NodePresentation", 7)?;
        record.serialize_field("dom_index", &self.dom_index)?;
        record.serialize_field("label_type", &self.authored.label_type)?;
        record.serialize_field("classes", &self.authored.classes)?;
        record.serialize_field("styles", &self.authored.styles)?;
        record.serialize_field("link", &self.authored.link)?;
        record.serialize_field("link_target", &self.authored.link_target)?;
        record.serialize_field("have_callback", &self.authored.have_callback)?;
        record.end()
    }
}

impl AgentflowDiagramRenderModel {
    /// Projects the domain graph into the shared flow layout model.
    ///
    /// Agentflow owns its semantic JSON and diagnostics, while Mermaid 12 uses the unified
    /// flow renderer for node geometry and ELK/Dagre routing. Keeping this adapter explicit
    /// prevents the compatibility model from silently acquiring Flowchart-only fields.
    pub fn to_flowchart_model(
        &self,
    ) -> (
        crate::diagrams::flowchart::FlowchartModel,
        crate::diagrams::flowchart::FlowchartRenderContext,
    ) {
        use crate::diagrams::flowchart::{
            FlowEdge, FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowSubgraph,
            FlowchartModel, FlowchartRenderContext,
        };
        let nodes = self
            .vertices
            .iter()
            .map(|node| {
                flow_node(
                    &node.id,
                    node.label.clone(),
                    normalized_render_shape(node.shape.as_deref()),
                    node.vertex_kind,
                )
            })
            .chain(self.connectors.iter().map(|connector| {
                flow_node(
                    &connector.id,
                    Some(
                        connector
                            .title
                            .clone()
                            .unwrap_or_else(|| connector.id.clone()),
                    ),
                    "roundedRect",
                    AgentflowVertexKind::Connector,
                )
            }))
            .collect();
        let edges = self
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                let (arrow, end_marker, stroke_kind) = match edge.edge_semantic {
                    AgentflowEdgeSemantic::Sequence => {
                        ("-->", FlowEdgeMarker::Point, FlowEdgeStroke::Normal)
                    }
                    AgentflowEdgeSemantic::Reference => {
                        ("-.-", FlowEdgeMarker::None, FlowEdgeStroke::Dotted)
                    }
                    AgentflowEdgeSemantic::Failure => {
                        ("--x", FlowEdgeMarker::Cross, FlowEdgeStroke::Normal)
                    }
                };
                FlowEdge {
                    id: edge.id.clone().unwrap_or_else(|| format!("edge-{index}")),
                    from: edge.start.clone(),
                    to: edge.end.clone(),
                    label: edge.label.clone(),
                    label_type: None,
                    edge_type: Some(edge.edge_type.clone()),
                    arrow: arrow.into(),
                    start_marker: FlowEdgeMarker::None,
                    end_marker,
                    is_user_defined_id: edge.is_user_defined_id,
                    stroke: Some(edge.stroke.clone()),
                    stroke_kind,
                    visibility: FlowEdgeVisibility::Visible,
                    interpolate: None,
                    classes: Vec::new(),
                    style: Vec::new(),
                    animate: None,
                    animation: None,
                    length: edge.length.max(1),
                }
            })
            .collect();
        let containment = self.containment();
        let subgraphs = self
            .sub_graphs
            .iter()
            .map(|graph| FlowSubgraph {
                id: graph.id.clone(),
                title: graph.title.clone().unwrap_or_default(),
                dir: graph.direction.clone(),
                has_explicit_dir: graph.direction.is_some(),
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: graph
                    .nodes
                    .iter()
                    .filter(|child| {
                        // Hidden containers never enter Agentflow's parent database. Project only
                        // admitted parent assignments so a backend cannot recreate a hidden parent.
                        containment.parents.get(child.as_str()).copied() == Some(graph.id.as_str())
                    })
                    .cloned()
                    .collect(),
                metadata: (!graph.metadata.is_empty())
                    .then(|| Value::Object(graph.metadata.clone())),
            })
            .collect();
        let mut model = FlowchartModel {
            keyword: "agentflow-beta".into(),
            direction: Some(self.direction.clone()),
            acc_title: self.acc_title.clone(),
            acc_descr: self.acc_descr.clone(),
            class_defs: Default::default(),
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes,
            edges,
            subgraphs,
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        };
        self.model.authored.apply_to_flowchart(&mut model);
        let collapsed = self
            .sub_graphs
            .iter()
            .filter(|graph| graph.metadata.get("view").and_then(Value::as_str) == Some("collapsed"))
            .map(|graph| graph.id.clone())
            .collect();
        let mut context = FlowchartRenderContext::new(
            Default::default(),
            Default::default(),
            collapsed,
            &model.subgraphs,
        );
        context.set_node_dom_indices(
            self.visuals
                .node_dom_indices
                .iter()
                .map(|(id, index)| (id.clone(), *index)),
        );
        context.set_collapsed_replacements(
            containment
                .collapsed_replacements
                .iter()
                .map(|(id, ancestor)| ((*id).to_string(), (*ancestor).to_string())),
        );
        context.set_subgraph_color_ordinals(containment.color_ordinals(&self.sub_graphs));
        model.edges = self
            .edges
            .iter()
            .zip(model.edges)
            .filter_map(|(source, mut edge)| {
                let (from, to) = containment.rendered_edge_endpoints(source)?;
                if edge.from != from {
                    from.clone_into(&mut edge.from);
                }
                if edge.to != to {
                    to.clone_into(&mut edge.to);
                }
                Some(edge)
            })
            .collect();
        (model, context)
    }

    /// Borrows the exact labels and kinds selected by the concrete Flowchart adapter.
    #[doc(hidden)]
    pub fn render_label_sources(&self) -> AgentflowRenderLabelSources<'_> {
        AgentflowRenderLabelSources {
            model: self,
            containment: self.containment(),
        }
    }
}

/// Operation-local label view; canonical labels are never copied for typography admission.
#[doc(hidden)]
pub struct AgentflowRenderLabelSources<'a> {
    model: &'a AgentflowDiagramRenderModel,
    containment: AgentflowContainment<'a>,
}

impl AgentflowRenderLabelSources<'_> {
    pub fn node_labels(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.model
            .vertices
            .iter()
            .map(|node| {
                (
                    node.label.as_deref().unwrap_or(&node.id),
                    self.model.authored.node_label_type(&node.id),
                )
            })
            .chain(self.model.connectors.iter().map(|node| {
                (
                    node.title.as_deref().unwrap_or(&node.id),
                    self.model.authored.node_label_type(&node.id),
                )
            }))
    }

    pub fn edge_labels(&self) -> impl Iterator<Item = Option<(&str, Option<&str>)>> {
        self.model.edges.iter().enumerate().map(|(index, edge)| {
            self.containment.rendered_edge_endpoints(edge)?;
            let label = edge.label.as_deref()?;
            let generated;
            let id = match edge.id.as_deref() {
                Some(id) => id,
                None => {
                    generated = format!("edge-{index}");
                    &generated
                }
            };
            Some((label, self.model.authored.edge_label_type(id)))
        })
    }

    pub fn subgraph_labels(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.model.sub_graphs.iter().map(|graph| {
            (
                graph.title.as_deref().unwrap_or_default(),
                self.model
                    .authored
                    .subgraph_label_types
                    .get(&graph.id)
                    .map(String::as_str),
            )
        })
    }
}

fn flow_node(
    id: &str,
    label: Option<String>,
    shape: &str,
    kind: AgentflowVertexKind,
) -> crate::diagrams::flowchart::FlowNode {
    crate::diagrams::flowchart::FlowNode {
        id: id.to_string(),
        provenance: Default::default(),
        label,
        label_type: None,
        layout_shape: Some(shape.to_string()),
        shape: Some(shape.to_string()),
        icon: None,
        form: None,
        pos: None,
        img: None,
        constraint: None,
        asset_width: None,
        asset_height: None,
        classes: vec![kind.css_class().into()],
        styles: Vec::new(),
        link: None,
        link_target: None,
        have_callback: false,
    }
}
