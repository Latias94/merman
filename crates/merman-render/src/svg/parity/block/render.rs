use super::super::*;
use crate::block::{
    BlockLabelBackgroundPlan, BlockNodeLabelPaintPlan, BlockNodePaintThemePlan, BlockNodeShellKind,
    BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry, BlockTypographyThemePlan,
    block_label_is_effectively_empty,
};
use crate::model::{LayoutEdge, LayoutPoint};
use crate::svg::parity::roughjs_common::{closed_path_d_from_points, ops_to_svg_path_d};

// Block diagram SVG renderer implementation (split from parity.rs).

// Mermaid's Block renderer expands the visible group bounds by a fixed five pixels. Block has no
// `diagramPadding` configuration field, so this must not be driven by an opaque config key.
const BLOCK_ROOT_VIEWBOX_PADDING: f64 = 5.0;

struct BlockLayoutEdgeIndex<'a> {
    by_id: std::collections::HashMap<&'a str, &'a LayoutEdge>,
}

impl<'a> BlockLayoutEdgeIndex<'a> {
    fn new(edges: &'a [LayoutEdge]) -> Self {
        let mut by_id = std::collections::HashMap::with_capacity(edges.len());
        for edge in edges {
            // Preserve the renderer's historical `iter().find()` first-match behavior for
            // malformed layouts with duplicate edge ids.
            let _ = by_id.entry(edge.id.as_str()).or_insert(edge);
        }
        Self { by_id }
    }

    fn get(&self, id: &str) -> Option<&'a LayoutEdge> {
        self.by_id.get(id).copied()
    }
}

/// Returns a conservative radius for one of the marker instances emitted by the shared Mermaid
/// marker set.
///
/// Marker definitions live in `markers.rs` and use `userSpaceOnUse`.  The generic SVG bounds
/// scanner intentionally ignores `<defs>`, so Block has to account for the referenced marker at
/// its path endpoint.  These values mirror the source geometry and marker viewport attributes;
/// the result is a geometric bound, not a tuned padding constant.
fn block_marker_extent_radius(marker: &str) -> Option<f64> {
    enum Shape<'a> {
        Polygon(&'a [(f64, f64)]),
        Circle { cx: f64, cy: f64, radius: f64 },
    }

    const POINT_END: &[(f64, f64)] = &[(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)];
    const POINT_START: &[(f64, f64)] = &[(0.0, 5.0), (10.0, 10.0), (10.0, 0.0)];

    let (
        view_box_width,
        view_box_height,
        marker_width,
        marker_height,
        ref_x,
        ref_y,
        stroke_width,
        shape,
    ) = match marker {
        "pointEnd" => (
            10.0,
            10.0,
            8.0,
            8.0,
            5.0,
            5.0,
            1.0,
            Shape::Polygon(POINT_END),
        ),
        "pointStart" => (
            10.0,
            10.0,
            8.0,
            8.0,
            4.5,
            5.0,
            1.0,
            Shape::Polygon(POINT_START),
        ),
        "circleEnd" => (
            10.0,
            10.0,
            11.0,
            11.0,
            11.0,
            5.0,
            1.0,
            Shape::Circle {
                cx: 5.0,
                cy: 5.0,
                radius: 5.0,
            },
        ),
        "circleStart" => (
            10.0,
            10.0,
            11.0,
            11.0,
            -1.0,
            5.0,
            1.0,
            Shape::Circle {
                cx: 5.0,
                cy: 5.0,
                radius: 5.0,
            },
        ),
        "crossEnd" => (
            11.0,
            11.0,
            11.0,
            11.0,
            12.0,
            5.2,
            2.0,
            Shape::Polygon(&[(1.0, 1.0), (10.0, 10.0), (10.0, 1.0), (1.0, 10.0)]),
        ),
        "crossStart" => (
            11.0,
            11.0,
            11.0,
            11.0,
            -1.0,
            5.2,
            2.0,
            Shape::Polygon(&[(1.0, 1.0), (10.0, 10.0), (10.0, 1.0), (1.0, 10.0)]),
        ),
        _ => return None,
    };

    let scale_x = marker_width / view_box_width;
    let scale_y = marker_height / view_box_height;
    let radius = match shape {
        Shape::Polygon(points) => points
            .iter()
            .map(|(x, y)| {
                let dx = (x - ref_x) * scale_x;
                let dy = (y - ref_y) * scale_y;
                dx.hypot(dy)
            })
            .fold(0.0, f64::max),
        Shape::Circle { cx, cy, radius } => {
            let center_dx = (cx - ref_x) * scale_x;
            let center_dy = (cy - ref_y) * scale_y;
            // The axis-aligned envelope is deliberately conservative for a rotated marker.
            center_dx.abs().hypot(center_dy.abs()) + radius * scale_x.max(scale_y)
        }
    };

    Some(radius + stroke_width * scale_x.max(scale_y) / 2.0)
}

fn include_block_marker_bounds(
    bounds: &mut Option<(f64, f64, f64, f64)>,
    point: &LayoutPoint,
    marker: &str,
) {
    let Some(radius) = block_marker_extent_radius(marker) else {
        return;
    };
    let candidate = (
        point.x - radius,
        point.y - radius,
        point.x + radius,
        point.y + radius,
    );
    if let Some(existing) = bounds.as_mut() {
        existing.0 = existing.0.min(candidate.0);
        existing.1 = existing.1.min(candidate.1);
        existing.2 = existing.2.max(candidate.2);
        existing.3 = existing.3.max(candidate.3);
    } else {
        *bounds = Some(candidate);
    }
}

