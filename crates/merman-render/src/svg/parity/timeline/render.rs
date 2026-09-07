use super::super::theme::TimelineTheme;
use super::super::*;
use crate::model::{TimelineLineLayout, TimelineNodeLayout, TimelineTaskLayout};
use merman_core::diagrams::timeline::TimelineDiagramRenderModel;

fn timeline_css(
    diagram_id: impl Copy + std::fmt::Display,
    effective_config: &serde_json::Value,
    theme: &TimelineTheme,
    resolved_font_family_css: &str,
    resolved_font_size_css: &str,
) -> TimelineCss {
    // Keep `:root` last (matches upstream Mermaid timeline SVG baselines).
    let parts = info_css_parts_with_resolved_typography(
        diagram_id,
        effective_config,
        resolved_font_family_css,
        resolved_font_size_css,
    );
    let root_rule = parts.root_rule;
    let root_typography_emitted = !root_rule.is_empty();
    let mut out = parts.css_prefix;

    let _ = write!(&mut out, r#"#{} .edge{{stroke-width:3;}}"#, diagram_id);
    for (i, section_theme) in theme.sections.iter().enumerate() {
        let section = i as i64 - 1;
        let sw = 17 - 3 * (i as i64);

        if theme.is_redux_theme {
            let border_color = theme
                .border_colors
                .get(i)
                .cloned()
                .unwrap_or_else(|| theme.node_border.clone());
            let redux_fill = if theme.is_color_theme && !theme.is_dark_theme {
                border_color.clone()
            } else {
                theme.main_bkg.clone()
            };
            let redux_stroke = if theme.is_color_theme {
                border_color
            } else {
                theme.node_border.clone()
            };
            let _ = write!(
                &mut out,
                r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle{{fill:{};stroke:{};stroke-width:{};filter:{};}}#{} .section-{} text{{fill:{};font-weight:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .lineWrapper line{{stroke:{};stroke-width:{};}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:{};}}#{} .disabled text{{fill:{};}}"#,
                diagram_id,
                section,
                diagram_id,
                section,
                diagram_id,
                section,
                redux_fill,
                redux_stroke,
                theme.stroke_width,
                scoped_svg_url(diagram_id, "drop-shadow"),
                diagram_id,
                section,
                theme.node_border,
                theme.font_weight,
                diagram_id,
                section,
                section_theme.c_scale_label,
                diagram_id,
                section,
                section_theme.c_scale,
                diagram_id,
                section,
                sw,
                diagram_id,
                section,
                section_theme.c_scale_inv,
                diagram_id,
                theme.node_border,
                theme.stroke_width,
                diagram_id,
                diagram_id,
                diagram_id,
                theme.disabled_fill,
                diagram_id,
                theme.disabled_text_fill,
            );
        } else {
            let _ = write!(
                &mut out,
                r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle,#{} .section-{} path{{fill:{};}}#{} .section-{} text{{fill:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .lineWrapper line{{stroke:{};}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:{};}}#{} .disabled text{{fill:{};}}"#,
                diagram_id,
                section,
                diagram_id,
                section,
                diagram_id,
                section,
                diagram_id,
                section,
                section_theme.c_scale,
                diagram_id,
                section,
                section_theme.c_scale_label,
                diagram_id,
                section,
                section_theme.c_scale_label,
                diagram_id,
                section,
                section_theme.c_scale,
                diagram_id,
                section,
                sw,
                diagram_id,
                section,
                section_theme.c_scale_inv,
                diagram_id,
                section_theme.c_scale_label,
                diagram_id,
                diagram_id,
                diagram_id,
                theme.disabled_fill,
                diagram_id,
                theme.disabled_text_fill,
            );
        }
    }

    let _ = write!(
        &mut out,
        r#"#{} .section-root rect,#{} .section-root path,#{} .section-root circle{{fill:{};}}#{} .section-root text{{fill:{};}}#{} .icon-container{{height:100%;display:flex;justify-content:center;align-items:center;}}#{} .edge{{fill:none;}}#{} .eventWrapper{{filter:brightness(120%);}}"#,
        diagram_id,
        diagram_id,
        diagram_id,
        theme.root_fill,
        diagram_id,
        theme.root_label,
        diagram_id,
        diagram_id,
        diagram_id
    );

    out.push_str(&root_rule);
    TimelineCss {
        css: out,
        font_family_css: parts.font_family,
        font_size_css: parts.font_size_css,
        base_typography_emitted: parts.base_typography_emitted,
        root_typography_emitted,
    }
}

struct TimelineCss {
    css: String,
    font_family_css: String,
    font_size_css: String,
    base_typography_emitted: bool,
    root_typography_emitted: bool,
}

fn write_timeline_connector(
    out: &mut impl SvgOutput,
    connector: &TimelineLineLayout,
    diagram_id: Option<SvgDiagramId<'_>>,
) -> Result<()> {
    fn write_connector(
        out: &mut impl SvgOutput,
        connector: &TimelineLineLayout,
        marker_end: impl std::fmt::Display,
    ) {
        let _ = write!(
            out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="{marker_end}" stroke-dasharray="5,5"/></g>"#,
            x1 = fmt(connector.x1),
            y1 = fmt(connector.y1),
            x2 = fmt(connector.x2),
            y2 = fmt(connector.y2),
        );
    }

    match diagram_id {
        Some(diagram_id) => write_connector(
            out,
            connector,
            escape_attr_display(scoped_svg_url(diagram_id, "arrowhead")),
        ),
        None => write_connector(out, connector, "url(#arrowhead)"),
    }
    out.checkpoint()
}

fn write_timeline_event_wrapper_open<'a>(
    out: &mut impl SvgOutput,
    event: &TimelineNodeLayout,
    opacity_token: Option<&'a str>,
) -> Result<(Option<&'a str>, bool)> {
    let output_start = out.len();
    match opacity_token {
        Some(opacity_token) => {
            let _ = write!(
                out,
                r#"<g class="eventWrapper" opacity="{opacity}" transform="translate({x}, {y})">"#,
                opacity = opacity_token,
                x = fmt(event.x),
                y = fmt(event.y)
            );
        }
        None => {
            let _ = write!(
                out,
                r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
                x = fmt(event.x),
                y = fmt(event.y)
            );
        }
    }
    out.checkpoint()?;
    let opening = out
        .as_str()
        .get(output_start..)
        .and_then(|output| output.split_once('>').map(|(opening, _)| opening))
        .unwrap_or_default();
    let actual_opacity = opening
        .split_once(" opacity=\"")
        .and_then(|(_, rest)| rest.split_once('"').map(|(value, _)| value));
    let opacity_matches = opening.contains("class=\"eventWrapper\"")
        && match opacity_token {
            Some(expected) => actual_opacity == Some(expected),
            None => actual_opacity.is_none(),
        };
    Ok((opacity_token, opacity_matches))
}

