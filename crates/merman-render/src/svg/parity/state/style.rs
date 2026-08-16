use super::*;

fn state_shadow_defs(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    compatibility: &crate::state::StateCompatibilityStyle,
) {
    let flood_color = if compatibility.dark_mode {
        "#FFFFFF"
    } else {
        "#000000"
    };
    let diagram_id = escape_xml(diagram_id);
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id.as_str(),
        flood_color,
        diagram_id.as_str(),
        flood_color
    );
}

fn state_gradient_defs(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config: &serde_json::Value,
) {
    if !config_bool(effective_config, &["themeVariables", "useGradient"]).unwrap_or(false) {
        return;
    }

    let gradient_start = config_string(effective_config, &["themeVariables", "gradientStart"])
        .or_else(|| config_string(effective_config, &["themeVariables", "primaryBorderColor"]))
        .unwrap_or_else(|| "#9370DB".to_string());
    let gradient_stop = config_string(effective_config, &["themeVariables", "gradientStop"])
        .or_else(|| {
            config_string(
                effective_config,
                &["themeVariables", "secondaryBorderColor"],
            )
        })
        .unwrap_or_else(|| gradient_start.clone());

    let diagram_id = escape_xml(diagram_id);
    let gradient_start = escape_xml(&gradient_start);
    let gradient_stop = escape_xml(&gradient_stop);
    let _ = write!(
        out,
        r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient></defs>"#,
        diagram_id.as_str(),
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
}

pub(super) fn write_state_theme_effect_application(
    out: &mut impl SvgOutput,
    scoped_filter_id: &str,
    effect: &crate::state::StateSvgEffect,
    region: crate::state::StateSvgFilterRegion,
) -> String {
    let [x, y, width, height] = region.as_array();
    let id = escape_attr(scoped_filter_id);
    let color = escape_attr(&effect.color().as_css());
    // Keep the SVG filter default explicit. `sRGB` causes a large dark-color gamma shift in the
    // native resvg path, while `linearRGB` preserves the authored color within 8-bit quantization.
    let _ = write!(
        out,
        r#"<defs><filter id="{}" filterUnits="objectBoundingBox" x="{}" y="{}" width="{}" height="{}" color-interpolation-filters="linearRGB"><feDropShadow in="SourceGraphic" dx="{}" dy="{}" stdDeviation="{}" flood-color="{}"/></filter></defs>"#,
        id,
        x,
        y,
        width,
        height,
        effect.offset_x(),
        effect.offset_y(),
        effect.std_deviation(),
        color,
    );
    format!("url(#{scoped_filter_id})")
}

pub(super) fn state_markers(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    style_plan: &crate::state::StateStylePlan,
) {
    let compatibility = style_plan.compatibility();
    let diagram_id = escape_xml(diagram_id);
    let transition_color = compatibility.transition_color.as_str();
    let marker_style_attr = if style_plan.transition_marker_style_attr().is_empty() {
        String::new()
    } else {
        format!(
            r#" style="{}""#,
            escape_xml(style_plan.transition_marker_style_attr())
        )
    };

    if compatibility.neo {
        let _ = write!(
            out,
            r#"<defs><marker id="{diagram_id}_stateDiagram-barbEnd" refX="19" refY="7" markerWidth="20" markerHeight="14" markerUnits="strokeWidth" orient="auto"><path d="M 19,7 L11,14 L13,7 L11,0 Z"{marker_style_attr}/></marker></defs><defs><marker id="{diagram_id}_stateDiagram-barbEnd-margin" refX="17" refY="7" markerWidth="20" markerHeight="14" markerUnits="userSpaceOnUse" orient="auto"><path d="M 19,7 L11,14 L13,7 L11,0 Z" fill="{}"{marker_style_attr}/></marker></defs>"#,
            escape_xml(transition_color)
        );
    } else {
        let _ = write!(
            out,
            r#"<defs><marker id="{diagram_id}_stateDiagram-barbEnd" refX="19" refY="7" markerWidth="20" markerHeight="14" markerUnits="userSpaceOnUse" orient="auto"><path d="M 19,7 L9,13 L14,7 L9,1 Z"{marker_style_attr}/></marker></defs>"#
        );
    }

    for edge in style_plan.edges() {
        let (Some(ordinal), style) = (edge.marker_ordinal(), edge.marker_style_attr()) else {
            continue;
        };
        if style.is_empty() {
            continue;
        }
        let marker_id = escape_attr(&state_transition_marker_id(diagram_id.as_str(), ordinal));
        let style = escape_attr(style);
        let transition_color = escape_attr(transition_color);
        if compatibility.neo {
            let _ = write!(
                out,
                r#"<defs><marker id="{}" refX="19" refY="7" markerWidth="20" markerHeight="14" markerUnits="strokeWidth" orient="auto"><path d="M 19,7 L11,14 L13,7 L11,0 Z" fill="{}" stroke="{}" style="{}"/></marker></defs>"#,
                marker_id, transition_color, transition_color, style
            );
        } else {
            let _ = write!(
                out,
                r#"<defs><marker id="{}" refX="19" refY="7" markerWidth="20" markerHeight="14" markerUnits="userSpaceOnUse" orient="auto"><path d="M 19,7 L9,13 L14,7 L9,1 Z" fill="{}" stroke="{}" style="{}"/></marker></defs>"#,
                marker_id, transition_color, transition_color, style
            );
        }
    }
}

pub(super) fn state_root_defs(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config: &serde_json::Value,
    style_plan: &crate::state::StateStylePlan,
) {
    state_shadow_defs(out, diagram_id, style_plan.compatibility());
    state_gradient_defs(out, diagram_id, effective_config);
}

fn write_state_class_declarations(
    out: &mut impl SvgOutput,
    styles: &[(
        usize,
        std::sync::Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
    )],
) -> crate::Result<()> {
    for (_, declaration) in styles {
        let semantic_key = declaration.property();
        let key = declaration.property_css();
        let value = declaration.value().trim();
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
            std::borrow::Cow::Owned(super::super::util::cssom_color_value(value))
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
    id: &str,
    class_id: &str,
    selector: std::fmt::Arguments<'_>,
    styles: &[(
        usize,
        std::sync::Arc<crate::diagram_theme::PreparedSourceStyleDeclaration>,
    )],
) -> crate::Result<()> {
    if write!(out, "#{id} .{class_id}{selector}{{").is_err() {
        return out.checkpoint();
    }
    write_state_class_declarations(out, styles)?;
    out.push('}');
    out.checkpoint()
}

pub(super) fn write_state_css(
    out: &mut impl SvgOutput,
    diagram_id: &str,
    effective_config: &serde_json::Value,
    style_plan: &crate::state::StateStylePlan,
) -> crate::Result<()> {
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
    let ff = crate::config::normalize_css_font_family(ff);
    let font_size = if uses_structured_typography {
        base_text_style.font_size
    } else {
        crate::config::config_theme_or_root_font_size_px(effective_config, 16.0)
    };
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
    let id = crate::svg::escape_css_identifier(diagram_id);
    let fragment_id = escape_xml(diagram_id);
    let text_color = theme.text_color.as_str();
    let title_color = theme.title_color.as_str();
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
    let use_gradient =
        config_bool(effective_config, &["themeVariables", "useGradient"]).unwrap_or(false);
    let neo_cluster_stroke = if use_gradient {
        format!("url(#{fragment_id}-gradient)")
    } else {
        state_border.clone()
    };
    let neo_radius =
        crate::config::config_f64_css_px(effective_config, &["themeVariables", "radius"])
            .unwrap_or(5.0)
            .max(0.0);
    let neo_drop_shadow = theme.drop_shadow.replace(
        "url(#drop-shadow)",
        &format!("url(#{fragment_id}-drop-shadow)"),
    );

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
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}"#,
        id
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
        r#"#{} .statediagram-cluster rect.outer{{rx:5px;ry:5px;}}"#,
        id
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
    let _ = write!(
        &mut css,
        r#"#{} .statediagram .edgeLabel{{color:red;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} [id$="-dependencyStart"],#{} [id$="-dependencyEnd"]{{fill:{};stroke:{};stroke-width:1;}}"#,
        id, id, line_color, line_color
    );
    let title_text_style = style_plan.title_text_style();
    if uses_structured_typography {
        let title_ff = crate::config::normalize_css_font_family(
            title_text_style.font_family.as_deref().unwrap_or(&ff),
        );
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
            title_color
        );
    } else {
        let _ = write!(
            &mut css,
            r#"#{} .statediagramTitleText{{text-anchor:middle;font-size:18px;fill:{};}}"#,
            id, title_color
        );
    }
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].statediagram-cluster rect{{fill:{};stroke:{};stroke-width:{};}}"#,
        id, main_bkg, neo_cluster_stroke, stroke_width
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
        id, state_border
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node rect,#{} [data-look="neo"].cluster rect,#{} [data-look="neo"].node polygon{{stroke:{};filter:{};}}"#,
        id, id, id, state_border, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].swimlane.cluster rect{{filter:none;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node path{{stroke:{};stroke-width:{};}}"#,
        id, state_border, stroke_width_px
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node .outer-path{{filter:{};}}"#,
        id, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node .neo-line path{{stroke:{};filter:none;}}"#,
        id, state_border
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node circle{{stroke:{};filter:{};}}"#,
        id, state_border, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].node circle .state-start{{fill:#000000;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].icon-shape .icon{{fill:{};filter:{};}}"#,
        id, state_border, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} [data-look="neo"].icon-shape .icon-neo path{{stroke:{};filter:{};}}"#,
        id, state_border, neo_drop_shadow
    );
    let _ = write!(
        &mut css,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, ff
    );

    css.checkpoint()?;

    if style_plan.classes().next().is_some() {
        let html_labels = crate::state::StateConfigView::new(effective_config)
            .render_settings()
            .html_labels;
        for sc in style_plan.classes() {
            if !sc.styles().is_empty() {
                if html_labels {
                    write_state_class_rule(
                        &mut *css,
                        id.as_str(),
                        sc.id(),
                        format_args!("&gt;*"),
                        sc.styles(),
                    )?;
                    write_state_class_rule(
                        &mut *css,
                        id.as_str(),
                        sc.id(),
                        format_args!(" span"),
                        sc.styles(),
                    )?;
                } else {
                    for element in ["rect", "polygon", "ellipse", "circle", "path"] {
                        write_state_class_rule(
                            &mut *css,
                            id.as_str(),
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
                    id.as_str(),
                    sc.id(),
                    format_args!(" tspan"),
                    sc.text_styles(),
                )?;
            }
        }
    }

    css.checkpoint()
}

pub(super) fn state_value_to_label_text(v: &serde_json::Value) -> String {
    crate::state::state_value_to_label_text(v)
}

pub(super) fn state_node_label_text(n: &StateSvgNode) -> String {
    n.label
        .as_ref()
        .map(state_value_to_label_text)
        .unwrap_or_else(|| n.id.clone())
}

pub(super) fn state_node_label_html_with_style(
    raw: &str,
    span_style: Option<&str>,
    font_size: f64,
) -> String {
    let style_attr = span_style
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#" style="{}""#, escape_xml_display(s)))
        .unwrap_or_default();
    format!(
        r#"<span{} class="nodeLabel markdown-node-label">{}</span>"#,
        style_attr,
        crate::state::state_node_label_xhtml(raw, font_size)
    )
}

pub(super) fn state_prepared_node_label_html_with_style(
    prepared: &crate::state::PreparedStateLabel,
    span_style: Option<&str>,
) -> String {
    let style = prepared.merge_emission_font_style(span_style);
    let style_attr = format!(r#" style="{}""#, escape_xml_display(&style));
    format!(
        r#"<span{} class="nodeLabel markdown-node-label"><p>{}</p></span>"#,
        style_attr,
        state_prepared_html_lines(prepared),
    )
}

fn state_escape_amp_preserving_entities(raw: &str) -> String {
    crate::xml::normalize_html_entities_for_xml(raw).into_owned()
}

fn state_normalize_br_tags(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut cur = 0usize;
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let b1 = bytes[i + 1];
        let b2 = bytes[i + 2];
        if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
            i += 1;
            continue;
        }
        let next = bytes.get(i + 3).copied();
        if let Some(n) = next
            && !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n')
        {
            i += 1;
            continue;
        }
        if i > cur {
            out.push_str(&raw[cur..i]);
        }
        let Some(end_rel) = bytes[i..].iter().position(|&c| c == b'>') else {
            cur = i;
            break;
        };
        out.push('\n');
        i = i + end_rel + 1;
        cur = i;
    }
    if cur < raw.len() {
        out.push_str(&raw[cur..]);
    }
    out
}

fn write_state_html_lines_with_br(out: &mut String, normalized: &str) {
    for (idx, line) in normalized.split('\n').enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        // State diagram labels are sanitized upstream (entities + limited tags). Preserve entities
        // like `&lt;` without double-escaping, while still making stray `&` XML-safe.
        out.push_str(&state_escape_amp_preserving_entities(line));
    }
}

