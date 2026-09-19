use super::super::*;
use super::SequenceEmitCheckpoints;
use super::activation::build_sequence_activation_plan;
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
use super::messages::{
    SequenceMessageRenderContext, has_sequence_message_line_candidates, render_sequence_messages,
};
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

    let mut settings = SequenceRenderSettings::from_effective_config(effective_config);
    settings.apply_typography_plan(prepared.typography());
    let mut typography_receipt =
        crate::sequence::SequenceTypographyThemeReceipt::from_plan(prepared.typography());
    let note_text_shadow = super::text_effect::SequenceTextShadow::resolve(
        options,
        crate::sequence::SequenceTypographyRole::Note,
        prepared.typography().note(),
        &mut typography_receipt,
    );
    let loop_text_shadow = super::text_effect::SequenceTextShadow::resolve(
        options,
        crate::sequence::SequenceTypographyRole::Loop,
        prepared.typography().loop_label(),
        &mut typography_receipt,
    );
    let defer_text_bounds = note_text_shadow.needs_bounds() || loop_text_shadow.needs_bounds();
    let actor_fill_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.actorBkg",
    );
    let actor_stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.actorBorder",
    );
    let lifeline_stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.actorLineColor",
    );
    let message_stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.signalColor",
    );
    let sequence_number_fill_overridden =
        merman_core::__private::config_path_overrides_typed_default(
            sanitize_config,
            "themeVariables.sequenceNumberColor",
        );
    let loop_fill_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.labelBoxBkgColor",
    );
    let loop_stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.labelBoxBorderColor",
    );
    let note_fill_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.noteBkgColor",
    );
    let note_stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.noteBorderColor",
    );
    let activation_fill_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.activationBkgColor",
    );
    let activation_stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.activationBorderColor",
    );
    let mut actor_theme =
        resolve_sequence_actor_theme(options, sanitize_config, !model.actor_order.is_empty())?;
    let mut lifeline_theme = resolve_sequence_lifeline_theme(
        options,
        !model.actor_order.is_empty(),
        lifeline_stroke_overridden,
    )?;
    let mut message_theme = resolve_sequence_message_theme(
        options,
        has_sequence_message_line_candidates(model),
        message_stroke_overridden,
    )?;
    let mut sequence_number_theme =
        resolve_sequence_number_theme(options, sequence_number_fill_overridden)?;
    let mut loop_theme =
        resolve_sequence_loop_theme(options, loop_fill_overridden, loop_stroke_overridden)?;
    let note_count = model
        .messages
        .iter()
        .filter(|message| {
            message.semantic_kind() == merman_core::diagrams::sequence::SequenceMessageKind::Note
        })
        .count();
    let mut note_theme = resolve_sequence_static_rect_theme(
        options,
        crate::diagram_theme::ThemeTarget::Note,
        note_count != 0,
        note_fill_overridden,
        note_stroke_overridden,
    )?;

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
        message_theme.effect.take(),
        &mut message_theme.receipt,
        options,
        settings.right_angles,
        settings.actor_height,
        checkpoints,
    )?;
    let actor_shadows = super::actor_effect::SequenceActorShadowPlan::prepare(
        actor_theme.effect.take(),
        &nodes_by_id,
        model,
        settings.mirror_actors,
        actor_theme.rect_style,
        effective_config,
        &mut actor_theme.receipt,
        options,
    )?;
    let note_paint = super::notes::SequenceNotePaintPlan::prepare(
        model,
        &nodes_by_id,
        note_theme.stroke_width,
        note_theme.radius,
        note_theme.effect.take(),
        &mut note_theme.receipt,
        options,
    )?;
    prepared
        .expected_effect_applications()
        .set(actor_shadows.len() + message_paint.len() + note_paint.len());
    let diagram_id = options.diagram_id_or("merman");
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_document = write_sequence_svg_root_open(
        &mut out,
        layout,
        model,
        diagram_id,
        options.resource_policy(),
        actor_theme
            .rect_style
            .stroke_width
            .map(f64::from)
            .unwrap_or(0.0)
            / 2.0,
        &[
            actor_shadows.bounds.as_ref(),
            message_paint.bounds.as_ref(),
            note_paint.bounds.as_ref(),
        ],
        defer_text_bounds,
    )?;

    let activation_plan = build_sequence_activation_plan(
        model,
        &nodes_by_id,
        &edges_by_id,
        settings.activation_width,
        checkpoints,
    )?;
    let activation_count = activation_plan.rect_count();
    let mut activation_theme = resolve_sequence_static_rect_theme(
        options,
        crate::diagram_theme::ThemeTarget::Activation,
        activation_count != 0,
        activation_fill_overridden,
        activation_stroke_overridden,
    )?;

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
        prepared.typography().actor(),
        &typography_receipt,
        checkpoints,
    )?;
    out.checkpoint()?;

    let actor_ctx = SequenceActorRenderContext {
        rect_style: actor_theme.rect_style,
        geometry_receipt: &actor_theme.receipt,
        shadow_plan: &actor_shadows,
        shadow_evidence: prepared.effect_evidence(),
        model,
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

    if settings.mirror_actors {
        render_sequence_bottom_actors(&mut out, &actor_ctx)?;
        out.checkpoint()?;
    }

    // Top actors + lifelines.
    render_sequence_top_actors_and_lifelines(&mut out, &actor_ctx, &mut lifeline_theme.receipt)?;
    out.checkpoint()?;

    out.push_str("<style>");
    let css_emission = write_sequence_css_with_theme_adapter(
        &mut out,
        diagram_id,
        settings.actor_label_font_size,
        effective_config,
        SequenceThemeCssAdapter {
            base_font_family: Some(prepared.typography().base_font_family_css()),
            actor_fill: actor_theme.typed_fill.as_deref(),
            actor_stroke: actor_theme.typed_stroke.as_deref(),
            lifeline_stroke: lifeline_theme.typed_stroke.as_deref(),
            lifeline_stroke_width: lifeline_theme.typed_stroke_width,
            message_stroke: message_theme.typed_stroke.as_deref(),
            message_stroke_width: message_theme.typed_stroke_width,
            sequence_number_fill: sequence_number_theme.typed_fill.as_deref(),
            loop_fill: loop_theme.typed_fill.as_deref(),
            loop_stroke: loop_theme.typed_stroke.as_deref(),
            note_fill: note_theme.typed_fill.as_deref(),
            note_stroke: note_theme.typed_stroke.as_deref(),
            activation_fill: activation_theme.typed_fill.as_deref(),
            activation_stroke: activation_theme.typed_stroke.as_deref(),
            actor_typography: Some(prepared.typography().actor()),
            message_typography: Some(prepared.typography().message()),
            note_typography: Some(prepared.typography().note()),
            loop_typography: Some(prepared.typography().loop_label()),
        },
    );
    sequence_number_theme.receipt.record_stylesheet_emission(
        css_emission.sequence_number_fill(),
        css_emission.typed_sequence_number_fill(),
    );
    for surface in crate::sequence::SequenceTextSurface::ALL {
        let (final_fill, typed_fill) = css_emission.text_surface_fill(surface);
        typography_receipt.record_stylesheet_emission(surface, final_fill, typed_fill);
    }
    loop_theme.receipt.record_stylesheet_emission(
        css_emission.loop_fill(),
        css_emission.typed_loop_fill(),
        css_emission.loop_stroke(),
        css_emission.typed_loop_stroke(),
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
            lifeline_theme.receipt,
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
        actor_theme.record_ordinal(options, actor_index + 1)?;
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
    prepared.theme_evidence().record_actor_emission(
        model.actor_order.len(),
        actor_fill_emitted,
        actor_fill_unhandled,
        actor_fill_overridden,
        actor_stroke_emitted,
        actor_stroke_unhandled,
        actor_stroke_overridden,
        actor_theme.receipt,
    );

    // Mermaid's sequence output includes a shared set of <defs> for icons/markers.
    write_scoped_sequence_base_defs(&mut out, diagram_id)?;

    render_sequence_actor_man_tops(
        &mut out,
        model,
        &nodes_by_id,
        settings.actor_height,
        diagram_id,
        &settings.actor_text_style,
        prepared.math_sidecar(),
        prepared.typography().actor(),
        &typography_receipt,
        checkpoints,
    )?;
    out.checkpoint()?;

    let block_widths_by_id = crate::sequence::sequence_block_widths_for_render(
        model,
        prepared,
        &nodes_by_id,
        sanitize_config,
        measurer,
        options.work_meter(),
    )?;

    let interaction_ctx = SequenceInteractionRenderContext {
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
        &loop_theme.receipt,
        &mut note_theme.receipt,
        &mut activation_theme.receipt,
    )?;
    out.checkpoint()?;
    prepared.theme_evidence().record_loop_emission(
        crate::sequence::SequenceLoopThemeEmission::from_terminal_writer(
            loop_theme.typed_fill.as_deref(),
            loop_fill_overridden,
            loop_theme.typed_stroke.as_deref(),
            loop_stroke_overridden,
            loop_theme.receipt,
        ),
    );
    prepared.theme_evidence().record_note_emission(
        crate::sequence::SequenceStaticRectThemeEmission::from_terminal_writer(
            note_count,
            note_theme.typed_fill.as_deref(),
            note_fill_overridden,
            note_theme.typed_stroke.as_deref(),
            note_stroke_overridden,
            note_theme.receipt,
        ),
    );
    prepared.theme_evidence().record_activation_emission(
        crate::sequence::SequenceStaticRectThemeEmission::from_terminal_writer(
            activation_count,
            activation_theme.typed_fill.as_deref(),
            activation_fill_overridden,
            activation_theme.typed_stroke.as_deref(),
            activation_stroke_overridden,
            activation_theme.receipt,
        ),
    );

    let message_ctx = SequenceMessageRenderContext {
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
        legacy_label_font_size: settings.actor_label_font_size,
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
        &mut message_theme.receipt,
        &mut sequence_number_theme.receipt,
        Some(css_emission.sequence_number_fill()),
    )?;
    prepared.theme_evidence().record_message_emission(
        crate::sequence::SequenceMessageThemeEmission::from_terminal_writer(
            message_theme.typed_stroke.as_deref(),
            message_theme.typed_stroke_width_won,
            message_stroke_overridden,
            message_theme.selected_property,
            message_theme.receipt,
        ),
    );
    prepared.theme_evidence().record_sequence_number_emission(
        crate::sequence::SequenceNumberLabelThemeEmission::from_terminal_writer(
            sequence_number_theme.typed_fill.as_deref(),
            sequence_number_fill_overridden,
            sequence_number_theme.receipt,
        ),
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
        },
        prepared.actor_popup_widths(),
        prepared.typography().actor(),
        &typography_receipt,
        checkpoints,
    )?;
    out.checkpoint()?;

    if settings.mirror_actors {
        render_sequence_actor_man_bottoms(
            &mut out,
            model,
            &nodes_by_id,
            settings.actor_height,
            settings.label_box_height,
            diagram_id,
            &settings.actor_text_style,
            prepared.math_sidecar(),
            prepared.typography().actor(),
            &typography_receipt,
            checkpoints,
        )?;
        out.checkpoint()?;
    }

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
    let root_document = if defer_text_bounds {
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::SEQUENCE, diagram_id)
            .with_resource_policy(options.resource_policy())
            .finish_document(
                &mut out,
                root_document,
                root_svg::RootViewportSpec::responsive(super::root::sequence_root_bounds(
                    layout,
                    actor_theme
                        .rect_style
                        .stroke_width
                        .map(f64::from)
                        .unwrap_or(0.0)
                        / 2.0,
                    &[
                        actor_shadows.bounds.as_ref(),
                        message_paint.bounds.as_ref(),
                        note_paint.bounds.as_ref(),
                        note_text_shadow.bounds.borrow().as_ref(),
                        loop_text_shadow.bounds.borrow().as_ref(),
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
            + note_text_shadow.len()
            + loop_text_shadow.len(),
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
    prepared.theme_evidence().record_unsupported_text_emission(
        options.resolved_theme(),
        prepared.diagram_title().is_some(),
        options.work_meter(),
    )?;
    Ok(rooted)
}

struct SequenceActorThemeResolution {
    rect_style: super::actor_shapes::SequenceActorRectStyle,
    effect: Option<crate::diagram_theme::SvgShadowEffect>,
    typed_fill: Option<String>,
    typed_stroke: Option<String>,
    has_ordinal_routes: bool,
    receipt: crate::sequence::SequenceActorThemeReceipt,
}

#[derive(Default)]
struct SequenceLifelineThemeResolution {
    typed_stroke: Option<String>,
    typed_stroke_width: Option<f32>,
    typed_stroke_width_won: bool,
    selected_property: Option<crate::diagram_theme::ResolvedStyleProperty>,
    receipt: crate::sequence::SequenceLifelineThemeReceipt,
}

#[derive(Default)]
struct SequenceMessageThemeResolution {
    effect: Option<crate::diagram_theme::SvgShadowEffect>,
    typed_stroke: Option<String>,
    typed_stroke_width: Option<f32>,
    typed_stroke_width_won: bool,
    selected_property: Option<crate::diagram_theme::ResolvedStyleProperty>,
    receipt: crate::sequence::SequenceMessageThemeReceipt,
}

#[derive(Default)]
struct SequenceNumberLabelThemeResolution {
    typed_fill: Option<String>,
    receipt: crate::sequence::SequenceNumberLabelThemeReceipt,
}

#[derive(Default)]
struct SequenceLoopThemeResolution {
    typed_fill: Option<String>,
    typed_stroke: Option<String>,
    receipt: crate::sequence::SequenceLoopThemeReceipt,
}

#[derive(Default)]
struct SequenceStaticRectThemeResolution {
    stroke_width: Option<f32>,
    radius: Option<f32>,
    effect: Option<crate::diagram_theme::SvgShadowEffect>,
    typed_fill: Option<String>,
    typed_stroke: Option<String>,
    receipt: crate::sequence::SequenceStaticRectThemeReceipt,
}

impl SequenceActorThemeResolution {
    fn record_ordinal(&mut self, options: &SvgExecution<'_>, ordinal: usize) -> Result<()> {
        if !self.has_ordinal_routes {
            return Ok(());
        }
        let theme = options
            .resolved_theme()
            .expect("ordinal Sequence Actor routes require a resolved theme");
        let style = theme.style_with_work_meter(
            crate::diagram_theme::ThemeTarget::Actor,
            crate::diagram_theme::ThemeVariant::Default,
            Some(ordinal),
            options.work_meter(),
        )?;
        self.receipt.record_ordinal_style(&style);
        Ok(())
    }
}

fn resolve_sequence_actor_theme(
    options: &SvgExecution<'_>,
    sanitize_config: &merman_core::MermaidConfig,
    has_actors: bool,
) -> Result<SequenceActorThemeResolution> {
    use crate::diagram_theme::{
        FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
        FamilyThemeSelectorShape, ThemeTarget, ThemeVariant,
    };

    let Some(theme) = options.resolved_theme() else {
        return Ok(SequenceActorThemeResolution {
            rect_style: Default::default(),
            effect: None,
            typed_fill: None,
            typed_stroke: None,
            has_ordinal_routes: false,
            receipt: crate::sequence::SequenceActorThemeReceipt::default(),
        });
    };
    let has_actor_rule_routes = theme.family_mechanism_routes().iter().any(|route| {
        matches!(
            route.mechanism(),
            FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Actor,
                ..
            }
        )
    });
    let has_typed_actor_fill = theme.family_mechanism_routes().iter().any(|route| {
        route.disposition() == FamilyThemeDisposition::TypedAdapter
            && matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Actor,
                    facet: FamilyThemeRuleFacet::Fill(
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                    ),
                    ..
                }
            )
    });
    let has_typed_actor_stroke = theme.family_mechanism_routes().iter().any(|route| {
        route.disposition() == FamilyThemeDisposition::TypedAdapter
            && matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Actor,
                    facet: FamilyThemeRuleFacet::Stroke(
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                    ),
                    ..
                }
            )
    });
    let has_ordinal_routes = theme.family_mechanism_routes().iter().any(|route| {
        matches!(
            route.mechanism(),
            FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Actor,
                selector: FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                },
                ..
            }
        )
    });
    let mut receipt = crate::sequence::SequenceActorThemeReceipt::default();
    let style = if has_actors && has_actor_rule_routes {
        let style = theme.style_with_work_meter(
            ThemeTarget::Actor,
            ThemeVariant::Default,
            None,
            options.work_meter(),
        )?;
        receipt.record_static_style(&style);
        Some(style)
    } else {
        None
    };
    let typed_fill = (!merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.actorBkg",
    ) && has_typed_actor_fill)
        .then(|| style.as_ref().and_then(|style| css_paint(style.fill())))
        .flatten();
    let typed_stroke = (!merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.actorBorder",
    ) && has_typed_actor_stroke)
        .then(|| style.as_ref().and_then(|style| css_paint(style.stroke())))
        .flatten();
    let width_overridden = merman_core::__private::config_path_overrides_typed_default(
        sanitize_config,
        "themeVariables.strokeWidth",
    );
    receipt.record_stroke_width_override(width_overridden);
    let rect_style = super::actor_shapes::SequenceActorRectStyle {
        stroke_width: style
            .as_ref()
            .and_then(|s| s.stroke_width())
            .filter(|_| !width_overridden),
        radius: style.as_ref().and_then(|s| s.radius()),
    };
    let effect_resolution = style
        .as_ref()
        .map(|style| style.effect_resolution().clone())
        .unwrap_or_default();
    let effect = match theme.resolve_effect(ThemeTarget::Actor, &effect_resolution) {
        None => None,
        Some(crate::diagram_theme::ResolvedThemeEffect::ClearedByRule) => {
            receipt.effect_requested = true;
            receipt.effect_cleared = true;
            None
        }
        Some(resolved) => {
            receipt.effect_requested = true;
            let graph = match resolved {
                crate::diagram_theme::ResolvedThemeEffect::Rule { graph } => graph,
                crate::diagram_theme::ResolvedThemeEffect::Binding { graph, .. } => {
                    receipt.effect_binding_used = true;
                    graph
                }
                crate::diagram_theme::ResolvedThemeEffect::ClearedByRule => unreachable!(),
            };
            let effect = graph.and_then(crate::diagram_theme::SvgShadowEffect::from_graph);
            receipt.effect_unhandled = effect.is_none();
            effect
        }
    };
    Ok(SequenceActorThemeResolution {
        effect,
        rect_style,
        typed_fill,
        typed_stroke,
        has_ordinal_routes,
        receipt,
    })
}

