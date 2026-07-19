use crate::Result;
use cssparser::{Delimiter, Parser, ParserInput, Token};
use std::borrow::Cow;

use super::css_sanitize::strip_css_deg_units;
use super::presentation_fallback::is_mermaid_missing_amount_hsl;
use super::util::{SvgTagScanner, escape_xml_attr, next_svg_quoted_attr, start_tag_name};
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, Copy, Default)]
pub struct SanitizeSvgAttributesPostprocessor;

impl SvgPostprocessor for SanitizeSvgAttributesPostprocessor {
    fn name(&self) -> &'static str {
        "sanitize-svg-attributes"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        Ok(Cow::Owned(sanitize_element_attributes(&svg)))
    }
}

pub(crate) fn sanitize_element_attributes(svg: &str) -> String {
    let mut out = String::with_capacity(svg.len());
    let mut scanner = SvgTagScanner::new(svg);
    let mut copied_until = 0;

    while let Some(tag) = scanner.next() {
        out.push_str(&svg[copied_until..tag.start()]);

        let raw_tag = tag.raw();
        if let Some(active_name) = active_svg_element_name(raw_tag) {
            copied_until = if tag.is_self_closing() {
                scanner.cursor()
            } else {
                find_close_tag_end(svg, scanner.cursor(), active_name).unwrap_or(scanner.cursor())
            };
            scanner.skip_to(copied_until);
            continue;
        }

        if is_bad_rect_tag(raw_tag) {
            copied_until = if tag.is_self_closing() {
                scanner.cursor()
            } else {
                svg[scanner.cursor()..]
                    .find("</rect>")
                    .map(|rel_close| scanner.cursor() + rel_close + "</rect>".len())
                    .unwrap_or(scanner.cursor())
            };
            scanner.skip_to(copied_until);
            continue;
        }
        out.push_str(&sanitize_tag_attributes(raw_tag));
        copied_until = scanner.cursor();
    }

    out.push_str(&svg[copied_until..]);
    out
}

fn sanitize_tag_attributes(tag: &str) -> Cow<'_, str> {
    if tag.starts_with("</")
        || tag.starts_with("<!--")
        || tag.starts_with("<!")
        || tag.starts_with("<?")
    {
        return Cow::Borrowed(tag);
    }

    let mut changed = false;
    let mut out = String::new();
    let mut copied_until = 0usize;
    let mut cursor = 0usize;

    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        let name = &tag[attr.name_start..attr.name_end];
        let value = &tag[attr.value_start..attr.value_end];

        let replacement = sanitized_attr_replacement(name, value);
        if let AttrReplacement::Unchanged = replacement {
            cursor = attr.full_end;
            continue;
        }

        if !changed {
            out = String::with_capacity(tag.len());
            changed = true;
        }
        out.push_str(&tag[copied_until..attr.full_start]);
        match replacement {
            AttrReplacement::Unchanged => {}
            AttrReplacement::Drop => {}
            AttrReplacement::Replace(replacement) => out.push_str(&replacement),
        }
        copied_until = attr.full_end;
        cursor = attr.full_end;
    }

    if changed {
        out.push_str(&tag[copied_until..]);
        Cow::Owned(out)
    } else {
        Cow::Borrowed(tag)
    }
}

enum AttrReplacement {
    Unchanged,
    Drop,
    Replace(String),
}