fn negative_radius_token(token: &str, radius_value: f64) -> String {
    if radius_value == 0.0 {
        "0".to_string()
    } else {
        format!("-{token}")
    }
}

fn observe_timeline_event_radius<'a>(
    output: &str,
    event: &TimelineNodeLayout,
    event_radius: Option<(&'a str, f64)>,
    is_redux_theme: bool,
) -> (Option<&'a str>, bool) {
    let Some(path_d) = output
        .split_once("class=\"node-bkg node-undefined\" d=\"")
        .and_then(|(_, rest)| rest.split_once('"').map(|(path, _)| path))
    else {
        return (None, false);
    };

    let w = event.width.max(1.0);
    let h = event.height.max(1.0);
    let expected_tokens = match event_radius {
        Some((radius_token, radius_value)) => {
            let negative_token = negative_radius_token(radius_token, radius_value);
            vec![
                "M0".to_string(),
                fmt(h - radius_value).to_string(),
                format!("v{}", fmt(-h + 2.0 * radius_value)),
                format!("q0,{negative_token}"),
                format!("{radius_token},{negative_token}"),
                format!("h{}", fmt(w - 2.0 * radius_value)),
                format!("q{radius_token},0"),
                format!("{radius_token},{radius_token}"),
                format!("v{}", fmt(h - radius_value)),
                "H0".to_string(),
                "Z".to_string(),
            ]
        }
        None if is_redux_theme => {
            let radius = crate::timeline::MERMAID_EVENT_RADIUS_PX;
            vec![
                "M0".to_string(),
                fmt(h - radius).to_string(),
                format!("v{}", fmt(-(h - radius))),
                format!("h{}", fmt(w)),
                format!("v{}", fmt(h)),
                "H0".to_string(),
                "Z".to_string(),
            ]
        }
        None => {
            let radius = crate::timeline::MERMAID_EVENT_RADIUS_PX;
            vec![
                "M0".to_string(),
                fmt(h - radius).to_string(),
                format!("v{}", fmt(-h + 2.0 * radius)),
                format!("q0,-{}", crate::timeline::MERMAID_EVENT_RADIUS_TOKEN),
                format!(
                    "{token},-{token}",
                    token = crate::timeline::MERMAID_EVENT_RADIUS_TOKEN
                ),
                format!("h{}", fmt(w - 2.0 * radius)),
                format!(
                    "q{token},0",
                    token = crate::timeline::MERMAID_EVENT_RADIUS_TOKEN
                ),
                format!(
                    "{token},{token}",
                    token = crate::timeline::MERMAID_EVENT_RADIUS_TOKEN
                ),
                format!("v{}", fmt(h - radius)),
                "H0".to_string(),
                "Z".to_string(),
            ]
        }
    };
    let actual_tokens = path_d.split_whitespace().collect::<Vec<_>>();
    let matches = actual_tokens.len() == expected_tokens.len()
        && actual_tokens
            .iter()
            .zip(expected_tokens.iter())
            .all(|(actual, expected)| *actual == expected);

    match event_radius {
        Some((token, _)) => (matches.then_some(token), matches),
        None if is_redux_theme => (None, matches),
        None => (
            matches.then_some(crate::timeline::MERMAID_EVENT_RADIUS_TOKEN),
            matches,
        ),
    }
}

