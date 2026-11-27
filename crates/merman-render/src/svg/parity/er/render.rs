use super::super::*;
use crate::svg::parity::util::html_label_overflow_attr;
use merman_render_graph::er::{
    ErMarkerDefs, ErSvgGeometry, ErSvgLayoutView, ErSvgRouteKind, er_color_indices,
    er_redux_color_id, er_rel_idx_from_edge_id, er_unified_marker_id, write_er_marker_definitions,
    write_er_stylesheet,
};
#[cfg(feature = "layout-shoal")]
mod canonical;
mod swimlane;

// ER diagram SVG renderer implementation (split from parity.rs).

fn er_text_style_attr(source_style: &str) -> String {
    format!(r#"style="{source_style}""#)
}

// ELK and Swimlane normalize ER's basis curve to rounded after routing.
fn er_edge_path_d(
    points: &[crate::model::LayoutPoint],
    rounded_layout: bool,
    missing_section: bool,
    native: Option<&crate::er::ErNativeStyle>,
) -> String {
    if missing_section {
        return super::super::curve::curve_linear_path_d(points);
    }
    use merman_theme_contract::v2::EdgeStyle;
    match native.map(|style| style.edge_style) {
        Some(EdgeStyle::Orthogonal) => {
            return super::super::curve::curve_step_path_d_and_bounds(points).0;
        }
        Some(EdgeStyle::Straight | EdgeStyle::Sketch) => {
            return super::super::curve::curve_linear_path_d(points);
        }
        _ => {}
    }
    if rounded_layout && native.is_none() {
        return super::super::curve::curve_rounded_path_d(points, 5.0);
    }
    if let [a, b] = points {
        return curve_basis_path_d(&[
            a.clone(),
            crate::model::LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            },
            b.clone(),
        ]);
    }
    curve_basis_path_d(points)
}

fn write_er_svg_box_label(out: &mut impl SvgOutput, label: &crate::er::ErBoxLabel) {
    out.push_str(label.svg_markup());
}

impl crate::er::ErBoxLabel {
    pub(crate) fn prepare_svg_markup(
        &self,
        lines: &[Vec<(String, crate::text::MermaidMarkdownWordType)>],
        flattened_first_row: Option<&str>,
        style: &str,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<String> {
        let mut markup = String::new();
        super::super::label::write_svg_markdown_word_lines_with_style(
            &mut markup,
            lines,
            Some(style),
            crate::text::SvgTextEntityMode::CreateTextSource,
        );
        work_meter.charge(markup.len())?;
        if let Some(text) = flattened_first_row {
            // Apply erBox's one post-createText textContent mutation to the exact shared
            // serialization. Untouched rows retain the shared writer's word attributes.
            let range = {
                let document = crate::xml_dom::Document::parse(&markup).map_err(|error| {
                    Error::InvalidModel {
                        message: format!("invalid ER createText projection: {error}"),
                    }
                })?;
                document
                    .root_element()
                    .first_element_child()
                    .map(|row| row.range())
            };
            if let Some(range) = range {
                let row = &markup[range.clone()];
                if !row.ends_with("/>") {
                    let opening_end = range.start + row.find('>').expect("SVG row opening") + 1;
                    let closing_start = range.end - "</tspan>".len();
                    markup.replace_range(opening_end..closing_start, &escape_xml(text));
                }
            }
        }
        Ok(markup)
    }

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
    let lines = crate::graph_label::flowchart_non_markdown_svg_source_word_lines(text);
    crate::svg::parity::label::write_svg_text_source_word_lines(out, &lines, true, false);
}

fn write_er_svg_label_background(out: &mut impl SvgOutput) {
    out.push_str(r#"<g><rect class="background" style="stroke: none"/></g>"#);
}

#[derive(Clone, Copy)]
struct ErSubgraphRenderContext<'a> {
    diagram_id: SvgDiagramId<'a>,
    data_look: &'a str,
    uses_canonical_routes: bool,
    allow_html_label_overflow: bool,
    use_html_labels: bool,
    measurer: &'a dyn crate::text::TextMeasurer,
    label_style: &'a crate::text::TextStyle,
    translate_x: f64,
    translate_y: f64,
    theme: &'a crate::er::ErEntityThemePlan,
    swimlane_html_labels: bool,
    synthetic_color_ordinal: usize,
    work_meter: &'a crate::resources::OperationWorkMeter,
    rough_randomness: &'a roughr::core::RoughRandomness,
}

fn render_er_subgraph_cluster(
    out: &mut impl SvgOutput,
    cluster: &crate::model::LayoutCluster,
    subgraph: &crate::er::ErSubgraph,
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
        r#"<g id="{}-{}" class="{}" data-look="{}""#,
        escape_xml(&format!("{}", context.diagram_id)),
        escape_xml(&subgraph.id),
        escape_xml(&class_attr),
        escape_xml(context.data_look),
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
        let _ = write!(
            out,
            r#"<g class="cluster-label" transform="translate({}, {})"><foreignObject x="{}" y="{}" width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; text-align: center;"><span class="nodeLabel" {}>{}</span></div></foreignObject></g>"#,
            fmt(title_x),
            fmt(title_y),
            fmt(-title_width / 2.0),
            fmt(-title_height / 2.0),
            fmt(title_width),
            fmt(title_height),
            html_label_overflow_attr(context.allow_html_label_overflow, title_width, title_height),
            text_style_attr,
            prepared.fragment(),
        );
    } else {
        let _ = write!(
            out,
            r#"<g class="cluster-label" transform="translate({}, {})" {}>"#,
            fmt(title_x),
            fmt(title_y - title_height / 2.0),
            visual.svg_source_text_attr,
        );
        if subgraph.label_type == "string" || subgraph.label_type == "text" {
            crate::svg::parity::label::write_svg_text_centered_from_create_text_source_with_style(
                out,
                &subgraph.title,
                "",
            );
        } else {
            crate::svg::parity::label::write_svg_text_markdown_wrapped_centered_with_style(
                out,
                &subgraph.title,
                "",
                context.measurer,
                context.label_style,
                None,
            );
        }
        out.push_str("</g>");
    }
    out.push_str("</g>");
    out.push('\n');
    Ok(())
}

