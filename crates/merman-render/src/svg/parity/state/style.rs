use super::*;

pub(super) fn state_markers(out: &mut String, diagram_id: &str) {
    let diagram_id = escape_xml(diagram_id);
    let _ = write!(
        out,
        r#"<defs><marker id="{diagram_id}_stateDiagram-barbEnd" refX="19" refY="7" markerWidth="20" markerHeight="14" markerUnits="userSpaceOnUse" orient="auto"><path d="M 19,7 L9,13 L14,7 L9,1 Z"/></marker></defs>"#
    );
}

pub(super) fn state_css(
    diagram_id: &str,
    model: &StateSvgModel,
    effective_config: &serde_json::Value,
) -> String {
    fn font_family_css(effective_config: &serde_json::Value) -> String {
        let mut ff = config_string(effective_config, &["fontFamily"])
            .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
            .unwrap_or_else(|| "\"trebuchet ms\",verdana,arial,sans-serif".to_string());
        ff = ff.replace(", ", ",").replace(",\t", ",");
        // Mermaid's default config value sometimes includes a trailing `;` in `fontFamily`
        // (e.g. `"trebuchet ms", verdana, arial, sans-serif;`). Mermaid's emitted CSS does not.
        ff.trim().trim_end_matches(';').to_string()
    }

    fn normalize_decl(s: &str) -> Option<(String, String)> {
        let s = s.trim().trim_end_matches(';').trim();
        if s.is_empty() {
            return None;
        }
        let (k, v) = s.split_once(':')?;
        let key = k.trim().to_string();
        let mut val = v.trim().to_string();
        // Mermaid emits class styles with `!important` (no spaces).
        if !val.to_lowercase().contains("!important") {
            val.push_str("!important");
        } else {
            val = val.replace(" !important", "!important");
        }
        Some((key, val))
    }

    fn class_decl_block(styles: &[String], text_styles: &[String]) -> String {
        let mut out = String::new();
        for raw in styles.iter().chain(text_styles.iter()) {
            let Some((k, v)) = normalize_decl(raw) else {
                continue;
            };
            // Mermaid tightens `prop: value` -> `prop:value`.
            let _ = write!(&mut out, "{}:{};", k, v);
        }
        out
    }

    fn should_duplicate_class_rules(styles: &[String], text_styles: &[String]) -> bool {
        let has_fontish = |s: &str| {
            let s = s.trim_start().to_lowercase();
            s.starts_with("font-") || s.starts_with("text-")
        };
        styles.iter().any(|s| has_fontish(s)) || text_styles.iter().any(|s| has_fontish(s))
    }

    let ff = font_family_css(effective_config);
    let font_size = config_f64(effective_config, &["fontSize"])
        .unwrap_or(16.0)
        .max(1.0);
    let id = escape_xml(diagram_id);

    // Keep the base stylesheet byte-for-byte compatible with Mermaid@11.12.2.
    let mut css = String::new();
    let font_size_s = fmt(font_size);
    let _ = write!(
        &mut css,
        r#"#{}{{font-family:{};font-size:{}px;fill:#333;}}"#,
        id, ff, font_size_s
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
    let _ = write!(&mut css, r#"#{} .error-icon{{fill:#552222;}}"#, id);
    let _ = write!(
        &mut css,
        r#"#{} .error-text{{fill:#552222;stroke:#552222;}}"#,
        id
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
        r#"#{} .marker{{fill:#333333;stroke:#333333;}}"#,
        id
    );
    let _ = write!(&mut css, r#"#{} .marker.cross{{stroke:#333333;}}"#, id);
    let _ = write!(
        &mut css,
        r#"#{} svg{{font-family:{};font-size:{}px;}}"#,
        id, ff, font_size_s
    );
    let _ = write!(&mut css, r#"#{} p{{margin:0;}}"#, id);
    let _ = write!(
        &mut css,
        r#"#{} defs #statediagram-barbEnd{{fill:#333333;stroke:#333333;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup text{{fill:#9370DB;stroke:none;font-size:10px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup text{{fill:#333;stroke:none;font-size:10px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup .state-title{{font-weight:bolder;fill:#131300;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup rect{{fill:#ECECFF;stroke:#9370DB;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} g.stateGroup line{{stroke:#333333;stroke-width:1;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .transition{{stroke:#333333;stroke-width:1;fill:none;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateGroup .composit{{fill:white;border-bottom:1px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateGroup .alt-composit{{fill:#e0e0e0;border-bottom:1px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .state-note{{stroke:#aaaa33;fill:#fff5ad;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .state-note text{{fill:black;stroke:none;font-size:10px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .stateLabel .box{{stroke:none;stroke-width:0;fill:#ECECFF;opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edgeLabel .label rect{{fill:#ECECFF;opacity:0.5;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edgeLabel{{background-color:rgba(232,232,232, 0.8);text-align:center;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edgeLabel p{{background-color:rgba(232,232,232, 0.8);}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .edgeLabel rect{{opacity:0.5;background-color:rgba(232,232,232, 0.8);fill:rgba(232,232,232, 0.8);}}"#,
        id
    );
    let _ = write!(&mut css, r#"#{} .edgeLabel .label text{{fill:#333;}}"#, id);
    let _ = write!(&mut css, r#"#{} .label div .edgeLabel{{color:#333;}}"#, id);
    let _ = write!(
        &mut css,
        r#"#{} .stateLabel text{{fill:#131300;font-size:10px;font-weight:bold;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .node circle.state-start{{fill:#333333;stroke:#333333;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .node .fork-join{{fill:#333333;stroke:#333333;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .node circle.state-end{{fill:#9370DB;stroke:white;stroke-width:1.5;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .end-state-inner{{fill:white;stroke-width:1.5;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .node rect{{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .node polygon{{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} #statediagram-barbEnd{{fill:#333333;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster rect{{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .cluster-label,#{} .nodeLabel{{color:#131300;}}"#,
        id, id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster rect.outer{{rx:5px;ry:5px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-state .divider{{stroke:#9370DB;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-state .title-state{{rx:5px;ry:5px;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster.statediagram-cluster .inner{{fill:white;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-cluster.statediagram-cluster-alt .inner{{fill:#f0f0f0;}}"#,
        id
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
        r#"#{} .statediagram-state rect.divider{{stroke-dasharray:10,10;fill:#f0f0f0;}}"#,
        id
    );
    let _ = write!(&mut css, r#"#{} .note-edge{{stroke-dasharray:5;}}"#, id);
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note rect{{fill:#fff5ad;stroke:#aaaa33;stroke-width:1px;rx:0;ry:0;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note rect{{fill:#fff5ad;stroke:#aaaa33;stroke-width:1px;rx:0;ry:0;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note text{{fill:black;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram-note .nodeLabel{{color:black;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagram .edgeLabel{{color:red;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} #dependencyStart,#{} #dependencyEnd{{fill:#333333;stroke:#333333;stroke-width:1;}}"#,
        id, id
    );
    let _ = write!(
        &mut css,
        r#"#{} .statediagramTitleText{{text-anchor:middle;font-size:18px;fill:#333;}}"#,
        id
    );
    let _ = write!(
        &mut css,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, ff
    );

    if !model.style_classes.is_empty() {
        // Mermaid keeps classDef ordering stable and appends each class as:
        //   `#id .class&gt;*{...}#id .class span{...}`
        for sc in model.style_classes.values() {
            let decls = class_decl_block(&sc.styles, &sc.text_styles);
            if decls.is_empty() {
                continue;
            }
            let repeats = if should_duplicate_class_rules(&sc.styles, &sc.text_styles) {
                2
            } else {
                1
            };
            for _ in 0..repeats {
                let _ = write!(
                    &mut css,
                    r#"#{} .{}&gt;*{{{}}}#{} .{} span{{{}}}"#,
                    id, sc.id, decls, id, sc.id, decls
                );
            }
        }
    }

    css
}

pub(super) fn state_value_to_label_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(a) => {
            let mut parts: Vec<&str> = Vec::new();
            for item in a {
                if let Some(s) = item.as_str() {
                    parts.push(s);
                }
            }
            if parts.is_empty() {
                return "".to_string();
            }
            parts.join("\n")
        }
        _ => "".to_string(),
    }
}

pub(super) fn state_node_label_text(n: &StateSvgNode) -> String {
    n.label
        .as_ref()
        .map(state_value_to_label_text)
        .unwrap_or_else(|| n.id.clone())
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StateInlineDecl<'a> {
    pub(super) key: &'a str,
    pub(super) val: &'a str,
}

pub(super) fn state_parse_inline_decl(raw: &str) -> Option<StateInlineDecl<'_>> {
    let raw = raw.trim().trim_end_matches(';').trim();
    if raw.is_empty() {
        return None;
    }
    let (k, v) = raw.split_once(':')?;
    let key = k.trim();
    let val = v.trim();
    if key.is_empty() || val.is_empty() {
        return None;
    }
    Some(StateInlineDecl { key, val })
}

pub(super) fn state_is_text_style_key(key: &str) -> bool {
    let k = key.trim().to_ascii_lowercase();
    k == "color" || k.starts_with("font-") || k.starts_with("text-")
}

pub(super) fn state_compact_style_attr(decls: &[StateInlineDecl<'_>]) -> String {
    let mut out = String::new();
    for (idx, d) in decls.iter().enumerate() {
        if idx > 0 {
            out.push(';');
        }
        out.push_str(d.key.trim());
        out.push(':');
        out.push_str(d.val.trim());
        if !d.val.to_ascii_lowercase().contains("!important") {
            out.push_str(" !important");
        }
    }
    out
}

pub(super) fn state_div_style_prefix(decls: &[StateInlineDecl<'_>]) -> String {
    let mut out = String::new();
    for d in decls {
        out.push_str(d.key.trim());
        out.push_str(": ");
        out.push_str(d.val.trim());
        if !d.val.to_ascii_lowercase().contains("!important") {
            out.push_str(" !important");
        }
        out.push_str("; ");
    }
    out
}

pub(super) fn state_node_label_html_with_style(raw: &str, span_style: Option<&str>) -> String {
    let style_attr = span_style
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#" style="{}""#, escape_xml_display(s)))
        .unwrap_or_default();
    format!(
        r#"<span{} class="nodeLabel">{}</span>"#,
        style_attr,
        html_paragraph_with_br(raw)
    )
}

#[allow(dead_code)]
pub(super) fn state_node_label_inline_html_with_style(
    raw: &str,
    span_style: Option<&str>,
) -> String {
    let style_attr = span_style
        .filter(|s| !s.is_empty())
        .map(|s| format!(r#" style="{}""#, escape_xml_display(s)))
        .unwrap_or_default();
    format!(
        r#"<span{} class="nodeLabel">{}</span>"#,
        style_attr,
        html_inline_with_br(raw)
    )
}

pub(super) fn html_paragraph_with_br(raw: &str) -> String {
    fn escape_amp_preserving_entities(raw: &str) -> String {
        fn is_valid_entity(entity: &str) -> bool {
            if entity.is_empty() {
                return false;
            }
            if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
            }
            if let Some(dec) = entity.strip_prefix('#') {
                return !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit());
            }
            let mut it = entity.chars();
            let Some(first) = it.next() else {
                return false;
            };
            if !first.is_ascii_alphabetic() {
                return false;
            }
            it.all(|c| c.is_ascii_alphanumeric())
        }

        let mut out = String::with_capacity(raw.len());
        let mut i = 0usize;
        while let Some(rel) = raw[i..].find('&') {
            let amp = i + rel;
            out.push_str(&raw[i..amp]);
            let tail = &raw[amp + 1..];
            if let Some(semi_rel) = tail.find(';') {
                let semi = amp + 1 + semi_rel;
                let entity = &raw[amp + 1..semi];
                if is_valid_entity(entity) {
                    out.push_str(&raw[amp..=semi]);
                    i = semi + 1;
                    continue;
                }
            }
            out.push_str("&amp;");
            i = amp + 1;
        }
        out.push_str(&raw[i..]);
        out
    }

    fn normalize_br_tags(raw: &str) -> String {
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
            if let Some(n) = next {
                if !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n') {
                    i += 1;
                    continue;
                }
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

    let normalized = normalize_br_tags(raw);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out = String::new();
    out.push_str("<p>");
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        // State diagram labels are sanitized upstream (entities + limited tags). Preserve entities
        // like `&lt;` without double-escaping, while still making stray `&` XML-safe.
        out.push_str(&escape_amp_preserving_entities(line));
    }
    out.push_str("</p>");
    out
}

pub(super) fn html_inline_with_br(raw: &str) -> String {
    fn escape_amp_preserving_entities(raw: &str) -> String {
        fn is_valid_entity(entity: &str) -> bool {
            if entity.is_empty() {
                return false;
            }
            if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
            }
            if let Some(dec) = entity.strip_prefix('#') {
                return !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit());
            }
            let mut it = entity.chars();
            let Some(first) = it.next() else {
                return false;
            };
            if !first.is_ascii_alphabetic() {
                return false;
            }
            it.all(|c| c.is_ascii_alphanumeric())
        }

        let mut out = String::with_capacity(raw.len());
        let mut i = 0usize;
        while let Some(rel) = raw[i..].find('&') {
            let amp = i + rel;
            out.push_str(&raw[i..amp]);
            let tail = &raw[amp + 1..];
            if let Some(semi_rel) = tail.find(';') {
                let semi = amp + 1 + semi_rel;
                let entity = &raw[amp + 1..semi];
                if is_valid_entity(entity) {
                    out.push_str(&raw[amp..=semi]);
                    i = semi + 1;
                    continue;
                }
            }
            out.push_str("&amp;");
            i = amp + 1;
        }
        out.push_str(&raw[i..]);
        out
    }

    fn normalize_br_tags(raw: &str) -> String {
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
            if let Some(n) = next {
                if !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n') {
                    i += 1;
                    continue;
                }
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

    let normalized = normalize_br_tags(raw);
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push_str("<br />");
        }
        out.push_str(&escape_amp_preserving_entities(line));
    }
    out
}

pub(super) fn state_node_label_html(raw: &str) -> String {
    format!(
        r#"<span class="nodeLabel">{}</span>"#,
        html_paragraph_with_br(raw)
    )
}

pub(super) fn state_node_label_inline_html(raw: &str) -> String {
    format!(
        r#"<span class="nodeLabel">{}</span>"#,
        html_inline_with_br(raw)
    )
}

pub(super) fn state_edge_label_html(raw: &str) -> String {
    // Mermaid runs edge labels through its `markdownToHTML()` pipeline when `htmlLabels=true`.
    // Even for "plain" labels this mostly behaves like a paragraph wrapper, but it also
    // recognizes emphasis/strong spans such as `_and_` -> `<em>and</em>`.
    //
    // We don't embed Mermaid's `marked` lexer in Rust. Instead we:
    // - use the same "paragraph vs raw" heuristic as the rest of the renderer
    // - parse the small subset of inline formatting that upstream actually emits (`<em>/<strong>`)
    //
    // Note: upstream sanitizes state labels; we intentionally keep entity-like sequences
    // (e.g. `&lt;`) without double-escaping.

    fn escape_amp_preserving_entities(raw: &str) -> String {
        fn is_valid_entity(entity: &str) -> bool {
            if entity.is_empty() {
                return false;
            }
            if let Some(hex) = entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
            {
                return !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
            }
            if let Some(dec) = entity.strip_prefix('#') {
                return !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit());
            }
            let mut it = entity.chars();
            let Some(first) = it.next() else {
                return false;
            };
            if !first.is_ascii_alphabetic() {
                return false;
            }
            it.all(|c| c.is_ascii_alphanumeric())
        }

        let mut out = String::with_capacity(raw.len());
        let mut i = 0usize;
        while let Some(rel) = raw[i..].find('&') {
            let amp = i + rel;
            out.push_str(&raw[i..amp]);
            let tail = &raw[amp + 1..];
            if let Some(semi_rel) = tail.find(';') {
                let semi = amp + 1 + semi_rel;
                let entity = &raw[amp + 1..semi];
                if is_valid_entity(entity) {
                    out.push_str(&raw[amp..=semi]);
                    i = semi + 1;
                    continue;
                }
            }
            out.push_str("&amp;");
            i = amp + 1;
        }
        out.push_str(&raw[i..]);
        out
    }

    fn normalize_br_tags(raw: &str) -> String {
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
            if let Some(n) = next {
                if !matches!(n, b'>' | b'/' | b' ' | b'\t' | b'\r' | b'\n') {
                    i += 1;
                    continue;
                }
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

    let normalized = normalize_br_tags(raw);

    if !crate::text::mermaid_markdown_wants_paragraph_wrap(&normalized) {
        // Upstream falls back to raw Markdown for unsupported block constructs without wrapping.
        return html_inline_with_br(&normalized);
    }

    let escape_xhtml_text = |raw: &str| -> String {
        // `pulldown-cmark` decodes entities like `&lt;` into `<` in `Event::Text`.
        // Convert those back into XML-safe text while preserving any valid entities that remain.
        let s = escape_amp_preserving_entities(raw);
        let mut out = String::with_capacity(s.len());
        for ch in s.chars() {
            match ch {
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                _ => out.push(ch),
            }
        }
        out
    };

    let parser = pulldown_cmark::Parser::new_ext(
        &normalized,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    )
    .map(|ev| match ev {
        pulldown_cmark::Event::SoftBreak => pulldown_cmark::Event::HardBreak,
        other => other,
    });

    let mut out = String::new();
    let mut saw_paragraph = false;
    for ev in parser {
        match ev {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Paragraph) => {
                saw_paragraph = true;
                out.push_str("<p>");
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Paragraph) => {
                out.push_str("</p>");
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Emphasis) => {
                out.push_str("<em>");
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Emphasis) => {
                out.push_str("</em>");
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Strong) => {
                out.push_str("<strong>");
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Strong) => {
                out.push_str("</strong>");
            }
            pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                out.push_str(&escape_xhtml_text(&t));
            }
            pulldown_cmark::Event::HardBreak | pulldown_cmark::Event::SoftBreak => {
                out.push_str("<br />");
            }
            pulldown_cmark::Event::Html(t) => {
                // Preserve safety and XML well-formedness: treat raw HTML as literal text.
                out.push_str(&escape_xhtml_text(&t));
            }
            _ => {}
        }
    }

    if !saw_paragraph {
        out.push_str("<p></p>");
    }
    out
}