fn observe_timeline_event_fill(output: &str, expected_fill: Option<&str>) -> bool {
    let Some(path_opening) = output
        .split_once("<path ")
        .and_then(|(_, rest)| rest.split_once("/>").map(|(opening, _)| opening))
        .filter(|opening| opening.contains("class=\"node-bkg node-undefined\""))
    else {
        return false;
    };

    let actual_style = path_opening
        .split_once(" style=\"")
        .and_then(|(_, rest)| rest.split_once('"').map(|(style, _)| style));
    let actual_fill = actual_style.and_then(|style| {
        style.split(';').find_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            property
                .trim()
                .eq_ignore_ascii_case("fill")
                .then_some(value.trim())
        })
    });
    match expected_fill {
        Some(fill) => actual_fill == Some(escape_attr(fill).as_ref()),
        None => actual_fill.is_none(),
    }
}

struct TimelineEventEmissionState<'a> {
    theme: &'a crate::timeline::TimelineEventTheme,
    next_event_index: usize,
    receipt: Option<crate::timeline::TimelineEventThemeReceipt>,
}

struct TimelineTypographyEmissionState<'a> {
    plan: &'a crate::timeline::TimelineTypographyThemePlan,
    receipt: Option<crate::timeline::TimelineTypographyThemeReceipt<'a>>,
}

impl<'a> TimelineTypographyEmissionState<'a> {
    fn new(
        plan: &'a crate::timeline::TimelineTypographyThemePlan,
        layout: &'a TimelineDiagramLayout,
    ) -> Self {
        Self {
            plan,
            receipt: plan.begin_terminal_receipt(layout),
        }
    }

    fn record_css(&mut self, css: &TimelineCss) {
        if let Some(receipt) = self.receipt.as_mut() {
            receipt.record_css_emission(
                &css.font_family_css,
                &css.font_size_css,
                css.base_typography_emitted,
                css.root_typography_emitted,
            );
        }
    }

    fn record_text_run(&mut self, text: &str) {
        if let Some(receipt) = self.receipt.as_mut() {
            receipt.record_text_run(text);
        }
    }

