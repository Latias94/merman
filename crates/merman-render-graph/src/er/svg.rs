//! Complete Mermaid-compatible ER SVG emission.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;

use base64::Engine as _;
use merman_model::{
    OperationPhase,
    families::er::{ErDiagramModel, ErEntity, ErRelationship, ErSubgraph},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutPoint, Result,
    html::html_label_overflow_attr,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg,
    svg_css::SvgId,
    svg_edge_parity::{curve, edge_label_geometry, edge_path, line_hops, svg_path_length_from_d},
    svg_output::{BoundedSvgOutput, SvgOutput},
    text::{
        self, TextMeasurer, inline_html::SvgTextEntityMode,
        literal_label::flowchart_non_markdown_svg_source_word_lines, svg_label as label,
    },
    xml::{escape_attr, escape_attr_display, escape_xml},
};
#[cfg(feature = "layout-shoal")]
use merman_render_core::{route_paint::CanonicalPaintScratch, svg_output::cutout};
use merman_theme_contract::v2::{Color, EdgeStyle};
use roughr::core::RoughRandomness;

use super::{
    ErBoxLabel, ErConfigView, ErCssBinding, ErEntityThemePlan, ErMarkerDefs, ErNativeStyle,
    ErPreparedLabels, ErPreparedRelationshipLabel, ErSvgGeometry, ErSvgLayoutView, ErSvgRouteKind,
    ErSwimlanePaintLayout, er_color_indices, er_redux_color_id, er_rel_idx_from_edge_id,
    er_unified_marker_id, geometry::er_relationship_dom_id,
    markers::write_er_marker_definitions_with_inset,
    stylesheet::write_er_stylesheet_with_cluster_titles,
};

#[cfg(feature = "layout-grid")]
mod grid;
mod hand_drawn;
#[cfg(feature = "layout-layered")]
mod layered;
mod swimlane;
mod zero_markers;

/// A borrow of the exact prepared artifact shared by the style plan and writer.
pub struct ErPreparedSvgLayout<'a> {
    pub geometry: ErSvgLayoutView<'a>,
    pub swimlane: Option<&'a ErSwimlanePaintLayout>,
    pub prepared_labels: &'a Arc<ErPreparedLabels>,
}

/// One canonical edge and the data points borrowed from the operation scratch buffer.
#[cfg(feature = "layout-shoal")]
pub struct ErCanonicalEdge<'points> {
    pub d: String,
    pub points: &'points [LayoutPoint],
    pub bounds: kurbo::Rect,
}

/// The caller's existing canonical adapter, invoked at the edge's emission checkpoint.
#[cfg(feature = "layout-shoal")]
pub type ErCanonicalEdgePainter<'a> = dyn for<'points> Fn(
        &LayoutEdge,
        kurbo::Vec2,
        &RoughRandomness,
        &'points mut CanonicalPaintScratch,
    ) -> Result<ErCanonicalEdge<'points>>
    + 'a;

/// Host-owned SVG settings for one ER emission. The viewport and ID share the same projection.
pub struct ErSvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: root_svg::RootViewportContext<'a>,
    pub canvas_background: Option<Color>,
    pub canvas_has_texture: bool,
    pub elk_parity: bool,
    pub swimlane: bool,
    #[cfg(feature = "layout-grid")]
    pub grid: Option<merman_render_core::graph_layout::mermaid_grid::GridPaint>,
    #[cfg(feature = "layout-layered")]
    pub layered: Option<&'a super::ErLayeredPaint>,
    #[cfg(feature = "layout-shoal")]
    pub canonical_routes: Option<&'a ErCanonicalEdgePainter<'a>>,
    pub work_meter: &'a OperationWorkMeter,
    pub rough_randomness: &'a RoughRandomness,
    pub include_edges: bool,
    pub viewbox_padding: f64,
}

fn er_text_style_attr(source_style: &str) -> String {
    format!(r#"style="{source_style}""#)
}

// ELK and Swimlane normalize ER's basis curve to rounded after routing.
fn er_edge_path_d(
    points: &[LayoutPoint],
    rounded_layout: bool,
    missing_section: bool,
    native: Option<&ErNativeStyle>,
) -> String {
    if missing_section {
        return curve::curve_linear_path_d(points);
    }
    match native.map(|style| style.edge_style) {
        Some(EdgeStyle::Orthogonal) => {
            return curve::curve_step_path_d_and_bounds(points).0;
        }
        Some(EdgeStyle::Straight | EdgeStyle::Sketch) => {
            return curve::curve_linear_path_d(points);
        }
        _ => {}
    }
    if rounded_layout && native.is_none() {
        return curve::curve_rounded_path_d(points, 5.0);
    }
    if let [a, b] = points {
        return curve::curve_basis_path_d(&[
            a.clone(),
            LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            },
            b.clone(),
        ]);
    }
    curve::curve_basis_path_d(points)
}

fn write_er_svg_box_label(out: &mut impl SvgOutput, label: &ErBoxLabel) {
    out.push_str(label.svg_markup());
}

