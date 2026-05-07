use super::super::*;
use super::activation::{build_sequence_activation_plan, render_sequence_activation_group};
use super::actors::{
    render_sequence_actor_man_bottoms, render_sequence_actor_man_tops,
    render_sequence_actor_popup_menus, render_sequence_bottom_actors,
    render_sequence_top_actors_and_lifelines,
};
use super::blocks::{
    SequenceBlock, collect_sequence_blocks, frame_x_from_actors, render_critical_sequence_block,
    render_sectioned_sequence_block, render_simple_sequence_block,
};
use super::frames::render_sequence_box_frames_and_rect_blocks;
use super::messages::render_sequence_messages;
use super::notes::render_sequence_notes;
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
                        render_sectioned_sequence_block(
                            &mut out,
                            "alt",
                            sections,
                            true,
                            _frame_x1,
                            _frame_x2,
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                            label_box_height,
                            box_text_margin,
                            measurer,
                            &loop_text_style,
                        );
                    }
                    SequenceBlock::Par { sections } => {
                        render_sectioned_sequence_block(
                            &mut out,
                            "par",
                            sections,
                            false,
                            _frame_x1,
                            _frame_x2,
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                            label_box_height,
                            box_text_margin,
                            measurer,
                            &loop_text_style,
                        );
                    }
                    SequenceBlock::Loop {
                        raw_label,
                        message_ids,
                    } => {
                        render_simple_sequence_block(
                            &mut out,
                            "loop",
                            raw_label,
                            message_ids,
                            _frame_x1,
                            _frame_x2,
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                            label_box_height,
                            measurer,
                            &loop_text_style,
                        );
                    }
                    SequenceBlock::Opt {
                        raw_label,
                        message_ids,
                    } => {
                        render_simple_sequence_block(
                            &mut out,
                            "opt",
                            raw_label,
                            message_ids,
                            _frame_x1,
                            _frame_x2,
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                            label_box_height,
                            measurer,
                            &loop_text_style,
                        );
                    }
                    SequenceBlock::Break {
                        raw_label,
                        message_ids,
                    } => {
                        render_simple_sequence_block(
                            &mut out,
                            "break",
                            raw_label,
                            message_ids,
                            _frame_x1,
                            _frame_x2,
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                            label_box_height,
                            measurer,
                            &loop_text_style,
                        );
                    }
                    SequenceBlock::Critical { sections } => {
                        render_critical_sequence_block(
                            &mut out,
                            sections,
                            _frame_x1,
                            _frame_x2,
                            &msg_endpoints,
                            &actor_nodes_by_id,
                            &edges_by_id,
                            &nodes_by_id,
                            label_box_height,
                            box_text_margin,
                            measurer,
                            &loop_text_style,
                        );
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