    fn record_base_text_run(&mut self, text: &str) {
        if let Some(receipt) = self.receipt.as_mut() {
            receipt.record_base_text_run(text);
        }
    }

    fn finish(self) -> Result<()> {
        if self
            .receipt
            .is_some_and(|receipt| !self.plan.record_terminal(receipt))
        {
            return Err(crate::Error::InvalidModel {
                message: "Timeline typography receipt did not match the terminal SVG".to_string(),
            });
        }
        Ok(())
    }
}

impl<'a> TimelineEventEmissionState<'a> {
    fn new(theme: &'a crate::timeline::TimelineEventTheme, is_redux_theme: bool) -> Self {
        Self {
            theme,
            next_event_index: 0,
            receipt: theme.begin_terminal_receipt(is_redux_theme),
        }
    }

    fn write_event_wrapper_open(
        &self,
        out: &mut impl SvgOutput,
        event: &TimelineNodeLayout,
    ) -> Result<(usize, Option<&'a str>, bool)> {
        let event_index = self.next_event_index;
        let expected_opacity_token = self.theme.opacity_token_for_event(event_index);
        let (emitted_opacity_token, emitted_opacity_matches) =
            write_timeline_event_wrapper_open(out, event, expected_opacity_token)?;
        Ok((event_index, emitted_opacity_token, emitted_opacity_matches))
    }

    fn palette_slot_for_node(&self, node: &TimelineNodeLayout) -> Option<usize> {
        self.theme.palette_slot_for_section(&node.section_class)
    }

    fn palette_fill_for_slot(&self, slot: usize) -> Option<&str> {
        self.theme.palette_fill_for_slot(slot)
    }

    fn fill_for_event(&self, event_index: usize) -> Option<&str> {
        self.theme.fill_for_event(event_index)
    }

    fn palette_line_stroke_for_slot(&self, slot: usize) -> Option<&str> {
        self.theme.palette_line_stroke_for_slot(slot)
    }

    fn record_palette_node(
        &mut self,
        slot: Option<usize>,
        emitted_fill: Option<&str>,
        emitted_line_stroke: Option<&str>,
    ) {
        if let Some(receipt) = self.receipt.as_mut() {
            receipt.record_palette_node(slot, emitted_fill, emitted_line_stroke);
        }
    }

    fn record_event_terminal(
        &mut self,
        event_index: usize,
        emitted_opacity_token: Option<&str>,
        emitted_opacity_matches: bool,
        emitted_radius_token: Option<&str>,
        emitted_radius_geometry_matches: bool,
        emitted_fill: Option<&str>,
        emitted_fill_matches: bool,
    ) {
        if let Some(receipt) = self.receipt.as_mut() {
            receipt.record_checkpointed_event(
                event_index,
                emitted_opacity_token,
                emitted_opacity_matches,
                emitted_radius_token,
                emitted_radius_geometry_matches,
                emitted_fill,
                emitted_fill_matches,
            );
        }
        self.next_event_index += 1;
    }

    fn finish(self) -> Result<()> {
        if self
            .receipt
            .is_some_and(|receipt| !self.theme.record_terminal(receipt))
        {
            return Err(crate::Error::InvalidModel {
                message: "Timeline event terminal receipt did not match the terminal SVG"
                    .to_string(),
            });
        }
        Ok(())
    }
}

