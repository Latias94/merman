//! Shared Class text and box measurements, written into the selected layout input.

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
use super::EdgeTerminalMetrics;
#[cfg(feature = "layout-elk")]
use super::class_arrow_type_for_relation_end;
use super::{
    ClassBoxMeasureCtx, ClassDiagramModel, ClassNodeLabelPlan, ClassPreparedLabels,
    ClassPreparedSvgEdgeLabel, ClassRenderConfig, class_box_dimensions,
    class_cardinality_text_style, class_layout_parent, class_namespace_ids_in_decl_order,
    class_namespace_label, class_requires_math, note_dimensions, plain_label_metrics,
    prepare_edge_title, validate_class_namespace_hierarchy,
};
#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
use indexmap::IndexMap;
use merman_model::families::class::ClassRelation;
use merman_model::presentation::PresentationConfig;
use merman_render_core::Result;
use merman_render_core::entities::decode_entities_minimal;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
use merman_render_core::graph_layout::{Direction, direction_from_rank};
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::math::MathRenderer;
#[cfg(feature = "layout-grid")]
use merman_render_core::resources::OperationWorkMeter;
use merman_render_core::text::{TextMeasurer, WrapMode};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use merman_render_core::text::create_text_html::FlowchartLabelMetricsRequest;

/// Interface labels use the caller's shared createText layout metrics.
pub type MeasureInterfaceLabel =
    fn(FlowchartLabelMetricsRequest<'_>) -> merman_render_core::text::TextMetrics;

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
pub struct MeasuredClassDiagram {
    pub graph: MeasuredGraph,
    pub class_label_plans_by_id: FxHashMap<String, Arc<ClassNodeLabelPlan>>,
    pub prepared_labels: ClassPreparedLabels,
    pub node_label_metrics_by_id: HashMap<String, (f64, f64)>,
    pub namespace_label_metrics_by_id: HashMap<String, (f64, f64)>,
}

#[derive(Default)]
pub struct MeasuredLabels {
    pub class_label_plans_by_id: FxHashMap<String, Arc<ClassNodeLabelPlan>>,
    pub prepared_labels: ClassPreparedLabels,
    pub node_label_metrics_by_id: HashMap<String, (f64, f64)>,
}

pub struct RelationTerminals<'a> {
    pub relation: &'a ClassRelation,
    pub start_right: Option<(f64, f64)>,
    pub end_left: Option<(f64, f64)>,
}

pub trait MeasurementTarget<'a> {
    #[cfg(feature = "layout-grid")]
    fn grid_label_work(&self) -> Option<(&OperationWorkMeter, &OperationLayoutWorkControl)> {
        None
    }

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

pub fn positive_label_size((width, height): (f64, f64)) -> Option<(f64, f64)> {
    (width > 0.0 && height > 0.0).then_some((width, height))
}