impl ErBoxLabel {
    fn emit_html_div(
        &self,
        out: &mut impl SvgOutput,
        style_prefix: &str,
        span_style_attr: &str,
        markdown_node_label: bool,
        align: &str,
    ) {
        let layout = self
            .html_layout()
            .expect("ER HTML projection is prepared before emission");
        let (display, whitespace) = if layout.break_spaces {
            ("table", "break-spaces")
        } else {
            ("table-cell", "nowrap")
        };
        let _ = write!(
            out,
            r#"<div xmlns="http://www.w3.org/1999/xhtml" style="{}display: {}; white-space: {}; line-height: 1.5; max-width: {}px; text-align: {};"#,
            escape_attr(style_prefix),
            display,
            whitespace,
            layout.width_budget,
            align,
        );
        if layout.break_spaces {
            let _ = write!(out, " width: {}px;", layout.width_budget);
        }
        out.push_str(r#"">"#);
        if self.uses_generic_workaround() {
            out.push_str(self.xhtml_fragment());
        } else {
            let class = if markdown_node_label {
                "nodeLabel markdown-node-label"
            } else {
                "nodeLabel"
            };
            let _ = write!(out, r#"<span class="{}"{}>"#, class, span_style_attr);
            if !self.rendered_text().is_empty() {
                out.push_str(self.xhtml_fragment());
            }
            out.push_str("</span>");
        }
        out.push_str("</div>");
    }
}

fn write_er_svg_plain_label(out: &mut impl SvgOutput, text: &str) {
    let lines = flowchart_non_markdown_svg_source_word_lines(text);
    label::write_svg_text_source_word_lines(out, &lines, true, false);
}

fn write_er_svg_label_background(out: &mut impl SvgOutput) {
    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
}

#[derive(Clone, Copy)]
struct ErSubgraphRenderContext<'a> {
    data_look: &'a str,
    uses_canonical_routes: bool,
    layered_titles: bool,
    allow_html_label_overflow: bool,
    use_html_labels: bool,
    measurer: &'a dyn text::TextMeasurer,
    label_style: &'a text::TextStyle,
    translate_x: f64,
    translate_y: f64,
    theme: &'a ErEntityThemePlan,
    swimlane_html_labels: bool,
    synthetic_color_ordinal: usize,
    work_meter: &'a OperationWorkMeter,
    rough_randomness: &'a roughr::core::RoughRandomness,
}

fn render_er_subgraph_cluster(
    out: &mut impl SvgOutput,
    cluster: &LayoutCluster,
    dom_id: &str,
    subgraph: &ErSubgraph,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    let source_style = context
        .theme
        .subgraph_source_style(&subgraph.id)
        .ok_or_else(|| Error::InvalidModel {
            message: format!(
                "ER prepared source style is missing subgraph {}",
                subgraph.id
            ),
        })?;
    let visual = source_style.visual();
    let rect_style_attr = &visual.rect_style_attr;
    let text_style_attr = &visual.source_text_attr;
    let class_attr = if subgraph.classes.is_empty() {
        "cluster".to_string()
    } else {
        format!("cluster {}", subgraph.classes.join(" "))
    };
    let width = cluster.width.max(1.0);
    let height = cluster.height.max(1.0);
    let left = cluster.x - width / 2.0 + context.translate_x;
    let top = cluster.y - height / 2.0 + context.translate_y;
    let title_width = cluster.title_label.width.max(0.0);
    let title_height = cluster.title_label.height.max(0.0);
    let title_x = cluster.title_label.x + context.translate_x;
    let title_y = cluster.title_label.y + context.translate_y;
    let _ = write!(
        out,
        r#"<g id="{}" class="{}" data-look="{}""#,
        escape_attr(dom_id),
        escape_attr(&class_attr),
        escape_attr(context.data_look),
    );
    if context.uses_canonical_routes {
        let _ = write!(
            out,
            r#" data-et="cluster" data-id="{}""#,
            escape_attr(&cluster.id)
        );
    }
    let _ = write!(
        out,
        r#"><rect class="basic label-container" {} x="{}" y="{}" width="{}" height="{}"/>"#,
        rect_style_attr,
        fmt(left),
        fmt(top),
        fmt(width),
        fmt(height),
    );
    if context.use_html_labels {
        let prepared = context
            .theme
            .html_subgraph_label(&subgraph.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared HTML title is missing subgraph {}", subgraph.id),
            })?;
        if context.layered_titles && subgraph.label_type == "markdown" {
            context
                .work_meter
                .charge_emit_work(prepared.fragment().len())?;
            let metrics = text::measure_xhtml_label_fragment_with_terminal_fonts(
                context.measurer,
                prepared.fragment(),
                context.label_style,
                Some(width),
                super::labels::HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
            );
            context.work_meter.checkpoint(OperationPhase::Emit)?;
            let title_width = metrics.width.max(0.0);
            let title_height = metrics.height.max(0.0);
            let constrained = title_width >= width;
            let _ = write!(
                out,
                r#"<g class="cluster-label" transform="translate({}, {})"><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: {}; white-space: {}; line-height: 1.5; max-width: {}px; text-align: center;"#,
                fmt(title_x - title_width / 2.0),
                fmt(top + cluster.title_margin_top),
                fmt(title_width),
                fmt(title_height),
                if constrained { "table" } else { "table-cell" },
                if constrained {
                    "break-spaces"
                } else {
                    "nowrap"
                },
                fmt(width),
            );
            if constrained {
                let _ = write!(out, " width: {}px;", fmt(width));
            }
            let _ = write!(
                out,
                r#""><span class="nodeLabel" {}>{}</span></div></foreignObject></g>"#,
                text_style_attr,
                prepared.fragment()
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="cluster-label" transform="translate({}, {})"><foreignObject x="{}" y="{}" width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; text-align: center;"><span class="nodeLabel" {}>{}</span></div></foreignObject></g>"#,
                fmt(title_x),
                fmt(title_y),
                fmt(-title_width / 2.0),
                fmt(-title_height / 2.0),
                fmt(title_width),
                fmt(title_height),
                html_label_overflow_attr(
                    context.allow_html_label_overflow,
                    title_width,
                    title_height
                ),
                text_style_attr,
                prepared.fragment(),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<g class="cluster-label" transform="translate({}, {})" {}>"#,
            fmt(title_x),
            fmt(title_y - title_height / 2.0),
            visual.svg_source_text_attr,
        );
        if subgraph.label_type == "string" || subgraph.label_type == "text" {
            label::write_svg_text_centered_from_create_text_source_with_style(
                out,
                &subgraph.title,
                "",
            );
        } else {
            if context.layered_titles {
                context.work_meter.charge_emit_work(subgraph.title.len())?;
            }
            label::write_svg_text_markdown_wrapped_centered_with_style(
                out,
                &subgraph.title,
                "",
                context.measurer,
                context.label_style,
                context.layered_titles.then_some(width),
            );
            if context.layered_titles {
                context.work_meter.checkpoint(OperationPhase::Emit)?;
            }
        }
        out.push_str("</g>");
    }
    out.push_str("</g>");
    out.push('\n');
    Ok(())
}

fn render_er_subgraph_clusters(
    out: &mut impl SvgOutput,
    clusters: &[LayoutCluster],
    dom_ids: &[String],
    model: &ErDiagramModel,
    swimlane: Option<&ErSwimlanePaintLayout>,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    let mut subgraphs = std::collections::HashMap::new();
    for group in &model.subgraphs {
        subgraphs.entry(group.id.as_str()).or_insert(group);
    }
    let lanes: std::collections::HashMap<_, _> = swimlane
        .into_iter()
        .flat_map(|layout| &layout.lanes)
        .filter(|lane| lane.parent_id.is_none())
        .map(|lane| (lane.id.as_str(), lane))
        .collect();
    for (cluster, dom_id) in clusters.iter().zip(dom_ids) {
        let subgraph = subgraphs.get(cluster.id.as_str()).copied();
        if let (Some(lane), Some(paint)) = (lanes.get(cluster.id.as_str()), swimlane) {
            swimlane::render_cluster(out, cluster, dom_id, lane, paint, subgraph, context)?;
            continue;
        }
        let Some(subgraph) = subgraph else {
            continue;
        };
        render_er_subgraph_cluster(out, cluster, dom_id, subgraph, context)?;
    }
    Ok(())
}

fn er_cluster_dom_ids(
    clusters: &[LayoutCluster],
    model: &ErDiagramModel,
    geometry: &ErSvgGeometry<'_>,
    label_rect_nodes: bool,
    diagram_id: SvgId<'_>,
    work: &OperationWorkMeter,
) -> Result<Vec<String>> {
    work.charge(
        clusters
            .len()
            .saturating_add(geometry.nodes().len())
            .saturating_add(geometry.edges().len()),
    )?;
    let subgraphs: HashSet<_> = model
        .subgraphs
        .iter()
        .map(|group| group.id.as_str())
        .collect();
    let original: Vec<_> = clusters
        .iter()
        .map(|cluster| {
            if subgraphs.contains(cluster.id.as_str()) {
                format!("{diagram_id}-{}", cluster.id)
            } else {
                cluster.id.clone()
            }
        })
        .collect();
    let original_ids: HashSet<_> = original.iter().map(String::as_str).collect();
    let mut occupied = HashSet::from([
        diagram_id.semantic_str().to_owned(),
        format!("chart-title-{diagram_id}"),
        format!("chart-desc-{diagram_id}"),
        format!("{diagram_id}-drop-shadow"),
        format!("{diagram_id}-drop-shadow-small"),
        format!("{diagram_id}-gradient"),
    ]);
    for node in geometry.nodes().iter().filter(|node| !node.is_cluster) {
        occupied.insert(format!("{diagram_id}-{}", node.id));
        if node.id.contains("---") {
            occupied.insert(node.id.clone());
        }
    }
    for edge in geometry.edges() {
        let id = er_edge_dom_id(&edge.id, &model.relationships);
        occupied.insert(format!("{diagram_id}-{id}"));
        if label_rect_nodes {
            occupied.insert(swimlane::relationship_label_dom_id(edge, &id));
        }
    }
    let internal_prefix = format!("{diagram_id}_er-");
    let mut ordinal = 0;
    original
        .iter()
        .map(|id| {
            work.checkpoint(OperationPhase::Emit)?;
            if !id.starts_with(&internal_prefix) && occupied.insert(id.clone()) {
                return Ok(id.clone());
            }
            loop {
                let candidate = format!("{diagram_id}-er-cluster-{ordinal}");
                ordinal += 1;
                if !original_ids.contains(candidate.as_str()) && occupied.insert(candidate.clone())
                {
                    return Ok(candidate);
                }
            }
        })
        .collect()
}

fn er_edge_dom_id(edge_id: &str, relationships: &[ErRelationship]) -> String {
    let Some(idx) = er_rel_idx_from_edge_id(edge_id) else {
        return edge_id.to_string();
    };
    let Some(rel) = relationships.get(idx) else {
        return edge_id.to_string();
    };
    // The SVG-facing layout projection merges Dagre's three internal self-loop segments into
    // one logical relationship edge. Mermaid's common painter consequently uses the ordinary
    // relationship id for that visible edge; the `cyclic-special-*` ids belong only to the
    // hidden helper labels that remain in the layout model.
    er_relationship_dom_id(rel, idx)
}

#[derive(Default)]
struct RoughRectBorderPath {
    path: String,
    fill_path: String,
    coordinates: [String; 4],
    ryu: ryu_js::Buffer,
}

#[inline]
fn push_rough_f64(out: &mut String, ryu: &mut ryu_js::Buffer, value: f64) {
    if value == 0.0 {
        if value.is_sign_negative() {
            out.push_str("-0");
        } else {
            out.push('0');
        }
    } else if value.is_finite() {
        let abs = value.abs();
        if (1e-6..1e15).contains(&abs) {
            out.push_str(ryu.format_finite(value));
        } else {
            let _ = write!(out, "{value}");
        }
    } else {
        let _ = write!(out, "{value}");
    }
}

impl RoughRectBorderPath {
    fn fill_path_d(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> &str {
        self.fill_path.clear();
        self.fill_path.push('M');
        push_rough_f64(&mut self.fill_path, &mut self.ryu, x0);
        self.fill_path.push(' ');
        push_rough_f64(&mut self.fill_path, &mut self.ryu, y0);
        self.fill_path.push_str(" L");
        push_rough_f64(&mut self.fill_path, &mut self.ryu, x1);
        self.fill_path.push(' ');
        push_rough_f64(&mut self.fill_path, &mut self.ryu, y0);
        self.fill_path.push_str(" L");
        push_rough_f64(&mut self.fill_path, &mut self.ryu, x1);
        self.fill_path.push(' ');
        push_rough_f64(&mut self.fill_path, &mut self.ryu, y1);
        self.fill_path.push_str(" L");
        push_rough_f64(&mut self.fill_path, &mut self.ryu, x0);
        self.fill_path.push(' ');
        push_rough_f64(&mut self.fill_path, &mut self.ryu, y1);
        &self.fill_path
    }

    fn path_d(
        &mut self,
        randomness: &roughr::core::RoughRandomness,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> &str {
        let w = (x1 - x0).max(0.0);
        let h = (y1 - y0).max(0.0);
        let x2 = x0 + w;
        let y2 = y0 + h;
        let mut options = roughr::core::OptionsBuilder::default()
            .randomness(randomness.clone())
            .build()
            .expect("ER rough rectangle options must be valid");

        self.path.clear();
        for (text, value) in self.coordinates.iter_mut().zip([x0, y0, x2, y2]) {
            text.clear();
            push_rough_f64(text, &mut self.ryu, value);
        }
        let [left, top, right, bottom] = self.coordinates.each_ref().map(String::as_str);

        // Rough.js v4.6.6 rectangle -> polygon -> linearPath follows the four sides in order.
        for (horizontal, (start, start_text), (end, end_text), (fixed, fixed_text)) in [
            (true, (x0, left), (x2, right), (y0, top)),
            (false, (y0, top), (y2, bottom), (x2, right)),
            (true, (x2, right), (x0, left), (y2, bottom)),
            (false, (y2, bottom), (y0, top), (x0, left)),
        ] {
            let delta = end - start;
            // The fixed-axis delta is +0 for finite coordinates. With positive d, the original
            // addition turns -0 into +0. Subtracting a non-finite coordinate from itself yields NaN.
            let fixed_control = if fixed == 0.0 {
                "0"
            } else if fixed.is_finite() {
                fixed_text
            } else {
                "NaN"
            };

            // Even at roughness=0, each stroke draws a divergePoint and consumes ten
            // more random values for midDisp, moveTo, and bcurveTo.
            for _ in 0..2 {
                let d = 0.2 + options.random() * 0.2;
                for _ in 0..10 {
                    let _ = options.random();
                }
                let c1 = start + delta * d;
                let c2 = start + delta * 2.0 * d;
                if horizontal {
                    self.path.push('M');
                    self.path.push_str(start_text);
                    self.path.push(' ');
                    self.path.push_str(fixed_text);
                    self.path.push_str(" C");
                    push_rough_f64(&mut self.path, &mut self.ryu, c1);
                    self.path.push(' ');
                    self.path.push_str(fixed_control);
                    self.path.push_str(", ");
                    push_rough_f64(&mut self.path, &mut self.ryu, c2);
                    self.path.push(' ');
                    self.path.push_str(fixed_control);
                    self.path.push_str(", ");
                    self.path.push_str(end_text);
                    self.path.push(' ');
                    self.path.push_str(fixed_text);
                    self.path.push(' ');
                } else {
                    self.path.push('M');
                    self.path.push_str(fixed_text);
                    self.path.push(' ');
                    self.path.push_str(start_text);
                    self.path.push_str(" C");
                    self.path.push_str(fixed_control);
                    self.path.push(' ');
                    push_rough_f64(&mut self.path, &mut self.ryu, c1);
                    self.path.push_str(", ");
                    self.path.push_str(fixed_control);
                    self.path.push(' ');
                    push_rough_f64(&mut self.path, &mut self.ryu, c2);
                    self.path.push_str(", ");
                    self.path.push_str(fixed_text);
                    self.path.push(' ');
                    self.path.push_str(end_text);
                    self.path.push(' ');
                }
            }
        }

        self.path.trim_end()
    }
}

pub fn render_er_diagram_svg_model(
    layout: ErPreparedSvgLayout<'_>,
    model: &ErDiagramModel,
    entity_theme: &ErEntityThemePlan,
    effective_config: &PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &ErSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let ErPreparedSvgLayout {
        geometry: layout,
        swimlane,
        prepared_labels,
    } = layout;
    let clusters = layout.clusters;
    let diagram_id = options.diagram_id;
    // Mermaid's internal diagram type for ER is `er` (not `erDiagram`), and marker ids are derived
    // from this type (e.g. `<diagramId>_er-zeroOrMoreEnd`).
    let diagram_type = "er";
    let binding = entity_theme.css_binding();
    let marker_background = if clusters.is_empty() {
        zero_markers::background(
            effective_config,
            binding,
            options.canvas_background,
            options.canvas_has_texture,
        )
    } else {
        // The backing beneath a zero marker can be a subgraph or a lane rather than the canvas.
        std::borrow::Cow::Borrowed("none")
    };
    let effective_config = effective_config.as_value();
    let native = binding.native.as_ref();
    let sketch = native.is_some_and(|style| style.edge_style == EdgeStyle::Sketch);
    let er_render_settings = &binding.render_settings;
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = options.canonical_routes;
    #[cfg(feature = "layout-shoal")]
    let uses_canonical_routes = canonical_routes.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let uses_canonical_routes = false;
    let is_elk_layout = options.elk_parity;
    let is_swimlane_layout = options.swimlane;
    #[cfg(feature = "layout-grid")]
    let grid = options
        .grid
        .map(|settings| grid::paint_settings(settings, native));
    #[cfg(feature = "layout-grid")]
    let is_grid_layout = grid.is_some();
    #[cfg(not(feature = "layout-grid"))]
    let is_grid_layout = false;
    #[cfg(feature = "layout-layered")]
    let layered = options.layered.and_then(super::ErLayeredPaint::routes);
    #[cfg(feature = "layout-layered")]
    let is_layered_layout = layered.is_some();
    #[cfg(not(feature = "layout-layered"))]
    let is_layered_layout = false;
    let label_rect_nodes = is_swimlane_layout || is_grid_layout;
    let allow_html_label_overflow = uses_canonical_routes || is_swimlane_layout;
    let rounded_layout = is_elk_layout || is_swimlane_layout;
    let route_kind = if uses_canonical_routes {
        ErSvgRouteKind::Canonical
    } else if is_elk_layout {
        ErSvgRouteKind::Elk
    } else if is_swimlane_layout {
        ErSvgRouteKind::Swimlane
    } else {
        ErSvgRouteKind::Dagre
    };
    #[cfg(feature = "layout-grid")]
    let route_kind = if is_grid_layout {
        ErSvgRouteKind::Grid
    } else {
        route_kind
    };
    #[cfg(feature = "layout-layered")]
    let route_kind = if is_layered_layout {
        ErSvgRouteKind::Layered
    } else {
        route_kind
    };
    let data_look = er_render_settings.diagram_look.as_str();
    let redux_border_colors = &binding.redux_border_colors;
    let color_indices = (!is_layered_layout).then(|| er_color_indices(model));
    let node_border = &binding.node_border;
    let font_family = er_render_settings.font_family.clone();
    let font_size = er_render_settings.font_size;
    let title_top_margin = er_render_settings.title_top_margin;
    let use_max_width = er_render_settings.use_max_width;
    let label_style = er_render_settings.label_style.clone();
    let edge_html_labels = entity_theme.relationship_html_labels();
    #[cfg(feature = "layout-shoal")]
    let cutout_labels = uses_canonical_routes
        && cutout::is_transparent_paint(&binding.edge_label_background)
        && (!edge_html_labels
            || binding
                .label_background
                .as_ref()
                .is_ok_and(|paint| cutout::is_transparent_paint(paint)));
    let swimlane_html_labels = ErConfigView::new(effective_config).swimlane_title_html_labels();
    let entity_wrap_mode = er_render_settings.entity_html_label_wrap_mode;
    let entity_measurement = er_render_settings.entity_measurement;
    let hand_drawn_seed = options.rough_randomness;
    let hand_drawn =
        (uses_canonical_routes && data_look == "handDrawn").then_some(hand_drawn::HandDrawn {
            work_meter: options.work_meter,
            randomness: hand_drawn_seed,
        });
    let insert_title_top_margin = er_render_settings.insert_title_top_margin;
    let geometry = ErSvgGeometry::prepare(
        layout,
        route_kind,
        &model.relationships,
        effective_config,
        options.work_meter,
    )?;
    let nodes = geometry.nodes();
    let edges = geometry.edges();
    #[cfg(feature = "layout-layered")]
    if let Some(routes) = layered
        && routes.edges.len() != edges.len()
    {
        return Err(super::layered::invalid_projection(
            "route count does not match SVG edges",
        ));
    }

    let include_md_parent = edges.iter().any(|e| {
        matches!(
            e.start_marker.as_deref(),
            Some("MD_PARENT_START") | Some("MD_PARENT_END")
        ) || matches!(
            e.end_marker.as_deref(),
            Some("MD_PARENT_START") | Some("MD_PARENT_END")
        )
    });

    let diagram_title = diagram_title.map(str::trim).filter(|t| !t.is_empty());
    let is_empty_diagram = nodes.is_empty() && edges.is_empty() && diagram_title.is_none();

    let bounds = geometry.bounds().unwrap_or({
        if is_empty_diagram {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            }
        } else {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            }
        }
    });

