use merman_model::OperationPhase;
use merman_render_core::{
    Error, Result,
    resources::OperationWorkMeter,
    root_svg::{
        DiagramBounds, RootChrome, RootDomProfile, RootMaxWidth, RootStylePlacement,
        RootViewportContext, RootViewportSpec, RootedSvg, SvgRootFixedHeightPlacement,
    },
    svg_css::SvgId,
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::{TextMeasurer, TextStyle, svg_text_vertical_extents_px},
};

use super::super::{
    SankeyDiagramLayout, SankeyStyle,
    css::write_sankey_css,
    theme::{SankeyLinkPaint, SankeyNodePalette},
};
use super::{SankeySvgBody, SankeySvgBodyStyle, SankeySvgLink, SankeySvgLinkPaint};

fn prepared_svg_link<'a>(
    layout: &'a SankeyDiagramLayout,
    node_palette: &'a SankeyNodePalette,
    link_index: usize,
) -> Option<SankeySvgLink<'a>> {
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
}

/// Emits the complete Sankey SVG before shared canvas and terminal admission.
pub fn render_sankey_diagram_svg(
    layout: &SankeyDiagramLayout,
    style: &SankeyStyle,
    diagram_id: SvgId<'_>,
    scope_generated_ids: bool,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
    measurer: &dyn TextMeasurer,
) -> Result<RootedSvg> {
    let node_palette = &style.palette;
    let label_texts = node_palette.labels();
    if label_texts.len() != layout.nodes.len() {
        return Err(Error::InvalidModel {
            message: "Sankey prepared labels do not match the terminal node count".to_string(),
        });
    }
    let layout_width = layout.width.max(1.0);
    let layout_height = layout.height.max(1.0);
    const DEFAULT_ASCENT_EM: f64 = 0.9285714286;
    const DEFAULT_DESCENT_EM: f64 = 0.262;
    let label_font_size = style.label_size;
    let label_style = TextStyle {
        font_family: Some(style.common_css.font_family().to_string()),
        font_size: label_font_size,
        font_weight: style.label_weight.map(|weight| weight.to_string()),
        font_style: None,
    };
    let mut max_value = 0.0;
    let mut central_node_layer = 0usize;
    for node in &layout.nodes {
        work_meter.checkpoint(OperationPhase::Emit)?;
        if node.value > max_value {
            max_value = node.value;
            central_node_layer = node.layer;
        }
    }
    let body_style = SankeySvgBodyStyle {
        links_before_nodes: node_palette.links_before_nodes(),
        link_blend_mode_css: node_palette.link_blend_mode_css(),
        link_stroke_opacity: node_palette.link_stroke_opacity(),
        outlined_labels: style.outlined_labels,
        show_values: style.show_values,
        label_font_size,
        label_weight: style.label_weight,
        central_node_layer,
    };

    let mut min_x: f64 = 0.0;
    let mut min_y: f64 = 0.0;
    let mut max_x = layout_width;
    let mut max_y = layout_height;

    for (n, label) in layout.nodes.iter().zip(label_texts) {
        work_meter.checkpoint(OperationPhase::Emit)?;
        min_x = min_x.min(n.x0);
        min_y = min_y.min(n.y0);
        max_x = max_x.max(n.x1);
        max_y = max_y.max(n.y1);

        let (label_x, anchor) = body_style.label_position(n, layout_width);
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

        let dy_em = body_style.label_dy_em();
        let baseline_y = (n.y0 + n.y1) / 2.0 + dy_em * label_font_size;
        let (ascent, descent) = svg_text_vertical_extents_px(
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

    let root_spec = RootViewportSpec::mermaid(
        DiagramBounds::from_view_box(min_x, min_y, vb_w, vb_h),
        style.use_max_width,
    )
    .with_max_width(RootMaxWidth::SvgNumber(vb_w));

    let mut out = BoundedSvgOutput::new(work_meter);
    let root_document = root_viewport.write_open(
        &mut out,
        root_spec,
        RootChrome {
            dom: RootDomProfile {
                fixed_height_placement: SvgRootFixedHeightPlacement::AfterXmlns,
                fixed_style_placement: RootStylePlacement::Tail,
                trailing_newline: false,
                ..Default::default()
            },
            ..RootChrome::new(diagram_id, "sankey")
        },
    )?;
    out.push_str("<style>");
    out.checkpoint()?;
    write_sankey_css(&mut XmlTextOutput::new(&mut out), diagram_id, style)?;
    out.push_str("</style><g/>");
    out.checkpoint()?;

    let mut body = SankeySvgBody::new(layout, scope_generated_ids.then_some(diagram_id));
    body.write_contents(
        &mut out,
        &body_style,
        label_texts.iter().map(|label| label.text()),
        |index, id| node_palette.fill_for(index, id),
        |link_index| prepared_svg_link(layout, node_palette, link_index),
        work_meter,
    )?;
    out.push_str("</svg>");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sankey::{SankeyLinkLayout, SankeyNodeLayout, test_support::presentation_config};
    use merman_render_core::{resources::RenderResourcePolicy, svg_css::SvgDiagramIdProjection};
    use std::sync::Arc;

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
    ) -> SankeyLinkLayout {
        SankeyLinkLayout {
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
        links: Vec<SankeyLinkLayout>,
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

    fn baseline_node_palette(layout: &SankeyDiagramLayout) -> SankeyNodePalette {
        let work_meter = Arc::new(test_link_work_meter());
        SankeyNodePalette::prepare_labels_and_nodes(
            merman_model::presentation::PresentationConfig::default().as_value(),
            layout,
            &work_meter,
        )
        .expect("resolve baseline Sankey node palette")
    }

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
            |link_index| prepared_svg_link(layout, node_palette, link_index),
        )
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
        assert!(matches!(prepared.paint, SankeyLinkPaint::Solid("#4e79a7")));
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

    fn test_link_work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
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
            let config = presentation_config(serde_json::json!({
                "sankey": {"nodeColors": {"A": "var(--node)", "B": "currentColor"}}
            }));
            let mut palette = SankeyNodePalette::prepare_labels_and_nodes(
                config.as_value(),
                &layout,
                &Arc::new(test_link_work_meter()),
            )
            .expect("prepare explicit palette");
            palette
                .prepare_links(&layout, mode, &test_link_work_meter())
                .expect("prepare link mode");
            for _ in 0..2 {
                let meter = test_link_work_meter();
                let projection = SvgDiagramIdProjection::new(&meter);
                let diagram_id = SvgId::new("example").unwrap().with_projection(&projection);
                let mut body = SankeySvgBody::new(&layout, Some(diagram_id));
                let mut out = String::new();
                write_sankey_links(&mut out, &mut body, &layout, &palette)
                    .expect("emit prepared links");
                projection.finish().expect("admit scoped Sankey IDs");
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