#[allow(
    clippy::too_many_arguments,
    reason = "Class measurement retains operation-owned labels alongside the layout input."
)]
#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
pub fn measure_class_diagram(
    model: &ClassDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render: &ClassRenderConfig,
    uses_elk_adapter: bool,
    measure_interface_label: MeasureInterfaceLabel,
    work: Option<&OperationLayoutWorkControl>,
) -> Result<MeasuredClassDiagram> {
    let settings = render.layout_settings();
    let mut target = OrderedMeasurement {
        graph: MeasuredGraph {
            direction: direction_from_rank(&model.model().direction),
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
        measure_interface_label,
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
    reason = "Class layout targets share one traversal with the operation's render inputs."
)]
pub fn measure_into<'a>(
    model: &'a ClassDiagramModel,
    mermaid_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render_config: &ClassRenderConfig,
    uses_elk_adapter: bool,
    measure_interface_label: MeasureInterfaceLabel,
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

    let class_box_measure_ctx = ClassBoxMeasureCtx {
        measurer,
        mermaid_config,
        math_renderer,
        text_style,
        code_text_style: render_config.code_text_style(),
        native_title_style: render_config
            .native_style
            .as_ref()
            .map(|native| &native.title_style),
        html_calc_text_style: render_config.html_calc_text_style(),
        wrap_probe_font_size: settings.wrap_probe_font_size,
        wrap_mode: wrap_mode_node,
        padding: settings.class_padding,
        hide_empty_members_box: settings.hide_empty_members_box,
        capture_row_metrics,
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
        let (w, h, label_plan, svg_title) = class_box_dimensions(c.node, &class_box_measure_ctx);
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
        target.node(
            c.node.id.as_str(),
            class_layout_parent(model, c.parent),
            (w, h),
        )?;
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
        let metrics = measure_interface_label(FlowchartLabelMetricsRequest {
            measurer,
            raw_label: &label,
            label_type: "text",
            style: text_style,
            max_width_px: Some(render_config.interface_wrapping_width),
            wrap_mode: wrap_mode_node,
            config: mermaid_config,
            math_renderer,
        });
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
            merman_render_core::text::line_break::mermaid_html_breaks_to_newlines(&rel.title)
        } else {
            Cow::Borrowed(rel.title.as_str())
        };
        let prepare_regular_title = || {
            prepare_edge_title(
                &title,
                measurer,
                text_style,
                wrap_mode_label,
                mermaid_config,
                math_renderer,
            )
        };
        #[cfg(not(feature = "layout-grid"))]
        let (lw, lh, prepared) = prepare_regular_title();
        #[cfg(feature = "layout-grid")]
        let grid_work = target.grid_label_work();
        #[cfg(feature = "layout-grid")]
        let grid = grid_work.is_some();
        #[cfg(feature = "layout-grid")]
        let (lw, lh, prepared) = if let Some((meter, work)) = grid_work {
            if rel.title.is_empty() {
                (0.0, 0.0, None)
            } else {
                let label = super::grid::prepare_label_rect(
                    &rel.title,
                    &rel.id,
                    mermaid_config,
                    render_config,
                    measurer,
                    math_renderer,
                    work,
                    meter,
                )?;
                let size = label.size;
                labels
                    .prepared_labels
                    .grid_by_id
                    .insert(rel.id.clone(), label);
                (size.width, size.height, None)
            }
        } else {
            prepare_regular_title()
        };
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
            #[cfg(feature = "layout-grid")]
            let text = text.filter(|&text| !grid || text != "none");
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

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
struct OrderedMeasurement {
    graph: MeasuredGraph,
    namespace_labels: HashMap<String, (f64, f64)>,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
impl<'a> MeasurementTarget<'a> for OrderedMeasurement {
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
        // Mermaid's fallback terminal placement reserves 10px for each authored cardinality,
        // including when its arrow type is `none`.
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

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
#[derive(Default)]
pub struct MeasuredNode {
    pub width: f64,
    pub height: f64,
    pub is_namespace: bool,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
pub struct MeasuredEdge {
    pub width: f64,
    pub height: f64,
    pub terminals: Option<EdgeTerminalMetrics>,
}

/// Ordered Class measurements shared by the Dagre and ELK adapters.
#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
pub struct MeasuredGraph {
    pub direction: Direction,
    pub nodesep: f64,
    pub ranksep: f64,
    pub nodes: IndexMap<String, MeasuredNode>,
    // Parent references may insert nodes before their declarations. Preserve hierarchy mutation
    // order separately so those placeholders do not reorder a namespace's declared children.
    pub parents: IndexMap<String, String>,
    // A named edge is scoped to its ordered pair of endpoints.
    pub edges: IndexMap<(String, String, String), MeasuredEdge>,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
impl MeasuredGraph {
    pub fn set_node(&mut self, id: String, mut node: MeasuredNode) {
        if let Some(existing) = self.nodes.get(&id) {
            node.is_namespace |= existing.is_namespace;
        }
        self.nodes.insert(id, node);
    }

    pub fn set_parent(&mut self, child: String, parent: String) {
        // A forward reference reserves the parent's slot until its declaration.
        self.nodes.entry(child.clone()).or_default();
        self.nodes.entry(parent.clone()).or_default();
        self.parents.insert(child, parent);
    }

    pub fn set_edge(&mut self, source: String, target: String, id: String, edge: MeasuredEdge) {
        self.nodes.entry(source.clone()).or_default();
        self.nodes.entry(target.clone()).or_default();
        self.edges.insert((source, target, id), edge);
    }
}