fn resolve_sequence_message_theme(
    options: &SvgExecution<'_>,
    has_lines: bool,
    stroke_overridden: bool,
) -> Result<SequenceMessageThemeResolution> {
    use crate::diagram_theme::{
        FamilyThemeMechanism, FamilyThemeSelectorShape, ResolvedStyleProperty, ThemeTarget,
        ThemeVariant,
    };

    if !has_lines {
        return Ok(SequenceMessageThemeResolution::default());
    }
    let Some(theme) = options.resolved_theme() else {
        return Ok(SequenceMessageThemeResolution::default());
    };
    let has_rule_routes = theme.family_mechanism_routes().iter().any(|route| {
        matches!(
            route.mechanism(),
            FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Message,
                selector: FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default),
                },
                ..
            } | FamilyThemeMechanism::EffectBinding {
                target: ThemeTarget::Message,
                ..
            }
        )
    });
    if !has_rule_routes {
        return Ok(SequenceMessageThemeResolution::default());
    }

    let style = theme.style_with_work_meter(
        ThemeTarget::Message,
        ThemeVariant::Default,
        None,
        options.work_meter(),
    )?;
    let mut receipt = crate::sequence::SequenceMessageThemeReceipt::default();
    receipt.record_static_style(&style);
    // Mermaid has one signalColor for the entire Message surface. A winning stroke is the
    // primary source; when it is absent, a winning fill is projected into that same CSS color.
    // Keep the selected property even when its paint cannot be emitted so evidence can account
    // for the losing facet and fail closed for unsupported winners.
    let selected_property = if style.stroke_resolution().winner().is_some() {
        Some(ResolvedStyleProperty::Stroke)
    } else if style.fill_resolution().winner().is_some() {
        Some(ResolvedStyleProperty::Fill)
    } else {
        None
    };
    let typed_stroke = (!stroke_overridden)
        .then(|| match selected_property {
            Some(ResolvedStyleProperty::Stroke) => typed_static_sequence_stroke(theme, &style),
            Some(ResolvedStyleProperty::Fill) => typed_static_sequence_fill(theme, &style),
            _ => None,
        })
        .flatten();
    let effect = match theme.resolve_effect(ThemeTarget::Message, style.effect_resolution()) {
        None => None,
        Some(crate::diagram_theme::ResolvedThemeEffect::ClearedByRule) => {
            receipt.effect_requested = true;
            receipt.effect_cleared = true;
            None
        }
        Some(resolved) => {
            receipt.effect_requested = true;
            let graph = match resolved {
                crate::diagram_theme::ResolvedThemeEffect::Rule { graph } => graph,
                crate::diagram_theme::ResolvedThemeEffect::Binding { graph, .. } => {
                    receipt.effect_binding_used = true;
                    graph
                }
                crate::diagram_theme::ResolvedThemeEffect::ClearedByRule => unreachable!(),
            };
            let effect = graph.and_then(crate::diagram_theme::SvgShadowEffect::from_graph);
            receipt.effect_unhandled = effect.is_none();
            effect
        }
    };
    Ok(SequenceMessageThemeResolution {
        effect,
        typed_stroke,
        typed_stroke_width: style.stroke_width(),
        typed_stroke_width_won: style.stroke_width_resolution().winner().is_some(),
        selected_property,
        receipt,
    })
}

