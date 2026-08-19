use super::super::*;

struct SankeyTerminalNodeEntry<'layout> {
    first_node: &'layout crate::model::SankeyNodeLayout,
    terminal_uid: String,
}

/// Operation-local index shared by node emission and link endpoint resolution.
///
/// Mermaid resolves duplicate endpoint IDs to the first node, while the existing terminal node-ID
/// map assigned the last generated UID to every duplicate occurrence. Keeping both facts in one
/// entry removes the per-link node scans without changing either behavior.
struct SankeyTerminalNodeIndex<'layout> {
    by_id: std::collections::HashMap<&'layout str, SankeyTerminalNodeEntry<'layout>>,
    #[cfg(test)]
    indexed_node_visits: usize,
    #[cfg(test)]
    endpoint_lookups: std::cell::Cell<usize>,
}

impl<'layout> SankeyTerminalNodeIndex<'layout> {
    fn new(
        nodes: &'layout [crate::model::SankeyNodeLayout],
        next_generated_id: &mut impl FnMut(&str) -> String,
    ) -> Self {
        let mut by_id = std::collections::HashMap::with_capacity(nodes.len());
        #[cfg(test)]
        let mut indexed_node_visits = 0;

        for node in nodes {
            #[cfg(test)]
            {
                indexed_node_visits += 1;
            }
            let terminal_uid = next_generated_id("node-");
            match by_id.entry(node.id.as_str()) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(SankeyTerminalNodeEntry {
                        first_node: node,
                        terminal_uid,
                    });
                }
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().terminal_uid = terminal_uid;
                }
            }
        }

        Self {
            by_id,
            #[cfg(test)]
            indexed_node_visits,
            #[cfg(test)]
            endpoint_lookups: std::cell::Cell::new(0),
        }
    }

    fn terminal_uid(&self, node_id: &str) -> Option<&str> {
        self.by_id
            .get(node_id)
            .map(|entry| entry.terminal_uid.as_str())
    }

    fn endpoint(&self, node_id: &str) -> Option<&'layout crate::model::SankeyNodeLayout> {
        #[cfg(test)]
        self.endpoint_lookups
            .set(self.endpoint_lookups.get().saturating_add(1));
        self.by_id.get(node_id).map(|entry| entry.first_node)
    }

    fn resolve_link_endpoints(
        &self,
        link: &crate::model::SankeyLinkLayout,
    ) -> Result<(
        &'layout crate::model::SankeyNodeLayout,
        &'layout crate::model::SankeyNodeLayout,
    )> {
        let source = self
            .endpoint(&link.source)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing source node {}", link.source),
            })?;
        let target = self
            .endpoint(&link.target)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing target node {}", link.target),
            })?;
        Ok((source, target))
    }

    #[cfg(test)]
    fn lookup_stats(&self) -> SankeyEndpointLookupStats {
        SankeyEndpointLookupStats {
            indexed_node_visits: self.indexed_node_visits,
            endpoint_lookups: self.endpoint_lookups.get(),
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SankeyEndpointLookupStats {
    indexed_node_visits: usize,
    endpoint_lookups: usize,
}

fn write_sankey_nodes(
    out: &mut impl SvgOutput,
    nodes: &[crate::model::SankeyNodeLayout],
    node_palette: &crate::sankey::SankeyNodePalettePlan,
    mut receipt: Option<&mut crate::sankey::SankeyNodePaletteReceipt>,
    terminal_node_index: &SankeyTerminalNodeIndex<'_>,
    scope_generated_ids: bool,
    diagram_id: &str,
) -> Result<()> {
    for (node_index, node) in nodes.iter().enumerate() {
        let fallback_uid;
        let node_uid = match terminal_node_index.terminal_uid(&node.id) {
            Some(node_uid) => node_uid,
            None => {
                fallback_uid = if scope_generated_ids {
                    scoped_svg_id(diagram_id, "node-0")
                } else {
                    "node-0".to_string()
                };
                fallback_uid.as_str()
            }
        };
        let x = node.x0;
        let y = node.y0;
        let w = node.x1 - node.x0;
        let h = node.y1 - node.y0;
        let fill =
            node_palette
                .fill_for(node_index, &node.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "Sankey node palette has no paint for node {} at ordinal {}",
                        node.id,
                        node_index + 1
                    ),
                })?;
        let _ = write!(
            out,
            r#"<g class="node" id="{id}" transform="translate({x},{y})" x="{x}" y="{y}"><rect height="{h}" width="{w}" fill="{fill}"/></g>"#,
            id = escape_xml(node_uid),
            x = fmt(x),
            y = fmt(y),
            h = fmt(h),
            w = fmt(w),
            fill = escape_attr(&fill),
        );
        out.checkpoint()?;
        if let Some(receipt) = receipt.as_deref_mut() {
            receipt.record_node(
                node_palette,
                node_index,
                &node.id,
                node_index + 1,
                Some(fill),
            );
        }
    }
    Ok(())
}

