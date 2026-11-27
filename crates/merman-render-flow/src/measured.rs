//! Measured leaf shapes and edge labels shared by Flowchart graph adapters.

#[cfg(any(
    feature = "layout-grid",
    feature = "layout-layered",
    feature = "layout-shoal"
))]
use merman_model::families::flowchart::FlowSubgraph;
use merman_model::{
    families::flowchart::{FlowEdge, FlowNode},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Result,
    config::mermaid_config_diagram_look,
    math::MathRenderer,
    text::{
        TextMeasurer, TextStyle, WrapMode, create_text_html::FlowchartLabelMetricsRequest,
        literal_label::FlowchartSvgWidthMode, measure_wrapped_markdown_with_inline_styles,
    },
};
#[cfg(any(
    feature = "layout-grid",
    feature = "layout-layered",
    feature = "layout-shoal"
))]
use merman_render_core::{config::config_diagram_look, text::TextMetrics};

#[cfg(feature = "layout-shoal")]
use crate::node_geometry::PreparedFlowchartNodeGeometry;
#[cfg(any(
    feature = "layout-grid",
    feature = "layout-layered",
    feature = "layout-shoal"
))]
use crate::style::flowchart_effective_text_style_for_classes_with_provenance;
use crate::{
    bpmn,
    class_style::{
        FlowchartClassDefinitions, FlowchartLabelDom, FlowchartStructuralLabelTypography,
    },
    layout_config::{FlowchartConfigView, FlowchartLayoutSettings},
    node_geometry::{
        NodeLayoutDimensionsRequest, flowchart_node_label_min_width, flowchart_node_svg_width_mode,
    },
    projection::{FlowchartEdgeKey, FlowchartRenderModelRef},
    style::{
        FlowchartEdgeStylePlan, flowchart_apply_html_node_class_box_metrics,
        flowchart_node_layout_style,
    },
    svg_label::{
        FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder,
        measure_flowchart_svg_label_for_layout_with_metrics_and_source_style,
        measure_flowchart_svg_label_for_layout_with_source_style,
    },
};

pub struct MeasuredLabel {
    pub width: f64,
    pub height: f64,
}

pub struct MeasuredNode {
    pub width: f64,
    pub height: f64,
    pub label: MeasuredLabel,
    #[cfg(feature = "layout-shoal")]
    pub geometry: Option<PreparedFlowchartNodeGeometry>,
}

#[cfg(feature = "layout-shoal")]
impl MeasuredNode {
    pub fn widen_decision(&mut self, padding: f64) {
        let (w, h) = (self.label.width + padding, self.label.height + padding);
        self.width = w + 1.6 * h;
        self.height = w / 1.6 + h;
    }
}

pub struct FlowchartMeasurement<'a> {
    model: FlowchartRenderModelRef<'a>,
    class_defs: &'a FlowchartClassDefinitions<'a>,
    effective_config: &'a PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &'a FlowchartEdgeStylePlan,
    pub settings: FlowchartLayoutSettings,
}

impl<'a> FlowchartMeasurement<'a> {
    pub fn new(
        model: FlowchartRenderModelRef<'a>,
        class_defs: &'a FlowchartClassDefinitions<'a>,
        effective_config: &'a PresentationConfig,
        measurer: &'a dyn TextMeasurer,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        edge_style_plan: &'a FlowchartEdgeStylePlan,
    ) -> Self {
        let settings = svg_label_sidecar.map_or_else(
            || FlowchartConfigView::new(effective_config).layout_settings(),
            |sidecar| sidecar.layout_settings(effective_config),
        );
        Self {
            model,
            class_defs,
            effective_config,
            measurer,
            math_renderer,
            svg_label_sidecar,
            edge_style_plan,
            settings,
        }
    }

    fn label_base_style(&self, mode: WrapMode) -> &TextStyle {
        if mode == WrapMode::HtmlLike {
            &self.settings.html_label_text_style
        } else {
            &self.settings.text_style
        }
    }

    #[cfg(feature = "layout-layered")]
    pub(crate) fn layered_spacing_scale(&self) -> f64 {
        self.svg_label_sidecar
            .and_then(FlowchartSvgLabelSidecarBuilder::native_density)
            .map_or(1.0, |density| {
                crate::layout_config::density_scales(density).1
            })
    }

