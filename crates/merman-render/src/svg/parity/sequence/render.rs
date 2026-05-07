use super::super::*;
use super::activation::{build_sequence_activation_plan, render_sequence_activation_group};
use super::actors::{
    render_sequence_actor_man_bottoms, render_sequence_actor_man_tops,
    render_sequence_actor_popup_menus, render_sequence_bottom_actors,
    render_sequence_top_actors_and_lifelines,
};
use super::blocks::{
    SequenceBlock, collect_sequence_blocks, display_block_label, frame_x_from_actors,
    frame_x_from_message_ids, item_y_range, wrap_svg_text_lines, write_block_frame,
    write_block_label_box, write_loop_text_lines,
};
use super::frames::render_sequence_box_frames_and_rect_blocks;
use super::messages::render_sequence_messages;
use super::notes::render_sequence_notes;
use crate::generated::sequence_text_overrides_11_12_2 as sequence_text_overrides;
use rustc_hash::FxHashMap;

use super::css::sequence_css;
use super::model::*;

pub(super) fn render_sequence_diagram_svg(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let sanitize_config = merman_core::MermaidConfig::from_value(effective_config.clone());
    let model: SequenceSvgModel = crate::json::from_value_ref(semantic)?;
    render_sequence_diagram_svg_inner(
        layout,
        model,
        effective_config,
        &sanitize_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_sequence_diagram_svg_with_config(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: SequenceSvgModel = crate::json::from_value_ref(semantic)?;
    render_sequence_diagram_svg_model_with_config(
        layout,
        &model,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_sequence_diagram_svg_model_with_config(
    layout: &SequenceDiagramLayout,
    model: &SequenceSvgModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render_sequence_diagram_svg_inner(
        layout,
        model.clone(),
        effective_config.as_value(),
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

fn render_sequence_diagram_svg_inner(
    layout: &SequenceDiagramLayout,
    mut model: SequenceSvgModel,
    effective_config: &serde_json::Value,
    sanitize_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    if model.title.as_deref().is_none_or(|t| t.trim().is_empty()) {
        if let Some(title) = diagram_title.map(str::trim).filter(|t| !t.is_empty()) {
            model.title = Some(title.to_string());
        }
    }

    let seq_cfg = effective_config
        .get("sequence")
        .unwrap_or(&serde_json::Value::Null);
    let force_menus = seq_cfg
        .get("forceMenus")
        .or_else(|| effective_config.get("forceMenus"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mirror_actors = seq_cfg
        .get("mirrorActors")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let diagram_margin_x = seq_cfg
        .get("diagramMarginX")
        .and_then(|v| v.as_f64())
        .unwrap_or(50.0)
        .max(0.0);
    let box_margin = seq_cfg
        .get("boxMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(10.0)
        .max(0.0);
    let actor_height = seq_cfg
        .get("height")
        .and_then(|v| v.as_f64())
        .unwrap_or(65.0)
        .max(1.0);
    let box_text_margin = seq_cfg
        .get("boxTextMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(5.0)
        .max(0.0);
    let message_align = seq_cfg
        .get("messageAlign")
        .and_then(|v| v.as_str())
        .unwrap_or("center");
    let _message_margin = seq_cfg
        .get("messageMargin")
        .and_then(|v| v.as_f64())
        .unwrap_or(35.0)
        .max(0.0);
    let _bottom_margin_adj = seq_cfg
        .get("bottomMarginAdj")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let label_box_height = seq_cfg
        .get("labelBoxHeight")
        .and_then(|v| v.as_f64())
        .unwrap_or(20.0)
        .max(0.0);
    let right_angles = seq_cfg
        .get("rightAngles")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let wrap_padding = seq_cfg
        .get("wrapPadding")
        .and_then(|v| v.as_f64())
        .unwrap_or(10.0)
        .max(0.0);
    let sequence_width = seq_cfg
        .get("width")
        .and_then(|v| v.as_f64())
        .unwrap_or(150.0)
        .max(1.0);
    // Upstream Mermaid's Sequence renderer treats the global `fontSize` as authoritative. Even
    // when per-sequence overrides like `sequence.messageFontSize` are set via frontmatter/init,
    // the effective SVG output sticks to the global font size as long as it is present.
    let actor_label_font_size = effective_config
        .get("fontSize")
        .and_then(|v| v.as_f64())
        .or_else(|| seq_cfg.get("messageFontSize").and_then(|v| v.as_f64()))
        .unwrap_or(16.0)
        .max(1.0);
    let loop_text_style = TextStyle {
        font_family: effective_config
            .get("fontFamily")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        font_size: actor_label_font_size,
        font_weight: Some("400".to_string()),
    };
    let note_text_style = TextStyle {
        font_family: loop_text_style.font_family.clone(),
        font_size: actor_label_font_size,
        font_weight: Some("400".to_string()),
    };
    let actor_wrap_width = (sequence_width - 2.0 * wrap_padding).max(1.0);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    // Upstream Mermaid viewports are driven by browser layout pipelines and often land on an `f32`
    // lattice (e.g. `...49998474121094`). Mirror that by quantizing the extrema to `f32` first,
    // then computing width/height in `f32` space.
    let min_x_f32 = bounds.min_x as f32;
    let min_y_f32 = bounds.min_y as f32;
    let max_x_f32 = bounds.max_x as f32;
    let max_y_f32 = bounds.max_y as f32;

    let vb_min_x = min_x_f32 as f64;
    let vb_min_y = min_y_f32 as f64;
    let vb_w = ((max_x_f32 - min_x_f32).max(1.0)) as f64;
    let vb_h = ((max_y_f32 - min_y_f32).max(1.0)) as f64;

    let mut nodes_by_id: FxHashMap<&str, &LayoutNode> =
        FxHashMap::with_capacity_and_hasher(layout.nodes.len(), Default::default());
    for n in &layout.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut edges_by_id: FxHashMap<&str, &crate::model::LayoutEdge> =
        FxHashMap::with_capacity_and_hasher(layout.edges.len(), Default::default());
    for e in &layout.edges {
        edges_by_id.insert(e.id.as_str(), e);
    }

    let mut out = String::new();
    let aria_labelledby_attr = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id_esc}"));
    let aria_describedby_attr = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id_esc}"));
    let mut max_w_attr = fmt_string(vb_w);
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    if let Some((viewbox, max_w)) =
        crate::generated::sequence_root_overrides_11_12_2::lookup_sequence_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }

    let style_attr = format!("max-width: {max_w_attr}px; background-color: white;");
    root_svg::push_svg_root_open(
        &mut out,
        root_svg::SvgRootAttrs {
            width: root_svg::SvgRootWidth::Percent100,
            style_attr: Some(style_attr.as_str()),
            viewbox_attr: Some(viewbox_attr.as_str()),
            aria_labelledby: aria_labelledby_attr.as_deref(),
            aria_describedby: aria_describedby_attr.as_deref(),
            trailing_newline: false,
            ..root_svg::SvgRootAttrs::new(diagram_id, "sequence")
        },
    );

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml_display(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml_display(desc)
        );
    }

    render_sequence_box_frames_and_rect_blocks(
        &mut out,
        &model,
        &nodes_by_id,
        actor_label_font_size,
        box_margin,
        box_text_margin,
    );

    if mirror_actors {
        render_sequence_bottom_actors(
            &mut out,
            &model,
            &nodes_by_id,
            actor_wrap_width,
            label_box_height,
            measurer,
            &loop_text_style,
        );
    }

    // Top actors + lifelines.
    render_sequence_top_actors_and_lifelines(
        &mut out,
        &model,
        &nodes_by_id,
        &edges_by_id,
        actor_wrap_width,
        actor_height,
        measurer,
        &loop_text_style,
    );

    let _ = write!(
        &mut out,
        r#"<style>{}</style><g/>"#,
        sequence_css(diagram_id)
    );

    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    out.push_str(MERMAID_SEQUENCE_BASE_DEFS_11_12_2);

    render_sequence_actor_man_tops(&mut out, &model, &nodes_by_id, actor_height);

    // Mermaid creates activation placeholders at ACTIVE_START and inserts the `<rect>` once the
    // corresponding ACTIVE_END is encountered. We store the final rect geometry during this
    // first pass and remember which message id should emit which activation group.
    let activation_plan = build_sequence_activation_plan(
        &model,
        &nodes_by_id,
        &edges_by_id,
        seq_cfg,
        effective_config,
    );

    let (blocks_by_end_id, blocks) = collect_sequence_blocks(&model);

    if let Some((_frame_x1, _frame_x2)) = frame_x_from_actors(&model, &nodes_by_id) {
        let mut actor_nodes_by_id: FxHashMap<&str, &LayoutNode> =
            FxHashMap::with_capacity_and_hasher(model.actors.len(), Default::default());
        for actor_id in &model.actor_order {
            let node_id = format!("actor-top-{actor_id}");
            let Some(n) = nodes_by_id.get(node_id.as_str()).copied() else {
                continue;
            };
            actor_nodes_by_id.insert(actor_id.as_str(), n);
        }

        let mut msg_endpoints: FxHashMap<&str, (&str, &str)> =
            FxHashMap::with_capacity_and_hasher(model.messages.len(), Default::default());
        for msg in &model.messages {
            let (Some(from), Some(to)) = (msg.from.as_deref(), msg.to.as_deref()) else {
                continue;
            };
            msg_endpoints.insert(msg.id.as_str(), (from, to));
        }

        render_sequence_notes(
            &mut out,
            &model,
            &nodes_by_id,
            measurer,
            actor_label_font_size,
            wrap_padding,
            &note_text_style,
        );

        for msg in &model.messages {
            render_sequence_activation_group(&mut out, &activation_plan, &msg.id);

            let Some(idxs) = blocks_by_end_id.get(&msg.id) else {
                continue;
            };
            for idx in idxs {
                let Some(block) = blocks.get(*idx) else {
                    continue;
                };
                match block {
                    SequenceBlock::Alt { sections } => {
                        if sections.is_empty() {
                            continue;
                        }

                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for sec in sections {
                            for msg_id in &sec.message_ids {
                                if let Some((y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    false,
                                ) {
                                    min_y = min_y.min(y0);
                                    max_y = max_y.max(y1);
                                }
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            sections.iter().flat_map(|s| s.message_ids.iter()),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let header_offset = if sections
                            .first()
                            .is_some_and(|s| s.raw_label.trim().is_empty())
                        {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            // When the critical label wraps, Mermaid increases the header height so the
                            // frame starts higher (see upstream `adjustLoopHeightForWrap(...)`).
                            let base = 79.0;
                            let label_box_right = frame_x1 + 50.0;
                            let max_w = (frame_x2 - label_box_right).max(0.0);
                            let label = display_block_label(&sections[0].raw_label, true)
                                .unwrap_or_else(|| "\u{200B}".to_string());
                            let wrapped = wrap_svg_text_lines(
                                &label,
                                measurer,
                                &loop_text_style,
                                Some(max_w),
                            );
                            let extra_lines = wrapped.len().saturating_sub(1) as f64;
                            let extra_per_line =
                                (sequence_text_overrides::sequence_text_line_step_px(
                                    loop_text_style.font_size,
                                ) - box_text_margin)
                                    .max(0.0);
                            base + extra_lines * extra_per_line
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);

                        // frame
                        write_block_frame(&mut out, frame_x1, frame_x2, frame_y1, frame_y2);

                        // separators (dashed)
                        // Keep separator endpoints identical to the frame endpoints to match upstream
                        // Mermaid output and avoid sub-pixel gaps at the frame border.
                        let dash_x1 = frame_x1;
                        let dash_x2 = frame_x2;
                        let mut section_max_ys: Vec<f64> = Vec::new();
                        for sec in sections {
                            let mut sec_max_y = f64::NEG_INFINITY;
                            for msg_id in &sec.message_ids {
                                if let Some((_y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    true,
                                ) {
                                    sec_max_y = sec_max_y.max(y1);
                                }
                            }
                            if !sec_max_y.is_finite() {
                                sec_max_y = min_y;
                            }
                            section_max_ys.push(sec_max_y);
                        }
                        let mut sep_ys: Vec<f64> = Vec::new();
                        for sec_max_y in section_max_ys
                            .iter()
                            .take(section_max_ys.len().saturating_sub(1))
                        {
                            sep_ys.push(*sec_max_y + 15.0);
                        }
                        for y in &sep_ys {
                            let _ = write!(
                                &mut out,
                                r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
                                x1 = fmt(dash_x1),
                                x2 = fmt(dash_x2),
                                y = fmt(*y)
                            );
                        }

                        // label box + label text
                        // This matches Mermaid's label-box shape: a 50px-wide header with a 8.4px cut.
                        write_block_label_box(&mut out, frame_x1, frame_y1, "alt");

                        // section labels
                        let label_box_right = frame_x1 + 50.0;
                        let main_text_x = (label_box_right + frame_x2) / 2.0;
                        let center_text_x = (frame_x1 + frame_x2) / 2.0;
                        for (i, sec) in sections.iter().enumerate() {
                            let Some(label_text) = display_block_label(&sec.raw_label, i == 0)
                            else {
                                continue;
                            };
                            if i == 0 {
                                let y = frame_y1 + 18.0;
                                let max_w = (frame_x2 - label_box_right).max(0.0);
                                write_loop_text_lines(
                                    &mut out,
                                    measurer,
                                    &loop_text_style,
                                    main_text_x,
                                    y,
                                    Some(max_w),
                                    &label_text,
                                    true,
                                );
                                continue;
                            }
                            let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                            write_loop_text_lines(
                                &mut out,
                                measurer,
                                &loop_text_style,
                                center_text_x,
                                y,
                                None,
                                &label_text,
                                false,
                            );
                        }

                        out.push_str("</g>");
                    }
                    SequenceBlock::Par { sections } => {
                        if sections.is_empty() {
                            continue;
                        }

                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for sec in sections {
                            for msg_id in &sec.message_ids {
                                if let Some((y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    false,
                                ) {
                                    min_y = min_y.min(y0);
                                    max_y = max_y.max(y1);
                                }
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            sections.iter().flat_map(|s| s.message_ids.iter()),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let header_offset = if sections
                            .first()
                            .is_some_and(|s| s.raw_label.trim().is_empty())
                        {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            79.0
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);

                        // frame
                        write_block_frame(&mut out, frame_x1, frame_x2, frame_y1, frame_y2);

                        // separators (dashed)
                        let dash_x1 = frame_x1;
                        let dash_x2 = frame_x2;
                        let mut section_max_ys: Vec<f64> = Vec::new();
                        for sec in sections {
                            let mut sec_max_y = f64::NEG_INFINITY;
                            for msg_id in &sec.message_ids {
                                if let Some((_y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    true,
                                ) {
                                    sec_max_y = sec_max_y.max(y1);
                                }
                            }
                            if !sec_max_y.is_finite() {
                                sec_max_y = min_y;
                            }
                            section_max_ys.push(sec_max_y);
                        }
                        let mut sep_ys: Vec<f64> = Vec::new();
                        for sec_max_y in section_max_ys
                            .iter()
                            .take(section_max_ys.len().saturating_sub(1))
                        {
                            sep_ys.push(*sec_max_y + 15.0);
                        }
                        for y in &sep_ys {
                            let _ = write!(
                                &mut out,
                                r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
                                x1 = fmt(dash_x1),
                                x2 = fmt(dash_x2),
                                y = fmt(*y)
                            );
                        }

                        // label box + label text
                        write_block_label_box(&mut out, frame_x1, frame_y1, "par");

                        // section labels
                        let label_box_right = frame_x1 + 50.0;
                        let main_text_x = (label_box_right + frame_x2) / 2.0;
                        let center_text_x = (frame_x1 + frame_x2) / 2.0;
                        for (i, sec) in sections.iter().enumerate() {
                            let Some(label_text) = display_block_label(&sec.raw_label, i == 0)
                            else {
                                continue;
                            };
                            if i == 0 {
                                let y = frame_y1 + 18.0;
                                let max_w = (frame_x2 - label_box_right).max(0.0);
                                write_loop_text_lines(
                                    &mut out,
                                    measurer,
                                    &loop_text_style,
                                    main_text_x,
                                    y,
                                    Some(max_w),
                                    &label_text,
                                    true,
                                );
                                continue;
                            }
                            let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                            write_loop_text_lines(
                                &mut out,
                                measurer,
                                &loop_text_style,
                                center_text_x,
                                y,
                                None,
                                &label_text,
                                false,
                            );
                        }

                        out.push_str("</g>");
                    }
                    SequenceBlock::Loop {
                        raw_label,
                        message_ids,
                    } => {
                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for msg_id in message_ids {
                            if let Some((y0, y1)) = item_y_range(
                                &edges_by_id,
                                &nodes_by_id,
                                &msg_endpoints,
                                msg_id,
                                false,
                            ) {
                                min_y = min_y.min(y0);
                                max_y = max_y.max(y1);
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            message_ids.iter(),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        // Mermaid draws the loop frame far enough above the first message line to
                        // leave room for the header label box + label text.
                        let header_offset = if raw_label.trim().is_empty() {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            79.0
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);
                        write_block_frame(&mut out, frame_x1, frame_x2, frame_y1, frame_y2);
                        write_block_label_box(&mut out, frame_x1, frame_y1, "loop");
                        let label_box_right = frame_x1 + 50.0;
                        let text_x = (label_box_right + frame_x2) / 2.0;
                        let text_y = frame_y1 + 18.0;
                        let label = display_block_label(raw_label, true)
                            .unwrap_or_else(|| "\u{200B}".to_string());
                        let max_w = (frame_x2 - label_box_right).max(0.0);
                        write_loop_text_lines(
                            &mut out,
                            measurer,
                            &loop_text_style,
                            text_x,
                            text_y,
                            Some(max_w),
                            &label,
                            true,
                        );
                        out.push_str("</g>");
                    }
                    SequenceBlock::Opt {
                        raw_label,
                        message_ids,
                    } => {
                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for msg_id in message_ids {
                            if let Some((y0, y1)) = item_y_range(
                                &edges_by_id,
                                &nodes_by_id,
                                &msg_endpoints,
                                msg_id,
                                false,
                            ) {
                                min_y = min_y.min(y0);
                                max_y = max_y.max(y1);
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            message_ids.iter(),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let header_offset = if raw_label.trim().is_empty() {
                            (79.0 - label_box_height).max(0.0)
                        } else {
                            79.0
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);
                        write_block_frame(&mut out, frame_x1, frame_x2, frame_y1, frame_y2);
                        write_block_label_box(&mut out, frame_x1, frame_y1, "opt");
                        let label_box_right = frame_x1 + 50.0;
                        let text_x = (label_box_right + frame_x2) / 2.0;
                        let text_y = frame_y1 + 18.0;
                        let label = display_block_label(raw_label, true)
                            .unwrap_or_else(|| "\u{200B}".to_string());
                        let max_w = (frame_x2 - label_box_right).max(0.0);
                        write_loop_text_lines(
                            &mut out,
                            measurer,
                            &loop_text_style,
                            text_x,
                            text_y,
                            Some(max_w),
                            &label,
                            true,
                        );
                        out.push_str("</g>");
                    }
                    SequenceBlock::Break {
                        raw_label,
                        message_ids,
                    } => {
                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for msg_id in message_ids {
                            if let Some((y0, y1)) = item_y_range(
                                &edges_by_id,
                                &nodes_by_id,
                                &msg_endpoints,
                                msg_id,
                                false,
                            ) {
                                min_y = min_y.min(y0);
                                max_y = max_y.max(y1);
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (frame_x1, frame_x2, _min_left) = frame_x_from_message_ids(
                            message_ids.iter(),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));

                        let frame_y1 = min_y - 93.0;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);
                        write_block_frame(&mut out, frame_x1, frame_x2, frame_y1, frame_y2);
                        write_block_label_box(&mut out, frame_x1, frame_y1, "break");
                        let label_box_right = frame_x1 + 50.0;
                        let text_x = (label_box_right + frame_x2) / 2.0;
                        let text_y = frame_y1 + 18.0;
                        let label = display_block_label(raw_label, true)
                            .unwrap_or_else(|| "\u{200B}".to_string());
                        let max_w = (frame_x2 - label_box_right).max(0.0);
                        write_loop_text_lines(
                            &mut out,
                            measurer,
                            &loop_text_style,
                            text_x,
                            text_y,
                            Some(max_w),
                            &label,
                            true,
                        );
                        out.push_str("</g>");
                    }
                    SequenceBlock::Critical { sections } => {
                        if sections.is_empty() {
                            continue;
                        }

                        let mut min_y = f64::INFINITY;
                        let mut max_y = f64::NEG_INFINITY;
                        for sec in sections {
                            for msg_id in &sec.message_ids {
                                if let Some((y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    false,
                                ) {
                                    min_y = min_y.min(y0);
                                    max_y = max_y.max(y1);
                                }
                            }
                        }
                        if !min_y.is_finite() || !max_y.is_finite() {
                            continue;
                        }

                        let (mut frame_x1, frame_x2, min_left) = frame_x_from_message_ids(
                            sections.iter().flat_map(|s| s.message_ids.iter()),
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                        )
                        .unwrap_or((_frame_x1, _frame_x2, f64::INFINITY));
                        if sections.len() > 1 && min_left.is_finite() {
                            // Mermaid's `critical` w/ `option` sections widens the frame to the left.
                            frame_x1 = frame_x1.min(min_left - 9.0);
                        }

                        let header_offset = if sections
                            .first()
                            .is_some_and(|s| s.raw_label.trim().is_empty())
                        {
                            (79.0 - label_box_height).max(0.0)
                        } else if sections.len() > 1 {
                            // Mermaid does not apply the wrap height adjustment for multi-section
                            // `critical` blocks (those with one or more `option` sections).
                            79.0
                        } else {
                            // Mermaid's `adjustLoopHeightForWrap(...)` expands the header height when the
                            // section label wraps to multiple lines. This affects the frame's top y.
                            let label_text = display_block_label(&sections[0].raw_label, true)
                                .unwrap_or_else(|| "\u{200B}".to_string());
                            let label_box_right = frame_x1 + 50.0;
                            let max_w = (frame_x2 - label_box_right).max(0.0);
                            let wrapped = wrap_svg_text_lines(
                                &label_text,
                                measurer,
                                &loop_text_style,
                                Some(max_w),
                            );
                            let extra_lines = wrapped.len().saturating_sub(1) as f64;
                            let extra_per_line =
                                (sequence_text_overrides::sequence_text_line_step_px(
                                    loop_text_style.font_size,
                                ) - box_text_margin)
                                    .max(0.0);
                            79.0 + extra_lines * extra_per_line
                        };
                        let frame_y1 = min_y - header_offset;
                        let frame_y2 = max_y + 10.0;

                        out.push_str(r#"<g>"#);

                        // frame
                        write_block_frame(&mut out, frame_x1, frame_x2, frame_y1, frame_y2);

                        // separators (dashed)
                        let dash_x1 = frame_x1;
                        let dash_x2 = frame_x2;
                        let mut section_max_ys: Vec<f64> = Vec::new();
                        for sec in sections {
                            let mut sec_max_y = f64::NEG_INFINITY;
                            for msg_id in &sec.message_ids {
                                if let Some((_y0, y1)) = item_y_range(
                                    &edges_by_id,
                                    &nodes_by_id,
                                    &msg_endpoints,
                                    msg_id,
                                    true,
                                ) {
                                    sec_max_y = sec_max_y.max(y1);
                                }
                            }
                            if !sec_max_y.is_finite() {
                                sec_max_y = min_y;
                            }
                            section_max_ys.push(sec_max_y);
                        }
                        let mut sep_ys: Vec<f64> = Vec::new();
                        for sec_max_y in section_max_ys
                            .iter()
                            .take(section_max_ys.len().saturating_sub(1))
                        {
                            sep_ys.push(*sec_max_y + 15.0);
                        }
                        for y in &sep_ys {
                            let _ = write!(
                                &mut out,
                                r#"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" class="loopLine" style="stroke-dasharray: 3, 3;"/>"#,
                                x1 = fmt(dash_x1),
                                x2 = fmt(dash_x2),
                                y = fmt(*y)
                            );
                        }

                        // label box + label text
                        write_block_label_box(&mut out, frame_x1, frame_y1, "critical");

                        // section labels
                        let label_box_right = frame_x1 + 50.0;
                        let main_text_x = (label_box_right + frame_x2) / 2.0;
                        let center_text_x = (frame_x1 + frame_x2) / 2.0;
                        for (i, sec) in sections.iter().enumerate() {
                            let Some(label_text) = display_block_label(&sec.raw_label, i == 0)
                            else {
                                continue;
                            };
                            if i == 0 {
                                let y = frame_y1 + 18.0;
                                let max_w = (frame_x2 - label_box_right).max(0.0);
                                write_loop_text_lines(
                                    &mut out,
                                    measurer,
                                    &loop_text_style,
                                    main_text_x,
                                    y,
                                    Some(max_w),
                                    &label_text,
                                    true,
                                );
                                continue;
                            }
                            let y = sep_ys.get(i - 1).copied().unwrap_or(frame_y1) + 18.0;
                            write_loop_text_lines(
                                &mut out,
                                measurer,
                                &loop_text_style,
                                center_text_x,
                                y,
                                None,
                                &label_text,
                                false,
                            );
                        }

                        out.push_str("</g>");
                    }
                }
            }
        }
    }

    render_sequence_messages(
        &mut out,
        &model,
        &nodes_by_id,
        &edges_by_id,
        measurer,
        message_align,
        actor_height,
        actor_label_font_size,
        sequence_width,
        wrap_padding,
        right_angles,
        &loop_text_style,
    );

    render_sequence_actor_popup_menus(
        &mut out,
        &model,
        &nodes_by_id,
        sanitize_config,
        force_menus,
        mirror_actors,
        actor_height,
    );

    if mirror_actors {
        render_sequence_actor_man_bottoms(
            &mut out,
            &model,
            &nodes_by_id,
            actor_height,
            label_box_height,
        );
    }

    if let Some(title) = model.title.as_deref() {
        // Mermaid sequence titles are currently emitted as a plain `<text>` node.
        // Mermaid positions the title using the inner (content) box width:
        // `x = (box.stopx - box.startx) / 2 - 2 * diagramMarginX`.
        let title_x = ((vb_w - 2.0 * diagram_margin_x) / 2.0) - 2.0 * diagram_margin_x;
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="-25">{text}</text>"#,
            x = fmt(title_x),
            text = escape_xml(title)
        );
    }

    out.push_str("</svg>\n");
    Ok(out)
}
