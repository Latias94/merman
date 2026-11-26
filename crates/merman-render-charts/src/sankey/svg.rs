use std::borrow::Cow;
use std::collections::HashMap;

use merman_model::OperationPhase;
use merman_render_core::{
    Error, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    svg_css::SvgId,
    svg_output::SvgOutput,
    xml::{escape_attr, escape_attr_display, escape_xml, escape_xml_raw_into},
};

use super::{SankeyDiagramLayout, SankeyNodeLayout};

mod render;

pub use render::render_sankey_diagram_svg;

#[derive(Clone, Copy)]
struct SankeyScopedId<'a> {
    diagram_id: Option<SvgId<'a>>,
    local_id: &'a str,
}

impl std::fmt::Display for SankeyScopedId<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.diagram_id {
            Some(diagram_id) => write!(formatter, "{diagram_id}-{}", self.local_id),
            None => formatter.write_str(self.local_id),
        }
    }
}

/// The stroke colors already selected for a link.
pub enum SankeySvgLinkPaint<'a> {
    Solid(&'a str),
    Gradient { source: &'a str, target: &'a str },
}

/// The prepared endpoints and paint borrowed for one link element.
pub struct SankeySvgLink<'a> {
    pub source: &'a SankeyNodeLayout,
    pub target: &'a SankeyNodeLayout,
    pub paint: SankeySvgLinkPaint<'a>,
}

pub struct SankeySvgBodyStyle<'a> {
    pub links_before_nodes: bool,
    pub link_blend_mode_css: &'a str,
    pub link_stroke_opacity: &'a str,
    pub outlined_labels: bool,
    pub show_values: bool,
    pub label_font_size: f64,
    pub label_weight: Option<u16>,
    pub central_node_layer: usize,
}

impl SankeySvgBodyStyle<'_> {
    pub fn label_position(
        &self,
        node: &SankeyNodeLayout,
        layout_width: f64,
    ) -> (f64, &'static str) {
        let label_gap_x = 6.0;
        let on_right = if self.outlined_labels {
            node.layer >= self.central_node_layer
        } else {
            node.x0 < layout_width / 2.0
        };
        if on_right {
            (node.x1 + label_gap_x, "start")
        } else {
            (node.x0 - label_gap_x, "end")
        }
    }

    pub const fn label_dy_em(&self) -> f64 {
        if self.show_values { 0.0 } else { 0.35 }
    }
}

/// Writes Sankey's node, link, and label layers into an open SVG.
pub struct SankeySvgBody<'a> {
    layout: &'a SankeyDiagramLayout,
    diagram_id: Option<SvgId<'a>>,
    terminal_node_ids: HashMap<String, String>,
    uid_count: usize,
}

fn next_generated_id(uid_count: &mut usize, prefix: &str) -> String {
    *uid_count += 1;
    format!("{prefix}{uid_count}")
}

impl<'a> SankeySvgBody<'a> {
    /// Reserves all node IDs before links can allocate gradient IDs, with the
    /// last ID winning for duplicate node occurrences.
    pub fn new(layout: &'a SankeyDiagramLayout, diagram_id: Option<SvgId<'a>>) -> Self {
        let mut uid_count = 0;
        let mut terminal_node_ids = HashMap::with_capacity(layout.nodes.len());
        for node in &layout.nodes {
            terminal_node_ids.insert(node.id.clone(), next_generated_id(&mut uid_count, "node-"));
        }
        Self {
            layout,
            diagram_id,
            terminal_node_ids,
            uid_count,
        }
    }

