use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::diagrams::zenuml::{ZenumlDiagramRenderModel, ZenumlStatementKind};
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
    pub max_zenuml_participants: Option<usize>,
    pub max_zenuml_statements: Option<usize>,
    pub max_zenuml_fragments: Option<usize>,
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
            max_zenuml_participants: Some(8_000),
            max_zenuml_statements: Some(16_000),
            max_zenuml_fragments: Some(2_000),
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
            max_zenuml_participants: Some(4_000),
            max_zenuml_statements: Some(8_000),
            max_zenuml_fragments: Some(1_000),
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
            max_zenuml_participants: Some(50_000),
            max_zenuml_statements: Some(100_000),
            max_zenuml_fragments: Some(10_000),
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
            max_zenuml_participants: None,
            max_zenuml_statements: None,
            max_zenuml_fragments: None,
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
        model: &FlowchartModel,
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

    pub fn check_zenuml_complexity(
        &self,
        model: &ZenumlDiagramRenderModel,
    ) -> Result<ZenumlComplexity, ResourceLimitExceeded> {
        let complexity = ZenumlComplexity::from_model(model);
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_zenuml_participants",
            complexity.participants,
            self.max_zenuml_participants,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_zenuml_statements",
            complexity.statements,
            self.max_zenuml_statements,
        )?;
        check_limit(
            ResourceLimitPhase::LayoutModel,
            "max_zenuml_fragments",
            complexity.fragments,
            self.max_zenuml_fragments,
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
    pub fn from_model(model: &FlowchartModel) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZenumlComplexity {
    pub participants: usize,
    pub statements: usize,
    pub fragments: usize,
    pub label_bytes: usize,
}

impl ZenumlComplexity {
    pub fn from_model(model: &ZenumlDiagramRenderModel) -> Self {
        let common_label_bytes = [
            model.title.as_deref(),
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.starter.as_deref(),
        ]
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
            group
                .participant_names
                .iter()
                .fold(total.saturating_add(group.id.len()), |subtotal, name| {
                    subtotal.saturating_add(name.len())
                })
        });
        let mut complexity = Self {
            participants: model.participants.len(),
            statements: 0,
            fragments: 0,
            label_bytes: common_label_bytes
                .saturating_add(participant_label_bytes)
                .saturating_add(group_label_bytes),
        };
        let mut pending = vec![model.statements.as_slice()];
        while let Some(statements) = pending.pop() {
            for statement in statements {
                complexity.statements = complexity.statements.saturating_add(1);
                complexity.label_bytes = complexity
                    .label_bytes
                    .saturating_add(statement.comment.as_deref().map_or(0, str::len));
                match &statement.kind {
                    ZenumlStatementKind::Message {
                        from,
                        to,
                        label,
                        assignment,
                        body,
                        ..
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(from.len())
                            .saturating_add(to.len())
                            .saturating_add(label.len())
                            .saturating_add(assignment.as_deref().map_or(0, str::len));
                        pending.push(body);
                    }
                    ZenumlStatementKind::Creation {
                        from,
                        to,
                        constructor,
                        assignment,
                        label,
                        body,
                        ..
                    } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(from.len())
                            .saturating_add(to.len())
                            .saturating_add(constructor.len())
                            .saturating_add(assignment.as_deref().map_or(0, str::len))
                            .saturating_add(label.len());
                        pending.push(body);
                    }
                    ZenumlStatementKind::Return { from, to, label } => {
                        complexity.label_bytes = complexity
                            .label_bytes
                            .saturating_add(from.len())
                            .saturating_add(to.len())
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
                            pending.push(&section.statements);
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
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};

    struct ZenumlLimitCase {
        name: &'static str,
        configure: fn(&mut RenderResourceLimits),
    }

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
    fn zenuml_complexity_includes_common_and_inline_decorations() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "zenuml\naccTitle: Access title\naccDescr: Access description\nA->[rocket]B.call()\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model else {
            panic!("expected ZenUML model");
        };
        let complexity = ZenumlComplexity::from_model(&model);

        assert_eq!(complexity.participants, 2);
        assert_eq!(complexity.statements, 1);
        let required = ["Access title", "Access description", "rocket", "call()"]
            .into_iter()
            .map(str::len)
            .sum::<usize>();
        assert!(complexity.label_bytes >= required);
    }

    #[test]
    fn zenuml_structural_limits_are_owned_and_independent() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "zenuml\nA.call() {\n  if(ok) {\n    B.work()\n  }\n}\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Zenuml(model) = parsed.model else {
            panic!("expected ZenUML model");
        };

        let cases = [
            ZenumlLimitCase {
                name: "max_zenuml_participants",
                configure: |limits| limits.max_zenuml_participants = Some(1),
            },
            ZenumlLimitCase {
                name: "max_zenuml_statements",
                configure: |limits| limits.max_zenuml_statements = Some(1),
            },
            ZenumlLimitCase {
                name: "max_zenuml_fragments",
                configure: |limits| limits.max_zenuml_fragments = Some(0),
            },
        ];
        for case in cases {
            let mut limits = RenderResourceLimits::unbounded_for_trusted_input();
            (case.configure)(&mut limits);
            let error = limits.check_zenuml_complexity(&model).unwrap_err();
            assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
            assert_eq!(error.limit, case.name);
        }
    }

    #[test]
    fn flowchart_complexity_counts_layout_nodes_and_labels() {
        let model = FlowchartModel {
            keyword: "graph".to_string(),
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
                shape: None,
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
                arrow: "-->".to_string(),
                is_user_defined_id: false,
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