fn render_er_subgraph_clusters(
    out: &mut impl SvgOutput,
    clusters: &[crate::model::LayoutCluster],
    model: &merman_model::families::er::ErDiagramModel,
    swimlane: Option<&crate::er::ErSwimlanePaintLayout>,
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
    for cluster in clusters {
        let subgraph = subgraphs.get(cluster.id.as_str()).copied();
        if let (Some(lane), Some(paint)) = (lanes.get(cluster.id.as_str()), swimlane) {
            swimlane::render_cluster(out, cluster, lane, paint, subgraph, context)?;
            continue;
        }
        let Some(subgraph) = subgraph else {
            continue;
        };
        render_er_subgraph_cluster(out, cluster, subgraph, context)?;
    }
    Ok(())
}

fn er_edge_dom_id(edge_id: &str, relationships: &[crate::er::ErRelationship]) -> String {
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
    format!("id_{}_{}_{}", rel.entity_a, rel.entity_b, idx)
}

struct RoughRectBorderPath {
    path: String,
    fill_path: String,
    coordinates: [String; 4],
    ryu: ryu_js::Buffer,
}

impl Default for RoughRectBorderPath {
    fn default() -> Self {
        Self {
            path: String::new(),
            fill_path: String::new(),
            coordinates: Default::default(),
            ryu: ryu_js::Buffer::new(),
        }
    }
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

#[allow(
    clippy::too_many_arguments,
    reason = "Layout, route metadata, and the resolved terminal theme have distinct owners."
)]
pub(crate) fn render_er_diagram_svg_model(
    layout: &ErDiagramLayout,
    route_contract: &crate::graph_layout::RouteContract,
    model: &merman_model::families::er::ErDiagramModel,
    entity_theme: &crate::er::ErEntityThemePlan,
    effective_config: &merman_model::presentation::PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let effective_config = effective_config.as_value();
    let diagram_id = options.diagram_id_or("merman");
    // Mermaid's internal diagram type for ER is `er` (not `erDiagram`), and marker ids are derived
    // from this type (e.g. `<diagramId>_er-zeroOrMoreEnd`).
    let diagram_type = "er";
    let binding = entity_theme.css_binding();
    let native = binding.native.as_ref();
    let er_render_settings = &binding.render_settings;
    #[cfg(feature = "layout-shoal")]
    let canonical_routes = route_contract.canonical_routes();
    #[cfg(feature = "layout-shoal")]
    let uses_canonical_routes = canonical_routes.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let uses_canonical_routes = false;
    let is_elk_layout = route_contract.is_elk_parity();
    let is_swimlane_layout = matches!(route_contract, crate::graph_layout::RouteContract::Swimlane);
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
    let data_look = er_render_settings.diagram_look.as_str();
    let redux_border_colors = &binding.redux_border_colors;
    let color_indices = er_color_indices(model);
    let node_border = &binding.node_border;
    let main_bkg = &binding.main_bkg;
    let font_family = er_render_settings.font_family.clone();
    let font_size = er_render_settings.font_size;
    let title_top_margin = er_render_settings.title_top_margin;
    let use_max_width = er_render_settings.use_max_width;
    let label_style = er_render_settings.label_style.clone();
    let edge_html_labels = entity_theme.relationship_html_labels();
    let entity_wrap_mode = er_render_settings.entity_html_label_wrap_mode;
    let entity_measurement = er_render_settings.entity_measurement;
    let hand_drawn_seed =
        options.rough_randomness(er_render_settings.hand_drawn_seed, "render.er.roughjs");
    let insert_title_top_margin = er_render_settings.insert_title_top_margin;
    let geometry = ErSvgGeometry::prepare(
        ErSvgLayoutView {
            clusters: &layout.clusters,
            nodes: &layout.nodes,
            edges: &layout.edges,
            render_edges: &layout.render_edges,
        },
        route_kind,
        &model.relationships,
        effective_config,
        options.work_meter(),
    )?;
    let nodes = geometry.nodes();
    let edges = geometry.edges();

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
        let title_style = crate::text::TextStyle {
            font_family: native
                .and_then(|style| style.title_family.clone())
                .or_else(|| Some(font_family.clone())),
            font_size,
            font_weight: native.map(|style| style.title_weight.clone()),
            font_style: None,
        };
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let (title_ascent, title_descent) = crate::text::svg_text_vertical_extents_px(
            measurer,
            title,
            &title_style,
            crate::text::svg_title_bbox_vertical_extents_px(&title_style),
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
    let mut out = BoundedSvgOutput::new(options.work_meter());
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
    let root_viewport = options.root_viewport(crate::DiagramFamilyId::ER, diagram_id);
    let root_plan = root_viewport.plan(root_spec(initial_bounds))?;

    let has_acc_title = model.acc_title.as_ref().is_some_and(|s| !s.is_empty());
    let has_acc_descr = model.acc_descr.as_ref().is_some_and(|s| !s.is_empty());
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, diagram_type);
    root_chrome.class = Some("erDiagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    let root_document = if uses_canonical_routes {
        root_viewport.begin_document(
            &mut out,
            root_svg::DeferredRootSpec::mermaid(use_max_width),
            root_chrome,
        )?
    } else {
        root_viewport.write_plan(&mut out, &root_plan, root_chrome)?
    };
    options.checkpoint_emit()?;

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

    write_er_stylesheet(&mut out, diagram_id.semantic_str(), data_look, binding)?;

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");

    write_er_marker_definitions(
        &mut out,
        ErMarkerDefs {
            diagram_id,
            include_md_parent,
            canonical: uses_canonical_routes,
            neo: data_look == "neo",
            neo_stroke_width: binding.neo_marker_stroke_width,
            main_bkg,
        },
    )?;

    let mut entity_by_id: std::collections::HashMap<&str, &crate::er::ErEntity> =
        std::collections::HashMap::new();
    for e in model.entities.values() {
        entity_by_id.insert(e.id.as_str(), e);
    }
    let subgraph_context = ErSubgraphRenderContext {
        diagram_id,
        data_look,
        uses_canonical_routes,
        allow_html_label_overflow,
        use_html_labels: edge_html_labels,
        measurer,
        label_style: &label_style,
        translate_x,
        translate_y,
        theme: entity_theme,
        swimlane_html_labels: crate::er::ErConfigView::new(effective_config)
            .swimlane_title_html_labels(),
        synthetic_color_ordinal: nodes.iter().filter(|node| !node.is_cluster).count(),
        work_meter: options.work_meter(),
        rough_randomness: &hand_drawn_seed,
    };

    // Mermaid 12 keeps the cluster wrapper in the common painter for both layout providers.
    let _ = writeln!(&mut out, r#"<g class="root">"#);
    if layout.clusters.is_empty() {
        out.push_str(r#"<g class="clusters"/>"#);
    } else {
        out.push_str(r#"<g class="clusters">"#);
        render_er_subgraph_clusters(
            &mut out,
            &layout.clusters,
            model,
            layout.swimlane.as_ref(),
            subgraph_context,
        )?;
        out.push_str("</g>");
    }

    if is_elk_layout {
        out.push_str(r#"<g class="edges edgePaths">"#);
    } else {
        out.push_str(r#"<g class="edgePaths">"#);
    }
    let line_hops_enabled = rounded_layout
        && options.debug.include_edges
        && effective_config
            .pointer(if is_swimlane_layout {
                "/swimlane/lineHops"
            } else {
                "/elk/lineHops"
            })
            .and_then(serde_json::Value::as_bool)
            != Some(false);
    let shifted_hop_points: Vec<Vec<crate::model::LayoutPoint>> = if line_hops_enabled {
        options
            .work_meter()
            .charge(edges.iter().fold(0usize, |work, edge| {
                work.saturating_add(edge.points.len()).saturating_add(1)
            }))?;
        edges
            .iter()
            .map(|edge| {
                edge.points
                    .iter()
                    .map(|point| crate::model::LayoutPoint {
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
        options.work_meter().charge(edges.len())?;
        let hop_edges: Vec<_> = edges
            .iter()
            .zip(&shifted_hop_points)
            .map(|(edge, points)| super::super::line_hops::LineHopEdge {
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
            swimlane::line_hop_paths(effective_config, &hop_edges, options.work_meter())?
        } else {
            super::super::line_hops::elk_line_hop_paths(
                effective_config,
                &hop_edges,
                options.work_meter(),
            )?
        }
    } else {
        std::collections::HashMap::new()
    };
    out.checkpoint()?;
    if options.debug.include_edges {
        #[cfg(feature = "layout-shoal")]
        let mut paint_scratch = crate::graph_layout::canonical::CanonicalPaintScratch::default();
        let mut points_json = Vec::new();
        let mut points_base64 = String::new();
        for e in edges {
            let missing_section = geometry.is_missing_section(&e.id);
            if !uses_canonical_routes
                && (e.points.is_empty() || (!missing_section && e.points.len() < 2))
            {
                continue;
            }
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let edge_svg_id = format!("{diagram_id}-{edge_dom_id}");
            options.checkpoint_emit()?;
            let is_dashed = e.stroke_dasharray.as_deref() == Some("8,8");
            let pattern_class = if is_dashed {
                "edge-pattern-dashed"
            } else {
                "edge-pattern-solid"
            };
            let line_classes = format!("edge-thickness-normal {pattern_class} relationshipLine");
            let legacy_points;
            let (original_d, shifted) = 'path: {
                #[cfg(feature = "layout-shoal")]
                if let Some(routes) = canonical_routes {
                    let painted = canonical::paint_edge(
                        routes,
                        e,
                        kurbo::Vec2::new(translate_x, translate_y),
                        binding,
                        options.work_meter(),
                        &hand_drawn_seed,
                        &mut paint_scratch,
                    )?;
                    content_bounds.min_x = content_bounds.min_x.min(painted.bounds.x0);
                    content_bounds.min_y = content_bounds.min_y.min(painted.bounds.y0);
                    content_bounds.max_x = content_bounds.max_x.max(painted.bounds.x1);
                    content_bounds.max_y = content_bounds.max_y.max(painted.bounds.y1);
                    break 'path (painted.d, painted.points);
                }
                legacy_points = e
                    .points
                    .iter()
                    .map(|p| crate::model::LayoutPoint {
                        x: p.x + translate_x,
                        y: p.y + translate_y,
                    })
                    .collect::<Vec<_>>();
                let d = er_edge_path_d(&legacy_points, rounded_layout, missing_section, native);
                break 'path (d, legacy_points.as_slice());
            };
            points_json.clear();
            points_json.reserve(128);
            if serde_json::to_writer(&mut points_json, shifted).is_err() {
                points_json.clear();
            }
            points_base64.clear();
            base64::engine::general_purpose::STANDARD
                .encode_string(&points_json, &mut points_base64);
            let hopped_d = hop_paths.get(e.id.as_str()).map(String::as_str);
            let d = hopped_d.unwrap_or(&original_d);

            let mut edge_style = String::new();
            if data_look == "neo"
                && let Some(length) = super::super::svg_path_length_from_d(&original_d)
            {
                super::super::edge_path::write_neo_edge_mask(
                    &mut edge_style,
                    length,
                    None,
                    None,
                    is_dashed,
                    false,
                );
            }
            edge_style.push_str(if is_elk_layout {
                "fill:none;;;fill:none"
            } else {
                "undefined;;;undefined"
            });
            let edge_style = if let Some(hopped_d) = hopped_d {
                super::super::line_hops::rewrite_style_after_line_hop(
                    &edge_style,
                    hopped_d,
                    options.work_meter(),
                )?
            } else {
                std::borrow::Cow::Borrowed(edge_style.as_str())
            };

            // The standard Base64 alphabet is safe in a quoted XML attribute.
            let _ = write!(
                &mut out,
                r#"<path d="{}" id="{}" class="{}" style="{}" data-edge="true" data-et="edge" data-id="{}" data-points="{}" data-look="{}""#,
                escape_xml(d),
                escape_xml(&edge_svg_id),
                escape_xml(&line_classes),
                escape_attr(&edge_style),
                escape_xml(&edge_dom_id),
                points_base64,
                escape_xml(data_look)
            );
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
            options.checkpoint_emit()?;
            out.push_str(" />");
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if is_swimlane_layout {
        out.push_str(r#"<g class="edgeLabels"/><g class="nodes">"#);
    } else {
        out.push_str(r#"<g class="edgeLabels">"#);
    }
    out.checkpoint()?;
    if options.debug.include_edges {
        for e in edges {
            let rel_idx = er_rel_idx_from_edge_id(&e.id)
                .and_then(|idx| model.relationships.get(idx).map(|r| (idx, r)));

            let rel_text_raw = rel_idx.map(|(_, r)| r.role_a.as_str()).unwrap_or("");
            let edge_dom_id = er_edge_dom_id(&e.id, &model.relationships);
            let relationship_index = rel_idx.map(|(index, _)| index);
            let prepared_relationship =
                relationship_index.and_then(|index| layout.prepared_labels.relationship(index));
            // The common ELK/Swimlane painter checks hasEdgeLabel before preparing
            // a terminal; Dagre also emits its zero-size wrapper for an empty role.
            if rel_text_raw.is_empty() && (is_elk_layout || is_swimlane_layout) {
                continue;
            }
            if is_swimlane_layout {
                if let Some((index, _)) = rel_idx {
                    swimlane::render_relationship_label(
                        &mut out,
                        e,
                        index,
                        model,
                        layout,
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
                        .and_then(crate::er::ErPreparedRelationshipLabel::svg)
                        .ok_or_else(|| crate::Error::InvalidModel {
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
                let (x, y) = super::super::edge_label_geometry::calc_label_position(&e.points)
                    .map(|point| (point.x + translate_x, point.y + translate_y))
                    .unwrap_or((0.0, 0.0));
                (0.0, 0.0, x, y)
            };

            if !uses_canonical_routes && w > 0.0 && h > 0.0 && !geometry.is_missing_section(&e.id) {
                // Mermaid 12.1 preserves the layout anchor and adds only the midpoint delta
                // from paint-time clipping. ER has no additional clipping here: curve and marker
                // projection change `d`, while both label polylines remain the layout route.
                let shifted: Vec<crate::model::LayoutPoint> = e
                    .points
                    .iter()
                    .map(|p| crate::model::LayoutPoint {
                        x: p.x + translate_x,
                        y: p.y + translate_y,
                    })
                    .collect();
                let rendered_d = er_edge_path_d(&shifted, rounded_layout, false, native);
                let position = super::super::edge_label_geometry::position_edge_label(
                    crate::model::LayoutPoint { x: cx, y: cy },
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
                    escape_xml_display(&edge_dom_id),
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
                        .is_some_and(crate::er::ErPreparedRelationshipLabel::html_is_constrained);
                    let max_width = crate::text::MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX;
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
                            .map(crate::er::ErPreparedRelationshipLabel::xhtml_fragment)
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
                    crate::svg::parity::label::write_svg_markdown_word_lines_centered_with_style(
                        &mut out,
                        &svg_label.word_lines,
                        Some(""),
                        crate::text::SvgTextEntityMode::CreateTextSource,
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
                        escape_xml_display(&edge_dom_id)
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
                        escape_xml_display(&edge_dom_id)
                    );
                    out.push_str(r#" transform="translate(0, 0)">"#);
                    // createText gates addSvgBackground on the raw source's truthiness.
                    // An empty role returns the text terminal directly, with one empty row.
                    crate::svg::parity::label::write_svg_text_centered_from_create_text_source(
                        &mut out, "", false,
                    );
                    out.push_str("</g></g>");
                }
            }
            out.checkpoint()?;
        }
    }
    if !is_swimlane_layout {
        out.push_str("</g>\n");
    }
    out.checkpoint()?;

    // Entities drawn after relationships so they cover markers when overlapping.
    if !is_swimlane_layout {
        out.push_str(r#"<g class="nodes">"#);
    }
    out.checkpoint()?;
    let mut rough_border = RoughRectBorderPath::default();
    for n in nodes {
        if n.is_cluster {
            continue;
        }
        let Some(entity) = entity_by_id.get(n.id.as_str()).copied() else {
            if n.id.contains("---") {
                let cx = n.x + translate_x;
                let cy = n.y + translate_y;
                let _ = write!(
                    &mut out,
                    r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})">"#,
                    escape_xml(&n.id),
                    fmt(cx),
                    fmt(cy)
                );
                out.push_str(r#"<rect width="0.1" height="0.1"/>"#);
                if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
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
            source_style.label_style(matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike));
        let label_div_style_prefix = format!(
            "{}{}",
            visual.label_div_color_prefix, visual.label_div_typography_prefix,
        );
        let measure =
            layout
                .prepared_labels
                .entity(&entity.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("ER prepared labels are missing entity {}", entity.id),
                })?;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        if (measure.width - w).abs() > 1e-3 || (measure.height - h).abs() > 1e-3 {
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

        let group_class = if entity.css_classes.trim().is_empty() {
            "node".to_string()
        } else {
            format!("node {}", entity.css_classes.trim())
        };
        let color_id_attr = color_indices
            .get(entity.id.as_str())
            .and_then(|index| er_redux_color_id(redux_border_colors, *index))
            .map(|color_id| format!(r#" data-color-id="{}""#, escape_xml(&color_id)))
            .unwrap_or_default();
        let _ = write!(
            &mut out,
            r#"<g id="{}-{}" class="{}" data-look="{}"{}"#,
            diagram_id,
            escape_xml(&entity.id),
            escape_xml(&group_class),
            escape_xml(data_look),
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
            {
                let rect_style_attr = &visual.rect_style_attr;
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
                out.checkpoint()?;
            }
            let wrap_mode = entity_wrap_mode;
            let label_metrics = measure.unformatted_label_metrics;
            let lw = if wrap_mode == crate::text::WrapMode::HtmlLike {
                measure.label_html_width.max(0.0)
            } else {
                label_metrics.width.max(0.0)
            };
            let lh = label_metrics.height.max(0.0);

            if matches!(wrap_mode, crate::text::WrapMode::HtmlLike) {
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
        let name_row_h = (measure.label_height + measure.text_padding).max(1.0);
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

        if let Some(native) = native {
            let clip_id = format!("{diagram_id}-er-table-{entity_index}");
            let (left, top, width, height, radius) =
                (fmt(ox), fmt(oy), fmt(w), fmt(h), fmt(native.radius));
            let opacity_css = |property| {
                source_style
                    .rect_value(property)
                    .map_or_else(String::new, |value| {
                        format!(";{property}:{}", escape_attr(value))
                    })
            };
            let fill_opacity = opacity_css("fill-opacity");
            out.push_str(r#"<g class="er-native-table""#);
            if let Some(opacity) = source_style.rect_value("opacity") {
                let _ = write!(&mut out, r#" style="opacity:{}""#, escape_attr(opacity));
            }
            out.push_str(">");
            // Keep the header and row fills disjoint for alpha compositing.
            let _ = write!(
                &mut out,
                r##"<defs><clipPath id="{clip_id}"><rect x="{left}" y="{top}" width="{width}" height="{height}" rx="{radius}"/></clipPath></defs><g class="outer-path" clip-path="url(#{clip_id})"><rect class="er-native-header" x="{left}" y="{top}" width="{width}" height="{}" style="fill:{};stroke:none{fill_opacity}"/>"##,
                fmt(name_row_h),
                escape_attr(box_fill),
            );
            let mut y = sep_y;
            for (idx, (row, visual)) in measure.rows.iter().zip(&visual.rows).enumerate() {
                let row_height = row.height.max(1.0);
                let fill = source_style.fill().unwrap_or(&visual.fill);
                let row_class = if idx.is_multiple_of(2) {
                    "row-rect-odd"
                } else {
                    "row-rect-even"
                };
                let _ = write!(
                    &mut out,
                    r#"<rect class="{row_class}" x="{left}" y="{}" width="{width}" height="{}" style="fill:{};stroke:none{fill_opacity}"/>"#,
                    fmt(y),
                    fmt(row_height),
                    escape_attr(fill),
                );
                y += row_height;
                out.checkpoint()?;
            }
            let stroke_width = source_style
                .rect_value("stroke-width")
                .map_or_else(|| fmt(native.border_width).to_string(), str::to_owned);
            let dash = source_style
                .rect_value("stroke-dasharray")
                .unwrap_or("none");
            let stroke_css = format!(
                "stroke-width:{};stroke-dasharray:{}{}",
                escape_attr(&stroke_width),
                escape_attr(dash),
                opacity_css("stroke-opacity"),
            );
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
            // Neo's common path rules would override presentation attributes.
            let divider_stroke = source_style.stroke().unwrap_or(&native.guide);
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
                rough_border.path_d(&hand_drawn_seed, box_x0, box_y0, box_x1, box_y1),
                escape_attr(box_stroke),
                stroke_width_attr,
                override_style_attr
            );
            out.push_str("</g>");
            out.checkpoint()?;
            // Row rectangles
            let escaped_node_border = escape_xml(node_border);
            let mut y = sep_y;
            for (idx, row) in measure.rows.iter().enumerate() {
                let row_h = row.height.max(1.0);
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
                        escape_xml(fill),
                        row_fill_style_attr
                    );
                }
                let _ = write!(
                    &mut out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{} />"#,
                    rough_border.path_d(&hand_drawn_seed, box_x0, y0, box_x1, y1),
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
        if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
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
            let row_h = row.height.max(1.0);
            // erBox top-aligns independently measured cells within the shared row.
            let cell_y = row_top + measure.text_padding / 2.0;

            let [type_w, name_w, keys_w, comment_w] = row.cell_widths;
            let [type_h, name_h, keys_h, comment_h] = row.cell_heights;

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
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

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
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

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
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

            if matches!(entity_wrap_mode, crate::text::WrapMode::HtmlLike) {
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

        if native.is_none() {
            let escaped_box_fill = escape_xml(box_fill);
            let divider_path_attrs = format!(
                r#" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{}"#,
                escape_xml(box_stroke),
                stroke_width_attr,
                override_style_attr,
            );
            write_divider_group(
                &mut out,
                &mut rough_border,
                &hand_drawn_seed,
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
                    &hand_drawn_seed,
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
                &hand_drawn_seed,
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

    let root_document = if uses_canonical_routes {
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
    binding: &crate::er::ErCssBinding,
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
    binding: &crate::er::ErCssBinding,
) -> Result<()> {
    let Some((gradient_start, gradient_stop)) = &binding.gradient else {
        return Ok(());
    };

    let gradient_start = escape_xml(gradient_start);
    let gradient_stop = escape_xml(gradient_stop);
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
mod tests {
    use crate::DiagramFamilyId;
    use crate::model::{Bounds, ErDiagramLayout, LayoutNode};
    use crate::svg::{SvgRenderOptions, with_test_svg_execution};
    use indexmap::IndexMap;
    use merman_model::families::er::{ErDiagramModel, ErEntity};
    use serde_json::json;

    #[test]
    fn er_unstyled_unwrapped_svg_labels_preserve_terminal_bytes() {
        let config = json!({ "htmlLabels": false, "flowchart": { "wrappingWidth": 1000 } });
        let settings = crate::er::ErConfigView::new(&config)
            .render_settings_with_resolved_typography(None, None);
        let measurer = crate::text::DeterministicTextMeasurer::default();
        for source in [
            "Account",
            "**Account** *owner*",
            "A&amp;lt;br&amp;gt;B",
            "First<br/>Last",
        ] {
            let entity = ErEntity {
                id: "E".to_owned(),
                label: source.to_owned(),
                ..Default::default()
            };
            let measured = crate::er::measure_entity_box(
                &entity,
                &measurer,
                &settings.label_style,
                &settings.label_style,
                settings.entity_measurement,
            );
            let mut actual = String::new();
            super::write_er_svg_box_label(&mut actual, &measured.label);
            let mut previous = String::new();
            crate::svg::parity::label::write_svg_text_markdown_from_create_text_source(
                &mut previous,
                measured.label.svg_text_source(),
                true,
            );
            assert_eq!(actual, previous, "unchanged unwrapped label: {source}");
        }
    }

    #[test]
    fn er_redux_colors_follow_entity_insertion_ids() {
        let config = json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": ["#e879f9", "#2dd4bf"],
                "bkgColorArray": ["#fdf4ff", "#f0fdfa"],
                "THEME_COLOR_LIMIT": 2
            }
        });
        let zeta = ErEntity {
            id: "entity-ZETA-0".to_string(),
            label: "ZETA".to_string(),
            shape: "erBox".to_string(),
            css_classes: "default".to_string(),
            ..ErEntity::default()
        };
        let alpha = ErEntity {
            id: "entity-ALPHA-1".to_string(),
            label: "ALPHA".to_string(),
            shape: "erBox".to_string(),
            css_classes: "default".to_string(),
            ..ErEntity::default()
        };
        let model = ErDiagramModel {
            direction: "TB".to_string(),
            entities: IndexMap::from([
                ("ALPHA".to_string(), alpha.clone()),
                ("ZETA".to_string(), zeta.clone()),
            ]),
            ..ErDiagramModel::default()
        };
        let color_indices = super::er_color_indices(&model);
        assert_eq!(color_indices.get(&zeta.id), Some(&0));
        assert_eq!(color_indices.get(&alpha.id), Some(&1));
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let settings = crate::er::ErConfigView::new(&config)
            .render_settings_with_resolved_typography(None, None);
        let measure = |entity: &ErEntity| {
            crate::er::measure_entity_box(
                entity,
                &measurer,
                &settings.label_style,
                &settings.label_style,
                settings.entity_measurement,
            )
        };
        let zeta_measure = measure(&zeta);
        let alpha_measure = measure(&alpha);
        let layout = ErDiagramLayout {
            // Deliberately reverse the layout vector. Mermaid colorIndex belongs to the entity,
            // not to whichever order the layout backend returns nodes in.
            nodes: vec![
                LayoutNode {
                    id: alpha.id.clone(),
                    x: 220.0,
                    y: 80.0,
                    width: alpha_measure.width,
                    height: alpha_measure.height,
                    is_cluster: false,
                    label_width: None,
                    label_height: None,
                },
                LayoutNode {
                    id: zeta.id.clone(),
                    x: 80.0,
                    y: 80.0,
                    width: zeta_measure.width,
                    height: zeta_measure.height,
                    is_cluster: false,
                    label_width: None,
                    label_height: None,
                },
            ],
            edges: Vec::new(),
            render_edges: Vec::new(),
            swimlane: None,
            clusters: Vec::new(),
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 300.0,
                max_y: 160.0,
            }),
            prepared_labels: crate::er::prepare_er_labels(&model, &config, &measurer, false),
        };
        let options = SvgRenderOptions {
            diagram_id: Some("er-colors".to_string()),
            ..SvgRenderOptions::default()
        };
        let effective_config = crate::test_support::presentation_config(config.clone());
        let relationship_html_labels = crate::er::ErConfigView::new(effective_config.as_value())
            .relationship_html_labels(false);
        let svg = with_test_svg_execution(DiagramFamilyId::ER, &options, |options| {
            let entity_theme = crate::er::ErEntityThemePlan::resolve(
                &effective_config,
                None,
                relationship_html_labels,
                &model,
                &layout,
                options.work_meter(),
            )
            .expect("resolve baseline ER entity theme");
            super::render_er_diagram_svg_model(
                &layout,
                &crate::graph_layout::RouteContract::DagreParity,
                &model,
                &entity_theme,
                &effective_config,
                None,
                &measurer,
                options,
            )
        })
        .and_then(|svg| svg.into_string_for(DiagramFamilyId::ER))
        .unwrap();

        let zeta_dom = r#"<g id="er-colors-entity-ZETA-0" class="node default" data-look="classic" data-color-id="color-0""#;
        let alpha_dom = r#"<g id="er-colors-entity-ALPHA-1" class="node default" data-look="classic" data-color-id="color-1""#;
        assert!(svg.contains(zeta_dom), "{svg}");
        assert!(svg.contains(alpha_dom), "{svg}");
        assert!(svg.find(zeta_dom).unwrap() < svg.find(alpha_dom).unwrap());
    }
}
