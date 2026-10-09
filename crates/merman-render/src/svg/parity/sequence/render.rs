use super::super::*;
use super::SequenceEmitCheckpoints;
use super::activation::SequenceActivationPlan;
use super::actor_man::{render_sequence_actor_man_bottoms, render_sequence_actor_man_tops};
use super::actor_popup::{SequenceActorPopupOptions, render_sequence_actor_popup_menus};
use super::actor_shapes::{
    ActorFillCoverage, ActorStrokeCoverage, LIFELINE_STROKE_WIDTH_PX, actor_fill_coverage,
    actor_stroke_coverage,
};
use super::actors::{
    SequenceActorRenderContext, render_sequence_bottom_actors,
    render_sequence_top_actors_and_lifelines,
};
use super::frames::{SequenceFrameRenderOptions, render_sequence_box_frames_and_rect_blocks};
use super::interactions::{SequenceInteractionRenderContext, render_sequence_interaction_overlays};
use super::messages::{SequenceMessageRenderContext, render_sequence_messages};
use super::root::write_sequence_svg_root_open;
use super::settings::SequenceRenderSettings;
use rustc_hash::FxHashMap;

use super::css::{SequenceThemeCssAdapter, write_sequence_css_with_theme_adapter};
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
    let mut cursor = 0usize;
    while cursor < fragment.len() {
        let next = SEQUENCE_SCOPED_DEF_IDS
            .iter()
            .filter_map(|&(needle, local_id)| {
                fragment[cursor..]
                    .find(needle)
                    .map(|offset| (cursor + offset, needle, local_id))
            })
            .min_by_key(|&(offset, _, _)| offset);
        let Some((offset, needle, local_id)) = next else {
            out.push_str(&fragment[cursor..]);
            return out.checkpoint();
        };
        out.push_str(&fragment[cursor..offset]);
        let _ = write!(
            out,
            r#"id="{}-{}""#,
            escape_attr_display(diagram_id),
            local_id
        );
        out.checkpoint()?;
        cursor = offset + needle.len();
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

pub(in crate::svg::parity) fn render_sequence_diagram_svg_model_with_config(
    prepared: &crate::sequence::SequencePreparedArtifact,
    model: &SequenceSvgModel,
    effective_config: &merman_core::MermaidConfig,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let sanitize_config = effective_config;
    let effective_config = effective_config.as_value();
    let checkpoints = SequenceEmitCheckpoints::for_emit(options.work_meter());
    checkpoints.checkpoint()?;
    let layout = prepared.layout();
    let compat = prepared.typography().compat_binding();

    let settings =
        SequenceRenderSettings::from_resolved_typography(effective_config, prepared.typography());
    let mut typography_receipt =
        crate::sequence::SequenceTypographyThemeReceipt::from_plan(prepared.typography());
    let note_text_shadow = super::text_effect::SequenceTextShadow::from_prepared(
        options,
        crate::sequence::SequenceTypographyRole::Note,
        prepared.typography().note(),
        &mut typography_receipt,
    );
    let loop_text_shadow = super::text_effect::SequenceTextShadow::from_prepared(
        options,
        crate::sequence::SequenceTypographyRole::Loop,
        prepared.typography().loop_label(),
        &mut typography_receipt,
    );
    let actor_text_shadow = super::text_effect::SequenceTextShadow::from_prepared(
        options,
        crate::sequence::SequenceTypographyRole::Actor,
        prepared.typography().actor(),
        &mut typography_receipt,
    );
    let defer_text_bounds = note_text_shadow.needs_bounds()
        || loop_text_shadow.needs_bounds()
        || actor_text_shadow.needs_bounds();
    let terminal = prepared.terminal_theme();
    let actor_theme = &terminal.actor;
    let lifeline_theme = &terminal.lifeline;
    let message_theme = &terminal.message;
    let keyword_theme = &terminal.keyword;
    let frame_theme = &terminal.frame;
    let note_theme = &terminal.note;
    let mut actor_receipt = actor_theme.fresh_receipt();
    let mut lifeline_receipt = lifeline_theme.fresh_receipt();
    let mut message_receipt = message_theme.fresh_receipt();
    let mut keyword_receipt = keyword_theme.fresh_receipt();
    let mut frame_receipt = frame_theme.fresh_receipt();
    let mut note_receipt = note_theme.fresh_receipt();
    let actor_rect_style = super::actor_shapes::SequenceActorRectStyle {
        stroke_width: actor_theme.stroke_width,
        radius: actor_theme.radius,
    };
    let actor_fill_overridden = actor_theme.fill_overridden;
    let actor_stroke_overridden = actor_theme.stroke_overridden;
    let lifeline_stroke_overridden = lifeline_theme.stroke_overridden;
    let message_stroke_overridden = message_theme.stroke_overridden;
    let keyword_fill_overridden = keyword_theme.fill_overridden;
    let keyword_stroke_overridden = keyword_theme.stroke_overridden;
    let note_fill_overridden = note_theme.fill_overridden;
    let note_stroke_overridden = note_theme.stroke_overridden;
    let note_count = terminal.note_count;
    let sequence_number_theme = prepared.number_theme();
    let sequence_number_fill_overridden = sequence_number_theme.fill_overridden();
    let mut sequence_number_receipt = sequence_number_theme.fresh_receipt();
    let activation_theme = prepared.activation_theme();
    let activation_fill_overridden = activation_theme.fill_overridden;
    let activation_stroke_overridden = activation_theme.stroke_overridden;
    let mut lifeline_paint =
        super::actor_effect::SequenceLifelinePaint::new(lifeline_theme.effect.clone());
    let mut defer_paint_bounds = defer_text_bounds
        || lifeline_paint.has_effect()
        || lifeline_theme.typed_stroke_width.is_some();

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
        message_theme.typed_stroke_width,
        message_theme.effect.clone(),
        &mut message_receipt,
        options,
        settings.right_angles,
        settings.actor_height,
        checkpoints,
    )?;
    let actor_shadows = super::actor_effect::SequenceActorShadowPlan::prepare(
        actor_theme.effect.clone(),
        &nodes_by_id,
        model,
        settings.mirror_actors,
        actor_rect_style,
        compat,
        &mut actor_receipt,
        options,
    )?;
    let note_paint = super::notes::SequenceNotePaintPlan::prepare(
        model,
        &nodes_by_id,
        note_theme.stroke_width,
        note_theme.radius,
        note_theme.effect.clone(),
        &mut note_receipt,
        options,
    )?;
    prepared
        .expected_effect_applications()
        .set(actor_shadows.len() + message_paint.len() + note_paint.len());
    let mut activation_plan = SequenceActivationPlan::new(prepared.activation_geometry());
    let activation_count = prepared.activation_geometry().rect_count();
    let mut activation_receipt = activation_theme.fresh_receipt();

    activation_plan.prepare_paint(
        activation_theme.typed_fill.is_some(),
        activation_theme.typed_stroke.is_some(),
        activation_theme.stroke_width,
        activation_theme.radius,
        activation_theme.effect.clone(),
        &mut activation_receipt,
        options,
    )?;

    defer_paint_bounds |= frame_theme.effect.is_some()
        || frame_theme.stroke_width.is_some()
        || keyword_theme.effect.is_some()
        || keyword_theme.stroke_width.is_some();
    let box_paint_bounds = super::frames::builtin_box_paint_bounds(
        prepared.box_layouts(),
        prepared.box_height(),
        settings.box_margin,
        compat,
        checkpoints,
    )?;
    let diagram_id = options.diagram_id_or("merman");
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document = write_sequence_svg_root_open(
        &mut out,
        layout,
        model,
        diagram_id,
        options.resource_policy(),
        actor_theme.stroke_width.map(f64::from).unwrap_or(0.0) / 2.0,
        &[
            box_paint_bounds.as_ref(),
            actor_shadows.bounds.as_ref(),
            message_paint.bounds.as_ref(),
            note_paint.bounds.as_ref(),
            activation_plan.bounds.as_ref(),
        ],
        defer_paint_bounds,
    )?;

    {
        let actor_ctx = SequenceActorRenderContext {
            text_shadow: &actor_text_shadow,
            rect_style: actor_rect_style,
            geometry_receipt: &actor_receipt,
            shadow_plan: &actor_shadows,
            shadow_evidence: prepared.effect_evidence(),
            model,
            diagram_id,
            compat,
            typed_fill: actor_theme.typed_fill.as_deref(),
            typed_stroke: actor_theme.typed_stroke.as_deref(),
            nodes_by_id: &nodes_by_id,
            edges_by_id: &edges_by_id,
            math_sidecar: prepared.math_sidecar(),
            actor_wrap_width: settings.actor_wrap_width,
            actor_height: settings.actor_height,
            label_box_height: settings.label_box_height,
            measurer,
            actor_text_style: &settings.actor_text_style,
            actor_typography: prepared.typography().actor(),
            typography_receipt: &typography_receipt,
            lifeline_effective_stroke_width: lifeline_theme
                .typed_stroke_width
                .map(f64::from)
                .unwrap_or(LIFELINE_STROKE_WIDTH_PX),
            checkpoints,
        };

        render_sequence_box_frames_and_rect_blocks(
            &mut out,
            model,
            &nodes_by_id,
            SequenceFrameRenderOptions {
                actor_text_style: &settings.actor_text_style,
                box_layouts: prepared.box_layouts(),
                box_title_height: prepared.box_title_height(),
                box_height: prepared.box_height(),
                box_margin: settings.box_margin,
                box_text_margin: settings.box_text_margin,
                rect_default_fill: &settings.rect_default_fill,
            },
            &actor_ctx.label_context(),
            checkpoints,
        )?;
        out.checkpoint()?;

        if settings.mirror_actors {
            render_sequence_bottom_actors(&mut out, &actor_ctx)?;
            out.checkpoint()?;
        }

        // Top actors + lifelines.
        render_sequence_top_actors_and_lifelines(
            &mut out,
            &actor_ctx,
            &mut lifeline_receipt,
            &mut lifeline_paint,
            options,
        )?;
        out.checkpoint()?;
    }

    out.push_str("<style>");
    let css_emission = write_sequence_css_with_theme_adapter(
        &mut out,
        diagram_id,
        settings.actor_label_font_size,
        compat,
        SequenceThemeCssAdapter {
            base_font_family: prepared
                .typography()
                .base_typed_properties()
                .contains(&crate::diagram_theme::ThemeTypographyProperty::FontStack)
                .then(|| prepared.typography().base_font_family_css()),
            base_font_size_px: prepared
                .typography()
                .base_typed_properties()
                .contains(&crate::diagram_theme::ThemeTypographyProperty::FontSize)
                .then(|| prepared.typography().base_font_size_px()),
            actor_fill: actor_theme.typed_fill.as_deref(),
            actor_stroke: actor_theme.typed_stroke.as_deref(),
            lifeline_stroke: lifeline_theme.typed_stroke.as_deref(),
            lifeline_stroke_width: lifeline_theme.typed_stroke_width,
            message_stroke: message_theme.typed_stroke.as_deref(),
            message_stroke_width: message_theme.typed_stroke_width,
            sequence_number_fill: sequence_number_theme.typed_fill(),
            frame_stroke: frame_theme.typed_stroke.as_deref(),
            keyword_fill: keyword_theme.typed_fill.as_deref(),
            keyword_stroke: keyword_theme.typed_stroke.as_deref(),
            note_fill: note_theme.typed_fill.as_deref(),
            note_stroke: note_theme.typed_stroke.as_deref(),
            activation_fill: activation_theme.typed_fill.as_deref(),
            activation_stroke: activation_theme.typed_stroke.as_deref(),
            actor_typography: Some(prepared.typography().actor()),
            message_typography: Some(prepared.typography().message()),
            note_typography: Some(prepared.typography().note()),
            loop_typography: Some(prepared.typography().loop_label()),
        },
    )?;
    sequence_number_receipt.record_stylesheet_emission(
        css_emission.sequence_number_fill(),
        css_emission.typed_sequence_number_fill(),
    );
    for surface in crate::sequence::SequenceTextSurface::ALL {
        let (final_fill, typed_fill) = css_emission.text_surface_fill(surface);
        typography_receipt.record_stylesheet_emission(surface, final_fill, typed_fill);
    }
    keyword_receipt.record_stylesheet_emission(
        css_emission.keyword_fill(),
        css_emission.typed_keyword_fill(),
        css_emission.keyword_stroke(),
        css_emission.typed_keyword_stroke(),
    );
    out.push_str("</style><g/>");
    out.checkpoint()?;
    prepared.theme_evidence().record_lifeline_emission(
        crate::sequence::SequenceLifelineThemeEmission::from_terminal_writer(
            lifeline_theme.typed_stroke.as_deref(),
            lifeline_theme.typed_stroke_width,
            lifeline_theme.typed_stroke_width_won,
            lifeline_theme.selected_property,
            lifeline_stroke_overridden,
            lifeline_receipt,
        ),
    );
    let mut actor_fill_emitted = false;
    let mut actor_fill_unhandled = false;
    let mut actor_stroke_emitted = false;
    let mut actor_stroke_unhandled = false;
    for (actor_index, actor_id) in model.actor_order.iter().enumerate() {
        checkpoints.checkpoint_loop(actor_index)?;
        let Some(actor) = model.actors.get(actor_id) else {
            continue;
        };
        let coverage = actor_fill_coverage(actor);
        match coverage {
            ActorFillCoverage::TypedCss => actor_fill_emitted = true,
            ActorFillCoverage::Unhandled => actor_fill_unhandled = true,
        }
        match actor_stroke_coverage(actor) {
            ActorStrokeCoverage::TypedCss => actor_stroke_emitted = true,
            ActorStrokeCoverage::Unhandled => actor_stroke_unhandled = true,
        }
    }
    actor_fill_emitted &= actor_theme.typed_fill.is_some();
    actor_fill_unhandled &= actor_theme.typed_fill.is_some();
    actor_stroke_emitted &= actor_theme.typed_stroke.is_some();
    actor_stroke_unhandled &= actor_theme.typed_stroke.is_some();
    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    write_scoped_sequence_base_defs(&mut out, diagram_id)?;
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
        typed_fill: actor_theme.typed_fill.as_deref(),
        typed_stroke: actor_theme.typed_stroke.as_deref(),
        translate_y: 0.0,
        shadow: &actor_text_shadow,
        shadow_evidence: prepared.effect_evidence(),
        wrap_width_px: settings.actor_wrap_width,
        measurer,
        style: &settings.actor_text_style,
        typography: prepared.typography().actor(),
        typography_receipt: &typography_receipt,
        math_sidecar: prepared.math_sidecar(),
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

    if frame_theme.typed_stroke.is_some() {
        frame_receipt.record_stylesheet_emission(
            "",
            None,
            css_emission.frame_stroke(),
            frame_theme.typed_stroke.as_deref(),
        );
    }
    let frame_paint = super::control_paint::SequenceControlPaint::new(
        &frame_receipt,
        frame_theme.effect.clone(),
        frame_theme.stroke_width,
        crate::diagram_theme::ThemeTarget::Loop,
        options,
    );
    let keyword_paint = super::control_paint::SequenceControlPaint::new(
        &keyword_receipt,
        keyword_theme.effect.clone(),
        keyword_theme.stroke_width,
        crate::diagram_theme::ThemeTarget::LoopLabelBackground,
        options,
    );
    let interaction_ctx = SequenceInteractionRenderContext {
        sanitize_config,
        compat,
        frame_paint: &frame_paint,
        keyword_paint: &keyword_paint,
        note_text_shadow: &note_text_shadow,
        loop_text_shadow: &loop_text_shadow,
        note_paint: &note_paint,
        shadow_evidence: prepared.effect_evidence(),
        model,
        block_widths_by_id: &block_widths_by_id,
        block_layouts_by_id: &layout.block_layouts_by_id,
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        math_sidecar: prepared.math_sidecar(),
        settings: &settings,
        typography: prepared.typography(),
        block_label_box_metrics: prepared.block_label_box_metrics(),
        measurer,
        typography_receipt: &typography_receipt,
        checkpoints,
    };
    render_sequence_interaction_overlays(
        &mut out,
        &interaction_ctx,
        &activation_plan,
        &mut note_receipt,
        &mut activation_receipt,
    )?;
    out.checkpoint()?;
    prepared.theme_evidence().record_note_emission(
        crate::sequence::SequenceStaticRectThemeEmission::from_terminal_writer(
            note_count,
            note_theme.typed_fill.as_deref(),
            note_fill_overridden,
            note_theme.typed_stroke.as_deref(),
            note_stroke_overridden,
            note_receipt,
        ),
    );
    prepared.theme_evidence().record_activation_emission(
        crate::sequence::SequenceStaticRectThemeEmission::from_terminal_writer(
            activation_count,
            activation_theme.typed_fill.as_deref(),
            activation_fill_overridden,
            activation_theme.typed_stroke.as_deref(),
            activation_stroke_overridden,
            activation_receipt,
        ),
    );

    let actor_ctx = SequenceActorRenderContext {
        text_shadow: &actor_text_shadow,
        rect_style: actor_rect_style,
        geometry_receipt: &actor_receipt,
        shadow_plan: &actor_shadows,
        shadow_evidence: prepared.effect_evidence(),
        model,
        diagram_id,
        compat,
        typed_fill: actor_theme.typed_fill.as_deref(),
        typed_stroke: actor_theme.typed_stroke.as_deref(),
        nodes_by_id: &nodes_by_id,
        edges_by_id: &edges_by_id,
        math_sidecar: prepared.math_sidecar(),
        actor_wrap_width: settings.actor_wrap_width,
        actor_height: settings.actor_height,
        label_box_height: settings.label_box_height,
        measurer,
        actor_text_style: &settings.actor_text_style,
        actor_typography: prepared.typography().actor(),
        typography_receipt: &typography_receipt,
        lifeline_effective_stroke_width: lifeline_theme
            .typed_stroke_width
            .map(f64::from)
            .unwrap_or(LIFELINE_STROKE_WIDTH_PX),
        checkpoints,
    };

    // Mermaid appends glyph actors after overlays, before drawing messages.
    render_sequence_actor_man_tops(&mut out, &actor_ctx, diagram_id)?;

    let message_ctx = SequenceMessageRenderContext {
        sanitize_config,
        model,
        paint_plan: &message_paint,
        shadow_evidence: prepared.effect_evidence(),
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
        typography_receipt: &typography_receipt,
        checkpoints,
    };
    render_sequence_messages(
        &mut out,
        &message_ctx,
        &mut message_receipt,
        &mut sequence_number_receipt,
        Some(css_emission.sequence_number_fill()),
    )?;
    prepared.theme_evidence().record_message_emission(
        crate::sequence::SequenceMessageThemeEmission::from_terminal_writer(
            message_theme.typed_stroke.as_deref(),
            message_theme.typed_stroke_width_won,
            message_stroke_overridden,
            message_theme.selected_property,
            message_receipt,
        ),
    );
    prepared.theme_evidence().record_sequence_number_emission(
        crate::sequence::SequenceNumberLabelThemeEmission::from_terminal_writer(
            sequence_number_theme.typed_fill(),
            sequence_number_fill_overridden,
            sequence_number_receipt,
        ),
    );

    if settings.mirror_actors {
        render_sequence_actor_man_bottoms(&mut out, &actor_ctx, diagram_id)?;
    }

    prepared.theme_evidence().record_actor_emission(
        model.actor_order.len(),
        actor_fill_emitted,
        actor_fill_unhandled,
        actor_fill_overridden,
        actor_stroke_emitted,
        actor_stroke_unhandled,
        actor_stroke_overridden,
        actor_receipt,
    );

    render_sequence_actor_popup_menus(
        &mut out,
        model,
        &nodes_by_id,
        sanitize_config,
        SequenceActorPopupOptions {
            force_menus: settings.force_menus,
            mirror_actors: settings.mirror_actors,
            actor_height: settings.actor_height,
            actor_text_style: &settings.actor_text_style,
        },
        prepared.actor_popup_widths(),
        &actor_labels,
        checkpoints,
    )?;
    out.checkpoint()?;

    if let Some(title) = prepared.diagram_title() {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}">{text}</text>"#,
            x = fmt(title.x()),
            y = fmt(title.y()),
            text = escape_xml_display(title.text())
        );
        out.checkpoint()?;
    }

    checkpoints.checkpoint()?;
    out.push_str("</svg>\n");
    let root_document = if defer_paint_bounds {
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::SEQUENCE, diagram_id)
            .with_resource_policy(options.resource_policy())
            .finish_document(
                &mut out,
                root_document,
                root_svg::RootViewportSpec::responsive(super::root::sequence_root_bounds(
                    layout,
                    actor_theme.stroke_width.map(f64::from).unwrap_or(0.0) / 2.0,
                    &[
                        box_paint_bounds.as_ref(),
                        actor_shadows.bounds.as_ref(),
                        message_paint.bounds.as_ref(),
                        note_paint.bounds.as_ref(),
                        activation_plan.bounds.as_ref(),
                        note_text_shadow.bounds.borrow().as_ref(),
                        loop_text_shadow.bounds.borrow().as_ref(),
                        actor_text_shadow.bounds.borrow().as_ref(),
                        lifeline_paint.bounds.as_ref(),
                        frame_paint.bounds.borrow().as_ref(),
                        keyword_paint.bounds.borrow().as_ref(),
                    ],
                )),
            )?
    } else {
        root_document
    };
    prepared.expected_effect_applications().set(
        actor_shadows.len()
            + message_paint.len()
            + note_paint.len()
            + activation_plan.effect_count()
            + lifeline_paint.emitted_count()
            + frame_paint.len()
            + keyword_paint.len()
            + note_text_shadow.len()
            + loop_text_shadow.len()
            + actor_text_shadow.len(),
    );

    prepared.theme_evidence().record_control_emission(
        crate::diagram_theme::ThemeTarget::LoopLabelBackground,
        crate::sequence::SequenceControlThemeEmission::from_terminal_writer(
            keyword_theme.typed_fill.as_deref(),
            keyword_fill_overridden,
            keyword_theme.typed_stroke.as_deref(),
            keyword_stroke_overridden,
            keyword_receipt,
        ),
    );
    prepared.theme_evidence().record_control_emission(
        crate::diagram_theme::ThemeTarget::Loop,
        crate::sequence::SequenceControlThemeEmission::from_terminal_writer(
            None,
            false,
            frame_theme.typed_stroke.as_deref(),
            keyword_stroke_overridden,
            frame_receipt,
        ),
    );
    let svg = prepared.text_sidecar().bind_terminal_svg(out.finish()?)?;
    typography_receipt.record_terminal_svg(
        &svg,
        diagram_id.semantic_str(),
        options.work_meter(),
    )?;
    prepared
        .theme_evidence()
        .record_typography_emission(typography_receipt);
    let rooted = root_document.complete(svg)?;
    Ok(rooted)
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
        assert!(short.as_str().len() <= expected.len() - 1);
    }
}
