//! Flowchart node renderer.

use super::super::*;
use crate::svg::parity::timing::RenderTiming;

pub(in crate::svg::parity::flowchart) mod emission;
pub(in crate::svg::parity::flowchart) mod geom;
pub(in crate::svg::parity::flowchart) mod helpers;
mod label;
pub(in crate::svg::parity) mod roughjs;
pub(in crate::svg::parity::flowchart) mod shapes;

pub(in crate::svg::parity::flowchart) use helpers::compute_node_label_metrics;
pub(in crate::svg::parity::flowchart::render) struct FlowchartNodeRenderCommon<'a> {
    pub node_id: &'a str,
    pub shape: &'a str,
    pub look: &'a str,
    pub layout_node: &'a crate::model::LayoutNode,
    pub node_classes: &'a [String],
    pub node_styles: &'a [String],
    pub node_icon: Option<&'a str>,
    pub node_img: Option<&'a str>,
    pub node_pos: Option<&'a str>,
    pub node_constraint: Option<&'a str>,
    pub node_asset_width: Option<f64>,
    pub node_asset_height: Option<f64>,
    label_emission: &'a label::FlowchartNodeLabelEmissionPlan<'a>,
    pub style: &'a str,
    pub effect_filter_attr: &'a str,
    /// Theme-only declarations for no-label surfaces that do not consume the complete source
    /// style string (notably flowchart-v2 start nodes).
    pub theme_style: &'a str,
    pub label_style: &'a str,
    pub rough_group_style: &'a str,
    pub fill_color: &'a str,
    pub stroke_color: &'a str,
    pub stroke_width: f32,
    pub stroke_dasharray: &'a str,
    pub typed_corner_radius: Option<f64>,
    pub source_corner_radii: [Option<f64>; 2],
    pub neo_corner_radius: f64,
    pub hand_drawn_seed: &'a roughr::core::RoughRandomness,
    pub work_meter: &'a crate::resources::OperationWorkMeter,
    pub wrapped_in_a: bool,
    pub timing: RenderTiming,
}

impl FlowchartNodeRenderCommon<'_> {
    pub(super) fn write_rectangle_radii(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
        fallback: Option<f64>,
    ) {
        for (axis, source) in ["rx", "ry"].into_iter().zip(self.source_corner_radii) {
            if let Some(radius) = source.or(fallback) {
                let _ = write!(
                    out,
                    " {axis}=\"{}\"",
                    crate::svg::parity::fmt_display(radius)
                );
            }
        }
    }

    pub(super) fn write_rectangle_corner_style(
        &self,
        out: &mut impl crate::svg::parity::SvgOutput,
    ) {
        // Keep admitted source winners and typed geometry consistent with native attributes.
        // Neo defaults and differently cased source spellings must not override them in CSS.
        for (axis, source) in ["rx", "ry"].into_iter().zip(self.source_corner_radii) {
            if let Some(radius) = source.or(self.typed_corner_radius) {
                let radius = crate::svg::parity::fmt_display(radius);
                let _ = write!(out, ";{axis}:{radius}px !important");
            }
        }
    }

    pub(super) fn look_is_neo(&self) -> bool {
        self.look == "neo"
    }

    pub(super) fn look_is_hand_drawn(&self) -> bool {
        self.look == "handDrawn"
    }
}

fn flowchart_hand_drawn_shape_group_style(inline_styles: &[String]) -> String {
    let mut node_decls: Vec<String> = Vec::new();
    let mut text_decls: Vec<String> = Vec::new();

    for raw in inline_styles {
        for decl in crate::flowchart::flowchart_split_mermaid_style_decls(raw) {
            let Some((key, value)) = crate::mermaid_style::parse_safe_style_decl(decl) else {
                continue;
            };
            if is_text_style_key(key) {
                text_decls.push(format!("{key}:{value}"));
            } else {
                node_decls.push(format!("{key}:{value} !important"));
            }
        }
    }

    if node_decls.is_empty() {
        text_decls.join(";")
    } else {
        node_decls.join(";")
    }
}