pub(crate) fn render_timeline_diagram_svg_model(
    layout: &TimelineDiagramLayout,
    _model: &TimelineDiagramRenderModel,
    event_theme: &crate::timeline::TimelineEventTheme,
    typography_theme: &crate::timeline::TimelineTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_timeline_diagram_svg_inner(
        layout,
        event_theme,
        typography_theme,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

fn render_timeline_diagram_svg_inner(
    layout: &TimelineDiagramLayout,
    event_theme: &crate::timeline::TimelineEventTheme,
    typography_theme: &crate::timeline::TimelineTypographyThemePlan,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let theme = MermaidThemeAdapter::new(effective_config).timeline();
    let is_redux_theme = theme.is_redux_theme;

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });

    fn node_line_class(section_class: &str) -> String {
        let rest = section_class
            .strip_prefix("section-")
            .unwrap_or(section_class);
        format!("node-line-{rest}")
    }

    fn render_node(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        n: &crate::model::TimelineNodeLayout,
        is_redux_theme: bool,
        is_event: bool,
        event_index: Option<usize>,
        event_radius: Option<(&str, f64)>,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        let node_local_id = format!("node-{node_count}");
        let node_id = scoped_svg_id(diagram_id, &node_local_id);
        *node_count += 1;
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let rd = event_radius.map_or(crate::timeline::MERMAID_EVENT_RADIUS_PX, |(_, value)| value);
        let d = if let Some((radius_token, radius_value)) = event_radius {
            let negative_radius = negative_radius_token(radius_token, radius_value);
            format!(
                "M0 {y0} v{v1} q0,{negative_radius} {rd},{negative_radius} h{hw} q{rd},0 {rd},{rd} v{v2} H0 Z",
                y0 = fmt(h - radius_value),
                v1 = fmt(-h + 2.0 * radius_value),
                rd = radius_token,
                hw = fmt(w - 2.0 * radius_value),
                v2 = fmt(h - radius_value),
            )
        } else if is_redux_theme {
            format!(
                "M0 {y0} v{v1} h{w} v{h} H0 Z",
                y0 = fmt(h - rd),
                v1 = fmt(-(h - rd)),
                w = fmt(w),
                h = fmt(h),
            )
        } else {
            format!(
                "M0 {y0} v{v1} q0,-{radius} {radius},-{radius} h{hw} q{radius},0 {radius},{radius} v{v2} H0 Z",
                y0 = fmt(h - rd),
                v1 = fmt(-h + 2.0 * rd),
                radius = fmt(crate::timeline::MERMAID_EVENT_RADIUS_PX),
                hw = fmt(w - 2.0 * rd),
                v2 = fmt(h - rd),
            )
        };

        let node_palette_slot = event_emission.palette_slot_for_node(n);
        let palette_fill = node_palette_slot
            .and_then(|slot| event_emission.palette_fill_for_slot(slot))
            .map(str::to_owned);
        let palette_line_stroke = node_palette_slot
            .and_then(|slot| event_emission.palette_line_stroke_for_slot(slot))
            .map(str::to_owned);
        let emitted_palette_slot = palette_fill.as_ref().and(node_palette_slot);
        let direct_fill = event_index
            .and_then(|event_index| event_emission.fill_for_event(event_index))
            .map(str::to_owned);
        let effective_fill = direct_fill.as_deref().or(palette_fill.as_deref());
        let emitted_palette_fill = if direct_fill.is_none() {
            palette_fill.as_deref()
        } else {
            None
        };
        let fill_style = effective_fill
            .as_deref()
            .map(|fill| format!(r#" style="fill:{};""#, escape_attr(fill)))
            .unwrap_or_default();

        let _ = write!(
            out,
            r#"<g class="timeline-node {section_class}">"#,
            section_class = escape_attr(&n.section_class)
        );
        out.checkpoint()?;
        out.push_str("<g>");
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"<path id="{node_id}" class="node-bkg node-undefined" d="{d}"{fill_style}/>"#,
            node_id = escape_attr_display(node_id),
            d = escape_attr(&d),
            fill_style = fill_style,
        );
        out.checkpoint()?;
        let emitted_line_stroke = if !is_redux_theme {
            let line_style = palette_line_stroke
                .as_deref()
                .map(|stroke| format!(r#" style="stroke:{};""#, escape_attr(stroke)))
                .unwrap_or_default();
            let _ = write!(
                out,
                r#"<line class="{line_class}"{line_style} x1="0" y1="{y}" x2="{x2}" y2="{y}"/>"#,
                line_class = escape_attr(&node_line_class(&n.section_class)),
                line_style = line_style,
                y = fmt(h),
                x2 = fmt(w)
            );
            out.checkpoint()?;
            palette_line_stroke.as_deref()
        } else {
            None
        };
        out.push_str("</g>");
        out.checkpoint()?;

        let tx = w / 2.0;
        let ty = if is_redux_theme {
            if is_event {
                n.padding / 2.0 + 3.0
            } else {
                n.padding
            }
        } else {
            n.padding / 2.0
        };
        let _ = write!(
            out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(tx),
            y = fmt(ty)
        );
        out.checkpoint()?;
        out.push_str(r#"<text dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle">"#);
        out.checkpoint()?;
        for (idx, line) in n.label_lines.iter().enumerate() {
            let dy = if idx == 0 { "1em" } else { "1.1em" };
            let _ = write!(
                out,
                r#"<tspan x="0" dy="{dy}">{text}</tspan>"#,
                dy = dy,
                text = escape_xml(line)
            );
            out.checkpoint()?;
            typography_emission.record_base_text_run(line);
        }
        out.push_str("</text></g></g>");
        out.checkpoint()?;
        event_emission.record_palette_node(
            emitted_palette_slot,
            emitted_palette_fill,
            emitted_line_stroke,
        );
        options.checkpoint_emit()
    }

    fn render_event_node<'a>(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        event: &TimelineNodeLayout,
        is_redux_theme: bool,
        event_index: usize,
        expected_effective_fill: Option<&str>,
        event_radius: Option<(&'a str, f64)>,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<(Option<&'a str>, bool, bool)> {
        let output_start = out.len();
        render_node(
            out,
            diagram_id,
            node_count,
            event,
            is_redux_theme,
            true,
            Some(event_index),
            event_radius,
            event_emission,
            typography_emission,
            options,
        )?;
        let output = out.as_str().get(output_start..).unwrap_or_default();
        let radius_observation =
            observe_timeline_event_radius(output, event, event_radius, is_redux_theme);
        Ok((
            radius_observation.0,
            radius_observation.1,
            observe_timeline_event_fill(output, expected_effective_fill),
        ))
    }

    fn render_event(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        event: &TimelineNodeLayout,
        is_redux_theme: bool,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        let (event_index, emitted_opacity_token, emitted_opacity_matches) =
            event_emission.write_event_wrapper_open(out, event)?;
        let event_radius = event_emission.theme.radius_for_event(event_index);
        let event_fill = event_emission
            .fill_for_event(event_index)
            .map(str::to_owned);
        let effective_fill = event_fill.clone().or_else(|| {
            event_emission
                .palette_slot_for_node(event)
                .and_then(|slot| event_emission.palette_fill_for_slot(slot))
                .map(str::to_owned)
        });
        let (emitted_radius_token, emitted_radius_geometry_matches, emitted_fill_matches) =
            render_event_node(
                out,
                diagram_id,
                node_count,
                event,
                is_redux_theme,
                event_index,
                effective_fill.as_deref(),
                event_radius,
                event_emission,
                typography_emission,
                options,
            )?;
        out.push_str("</g>");
        out.checkpoint()?;
        event_emission.record_event_terminal(
            event_index,
            emitted_opacity_token,
            emitted_opacity_matches,
            emitted_radius_token,
            emitted_radius_geometry_matches,
            event_fill.as_deref(),
            emitted_fill_matches,
        );
        Ok(())
    }

    fn render_task(
        out: &mut impl SvgOutput,
        diagram_id: SvgDiagramId<'_>,
        node_count: &mut usize,
        task: &TimelineTaskLayout,
        direction: merman_core::diagrams::timeline::TimelineDirection,
        is_redux_theme: bool,
        event_emission: &mut TimelineEventEmissionState<'_>,
        typography_emission: &mut TimelineTypographyEmissionState<'_>,
        options: &SvgExecution<'_>,
    ) -> Result<()> {
        let node = &task.node;
        let _ = write!(
            out,
            r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        out.checkpoint()?;
        render_node(
            out,
            diagram_id,
            node_count,
            node,
            is_redux_theme,
            false,
            None,
            None,
            event_emission,
            typography_emission,
            options,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        match direction {
            merman_core::diagrams::timeline::TimelineDirection::LeftToRight => {
                for connector in &task.connectors {
                    options.checkpoint_emit()?;
                    write_timeline_connector(out, connector, Some(diagram_id))?;
                }
                for event in &task.events {
                    render_event(
                        out,
                        diagram_id,
                        node_count,
                        event,
                        is_redux_theme,
                        event_emission,
                        typography_emission,
                        options,
                    )?;
                }
            }
            merman_core::diagrams::timeline::TimelineDirection::TopDown => {
                for (index, event) in task.events.iter().enumerate() {
                    render_event(
                        out,
                        diagram_id,
                        node_count,
                        event,
                        is_redux_theme,
                        event_emission,
                        typography_emission,
                        options,
                    )?;
                    if let Some(connector) = task.connectors.get(index) {
                        options.checkpoint_emit()?;
                        write_timeline_connector(out, connector, None)?;
                    }
                }
                for connector in task.connectors.iter().skip(task.events.len()) {
                    options.checkpoint_emit()?;
                    write_timeline_connector(out, connector, None)?;
                }
            }
        }
        Ok(())
    }

    let root_bounds = root_svg::DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        0.0,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(root_bounds.width));

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::TIMELINE, diagram_id)
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
                    ..root_svg::RootChrome::new(diagram_id, "timeline")
                },
            )?;
    out.checkpoint()?;
    let arrowhead_id =
        if layout.direction == merman_core::diagrams::timeline::TimelineDirection::TopDown {
            "undefined-arrowhead".to_string()
        } else {
            scoped_svg_id(diagram_id, "arrowhead").to_string()
        };
    options.checkpoint_emit()?;

    // Mermaid's vertical renderer lowers the activity axis to the first root child and invokes
    // marker initialization without a diagram id. Preserve both observable source behaviors.
    if layout.direction == merman_core::diagrams::timeline::TimelineDirection::TopDown {
        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="url(#arrowhead)"/></g>"#,
            x1 = fmt(layout.activity_line.x1),
            y1 = fmt(layout.activity_line.y1),
            x2 = fmt(layout.activity_line.x2),
            y2 = fmt(layout.activity_line.y2),
        );
        out.checkpoint()?;
    }

    let mut typography_emission = TimelineTypographyEmissionState::new(typography_theme, layout);
    let css = timeline_css(
        diagram_id,
        effective_config,
        &theme,
        typography_theme.font_family_css(),
        typography_theme.font_size_css(),
    );
    options.checkpoint_emit()?;
    let _ = write!(&mut out, r#"<style>{}</style>"#, css.css);
    out.checkpoint()?;
    typography_emission.record_css(&css);
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.checkpoint()?;
    let mut node_count = 0usize;
    let mut event_emission = TimelineEventEmissionState::new(event_theme, is_redux_theme);
    let _ = write!(
        &mut out,
        r#"<defs><marker id="{}" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
        escape_attr(&arrowhead_id)
    );
    out.checkpoint()?;

    for section in &layout.sections {
        options.checkpoint_emit()?;
        let node = &section.node;
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        out.checkpoint()?;
        render_node(
            &mut out,
            diagram_id,
            &mut node_count,
            node,
            is_redux_theme,
            false,
            None,
            None,
            &mut event_emission,
            &mut typography_emission,
            options,
        )?;
        out.push_str("</g>");
        out.checkpoint()?;

        for task in &section.tasks {
            options.checkpoint_emit()?;
            render_task(
                &mut out,
                diagram_id,
                &mut node_count,
                task,
                layout.direction,
                is_redux_theme,
                &mut event_emission,
                &mut typography_emission,
                options,
            )?;
        }
    }

    for task in &layout.orphan_tasks {
        options.checkpoint_emit()?;
        render_task(
            &mut out,
            diagram_id,
            &mut node_count,
            task,
            layout.direction,
            is_redux_theme,
            &mut event_emission,
            &mut typography_emission,
            options,
        )?;
    }

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="4ex" font-weight="bold" y="{y}">{text}</text>"#,
            x = fmt(layout.title_x),
            y = fmt(layout.title_y),
            text = escape_xml(title)
        );
        out.checkpoint()?;
        typography_emission.record_text_run(title);
    }

    if layout.direction == merman_core::diagrams::timeline::TimelineDirection::LeftToRight {
        options.checkpoint_emit()?;
        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="{marker_end}"/></g>"#,
            x1 = fmt(layout.activity_line.x1),
            y1 = fmt(layout.activity_line.y1),
            x2 = fmt(layout.activity_line.x2),
            y2 = fmt(layout.activity_line.y2),
            marker_end = scoped_svg_url(diagram_id, "arrowhead"),
        );
        out.checkpoint()?;
    }

    out.push_str("</svg>\n");
    options.checkpoint_emit()?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    event_emission.finish()?;
    typography_emission.finish()?;
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::model::{Bounds, TimelineDiagramLayout, TimelineLineLayout, TimelineNodeLayout};
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

    fn test_event_node(width: f64, height: f64) -> TimelineNodeLayout {
        TimelineNodeLayout {
            x: 0.0,
            y: 0.0,
            width,
            height,
            content_width: width,
            padding: 0.0,
            section_class: "section-0".to_string(),
            label: "event".to_string(),
            label_lines: vec!["event".to_string()],
            kind: "event".to_string(),
        }
    }

    #[test]
    fn timeline_radius_observer_rejects_incomplete_custom_path() {
        let event = test_event_node(100.0, 50.0);
        let output = r#"<path class="node-bkg node-undefined" d="q0,-12 12,-12 q12,0 12,12"/>"#;

        let (token, matches) =
            observe_timeline_event_radius(output, &event, Some(("12", 12.0)), false);

        assert_eq!(token, None);
        assert!(!matches);
    }

    #[test]
    fn timeline_radius_observer_rejects_truncated_redux_path() {
        let event = test_event_node(100.0, 50.0);
        let output = r#"<path class="node-bkg node-undefined" d="M0 0"/>"#;

        let (token, matches) = observe_timeline_event_radius(output, &event, None, true);

        assert_eq!(token, None);
        assert!(!matches);
    }

    #[test]
    fn timeline_connector_stops_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();
        let connector = TimelineLineLayout {
            kind: "connector".to_string(),
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 10.0,
        };

        let error = write_timeline_connector(&mut out, &connector, None)
            .expect_err("the rejecting sink must stop Timeline connector emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Timeline connector emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn timeline_root_honors_disabled_max_width() {
        let layout = TimelineDiagramLayout {
            direction: merman_core::diagrams::timeline::TimelineDirection::LeftToRight,
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 320.0,
                max_y: 180.0,
            }),
            left_margin: 150.0,
            base_x: 50.0,
            base_y: 50.0,
            pre_title_box_width: 0.0,
            sections: Vec::new(),
            orphan_tasks: Vec::new(),
            activity_line: TimelineLineLayout {
                kind: "activity".to_string(),
                x1: 50.0,
                y1: 0.0,
                x2: 320.0,
                y2: 0.0,
            },
            title: None,
            title_x: 0.0,
            title_y: 20.0,
            use_max_width: false,
        };
        let options = SvgRenderOptions {
            diagram_id: Some("timelineFixed".to_string()),
            ..Default::default()
        };
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session");
        let debug = SvgDebugOptions::default();
        let execution =
            SvgExecution::unthemed_for_test(&options, &debug, &session, DiagramFamilyId::TIMELINE)
                .expect("SVG execution");

        let svg = render_timeline_diagram_svg_inner(
            &layout,
            &crate::timeline::TimelineEventTheme::baseline(),
            &crate::timeline::TimelineTypographyThemePlan::resolve(
                None,
                &merman_core::MermaidConfig::default(),
            ),
            &serde_json::json!({}),
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            &execution,
        )
        .unwrap();
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="320""#), "{root_open}");
        assert!(root_open.contains(r#"height="180""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="0 0 320 180""#),
            "{root_open}"
        );
        assert!(
            root_open.contains(r#"style="background-color: white;""#),
            "{root_open}"
        );
        assert!(!root_open.contains("max-width"), "{root_open}");
    }
}