fn sanitized_attr_replacement(name: &str, value: &str) -> AttrReplacement {
    if should_drop_attribute(name, value) {
        return AttrReplacement::Drop;
    }

    if let Some(value) = normalize_px_attribute(name, value) {
        return AttrReplacement::Replace(format!(r#" {name}="{value}""#));
    }

    if name.eq_ignore_ascii_case("style") {
        let sanitized = sanitize_style_attribute(value);
        if sanitized.trim().is_empty() {
            return AttrReplacement::Drop;
        }
        if sanitized != value {
            return AttrReplacement::Replace(format!(r#" style="{sanitized}""#));
        }
    }

    AttrReplacement::Unchanged
}

fn should_drop_attribute(name: &str, value: &str) -> bool {
    if name.eq_ignore_ascii_case("style") {
        return false;
    }

    if is_event_handler_attribute(name) || is_unsafe_url_attribute(name, value) {
        return true;
    }

    let normalized = name.to_ascii_lowercase();
    if is_url_function_attribute(&normalized) && css_value_violates_url_safety(value) {
        return true;
    }

    if value.trim().is_empty() {
        return matches!(
            normalized.as_str(),
            "fill"
                | "stroke"
                | "width"
                | "height"
                | "x"
                | "y"
                | "x1"
                | "x2"
                | "y1"
                | "y2"
                | "r"
                | "cx"
                | "cy"
                | "rx"
                | "ry"
                | "stroke-width"
                | "transform"
                | "d"
                | "points"
        );
    }

    match normalized.as_str() {
        "fill" | "stroke" => is_mermaid_missing_amount_hsl(value),
        "width" | "height" | "x" | "y" | "x1" | "x2" | "y1" | "y2" | "r" | "cx" | "cy" | "rx"
        | "ry" | "stroke-width" => is_provably_invalid_scalar(value),
        "transform" => is_invalid_svg_transform(value),
        "d" | "points" => contains_non_finite_numeric_token(value),
        _ => false,
    }
}

fn is_event_handler_attribute(name: &str) -> bool {
    let name = local_name(name.trim());
    name.len() > 2
        && name
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
        && name.as_bytes()[2].is_ascii_alphabetic()
}

fn is_unsafe_url_attribute(name: &str, value: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    if !matches!(normalized.as_str(), "href" | "xlink:href" | "src") {
        return false;
    }

    is_unsafe_url_value(value)
}

fn is_url_function_attribute(name: &str) -> bool {
    matches!(
        name,
        "fill"
            | "stroke"
            | "filter"
            | "clip-path"
            | "mask"
            | "marker-start"
            | "marker-mid"
            | "marker-end"
    )
}

fn is_unsafe_url_value(value: &str) -> bool {
    let value = normalize_url_attr_for_scheme_check(value);
    if value.is_empty() || value.starts_with('#') {
        return false;
    }

    if value.starts_with("data:") {
        return !is_safe_data_image_url(&value);
    }

    let Some(colon) = value.find(':') else {
        return false;
    };
    let scheme = &value[..colon];
    !matches!(scheme, "http" | "https" | "mailto")
}

fn css_value_violates_url_safety(value: &str) -> bool {
    let decoded = merman_core::entities::decode_html_entities_to_unicode(value);
    let mut input = ParserInput::new(&decoded);
    let mut parser = Parser::new(&mut input);
    validate_css_urls(&mut parser).is_err()
}

fn validate_css_urls<'i, 't>(
    parser: &mut Parser<'i, 't>,
) -> std::result::Result<(), cssparser::ParseError<'i, ()>> {
    while !parser.is_exhausted() {
        let token = parser.next_including_whitespace()?.clone();
        match token {
            Token::UnquotedUrl(url) if is_unsafe_url_value(&url) => {
                return Err(parser.new_custom_error(()));
            }
            Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                parser.parse_nested_block(|nested| {
                    let url = nested.expect_string_cloned()?;
                    if is_unsafe_url_value(&url) {
                        return Err(nested.new_custom_error(()));
                    }
                    Ok(())
                })?;
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                parser.parse_nested_block(validate_css_urls)?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(parser.new_custom_error(()));
            }
            _ => {}
        }
    }
    Ok(())
}

fn normalize_url_attr_for_scheme_check(value: &str) -> String {
    let decoded = merman_core::entities::decode_html_entities_to_unicode(value);
    let mut out = String::with_capacity(decoded.len());
    for ch in decoded.trim().chars() {
        if !ch.is_whitespace() && !ch.is_control() {
            out.extend(ch.to_lowercase());
        }
    }
    out
}

fn is_safe_data_image_url(value: &str) -> bool {
    let Some(media_type) = value.strip_prefix("data:") else {
        return false;
    };
    let media_type = media_type
        .split_once(',')
        .map_or(media_type, |(head, _)| head);
    matches!(
        media_type.split(';').next().unwrap_or_default(),
        "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp"
    )
}

fn normalize_px_attribute(name: &str, value: &str) -> Option<String> {
    let normalized = name.to_ascii_lowercase();
    let guarded = matches!(
        normalized.as_str(),
        "width"
            | "height"
            | "x"
            | "y"
            | "x1"
            | "x2"
            | "y1"
            | "y2"
            | "r"
            | "cx"
            | "cy"
            | "rx"
            | "ry"
            | "stroke-width"
    );
    if !guarded {
        return None;
    }

    let trimmed = value.trim();
    let number = trimmed.strip_suffix("px")?.trim();
    if number.parse::<f64>().is_ok_and(f64::is_finite) {
        Some(number.to_string())
    } else {
        None
    }
}

fn is_start_or_empty_tag(tag: &str, expected: &str) -> bool {
    let tag = tag.trim_start();
    if !tag.starts_with('<') || tag.starts_with("</") || tag.starts_with("<!--") {
        return false;
    }

    let name = tag[1..]
        .chars()
        .take_while(|ch| !ch.is_whitespace() && *ch != '/' && *ch != '>')
        .collect::<String>();
    name.eq_ignore_ascii_case(expected)
}

fn active_svg_element_name(tag: &str) -> Option<&str> {
    let name = start_tag_name(tag)?;
    matches_active_svg_element(name).then_some(name)
}

fn matches_active_svg_element(name: &str) -> bool {
    matches!(
        local_name(name).to_ascii_lowercase().as_str(),
        "script" | "iframe" | "object" | "embed" | "foreignobject"
    )
}

fn local_name(name: &str) -> &str {
    name.rsplit_once(':')
        .map_or(name, |(_, local_name)| local_name)
}

fn find_close_tag_end(svg: &str, from: usize, name: &str) -> Option<usize> {
    let mut scanner = SvgTagScanner::new(svg);
    scanner.skip_to(from);
    while let Some(tag) = scanner.next() {
        if close_tag_matches(tag.raw(), name) {
            return Some(scanner.cursor());
        }
    }
    None
}

fn close_tag_matches(tag: &str, expected: &str) -> bool {
    let tag = tag.trim_start();
    if !tag.starts_with("</") {
        return false;
    }
    let name_start = 2;
    let name_end = name_start
        + tag[name_start..]
            .find(|ch: char| ch.is_whitespace() || ch == '>')
            .unwrap_or(tag.len() - name_start);
    if name_start >= name_end {
        return false;
    }
    let after_name = tag[name_end..].trim();
    after_name == ">" && tag[name_start..name_end].eq_ignore_ascii_case(expected)
}

fn attr_value(tag: &str, name: &str) -> Option<String> {
    let mut cursor = 0usize;
    while let Some(attr) = next_svg_quoted_attr(tag, cursor) {
        if tag[attr.name_start..attr.name_end].eq_ignore_ascii_case(name) {
            return Some(tag[attr.value_start..attr.value_end].to_string());
        }
        cursor = attr.full_end;
    }
    None
}

fn is_missing_or_invalid_rect_dimension(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim) else {
        return true;
    };
    if value.is_empty() {
        return true;
    }
    if let Ok(n) = value.parse::<f64>() {
        return !n.is_finite() || n <= 0.0;
    }
    false
}

fn is_bad_rect_tag(tag: &str) -> bool {
    if !is_start_or_empty_tag(tag, "rect") {
        return false;
    }

    let width = attr_value(tag, "width");
    let height = attr_value(tag, "height");
    is_missing_or_invalid_rect_dimension(width.as_deref())
        || is_missing_or_invalid_rect_dimension(height.as_deref())
}

fn sanitize_style_attribute(value: &str) -> String {
    let mut out = Vec::new();

    for (property, value) in parse_css_declarations(value) {
        let normalized_value = strip_css_deg_units(&value);
        if normalized_value.is_empty()
            || is_invalid_style_property_value(&property, &normalized_value)
            || css_value_violates_url_safety(&normalized_value)
        {
            continue;
        }
        if property.to_ascii_lowercase().starts_with("animation") {
            continue;
        }

        out.push(format!("{property}:{normalized_value}"));
    }

    escape_xml_attr(&out.join(";"))
}

fn parse_css_declarations(value: &str) -> Vec<(String, String)> {
    let decoded = merman_core::entities::decode_html_entities_to_unicode(value);
    let mut input = ParserInput::new(&decoded);
    let mut parser = Parser::new(&mut input);
    let mut declarations = Vec::new();

    while !parser.is_exhausted() {
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?.to_string();
            declaration.expect_colon()?;
            let value_start = declaration.position();
            declaration.expect_no_error_token()?;
            let value = declaration.slice_from(value_start).trim().to_string();
            Ok::<_, cssparser::ParseError<'_, ()>>((property, value))
        });
        if let Ok(declaration) = declaration {
            declarations.push(declaration);
        }
    }
    declarations
}