fn resolve_sequence_number_theme(
    options: &SvgExecution<'_>,
    fill_overridden: bool,
) -> Result<SequenceNumberLabelThemeResolution> {
    use crate::diagram_theme::{FamilyThemeMechanism, ThemeTarget, ThemeVariant};

    let Some(theme) = options.resolved_theme() else {
        return Ok(SequenceNumberLabelThemeResolution::default());
    };
    let mut has_rule_routes = false;
    for route in theme.family_mechanism_routes().iter().copied() {
        let FamilyThemeMechanism::RuleFacet {
            target: ThemeTarget::SequenceNumberLabel,
            ..
        } = route.mechanism()
        else {
            continue;
        };
        has_rule_routes = true;
    }
    if !has_rule_routes {
        return Ok(SequenceNumberLabelThemeResolution::default());
    }

    let style = theme.style_with_work_meter(
        ThemeTarget::SequenceNumberLabel,
        ThemeVariant::Default,
        None,
        options.work_meter(),
    )?;
    let mut receipt = crate::sequence::SequenceNumberLabelThemeReceipt::default();
    receipt.record_static_style(&style);
    let typed_fill = (!fill_overridden)
        .then(|| typed_static_sequence_fill(theme, &style))
        .flatten();
    Ok(SequenceNumberLabelThemeResolution {
        typed_fill,
        receipt,
    })
}