fn union_block_bounds(bounds: &mut (f64, f64, f64, f64), candidate: (f64, f64, f64, f64)) {
    bounds.0 = bounds.0.min(candidate.0);
    bounds.1 = bounds.1.min(candidate.1);
    bounds.2 = bounds.2.max(candidate.2);
    bounds.3 = bounds.3.max(candidate.3);
}

fn block_edge_start_marker_inset(arrow: Option<&str>) -> f64 {
    match arrow.unwrap_or("").trim() {
        "arrow_point" => 4.5,
        _ => 0.0,
    }
}

fn block_edge_end_marker_inset(arrow: Option<&str>) -> f64 {
    match arrow.unwrap_or("").trim() {
        "arrow_point" => 4.0,
        _ => 0.0,
    }
}

fn move_point_towards(point: &LayoutPoint, target: &LayoutPoint, distance: f64) -> LayoutPoint {
    if distance.abs() <= 1e-12 {
        return point.clone();
    }
    let dx = target.x - point.x;
    let dy = target.y - point.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 1e-12 {
        return point.clone();
    }
    LayoutPoint {
        x: point.x + dx / len * distance,
        y: point.y + dy / len * distance,
    }
}

struct BlockEdgePoints {
    /// Points before marker insets, matching Mermaid's `data-points` payload.
    data_points: Vec<LayoutPoint>,
    /// Points after marker insets, matching the emitted path and marker anchors.
    rendered_points: Vec<LayoutPoint>,
}

/// Prepare the exact path from source endpoints and final layout geometry.
pub(crate) fn block_edge_path_data(
    edge: &merman_core::diagrams::block::BlockEdgeRenderModel,
    layout_edge: &LayoutEdge,
    shape_geometries_by_id: &std::collections::HashMap<&str, &BlockShapeGeometry>,
) -> String {
    let points = block_render_edge_points(edge, layout_edge, shape_geometries_by_id);
    super::super::curve::curve_basis_path_d_and_bounds(&points.rendered_points).0
}

fn block_render_edge_points(
    edge: &merman_core::diagrams::block::BlockEdgeRenderModel,
    layout_edge: &LayoutEdge,
    shape_geometries_by_id: &std::collections::HashMap<&str, &BlockShapeGeometry>,
) -> BlockEdgePoints {
    let data_points = match (
        shape_geometries_by_id.get(edge.start.as_str()),
        shape_geometries_by_id.get(edge.end.as_str()),
    ) {
        (Some(from), Some(to)) => {
            let mid = layout_edge.points.get(1).cloned().unwrap_or(LayoutPoint {
                x: from.allocated.x + (to.allocated.x - from.allocated.x) / 2.0,
                y: from.allocated.y + (to.allocated.y - from.allocated.y) / 2.0,
            });
            vec![from.intersect(&mid), mid.clone(), to.intersect(&mid)]
        }
        _ => layout_edge.points.clone(),
    };
    let mut rendered_points = data_points.clone();

    if rendered_points.len() >= 2 {
        let start_inset = block_edge_start_marker_inset(edge.arrow_type_start.as_deref());
        if start_inset > 0.0 {
            rendered_points[0] =
                move_point_towards(&rendered_points[0], &rendered_points[1], start_inset);
        }
        let end_inset = block_edge_end_marker_inset(edge.arrow_type_end.as_deref());
        if end_inset > 0.0 {
            let last = rendered_points.len() - 1;
            rendered_points[last] = move_point_towards(
                &rendered_points[last],
                &rendered_points[last - 1],
                end_inset,
            );
        }
    }

    BlockEdgePoints {
        data_points,
        rendered_points,
    }
}

fn write_important_declaration(out: &mut impl SvgOutput, key: &str, value: &str) -> Result<()> {
    let _ = write!(out, "{key}:{}!important;", escape_xml_display(value));
    out.checkpoint()
}