fn is_invalid_style_property_value(property: &str, value: &str) -> bool {
    match property.trim().to_ascii_lowercase().as_str() {
        "fill" | "stroke" | "color" | "flood-color" | "lighting-color" | "stop-color" => {
            is_mermaid_missing_amount_hsl(value)
        }
        "stroke-width" | "stroke-dashoffset" | "fill-opacity" | "stroke-opacity" | "opacity"
        | "x" | "y" | "width" | "height" | "rx" | "ry" => is_provably_invalid_scalar(value),
        "transform" => is_invalid_svg_transform(value),
        "d" => contains_non_finite_numeric_token(value),
        _ => false,
    }
}

fn is_provably_invalid_scalar(value: &str) -> bool {
    let value = value.trim();
    if value.eq_ignore_ascii_case("undefined") {
        return true;
    }

    let number = ["rem", "px", "%", "em", "ex", "in", "cm", "mm", "pt", "pc"]
        .into_iter()
        .find_map(|unit| value.strip_suffix(unit))
        .unwrap_or(value)
        .trim();
    number
        .parse::<f64>()
        .is_ok_and(|number| !number.is_finite())
}

fn is_invalid_svg_transform(value: &str) -> bool {
    let mut remaining = value.trim();
    if remaining.eq_ignore_ascii_case("none") {
        return false;
    }

    while !remaining.is_empty() {
        remaining = remaining.trim_start_matches(|ch: char| ch.is_whitespace() || ch == ',');
        if remaining.is_empty() {
            break;
        }
        let name_end = remaining
            .find(|ch: char| !ch.is_ascii_alphabetic())
            .unwrap_or(remaining.len());
        let name = &remaining[..name_end];
        let Some(expected) = transform_argument_counts(name) else {
            return false;
        };
        remaining = &remaining[name_end..];
        let Some(after_open) = remaining.strip_prefix('(') else {
            return true;
        };
        let Some(close) = after_open.find(')') else {
            return true;
        };
        let arguments_text = &after_open[..close];
        let arguments = arguments_text
            .split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter(|argument| !argument.is_empty())
            .collect::<Vec<_>>();
        if !expected.contains(&arguments.len())
            || arguments.iter().any(|argument| {
                argument
                    .parse::<f64>()
                    .map_or(true, |number| !number.is_finite())
            })
        {
            return true;
        }
        remaining = &after_open[close + 1..];
    }
    false
}

