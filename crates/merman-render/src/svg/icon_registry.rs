use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;
use std::fmt::Write as _;
use std::ops::Range;

const MERMAID_UNKNOWN_ICON_BODY: &str = r#"<g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g>"#;
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";

pub(in crate::svg) fn mermaid_unknown_icon_svg(
    width: impl std::fmt::Display,
    height: impl std::fmt::Display,
) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 80 80">{MERMAID_UNKNOWN_ICON_BODY}</svg>"#
    )
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum IconRegistryError {
    #[error("Iconify JSON error: {0}")]
    Json(String),
    #[error("Iconify JSON is missing a non-empty prefix")]
    MissingPrefix,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IconSvg {
    body: String,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl IconSvg {
    pub fn new(body: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            body: body.into(),
            left: 0.0,
            top: 0.0,
            width: width.max(1.0),
            height: height.max(1.0),
        }
    }

    pub fn with_viewbox(mut self, left: f64, top: f64, width: f64, height: f64) -> Self {
        self.left = left;
        self.top = top;
        self.width = width.max(1.0);
        self.height = height.max(1.0);
        self
    }

    pub fn to_svg(&self, width_px: f64, height_px: f64, extra_class: Option<&str>) -> String {
        self.to_svg_with_id_scope(width_px, height_px, extra_class, None)
    }

    pub fn to_svg_with_id_scope(
        &self,
        width_px: f64,
        height_px: f64,
        extra_class: Option<&str>,
        id_scope: Option<&str>,
    ) -> String {
        let body = id_scope
            .map(|scope| scope_svg_internal_ids(&self.body, scope))
            .unwrap_or_else(|| self.body.clone());
        let xmlns_xlink = if self.body.contains("xlink:") {
            r#" xmlns:xlink="http://www.w3.org/1999/xlink""#
        } else {
            ""
        };
        let class_attr = extra_class
            .map(|class| format!(r#" class="{}""#, escape_xml_attr(class)))
            .unwrap_or_default();
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"{xmlns_xlink}{class_attr} width="{w}" height="{h}" viewBox="{left} {top} {vw} {vh}">{body}</svg>"#,
            w = fmt(width_px.max(1.0)),
            h = fmt(height_px.max(1.0)),
            left = fmt(self.left),
            top = fmt(self.top),
            vw = fmt(self.width),
            vh = fmt(self.height),
            body = body
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct IconRegistry {
    icons: FxHashMap<String, IconSvg>,
}

impl IconRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.icons.is_empty()
    }

    pub fn insert(&mut self, name: impl Into<String>, icon: IconSvg) {
        self.icons
            .insert(normalize_icon_key(name.into().as_str()), icon);
    }

    pub fn register_iconify_json_str(
        &mut self,
        json: &str,
        prefix_override: Option<&str>,
    ) -> Result<(), IconRegistryError> {
        let value: Value =
            serde_json::from_str(json).map_err(|err| IconRegistryError::Json(err.to_string()))?;
        self.register_iconify_json_value(&value, prefix_override)
    }

    pub fn register_iconify_json_value(
        &mut self,
        value: &Value,
        prefix_override: Option<&str>,
    ) -> Result<(), IconRegistryError> {
        let prefix = prefix_override
            .map(str::trim)
            .filter(|prefix| !prefix.is_empty())
            .or_else(|| value.get("prefix").and_then(Value::as_str).map(str::trim))
            .filter(|prefix| !prefix.is_empty())
            .ok_or(IconRegistryError::MissingPrefix)?;

        let defaults = IconDefaults {
            left: number_field(value, "left").unwrap_or(0.0),
            top: number_field(value, "top").unwrap_or(0.0),
            width: number_field(value, "width").unwrap_or(16.0),
            height: number_field(value, "height").unwrap_or(16.0),
        };

        let icons = value
            .get("icons")
            .and_then(Value::as_object)
            .ok_or_else(|| IconRegistryError::Json("missing `icons` object".to_string()))?;

        for (name, icon_value) in icons {
            if let Some(icon) = icon_from_value(icon_value, defaults) {
                self.insert(format!("{prefix}:{name}"), icon);
            }
        }

        if let Some(aliases) = value.get("aliases").and_then(Value::as_object) {
            for alias_name in aliases.keys() {
                let mut seen = FxHashSet::default();
                if let Some(icon) =
                    resolve_alias_icon(alias_name, aliases, icons, defaults, &mut seen)
                {
                    self.insert(format!("{prefix}:{alias_name}"), icon);
                }
            }
        }

        Ok(())
    }

    pub fn svg_for(
        &self,
        icon_name: &str,
        width_px: f64,
        height_px: f64,
        fallback_prefix: Option<&str>,
        extra_class: Option<&str>,
    ) -> Option<String> {
        let key = resolve_icon_key(icon_name, fallback_prefix)?;
        self.icons
            .get(&key)
            .map(|icon| icon.to_svg(width_px, height_px, extra_class))
    }

    pub fn svg_for_scoped(
        &self,
        icon_name: &str,
        width_px: f64,
        height_px: f64,
        fallback_prefix: Option<&str>,
        extra_class: Option<&str>,
        id_scope: &str,
    ) -> Option<String> {
        let key = resolve_icon_key(icon_name, fallback_prefix)?;
        self.icons
            .get(&key)
            .map(|icon| icon.to_svg_with_id_scope(width_px, height_px, extra_class, Some(id_scope)))
    }
}

pub(in crate::svg) fn scope_svg_internal_ids(body: &str, scope: &str) -> String {
    scope_svg_internal_ids_from_xml(body, scope)
        .unwrap_or_else(|| scope_svg_internal_ids_fallback(body, scope))
}

fn scope_svg_internal_ids_from_xml(body: &str, scope: &str) -> Option<String> {
    const WRAPPER_PREFIX: &str =
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">"#;
    const WRAPPER_SUFFIX: &str = "</svg>";

    let mut wrapped =
        String::with_capacity(WRAPPER_PREFIX.len() + body.len() + WRAPPER_SUFFIX.len());
    wrapped.push_str(WRAPPER_PREFIX);
    wrapped.push_str(body);
    wrapped.push_str(WRAPPER_SUFFIX);
    let document = roxmltree::Document::parse(&wrapped).ok()?;

    let prefix = deterministic_iconify_id_prefix(scope);
    let mut id_replacements = FxHashMap::default();
    for attribute in document
        .descendants()
        .filter(|node| node.is_element())
        .flat_map(|node| node.attributes())
        .filter(|attribute| attribute.name() == "id" && attribute.namespace().is_none())
    {
        let Some(range) = body_attribute_value_range(attribute, WRAPPER_PREFIX.len(), body.len())
        else {
            continue;
        };
        let id = &body[range];
        if id.is_empty() || id_replacements.contains_key(id) {
            continue;
        }
        let replacement = format!("{prefix}{}", id_replacements.len());
        id_replacements.insert(id.to_string(), replacement);
    }
    if id_replacements.is_empty() {
        return Some(body.to_string());
    }

    let mut edits = Vec::new();
    for attribute in document
        .descendants()
        .filter(|node| node.is_element())
        .flat_map(|node| node.attributes())
    {
        let Some(range) = body_attribute_value_range(attribute, WRAPPER_PREFIX.len(), body.len())
        else {
            continue;
        };
        let value = &body[range.clone()];
        if let Some(rewritten) = rewrite_svg_attribute_value(attribute, value, &id_replacements) {
            edits.push((range, rewritten));
        }
    }

    for node in document.descendants().filter(|node| {
        node.is_text()
            && node
                .parent()
                .is_some_and(|parent| parent.has_tag_name("style"))
    }) {
        let range = node.range();
        if range.start < WRAPPER_PREFIX.len() || range.end > WRAPPER_PREFIX.len() + body.len() {
            continue;
        }
        let range = range.start - WRAPPER_PREFIX.len()..range.end - WRAPPER_PREFIX.len();
        let value = &body[range.clone()];
        if let Some(rewritten) = rewrite_url_references(value, &id_replacements) {
            edits.push((range, rewritten));
        }
    }

    Some(apply_string_edits(body, edits))
}

fn body_attribute_value_range(
    attribute: roxmltree::Attribute<'_, '_>,
    wrapper_prefix_len: usize,
    body_len: usize,
) -> Option<Range<usize>> {
    let range = attribute.range_value();
    if range.start < wrapper_prefix_len || range.end > wrapper_prefix_len + body_len {
        return None;
    }
    Some(range.start - wrapper_prefix_len..range.end - wrapper_prefix_len)
}

fn rewrite_svg_attribute_value(
    attribute: roxmltree::Attribute<'_, '_>,
    value: &str,
    id_replacements: &FxHashMap<String, String>,
) -> Option<String> {
    if let Some(namespace) = attribute.namespace() {
        if namespace == XLINK_NAMESPACE && attribute.name() == "href" {
            return rewrite_fragment_reference(value, id_replacements);
        }
        return None;
    }

    match attribute.name() {
        "id" => id_replacements.get(value).cloned(),
        "href" => rewrite_fragment_reference(value, id_replacements),
        "begin" | "end" => rewrite_smil_timing_references(value, id_replacements),
        "aria-activedescendant"
        | "aria-controls"
        | "aria-describedby"
        | "aria-details"
        | "aria-errormessage"
        | "aria-flowto"
        | "aria-labelledby"
        | "aria-owns" => rewrite_idref_list(value, id_replacements),
        "clip-path" | "color-profile" | "cursor" | "fill" | "filter" | "marker" | "marker-end"
        | "marker-mid" | "marker-start" | "mask" | "stroke" | "style" => {
            rewrite_url_references(value, id_replacements)
        }
        _ => None,
    }
}

fn rewrite_fragment_reference(
    value: &str,
    id_replacements: &FxHashMap<String, String>,
) -> Option<String> {
    value
        .strip_prefix('#')
        .and_then(|id| id_replacements.get(id))
        .map(|replacement| format!("#{replacement}"))
}

fn rewrite_idref_list(value: &str, id_replacements: &FxHashMap<String, String>) -> Option<String> {
    let mut edits = Vec::new();
    let mut token_start = None;
    for (index, ch) in value
        .char_indices()
        .chain(std::iter::once((value.len(), ' ')))
    {
        if ch.is_whitespace() {
            if let Some(start) = token_start.take()
                && let Some(replacement) = id_replacements.get(&value[start..index])
            {
                edits.push((start..index, replacement.clone()));
            }
        } else if token_start.is_none() {
            token_start = Some(index);
        }
    }
    apply_optional_string_edits(value, edits)
}

fn rewrite_smil_timing_references(
    value: &str,
    id_replacements: &FxHashMap<String, String>,
) -> Option<String> {
    let mut edits = Vec::new();
    let mut segment_start = 0;
    for segment in value.split_inclusive(';') {
        let content = segment.strip_suffix(';').unwrap_or(segment);
        let leading = content.len() - content.trim_start().len();
        let timing = &content[leading..];
        let matching_id = id_replacements
            .keys()
            .filter(|id| {
                timing.strip_prefix(id.as_str()).is_some_and(|rest| {
                    rest.strip_prefix('.')
                        .and_then(|rest| rest.chars().next())
                        .is_some_and(|ch| ch.is_ascii_alphabetic())
                })
            })
            .max_by_key(|id| id.len());
        if let Some(id) = matching_id {
            let start = segment_start + leading;
            edits.push((start..start + id.len(), id_replacements[id].clone()));
        }
        segment_start += segment.len();
    }
    apply_optional_string_edits(value, edits)
}

fn rewrite_url_references(
    value: &str,
    id_replacements: &FxHashMap<String, String>,
) -> Option<String> {
    let bytes = value.as_bytes();
    let mut edits = Vec::new();
    let mut scan = 0;
    while scan + 4 <= bytes.len() {
        let Some(relative) = bytes[scan..]
            .windows(4)
            .position(|window| window.eq_ignore_ascii_case(b"url("))
        else {
            break;
        };
        let function_start = scan + relative;
        let mut cursor = function_start + 4;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        let quote = match bytes.get(cursor) {
            Some(b'\'' | b'"') => {
                let quote = bytes[cursor];
                cursor += 1;
                Some(quote)
            }
            _ => None,
        };
        if bytes.get(cursor) != Some(&b'#') {
            scan = function_start + 4;
            continue;
        }
        let id_start = cursor + 1;
        let Some((id_end, function_end)) = url_reference_end(bytes, id_start, quote) else {
            scan = function_start + 4;
            continue;
        };
        if let Some(replacement) = id_replacements.get(&value[id_start..id_end]) {
            edits.push((id_start..id_end, replacement.clone()));
        }
        scan = function_end;
    }
    apply_optional_string_edits(value, edits)
}

fn url_reference_end(bytes: &[u8], id_start: usize, quote: Option<u8>) -> Option<(usize, usize)> {
    if let Some(quote) = quote {
        let relative_end = bytes[id_start..].iter().position(|byte| *byte == quote)?;
        let id_end = id_start + relative_end;
        let mut cursor = id_end + 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        (bytes.get(cursor) == Some(&b')')).then_some((id_end, cursor + 1))
    } else {
        let relative_end = bytes[id_start..].iter().position(|byte| *byte == b')')?;
        let function_end = id_start + relative_end + 1;
        let mut id_end = function_end - 1;
        while id_end > id_start && bytes[id_end - 1].is_ascii_whitespace() {
            id_end -= 1;
        }
        Some((id_end, function_end))
    }
}

fn apply_string_edits(input: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    if edits.is_empty() {
        return input.to_string();
    }
    edits.sort_by_key(|(range, _)| range.start);
    let added_capacity = edits
        .iter()
        .map(|(range, replacement)| replacement.len().saturating_sub(range.len()))
        .sum::<usize>();
    let mut out = String::with_capacity(input.len() + added_capacity);
    let mut previous_end = 0;
    for (range, replacement) in edits {
        if range.start < previous_end || range.end > input.len() {
            continue;
        }
        out.push_str(&input[previous_end..range.start]);
        out.push_str(&replacement);
        previous_end = range.end;
    }
    out.push_str(&input[previous_end..]);
    out
}

fn apply_optional_string_edits(input: &str, edits: Vec<(Range<usize>, String)>) -> Option<String> {
    (!edits.is_empty()).then(|| apply_string_edits(input, edits))
}

fn scope_svg_internal_ids_fallback(body: &str, scope: &str) -> String {
    let declarations = collect_svg_internal_id_declarations(body);
    if declarations.is_empty() {
        return body.to_string();
    }

    let prefix = deterministic_iconify_id_prefix(scope);
    let mut id_indexes = FxHashMap::default();
    for (_, id) in &declarations {
        let next_index = id_indexes.len();
        id_indexes.entry(id.clone()).or_insert(next_index);
    }

    let mut edits = declarations
        .into_iter()
        .map(|(range, id)| {
            let replacement = format!("{prefix}{}", id_indexes[&id]);
            (range, replacement)
        })
        .collect::<Vec<_>>();
    for (id, index) in id_indexes {
        let replacement = format!("{prefix}{index}");
        edits.extend(
            collect_svg_reference_ranges(body, &id)
                .into_iter()
                .map(|range| (range, replacement.clone())),
        );
    }
    apply_string_edits(body, edits)
}

fn collect_svg_internal_id_declarations(body: &str) -> Vec<(Range<usize>, String)> {
    let mut declarations = Vec::new();
    let mut index = 0;
    while let Some((_, quote, value_start)) = find_next_id_attr(body, index) {
        let Some(relative_end) = body[value_start..].find(quote) else {
            break;
        };
        let value_end = value_start + relative_end;
        if value_start < value_end {
            declarations.push((
                value_start..value_end,
                body[value_start..value_end].to_string(),
            ));
        }
        index = value_end + quote.len_utf8();
    }
    declarations
}

fn find_next_id_attr(body: &str, from: usize) -> Option<(usize, char, usize)> {
    let bytes = body.as_bytes();
    let mut i = from;
    while i + 4 <= bytes.len() {
        if bytes[i].is_ascii_whitespace()
            && bytes[i + 1] == b'i'
            && bytes[i + 2] == b'd'
            && bytes[i + 3] == b'='
            && i + 4 < bytes.len()
            && (bytes[i + 4] == b'"' || bytes[i + 4] == b'\'')
        {
            return Some((i, bytes[i + 4] as char, i + 5));
        }
        i += 1;
    }
    None
}

fn deterministic_iconify_id_prefix(scope: &str) -> String {
    format!("IconifyId{:016x}", stable_hash64(scope))
}

fn stable_hash64(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn collect_svg_reference_ranges(body: &str, id: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut scan = 0;
    while let Some(relative) = body[scan..].find(id) {
        let start = scan + relative;
        let end = start + id.len();
        if is_svg_reference_boundary(body, start, end) {
            ranges.push(start..end);
        }
        scan = end;
    }
    ranges
}

fn is_svg_reference_boundary(body: &str, start: usize, end: usize) -> bool {
    let Some(prev) = body[..start].chars().next_back() else {
        return false;
    };
    let Some(next) = body[end..].chars().next() else {
        return false;
    };
    (prev == '#' && matches!(next, '"' | '\'' | ')'))
        || (matches!(prev, ';' | '"' | '\'')
            && next == '.'
            && body[end + next.len_utf8()..]
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphabetic()))
}

#[derive(Debug, Clone, Copy)]
struct IconDefaults {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

fn resolve_alias_icon(
    alias_name: &str,
    aliases: &serde_json::Map<String, Value>,
    icons: &serde_json::Map<String, Value>,
    defaults: IconDefaults,
    seen: &mut FxHashSet<String>,
) -> Option<IconSvg> {
    if !seen.insert(alias_name.to_string()) {
        return None;
    }

    let alias = aliases.get(alias_name)?;
    let parent = alias.get("parent").and_then(Value::as_str)?;
    let parent_icon = icons
        .get(parent)
        .and_then(|value| icon_from_value(value, defaults))
        .or_else(|| resolve_alias_icon(parent, aliases, icons, defaults, seen))?;

    Some(merge_icon_overrides(parent_icon, alias, defaults))
}

fn icon_from_value(value: &Value, defaults: IconDefaults) -> Option<IconSvg> {
    let body = value.get("body")?.as_str()?.to_string();
    Some(
        IconSvg::new(
            body,
            number_field(value, "width").unwrap_or(defaults.width),
            number_field(value, "height").unwrap_or(defaults.height),
        )
        .with_viewbox(
            number_field(value, "left").unwrap_or(defaults.left),
            number_field(value, "top").unwrap_or(defaults.top),
            number_field(value, "width").unwrap_or(defaults.width),
            number_field(value, "height").unwrap_or(defaults.height),
        ),
    )
}

fn merge_icon_overrides(mut icon: IconSvg, value: &Value, defaults: IconDefaults) -> IconSvg {
    if let Some(body) = value.get("body").and_then(Value::as_str) {
        icon.body = body.to_string();
    }
    icon.left = number_field(value, "left").unwrap_or(icon.left);
    icon.top = number_field(value, "top").unwrap_or(icon.top);
    icon.width = number_field(value, "width")
        .or(Some(icon.width))
        .unwrap_or(defaults.width)
        .max(1.0);
    icon.height = number_field(value, "height")
        .or(Some(icon.height))
        .unwrap_or(defaults.height)
        .max(1.0);
    icon
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(number_value)
}

fn number_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn resolve_icon_key(icon_name: &str, fallback_prefix: Option<&str>) -> Option<String> {
    let icon_name = icon_name.trim();
    if icon_name.is_empty() {
        return None;
    }

    if let Some(without_provider) = icon_name.strip_prefix('@') {
        let mut parts = without_provider.split(':');
        let _provider = parts.next()?;
        let prefix = parts.next()?;
        let name = parts.next()?;
        if parts.next().is_none() {
            return Some(normalize_icon_key(&format!("{prefix}:{name}")));
        }
        return None;
    }

    let colon_parts = icon_name.split(':').collect::<Vec<_>>();
    match colon_parts.as_slice() {
        [prefix, name] if !prefix.is_empty() && !name.is_empty() => {
            return Some(normalize_icon_key(&format!("{prefix}:{name}")));
        }
        [provider, prefix, name]
            if !provider.is_empty() && !prefix.is_empty() && !name.is_empty() =>
        {
            return Some(normalize_icon_key(&format!("{prefix}:{name}")));
        }
        _ => {}
    }

    if let Some(prefix) = fallback_prefix
        .map(str::trim)
        .filter(|prefix| !prefix.is_empty())
    {
        return Some(normalize_icon_key(&format!("{prefix}:{icon_name}")));
    }

    let mut parts = icon_name.split('-');
    let prefix = parts.next()?;
    let name = parts.collect::<Vec<_>>().join("-");
    if prefix.is_empty() || name.is_empty() {
        return None;
    }
    Some(normalize_icon_key(&format!("{prefix}:{name}")))
}

fn normalize_icon_key(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn fmt(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let mut out = String::new();
    let _ = write!(&mut out, "{value:.6}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    if out == "-0" { "0".to_string() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_iconify_json_icons_and_aliases() {
        let json = r##"{
            "prefix": "test",
            "width": 24,
            "height": 24,
            "icons": {
                "rocket": { "body": "<path id=\"rocket\" d=\"M1 1H23V23H1z\"/>" }
            },
            "aliases": {
                "ship": { "parent": "rocket", "width": 32 }
            }
        }"##;

        let mut registry = IconRegistry::new();
        registry.register_iconify_json_str(json, None).unwrap();

        let rocket = registry
            .svg_for("test:rocket", 48.0, 48.0, None, None)
            .unwrap();
        assert!(rocket.contains(r#"viewBox="0 0 24 24""#));
        assert!(rocket.contains(r#"id="rocket""#));

        let alias = registry
            .svg_for("test:ship", 48.0, 48.0, None, None)
            .unwrap();
        assert!(alias.contains(r#"viewBox="0 0 32 24""#));
    }

    #[test]
    fn scoped_svg_rewrites_internal_ids_and_references() {
        let icon = IconSvg::new(
            r##"<g xmlns:meta="urn:test" meta:id="shape" meta:href="#shape" meta:begin="shape.end" meta:aria-controls="shape" meta:fill="url(#none)"><defs><clipPath id="none"><path id='shape' d="M0 0H1V1H0z"/></clipPath></defs><path fill="none"/><use href="#shape" xlink:href="#shape" clip-path="url(#none)"/><animate begin="shape.end;shape.click"/></g>"##,
            16.0,
            16.0,
        );

        let svg = icon.to_svg_with_id_scope(16.0, 16.0, None, Some("diagram-node-a"));

        assert!(!svg.contains(r#"id="none""#), "{svg}");
        assert!(!svg.contains(r#"id='shape'"#), "{svg}");
        assert!(svg.contains(r#"fill="none""#), "{svg}");
        assert!(svg.contains(r#"meta:id="shape""#), "{svg}");
        assert!(svg.contains(r##"meta:href="#shape""##), "{svg}");
        assert!(svg.contains(r#"meta:begin="shape.end""#), "{svg}");
        assert!(svg.contains(r#"meta:aria-controls="shape""#), "{svg}");
        assert!(svg.contains(r#"meta:fill="url(#none)""#), "{svg}");
        let scoped_ids = svg.match_indices(r#"id="IconifyId"#).count()
            + svg.match_indices(r#"id='IconifyId"#).count();
        assert_eq!(scoped_ids, 2, "{svg}");

        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let shape_id = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("id").is_some())
            .and_then(|node| node.attribute("id"))
            .expect("scoped shape id");
        let clip_id = document
            .descendants()
            .find(|node| node.has_tag_name("clipPath"))
            .and_then(|node| node.attribute("id"))
            .expect("scoped clip id");
        let use_node = document
            .descendants()
            .find(|node| node.has_tag_name("use"))
            .expect("use node");
        let expected_href = format!("#{shape_id}");
        assert_eq!(use_node.attribute("href"), Some(expected_href.as_str()));
        assert_eq!(
            use_node.attribute((XLINK_NAMESPACE, "href")),
            Some(expected_href.as_str())
        );
        assert_eq!(
            use_node.attribute("clip-path"),
            Some(format!("url(#{clip_id})").as_str())
        );
        let begin = document
            .descendants()
            .find(|node| node.has_tag_name("animate"))
            .and_then(|node| node.attribute("begin"))
            .expect("animate begin");
        assert_eq!(begin, format!("{shape_id}.end;{shape_id}.click"));
    }

    #[test]
    fn scoped_svg_is_deterministic_for_same_scope_and_differs_across_scopes() {
        let icon = IconSvg::new(
            r##"<defs><clipPath id="clip"><path d="M0 0H1V1H0z"/></clipPath></defs><path clip-path="url(#clip)"/>"##,
            16.0,
            16.0,
        );

        let a1 = icon.to_svg_with_id_scope(16.0, 16.0, None, Some("diagram-node-a"));
        let a2 = icon.to_svg_with_id_scope(16.0, 16.0, None, Some("diagram-node-a"));
        let b = icon.to_svg_with_id_scope(16.0, 16.0, None, Some("diagram-node-b"));

        assert_eq!(a1, a2);
        assert_ne!(a1, b);
    }

    #[test]
    fn scoped_svg_fallback_preserves_unrelated_sentinel_like_text() {
        let scope = "diagram-node-a";
        let sentinel = format!("IconifyIdsuffix{:016x}", stable_hash64(scope));
        let body = format!(
            r##"<g id="none" data-note="{sentinel}&broken"><path fill="none" clip-path="url(#none)"/></g>"##
        );
        let icon = IconSvg::new(body, 16.0, 16.0);

        let svg = icon.to_svg_with_id_scope(16.0, 16.0, None, Some(scope));

        assert!(
            svg.contains(&format!(r#"data-note="{sentinel}&broken""#)),
            "{svg}"
        );
        assert!(svg.contains(r#"fill="none""#), "{svg}");
        assert!(!svg.contains(r#"id="none""#), "{svg}");
        assert!(!svg.contains(r#"url(#none)"#), "{svg}");
        let scoped_id = svg
            .split_once(r#"<g id=""#)
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(id, _)| id)
            .expect("fallback scoped id");
        let url_target = svg
            .split_once("url(#")
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(id, _)| id)
            .expect("fallback URL target");
        assert_eq!(url_target, scoped_id);
    }

    #[test]
    fn resolves_hyphen_icon_names_without_fallback_prefix() {
        let mut registry = IconRegistry::new();
        registry.insert("logos:aws-lambda", IconSvg::new("<path/>", 16.0, 16.0));

        assert!(
            registry
                .svg_for("logos-aws-lambda", 16.0, 16.0, None, None)
                .is_some()
        );
    }
}
