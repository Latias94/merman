use super::*;
use merman_render_core::{
    config::normalize_css_font_family,
    svg_css::write_mermaid_base_css_root_rule,
    theme_svg::{cssom_color_value, marker::arrow_path},
};

fn write_state_palette_css<I: Copy + std::fmt::Display>(
    out: &mut impl SvgOutput,
    id: I,
    compatibility: &crate::state::StateCompatibilityPlan,
) -> Result<()> {
    if compatibility.redux_palette.is_empty() {
        return out.checkpoint();
    }
    let look = &compatibility.palette_look;
    for (index, (border, tint)) in compatibility.redux_palette.iter().enumerate() {
        for (selector, stroke, fill) in [
            ("rect.outer", true, true),
            ("rect.inner", true, false),
            ("rect.divider", true, true),
            (".outer path[stroke='none']", false, true),
            (".outer path[fill='none']", true, false),
            (".divider path[stroke='none']", false, true),
            (".divider path[fill='none']", true, false),
        ] {
            if !stroke && tint.is_none() {
                continue;
            }
            let _ = write!(
                out,
                r#"#{id} [data-look="{look}"][data-color-id="color-{index}"].statediagram-cluster {selector}{{"#
            );
            if stroke {
                let _ = write!(out, "stroke:{border};");
            }
            if fill && let Some(tint) = tint {
                let _ = write!(out, "fill:{tint};");
            }
            out.push('}');
            out.checkpoint()?;
        }
    }
    Ok(())
}

fn state_shadow_defs<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    compatibility: &crate::state::StateCompatibilityPlan,
) where
    I: SvgDiagramIdValue,
{
    let flood_color = if compatibility.dark_mode {
        "#FFFFFF"
    } else {
        "#000000"
    };
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        escape_attr_display(diagram_id),
        flood_color,
        escape_attr_display(diagram_id),
        flood_color
    );
}