fn transform_argument_counts(name: &str) -> Option<&'static [usize]> {
    match name {
        "matrix" => Some(&[6]),
        "translate" | "scale" => Some(&[1, 2]),
        "rotate" => Some(&[1, 3]),
        "skewX" | "skewY" => Some(&[1]),
        _ => None,
    }
}

fn contains_non_finite_numeric_token(value: &str) -> bool {
    ["NaN", "Infinity", "-Infinity", "+Infinity"]
        .into_iter()
        .any(|token| contains_delimited_numeric_token(value, token))
}

fn contains_delimited_numeric_token(value: &str, token: &str) -> bool {
    value.match_indices(token).any(|(start, _)| {
        let before = value[..start].chars().next_back();
        let after = value[start + token.len()..].chars().next();
        is_numeric_token_boundary(before) && is_numeric_token_boundary(after)
    })
}

fn is_numeric_token_boundary(ch: Option<char>) -> bool {
    ch.is_none_or(|ch| {
        ch.is_whitespace()
            || matches!(ch, ',' | '(' | ')')
            || matches!(
                ch,
                'M' | 'm'
                    | 'Z'
                    | 'z'
                    | 'L'
                    | 'l'
                    | 'H'
                    | 'h'
                    | 'V'
                    | 'v'
                    | 'C'
                    | 'c'
                    | 'S'
                    | 's'
                    | 'Q'
                    | 'q'
                    | 'T'
                    | 't'
                    | 'A'
                    | 'a'
            )
    })
}