    let mut content_bounds = bounds.clone();
    let diagram_title_x = diagram_title.map(|title| {
        let title_style = text::TextStyle {
            font_family: native
                .and_then(|style| style.title_family.clone())
                .or_else(|| Some(font_family.clone())),
            font_size,
            font_weight: native.map(|style| style.title_weight.clone()),
            font_style: None,
        };
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let (title_ascent, title_descent) = text::svg_text_vertical_extents_px(
            measurer,
            title,
            &title_style,
            text::svg_title_bbox_vertical_extents_px(&title_style),
        );
        let w = (content_bounds.max_x - content_bounds.min_x).max(1.0);
        let title_x = content_bounds.min_x + w / 2.0;
        let title_y = -title_top_margin;
        let title_min_x = title_x - title_left;
        let title_max_x = title_x + title_right;
        let title_min_y = title_y - title_ascent;
        let title_max_y = title_y + title_descent;
        content_bounds.min_x = content_bounds.min_x.min(title_min_x);
        content_bounds.max_x = content_bounds.max_x.max(title_max_x);
        content_bounds.min_y = content_bounds.min_y.min(title_min_y);
        content_bounds.max_y = content_bounds.max_y.max(title_max_y);
        title_x
    });

    let pad = options.viewbox_padding.max(0.0);
    let mut out = BoundedSvgOutput::new(options.work_meter);
    let root_bounds = |content_bounds: &Bounds| {
        if is_empty_diagram {
            let empty_span = (pad * 2.0).max(1.0);
            root_svg::DiagramBounds::from_view_box(-pad, -pad, empty_span, empty_span)
        } else {
            let content_w = (content_bounds.max_x - content_bounds.min_x).max(1.0);
            let content_h = (content_bounds.max_y - content_bounds.min_y).max(1.0);
            root_svg::DiagramBounds::from_view_box(
                content_bounds.min_x - pad,
                content_bounds.min_y - pad,
                content_w + pad * 2.0,
                content_h + pad * 2.0,
            )
        }
    };
    let root_spec = |bounds: root_svg::DiagramBounds| {
        root_svg::RootViewportSpec::mermaid(bounds, use_max_width)
            .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(bounds.width))
    };
    let (translate_x, translate_y) = (0.0, 0.0);
    let initial_bounds = root_bounds(&content_bounds);
    let root_viewport = &options.viewport;
    let root_plan = root_viewport.plan(root_spec(initial_bounds))?;