fn state_gradient_defs<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    compatibility: &crate::state::StateCompatibilityPlan,
) where
    I: SvgDiagramIdValue,
{
    if !compatibility.use_gradient {
        return;
    }

    let gradient_start = escape_xml(&compatibility.gradient_start);
    let gradient_stop = escape_xml(&compatibility.gradient_stop);
    let _ = write!(
        out,
        r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient></defs>"#,
        escape_attr_display(diagram_id),
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
}

pub(super) const STATE_BARB_TIP_X: u8 = 19;
pub(super) const STATE_BARB_CENTER_Y: u8 = 7;
pub(super) const STATE_BARB_VIEWPORT_WIDTH: u8 = 20;
const STATE_BARB_VIEWPORT_HEIGHT: u8 = 2 * STATE_BARB_CENTER_Y;

#[derive(Clone, Copy)]
struct StateBarb {
    ref_x: u8,
    rear_x: u8,
    notch_x: u8,
    half_width: u8,
}

const STATE_CLASSIC_BARB: StateBarb = StateBarb {
    ref_x: STATE_BARB_TIP_X,
    rear_x: 9,
    notch_x: 14,
    half_width: 6,
};
const STATE_NEO_BARB: StateBarb = StateBarb {
    ref_x: 17,
    rear_x: 11,
    notch_x: 13,
    half_width: 7,
};

impl StateBarb {
    fn for_look(neo: bool) -> Self {
        if neo {
            STATE_NEO_BARB
        } else {
            STATE_CLASSIC_BARB
        }
    }

    fn glyph_path(self) -> String {
        format!(
            "M {STATE_BARB_TIP_X},{STATE_BARB_CENTER_Y} L{},{} L{},{STATE_BARB_CENTER_Y} L{},{} Z",
            self.rear_x,
            STATE_BARB_CENTER_Y + self.half_width,
            self.notch_x,
            self.rear_x,
            STATE_BARB_CENTER_Y - self.half_width,
        )
    }
}

#[cfg(feature = "layout-shoal")]
pub fn state_canonical_marker(
    arrow_type_end: &str,
    neo: bool,
) -> merman_render_core::route_types::Marker {
    if !matches!(arrow_type_end.trim(), "arrow_barb" | "arrow_barb_neo") {
        return Default::default();
    }
    let barb = StateBarb::for_look(neo);
    merman_render_core::route_types::Marker {
        trim: f64::from(STATE_BARB_TIP_X - barb.ref_x),
        depth: f64::from(barb.ref_x - barb.rear_x),
        width: 2.0 * f64::from(barb.half_width),
    }
}

pub(super) fn state_markers<I>(out: &mut impl SvgOutput, diagram_id: I, ctx: &StateRenderCtx<'_>)
where
    I: SvgDiagramIdValue,
{
    let style_plan = ctx.style_plan;
    let compatibility = style_plan.compatibility();
    let transition_color = compatibility.transition_color.as_str();
    let barb = StateBarb::for_look(compatibility.is_neo());
    let (glyph_path, glyph_attributes) = if let Some(path) = arrow_path(compatibility.native_marker)
    {
        let attributes =
            if compatibility.native_marker == merman_theme_contract::v2::MarkerStyle::Open {
                r#" transform="translate(9 2)" style="fill:none !important;stroke-linecap:round""#
            } else {
                r#" transform="translate(9 2)""#
            };
        (std::borrow::Cow::Borrowed(path), attributes)
    } else {
        (std::borrow::Cow::Owned(barb.glyph_path()), "")
    };
    let ref_x = barb.ref_x;

    if compatibility.is_neo() {
        let _ = write!(
            out,
            r#"<defs><marker id="{}_stateDiagram-barbEnd" refX="{STATE_BARB_TIP_X}" refY="{STATE_BARB_CENTER_Y}" markerWidth="{STATE_BARB_VIEWPORT_WIDTH}" markerHeight="{STATE_BARB_VIEWPORT_HEIGHT}" markerUnits="strokeWidth" orient="auto"><path d="{glyph_path}"{glyph_attributes}/></marker></defs><defs><marker id="{}_stateDiagram-barbEnd-margin" refX="{ref_x}" refY="{STATE_BARB_CENTER_Y}" markerWidth="{STATE_BARB_VIEWPORT_WIDTH}" markerHeight="{STATE_BARB_VIEWPORT_HEIGHT}" markerUnits="userSpaceOnUse" orient="auto"><path d="{glyph_path}" fill="{}"{glyph_attributes}/></marker></defs>"#,
            escape_attr_display(diagram_id),
            escape_attr_display(diagram_id),
            escape_xml(transition_color)
        );
    } else {
        let _ = write!(
            out,
            r#"<defs><marker id="{}_stateDiagram-barbEnd" refX="{ref_x}" refY="{STATE_BARB_CENTER_Y}" markerWidth="{STATE_BARB_VIEWPORT_WIDTH}" markerHeight="{STATE_BARB_VIEWPORT_HEIGHT}" markerUnits="userSpaceOnUse" orient="auto"><path d="{glyph_path}"{glyph_attributes}/></marker></defs>"#,
            escape_attr_display(diagram_id)
        );
    }
}

pub(super) fn state_root_defs<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    style_plan: &crate::state::StateStylePlan,
) where
    I: SvgDiagramIdValue,
{
    state_shadow_defs(out, diagram_id, style_plan.compatibility());
    state_gradient_defs(out, diagram_id, style_plan.compatibility());
}

fn write_state_class_declarations(
    out: &mut impl SvgOutput,
    styles: &[crate::state::StateCssDeclaration],
) -> Result<()> {
    for declaration in styles {
        let semantic_key = declaration.property.as_str();
        let key = declaration.source_name.as_str();
        let value = declaration.value.trim();
        let value = if matches!(
            semantic_key,
            "background"
                | "background-color"
                | "border-color"
                | "color"
                | "fill"
                | "outline-color"
                | "stroke"
        ) {
            std::borrow::Cow::Owned(cssom_color_value(value))
        } else {
            std::borrow::Cow::Borrowed(value)
        };
        if write!(out, "{key}:{value}!important;").is_err() {
            return out.checkpoint();
        }
    }
    Ok(())
}

fn write_state_class_rule(
    out: &mut impl SvgOutput,
    id: impl std::fmt::Display,
    class_id: &str,
    selector: std::fmt::Arguments<'_>,
    styles: &[crate::state::StateCssDeclaration],
) -> Result<()> {
    if write!(out, "#{id} .{class_id}{selector}{{").is_err() {
        return out.checkpoint();
    }
    write_state_class_declarations(out, styles)?;
    out.push('}');
    out.checkpoint()
}

pub(super) fn write_state_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    style_plan: &crate::state::StateStylePlan,
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    out.checkpoint()?;

    let theme = style_plan.compatibility();
    let base_text_style = style_plan.base_text_style();
    let uses_structured_typography = style_plan.uses_structured_typography();
    let ff = if uses_structured_typography {
        base_text_style
            .font_family
            .as_deref()
            .unwrap_or(theme.font_family_css.as_str())
    } else {
        theme.font_family_css.as_str()
    };
    let ff = normalize_css_font_family(ff);
    let font_size = base_text_style.font_size;
    let font_weight_decl = if uses_structured_typography {
        base_text_style
            .font_weight
            .as_deref()
            .map(|value| format!("font-weight:{value};"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let font_style_decl = if uses_structured_typography {
        base_text_style
            .font_style
            .as_deref()
            .map(|value| format!("font-style:{value};"))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let id = CssSelectorDiagramId(diagram_id);
    let text_color = theme.text_color.as_str();
    let error_bkg = theme.error_bkg.as_str();
    let error_text = theme.error_text.as_str();
    let line_color = theme.line_color.as_str();
    let transition_color = theme.transition_color.as_str();
    let node_border = theme.node_border.as_str();
    let state_label_color = theme.state_label_color.as_str();
    let main_bkg = &theme.main_bkg;
    let background = &theme.background;
    let alt_background = theme.alt_background.as_str();
    let stroke_width = &theme.stroke_width;
    let stroke_width_px = &theme.stroke_width_px;
    let neo_stroke_width = &theme.neo_stroke_width;
    let neo_stroke_width_px = &theme.neo_stroke_width_px;
    let note_border = &theme.note_border;
    let note_bkg = &theme.note_bkg;
    let note_text = theme.note_text.as_str();
    let label_background = theme.label_background.as_str();
    let edge_label_background = theme.edge_label_background.as_str();
    let transition_label_color = theme.transition_label_color.as_str();
    let special_state_color = &theme.special_state_color;
    let inner_end_background = &theme.inner_end_background;
    let composite_background = theme.composite_background.as_str();
    let state_bkg = &theme.state_bkg;
    let state_border = &theme.state_border;
    let composite_title_background = theme.composite_title_background.as_str();
    let hand_drawn_state_fill = theme.terminal_paint_css(
        crate::state::StateTerminalSurface::StateHandDrawn,
        crate::state::StateTerminalPaintProperty::Fill,
        diagram_id,
    );
    let hand_drawn_state_stroke = theme.terminal_paint_css(
        crate::state::StateTerminalSurface::StateHandDrawn,
        crate::state::StateTerminalPaintProperty::Stroke,
        diagram_id,
    );
    let hand_drawn_state_stroke_width =
        theme.terminal_stroke_width_value(crate::state::StateTerminalSurface::StateHandDrawn);
    let neo_node_stroke = theme.terminal_paint_css(
        crate::state::StateTerminalSurface::StateNeo,
        crate::state::StateTerminalPaintProperty::Stroke,
        diagram_id,
    );
    let neo_cluster_stroke = theme.terminal_paint_css(
        crate::state::StateTerminalSurface::CompositeBodyNeo,
        crate::state::StateTerminalPaintProperty::Stroke,
        diagram_id,
    );
    let neo_radius = theme.neo_radius;
    let neo_drop_shadow = scoped_drop_shadow(diagram_id, &theme.drop_shadow);

    // Mirrors Mermaid 11.15 `diagrams/state/styles.js` + shared base stylesheet ordering.
    let mut css = out;
    let font_size_s = fmt(font_size);
    let _ = write!(
        &mut css,
        r#"#{}{{font-family:{};font-size:{}px;{}{}fill:{};}}"#,
        id, ff, font_size_s, font_weight_decl, font_style_decl, text_color
    );
    css.push_str("@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}");
    css.push_str("@keyframes dash{to{stroke-dashoffset:0;}}");
    let _ = write!(
        &mut css,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id
    );
    let _ = write!(&mut css, r#"#{} .error-icon{{fill:{};}}"#, id, error_bkg);
    let _ = write!(
        &mut css,
        r#"#{} .error-text{{fill:{};stroke:{};}}"#,
        id, error_text, error_text
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-thickness-normal{{stroke-width:{};}}"#,
        id, neo_stroke_width_px
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-thickness-thick{{stroke-width:3.5px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-pattern-solid{{stroke-dasharray:0;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-pattern-dashed{{stroke-dasharray:3;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .marker{{fill:{};stroke:{};}}"#,
        id, line_color, line_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .marker.cross{{stroke:{};}}"#,
        id, line_color
    );
    let _ = write!(
        &mut css,
        r#"#{} svg{{font-family:{};font-size:{}px;{}{}}}"#,
        id, ff, font_size_s, font_weight_decl, font_style_decl
    );
    let _ = write!(&mut css, r#"#{} p{{margin:0;}}"#, id);
    write_state_palette_css(css, id, theme)?;
    let _ = write!(
        &mut css,
        r#"#{} defs [id$="-barbEnd"]{{fill:{};stroke:{};}}"#,
        id, transition_color, transition_color
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup text{{fill:{};stroke:none;font-size:10px;}}"#,
        id, node_border
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup text{{fill:{};stroke:none;font-size:10px;}}"#,
        id, text_color
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup .state-title{{font-weight:bolder;fill:{};}}"#,
        id, state_label_color
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup rect{{fill:{};stroke:{};}}"#,
        id, main_bkg, node_border
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup line{{stroke:{};stroke-width:{};}}"#,
        id, line_color, stroke_width
    );
    let _ = write!(
        &mut css,
        r#"#{} .transition{{stroke:{};stroke-width:{};fill:none;}}"#,
        id, transition_color, stroke_width
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateGroup .composit{{fill:{};border-bottom:1px;}}"#,
        id, background
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateGroup .alt-composit{{fill:#e0e0e0;border-bottom:1px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .state-note{{stroke:{};fill:{};}}"#,
        id, note_border, note_bkg
    );
    let _ = write!(
        &mut css,
        r#"#{} .state-note text{{fill:{};stroke:none;font-size:10px;}}"#,
        id, note_text
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateLabel .box{{stroke:none;stroke-width:0;fill:{};opacity:0.5;}}"#,
        id, main_bkg
    );
    if theme.native_edge_label_paint.is_some() {
        let _ = write!(
            &mut css,
            r#"#{id} .edgeLabel{{text-align:center;}}#{id} .labelBkg{{background-color:transparent;}}"#,
        );
    } else {
        let _ = write!(
            &mut css,
            r#"#{} .edgeLabel .label rect{{fill:{};opacity:0.5;}}"#,
            id, label_background
        );
        let _ = write!(
            &mut css,
            r#"#{} .edgeLabel{{background-color:{};text-align:center;}}"#,
            id, edge_label_background
        );
        let _ = write!(
            &mut css,
            r#"#{} .edgeLabel p{{background-color:{};}}"#,
            id, edge_label_background
        );
        let _ = write!(
            &mut css,
            r#"#{} .edgeLabel rect{{opacity:0.5;background-color:{};fill:{};}}"#,
            id, edge_label_background, edge_label_background
        );
    }
    let _ = write!(
        &mut css,
        r#"#{} .edgeLabel .label text{{fill:{};}}"#,
        id, transition_label_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .label div .edgeLabel{{color:{};}}"#,
        id, transition_label_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateLabel text{{fill:{};font-size:10px;font-weight:bold;}}"#,
        id, state_label_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .node circle.state-start{{fill:{};stroke:{};}}"#,
        id, special_state_color, special_state_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .node .fork-join{{fill:{};stroke:{};}}"#,
        id, special_state_color, special_state_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .node circle.state-end{{fill:{};stroke:{};stroke-width:1.5;}}"#,
        id, inner_end_background, background
    );
    let _ = write!(
        &mut css,
        r#"#{} .end-state-inner{{fill:{};stroke-width:1.5;}}"#,
        id, composite_background
    );
    let _ = write!(
        &mut css,
        r#"#{} .node rect{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, state_bkg, state_border, stroke_width_px
    );
    let _ = write!(
        &mut css,
        r#"#{} .node polygon{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, main_bkg, state_border, stroke_width_px
    );
    let _ = write!(
        &mut css,
        r#"#{} [id$="-barbEnd"]{{fill:{};}}"#,
        id, line_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster rect{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, composite_title_background, state_border, stroke_width_px
    );
    let _ = write!(
        &mut css,
        r#"#{} .cluster-label,#{} .nodeLabel{{color:{};}}"#,
        id, id, state_label_color
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster rect.outer{{rx:{radius}px;ry:{radius}px;}}"#,
        id,
        radius = crate::state::StateCompatibilityPlan::CLASSIC_COMPOSITE_OUTER_RADIUS,
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-state .divider{{stroke:{};}}"#,
        id, state_border
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-state .title-state{{rx:5px;ry:5px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster.statediagram-cluster .inner{{fill:{};}}"#,
        id, composite_background
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster.statediagram-cluster-alt .inner{{fill:{};}}"#,
        id, alt_background
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster .inner{{rx:0;ry:0;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-state rect.basic{{rx:5px;ry:5px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-state rect.divider{{stroke-dasharray:10,10;fill:{};}}"#,
        id, alt_background
    );
    let _ = write!(&mut css, r#"#{} .note-edge{{stroke-dasharray:5;}}"#, id);
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note rect{{fill:{};stroke:{};stroke-width:1px;rx:0;ry:0;}}"#,
        id, note_bkg, note_border
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note rect{{fill:{};stroke:{};stroke-width:1px;rx:0;ry:0;}}"#,
        id, note_bkg, note_border
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note text{{fill:{};}}"#,
        id, note_text
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note .nodeLabel{{color:{};}}"#,
        id, note_text
    );
    if theme.is_hand_drawn() {
        let _ = write!(
            &mut css,
            r#"#{} [data-look="handDrawn"].node rect{{fill:{};stroke:{};stroke-width:{}px;}}"#,
            id, hand_drawn_state_fill, hand_drawn_state_stroke, hand_drawn_state_stroke_width
        );
        let _ = write!(
            &mut css,
            r#"#{} [data-look="handDrawn"].node line.divider{{stroke:{};stroke-width:{}px;}}"#,
            id, hand_drawn_state_stroke, hand_drawn_state_stroke_width
        );
    }
    let _ = write!(
        &mut css,
        r#"#{} .statediagram .edgeLabel{{color:red;}}"#,
        id
    );
    // State's dependency rule uses `strokeWidth || 1`; the shared edge rule uses `?? 1`.
    let dependency_stroke_width = if theme.dependency_numeric_zero_width {
        "1"
    } else {
        stroke_width.as_str()
    };
    let _ = write!(
        &mut css,
        r#"#{} [id$="-dependencyStart"],#{} [id$="-dependencyEnd"]{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, id, line_color, line_color, dependency_stroke_width
    );
    let title_text_style = style_plan.title_text_style();
    if uses_structured_typography {
        let title_ff =
            normalize_css_font_family(title_text_style.font_family.as_deref().unwrap_or(&ff));
        let title_weight_decl = title_text_style
            .font_weight
            .as_deref()
            .map(|value| format!("font-weight:{value};"))
            .unwrap_or_default();
        let title_style_decl = title_text_style
            .font_style
            .as_deref()
            .map(|value| format!("font-style:{value};"))
            .unwrap_or_default();
        let _ = write!(
            &mut css,
            r#"#{} .statediagramTitleText{{text-anchor:middle;font-family:{};font-size:{}px;{}{}fill:{};}}"#,
            id,
            title_ff,
            fmt(title_text_style.font_size),
            title_weight_decl,
            title_style_decl,
            text_color
        );
    } else {
        let _ = write!(
            &mut css,
            r#"#{} .statediagramTitleText{{text-anchor:middle;font-size:18px;fill:{};}}"#,
            id, text_color
        );
    }
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].statediagram-cluster rect{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, main_bkg, neo_cluster_stroke, neo_stroke_width
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].statediagram-cluster rect.outer{{rx:{}px;ry:{}px;filter:{};}}"#,
        id,
        fmt(neo_radius),
        fmt(neo_radius),
        neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} .node .neo-node{{stroke:{};}}"#,
        id, neo_node_stroke
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node rect,#{} [data-look="neo"].cluster rect,#{} [data-look="neo"].node polygon{{stroke:{};filter:{};}}"#,
        id, id, id, neo_node_stroke, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].swimlane.cluster rect{{filter:none;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node path{{stroke:{};stroke-width:{};}}"#,
        id, neo_node_stroke, neo_stroke_width_px
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node .outer-path{{filter:{};}}"#,
        id, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node .neo-line path{{stroke:{};filter:none;}}"#,
        id, neo_node_stroke
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node circle{{stroke:{};filter:{};}}"#,
        id, neo_node_stroke, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node circle .state-start{{fill:#000000;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].icon-shape .icon{{fill:{};filter:{};}}"#,
        id, neo_node_stroke, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].icon-shape .icon-neo path{{stroke:{};filter:{};}}"#,
        id, neo_node_stroke, neo_drop_shadow
    );
    let root_font_family = if uses_structured_typography {
        ff.clone()
    } else {
        theme.root_font_family_css.clone()
    };
    write_mermaid_base_css_root_rule(css, id, &root_font_family)?;

    css.checkpoint()?;

    if style_plan.classes().next().is_some() {
        let html_labels = theme.html_labels();
        for sc in style_plan.classes() {
            if !sc.styles().is_empty() {
                if html_labels {
                    write_state_class_rule(
                        &mut *css,
                        id,
                        sc.id(),
                        format_args!(">*"),
                        sc.styles(),
                    )?;
                    write_state_class_rule(
                        &mut *css,
                        id,
                        sc.id(),
                        format_args!(" span"),
                        sc.styles(),
                    )?;
                } else {
                    for element in ["rect", "polygon", "ellipse", "circle", "path"] {
                        write_state_class_rule(
                            &mut *css,
                            id,
                            sc.id(),
                            format_args!(" {element}"),
                            sc.styles(),
                        )?;
                    }
                }
            }

            if !sc.text_styles().is_empty() {
                write_state_class_rule(
                    &mut *css,
                    id,
                    sc.id(),
                    format_args!(" tspan"),
                    sc.text_styles(),
                )?;
            }
        }
    }

    css.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::state::StateStyleClass;
    use serde_json::json;

    #[test]
    fn state_rect_radius_preserves_source_fallback_and_zero_override() {
        for (config, expected) in [
            (json!({}), 5.0),
            (json!({"themeVariables": {"radius": null}}), 5.0),
            (
                json!({"look": "neo", "themeVariables": {"radius": 12}}),
                12.0,
            ),
            (
                json!({"look": "classic", "themeVariables": {"radius": 7.5}}),
                7.5,
            ),
            (json!({"themeVariables": {"radius": 0}}), 10.0),
            (json!({"themeVariables": {"radius": "0"}}), 0.0),
            (json!({"themeVariables": {"radius": "7.5"}}), 7.5),
            // Invalid values retain the existing numeric configuration contract.
            (json!({"themeVariables": {"radius": "invalid"}}), 5.0),
        ] {
            assert_eq!(
                style_plan(&StateSvgModel::default(), &config)
                    .compatibility()
                    .rect_radius,
                expected
            );
        }
    }

    fn model_with_hot_class() -> StateSvgModel {
        let mut model = StateSvgModel::default();
        model.style_classes.insert(
            "hot".to_string(),
            StateStyleClass {
                id: "hot".to_string(),
                styles: vec![
                    "fill:#ffdddd".to_string(),
                    "stroke:#d33".to_string(),
                    "stroke-width:2px".to_string(),
                    "color:#222".to_string(),
                ],
                text_styles: vec!["fill:#222".to_string()],
            },
        );
        model
    }

    fn style_plan(
        model: &StateSvgModel,
        config: &serde_json::Value,
    ) -> crate::state::StateStylePlan {
        crate::state::StateStylePlan::resolve_classic(model, config, true)
    }

    fn rendered_state_css(
        diagram_id: &str,
        plan: &crate::state::StateStylePlan,
        _config: &serde_json::Value,
    ) -> String {
        let mut css = String::new();
        write_state_css(&mut css, diagram_id, plan).expect("write State CSS");
        css
    }

    #[test]
    fn state_class_defs_keep_shape_and_text_styles_separate() {
        let model = model_with_hot_class();
        let config = json!({});
        let plan = style_plan(&model, &config);
        let css = rendered_state_css("st", &plan, &config);

        let declarations = "fill:rgb(255, 221, 221)!important;stroke:rgb(221, 51, 51)!important;stroke-width:2px!important;color:rgb(34, 34, 34)!important;";
        assert!(css.ends_with(&format!(
            "#st .hot>*{{{declarations}}}#st .hot span{{{declarations}}}#st .hot tspan{{fill:rgb(34, 34, 34)!important;}}"
        )));
        assert_eq!(css.matches("#st .hot>*").count(), 1);
        assert!(!css.contains("color:rgb(34, 34, 34)!important;fill:rgb(34, 34, 34)!important"));
    }

    #[test]
    fn state_shadow_color_follows_theme_name_not_dark_mode_variables() {
        let model = StateSvgModel::default();
        let light_named_config = json!({
            "theme": "base",
            "darkMode": true,
            "themeVariables": { "darkMode": true },
        });
        let light_named_plan = style_plan(&model, &light_named_config);
        let mut light_named = String::new();
        state_shadow_defs(&mut light_named, "st", light_named_plan.compatibility());
        assert!(light_named.contains(r##"flood-color="#000000""##));

        let dark_named_config = json!({
            "theme": "dark",
            "darkMode": false,
            "themeVariables": { "darkMode": false },
        });
        let dark_named_plan = style_plan(&model, &dark_named_config);
        let mut dark_named = String::new();
        state_shadow_defs(&mut dark_named, "st", dark_named_plan.compatibility());
        assert!(dark_named.contains(r##"flood-color="#FFFFFF""##));
    }

    #[test]
    fn state_class_defs_use_shape_selectors_without_html_labels() {
        let model = model_with_hot_class();
        let config = json!({ "htmlLabels": false });
        let plan = style_plan(&model, &config);
        let css = rendered_state_css("st", &plan, &config);

        for element in ["rect", "polygon", "ellipse", "circle", "path"] {
            assert!(css.contains(&format!("#st .hot {element}{{")));
        }
        assert!(!css.contains("#st .hot>*"));
        assert!(!css.contains("#st .hot span{"));
        assert_eq!(css.matches("#st .hot tspan{").count(), 1);
    }

    #[test]
    fn state_class_defs_do_not_duplicate_font_rules() {
        let mut model = StateSvgModel::default();
        model.style_classes.insert(
            "emphasis".to_string(),
            StateStyleClass {
                id: "emphasis".to_string(),
                styles: vec!["font-style:italic".to_string()],
                text_styles: vec![],
            },
        );

        let config = json!({});
        let plan = style_plan(&model, &config);
        let css = rendered_state_css("st", &plan, &config);
        assert_eq!(css.matches("#st .emphasis>*").count(), 1);
        assert_eq!(css.matches("#st .emphasis span{").count(), 1);
    }

    #[test]
    fn state_css_stops_at_the_class_rule_svg_byte_boundary() {
        use merman_render_core::resources::{
            OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
        };

        let config = json!({});
        let baseline_model = StateSvgModel::default();
        let baseline_plan = style_plan(&baseline_model, &config);
        let baseline_css = rendered_state_css("st", &baseline_plan, &config);

        let model = model_with_hot_class();
        let plan = style_plan(&model, &config);
        let max_svg_bytes = baseline_css.len() + 16;
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, max_svg_bytes)
            .expect("valid State CSS byte ceiling");
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        let error = write_state_css(&mut out, "st", &plan)
            .expect_err("the first class rule must cross the SVG byte ceiling");

        assert!(matches!(
            error,
            merman_render_core::Error::ResourceLimitExceeded(_)
        ));
        assert!(out.len() <= max_svg_bytes);
        assert!(out.as_str().starts_with(baseline_css.as_str()));
        assert!(!out.as_str().contains("#st .hot span{"));
    }

    #[test]
    fn state_css_honors_mermaid_11_16_theme_options() {
        let cfg = json!({
            "themeVariables": {
                "fontFamily": "Inter, Arial",
                "textColor": "#101010",
                "errorBkgColor": "#111111",
                "errorTextColor": "#121212",
                "transitionColor": "#202020",
                "lineColor": "#303030",
                "nodeBorder": "#404040",
                "stateLabelColor": "#505050",
                "mainBkg": "#606060",
                "background": "#707070",
                "altBackground": "#808080",
                "strokeWidth": 4,
                "noteBorderColor": "#909090",
                "noteBkgColor": "#a0a0a0",
                "noteTextColor": "#b0b0b0",
                "labelBackgroundColor": "#c0c0c0",
                "edgeLabelBackground": "#d0d0d0",
                "transitionLabelColor": "#e0e0e0",
                "specialStateColor": "#f0f0f0",
                "innerEndBackground": "#010101",
                "compositeBackground": "#020202",
                "stateBkg": "#030303",
                "stateBorder": "#040404",
                "compositeTitleBackground": "#050505"
            }
        });

        let model = StateSvgModel::default();
        let plan = style_plan(&model, &cfg);
        let css = rendered_state_css("st", &plan, &cfg);

        assert!(css.contains(r#"#st{font-family:Inter,Arial;font-size:16px;fill:#101010;}"#));
        assert!(css.contains(
            r#"#st .error-icon{fill:#111111;}#st .error-text{fill:#121212;stroke:#121212;}"#
        ));
        assert!(css.contains(
            r#"#st .marker{fill:#303030;stroke:#303030;}#st .marker.cross{stroke:#303030;}"#
        ));
        assert!(css.contains(r#"#st defs [id$="-barbEnd"]{fill:#202020;stroke:#202020;}"#));
        assert!(css.contains(r#"#st g.stateGroup rect{fill:#606060;stroke:#404040;}"#));
        assert!(css.contains(r#"#st .transition{stroke:#202020;stroke-width:4;fill:none;}"#));
        assert!(css.contains(r#"#st .edge-thickness-normal{stroke-width:4px;}"#));
        assert!(css.contains(r#"#st .state-note{stroke:#909090;fill:#a0a0a0;}"#));
        assert!(css.contains(r#"#st .edgeLabel .label rect{fill:#c0c0c0;opacity:0.5;}"#));
        assert!(css.contains(r#"#st .edgeLabel{background-color:#d0d0d0;text-align:center;}"#));
        assert!(css.contains(r#"#st .edgeLabel .label text{fill:#e0e0e0;}"#));
        assert!(
            css.contains(r#"#st .stateLabel text{fill:#505050;font-size:10px;font-weight:bold;}"#)
        );
        assert!(css.contains(r#"#st .node circle.state-start{fill:#f0f0f0;stroke:#f0f0f0;}"#));
        assert!(css.contains(
            r#"#st .node circle.state-end{fill:#010101;stroke:#707070;stroke-width:1.5;}"#
        ));
        assert!(css.contains(r#"#st .node rect{fill:#030303;stroke:#040404;stroke-width:4px;}"#));
        assert!(css.contains(
            r#"#st .statediagram-cluster rect{fill:#050505;stroke:#040404;stroke-width:4px;}"#
        ));
        assert!(css.contains(r#"#st .statediagram-note text{fill:#b0b0b0;}"#));
        assert!(css.contains(
            r#"#st .statediagramTitleText{text-anchor:middle;font-size:18px;fill:#101010;}"#
        ));
        assert!(css.contains(
            r##"#st [id$="-dependencyStart"],#st [id$="-dependencyEnd"]{fill:#303030;stroke:#303030;stroke-width:4;}"##
        ));
    }

    #[test]
    fn state_css_preserves_nullish_and_truthy_stroke_width_fallbacks() {
        for (stroke_width, edge_width, dependency_width) in [
            (json!(0), "0px", "1"),
            (json!("0"), "0px", "0"),
            (json!(null), "1px", "1"),
        ] {
            let config = json!({"themeVariables": {"strokeWidth": stroke_width}});
            let plan = style_plan(&StateSvgModel::default(), &config);
            let css = rendered_state_css("st", &plan, &config);
            assert!(css.contains(&format!(
                "#st .edge-thickness-normal{{stroke-width:{edge_width};}}"
            )));
            assert!(css.contains(&format!(
                r##"#st [id$="-dependencyStart"],#st [id$="-dependencyEnd"]{{fill:#333333;stroke:#333333;stroke-width:{dependency_width};}}"##
            )));
        }
    }

    #[test]
    fn state_css_emits_neo_cluster_theme_rules() {
        let cfg = json!({
            "look": "neo",
            "themeVariables": {
                "mainBkg": "#606060",
                "nodeBorder": "#030303",
                "stateBorder": "#040404",
                "strokeWidth": 4,
                "useGradient": true,
                "gradientStart": "#112233",
                "gradientStop": "#445566",
                "dropShadow": "url(#drop-shadow)",
                "radius": 3
            }
        });

        let model = StateSvgModel::default();
        let plan = style_plan(&model, &cfg);
        let css = rendered_state_css("st", &plan, &cfg);

        assert!(css.contains(
            r##"#st [data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:url(#st-gradient);stroke-width:4;}"##
        ));
        assert!(css.contains(
            r##"#st [data-look="neo"].statediagram-cluster rect.outer{rx:3px;ry:3px;filter:url(#st-drop-shadow);}"##
        ));
        assert!(css.contains(
            r##"#st [data-look="neo"].node path{stroke:url(#st-gradient);stroke-width:4px;}"##
        ));
        assert!(css.contains(
            r##"#st [data-look="neo"].node circle{stroke:url(#st-gradient);filter:url(#st-drop-shadow);}"##
        ));
    }

    #[test]
    fn state_css_keeps_neo_node_and_cluster_strokes_separate_without_gradient() {
        let cfg = json!({
            "look": "neo",
            "themeVariables": {
                "mainBkg": "#606060",
                "nodeBorder": "#112233",
                "stateBorder": "#445566",
                "strokeWidth": 0,
                "useGradient": false
            }
        });

        let model = StateSvgModel::default();
        let plan = style_plan(&model, &cfg);
        let css = rendered_state_css("st", &plan, &cfg);

        assert!(css.contains(r#"#st .transition{stroke:#333333;stroke-width:1;fill:none;}"#));
        assert!(css.contains(r#"#st .node rect{fill:#606060;stroke:#445566;stroke-width:1px;}"#));
        assert!(css.contains(
            r##"#st [data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:#445566;stroke-width:0;}"##
        ));
        assert!(
            css.contains(r##"#st [data-look="neo"].node path{stroke:#112233;stroke-width:0px;}"##)
        );
        assert!(
            css.contains(r##"#st [data-look="neo"].node circle{stroke:#112233;filter:none;}"##)
        );
    }

    #[test]
    fn state_css_emits_hand_drawn_rules_only_for_hand_drawn_look() {
        let model = StateSvgModel::default();
        let classic_css = rendered_state_css("st", &style_plan(&model, &json!({})), &json!({}));
        assert!(!classic_css.contains(r#"[data-look="handDrawn"].node rect"#));
        assert!(!classic_css.contains(r#"[data-look="handDrawn"].node line.divider"#));

        let hand_drawn_css = rendered_state_css(
            "st",
            &style_plan(&model, &json!({ "look": "handDrawn" })),
            &json!({ "look": "handDrawn" }),
        );
        assert!(hand_drawn_css.contains(r#"[data-look="handDrawn"].node rect"#));
        assert!(hand_drawn_css.contains(r#"[data-look="handDrawn"].node line.divider"#));
    }
}
