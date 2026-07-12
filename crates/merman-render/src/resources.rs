use merman_core::diagrams::flowchart::FlowchartV2Model;
use merman_core::models::class_diagram::ClassDiagram;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderResourceProfile {
    Interactive,
    TypstPackage,
    TrustedNative,
    UnboundedForTrustedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderResourceLimits {
    pub max_source_bytes: Option<usize>,
    pub max_svg_bytes: Option<usize>,
    pub max_flowchart_nodes: Option<usize>,
    pub max_flowchart_edges: Option<usize>,
    pub max_flowchart_subgraphs: Option<usize>,
    pub max_class_nodes: Option<usize>,
    pub max_class_edges: Option<usize>,
    pub max_class_namespaces: Option<usize>,
    pub max_label_bytes: Option<usize>,
}

impl Default for RenderResourceLimits {
    fn default() -> Self {
        Self::interactive()
    }
}

impl RenderResourceLimits {
    pub const fn interactive() -> Self {
        Self {
            max_source_bytes: Some(2 * MIB),
            max_svg_bytes: Some(24 * MIB),
            max_flowchart_nodes: Some(8_000),
            max_flowchart_edges: Some(16_000),
            max_flowchart_subgraphs: Some(2_000),
            max_class_nodes: Some(8_000),
            max_class_edges: Some(16_000),
            max_class_namespaces: Some(2_000),
            max_label_bytes: Some(2 * MIB),
        }
    }

    pub const fn typst_package() -> Self {
        Self {
            max_source_bytes: Some(MIB),
            max_svg_bytes: Some(12 * MIB),
            max_flowchart_nodes: Some(4_000),
            max_flowchart_edges: Some(8_000),
            max_flowchart_subgraphs: Some(1_000),
            max_class_nodes: Some(4_000),
            max_class_edges: Some(8_000),
            max_class_namespaces: Some(1_000),
            max_label_bytes: Some(MIB),
        }
    }

    pub const fn trusted_native() -> Self {
        Self {
            max_source_bytes: Some(16 * MIB),
            max_svg_bytes: Some(128 * MIB),
            max_flowchart_nodes: Some(50_000),
            max_flowchart_edges: Some(100_000),
            max_flowchart_subgraphs: Some(10_000),
            max_class_nodes: Some(50_000),
            max_class_edges: Some(100_000),
            max_class_namespaces: Some(10_000),
            max_label_bytes: Some(16 * MIB),
        }
    }

    pub const fn unbounded_for_trusted_input() -> Self {
        Self {
            max_source_bytes: None,
            max_svg_bytes: None,
            max_flowchart_nodes: None,
            max_flowchart_edges: None,
            max_flowchart_subgraphs: None,
            max_class_nodes: None,
            max_class_edges: None,
            max_class_namespaces: None,
            max_label_bytes: None,
        }
    }

    pub const fn for_profile(profile: RenderResourceProfile) -> Self {
        match profile {
            RenderResourceProfile::Interactive => Self::interactive(),
            RenderResourceProfile::TypstPackage => Self::typst_package(),
            RenderResourceProfile::TrustedNative => Self::trusted_native(),
            RenderResourceProfile::UnboundedForTrustedInput => Self::unbounded_for_trusted_input(),
        }
    }

    pub fn check_source_bytes(&self, source: &str) -> Result<(), ResourceLimitExceeded> {
        check_limit(
            ResourceLimitPhase::Source,
            "max_source_bytes",
            source.len(),
            self.max_source_bytes,
        )
    }

    pub fn check_svg_bytes(
        &self,
        svg: &str,
        phase: ResourceLimitPhase,
    ) -> Result<(), ResourceLimitExceeded> {
        check_limit(phase, "max_svg_bytes", svg.len(), self.max_svg_bytes)
    }

    pub fn check_flowchart_complexity(
        &self,
        model: &FlowchartV2Model,
    ) -> Result<FlowchartComplexity, ResourceLimitExceeded> {
        let complexity = FlowchartComplexity::from_model(model);
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_flowchart_nodes",
            complexity.nodes,
            self.max_flowchart_nodes,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_flowchart_edges",
            complexity.edges,
            self.max_flowchart_edges,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_flowchart_subgraphs",
            complexity.subgraphs,
            self.max_flowchart_subgraphs,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_label_bytes",
            complexity.label_bytes,
            self.max_label_bytes,
        )?;
        Ok(complexity)
    }

    pub fn check_class_complexity(
        &self,
        model: &ClassDiagram,
    ) -> Result<ClassComplexity, ResourceLimitExceeded> {
        let complexity = ClassComplexity::from_model(model);
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_class_nodes",
            complexity.nodes,
            self.max_class_nodes,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_class_edges",
            complexity.edges,
            self.max_class_edges,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_class_namespaces",
            complexity.namespaces,
            self.max_class_namespaces,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_label_bytes",
            complexity.label_bytes,
            self.max_label_bytes,
        )?;
        Ok(complexity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowchartComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub subgraphs: usize,
    pub label_bytes: usize,
}

impl FlowchartComplexity {
    pub fn from_model(model: &FlowchartV2Model) -> Self {
        let node_label_bytes = model
            .nodes
            .iter()
            .map(|node| optional_str_len(node.label.as_deref()) + node.id.len())
            .sum::<usize>();
        let edge_label_bytes = model
            .edges
            .iter()
            .map(|edge| {
                optional_str_len(edge.label.as_deref())
                    + edge.id.len()
                    + edge.from.len()
                    + edge.to.len()
            })
            .sum::<usize>();
        let subgraph_label_bytes = model
            .subgraphs
            .iter()
            .map(|subgraph| subgraph.id.len() + subgraph.title.len())
            .sum::<usize>();
        let tooltip_bytes = model.tooltips.values().map(String::len).sum::<usize>();

        Self {
            nodes: model.nodes.len().saturating_add(model.subgraphs.len()),
            edges: model.edges.len(),
            subgraphs: model.subgraphs.len(),
            label_bytes: node_label_bytes
                .saturating_add(edge_label_bytes)
                .saturating_add(subgraph_label_bytes)
                .saturating_add(tooltip_bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassComplexity {
    pub nodes: usize,
    pub edges: usize,
    pub namespaces: usize,
    pub label_bytes: usize,
}

impl ClassComplexity {
    pub fn from_model(model: &ClassDiagram) -> Self {
        let class_label_bytes = model
            .classes
            .values()
            .map(|node| {
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
            .relations
            .iter()
            .map(|rel| {
                rel.id
                    .len()
                    .saturating_add(rel.id1.len())
                    .saturating_add(rel.id2.len())
                    .saturating_add(rel.title.len())
                    .saturating_add(rel.relation_title_1.len())
                    .saturating_add(rel.relation_title_2.len())
            })
            .sum::<usize>();
        let note_label_bytes = model
            .notes
            .iter()
            .map(|note| {
                note.id
                    .len()
                    .saturating_add(note.text.len())
                    .saturating_add(note.class_id.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();
        let interface_label_bytes = model
            .interfaces
            .iter()
            .map(|iface| {
                iface
                    .id
                    .len()
                    .saturating_add(iface.label.len())
                    .saturating_add(iface.class_id.len())
            })
            .sum::<usize>();
        let namespace_label_bytes = model
            .namespaces
            .values()
            .map(|namespace| {
                namespace
                    .id
                    .len()
                    .saturating_add(namespace.label.len())
                    .saturating_add(namespace.parent.as_deref().map(str::len).unwrap_or(0))
            })
            .sum::<usize>();

        Self {
            nodes: model
                .classes
                .len()
                .saturating_add(model.notes.len())
                .saturating_add(model.interfaces.len())
                .saturating_add(model.namespaces.len()),
            edges: model.relations.len().saturating_add(
                model
                    .notes
                    .iter()
                    .filter(|note| note.class_id.is_some())
                    .count(),
            ),
            namespaces: model.namespaces.len(),
            label_bytes: class_label_bytes
                .saturating_add(relation_label_bytes)
                .saturating_add(note_label_bytes)
                .saturating_add(interface_label_bytes)
                .saturating_add(namespace_label_bytes),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimitPhase {
    Source,
    LayoutModel,
    SvgOutput,
    SvgPostprocess,
}

impl ResourceLimitPhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::LayoutModel => "layout_model",
            Self::SvgOutput => "svg_output",
            Self::SvgPostprocess => "svg_postprocess",
        }
    }
}

impl std::fmt::Display for ResourceLimitPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("resource limit exceeded during {phase}: {limit} actual={actual} max={max}")]
pub struct ResourceLimitExceeded {
    pub phase: ResourceLimitPhase,
    pub limit: &'static str,
    pub actual: usize,
    pub max: usize,
}

fn optional_str_len(value: Option<&str>) -> usize {
    value.map(str::len).unwrap_or(0)
}

fn check_limit(
    phase: ResourceLimitPhase,
    limit: &'static str,
    actual: usize,
    max: Option<usize>,
) -> Result<(), ResourceLimitExceeded> {
    let Some(max) = max else {
        return Ok(());
    };
    if actual <= max {
        return Ok(());
    }
    Err(ResourceLimitExceeded {
        phase,
        limit,
        actual,
        max,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph};

    #[test]
    fn source_limit_reports_structured_error() {
        let err = RenderResourceLimits {
            max_source_bytes: Some(4),
            ..RenderResourceLimits::unbounded_for_trusted_input()
        }
        .check_source_bytes("12345")
        .unwrap_err();

        assert_eq!(err.phase, ResourceLimitPhase::Source);
        assert_eq!(err.limit, "max_source_bytes");
        assert_eq!(err.actual, 5);
        assert_eq!(err.max, 4);
    }

    #[test]
    fn flowchart_complexity_counts_layout_nodes_and_labels() {
        let model = FlowchartV2Model {
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: None,
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: vec![FlowNode {
                id: "A".to_string(),
                label: Some("Alpha".to_string()),
                label_type: None,
                layout_shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            }],
            edges: vec![FlowEdge {
                id: "L-A-B".to_string(),
                from: "A".to_string(),
                to: "B".to_string(),
                label: Some("edge".to_string()),
                label_type: None,
                edge_type: None,
                stroke: None,
                interpolate: None,
                classes: Vec::new(),
                style: Vec::new(),
                animate: None,
                animation: None,
                length: 1,
            }],
            subgraphs: vec![FlowSubgraph {
                id: "cluster".to_string(),
                title: "Cluster".to_string(),
                dir: None,
                has_explicit_dir: false,
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: vec!["A".to_string()],
            }],
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        };

        let complexity = FlowchartComplexity::from_model(&model);
        assert_eq!(complexity.nodes, 2);
        assert_eq!(complexity.edges, 1);
        assert_eq!(complexity.subgraphs, 1);
        assert!(complexity.label_bytes >= "AlphaedgeCluster".len());
    }
}
