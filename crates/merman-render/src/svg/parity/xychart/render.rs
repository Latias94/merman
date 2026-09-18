use super::super::*;
use merman_core::diagrams::xychart::XyChartPlotType;

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

struct Node {
    tag: &'static str,
    attrs: Vec<(&'static str, String)>,
    text: Option<String>,
    children: Vec<usize>,
    series_terminal: Option<SeriesTerminal>,
    paint_terminal: Option<crate::xychart::XyChartPaintTerminalId>,
    shadow: Option<Box<TerminalShadow>>,
}

impl Node {
    fn attr(&mut self, name: &'static str, value: impl Into<String>) {
        self.attrs.push((name, value.into()));
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    fn observe_shadow(&self, recorder: &crate::diagram_theme::SvgShadowEvidenceRecorder) {
        if let Some(shadow) = &self.shadow
            && self.attribute("filter")
                == Some(escape_xml(&format!("url(#{})", shadow.filter_id)).as_str())
        {
            recorder.record_application(&shadow.effect, &shadow.filter_id, shadow.region);
        }
    }

    fn observe_paint(&self, receipt: &mut crate::xychart::XyChartPaintReceipt) {
        let Some(id) = self.paint_terminal else {
            return;
        };
        let attribute = |name: &str| self.attribute(name);
        let content = if matches!(id, crate::xychart::XyChartPaintTerminalId::Path { .. }) {
            attribute("d")
        } else {
            self.text.as_deref()
        };
        receipt.record(id, self.tag, content, attribute);
    }

    fn observe_series_paint(
        &self,
        plan: &crate::xychart::XyChartSeriesPaintPlan,
        receipt: &mut crate::xychart::XyChartSeriesPaintReceipt,
    ) {
        let attribute = |name: &str| self.attribute(name);
        match self.series_terminal {
            Some(SeriesTerminal::Bar { plot, mark }) => {
                receipt.record_mark(plan, plot, mark, XyChartPlotType::Bar, false, attribute)
            }
            Some(SeriesTerminal::Line { plot, mark }) => {
                receipt.record_mark(plan, plot, mark, XyChartPlotType::Line, false, attribute)
            }
            Some(SeriesTerminal::Legend { plot, plot_type }) => {
                receipt.record_mark(plan, plot, 0, plot_type, true, attribute)
            }
            Some(SeriesTerminal::LineLabel { plot, label }) => {
                receipt.record_line_label(plan, plot, label, attribute("fill"))
            }
            None => {}
        }
    }
}

fn node(tag: &'static str) -> Node {
    Node {
        tag,
        attrs: Vec::with_capacity(6),
        text: None,
        children: Vec::with_capacity(2),
        series_terminal: None,
        paint_terminal: None,
        shadow: None,
    }
}

fn push_child(arena: &mut Vec<Node>, parent: usize, child: Node) -> usize {
    let id = arena.len();
    arena.push(child);
    arena[parent].children.push(id);
    id
}

/// The callback observes only nodes whose complete bytes passed the output checkpoint.
fn render_node(
    out: &mut impl SvgOutput,
    arena: &[Node],
    id: usize,
    observe: &mut impl FnMut(&Node),
) -> Result<()> {
    let n = &arena[id];
    if let Some(shadow) = &n.shadow {
        super::super::shadow::write_theme_shadow_application(
            out,
            &shadow.filter_id,
            &shadow.effect,
            shadow.region,
        );
        out.checkpoint()?;
    }
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
            render_node(out, arena, *c, observe)?;
        }
        let _ = write!(out, "</{}>", n.tag);
        out.checkpoint()?;
    }
    observe(n);
    Ok(())
}

#[derive(Clone, Copy)]
enum SeriesTerminal {
    Bar {
        plot: usize,
        mark: usize,
    },
    Line {
        plot: usize,
        mark: usize,
    },
    LineLabel {
        plot: usize,
        label: usize,
    },
    Legend {
        plot: usize,
        plot_type: XyChartPlotType,
    },
}

/// Each filter belongs to one actual mark. The definition and reference share this immutable plan.
struct TerminalShadow {
    effect: std::sync::Arc<crate::diagram_theme::SvgShadowEffect>,
    filter_id: String,
    region: crate::diagram_theme::SvgFilterRegion,
}

