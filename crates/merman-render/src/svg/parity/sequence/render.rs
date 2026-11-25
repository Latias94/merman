use super::super::*;
use super::SequenceEmitCheckpoints;
use super::activation::SequenceActivationPlan;
use super::actor_man::{render_sequence_actor_man_bottoms, render_sequence_actor_man_tops};
use super::actor_popup::{SequenceActorPopupOptions, render_sequence_actor_popup_menus};
use super::actors::{
    SequenceActorRenderContext, render_sequence_bottom_actors,
    render_sequence_top_actors_and_lifelines,
};
use super::frames::{SequenceFrameRenderOptions, render_sequence_backgrounds};
use super::interactions::{SequenceInteractionRenderContext, render_sequence_interaction_overlays};
use super::messages::{SequenceMessageRenderContext, render_sequence_messages};
use super::root::write_sequence_svg_root_open;
use super::settings::SequenceRenderSettings;
use kurbo::{Rect, Shape};
use merman_theme_contract::v2::{Elevation, MarkerStyle};
use rustc_hash::FxHashMap;

use super::css::write_sequence_css;
use super::model::*;

const PINNED_MERMAID_SEQUENCE_BASE_DEFS: &str = include_str!("sequence_base_defs_11_16_0.svgfrag");
const MERMAID_SEQUENCE_EXTRA_MARKER_DEFS_PINNED: &str = r#"<defs><marker id="solidTopArrowHead" refX="7.9" refY="7.25" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 0 L 10 8 L 0 8 z"/></marker></defs><defs><marker id="solidBottomArrowHead" refX="7.9" refY="0.75" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 0 L 10 0 L 0 8 z"/></marker></defs><defs><marker id="stickTopArrowHead" refX="7.5" refY="7" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 0 L 7 7" stroke="black" stroke-width="1.5" fill="none"/></marker></defs><defs><marker id="stickBottomArrowHead" refX="7.5" refY="0" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="M 0 7 L 7 0" stroke="black" stroke-width="1.5" fill="none"/></marker></defs>"#;