fn resolve_sequence_loop_theme(
    options: &SvgExecution<'_>,
    fill_overridden: bool,
    stroke_overridden: bool,
) -> Result<SequenceLoopThemeResolution> {
    use crate::diagram_theme::{
        FamilyThemeMechanism, FamilyThemeSelectorShape, ThemeTarget, ThemeVariant,
    };

    let Some(theme) = options.resolved_theme() else {
        return Ok(SequenceLoopThemeResolution::default());
    };
    let mut has_rule_routes = false;
    for route in theme.family_mechanism_routes().iter().copied() {
        let FamilyThemeMechanism::RuleFacet {
            target: ThemeTarget::Loop,
            selector:
                FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default),
                },
            ..
        } = route.mechanism()
        else {
            continue;
        };
        has_rule_routes = true;
    }
    if !has_rule_routes {
        return Ok(SequenceLoopThemeResolution::default());
    }

    let style = theme.style_with_work_meter(
        ThemeTarget::Loop,
        ThemeVariant::Default,
        None,
        options.work_meter(),
    )?;
    let mut receipt = crate::sequence::SequenceLoopThemeReceipt::default();
    receipt.record_static_style(&style);
    let typed_fill = (!fill_overridden)
        .then(|| typed_static_sequence_fill(theme, &style))
        .flatten();
    let typed_stroke = (!stroke_overridden)
        .then(|| typed_static_sequence_stroke(theme, &style))
        .flatten();
    Ok(SequenceLoopThemeResolution {
        typed_fill,
        typed_stroke,
        receipt,
    })
}

