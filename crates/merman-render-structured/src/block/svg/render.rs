use std::{borrow::Cow, fmt::Write as _};

use base64::Engine as _;
use kurbo::Rect;
use merman_render_core::{
    Bounds, Error, LayoutEdge, LayoutPoint, Result,
    config::DiagramLook,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg,
    rough_svg::{
        closed_path_d_from_points, constrained_rough_edge_path, operation_randomness,
        paths::roughjs_paths_for_svg_path,
    },
    svg_bounds::{svg_emitted_bounds_from_svg, svg_path_bounds_from_d},
    svg_css::{SvgId as SvgDiagramId, available_local_svg_id, write_mermaid_base_css_root_rule},
    svg_edge_parity::{
        curve,
        markers::{self, BaseEdgeMarkerKind},
        path_numbers::json_stringify_points,
    },
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput, cutout},
    text::{literal_label::flowchart_non_markdown_svg_plain_text, svg_label as label},
    xml::{escape_attr, escape_attr_into},
};
use merman_theme_contract::v2::EdgeStyle;

use super::super::{
    BlockCssThemeBinding, BlockDiagramLayout, BlockEdgeMarkerGeometry, BlockNodeBindings,
    BlockOrthogonalRoute, BlockPreparedLabels, BlockRectangleKind, BlockShapeBoundary,
    BlockShapeGeometry, BlockTypographyThemePlan, SVG_EDGE_LABEL_BACKGROUND_PADDING,
    block_label_is_effectively_empty, block_render_edge_points, resolve_block_node_sources,
};
use super::BlockSvgContext;

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

fn block_marker_bounds(point: &LayoutPoint, kind: BaseEdgeMarkerKind) -> Option<Rect> {
    let radius = block_marker_extent_radius(kind.suffix())?;
    Some(Rect::new(
        point.x - radius,
        point.y - radius,
        point.x + radius,
        point.y + radius,
    ))
}

