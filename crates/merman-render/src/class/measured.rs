//! Shared Class text and box measurements, written into the selected layout input.

#[cfg(feature = "layout-elk")]
use super::class_arrow_type_for_relation_end;
use super::{
    ClassBoxMeasureCtx, ClassDiagramModel, ClassLayoutGraph, ClassNodeLabelPlan,
    ClassPreparedLabels, ClassPreparedSvgEdgeLabel, ClassRenderConfig, ClassTextThemePlan,
    EdgeTerminalMetrics, MeasuredClassDiagram, class_box_dimensions, class_cardinality_text_style,
    class_layout_parent, class_namespace_ids_in_decl_order, class_namespace_label,
    class_requires_math, normalize_dir, note_dimensions, plain_label_metrics, prepare_edge_title,
    validate_class_namespace_hierarchy,
};
use crate::entities::decode_entities_minimal;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::text::{TextMeasurer, WrapMode};
use crate::{Error, Result};
use indexmap::IndexMap;
use merman_model::families::class::ClassRelation;
use merman_model::presentation::PresentationConfig;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

#[cfg(feature = "layout-shoal")]
mod shoal;
#[cfg(feature = "layout-shoal")]
pub(super) use shoal::measure_for_shoal;

#[derive(Default)]
pub(super) struct MeasuredLabels {
    pub(super) class_label_plans_by_id: FxHashMap<String, Arc<ClassNodeLabelPlan>>,
    pub(super) prepared_labels: ClassPreparedLabels,
    pub(super) node_label_metrics_by_id: HashMap<String, (f64, f64)>,
}

struct RelationTerminals<'a> {
    relation: &'a ClassRelation,
    start_right: Option<(f64, f64)>,
    end_left: Option<(f64, f64)>,
}

trait MeasurementTarget<'a> {
    fn namespace(&mut self, id: &'a str, parent: Option<&'a str>, title: (f64, f64)) -> Result<()>;

    fn node(&mut self, id: &'a str, parent: Option<&'a str>, size: (f64, f64)) -> Result<()>;

    fn edge(
        &mut self,
        from: &'a str,
        to: &'a str,
        id: Cow<'a, str>,
        size: (f64, f64),
        terminals: Option<RelationTerminals<'a>>,
    ) -> Result<()>;
}

