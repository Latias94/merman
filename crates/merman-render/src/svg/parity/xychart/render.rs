use super::super::*;
use merman_core::diagrams::xychart::XyChartDiagramRenderModel;

// XYChart diagram SVG renderer implementation (split from parity.rs).

/// Mermaid uses JavaScript's `String#length` for XYChart bar labels, which counts UTF-16 code
/// units rather than Unicode scalar values.
fn javascript_string_length(text: &str) -> f64 {
    text.encode_utf16().count() as f64
}

/// Computes the result of Mermaid's one-pixel decrement loop without iterating once per pixel.
///
/// The upstream renderer decrements a candidate font size until it fits. That is observable for
/// ordinary dimensions, but a finite value such as `1e308` cannot make numerical progress when
/// subtracting one, so the browser algorithm never terminates. The fit predicates are monotonic;
/// solving their upper bound directly preserves the same discrete candidate for normal values and
/// makes extreme finite dimensions terminate in constant time.
fn font_size_after_unit_decrements(initial: f64, maximum_that_fits: f64) -> f64 {
    if !(initial.is_finite() && initial > 0.0) || maximum_that_fits.is_nan() {
        return 0.0;
    }
    if maximum_that_fits >= initial {
        return initial;
    }
    if maximum_that_fits <= 0.0 {
        return 0.0;
    }

    let decrements = (initial - maximum_that_fits).ceil();
    let candidate = initial - decrements;
    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        0.0
    }
}

fn horizontal_label_font_size(item_width: f64, label: &str, initial: f64, inset_px: f64) -> f64 {
    let denominator = javascript_string_length(label) * 0.7;
    let maximum_that_fits = if denominator > 0.0 {
        (item_width - inset_px) / denominator
    } else {
        f64::INFINITY
    };
    font_size_after_unit_decrements(initial, maximum_that_fits)
}

fn vertical_label_font_size(
    item_width: f64,
    item_height: f64,
    label: &str,
    initial: f64,
    y_offset: f64,
) -> f64 {
    let denominator = javascript_string_length(label) * 0.7;
    let horizontal_maximum = if denominator > 0.0 {
        item_width / denominator
    } else {
        f64::INFINITY
    };
    let maximum_that_fits = horizontal_maximum.min(item_height - y_offset);
    font_size_after_unit_decrements(initial, maximum_that_fits)
}

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
}

impl Node {
    fn attr(&mut self, name: &'static str, value: impl Into<String>) {
        self.attrs.push((name, value.into()));
    }

