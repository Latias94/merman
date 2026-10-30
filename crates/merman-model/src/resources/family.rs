//! Pure statistics over selected canonical family data.

use super::ModelComplexity;
#[cfg(all(feature = "diagram-class", feature = "presentation-data"))]
use crate::families::class::ClassDiagram;
#[cfg(feature = "diagram-class")]
use crate::families::class::ClassTopologyView;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
use crate::families::flowchart::FlowchartModel;
#[cfg(feature = "diagram-zenuml")]
use crate::families::zenuml::{ZenumlDiagramModel, ZenumlStatementKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub struct FlowchartComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub subgraphs: usize,
    pub label_bytes: usize,
    pub subgraph_depth: usize,
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
impl FlowchartComplexity {
    pub fn from_model(model: &FlowchartModel) -> Self {
        let mut complexity = Self::without_depth(model);
        complexity.subgraph_depth =
            flowchart_subgraph_depth_with_control(model, None).unwrap_or(usize::MAX);
        complexity
    }

    pub(super) fn without_depth(model: &FlowchartModel) -> Self {
        Self {
            nodes: model.nodes.len().saturating_add(model.subgraphs.len()),
            edges: model.edges.len(),
            subgraphs: model.subgraphs.len(),
            label_bytes: flowchart_text_bytes(model),
            subgraph_depth: 0,
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.edges),
            self.label_bytes,
            self.subgraph_depth,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-class")]
pub struct ClassComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub namespaces: usize,
    pub label_bytes: usize,
    pub namespace_depth: usize,
}

#[cfg(feature = "diagram-class")]
impl ClassComplexity {
    #[cfg(feature = "presentation-data")]
    pub fn from_model(model: &ClassDiagram) -> Self {
        Self::from_view(&model.topology_view())
    }

    /// Counts the same effective Class projection without requiring a visual carrier.
    pub fn from_view(model: &ClassTopologyView<'_>) -> Self {
        let class_label_bytes = model
            .classes()
            .map(|(_, node)| {
                node.id
                    .len()
                    .saturating_add(node.label.len())
                    .saturating_add(node.text.len())
                    .saturating_add(node.type_param.len())
                    .saturating_add(node.css_classes.len())
                    .saturating_add(node.tooltip.as_deref().map(str::len).unwrap_or(0))
                    .saturating_add(node.link.as_deref().map(str::len).unwrap_or(0))
                    .saturating_add(
                        node.members
                            .iter()
                            .chain(node.methods.iter())
                            .map(|member| {
                                member
                                    .display_text
                                    .len()
                                    .saturating_add(member.id.len())
                                    .saturating_add(member.parameters.len())
                                    .saturating_add(member.return_type.len())
                            })
                            .sum::<usize>(),
                    )
                    .saturating_add(node.annotations.iter().map(String::len).sum::<usize>())
                    .saturating_add(node.styles.iter().map(String::len).sum::<usize>())
            })
            .sum::<usize>();
        let relation_label_bytes = model
            .relations()
            .map(|rel| {
                rel.id
                    .len()
                    .saturating_add(rel.id1.len())
                    .saturating_add(rel.id2.len())
                    .saturating_add(rel.title.len())
                    .saturating_add(rel.relation_title_1.as_deref().map_or(0, str::len))
                    .saturating_add(rel.relation_title_2.as_deref().map_or(0, str::len))
            })
            .sum::<usize>();
        let note_label_bytes = model
            .notes()
            .map(|note| {
                note.id
                    .len()
                    .saturating_add(note.text.len())
                    .saturating_add(note.class_id.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();
        let interface_label_bytes = model
            .interfaces()
            .map(|iface| {
                iface
                    .id
                    .len()
                    .saturating_add(iface.label.len())
                    .saturating_add(iface.class_id.len())
            })
            .sum::<usize>();
        let namespace_label_bytes = model
            .namespaces()
            .map(|(_, namespace)| {
                namespace
                    .id
                    .len()
                    .saturating_add(namespace.label.len())
                    .saturating_add(namespace.parent.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();

        Self {
            label_bytes: class_label_bytes
                .saturating_add(relation_label_bytes)
                .saturating_add(note_label_bytes)
                .saturating_add(interface_label_bytes)
                .saturating_add(namespace_label_bytes),
            ..Self::topology_from_view(model)
        }
    }

    #[cfg(feature = "presentation-data")]
    /// Counts the topology of a full Class model.
    pub fn topology_from_model(model: &ClassDiagram) -> Self {
        Self::topology_from_view(&model.topology_view())
    }

    /// Counts topology only, excluding retained authored and visual text.
    pub fn topology_from_view(model: &ClassTopologyView<'_>) -> Self {
        let mut complexity = Self::topology_without_depth(model);
        complexity.namespace_depth = class_namespace_depth(model.effective_namespaces());
        complexity
    }

    pub(super) fn topology_without_depth(model: &ClassTopologyView<'_>) -> Self {
        Self {
            nodes: model
                .classes()
                .len()
                .saturating_add(model.notes().len())
                .saturating_add(model.interfaces().len())
                .saturating_add(model.namespaces().len()),
            edges: model
                .relations()
                .len()
                .saturating_add(model.notes().filter(|note| note.class_id.is_some()).count()),
            namespaces: model.namespaces().len(),
            label_bytes: 0,
            namespace_depth: 0,
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.edges),
            self.label_bytes,
            self.namespace_depth,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-zenuml")]
pub struct ZenumlComplexity {
    pub participants: usize,
    pub groups: usize,
    pub statements: usize,
    pub fragments: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-zenuml")]
impl ZenumlComplexity {
    pub fn from_model(model: &ZenumlDiagramModel) -> Self {
        let common_label_bytes = [model.title.as_deref(), model.starter.as_deref()]
            .into_iter()
            .flatten()
            .fold(0usize, |total, value| total.saturating_add(value.len()));
        let participant_label_bytes =
            model
                .participants
                .iter()
                .fold(0usize, |total, participant| {
                    [
                        Some(participant.name.as_str()),
                        participant.label.as_deref(),
                        participant.participant_type.as_deref(),
                        participant.stereotype.as_deref(),
                        participant.emoji.as_deref(),
                        participant.width_source.as_deref(),
                        participant.color.as_deref(),
                        participant.group_id.as_deref(),
                    ]
                    .into_iter()
                    .flatten()
                    .fold(total, |subtotal, value| {
                        subtotal.saturating_add(value.len())
                    })
                });
        let group_label_bytes = model.groups.iter().fold(0usize, |total, group| {
            group.participant_names.iter().fold(
                total.saturating_add(group.id.as_deref().map_or(0, str::len)),
                |subtotal, name| subtotal.saturating_add(name.len()),
            )
        });
        let mut complexity = Self {
            participants: model.participants.len(),
            groups: model.groups.len(),
            statements: 0,
            fragments: 0,
            label_bytes: common_label_bytes
                .saturating_add(participant_label_bytes)
                .saturating_add(group_label_bytes),
            nesting_depth: 0,
        };
        let mut pending = vec![(model.statements.as_slice(), 0usize)];
        while let Some((statements, nesting_depth)) = pending.pop() {
            complexity.nesting_depth = complexity.nesting_depth.max(nesting_depth);
            for statement in statements {
                complexity.statements = complexity.statements.saturating_add(1);
                complexity.label_bytes = complexity
                    .label_bytes
                    .saturating_add(statement.comment.as_deref().map_or(0, str::len));
                match &statement.kind {
                    ZenumlStatementKind::Message {
                        explicit_from,
                        resolved_from,
                        resolved_to,
                        label,
                        assignment,
                        body,
                        ..
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(explicit_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_to.as_deref().map_or(0, str::len))
                            .saturating_add(label.len())
                            .saturating_add(assignment.as_deref().map_or(0, str::len));
                        pending.push((body, nesting_depth.saturating_add(1)));
                    }
                    ZenumlStatementKind::Creation {
                        resolved_from,
                        resolved_to,
                        constructor,
                        parameters,
                        assignment,
                        label,
                        body,
                        ..
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(resolved_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_to.len())
                            .saturating_add(constructor.len())
                            .saturating_add(parameters.len())
                            .saturating_add(assignment.as_deref().map_or(0, str::len))
                            .saturating_add(label.len());
                        pending.push((body, nesting_depth.saturating_add(1)));
                    }
                    ZenumlStatementKind::Return {
                        explicit_from,
                        resolved_from,
                        explicit_to,
                        resolved_to,
                        label,
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(explicit_from.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_from.as_deref().map_or(0, str::len))
                            .saturating_add(explicit_to.as_deref().map_or(0, str::len))
                            .saturating_add(resolved_to.as_deref().map_or(0, str::len))
                            .saturating_add(label.len());
                    }
                    ZenumlStatementKind::Fragment {
                        label, sections, ..
                    } => {
                        complexity.fragments = complexity.fragments.saturating_add(1);
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(label.as_deref().map_or(0, str::len));
                        for section in sections {
                            complexity.label_bytes = complexity
                                .label_bytes
                                .saturating_add(section.label.as_deref().map_or(0, str::len));
                            pending.push((&section.statements, nesting_depth.saturating_add(1)));
                        }
                    }
                    ZenumlStatementKind::Reference {
                        participants,
                        label,
                    } => {
                        complexity.label_bytes = participants.iter().fold(
                            complexity.label_bytes.saturating_add(label.len()),
                            |total, participant| total.saturating_add(participant.len()),
                        );
                    }
                    ZenumlStatementKind::Divider { label } => {
                        complexity.label_bytes = complexity.label_bytes.saturating_add(label.len());
                    }
                }
            }
        }
        complexity
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.participants
                .saturating_add(self.groups)
                .saturating_add(self.statements),
            self.label_bytes,
            self.nesting_depth,
        )
    }
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
fn flowchart_text_bytes(model: &FlowchartModel) -> usize {
    let mut total = [
        Some(model.keyword.as_str()),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        model.direction.as_deref(),
    ]
    .into_iter()
    .flatten()
    .fold(0usize, |subtotal, value| {
        subtotal.saturating_add(value.len())
    });
    for (id, styles) in &model.class_defs {
        total = styles
            .iter()
            .fold(total.saturating_add(id.len()), |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    total = model.vertex_calls.iter().fold(total, |subtotal, value| {
        subtotal.saturating_add(value.len())
    });
    if let Some(defaults) = &model.edge_defaults {
        total = total.saturating_add(defaults.interpolate.as_deref().map_or(0, str::len));
        total = defaults.style.iter().fold(total, |subtotal, value| {
            subtotal.saturating_add(value.len())
        });
    }
    for node in &model.nodes {
        for value in [
            Some(node.id.as_str()),
            node.label.as_deref(),
            node.label_type.as_deref(),
            node.layout_shape.as_deref(),
            node.shape.as_deref(),
            node.icon.as_deref(),
            node.form.as_deref(),
            node.pos.as_deref(),
            node.img.as_deref(),
            node.constraint.as_deref(),
            node.link.as_deref(),
            node.link_target.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(value.len());
        }
        total = node
            .classes
            .iter()
            .chain(&node.styles)
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    for edge in &model.edges {
        for value in [
            Some(edge.id.as_str()),
            Some(edge.from.as_str()),
            Some(edge.to.as_str()),
            edge.label.as_deref(),
            edge.label_type.as_deref(),
            edge.edge_type.as_deref(),
            Some(edge.arrow.as_str()),
            edge.stroke.as_deref(),
            edge.interpolate.as_deref(),
            edge.animation.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(value.len());
        }
        total = edge
            .classes
            .iter()
            .chain(&edge.style)
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    for subgraph in &model.subgraphs {
        for value in [
            Some(subgraph.id.as_str()),
            Some(subgraph.title.as_str()),
            subgraph.dir.as_deref(),
            subgraph.label_type.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            total = total.saturating_add(value.len());
        }
        total = subgraph
            .classes
            .iter()
            .chain(&subgraph.styles)
            .chain(&subgraph.nodes)
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
    }
    total = model.tooltips.iter().fold(total, |subtotal, (id, value)| {
        subtotal
            .saturating_add(id.len())
            .saturating_add(value.len())
    });
    total.saturating_add(model.subgraph_vertex_styles.retained_bytes())
}

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(super) fn flowchart_subgraph_depth_with_control(
    model: &FlowchartModel,
    control: Option<&crate::OperationControl>,
) -> crate::OperationControlResult<usize> {
    if let Some(control) = control {
        control.checkpoint()?;
    }
    if model.subgraphs.is_empty() {
        return Ok(0);
    }
    let mut indices = std::collections::HashMap::with_capacity(model.subgraphs.len());
    for (index, subgraph) in model.subgraphs.iter().enumerate() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        indices.insert(subgraph.id.as_str(), index);
    }
    let mut incoming = vec![0usize; model.subgraphs.len()];
    for subgraph in &model.subgraphs {
        for member in &subgraph.nodes {
            if let Some(control) = control {
                control.checkpoint()?;
            }
            let Some(&child) = indices.get(member.as_str()) else {
                continue;
            };
            incoming[child] = incoming[child].saturating_add(1);
        }
    }
    let mut queue = std::collections::VecDeque::new();
    let mut depths = vec![1usize; model.subgraphs.len()];
    for (index, incoming) in incoming.iter().enumerate() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        if *incoming == 0 {
            queue.push_back(index);
        }
    }
    while let Some(parent) = queue.pop_front() {
        // Traverse existing members instead of copying an adjacency table into scratch.
        for member in &model.subgraphs[parent].nodes {
            if let Some(control) = control {
                control.checkpoint()?;
            }
            let Some(&child) = indices.get(member.as_str()) else {
                continue;
            };
            depths[child] = depths[child].max(depths[parent].saturating_add(1));
            incoming[child] = incoming[child].saturating_sub(1);
            if incoming[child] == 0 {
                queue.push_back(child);
            }
        }
    }
    Ok(depths.into_iter().max().unwrap_or(0))
}

#[cfg(feature = "diagram-class")]
/// Counts retained namespace parent depth without recursive traversal.
pub fn class_namespace_depth(
    namespaces: &indexmap::IndexMap<String, crate::families::class::Namespace>,
) -> usize {
    class_namespace_depth_with_control(namespaces, None).unwrap_or(usize::MAX)
}

#[cfg(feature = "diagram-class")]
pub(super) fn class_namespace_depth_with_control(
    namespaces: &indexmap::IndexMap<String, crate::families::class::Namespace>,
    control: Option<&crate::OperationControl>,
) -> crate::OperationControlResult<usize> {
    if let Some(control) = control {
        control.checkpoint()?;
    }
    if namespaces.is_empty() {
        return Ok(0);
    }
    let mut indices = std::collections::HashMap::with_capacity(namespaces.len());
    for (index, id) in namespaces.keys().enumerate() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        indices.insert(id.as_str(), index);
    }
    let mut parents = Vec::with_capacity(namespaces.len());
    for namespace in namespaces.values() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        parents.push(
            namespace
                .parent
                .as_deref()
                .and_then(|parent| indices.get(parent).copied()),
        );
    }
    let mut depths = vec![0usize; parents.len()];
    let mut completed = vec![false; parents.len()];
    for start in 0..parents.len() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        if completed[start] {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = std::collections::HashMap::new();
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            if let Some(control) = control {
                control.checkpoint()?;
            }
            if completed[index] || positions.contains_key(&index) {
                break;
            }
            positions.insert(index, path.len());
            path.push(index);
            cursor = parents[index];
        }
        let mut depth = cursor
            .filter(|index| completed[*index])
            .map(|index| depths[index])
            .unwrap_or(0);
        for index in path.into_iter().rev() {
            if let Some(control) = control {
                control.checkpoint()?;
            }
            depth = depth.saturating_add(1);
            depths[index] = depth;
            completed[index] = true;
        }
    }
    Ok(depths.into_iter().max().unwrap_or(0))
}

#[cfg(feature = "diagram-mindmap")]
use crate::families::mindmap::MindmapDiagramModel;
#[cfg(all(feature = "diagram-mindmap", feature = "presentation-data"))]
use crate::families::mindmap::MindmapDiagramRenderModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-mindmap")]
pub struct MindmapComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-mindmap")]
impl MindmapComplexity {
    pub fn from_semantic_model(model: &MindmapDiagramModel) -> Self {
        Self {
            nodes: model.nodes.len(),
            edges: model.nodes.iter().fold(0usize, |count, node| {
                count.saturating_add(node.children.len())
            }),
            label_bytes: model.nodes.iter().fold(0usize, |count, node| {
                [
                    Some(node.node_id.as_str()),
                    Some(node.descr.as_str()),
                    node.class.as_deref(),
                    node.icon.as_deref(),
                ]
                .into_iter()
                .flatten()
                .fold(count, |count, text| count.saturating_add(text.len()))
            }),
            nesting_depth: model
                .nodes
                .iter()
                .filter_map(|node| usize::try_from(node.level).ok())
                .max()
                .unwrap_or(0),
        }
    }

    #[cfg(feature = "presentation-data")]
    pub fn from_model(model: &MindmapDiagramRenderModel) -> Self {
        let node_label_bytes = model.nodes.iter().fold(0usize, |total, node| {
            let strings = [
                node.id.as_str(),
                node.dom_id.as_str(),
                node.label.as_str(),
                node.label_type.as_str(),
                node.shape.as_str(),
                node.css_classes.as_str(),
                node.look.as_str(),
                node.node_id.as_str(),
            ];
            let total = strings.into_iter().fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
            let total = node.css_styles.iter().fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
            total.saturating_add(node.icon.as_deref().map_or(0, str::len))
        });
        let edge_label_bytes = model.edges.iter().fold(0usize, |total, edge| {
            [
                edge.id.as_str(),
                edge.start.as_str(),
                edge.end.as_str(),
                edge.edge_type.as_str(),
                edge.curve.as_str(),
                edge.thickness.as_str(),
                edge.look.as_str(),
                edge.classes.as_str(),
            ]
            .into_iter()
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            })
        });

        Self {
            nodes: model.nodes.len(),
            edges: model.edges.len(),
            label_bytes: node_label_bytes.saturating_add(edge_label_bytes),
            nesting_depth: model
                .nodes
                .iter()
                .filter_map(|node| usize::try_from(node.level).ok())
                .max()
                .unwrap_or(0),
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.edges),
            self.label_bytes,
            self.nesting_depth,
        )
    }
}
