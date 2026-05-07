use super::super::*;
use super::actors::{
    render_sequence_actor_man_bottoms, render_sequence_actor_man_tops,
    render_sequence_actor_popup_menus, render_sequence_bottom_actors,
    render_sequence_top_actors_and_lifelines,
};
use super::frames::render_sequence_box_frames_and_rect_blocks;
use super::interactions::render_sequence_interaction_overlays;
use super::messages::render_sequence_messages;
use super::root::write_sequence_svg_root_open;
use super::settings::SequenceRenderSettings;
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
    let settings = SequenceRenderSettings::from_config(effective_config, seq_cfg);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let mut out = String::new();
    let root_metrics = write_sequence_svg_root_open(&mut out, layout, &model, diagram_id);

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

    render_sequence_box_frames_and_rect_blocks(
        &mut out,
        &model,
        &nodes_by_id,
        settings.actor_label_font_size,
        settings.box_margin,
        settings.box_text_margin,
    );

    if settings.mirror_actors {
        render_sequence_bottom_actors(
            &mut out,
            &model,
            &nodes_by_id,
            settings.actor_wrap_width,
            settings.label_box_height,
            measurer,
            &settings.loop_text_style,
        );
    }

    // Top actors + lifelines.
    render_sequence_top_actors_and_lifelines(
        &mut out,
        &model,
        &nodes_by_id,
        &edges_by_id,
        settings.actor_wrap_width,
        settings.actor_height,
        measurer,
        &settings.loop_text_style,
    );

    let _ = write!(
        &mut out,
        r#"<style>{}</style><g/>"#,
        sequence_css(diagram_id)
    );

    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    out.push_str(MERMAID_SEQUENCE_BASE_DEFS_11_12_2);

    render_sequence_actor_man_tops(&mut out, &model, &nodes_by_id, settings.actor_height);

    render_sequence_interaction_overlays(
        &mut out,
        &model,
        &nodes_by_id,
        &edges_by_id,
        seq_cfg,
        effective_config,
        &settings,
        measurer,
    );

    render_sequence_messages(
        &mut out,
        &model,
        &nodes_by_id,
        &edges_by_id,
        measurer,
        settings.message_align.as_str(),
        settings.actor_height,
        settings.actor_label_font_size,
        settings.sequence_width,
        settings.wrap_padding,
        settings.right_angles,
        &settings.loop_text_style,
    );

    render_sequence_actor_popup_menus(
        &mut out,
        &model,
        &nodes_by_id,
        sanitize_config,
        settings.force_menus,
        settings.mirror_actors,
        settings.actor_height,
    );

    if settings.mirror_actors {
        render_sequence_actor_man_bottoms(
            &mut out,
            &model,
            &nodes_by_id,
            settings.actor_height,
            settings.label_box_height,
        );
    }

    if let Some(title) = model.title.as_deref() {
        // Mermaid sequence titles are currently emitted as a plain `<text>` node.
        // Mermaid positions the title using the inner (content) box width:
        // `x = (box.stopx - box.startx) / 2 - 2 * diagramMarginX`.
        let title_x = ((root_metrics.viewbox_width - 2.0 * settings.diagram_margin_x) / 2.0)
            - 2.0 * settings.diagram_margin_x;
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