fn state_html_with_br(raw: &str, wrap_paragraph: bool) -> String {
    let decoded = crate::svg::parity::util::decode_mermaid_entities_for_render_text(raw);
    let normalized = state_normalize_br_tags(decoded.as_ref());
    let mut out = String::new();
    if wrap_paragraph {
        out.push_str("<p>");
    }
    write_state_html_lines_with_br(&mut out, &normalized);
    if wrap_paragraph {
        out.push_str("</p>");
    }
    out
}

fn html_paragraph_with_br(raw: &str) -> String {
    state_html_with_br(raw, true)
}

pub(super) fn state_node_label_plain_html(raw: &str) -> String {
    format!(
        r#"<span class="nodeLabel">{}</span>"#,
        html_paragraph_with_br(raw)
    )
}

pub(super) fn state_prepared_node_label_plain_html(
    prepared: &crate::state::PreparedStateLabel,
) -> String {
    let style = prepared.merge_emission_font_style(None);
    format!(
        r#"<span class="nodeLabel" style="{}"><p>{}</p></span>"#,
        escape_xml_display(&style),
        state_prepared_html_lines(prepared),
    )
}

pub(super) fn state_edge_label_html(raw: &str) -> String {
    crate::state::state_edge_label_xhtml(raw)
}

pub(super) fn state_prepared_edge_label_html(
    prepared: &crate::state::PreparedStateLabel,
) -> String {
    let style = prepared.merge_emission_font_style(None);
    format!(
        r#"<p style="{}">{}</p>"#,
        escape_xml_display(&style),
        state_prepared_html_lines(prepared)
    )
}

