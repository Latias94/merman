use std::borrow::Cow;
use std::collections::HashMap;

use merman_render_core::{
    Error, Result,
    number_format::canonical_number as fmt,
    svg_css::SvgId,
    svg_output::SvgOutput,
    xml::{escape_attr, escape_attr_display, escape_xml},
};

use super::{SankeyDiagramLayout, SankeyNodeLayout};

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

/// Writes Sankey's node and link layers into an open SVG in caller-selected order.
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
}