fn typed_static_sequence_fill(
    theme: &crate::diagram_theme::ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<String> {
    let origin = style.fill_resolution().winner()?;
    let facet =
        crate::diagram_theme::FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(crate::diagram_theme::FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    css_paint(style.fill())
}

fn typed_static_sequence_stroke(
    theme: &crate::diagram_theme::ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<String> {
    let origin = style.stroke_resolution().winner()?;
    let facet =
        crate::diagram_theme::FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(crate::diagram_theme::FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    css_paint(style.stroke())
}

fn resolve_sequence_lifeline_theme(
    options: &SvgExecution<'_>,
    has_lifelines: bool,
    paint_overridden: bool,
) -> Result<SequenceLifelineThemeResolution> {
    use crate::diagram_theme::{
        FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
        FamilyThemeSelectorShape, ResolvedStyleProperty, ThemeTarget, ThemeVariant,
    };

    if !has_lifelines {
        return Ok(SequenceLifelineThemeResolution::default());
    }
    let Some(theme) = options.resolved_theme() else {
        return Ok(SequenceLifelineThemeResolution::default());
    };
    let mut has_rule_routes = false;
    let mut has_typed_fill = false;
    let mut has_typed_stroke = false;
    let mut has_typed_stroke_width = false;
    for route in theme.family_mechanism_routes().iter().copied() {
        let FamilyThemeMechanism::RuleFacet {
            target: ThemeTarget::Lifeline,
            selector:
                FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default),
                },
            facet,
            ..
        } = route.mechanism()
        else {
            continue;
        };
        // Unsupported-only rules still need the terminal winner receipt.
        has_rule_routes = true;
        match facet {
            FamilyThemeRuleFacet::Fill(kind) => {
                has_typed_fill |= route.disposition() == FamilyThemeDisposition::TypedAdapter
                    && matches!(
                        kind,
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                    );
            }
            FamilyThemeRuleFacet::Stroke(kind) => {
                has_typed_stroke |= route.disposition() == FamilyThemeDisposition::TypedAdapter
                    && matches!(
                        kind,
                        FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                    );
            }
            FamilyThemeRuleFacet::StrokeWidth => {
                has_typed_stroke_width =
                    route.disposition() == FamilyThemeDisposition::TypedAdapter;
            }
            _ => {}
        }
    }
    if !has_rule_routes {
        return Ok(SequenceLifelineThemeResolution::default());
    }

    let style = theme.style_with_work_meter(
        ThemeTarget::Lifeline,
        ThemeVariant::Default,
        None,
        options.work_meter(),
    )?;
    let mut receipt = crate::sequence::SequenceLifelineThemeReceipt::default();
    receipt.record_static_style(&style);
    let selected_property = if style.stroke_resolution().winner().is_some() {
        Some(ResolvedStyleProperty::Stroke)
    } else if style.fill_resolution().winner().is_some() {
        Some(ResolvedStyleProperty::Fill)
    } else {
        None
    };
    let typed_stroke = (!paint_overridden)
        .then(|| match selected_property {
            Some(ResolvedStyleProperty::Stroke) if has_typed_stroke => css_paint(style.stroke()),
            Some(ResolvedStyleProperty::Fill) if has_typed_fill => css_paint(style.fill()),
            _ => None,
        })
        .flatten();
    let typed_stroke_width = has_typed_stroke_width
        .then(|| style.stroke_width())
        .flatten();
    // `Clear` intentionally has no CSS override value: every terminal Lifeline writer already
    // emits Mermaid's `0.5px` baseline. Keep the resolved winner separate from the optional CSS
    // value so the same completed actor-line receipt can seal both Value and Clear.
    let typed_stroke_width_won =
        has_typed_stroke_width && style.stroke_width_resolution().winner().is_some();
    Ok(SequenceLifelineThemeResolution {
        typed_stroke,
        typed_stroke_width,
        typed_stroke_width_won,
        selected_property,
        receipt,
    })
}

fn resolve_sequence_static_rect_theme(
    options: &SvgExecution<'_>,
    target: crate::diagram_theme::ThemeTarget,
    has_surfaces: bool,
    fill_overridden: bool,
    stroke_overridden: bool,
) -> Result<SequenceStaticRectThemeResolution> {
    use crate::diagram_theme::{
        FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
        FamilyThemeSelectorShape, ThemeVariant,
    };

    debug_assert!(matches!(
        target,
        crate::diagram_theme::ThemeTarget::Note | crate::diagram_theme::ThemeTarget::Activation
    ));
    if !has_surfaces {
        return Ok(SequenceStaticRectThemeResolution::default());
    }
    let Some(theme) = options.resolved_theme() else {
        return Ok(SequenceStaticRectThemeResolution::default());
    };
    let mut has_rule_routes = false;
    let mut has_typed_fill = false;
    let mut has_typed_stroke = false;
    for route in theme.family_mechanism_routes().iter().copied() {
        let FamilyThemeMechanism::RuleFacet {
            target: route_target,
            selector:
                FamilyThemeSelectorShape::Static {
                    variant: None | Some(ThemeVariant::Default),
                },
            facet,
            ..
        } = route.mechanism()
        else {
            continue;
        };
        if route_target != target {
            continue;
        }
        has_rule_routes = true;
        if route.disposition() != FamilyThemeDisposition::TypedAdapter {
            continue;
        }
        match facet {
            FamilyThemeRuleFacet::Fill(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ) => has_typed_fill = true,
            FamilyThemeRuleFacet::Stroke(
                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
            ) => has_typed_stroke = true,
            _ => {}
        }
    }
    let mut receipt = crate::sequence::SequenceStaticRectThemeReceipt::default();
    let style = if has_rule_routes {
        let style = theme.style_with_work_meter(
            target,
            ThemeVariant::Default,
            None,
            options.work_meter(),
        )?;
        receipt.record_static_style(&style);
        Some(style)
    } else {
        None
    };
    let typed_fill = (!fill_overridden && has_typed_fill)
        .then(|| style.as_ref().and_then(|style| css_paint(style.fill())))
        .flatten();
    let typed_stroke = (!stroke_overridden && has_typed_stroke)
        .then(|| style.as_ref().and_then(|style| css_paint(style.stroke())))
        .flatten();
    let mut effect = None;
    let mut stroke_width = None;
    let mut radius = None;
    if target == crate::diagram_theme::ThemeTarget::Note {
        stroke_width = style.as_ref().and_then(|style| style.stroke_width());
        radius = style.as_ref().and_then(|style| style.radius());
        let resolution = style
            .as_ref()
            .map(|s| s.effect_resolution().clone())
            .unwrap_or_default();
        match theme.resolve_effect(target, &resolution) {
            None => {}
            Some(crate::diagram_theme::ResolvedThemeEffect::ClearedByRule) => {
                receipt.effect_requested = true;
                receipt.effect_cleared = true;
            }
            Some(resolved) => {
                receipt.effect_requested = true;
                let graph = match resolved {
                    crate::diagram_theme::ResolvedThemeEffect::Rule { graph } => graph,
                    crate::diagram_theme::ResolvedThemeEffect::Binding { graph, .. } => {
                        receipt.effect_binding_used = true;
                        graph
                    }
                    crate::diagram_theme::ResolvedThemeEffect::ClearedByRule => unreachable!(),
                };
                effect = graph.and_then(crate::diagram_theme::SvgShadowEffect::from_graph);
                receipt.effect_unhandled = effect.is_none();
            }
        }
    }
    Ok(SequenceStaticRectThemeResolution {
        stroke_width,
        radius,
        effect,
        typed_fill,
        typed_stroke,
        receipt,
    })
}

fn css_paint(paint: Option<&crate::diagram_theme::CanvasPaint>) -> Option<String> {
    use crate::diagram_theme::CanvasPaint;

    match paint? {
        CanvasPaint::Transparent => Some("transparent".to_string()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
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
