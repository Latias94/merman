//! Measured leaf shapes and edge labels shared by Flowchart graph adapters.

use super::config::{FlowchartConfigView, FlowchartLayoutSettings};
use super::node::{NodeLayoutDimensionsRequest, node_layout_dimensions};
use super::{
    FlowEdge, FlowNode, FlowchartEdgeKey, FlowchartEdgeLabelPadding, FlowchartLabelMetricsRequest,
    FlowchartLabelTypography, FlowchartRenderModelRef, FlowchartSvgLabelOwner,
    FlowchartSvgLabelSidecarBuilder, FlowchartSvgWidthMode,
    flowchart_apply_html_node_class_box_metrics,
    flowchart_effective_text_style_for_node_classes_with_provenance, flowchart_node_svg_width_mode,
    measure_flowchart_svg_label_for_layout_with_metrics_style_and_terminal_foreground,
    measure_flowchart_svg_label_for_layout_with_terminal_foreground,
};
use crate::Result;
use crate::math::MathRenderer;
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use merman_model::presentation::PresentationConfig;

pub(super) struct MeasuredLabel {
    pub width: f64,
    pub height: f64,
}

pub(super) struct MeasuredNode {
    pub width: f64,
    pub height: f64,
    pub label: MeasuredLabel,
}

pub(super) struct FlowchartMeasurement<'a> {
    model: FlowchartRenderModelRef<'a>,
    effective_config: &'a PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &'a crate::svg::FlowchartEdgeStylePlan,
    typography: FlowchartLabelTypography,
    edge_label_padding: FlowchartEdgeLabelPadding,
    pub settings: FlowchartLayoutSettings,
}