#[cfg(test)]
mod tests {
    use super::sanitize_element_attributes;

    #[test]
    fn sanitize_style_attribute_drops_invalid_bare_declarations() {
        let svg = r#"<svg><path style="undefined; stroke: #333; undefined"/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(!out.contains("undefined"), "got: {out}");
        assert!(out.contains(r#"style="stroke:#333""#), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_drops_rects_without_positive_dimensions() {
        let svg = r#"<svg><rect/><rect width="0" height="10"/><rect width="12" height="8"/><g><rect width="NaN" height="10"><title>bad</title></rect></g></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(!out.contains("<rect/>"), "got: {out}");
        assert!(!out.contains(r#"width="0""#), "got: {out}");
        assert!(!out.contains("NaN"), "got: {out}");
        assert!(!out.contains("<title>bad</title>"), "got: {out}");
        assert!(
            out.contains(r#"<rect width="12" height="8"/>"#),
            "got: {out}"
        );
    }

    #[test]
    fn sanitize_element_attributes_scans_double_quoted_attrs_without_regex() {
        let svg = r#"<svg><path data-keep = "ok" x = "10px" stroke="" style="transform: rotate(45deg); animation: dash 1s; stroke: #333;"/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(out.contains(r#"data-keep = "ok""#), "got: {out}");
        assert!(out.contains(r#" x="10""#), "got: {out}");
        assert!(!out.contains(r#"stroke="""#), "got: {out}");
        assert!(
            out.contains(r#"style="transform:rotate(45);stroke:#333""#),
            "got: {out}"
        );
        assert!(!out.contains("animation"), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_scans_single_quoted_attrs() {
        let svg = r#"<svg><path x = '10px' style='animation: dash 1s; stroke: #333;'/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(out.contains(r#" x="10""#), "got: {out}");
        assert!(out.contains(r#"style="stroke:#333""#), "got: {out}");
        assert!(!out.contains("animation"), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_uses_scanned_attrs_for_bad_rect_detection() {
        let svg = r#"<svg><rect WIDTH = "12" HEIGHT = "8"/><rect width = "NaN" height = "8"><title>bad</title></rect></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(
            out.contains(r#"<rect WIDTH = "12" HEIGHT = "8"/>"#),
            "got: {out}"
        );
        assert!(!out.contains("NaN"), "got: {out}");
        assert!(!out.contains("<title>bad</title>"), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_strips_active_svg_elements() {
        let svg = r#"<svg><script>alert(1)</script><svg:script>alert(2)</svg:script><SCRIPT/><iframe src="https://example.com"></iframe><object data="x"></object><rect width="12" height="8"/></svg>"#;
        let out = sanitize_element_attributes(svg);

        assert!(!out.to_ascii_lowercase().contains("<script"), "got: {out}");
        assert!(
            !out.to_ascii_lowercase().contains("<svg:script"),
            "got: {out}"
        );
        assert!(!out.to_ascii_lowercase().contains("<iframe"), "got: {out}");
        assert!(!out.to_ascii_lowercase().contains("<object"), "got: {out}");
        assert!(
            out.contains(r#"<rect width="12" height="8"/>"#),
            "got: {out}"
        );
    }

    #[test]
    fn sanitize_element_attributes_strips_event_and_unsafe_url_attrs() {
        let svg = r##"<svg><a href="#local"><use href="#shape" xlink:href="#shape"/></a><a href="javascript&colon;alert(1)" onclick="alert(1)" svg:onload="alert(1)"><text>bad</text></a><image href="data:image/png;base64,AAAA"/><image href="data:text/html;base64,AAAA"/><image href="file:///etc/passwd"/><a href="java&#x3a;script:alert(1)"/></svg>"##;
        let out = sanitize_element_attributes(svg);
        let lower = out.to_ascii_lowercase();

        assert!(out.contains(r##"href="#local""##), "got: {out}");
        assert!(out.contains(r##"href="#shape""##), "got: {out}");
        assert!(out.contains(r##"xlink:href="#shape""##), "got: {out}");
        assert!(
            out.contains(r#"href="data:image/png;base64,AAAA""#),
            "got: {out}"
        );
        assert!(!lower.contains("onclick"), "got: {out}");
        assert!(!lower.contains("svg:onload"), "got: {out}");
        assert!(!lower.contains("javascript"), "got: {out}");
        assert!(!lower.contains("data:text/html"), "got: {out}");
        assert!(!lower.contains("file:///"), "got: {out}");
    }

    #[test]
    fn sanitize_element_attributes_preserves_safe_fragment_ids_with_invalid_token_prefixes() {
        let svg = r##"<svg><defs><linearGradient id="undefined"/><linearGradient id="nan"/><linearGradient id="undefined-gradient"/><linearGradient id="nan-stroke"/></defs><circle fill="url(#undefined)" stroke="url(#nan)"/><circle fill="url(#undefined-gradient)" stroke="url(#nan-stroke)"/></svg>"##;

        let out = sanitize_element_attributes(svg);

        assert!(out.contains(r##"fill="url(#undefined)""##), "{out}");
        assert!(out.contains(r##"stroke="url(#nan)""##), "{out}");
        assert!(
            out.contains(r##"fill="url(#undefined-gradient)""##),
            "{out}"
        );
        assert!(out.contains(r##"stroke="url(#nan-stroke)""##), "{out}");
    }

    #[test]
    fn sanitize_style_preserves_custom_property_identifier_values() {
        let svg =
            r#"<svg><path style="--state: undefined; --number-kind: NaN; stroke: #333"/></svg>"#;

        let out = sanitize_element_attributes(svg);

        assert!(out.contains("--state:undefined"), "{out}");
        assert!(out.contains("--number-kind:NaN"), "{out}");
        assert!(out.contains("stroke:#333"), "{out}");
    }

    #[test]
    fn sanitize_style_keeps_xml_entities_and_nested_semicolons_inside_values() {
        let svg = r#"<svg><text style="font-family:&quot;a;b&quot;,sans-serif;--data:url(data:image/png;base64,AAAA);stroke:#333"/></svg>"#;

        let out = sanitize_element_attributes(svg);
        roxmltree::Document::parse(&out).expect("sanitized style must remain valid XML");

        assert!(
            out.contains("font-family:&quot;a;b&quot;,sans-serif"),
            "{out}"
        );
        assert!(
            out.contains("--data:url(data:image/png;base64,AAAA)"),
            "{out}"
        );
        assert!(out.contains("stroke:#333"), "{out}");
    }

    #[test]
    fn css_url_tokens_handle_quoted_parentheses_escapes_and_nested_functions() {
        let svg = r##"<svg><defs><linearGradient id="paint)close"/></defs><circle fill="url(&quot;#paint)close&quot;)" stroke="u\72l(&quot;j\61vascript:alert(1)&quot;)" filter="drop-shadow(0 0 2px u\72l(&quot;j\61vascript:alert(1)&quot;))"/></svg>"##;

        let out = sanitize_element_attributes(svg);

        assert!(
            out.contains(r##"fill="url(&quot;#paint)close&quot;)""##),
            "{out}"
        );
        assert!(!out.contains(" stroke="), "{out}");
        assert!(!out.contains(" filter="), "{out}");
        assert!(!out.contains(r"u\72l"), "{out}");
    }

    #[test]
    fn sanitize_element_attributes_strips_unsafe_css_url_functions() {
        let svg = r##"<svg><path fill="url(#paint)" stroke="url(javascript:alert(1))" style="clip-path: url(#clip); filter: url('java&#x73;cript:alert(1)'); stroke: #333"/></svg>"##;
        let out = sanitize_element_attributes(svg);
        let lower = out.to_ascii_lowercase();

        assert!(out.contains(r##"fill="url(#paint)""##), "got: {out}");
        assert!(out.contains("clip-path:url(#clip)"), "got: {out}");
        assert!(out.contains("stroke:#333"), "got: {out}");
        assert!(!lower.contains("javascript"), "got: {out}");
        assert!(!lower.contains(r#"stroke="url("#), "got: {out}");
        assert!(!lower.contains("filter:"), "got: {out}");
    }
}