fn write_sankey_links(
    out: &mut impl SvgOutput,
    links: &[crate::model::SankeyLinkLayout],
    terminal_node_index: &SankeyTerminalNodeIndex<'_>,
    node_palette: &crate::sankey::SankeyNodePalettePlan,
    link_color: &str,
    next_generated_id: &mut impl FnMut(&str) -> String,
) -> Result<()> {
    for link in links {
        let (source, target) = terminal_node_index.resolve_link_endpoints(link)?;

        let sx = source.x1;
        let tx = target.x0;
        let mx = (sx + tx) / 2.0;
        let path_d = format!(
            "M{sx},{y0}C{mx},{y0},{mx},{y1},{tx},{y1}",
            sx = fmt(sx),
            y0 = fmt(link.y0),
            mx = fmt(mx),
            y1 = fmt(link.y1),
            tx = fmt(tx),
        );

        out.push_str(r#"<g class="link" style="mix-blend-mode: multiply;">"#);
        out.checkpoint()?;

        let stroke = match link_color {
            "source" => node_palette
                .fill_for_id(&source.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("Sankey node palette has no source paint for {}", source.id),
                })?
                .to_string(),
            "target" => node_palette
                .fill_for_id(&target.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("Sankey node palette has no target paint for {}", target.id),
                })?
                .to_string(),
            "gradient" => {
                let gradient_id = next_generated_id("linearGradient-");
                let source_color =
                    node_palette
                        .fill_for_id(&source.id)
                        .ok_or_else(|| Error::InvalidModel {
                            message: format!(
                                "Sankey node palette has no gradient source paint for {}",
                                source.id
                            ),
                        })?;
                let target_color =
                    node_palette
                        .fill_for_id(&target.id)
                        .ok_or_else(|| Error::InvalidModel {
                            message: format!(
                                "Sankey node palette has no gradient target paint for {}",
                                target.id
                            ),
                        })?;
                let _ = write!(
                    out,
                    r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{x1}" x2="{x2}"><stop offset="0%" stop-color="{c1}"/><stop offset="100%" stop-color="{c2}"/></linearGradient>"#,
                    id = escape_attr(&gradient_id),
                    x1 = fmt(sx),
                    x2 = fmt(tx),
                    c1 = escape_attr(&source_color),
                    c2 = escape_attr(&target_color),
                );
                out.checkpoint()?;
                format!("url(#{})", gradient_id)
            }
            other => other.to_string(),
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

    Ok(())
}

pub(crate) fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    node_palette: &crate::sankey::SankeyNodePalettePlan,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let render_settings = crate::sankey::SankeyConfigView::new(effective_config).render_settings();
    let use_max_width = render_settings.use_max_width;
    let show_values = render_settings.show_values;
    let prefix = render_settings.prefix;
    let suffix = render_settings.suffix;
    let link_color = render_settings.link_color;
    let outlined_labels = render_settings.outlined_labels;

    let layout_width = layout.width.max(1.0);
    let layout_height = layout.height.max(1.0);
    let diagram_id = options.diagram_id.as_deref().unwrap_or("sankey");
    let scope_generated_ids = options.diagram_id.is_some();

    const DEFAULT_ASCENT_EM: f64 = 0.9285714286;
    const DEFAULT_DESCENT_EM: f64 = 0.262;
    let label_font_size: f64 = 14.0;
    let label_gap_x: f64 = 6.0;
    let label_hide_values_dy_em: f64 = 0.35;

    let mut min_x: f64 = 0.0;
    let mut min_y: f64 = 0.0;
    let mut max_x = layout_width;
    let mut max_y = layout_height;

    for n in &layout.nodes {
        min_x = min_x.min(n.x0);
        min_y = min_y.min(n.y0);
        max_x = max_x.max(n.x1);
        max_y = max_y.max(n.y1);

        let dy_em = if show_values {
            0.0
        } else {
            label_hide_values_dy_em
        };
        let baseline_y = (n.y0 + n.y1) / 2.0 + dy_em * label_font_size;
        let ascent = label_font_size * DEFAULT_ASCENT_EM;
        let descent = label_font_size * DEFAULT_DESCENT_EM;
        min_y = min_y.min(baseline_y - ascent);
        max_y = max_y.max(baseline_y + descent);
    }

    for l in &layout.links {
        let sw = l.width.max(1.0);
        let half = sw / 2.0;
        let y0 = l.y0.min(l.y1);
        let y1 = l.y0.max(l.y1);
        min_y = min_y.min(y0 - half);
        max_y = max_y.max(y1 + half);
    }

    let vb_w = (max_x - min_x).max(1.0);
    let vb_h = (max_y - min_y).max(1.0);

    let root_spec = root_svg::RootViewportSpec::mermaid(
        root_svg::DiagramBounds::from_view_box(min_x, min_y, vb_w, vb_h),
        use_max_width,
    )
    .with_max_width(root_svg::RootMaxWidth::SvgNumber(vb_w));

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::SANKEY, diagram_id).write_open(
            &mut out,
            root_spec,
            root_svg::RootChrome {
                dom: root_svg::RootDomProfile {
                    fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
                    fixed_style_placement: root_svg::RootStylePlacement::Tail,
                    trailing_newline: false,
                    ..Default::default()
                },
                ..root_svg::RootChrome::new(diagram_id, "sankey")
            },
        )?;
    out.push_str("<style>");
    out.checkpoint()?;
    let css = sankey_css(diagram_id, effective_config);
    out.push_str(&css);
    drop(css);
    out.checkpoint()?;
    out.push_str("</style><g/>");
    out.checkpoint()?;

    let mut uid_count: usize = 0;
    let mut next_generated_id = |prefix: &str| -> String {
        uid_count += 1;
        let local_id = format!("{prefix}{uid_count}");
        if scope_generated_ids {
            scoped_svg_id(diagram_id, &local_id)
        } else {
            local_id
        }
    };

    let terminal_node_index = SankeyTerminalNodeIndex::new(&layout.nodes, &mut next_generated_id);

    out.push_str(r#"<g class="nodes">"#);
    out.checkpoint()?;
    let mut node_palette_receipt = node_palette.begin_terminal_receipt();
    write_sankey_nodes(
        &mut out,
        &layout.nodes,
        node_palette,
        node_palette_receipt.as_mut(),
        &terminal_node_index,
        scope_generated_ids,
        diagram_id,
    )?;
    out.push_str("</g>");
    out.checkpoint()?;
    if let Some(receipt) = node_palette_receipt {
        if !node_palette.record_terminal(receipt) {
            return Err(Error::InvalidModel {
                message: "Sankey node palette receipt did not match the terminal SVG".to_string(),
            });
        }
    }

    let _ = write!(
        &mut out,
        r#"<g class="node-labels" font-size="{font_size}">"#,
        font_size = fmt(label_font_size)
    );
    out.checkpoint()?;
    let mut max_value = 0.0;
    let mut central_node_layer = 0usize;
    for n in &layout.nodes {
        if n.value > max_value {
            max_value = n.value;
            central_node_layer = n.layer;
        }
    }

    let append_labels = |out: &mut BoundedSvgOutput<'_>, class_name: Option<&str>| -> Result<()> {
        for n in &layout.nodes {
            let y = (n.y0 + n.y1) / 2.0;
            let (x, anchor) = if outlined_labels {
                if n.layer < central_node_layer {
                    (n.x0 - label_gap_x, "end")
                } else {
                    (n.x1 + label_gap_x, "start")
                }
            } else if n.x0 < layout_width / 2.0 {
                (n.x1 + label_gap_x, "start")
            } else {
                (n.x0 - label_gap_x, "end")
            };
            let dy = if show_values {
                "0em".to_string()
            } else {
                format!("{}em", fmt(label_hide_values_dy_em))
            };
            let v = (n.value * 100.0).round() / 100.0;
            let text = if show_values {
                format!("{}\n{}{}{}", n.id, prefix, v, suffix)
            } else {
                n.id.clone()
            };
            let class_attr = class_name
                .map(|class_name| format!(r#" class="{}""#, escape_attr(class_name)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"<text{class_attr} x="{x}" y="{y}" dy="{dy}" text-anchor="{anchor}">{text}</text>"#,
                class_attr = class_attr,
                x = fmt(x),
                y = fmt(y),
                dy = dy,
                anchor = anchor,
                text = escape_xml(&text),
            );
            out.checkpoint()?;
        }
        Ok(())
    };
    if outlined_labels {
        append_labels(&mut out, Some("sankey-label-bg"))?;
        append_labels(&mut out, Some("sankey-label-fg"))?;
    } else {
        append_labels(&mut out, None)?;
    }
    out.push_str("</g>");
    out.checkpoint()?;

    out.push_str(r#"<g class="links" fill="none" stroke-opacity="0.5">"#);
    out.checkpoint()?;

    write_sankey_links(
        &mut out,
        &layout.links,
        &terminal_node_index,
        node_palette,
        &link_color,
        &mut next_generated_id,
    )?;

    out.push_str("</g>");
    out.checkpoint()?;
    out.push_str("</svg>");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SankeyNodeLayout;
    use std::fmt;
    use std::ops::Range;

    fn sankey_node(id: impl Into<String>, index: usize, x0: f64, x1: f64) -> SankeyNodeLayout {
        SankeyNodeLayout {
            id: id.into(),
            index,
            depth: index,
            height: 0,
            layer: index,
            value: 1.0,
            x0,
            x1,
            y0: 0.0,
            y1: 20.0,
        }
    }

    fn sankey_link(
        index: usize,
        source: impl Into<String>,
        target: impl Into<String>,
        width: f64,
        y0: f64,
        y1: f64,
    ) -> crate::model::SankeyLinkLayout {
        crate::model::SankeyLinkLayout {
            index,
            source: source.into(),
            target: target.into(),
            value: width,
            width,
            y0,
            y1,
        }
    }

    fn sankey_layout(
        nodes: Vec<SankeyNodeLayout>,
        links: Vec<crate::model::SankeyLinkLayout>,
    ) -> SankeyDiagramLayout {
        SankeyDiagramLayout {
            bounds: None,
            width: 100.0,
            height: 100.0,
            node_width: 10.0,
            node_padding: 12.0,
            nodes,
            links,
        }
    }

    fn baseline_node_palette(layout: &SankeyDiagramLayout) -> crate::sankey::SankeyNodePalettePlan {
        let work_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        crate::sankey::SankeyNodePalettePlan::resolve(
            None,
            &merman_core::MermaidConfig::default(),
            layout,
            &work_meter,
        )
        .expect("resolve baseline Sankey node palette")
    }

    fn build_terminal_node_index(nodes: &[SankeyNodeLayout]) -> SankeyTerminalNodeIndex<'_> {
        let mut uid_count = 0;
        let mut next_generated_id = |prefix: &str| {
            uid_count += 1;
            format!("{prefix}{uid_count}")
        };
        SankeyTerminalNodeIndex::new(nodes, &mut next_generated_id)
    }

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn sankey_nodes_stop_after_the_first_svg_sink_failure() {
        let nodes = (0..4)
            .map(|index| crate::model::SankeyNodeLayout {
                id: format!("node-{index}"),
                index,
                depth: index,
                height: 0,
                layer: index,
                value: 1.0,
                x0: index as f64,
                x1: index as f64 + 10.0,
                y0: 0.0,
                y1: 20.0,
            })
            .collect::<Vec<_>>();
        let layout = crate::model::SankeyDiagramLayout {
            bounds: None,
            width: 100.0,
            height: 100.0,
            node_width: 10.0,
            node_padding: 12.0,
            nodes: nodes.clone(),
            links: Vec::new(),
        };
        let work_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let node_palette = crate::sankey::SankeyNodePalettePlan::resolve(
            None,
            &merman_core::MermaidConfig::default(),
            &layout,
            &work_meter,
        )
        .expect("resolve baseline Sankey node palette");
        let terminal_node_index = build_terminal_node_index(&nodes);
        let mut out = RejectAfterFirstWrite::default();

        let error = write_sankey_nodes(
            &mut out,
            &nodes,
            &node_palette,
            None,
            &terminal_node_index,
            false,
            "sankey",
        )
        .expect_err("the rejecting sink must stop Sankey node emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Sankey node emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn sankey_endpoint_index_preserves_representative_link_svg_and_linear_lookup_count() {
        let layout = sankey_layout(
            vec![
                sankey_node("A", 0, 0.0, 10.0),
                sankey_node("B", 1, 40.0, 50.0),
                sankey_node("C", 2, 80.0, 90.0),
            ],
            vec![
                sankey_link(0, "A", "B", 3.0, 5.0, 7.0),
                sankey_link(1, "B", "C", 4.0, 9.0, 11.0),
            ],
        );
        let node_palette = baseline_node_palette(&layout);
        let terminal_node_index = build_terminal_node_index(&layout.nodes);
        let mut uid_count = layout.nodes.len();
        let mut next_generated_id = |prefix: &str| {
            uid_count += 1;
            format!("{prefix}{uid_count}")
        };
        let mut out = String::new();

        write_sankey_links(
            &mut out,
            &layout.links,
            &terminal_node_index,
            &node_palette,
            "#445566",
            &mut next_generated_id,
        )
        .expect("write representative Sankey links");

        assert_eq!(
            out,
            concat!(
                r##"<g class="link" style="mix-blend-mode: multiply;"><path d="M10,5C25,5,25,7,40,7" stroke="#445566" stroke-width="3"/></g>"##,
                r##"<g class="link" style="mix-blend-mode: multiply;"><path d="M50,9C65,9,65,11,80,11" stroke="#445566" stroke-width="4"/></g>"##,
            )
        );
        assert_eq!(
            terminal_node_index.lookup_stats(),
            SankeyEndpointLookupStats {
                indexed_node_visits: layout.nodes.len(),
                endpoint_lookups: 2 * layout.links.len(),
            }
        );
    }

    #[test]
    fn sankey_endpoint_index_preserves_duplicate_first_winner_and_terminal_uid_order() {
        let layout = sankey_layout(
            vec![
                sankey_node("A", 0, 0.0, 10.0),
                sankey_node("A", 1, 20.0, 30.0),
                sankey_node("B", 2, 50.0, 60.0),
            ],
            vec![sankey_link(0, "A", "B", 2.0, 4.0, 6.0)],
        );
        let node_palette = baseline_node_palette(&layout);
        let terminal_node_index = build_terminal_node_index(&layout.nodes);
        let entry = terminal_node_index
            .by_id
            .get("A")
            .expect("duplicate node entry");
        assert!(std::ptr::eq(entry.first_node, &layout.nodes[0]));
        assert_eq!(entry.terminal_uid, "node-2");
        let mut uid_count = layout.nodes.len();
        let mut next_generated_id = |prefix: &str| {
            uid_count += 1;
            format!("{prefix}{uid_count}")
        };
        let mut out = String::new();

        write_sankey_links(
            &mut out,
            &layout.links,
            &terminal_node_index,
            &node_palette,
            "#778899",
            &mut next_generated_id,
        )
        .expect("write duplicate-ID Sankey link");

        assert_eq!(
            out,
            r##"<g class="link" style="mix-blend-mode: multiply;"><path d="M10,4C30,4,30,6,50,6" stroke="#778899" stroke-width="2"/></g>"##
        );
        assert_eq!(
            terminal_node_index.lookup_stats(),
            SankeyEndpointLookupStats {
                indexed_node_visits: 3,
                endpoint_lookups: 2,
            }
        );
    }

    #[test]
    fn sankey_endpoint_index_preserves_missing_endpoint_error_precedence() {
        let nodes = vec![
            sankey_node("A", 0, 0.0, 10.0),
            sankey_node("B", 1, 40.0, 50.0),
        ];
        let source_missing = sankey_link(0, "missing-source", "missing-target", 1.0, 0.0, 0.0);
        let terminal_node_index = build_terminal_node_index(&nodes);

        let error = terminal_node_index
            .resolve_link_endpoints(&source_missing)
            .expect_err("missing source must fail first");
        let Error::InvalidModel { message } = error else {
            panic!("expected invalid Sankey model error, got {error}");
        };
        assert_eq!(message, "missing source node missing-source");
        assert_eq!(
            terminal_node_index.lookup_stats(),
            SankeyEndpointLookupStats {
                indexed_node_visits: 2,
                endpoint_lookups: 1,
            }
        );

        let target_missing = sankey_link(0, "A", "missing-target", 1.0, 0.0, 0.0);
        let terminal_node_index = build_terminal_node_index(&nodes);
        let error = terminal_node_index
            .resolve_link_endpoints(&target_missing)
            .expect_err("missing target must fail after resolving the source");
        let Error::InvalidModel { message } = error else {
            panic!("expected invalid Sankey model error, got {error}");
        };
        assert_eq!(message, "missing target node missing-target");
        assert_eq!(
            terminal_node_index.lookup_stats(),
            SankeyEndpointLookupStats {
                indexed_node_visits: 2,
                endpoint_lookups: 2,
            }
        );
    }

    #[test]
    fn sankey_endpoint_index_long_chain_has_exact_linear_operation_counts() {
        // Nodes plus links stay one item below the constrained 16,000-item model boundary. This
        // is a structural work-counter check, not a wall-clock benchmark.
        const NODE_COUNT: usize = 8_000;
        let nodes = (0..NODE_COUNT)
            .map(|index| {
                sankey_node(
                    format!("node-{index}"),
                    index,
                    index as f64,
                    index as f64 + 1.0,
                )
            })
            .collect::<Vec<_>>();
        let links = (0..NODE_COUNT - 1)
            .map(|index| {
                sankey_link(
                    index,
                    format!("node-{index}"),
                    format!("node-{}", index + 1),
                    1.0,
                    index as f64,
                    index as f64 + 1.0,
                )
            })
            .collect::<Vec<_>>();
        let terminal_node_index = build_terminal_node_index(&nodes);

        for link in &links {
            terminal_node_index
                .resolve_link_endpoints(link)
                .expect("long-chain endpoint lookup");
        }

        assert_eq!(
            terminal_node_index.lookup_stats(),
            SankeyEndpointLookupStats {
                indexed_node_visits: NODE_COUNT,
                endpoint_lookups: 2 * links.len(),
            }
        );
    }
}