    /// Writes all body layers using one prepared literal label per layout node.
    pub fn write_contents<'label, 'paint>(
        &mut self,
        out: &mut impl SvgOutput,
        style: &SankeySvgBodyStyle<'_>,
        label_texts: impl Iterator<Item = &'label str> + Clone,
        fill_for: impl FnMut(usize, &str) -> Option<&'paint str>,
        mut terminal_link: impl FnMut(usize) -> Option<SankeySvgLink<'paint>>,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        if style.links_before_nodes {
            self.write_links(
                out,
                style.link_blend_mode_css,
                style.link_stroke_opacity,
                &mut terminal_link,
            )?;
        }
        self.write_nodes(out, fill_for)?;
        self.write_labels(out, style, label_texts, work_meter)?;
        if !style.links_before_nodes {
            self.write_links(
                out,
                style.link_blend_mode_css,
                style.link_stroke_opacity,
                &mut terminal_link,
            )?;
        }
        Ok(())
    }

    fn write_labels<'label>(
        &self,
        out: &mut impl SvgOutput,
        style: &SankeySvgBodyStyle<'_>,
        label_texts: impl Iterator<Item = &'label str> + Clone,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let label_weight = style
            .label_weight
            .map_or_else(String::new, |weight| format!(r#" font-weight="{weight}""#));
        let _ = write!(
            out,
            r#"<g class="node-labels" font-size="{font_size}"{label_weight}>"#,
            font_size = fmt(style.label_font_size)
        );
        out.checkpoint()?;
        if style.outlined_labels {
            self.write_label_layer(
                out,
                style,
                label_texts.clone(),
                Some("sankey-label-bg"),
                work_meter,
            )?;
            self.write_label_layer(out, style, label_texts, Some("sankey-label-fg"), work_meter)?;
        } else {
            self.write_label_layer(out, style, label_texts, None, work_meter)?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }

    fn write_label_layer<'label>(
        &self,
        out: &mut impl SvgOutput,
        style: &SankeySvgBodyStyle<'_>,
        label_texts: impl Iterator<Item = &'label str>,
        class_name: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> Result<()> {
        let layout_width = self.layout.width.max(1.0);
        for (node, label) in self.layout.nodes.iter().zip(label_texts) {
            work_meter.checkpoint(OperationPhase::Emit)?;
            let y = (node.y0 + node.y1) / 2.0;
            let (x, anchor) = style.label_position(node, layout_width);
            let dy = if style.show_values {
                "0em".to_string()
            } else {
                format!("{}em", fmt(style.label_dy_em()))
            };
            let class_attr = class_name
                .map(|class_name| format!(r#" class="{}""#, escape_attr(class_name)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"<text{class_attr} x="{x}" y="{y}" dy="{dy}" text-anchor="{anchor}">"#,
                class_attr = class_attr,
                x = fmt(x),
                y = fmt(y),
                dy = dy,
                anchor = anchor,
            );
            escape_xml_raw_into(out, label);
            out.push_str("</text>");
            out.checkpoint()?;
        }
        Ok(())
    }

    pub fn write_nodes<'paint>(
        &self,
        out: &mut impl SvgOutput,
        mut fill_for: impl FnMut(usize, &str) -> Option<&'paint str>,
    ) -> Result<()> {
        out.push_str(r#"<g class="nodes">"#);
        out.checkpoint()?;
        for (node_index, node) in self.layout.nodes.iter().enumerate() {
            let node_uid = SankeyScopedId {
                diagram_id: self.diagram_id,
                local_id: self
                    .terminal_node_ids
                    .get(&node.id)
                    .map_or("node-0", String::as_str),
            };
            let x = node.x0;
            let y = node.y0;
            let w = node.x1 - node.x0;
            let h = node.y1 - node.y0;
            let fill = fill_for(node_index, &node.id).ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "Sankey node palette has no paint for node {} at ordinal {}",
                    node.id,
                    node_index + 1
                ),
            })?;
            let _ = write!(
                out,
                r#"<g class="node" id="{id}" transform="translate({x},{y})" x="{x}" y="{y}"><rect height="{h}" width="{w}" fill="{fill}"/></g>"#,
                id = escape_attr_display(node_uid),
                x = fmt(x),
                y = fmt(y),
                h = fmt(h),
                w = fmt(w),
                fill = escape_attr(fill),
            );
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }

    /// Writes links using the family-selected blend mode, opacity, and paint.
    pub fn write_links<'paint>(
        &mut self,
        out: &mut impl SvgOutput,
        link_blend_mode_css: &str,
        link_stroke_opacity: &str,
        mut terminal_link: impl FnMut(usize) -> Option<SankeySvgLink<'paint>>,
    ) -> Result<()> {
        let _ = write!(
            out,
            r#"<g class="links" fill="none" stroke-opacity="{}">"#,
            link_stroke_opacity,
        );
        out.checkpoint()?;
        let link_group_open = format!(
            r#"<g class="link" style="mix-blend-mode: {};">"#,
            link_blend_mode_css,
        );
        for (link_index, link) in self.layout.links.iter().enumerate() {
            let prepared = terminal_link(link_index).ok_or_else(|| Error::InvalidModel {
                message: format!("Sankey has no prepared link at ordinal {}", link_index + 1),
            })?;
            let sx = prepared.source.x1;
            let tx = prepared.target.x0;
            let mx = (sx + tx) / 2.0;
            let path_d = format!(
                "M{sx},{y0}C{mx},{y0},{mx},{y1},{tx},{y1}",
                sx = fmt(sx),
                y0 = fmt(link.y0),
                mx = fmt(mx),
                y1 = fmt(link.y1),
                tx = fmt(tx),
            );

            out.push_str(&link_group_open);
            out.checkpoint()?;

            let stroke = match prepared.paint {
                SankeySvgLinkPaint::Solid(css) => Cow::Borrowed(css),
                SankeySvgLinkPaint::Gradient {
                    source: source_color,
                    target: target_color,
                } => {
                    let gradient_id = next_generated_id(&mut self.uid_count, "linearGradient-");
                    let scoped_gradient_id = SankeyScopedId {
                        diagram_id: self.diagram_id,
                        local_id: &gradient_id,
                    };
                    let _ = write!(
                        out,
                        r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{x1}" x2="{x2}"><stop offset="0%" stop-color="{c1}"/><stop offset="100%" stop-color="{c2}"/></linearGradient>"#,
                        id = escape_attr_display(scoped_gradient_id),
                        x1 = fmt(sx),
                        x2 = fmt(tx),
                        c1 = escape_attr(source_color),
                        c2 = escape_attr(target_color),
                    );
                    out.checkpoint()?;
                    Cow::Owned(format!("url(#{})", scoped_gradient_id))
                }
            };

            let stroke_width = link.width.max(1.0);
            let _ = write!(
                out,
                r#"<path d="{d}" stroke="{stroke}" stroke-width="{sw}"/></g>"#,
                d = escape_xml(&path_d),
                stroke = escape_attr(&stroke),
                sw = fmt(stroke_width),
            );
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sankey::SankeyLinkLayout;
    use merman_render_core::resources::RenderResourcePolicy;
    use std::cell::Cell;
    use std::{fmt, ops::Range};

    fn layout() -> SankeyDiagramLayout {
        let nodes = (0..3)
            .map(|index| SankeyNodeLayout {
                id: format!("node-{index}"),
                index,
                depth: index,
                height: 0,
                layer: index,
                value: 1.0,
                x0: 10.0 * index as f64,
                x1: 10.0 * index as f64 + 5.0,
                y0: 0.0,
                y1: 20.0,
            })
            .collect();
        let links = (0..2)
            .map(|index| SankeyLinkLayout {
                index,
                source: format!("node-{index}"),
                target: format!("node-{}", index + 1),
                value: 1.0,
                width: 1.0,
                y0: 5.0,
                y1: 7.0,
            })
            .collect();
        SankeyDiagramLayout {
            bounds: None,
            width: 100.0,
            height: 100.0,
            node_width: 5.0,
            node_padding: 12.0,
            nodes,
            links,
        }
    }

    struct RejectAfterCheckpoints {
        retained: String,
        checkpoints: usize,
        reject_after: usize,
        rejected_writes: usize,
    }

    impl RejectAfterCheckpoints {
        fn new(reject_after: usize) -> Self {
            Self {
                retained: String::new(),
                checkpoints: 0,
                reject_after,
                rejected_writes: 0,
            }
        }
    }

    impl fmt::Write for RejectAfterCheckpoints {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            if self.checkpoints == self.reject_after {
                self.rejected_writes += 1;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl SvgOutput for RejectAfterCheckpoints {
        fn push_str(&mut self, value: &str) {
            let _ = fmt::Write::write_str(self, value);
        }

        fn push(&mut self, value: char) {
            let _ = fmt::Write::write_char(self, value);
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            &self.retained
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected_writes != 0 {
                return Err(Error::InvalidModel {
                    message: "test SVG sink rejected a write".to_string(),
                });
            }
            self.checkpoints += 1;
            Ok(())
        }
    }

    #[test]
    fn sankey_nodes_stop_after_the_first_svg_sink_failure() {
        let layout = layout();
        let body = SankeySvgBody::new(&layout, None);
        let mut out = RejectAfterCheckpoints::new(1);
        let mut paints = 0;

        let error = body
            .write_nodes(&mut out, |_, _| {
                paints += 1;
                Some("#445566")
            })
            .expect_err("the rejecting sink must stop Sankey node emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(paints, 1);
        assert_eq!(out.rejected_writes, 1);
        assert_eq!(out.retained, r#"<g class="nodes">"#);
    }

    #[test]
    fn prepared_gradient_stops_before_path_when_gradient_sink_fails() {
        let layout = layout();
        let mut body = SankeySvgBody::new(&layout, None);
        let mut out = RejectAfterCheckpoints::new(2);
        let mut links = 0;

        let error = body
            .write_links(&mut out, "multiply", "0.5", |index| {
                links += 1;
                Some(SankeySvgLink {
                    source: &layout.nodes[index],
                    target: &layout.nodes[index + 1],
                    paint: SankeySvgLinkPaint::Gradient {
                        source: "#112233",
                        target: "#445566",
                    },
                })
            })
            .expect_err("failed gradient must stop path emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(links, 1);
        assert_eq!(out.rejected_writes, 1);
        assert_eq!(body.uid_count, layout.nodes.len() + 1);
        assert_eq!(
            out.retained,
            concat!(
                r#"<g class="links" fill="none" stroke-opacity="0.5">"#,
                r#"<g class="link" style="mix-blend-mode: multiply;">"#,
            )
        );
    }

    fn body_style(links_before_nodes: bool, outlined_labels: bool) -> SankeySvgBodyStyle<'static> {
        SankeySvgBodyStyle {
            links_before_nodes,
            link_blend_mode_css: if links_before_nodes {
                "normal"
            } else {
                "multiply"
            },
            link_stroke_opacity: if links_before_nodes { "0.35" } else { "0.5" },
            outlined_labels,
            show_values: false,
            label_font_size: 14.0,
            label_weight: None,
            central_node_layer: 1,
        }
    }

    #[test]
    fn sankey_body_keeps_layer_order_label_placement_and_scoped_gradients() {
        let layout = layout();
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for links_before_nodes in [false, true] {
            for outlined_labels in [false, true] {
                let mut style = body_style(links_before_nodes, outlined_labels);
                style.label_weight = Some(500);
                let mut out = String::new();
                let mut body = SankeySvgBody::new(&layout, SvgId::new("sk"));
                body.write_contents(
                    &mut out,
                    &style,
                    ["A&amp;<", "Second", "Third"].into_iter(),
                    |_, _| Some("#112233"),
                    |index| {
                        Some(SankeySvgLink {
                            source: &layout.nodes[index],
                            target: &layout.nodes[index + 1],
                            paint: SankeySvgLinkPaint::Gradient {
                                source: "#112233",
                                target: "#445566",
                            },
                        })
                    },
                    &work_meter,
                )
                .unwrap();
                let links = out.find(r#"class="links""#).unwrap();
                let nodes = out.find(r#"class="nodes""#).unwrap();
                let labels = out.find(r#"class="node-labels""#).unwrap();
                assert!(nodes < labels);
                assert!(if links_before_nodes {
                    links < nodes
                } else {
                    labels < links
                });
                assert!(out.contains(r#"font-size="14" font-weight="500""#));
                assert!(out.contains(r#"id="sk-linearGradient-4""#));
                assert!(out.contains(r##"stroke="url(#sk-linearGradient-5)""##));
                if outlined_labels {
                    let background = out.find(r#"class="sankey-label-bg""#).unwrap();
                    let foreground = out.find(r#"class="sankey-label-fg""#).unwrap();
                    assert!(labels < background && background < foreground);
                    assert_eq!(
                        out.matches(
                            r#"x="-6" y="10" dy="0.35em" text-anchor="end">A&amp;amp;&lt;</text>"#
                        )
                        .count(),
                        2
                    );
                } else {
                    assert!(out.contains(r#"<text x="11" y="10" dy="0.35em" text-anchor="start">A&amp;amp;&lt;</text>"#));
                    assert!(!out.contains("sankey-label-bg"));
                }
            }
        }
    }

    #[test]
    fn sankey_body_stops_at_first_rejected_label_before_later_layers() {
        let layout = layout();
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let mut out = RejectAfterCheckpoints::new(layout.nodes.len() + 3);
        let mut body = SankeySvgBody::new(&layout, None);
        let label_visits = Cell::new(0);
        let mut link_visits = 0;
        body.write_contents(
            &mut out,
            &body_style(false, true),
            ["First", "Second", "Third"]
                .into_iter()
                .inspect(|_| label_visits.set(label_visits.get() + 1)),
            |_, _| Some("#112233"),
            |_| {
                link_visits += 1;
                None
            },
            &work_meter,
        )
        .expect_err("the first rejected label must stop the body writer");
        assert_eq!(label_visits.get(), 1);
        assert_eq!(link_visits, 0);
        assert!(
            out.retained
                .ends_with(r#"<g class="node-labels" font-size="14">"#)
        );
    }
}