fn positive_label_size((width, height): (f64, f64)) -> Option<(f64, f64)> {
    (width > 0.0 && height > 0.0).then_some((width, height))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Class measurement retains operation-owned labels alongside the layout input."
)]
pub(super) fn measure_class_diagram(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render: &ClassRenderConfig,
    uses_elk_adapter: bool,
    typography: &ClassTextThemePlan,
    work: Option<&OperationLayoutWorkControl>,
) -> Result<MeasuredClassDiagram> {
    let settings = render.layout_settings();
    let mut target = LegacyMeasurement {
        graph: MeasuredGraph {
            direction: normalize_dir(&model.model().direction),
            nodesep: settings.nodesep,
            ranksep: settings.ranksep,
            nodes: IndexMap::new(),
            parents: IndexMap::new(),
            edges: IndexMap::new(),
        },
        namespace_labels: HashMap::new(),
    };
    let labels = measure_into(
        model,
        config,
        measurer,
        math_renderer,
        render,
        uses_elk_adapter,
        typography,
        work,
        &mut target,
    )?;
    Ok(MeasuredClassDiagram {
        graph: target.graph,
        class_label_plans_by_id: labels.class_label_plans_by_id,
        prepared_labels: labels.prepared_labels,
        node_label_metrics_by_id: labels.node_label_metrics_by_id,
        namespace_label_metrics_by_id: target.namespace_labels,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "Both layout targets share one traversal with the operation's render inputs."
)]
fn measure_into<'a>(
    model: &'a ClassDiagramModel,
    mermaid_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render_config: &ClassRenderConfig,
    uses_elk_adapter: bool,
    typography_theme: &ClassTextThemePlan,
    work_control: Option<&OperationLayoutWorkControl>,
    target: &mut impl MeasurementTarget<'a>,
) -> Result<MeasuredLabels> {
    validate_class_namespace_hierarchy(model)?;
    let settings = render_config.layout_settings();
    let wrap_mode_node = settings.wrap_mode_node;
    let wrap_mode_label = settings.wrap_mode_label;
    let wrap_mode_note = settings.wrap_mode_note;
    let text_style = render_config.text_style();
    let cardinality_text_style = class_cardinality_text_style(text_style);
    let contains_math = class_requires_math(model);
    let capture_row_metrics = matches!(wrap_mode_node, WrapMode::HtmlLike) || contains_math;
    let capture_note_label_metrics = matches!(wrap_mode_note, WrapMode::HtmlLike) || contains_math;
    let note_html_config = capture_note_label_metrics.then_some(mermaid_config);
    let mut labels = MeasuredLabels::default();
    let mut class_node_style_facts_by_id = BTreeMap::new();

    let class_box_measure_ctx = ClassBoxMeasureCtx {
        measurer,
        mermaid_config,
        math_renderer,
        text_style,
        inherited_font_size: typography_theme.visible_font_size_px(),
        html_calc_text_style: render_config.html_calc_text_style(),
        wrap_probe_font_size: settings.wrap_probe_font_size,
        wrap_mode: wrap_mode_node,
        padding: settings.class_padding,
        hide_empty_members_box: settings.hide_empty_members_box,
        capture_row_metrics,
        capture_style_facts: typography_theme.requires_node_style_facts(),
    };

    // Mermaid's ClassDB.getData() emits namespaces before classes, notes, and interfaces.
    for id in class_namespace_ids_in_decl_order(model) {
        let title = plain_label_metrics(
            class_namespace_label(model, id),
            measurer,
            text_style,
            if render_config.edge_use_html_labels {
                WrapMode::HtmlLike
            } else {
                WrapMode::SvgLike
            },
            mermaid_config,
            math_renderer,
        );
        let parent = class_layout_parent(
            model,
            model
                .namespace(id)
                .and_then(|ns| ns.namespace.parent.as_deref()),
        );
        target.namespace(id, parent, title)?;
    }

    for (_, c) in model.classes() {
        if let Some(work_control) = work_control {
            work_control.checked_add(0, 0)?;
        }
        let (w, h, label_plan, style_facts, svg_title) =
            class_box_dimensions(c.node, &class_box_measure_ctx);
        if let Some(mut label_plan) = label_plan {
            let id = c.id.clone();
            if let Some(work_control) = work_control {
                label_plan.reserve_retained_bytes(id.capacity(), work_control)?;
            }
            labels
                .class_label_plans_by_id
                .insert(id, Arc::new(label_plan));
        }
        if let Some(mut prepared) = svg_title {
            if let Some(work_control) = work_control {
                prepared.reserve_retained_bytes(&c.id, work_control)?;
            }
            labels
                .prepared_labels
                .titles_by_id
                .insert(c.id.clone(), prepared);
        }
        if class_box_measure_ctx.capture_style_facts {
            class_node_style_facts_by_id.insert(c.id.as_str().into(), style_facts);
        }
        target.node(
            c.node.id.as_str(),
            class_layout_parent(model, c.parent),
            (w, h),
        )?;
    }
    if typography_theme.requires_node_style_facts()
        && !typography_theme.seal_node_style_facts(class_node_style_facts_by_id)
    {
        return Err(Error::InvalidModel {
            message: "Class label style facts changed after layout preparation".to_string(),
        });
    }

    for note in model.notes() {
        let (w, h, metrics) = note_dimensions(
            &note.text,
            measurer,
            text_style,
            wrap_mode_note,
            settings.note_padding,
            note_html_config,
            math_renderer,
        );
        if capture_note_label_metrics {
            labels.node_label_metrics_by_id.insert(
                note.id.to_owned(),
                (metrics.width.max(0.0), metrics.height.max(0.0)),
            );
        }
        target.node(
            note.id,
            class_layout_parent(model, note.parent),
            (w.max(1.0), h.max(1.0)),
        )?;
    }

    // Interfaces remain at the root even when their related class belongs to a namespace.
    // squareRect adds fixed Neo label padding; classic interfaces have no node padding.
    let (interface_padding_x, interface_padding_y) = if render_config.look == "neo" {
        (32.0, 24.0)
    } else {
        (0.0, 0.0)
    };
    for iface in model.interfaces() {
        let label = decode_entities_minimal(iface.label.trim());
        let metrics = crate::graph_label::flowchart_label_metrics_for_layout(
            crate::graph_label::FlowchartLabelMetricsRequest {
                measurer,
                raw_label: &label,
                label_type: "text",
                style: text_style,
                max_width_px: Some(render_config.interface_wrapping_width),
                wrap_mode: wrap_mode_node,
                config: mermaid_config,
                math_renderer,
            },
        );
        let (tw, th) = (metrics.width.max(0.0), metrics.height.max(0.0));
        labels
            .node_label_metrics_by_id
            .insert(iface.id.to_owned(), (tw, th));
        target.node(
            iface.id,
            None,
            (
                (tw + interface_padding_x).max(1.0),
                (th + interface_padding_y).max(1.0),
            ),
        )?;
    }

    // Note attachments precede class relations and keep the note declaration index in their ID.
    for (i, note) in model.notes().enumerate() {
        let Some(class_id) = note.note.class_id.as_deref() else {
            continue;
        };
        if model.class(class_id).is_none() {
            continue;
        }
        target.edge(
            note.id,
            class_id,
            Cow::Owned(format!("edgeNote{i}")),
            (0.0, 0.0),
            None,
        )?;
    }

    // SVG terminals emit literal text; only HTML terminals render delimited math.
    let terminal_math_renderer = render_config
        .edge_use_html_labels
        .then_some(math_renderer)
        .flatten();
    for rel in model.relations() {
        if let Some(work_control) = work_control {
            work_control.checked_add(0, 0)?;
        }
        let title = if uses_elk_adapter {
            crate::text::mermaid_html_breaks_to_newlines(&rel.title)
        } else {
            Cow::Borrowed(rel.title.as_str())
        };
        let (lw, lh, prepared) = prepare_edge_title(
            &title,
            measurer,
            text_style,
            wrap_mode_label,
            mermaid_config,
            math_renderer,
        );
        if let Some(mut label) = prepared {
            let bytes = std::mem::size_of::<(String, ClassPreparedSvgEdgeLabel)>()
                .saturating_add(std::mem::size_of::<usize>())
                .saturating_add(rel.id.len())
                .saturating_add(label.label.retained_heap_bytes());
            let retained_reservation = work_control
                .map(|work_control| work_control.reserve_prepared_text_retained_bytes(bytes))
                .transpose()?;
            label._retained_reservation = retained_reservation;
            labels
                .prepared_labels
                .edges_by_id
                .insert(rel.id.clone(), label);
        }

        let measure_terminal = |text: Option<&str>| {
            positive_label_size(plain_label_metrics(
                text.unwrap_or_default(),
                measurer,
                &cardinality_text_style,
                wrap_mode_label,
                mermaid_config,
                terminal_math_renderer,
            ))
        };
        target.edge(
            rel.id1,
            rel.id2,
            Cow::Borrowed(rel.source.id.as_str()),
            (lw, lh),
            Some(RelationTerminals {
                relation: rel.source,
                start_right: measure_terminal(rel.relation_title_1.as_deref()),
                end_left: measure_terminal(rel.relation_title_2.as_deref()),
            }),
        )?;
    }

    Ok(labels)
}