impl<'a> FlowchartMeasurement<'a> {
    pub fn new(
        model: FlowchartRenderModelRef<'a>,
        effective_config: &'a PresentationConfig,
        measurer: &'a dyn TextMeasurer,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        edge_style_plan: &'a crate::svg::FlowchartEdgeStylePlan,
    ) -> Self {
        let settings = svg_label_sidecar.map_or_else(
            || FlowchartConfigView::new(effective_config).layout_settings(),
            |sidecar| sidecar.layout_settings(effective_config),
        );
        Self {
            model,
            effective_config,
            measurer,
            math_renderer,
            svg_label_sidecar,
            edge_style_plan,
            typography: svg_label_sidecar.map_or_else(
                FlowchartLabelTypography::default,
                FlowchartSvgLabelSidecarBuilder::label_typography,
            ),
            edge_label_padding: svg_label_sidecar.map_or_else(
                FlowchartEdgeLabelPadding::default,
                FlowchartSvgLabelSidecarBuilder::edge_label_padding,
            ),
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

    #[cfg(feature = "layout-shoal")]
    pub fn subgraph_title(
        &self,
        group: &super::FlowSubgraph,
        index: usize,
        charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<crate::text::TextMetrics> {
        let title = self.model.subgraph_title_for_render(index, group);
        let label_type = group.label_type.as_deref().unwrap_or("text");
        let collapsed = self.model.is_subgraph_collapsed(&group.id);
        let mode = if collapsed {
            self.settings.node_wrap_mode
        } else {
            self.settings.cluster_wrap_mode
        };
        let (classes, styles) = self.model.effective_subgraph_css(index, group);
        let structural = collapsed
            .then(|| {
                super::FlowchartStructuralLabelTypography::prepare(
                    &self.model.class_defs,
                    self.settings.edge_html_labels,
                    super::FlowchartLabelDom::node(
                        "collapsedGroup",
                        title,
                        label_type,
                        mode == WrapMode::HtmlLike,
                        crate::config::config_diagram_look(self.effective_config).is_hand_drawn(),
                    ),
                    charge,
                )
            })
            .transpose()?;
        let base = self.label_base_style(mode);
        let parent = structural
            .as_ref()
            .map_or(std::borrow::Cow::Borrowed(base), |structural| {
                self.typography.node_source_parent_style(
                    base,
                    &self.model.class_defs,
                    (classes, styles, false),
                    structural,
                )
            });
        let mut style = super::flowchart_effective_text_style_for_classes_with_provenance(
            parent.as_ref(),
            &self.model.class_defs,
            classes,
            styles,
        );
        if let Some(structural) = structural {
            structural.apply_to_content(&mut style.style);
        }
        let max_width_px = if collapsed {
            Some(self.settings.wrapping_width)
        } else {
            (label_type == "markdown").then_some(self.settings.cluster_title_wrapping_width)
        };
        let mut metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
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
            style.terminal_foreground(),
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

    pub fn node(
        &self,
        node: &FlowNode,
        owner: FlowchartSvgLabelOwner,
        direction: &str,
        charge: impl FnMut(usize) -> Result<()>,
    ) -> Result<MeasuredNode> {
        let raw_label = self.model.node_label_for_render(node).unwrap_or(&node.id);
        let wrapping_width =
            super::bpmn::wrapping_width(node.layout_shape.as_deref(), self.settings.wrapping_width);
        let label_type = node.label_type.as_deref().unwrap_or("text");
        let label_dom = super::FlowchartLabelDom::node(
            node.layout_shape.as_deref().unwrap_or("squareRect"),
            raw_label,
            label_type,
            self.settings.node_wrap_mode == WrapMode::HtmlLike,
            crate::config::config_diagram_look(self.effective_config).is_hand_drawn(),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            &self.model.class_defs,
            self.settings.edge_html_labels,
            label_dom,
            charge,
        )?;
        let node_parent = self.typography.node_source_parent_style(
            self.label_base_style(self.settings.node_wrap_mode),
            &self.model.class_defs,
            (&node.classes, &node.styles, true),
            &structural,
        );
        let mut node_text_style = flowchart_effective_text_style_for_node_classes_with_provenance(
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
        let mut metrics = measure_flowchart_svg_label_for_layout_with_terminal_foreground(
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
            node_text_style.terminal_foreground(),
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
            super::flowchart_node_label_min_width(
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
        let (width, height) = node_layout_dimensions(NodeLayoutDimensionsRequest {
            layout_shape: node.layout_shape.as_deref(),
            layout_direction: direction,
            metrics,
            has_label: !raw_label.is_empty(),
            wrapping_width,
            node_constraint: node.constraint.as_deref(),
            padding: self.settings.node_padding,
            look_is_neo: crate::config::mermaid_config_diagram_look(self.effective_config).is_neo(),
            state_padding: self.settings.state_padding,
            node_icon: node.icon.as_deref(),
            node_img: node.img.as_deref(),
            node_pos: node.pos.as_deref(),
            node_asset_width: node.asset_width,
            node_asset_height: node.asset_height,
        });

        Ok(MeasuredNode {
            width,
            height,
            label,
        })
    }

    pub fn edge_label(
        &self,
        edge: &FlowEdge,
        key: FlowchartEdgeKey,
    ) -> Result<Option<MeasuredLabel>> {
        let owner = FlowchartSvgLabelOwner::Edge(key.semantic_index());
        let label_text = self
            .model
            .edge_label_for_render(key.semantic_index(), edge)
            .unwrap_or_default();
        let label_type = edge.label_type.as_deref().unwrap_or("text");
        if crate::flowchart::flowchart_label_is_empty_for_render(label_text) {
            return Ok(None);
        }

        let edge_parent = self.edge_style_plan.label_parent_style_for(
            key,
            self.label_base_style(self.settings.edge_wrap_mode),
            &self.typography,
        )?;
        let edge_text_style = self
            .edge_style_plan
            .edge_label_text_style_for(key, edge_parent.as_ref())?;
        let metrics =
            if label_type == "markdown" && self.settings.edge_wrap_mode != WrapMode::HtmlLike {
                crate::text::measure_wrapped_markdown_with_inline_styles(
                    self.measurer,
                    label_text,
                    edge_text_style.as_ref(),
                    Some(self.settings.edge_label_wrapping_width),
                    self.settings.edge_wrap_mode,
                )
            } else if self.settings.edge_wrap_mode == WrapMode::SvgLike {
                measure_flowchart_svg_label_for_layout_with_metrics_style_and_terminal_foreground(
                    self.svg_label_sidecar,
                    Some(owner),
                    Some(edge.id.as_str()),
                    FlowchartLabelMetricsRequest {
                        measurer: self.measurer,
                        raw_label: label_text,
                        label_type,
                        // Mermaid wraps the temporary SVG text before applying `labelStyle`.
                        style: edge_parent.as_ref(),
                        max_width_px: Some(self.settings.edge_label_wrapping_width),
                        wrap_mode: self.settings.edge_wrap_mode,
                        config: self.effective_config,
                        math_renderer: self.math_renderer,
                    },
                    edge_text_style.as_ref(),
                    edge_text_style.terminal_foreground(),
                    FlowchartSvgWidthMode::Bbox,
                )
            } else {
                measure_flowchart_svg_label_for_layout_with_terminal_foreground(
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
                    edge_text_style.terminal_foreground(),
                    FlowchartSvgWidthMode::Bbox,
                )
            };

        let (width, height) = if self.settings.edge_html_labels {
            (metrics.width.max(1.0), metrics.height.max(1.0))
        } else {
            (
                (metrics.width + 4.0).max(1.0),
                (metrics.height + 4.0).max(1.0),
            )
        };
        let (width, height) = self.edge_label_padding.padded_size(width, height);

        Ok(Some(MeasuredLabel { width, height }))
    }
}