pub(super) fn state_prepared_html_label_token_attr(
    prepared: Option<&crate::state::PreparedStateLabel>,
) -> String {
    prepared
        .and_then(crate::state::PreparedStateLabel::label_id_for_emission)
        .map(|id| {
            format!(
                r#" {}="{}""#,
                crate::svg::fallback::PREPARED_TEXT_LABEL_DATA_ATTR,
                escape_attr(&id.as_svg_id())
            )
        })
        .unwrap_or_default()
}

pub(super) fn state_prepared_html_lines(prepared: &crate::state::PreparedStateLabel) -> String {
    let mut out = String::new();
    for (index, line) in prepared.wrapped_lines().enumerate() {
        if index > 0 {
            out.push_str("<br />");
        }
        let _ = write!(&mut out, "{}", escape_xml_display(line));
    }
    out
}

fn state_svg_text_style_attr(style_attr: &str) -> String {
    let mut out = String::with_capacity(style_attr.len());
    for declaration in style_attr.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim();
        let value = value.trim();
        if property.is_empty() || value.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(';');
        }
        let property = if property.eq_ignore_ascii_case("color") {
            "fill"
        } else {
            property
        };
        let _ = write!(&mut out, "{property}:{value}");
    }
    out
}

pub(super) fn state_svg_text_label(
    raw: &str,
    center_text: bool,
    style_attr: Option<&str>,
) -> String {
    let decoded = crate::svg::parity::util::decode_mermaid_entities_for_render_text(raw);
    let normalized = state_normalize_br_tags(decoded.as_ref());
    state_svg_text_label_lines(normalized.split('\n'), center_text, style_attr, None)
}