fn write_block_marker_references(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    kinds: [Option<BaseEdgeMarkerKind>; 2],
) {
    for (position, kind) in ["start", "end"].into_iter().zip(kinds) {
        if let Some(kind) = kind {
            let url = format!("url(#{diagram_id}_block-{})", kind.suffix());
            let _ = write!(out, r#" marker-{position}="{}""#, escape_attr(&url));
        }
    }
}

fn union_block_bounds(bounds: &mut (f64, f64, f64, f64), candidate: (f64, f64, f64, f64)) {
    bounds.0 = bounds.0.min(candidate.0);
    bounds.1 = bounds.1.min(candidate.1);
    bounds.2 = bounds.2.max(candidate.2);
    bounds.3 = bounds.3.max(candidate.3);
}

fn write_important_declaration(out: &mut impl SvgOutput, key: &str, value: &str) -> Result<()> {
    let _ = write!(out, "{key}:{value}!important;");
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

pub fn render_block_diagram_svg_model_with_theme(
    layout: &BlockDiagramLayout,
    model: &merman_model::families::block::BlockDiagramRenderModel,
    node_bindings: &BlockNodeBindings,
    edge_routes: &[Option<BlockOrthogonalRoute>],
    typography_theme: &BlockTypographyThemePlan,
    prepared_labels: &BlockPreparedLabels,
    options: &BlockSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    fn decode_block_label_html(raw: &str) -> String {
        // Mermaid's block diagram labels are rendered via an HTML foreignObject label helper,
        // which decodes HTML entities (notably `&nbsp;`).
        raw.replace("&nbsp;", "\u{00A0}")
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
        work_meter: &OperationWorkMeter,
    ) -> bool {
        if let Some((fill_d, stroke_d)) = roughjs_paths_for_svg_path(
            path_data,
            options.stroke_width,
            "0 0",
            work_meter,
            options.randomness,
        ) {
            if let Some((tx, ty)) = options.transform {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container outer-path" transform="translate({},{})">"#,
                    fmt(tx),
                    fmt(ty)
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
                fmt(options.stroke_width as f64),
                escape_attr(options.style)
            );
            out.push_str("</g>");
            true
        } else {
            false
        }
    }

    let nodes_by_id = resolve_block_node_sources(model);
    let shape_geometries_by_id: std::collections::HashMap<_, _> = layout
        .shape_geometries
        .iter()
        .map(|geometry| (geometry.id.as_str(), geometry))
        .collect();
    let layout_edges_by_id = BlockLayoutEdgeIndex::new(&layout.edges);
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
        theme: &BlockCssThemeBinding,
        options: &BlockSvgContext<'_>,
    ) -> Result<()> {
        fn declarations(values: &[(String, String)]) -> impl Iterator<Item = (&str, &str)> {
            values
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
        }
        for class_def in &theme.class_definitions {
            options.checkpoint_emit()?;
            let class = merman_render_core::svg_css::escape_css_identifier(&class_def.id);
            let mut shape_declarations = declarations(&class_def.shape);
            if let Some((key, value)) = shape_declarations.next() {
                let _ = write!(out, r#"#{diagram_id} .{class}>*{{"#);
                out.checkpoint()?;

                write_important_declaration(out, key, value)?;
                for (key, value) in shape_declarations {
                    write_important_declaration(out, key, value)?;
                }

                let _ = write!(out, r#"}}#{diagram_id} .{class} span{{"#);
                out.checkpoint()?;
                write_important_declarations(out, declarations(&class_def.shape))?;
                out.push('}');
                out.checkpoint()?;
            }

            let mut text_declarations = declarations(&class_def.text);
            if let Some((key, value)) = text_declarations.next() {
                let _ = write!(out, r#"#{diagram_id} .{class} tspan{{"#);
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

    fn write_block_css(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        typography_theme: &BlockTypographyThemePlan,
        cutout_labels: bool,
        options: &BlockSvgContext<'_>,
    ) -> Result<()> {
        let theme = typography_theme.css_binding();
        let font_family = typography_theme.font_family_css();
        let font_size = typography_theme.font_size_px();
        let font_size_css = fmt(font_size);
        let text_color = theme.text_color.as_str();
        let node_text_color = theme.node_text_color.as_str();
        let title_color = theme.title_color.as_str();
        let main_bkg = theme.main_bkg.as_str();
        let node_border = theme.node_border.as_str();
        let line_color = theme.line_color.as_str();
        let arrowhead_color = theme.arrowhead_color.as_str();
        let stroke_width = theme.stroke_width.as_str();
        let edge_label_background = if cutout_labels {
            "transparent"
        } else {
            theme.edge_label_background.as_str()
        };
        let cluster_bkg = theme.cluster_bkg.as_str();
        let cluster_border = theme.cluster_border.as_str();
        let native = typography_theme.native();
        let connector_width = native.map(|style| style.form.connector_width.to_string());
        let connector_width = connector_width.as_deref().unwrap_or("2.0");
        let (cluster_stroke_width, label_opacity) = if native.is_some() {
            (stroke_width, "1")
        } else {
            ("1", "0.5")
        };

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
        if native.is_none() {
            super::palette::write_palette_css(out, diagram_id, theme, options)?;
        }
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
            r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:{}px;}}#{} .flowchart-label text{{text-anchor:middle;}}#{} .node .label{{text-align:center;}}#{} .node.clickable{{cursor:pointer;}}"#,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            diagram_id,
            main_bkg,
            node_border,
            stroke_width,
            diagram_id,
            diagram_id,
            diagram_id
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"#{} .arrowheadPath,#{} .arrowMarkerPath{{fill:{};stroke:{};}}#{} .edgePaths .path{{stroke:{};stroke-width:{connector_width}px;}}#{} .flowchart-link{{stroke:{};fill:none;}}"#,
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
        if let Some(style) = native {
            let connector = fmt(style.form.connector_width);
            let thick = fmt(style.form.connector_width * 2.0);
            let weight = typography_theme
                .text_style()
                .font_weight
                .as_deref()
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"#{diagram_id}{{font-weight:{weight};}}#{diagram_id} .flowchart-link{{stroke-width:{connector}px;}}#{diagram_id} .flowchart-link.edge-thickness-thick{{stroke-width:{thick}px;}}"#,
            );
            out.checkpoint()?;
        }
        let _ = write!(
            out,
            r#"#{} .edgeLabel{{background-color:{};text-align:center;}}#{} .edgeLabel p{{margin:0;padding:0;display:inline;}}#{} .edgeLabel rect{{opacity:{label_opacity};background-color:{};fill:{};}}#{} .labelBkg{{background-color:{}}}"#,
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
            r#"#{} .node .cluster{{fill:{};stroke:{};stroke-width:{cluster_stroke_width}px;}}#{} .cluster text{{fill:{};}}#{} .cluster span,#{} .cluster p{{color:{};}}#{} .flowchartTitleText{{text-anchor:middle;font-size:18px;fill:{};}}"#,
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
        let _ = write_mermaid_base_css_root_rule(out, diagram_id, &theme.root_font_family);
        out.checkpoint()?;
        write_block_class_css(out, diagram_id, theme, options)?;
        Ok(())
    }

    let diagram_id = options.diagram_id;
    let native = typography_theme.native();
    let palette_size = typography_theme.css_binding().palette.len();
    let look = DiagramLook::from_raw(Some(&typography_theme.css_binding().look));
    let shoal_hand_drawn = options.shoal && look.is_hand_drawn();
    let hand_drawn_seed = operation_randomness(
        options.operation,
        typography_theme
            .css_binding()
            .hand_drawn_seed
            .unwrap_or(0.0),
        "render.block.roughjs",
    );
    let node_theme = typography_theme.css_binding();
    let cutout_labels =
        options.shoal && typography_theme.transparent_edge_labels(options.canvas_background);
    let html_labels = node_theme.html_labels;
    let node_fill_color = node_theme.main_bkg.as_str();
    let node_stroke_color = node_theme.node_border.as_str();
    // RoughJS uses 1.3px as its default node stroke width in Mermaid's handDrawnShapeStyles.
    // Keep this independent from the configured CSS stroke width used by ordinary shapes.
    let node_stroke_width = native.map_or(1.3, |style| style.form.stroke_width as f32);

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

    let mut out = BoundedSvgOutput::new(options.work_meter);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "block");
    root_chrome.dom.trailing_newline = false;
    let root_context = &options.viewport;
    let root_document = root_context.begin_document(
        &mut out,
        root_svg::DeferredRootSpec::responsive(),
        root_chrome,
    )?;
    options.checkpoint_emit()?;
    out.push_str("<style>");
    out.checkpoint()?;
    write_block_css(
        &mut XmlTextOutput::new(&mut out),
        diagram_id,
        typography_theme,
        cutout_labels,
        options,
    )?;
    out.push_str("</style><g/>");
    out.checkpoint()?;

    if let Some(look_defs) = &node_theme.look_defs {
        look_defs.write_shadow_defs(&mut out, diagram_id)?;
        if options.shoal && look_defs.uses_gradient() {
            out.push_str("<defs>");
            look_defs.write_gradient(&mut out, diagram_id)?;
            out.push_str("</defs>");
        } else {
            look_defs.write_gradient(&mut out, diagram_id)?;
        }
    }
    if let Some(style) = native {
        super::native::write_markers(&mut out, diagram_id, style.form.marker_style)?;
    } else {
        let mut base_edge_markers = String::new();
        markers::push_base_edge_markers(&mut base_edge_markers, diagram_id, "block");
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
    let mut has_thick_edge = false;

    if let Some(style) = native
        && !matches!(style.elevation, merman_theme_contract::v2::Elevation::Flat)
    {
        for (index, geometry) in layout.shape_geometries.iter().enumerate() {
            options.checkpoint_emit()?;
            let id = format!("{diagram_id}-block-depth-{index}");
            if let Some(bounds) =
                super::native::write_node_depth(&mut out, style, &id, geometry, options)?
            {
                union_block_bounds(
                    &mut content_bounds,
                    (bounds.x0, bounds.y0, bounds.x1, bounds.y1),
                );
            }
        }
    }

    for (node_index, n) in layout.nodes.iter().enumerate() {
        let Some(node) = nodes_by_id.get(n.id.as_str()) else {
            continue;
        };
        let binding = node_bindings
            .get(node_index)
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

        let geometry =
            shape_geometries_by_id
                .get(n.id.as_str())
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing Block shape geometry for node `{}`", n.id),
                })?;
        let disambiguated = if !options.shoal
            && node_theme
                .look_defs
                .as_ref()
                .is_some_and(|defs| defs.defines_local_id(&n.id, true))
        {
            Some(available_local_svg_id(
                format!("block-node-{node_index}"),
                nodes_by_id.len().saturating_add(layout.edges.len()),
                |id| {
                    nodes_by_id.contains_key(id)
                        || id
                            .strip_prefix(diagram_id.semantic_str())
                            .and_then(|suffix| suffix.strip_prefix('-'))
                            .is_some_and(|edge_id| layout_edges_by_id.get(edge_id).is_some())
                },
                options.work_meter,
            )?)
        } else {
            None
        };
        let mut id_attr = if options.shoal {
            format!(r#" id="{diagram_id}-block-node-{node_index}""#)
        } else {
            let id = disambiguated.as_deref().unwrap_or(&n.id);
            format!(r#" id="{}""#, escape_attr(&dom_id(diagram_id, id)))
        };
        if options.shoal || disambiguated.is_some() {
            let _ = write!(
                id_attr,
                r#" data-et="node" data-id="{}""#,
                escape_attr(&n.id)
            );
        }
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

        let sketch = if shoal_hand_drawn {
            super::native::hand_drawn_shape_path(geometry, &hand_drawn_seed, options)?
        } else {
            None
        };
        match (sketch, &geometry.boundary) {
            (Some(d), boundary) => {
                let class = match boundary {
                    BlockShapeBoundary::Rectangle {
                        kind: BlockRectangleKind::Composite,
                        ..
                    } => "basic cluster composite label-container",
                    _ => "basic label-container outer-path",
                };
                let _ = write!(
                    out,
                    r#"<path d="{}" class="{class}" style="{}"/>"#,
                    escape_attr(&d),
                    escape_attr(node_box_style),
                );
            }
            (
                None,
                BlockShapeBoundary::Rectangle {
                    width,
                    height,
                    radius,
                    kind,
                },
            ) => {
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
            }
            (None, BlockShapeBoundary::Circle { radius, .. }) => {
                let _ = write!(
                    &mut out,
                    r#"<circle class="basic label-container" style="{}" r="{}" cx="0" cy="0"/>"#,
                    escape_attr(node_box_style),
                    fmt(*radius),
                );
            }
            (
                None,
                BlockShapeBoundary::DoubleCircle {
                    outer_radius,
                    inner_radius,
                    ..
                },
            ) => {
                let circle_style = if options.shoal {
                    let mut style = String::new();
                    merman_render_core::mermaid_style::visit_parsed_style_declarations(
                        node_box_style,
                        |decl| {
                            if decl.property() != "opacity" {
                                let _ = write!(
                                    style,
                                    "{}:{};",
                                    decl.property_source(),
                                    decl.source_value()
                                );
                            }
                        },
                    );
                    Cow::Owned(style)
                } else {
                    Cow::Borrowed(node_box_style.as_str())
                };
                let circle_style = escape_attr(&circle_style);
                let _ = write!(
                    &mut out,
                    r#"<g class="basic label-container" style="{}"><circle class="outer-circle" style="{}" r="{}" cx="0" cy="0"/><circle class="inner-circle" style="{}" r="{}" cx="0" cy="0"/></g>"#,
                    escape_attr(node_box_style),
                    circle_style,
                    fmt(*outer_radius),
                    circle_style,
                    fmt(*inner_radius),
                );
            }
            (None, BlockShapeBoundary::Stadium { width, height }) => {
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
                    options.work_meter,
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
            }
            (
                None,
                BlockShapeBoundary::Cylinder {
                    width,
                    body_height,
                    radius_x,
                    radius_y,
                },
            ) => {
                let _ = write!(
                    &mut out,
                    r#"<path d="{}" class="basic label-container outer-path" style="{}" transform="translate({}, {})"/>"#,
                    super::native::cylinder_path(*width, *body_height, *radius_x, *radius_y),
                    escape_attr(node_box_style),
                    fmt(-width / 2.0),
                    fmt(-(body_height / 2.0 + radius_y))
                );
            }
            (
                None,
                BlockShapeBoundary::Polygon {
                    points,
                    translation,
                },
            ) => {
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
                        options.work_meter,
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
                        let _ = write!(&mut out, "{},{}", fmt(point.x), fmt(point.y));
                    }
                    let _ = write!(
                        &mut out,
                        r#"" class="label-container" style="{}" transform="translate({},{})"/>"#,
                        escape_attr(node_box_style),
                        fmt(translation.x),
                        fmt(translation.y)
                    );
                }
            }
        }
        out.checkpoint()?;
        let label = decode_block_label_html(node.label);
        let prepared_label = prepared_labels.node_label(&n.id);
        let label_effectively_empty =
            node.label.is_empty() || block_label_is_effectively_empty(&label);
        let (label_tx, label_ty, label_w, label_h) = if label_effectively_empty {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let label_w = n.label_width.unwrap_or(0.0).max(0.0);
            let label_h = n.label_height.unwrap_or(0.0).max(0.0);
            let label_dx = geometry.label_offset_x(node.block_type);
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
        if html_labels {
            let span_style_attr = if node_text_style.is_empty() {
                String::new()
            } else {
                format!(r#" style="{}""#, escape_attr(node_text_style))
            };
            let label_markup = prepared_label.xhtml().ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Block HTML label for node `{}`", n.id),
            })?;
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
            let label_text = flowchart_non_markdown_svg_plain_text(&label);
            let _ = write!(
                &mut out,
                r#"<g class="label" style="{}" transform="translate({}, {})"><rect/>"#,
                escape_attr(node_text_style),
                fmt(label_tx + label_w / 2.0),
                fmt(label_ty),
            );
            let _ = label::write_svg_text_centered_with_style(
                &mut out,
                &label_text,
                node_svg_text_style,
            );
            out.push_str("</g>");
        }

        out.checkpoint()?;
        out.push_str("</g>");
        out.checkpoint()?;
    }

    let edge_style = native.map(|style| style.form.edge_style);
    for (edge_index, e) in model.edges.iter().enumerate() {
        let Some(le) = layout_edges_by_id.get(&e.id) else {
            continue;
        };
        let fallback_points;
        let fallback_d;
        let route = edge_routes.get(edge_index).and_then(Option::as_ref);
        let (d, points, marker_refs, mut path_bounds) = 'geometry: {
            if let Some(route) = route {
                let b = &route.bounds;
                break 'geometry (
                    route.path.as_str(),
                    le.points.as_slice(),
                    [Some(&route.marker_refs[0]), Some(&route.marker_refs[1])],
                    Some(Rect::new(b.min_x, b.min_y, b.max_x, b.max_y)),
                );
            }
            fallback_points = block_render_edge_points(e, le, &shape_geometries_by_id, edge_style);
            let (d, bounds) = match edge_style {
                None | Some(EdgeStyle::Curved) => {
                    curve::curve_basis_path_d_and_bounds(&fallback_points.rendered_points)
                }
                Some(EdgeStyle::Straight | EdgeStyle::Orthogonal | EdgeStyle::Sketch) => {
                    curve::curve_linear_path_d_and_bounds(&fallback_points.rendered_points)
                }
            };
            fallback_d = d;
            (
                fallback_d.as_str(),
                fallback_points.data_points.as_slice(),
                [
                    fallback_points.rendered_points.first(),
                    fallback_points.rendered_points.last(),
                ],
                bounds.map(|b| Rect::new(b.min_x, b.min_y, b.max_x, b.max_y)),
            )
        };
        let marker_kinds = [
            BaseEdgeMarkerKind::from_arrow(e.arrow_type_start.as_deref(), true),
            BaseEdgeMarkerKind::from_arrow(e.arrow_type_end.as_deref(), false),
        ];
        let sketch = edge_style == Some(EdgeStyle::Sketch) || (shoal_hand_drawn && route.is_some());
        let rough_d = if sketch && points.len() >= 2 {
            let markers = route.map_or_else(
                || {
                    let [start, end] = marker_kinds.map(|kind| {
                        kind.and_then(BlockEdgeMarkerGeometry::from_kind)
                            .map(|geometry| geometry.routing)
                            .unwrap_or_default()
                    });
                    (start, end)
                },
                |route| route.markers,
            );
            let d = constrained_rough_edge_path(d, markers, options.work_meter, &hand_drawn_seed)?;
            options.work_meter.charge(d.len())?;
            if let Some(b) = svg_path_bounds_from_d(&d) {
                let rough_bounds = Rect::new(b.min_x, b.min_y, b.max_x, b.max_y);
                path_bounds = Some(path_bounds.map_or(rough_bounds, |b| b.union(rough_bounds)));
            }
            Some(d)
        } else {
            None
        };
        let d = rough_d.as_deref().unwrap_or(d);
        if let Some(b) = path_bounds {
            union_block_bounds(&mut content_bounds, (b.x0, b.y0, b.x1, b.y1));
        }
        let marker_rects =
            [0, 1].map(|end| block_marker_bounds(marker_refs[end]?, marker_kinds[end]?));
        for b in marker_rects.iter().flatten() {
            let extents = (b.x0, b.y0, b.x1, b.y1);
            union_block_bounds(marker_bounds.get_or_insert(extents), extents);
        }
        has_rendered_edge |= !points.is_empty();
        let data_points =
            base64::engine::general_purpose::STANDARD.encode(json_stringify_points(points));
        let thickness = e.thickness.as_str();
        has_thick_edge |= thickness == "thick";
        let pattern = e.pattern.as_str();
        let (path_id, data_id) = if options.shoal {
            (
                format!("{diagram_id}-block-edge-{edge_index}"),
                Cow::Borrowed(e.id.as_str()),
            )
        } else {
            let data_id = dom_id(diagram_id, &e.id);
            let path_id = dom_id(diagram_id, &data_id);
            (path_id, Cow::Owned(data_id))
        };
        let label_gap = le
            .label
            .as_ref()
            .filter(|label| {
                cutout_labels
                    && !e.label.trim().is_empty()
                    && label.width > 0.0
                    && label.height > 0.0
            })
            .map(|label| Rect::from_center_size((label.x, label.y), (label.width, label.height)))
            .filter(|gap| gap.is_finite());
        let mut shaft_markers = marker_kinds;
        let mut separate_markers = [None; 2];
        let cutout_id = if let Some((gap, mut paint_bounds)) = label_gap.zip(path_bounds) {
            for (end, rect) in marker_rects.into_iter().enumerate() {
                if let Some(rect) = rect {
                    if gap.intersect(rect).area() > 0.0 {
                        // SVG clips attached markers together with their shaft. Paint this marker
                        // on an unpainted copy so the hole can cover the full caption.
                        separate_markers[end] = shaft_markers[end].take();
                    } else {
                        paint_bounds = paint_bounds.union(rect);
                    }
                }
            }
            let stroke_width = match (native, thickness) {
                (Some(style), "thick") => style.form.connector_width * 2.0,
                (Some(style), _) => style.form.connector_width,
                (None, "thick") => 3.5,
                (None, _) => node_theme.edge_stroke_width_px.unwrap_or(1.0).abs(),
            };
            let id = format!("{path_id}-label-cutout");
            // Include the stroke at joins, up to the default SVG miter limit of four half-widths.
            cutout::write_defs(
                &mut out,
                &id,
                paint_bounds,
                stroke_width * 2.0 + 1.0,
                &[gap],
                options.work_meter,
            )?;
            Some(id)
        } else {
            None
        };
        let _ = write!(
            &mut out,
            r#"<path d="{}" id="{}" class="edge-thickness-{} edge-pattern-{}"#,
            escape_attr(d),
            escape_attr(&path_id),
            thickness,
            pattern,
        );
        if options.shoal {
            out.push_str(r#" flowchart-link""#);
        } else {
            let _ = write!(
                &mut out,
                r#" edge-thickness-{thickness} edge-pattern-{pattern} flowchart-link LS-a1 LE-b1" style="undefined;;;undefined""#
            );
        }
        out.push_str(r#" data-edge="true" data-et="edge" data-id=""#);
        escape_attr_into(&mut out, &data_id);
        let _ = write!(&mut out, r#"" data-points="{data_points}""#);

        write_block_marker_references(&mut out, diagram_id, shaft_markers);
        if let Some(id) = cutout_id {
            cutout::write_reference(&mut out, id);
        }
        options.checkpoint_emit()?;
        out.push_str("/>");
        if separate_markers.iter().any(Option::is_some) {
            let _ = write!(
                &mut out,
                r#"<path d="{}" fill="none" stroke="none""#,
                escape_attr(d),
            );
            write_block_marker_references(&mut out, diagram_id, separate_markers);
            out.push_str("/>");
        }
        out.checkpoint()?;
    }

    for (edge_index, e) in model.edges.iter().enumerate() {
        let Some(le) = layout_edges_by_id.get(&e.id) else {
            continue;
        };
        let Some(lbl) = le.label.as_ref().filter(|_| !e.label.trim().is_empty()) else {
            continue;
        };
        let prepared_label = prepared_labels.edge_label(edge_index);
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
                r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label" data-id="{}" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel">{}</span></div></foreignObject></g></g>"#,
                fmt(lbl.x),
                fmt(lbl.y),
                escape_attr(&e.id),
                fmt(-lbl.width / 2.0),
                fmt(-lbl.height / 2.0),
                fmt(lbl.width),
                fmt(lbl.height),
                prepared_label.xhtml().ok_or_else(|| Error::InvalidModel {
                    message: format!("missing prepared Block HTML label for edge `{}`", e.id),
                })?
            );
        } else {
            let label_text =
                flowchart_non_markdown_svg_plain_text(&decode_block_label_html(&e.label));
            options
                .work_meter
                .charge_emit_work(1usize.saturating_add(label_text.len().div_ceil(64)))?;
            let text_y = options
                .text_measurer
                .measure_svg_create_text_bbox_y_offset_px(
                    &label_text,
                    &prepared_label.typography.style,
                );
            let padding = SVG_EDGE_LABEL_BACKGROUND_PADDING;
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
            label::write_svg_text_centered(&mut out, &label_text, true);
            out.push_str("</g></g>");
        }
        out.checkpoint()?;
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
    } else if let Some(style) = native {
        style.form.stroke_width / 2.0
    } else {
        node_theme.edge_stroke_width_px.map(f64::abs).unwrap_or(1.0) / 2.0
    };
    let rough_stroke_outset = if has_rough_path {
        f64::from(node_stroke_width) / 2.0
    } else {
        0.0
    };
    let edge_stroke_outset = if !has_rendered_edge {
        0.0
    } else if let Some(style) = native {
        style.form.connector_width * if has_thick_edge { 1.0 } else { 0.5 }
    } else {
        node_theme.edge_stroke_width_px.map(f64::abs).unwrap_or(1.0) / 2.0
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
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests;