const SEQUENCE_SCOPED_DEF_IDS: [(&str, &str); 11] = [
    (r#"id="computer""#, "computer"),
    (r#"id="database""#, "database"),
    (r#"id="clock""#, "clock"),
    (r#"id="arrowhead""#, "arrowhead"),
    (r#"id="crosshead""#, "crosshead"),
    (r#"id="filled-head""#, "filled-head"),
    (r#"id="sequencenumber""#, "sequencenumber"),
    (r#"id="solidTopArrowHead""#, "solidTopArrowHead"),
    (r#"id="solidBottomArrowHead""#, "solidBottomArrowHead"),
    (r#"id="stickTopArrowHead""#, "stickTopArrowHead"),
    (r#"id="stickBottomArrowHead""#, "stickBottomArrowHead"),
];

fn write_scoped_sequence_defs_fragment(
    out: &mut impl SvgOutput,
    fragment: &str,
    diagram_id: impl Copy + std::fmt::Display,
) -> Result<()> {
    let mut written = 0;
    for (offset, _) in fragment.match_indices("id=\"") {
        let Some((needle, local_id)) = SEQUENCE_SCOPED_DEF_IDS
            .iter()
            .copied()
            .find(|(needle, _)| fragment[offset..].starts_with(needle))
        else {
            continue;
        };
        out.push_str(&fragment[written..offset]);
        let _ = write!(
            out,
            r#"id="{}-{}""#,
            escape_attr_display(diagram_id),
            local_id
        );
        out.checkpoint()?;
        written = offset + needle.len();
    }
    if written < fragment.len() {
        out.push_str(&fragment[written..]);
        return out.checkpoint();
    }
    Ok(())
}

fn write_scoped_sequence_base_defs(
    out: &mut impl SvgOutput,
    diagram_id: impl Copy + std::fmt::Display,
) -> Result<()> {
    write_scoped_sequence_defs_fragment(out, PINNED_MERMAID_SEQUENCE_BASE_DEFS, diagram_id)?;
    write_scoped_sequence_defs_fragment(out, MERMAID_SEQUENCE_EXTRA_MARKER_DEFS_PINNED, diagram_id)
}

fn write_native_sequence_defs(
    out: &mut impl SvgOutput,
    id: SvgDiagramId<'_>,
    marker: MarkerStyle,
) -> Result<()> {
    let fragment = PINNED_MERMAID_SEQUENCE_BASE_DEFS;
    let Some((symbols, markers)) = fragment.split_once(r#"<defs><marker id="arrowhead""#) else {
        return write_scoped_sequence_base_defs(out, id);
    };
    let Some((_, remaining)) = markers.split_once("</marker></defs>") else {
        return write_scoped_sequence_base_defs(out, id);
    };
    write_scoped_sequence_defs_fragment(out, symbols, id)?;
    let (path, attributes) = match marker {
        MarkerStyle::Classic => ("M -1 0 L 10 5 L 0 10 z", ""),
        MarkerStyle::Stealth => ("M -1 0 L 10 5 L -1 10 L 2 5 Z", ""),
        MarkerStyle::Filled => ("M 0 0 L 10 5 L 0 10 Z", ""),
        MarkerStyle::Open => (
            "M 0 0 L 10 5 L 0 10",
            r#" class="merman-sequence-open-arrow""#,
        ),
    };
    let _ = write!(
        out,
        r#"<defs><marker id="{}-arrowhead" refX="7.9" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto-start-reverse"><path d="{path}"{attributes}/></marker></defs>"#,
        escape_attr_display(id)
    );
    out.checkpoint()?;
    write_scoped_sequence_defs_fragment(out, remaining, id)?;
    write_scoped_sequence_defs_fragment(out, MERMAID_SEQUENCE_EXTRA_MARKER_DEFS_PINNED, id)
}

pub(in crate::svg::parity) fn render_sequence_diagram_svg_model_with_config(
    prepared: &crate::sequence::SequencePreparedArtifact,
    model: &SequenceSvgModel,
    effective_config: &merman_model::presentation::PresentationConfig,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let model = prepared.model_for_render(model);
    let sanitize_config = effective_config;
    let effective_config = effective_config.as_value();
    let checkpoints = SequenceEmitCheckpoints::for_emit(options.work_meter());
    checkpoints.checkpoint()?;
    let layout = prepared.layout();
    let native = prepared.native();
    let compat = prepared.typography().compat_binding();

    let settings =
        SequenceRenderSettings::from_resolved_typography(effective_config, prepared.typography());
    let stroke_width = native.map(|theme| theme.form.stroke_width as f32);
    let radius = native.map(|theme| theme.form.radius as f32);
    let actor_rect_style = super::actor_shapes::SequenceActorRectStyle {
        stroke_width,
        radius,
    };
    // An explicit canvas needs the painted lifeline tail beyond Mermaid's layout viewport.
    let has_root_canvas =
        native.is_some() || options.family.session().canvas_background().is_some();
    let lifeline_stroke_width = super::css::lifeline_effective_stroke_width(stroke_width);
    let mut lifeline_bounds = None;
    let defer_paint_bounds = has_root_canvas;

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
    let message_paint = super::messages::SequenceMessagePaintPlan::prepare(
        model,
        &nodes_by_id,
        &edges_by_id,
        native.map(|theme| theme.form.connector_width as f32),
        settings.right_angles,
        settings.actor_height,
        checkpoints,
    )?;
    let actor_paint_bounds = super::actor_effect::builtin_actor_paint_bounds(
        &nodes_by_id,
        model,
        settings.mirror_actors,
        has_root_canvas,
        actor_rect_style,
        compat,
        options,
    )?;
    let note_paint = super::notes::SequenceNotePaintPlan::prepare(
        model,
        &nodes_by_id,
        stroke_width,
        radius,
        options,
    )?;
    let mut activation_plan =
        SequenceActivationPlan::new(&layout.activations, prepared.activation_geometry());
    activation_plan.prepare_paint(stroke_width, radius, options)?;
    let box_paint_bounds =
        super::frames::builtin_box_paint_bounds(&layout.boxes, compat, checkpoints)?;
    let diagram_id = options.diagram_id_or("merman");
    let root_viewport = options.root_viewport(crate::DiagramFamilyId::SEQUENCE, diagram_id);
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut native_actor_depth_bounds = None;
    let root_document = write_sequence_svg_root_open(
        &mut out,
        layout,
        model,
        diagram_id,
        &root_viewport,
        stroke_width.map(f64::from).unwrap_or(0.0) / 2.0,
        &[
            box_paint_bounds.as_ref(),
            actor_paint_bounds.as_ref(),
            message_paint.bounds.as_ref(),
            note_paint.bounds.as_ref(),
            activation_plan.bounds.as_ref(),
        ],
        defer_paint_bounds,
    )?;

    {
        let actor_ctx = SequenceActorRenderContext {
            rect_style: actor_rect_style,
            model,
            diagram_id,
            compat,
            native: native.is_some(),
            security_level_loose: sanitize_config.get_str("securityLevel") == Some("loose"),
            nodes_by_id: &nodes_by_id,
            edges_by_id: &edges_by_id,
            math_sidecar: prepared.math_sidecar(),
            actor_label_footprints: prepared.actor_label_footprints(),
            actor_wrap_width: settings.actor_wrap_width,
            actor_height: settings.actor_height,
            label_box_height: settings.label_box_height,
            measurer,
            actor_text_style: &settings.actor_text_style,
            actor_typography: prepared.typography().actor(),
            lifeline_effective_stroke_width: lifeline_stroke_width,
            checkpoints,
        };

        render_sequence_backgrounds(
            &mut out,
            model,
            &nodes_by_id,
            SequenceFrameRenderOptions {
                actor_text_style: &settings.actor_text_style,
                boxes: &layout.boxes,
                box_indices: prepared.box_indices(),
                rect_default_fill: &settings.rect_default_fill,
                native_frames: native.map(|theme| (theme, layout.frames.as_slice())),
            },
            &actor_ctx.label_context(),
            checkpoints,
        )?;
        out.checkpoint()?;

        if let Some(native) = native {
            native_actor_depth_bounds =
                write_native_actor_depths(&mut out, native, &actor_ctx, settings.mirror_actors)?;
        }

        if settings.mirror_actors {
            render_sequence_bottom_actors(&mut out, &actor_ctx)?;
            out.checkpoint()?;
        }

        // Top actors + lifelines.
        render_sequence_top_actors_and_lifelines(
            &mut out,
            &actor_ctx,
            has_root_canvas.then_some(&mut lifeline_bounds),
        )?;
        out.checkpoint()?;
    }

    out.push_str("<style>");
    write_sequence_css(
        &mut super::super::output::XmlTextOutput::new(&mut out),
        diagram_id,
        prepared,
    )?;
    out.push_str("</style><g/>");
    out.checkpoint()?;
    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    if let Some(native) = native {
        write_native_sequence_defs(&mut out, diagram_id, native.form.marker_style)?;
    } else {
        write_scoped_sequence_base_defs(&mut out, diagram_id)?;
    }
    if compat.is_neo {
        let flood_color = compat.neo_flood_color;
        let offset = super::super::look_defs::NEO_SHADOW_OFFSET_PX;
        let _ = write!(
            out,
            r#"<defs><filter id="{diagram_id}-drop-shadow" height="130%" width="130%"><feDropShadow dx="{offset}" dy="{offset}" stdDeviation="0" flood-opacity="0.06" flood-color="{flood_color}"/></filter></defs>"#
        );
        out.checkpoint()?;
    }

    let actor_labels = super::actor_shapes::ActorLabelContext {
        compat,
        diagram_id,
        native: native.is_some(),
        wrap_width_px: settings.actor_wrap_width,
        measurer,
        style: &settings.actor_text_style,
        typography: prepared.typography().actor(),
        math_sidecar: prepared.math_sidecar(),
        actor_label_footprints: prepared.actor_label_footprints(),
        actor_index: None,
        checkpoints,
    };
    let block_widths_by_id = crate::sequence::sequence_block_widths_for_render(
        model,
        prepared,
        &nodes_by_id,
        sanitize_config,
        measurer,
        options.work_meter(),
    )?;

    let frame_paint = super::control_paint::SequenceControlPaint::new(stroke_width, false);
    let keyword_paint = super::control_paint::SequenceControlPaint::new(stroke_width, true);
    let interaction_ctx = SequenceInteractionRenderContext {
        sanitize_config,
        compat,
        frame_paint: &frame_paint,
        keyword_paint: &keyword_paint,
        note_paint: &note_paint,
        model,
        block_widths_by_id: &block_widths_by_id,
        frames: &layout.frames,
        frame_placements: prepared.frame_placements(),
        nodes_by_id: &nodes_by_id,
        math_sidecar: prepared.math_sidecar(),
        settings: &settings,
        typography: prepared.typography(),
        block_label_box_metrics: prepared.block_label_box_metrics(),
        measurer,
        checkpoints,
    };
    render_sequence_interaction_overlays(&mut out, &interaction_ctx, &activation_plan)?;
    out.checkpoint()?;

    let actor_ctx = SequenceActorRenderContext {
        rect_style: actor_rect_style,
        model,
        diagram_id,
        compat,
        native: native.is_some(),
        security_level_loose: sanitize_config.get_str("securityLevel") == Some("loose"),
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        math_sidecar: prepared.math_sidecar(),
        actor_label_footprints: prepared.actor_label_footprints(),
        actor_wrap_width: settings.actor_wrap_width,
        actor_height: settings.actor_height,
        label_box_height: settings.label_box_height,
        measurer,
        actor_text_style: &settings.actor_text_style,
        actor_typography: prepared.typography().actor(),
        lifeline_effective_stroke_width: lifeline_stroke_width,
        checkpoints,
    };

    // Mermaid appends glyph actors after overlays, before drawing messages.
    render_sequence_actor_man_tops(&mut out, &actor_ctx, diagram_id)?;

    let message_ctx = SequenceMessageRenderContext {
        sanitize_config,
        model,
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        math_sidecar: prepared.math_sidecar(),
        measurer,
        message_align: settings.message_align.as_str(),
        diagram_id,
        actor_height: settings.actor_height,
        sequence_width: settings.sequence_width,
        activation_width: settings.activation_width,
        wrap_padding: settings.wrap_padding,
        right_angles: settings.right_angles,
        message_text_style: &settings.message_text_style,
        message_typography: prepared.typography().message(),
        checkpoints,
    };
    render_sequence_messages(&mut out, &message_ctx)?;

    if settings.mirror_actors {
        render_sequence_actor_man_bottoms(&mut out, &actor_ctx, diagram_id)?;
    }

    render_sequence_actor_popup_menus(
        &mut out,
        model,
        &nodes_by_id,
        sanitize_config,
        SequenceActorPopupOptions {
            is_neo: compat.is_neo,
            force_menus: settings.force_menus,
            mirror_actors: settings.mirror_actors,
            actor_text_style: &settings.actor_text_style,
        },
        prepared,
        &actor_labels,
        checkpoints,
    )?;
    out.checkpoint()?;

    if let Some(title) = layout.title.as_ref() {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}""#,
            x = fmt(title.x),
            y = fmt(title.y),
        );
        if let Some(native) = native {
            let _ = write!(&mut out, r#" fill="{}""#, native.title);
        }
        if let Some(style) = prepared.typography().native_title_css() {
            let _ = write!(&mut out, r#" style="{}""#, escape_attr(&style));
        }
        let _ = write!(&mut out, ">{}</text>", escape_xml_display(&title.text));
        out.checkpoint()?;
    }

    checkpoints.checkpoint()?;
    out.push_str("</svg>\n");
    let root_document = if defer_paint_bounds {
        root_viewport.finish_document(
            &mut out,
            root_document,
            root_svg::RootViewportSpec::responsive(super::root::sequence_root_bounds(
                layout,
                stroke_width.map(f64::from).unwrap_or(0.0) / 2.0,
                &[
                    box_paint_bounds.as_ref(),
                    actor_paint_bounds.as_ref(),
                    native_actor_depth_bounds.as_ref(),
                    message_paint.bounds.as_ref(),
                    note_paint.bounds.as_ref(),
                    activation_plan.bounds.as_ref(),
                    lifeline_bounds.as_ref(),
                    frame_paint.bounds.borrow().as_ref(),
                    keyword_paint.bounds.borrow().as_ref(),
                ],
            )),
        )?
    } else {
        root_document
    };
    root_document.complete(out.finish()?)
}

fn write_native_actor_depths(
    out: &mut impl SvgOutput,
    native: &crate::sequence::SequenceNativeStyle,
    ctx: &SequenceActorRenderContext<'_>,
    mirror: bool,
) -> Result<Option<Bounds>> {
    if matches!(native.elevation, Elevation::Flat) {
        return Ok(None);
    }
    let mut bounds: Option<Rect> = None;
    for (index, actor_id) in ctx.model.actor_order.iter().enumerate() {
        ctx.checkpoints.checkpoint_loop(index)?;
        if !ctx
            .model
            .actors
            .get(actor_id)
            .is_some_and(super::actor_shapes::actor_rect_geometry_supported)
        {
            continue;
        }
        let placements = if mirror {
            &["top", "bottom"][..]
        } else {
            &["top"][..]
        };
        for placement in placements {
            let id = format!("actor-{placement}-{actor_id}");
            let Some(node) = ctx.nodes_by_id.get(id.as_str()) else {
                continue;
            };
            let (x, y) = super::geometry::node_left_top(node);
            let surface = Rect::from_origin_size((x, y), (node.width, node.height));
            let silhouette = surface.to_rounded_rect(native.form.radius).to_path(0.1);
            let scoped = format!(
                "{}-actor-{index}-{placement}",
                ctx.diagram_id.semantic_str()
            );
            let _ = crate::theme::elevation_svg::write_depth(
                out,
                &native.elevation,
                &scoped,
                &silhouette,
                (0.0, 0.0),
            );
            out.checkpoint()?;
            let depth = crate::theme::elevation_svg::bounds(&native.elevation, surface);
            bounds = Some(bounds.map_or(depth, |bounds| bounds.union(depth)));
        }
    }
    Ok(bounds.map(|bounds| Bounds {
        min_x: bounds.x0,
        min_y: bounds.y0,
        max_x: bounds.x1,
        max_y: bounds.y1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    fn reference_scoped_sequence_base_defs(diagram_id: &str) -> String {
        let mut defs = PINNED_MERMAID_SEQUENCE_BASE_DEFS.to_string();
        defs.push_str(MERMAID_SEQUENCE_EXTRA_MARKER_DEFS_PINNED);
        for &(_, local_id) in &SEQUENCE_SCOPED_DEF_IDS {
            let bare = format!(r#"id="{local_id}""#);
            let scoped = format!(
                r#"id="{}""#,
                escape_attr_display(scoped_svg_id(diagram_id, local_id))
            );
            defs = defs.replace(&bare, &scoped);
        }
        defs
    }

    #[test]
    fn sequence_base_defs_stream_byte_identically_to_the_pinned_scoping_rule() {
        let diagram_id = "sequence<&budget";
        let expected = reference_scoped_sequence_base_defs(diagram_id);
        let mut actual = String::new();

        write_scoped_sequence_base_defs(&mut actual, diagram_id)
            .expect("stream scoped Sequence defs");

        assert_eq!(actual, expected);
    }

    #[test]
    fn sequence_base_defs_reject_before_retaining_one_byte_over_budget() {
        let diagram_id = "sequence-budget";
        let expected = reference_scoped_sequence_base_defs(diagram_id);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len())
            .expect("valid exact SVG limit");
        let exact_meter = OperationWorkMeter::new(exact_policy);
        let mut exact = BoundedSvgOutput::new(&exact_meter);
        write_scoped_sequence_base_defs(&mut exact, diagram_id)
            .expect("exact Sequence defs budget");
        assert_eq!(
            exact.finish().expect("finish exact Sequence defs"),
            expected
        );

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len() - 1)
            .expect("valid short SVG limit");
        let short_meter = OperationWorkMeter::new(short_policy);
        let mut short = BoundedSvgOutput::new(&short_meter);
        assert!(matches!(
            write_scoped_sequence_base_defs(&mut short, diagram_id),
            Err(crate::Error::ResourceLimitExceeded(_))
        ));
        assert!(short.as_str().len() < expected.len());
    }
}