fn prepare_series_shadows(
    arena: &mut [Node],
    series: &crate::xychart::XyChartSeriesPaintPlan,
    diagram_id: &str,
    layout: &XyChartDiagramLayout,
    options: &SvgExecution<'_>,
) -> Result<(root_svg::DiagramBounds, usize)> {
    if !series.has_effect() {
        return Ok((
            root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height),
            0,
        ));
    }
    let resources = options.theme_resource_policy();
    let mut bounds = [0.0_f64, 0.0_f64, layout.width, layout.height];
    let mut expected = 0;
    for (terminal, node) in arena.iter_mut().enumerate() {
        let plot = match node.series_terminal {
            Some(
                SeriesTerminal::Bar { plot, .. }
                | SeriesTerminal::Line { plot, .. }
                | SeriesTerminal::Legend { plot, .. },
            ) => plot,
            _ => continue,
        };
        let Some(effect) = series.effect(plot) else {
            continue;
        };
        expected += 1;
        options
            .work_meter()
            .charge(node.attribute("d").map_or(1, str::len))?;
        options
            .work_meter()
            .charge(effect.stages().len().saturating_mul(3))?;
        let geometry = match node.tag {
            "rect" => {
                let number = |name| {
                    node.attribute(name)
                        .and_then(|value| value.parse::<f64>().ok())
                };
                number("x")
                    .zip(number("y"))
                    .zip(number("width").zip(number("height")))
                    .filter(|((_, _), (width, height))| *width >= 0.0 && *height >= 0.0)
                    .map(|((x, y), (width, height))| [x, y, x + width, y + height])
            }
            "path" => node
                .attribute("d")
                .and_then(svg_path_bounds_from_d)
                .map(|path| [path.min_x, path.min_y, path.max_x, path.max_y]),
            _ => None,
        };
        let Some([min_x, min_y, max_x, max_y]) = geometry else {
            continue;
        };
        let Some(stroke_width) = node
            .attribute("stroke-width")
            .and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        if !stroke_width.is_finite() || stroke_width < 0.0 {
            continue;
        }
        // Paths use SVG's default miter limit of four; rectangles have right-angle corners.
        let stroke_outset = stroke_width / 2.0 * if node.tag == "path" { 4.0 } else { 1.0 };
        let source = crate::diagram_theme::EffectOutsets {
            top: stroke_outset,
            right: stroke_outset,
            bottom: stroke_outset,
            left: stroke_outset,
        };
        let Some(materialized) =
            effect.materialize_user_space(&resources, min_x, min_y, max_x, max_y, source)?
        else {
            continue;
        };
        let region = materialized.region();
        let [x, y, width, height] = region.as_array().map(f64::from);
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x + width);
        bounds[3] = bounds[3].max(y + height);
        let filter_id = format!(
            "{diagram_id}-xychart-series-{plot}-terminal-{terminal}-theme-effect-{}",
            effect.id()
        );
        node.attr("filter", escape_xml(&format!("url(#{filter_id})")));
        node.shadow = Some(Box::new(TerminalShadow {
            effect: effect.clone(),
            filter_id,
            region,
        }));
    }
    Ok((
        root_svg::DiagramBounds::from_extents(bounds[0], bounds[1], bounds[2], bounds[3], 0.0),
        expected,
    ))
}

fn prepare_text_shadows(
    arena: &mut [Node],
    paint: &crate::xychart::XyChartPaintPlan,
    diagram_id: &str,
    layout: &XyChartDiagramLayout,
    bounds: root_svg::DiagramBounds,
    options: &SvgExecution<'_>,
) -> Result<(root_svg::DiagramBounds, bool)> {
    if paint.expected_effect_applications() == 0 {
        return Ok((bounds, true));
    }
    let mut extents = [
        bounds.min_x,
        bounds.min_y,
        bounds.min_x + bounds.width,
        bounds.min_y + bounds.height,
    ];
    let mut complete = true;
    for node in arena {
        let Some(id @ crate::xychart::XyChartPaintTerminalId::Text { drawable, item }) =
            node.paint_terminal
        else {
            continue;
        };
        let Some(shadow) = paint.text_effect(id) else {
            continue;
        };
        let crate::model::XyChartDrawableElem::Text { data, .. } = &layout.drawables[drawable]
        else {
            continue;
        };
        let label = &data[item];
        if !shadow.width.is_finite()
            || !shadow.height.is_finite()
            || shadow.width <= 0.0
            || shadow.height <= 0.0
        {
            complete = false;
            continue;
        }
        options
            .work_meter()
            .charge(shadow.effect.stages().len().saturating_mul(3))?;
        // Keep blur margins in absolute SVG units. Scaling percentages derived from
        // estimated text dimensions would shrink the halo when the host uses a smaller bbox.
        // Native export independently verifies actual glyph ink against this allocation.
        let x = match label.horizontal_pos.as_str() {
            "left" => 0.0,
            "right" => -shadow.width,
            _ => -shadow.width / 2.0,
        };
        let y = if label.vertical_pos == "top" {
            0.0
        } else {
            -shadow.height / 2.0
        };
        let Some(materialized) = shadow.effect.materialize_user_space(
            &options.theme_resource_policy(),
            x,
            y,
            x + shadow.width,
            y + shadow.height,
            // One font em reserves local paint space beyond approximate layout metrics.
            // This is an allocation policy, not an ink bound: long strings or unusual
            // host fonts can exceed it and must still fail native containment.
            crate::diagram_theme::EffectOutsets {
                top: label.font_size,
                right: label.font_size,
                bottom: label.font_size,
                left: label.font_size,
            },
        )?
        else {
            complete = false;
            continue;
        };
        let [left, top, width, height] = materialized.region().as_array().map(f64::from);
        let (sin, cos) = label.rotation.to_radians().sin_cos();
        for px in [left, left + width] {
            for py in [top, top + height] {
                let rx = label.x + px * cos - py * sin;
                let ry = label.y + px * sin + py * cos;
                extents[0] = extents[0].min(rx);
                extents[1] = extents[1].min(ry);
                extents[2] = extents[2].max(rx);
                extents[3] = extents[3].max(ry);
            }
        }
        let filter_id = paint
            .filter_id(diagram_id, id)
            .expect("text effect has a filter identity");
        node.attr("filter", escape_xml(&format!("url(#{filter_id})")));
        node.shadow = Some(Box::new(TerminalShadow {
            effect: shadow.effect.clone(),
            filter_id,
            region: materialized.region(),
        }));
    }
    Ok((
        root_svg::DiagramBounds::from_extents(extents[0], extents[1], extents[2], extents[3], 0.0),
        complete,
    ))
}

