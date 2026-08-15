use super::*;
use merman_core::svg_security::{
    MermaidNavigationSecurity, normalize_mermaid_tooltip_attribute, prepare_mermaid_navigation_href,
};

fn rounded_rect_rough_cache_key(
    w: f64,
    h: f64,
    radius: f64,
    seed: roughr::core::RoughJsSeed,
) -> StateRoughCacheKey {
    let radius = super::roughjs::normalized_rounded_rect_radius(w, h, radius);
    StateRoughCacheKey {
        tag: 6,
        a: w.to_bits(),
        b: h.to_bits(),
        c: radius.to_bits(),
        seed,
    }
}

pub(super) fn render_state_node_svg(
    out: &mut impl SvgOutput,
    ctx: &StateRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing: super::timing::RenderTiming,
    details: &mut StateRenderDetails,
    effect_outsets: &mut crate::state::StateEffectOutsets,
) -> Result<()> {
    let Some(node) = ctx.nodes_by_id.get(node_id).copied() else {
        return Ok(());
    };
    let Some(ln) = ctx.layout_nodes_by_id.get(node_id).copied() else {
        return Ok(());
    };
    if ln.is_cluster {
        return Ok(());
    }
    let cx = ln.x - origin_x;
    let cy = ln.y - origin_y;
    let w = ln.width.max(1.0);
    let h = ln.height.max(1.0);

    #[inline]
    fn cached_circle(
        ctx: &StateRenderCtx<'_>,
        key: StateRoughCacheKey,
        allow_cache: bool,
        build: impl FnOnce() -> String,
    ) -> Rc<String> {
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_draw_request(StateRoughGeometryKind::Circle);
        if !allow_cache {
            #[cfg(test)]
            ctx.rough_lifecycle_probe
                .record_bypass_build(StateRoughGeometryKind::Circle);
            return Rc::new(build());
        }
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_operation_lookup(StateRoughGeometryKind::Circle);
        let existing = ctx.rough_cache.get_circle(key);
        if let Some(v) = existing {
            #[cfg(test)]
            ctx.rough_lifecycle_probe
                .record_operation_hit(StateRoughGeometryKind::Circle);
            return v;
        }
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_operation_miss(StateRoughGeometryKind::Circle);
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_operation_build(StateRoughGeometryKind::Circle);
        let built = Rc::new(build());
        ctx.rough_cache.insert_circle(key, Rc::clone(&built));
        #[cfg(test)]
        state_rough_lifecycle_observe_operation_cache(ctx);
        built
    }

    #[inline]
    fn cached_paths(
        ctx: &StateRenderCtx<'_>,
        key: StateRoughCacheKey,
        allow_cache: bool,
        build: impl FnOnce() -> (String, String),
    ) -> (Rc<String>, Rc<String>) {
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_draw_request(StateRoughGeometryKind::Paths);
        if !allow_cache {
            #[cfg(test)]
            ctx.rough_lifecycle_probe
                .record_bypass_build(StateRoughGeometryKind::Paths);
            let (fill_d, stroke_d) = build();
            return (Rc::new(fill_d), Rc::new(stroke_d));
        }
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_operation_lookup(StateRoughGeometryKind::Paths);
        let existing = ctx.rough_cache.get_paths(key);
        if let Some(v) = existing {
            #[cfg(test)]
            ctx.rough_lifecycle_probe
                .record_operation_hit(StateRoughGeometryKind::Paths);
            return v;
        }
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_operation_miss(StateRoughGeometryKind::Paths);
        #[cfg(test)]
        ctx.rough_lifecycle_probe
            .record_operation_build(StateRoughGeometryKind::Paths);
        let (fill_d, stroke_d) = build();
        let built = (Rc::new(fill_d), Rc::new(stroke_d));
        ctx.rough_cache
            .insert_paths(key, (Rc::clone(&built.0), Rc::clone(&built.1)));
        #[cfg(test)]
        state_rough_lifecycle_observe_operation_cache(ctx);
        built
    }

    let node_class = if node.css_classes.trim().is_empty() {
        "node".to_string()
    } else {
        format!("node {}", node.css_classes)
    };
    let node_dom_id = state_scoped_dom_id(ctx, &node.dom_id);
    let data_look = state_data_look(ctx);
    // A fallback `Math.random()` stream is ordered across shapes, so cache hits would otherwise
    // skip consumption and change subsequent output.
    let allow_rough_cache = !ctx.hand_drawn_seed.seed().may_use_math_random();
    let node_style = ctx.style_plan.node(node_id);
    let compatibility = ctx.style_plan.compatibility();
    let node_text_style = node_style
        .map(crate::state::StateNodeStylePlan::text_style)
        .unwrap_or_else(|| ctx.style_plan.base_text_style());
    let shape_style_attr = node_style
        .map(crate::state::StateNodeStylePlan::shape_style_attr)
        .unwrap_or_default();
    let semantic_shape_style_attr = node_style
        .map(crate::state::StateNodeStylePlan::semantic_shape_style_attr)
        .unwrap_or_default();
    let source_shape_style_attr = node_style
        .map(crate::state::StateNodeStylePlan::source_shape_style_attr)
        .unwrap_or_default();
    let text_style_attr = node_style
        .map(crate::state::StateNodeStylePlan::label_style_attr)
        .unwrap_or_default();
    let div_style_prefix = node_style
        .map(crate::state::StateNodeStylePlan::div_style_prefix)
        .unwrap_or_default();
    let fill_override = node_style.and_then(crate::state::StateNodeStylePlan::fill_override);
    let stroke_override = node_style.and_then(crate::state::StateNodeStylePlan::stroke_override);
    let stroke_width_override =
        node_style.and_then(crate::state::StateNodeStylePlan::stroke_width_override);
    let classic_stroke_paint_width = node_style
        .and_then(crate::state::StateNodeStylePlan::classic_stroke_paint_width)
        .unwrap_or(0.0);
    let radius_override = node_style.and_then(crate::state::StateNodeStylePlan::radius_override);
    let padding_override = node_style.and_then(crate::state::StateNodeStylePlan::padding_override);

    match node.shape.as_str() {
        "stateStart" => {
            let semantic_style = escape_xml_display(semantic_shape_style_attr);
            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r#"<g class="node default" id="{}" data-look="{}" transform="translate({}, {})"><circle class="state-start" r="7" width="14" height="14" style="{}"/></g>"#,
                escape_xml_display(&node_dom_id),
                escape_xml_display(data_look),
                fmt_display(cx),
                fmt_display(cy),
                semantic_style,
            );
            drop(_g_emit);
        }
        "stateEnd" => {
            let rough_start = timing.start();
            if timing.is_enabled() {
                details.leaf_roughjs_calls += 2;
                details.leaf_roughjs_unique.insert(StateRoughCacheKey {
                    tag: 1,
                    a: 14.0f64.to_bits(),
                    b: 0,
                    c: 0,
                    seed: ctx.hand_drawn_seed.seed(),
                });
                details.leaf_roughjs_unique.insert(StateRoughCacheKey {
                    tag: 2,
                    a: 5.0f64.to_bits(),
                    b: 0,
                    c: 0,
                    seed: ctx.hand_drawn_seed.seed(),
                });
            }
            let outer_key = StateRoughCacheKey {
                tag: 1,
                a: 14.0f64.to_bits(),
                b: 0,
                c: 0,
                seed: ctx.hand_drawn_seed.seed(),
            };
            let inner_key = StateRoughCacheKey {
                tag: 2,
                a: 5.0f64.to_bits(),
                b: 0,
                c: 0,
                seed: ctx.hand_drawn_seed.seed(),
            };

            let outer_d = cached_circle(ctx, outer_key, allow_rough_cache, || {
                roughjs_circle_path_d(14.0, &ctx.hand_drawn_seed)
                    .unwrap_or_else(|| "M0,0".to_string())
            });
            let inner_d = cached_circle(ctx, inner_key, allow_rough_cache, || {
                roughjs_circle_path_d(5.0, &ctx.hand_drawn_seed)
                    .unwrap_or_else(|| "M0,0".to_string())
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }
            let shape_style_escaped = escape_attr(&shape_style_attr);
            let inner_style = match (
                node_style
                    .map(crate::state::StateNodeStylePlan::special_state_inner_style_attr)
                    .unwrap_or_default(),
                source_shape_style_attr,
            ) {
                ("", source) => source.to_string(),
                (semantic, "") => semantic.to_string(),
                (semantic, source) => format!("{semantic};{source}"),
            };
            let inner_style_escaped = escape_attr(&inner_style);
            let outer_fill = fill_override.unwrap_or(compatibility.end_outer_fill.as_str());
            let outer_stroke = stroke_override.unwrap_or(compatibility.end_outer_stroke.as_str());
            let inner_fill = compatibility.inner_end_background.as_str();
            let inner_stroke = compatibility.end_inner_stroke.as_str();
            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="node default" id="{}" data-look="{}" transform="translate({}, {})"><g class="outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="0 0" style="{}"/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="0 0" style="{}"/></g></g></g>"##,
                escape_attr(&node_dom_id),
                escape_attr(data_look),
                fmt(cx),
                fmt(cy),
                outer_d.as_str(),
                escape_attr(outer_fill),
                shape_style_escaped,
                outer_d.as_str(),
                escape_attr(outer_stroke),
                shape_style_escaped,
                inner_d.as_str(),
                escape_attr(inner_fill),
                inner_style_escaped,
                inner_d.as_str(),
                escape_attr(inner_stroke),
                inner_style_escaped,
            );
            drop(_g_emit);
        }
        "fork" | "join" => {
            let rough_start = timing.start();
            let key = StateRoughCacheKey {
                tag: 3,
                a: w.to_bits(),
                b: h.to_bits(),
                c: 0,
                seed: ctx.hand_drawn_seed.seed(),
            };
            if timing.is_enabled() {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, allow_rough_cache, || {
                roughjs_paths_for_rect(StateRoughRectSpec {
                    x: -w / 2.0,
                    y: -h / 2.0,
                    w,
                    h,
                    fill: "#333333",
                    stroke: "#333333",
                    stroke_width: 1.3,
                    randomness: &ctx.hand_drawn_seed,
                })
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }
            let fill_attr = fill_override.unwrap_or(compatibility.special_state_color.as_str());
            let stroke_attr = stroke_override.unwrap_or(compatibility.special_state_color.as_str());
            let stroke_width_attr = stroke_width_override.unwrap_or(1.3).max(0.0);
            let shape_style_escaped = escape_attr(&shape_style_attr);
            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g></g>"##,
                escape_xml_display(&node_class),
                escape_xml_display(&node_dom_id),
                escape_xml_display(data_look),
                fmt_display(cx),
                fmt_display(cy),
                fill_d.as_str(),
                escape_xml_display(fill_attr),
                shape_style_escaped,
                stroke_d.as_str(),
                escape_xml_display(stroke_attr),
                fmt_display(stroke_width_attr),
                shape_style_escaped
            );
            drop(_g_emit);
        }
        "choice" => {
            let rough_start = timing.start();
            let key = StateRoughCacheKey {
                tag: 4,
                a: w.to_bits(),
                b: h.to_bits(),
                c: 0,
                seed: ctx.hand_drawn_seed.seed(),
            };
            if timing.is_enabled() {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, allow_rough_cache, || {
                roughjs_paths_for_svg_path(
                    &mermaid_choice_diamond_path_data(w, h),
                    "#ECECFF",
                    "#9370DB",
                    1.3,
                    "0 0",
                    &ctx.hand_drawn_seed,
                )
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }

            let fill_attr = fill_override.unwrap_or(compatibility.main_bkg.as_str());
            let stroke_attr = stroke_override.unwrap_or(compatibility.state_border.as_str());
            let stroke_width_attr = stroke_width_override
                .unwrap_or(compatibility.rough_stroke_width_value)
                .max(0.0);
            let shape_style_escaped = escape_attr(&shape_style_attr);
            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            let _ = write!(
                out,
                r##"<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g></g>"##,
                escape_xml_display(&node_class),
                escape_xml_display(&node_dom_id),
                escape_xml_display(data_look),
                fmt_display(cx),
                fmt_display(cy),
                fill_d.as_str(),
                escape_xml_display(fill_attr),
                shape_style_escaped,
                stroke_d.as_str(),
                escape_xml_display(stroke_attr),
                fmt_display(stroke_width_attr),
                shape_style_escaped
            );
            drop(_g_emit);
        }
        "note" => {
            let label = state_node_label_text(node);
            let prepared_label = ctx.label_sidecar.node(node_id);
            let measure_start = timing.start();
            let wrap_mode = if ctx.html_labels {
                WrapMode::HtmlLike
            } else {
                WrapMode::SvgLike
            };
            let measurement = prepared_label.map_or_else(
                || {
                    crate::state::measure_state_markdown_label(
                        &label,
                        ctx.measurer,
                        node_text_style,
                        Some(ctx.html_label_wrapping_width),
                        wrap_mode,
                    )
                },
                |prepared| crate::state::StateLabelMeasurement {
                    metrics: prepared.metrics(),
                    uses_html_wrapping_table: prepared.uses_html_wrapping_table(),
                },
            );
            let metrics = &measurement.metrics;
            if let Some(s) = measure_start {
                details.leaf_nodes_measure += s.elapsed();
            }
            let lw = metrics.width.max(0.0);
            let lh = metrics.height.max(0.0);
            let rough_start = timing.start();
            let key = StateRoughCacheKey {
                tag: 5,
                a: w.to_bits(),
                b: h.to_bits(),
                c: 0,
                seed: ctx.hand_drawn_seed.seed(),
            };
            if timing.is_enabled() {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, allow_rough_cache, || {
                roughjs_paths_for_rect(StateRoughRectSpec {
                    x: -w / 2.0,
                    y: -h / 2.0,
                    w,
                    h,
                    fill: "#fff5ad",
                    stroke: "#aaaa33",
                    stroke_width: 1.3,
                    randomness: &ctx.hand_drawn_seed,
                })
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }
            let label_html_start = timing.start();
            let label_span_style = (!text_style_attr.is_empty()).then_some(text_style_attr);
            let label_dom = if let Some(prepared) = prepared_label {
                if ctx.html_labels {
                    state_prepared_node_label_html_with_style(prepared, label_span_style)
                } else {
                    state_prepared_svg_text_label(prepared, false, label_span_style)
                }
            } else if ctx.html_labels {
                state_node_label_html_with_style(
                    &label,
                    label_span_style,
                    node_text_style.font_size,
                )
            } else {
                state_svg_text_label(&label, false, label_span_style)
            };
            if let Some(s) = label_html_start {
                details.leaf_nodes_label_html += s.elapsed();
            }
            let fill_attr = fill_override.unwrap_or(compatibility.note_bkg.as_str());
            let stroke_attr = stroke_override.unwrap_or(compatibility.note_border.as_str());
            let stroke_width_attr = stroke_width_override.unwrap_or(1.3).max(0.0);
            let shape_style_escaped = escape_xml_display(shape_style_attr);
            let label_style_escaped = escape_xml_display(text_style_attr);
            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            if ctx.html_labels {
                let prepared_token_attr = state_prepared_html_label_token_attr(prepared_label);
                let div_style = if measurement.uses_html_wrapping_table {
                    format!(
                        "{}display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;",
                        div_style_prefix,
                        fmt(ctx.html_label_wrapping_width),
                        fmt(ctx.html_label_wrapping_width),
                    )
                } else {
                    format!(
                        "{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                        div_style_prefix,
                        fmt(ctx.html_label_wrapping_width),
                    )
                };
                let _ = write!(
                    out,
                    r##"<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"><g class="basic label-container outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g><g class="label noteLabel" style="{}" transform="translate({}, {})"><rect/><foreignObject{} width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">{}</div></foreignObject></g></g>"##,
                    escape_xml_display(&node_class),
                    escape_xml_display(&node_dom_id),
                    escape_xml_display(data_look),
                    fmt_display(cx),
                    fmt_display(cy),
                    fill_d.as_str(),
                    escape_xml_display(fill_attr),
                    shape_style_escaped,
                    stroke_d.as_str(),
                    escape_xml_display(stroke_attr),
                    fmt_display(stroke_width_attr),
                    shape_style_escaped,
                    label_style_escaped,
                    fmt_display(-lw / 2.0),
                    fmt_display(-lh / 2.0),
                    prepared_token_attr,
                    fmt_display(lw),
                    fmt_display(lh),
                    div_style,
                    label_dom
                );
            } else {
                let _ = write!(
                    out,
                    r##"<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"><g class="basic label-container outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g><g class="label noteLabel" style="{}" transform="translate({}, {})"><rect/>{}</g></g>"##,
                    escape_xml_display(&node_class),
                    escape_xml_display(&node_dom_id),
                    escape_xml_display(data_look),
                    fmt_display(cx),
                    fmt_display(cy),
                    fill_d.as_str(),
                    escape_xml_display(fill_attr),
                    shape_style_escaped,
                    stroke_d.as_str(),
                    escape_xml_display(stroke_attr),
                    fmt_display(stroke_width_attr),
                    shape_style_escaped,
                    label_style_escaped,
                    fmt_display(-lw / 2.0),
                    fmt_display(-lh / 2.0),
                    label_dom
                );
            }
            drop(_g_emit);
        }
        "rectWithTitle" => {
            let title = node
                .label
                .as_ref()
                .map(state_value_to_label_text)
                .unwrap_or_else(|| node.id.clone());
            let desc = node
                .description
                .as_ref()
                .map(|v| v.join("\n"))
                .unwrap_or_default();
            let prepared_title = ctx.label_sidecar.node_title(node_id);
            let prepared_description = ctx.label_sidecar.node_description(node_id);
            let measure_start = timing.start();
            let title_metrics = prepared_title.map_or_else(
                || {
                    ctx.measurer
                        .measure_wrapped(&title, node_text_style, None, WrapMode::HtmlLike)
                },
                crate::state::PreparedStateLabel::metrics,
            );
            let desc_metrics = prepared_description.map_or_else(
                || {
                    ctx.measurer
                        .measure_wrapped(&desc, node_text_style, None, WrapMode::HtmlLike)
                },
                crate::state::PreparedStateLabel::metrics,
            );
            if let Some(s) = measure_start {
                details.leaf_nodes_measure += s.elapsed();
            }

            let title_w = title_metrics.width.max(0.0);
            let title_h = title_metrics.height.max(0.0);
            let desc_w = desc_metrics.width.max(0.0);
            let desc_h = desc_metrics.height.max(0.0);
            let padding = padding_override
                .or(node.padding)
                .unwrap_or(ctx.state_padding)
                .max(0.0);
            let geometry = crate::state::RectWithTitleGeometry::from_metrics(
                title_w, title_h, desc_w, desc_h, padding,
            );
            let label_html_start = timing.start();
            let label_span_style = (!text_style_attr.is_empty()).then_some(text_style_attr);
            let (title_dom, desc_dom) =
                if let (Some(title), Some(description)) = (prepared_title, prepared_description) {
                    if ctx.html_labels {
                        (
                            state_prepared_node_label_plain_html(title),
                            state_prepared_node_label_plain_html(description),
                        )
                    } else {
                        (
                            state_prepared_svg_text_label(title, false, label_span_style),
                            state_prepared_svg_text_label(description, false, label_span_style),
                        )
                    }
                } else if ctx.html_labels {
                    (
                        state_node_label_plain_html(&title),
                        state_node_label_plain_html(&desc),
                    )
                } else {
                    (
                        state_svg_text_label(&title, false, label_span_style),
                        state_svg_text_label(&desc, false, label_span_style),
                    )
                };
            if let Some(s) = label_html_start {
                details.leaf_nodes_label_html += s.elapsed();
            }
            let shape_style_escaped = escape_xml_display(shape_style_attr);
            let label_style_escaped = escape_xml_display(text_style_attr);
            let html_div_style_raw = format!(
                "{}display: table-cell; white-space: nowrap; line-height: 1.5;",
                div_style_prefix
            );
            let html_div_style = escape_xml_display(&html_div_style_raw);
            let radius_attrs = radius_override
                .map(|radius| {
                    format!(
                        r#" rx="{}" ry="{}""#,
                        fmt_display(radius.max(0.0)),
                        fmt_display(radius.max(0.0))
                    )
                })
                .unwrap_or_default();
            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            if ctx.html_labels {
                let prepared_pair = prepared_title.zip(prepared_description);
                let (title_token_attr, description_token_attr) = prepared_pair.map_or_else(
                    || (String::new(), String::new()),
                    |(title, description)| {
                        (
                            state_prepared_html_label_token_attr(Some(title)),
                            state_prepared_html_label_token_attr(Some(description)),
                        )
                    },
                );
                let _ = write!(
                    out,
                    r#"<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"><g><rect class="outer title-state" style="{}"{} x="{}" y="{}" width="{}" height="{}"/><line class="divider" x1="{}" x2="{}" y1="{}" y2="{}"/></g><g class="label" style="{}" transform="translate({}, {})"><foreignObject{} width="{}" height="{}" transform="translate( {}, 0)"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">{}</div></foreignObject><foreignObject{} width="{}" height="{}" transform="translate( {}, {})"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">{}</div></foreignObject></g></g>"#,
                    escape_xml_display(&node_class),
                    escape_xml_display(&node_dom_id),
                    escape_xml_display(data_look),
                    fmt_display(cx),
                    fmt_display(cy),
                    shape_style_escaped,
                    radius_attrs,
                    fmt_display(-w / 2.0),
                    fmt_display(-h / 2.0),
                    fmt_display(w),
                    fmt_display(h),
                    fmt_display(-w / 2.0),
                    fmt_display(w / 2.0),
                    fmt_display(geometry.divider_y),
                    fmt_display(geometry.divider_y),
                    label_style_escaped,
                    fmt_display(geometry.label_x),
                    fmt_display(geometry.label_y),
                    title_token_attr,
                    fmt_display(title_w),
                    fmt_display(title_h),
                    fmt_display(geometry.title_x),
                    html_div_style,
                    title_dom,
                    description_token_attr,
                    fmt_display(desc_w),
                    fmt_display(desc_h),
                    fmt_display(geometry.description_x),
                    fmt_display(geometry.description_y),
                    html_div_style,
                    desc_dom
                );
            } else {
                let _ = write!(
                    out,
                    r#"<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"><g><rect class="outer title-state" style="{}"{} x="{}" y="{}" width="{}" height="{}"/><line class="divider" x1="{}" x2="{}" y1="{}" y2="{}"/></g><g class="label" style="{}" transform="translate({}, {})"><g transform="translate({}, 0)">{}</g><g transform="translate({}, {})">{}</g></g></g>"#,
                    escape_xml_display(&node_class),
                    escape_xml_display(&node_dom_id),
                    escape_xml_display(data_look),
                    fmt_display(cx),
                    fmt_display(cy),
                    shape_style_escaped,
                    radius_attrs,
                    fmt_display(-w / 2.0),
                    fmt_display(-h / 2.0),
                    fmt_display(w),
                    fmt_display(h),
                    fmt_display(-w / 2.0),
                    fmt_display(w / 2.0),
                    fmt_display(geometry.divider_y),
                    fmt_display(geometry.divider_y),
                    label_style_escaped,
                    fmt_display(geometry.label_x),
                    fmt_display(geometry.label_y),
                    fmt_display(geometry.title_x),
                    title_dom,
                    fmt_display(geometry.description_x),
                    fmt_display(geometry.description_y),
                    desc_dom
                );
            }
            drop(_g_emit);
        }
        _ => {
            let label = state_node_label_text(node);
            let prepared_label = ctx.label_sidecar.node(node_id);

            let measure_start = timing.start();
            let wrap_mode = if ctx.html_labels {
                WrapMode::HtmlLike
            } else {
                WrapMode::SvgLike
            };
            let measurement = prepared_label.map_or_else(
                || {
                    crate::state::measure_state_markdown_label(
                        &label,
                        ctx.measurer,
                        node_text_style,
                        Some(ctx.html_label_wrapping_width),
                        wrap_mode,
                    )
                },
                |prepared| crate::state::StateLabelMeasurement {
                    metrics: prepared.metrics(),
                    uses_html_wrapping_table: prepared.uses_html_wrapping_table(),
                },
            );
            let metrics = &measurement.metrics;
            if let Some(s) = measure_start {
                details.leaf_nodes_measure += s.elapsed();
            }

            let lw = metrics.width.max(0.0);
            let lh = metrics.height.max(0.0);

            let mut link_open = String::new();
            let mut link_close = String::new();
            let mut node_title_attr = String::new();
            if let Some(links) = ctx.links.get(node_id) {
                let mut push_link = |link: &StateSvgLink| {
                    let title_attr = if link.tooltip.is_empty() {
                        String::new()
                    } else {
                        let tooltip = if ctx.security_level_loose {
                            link.tooltip.as_str()
                        } else {
                            normalize_mermaid_tooltip_attribute(&link.tooltip)
                        };
                        format!(r#" title="{}""#, escape_attr(tooltip))
                    };
                    if !title_attr.is_empty() {
                        node_title_attr = title_attr.clone();
                    }

                    if let Some(href) = prepare_mermaid_navigation_href(
                        &link.url,
                        MermaidNavigationSecurity::from_security_level_loose(
                            ctx.security_level_loose,
                        ),
                    ) {
                        let target_attr = if ctx.security_level_loose {
                            r#" target="_blank""#
                        } else {
                            ""
                        };
                        link_open.push_str(&format!(
                            r#"<a xlink:href="{}"{}{}>"#,
                            href.as_serialized_str(),
                            target_attr,
                            title_attr
                        ));
                    } else {
                        link_open.push_str(&format!(r#"<a{}>"#, title_attr));
                    }
                    link_close.push_str("</a>");
                };

                match links {
                    StateSvgLinks::One(link) => push_link(link),
                    StateSvgLinks::Many(links) => {
                        for link in links {
                            push_link(link);
                        }
                    }
                }
            }

            let fill_attr = fill_override.unwrap_or(compatibility.state_bkg.as_str());
            let stroke_attr = stroke_override.unwrap_or(compatibility.state_border.as_str());
            let stroke_width_attr = stroke_width_override
                .unwrap_or(compatibility.rough_stroke_width_value)
                .max(0.0);

            let label_span_style = if text_style_attr.is_empty() {
                None
            } else {
                Some(text_style_attr)
            };
            let label_html_start = timing.start();
            let label_dom = if let Some(prepared) = prepared_label {
                if ctx.html_labels {
                    state_prepared_node_label_html_with_style(prepared, label_span_style)
                } else {
                    state_prepared_svg_text_label(prepared, false, label_span_style)
                }
            } else if ctx.html_labels {
                state_node_label_html_with_style(
                    &label,
                    label_span_style,
                    node_text_style.font_size,
                )
            } else {
                state_svg_text_label(&label, false, label_span_style)
            };
            if let Some(s) = label_html_start {
                details.leaf_nodes_label_html += s.elapsed();
            }

            let div_style = if measurement.uses_html_wrapping_table {
                format!(
                    r#"{}display: table; white-space: break-spaces; line-height: 1.5; max-width: {}px; text-align: center; width: {}px;"#,
                    div_style_prefix,
                    fmt(ctx.html_label_wrapping_width),
                    fmt(ctx.html_label_wrapping_width),
                )
            } else {
                format!(
                    r#"{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"#,
                    div_style_prefix,
                    fmt(ctx.html_label_wrapping_width)
                )
            };

            if data_look != "handDrawn" {
                let effect_filter = if let Some(binding) =
                    node_style.and_then(crate::state::StateNodeStylePlan::effect)
                {
                    if let Some((effect, materialized)) =
                        ctx.style_plan.effect_plan().materialize_classic_rect(
                            binding.effect_id(),
                            w,
                            h,
                            classic_stroke_paint_width,
                        )?
                    {
                        effect_outsets.include(materialized.outsets());
                        let region = materialized.region();
                        let scoped_filter_id =
                            format!("{}-theme-effect-{}", node_dom_id, effect.id());
                        let filter_url = write_state_theme_effect_application(
                            out,
                            &scoped_filter_id,
                            effect,
                            region,
                        );
                        Some((effect, scoped_filter_id, filter_url, region))
                    } else {
                        None
                    }
                } else {
                    None
                }
                .map(|(effect, scoped_filter_id, filter_url, region)| {
                    let attr = format!(r#" filter="{}""#, escape_attr(&filter_url));
                    (effect, scoped_filter_id, attr, region)
                });
                let effect_filter_attr = effect_filter
                    .as_ref()
                    .map(|(_, _, attr, _)| attr.as_str())
                    .unwrap_or_default();
                let rect_radius = radius_override
                    .unwrap_or_else(|| if data_look == "neo" { 3.0 } else { 5.0 })
                    .max(0.0);
                let rect_style = escape_xml_display(&shape_style_attr);
                let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
                if ctx.html_labels {
                    let prepared_token_attr = state_prepared_html_label_token_attr(prepared_label);
                    let _ = write!(
                        out,
                        r##"{}<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"{}><rect class="basic label-container" style="{}"{} rx="{}" ry="{}" x="{}" y="{}" width="{}" height="{}"/><g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject{} width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">{}</div></foreignObject></g></g>{}"##,
                        link_open,
                        escape_xml_display(&node_class),
                        escape_xml_display(&node_dom_id),
                        escape_xml_display(data_look),
                        fmt_display(cx),
                        fmt_display(cy),
                        node_title_attr,
                        rect_style,
                        effect_filter_attr,
                        fmt_display(rect_radius),
                        fmt_display(rect_radius),
                        fmt_display(-w / 2.0),
                        fmt_display(-h / 2.0),
                        fmt_display(w),
                        fmt_display(h),
                        escape_xml_display(&text_style_attr),
                        fmt_display(-lw / 2.0),
                        fmt_display(-lh / 2.0),
                        prepared_token_attr,
                        fmt_display(lw),
                        fmt_display(lh),
                        div_style,
                        label_dom,
                        link_close
                    );
                } else {
                    let _ = write!(
                        out,
                        r##"{}<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"{}><rect class="basic label-container" style="{}"{} rx="{}" ry="{}" x="{}" y="{}" width="{}" height="{}"/><g class="label" style="{}" transform="translate({}, {})"><rect/>{}</g></g>{}"##,
                        link_open,
                        escape_xml_display(&node_class),
                        escape_xml_display(&node_dom_id),
                        escape_xml_display(data_look),
                        fmt_display(cx),
                        fmt_display(cy),
                        node_title_attr,
                        rect_style,
                        effect_filter_attr,
                        fmt_display(rect_radius),
                        fmt_display(rect_radius),
                        fmt_display(-w / 2.0),
                        fmt_display(-h / 2.0),
                        fmt_display(w),
                        fmt_display(h),
                        escape_xml_display(&text_style_attr),
                        fmt_display(-lw / 2.0),
                        fmt_display(-lh / 2.0),
                        label_dom,
                        link_close
                    );
                }
                if let Some((effect, scoped_filter_id, _, region)) = effect_filter.as_ref() {
                    ctx.effect_evidence
                        .record_application(effect, scoped_filter_id, *region);
                }
                drop(_g_emit);
                return Ok(());
            }

            let rough_start = timing.start();
            let rect_radius = super::roughjs::normalized_rounded_rect_radius(
                w,
                h,
                radius_override.unwrap_or(5.0),
            );
            let key = rounded_rect_rough_cache_key(w, h, rect_radius, ctx.hand_drawn_seed.seed());
            if timing.is_enabled() {
                details.leaf_roughjs_calls += 1;
                details.leaf_roughjs_unique.insert(key);
            }
            let (fill_d, stroke_d) = cached_paths(ctx, key, allow_rough_cache, || {
                roughjs_paths_for_svg_path(
                    &mermaid_rounded_rect_path_data(w, h, rect_radius),
                    "#ECECFF",
                    "#9370DB",
                    1.3,
                    "0 0",
                    &ctx.hand_drawn_seed,
                )
                .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()))
            });
            if let Some(s) = rough_start {
                details.leaf_nodes_roughjs += s.elapsed();
            }

            let _g_emit = detail_guard(timing, &mut details.leaf_nodes_emit);
            if ctx.html_labels {
                let prepared_token_attr = state_prepared_html_label_token_attr(prepared_label);
                let _ = write!(
                    out,
                    r##"{}<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"{}><g class="basic label-container outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g><g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject{} width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}">{}</div></foreignObject></g></g>{}"##,
                    link_open,
                    escape_xml_display(&node_class),
                    escape_xml_display(&node_dom_id),
                    escape_xml_display(data_look),
                    fmt_display(cx),
                    fmt_display(cy),
                    node_title_attr,
                    fill_d.as_str(),
                    escape_xml_display(fill_attr),
                    escape_xml_display(&shape_style_attr),
                    stroke_d.as_str(),
                    escape_xml_display(stroke_attr),
                    fmt_display(stroke_width_attr),
                    escape_xml_display(&shape_style_attr),
                    escape_xml_display(&text_style_attr),
                    fmt_display(-lw / 2.0),
                    fmt_display(-lh / 2.0),
                    prepared_token_attr,
                    fmt_display(lw),
                    fmt_display(lh),
                    div_style,
                    label_dom,
                    link_close
                );
            } else {
                let _ = write!(
                    out,
                    r##"{}<g class="{}" id="{}" data-look="{}" transform="translate({}, {})"{}><g class="basic label-container outer-path"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="{}"/></g><g class="label" style="{}" transform="translate({}, {})"><rect/>{}</g></g>{}"##,
                    link_open,
                    escape_xml_display(&node_class),
                    escape_xml_display(&node_dom_id),
                    escape_xml_display(data_look),
                    fmt_display(cx),
                    fmt_display(cy),
                    node_title_attr,
                    fill_d.as_str(),
                    escape_xml_display(fill_attr),
                    escape_xml_display(&shape_style_attr),
                    stroke_d.as_str(),
                    escape_xml_display(stroke_attr),
                    fmt_display(stroke_width_attr),
                    escape_xml_display(&shape_style_attr),
                    escape_xml_display(&text_style_attr),
                    fmt_display(-lw / 2.0),
                    fmt_display(-lh / 2.0),
                    label_dom,
                    link_close
                );
            }
            drop(_g_emit);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_cache_key_tracks_normalized_radius() {
        let seed = roughr::core::RoughJsSeed::new(1.0);
        let square = rounded_rect_rough_cache_key(100.0, 40.0, 0.0, seed);
        let rounded = rounded_rect_rough_cache_key(100.0, 40.0, 12.0, seed);
        let clamped = rounded_rect_rough_cache_key(100.0, 40.0, 100.0, seed);
        let max = rounded_rect_rough_cache_key(100.0, 40.0, 20.0, seed);

        assert_ne!(square, rounded);
        assert_eq!(clamped, max);
        assert_eq!(square.c, 0.0f64.to_bits());
        assert_eq!(rounded.c, 12.0f64.to_bits());
    }
}