pub(in crate::svg::parity::flowchart::render) struct FlowchartNodeLabelState<'a> {
    pub text: &'a str,
    pub label_type: &'a str,
    pub dx: f64,
    pub dy: f64,
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_node(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing: RenderTiming,
    details: &mut FlowchartRenderDetails,
) -> crate::Result<()> {
    let Some(layout_node) = ctx.layout_nodes_by_id.get(node_id) else {
        return Ok(());
    };

    let x = layout_node.x + ctx.tx - origin_x;
    let y = layout_node.y + ctx.ty - origin_y;

    if let Some(dom_id) = ctx.document_ids.synthetic_label(node_id)
        && helpers::try_render_self_loop_label_placeholder(
            out,
            node_id,
            dom_id,
            x,
            y,
            ctx.node_html_labels,
        )
    {
        return Ok(());
    }

    let Some(resolved) = helpers::resolve_node_render_info(ctx, node_id) else {
        return Ok(());
    };

    let tooltip = ctx.tooltips.get(node_id).map(|s| s.as_str()).unwrap_or("");
    let tooltip_enabled = !tooltip.trim().is_empty();

    let look = flowchart_config_look(ctx.config);
    let node_dom_id = ctx
        .document_ids
        .node(node_id)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!("missing prepared Flowchart DOM id for node `{node_id}`"),
        })?;
    let class_attr_base = if look == "handDrawn" {
        match resolved.class_attr_base {
            "node default" => "rough-node default",
            "node" => "rough-node",
            other => other,
        }
    } else {
        resolved.class_attr_base
    };
    let wrapped_in_a = resolved.wrapped_in_a;
    let href = resolved.href;
    let target = resolved.target;
    let shape: &str = resolved.shape;
    let node_icon = resolved.node_icon;
    let node_img = resolved.node_img;
    let node_pos = resolved.node_pos;
    let node_constraint = resolved.node_constraint;
    let node_asset_width = resolved.node_asset_width;
    let node_asset_height = resolved.node_asset_height;
    let node_styles = resolved.node_styles;
    let node_classes = resolved.node_classes;

    let empty_classes: &[String] = &[];
    let node_classes_for_wrapper = match shape {
        // Mermaid flowchart-v2 start/stop nodes do not carry classDef classes on the wrapper.
        // Styling is applied via inline styles on the shape paths (stop) or ignored (start).
        "sm-circ" | "small-circle" | "start" | "fr-circ" | "framed-circle" | "stop" => {
            empty_classes
        }
        _ => node_classes,
    };
    let wrapper_classes =
        helpers::NodeWrapperClasses::new(class_attr_base, node_classes_for_wrapper);
    helpers::open_node_wrapper(
        out,
        helpers::NodeWrapperAttrs {
            dom_id: node_dom_id,
            data_id: node_id,
            classes: wrapper_classes,
            wrapped_in_a,
            href: href.as_ref(),
            target,
            x,
            y,
            tooltip_enabled,
            tooltip,
            look,
            color_slot: super::super::agentflow::container_color_slot(ctx, node_id),
        },
    );
    ctx.checkpoint_emit()?;

    let style_start = timing.start();
    let prepared_effect = ctx.node_effects.get().and_then(|plan| plan.node(node_id));
    let mut fallback_source;
    let (compiled_styles, mut style) = if let Some(prepared) = prepared_effect {
        (
            &prepared.source,
            std::borrow::Cow::Borrowed(prepared.source.node_style.as_str()),
        )
    } else {
        fallback_source =
            flowchart_compile_node_styles(ctx.class_defs, node_classes, node_styles, &[]);
        let style = std::mem::take(&mut fallback_source.node_style);
        (&fallback_source, std::borrow::Cow::Owned(style))
    };
    if let Some(s) = style_start {
        details.node_style_compile += s.elapsed();
    }
    let fill_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_fill_status(),
        ctx.node_fill_config_override,
    );
    let stroke_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_stroke_status(),
        ctx.node_border_config_override,
    );
    let stroke_width_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_stroke_width_status(),
        ctx.node_stroke_width_config_override,
    );
    let stroke_dasharray_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_stroke_dasharray_status(),
        false,
    );
    let radius_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_radius_status(),
        ctx.node_corner_radius_config_override,
    );
    let font_stack_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_font_stack_status(),
        ctx.node_typography_config_ownership.font_stack,
    );
    let font_size_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_font_size_status(),
        ctx.node_typography_config_ownership.font_size,
    );
    let label_fill_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
        compiled_styles.source_label_foreground_status(),
        ctx.node_label_fill_config_override,
    );
    let fallback_theme;
    let node_theme = if let Some(prepared) = prepared_effect {
        &prepared.style
    } else {
        fallback_theme = crate::flowchart::FlowchartNodeThemeStyle::resolve(
            ctx.resolved_theme,
            ctx.node_theme_ordinals.get(node_id).copied(),
            ctx.work_meter,
        )?;
        &fallback_theme
    };
    let source_filter = compiled_styles.source_filter_status();
    let typed_fill_selected = node_theme.fill_value(fill_precedence, true).is_some();
    let typed_stroke_selected = node_theme.stroke_value(stroke_precedence, true).is_some();
    let typed_stroke_width_selected = node_theme
        .stroke_width_value(stroke_width_precedence, true)
        .is_some();
    let typed_stroke_dasharray_selected = node_theme
        .stroke_dasharray_value(stroke_dasharray_precedence, true)
        .is_some();
    let typed_radius = node_theme.radius_value(radius_precedence, true);
    let typed_radius_selected = typed_radius.is_some();
    let typed_font_stack_selected = node_theme.font_stack_selected(font_stack_precedence);
    let typed_font_size_selected = node_theme.font_size_selected(font_size_precedence);
    let typed_label_fill = node_theme.label_fill_value(label_fill_precedence, true);
    let mut theme_style = String::new();
    node_theme.append_inline_style(
        &mut theme_style,
        fill_precedence,
        stroke_precedence,
        stroke_width_precedence,
        stroke_dasharray_precedence,
        true,
        true,
        true,
        true,
    );
    if !theme_style.is_empty() {
        if !style.is_empty() {
            style.to_mut().push(';');
        }
        style.to_mut().push_str(&theme_style);
    }
    let rough_group_style = flowchart_hand_drawn_shape_group_style(node_styles);
    let fill_color = compiled_styles
        .fill
        .as_deref()
        .or_else(|| node_theme.fill_value(fill_precedence, true))
        .unwrap_or(ctx.node_fill_color.as_str());
    let stroke_color = compiled_styles
        .stroke
        .as_deref()
        .or_else(|| node_theme.stroke_value(stroke_precedence, true))
        .unwrap_or(ctx.node_border_color.as_str());
    let stroke_width = compiled_styles
        .admitted_stroke_width_value()
        .or_else(|| {
            ctx.node_stroke_width_config_override
                .then_some(ctx.node_stroke_width)
        })
        .or_else(|| node_theme.stroke_width_value(stroke_width_precedence, true))
        .unwrap_or(1.3);
    let stroke_dasharray = compiled_styles
        .stroke_dasharray
        .as_deref()
        .or_else(|| node_theme.stroke_dasharray_value(stroke_dasharray_precedence, true))
        .unwrap_or("0 0")
        .trim();
    let label_emission = label::FlowchartNodeLabelEmissionPlan::new(
        node_classes,
        node_styles,
        compiled_styles,
        typed_label_fill,
    );

    let effect_application = prepared_effect
        .and_then(|prepared| prepared.shadow.zip(prepared.style.effect()))
        .map(|(materialized, effect)| {
            let id = format!("{}-theme-effect-{}", node_dom_id, effect.id());
            let reference = crate::svg::parity::shadow::write_theme_shadow_application(
                out,
                &id,
                effect,
                materialized.region(),
            );
            let attribute = format!(r#" filter="{}""#, escape_attr(&reference));
            (id, effect, materialized.region(), attribute)
        });
    let common = FlowchartNodeRenderCommon {
        node_id,
        shape,
        look,
        layout_node,
        node_classes,
        node_styles,
        node_icon,
        node_img,
        node_pos,
        node_constraint,
        node_asset_width,
        node_asset_height,
        label_emission: &label_emission,
        style: &style,
        effect_filter_attr: effect_application
            .as_ref()
            .map_or("", |(_, _, _, attr)| attr.as_str()),
        theme_style: &theme_style,
        label_style: &compiled_styles.label_style,
        rough_group_style: &rough_group_style,
        fill_color,
        stroke_color,
        stroke_width,
        stroke_dasharray,
        typed_corner_radius: typed_radius.map(f64::from),
        source_corner_radii: compiled_styles.rectangle_source_radii(),
        neo_corner_radius: ctx.node_corner_radius,
        hand_drawn_seed: &ctx.hand_drawn_seed,
        work_meter: ctx.work_meter,
        wrapped_in_a,
        timing,
    };
    let mut label = FlowchartNodeLabelState {
        text: if resolved.label_text_is_node_id {
            node_id
        } else {
            resolved.label_text
        },
        label_type: resolved.label_type,
        dx: 0.0,
        dy: 0.0,
    };

    if ctx.text_surface_paint.generic_text.requested() {
        ctx.work_meter.charge(label.text.len())?;
    }
    let (shape_outcome, no_label) = if shape == "collapsedGroup" {
        // Mermaid appends the separator and ellipsis after the labelHelper output.
        let geometry = shapes::render_collapsed_group_body(out, ctx, &common, &mut label, details);
        out.checkpoint()?;
        let label_receipt =
            label::render_flowchart_node_label_before_tail(out, ctx, &common, &label, details);
        shapes::render_collapsed_group_indicators(out, geometry);
        out.checkpoint()?;
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
        (
            crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeRenderOutcome::new(
                true,
                crate::svg::parity::flowchart::render::node::emission::FlowchartNodeShapeEmissionReceipt::unverified(),
            )
            .with_label(label_receipt),
            false,
        )
    } else if let Some(outcome) = shapes::try_render_flowchart_no_label(out, ctx, &common, details)
    {
        (outcome, true)
    } else {
        (
            shapes::render_flowchart_shape(out, ctx, &common, &mut label, details)?,
            false,
        )
    };
    if let Some((id, effect, region, _)) = &effect_application {
        ctx.effect_evidence.record_application(effect, id, *region);
    }
    let label_receipt = if no_label {
        out.push_str("</g>");
        if common.wrapped_in_a {
            out.push_str("</a>");
        }
        None
    } else if shape_outcome.closes_wrapper() {
        Some(shape_outcome.label())
    } else {
        Some(label::render_flowchart_node_label(
            out, ctx, &common, &label, details,
        ))
    };

    if let Some(label_receipt) = label_receipt {
        ctx.text_surface_paint.background.record_terminal(
            label_receipt.background_has_area(),
            label_receipt.background_has_area(),
            ctx.work_meter,
        )?;
        ctx.text_surface_paint.generic_text.record_label(
            crate::flowchart::FlowchartTextPaintChannel::Node,
            label_receipt
                .text_paint_facts()
                .unwrap_or_else(crate::flowchart::FlowchartTextPaintFacts::unknown),
            compiled_styles.source_label_foreground_status(),
            ctx.work_meter,
        )?;
        ctx.record_base_typography_label_emission(
            crate::flowchart::FlowchartBaseTypographyLabelEmission::new(
                ctx.svg_label_sidecar.and_then(|sidecar| {
                    sidecar.node_owner(node_id, ctx.swimlane_direction.is_some())
                }),
                label_receipt.typography_applicable(),
                label_receipt.typography_verified(),
            )
            .with_source_facets(
                compiled_styles
                    .source_font_stack_status()
                    .merge(label_receipt.html_font_stack_status()),
                compiled_styles
                    .source_font_size_status()
                    .merge(label_receipt.html_font_size_status()),
            )
            .with_target_selection(typed_font_stack_selected, typed_font_size_selected),
        );
    }

    let mut source_evidence =
        compiled_styles.shape_source_evidence(node_id, shape_outcome.emission(), |class_id| {
            wrapper_classes.contains(class_id)
        });
    let (font_stack_emission, font_size_emission, label_fill_emission) =
        label_receipt.map_or((None, None, None), |label_receipt| {
            source_evidence
                .residuals
                .extend(compiled_styles.label_source_residuals(node_id, label_receipt));
            let reach = label_receipt.prepared_typography_reach();
            (
                reach.map(|reach| {
                    crate::flowchart::FlowchartThemeFacetEmission::new(
                        crate::flowchart::FlowchartFacetPrecedence::new(
                            compiled_styles.emitted_source_font_stack_status(label_receipt),
                            ctx.node_typography_config_ownership.font_stack,
                        ),
                        typed_font_stack_selected && reach.is_verified(),
                    )
                }),
                reach.map(|reach| {
                    crate::flowchart::FlowchartThemeFacetEmission::new(
                        crate::flowchart::FlowchartFacetPrecedence::new(
                            compiled_styles.emitted_source_font_size_status(label_receipt),
                            ctx.node_typography_config_ownership.font_size,
                        ),
                        typed_font_size_selected && reach.is_verified(),
                    )
                }),
                label_receipt.label_fill_reach().and_then(|reach| {
                    node_theme.has_label_fill_route().then(|| {
                        crate::flowchart::FlowchartThemeFacetEmission::new(
                            crate::flowchart::FlowchartFacetPrecedence::new(
                                compiled_styles
                                    .emitted_source_label_foreground_status(label_receipt),
                                ctx.node_label_fill_config_override,
                            ),
                            typed_label_fill.is_some() && reach.is_verified(),
                        )
                    })
                }),
            )
        });
    let font_weight_emission = label_receipt.and_then(|receipt| {
        receipt.font_weight_reach().map(|reach| {
            let precedence = crate::flowchart::FlowchartFacetPrecedence::new(
                compiled_styles.emitted_source_font_weight_status(receipt),
                ctx.node_typography_config_ownership.font_weight.is_some() && reach.is_verified(),
            );
            crate::flowchart::FlowchartThemeFacetEmission::new(
                precedence,
                node_theme.font_weight_selected(precedence) && reach.is_verified(),
            )
        })
    });
    if ctx.resolved_theme.is_some() || !source_evidence.residuals.is_empty() {
        ctx.theme_evidence.record_node_emission(
            node_theme,
            crate::flowchart::FlowchartNodeThemeEmission {
                label_effect: label_receipt.and_then(|receipt| receipt.effect_reach()).map(|reach| {
                    crate::flowchart::FlowchartThemeFacetEmission::new(
                        crate::flowchart::FlowchartFacetPrecedence::new(compiled_styles.source_label_filter_status(), false),
                        reach.is_verified() || (node_theme.label_effect_is_cleared()
                            && !ctx.node_html_labels && compiled_styles.label_shadow_source_is_bounded()
                            && super::super::style::label_shadow_structural_styles_are_bounded(ctx.class_defs,
                                &["root", "nodes", "node", "label", "nodeLabel", "text-outer-tspan", "row"])),
                    )
                }),
                effect: crate::flowchart::FlowchartThemeFacetEmission::new(
                    crate::flowchart::FlowchartFacetPrecedence::new(source_filter, false),
                    node_theme.effect().is_none() || effect_application.is_some(),
                ),
                fill: crate::flowchart::FlowchartThemeFacetEmission::new(
                    crate::flowchart::FlowchartFacetPrecedence::new(
                        source_evidence.fill,
                        ctx.node_fill_config_override,
                    ),
                    typed_fill_selected && shape_outcome.emission().typed_fill_verified(),
                ),
                stroke: crate::flowchart::FlowchartThemeFacetEmission::new(
                    crate::flowchart::FlowchartFacetPrecedence::new(
                        source_evidence.stroke,
                        ctx.node_border_config_override,
                    ),
                    typed_stroke_selected && shape_outcome.emission().typed_stroke_verified(),
                ),
                stroke_width: crate::flowchart::FlowchartThemeFacetEmission::new(
                    crate::flowchart::FlowchartFacetPrecedence::new(
                        source_evidence.stroke_width,
                        ctx.node_stroke_width_config_override,
                    ),
                    typed_stroke_width_selected
                        && shape_outcome.emission().typed_stroke_width_verified(),
                ),
                stroke_dasharray: crate::flowchart::FlowchartThemeFacetEmission::new(
                    crate::flowchart::FlowchartFacetPrecedence::new(
                        source_evidence.stroke_dasharray,
                        false,
                    ),
                    typed_stroke_dasharray_selected
                        && shape_outcome.emission().typed_stroke_dasharray_verified(),
                ),
                radius: shape_outcome.emission().typed_radius_emission(
                    crate::flowchart::FlowchartFacetPrecedence::new(
                        source_evidence.radius,
                        ctx.node_corner_radius_config_override,
                    ),
                    typed_radius_selected,
                ),
                label_fill: label_fill_emission,
                font_stack: font_stack_emission,
                font_size: font_size_emission,
                font_weight: font_weight_emission,
            },
            &source_evidence.residuals,
            ctx.work_meter,
        )?;
    }

    // HandDrawn RoughJS generation may reject before the bounded SVG sink sees its output. Replay
    // that terminal here so a fallback shape cannot silently turn a resource rejection into a
    // successful render.
    ctx.checkpoint_emit()?;
    Ok(())
}
