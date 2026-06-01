use super::super::*;

pub(super) fn class_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
    font_family: &str,
    font_size_css: &str,
) -> String {
    let id = escape_xml(diagram_id);
    let theme = SvgTheme::new(effective_config);
    let font_family = normalize_css_font_family(font_family);
    let font_family = if font_family.is_empty() {
        "\"trebuchet ms\",verdana,arial,sans-serif"
    } else {
        font_family.as_str()
    };
    let class_text = theme.color(
        "classText",
        &theme.color("primaryTextColor", &theme.color("textColor", "#333")),
    );
    let note_text = theme.color("noteTextColor", "#333");
    let line_color = theme.color("lineColor", "#333333");
    let main_bkg = theme.color("mainBkg", "#ECECFF");
    let text_color = theme.color("textColor", class_text.as_str());
    let stroke_width = theme.color("strokeWidth", "1");

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{};fill:{};}}"#,
        id.as_str(),
        font_family,
        font_size_css,
        class_text
    );
    let _ = write!(
        &mut out,
        r#"#{} p{{margin:0;}}#{} .nodeLabel,#{} .edgeLabel{{color:{};}}#{} .noteLabel .nodeLabel,#{} .noteLabel .edgeLabel{{color:{};}}#{} .label text{{fill:{};}}#{} .label span{{fill:{};color:{};}}"#,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        class_text,
        id.as_str(),
        id.as_str(),
        note_text,
        id.as_str(),
        class_text,
        id.as_str(),
        class_text,
        class_text
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel .label rect{{fill:{};}}#{} .labelBkg{{background:{}}}#{} .edgeLabel .label span{{background:{}}}#{} .relation{{stroke:{};stroke-width:{};fill:none;}}#{} .dashed-line{{stroke-dasharray:3;}}#{} .dotted-line{{stroke-dasharray:1 2;}}"#,
        id.as_str(),
        main_bkg,
        id.as_str(),
        main_bkg,
        id.as_str(),
        main_bkg,
        id.as_str(),
        line_color,
        stroke_width,
        id.as_str(),
        id.as_str()
    );
    let _ = write!(
        &mut out,
        r#"#{} [id$="-compositionStart"],#{} .composition,#{} [id$="-compositionEnd"]{{fill:{}!important;stroke:{}!important;stroke-width:1;}}#{} [id$="-dependencyStart"],#{} .dependency,#{} [id$="-dependencyEnd"]{{fill:{}!important;stroke:{}!important;stroke-width:1;}}"#,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        line_color,
        line_color,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        line_color,
        line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} [id$="-extensionStart"],#{} .extension,#{} [id$="-extensionEnd"],#{} [id$="-aggregationStart"],#{} .aggregation,#{} [id$="-aggregationEnd"]{{fill:transparent!important;stroke:{}!important;stroke-width:1;}}#{} [id$="-lollipopStart"],#{} .lollipop,#{} [id$="-lollipopEnd"]{{fill:{}!important;stroke:{}!important;stroke-width:1;}}"#,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        line_color,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        main_bkg,
        line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} g.clickable{{cursor:pointer;}}#{} .classTitle,#{} .classTitleText,#{} .classDiagramTitleText{{font-weight:bolder;text-anchor:middle;font-size:18px;fill:{};}}"#,
        id.as_str(),
        id.as_str(),
        id.as_str(),
        id.as_str(),
        text_color
    );
    out
}
