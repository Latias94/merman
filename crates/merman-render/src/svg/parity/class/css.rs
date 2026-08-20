use super::super::*;

fn write_class_marker_css(
    out: &mut String,
    id: SvgDiagramId<'_>,
    marker_id_suffix: &str,
    marker_class: &str,
    fill: &str,
    line_color: &str,
) {
    let _ = write!(
        out,
        r#"#{} [id$="-{}"],#{} .{}{{fill:{}!important;stroke:{}!important;stroke-width:1;}}"#,
        id, marker_id_suffix, id, marker_class, fill, line_color
    );
}

fn write_class_icon_css(out: &mut String, id: SvgDiagramId<'_>) {
    let _ = write!(
        out,
        r#"#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}"#,
        id, id
    );
}

pub(super) fn class_css(
    diagram_id: SvgDiagramId<'_>,
    effective_config: &serde_json::Value,
    render_font_family: &str,
    _render_font_size_css: &str,
) -> String {
    // Mermaid compiles this stylesheet from resolved theme variables; render metrics have a
    // separate legacy precedence and must not replace the CSS font-size spelling.
    let parts =
        super::super::css::info_css_parts_with_raw_theme_font_size(diagram_id, effective_config);
    let theme = PresentationTheme::new(effective_config).class_diagram();
    let mut out = parts.css_prefix;
    let fallback_font_family = normalize_css_font_family(render_font_family);
    let font_family = if parts.font_family.is_empty() {
        fallback_font_family.as_str()
    } else {
        parts.font_family.as_str()
    };
    let class_text = theme.class_text.as_str();
    let note_text = theme.note_text.as_str();
    let line_color = theme.common.line_color.as_str();
    let main_bkg = theme.main_bkg.as_str();
    let node_border = theme.node_border.as_str();
    let class_group_text = theme.class_group_text.as_str();
    let cluster_bkg = theme.cluster_bkg.as_str();
    let cluster_border = theme.cluster_border.as_str();
    let title_color = theme.title_color.as_str();
    let text_color = theme.text_color.as_str();
    let stroke_width = theme.stroke_width.as_str();
    let edge_label_background = theme_token(
        effective_config,
        "edgeLabelBackground",
        "rgba(232,232,232, 0.8)",
    );

    let _ = write!(
        &mut out,
        r#"#{} g.classGroup text{{fill:{};stroke:none;font-family:{};font-size:10px;}}#{} g.classGroup text .title{{font-weight:bolder;}}#{} .cluster-label text{{fill:{};}}#{} .cluster-label span{{color:{};}}#{} .cluster-label span p{{background-color:transparent;}}#{} .cluster rect{{fill:{};stroke:{};stroke-width:1px;}}#{} .cluster text{{fill:{};}}#{} .cluster span{{color:{};}}#{} .nodeLabel,#{} .edgeLabel{{color:{};}}#{} .noteLabel .nodeLabel,#{} .noteLabel .edgeLabel{{color:{};}}#{} .edgeLabel .label rect{{fill:{};}}#{} .label text{{fill:{};}}#{} .labelBkg{{background:{};}}#{} .edgeLabel .label span{{background:{};}}#{} .classTitle{{font-weight:bolder;}}"#,
        diagram_id,
        class_group_text,
        font_family,
        diagram_id,
        diagram_id,
        title_color,
        diagram_id,
        title_color,
        diagram_id,
        diagram_id,
        cluster_bkg,
        cluster_border,
        diagram_id,
        title_color,
        diagram_id,
        title_color,
        diagram_id,
        diagram_id,
        class_text,
        diagram_id,
        diagram_id,
        note_text,
        diagram_id,
        main_bkg,
        diagram_id,
        class_text,
        diagram_id,
        main_bkg,
        diagram_id,
        main_bkg,
        diagram_id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:{};}}#{} .divider{{stroke:{};stroke-width:1;}}#{} g.clickable{{cursor:pointer;}}#{} g.classGroup rect{{fill:{};stroke:{};}}#{} g.classGroup line{{stroke:{};stroke-width:1;}}#{} .classLabel .box{{stroke:none;stroke-width:0;fill:{};opacity:0.5;}}#{} .classLabel .label{{fill:{};font-size:10px;}}#{} .relation{{stroke:{};stroke-width:{};fill:none;}}#{} .dashed-line{{stroke-dasharray:3;}}#{} .dotted-line{{stroke-dasharray:1 2;}}"#,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        main_bkg,
        node_border,
        stroke_width,
        diagram_id,
        node_border,
        diagram_id,
        diagram_id,
        main_bkg,
        node_border,
        diagram_id,
        node_border,
        diagram_id,
        main_bkg,
        diagram_id,
        node_border,
        diagram_id,
        line_color,
        stroke_width,
        diagram_id,
        diagram_id
    );

    for (marker_id_suffix, marker_class, fill) in [
        ("compositionStart", "composition", line_color),
        ("compositionEnd", "composition", line_color),
        ("dependencyStart", "dependency", line_color),
        ("dependencyEnd", "dependency", line_color),
        ("extensionStart", "extension", "transparent"),
        ("extensionEnd", "extension", "transparent"),
        ("aggregationStart", "aggregation", "transparent"),
        ("aggregationEnd", "aggregation", "transparent"),
        ("lollipopStart", "lollipop", main_bkg),
        ("lollipopEnd", "lollipop", main_bkg),
    ] {
        write_class_marker_css(
            &mut out,
            diagram_id,
            marker_id_suffix,
            marker_class,
            fill,
            line_color,
        );
    }

    let _ = write!(
        &mut out,
        r#"#{} .edgeTerminals{{font-size:11px;line-height:initial;}}#{} .classTitleText{{text-anchor:middle;font-size:18px;fill:{};}}#{} .edgeLabel[data-look="neo"]{{background-color:{};text-align:center;}}#{} .edgeLabel[data-look="neo"] p{{background-color:{};}}#{} .edgeLabel[data-look="neo"] rect{{opacity:0.5;background-color:{};fill:{};}}"#,
        diagram_id,
        diagram_id,
        text_color,
        diagram_id,
        edge_label_background,
        diagram_id,
        edge_label_background,
        diagram_id,
        edge_label_background,
        edge_label_background
    );

    write_class_icon_css(&mut out, diagram_id);
    out.push_str(&parts.root_rule);
    out
}
