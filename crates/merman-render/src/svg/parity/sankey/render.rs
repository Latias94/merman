use super::super::*;
use crate::sankey::{SankeyLinkPaint, SankeyNodePalette};
use merman_render_charts::sankey::{SankeySvgBody, SankeySvgLink, SankeySvgLinkPaint};

fn write_sankey_links(
    out: &mut impl SvgOutput,
    body: &mut SankeySvgBody<'_>,
    layout: &SankeyDiagramLayout,
    node_palette: &SankeyNodePalette,
) -> Result<()> {
    body.write_links(
        out,
        node_palette.link_blend_mode_css(),
        node_palette.link_stroke_opacity(),
        |link_index| {
            let prepared = node_palette.terminal_link(link_index)?;
            Some(SankeySvgLink {
                source: &layout.nodes[prepared.source_layout_index],
                target: &layout.nodes[prepared.target_layout_index],
                paint: match prepared.paint {
                    SankeyLinkPaint::Solid(css) => SankeySvgLinkPaint::Solid(css),
                    SankeyLinkPaint::Gradient { source, target } => {
                        SankeySvgLinkPaint::Gradient { source, target }
                    }
                },
            })
        },
    )
}

pub(crate) fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    style: &crate::sankey::SankeyStyle,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let node_palette = &style.palette;
    let show_values = style.show_values;
    let label_texts = node_palette.labels();
    if label_texts.len() != layout.nodes.len() {
        return Err(Error::InvalidModel {
            message: "Sankey prepared labels do not match the terminal node count".to_string(),
        });
    }
    let outlined_labels = style.outlined_labels;

    let layout_width = layout.width.max(1.0);
    let layout_height = layout.height.max(1.0);
    let diagram_id = options.diagram_id_or("sankey");
    let scope_generated_ids = options.has_explicit_diagram_id();

    const DEFAULT_ASCENT_EM: f64 = 0.9285714286;
    const DEFAULT_DESCENT_EM: f64 = 0.262;
    let label_font_size = style.label_size;
    let label_gap_x: f64 = 6.0;
    let label_hide_values_dy_em: f64 = 0.35;
    let measurer = options.text_measurer();
    let label_style = crate::text::TextStyle {
        font_family: Some(style.common_css.font_family().to_string()),
        font_size: label_font_size,
        font_weight: style.label_weight.map(|weight| weight.to_string()),
        font_style: None,
    };
    let mut max_value = 0.0;
    let mut central_node_layer = 0usize;
    for node in &layout.nodes {
        options.checkpoint_emit()?;
        if node.value > max_value {
            max_value = node.value;
            central_node_layer = node.layer;
        }
    }
    let label_position = |node: &crate::model::SankeyNodeLayout| {
        let on_right = if outlined_labels {
            node.layer >= central_node_layer
        } else {
            node.x0 < layout_width / 2.0
        };
        if on_right {
            (node.x1 + label_gap_x, "start")
        } else {
            (node.x0 - label_gap_x, "end")
        }
    };

    let mut min_x: f64 = 0.0;
    let mut min_y: f64 = 0.0;
    let mut max_x = layout_width;
    let mut max_y = layout_height;

    for (n, label) in layout.nodes.iter().zip(label_texts) {
        options.checkpoint_emit()?;
        min_x = min_x.min(n.x0);
        min_y = min_y.min(n.y0);
        max_x = max_x.max(n.x1);
        max_y = max_y.max(n.y1);

        let (label_x, anchor) = label_position(n);
        let label_width = measurer.measure_svg_raw_text_bbox_width_px(label.text(), &label_style);
        if label_width > 0.0 {
            let label_left = if anchor == "end" {
                label_x - label_width
            } else {
                label_x
            };
            min_x = min_x.min(label_left);
            max_x = max_x.max(label_left + label_width);
        }

        let dy_em = if show_values {
            0.0
        } else {
            label_hide_values_dy_em
        };
        let baseline_y = (n.y0 + n.y1) / 2.0 + dy_em * label_font_size;
        let (ascent, descent) = crate::text::svg_text_vertical_extents_px(
            measurer,
            label.text(),
            &label_style,
            (
                label_font_size * DEFAULT_ASCENT_EM,
                label_font_size * DEFAULT_DESCENT_EM,
            ),
        );
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
        style.use_max_width,
    )
    .with_max_width(root_svg::RootMaxWidth::SvgNumber(vb_w));

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::SANKEY, diagram_id)
        .write_open(
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
    super::css::write_sankey_css(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        diagram_id,
        style,
    )?;
    out.push_str("</style><g/>");
    out.checkpoint()?;

    let mut body = SankeySvgBody::new(layout, scope_generated_ids.then_some(diagram_id));
    if node_palette.links_before_nodes() {
        write_sankey_links(&mut out, &mut body, layout, node_palette)?;
    }

    body.write_nodes(&mut out, |index, id| node_palette.fill_for(index, id))?;
    let label_weight = style
        .label_weight
        .map_or_else(String::new, |weight| format!(r#" font-weight="{weight}""#));
    let _ = write!(
        &mut out,
        r#"<g class="node-labels" font-size="{font_size}"{label_weight}>"#,
        font_size = fmt(label_font_size)
    );
    out.checkpoint()?;

    let append_labels = |out: &mut BoundedSvgOutput<'_>,
                         surface: crate::sankey::SankeyLabelSurface|
     -> Result<()> {
        let class_name = surface.class_name();
        for (n, label) in layout.nodes.iter().zip(label_texts) {
            options.checkpoint_emit()?;
            let y = (n.y0 + n.y1) / 2.0;
            let (x, anchor) = label_position(n);
            let dy = if show_values {
                "0em".to_string()
            } else {
                format!("{}em", fmt(label_hide_values_dy_em))
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
            super::super::util::escape_xml_raw_into(out, label.text());
            out.push_str("</text>");
            out.checkpoint()?;
        }
        Ok(())
    };
    if outlined_labels {
        append_labels(
            &mut out,
            crate::sankey::SankeyLabelSurface::OutlineBackground,
        )?;
        append_labels(
            &mut out,
            crate::sankey::SankeyLabelSurface::OutlineForeground,
        )?;
    } else {
        append_labels(&mut out, crate::sankey::SankeyLabelSurface::Plain)?;
    }
    out.push_str("</g>");
    out.checkpoint()?;
    if !node_palette.links_before_nodes() {
        write_sankey_links(&mut out, &mut body, layout, node_palette)?;
    }
    out.push_str("</svg>");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SankeyNodeLayout;

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

    fn baseline_node_palette(layout: &SankeyDiagramLayout) -> crate::sankey::SankeyNodePalette {
        let work_meter = std::sync::Arc::new(crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        crate::sankey::SankeyNodePalette::prepare_labels_and_nodes(
            merman_model::presentation::PresentationConfig::default().as_value(),
            layout,
            &work_meter,
        )
        .expect("resolve baseline Sankey node palette")
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
        let mut node_palette = baseline_node_palette(&layout);
        node_palette
            .prepare_links(&layout, "#445566", &test_link_work_meter())
            .expect("prepare representative links");
        let mut body = SankeySvgBody::new(&layout, None);
        let mut out = String::new();

        write_sankey_links(&mut out, &mut body, &layout, &node_palette)
            .expect("write representative Sankey links");

        assert_eq!(
            out,
            concat!(
                r#"<g class="links" fill="none" stroke-opacity="0.5">"#,
                r##"<g class="link" style="mix-blend-mode: multiply;"><path d="M10,5C25,5,25,7,40,7" stroke="#445566" stroke-width="3"/></g>"##,
                r##"<g class="link" style="mix-blend-mode: multiply;"><path d="M50,9C65,9,65,11,80,11" stroke="#445566" stroke-width="4"/></g>"##,
                "</g>",
            )
        );
        assert_eq!(
            node_palette.link_lookup_stats(),
            (layout.nodes.len(), 2 * layout.links.len())
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
        let mut node_palette = baseline_node_palette(&layout);
        node_palette
            .prepare_links(&layout, "source", &test_link_work_meter())
            .expect("prepare duplicate-ID link");
        let prepared = node_palette
            .terminal_link(0)
            .expect("prepared duplicate link");
        assert_eq!(prepared.source_layout_index, 0);
        assert!(matches!(
            prepared.paint,
            crate::sankey::SankeyLinkPaint::Solid("#4e79a7")
        ));
        let mut body = SankeySvgBody::new(&layout, None);
        let mut out = String::new();
        body.write_nodes(&mut out, |index, id| node_palette.fill_for(index, id))
            .expect("write duplicate-ID Sankey nodes");
        assert!(!out.contains(r#"id="node-1""#));
        assert_eq!(out.matches(r#"id="node-2""#).count(), 2);
        assert_eq!(out.matches(r#"id="node-3""#).count(), 1);
        out.clear();

        write_sankey_links(&mut out, &mut body, &layout, &node_palette)
            .expect("write duplicate-ID Sankey link");

        assert_eq!(
            out,
            r##"<g class="links" fill="none" stroke-opacity="0.5"><g class="link" style="mix-blend-mode: multiply;"><path d="M10,4C30,4,30,6,50,6" stroke="#4e79a7" stroke-width="2"/></g></g>"##
        );
        assert_eq!(node_palette.link_lookup_stats(), (3, 2));
    }

    fn test_link_work_meter() -> crate::resources::OperationWorkMeter {
        crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
    }

    #[test]
    fn prepared_links_preserve_missing_endpoint_error_precedence() {
        for (source, target, expected, lookups) in [
            (
                "missing-source",
                "missing-target",
                "missing source node missing-source",
                1,
            ),
            (
                "A",
                "missing-target",
                "missing target node missing-target",
                2,
            ),
        ] {
            let layout = sankey_layout(
                vec![
                    sankey_node("A", 0, 0.0, 10.0),
                    sankey_node("B", 1, 40.0, 50.0),
                ],
                vec![sankey_link(0, source, target, 1.0, 0.0, 0.0)],
            );
            let mut palette = baseline_node_palette(&layout);
            let error = palette
                .prepare_links(&layout, "gradient", &test_link_work_meter())
                .expect_err("missing endpoint fails during preparation");
            let Error::InvalidModel { message } = error else {
                panic!("expected invalid Sankey model error, got {error}");
            };
            assert_eq!(message, expected);
            assert_eq!(palette.link_lookup_stats(), (2, lookups));
        }
    }

    #[test]
    fn prepared_link_modes_preserve_gradient_ids_and_reuse_final_node_paints() {
        let layout = sankey_layout(
            vec![
                sankey_node("A", 0, 0.0, 10.0),
                sankey_node("B", 1, 40.0, 50.0),
            ],
            vec![sankey_link(0, "A", "B", 0.25, 5.0, 7.0)],
        );
        for (mode, expected_stroke) in [
            ("source", "var(--node)"),
            ("target", "currentColor"),
            ("var(--link)", "var(--link)"),
            ("gradient", "url(#example-linearGradient-3)"),
        ] {
            let config = crate::test_support::presentation_config(serde_json::json!({
                "sankey": {"nodeColors": {"A": "var(--node)", "B": "currentColor"}}
            }));
            let mut palette = crate::sankey::SankeyNodePalette::prepare_labels_and_nodes(
                config.as_value(),
                &layout,
                &std::sync::Arc::new(test_link_work_meter()),
            )
            .expect("prepare explicit palette");
            palette
                .prepare_links(&layout, mode, &test_link_work_meter())
                .expect("prepare link mode");
            for _ in 0..2 {
                let request = SvgRenderOptions {
                    diagram_id: Some("example".into()),
                    ..SvgRenderOptions::default()
                };
                let mut out = String::new();
                with_test_svg_execution(crate::DiagramFamilyId::SANKEY, &request, |execution| {
                    let mut body =
                        SankeySvgBody::new(&layout, Some(execution.diagram_id_or("sankey")));
                    write_sankey_links(&mut out, &mut body, &layout, &palette)
                })
                .expect("emit prepared links");
                assert!(out.contains(&format!("stroke=\"{expected_stroke}\"")));
                assert!(out.contains("stroke-width=\"1\""));
                if mode == "gradient" {
                    assert!(out.contains("x1=\"10\" x2=\"40\""));
                    assert!(out.contains("stop-color=\"var(--node)\""));
                    assert!(out.contains("stop-color=\"currentColor\""));
                } else {
                    assert!(!out.contains("linearGradient"));
                }
            }
        }
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
        let layout = sankey_layout(nodes, links);
        let mut palette = baseline_node_palette(&layout);
        palette
            .prepare_links(&layout, "gradient", &test_link_work_meter())
            .expect("prepare long-chain links");
        assert_eq!(
            palette.link_lookup_stats(),
            (NODE_COUNT, 2 * layout.links.len())
        );
    }
}