    let has_acc_title = model.acc_title.as_ref().is_some_and(|s| !s.is_empty());
    let has_acc_descr = model.acc_descr.as_ref().is_some_and(|s| !s.is_empty());
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, diagram_type);
    root_chrome.class = Some("erDiagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    #[cfg(feature = "layout-layered")]
    if options.layered.is_some() && !is_layered_layout {
        root_chrome.extra_attrs = &[("data-layout-fallback", "dagre")];
    }
    let defer_paint_bounds = uses_canonical_routes || is_grid_layout || is_layered_layout;
    let root_document = if defer_paint_bounds {
        root_viewport.begin_document(
            &mut out,
            root_svg::DeferredRootSpec::mermaid(use_max_width),
            root_chrome,
        )?
    } else {
        root_viewport.write_plan(&mut out, &root_plan, root_chrome)?
    };
    options.work_meter.checkpoint(OperationPhase::Emit)?;

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}"#,
            diagram_id,
            escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
        out.push_str("</title>");
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}"#,
            diagram_id,
            escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
        out.push_str("</desc>");
    }
    out.checkpoint()?;

    write_er_stylesheet_with_cluster_titles(
        &mut out,
        diagram_id.semantic_str(),
        data_look,
        binding,
        !clusters.is_empty() && (edge_html_labels || (is_swimlane_layout && swimlane_html_labels)),
    )?;

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");

    write_er_marker_definitions_with_inset(
        &mut out,
        ErMarkerDefs {
            diagram_id,
            include_md_parent,
            canonical: uses_canonical_routes,
            neo: data_look == "neo",
            neo_stroke_width: binding.neo_marker_stroke_width,
            main_bkg: &marker_background,
        },
        uses_canonical_routes && model.direction.trim().eq_ignore_ascii_case("BT"),
    )?;

    let mut entity_by_id: std::collections::HashMap<&str, &ErEntity> =
        std::collections::HashMap::new();
    for e in model.entities.values() {
        entity_by_id.insert(e.id.as_str(), e);
    }
    let subgraph_context = ErSubgraphRenderContext {
        data_look,
        uses_canonical_routes,
        layered_titles: is_layered_layout,
        allow_html_label_overflow,
        use_html_labels: edge_html_labels,
        measurer,
        label_style: &label_style,
        translate_x,
        translate_y,
        theme: entity_theme,
        swimlane_html_labels,
        synthetic_color_ordinal: nodes.iter().filter(|node| !node.is_cluster).count(),
        work_meter: options.work_meter,
        rough_randomness: hand_drawn_seed,
    };

    // Mermaid 12 keeps the cluster wrapper in the common painter for both layout providers.
    let _ = writeln!(&mut out, r#"<g class="root">"#);
    if clusters.is_empty() {
        out.push_str(r#"<g class="clusters"/>"#);
    } else {
        let cluster_ids = er_cluster_dom_ids(
            clusters,
            model,
            &geometry,
            label_rect_nodes,
            diagram_id,
            options.work_meter,
        )?;
        out.push_str(r#"<g class="clusters">"#);
        render_er_subgraph_clusters(
            &mut out,
            clusters,
            &cluster_ids,
            model,
            swimlane,
            subgraph_context,
        )?;
        out.push_str("</g>");
    }

    if is_elk_layout || is_grid_layout || is_layered_layout {
        out.push_str(r#"<g class="edges edgePaths">"#);
    } else {
        out.push_str(r#"<g class="edgePaths">"#);
    }
    let line_hops_enabled = rounded_layout
        && !sketch
        && options.include_edges
        && effective_config
            .pointer(if is_swimlane_layout {
                "/swimlane/lineHops"
            } else {
                "/elk/lineHops"
            })
            .and_then(serde_json::Value::as_bool)
            != Some(false);
    let shifted_hop_points: Vec<Vec<LayoutPoint>> = if line_hops_enabled {
        options
            .work_meter
            .charge(edges.iter().fold(0usize, |work, edge| {
                work.saturating_add(edge.points.len()).saturating_add(1)
            }))?;
        edges
            .iter()
            .map(|edge| {
                edge.points
                    .iter()
                    .map(|point| LayoutPoint {
                        x: point.x + translate_x,
                        y: point.y + translate_y,
                    })
                    .collect()
            })
            .collect()
    } else {
        Vec::new()
    };
    let hop_paths = if line_hops_enabled {
        options.work_meter.charge(edges.len())?;
        let hop_edges: Vec<_> = edges
            .iter()
            .zip(&shifted_hop_points)
            .map(|(edge, points)| line_hops::LineHopEdge {
                id: edge.id.as_str(),
                points,
                curve: Some(if geometry.is_missing_section(&edge.id) {
                    "linear"
                } else {
                    "rounded"
                }),
                arrow_type_start: None,
                arrow_type_end: None,
            })
            .collect();
        if is_swimlane_layout {
            swimlane::line_hop_paths(effective_config, &hop_edges, options.work_meter)?
        } else {
            line_hops::elk_line_hop_paths(effective_config, &hop_edges, options.work_meter)?
        }
    } else {
        std::collections::HashMap::new()
    };
    #[cfg(feature = "layout-grid")]
    let hop_paths = if let Some(settings) = grid.filter(|_| options.include_edges && !sketch) {
        grid::line_hop_paths(edges, settings, options.work_meter)?
    } else {
        hop_paths
    };
    #[cfg(feature = "layout-layered")]
    let hop_paths = if let Some(routes) = layered.filter(|_| options.include_edges && !sketch) {
        layered::line_hop_paths(edges, routes, effective_config, options.work_meter)?
    } else {
        hop_paths
    };
    out.checkpoint()?;
    if options.include_edges {
        #[cfg(feature = "layout-shoal")]
        let mut paint_scratch = CanonicalPaintScratch::default();
        let mut points_json = Vec::new();
        let mut points_base64 = String::new();
        #[cfg(feature = "layout-layered")]
        let mut axis_json = layered::AxisPointJson::default();
        for (edge_index, e) in edges.iter().enumerate() {
            let missing_section = geometry.is_missing_section(&e.id);
            if !uses_canonical_routes
                && (e.points.is_empty() || (!missing_section && e.points.len() < 2))
            {
                continue;
            }
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let edge_svg_id = format!("{diagram_id}-{edge_dom_id}");
            options.work_meter.checkpoint(OperationPhase::Emit)?;
            let is_dashed = e.stroke_dasharray.as_deref() == Some("8,8");
            let pattern_class = if is_dashed {
                "edge-pattern-dashed"
            } else {
                "edge-pattern-solid"
            };
            let line_classes = format!("edge-thickness-normal {pattern_class} relationshipLine");
            #[cfg(feature = "layout-layered")]
            let layered_route = layered.map(|routes| (&routes.edges[edge_index], routes.curve));
            #[cfg(feature = "layout-shoal")]
            let mut canonical_paint_bounds = None;
            let legacy_points;
            let (original_d, shifted) = 'path: {
                #[cfg(feature = "layout-shoal")]
                if let Some(paint) = canonical_routes {
                    let painted = paint(
                        e,
                        kurbo::Vec2::new(translate_x, translate_y),
                        hand_drawn_seed,
                        &mut paint_scratch,
                    )?;
                    content_bounds.min_x = content_bounds.min_x.min(painted.bounds.x0);
                    content_bounds.min_y = content_bounds.min_y.min(painted.bounds.y0);
                    content_bounds.max_x = content_bounds.max_x.max(painted.bounds.x1);
                    content_bounds.max_y = content_bounds.max_y.max(painted.bounds.y1);
                    canonical_paint_bounds = cutout_labels.then_some(painted.bounds);
                    break 'path (painted.d, painted.points);
                }
                let points = if is_layered_layout {
                    // Layered leaves and clusters are rectangular, so the solved contacts and
                    // the emitted metadata share the same untranslated points.
                    e.points.as_slice()
                } else {
                    legacy_points = e
                        .points
                        .iter()
                        .map(|p| LayoutPoint {
                            x: p.x + translate_x,
                            y: p.y + translate_y,
                        })
                        .collect::<Vec<_>>();
                    legacy_points.as_slice()
                };
                let fallback_path =
                    || er_edge_path_d(points, rounded_layout, missing_section, native);
                #[cfg(feature = "layout-grid")]
                let grid_path = grid
                    .map(|settings| grid::shaft(points, settings, options.work_meter))
                    .transpose()?;
                #[cfg(feature = "layout-grid")]
                let fallback_path = || grid_path.unwrap_or_else(fallback_path);
                #[cfg(feature = "layout-layered")]
                let mut d = match layered_route {
                    Some((route, curve)) => layered::shaft(route, curve, options.work_meter)?,
                    None => fallback_path(),
                };
                #[cfg(not(feature = "layout-layered"))]
                let mut d = fallback_path();
                if let Some(native) = native.filter(|_| sketch && !d.is_empty()) {
                    let [start, end] =
                        [e.start_marker.as_deref(), e.end_marker.as_deref()].map(|marker| {
                            super::markers::er_compatibility_sketch_marker(
                                marker,
                                data_look == "neo",
                                native.connector_width,
                            )
                        });
                    d = merman_render_core::rough_svg::constrained_rough_edge_path(
                        &d,
                        (start, end),
                        options.work_meter,
                        hand_drawn_seed,
                    )?;
                    if let Some(bounds) = merman_render_core::svg_bounds::svg_path_bounds_from_d(&d)
                    {
                        let outset = native.connector_width / std::f64::consts::SQRT_2;
                        content_bounds.min_x = content_bounds.min_x.min(bounds.min_x - outset);
                        content_bounds.min_y = content_bounds.min_y.min(bounds.min_y - outset);
                        content_bounds.max_x = content_bounds.max_x.max(bounds.max_x + outset);
                        content_bounds.max_y = content_bounds.max_y.max(bounds.max_y + outset);
                    }
                }
                break 'path (d, points);
            };
            points_base64.clear();
            #[cfg(feature = "layout-layered")]
            if let Some((route, _)) = layered_route {
                axis_json.encode(&mut points_base64, shifted, route, options.work_meter)?;
            }
            if !is_layered_layout {
                points_json.clear();
                points_json.reserve(128);
                if serde_json::to_writer(&mut points_json, shifted).is_err() {
                    points_json.clear();
                }
                base64::engine::general_purpose::STANDARD
                    .encode_string(&points_json, &mut points_base64);
            }
            let hopped_d = hop_paths.get(e.id.as_str()).map(String::as_str);
            let d = hopped_d.unwrap_or(&original_d);
            if is_grid_layout || is_layered_layout {
                options.work_meter.charge_emit_work(d.len())?;
                if let Some(bounds) = merman_render_core::svg_bounds::svg_path_bounds_from_d(d) {
                    content_bounds.min_x = content_bounds.min_x.min(bounds.min_x);
                    content_bounds.min_y = content_bounds.min_y.min(bounds.min_y);
                    content_bounds.max_x = content_bounds.max_x.max(bounds.max_x);
                    content_bounds.max_y = content_bounds.max_y.max(bounds.max_y);
                }
            }

            let mut edge_style = String::new();
            if data_look == "neo"
                && !sketch
                && let Some(length) = svg_path_length_from_d(&original_d)
            {
                edge_path::write_neo_edge_mask(
                    &mut edge_style,
                    length,
                    None,
                    None,
                    is_dashed,
                    false,
                );
            }
            if !uses_canonical_routes {
                edge_style.push_str(if is_elk_layout {
                    "fill:none;;;fill:none"
                } else {
                    "undefined;;;undefined"
                });
            }
            let edge_style = if let Some(hopped_d) = hopped_d {
                if is_grid_layout || is_layered_layout {
                    line_hops::rewrite_style_after_line_hop_with_phase(
                        &edge_style,
                        hopped_d,
                        OperationPhase::Emit,
                        options.work_meter,
                    )?
                } else {
                    line_hops::rewrite_style_after_line_hop(
                        &edge_style,
                        hopped_d,
                        options.work_meter,
                    )?
                }
            } else {
                std::borrow::Cow::Borrowed(edge_style.as_str())
            };

            let marker_cutout = if marker_background == "none" {
                let id = format!("{diagram_id}_er-marker-hole-{edge_index}");
                zero_markers::write_cutout(
                    &mut out,
                    &id,
                    d,
                    e,
                    binding,
                    uses_canonical_routes,
                    options.work_meter,
                )?
                .then_some(id)
            } else {
                None
            };
            #[cfg(feature = "layout-shoal")]
            let mut caption_cutout = None;
            #[cfg(feature = "layout-shoal")]
            if let Some((bounds, label)) = canonical_paint_bounds.zip(e.label.as_ref())
                && er_rel_idx_from_edge_id(&e.id)
                    .and_then(|index| model.relationships.get(index))
                    .is_some_and(|relation| !relation.role_a.trim().is_empty())
            {
                let gap = kurbo::Rect::from_center_size(
                    (label.x + translate_x, label.y + translate_y),
                    (label.width, label.height),
                );
                if bounds.intersect(gap).area() > 0.0 {
                    let id = format!("{diagram_id}_er-edge-label-{edge_index}");
                    cutout::write_defs(&mut out, &id, bounds, 1.0, &[gap], options.work_meter)?;
                    caption_cutout = Some(id);
                }
            }

            // The standard Base64 alphabet is safe in a quoted XML attribute.
            let _ = write!(
                &mut out,
                r#"<path d="{}" id="{}" class="{}""#,
                escape_attr(d),
                escape_attr(&edge_svg_id),
                escape_attr(&line_classes),
            );
            if !uses_canonical_routes || !edge_style.is_empty() {
                let _ = write!(&mut out, r#" style="{}""#, escape_attr(&edge_style));
            }
            let _ = write!(
                &mut out,
                r#" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
                escape_attr(&edge_dom_id),
                points_base64,
                escape_attr(data_look)
            );
            if let Some(id) = marker_cutout {
                let _ = write!(&mut out, r#" mask="url(#{})""#, escape_attr(&id));
            }
            #[cfg(feature = "layout-shoal")]
            if let Some(id) = caption_cutout {
                cutout::write_reference(&mut out, id);
            }
            let start_marker_id = e
                .start_marker
                .as_deref()
                .map(|marker| er_unified_marker_id(diagram_id, marker));
            let end_marker_id = e
                .end_marker
                .as_deref()
                .map(|marker| er_unified_marker_id(diagram_id, marker));
            if let Some(marker) = &start_marker_id {
                let _ = write!(&mut out, r#" marker-start="url(#{})""#, escape_attr(marker));
            }
            if let Some(marker) = &end_marker_id {
                let _ = write!(&mut out, r#" marker-end="url(#{})""#, escape_attr(marker));
            }
            options.work_meter.checkpoint(OperationPhase::Emit)?;
            out.push_str(" />");
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if label_rect_nodes {
        out.push_str(r#"<g class="edgeLabels"/><g class="nodes">"#);
    } else {
        out.push_str(r#"<g class="edgeLabels">"#);
    }
    out.checkpoint()?;
    if options.include_edges && !is_grid_layout {
        for e in edges {
            let rel_idx = er_rel_idx_from_edge_id(&e.id)
                .and_then(|idx| model.relationships.get(idx).map(|r| (idx, r)));

            let rel_text_raw = rel_idx.map(|(_, r)| r.role_a.as_str()).unwrap_or("");
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let relationship_index = rel_idx.map(|(index, _)| index);
            let prepared_relationship =
                relationship_index.and_then(|index| prepared_labels.relationship(index));
            // The common ELK/Swimlane/Layered painter checks hasEdgeLabel before preparing
            // a terminal; Dagre also emits its zero-size wrapper for an empty role.
            if rel_text_raw.is_empty() && (is_elk_layout || is_swimlane_layout || is_layered_layout)
            {
                continue;
            }
            if is_swimlane_layout {
                if let Some((index, _)) = rel_idx {
                    swimlane::render_relationship_label(
                        &mut out,
                        e,
                        index,
                        model,
                        prepared_labels,
                        subgraph_context,
                    )?;
                }
                continue;
            }
            let svg_relationship = if edge_html_labels {
                None
            } else {
                Some(
                    prepared_relationship
                        .and_then(ErPreparedRelationshipLabel::svg)
                        .ok_or_else(|| Error::InvalidModel {
                            message: format!(
                                "missing prepared ER SVG relationship label for `{}`",
                                e.id
                            ),
                        })?,
                )
            };
            let (w, h, mut cx, mut cy) = if let Some(lbl) = &e.label {
                (
                    lbl.width.max(0.0),
                    lbl.height.max(0.0),
                    lbl.x + translate_x,
                    lbl.y + translate_y,
                )
            } else {
                let (x, y) = edge_label_geometry::calc_label_position(&e.points)
                    .map(|point| (point.x + translate_x, point.y + translate_y))
                    .unwrap_or((0.0, 0.0));
                (0.0, 0.0, x, y)
            };
            #[cfg(feature = "layout-layered")]
            if e.label.is_none()
                && let Some(routes) = layered
            {
                cy += routes.label_y_offset;
            }

            if !uses_canonical_routes
                && !is_layered_layout
                && w > 0.0
                && h > 0.0
                && !geometry.is_missing_section(&e.id)
            {
                // Mermaid 12.1 preserves the layout anchor and adds only the midpoint delta
                // from paint-time clipping. ER has no additional clipping here: curve and marker
                // projection change `d`, while both label polylines remain the layout route.
                let shifted: Vec<LayoutPoint> = e
                    .points
                    .iter()
                    .map(|p| LayoutPoint {
                        x: p.x + translate_x,
                        y: p.y + translate_y,
                    })
                    .collect();
                let rendered_d = er_edge_path_d(&shifted, rounded_layout, false, native);
                let position = edge_label_geometry::position_edge_label(
                    LayoutPoint { x: cx, y: cy },
                    Some(&shifted),
                    &shifted,
                    &rendered_d,
                    false,
                );
                cx = position.x;
                cy = position.y;
            }

            if w > 0.0 && h > 0.0 {
                // Mermaid ER relationship labels follow Mermaid's effective HTML-label
                // resolution: root `htmlLabels`, then `flowchart.htmlLabels`, then default
                // `true`. When both are unset, upstream still emits HTML `<foreignObject>`
                // labels through `createText(...)`.
                let _ = write!(
                    &mut out,
                    r#"<g class="edgeLabel" transform="translate({}, {})">"#,
                    fmt(cx),
                    fmt(cy)
                );
                // Layout reserves the padded group, but computeLabelTransform centers
                // the text's ink bbox, including its local origin and asymmetry.
                let (label_tx, label_ty) = svg_relationship
                    .map(|label| {
                        let ink = &label.ink_bounds;
                        (
                            -(ink.min_x + ink.max_x) / 2.0,
                            -(ink.min_y + ink.max_y) / 2.0,
                        )
                    })
                    .unwrap_or((-w / 2.0, -h / 2.0));
                let _ = write!(
                    &mut out,
                    r#"<g class="label" data-id="{}" transform="translate({}, {})""#,
                    escape_attr_display(&edge_dom_id),
                    fmt(label_tx),
                    fmt(label_ty)
                );
                out.push('>');
                if edge_html_labels {
                    let _ = write!(
                        &mut out,
                        r#"<foreignObject width="{}" height="{}"{}>"#,
                        fmt(w),
                        fmt(h),
                        html_label_overflow_attr(allow_html_label_overflow, w, h),
                    );
                    let constrained = prepared_relationship
                        .is_some_and(ErPreparedRelationshipLabel::html_is_constrained);
                    let max_width = text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
                    let _ = write!(
                        &mut out,
                        r#"<div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: {}; white-space: {}; line-height: 1.5; max-width: {}px; text-align: center;"#,
                        if constrained { "table" } else { "table-cell" },
                        if constrained {
                            "break-spaces"
                        } else {
                            "nowrap"
                        },
                        fmt(max_width),
                    );
                    if constrained {
                        let _ = write!(&mut out, " width: {}px;", fmt(max_width));
                    }
                    out.push_str(r#""><span class="edgeLabel">"#);
                    // Mermaid ER relationship labels use the generic HTML edge-label path, so they
                    // inherit `markdownToHTML()` semantics (Markdown emphasis + inline `<br/>`
                    // handling) rather than rendering literal `**...**` marker text.
                    out.push_str(
                        prepared_relationship
                            .map(ErPreparedRelationshipLabel::xhtml_fragment)
                            .unwrap_or_default(),
                    );
                    out.push_str(r#"</span></div></foreignObject></g></g>"#);
                } else if let Some(svg_label) = svg_relationship {
                    out.push_str("<g>");
                    if let Some(background) = &svg_label.background_bounds {
                        let _ = write!(
                            &mut out,
                            r#"<rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                            fmt(background.min_x),
                            fmt(background.min_y),
                            fmt(background.max_x - background.min_x),
                            fmt(background.max_y - background.min_y)
                        );
                    }
                    // The shared writer places the anchor on the text and each outer
                    // tspan, matching createFormattedText's centered terminal DOM.
                    label::write_svg_markdown_word_lines_centered_with_style(
                        &mut out,
                        &svg_label.word_lines,
                        Some(""),
                        SvgTextEntityMode::CreateTextSource,
                    );
                    out.push_str("</g></g></g>");
                }
            } else {
                if edge_html_labels {
                    // Ordinary empty and whitespace roles retain zero-size wrappers,
                    // without reserving visible text geometry.
                    let _ = write!(
                        &mut out,
                        r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label""#,
                        fmt(cx),
                        fmt(cy)
                    );
                    let _ = write!(
                        &mut out,
                        r#" data-id="{}""#,
                        escape_attr_display(&edge_dom_id)
                    );
                    out.push_str(r#" transform="translate(0, 0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"></span></div></foreignObject></g></g>"#);
                } else {
                    let _ = write!(
                        &mut out,
                        r#"<g class="edgeLabel" transform="translate({}, {})"><g class="label""#,
                        fmt(cx),
                        fmt(cy)
                    );
                    let _ = write!(
                        &mut out,
                        r#" data-id="{}""#,
                        escape_attr_display(&edge_dom_id)
                    );
                    out.push_str(r#" transform="translate(0, 0)">"#);
                    // createText gates addSvgBackground on the raw source's truthiness.
                    // An empty role returns the text terminal directly, with one empty row.
                    label::write_svg_text_centered_from_create_text_source(&mut out, "", false);
                    out.push_str("</g></g>");
                }
            }
            out.checkpoint()?;
        }
    }
    if !label_rect_nodes {
        out.push_str("</g>\n");
    }
    out.checkpoint()?;

    // Entities drawn after relationships so they cover markers when overlapping.
    if !label_rect_nodes {
        out.push_str(r#"<g class="nodes">"#);
    }
    out.checkpoint()?;
    let mut rough_border = RoughRectBorderPath::default();
    for (ordinal, n) in nodes.iter().filter(|n| !n.is_cluster).enumerate() {
        let Some(entity) = entity_by_id.get(n.id.as_str()).copied() else {
            if n.id.contains("---") {
                let cx = n.x + translate_x;
                let cy = n.y + translate_y;
                let _ = write!(
                    &mut out,
                    r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})">"#,
                    escape_attr(&n.id),
                    fmt(cx),
                    fmt(cy)
                );
                out.push_str(r#"<rect width="0.1" height="0.1"/>"#);
                if matches!(entity_wrap_mode, text::WrapMode::HtmlLike) {
                    out.push_str(r#"<g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#);
                } else {
                    out.push_str(
                        r#"<g class="label" style="" transform="translate(0, 0)"><rect/>"#,
                    );
                    write_er_svg_label_background(&mut out);
                    write_er_svg_plain_label(&mut out, "");
                    out.push_str("</g></g>");
                }
            }
            out.checkpoint()?;
            continue;
        };

        let Some(entity_index) = entity_theme.index_for_entity_id(&entity.id) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "ER entity theme plan is missing semantic entity {}",
                    entity.id
                ),
            });
        };

        let source_style =
            entity_theme
                .source_style(entity_index)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "ER entity theme plan is missing source style for {}",
                        entity.id
                    ),
                })?;
        let visual = source_style.visual();
        let source_label_style =
            source_style.label_style(matches!(entity_wrap_mode, text::WrapMode::HtmlLike));
        let label_div_style_prefix = format!(
            "{}{}",
            visual.label_div_color_prefix, visual.label_div_typography_prefix,
        );
        let measure = prepared_labels
            .entity(&entity.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared labels are missing entity {}", entity.id),
            })?;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let width_delta = w - measure.width;
        let height_delta = h - measure.height;
        // Shoal can grow the measured entity to fit the marker fan.
        let dimensions_match = if uses_canonical_routes {
            width_delta >= -1e-3 && height_delta >= -1e-3
        } else {
            width_delta.abs() <= 1e-3 && height_delta.abs() <= 1e-3
        };
        if !dimensions_match {
            return Err(Error::InvalidModel {
                message: format!(
                    "ER entity measured size mismatch for {}: layout=({},{}), measure=({}, {})",
                    n.id, w, h, measure.width, measure.height
                ),
            });
        }

        let cx = n.x + translate_x;
        let cy = n.y + translate_y;
        let ox = -w / 2.0;
        let oy = -h / 2.0;
        let node_rect = kurbo::Rect::new(ox, oy, ox + w, oy + h);
        let radius = native.map_or(0.0, |style| style.radius);
        let outline = hand_drawn
            .as_ref()
            .map(|draw| draw.outline(node_rect, radius))
            .transpose()?;
        if let Some(outline) = &outline {
            let stroke_width = source_style
                .rect_value("stroke-width")
                .and_then(|value| value.trim().trim_end_matches("px").trim().parse().ok())
                .filter(|width: &f64| width.is_finite())
                .unwrap_or_else(|| native.map_or(1.3, |style| style.border_width));
            let stroke_outset = stroke_width.max(0.0) * std::f64::consts::FRAC_1_SQRT_2;
            let bounds = outline.bounds.inflate(stroke_outset, stroke_outset);
            content_bounds.min_x = content_bounds.min_x.min(cx + bounds.x0);
            content_bounds.min_y = content_bounds.min_y.min(cy + bounds.y0);
            content_bounds.max_x = content_bounds.max_x.max(cx + bounds.x1);
            content_bounds.max_y = content_bounds.max_y.max(cy + bounds.y1);
        }

        let group_class = if entity.css_classes.trim().is_empty() {
            "node".to_string()
        } else {
            format!("node {}", entity.css_classes.trim())
        };
        let color_id_attr = color_indices
            .as_ref()
            .map_or(Some(ordinal), |indices| {
                indices.get(entity.id.as_str()).copied()
            })
            .and_then(|index| er_redux_color_id(redux_border_colors, index))
            .map(|color_id| format!(r#" data-color-id="{}""#, escape_attr(&color_id)))
            .unwrap_or_default();
        let _ = write!(
            &mut out,
            r#"<g id="{}-{}" class="{}" data-look="{}"{}"#,
            diagram_id,
            escape_attr(&entity.id),
            escape_attr(&group_class),
            escape_attr(data_look),
            color_id_attr,
        );
        if uses_canonical_routes {
            let _ = write!(
                &mut out,
                r#" data-et="node" data-id="{}""#,
                escape_attr(&n.id)
            );
        }
        if native.is_some() && source_style.fill().is_none() {
            out.push_str(r#" data-native-er-heading="true""#);
        }
        let _ = write!(
            &mut out,
            r#" transform="translate({}, {})">"#,
            fmt(cx),
            fmt(cy)
        );
        out.checkpoint()?;

        if entity.attributes.is_empty() {
            let entity_name_style_attr = er_text_style_attr(&source_label_style);
            let rect_style_attr = &visual.rect_style_attr;
            if let Some(outline) = &outline {
                let _ = write!(
                    &mut out,
                    r#"<path class="basic label-container" {rect_style_attr} d="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
                    escape_attr(&outline.d),
                    escape_attr(&visual.box_fill),
                    escape_attr(&visual.box_stroke),
                    escape_attr(&binding.stroke_width),
                );
            } else {
                let radius = native.map_or_else(String::new, |style| {
                    format!(r#" rx="{}""#, fmt(style.radius))
                });
                let _ = write!(
                    &mut out,
                    r#"<rect class="basic label-container" {} x="{}" y="{}" width="{}" height="{}"{radius}/>"#,
                    rect_style_attr,
                    fmt(ox),
                    fmt(oy),
                    fmt(w),
                    fmt(h)
                );
            }
            out.checkpoint()?;
            let wrap_mode = entity_wrap_mode;
            let label_metrics = measure.unformatted_label_metrics;
            let lw = if wrap_mode == text::WrapMode::HtmlLike {
                measure.label_html_width.max(0.0)
            } else {
                label_metrics.width.max(0.0)
            };
            let lh = label_metrics.height.max(0.0);

            if matches!(wrap_mode, text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label" transform="translate({}, {})" {}><rect/><foreignObject width="{}" height="{}"{}>"#,
                    fmt(-lw / 2.0),
                    fmt(-lh / 2.0),
                    entity_name_style_attr,
                    fmt(lw),
                    fmt(lh),
                    html_label_overflow_attr(allow_html_label_overflow, lw, lh),
                );
                measure.label.emit_html_div(
                    &mut out,
                    &label_div_style_prefix,
                    &visual.span_style_attr,
                    true,
                    "center",
                );
                out.push_str("</foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label" transform="translate({}, {})" {}><rect/><g transform="translate({}, 0)">"#,
                    fmt(0.0),
                    fmt(-lh / 2.0),
                    entity_name_style_attr,
                    fmt(-lw / 2.0),
                );
                write_er_svg_label_background(&mut out);
                write_er_svg_box_label(&mut out, &measure.label);
                out.push_str("</g></g>");
            }
            out.push_str("</g>");
            out.checkpoint()?;
            continue;
        }

        let label_div_color_prefix = label_div_style_prefix.as_str();
        let span_style_attr = visual.span_style_attr.as_str();

        // Mermaid erBox.ts uses the configured label mode and paths for the table rows.
        let row_extra_height = height_delta.max(0.0) / (measure.rows.len() as f64 + 1.0);
        let name_row_h = (measure.label_height + measure.text_padding).max(1.0) + row_extra_height;
        let box_x0 = ox;
        let box_y0 = oy;
        let box_x1 = ox + w;
        let box_y1 = oy + h;
        let sep_y = oy + name_row_h;
        let type_col_w = measure.type_col_w.max(0.0);
        let name_col_w = measure.name_col_w.max(0.0);
        let key_col_w = measure.key_col_w.max(0.0);
        let _comment_col_w = measure.comment_col_w.max(0.0);

        let box_fill = visual.box_fill.as_str();
        let box_stroke = visual.box_stroke.as_str();
        let stroke_width_attr = visual.stroke_width_attr.as_str();
        let group_style_attr = &visual.group_style_attr;
        let override_style_attr = visual.override_style_attr.as_str();
        let base_fill_style_attr = &visual.base_fill_style_attr;
        let divider_xs = [
            Some(ox + type_col_w),
            measure.has_key.then_some(ox + type_col_w + name_col_w),
            measure
                .has_comment
                .then_some(ox + type_col_w + name_col_w + key_col_w),
        ];

        fn thin_divider_rect_bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> (f64, f64, f64, f64) {
            let half = 0.00005;
            if (y1 - y0).abs() <= (x1 - x0).abs() {
                (x0, y0 - half, x1, y0 + half)
            } else {
                (x0 - half, y0, x0 + half, y1)
            }
        }

        if native.is_some() || outline.is_some() {
            let clip_id = format!("{diagram_id}_er-table-{entity_index}");
            let (left, top, width, height, radius) =
                (fmt(ox), fmt(oy), fmt(w), fmt(h), fmt(radius));
            let fill_bounds = outline
                .as_ref()
                .map_or(node_rect, |outline| node_rect.union(outline.bounds));
            let (fill_left, fill_top, fill_width) = (
                fmt(fill_bounds.x0),
                fmt(fill_bounds.y0),
                fmt(fill_bounds.width()),
            );
            let opacity_css = |property| {
                source_style
                    .rect_value(property)
                    .map_or_else(String::new, |value| {
                        format!(";{property}:{}", escape_attr(value))
                    })
            };
            let fill_opacity = opacity_css("fill-opacity");
            let table_class = if native.is_some() {
                "er-native-table"
            } else {
                "er-hand-drawn-table"
            };
            let _ = write!(&mut out, r#"<g class="{table_class}""#);
            if let Some(opacity) = source_style.rect_value("opacity") {
                let _ = write!(&mut out, r#" style="opacity:{}""#, escape_attr(opacity));
            }
            out.push_str(">");
            // Keep the header and row fills disjoint for alpha compositing.
            let _ = write!(&mut out, r#"<defs><clipPath id="{clip_id}">"#);
            if let Some(outline) = &outline {
                let _ = write!(&mut out, r#"<path d="{}"/>"#, escape_attr(&outline.d));
            } else {
                let _ = write!(
                    &mut out,
                    r#"<rect x="{left}" y="{top}" width="{width}" height="{height}" rx="{radius}"/>"#,
                );
            }
            let header_height = if outline.is_some() {
                sep_y - fill_bounds.y0
            } else {
                name_row_h
            };
            let _ = write!(
                &mut out,
                r##"</clipPath></defs><g class="outer-path" clip-path="url(#{clip_id})"><rect class="er-native-header" x="{fill_left}" y="{fill_top}" width="{fill_width}" height="{}" style="fill:{};stroke:none{fill_opacity}"/>"##,
                fmt(header_height),
                escape_attr(box_fill),
            );
            let mut y = sep_y;
            for (idx, (row, visual)) in measure.rows.iter().zip(&visual.rows).enumerate() {
                let row_height = row.height.max(1.0) + row_extra_height;
                let row_fill = if native.is_some() || visual.has_fill {
                    visual.fill.as_str()
                } else {
                    box_fill
                };
                let fill = if native.is_some()
                    || !idx.is_multiple_of(2)
                    || binding.source_fill_owns_odd_rows()
                {
                    source_style.fill().unwrap_or(row_fill)
                } else {
                    row_fill
                };
                let row_class = if idx.is_multiple_of(2) {
                    "row-rect-odd"
                } else {
                    "row-rect-even"
                };
                let paint_height = if outline.is_some() && idx + 1 == measure.rows.len() {
                    (fill_bounds.y1 - y).max(row_height)
                } else {
                    row_height
                };
                let _ = write!(
                    &mut out,
                    r#"<rect class="{row_class}" x="{fill_left}" y="{}" width="{fill_width}" height="{}" style="fill:{};stroke:none{fill_opacity}"/>"#,
                    fmt(y),
                    fmt(paint_height),
                    escape_attr(fill),
                );
                y += row_height;
                out.checkpoint()?;
            }
            let stroke_width = source_style.rect_value("stroke-width").map_or_else(
                || {
                    native.map_or_else(
                        || stroke_width_attr.to_owned(),
                        |style| fmt(style.border_width).to_string(),
                    )
                },
                str::to_owned,
            );
            let dash = source_style
                .rect_value("stroke-dasharray")
                .unwrap_or("none");
            let stroke_css = format!(
                "stroke-width:{};stroke-dasharray:{}{}",
                escape_attr(&stroke_width),
                escape_attr(dash),
                opacity_css("stroke-opacity"),
            );
            if let (Some(draw), Some(outline)) = (&hand_drawn, &outline) {
                let outline_class = if native.is_some() {
                    "er-native-outline"
                } else {
                    "er-hand-drawn-outline"
                };
                let _ = write!(
                    &mut out,
                    r##"</g><path class="{outline_class}" d="{}" style="fill:none;stroke:{};{stroke_css}"/><g class="divider" clip-path="url(#{clip_id})"><path d=""##,
                    escape_attr(&outline.d),
                    escape_attr(box_stroke),
                );
                draw.write_divider(
                    &mut out,
                    (fill_bounds.x0, sep_y).into(),
                    (fill_bounds.x1, sep_y).into(),
                )?;
                if native.is_none() {
                    let mut y = sep_y;
                    for row in measure
                        .rows
                        .iter()
                        .take(measure.rows.len().saturating_sub(1))
                    {
                        y += row.height.max(1.0) + row_extra_height;
                        draw.write_divider(
                            &mut out,
                            (fill_bounds.x0, y).into(),
                            (fill_bounds.x1, y).into(),
                        )?;
                    }
                }
                for x in divider_xs.into_iter().flatten() {
                    draw.write_divider(&mut out, (x, sep_y).into(), (x, fill_bounds.y1).into())?;
                }
            } else {
                let _ = write!(
                    &mut out,
                    r#"</g><rect class="er-native-outline" x="{left}" y="{top}" width="{width}" height="{height}" rx="{radius}" style="fill:none;stroke:{};{stroke_css}"/>"#,
                    escape_attr(box_stroke),
                );
                // Stroke the separators together so translucent strokes are composited once.
                let _ = write!(
                    &mut out,
                    r#"<g class="divider"><path d="M{} {}H{}"#,
                    fmt(box_x0),
                    fmt(sep_y),
                    fmt(box_x1),
                );
                for x in divider_xs.into_iter().flatten() {
                    let _ = write!(&mut out, " M{} {}V{}", fmt(x), fmt(sep_y), fmt(box_y1));
                }
            }
            // Neo's common path rules would override presentation attributes.
            let divider_stroke = source_style
                .stroke()
                .unwrap_or_else(|| native.map_or(box_stroke, |style| style.guide.as_str()));
            let _ = write!(
                &mut out,
                r#"" fill="none" style="stroke:{};{stroke_css}"/></g></g>"#,
                escape_attr(divider_stroke),
            );
            out.checkpoint()?;
        } else {
            // Base box (fill + border)
            let _ = write!(&mut out, r#"<g {} class="outer-path">"#, group_style_attr);
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"{} />"#,
                rough_border.fill_path_d(box_x0, box_y0, box_x1, box_y1),
                escape_attr(box_fill),
                base_fill_style_attr
            );
            let _ = write!(
                &mut out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                rough_border.path_d(hand_drawn_seed, box_x0, box_y0, box_x1, box_y1),
                escape_attr(box_stroke),
                stroke_width_attr,
                override_style_attr
            );
            out.push_str("</g>");
            out.checkpoint()?;
            // Row rectangles
            let escaped_node_border = escape_attr(node_border);
            let mut y = sep_y;
            for (idx, row) in measure.rows.iter().enumerate() {
                let row_h = row.height.max(1.0) + row_extra_height;
                let y0 = y;
                let y1 = y + row_h;
                y = y1;
                let is_odd = idx % 2 == 0;
                let row_class = if is_odd {
                    "row-rect-odd"
                } else {
                    "row-rect-even"
                };
                let row_visual = visual.rows.get(idx).ok_or_else(|| Error::InvalidModel {
                    message: format!("ER prepared visual is missing row {idx} for {}", entity.id),
                })?;
                let _ = write!(
                    &mut out,
                    r#"<g {} class="{}">"#,
                    group_style_attr, row_class
                );
                let row_override_style_attr = &row_visual.border_style_attr;
                let row_fill_style_attr = &row_visual.fill_style_attr;
                let has_row_fill = row_visual.has_fill;
                if has_row_fill {
                    let fill = row_visual.fill.as_str();
                    let _ = write!(
                        &mut out,
                        r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"{} />"#,
                        rough_border.fill_path_d(box_x0, y0, box_x1, y1),
                        escape_attr(fill),
                        row_fill_style_attr
                    );
                }
                let _ = write!(
                    &mut out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                    rough_border.path_d(hand_drawn_seed, box_x0, y0, box_x1, y1),
                    escaped_node_border,
                    stroke_width_attr,
                    row_override_style_attr
                );
                out.push_str("</g>");
                out.checkpoint()?;
            }
        }
        // ER entity tables read root `htmlLabels` directly. This intentionally differs from
        // relationship labels, whose renderer consults `flowchart.htmlLabels` as a fallback.
        // The HTML path keeps Mermaid's foreignObject structure; the SVG path below mirrors
        // createText(..., useHtmlLabels: false) with a background group and text/tspan output.
        let line_h = measure.label_height.max(1.0);
        let mut pad = entity_measurement.diagram_padding;
        // Keep parity with Mermaid's erBox.ts `if (!config.htmlLabels) { PADDING *= 1.25; }`:
        // when `htmlLabels` is unset (undefined), upstream still applies the 1.25 multiplier.
        if !entity_measurement.html_labels_raw {
            pad *= 1.25;
        }

        let name_w = measure.label_html_width.max(0.0);
        let name_x = -name_w / 2.0;
        let name_y = oy + name_row_h / 2.0 - line_h / 2.0;
        let entity_name_style_attr = er_text_style_attr(&source_label_style);
        if matches!(entity_wrap_mode, text::WrapMode::HtmlLike) {
            let _ = write!(
                &mut out,
                r#"<g class="label name" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"{}>"#,
                fmt(name_x),
                fmt(name_y),
                entity_name_style_attr,
                fmt(name_w),
                fmt(line_h),
                html_label_overflow_attr(allow_html_label_overflow, name_w, line_h),
            );
            measure.label.emit_html_div(
                &mut out,
                label_div_color_prefix,
                span_style_attr,
                false,
                "start",
            );
            out.push_str("</foreignObject></g>");
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="label name" transform="translate({}, {})" {}>"#,
                fmt(name_x),
                fmt(name_y),
                entity_name_style_attr,
            );
            write_er_svg_box_label(&mut out, &measure.label);
            out.push_str("</g>");
        }
        out.checkpoint()?;

        let left_text_x = ox + pad / 2.0;
        let type_left = left_text_x;
        let name_left = left_text_x + type_col_w;
        let key_left = left_text_x + type_col_w + name_col_w;
        let comment_left = left_text_x + type_col_w + name_col_w + key_col_w;

        let mut row_top = sep_y;
        for row in &measure.rows {
            let row_h = row.height.max(1.0) + row_extra_height;
            // erBox top-aligns independently measured cells within the shared row.
            let cell_y = row_top + (measure.text_padding + row_extra_height) / 2.0;

            let [type_w, name_w, keys_w, comment_w] = row.cell_widths;
            let [type_h, name_h, keys_h, comment_h] = row.cell_heights;

            if matches!(entity_wrap_mode, text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-type" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"{}>"#,
                    fmt(type_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                    fmt(type_w),
                    fmt(type_h),
                    html_label_overflow_attr(allow_html_label_overflow, type_w, type_h),
                );
                row.type_label.emit_html_div(
                    &mut out,
                    label_div_color_prefix,
                    span_style_attr,
                    false,
                    "start",
                );
                out.push_str("</foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-type" transform="translate({}, {})" {}>"#,
                    fmt(type_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                );
                write_er_svg_box_label(&mut out, &row.type_label);
                out.push_str("</g>");
            }

            if matches!(entity_wrap_mode, text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-name" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"{}>"#,
                    fmt(name_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                    fmt(name_w),
                    fmt(name_h),
                    html_label_overflow_attr(allow_html_label_overflow, name_w, name_h),
                );
                row.name_label.emit_html_div(
                    &mut out,
                    label_div_color_prefix,
                    span_style_attr,
                    false,
                    "start",
                );
                out.push_str("</foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-name" transform="translate({}, {})" {}>"#,
                    fmt(name_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                );
                write_er_svg_box_label(&mut out, &row.name_label);
                out.push_str("</g>");
            }

            if matches!(entity_wrap_mode, text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-keys" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"{}>"#,
                    fmt(key_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                    fmt(keys_w),
                    fmt(keys_h),
                    html_label_overflow_attr(allow_html_label_overflow, keys_w, keys_h),
                );
                row.key_label.emit_html_div(
                    &mut out,
                    label_div_color_prefix,
                    span_style_attr,
                    false,
                    "start",
                );
                out.push_str("</foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-keys" transform="translate({}, {})" {}>"#,
                    fmt(key_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                );
                write_er_svg_box_label(&mut out, &row.key_label);
                out.push_str("</g>");
            }

            if matches!(entity_wrap_mode, text::WrapMode::HtmlLike) {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-comment" transform="translate({}, {})" {}><foreignObject width="{}" height="{}"{}>"#,
                    fmt(comment_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                    fmt(comment_w),
                    fmt(comment_h),
                    html_label_overflow_attr(allow_html_label_overflow, comment_w, comment_h),
                );
                row.comment_label.emit_html_div(
                    &mut out,
                    label_div_color_prefix,
                    span_style_attr,
                    false,
                    "start",
                );
                out.push_str("</foreignObject></g>");
            } else {
                let _ = write!(
                    &mut out,
                    r#"<g class="label attribute-comment" transform="translate({}, {})" {}>"#,
                    fmt(comment_left),
                    fmt(cell_y),
                    entity_name_style_attr,
                );
                write_er_svg_box_label(&mut out, &row.comment_label);
                out.push_str("</g>");
            }

            out.checkpoint()?;

            row_top += row_h;
        }

        // Mermaid `erBox.ts` draws the header separator twice:
        // - once as the explicit "Name line"
        // - once via the later `yOffsets` pass (which always contains `0`)
        #[allow(clippy::too_many_arguments)]
        fn write_divider_group(
            out: &mut impl SvgOutput,
            rough_border: &mut RoughRectBorderPath,
            hand_drawn_seed: &roughr::core::RoughRandomness,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
            escaped_fill: &str,
            fill_style_attr: &str,
            divider_path_attrs: &str,
        ) -> Result<()> {
            let (rx0, ry0, rx1, ry1) = thin_divider_rect_bounds(x0, y0, x1, y1);
            out.push_str(r#"<g class="divider"><path d=""#);
            out.push_str(rough_border.fill_path_d(rx0, ry0, rx1, ry1));
            out.push_str(r#"" stroke="none" stroke-width="0" fill=""#);
            out.push_str(escaped_fill);
            out.push_str(r#"" fill-rule="evenodd""#);
            out.push_str(fill_style_attr);
            out.push_str(r#"/><path d=""#);
            out.push_str(rough_border.path_d(hand_drawn_seed, rx0, ry0, rx1, ry1));
            out.push('"');
            out.push_str(divider_path_attrs);
            out.push_str(" /></g>");
            out.checkpoint()
        }

        if native.is_none() && outline.is_none() {
            let escaped_box_fill = escape_attr(box_fill);
            let divider_path_attrs = format!(
                r#" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{}"#,
                escape_attr(box_stroke),
                stroke_width_attr,
                override_style_attr,
            );
            write_divider_group(
                &mut out,
                &mut rough_border,
                hand_drawn_seed,
                box_x0,
                sep_y,
                box_x1,
                sep_y,
                &escaped_box_fill,
                base_fill_style_attr,
                &divider_path_attrs,
            )?;
            for x in divider_xs.into_iter().flatten() {
                write_divider_group(
                    &mut out,
                    &mut rough_border,
                    hand_drawn_seed,
                    x,
                    sep_y,
                    x,
                    box_y1,
                    &escaped_box_fill,
                    base_fill_style_attr,
                    &divider_path_attrs,
                )?;
            }
            write_divider_group(
                &mut out,
                &mut rough_border,
                hand_drawn_seed,
                box_x0,
                sep_y,
                box_x1,
                sep_y,
                &escaped_box_fill,
                base_fill_style_attr,
                &divider_path_attrs,
            )?;
        }

        out.push_str("</g>");
        out.checkpoint()?;
    }
    #[cfg(feature = "layout-grid")]
    if is_grid_layout && options.include_edges {
        grid::write_labels(
            &mut out,
            nodes,
            edges,
            model,
            prepared_labels,
            diagram_id,
            subgraph_context,
        )?;
    }
    out.push_str("</g>\n");
    out.checkpoint()?;
    out.push_str("</g>\n</g>\n");
    out.checkpoint()?;

    push_er_gradient(&mut out, diagram_id.semantic_str(), binding)?;

    if let Some(title) = diagram_title {
        // Mermaid `utils.insertTitle(...)` appends the title after rendering the graph content.
        // - `text-anchor="middle"`
        // - `x = bounds.x + bounds.width / 2`
        // - `y = -titleTopMargin` (default: 25)
        let fallback_title_x = root_plan
            .view_box()
            .map(|view_box| view_box.min_x + view_box.width / 2.0)
            .unwrap_or(initial_bounds.width / 2.0);
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="erDiagramTitleText">{}"#,
            fmt(diagram_title_x.unwrap_or(fallback_title_x)),
            fmt(-insert_title_top_margin),
            escape_xml(title)
        );
        out.push_str("</text>\n");
        out.checkpoint()?;
    }

    push_er_shadow_defs(&mut out, diagram_id.semantic_str(), binding)?;

    let root_document = if defer_paint_bounds {
        root_viewport.finish_document(
            &mut out,
            root_document,
            root_spec(root_bounds(&content_bounds)),
        )?
    } else {
        root_document
    };
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

fn push_er_shadow_defs(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    binding: &ErCssBinding,
) -> Result<()> {
    let flood_color = binding.shadow_flood;
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id, flood_color, diagram_id, flood_color
    );
    out.checkpoint()
}

fn push_er_gradient(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    binding: &ErCssBinding,
) -> Result<()> {
    let Some((gradient_start, gradient_stop)) = &binding.gradient else {
        return Ok(());
    };

    let gradient_start = escape_attr(gradient_start);
    let gradient_stop = escape_attr(gradient_stop);
    let _ = write!(
        out,
        r#"<linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        diagram_id,
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
    out.checkpoint()
}

#[cfg(test)]
mod tests;
