//! Complete ordinary and Agentflow cluster layers on the bounded SVG sink.

use kurbo::{Point, Size};
use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    Result,
    html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
    math::MathRenderer,
    mermaid_style,
    number_format::canonical_number as number,
    svg_output::SvgOutput,
    text::{
        TextMeasurer, TextStyle, literal_label::FlowchartSvgWidthMode,
        svg_label::write_measured_svg_title,
    },
    xml::{escape_attr_display, escape_xml_display},
};
use merman_theme_contract::v2::Color;

use super::{
    ClusterHandDrawn, ClusterShellForm, ClusterShellPaint, PaintedClusterFrame, write_cluster_shell,
};
use crate::{
    document::FlowchartDocumentId,
    svg_label::{
        FlowchartPreparedLabelResolution, FlowchartSvgLabelOwner, FlowchartSvgLabelRenderPlan,
        FlowchartSvgLabelSidecar, OptionalStyleAttr, flowchart_label_html_with_prepared_label,
        write_flowchart_svg_label_plan_with_style, write_flowchart_svg_text_markdown,
        write_flowchart_svg_text_markdown_wrapped,
        write_flowchart_svg_text_markdown_wrapped_with_style,
    },
};

const TITLE_HALO_WIDTH: u8 = 5;

pub struct FlowchartClusterShape<'a> {
    pub frame: PaintedClusterFrame,
    pub paint: ClusterShellPaint<'a>,
    pub form: ClusterShellForm<'a>,
    pub rough: Option<ClusterHandDrawn<'a>>,
}

impl FlowchartClusterShape<'_> {
    fn write(self, out: &mut impl SvgOutput) {
        write_cluster_shell(out, self.frame, self.paint, self.form, self.rough);
    }
}

pub struct FlowchartClusterSvg<'a> {
    pub semantic_id: &'a str,
    pub document_id: FlowchartDocumentId<'a>,
    pub classes: &'a [String],
    pub agentflow: bool,
    pub look: &'a str,
    pub color_slot: Option<usize>,
    pub struck: bool,
}

impl FlowchartClusterSvg<'_> {
    fn attributes(&self) -> (String, String) {
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
        if self.agentflow {
            class_attr.push_str(" flow-cluster");
        }
        let color_attr = self
            .color_slot
            .map(|slot| format!(r#" data-color-id="color-{slot}""#))
            .unwrap_or_default();
        (class_attr, color_attr)
    }

    pub fn write_background(
        self,
        out: &mut impl SvgOutput,
        shape: FlowchartClusterShape<'_>,
    ) -> Result<()> {
        let (class_attr, color_attr) = self.attributes();
        let Self {
            semantic_id,
            document_id: cluster_dom_id,
            look: data_look,
            struck,
            ..
        } = self;
        let _ = write!(
            out,
            r#"<g class="{}" data-merman-theme-depth="container" data-look="{}"{color_attr}"#,
            escape_attr_display(&class_attr),
            escape_attr_display(data_look),
        );
        if struck {
            let _ = write!(
                out,
                r#" id="{cluster_dom_id}" data-id="{}" data-et="cluster""#,
                escape_xml_display(semantic_id),
            );
        }
        out.push('>');
        shape.write(out);
        out.push_str("</g>");
        out.checkpoint()
    }

    pub fn write_foreground(
        self,
        out: &mut impl SvgOutput,
        shape: Option<FlowchartClusterShape<'_>>,
        title: FlowchartClusterTitle<'_>,
        halo: Option<Color>,
        overlay: bool,
    ) {
        let (class_attr, color_attr) = self.attributes();
        let Self {
            semantic_id,
            document_id: cluster_dom_id,
            look: data_look,
            ..
        } = self;
        let _ = write!(out, r#"<g class="{}""#, escape_xml_display(&class_attr));
        if !overlay {
            let _ = write!(
                out,
                r#" id="{cluster_dom_id}" data-id="{}" data-et="cluster""#,
                escape_xml_display(semantic_id),
            );
        }
        let _ = write!(
            out,
            r#" data-look="{}"{color_attr}>"#,
            escape_xml_display(data_look),
        );
        if let Some(shape) = shape {
            shape.write(out);
        }
        title.write(out, cluster_dom_id, halo, overlay);
        out.push_str("</g></g>");
    }
}

pub struct FlowchartClusterTitle<'a> {
    pub position: Point,
    pub size: Size,
    pub markdown_wrap_width: f64,
    pub measured_title_width: Option<f64>,
    pub label_type: &'a str,
    pub source: &'a str,
    pub label_style: &'a str,
    pub text_style: &'a TextStyle,
    pub html_labels: bool,
    pub uses_elk_adapter_dom: bool,
    pub owner: Option<FlowchartSvgLabelOwner>,
    pub sidecar: Option<&'a FlowchartSvgLabelSidecar>,
    pub measurer: &'a dyn TextMeasurer,
    pub config: &'a PresentationConfig,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

impl FlowchartClusterTitle<'_> {
    fn write(
        self,
        out: &mut impl SvgOutput,
        cluster_dom_id: FlowchartDocumentId<'_>,
        halo: Option<Color>,
        overlay: bool,
    ) {
        let Self {
            position,
            size,
            markdown_wrap_width,
            measured_title_width,
            label_type,
            source: render_title,
            label_style,
            text_style: title_text_style,
            html_labels,
            uses_elk_adapter_dom,
            owner: title_owner,
            sidecar,
            measurer,
            config,
            math_renderer,
        } = self;
        let (label_left, label_top) = (position.x, position.y);
        let (label_w, label_h) = (size.width, size.height);
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
                if let Some(width) = measured_title_width
                    && matches!(label_type, "text" | "string")
                {
                    write_measured_svg_title(
                        out,
                        &prepared.wrapped_lines(),
                        &svg_text_style,
                        None,
                        width,
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
