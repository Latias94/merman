use super::super::*;
use super::SequenceEmitCheckpoints;
use super::actor_man::{render_sequence_actor_man_bottoms, render_sequence_actor_man_tops};
use super::actor_popup::{SequenceActorPopupOptions, render_sequence_actor_popup_menus};
use super::actors::{
    SequenceActorRenderContext, render_sequence_bottom_actors,
    render_sequence_top_actors_and_lifelines,
};
use super::frames::{SequenceFrameRenderOptions, render_sequence_box_frames_and_rect_blocks};
use super::interactions::{SequenceInteractionRenderContext, render_sequence_interaction_overlays};
use super::messages::{
    SequenceMessageRenderContext, render_sequence_messages, sequence_message_diagram_id_occurrences,
};
use super::root::write_sequence_svg_root_open;
use super::settings::SequenceRenderSettings;
use crate::resources::ResourceLimitPhase;
use merman_core::OperationPhase;
use rustc_hash::FxHashMap;

use super::css::{SEQUENCE_CSS_DIAGRAM_ID_OCCURRENCES, sequence_css};
use super::model::*;

const PINNED_MERMAID_SEQUENCE_BASE_DEFS: &str = include_str!("sequence_base_defs_11_16_0.svgfrag");
const MERMAID_SEQUENCE_EXTRA_MARKER_DEFS_PINNED: &str = r#"<defs><marker id="solidTopArrowHead" refX="7.9" refY="7.25" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 0 L 10 8 L 0 8 z"/></marker></defs><defs><marker id="solidBottomArrowHead" refX="7.9" refY="0.75" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 0 L 10 0 L 0 8 z"/></marker></defs><defs><marker id="stickTopArrowHead" refX="7.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 0 L 7 7" stroke="black" stroke-width="1.5" fill="none"/></marker></defs><defs><marker id="stickBottomArrowHead" refX="7.5" refY="0" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 7 L 7 0" stroke="black" stroke-width="1.5" fill="none"/></marker></defs>"#;

const SEQUENCE_SCOPED_BASE_DEF_IDS: [&str; 11] = [
    "computer",
    "database",
    "clock",
    "arrowhead",
    "crosshead",
    "filled-head",
    "sequencenumber",
    "solidTopArrowHead",
    "solidBottomArrowHead",
    "stickTopArrowHead",
    "stickBottomArrowHead",
];

fn write_scoped_sequence_base_defs_fragment(out: &mut String, fragment: &str, diagram_id: &str) {
    const ID_ATTRIBUTE_START: &str = "id=\"";

    let mut cursor = 0usize;
    while let Some(relative_start) = fragment[cursor..].find(ID_ATTRIBUTE_START) {
        let attribute_start = cursor + relative_start;
        let value_start = attribute_start + ID_ATTRIBUTE_START.len();
        let Some(relative_end) = fragment[value_start..].find('"') else {
            break;
        };
        let value_end = value_start + relative_end;
        let local_id = &fragment[value_start..value_end];

        out.push_str(&fragment[cursor..value_start]);
        if SEQUENCE_SCOPED_BASE_DEF_IDS.contains(&local_id) {
            escape_attr_into(out, diagram_id);
            out.push('-');
            out.push_str(local_id);
        } else {
            out.push_str(local_id);
        }
        cursor = value_end;
    }
    out.push_str(&fragment[cursor..]);
}

fn write_scoped_sequence_base_defs(out: &mut String, diagram_id: &str) {
    write_scoped_sequence_base_defs_fragment(out, PINNED_MERMAID_SEQUENCE_BASE_DEFS, diagram_id);
    write_scoped_sequence_base_defs_fragment(
        out,
        MERMAID_SEQUENCE_EXTRA_MARKER_DEFS_PINNED,
        diagram_id,
    );
}