pub(crate) fn render_xychart_diagram_svg(
    artifact: &crate::family::XyChartFamilyArtifact,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let layout = artifact.pair().layout();
    let model = artifact.pair().semantic();
    let series_paint = artifact.series_paint();
    let paint_theme = artifact.paint_theme();
    let typography_theme = artifact.typography_theme();
    let effect_evidence = artifact.effect_evidence();
    let expected_effect_applications = artifact.expected_effect_applications();
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

    if series_paint.plot_count() != model.plots.len() {
        return Err(crate::Error::InvalidModel {
            message: "XY Chart series paint plan does not match the terminal model".to_string(),
        });
    }

    let diagram_id = options.diagram_id_or("xychart");
    let acc_title = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(|description| description.trim_end_matches('\n'))
        .filter(|description| !description.trim().is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let data_label_config = if layout.show_data_label {
        Some((
            layout.show_data_label_outside_bar,
            paint_theme.data_label_color(),
        ))
    } else {
        None
    };
    let mut series_paint_receipt = series_paint.begin_terminal_receipt();
    let mut paint_receipt =
        paint_theme.begin_terminal_receipt(escape_xml, diagram_id.semantic_str());
    let mut typography_receipt = typography_theme.begin_terminal_receipt();

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

    for (drawable, shape) in layout.drawables.iter().enumerate() {
        match shape {
            crate::model::XyChartDrawableElem::Rect { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);
                let bar_plot_index = plot_group_index(group_texts, "bar-plot-");

                // Append rect elements.
                for (mark_index, r) in data.iter().enumerate() {
                    let mut n = node("rect");
                    n.attr("x", fmt_xy(r.x));
                    if !r.y.is_nan() {
                        n.attr("y", fmt_xy(r.y));
                    }
                    n.attr("width", fmt_xy(r.width));
                    n.attr("height", fmt_xy(r.height));
                    n.attr("fill", escape_xml(&r.fill));
                    n.attr("stroke", escape_xml(&r.stroke_fill));
                    n.attr("stroke-width", fmt_xy(r.stroke_width));
                    n.series_terminal = bar_plot_index.map(|plot| SeriesTerminal::Bar {
                        plot,
                        mark: mark_index,
                    });
                    let series = bar_plot_index.or_else(|| {
                        (group_texts.as_slice() == ["legend", "markers"])
                            .then(|| series_paint.legend_plot(XyChartPlotType::Bar, mark_index))
                            .flatten()
                    });
                    if let Some(plot) = series {
                        for (name, value) in series_paint.opacity_attributes(plot) {
                            n.attr(name, value);
                        }
                        if bar_plot_index.is_none() {
                            n.series_terminal = Some(SeriesTerminal::Legend {
                                plot,
                                plot_type: XyChartPlotType::Bar,
                            });
                        }
                    }
                    push_child(&mut arena, parent, n);
                }

                // Mermaid emits data labels for every rectangle group, including legend markers.
                if let Some((show_data_label_outside_bar, data_label_color)) = &data_label_config {
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
                                t.paint_terminal = paint_theme.terminal_id(
                                    crate::xychart::XyChartPaintTerminalId::DataLabel {
                                        drawable,
                                        item: item.item,
                                    },
                                );
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
                                t.paint_terminal = paint_theme.terminal_id(
                                    crate::xychart::XyChartPaintTerminalId::DataLabel {
                                        drawable,
                                        item: item.item,
                                    },
                                );
                                push_child(&mut arena, parent, t);
                            }
                        }
                    }
                }
            }
            crate::model::XyChartDrawableElem::Text { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);
                let line_label_plot_index = (group_texts.get(2).map(String::as_str)
                    == Some("labels"))
                .then(|| plot_group_index(group_texts, "line-plot-"))
                .flatten();

                for (label_index, t) in data.iter().enumerate() {
                    let mut n = node("text");
                    n.attr("x", "0");
                    n.attr("y", "0");
                    n.attr("fill", escape_xml(&t.fill));
                    n.attr("font-size", fmt_string(t.font_size));
                    if let Some(weight) = crate::xychart::XyChartTextRole::from_groups(
                        group_texts,
                        &layout.chart_orientation,
                    )
                    .and_then(|role| typography_theme.font_weight(role))
                    {
                        n.attr("font-weight", weight);
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
                    n.paint_terminal =
                        paint_theme.terminal_id(crate::xychart::XyChartPaintTerminalId::Text {
                            drawable,
                            item: label_index,
                        });
                    n.series_terminal =
                        line_label_plot_index.map(|plot| SeriesTerminal::LineLabel {
                            plot,
                            label: label_index,
                        });
                    push_child(&mut arena, parent, n);
                }
            }
            crate::model::XyChartDrawableElem::Path { group_texts, data } => {
                if data.is_empty() {
                    continue;
                }
                let parent = ensure_group_path(&mut arena, &mut groups_by_path, group_texts);
                let line_plot_index = plot_group_index(group_texts, "line-plot-");

                for (mark_index, p) in data.iter().enumerate() {
                    let mut n = node("path");
                    n.attr("d", escape_xml(&p.path));
                    n.attr("fill", escape_xml(p.fill.as_deref().unwrap_or("none")));
                    n.attr("stroke", escape_xml(&p.stroke_fill));
                    n.attr("stroke-width", fmt_xy(p.stroke_width));
                    if let Some(opacity) =
                        paint_theme.path_opacity(group_texts, &layout.chart_orientation)
                    {
                        n.attr("opacity", opacity.to_string());
                    }
                    n.paint_terminal =
                        paint_theme.terminal_id(crate::xychart::XyChartPaintTerminalId::Path {
                            drawable,
                            item: mark_index,
                        });
                    n.series_terminal = line_plot_index.map(|plot| SeriesTerminal::Line {
                        plot,
                        mark: mark_index,
                    });
                    let series = line_plot_index.or_else(|| {
                        (group_texts.as_slice() == ["legend", "markers"])
                            .then(|| series_paint.legend_plot(XyChartPlotType::Line, mark_index))
                            .flatten()
                    });
                    if let Some(plot) = series {
                        for (name, value) in series_paint.opacity_attributes(plot) {
                            n.attr(name, value);
                        }
                        if line_plot_index.is_none() {
                            n.series_terminal = Some(SeriesTerminal::Legend {
                                plot,
                                plot_type: XyChartPlotType::Line,
                            });
                        }
                    }
                    push_child(&mut arena, parent, n);
                }
            }
        }
    }

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let (root_bounds, series_effect_count) = prepare_series_shadows(
        &mut arena,
        series_paint,
        diagram_id.semantic_str(),
        layout,
        options,
    )?;
    let (root_bounds, text_effect_allocation_complete) = prepare_text_shadows(
        &mut arena,
        paint_theme,
        diagram_id.semantic_str(),
        layout,
        root_bounds,
        options,
    )?;
    expected_effect_applications
        .set(series_effect_count + paint_theme.expected_effect_applications())
        .map_err(|_| crate::Error::InvalidModel {
            message: "XY Chart effects were emitted more than once".into(),
        })?;
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "xychart");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::XY_CHART, diagram_id)
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
    let mut css = String::new();
    push_xychart_css(
        &mut css,
        diagram_id.semantic_str(),
        typography_theme.font_family_css(),
    );
    out.push_str(&css);
    drop(css);
    out.checkpoint()?;
    out.push_str("</style>");
    out.checkpoint()?;
    if let Some(receipt) = typography_receipt.as_mut() {
        receipt.record_css(typography_theme.font_family_css());
    }

    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;

    render_node(&mut out, &arena, 0, &mut |emitted| {
        emitted.observe_shadow(effect_evidence);
        if let Some(receipt) = paint_receipt.as_mut() {
            emitted.observe_paint(receipt);
        }
        if emitted.tag == "text"
            && emitted.text.as_deref().is_some_and(|text| !text.is_empty())
            && let Some(receipt) = typography_receipt.as_mut()
        {
            receipt.record_visible_text();
        }
        if let Some(receipt) = series_paint_receipt.as_mut() {
            emitted.observe_series_paint(series_paint, receipt);
        }
    })?;
    write_xychart_temporary_group(&mut out)?;
    out.push_str("</svg>\n");
    let rooted = root_document.complete(out.finish()?)?;
    if series_paint_receipt.is_some_and(|receipt| !series_paint.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "XY Chart series paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if paint_receipt.is_some_and(|receipt| !paint_theme.record_terminal(receipt))
        && text_effect_allocation_complete
    {
        return Err(crate::Error::InvalidModel {
            message: "XY Chart paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = typography_receipt {
        let _ = typography_theme.record_terminal(receipt);
    }
    Ok(rooted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::xychart::XyChartDiagramRenderModel;
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
    fn xychart_terminal_observation_requires_a_fully_written_node() {
        let mut text = node("text");
        text.attr("fill", "#123456");
        text.text = Some("Visible".into());
        let arena = [text];
        let mut complete = RejectAfterWrites {
            allowed_writes: usize::MAX,
            ..Default::default()
        };
        let mut observed = 0;
        render_node(&mut complete, &arena, 0, &mut |_| observed += 1).unwrap();
        assert_eq!(observed, 1);
        assert_eq!(
            complete.as_str(),
            r##"<text fill="#123456">Visible</text>"##
        );

        for allowed_writes in 0..complete.write_attempts {
            let mut out = RejectAfterWrites {
                allowed_writes,
                ..Default::default()
            };
            let mut observed = 0;
            assert!(render_node(&mut out, &arena, 0, &mut |_| observed += 1).is_err());
            assert_eq!(observed, 0, "partial node after {allowed_writes} writes");
        }
    }

    #[test]
    fn xychart_paint_receipt_binds_owner_and_exactly_one_written_title() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch, ThemeTarget,
        };
        for with_effect in [false, true] {
            let mut style =
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
            style.typography.font_weight = crate::diagram_theme::Specified::Value(700);
            let mut spec = DiagramThemeSpec::new();
            if with_effect {
                use crate::diagram_theme::{
                    DiagramEffectSet, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue,
                };
                style = style.with_effect("glow").unwrap();
                spec = spec.with_effects(
                    DiagramEffectSet::default()
                        .with_graph(
                            EffectGraph::new(
                                "glow",
                                [EffectPrimitive::DropShadow {
                                    input: EffectInput::SourceGraphic,
                                    offset_x: 0.0,
                                    offset_y: 0.0,
                                    blur_radius: 3.0,
                                    spread: 0.0,
                                    color: ThemeColorValue::parse("#00f2ff").unwrap(),
                                }],
                            )
                            .unwrap(),
                        )
                        .unwrap(),
                );
            }
            let theme = DiagramThemeCompiler::new()
                .compile(spec.with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Title, style)),
                ))
                .unwrap();
            let theme = theme.resolve(crate::DiagramFamilyId::XY_CHART);
            let typography = crate::xychart::XyChartTypographyThemePlan::resolve(
                Some(&theme),
                &merman_core::MermaidConfig::default(),
                &crate::resources::OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::interactive(),
                ),
            )
            .unwrap();
            let prepare = || {
                let mut layout: crate::model::XyChartDiagramLayout =
                serde_json::from_value(serde_json::json!({
                    "width": 700.0, "height": 500.0, "chartOrientation": "vertical",
                    "showDataLabel": false, "showDataLabelOutsideBar": false, "labelData": [], "backgroundColor": "white",
                    "drawables": []
                }))
                .unwrap();
                layout
                    .drawables
                    .push(crate::model::XyChartDrawableElem::Text {
                        group_texts: vec!["chart-title".into()],
                        data: vec![crate::model::XyChartTextData {
                            text: "A & B".into(),
                            x: 0.0,
                            y: 0.0,
                            fill: "black".into(),
                            font_size: 20.0,
                            rotation: 0.0,
                            vertical_pos: "middle".into(),
                            horizontal_pos: "center".into(),
                        }],
                    });
                crate::xychart::XyChartPaintPlan::resolve(
                    Some(&theme),
                    &merman_core::MermaidConfig::default(),
                    &mut layout,
                    &typography,
                    &crate::text::DeterministicTextMeasurer::default(),
                    &crate::resources::OperationWorkMeter::new(
                        crate::resources::RenderResourcePolicy::interactive(),
                    ),
                )
                .unwrap()
            };
            for mutation in [
                "none",
                "detached",
                "missing-fill",
                "missing-font-size",
                "wrong-font-size",
                "missing-font-weight",
                "wrong-font-weight",
                "wrong-text",
                "repeated",
                "wrong-role",
                "unwritten",
                "foreign",
                "missing-filter",
                "wrong-filter",
                "transform",
                "text-anchor",
                "dominant-baseline",
                "unexpected-filter",
            ] {
                if !with_effect
                    && matches!(
                        mutation,
                        "missing-filter"
                            | "wrong-filter"
                            | "transform"
                            | "text-anchor"
                            | "dominant-baseline"
                    )
                {
                    continue;
                }
                let plan = prepare();
                let foreign = prepare();
                let mut receipt = if mutation == "foreign" {
                    &foreign
                } else {
                    &plan
                }
                .begin_terminal_receipt(escape_xml, "test")
                .unwrap();
                let mut arena = vec![node("g")];
                let mut title = node("text");
                title.attr("fill", "#123456");
                title.attr("font-size", "20");
                title.attr("font-weight", "700");
                title.text = Some(escape_xml("A & B"));
                title.paint_terminal = Some(crate::xychart::XyChartPaintTerminalId::Text {
                    drawable: 0,
                    item: 0,
                });
                if let Some(filter) = plan.filter_id("test", title.paint_terminal.unwrap()) {
                    title.attr("filter", format!("url(#{filter})"));
                    title.attr("transform", "translate(0, 0) rotate(0)");
                    title.attr("text-anchor", "middle");
                    title.attr("dominant-baseline", "middle");
                }
                let id = push_child(&mut arena, 0, title);
                match mutation {
                    "missing-filter" => arena[id].attrs.retain(|(key, _)| *key != "filter"),
                    "wrong-filter" | "unexpected-filter" => {
                        arena[id].attrs.retain(|(key, _)| *key != "filter");
                        arena[id].attr("filter", "url(#another-real-filter)");
                    }
                    "transform" | "text-anchor" | "dominant-baseline" => {
                        arena[id]
                            .attrs
                            .iter_mut()
                            .find(|(key, _)| *key == mutation)
                            .unwrap()
                            .1 = "wrong".into();
                    }
                    "detached" => arena[0].children.clear(),
                    "missing-fill" => arena[id].attrs.clear(),
                    "missing-font-size" => arena[id].attrs.retain(|(key, _)| *key != "font-size"),
                    "wrong-font-size" => {
                        arena[id]
                            .attrs
                            .iter_mut()
                            .find(|(key, _)| *key == "font-size")
                            .unwrap()
                            .1 = "0".into()
                    }
                    "missing-font-weight" => {
                        arena[id].attrs.retain(|(key, _)| *key != "font-weight")
                    }
                    "wrong-font-weight" => {
                        arena[id]
                            .attrs
                            .iter_mut()
                            .find(|(key, _)| *key == "font-weight")
                            .unwrap()
                            .1 = "400".into();
                    }
                    "wrong-text" => arena[id].text = Some("Other".into()),
                    "repeated" => arena[0].children.push(id),
                    "wrong-role" => arena[id].paint_terminal = None,
                    _ => {}
                }
                if mutation != "unwritten" {
                    render_node(&mut String::new(), &arena, 0, &mut |emitted| {
                        emitted.observe_paint(&mut receipt);
                    })
                    .unwrap();
                }
                assert_eq!(
                    plan.record_terminal(receipt),
                    mutation == "none",
                    "{mutation}"
                );
                assert_eq!(
                    plan.finish_evidence().applied().len(),
                    usize::from(mutation == "none")
                );
            }
        }
    }

    #[test]
    fn xychart_paint_receipt_requires_every_axis_text_path_and_rectangle_label() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch, ThemeTarget,
        };
        use crate::xychart::XyChartPaintTerminalId;
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                [ThemeTarget::Text, ThemeTarget::Axis].into_iter().fold(
                    ThemeRuleSet::default(),
                    |rules, target| {
                        rules.with_rule(ThemeRule::new(
                            target,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        ))
                    },
                ),
            ))
            .unwrap()
            .resolve(crate::DiagramFamilyId::XY_CHART);
        let typography = crate::xychart::XyChartTypographyThemePlan::resolve(
            Some(&theme),
            &merman_core::MermaidConfig::default(),
            &crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::interactive(),
            ),
        )
        .unwrap();
        let prepare = || {
            let mut layout: crate::model::XyChartDiagramLayout =
                serde_json::from_value(serde_json::json!({
                    "width": 700.0, "height": 500.0, "chartOrientation": "vertical",
                    "showDataLabel": true, "showDataLabelOutsideBar": false,
                    "labelData": ["4"], "backgroundColor": "white", "drawables": []
                }))
                .unwrap();
            layout.drawables = vec![
                crate::model::XyChartDrawableElem::Text {
                    group_texts: vec!["bottom-axis".into(), "label".into()],
                    data: vec![crate::model::XyChartTextData {
                        text: "A & B".into(),
                        x: 0.0,
                        y: 0.0,
                        fill: "black".into(),
                        font_size: f64::from(f32::from_bits(0x417f_ffff)),
                        rotation: 0.0,
                        vertical_pos: "middle".into(),
                        horizontal_pos: "center".into(),
                    }],
                },
                crate::model::XyChartDrawableElem::Path {
                    group_texts: vec!["bottom-axis".into(), "axis-line".into()],
                    data: vec![crate::model::XyChartPathData {
                        path: "M 0,0 L 10,0".into(),
                        fill: None,
                        stroke_fill: "black".into(),
                        stroke_width: 2.000_000_1,
                    }],
                },
                crate::model::XyChartDrawableElem::Rect {
                    group_texts: vec!["plot".into(), "bar-plot-0".into()],
                    data: vec![crate::model::XyChartRectData {
                        x: 0.0,
                        y: 0.0,
                        width: 40.0,
                        height: 80.0,
                        fill: "red".into(),
                        stroke_fill: "red".into(),
                        stroke_width: 0.0,
                    }],
                },
                crate::model::XyChartDrawableElem::Rect {
                    group_texts: vec!["legend".into(), "markers".into()],
                    data: vec![crate::model::XyChartRectData {
                        x: 0.0,
                        y: 0.0,
                        width: 4.0,
                        height: 4.0,
                        fill: "red".into(),
                        stroke_fill: "red".into(),
                        stroke_width: 0.0,
                    }],
                },
            ];
            crate::xychart::XyChartPaintPlan::resolve(
                Some(&theme),
                &merman_core::MermaidConfig::default(),
                &mut layout,
                &typography,
                &crate::text::DeterministicTextMeasurer::default(),
                &crate::resources::OperationWorkMeter::new(
                    crate::resources::RenderResourcePolicy::interactive(),
                ),
            )
            .unwrap()
        };
        for terminal in 0..4 {
            for mutation in [
                "none",
                "detached",
                "missing-paint",
                "wrong-content",
                "repeated",
                "wrong-role",
                "foreign",
                "partial-write",
                "wrong-font-size",
                "wrong-axis-font-size",
                "missing-axis-font-size",
                "wrong-stroke-width",
                "missing-stroke-width",
            ] {
                let plan = prepare();
                let foreign = prepare();
                let mut receipt = if mutation == "foreign" {
                    &foreign
                } else {
                    &plan
                }
                .begin_terminal_receipt(escape_xml, "test")
                .unwrap();
                let mut arena = vec![node("g")];
                for (id, tag, content) in [
                    (
                        XyChartPaintTerminalId::Text {
                            drawable: 0,
                            item: 0,
                        },
                        "text",
                        "A & B",
                    ),
                    (
                        XyChartPaintTerminalId::Path {
                            drawable: 1,
                            item: 0,
                        },
                        "path",
                        "M 0,0 L 10,0",
                    ),
                    (
                        XyChartPaintTerminalId::DataLabel {
                            drawable: 2,
                            item: 0,
                        },
                        "text",
                        "4",
                    ),
                    (
                        XyChartPaintTerminalId::DataLabel {
                            drawable: 3,
                            item: 0,
                        },
                        "text",
                        "4",
                    ),
                ] {
                    let mut n = node(tag);
                    if tag == "path" {
                        n.attr("d", content);
                        n.attr("stroke", "#123456");
                        n.attr("stroke-width", "2");
                    } else {
                        n.text = Some(escape_xml(content));
                        n.attr("fill", "#123456");
                    }
                    if matches!(id, XyChartPaintTerminalId::Text { .. }) {
                        n.attr("font-size", "16");
                    }
                    if let XyChartPaintTerminalId::DataLabel { drawable, .. } = id {
                        n.attr("font-size", if drawable == 2 { "57px" } else { "0px" });
                    }
                    n.paint_terminal = plan.terminal_id(id);
                    push_child(&mut arena, 0, n);
                }
                let id = terminal + 1;
                match mutation {
                    "detached" => {
                        arena[0].children.remove(terminal);
                    }
                    "missing-paint" => arena[id]
                        .attrs
                        .retain(|(key, _)| *key != "fill" && *key != "stroke"),
                    "wrong-content" if terminal == 1 => {
                        arena[id].attrs[0].1 = "M 0,0 L 20,0".into()
                    }
                    "wrong-content" => arena[id].text = Some("Other".into()),
                    "repeated" => arena[0].children.push(id),
                    "wrong-role" => arena[id].paint_terminal = None,
                    "wrong-axis-font-size" => {
                        arena[1]
                            .attrs
                            .iter_mut()
                            .find(|(key, _)| *key == "font-size")
                            .unwrap()
                            .1 = "0".into()
                    }
                    "missing-axis-font-size" => {
                        arena[1].attrs.retain(|(key, _)| *key != "font-size")
                    }
                    "wrong-stroke-width" => {
                        arena[2]
                            .attrs
                            .iter_mut()
                            .find(|(key, _)| *key == "stroke-width")
                            .unwrap()
                            .1 = "0".into()
                    }
                    "missing-stroke-width" => {
                        arena[2].attrs.retain(|(key, _)| *key != "stroke-width")
                    }
                    "wrong-font-size" => {
                        // Mutate the positive-area data label regardless of the selected terminal.
                        let value = arena[3]
                            .attrs
                            .iter_mut()
                            .find(|(key, _)| *key == "font-size")
                            .unwrap();
                        value.1 = "0px".into();
                    }
                    _ => {}
                }
                if mutation == "partial-write" {
                    let mut sink = RejectAfterWrites::default();
                    assert!(
                        render_node(&mut sink, &arena, 0, &mut |n| n.observe_paint(&mut receipt))
                            .is_err()
                    );
                } else {
                    render_node(&mut String::new(), &arena, 0, &mut |n| {
                        n.observe_paint(&mut receipt)
                    })
                    .unwrap();
                }
                assert_eq!(
                    plan.record_terminal(receipt),
                    mutation == "none",
                    "{terminal}: {mutation}"
                );
                assert_eq!(
                    plan.finish_evidence().applied().len(),
                    if mutation == "none" { 2 } else { 0 }
                );
            }
        }
    }

    #[test]
    fn xychart_tick_receipt_requires_exact_opacity_and_clear() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule, ThemeRuleSet,
            ThemeStylePatch, ThemeTarget,
        };
        for (requested, expected) in [
            (Specified::Value(0.3), Some("0.3")),
            (Specified::Clear, None),
        ] {
            let mut style = ThemeStylePatch::default();
            style.paint.opacity = requested;
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::AxisTick, style)),
                ))
                .unwrap()
                .resolve(crate::DiagramFamilyId::XY_CHART);
            let config = merman_core::MermaidConfig::default();
            let meter = crate::resources::OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::interactive(),
            );
            let typography =
                crate::xychart::XyChartTypographyThemePlan::resolve(Some(&theme), &config, &meter)
                    .unwrap();
            for actual in [None, Some("0.3"), Some("0.7")] {
                let mut layout: crate::model::XyChartDiagramLayout = serde_json::from_value(serde_json::json!({
                    "width":700.0, "height":500.0, "chartOrientation":"vertical", "showDataLabel":false,
                    "showDataLabelOutsideBar":false, "labelData":[], "backgroundColor":"white", "drawables":[]
                })).unwrap();
                layout
                    .drawables
                    .push(crate::model::XyChartDrawableElem::Path {
                        group_texts: vec!["bottom-axis".into(), "ticks".into()],
                        data: vec![crate::model::XyChartPathData {
                            path: "M 0,0 L 0,5".into(),
                            fill: None,
                            stroke_fill: "black".into(),
                            stroke_width: 2.0,
                        }],
                    });
                let plan = crate::xychart::XyChartPaintPlan::resolve(
                    Some(&theme),
                    &config,
                    &mut layout,
                    &typography,
                    &crate::text::DeterministicTextMeasurer::default(),
                    &meter,
                )
                .unwrap();
                let mut receipt = plan.begin_terminal_receipt(escape_xml, "ticks").unwrap();
                let mut path = node("path");
                path.attr("d", "M 0,0 L 0,5");
                path.attr("stroke", "black");
                path.attr("stroke-width", "2");
                if let Some(value) = actual {
                    path.attr("opacity", value);
                }
                path.paint_terminal =
                    plan.terminal_id(crate::xychart::XyChartPaintTerminalId::Path {
                        drawable: 0,
                        item: 0,
                    });
                render_node(&mut String::new(), &[path], 0, &mut |n| {
                    n.observe_paint(&mut receipt)
                })
                .unwrap();
                assert_eq!(
                    plan.record_terminal(receipt),
                    actual == expected,
                    "actual={actual:?}, expected={expected:?}"
                );
                assert_eq!(
                    plan.finish_evidence().applied().len(),
                    usize::from(actual == expected)
                );
            }
        }
    }

    #[test]
    fn xychart_series_receipt_rejects_detached_missing_and_repeated_terminals() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
            ThemeTarget,
        };
        let model: XyChartDiagramRenderModel = serde_json::from_value(serde_json::json!({
            "xAxis": { "type": "band", "categories": ["A"] },
            "yAxis": { "type": "linear", "min": 0, "max": 10 },
            "plots": [{ "type": "bar", "values": [4], "data": [["A", 4]] }]
        }))
        .unwrap();
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::ChartSeries, palette),
            ))
            .unwrap()
            .resolve(crate::DiagramFamilyId::XY_CHART);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::interactive(),
        );
        for mutation in ["none", "detached", "missing-fill", "repeated", "unwritten"] {
            let plan = crate::xychart::XyChartSeriesPaintPlan::resolve(
                Some(&theme),
                &merman_core::MermaidConfig::default(),
                &model,
                &meter,
            )
            .unwrap();
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            let mut arena = vec![node("g")];
            let mut bar = node("rect");
            bar.attr("fill", "#123456");
            bar.attr("stroke", "#123456");
            bar.attr("stroke-width", "0");
            bar.series_terminal = Some(SeriesTerminal::Bar { plot: 0, mark: 0 });
            let id = push_child(&mut arena, 0, bar);
            match mutation {
                "detached" => arena[0].children.clear(),
                "missing-fill" => arena[id].attrs.retain(|(name, _)| *name != "fill"),
                "repeated" => arena[0].children.push(id),
                _ => {}
            }
            let mut out = String::new();
            if mutation != "unwritten" {
                render_node(&mut out, &arena, 0, &mut |emitted| {
                    emitted.observe_series_paint(&plan, &mut receipt);
                })
                .unwrap();
            }
            assert_eq!(
                plan.record_terminal(receipt),
                mutation == "none",
                "{mutation}"
            );
        }
    }

    #[test]
    fn xychart_series_receipt_requires_each_width_alpha_and_visible_legend_terminal() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, Specified, ThemeRule,
            ThemeRuleSet, ThemeStylePatch, ThemeTarget,
        };
        let model: XyChartDiagramRenderModel = serde_json::from_value(serde_json::json!({
            "xAxis": { "type": "band", "categories": ["A"] },
            "yAxis": { "type": "linear", "min": 0, "max": 10 },
            "plots": [
                { "type": "bar", "title": "Bar", "values": [4], "data": [["A", 4]] },
                { "type": "line", "title": "Line", "values": [7], "data": [["A", 7]] }
            ]
        }))
        .unwrap();
        let mut style = ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#ff0000").unwrap())
            .with_stroke(CanvasPaint::solid("#00ff00").unwrap())
            .with_stroke_width(4.0)
            .unwrap();
        style.paint.opacity = Specified::Value(0.5);
        style.paint.fill_opacity = Specified::Value(0.25);
        style.stroke.stroke_opacity = Specified::Value(0.75);
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::ChartSeries, style)),
            ))
            .unwrap()
            .resolve(crate::DiagramFamilyId::XY_CHART);
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::interactive(),
        );
        for mutation in [
            "none",
            "missing-width",
            "wrong-width",
            "missing-opacity",
            "wrong-fill-opacity",
            "wrong-stroke-opacity",
            "missing-later-plot",
            "missing-legend",
            "wrong-legend-width",
            "wrong-legend-opacity",
            "repeated-legend",
        ] {
            let plan = crate::xychart::XyChartSeriesPaintPlan::resolve(
                Some(&theme),
                &merman_core::MermaidConfig::default(),
                &model,
                &meter,
            )
            .unwrap();
            assert!(plan.record_legend_layout(vec![0, 1]));
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            let mut arena = vec![node("g")];
            for terminal in [
                SeriesTerminal::Bar { plot: 0, mark: 0 },
                SeriesTerminal::Line { plot: 1, mark: 0 },
                SeriesTerminal::Legend {
                    plot: 0,
                    plot_type: XyChartPlotType::Bar,
                },
                SeriesTerminal::Legend {
                    plot: 1,
                    plot_type: XyChartPlotType::Line,
                },
            ] {
                let mut mark = node("path");
                mark.attr("fill", "#ff0000");
                mark.attr("stroke", "#00ff00");
                mark.attr("stroke-width", "4");
                mark.attr("opacity", "0.5");
                mark.attr("fill-opacity", "0.25");
                mark.attr("stroke-opacity", "0.75");
                mark.series_terminal = Some(terminal);
                push_child(&mut arena, 0, mark);
            }
            let changed = match mutation {
                "missing-width" => Some((1, "stroke-width", None)),
                "wrong-width" => Some((1, "stroke-width", Some("5"))),
                "missing-opacity" => Some((1, "opacity", None)),
                "wrong-fill-opacity" => Some((1, "fill-opacity", Some("0.5"))),
                "wrong-stroke-opacity" => Some((2, "stroke-opacity", Some("1"))),
                "wrong-legend-width" => Some((3, "stroke-width", Some("0"))),
                "wrong-legend-opacity" => Some((4, "opacity", Some("1"))),
                _ => None,
            };
            if let Some((id, attr, replacement)) = changed {
                arena[id].attrs.retain(|(name, _)| *name != attr);
                if let Some(value) = replacement {
                    arena[id].attr(attr, value);
                }
            }
            match mutation {
                "missing-later-plot" => arena[0].children.retain(|id| *id != 2),
                "missing-legend" => arena[0].children.retain(|id| *id != 4),
                "repeated-legend" => arena[0].children.push(4),
                _ => {}
            }
            let mut out = String::new();
            render_node(&mut out, &arena, 0, &mut |emitted| {
                emitted.observe_series_paint(&plan, &mut receipt)
            })
            .unwrap();
            assert_eq!(
                plan.record_terminal(receipt),
                mutation == "none",
                "{mutation}"
            );
            let evidence = plan.finish_evidence();
            assert_eq!(
                evidence.applied().len(),
                usize::from(mutation == "none"),
                "{mutation}"
            );
        }
    }

    #[test]
    fn xychart_shadow_receipt_requires_each_written_definition_and_exact_filter_reference() {
        use crate::diagram_theme::{
            EffectGraph, EffectInput, EffectPrimitive, SvgFilterRegion, SvgShadowEffect,
            SvgShadowEvidenceRecorder, ThemeColorValue,
        };
        let effect = std::sync::Arc::new(
            SvgShadowEffect::from_graph(
                &EffectGraph::new(
                    "glow",
                    [
                        EffectPrimitive::DropShadow {
                            input: EffectInput::SourceGraphic,
                            offset_x: 2.0,
                            offset_y: 3.0,
                            blur_radius: 1.0,
                            spread: 0.0,
                            color: ThemeColorValue::parse("#ff0000").unwrap(),
                        },
                        EffectPrimitive::DropShadow {
                            input: EffectInput::Previous,
                            offset_x: -2.0,
                            offset_y: -3.0,
                            blur_radius: 2.0,
                            spread: 0.0,
                            color: ThemeColorValue::parse("#0000ff").unwrap(),
                        },
                    ],
                )
                .unwrap(),
            )
            .unwrap(),
        );
        for mutation in [
            "none",
            "missing-filter",
            "changed-filter",
            "missing-later-filter",
            "missing-definition",
            "detached",
            "repeated",
            "unwritten",
        ] {
            let recorder = SvgShadowEvidenceRecorder::default();
            let mut arena = vec![node("g")];
            for terminal in 0..2 {
                let mut mark = node("path");
                mark.attr("d", "M0,0 L20,0");
                let filter_id = format!("chart-{terminal}");
                mark.attr("filter", format!("url(#{filter_id})"));
                mark.shadow = Some(Box::new(TerminalShadow {
                    effect: effect.clone(),
                    filter_id,
                    region: SvgFilterRegion::try_bounded_user_space(-20.0, -20.0, 60.0, 40.0)
                        .unwrap(),
                }));
                push_child(&mut arena, 0, mark);
            }
            match mutation {
                "missing-filter" => arena[1].attrs.retain(|(name, _)| *name != "filter"),
                "changed-filter" => {
                    arena[1]
                        .attrs
                        .iter_mut()
                        .find(|(name, _)| *name == "filter")
                        .unwrap()
                        .1 = "url(#other)".into()
                }
                "missing-later-filter" => arena[2].attrs.retain(|(name, _)| *name != "filter"),
                "missing-definition" => arena[2].shadow = None,
                "detached" => {
                    arena[0].children.pop();
                }
                "repeated" => arena[0].children.push(2),
                _ => {}
            }
            if mutation != "unwritten" {
                render_node(&mut String::new(), &arena, 0, &mut |node| {
                    node.observe_shadow(&recorder)
                })
                .unwrap();
            } else {
                let mut sink = RejectAfterWrites {
                    allowed_writes: 1,
                    ..Default::default()
                };
                assert!(
                    render_node(&mut sink, &arena, 0, &mut |node| node
                        .observe_shadow(&recorder))
                    .is_err()
                );
            }
            assert_eq!(
                recorder.finish().is_some_and(
                    |receipt| receipt.filter_count() == 2 && receipt.reference_count() == 2
                ),
                mutation == "none",
                "{mutation}"
            );
        }
    }
}