fn write_important_declarations<'a>(
    out: &mut impl SvgOutput,
    declarations: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<()> {
    for (key, value) in declarations {
        write_important_declaration(out, key, value)?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
pub(crate) fn render_block_diagram_svg_model_with_theme(
    layout: &BlockDiagramLayout,
    model: &merman_core::diagrams::block::BlockDiagramRenderModel,
    node_paint_theme: &BlockNodePaintThemePlan,
    node_label_paint_theme: &BlockNodeLabelPaintPlan,
    edge_paint_theme: &crate::block::BlockEdgePaintPlan,
    marker_paint_theme: &crate::block::BlockMarkerPaintPlan,
    label_background_theme: &BlockLabelBackgroundPlan,
    typography_theme: &BlockTypographyThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    fn decode_block_label_html(raw: &str) -> String {
        // Mermaid's block diagram labels are rendered via an HTML foreignObject label helper,
        // which decodes HTML entities (notably `&nbsp;`).
        raw.replace("&nbsp;", "\u{00A0}")
    }

    fn roughjs_block_paths(
        path_data: &str,
        stroke_width: f32,
        randomness: &roughr::core::RoughRandomness,
    ) -> Option<(String, String)> {
        let mut stroke_options = roughr::core::OptionsBuilder::default()
            .randomness(randomness.clone())
            .roughness(0.0)
            .bowing(1.0)
            .fill_style(roughr::core::FillStyle::Solid)
            .stroke_width(stroke_width)
            .stroke_line_dash(vec![0.0, 0.0])
            .stroke_line_dash_offset(0.0)
            .fill_line_dash(vec![0.0, 0.0])
            .fill_line_dash_offset(0.0)
            .disable_multi_stroke(false)
            .disable_multi_stroke_fill(false)
            .build()
            .ok()?;

        // Mermaid's RoughJS path emitter draws the outline first, then reuses the advanced
        // randomizer for the solid fill pass.  Keep that order even at roughness zero because the
        // seeded path data is part of the stable SVG contract.
        let stroke_opset =
            roughr::renderer::svg_path::<f64>(path_data.to_string(), &mut stroke_options);
        let distance = 0.5;
        let sets = roughr::points_on_path::points_on_path::<f64>(
            path_data.to_string(),
            Some(1.0),
            Some(distance),
        );
        let mut fill_options = stroke_options.clone();
        let fill_opset = if sets.len() == 1 {
            fill_options.disable_multi_stroke = Some(true);
            fill_options.roughness = Some(0.0);
            let mut opset =
                roughr::renderer::svg_path::<f64>(path_data.to_string(), &mut fill_options);
            opset.ops = opset
                .ops
                .iter()
                .cloned()
                .enumerate()
                .filter_map(|(index, op)| {
                    if index != 0 && op.op == roughr::core::OpType::Move {
                        None
                    } else {
                        Some(op)
                    }
                })
                .collect();
            opset
        } else {
            roughr::renderer::solid_fill_polygon(&sets, &mut fill_options)
        };

        Some((
            ops_to_svg_path_d(&fill_opset),
            ops_to_svg_path_d(&stroke_opset),
        ))
    }

    fn block_stadium_points(width: f64, height: f64) -> Vec<LayoutPoint> {
        fn circle_points(
            center_x: f64,
            center_y: f64,
            radius: f64,
            count: usize,
            start_deg: f64,
            end_deg: f64,
        ) -> Vec<LayoutPoint> {
            let start = start_deg.to_radians();
            let step = (end_deg.to_radians() - start) / (count.saturating_sub(1).max(1) as f64);
            (0..count)
                .map(|index| {
                    let angle = start + index as f64 * step;
                    // Mermaid's generateCirclePoints() negates generated coordinates.
                    LayoutPoint {
                        x: -(center_x + radius * angle.cos()),
                        y: -(center_y + radius * angle.sin()),
                    }
                })
                .collect()
        }

        let radius = height / 2.0;
        let mut points = vec![
            LayoutPoint {
                x: -width / 2.0 + radius,
                y: -height / 2.0,
            },
            LayoutPoint {
                x: width / 2.0 - radius,
                y: -height / 2.0,
            },
        ];
        points.extend(circle_points(
            -width / 2.0 + radius,
            0.0,
            radius,
            50,
            90.0,
            270.0,
        ));
        points.push(LayoutPoint {
            x: width / 2.0 - radius,
            y: height / 2.0,
        });
        points.extend(circle_points(
            width / 2.0 - radius,
            0.0,
            radius,
            50,
            270.0,
            450.0,
        ));
        points
    }

    struct RoughPathRenderOptions<'a> {
        style: &'a str,
        fill: &'a str,
        stroke: &'a str,
        stroke_width: f32,
        randomness: &'a roughr::core::RoughRandomness,
        transform: Option<(f64, f64)>,
    }

    fn emit_rough_paths(
        out: &mut impl SvgOutput,
        path_data: &str,
        options: RoughPathRenderOptions<'_>,
    ) -> bool {
        if let Some((fill_d, stroke_d)) =
            roughjs_block_paths(path_data, options.stroke_width, options.randomness)
        {
            if let Some((tx, ty)) = options.transform {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container outer-path" transform="translate({},{})">"#,
                    fmt_display(tx),
                    fmt_display(ty)
                );
            } else {
                let _ = write!(out, r#"<g class="basic label-container outer-path">"#);
            }
            let _ = write!(
                out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                escape_attr(&fill_d),
                escape_attr(options.fill),
                escape_attr(options.style)
            );
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/>"#,
                escape_attr(&stroke_d),
                escape_attr(options.stroke),
                fmt_display(options.stroke_width as f64),
                escape_attr(options.style)
            );
            out.push_str("</g>");
            true
        } else {
            false
        }
    }

    let nodes_by_id = crate::block::resolve_block_node_sources(model);
    let shape_geometries_by_id: std::collections::HashMap<_, _> = layout
        .shape_geometries
        .iter()
        .map(|geometry| (geometry.id.as_str(), geometry))
        .collect();
    let layout_edges_by_id = BlockLayoutEdgeIndex::new(&layout.edges);
    let mut node_paint_receipt = node_paint_theme.begin_terminal_receipt();
    let mut typography_receipt = typography_theme.begin_terminal_receipt();

    fn marker_id(diagram_id: SvgDiagramId<'_>, marker: &str) -> String {
        format!("{diagram_id}_block-{marker}")
    }

    fn marker_url(diagram_id: SvgDiagramId<'_>, marker: &str) -> String {
        format!("url(#{})", marker_id(diagram_id, marker))
    }

    fn dom_id(diagram_id: SvgDiagramId<'_>, raw_id: &str) -> String {
        if diagram_id.semantic_str().is_empty() {
            raw_id.to_string()
        } else {
            format!("{diagram_id}-{raw_id}")
        }
    }

    fn write_block_class_css(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        theme: &crate::block::BlockCssThemeBinding,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        fn declarations(values: &[(String, String)]) -> impl Iterator<Item = (&str, &str)> {
            values
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
        }
        for class_def in &theme.class_definitions {
            options.checkpoint_emit()?;
            let class = escape_xml(&class_def.id);
            let mut shape_declarations = declarations(&class_def.shape);
            if let Some((key, value)) = shape_declarations.next() {
                let _ = write!(out, r#"#{diagram_id} .{}&gt;*{{"#, class.as_str());
                out.checkpoint()?;

                write_important_declaration(out, key, value)?;
                for (key, value) in shape_declarations {
                    write_important_declaration(out, key, value)?;
                }

                let _ = write!(out, r#"}}#{diagram_id} .{} span{{"#, class.as_str());
                out.checkpoint()?;
                write_important_declarations(out, declarations(&class_def.shape))?;
                out.push('}');
                out.checkpoint()?;
            }

            let mut text_declarations = declarations(&class_def.text);
            if let Some((key, value)) = text_declarations.next() {
                let _ = write!(out, r#"#{diagram_id} .{} tspan{{"#, class.as_str());
                out.checkpoint()?;
                write_important_declaration(out, key, value)?;
                write_important_declarations(out, text_declarations)?;
                out.push('}');
                out.checkpoint()?;
            }
            options.checkpoint_emit()?;
        }
        Ok(())
    }

    struct BlockCssEmission<'a> {
        font_family_css: &'a str,
        font_size_css: Box<str>,
    }

    fn write_block_css<'a>(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        typography_theme: &'a BlockTypographyThemePlan,
        label_background_theme: &BlockLabelBackgroundPlan,
        options: &SvgExecution<'_>,
    ) -> Result<BlockCssEmission<'a>> {
        let theme = typography_theme.css_binding();
        let font_family = typography_theme.font_family_css();
        let font_size = typography_theme.font_size_px();
        let font_size_css: Box<str> = fmt(font_size).to_string().into();
        let text_color = theme.text_color.as_str();
        let node_text_color = theme.node_text_color.as_str();
        let title_color = theme.title_color.as_str();
        let main_bkg = theme.main_bkg.as_str();
        let node_border = theme.node_border.as_str();
        let line_color = theme.line_color.as_str();
        let arrowhead_color = theme.arrowhead_color.as_str();
        let stroke_width = theme.stroke_width.as_str();
        let edge_label_background = label_background_theme.color();
        let cluster_bkg = theme.cluster_bkg.as_str();
        let cluster_border = theme.cluster_border.as_str();

        let _ = write!(
            out,
            r#"#{}{{font-family:{};font-size:{}px;fill:{};}}"#,
            diagram_id, font_family, font_size_css, node_text_color
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .edge-thickness-normal{{stroke-width:{}px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
            diagram_id, stroke_width, diagram_id, diagram_id, diagram_id, diagram_id, diagram_id
        );
        out.checkpoint()?;
        super::palette::write_palette_css(out, diagram_id, theme, options)?;
        let _ = write!(
            out,
            r#"#{} .label{{font-family:{};color:{};}}#{} p{{margin:0;}}#{} .label text,#{} span,#{} p{{fill:{};color:{};}}"#,
            diagram_id,
            font_family,
            node_text_color,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            node_text_color,
            node_text_color
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .cluster-label text{{fill:{};}}#{} .cluster-label span,#{} .cluster-label p{{color:{};}}"#,
            diagram_id, title_color, diagram_id, diagram_id, title_color
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:1px;}}#{} .flowchart-label text{{text-anchor:middle;}}#{} .node .label{{text-align:center;}}#{} .node.clickable{{cursor:pointer;}}"#,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            main_bkg,
            node_border,
            diagram_id,
            diagram_id,
            diagram_id
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .arrowheadPath,#{} .arrowMarkerPath{{fill:{};stroke:{};}}#{} .edgePaths .path{{stroke:{};stroke-width:2.0px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
            diagram_id,
            diagram_id,
            arrowhead_color,
            line_color,
            diagram_id,
            line_color,
            diagram_id,
            line_color
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .edgeLabel{{background-color:{};text-align:center;}}#{} .edgeLabel p{{margin:0;padding:0;display:inline;}}#{} .edgeLabel rect{{opacity:0.5;background-color:{};fill:{};}}#{} .labelBkg{{background-color:{}}}"#,
            diagram_id,
            edge_label_background,
            diagram_id,
            diagram_id,
            edge_label_background,
            edge_label_background,
            diagram_id,
            edge_label_background
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .node .cluster{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span,#{} .cluster p{{color:{};}}#{} .flowchartTitleText{{text-anchor:middle;font-size:18px;fill:{};}}"#,
            diagram_id,
            cluster_bkg,
            cluster_border,
            diagram_id,
            title_color,
            diagram_id,
            diagram_id,
            title_color,
            diagram_id,
            text_color,
        );
        let _ = crate::svg::parity::css::write_mermaid_base_css_root_rule_to(
            out,
            diagram_id,
            &theme.root_font_family,
        );
        out.checkpoint()?;
        write_block_class_css(out, diagram_id, theme, options)?;
        Ok(BlockCssEmission {
            font_family_css: font_family,
            font_size_css,
        })
    }

    let diagram_id = options.diagram_id_or("merman");
    let palette_size = typography_theme.css_binding().palette.len();
    let look = crate::config::DiagramLook::from_raw(Some(&typography_theme.css_binding().look));
    let hand_drawn_seed = options.rough_randomness(
        typography_theme
            .css_binding()
            .hand_drawn_seed
            .unwrap_or(options.seed() as f64),
        "render.block.roughjs",
    );
    let node_theme = typography_theme.css_binding();
    let html_labels = node_theme.html_labels;
    let node_fill_color = node_theme.main_bkg.as_str();
    let node_stroke_color = node_theme.node_border.as_str();
    // RoughJS uses 1.3px as its default node stroke width in Mermaid's handDrawnShapeStyles.
    // Keep this independent from the CSS `stroke-width: 1px` rule used by ordinary shapes.
    let node_stroke_width = 1.3_f32;

    let layout_bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    // Mermaid derives Block's root viewport from the rendered node group (`getBBox()`), not the
    // grid slots used during layout.  Use the shared shape geometry so circles, arrows, and
    // slanted shapes that extend beyond their slots remain inside the root viewBox.
    let rendered_bounds = layout
        .shape_geometries
        .iter()
        .map(BlockShapeGeometry::rendered_extents)
        .fold(None, |bounds: Option<(f64, f64, f64, f64)>, extents| {
            Some(match bounds {
                None => extents,
                Some((min_x, min_y, max_x, max_y)) => (
                    min_x.min(extents.0),
                    min_y.min(extents.1),
                    max_x.max(extents.2),
                    max_y.max(extents.3),
                ),
            })
        })
        .unwrap_or((
            layout_bounds.min_x,
            layout_bounds.min_y,
            layout_bounds.max_x,
            layout_bounds.max_y,
        ));

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "block");
    root_chrome.dom.trailing_newline = false;
    let root_context =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::BLOCK, diagram_id)
            .with_resource_policy(options.resource_policy());
    let root_document = root_context.begin_document(
        &mut out,
        root_svg::DeferredRootSpec::responsive(),
        root_chrome,
    )?;
    options.checkpoint_emit()?;
    out.push_str("<style>");
    out.checkpoint()?;
    let css_emission = write_block_css(
        &mut out,
        diagram_id,
        typography_theme,
        label_background_theme,
        options,
    )?;
    label_background_theme.record_stylesheet();
    let mut node_label_receipt = node_label_paint_theme.begin_terminal_receipt();
    let mut edge_paint_receipt = edge_paint_theme.begin_terminal_receipt();
    let typography_css_matches = typography_theme.observe_css(
        typography_receipt.as_mut(),
        css_emission.font_family_css,
        &css_emission.font_size_css,
    );
    out.push_str("</style><g/>");
    out.checkpoint()?;

    node_theme
        .look_defs
        .write_shadow_defs(&mut out, diagram_id)?;
    node_theme.look_defs.write_gradient(&mut out, diagram_id)?;
    let mut marker_paint_receipt = marker_paint_theme.begin_terminal_receipt();
    if let Some(receipt) = marker_paint_receipt.as_mut() {
        for (index, definition) in marker_paint_theme.definitions().iter().enumerate() {
            options.checkpoint_emit()?;
            let start = out.len();
            let mut marker = String::new();
            super::super::markers::push_base_edge_marker(
                &mut marker,
                diagram_id,
                "block",
                definition.kind(),
                definition.suffix(),
                definition.paint_style(),
            );
            out.push_str(&marker);
            out.checkpoint()?;
            marker_paint_theme.observe_definition(
                receipt,
                index,
                diagram_id.semantic_str(),
                &out.as_str()[start..],
            );
        }
    } else {
        let mut base_edge_markers = String::new();
        super::super::markers::push_base_edge_markers(&mut base_edge_markers, diagram_id, "block");
        out.push_str(&base_edge_markers);
        options.checkpoint_emit()?;
    }

    out.push_str(r#"<g class="block">"#);
    out.checkpoint()?;

    // The root viewport is finalized after the complete rendered group exists.  Keep the
    // scanner range restricted to visible Block content so `<defs>` markers and the stylesheet
    // do not become accidental contributors to the root bbox.
    let bounds_scan_start = out.len();
    let mut content_bounds = rendered_bounds;
    let mut marker_bounds: Option<(f64, f64, f64, f64)> = None;
    let mut has_rough_path = false;
    let mut has_rendered_edge = false;

    // Ordinary rendering owns terminal completeness. Keep duplicate output attempts so sink
    // failures retain precedence over the deferred terminal error.
    let mut checkpointed_nodes = std::collections::HashSet::new();
    let mut node_shells_match = true;
    for (node_index, n) in layout.nodes.iter().enumerate() {
        let Some(node) = nodes_by_id.get(n.id.as_str()) else {
            continue;
        };
        let binding = node_paint_theme
            .terminal_binding(node_index)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Block terminal binding for `{}`", n.id),
            })?;
        let class_str = if node.classes.is_empty() {
            "default flowchart-label".to_owned()
        } else {
            format!("{} flowchart-label", node.classes.join(" "))
        };
        let node_box_style = &binding.box_style;
        let node_text_style = &binding.text_style;
        let node_svg_text_style = &binding.svg_text_style;
        let node_div_style_prefix = &binding.div_style_prefix;
        let typed_label_color = binding.html_paragraph_color.as_deref();

        let geometry =
            shape_geometries_by_id
                .get(n.id.as_str())
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing Block shape geometry for node `{}`", n.id),
                })?;
        let node_shell_start = out.len();
        let id_attr = format!(r#" id="{}""#, escape_attr(&dom_id(diagram_id, &n.id)));
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<g class="node {}"{} transform="translate({}, {})" data-look="{}""#,
            escape_attr(&class_str),
            id_attr,
            fmt(geometry.allocated.x),
            fmt(geometry.allocated.y),
            escape_attr(look.as_str())
        );
        if palette_size > 0
            && let Some(index) = node.color_index
        {
            let _ = write!(out, r#" data-color-id="color-{}""#, index % palette_size);
        }
        out.push('>');

        let shell_kinds: &[BlockNodeShellKind];
        match &geometry.boundary {
            BlockShapeBoundary::Rectangle {
                width,
                height,
                radius,
                kind,
            } => {
                let class = match kind {
                    BlockRectangleKind::Basic => "basic label-container",
                    BlockRectangleKind::Composite => "basic cluster composite label-container",
                };
                let _ = write!(
                    &mut out,
                    r#"<rect class="{}" rx="{}" ry="{}" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                    class,
                    fmt(*radius),
                    fmt(*radius),
                    escape_attr(node_box_style),
                    fmt(-width / 2.0),
                    fmt(-height / 2.0),
                    fmt(*width),
                    fmt(*height)
                );
                shell_kinds = &[BlockNodeShellKind::Rect];
            }
            BlockShapeBoundary::Circle { radius, .. } => {
                let _ = write!(
                    &mut out,
                    r#"<circle class="basic label-container" style="{}" r="{}" cx="0" cy="0"/>"#,
                    escape_attr(node_box_style),
                    fmt(*radius),
                );
                shell_kinds = &[BlockNodeShellKind::Circle];
            }
            BlockShapeBoundary::DoubleCircle {
                outer_radius,
                inner_radius,
                ..
            } => {
                let _ = write!(
                    &mut out,
                    r#"<g class="basic label-container" style="{}"><circle class="outer-circle" style="{}" r="{}" cx="0" cy="0"/><circle class="inner-circle" style="{}" r="{}" cx="0" cy="0"/></g>"#,
                    escape_attr(node_box_style),
                    escape_attr(node_box_style),
                    fmt(*outer_radius),
                    escape_attr(node_box_style),
                    fmt(*inner_radius),
                );
                shell_kinds = &[BlockNodeShellKind::Circle, BlockNodeShellKind::Circle];
            }
            BlockShapeBoundary::Stadium { width, height } => {
                let points = block_stadium_points(*width, *height);
                let path_data = closed_path_d_from_points(
                    &points
                        .iter()
                        .map(|point| (point.x, point.y))
                        .collect::<Vec<_>>(),
                );
                let rough_emitted = emit_rough_paths(
                    &mut out,
                    &path_data,
                    RoughPathRenderOptions {
                        style: node_box_style,
                        fill: node_fill_color,
                        stroke: node_stroke_color,
                        stroke_width: node_stroke_width,
                        randomness: &hand_drawn_seed,
                        transform: None,
                    },
                );
                if rough_emitted {
                    has_rough_path = true;
                } else {
                    let radius = height / 2.0;
                    let _ = write!(
                        &mut out,
                        r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}"/>"#,
                        escape_attr(node_box_style),
                        fmt(-width / 2.0),
                        fmt(-height / 2.0),
                        fmt(*width),
                        fmt(*height),
                        fmt(radius),
                        fmt(radius)
                    );
                }
                shell_kinds = &[BlockNodeShellKind::Rect];
            }
            BlockShapeBoundary::Cylinder {
                width,
                body_height,
                radius_x,
                radius_y,
            } => {
                let _ = write!(
                    &mut out,
                    r#"<path d="M{},{} a{},{} 0,0,0 {} 0 a{},{} 0,0,0 {} 0 l0,{} a{},{} 0,0,0 {} 0 l0,{}" class="basic label-container outer-path" style="{}" transform="translate({}, {})"/>"#,
                    fmt_display(0.0),
                    fmt_display(*radius_y),
                    fmt_display(*radius_x),
                    fmt_display(*radius_y),
                    fmt_display(*width),
                    fmt_display(*radius_x),
                    fmt_display(*radius_y),
                    fmt_display(-width),
                    fmt_display(*body_height),
                    fmt_display(*radius_x),
                    fmt_display(*radius_y),
                    fmt_display(*width),
                    fmt_display(-body_height),
                    escape_attr(node_box_style),
                    fmt_display(-width / 2.0),
                    fmt_display(-(body_height / 2.0 + radius_y))
                );
                shell_kinds = &[BlockNodeShellKind::Path];
            }
            BlockShapeBoundary::Polygon {
                points,
                translation,
            } => {
                let odd_shape = matches!(node.block_type, "odd" | "rect_left_inv_arrow");
                let points_as_tuples: Vec<(f64, f64)> =
                    points.iter().map(|point| (point.x, point.y)).collect();
                let path_data = closed_path_d_from_points(&points_as_tuples);
                let rough_emitted = odd_shape
                    && emit_rough_paths(
                        &mut out,
                        &path_data,
                        RoughPathRenderOptions {
                            style: node_box_style,
                            fill: node_fill_color,
                            stroke: node_stroke_color,
                            stroke_width: node_stroke_width,
                            randomness: &hand_drawn_seed,
                            transform: Some((translation.x, translation.y)),
                        },
                    );
                if rough_emitted {
                    has_rough_path = true;
                    // The odd shape is always emitted through RoughJS in Mermaid 11.17.2,
                    // including the default look (roughness is simply zero there).
                } else {
                    out.push_str(r#"<polygon points=""#);
                    for (index, point) in points.iter().enumerate() {
                        if index > 0 {
                            out.push(' ');
                        }
                        let _ = write!(
                            &mut out,
                            "{},{}",
                            fmt_display(point.x),
                            fmt_display(point.y)
                        );
                    }
                    let _ = write!(
                        &mut out,
                        r#"" class="label-container" style="{}" transform="translate({},{})"/>"#,
                        escape_attr(node_box_style),
                        fmt_display(translation.x),
                        fmt_display(translation.y)
                    );
                }
                shell_kinds = &[BlockNodeShellKind::Polygon];
            }
        }
        out.checkpoint()?;
        node_shells_match &= shell_kinds == geometry.boundary.canonical_shell_kinds();
        checkpointed_nodes.insert(n.id.as_str());
        if let Some(receipt) = node_paint_receipt.as_mut() {
            receipt.observe_checkpointed_node(
                node_index,
                diagram_id.semantic_str(),
                &out.as_str()[node_shell_start..],
            );
        }

        let label = decode_block_label_html(node.label);
        let label_effectively_empty =
            node.label.is_empty() || block_label_is_effectively_empty(&label);
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_visible_label(&label);
        }
        let (label_tx, label_ty, label_w, label_h) = if label_effectively_empty {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let label_w = n.label_width.unwrap_or(0.0).max(0.0);
            let label_h = n.label_height.unwrap_or(0.0).max(0.0);
            let label_dx = if matches!(node.block_type, "odd" | "rect_left_inv_arrow") {
                match &geometry.boundary {
                    BlockShapeBoundary::Polygon { translation, .. } => translation.x,
                    _ => 0.0,
                }
            } else {
                0.0
            };
            (label_dx - label_w / 2.0, -label_h / 2.0, label_w, label_h)
        };
        if label_w > 0.0 && label_h > 0.0 {
            union_block_bounds(
                &mut content_bounds,
                (
                    geometry.allocated.x + label_tx,
                    geometry.allocated.y + label_ty,
                    geometry.allocated.x + label_tx + label_w,
                    geometry.allocated.y + label_ty + label_h,
                ),
            );
        }
        let label_start = out.len();
        if html_labels {
            let span_style_attr = if node_text_style.is_empty() {
                String::new()
            } else {
                format!(r#" style="{}""#, escape_attr(node_text_style))
            };
            let label_markup = if node.label.is_empty() {
                String::new()
            } else {
                match typed_label_color {
                    Some(color) => format!(
                        r#"<p style="color:{};">{}</p>"#,
                        escape_attr(color),
                        escape_xml(&label)
                    ),
                    None => format!("<p>{}</p>", escape_xml(&label)),
                }
            };
            let _ = write!(
                &mut out,
                r#"<g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"{}>{}</span></div></foreignObject></g>"#,
                escape_attr(node_text_style),
                fmt(label_tx),
                fmt(label_ty),
                fmt(label_w),
                fmt(label_h),
                escape_attr(node_div_style_prefix),
                span_style_attr,
                label_markup
            );
        } else {
            let label_text =
                crate::flowchart::flowchart_label_plain_text_for_layout(&label, "text", false);
            let _ = write!(
                &mut out,
                r#"<g class="label" style="{}" transform="translate({}, {})"><rect/>"#,
                escape_attr(node_text_style),
                fmt(label_tx + label_w / 2.0),
                fmt(label_ty),
            );
            crate::svg::parity::label::write_svg_text_centered_with_style(
                &mut out,
                &label_text,
                node_svg_text_style,
            );
            out.push_str("</g>");
        }

        out.checkpoint()?;
        if let Some(receipt) = node_label_receipt.as_mut() {
            node_label_paint_theme.observe_label(receipt, node_index, &out.as_str()[label_start..]);
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }

    for (edge_index, e) in model.edges.iter().enumerate() {
        let Some(le) = layout_edges_by_id.get(&e.id) else {
            continue;
        };
        let edge_start = out.len();
        let edge_points = block_render_edge_points(e, le, &shape_geometries_by_id);
        let data_points = base64::engine::general_purpose::STANDARD
            .encode(json_stringify_points(&edge_points.data_points));
        let (d, path_bounds) =
            super::super::curve::curve_basis_path_d_and_bounds(&edge_points.rendered_points);
        if let Some(path_bounds) = path_bounds {
            union_block_bounds(
                &mut content_bounds,
                (
                    path_bounds.min_x,
                    path_bounds.min_y,
                    path_bounds.max_x,
                    path_bounds.max_y,
                ),
            );
        }
        if !edge_points.rendered_points.is_empty() {
            has_rendered_edge = true;
        }
        let class_attr = "edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1";
        let prefixed_edge_id = dom_id(diagram_id, &e.id);
        let path_id = dom_id(diagram_id, &prefixed_edge_id);
        let edge_style =
            edge_paint_theme
                .declaration(edge_index)
                .ok_or_else(|| Error::InvalidModel {
                    message: "Block edge declaration plan does not match semantic edges".to_owned(),
                })?;
        let _ = write!(
            &mut out,
            r#"<path d="{}" id="{}" class="{}" style="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}""#,
            escape_attr(&d),
            escape_attr(&path_id),
            escape_attr(class_attr),
            escape_attr(edge_style),
            escape_attr(&prefixed_edge_id),
            escape_attr(&data_points)
        );

        if let Some((kind, suffix)) = marker_paint_theme.final_reference(edge_index, true) {
            if let Some(point) = edge_points.rendered_points.first() {
                include_block_marker_bounds(&mut marker_bounds, point, kind.suffix());
            }
            let _ = write!(
                &mut out,
                r#" marker-start="{}""#,
                escape_attr(&marker_url(diagram_id, suffix))
            );
        }
        if let Some((kind, suffix)) = marker_paint_theme.final_reference(edge_index, false) {
            if let Some(point) = edge_points.rendered_points.last() {
                include_block_marker_bounds(&mut marker_bounds, point, kind.suffix());
            }
            let _ = write!(
                &mut out,
                r#" marker-end="{}""#,
                escape_attr(&marker_url(diagram_id, suffix))
            );
        }
        options.checkpoint_emit()?;
        out.push_str("/>");
        out.checkpoint()?;
        if let Some(receipt) = marker_paint_receipt.as_mut() {
            marker_paint_theme.observe_path(
                receipt,
                edge_index,
                diagram_id.semantic_str(),
                &out.as_str()[edge_start..],
            );
        }
        if let Some(receipt) = edge_paint_receipt.as_mut() {
            edge_paint_theme.observe_path(
                receipt,
                edge_index,
                diagram_id.semantic_str(),
                &out.as_str()[edge_start..],
            );
        }
    }

    for e in &model.edges {
        let Some(le) = layout_edges_by_id.get(&e.id) else {
            continue;
        };
        let Some(lbl) = le.label.as_ref().filter(|_| !e.label.trim().is_empty()) else {
            continue;
        };
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_visible_label(&e.label);
        }
        union_block_bounds(
            &mut content_bounds,
            (
                lbl.x - lbl.width / 2.0,
                lbl.y - lbl.height / 2.0,
                lbl.x + lbl.width / 2.0,
                lbl.y + lbl.height / 2.0,
            ),
        );

        if html_labels {
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"><p>{}</p></span></div></foreignObject></g></g>"#,
                fmt(lbl.x),
                fmt(lbl.y),
                escape_attr(&e.id),
                fmt(-lbl.width / 2.0),
                fmt(-lbl.height / 2.0),
                fmt(lbl.width),
                fmt(lbl.height),
                escape_xml(&decode_block_label_html(&e.label))
            );
        } else {
            let label_text = crate::flowchart::flowchart_label_plain_text_for_layout(
                &decode_block_label_html(&e.label),
                "text",
                false,
            );
            options
                .work_meter()
                .charge_emit_work(1usize.saturating_add(label_text.len().div_ceil(64)))?;
            let text_y = options
                .text_measurer()
                .measure_svg_create_text_bbox_y_offset_px(
                    &label_text,
                    typography_theme.text_style(),
                );
            let padding = crate::block::SVG_EDGE_LABEL_BACKGROUND_PADDING;
            let text_width = (lbl.width - 2.0 * padding).max(0.0);
            let text_height = (lbl.height - 2.0 * padding).max(0.0);
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate(0, {})"><rect class="background" style="stroke: none" x="{}" y="{}" width="{}" height="{}"/>"#,
                fmt(lbl.x),
                fmt(lbl.y),
                escape_attr(&e.id),
                fmt(-text_y - text_height / 2.0),
                fmt(-text_width / 2.0 - padding),
                fmt(text_y - padding),
                fmt(lbl.width),
                fmt(lbl.height),
            );
            crate::svg::parity::label::write_svg_text_centered(&mut out, &label_text, true);
            out.push_str("</g></g>");
        }
        out.checkpoint()?;
        label_background_theme.record_terminal(
            lbl.width.is_finite() && lbl.height.is_finite() && lbl.width > 0.0 && lbl.height > 0.0,
            true,
            options.work_meter(),
        )?;
    }

    let bounds_scan_end = out.len();
    out.push_str("</g></svg>\n");
    options.checkpoint_emit()?;
    let mut final_bounds = content_bounds;
    if let Some(emitted_bounds) =
        svg_emitted_bounds_from_svg(&out.as_str()[bounds_scan_start..bounds_scan_end])
    {
        union_block_bounds(
            &mut final_bounds,
            (
                emitted_bounds.min_x,
                emitted_bounds.min_y,
                emitted_bounds.max_x,
                emitted_bounds.max_y,
            ),
        );
    }
    if let Some(marker_bounds) = marker_bounds {
        union_block_bounds(&mut final_bounds, marker_bounds);
    }

    // SVG's geometric bbox excludes stroke width.  Expand by the largest stroke actually
    // emitted by this renderer: ordinary node/edge CSS strokes and the explicit RoughJS stroke.
    // The values come from the emitted CSS/RoughJS options rather than a family-specific padding
    // constant, so custom theme stroke widths remain represented when they are numeric CSS px.
    let node_stroke_outset: f64 = if layout.shape_geometries.is_empty() {
        0.0
    } else {
        0.5
    };
    let rough_stroke_outset = if has_rough_path {
        f64::from(node_stroke_width) / 2.0
    } else {
        0.0
    };
    let edge_stroke_outset = if has_rendered_edge {
        node_theme.edge_stroke_width_px.map(f64::abs).unwrap_or(1.0) / 2.0
    } else {
        0.0
    };
    let stroke_outset = node_stroke_outset
        .max(rough_stroke_outset)
        .max(edge_stroke_outset);
    if stroke_outset.is_finite() && stroke_outset > 0.0 {
        final_bounds.0 -= stroke_outset;
        final_bounds.1 -= stroke_outset;
        final_bounds.2 += stroke_outset;
        final_bounds.3 += stroke_outset;
    }

    let root_bounds = root_svg::DiagramBounds::from_extents(
        final_bounds.0,
        final_bounds.1,
        final_bounds.2,
        final_bounds.3,
        BLOCK_ROOT_VIEWBOX_PADDING,
    );
    let root_document = root_context.finish_document(
        &mut out,
        root_document,
        root_svg::RootViewportSpec::responsive(root_bounds)
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width)),
    )?;
    let rooted = root_document.complete(out.finish()?)?;
    if !node_shells_match
        || checkpointed_nodes.len() != layout.nodes.len()
        || !node_paint_theme.record_terminal(node_paint_receipt)
    {
        return Err(Error::InvalidModel {
            message: "Block node shell paint terminal receipt was incomplete".to_string(),
        });
    }
    if !typography_css_matches
        || typography_receipt.is_some_and(|receipt| !typography_theme.record_terminal(receipt))
        || !node_label_paint_theme.record_terminal(node_label_receipt)
    {
        return Err(Error::InvalidModel {
            message: "Block typography terminal receipt was incomplete".to_string(),
        });
    }
    if !edge_paint_theme.record_terminal(edge_paint_receipt) {
        return Err(Error::InvalidModel {
            message: "Block edge paint terminal receipt was incomplete".to_string(),
        });
    }
    if !marker_paint_theme.record_terminal(marker_paint_receipt) {
        return Err(Error::InvalidModel {
            message: "Block marker paint terminal receipt was incomplete".to_string(),
        });
    }
    Ok(rooted)
}

#[cfg(test)]
mod tests;
