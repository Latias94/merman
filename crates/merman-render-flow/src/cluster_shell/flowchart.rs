//! Complete ordinary and Agentflow cluster layers on the bounded SVG sink.

use kurbo::{Point, Vec2};
use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    LayoutCluster, Result,
    config::DiagramLook,
    html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
    math::MathRenderer,
    mermaid_style,
    number_format::canonical_number as number,
    resources::OperationWorkMeter,
    svg_output::SvgOutput,
    text::{
        TextMeasurer, TextMetrics, literal_label::FlowchartSvgWidthMode,
        svg_label::write_measured_svg_title,
    },
    theme_svg::edge_label_background::literal_fill,
    xml::{escape_attr_display, escape_xml_display},
};
use merman_theme_contract::v2::Color;
use roughr::core::RoughRandomness;
use rustc_hash::FxHashMap;

use super::{
    ClusterHandDrawn, ClusterShellForm, ClusterShellPaint, PaintedClusterFrame,
    cluster_container_radius, write_cluster_shell,
};
use crate::{
    document::FlowchartDocumentId,
    graph::FlowchartClusterPass,
    layout_config::FLOWCHART_FIXED_LABEL_WRAP_WIDTH,
    node_inventory::{FlowchartPreparedCluster, FlowchartPreparedNodes},
    projection::FlowchartRenderModelRef,
    style::compiled::FlowchartCompiledStyles,
    stylesheet::ContainerPalette,
    svg_label::{
        FlowchartPreparedLabelResolution, FlowchartSvgLabelOwner, FlowchartSvgLabelRenderPlan,
        FlowchartSvgLabelSidecar, OptionalStyleAttr, flowchart_label_html_with_prepared_label,
        write_flowchart_svg_label_plan_with_style, write_flowchart_svg_text_markdown,
        write_flowchart_svg_text_markdown_wrapped,
        write_flowchart_svg_text_markdown_wrapped_with_style,
    },
    theme::{FlowchartCompatibilityBinding, FlowchartNativeStyle},
};

const TITLE_HALO_WIDTH: u8 = 5;

/// Prepared paint and the operation's Rough inputs shared by normal clusters and lanes.
pub struct FlowchartClusterShape<'a> {
    pub terminal: &'a FlowchartPreparedCluster,
    pub work_meter: &'a OperationWorkMeter,
    pub randomness: &'a RoughRandomness,
}

impl<'a> ClusterHandDrawn<'a> {
    pub fn for_compiled_styles(
        styles: &'a FlowchartCompiledStyles,
        work_meter: &'a OperationWorkMeter,
        randomness: &'a RoughRandomness,
    ) -> Self {
        Self {
            stroke_width: styles.stroke_width.as_deref(),
            stroke_dasharray: styles.stroke_dasharray.as_deref(),
            work_meter,
            randomness,
        }
    }
}

impl<'a> FlowchartClusterShape<'a> {
    pub(super) fn paint(&self) -> ClusterShellPaint<'a> {
        ClusterShellPaint {
            style: &self.terminal.rect_style,
            fill: &self.terminal.fill,
            stroke: &self.terminal.stroke,
        }
    }

    pub(super) fn hand_drawn(&self, look: DiagramLook<'_>) -> Option<ClusterHandDrawn<'a>> {
        look.is_hand_drawn().then(|| {
            ClusterHandDrawn::for_compiled_styles(
                &self.terminal.compiled,
                self.work_meter,
                self.randomness,
            )
        })
    }

    /// Use the selected palette background when an Agentflow container has one.
    pub fn agentflow_fill(
        base_fill: &'a str,
        palette: &'a ContainerPalette<'_>,
        color_slot: Option<usize>,
    ) -> &'a str {
        color_slot
            .and_then(|slot| palette.background_color(slot))
            .unwrap_or(base_fill)
    }
}