fn sequence_diagram_id_occurrences(
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    mirror_actors: bool,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<Option<usize>> {
    let root_occurrences = 1usize
        .checked_add(model.acc_title.is_some().then_some(2).unwrap_or(0))
        .and_then(|count| count.checked_add(model.acc_descr.is_some().then_some(2).unwrap_or(0)));
    let Some(mut occurrences) = root_occurrences
        .and_then(|count| count.checked_add(SEQUENCE_CSS_DIAGRAM_ID_OCCURRENCES))
        .and_then(|count| count.checked_add(SEQUENCE_SCOPED_BASE_DEF_IDS.len()))
    else {
        return Ok(None);
    };

    for (actor_index, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_index)?;
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        if actor.actor_type != "control" {
            continue;
        }

        if nodes_by_id.contains_key(format!("actor-top-{actor_id}").as_str()) {
            let Some(next) = occurrences.checked_add(2) else {
                return Ok(None);
            };
            occurrences = next;
        }
        if mirror_actors && nodes_by_id.contains_key(format!("actor-bottom-{actor_id}").as_str()) {
            let Some(next) = occurrences.checked_add(2) else {
                return Ok(None);
            };
            occurrences = next;
        }
    }
    checkpoints.checkpoint()?;

    let Some(message_occurrences) =
        sequence_message_diagram_id_occurrences(model, edges_by_id, checkpoints)?
    else {
        return Ok(None);
    };
    Ok(occurrences.checked_add(message_occurrences))
}

fn preflight_sequence_diagram_id(
    diagram_id: &str,
    model: &SequenceSvgModel,
    nodes_by_id: &FxHashMap<&str, &LayoutNode>,
    edges_by_id: &FxHashMap<&str, &crate::model::LayoutEdge>,
    mirror_actors: bool,
    checkpoints: SequenceEmitCheckpoints<'_>,
    work_meter: &crate::resources::OperationWorkMeter,
) -> Result<()> {
    let Some(occurrences) = sequence_diagram_id_occurrences(
        model,
        nodes_by_id,
        edges_by_id,
        mirror_actors,
        checkpoints,
    )?
    else {
        return Err(work_meter
            .terminate_svg_byte_count_overflow(ResourceLimitPhase::SvgOutput, OperationPhase::Emit)
            .into());
    };
    let Some(projected_bytes) = diagram_id.len().checked_mul(occurrences) else {
        return Err(work_meter
            .terminate_svg_byte_count_overflow(ResourceLimitPhase::SvgOutput, OperationPhase::Emit)
            .into());
    };
    work_meter
        .preflight_svg_byte_count(
            projected_bytes,
            ResourceLimitPhase::SvgOutput,
            OperationPhase::Emit,
        )
        .map_err(Into::into)
}

