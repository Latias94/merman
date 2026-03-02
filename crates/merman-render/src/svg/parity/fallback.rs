#![forbid(unsafe_code)]

// NOTE: This module intentionally keeps parsing "cheap" and non-validating.
// It is a best-effort readability fallback for SVG consumers that do not fully
// support HTML inside `<foreignObject>` (e.g. many rasterizers).

#[derive(Clone, Copy, Debug, Default)]
struct Translate {
    x: f64,
    y: f64,
}

#[derive(Clone, Debug, Default)]
struct GFrame {
    translate: Translate,
    class_tokens: Vec<String>,
}

fn parse_attr_str<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!(r#"{key}=""#);
    let i = tag.find(&needle)?;
    let rest = &tag[i + needle.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].trim())
}

fn parse_attr_f64(tag: &str, key: &str) -> Option<f64> {
    parse_attr_str(tag, key)?.parse::<f64>().ok()
}

fn is_self_closing(tag: &str) -> bool {
    tag.trim_end().ends_with("/>")
}

fn parse_translate(transform: &str) -> Translate {
    let lower = transform.to_ascii_lowercase();
    let Some(i) = lower.find("translate(") else {
        return Translate::default();
    };
    let after = &transform[i + "translate(".len()..];
    let Some(end) = after.find(')') else {
        return Translate::default();
    };
    let args = &after[..end];

    let mut nums = Vec::<f64>::with_capacity(2);
    let mut cur = String::new();
    for ch in args.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'e' || ch == 'E' {
            cur.push(ch);
        } else if !cur.is_empty() {
            if let Ok(v) = cur.parse::<f64>() {
                nums.push(v);
            }
            cur.clear();
        }
    }
    if !cur.is_empty() {
        if let Ok(v) = cur.parse::<f64>() {
            nums.push(v);
        }
    }

    Translate {
        x: *nums.first().unwrap_or(&0.0),
        y: *nums.get(1).unwrap_or(&0.0),
    }
}