    #[cfg(feature = "layout-shoal")]
    pub fn subgraph_title(
        &self,
        group: &FlowSubgraph,
        index: usize,
        charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<TextMetrics> {
        let label_type = group.label_type.as_deref().unwrap_or("text");
        let collapsed = self.model.is_subgraph_collapsed(&group.id);
        let mode = if collapsed {
            self.settings.node_wrap_mode
        } else {
            self.settings.cluster_wrap_mode
        };
        let max_width_px = if collapsed {
            Some(self.settings.wrapping_width)
        } else {
            (label_type == "markdown").then_some(self.settings.cluster_title_wrapping_width)
        };
        self.measure_subgraph_title(group, index, mode, max_width_px, charge)
    }

    #[cfg(any(feature = "layout-grid", feature = "layout-layered"))]
    pub(crate) fn check_measurement<T>(&self, result: Result<T>) -> Result<T> {
        if let Some(labels) = self.svg_label_sidecar {
            labels.check_prepared_label_errors()?;
        }
        result
    }

    #[cfg(feature = "layout-grid")]
    pub(crate) fn grid_subgraph_title(
        &self,
        group: &FlowSubgraph,
        index: usize,
        charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<TextMetrics> {
        // Grid uses createGraph's default measureGroupLabel and the normal labelHelper width.
        self.measure_subgraph_title(
            group,
            index,
            self.settings.node_wrap_mode,
            Some(self.settings.wrapping_width),
            charge,
        )
    }

    #[cfg(feature = "layout-layered")]
    pub(crate) fn layered_subgraph_title(
        &self,
        group: &FlowSubgraph,
        index: usize,
        charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<TextMetrics> {
        let collapsed = self.model.is_subgraph_collapsed(&group.id);
        let mode = if collapsed {
            self.settings.node_wrap_mode
        } else {
            self.settings.cluster_wrap_mode
        };
        let max_width_px = (collapsed || group.label_type.as_deref() == Some("markdown"))
            .then_some(self.settings.wrapping_width);
        self.measure_subgraph_title(group, index, mode, max_width_px, charge)
    }

    #[cfg(any(
        feature = "layout-grid",
        feature = "layout-layered",
        feature = "layout-shoal"
    ))]
    fn measure_subgraph_title(
        &self,
        group: &FlowSubgraph,
        index: usize,
        mode: WrapMode,
        max_width_px: Option<f64>,
        charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<TextMetrics> {
        let title = self.model.subgraph_title_for_render(index, group);
        let label_type = group.label_type.as_deref().unwrap_or("text");
        let collapsed = self.model.is_subgraph_collapsed(&group.id);
        let (classes, styles) = self.model.effective_subgraph_css(index, group);
        let structural = collapsed
            .then(|| {
                FlowchartStructuralLabelTypography::prepare(
                    self.class_defs,
                    self.settings.edge_html_labels,
                    FlowchartLabelDom::node(
                        "collapsedGroup",
                        title,
                        label_type,
                        mode == WrapMode::HtmlLike,
                        config_diagram_look(self.effective_config).is_hand_drawn(),
                    ),
                    charge,
                )
            })
            .transpose()?;
        let base = self.label_base_style(mode);
        let parent = structural
            .as_ref()
            .map_or(std::borrow::Cow::Borrowed(base), |structural| {
                structural.apply_to_parent(std::borrow::Cow::Borrowed(base))
            });
        let mut style = flowchart_effective_text_style_for_classes_with_provenance(
            parent.as_ref(),
            &self.model.class_defs,
            classes,
            styles,
        );
        if let Some(structural) = structural {
            structural.apply_to_content(&mut style.style);
        }
        let mut metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            self.svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::SubgraphTitle(index)),
            Some(&group.id),
            FlowchartLabelMetricsRequest {
                measurer: self.measurer,
                raw_label: title,
                label_type,
                style: style.as_ref(),
                max_width_px,
                wrap_mode: mode,
                config: self.effective_config,
                math_renderer: self.math_renderer,
            },
            Some(&style),
            FlowchartSvgWidthMode::Bbox,
        );
        if collapsed && mode == WrapMode::HtmlLike && self.settings.edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                title,
                label_type,
                style.as_ref(),
                &self.model.class_defs,
                classes,
            );
        }
        Ok(metrics)
    }

    #[cfg(feature = "layout-grid")]
    pub(crate) fn grid_edge_label(
        &self,
        edge: &FlowEdge,
        key: FlowchartEdgeKey,
    ) -> Result<Option<MeasuredLabel>> {
        let Some(label) = self
            .model
            .edge_label_for_render(key.semantic_index(), edge)
            .filter(|label| !label.is_empty())
        else {
            return Ok(None);
        };
        // Grid turns an edge label into its own labelRect node. It inherits node label mode and
        // wrapping width. The helper has no labelType field or ordinary edge-label padding.
        let mode = self.settings.node_wrap_mode;
        let html_class_defs = (mode == WrapMode::HtmlLike && self.settings.edge_html_labels)
            .then_some(self.class_defs);
        let style = self.edge_style_plan.grid_label_rect_text_style_for(
            key,
            self.label_base_style(mode),
            mode,
            html_class_defs,
        )?;
        let metrics_style = if mode == WrapMode::SvgLike {
            self.edge_style_plan
                .grid_label_rect_svg_bbox_style(key, style.as_ref())?
        } else {
            std::borrow::Cow::Borrowed(style.as_ref())
        };
        let metrics = measure_flowchart_svg_label_for_layout_with_metrics_and_source_style(
            self.svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::Edge(key.semantic_index())),
            Some(edge.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer: self.measurer,
                raw_label: label,
                label_type: "text",
                style: style.as_ref(),
                max_width_px: Some(self.settings.wrapping_width),
                wrap_mode: mode,
                config: self.effective_config,
                math_renderer: self.math_renderer,
            },
            &metrics_style,
            Some(&style),
            FlowchartSvgWidthMode::Bbox,
        );
        Ok(Some(MeasuredLabel {
            width: metrics.width.max(0.1),
            height: metrics.height.max(0.1),
        }))
    }

    pub fn node(
        &self,
        node: &FlowNode,
        owner: FlowchartSvgLabelOwner,
        direction: &str,
        mut charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<MeasuredNode> {
        let raw_label = self.model.node_label_for_render(node).unwrap_or(&node.id);
        let wrapping_width =
            bpmn::wrapping_width(node.layout_shape.as_deref(), self.settings.wrapping_width);
        let label_type = node.label_type.as_deref().unwrap_or("text");
        let look = mermaid_config_diagram_look(self.effective_config);
        let label_dom = FlowchartLabelDom::node(
            node.layout_shape.as_deref().unwrap_or("squareRect"),
            raw_label,
            label_type,
            self.settings.node_wrap_mode == WrapMode::HtmlLike,
            look.is_hand_drawn(),
        );
        let structural = FlowchartStructuralLabelTypography::prepare(
            self.class_defs,
            self.settings.edge_html_labels,
            label_dom,
            &mut charge,
        )?;
        let node_base = self.label_base_style(self.settings.node_wrap_mode);
        let node_parent = structural.apply_to_parent(std::borrow::Cow::Borrowed(node_base));
        let (mut node_text_style, rectangle_size) = flowchart_node_layout_style(
            node_parent.as_ref(),
            &self.model.class_defs,
            &node.classes,
            &node.styles,
        );
        structural.apply_to_content(&mut node_text_style.style);
        let svg_width_mode = flowchart_node_svg_width_mode(
            raw_label,
            label_type,
            self.settings.node_wrap_mode,
            node.layout_shape.as_deref().unwrap_or("squareRect"),
        );
        let mut metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            self.svg_label_sidecar,
            Some(owner),
            Some(node.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer: self.measurer,
                raw_label,
                label_type,
                style: node_text_style.as_ref(),
                max_width_px: Some(wrapping_width),
                wrap_mode: self.settings.node_wrap_mode,
                config: self.effective_config,
                math_renderer: self.math_renderer,
            },
            Some(&node_text_style),
            svg_width_mode,
        );
        if self.settings.node_wrap_mode == WrapMode::HtmlLike && self.settings.edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                raw_label,
                label_type,
                node_text_style.as_ref(),
                &self.model.class_defs,
                &node.classes,
            );
        }

        metrics = metrics.with_label_min_width(
            raw_label,
            flowchart_node_label_min_width(
                raw_label,
                node.layout_shape.as_deref(),
                self.effective_config,
            ),
            None,
        );
        let label = MeasuredLabel {
            width: metrics.width,
            height: metrics.height,
        };
        let ((width, height), _geometry) = NodeLayoutDimensionsRequest {
            layout_shape: node.layout_shape.as_deref(),
            layout_direction: direction,
            metrics,
            has_label: !raw_label.is_empty(),
            wrapping_width,
            node_constraint: node.constraint.as_deref(),
            padding: self.settings.node_padding,
            look_is_neo: look.is_neo(),
            state_padding: self.settings.state_padding,
            node_icon: node.icon.as_deref(),
            node_img: node.img.as_deref(),
            node_pos: node.pos.as_deref(),
            node_asset_width: node.asset_width,
            node_asset_height: node.asset_height,
        }
        .measure(
            rectangle_size,
            look.is_hand_drawn(),
            self.svg_label_sidecar.map(|builder| (builder, owner)),
            charge,
        )?;

        Ok(MeasuredNode {
            width,
            height,
            label,
            #[cfg(feature = "layout-shoal")]
            geometry: _geometry,
        })
    }

    pub fn edge_label(
        &self,
        edge: &FlowEdge,
        key: FlowchartEdgeKey,
    ) -> Result<Option<MeasuredLabel>> {
        self.measure_edge_label(edge, key, false).map(|label| {
            label.map(|label| MeasuredLabel {
                width: label.width.max(1.0),
                height: label.height.max(1.0),
            })
        })
    }

    #[cfg(feature = "layout-layered")]
    pub(crate) fn layered_edge_label(
        &self,
        edge: &FlowEdge,
        key: FlowchartEdgeKey,
    ) -> Result<Option<MeasuredLabel>> {
        self.measure_edge_label(edge, key, true)
    }

    fn measure_edge_label(
        &self,
        edge: &FlowEdge,
        key: FlowchartEdgeKey,
        reserve_nonblank_only: bool,
    ) -> Result<Option<MeasuredLabel>> {
        let owner = FlowchartSvgLabelOwner::Edge(key.semantic_index());
        let label_text = self
            .model
            .edge_label_for_render(key.semantic_index(), edge)
            .unwrap_or_default();
        let label_type = edge.label_type.as_deref().unwrap_or("text");
        if label_text.is_empty() {
            return Ok(None);
        }

        let edge_parent = self.label_base_style(self.settings.edge_wrap_mode);
        let edge_text_style = self
            .edge_style_plan
            .edge_label_text_style_for(key, edge_parent)?;
        let metrics =
            if label_type == "markdown" && self.settings.edge_wrap_mode != WrapMode::HtmlLike {
                measure_wrapped_markdown_with_inline_styles(
                    self.measurer,
                    label_text,
                    edge_text_style.as_ref(),
                    Some(self.settings.edge_label_wrapping_width),
                    self.settings.edge_wrap_mode,
                )
            } else if self.settings.edge_wrap_mode == WrapMode::SvgLike {
                measure_flowchart_svg_label_for_layout_with_metrics_and_source_style(
                    self.svg_label_sidecar,
                    Some(owner),
                    Some(edge.id.as_str()),
                    FlowchartLabelMetricsRequest {
                        measurer: self.measurer,
                        raw_label: label_text,
                        label_type,
                        // Mermaid wraps the temporary SVG text before applying `labelStyle`.
                        style: edge_parent,
                        max_width_px: Some(self.settings.edge_label_wrapping_width),
                        wrap_mode: self.settings.edge_wrap_mode,
                        config: self.effective_config,
                        math_renderer: self.math_renderer,
                    },
                    edge_text_style.as_ref(),
                    Some(&edge_text_style),
                    FlowchartSvgWidthMode::Bbox,
                )
            } else {
                measure_flowchart_svg_label_for_layout_with_source_style(
                    self.svg_label_sidecar,
                    Some(owner),
                    Some(edge.id.as_str()),
                    FlowchartLabelMetricsRequest {
                        measurer: self.measurer,
                        raw_label: label_text,
                        label_type,
                        style: edge_text_style.as_ref(),
                        max_width_px: Some(self.settings.edge_label_wrapping_width),
                        wrap_mode: self.settings.edge_wrap_mode,
                        config: self.effective_config,
                        math_renderer: self.math_renderer,
                    },
                    Some(&edge_text_style),
                    FlowchartSvgWidthMode::Bbox,
                )
            };

        if reserve_nonblank_only
            && merman_render_core::text::trim_ecmascript_whitespace(label_text).is_empty()
        {
            return Ok(None);
        }
        // The SVG label's background extends its measured ink by two units on each side.
        let (width, height) = if self.settings.edge_html_labels {
            (metrics.width, metrics.height)
        } else {
            (metrics.width + 4.0, metrics.height + 4.0)
        };

        Ok(Some(MeasuredLabel { width, height }))
    }
}