    fn observe_series_paint(
        &self,
        plan: &crate::xychart::XyChartSeriesPaintPlan,
        receipt: &mut crate::xychart::XyChartSeriesPaintReceipt,
    ) {
        let attribute = |name| {
            self.attrs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(key, value)| (*key, value.as_str()))
        };
        match self.series_terminal {
            Some(SeriesTerminal::Bar { plot, mark }) => {
                receipt.record_bar_mark(plan, plot, mark, attribute("fill"), attribute("stroke"))
            }
            Some(SeriesTerminal::Line { plot, mark }) => {
                receipt.record_line_mark(plan, plot, mark, attribute("stroke"))
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
    Bar { plot: usize, mark: usize },
    Line { plot: usize, mark: usize },
    LineLabel { plot: usize, label: usize },
}

pub(crate) fn render_xychart_diagram_svg(
    layout: &XyChartDiagramLayout,
    model: &XyChartDiagramRenderModel,
    series_paint: &crate::xychart::XyChartSeriesPaintPlan,
    typography_theme: &crate::xychart::XyChartTypographyThemePlan,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
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

    fn data_label_color(effective_config: &serde_json::Value) -> String {
        let configured = config_string(
            effective_config,
            &["themeVariables", "xyChart", "dataLabelColor"],
        );
        configured
            .or_else(|| config_string(effective_config, &["themeVariables", "primaryTextColor"]))
            .unwrap_or_else(|| "black".to_string())
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
            data_label_color(effective_config),
        ))
    } else {
        None
    };
    let mut series_paint_receipt = series_paint.begin_terminal_receipt();
    let mut typography_receipt = typography_theme.begin_terminal_receipt();

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_bounds = root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height);
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
                    push_child(&mut arena, parent, n);
                }

                // Optional bar data labels (Mermaid emits these in the renderer, not the DB).
                if let Some((show_data_label_outside_bar, data_label_color)) = &data_label_config {
                    let bar_data_label_scale_factor = 0.7;
                    let bar_data_label_inset_px = 10.0;

                    #[derive(Clone)]
                    struct BarItem<'a> {
                        rect: &'a crate::model::XyChartRectData,
                        label: &'a str,
                    }

                    let mut valid_items: Vec<BarItem<'_>> = Vec::with_capacity(data.len());
                    for (idx, r) in data.iter().enumerate() {
                        let Some(label) = layout.label_data.get(idx) else {
                            continue;
                        };
                        if r.width > 0.0 && r.height > 0.0 {
                            valid_items.push(BarItem { rect: r, label });
                        }
                    }

                    if !valid_items.is_empty() {
                        if layout.chart_orientation == "horizontal" {
                            let mut min_font = f64::INFINITY;
                            for item in &valid_items {
                                let fs = horizontal_label_font_size(
                                    item.rect.width,
                                    item.label,
                                    item.rect.height * bar_data_label_scale_factor,
                                    bar_data_label_inset_px,
                                );
                                min_font = min_font.min(fs);
                            }
                            let uniform = if min_font.is_finite() { min_font } else { 0.0 }
                                .floor()
                                .max(0.0);
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
                            let mut min_font = f64::INFINITY;
                            for item in &valid_items {
                                let denominator = javascript_string_length(item.label)
                                    * bar_data_label_scale_factor;
                                let initial = if denominator <= 0.0 {
                                    0.0
                                } else {
                                    item.rect.width / denominator
                                };
                                let fs = vertical_label_font_size(
                                    item.rect.width,
                                    item.rect.height,
                                    item.label,
                                    initial,
                                    y_offset,
                                );
                                min_font = min_font.min(fs);
                            }
                            let uniform = if min_font.is_finite() { min_font } else { 0.0 }
                                .floor()
                                .max(0.0);
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
                    n.series_terminal = line_plot_index.map(|plot| SeriesTerminal::Line {
                        plot,
                        mark: mark_index,
                    });
                    push_child(&mut arena, parent, n);
                }
            }
        }
    }

    render_node(&mut out, &arena, 0, &mut |emitted| {
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
    if let Some(receipt) = typography_receipt {
        let _ = typography_theme.record_terminal(receipt);
    }
    Ok(rooted)
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

    fn upstream_decrement(initial: f64, maximum_that_fits: f64) -> f64 {
        let mut font_size = initial;
        while font_size > maximum_that_fits && font_size > 0.0 {
            font_size -= 1.0;
        }
        font_size
    }

    #[test]
    fn closed_form_font_sizing_matches_mermaid_for_normal_dimensions() {
        for (initial, maximum_that_fits) in [(10.2, 8.0), (10.0, 8.8), (8.0, 8.0), (0.5, 0.0)] {
            assert_eq!(
                font_size_after_unit_decrements(initial, maximum_that_fits)
                    .floor()
                    .max(0.0),
                upstream_decrement(initial, maximum_that_fits)
                    .floor()
                    .max(0.0),
            );
        }
    }

    #[test]
    fn huge_finite_bar_dimensions_complete_without_a_decrement_loop() {
        let font_size = horizontal_label_font_size(1e308, "123", 7e307, 10.0);

        assert!(font_size.is_finite());
        assert!(font_size > 0.0);
    }

    #[test]
    fn bar_label_length_uses_javascript_utf16_code_units() {
        assert_eq!(javascript_string_length("A"), 1.0);
        assert_eq!(javascript_string_length("\u{1F469}\u{200D}\u{1F4BB}"), 5.0);
    }
}