fn sum_translate(stack: &[GFrame]) -> Translate {
    let mut acc = Translate::default();
    for t in stack {
        acc.x += t.translate.x;
        acc.y += t.translate.y;
    }
    acc
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn strip_html_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn htmlish_to_text_lines(html: &str) -> Vec<String> {
    // Mermaid foreignObject labels often look like:
    //   <div class="label">Line 1<br/>Line 2</div>
    // We treat `<br>` as line breaks and strip remaining tags.
    let mut normalized = html.replace("<br/>", "\n");
    normalized = normalized.replace("<br />", "\n");
    normalized = normalized.replace("<br>", "\n");
    normalized = normalized.replace("</br>", "\n");
    let text = strip_html_tags(&normalized);

    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

fn extract_css_background_color_for_class(svg: &str, class_name: &str) -> Option<String> {
    // Mermaid parity SVGs inline styles in a `<style>` element and typically emit rules like:
    //   #<id> .labelBkg{background-color:rgba(...);}
    // This is a cheap non-validating parser that looks for `.className{...}` and then extracts the
    // first `background-color:` declaration within that block.
    let needle = format!(".{class_name}{{");
    let mut search = 0usize;
    while let Some(rel) = svg[search..].find(&needle) {
        let i = search + rel + needle.len();
        let Some(end_rel) = svg[i..].find('}') else {
            return None;
        };
        let block = &svg[i..i + end_rel];
        if let Some(k) = block.find("background-color:") {
            let after = &block[k + "background-color:".len()..];
            let end = after.find(';').unwrap_or(after.len());
            let value = after[..end].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        search = i + end_rel + 1;
    }
    None
}

fn extract_css_text_fill_for_class(svg: &str, class_name: &str) -> Option<String> {
    // Mermaid parity SVGs inline styles in a `<style>` element and typically emit rules like:
    //   #<id> .section-root text{fill:#ffffff;}
    // This is a cheap non-validating parser that looks for the pattern and extracts the value.
    let needle = format!(".{class_name} text{{fill:");
    let mut search = 0usize;
    while let Some(rel) = svg[search..].find(&needle) {
        let i = search + rel + needle.len();
        let after = &svg[i..];
        let end = after
            .find(';')
            .or_else(|| after.find('}'))
            .unwrap_or(after.len());
        let value = after[..end].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
        search = i + end;
    }
    None
}

fn extract_inline_html_color(html: &str) -> Option<String> {
    // Look for `style="...color: <value> ..."` inside the foreignObject HTML fragment.
    let lower = html.to_ascii_lowercase();
    let Some(style_i) = lower.find("style=\"") else {
        return None;
    };
    let after = &html[style_i + "style=\"".len()..];
    let Some(end_quote) = after.find('"') else {
        return None;
    };
    let style = &after[..end_quote];
    let lower_style = style.to_ascii_lowercase();
    let Some(color_i) = lower_style.find("color:") else {
        return None;
    };
    let after_color = &style[color_i + "color:".len()..];
    let end = after_color.find(';').unwrap_or(after_color.len());
    let mut value = after_color[..end].trim().to_string();
    if let Some(v) = value.strip_suffix("!important") {
        value = v.trim().to_string();
    }
    if value.is_empty() { None } else { Some(value) }
}

fn parse_class_tokens(tag: &str) -> Vec<String> {
    let Some(s) = parse_attr_str(tag, "class") else {
        return Vec::new();
    };
    s.split_whitespace().map(|t| t.to_string()).collect()
}

fn extract_svg_text_fill_from_ancestors(svg: &str, g_stack: &[GFrame]) -> Option<String> {
    // Prefer the closest ancestor's classes (more specific) by scanning frames from inner -> outer.
    for frame in g_stack.iter().rev() {
        for token in frame.class_tokens.iter().rev() {
            if let Some(fill) = extract_css_text_fill_for_class(svg, token) {
                return Some(fill);
            }
        }
    }
    None
}

/// Adds a best-effort `<text>/<tspan>` overlay extracted from Mermaid label `<foreignObject>`
/// content.
///
/// Many headless SVG renderers and rasterizers do not fully support HTML inside `<foreignObject>`.
/// The returned SVG aims to be *more readable* for raster outputs and UI previews.
///
/// Important:
/// - This does not aim for Mermaid DOM parity.
/// - For parity-focused SVG output, keep the original SVG unchanged.
pub fn foreign_object_label_fallback_svg_text(svg: &str) -> String {
    if !svg.contains("<foreignObject") {
        return svg.to_string();
    }

    let close_tag = "</foreignObject>";
    let mut out = String::with_capacity(svg.len() + 2048);
    let mut overlays = String::new();
    let mut g_stack: Vec<GFrame> = Vec::new();
    let label_bkg_default = "rgba(232, 232, 232, 0.5)".to_string();
    let label_bkg = extract_css_background_color_for_class(svg, "labelBkg")
        .unwrap_or_else(|| label_bkg_default);

    let mut i = 0usize;
    while let Some(lt_rel) = svg[i..].find('<') {
        let lt = i + lt_rel;
        out.push_str(&svg[i..lt]);

        let Some(gt_rel) = svg[lt..].find('>') else {
            out.push_str(&svg[lt..]);
            i = svg.len();
            break;
        };
        let gt = lt + gt_rel + 1;
        let tag = &svg[lt..gt];

        // Comments / declarations: passthrough.
        if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("<?") {
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("</g") {
            let _ = g_stack.pop();
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("<g") {
            let t = parse_attr_str(tag, "transform")
                .map(parse_translate)
                .unwrap_or_default();
            let class_tokens = parse_class_tokens(tag);
            if !is_self_closing(tag) {
                g_stack.push(GFrame {
                    translate: t,
                    class_tokens,
                });
            }
            out.push_str(tag);
            i = gt;
            continue;
        }

        if tag.starts_with("<foreignObject") {
            let start_end = gt;
            let Some(close_rel) = svg[start_end..].find(close_tag) else {
                out.push_str(&svg[lt..]);
                i = svg.len();
                break;
            };
            let inner_start = start_end;
            let inner_end = inner_start + close_rel;
            let inner = &svg[inner_start..inner_end];
            let i_next = inner_end + close_tag.len();

            out.push_str(&svg[lt..i_next]);

            let width = parse_attr_f64(tag, "width").unwrap_or(0.0);
            let height = parse_attr_f64(tag, "height").unwrap_or(0.0);
            if width > 0.0 && height > 0.0 {
                let x = parse_attr_f64(tag, "x").unwrap_or(0.0);
                let y = parse_attr_f64(tag, "y").unwrap_or(0.0);
                let base = sum_translate(&g_stack);

                let abs_x = base.x + x;
                let abs_y = base.y + y;
                let (anchor, text_x) = match parse_attr_str(tag, "text-anchor") {
                    Some("start") => ("start", abs_x),
                    Some("end") => ("end", abs_x + width),
                    _ => ("middle", abs_x + width / 2.0),
                };
                let text_y = abs_y + height / 2.0;

                let lines = htmlish_to_text_lines(inner);
                if !lines.is_empty() {
                    overlays.push_str(r#"<g data-merman-foreignobject="fallback">"#);

                    let wants_label_bkg = inner.contains("labelBkg");
                    if wants_label_bkg {
                        overlays.push_str(&format!(
                            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#,
                            abs_x, abs_y, width, height, label_bkg
                        ));
                    }

                    let font_size = 16.0_f64;
                    let line_height = font_size * 1.5;
                    let n = lines.len() as f64;
                    let y0 = text_y - (line_height * (n - 1.0)) / 2.0;
                    let fill = extract_inline_html_color(inner)
                        .or_else(|| extract_svg_text_fill_from_ancestors(svg, &g_stack))
                        .unwrap_or_else(|| "#333".to_string());
                    // Avoid quoting font families inside an XML attribute that is already quoted.
                    // Rasterizers can be strict about unescaped quotes in `style="..."`.
                    let font_family = "trebuchet ms,verdana,arial,sans-serif";

                    for (idx, line) in lines.iter().enumerate() {
                        let y_line = y0 + (idx as f64) * line_height;
                        let text = escape_xml_text(line);
                        overlays.push_str(&format!(
                            r##"<text x="{}" y="{}" dominant-baseline="central" alignment-baseline="central" fill="{}" style="text-anchor: {}; font-size: {}px; font-family: {};">{}</text>"##,
                            text_x, y_line, fill, anchor, font_size, font_family, text
                        ));
                    }

                    overlays.push_str("</g>");
                }
            }

            i = i_next;
            continue;
        }

        out.push_str(tag);
        i = gt;
    }

    if i < svg.len() {
        out.push_str(&svg[i..]);
    }

    if overlays.is_empty() {
        return out;
    }

    if let Some(idx) = out.rfind("</svg>") {
        let mut with_overlays = String::with_capacity(out.len() + overlays.len() + 64);
        with_overlays.push_str(&out[..idx]);
        with_overlays.push_str(&overlays);
        with_overlays.push_str(&out[idx..]);
        with_overlays
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::foreign_object_label_fallback_svg_text;

    #[test]
    fn foreign_object_overlay_accounts_for_parent_translate() {
        let svg = r#"<svg viewBox="90 -310 425 99" xmlns="http://www.w3.org/2000/svg"><g transform="translate(183.3046875, -300)"><foreignObject width="33.390625" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Todo</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            out.contains(r#"x="200""#),
            "expected x=200 center placement"
        );
        assert!(
            out.contains(r#"y="-288""#),
            "expected y=-288 center placement"
        );
        assert!(
            out.contains(">Todo<"),
            "expected text content to be present"
        );
    }

    #[test]
    fn foreign_object_overlay_renders_label_bkg_rect_when_present() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>#d .labelBkg{background-color:rgba(232,232,232,0.5);}</style><g id="d"><foreignObject x="10" y="20" width="30" height="24"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg"><p>Hello</p></div></foreignObject></g></svg>"#;
        let out = foreign_object_label_fallback_svg_text(svg);
        assert!(
            out.contains(r#"fill="rgba(232,232,232,0.5)""#),
            "expected labelBkg fill"
        );
        assert!(
            out.contains(r#"<rect x="10" y="20" width="30" height="24""#),
            "expected rect with foreignObject bounds"
        );
    }
}