struct LegacyMeasurement {
    graph: MeasuredGraph,
    namespace_labels: HashMap<String, (f64, f64)>,
}

impl<'a> MeasurementTarget<'a> for LegacyMeasurement {
    fn namespace(&mut self, id: &str, parent: Option<&str>, title: (f64, f64)) -> Result<()> {
        self.graph.set_node(
            id.to_owned(),
            MeasuredNode {
                is_namespace: true,
                ..Default::default()
            },
        );
        self.namespace_labels.insert(id.to_owned(), title);
        if let Some(parent) = parent {
            self.graph.set_parent(id.to_owned(), parent.to_owned());
        }
        Ok(())
    }

    fn node(&mut self, id: &str, parent: Option<&str>, (width, height): (f64, f64)) -> Result<()> {
        self.graph.set_node(
            id.to_owned(),
            MeasuredNode {
                width,
                height,
                ..Default::default()
            },
        );
        if let Some(parent) = parent {
            self.graph.set_parent(id.to_owned(), parent.to_owned());
        }
        Ok(())
    }

    fn edge(
        &mut self,
        from: &str,
        to: &str,
        id: Cow<'a, str>,
        (width, height): (f64, f64),
        terminals: Option<RelationTerminals<'a>>,
    ) -> Result<()> {
        // Mermaid's arrow type strings are truthy even for `none`. Dagre's terminal placement
        // reserves the 10px offset on each end with an authored label.
        let marker_offset = |label: Option<&str>| {
            if label.is_some_and(|label| !label.trim().is_empty()) {
                10.0
            } else {
                0.0
            }
        };
        self.graph.set_edge(
            from.to_owned(),
            to.to_owned(),
            id.into_owned(),
            MeasuredEdge {
                width,
                height,
                terminals: terminals.map(|terminals| EdgeTerminalMetrics {
                    start_left: None,
                    start_right: terminals.start_right,
                    end_left: terminals.end_left,
                    end_right: None,
                    start_marker: marker_offset(terminals.relation.relation_title_1.as_deref()),
                    end_marker: marker_offset(terminals.relation.relation_title_2.as_deref()),
                    #[cfg(feature = "layout-elk")]
                    start_arrow_type: class_arrow_type_for_relation_end(
                        terminals.relation.relation.type1,
                    ),
                    #[cfg(feature = "layout-elk")]
                    end_arrow_type: class_arrow_type_for_relation_end(
                        terminals.relation.relation.type2,
                    ),
                }),
            },
        );
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct MeasuredNode {
    pub width: f64,
    pub height: f64,
    pub is_namespace: bool,
}

pub(super) struct MeasuredEdge {
    pub width: f64,
    pub height: f64,
    pub terminals: Option<EdgeTerminalMetrics>,
}

/// Legacy Dagre and ELK input, including Graphlib's insertion and parent mutation order.
pub(super) struct MeasuredGraph {
    pub direction: String,
    pub nodesep: f64,
    pub ranksep: f64,
    pub nodes: IndexMap<String, MeasuredNode>,
    // Parent references may insert nodes before their declarations. Preserve hierarchy mutation
    // order separately so those placeholders do not reorder a namespace's declared children.
    pub parents: IndexMap<String, String>,
    // Source, target, and name together identify a Graphlib multiedge.
    pub edges: IndexMap<(String, String, String), MeasuredEdge>,
}

impl MeasuredGraph {
    pub fn set_node(&mut self, id: String, mut node: MeasuredNode) {
        if let Some(existing) = self.nodes.get(&id) {
            node.is_namespace |= existing.is_namespace;
        }
        self.nodes.insert(id, node);
    }

    pub fn set_parent(&mut self, child: String, parent: String) {
        // Graphlib inserts a referenced parent immediately, even before its declaration.
        self.nodes.entry(child.clone()).or_default();
        self.nodes.entry(parent.clone()).or_default();
        self.parents.insert(child, parent);
    }

    pub fn set_edge(&mut self, source: String, target: String, id: String, edge: MeasuredEdge) {
        self.nodes.entry(source.clone()).or_default();
        self.nodes.entry(target.clone()).or_default();
        self.edges.insert((source, target, id), edge);
    }

    pub fn to_dagre(&self) -> ClassLayoutGraph {
        use dugong::graphlib::{Graph, GraphOptions};
        use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};

        let mut graph = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        graph.set_graph(GraphLabel {
            rankdir: super::rank_dir_from(&self.direction),
            nodesep: self.nodesep,
            ranksep: self.ranksep,
            // The SVG viewport adds Mermaid's graph margins after layout.
            marginx: 0.0,
            marginy: 0.0,
            ..Default::default()
        });
        for (id, node) in &self.nodes {
            graph.set_node(
                id.clone(),
                NodeLabel {
                    width: node.width,
                    height: node.height,
                    ..Default::default()
                },
            );
        }
        for (child, parent) in &self.parents {
            graph.set_parent(child.clone(), parent.clone());
        }
        for ((source, target, id), edge) in &self.edges {
            let mut label = EdgeLabel {
                width: edge.width,
                height: edge.height,
                labelpos: LabelPos::C,
                labeloffset: 10.0,
                minlen: 1,
                weight: 1.0,
                ..Default::default()
            };
            if let Some(meta) = &edge.terminals {
                for (key, metrics) in [
                    ("startLeft", meta.start_left),
                    ("startRight", meta.start_right),
                    ("endLeft", meta.end_left),
                    ("endRight", meta.end_right),
                ] {
                    if let Some((width, height)) = metrics {
                        super::set_extras_label_metrics(&mut label.extras, key, width, height);
                    }
                }
                label
                    .extras
                    .insert("startMarker".to_string(), meta.start_marker.into());
                label
                    .extras
                    .insert("endMarker".to_string(), meta.end_marker.into());
            }
            graph.set_edge_named(
                source.clone(),
                target.clone(),
                Some(id.clone()),
                Some(label),
            );
        }
        graph
    }
}

#[cfg(test)]
mod tests {
    use super::{MeasuredGraph, MeasuredNode};
    use indexmap::IndexMap;

    #[test]
    fn forward_namespace_parents_do_not_reorder_declared_children() {
        let mut measured = MeasuredGraph {
            direction: "TB".into(),
            nodesep: 50.0,
            ranksep: 50.0,
            nodes: IndexMap::new(),
            parents: IndexMap::new(),
            edges: IndexMap::new(),
        };
        for (child, parent) in [("Child", "Parent"), ("Sibling", "Root"), ("Parent", "Root")] {
            measured.set_node(
                child.into(),
                MeasuredNode {
                    is_namespace: true,
                    ..Default::default()
                },
            );
            measured.set_parent(child.into(), parent.into());
        }
        measured.set_node(
            "Root".into(),
            MeasuredNode {
                is_namespace: true,
                ..Default::default()
            },
        );

        let graph = measured.to_dagre();
        assert_eq!(graph.node_ids(), ["Child", "Parent", "Sibling", "Root"]);
        assert_eq!(graph.children("Root"), ["Sibling", "Parent"]);
        assert_eq!(graph.parent("Child"), Some("Parent"));
        assert_eq!(graph.parent("Parent"), Some("Root"));
    }
}
