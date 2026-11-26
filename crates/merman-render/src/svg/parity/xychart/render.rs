use super::super::*;
use crate::model::{XyChartDrawableElem, XyChartRectData};
use crate::theme::{FamilyTheme, elevation_svg, pattern_svg::data_pattern};
use kurbo::{BezPath, PathEl, Rect, RoundedRect, RoundedRectRadii, Shape};
use merman_model::families::xy_chart::{
    XyChartAxisModel, XyChartDiagramRenderModel, XyChartPlotType,
};
use merman_theme_contract::v2::Elevation;

// XYChart diagram SVG renderer implementation (split from parity.rs).

fn write_xychart_temporary_group(out: &mut impl SvgOutput) -> Result<()> {
    out.push_str(r#"<g class="mermaid-tmp-group"/>"#);
    out.checkpoint()
}

fn plot_group_index(group_texts: &[String], prefix: &str) -> Option<usize> {
    if group_texts.first().map(String::as_str) != Some("plot") {
        return None;
    }
    group_texts
        .get(1)
        .and_then(|group| group.strip_prefix(prefix))
        .and_then(|index| index.parse().ok())
}

fn bar_surfaces(
    layout: &XyChartDiagramLayout,
) -> impl Iterator<Item = (usize, usize, &XyChartRectData)> {
    layout
        .drawables
        .iter()
        .filter_map(|drawable| match drawable {
            XyChartDrawableElem::Rect { group_texts, data } => {
                Some((plot_group_index(group_texts, "bar-plot-")?, data))
            }
            _ => None,
        })
        .flat_map(|(plot, bars)| {
            bars.iter()
                .enumerate()
                .map(move |(index, bar)| (plot, index, bar))
        })
        .filter(|(_, _, bar)| {
            [bar.x, bar.y, bar.width, bar.height]
                .into_iter()
                .all(f64::is_finite)
                && bar.width > 0.0
                && bar.height > 0.0
        })
}

fn bar_outline(
    bounds: Rect,
    model: &XyChartDiagramRenderModel,
    plot: usize,
    index: usize,
    radius: f64,
) -> BezPath {
    let value = model.plots.get(plot).and_then(|plot| plot.data.get(index));
    let toward_top_or_right = match (&model.y_axis, value.and_then(|(_, value)| *value)) {
        (XyChartAxisModel::Linear { min, max, .. }, Some(value)) => {
            let (start, end) = (min.unwrap_or(0.0), max.unwrap_or(1.0));
            // Use zero when the value axis contains it, or the nearest domain endpoint.
            let baseline = 0.0_f64.max(start.min(end)).min(start.max(end));
            (value >= baseline) != (start > end)
        }
        _ => true,
    };
    let radii = match (model.orientation == "horizontal", toward_top_or_right) {
        (true, true) => RoundedRectRadii::new(0.0, radius, radius, 0.0),
        (true, false) => RoundedRectRadii::new(radius, 0.0, 0.0, radius),
        (false, true) => RoundedRectRadii::new(radius, radius, 0.0, 0.0),
        (false, false) => RoundedRectRadii::new(0.0, 0.0, radius, radius),
    };
    RoundedRect::from_rect(bounds, radii)
        .path_elements(0.1)
        .filter(|element| !matches!(element, PathEl::CurveTo(a, b, c) if a == b && b == c))
        .collect()
}

fn write_native_chart_defs(
    out: &mut BoundedSvgOutput<'_>,
    diagram_id: SvgDiagramId<'_>,
    theme: &FamilyTheme,
    model: &XyChartDiagramRenderModel,
    layout: &XyChartDiagramLayout,
) -> Result<()> {
    if theme.data.pattern_encoding {
        out.push_str("<defs>");
        for (ordinal, plot) in model.plots.iter().enumerate() {
            if plot.plot_type == XyChartPlotType::Bar {
                let _ = data_pattern(
                    out,
                    &format!("{diagram_id}-xy-series-{ordinal}"),
                    ordinal,
                    theme.data.fill(ordinal),
                    theme.data.fill_ink(ordinal),
                );
                out.checkpoint()?;
            }
        }
        out.push_str("</defs>");
    }

    if theme.elevation != Elevation::Flat {
        let radius = theme.form.chart.bar_radius;
        for (ordinal, (plot, index, bar)) in bar_surfaces(layout).enumerate() {
            let bounds = Rect::new(0.0, 0.0, bar.width, bar.height);
            let silhouette = bar_outline(bounds, model, plot, index, radius);
            let id = format!("{diagram_id}-xy-bar-{ordinal}");
            let _ =
                elevation_svg::write_depth(out, &theme.elevation, &id, &silhouette, (bar.x, bar.y));
            out.checkpoint()?;
        }
    }
    out.checkpoint()
}

struct Node {
    tag: &'static str,
    attrs: Vec<(&'static str, String)>,
    text: Option<String>,
    children: Vec<usize>,
}

impl Node {
    fn attr(&mut self, name: &'static str, value: impl Into<String>) {
        self.attrs.push((name, value.into()));
    }
}

fn node(tag: &'static str) -> Node {
    Node {
        tag,
        attrs: Vec::with_capacity(6),
        text: None,
        children: Vec::with_capacity(2),
    }
}

fn line_marker_center(path: &str) -> Option<(f64, f64)> {
    use svgtypes::PathSegment;

    // The layout stores markers as two half-circle arcs. The first chord bisects the circle.
    let mut segments = svgtypes::PathParser::from(path);
    let PathSegment::MoveTo { abs: true, x, y } = segments.next()?.ok()? else {
        return None;
    };
    let PathSegment::EllipticalArc {
        abs: false,
        x: diameter,
        ..
    } = segments.next()?.ok()?
    else {
        return None;
    };
    Some((x + diameter / 2.0, y))
}

fn push_child(arena: &mut Vec<Node>, parent: usize, child: Node) -> usize {
    let id = arena.len();
    arena.push(child);
    arena[parent].children.push(id);
    id
}

fn render_node(out: &mut impl SvgOutput, arena: &[Node], id: usize) -> Result<()> {
    let n = &arena[id];
    let _ = write!(out, "<{}", n.tag);
    out.checkpoint()?;
    for (k, v) in &n.attrs {
        let _ = write!(out, r#" {k}="{v}""#);
        out.checkpoint()?;
    }
    if n.children.is_empty() && n.text.as_deref().unwrap_or("").is_empty() {
        out.push_str("/>");
        out.checkpoint()?;
    } else {
        out.push('>');
        out.checkpoint()?;
        if let Some(t) = n.text.as_deref() {
            out.push_str(t);
            out.checkpoint()?;
        }
        for c in &n.children {
            render_node(out, arena, *c)?;
        }
        let _ = write!(out, "</{}>", n.tag);
        out.checkpoint()?;
    }
    Ok(())
}

pub(crate) fn render_xychart_diagram_svg(
    artifact: &crate::family::XyChartFamilyArtifact,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let layout = artifact.pair().layout();
    let model = artifact.pair().semantic();
    let typography_theme = artifact.typography_theme();
    let native = options.native_family_theme();
    use rustc_hash::FxHashMap;
    use std::collections::hash_map::Entry;

    fn text_anchor(horizontal_pos: &str) -> &'static str {
        match horizontal_pos {
            "left" => "start",
            "right" => "end",
            _ => "middle",
        }
    }

    fn ensure_group_path<'a>(
        arena: &mut Vec<Node>,
        groups_by_path: &mut FxHashMap<(usize, &'a str), usize>,
        group_texts: &'a [String],
    ) -> usize {
        let mut parent = 0usize;
        for seg in group_texts {
            let class = seg.as_str();
            let gid = match groups_by_path.entry((parent, class)) {
                Entry::Occupied(entry) => *entry.get(),
                Entry::Vacant(entry) => {
                    let mut g = node("g");
                    g.attr("class", class);
                    let id = push_child(arena, parent, g);
                    entry.insert(id);
                    id
                }
            };
            parent = gid;
        }
        parent
    }

    fn dominant_baseline(vertical_pos: &str) -> &'static str {
        if vertical_pos == "top" {
            "text-before-edge"
        } else {
            "middle"
        }
    }

    fn fmt_xy(v: f64) -> String {
        if v.is_nan() {
            return "NaN".to_string();
        }
        if !v.is_finite() {
            return "NaN".to_string();
        }
        fmt_string(v)
    }

    let diagram_id = options.diagram_id_or("xychart");
    let acc_title = model.acc_title.as_deref().filter(|title| !title.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .filter(|description| !description.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let data_label_config = if layout.show_data_label {
        Some((
            layout.show_data_label_outside_bar,
            artifact.data_label_color(),
        ))
    } else {
        None
    };
    // Build the `.main` group as an ordered DOM tree, matching Mermaid's D3 `getGroup()` behavior.
    let mut arena: Vec<Node> = Vec::with_capacity(layout.drawables.len().saturating_mul(4) + 2);
    arena.push(node("g"));
    arena[0].attr("class", "main");

    // Background rectangle.
    let mut bg = node("rect");
    bg.attr("width", fmt_xy(layout.width));
    bg.attr("height", fmt_xy(layout.height));
    bg.attr("class", "background");
    bg.attr("fill", escape_xml(&layout.background_color));
    push_child(&mut arena, 0, bg);

    let mut groups_by_path: FxHashMap<(usize, &str), usize> = FxHashMap::with_capacity_and_hasher(
        layout.drawables.len().saturating_mul(2) + 4,
        Default::default(),
    );

    for shape in &layout.drawables {
        match shape {
            XyChartDrawableElem::Rect { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);
                let bar_plot_index = plot_group_index(group_texts, "bar-plot-");

                for (mark_index, r) in data.iter().enumerate() {
                    let outline = native.zip(bar_plot_index).and_then(|(theme, plot)| {
                        let radius = theme.form.chart.bar_radius;
                        let bounds = Rect::new(r.x, r.y, r.x + r.width, r.y + r.height);
                        (radius > 0.0
                            && bounds.is_finite()
                            && bounds.width() > 0.0
                            && bounds.height() > 0.0)
                            .then(|| bar_outline(bounds, model, plot, mark_index, radius))
                    });
                    let mut n = if let Some(outline) = outline {
                        let mut shape = node("path");
                        shape.attr("d", outline.to_svg());
                        shape
                    } else {
                        let mut shape = node("rect");
                        shape.attr("x", fmt_xy(r.x));
                        if !r.y.is_nan() {
                            shape.attr("y", fmt_xy(r.y));
                        }
                        shape.attr("width", fmt_xy(r.width));
                        shape.attr("height", fmt_xy(r.height));
                        shape
                    };
                    let series = bar_plot_index.or_else(|| {
                        if group_texts.as_slice() == ["legend", "markers"] {
                            artifact.bar_legend_plots().get(mark_index).copied()
                        } else {
                            None
                        }
                    });
                    let pattern =
                        series.filter(|_| native.is_some_and(|theme| theme.data.pattern_encoding));
                    n.attr(
                        "fill",
                        match pattern {
                            Some(plot) => {
                                escape_xml(&format!("url(#{diagram_id}-xy-series-{plot})"))
                            }
                            None => escape_xml(&r.fill),
                        },
                    );
                    n.attr("stroke", escape_xml(&r.stroke_fill));
                    n.attr("stroke-width", fmt_xy(r.stroke_width));
                    if let Some(theme) = native.filter(|_| series.is_some()) {
                        if bar_plot_index.is_none() {
                            let radius = theme
                                .form
                                .chart
                                .bar_radius
                                .min(r.width / 2.0)
                                .min(r.height / 2.0);
                            if radius > 0.0 {
                                n.attr("rx", fmt_xy(radius));
                            }
                        }
                        if theme.form.chart.bar_wireframe {
                            n.attr("fill-opacity", "0.12");
                        }
                    }
                    push_child(&mut arena, parent, n);
                }

                // Mermaid emits data labels for every rectangle group, including legend markers.
                if let Some((show_data_label_outside_bar, data_label_color)) = &data_label_config {
                    let native_fill_ink = native
                        .filter(|theme| {
                            !*show_data_label_outside_bar && !theme.form.chart.bar_wireframe
                        })
                        .zip(bar_plot_index)
                        .map(|(theme, plot)| theme.data.fill_ink(plot).to_string());
                    let data_label_color = native_fill_ink.as_deref().unwrap_or(data_label_color);
                    let bar_data_label_inset_px = crate::xychart::XY_CHART_DATA_LABEL_INSET_PX;
                    let labels = crate::xychart::rect_data_labels(
                        data,
                        &layout.label_data,
                        &layout.chart_orientation,
                    );
                    let valid_items = labels.items;
                    let uniform = labels.font_size;

                    if !valid_items.is_empty() {
                        if layout.chart_orientation == "horizontal" {
                            for item in &valid_items {
                                let mut t = node("text");
                                let x = if *show_data_label_outside_bar {
                                    item.rect.x + item.rect.width + bar_data_label_inset_px
                                } else {
                                    item.rect.x + item.rect.width - bar_data_label_inset_px
                                };
                                t.attr("x", fmt_xy(x));
                                t.attr("y", fmt_xy(item.rect.y + item.rect.height / 2.0));
                                t.attr(
                                    "text-anchor",
                                    if *show_data_label_outside_bar {
                                        "start"
                                    } else {
                                        "end"
                                    },
                                );
                                t.attr("dominant-baseline", "middle");
                                t.attr("fill", escape_xml(data_label_color));
                                t.attr("font-size", format!("{}px", fmt_xy(uniform)));
                                t.text = Some(escape_xml(item.label));
                                push_child(&mut arena, parent, t);
                            }
                        } else {
                            let y_offset = bar_data_label_inset_px;
                            for item in &valid_items {
                                let mut t = node("text");
                                t.attr("x", fmt_xy(item.rect.x + item.rect.width / 2.0));
                                let y = if *show_data_label_outside_bar {
                                    item.rect.y - y_offset
                                } else {
                                    item.rect.y + y_offset
                                };
                                t.attr("y", fmt_xy(y));
                                t.attr("text-anchor", "middle");
                                t.attr(
                                    "dominant-baseline",
                                    if *show_data_label_outside_bar {
                                        "auto"
                                    } else {
                                        "hanging"
                                    },
                                );
                                t.attr("fill", escape_xml(data_label_color));
                                t.attr("font-size", format!("{}px", fmt_xy(uniform)));
                                t.text = Some(escape_xml(item.label));
                                push_child(&mut arena, parent, t);
                            }
                        }
                    }
                }
            }
            XyChartDrawableElem::Text { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);
                for t in data {
                    let mut n = node("text");
                    n.attr("x", "0");
                    n.attr("y", "0");
                    n.attr("fill", escape_xml(&t.fill));
                    let role = crate::xychart::XyChartTextRole::from_groups(
                        group_texts,
                        &layout.chart_orientation,
                    );
                    let raw_size = role
                        .map(|role| typography_theme.svg_font_size(role).to_owned())
                        .unwrap_or_else(|| fmt_string(t.font_size));
                    n.attr("font-size", escape_xml(&raw_size));
                    if let Some(weight) = role.and_then(|role| typography_theme.font_weight(role)) {
                        n.attr("font-weight", weight);
                    }
                    if role == Some(crate::xychart::XyChartTextRole::Title)
                        && let Some(font) = typography_theme.native_title_family_css()
                    {
                        n.attr("font-family", escape_xml(font));
                    }
                    n.attr("dominant-baseline", dominant_baseline(&t.vertical_pos));
                    n.attr("text-anchor", text_anchor(&t.horizontal_pos));
                    let rot = t.rotation;
                    n.attr(
                        "transform",
                        format!(
                            "translate({}, {}) rotate({})",
                            fmt_xy(t.x),
                            fmt_xy(t.y),
                            fmt_xy(rot)
                        ),
                    );
                    n.text = Some(escape_xml(&t.text));
                    push_child(&mut arena, parent, n);
                }
            }
            XyChartDrawableElem::Path { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);
                let gridlines = native.is_some() && group_texts.as_slice() == ["gridlines"];
                let marker_fill = native
                    .filter(|_| plot_group_index(group_texts, "line-markers-").is_some())
                    .map(|theme| theme.roles.surface_node.first_color().to_string());
                for p in data {
                    if let Some(fill) = marker_fill.as_deref()
                        && let Some((cx, cy)) = line_marker_center(&p.path)
                    {
                        let mut marker = node("circle");
                        marker.attr("cx", fmt_xy(cx));
                        marker.attr("cy", fmt_xy(cy));
                        marker.attr("r", "3.2");
                        marker.attr("fill", fill);
                        marker.attr("stroke", escape_xml(p.fill.as_deref().unwrap_or("none")));
                        marker.attr("stroke-width", "1.8");
                        push_child(&mut arena, parent, marker);
                        continue;
                    }
                    let mut n = node("path");
                    n.attr("d", escape_xml(&p.path));
                    n.attr("fill", escape_xml(p.fill.as_deref().unwrap_or("none")));
                    n.attr("stroke", escape_xml(&p.stroke_fill));
                    if gridlines {
                        n.attr("stroke-width", "0.8");
                        n.attr("stroke-dasharray", "3 3");
                    } else {
                        n.attr("stroke-width", fmt_xy(p.stroke_width));
                    }
                    push_child(&mut arena, parent, n);
                }
            }
        }
    }

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_bounds = if let Some(theme) = native.filter(|theme| theme.elevation != Elevation::Flat)
    {
        let bounds = bar_surfaces(layout).fold(
            Rect::new(0.0, 0.0, layout.width, layout.height),
            |bounds, (_, _, bar)| {
                let bar = Rect::new(bar.x, bar.y, bar.x + bar.width, bar.y + bar.height);
                bounds.union(elevation_svg::bounds(&theme.elevation, bar))
            },
        );
        root_svg::DiagramBounds::from_view_box(
            bounds.x0,
            bounds.y0,
            bounds.width(),
            bounds.height(),
        )
    } else {
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height)
    };
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "xychart");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::XY_CHART, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;
    out.checkpoint()?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
        out.checkpoint()?;
    }
    if let Some(description) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    typography_theme.common_css().write(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        diagram_id.semantic_str(),
    )?;
    out.checkpoint()?;
    out.push_str("</style>");
    out.checkpoint()?;
    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    if let Some(theme) = native {
        write_native_chart_defs(&mut out, diagram_id, theme, model, layout)?;
    }

    render_node(&mut out, &arena, 0)?;
    write_xychart_temporary_group(&mut out)?;
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct RejectAfterWrites {
        allowed_writes: usize,
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterWrites {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts > self.allowed_writes {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterWrites {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterWrites {
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
                    message: "test SVG sink rejected a write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn xychart_temporary_group_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterWrites::default();

        let error = write_xychart_temporary_group(&mut out)
            .expect_err("the rejecting sink must stop XYChart temporary-group emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "XYChart temporary-group emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn xychart_node_stops_after_the_first_svg_sink_failure() {
        let mut text = node("text");
        text.attr("fill", "#123456");
        text.text = Some("Visible".into());
        let arena = [text];
        let mut complete = RejectAfterWrites {
            allowed_writes: usize::MAX,
            ..Default::default()
        };
        render_node(&mut complete, &arena, 0).unwrap();
        assert_eq!(
            complete.as_str(),
            r##"<text fill="#123456">Visible</text>"##
        );

        for allowed_writes in 0..complete.write_attempts {
            let mut out = RejectAfterWrites {
                allowed_writes,
                ..Default::default()
            };
            assert!(render_node(&mut out, &arena, 0).is_err());
            assert_eq!(out.write_attempts, allowed_writes + 1);
        }
    }
}
