//! Usecase role colors and opt-in participant palettes from Mermaid 12 styles.ts.

use super::*;
use serde_json::Value;

pub(super) fn font_style(config: &Value) -> TextStyle {
    TextStyle {
        font_family: config_string(config, &["themeVariables", "fontFamily"])
            .or_else(|| config_string(config, &["fontFamily"])),
        font_size: config
            .pointer("/themeVariables/fontSize")
            .and_then(|value| {
                value
                    .as_f64()
                    .or_else(|| value.as_str()?.trim_end_matches("px").parse().ok())
            })
            .unwrap_or(16.0),
        ..Default::default()
    }
}

fn palette(config: &Value) -> &[Value] {
    if !matches!(
        config.get("theme").and_then(Value::as_str),
        Some("redux-color" | "redux-dark-color")
    ) {
        return &[];
    }
    config
        .pointer("/themeVariables/borderColorArray")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn look(config: &Value) -> String {
    let value = match config.get("look") {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        _ => String::new(),
    };
    if !value.is_empty()
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'-')
    {
        value
    } else {
        "classic".into()
    }
}

pub(super) fn appearance_attributes(config: &Value, color_index: Option<usize>) -> String {
    let mut attributes = format!(r#" data-look="{}""#, look(config));
    if let Some(index) = color_index.filter(|_| !palette(config).is_empty()) {
        let _ = write!(
            attributes,
            r#" data-color-id="color-{}""#,
            index % palette(config).len()
        );
    }
    attributes
}

pub(super) fn write_css(out: &mut String, id: SvgDiagramId<'_>, cfg: &Value) {
    let token = |keys: &[&str], default: &str| {
        keys.iter()
            .find_map(|key| config_string(cfg, &["themeVariables", key]))
            .unwrap_or_else(|| default.into())
    };
    let main = token(&["mainBkg", "primaryColor"], "#ECECFF");
    let node_border = token(&["nodeBorder", "primaryColor"], "#9370DB");
    let body = token(&["usecaseBkg", "mainBkg"], &main);
    let border = token(
        &["usecaseBorder", "nodeBorder", "primaryColor"],
        &node_border,
    );
    let actor = token(&["usecaseActorBkg", "actorBkg", "mainBkg"], &main);
    let actor_border = token(
        &["usecaseActorBorder", "actorBorder", "primaryColor"],
        &node_border,
    );
    let boundary = token(&["usecaseBoundaryBkg", "clusterBkg"], "#ffffde");
    let boundary_border = token(&["usecaseBoundaryBorder", "clusterBorder"], "#aaaa33");
    let line = token(&["lineColor"], "#333");
    let include = token(&["usecaseIncludeLine", "lineColor"], &line);
    let extend = token(&["usecaseExtendLine", "lineColor"], &line);
    let text = token(&["primaryTextColor"], "#333");
    let actor_text = token(&["actorTextColor", "primaryTextColor"], &text);
    let title = token(&["titleColor", "primaryTextColor"], &text);
    let note = token(&["noteBkgColor"], "#fff5ad");
    let note_border = token(&["noteBorderColor"], "#aaaa33");
    let note_text = token(&["noteTextColor"], &text);
    let edge_background = token(&["edgeLabelBackground"], &main);
    let mut css = String::new();
    let font = font_style(cfg);
    let _ = write!(
        css,
        "#{id}{{font-family:{};font-size:{}px;fill:{text}}}",
        font.font_family.as_deref().unwrap_or("sans-serif"),
        fmt(font.font_size)
    );
    let _ = write!(
        css,
        r#"
#{id} text,#{id} .nodeLabel{{fill:{text};color:{text}}}
#{id} .usecase-element ellipse,#{id} .usecase-element rect{{fill:{body};stroke:{border};stroke-width:2px}}
#{id} .usecase-actor-glyph{{fill:{actor};stroke:{actor_border};stroke-width:2px}}
#{id} .usecase-actor-stick{{fill:none}}
#{id} .usecase-actor .nodeLabel,#{id} .actor-label{{color:{actor_text};fill:{actor_text}}}
#{id} .system-boundary rect{{fill:{boundary};stroke:{boundary_border};stroke-width:1px}}
#{id} .system-boundary-title text,#{id} .system-boundary-title span{{fill:{title};color:{title}}}
#{id} .relationship{{fill:none;stroke:{line};stroke-width:1px}}
#{id} .relationship-include{{stroke:{include};stroke-dasharray:3}}
#{id} .relationship-extend{{stroke:{extend};stroke-dasharray:3}}
#{id} .relationship-note{{stroke-dasharray:3}}
#{id} .marker{{fill:{line};stroke:{line}}}#{id} .marker.extension{{fill:{main};stroke:{line}}}
#{id} .usecase-note .label-container{{fill:{note};stroke:{note_border}}}
#{id} .usecase-note .nodeLabel{{color:{note_text};fill:{note_text}}}
#{id} .usecase-json-table rect,#{id} .usecase-json-cell{{fill:{main};stroke:{node_border}}}
#{id} .usecase-business-marker{{stroke:{border};fill:none}}
#{id} .edgeLabel,#{id} .edgeLabel p{{background-color:{edge_background}}}
#{id} .edgeLabel .label rect{{fill:{edge_background}}}
@keyframes dash{{to{{stroke-dashoffset:0}}}}
#{id} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round}}
#{id} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round}}
"#
    );
    let look = look(cfg);
    let rotate = cfg.pointer("/usecase/colorScheme").and_then(Value::as_str) == Some("rotate");
    let backgrounds = cfg
        .pointer("/themeVariables/bkgColorArray")
        .and_then(Value::as_array);
    for (index, color) in palette(cfg).iter().enumerate() {
        let color = color
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| color.to_string());
        let fill = backgrounds
            .filter(|values| !values.is_empty())
            .map(|values| {
                let value = &values[index % values.len()];
                format!(
                    "fill:{};",
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                )
            })
            .unwrap_or_default();
        let slot = format!(r##"#{id} [data-look="{look}"][data-color-id="color-{index}"]"##);
        let _ = write!(
            css,
            "{slot}.system-boundary rect.boundary-body,{slot}.system-boundary rect.boundary-tab,{slot}.system-boundary .boundary-body path,{slot}.system-boundary .boundary-tab path{{stroke:{color};{fill}}}"
        );
        if rotate {
            let _ = write!(
                css,
                "{slot}.usecase-element ellipse,{slot}.usecase-element rect{{stroke:{color};{fill}}}{slot}.usecase-element .usecase-business-marker{{stroke:{color}}}{slot}.usecase-actor .usecase-actor-glyph{{stroke:{color};{fill}}}"
            );
            if look != "handDrawn" {
                let _ = write!(
                    css,
                    "{slot}.usecase-actor .usecase-actor-glyph path,{slot}.usecase-actor .usecase-actor-glyph circle{{stroke:{color}}}"
                );
            }
        }
    }
    // CSS is XML character data. Preserve declarations while preventing a token from closing style.
    out.push_str("<style>");
    util::escape_xml_raw_into(out, &css);
    out.push_str("</style>");
}
