use super::super::*;

fn write_sankey_nodes(
    out: &mut impl SvgOutput,
    nodes: &[crate::model::SankeyNodeLayout],
    node_palette: &crate::sankey::SankeyNodePalettePlan,
    mut receipt: Option<&mut crate::sankey::SankeyNodePaletteReceipt>,
    node_uid_by_id: &std::collections::HashMap<String, String>,
    scope_generated_ids: bool,
    diagram_id: &str,
) -> Result<()> {
    for (node_index, node) in nodes.iter().enumerate() {
        let node_uid = node_uid_by_id.get(&node.id).cloned().unwrap_or_else(|| {
            if scope_generated_ids {
                scoped_svg_id(diagram_id, "node-0")
            } else {
                "node-0".to_string()
            }
        });
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
            id = escape_xml(&node_uid),
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

    let mut node_uid_by_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for n in &layout.nodes {
        node_uid_by_id.insert(n.id.clone(), next_generated_id("node-"));
    }

    out.push_str(r#"<g class="nodes">"#);
    out.checkpoint()?;
    let mut node_palette_receipt = node_palette.begin_terminal_receipt();
    write_sankey_nodes(
        &mut out,
        &layout.nodes,
        node_palette,
        node_palette_receipt.as_mut(),
        &node_uid_by_id,
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

    for l in &layout.links {
        let source = layout
            .nodes
            .iter()
            .find(|n| n.id == l.source)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing source node {}", l.source),
            })?;
        let target = layout
            .nodes
            .iter()
            .find(|n| n.id == l.target)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing target node {}", l.target),
            })?;

        let sx = source.x1;
        let tx = target.x0;
        let mx = (sx + tx) / 2.0;
        let path_d = format!(
            "M{sx},{y0}C{mx},{y0},{mx},{y1},{tx},{y1}",
            sx = fmt(sx),
            y0 = fmt(l.y0),
            mx = fmt(mx),
            y1 = fmt(l.y1),
            tx = fmt(tx),
        );

        out.push_str(r#"<g class="link" style="mix-blend-mode: multiply;">"#);
        out.checkpoint()?;

        let stroke = match link_color.as_str() {
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
                    &mut out,
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

        let stroke_width = l.width.max(1.0);
        let _ = write!(
            &mut out,
            r#"<path d="{d}" stroke="{stroke}" stroke-width="{sw}"/></g>"#,
            d = escape_xml(&path_d),
            stroke = escape_attr(&stroke),
            sw = fmt(stroke_width),
        );
        out.checkpoint()?;
    }

    out.push_str("</g>");
    out.checkpoint()?;
    out.push_str("</svg>");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::ops::Range;

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
        let node_uid_by_id = std::collections::HashMap::new();
        let mut out = RejectAfterFirstWrite::default();

        let error = write_sankey_nodes(
            &mut out,
            &nodes,
            &node_palette,
            None,
            &node_uid_by_id,
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
}