/// A normal cluster's resolved frame, presentation inputs, and document identity.
pub struct FlowchartClusterSvg<'a> {
    pub semantic_id: &'a str,
    pub document_id: FlowchartDocumentId<'a>,
    pub classes: &'a [String],
    pub model: &'a FlowchartRenderModelRef<'a>,
    pub diagram_type: &'a str,
    pub compatibility: &'a FlowchartCompatibilityBinding,
    pub native: Option<&'a FlowchartNativeStyle>,
    pub palette: &'a ContainerPalette<'a>,
    pub canonical_axis: bool,
    pub struck: bool,
    pub frame: PaintedClusterFrame,
    pub prepared_nodes: &'a FlowchartPreparedNodes,
    pub parent: &'a FxHashMap<&'a str, &'a str>,
}

impl FlowchartClusterSvg<'_> {
    /// Write the selected layer after the caller's terminal admission and title measurement.
    pub fn write(
        self,
        out: &mut impl SvgOutput,
        pass: FlowchartClusterPass,
        shape: FlowchartClusterShape<'_>,
        title: FlowchartClusterTitle<'_>,
    ) -> Result<()> {
        let color_slot = self.color_slot(self.semantic_id);
        if pass == FlowchartClusterPass::Background {
            return self.write_background(out, shape, color_slot);
        }
        let halo = self
            .struck
            .then(|| self.title_halo_color(title.config, shape.work_meter))
            .transpose()?
            .flatten();
        self.write_foreground(out, pass, shape, title, halo, color_slot);
        Ok(())
    }

    fn is_agentflow(&self) -> bool {
        self.diagram_type == "agentflow"
    }

    fn color_slot(&self, id: &str) -> Option<usize> {
        let ordinal = self.model.subgraph_color_ordinal(id)?;
        self.palette.slot_for_ordinal(self.diagram_type, ordinal)
    }

    /// The compatibility canvas is also used by the facade's edge fill policy.
    pub fn compat_theme_canvas_fallback(config: &PresentationConfig) -> Color {
        match config.get_str("theme") {
            Some("dark" | "redux-dark" | "redux-dark-color" | "neo-dark") => {
                Color::rgb(0x1F, 0x20, 0x20)
            }
            _ => Color::rgb(255, 255, 255),
        }
    }

    fn title_halo_color(
        &self,
        config: &PresentationConfig,
        work: &OperationWorkMeter,
    ) -> Result<Option<Color>> {
        let canvas = self.native.map(|native| native.canvas).or_else(|| {
            config
                .get_str("themeVariables.background")
                .and_then(literal_fill)
        });
        let mut surface = Color::TRANSPARENT;
        let mut current = Some(self.semantic_id);
        while let Some(id) = current {
            work.charge(1)?;
            let terminal = self.prepared_nodes.cluster(id)?;
            let fill = terminal
                .compiled
                .fill
                .as_deref()
                .and_then(literal_fill)
                .or_else(|| {
                    let fill = if self.is_agentflow() {
                        FlowchartClusterShape::agentflow_fill(
                            &terminal.fill,
                            self.palette,
                            self.color_slot(id),
                        )
                    } else {
                        &terminal.fill
                    };
                    literal_fill(fill)
                })
                .unwrap_or(Color::TRANSPARENT);
            let layer = fill.composite(Color::TRANSPARENT, terminal.compiled.fill_opacity);
            surface = surface.composite(layer, 1.0);
            if surface.channels()[3] == u8::MAX {
                return Ok(Some(surface));
            }
            current = self.parent.get(id).copied();
        }
        match canvas {
            Some(canvas) if canvas.channels()[3] == u8::MAX => {
                Ok(Some(surface.composite(canvas, 1.0)))
            }
            Some(_) => Ok(None),
            None => Ok(Some(
                surface.composite(Self::compat_theme_canvas_fallback(config), 1.0),
            )),
        }
    }

    fn write_shape(
        self,
        out: &mut impl SvgOutput,
        shape: FlowchartClusterShape<'_>,
        color_slot: Option<usize>,
    ) {
        let ClusterShellPaint {
            style,
            fill,
            stroke,
        } = shape.paint();
        let agentflow = self.is_agentflow();
        let fill = if agentflow {
            FlowchartClusterShape::agentflow_fill(fill, self.palette, color_slot)
        } else {
            fill
        };
        let form = if let Some(native) = self.native {
            ClusterShellForm::Native {
                radius: cluster_container_radius(
                    Some(native.form.container_radius),
                    agentflow,
                    self.frame.size,
                ),
                stroke_width: native.form.stroke_width,
            }
        } else if agentflow {
            ClusterShellForm::Agentflow {
                stroke: &self.compatibility.agentflow_container_stroke,
                canonical_fill: self.canonical_axis.then(|| {
                    (
                        shape.terminal.compiled.fill.as_deref().unwrap_or(fill),
                        shape.terminal.compiled.fill_opacity,
                    )
                }),
            }
        } else {
            ClusterShellForm::Flowchart {
                inline_fill: self.canonical_axis && self.palette.color_count() > 0,
            }
        };
        write_cluster_shell(
            out,
            self.frame,
            ClusterShellPaint {
                style,
                fill,
                stroke,
            },
            form,
            shape.hand_drawn(self.compatibility.look),
        );
    }

    fn attributes(&self, color_slot: Option<usize>) -> (String, String) {
        let mut class_attr = String::new();
        for c in self.classes {
            let c = c.trim();
            if c.is_empty() {
                continue;
            }
            if !class_attr.is_empty() {
                class_attr.push(' ');
            }
            class_attr.push_str(c);
        }
        if !class_attr.is_empty() {
            class_attr.push(' ');
        }
        class_attr.push_str("cluster");
        if self.is_agentflow() {
            class_attr.push_str(" flow-cluster");
        }
        let color_attr = color_slot
            .map(|slot| format!(r#" data-color-id="color-{slot}""#))
            .unwrap_or_default();
        (class_attr, color_attr)
    }

    fn write_background(
        self,
        out: &mut impl SvgOutput,
        shape: FlowchartClusterShape<'_>,
        color_slot: Option<usize>,
    ) -> Result<()> {
        let (class_attr, color_attr) = self.attributes(color_slot);
        let cluster_dom_id = self.document_id;
        let data_look = self.compatibility.look.as_str();
        let _ = write!(
            out,
            r#"<g class="{}" data-merman-theme-depth="container" data-look="{}"{color_attr}"#,
            escape_attr_display(&class_attr),
            escape_attr_display(data_look),
        );
        if self.struck {
            let _ = write!(
                out,
                r#" id="{cluster_dom_id}" data-id="{}" data-et="cluster""#,
                escape_xml_display(self.semantic_id),
            );
        }
        out.push('>');
        self.write_shape(out, shape, color_slot);
        out.push_str("</g>");
        out.checkpoint()
    }

    fn write_foreground(
        self,
        out: &mut impl SvgOutput,
        pass: FlowchartClusterPass,
        shape: FlowchartClusterShape<'_>,
        title: FlowchartClusterTitle<'_>,
        halo: Option<Color>,
        color_slot: Option<usize>,
    ) {
        let (class_attr, color_attr) = self.attributes(color_slot);
        let cluster_dom_id = self.document_id;
        let data_look = self.compatibility.look.as_str();
        let overlay = self.struck && pass == FlowchartClusterPass::Label;
        let frame_top = self.frame.origin.y;
        let terminal = shape.terminal;
        let _ = write!(out, r#"<g class="{}""#, escape_xml_display(&class_attr));
        if !overlay {
            let _ = write!(
                out,
                r#" id="{cluster_dom_id}" data-id="{}" data-et="cluster""#,
                escape_xml_display(self.semantic_id),
            );
        }
        let _ = write!(
            out,
            r#" data-look="{}"{color_attr}>"#,
            escape_xml_display(data_look),
        );
        if pass == FlowchartClusterPass::Complete {
            self.write_shape(out, shape, color_slot);
        }
        title.write(out, cluster_dom_id, frame_top, terminal, halo, overlay);
        out.push_str("</g></g>");
    }
}

/// Layout placement and the original title inputs used by the terminal label writers.
pub struct FlowchartClusterTitle<'a> {
    pub layout: &'a LayoutCluster,
    pub translation: Vec2,
    pub group_origin: Point,
    pub canonical_group_title: bool,
    pub painted_markdown: Option<TextMetrics>,
    pub markdown_wrap_width: f64,
    pub label_type: &'a str,
    pub source: &'a str,
    pub html_labels: bool,
    pub uses_elk_adapter_dom: bool,
    pub owner: Option<FlowchartSvgLabelOwner>,
    pub sidecar: Option<&'a FlowchartSvgLabelSidecar>,
    pub measurer: &'a dyn TextMeasurer,
    pub config: &'a PresentationConfig,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

impl FlowchartClusterTitle<'_> {
    /// ELK measures Markdown against the allocated node width, before any paint expansion.
    pub fn markdown_wrap_width(layout: &LayoutCluster, uses_elk_adapter_dom: bool) -> f64 {
        if uses_elk_adapter_dom {
            layout.width.max(1.0)
        } else {
            FLOWCHART_FIXED_LABEL_WRAP_WIDTH
        }
    }

    fn write(
        self,
        out: &mut impl SvgOutput,
        cluster_dom_id: FlowchartDocumentId<'_>,
        frame_top: f64,
        terminal: &FlowchartPreparedCluster,
        halo: Option<Color>,
        overlay: bool,
    ) {
        let Self {
            layout,
            translation,
            group_origin,
            canonical_group_title,
            painted_markdown,
            markdown_wrap_width,
            label_type,
            source: render_title,
            html_labels,
            uses_elk_adapter_dom,
            owner: title_owner,
            sidecar,
            measurer,
            config,
            math_renderer,
        } = self;
        let label_style = terminal.compiled.label_style.trim();
        let title_text_style = terminal.title_text_style.as_ref();
        let label_w = painted_markdown
            .map_or(layout.title_label.width, |metrics| metrics.width)
            .max(0.0);
        let label_h = painted_markdown
            .map_or(layout.title_label.height, |metrics| metrics.height)
            .max(0.0);
        let (label_left, label_top) = if canonical_group_title {
            (
                layout.title_label.x + translation.x - group_origin.x - label_w / 2.0,
                layout.title_label.y + translation.y - group_origin.y - label_h / 2.0,
            )
        } else {
            (
                layout.x + translation.x - group_origin.x - label_w / 2.0,
                frame_top + layout.title_margin_top.max(0.0),
            )
        };
        let _ = write!(
            out,
            r#"<g class="cluster-label" transform="translate({},{})""#,
            number(label_left),
            number(label_top),
        );
        if overlay {
            let _ = write!(out, r#" data-merman-cluster-title-for="{cluster_dom_id}""#);
        }
        if let Some(color) = halo {
            let _ = write!(
                out,
                r#" data-merman-title-struck="true" stroke="{color}" stroke-width="{TITLE_HALO_WIDTH}" stroke-linejoin="round" paint-order="stroke""#,
            );
        }
        out.push('>');
        // A native text branch keeps the title and its outline when raster export strips HTML.
        let mut svg_fallback = false;
        if html_labels {
            let prepared_label = sidecar
                .zip(title_owner)
                .map_or(Default::default(), |(sidecar, owner)| {
                    sidecar.prepared_label_for_terminal(owner, render_title)
                });
            let title_html = flowchart_label_html_with_prepared_label(
                render_title,
                label_type,
                config,
                math_renderer,
                prepared_label,
            );
            svg_fallback = halo.is_some()
                && !matches!(
                    prepared_label,
                    FlowchartPreparedLabelResolution::PreparedMath(_)
                );
            if svg_fallback {
                out.push_str("<switch>");
            }
            let halo_style = halo.map(|color| {
                if label_style.is_empty() {
                    format!("-webkit-text-stroke:{TITLE_HALO_WIDTH}px {color};paint-order:stroke fill")
                } else {
                    format!(
                        "{label_style};-webkit-text-stroke:{TITLE_HALO_WIDTH}px {color};paint-order:stroke fill"
                    )
                }
            });
            // CSS color hex tokens like `#1F2335;` must not run through Mermaid `#entity;` text
            // decoding (which turns `#1F2335;` into `&amp;1F2335;` and drops the browser halo).
            let span_style_attr = OptionalStyleAttr(halo_style.as_deref().unwrap_or(label_style));
            // Min-content expansion does not undo the wrap selected by the first measurement.
            let markdown_uses_wrapped_box = label_w >= markdown_wrap_width - 1e-3;
            let div_style = if label_type != "markdown" {
                "display: table-cell; white-space: nowrap; line-height: 1.5;".to_string()
            } else if markdown_uses_wrapped_box {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                    mw = number(markdown_wrap_width)
                )
            } else {
                format!(
                    "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {mw}px; text-align: center;",
                    mw = number(markdown_wrap_width)
                )
            };
            let _ = write!(
                out,
                r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject>"#,
                number(label_w),
                number(label_h),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_xml_display(&div_style),
                span_style_attr,
                title_html
            );
        }
        if !html_labels || svg_fallback {
            let svg_text_style = cluster_svg_text_style(label_style);
            out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
            if label_type == "markdown" {
                if uses_elk_adapter_dom || svg_fallback {
                    if !svg_text_style.is_empty() {
                        write_flowchart_svg_text_markdown_wrapped_with_style(
                            out,
                            render_title,
                            &svg_text_style,
                            measurer,
                            title_text_style,
                            Some(markdown_wrap_width),
                        );
                    } else {
                        write_flowchart_svg_text_markdown_wrapped(
                            out,
                            render_title,
                            true,
                            measurer,
                            title_text_style,
                            Some(markdown_wrap_width),
                        );
                    }
                } else {
                    write_flowchart_svg_text_markdown(out, render_title, true);
                }
            } else {
                let prepared = FlowchartSvgLabelRenderPlan::new(
                    sidecar,
                    title_owner,
                    render_title,
                    measurer,
                    title_text_style,
                    None,
                    true,
                    FlowchartSvgWidthMode::Bbox,
                );
                if canonical_group_title && matches!(label_type, "text" | "string") {
                    write_measured_svg_title(
                        out,
                        &prepared.wrapped_lines(),
                        &svg_text_style,
                        None,
                        layout.title_label.width,
                    );
                } else {
                    write_flowchart_svg_label_plan_with_style(
                        out,
                        &prepared,
                        true,
                        (!svg_text_style.is_empty()).then_some(svg_text_style.as_str()),
                    );
                }
            }
            out.push_str("</g>");
            if svg_fallback {
                out.push_str("</switch>");
            }
        }
    }
}

fn cluster_svg_text_style(label_style: &str) -> String {
    if label_style.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(label_style.len());
    let mut color_value: Option<&str> = None;
    let mut has_fill = false;
    mermaid_style::visit_parsed_style_declarations(label_style, |decl| {
        if decl.property() == "fill" {
            has_fill = true;
        } else if decl.property() == "color" {
            let val = decl.source_value().trim();
            if !val.is_empty() {
                color_value = Some(val);
            }
        }
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str(decl.property_css());
        out.push(':');
        out.push_str(decl.source_value());
    });
    if !has_fill && let Some(val) = color_value {
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str("fill:");
        out.push_str(val);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::cluster_svg_text_style;

    #[test]
    fn cluster_svg_text_style_preserves_quoted_semicolons_and_mirrors_color() {
        assert_eq!(
            cluster_svg_text_style(
                r#"font-family:"Foo;color:red" !important;COLOR:#fff !important"#
            ),
            r#"font-family:"Foo;color:red" !important;COLOR:#fff !important;fill:#fff !important"#
        );
        assert_eq!(
            cluster_svg_text_style(r#"font-family:"Foo;color:red" !important"#),
            r#"font-family:"Foo;color:red" !important"#
        );
    }
}