pub(in crate::svg::parity) fn render_sequence_diagram_svg_model_with_config(
    prepared: &crate::sequence::SequencePreparedArtifact,
    model: &SequenceSvgModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_sequence_diagram_svg_inner(
        prepared,
        model,
        effective_config.as_value(),
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

fn render_sequence_diagram_svg_inner(
    prepared: &crate::sequence::SequencePreparedArtifact,
    model: &SequenceSvgModel,
    effective_config: &serde_json::Value,
    sanitize_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let checkpoints = SequenceEmitCheckpoints::new(options.work_meter());
    checkpoints.checkpoint()?;
    let layout = prepared.layout();
    let effective_title =
        crate::sequence::sequence_render_title(model.title.as_deref(), diagram_title);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let settings = SequenceRenderSettings::from_effective_config(effective_config);

    let mut nodes_by_id: FxHashMap<&str, &LayoutNode> =
        FxHashMap::with_capacity_and_hasher(layout.nodes.len(), Default::default());
    for (node_index, node) in layout.nodes.iter().enumerate() {
        checkpoints.checkpoint_loop(node_index)?;
        nodes_by_id.insert(node.id.as_str(), node);
    }

    let mut edges_by_id: FxHashMap<&str, &crate::model::LayoutEdge> =
        FxHashMap::with_capacity_and_hasher(layout.edges.len(), Default::default());
    for (edge_index, edge) in layout.edges.iter().enumerate() {
        checkpoints.checkpoint_loop(edge_index)?;
        edges_by_id.insert(edge.id.as_str(), edge);
    }
    checkpoints.checkpoint()?;

    preflight_sequence_diagram_id(
        diagram_id,
        model,
        &nodes_by_id,
        &edges_by_id,
        settings.mirror_actors,
        checkpoints,
        options.work_meter(),
    )?;

    let mut out = String::new();
    checkpoints.checkpoint()?;
    let root_metrics = write_sequence_svg_root_open(&mut out, layout, model, diagram_id)?;

    render_sequence_box_frames_and_rect_blocks(
        &mut out,
        model,
        &nodes_by_id,
        SequenceFrameRenderOptions {
            actor_label_font_size: settings.actor_label_font_size,
            box_margin: settings.box_margin,
            box_text_margin: settings.box_text_margin,
            rect_default_fill: &settings.rect_default_fill,
        },
        checkpoints,
    )?;

    let actor_ctx = SequenceActorRenderContext {
        model,
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        sanitize_config,
        math_renderer: options.math_renderer(),
        actor_wrap_width: settings.actor_wrap_width,
        actor_height: settings.actor_height,
        label_box_height: settings.label_box_height,
        measurer,
        loop_text_style: &settings.loop_text_style,
        checkpoints,
    };

    if settings.mirror_actors {
        render_sequence_bottom_actors(&mut out, &actor_ctx)?;
    }

    // Top actors + lifelines.
    render_sequence_top_actors_and_lifelines(&mut out, &actor_ctx)?;

    checkpoints.checkpoint()?;
    let _ = write!(
        &mut out,
        r#"<style>{}</style><g/>"#,
        sequence_css(diagram_id, settings.actor_label_font_size, effective_config)
    );

    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    checkpoints.checkpoint()?;
    write_scoped_sequence_base_defs(&mut out, diagram_id);

    render_sequence_actor_man_tops(
        &mut out,
        model,
        &nodes_by_id,
        settings.actor_height,
        diagram_id,
        checkpoints,
    )?;

    let block_widths_by_id = crate::sequence::sequence_block_widths_for_render(
        model,
        prepared,
        &nodes_by_id,
        sanitize_config,
        measurer,
        options.math_renderer(),
        options.work_meter(),
    )?;

    let interaction_ctx = SequenceInteractionRenderContext {
        model,
        block_widths_by_id: &block_widths_by_id,
        block_layouts_by_id: &layout.block_layouts_by_id,
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        sanitize_config,
        math_renderer: options.math_renderer(),
        settings: &settings,
        measurer,
        checkpoints,
    };
    render_sequence_interaction_overlays(&mut out, &interaction_ctx)?;

    let message_ctx = SequenceMessageRenderContext {
        model,
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        sanitize_config,
        math_renderer: options.math_renderer(),
        measurer,
        message_align: settings.message_align.as_str(),
        diagram_id,
        actor_height: settings.actor_height,
        actor_label_font_size: settings.actor_label_font_size,
        sequence_width: settings.sequence_width,
        activation_width: settings.activation_width,
        wrap_padding: settings.wrap_padding,
        right_angles: settings.right_angles,
        loop_text_style: &settings.loop_text_style,
        checkpoints,
    };
    render_sequence_messages(&mut out, &message_ctx)?;

    render_sequence_actor_popup_menus(
        &mut out,
        model,
        &nodes_by_id,
        sanitize_config,
        SequenceActorPopupOptions {
            force_menus: settings.force_menus,
            mirror_actors: settings.mirror_actors,
            actor_height: settings.actor_height,
        },
        checkpoints,
    )?;

    if settings.mirror_actors {
        render_sequence_actor_man_bottoms(
            &mut out,
            model,
            &nodes_by_id,
            settings.actor_height,
            settings.label_box_height,
            diagram_id,
            checkpoints,
        )?;
    }

    if let Some(title) = effective_title {
        // Mermaid sequence titles are currently emitted as a plain `<text>` node.
        // Mermaid positions the title using the inner (content) box width:
        // `x = (box.stopx - box.startx) / 2 - 2 * diagramMarginX`.
        let title_x = ((root_metrics.viewbox_width - 2.0 * settings.diagram_margin_x) / 2.0)
            - 2.0 * settings.diagram_margin_x;
        checkpoints.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="-25">{text}</text>"#,
            x = fmt(title_x),
            text = escape_xml(title)
        );
    }

    checkpoints.checkpoint()?;
    out.push_str("</svg>\n");
    checkpoints.checkpoint()?;
    let rooted = root_metrics.document.complete(out)?;
    checkpoints.checkpoint()?;
    Ok(rooted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_defs_are_scoped_in_one_output_pass() {
        let mut defs = String::new();
        write_scoped_sequence_base_defs(&mut defs, "sequence-root");

        for local_id in SEQUENCE_SCOPED_BASE_DEF_IDS {
            assert!(
                defs.contains(&format!(r#"id="sequence-root-{local_id}""#)),
                "missing scoped definition {local_id}"
            );
            assert!(
                !defs.contains(&format!(r#"id="{local_id}""#)),
                "bare definition {local_id} survived scoping"
            );
        }
    }
}