pub(crate) fn state_prepared_svg_text_label(
    prepared: &crate::state::PreparedStateLabel,
    center_text: bool,
    style_attr: Option<&str>,
) -> String {
    let style_attr = prepared.merge_emission_font_style(style_attr);
    state_svg_text_label_lines(
        prepared.wrapped_lines(),
        center_text,
        Some(&style_attr),
        prepared.label_id_for_emission(),
    )
}

fn state_svg_text_label_lines<'a>(
    lines: impl IntoIterator<Item = &'a str>,
    center_text: bool,
    style_attr: Option<&str>,
    label_id: Option<crate::text::PreparedTextLabelId>,
) -> String {
    let text_anchor = if center_text {
        r#" text-anchor="middle""#
    } else {
        ""
    };
    let svg_style_attr = style_attr
        .map(state_svg_text_style_attr)
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#" style="{}""#, escape_attr(&s)))
        .unwrap_or_default();
    let label_id_attr = label_id
        .map(|id| format!(r#" id="{id}""#))
        .unwrap_or_default();

    let mut out = format!(r#"<text y="-10.1"{text_anchor}{svg_style_attr}{label_id_attr}>"#);
    for (idx, line) in lines.into_iter().enumerate() {
        let y = idx as f64 * 1.1 - 0.1;
        let _ = write!(
            &mut out,
            r#"<tspan class="text-outer-tspan row" x="0" y="{}em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal"{}>{}</tspan></tspan>"#,
            fmt_display(y),
            svg_style_attr,
            escape_xml_display(line)
        );
    }
    out.push_str("</text>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue};
    use merman_core::diagrams::state::StateDiagramRenderStyleClass;
    use serde_json::json;

    fn state_theme_effect(std_deviation: f32) -> crate::state::StateSvgEffect {
        crate::state::StateSvgEffect::from_graph_for_test(
            &EffectGraph::new(
                "soft-shadow",
                [EffectPrimitive::DropShadow {
                    input: EffectInput::SourceGraphic,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur_radius: std_deviation,
                    spread: 0.0,
                    color: ThemeColorValue::parse("rgba(0, 242, 255, 0.5)").unwrap(),
                }],
            )
            .unwrap(),
        )
        .expect("supported State drop shadow")
    }

    fn model_with_hot_class() -> StateSvgModel {
        let mut model = StateSvgModel::default();
        model.style_classes.insert(
            "hot".to_string(),
            StateDiagramRenderStyleClass {
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
        crate::state::StateStylePlan::resolve_unthemed(model, config)
    }

    fn rendered_state_css(
        diagram_id: &str,
        config: &serde_json::Value,
        plan: &crate::state::StateStylePlan,
    ) -> String {
        let mut css = String::new();
        write_state_css(&mut css, diagram_id, config, plan).expect("write State CSS");
        css
    }

    #[test]
    fn state_theme_effect_emits_the_authored_std_deviation() {
        let effect = state_theme_effect(8.0);
        let region = crate::state::StateSvgFilterRegion::try_bounded(-0.66, -1.65, 2.32, 4.3)
            .expect("bounded soft-shadow region");
        let mut svg = String::new();

        let reference = write_state_theme_effect_application(
            &mut svg,
            "diagram-state-theme-effect-soft-shadow",
            &effect,
            region,
        );

        assert_eq!(reference, "url(#diagram-state-theme-effect-soft-shadow)");
        assert!(svg.contains(r#"stdDeviation="8""#));
        assert!(svg.contains(r#"color-interpolation-filters="linearRGB""#));
    }

    #[test]
    fn state_class_defs_keep_shape_and_text_styles_separate() {
        let model = model_with_hot_class();
        let config = json!({});
        let plan = style_plan(&model, &config);
        let css = rendered_state_css("st", &config, &plan);

        let declarations = "fill:rgb(255, 221, 221)!important;stroke:rgb(221, 51, 51)!important;stroke-width:2px!important;color:rgb(34, 34, 34)!important;";
        assert!(css.ends_with(&format!(
            "#st .hot&gt;*{{{declarations}}}#st .hot span{{{declarations}}}#st .hot tspan{{fill:rgb(34, 34, 34)!important;}}"
        )));
        assert_eq!(css.matches("#st .hot&gt;*").count(), 1);
        assert!(!css.contains("color:rgb(34, 34, 34)!important;fill:rgb(34, 34, 34)!important"));
    }

    #[test]
    fn state_svg_text_projects_color_and_emphasis_to_inner_tspan() {
        let svg = state_svg_text_label(
            "Styled",
            true,
            Some(
                "color:#123456 !important;font-weight:700 !important;font-style:italic !important",
            ),
        );

        assert!(svg.contains(
            r#"<text y="-10.1" text-anchor="middle" style="fill:#123456 !important;font-weight:700 !important;font-style:italic !important">"#
        ));
        assert!(svg.contains(
            r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal" style="fill:#123456 !important;font-weight:700 !important;font-style:italic !important">Styled</tspan>"#
        ));
        assert!(!svg.contains("color:#123456"));
    }

    #[test]
    fn state_svg_text_applies_the_prepared_style_to_each_line() {
        let svg = state_svg_text_label(
            "first\nsecond",
            false,
            Some("color:#abcdef;font-weight:bold"),
        );
        assert_eq!(
            svg.matches("style=\"fill:#abcdef;font-weight:bold\"")
                .count(),
            3
        );
    }

    #[test]
    fn state_shadow_uses_explicit_typed_theme_dark_mode() {
        let model = StateSvgModel::default();
        let dark_config = json!({
            "theme": "base",
            "themeVariables": { "darkMode": true }
        });
        let dark_plan = style_plan(&model, &dark_config);
        let mut dark = String::new();
        state_shadow_defs(&mut dark, "st", dark_plan.compatibility());
        assert!(dark.contains(r##"flood-color="#FFFFFF""##));

        let light_config = json!({
            "theme": "dark",
            "darkMode": false
        });
        let light_plan = style_plan(&model, &light_config);
        let mut light = String::new();
        state_shadow_defs(&mut light, "st", light_plan.compatibility());
        assert!(light.contains(r##"flood-color="#000000""##));
    }

    #[test]
    fn state_class_defs_use_shape_selectors_without_html_labels() {
        let model = model_with_hot_class();
        let config = json!({ "htmlLabels": false });
        let plan = style_plan(&model, &config);
        let css = rendered_state_css("st", &config, &plan);

        for element in ["rect", "polygon", "ellipse", "circle", "path"] {
            assert!(css.contains(&format!("#st .hot {element}{{")));
        }
        assert!(!css.contains("#st .hot&gt;*"));
        assert!(!css.contains("#st .hot span{"));
        assert_eq!(css.matches("#st .hot tspan{").count(), 1);
    }

    #[test]
    fn state_class_defs_do_not_duplicate_font_rules() {
        let mut model = StateSvgModel::default();
        model.style_classes.insert(
            "emphasis".to_string(),
            StateDiagramRenderStyleClass {
                id: "emphasis".to_string(),
                styles: vec!["font-style:italic".to_string()],
                text_styles: vec![],
            },
        );

        let config = json!({});
        let plan = style_plan(&model, &config);
        let css = rendered_state_css("st", &config, &plan);
        assert_eq!(css.matches("#st .emphasis&gt;*").count(), 1);
        assert_eq!(css.matches("#st .emphasis span{").count(), 1);
    }

    #[test]
    fn state_css_stops_at_the_class_rule_svg_byte_boundary() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

        let config = json!({});
        let baseline_model = StateSvgModel::default();
        let baseline_plan = style_plan(&baseline_model, &config);
        let baseline_css = rendered_state_css("st", &config, &baseline_plan);

        let model = model_with_hot_class();
        let plan = style_plan(&model, &config);
        let max_svg_bytes = baseline_css.len() + 16;
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, max_svg_bytes)
            .expect("valid State CSS byte ceiling");
        let meter = OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);

        let error = write_state_css(&mut out, "st", &config, &plan)
            .expect_err("the first class rule must cross the SVG byte ceiling");

        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
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
        let css = rendered_state_css("st", &cfg, &plan);

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
            r##"#st [id$="-dependencyStart"],#st [id$="-dependencyEnd"]{fill:#303030;stroke:#303030;stroke-width:1;}"##
        ));
    }

    #[test]
    fn state_css_emits_neo_cluster_theme_rules() {
        let cfg = json!({
            "look": "neo",
            "themeVariables": {
                "mainBkg": "#606060",
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
        let css = rendered_state_css("st", &cfg, &plan);

        assert!(css.contains(
            r##"#st [data-look="neo"].statediagram-cluster rect{fill:#606060;stroke:url(#st-gradient);stroke-width:4;}"##
        ));
        assert!(css.contains(
            r##"#st [data-look="neo"].statediagram-cluster rect.outer{rx:3px;ry:3px;filter:url(#st-drop-shadow);}"##
        ));
    }
}
