mod source_edit_map;

pub use source_edit_map::PreprocessedSource;

use crate::{DetectorRegistry, EditorLexemeKind, Error, MermaidConfig, Result, SourceSpan};
use serde_json::{Map, Value};
use source_edit_map::{ReplacementMapping, SourceEdit};
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub struct PreprocessResult {
    pub source: PreprocessedSource,
    pub title: Option<String>,
    pub config: MermaidConfig,
}

impl PreprocessResult {
    pub fn code(&self) -> &str {
        self.source.text()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontmatterByteSpan {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct FrontmatterBlock<'a> {
    pub full: FrontmatterByteSpan,
    pub body: FrontmatterByteSpan,
    pub indent: &'a str,
    pub dedented_body: Cow<'a, str>,
    pub stripped: &'a str,
}

const MAX_CONFIG_NESTING_DEPTH: usize = crate::MAX_DIAGRAM_NESTING_DEPTH;

pub fn preprocess_diagram(input: &str, registry: &DetectorRegistry) -> Result<PreprocessResult> {
    preprocess_diagram_with_known_type(input, registry, None)
}

pub fn preprocess_diagram_with_known_type(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<PreprocessResult> {
    preprocess_single_pass(PreprocessedSource::new(input), registry, diagram_type)
        .map(prepare_parser_code)
}

pub(crate) fn preprocess_mermaid_public_parse_pipeline(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<PreprocessResult> {
    let outer = preprocess_single_pass(PreprocessedSource::new(input), registry, diagram_type)?;
    // Mermaid `parse()` calls `preprocessDiagram()` in `processAndSetConfigs()` and again in
    // `getDiagramFromText()`. Only `Diagram.fromText()` prepares entities for the family parser.
    let inner = preprocess_single_pass(outer.source, registry, diagram_type)?;
    Ok(PreprocessResult {
        source: prepare_parser_text(inner.source),
        title: outer.title,
        config: outer.config,
    })
}

fn preprocess_single_pass(
    mut source: PreprocessedSource,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<PreprocessResult> {
    cleanup_text(&mut source);
    let (frontmatter_len, title, mut frontmatter_config) = {
        let (without_frontmatter, title, config) = process_frontmatter(source.text())?;
        (
            source.text().len() - without_frontmatter.len(),
            title,
            config,
        )
    };
    if frontmatter_len > 0 {
        source.record_global_lexeme(
            EditorLexemeKind::Frontmatter,
            SourceSpan::new(0, frontmatter_len),
        );
        source.apply_edits(vec![SourceEdit::delete(0..frontmatter_len)]);
    }

    let (directive_config, directive_removals) =
        process_directives(source.text(), registry, diagram_type)?;
    for removal in &directive_removals {
        source.record_global_lexeme(
            EditorLexemeKind::Directive,
            SourceSpan::new(removal.start, removal.end),
        );
    }
    source.apply_edits(
        directive_removals
            .into_iter()
            .map(SourceEdit::delete)
            .collect(),
    );

    frontmatter_config.deep_merge(directive_config.as_value());

    remove_mermaid_comments(&mut source);
    Ok(PreprocessResult {
        source,
        title,
        config: frontmatter_config,
    })
}

fn prepare_parser_code(mut preprocessed: PreprocessResult) -> PreprocessResult {
    preprocessed.source = prepare_parser_text(preprocessed.source);
    preprocessed
}

fn prepare_parser_text(mut source: PreprocessedSource) -> PreprocessedSource {
    if source.text().contains('#') {
        encode_mermaid_entities_like_upstream(&mut source);
    }
    source
}

fn cleanup_text(source: &mut PreprocessedSource) {
    normalize_crlf(source);

    // Mermaid performs this HTML attribute rewrite as part of preprocessing.
    if source.text().contains('<') && source.text().contains("=\"") {
        normalize_html_tag_attributes_like_upstream(source);
    }
}

fn remove_mermaid_comments(source: &mut PreprocessedSource) {
    if source.text().contains("%%") {
        let mut edits = Vec::new();
        let mut comments = Vec::new();
        let mut line_start = 0usize;
        for line in source.text().split_inclusive('\n') {
            let trimmed = line.trim_start();
            if let Some(after_marker) = trimmed.strip_prefix("%%") {
                let has_comment_body = after_marker.chars().next().is_some_and(|ch| ch != '\n');
                if !after_marker.starts_with('{') && has_comment_body {
                    let range = line_start..line_start + line.len();
                    comments.push(SourceSpan::new(range.start, range.end));
                    edits.push(SourceEdit::delete(range));
                }
            }
            line_start += line.len();
        }
        for span in comments {
            source.record_global_lexeme(EditorLexemeKind::Comment, span);
        }
        source.apply_edits(edits);
    }

    let trimmed_len = source.text().trim_start().len();
    let leading_whitespace = source.text().len() - trimmed_len;
    if leading_whitespace > 0 {
        source.apply_edits(vec![SourceEdit::delete(0..leading_whitespace)]);
    }
}

fn normalize_crlf(source: &mut PreprocessedSource) {
    if !source.text().contains('\r') {
        return;
    }
    let bytes = source.text().as_bytes();
    let mut edits = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        if bytes[cursor] == b'\r' {
            let end = cursor + usize::from(bytes.get(cursor + 1) == Some(&b'\n')) + 1;
            edits.push(SourceEdit::replace(
                cursor..end,
                "\n",
                if end - cursor == 1 {
                    ReplacementMapping::ExactBytes
                } else {
                    ReplacementMapping::Boundaries
                },
            ));
            cursor = end;
        } else {
            cursor += 1;
        }
    }
    source.apply_edits(edits);
}

fn normalize_html_tag_attributes_like_upstream(source: &mut PreprocessedSource) {
    let text = source.text();
    let bytes = text.as_bytes();
    let mut probe = 0usize;
    let mut edits = Vec::new();

    while let Some(rel_start) = text[probe..].find('<') {
        let start = probe + rel_start;
        let tag_start = start + 1;
        if tag_start >= bytes.len() || !is_mermaid_js_word_byte(bytes[tag_start]) {
            probe = tag_start;
            continue;
        }

        let mut tag_end = tag_start + 1;
        while tag_end < bytes.len() && is_mermaid_js_word_byte(bytes[tag_end]) {
            tag_end += 1;
        }

        let Some(rel_end) = text[tag_end..].find('>') else {
            probe = tag_start;
            continue;
        };
        let end = tag_end + rel_end;

        html_attribute_quote_edits(text, tag_end, end, &mut edits);

        probe = end + 1;
    }
    source.apply_edits(edits);
}

fn html_attribute_quote_edits(
    text: &str,
    attributes_start: usize,
    attributes_end: usize,
    edits: &mut Vec<SourceEdit>,
) {
    let attributes = &text[attributes_start..attributes_end];
    let mut probe = 0usize;

    while let Some(rel_start) = attributes[probe..].find("=\"") {
        let start = probe + rel_start;
        let value_start = start + 2;
        let Some(rel_end) = attributes[value_start..].find('"') else {
            probe = value_start;
            continue;
        };
        let end = value_start + rel_end;

        let opening_quote = attributes_start + start + 1;
        let closing_quote = attributes_start + end;
        edits.push(SourceEdit::replace(
            opening_quote..opening_quote + 1,
            "'",
            ReplacementMapping::ExactBytes,
        ));
        edits.push(SourceEdit::replace(
            closing_quote..closing_quote + 1,
            "'",
            ReplacementMapping::ExactBytes,
        ));

        probe = end + 1;
    }
}

fn encode_mermaid_entities_like_upstream(source: &mut PreprocessedSource) {
    if !source.text().contains('#') {
        return;
    }

    // Mirrors Mermaid `encodeEntities` (Mermaid@11.12.2):
    //
    // 1) Protect `style...:#...;` and `classDef...:#...;` so color hex fragments are not mistaken
    //    as entities by the `/#\\w+;/g` pass.
    // 2) Encode `#<name>;` and `#<number>;` sequences into placeholders that do not contain `#`/`;`.
    if source.text().contains("style") && source.text().contains(';') {
        strip_hex_style_semicolons_like_upstream(source, "style");
    }

    if source.text().contains("classDef") && source.text().contains(';') {
        strip_hex_style_semicolons_like_upstream(source, "classDef");
    }

    if source.text().contains(';') {
        encode_entity_placeholders_like_upstream(source);
    }
}

fn encode_entity_placeholders_like_upstream(source: &mut PreprocessedSource) {
    let text = source.text();
    let bytes = text.as_bytes();
    let mut cursor = 0usize;
    let mut edits = Vec::new();

    while let Some(rel_hash) = text[cursor..].find('#') {
        let start = cursor + rel_hash;
        let mut end = start + 1;
        while end < bytes.len() && is_mermaid_js_word_byte(bytes[end]) {
            end += 1;
        }

        if end > start + 1 && bytes.get(end) == Some(&b';') {
            let inner = &text[start + 1..end];
            let prefix = if inner.bytes().all(|b| b.is_ascii_digit()) {
                "ﬂ°°"
            } else {
                "ﬂ°"
            };
            edits.push(SourceEdit::replace(
                start..start + 1,
                prefix,
                ReplacementMapping::Boundaries,
            ));
            edits.push(SourceEdit::replace(
                end..end + 1,
                "¶ß",
                ReplacementMapping::Boundaries,
            ));
            cursor = end + 1;
        } else {
            cursor = start + 1;
        }
    }
    source.apply_edits(edits);
}

fn is_mermaid_js_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn strip_hex_style_semicolons_like_upstream(source: &mut PreprocessedSource, keyword: &str) {
    let text = source.text();
    let mut edits = Vec::new();
    let mut line_start = 0usize;

    for (idx, ch) in text.char_indices() {
        if ch == '\n' {
            collect_hex_style_semicolon_edits(
                &text[line_start..idx],
                line_start,
                keyword,
                &mut edits,
            );
            line_start = idx + ch.len_utf8();
        }
    }

    collect_hex_style_semicolon_edits(&text[line_start..], line_start, keyword, &mut edits);
    source.apply_edits(edits);
}

fn collect_hex_style_semicolon_edits(
    line: &str,
    line_start: usize,
    keyword: &str,
    edits: &mut Vec<SourceEdit>,
) {
    let mut cursor = 0usize;
    while let Some(semicolon) = find_hex_style_match(line, keyword, cursor) {
        edits.push(SourceEdit::delete(
            line_start + semicolon..line_start + semicolon + 1,
        ));
        cursor = semicolon + 1;
    }
}

fn find_hex_style_match(line: &str, keyword: &str, search_start: usize) -> Option<usize> {
    let mut probe = search_start;
    while let Some(rel_start) = line[probe..].find(keyword) {
        let start = probe + rel_start;
        if let Some(semicolon) = find_hex_style_match_end(line, start + keyword.len()) {
            return Some(semicolon);
        }
        probe = start + keyword.len();
    }
    None
}

fn find_hex_style_match_end(line: &str, search_start: usize) -> Option<usize> {
    let mut probe = search_start;
    while let Some(rel_colon) = line[probe..].find(':') {
        let colon = probe + rel_colon;
        let mut hash = None;
        for (rel, ch) in line[colon + 1..].char_indices() {
            if ch.is_whitespace() {
                break;
            }
            if ch == '#' {
                hash = Some(colon + 1 + rel);
                break;
            }
        }

        if let Some(hash) = hash {
            return line[hash + 1..].rfind(';').map(|rel| hash + 1 + rel);
        }

        probe = colon + 1;
    }
    None
}

fn process_frontmatter(input: &str) -> Result<(&str, Option<String>, MermaidConfig)> {
    let Some((yaml_body, stripped)) = split_frontmatter(input) else {
        return Ok((input, None, MermaidConfig::empty_object()));
    };

    #[cfg(not(feature = "full-config"))]
    {
        let _ = yaml_body;
        return Ok((stripped, None, MermaidConfig::empty_object()));
    }

    #[cfg(feature = "full-config")]
    {
        if config_nesting_exceeds_limit(yaml_body.as_ref()) {
            return Err(Error::InvalidFrontMatterYaml {
                message: format!("config nesting exceeds {MAX_CONFIG_NESTING_DEPTH} levels"),
            });
        }

        let parsed =
            crate::yaml_config::parse_yaml_value(yaml_body.as_ref(), MAX_CONFIG_NESTING_DEPTH)
                .map_err(|e| Error::InvalidFrontMatterYaml { message: e })?;
        let parsed_obj = match parsed {
            Value::Object(obj) => obj,
            other => {
                crate::config::drop_value_nonrecursive(other);
                Default::default()
            }
        };

        let mut title = None;
        let mut display_mode = None;

        if let Some(t) = parsed_obj
            .get("title")
            .filter(|value| frontmatter_truthy(value))
        {
            title = Some(frontmatter_to_string(t));
        }
        if let Some(dm) = parsed_obj
            .get("displayMode")
            .filter(|value| frontmatter_truthy(value))
        {
            display_mode = Some(frontmatter_to_string(dm));
        }

        let mut config = MermaidConfig::empty_object();
        merge_top_level_frontmatter_diagram_configs(&parsed_obj, &mut config);
        if let Some(v) = parsed_obj
            .get("config")
            .filter(|value| frontmatter_truthy(value))
        {
            config.deep_merge(v);
        }
        crate::config::mirror_legacy_font_family_into_theme_variables(&mut config);
        if let Some(dm) = display_mode {
            config.set_value("gantt.displayMode", Value::String(dm));
        }

        crate::config::drop_value_nonrecursive(Value::Object(parsed_obj));
        Ok((stripped, title, config))
    }
}

fn split_frontmatter(input: &str) -> Option<(Cow<'_, str>, &str)> {
    split_frontmatter_block(input).map(|block| (block.dedented_body, block.stripped))
}

pub fn split_frontmatter_block(input: &str) -> Option<FrontmatterBlock<'_>> {
    let open_line_end = input.find('\n')?;
    let open_line = input[..open_line_end].trim_end_matches('\r');
    let indent_end = frontmatter_indent_end(open_line);
    let indent = &open_line[..indent_end];
    let after_indent = &open_line[indent_end..];
    if !after_indent.starts_with("---") || !after_indent[3..].trim().is_empty() {
        return None;
    }

    let body_start = open_line_end + 1;
    let rest = &input[body_start..];
    let mut offset = 0usize;

    for line in rest.split_inclusive('\n') {
        let without_newline = line.trim_end_matches(['\r', '\n']);
        if is_frontmatter_closing_line(without_newline, indent) {
            let body = rest[..offset].strip_suffix('\n').unwrap_or(&rest[..offset]);
            let stripped = &rest[offset + line.len()..];
            return Some(FrontmatterBlock {
                full: FrontmatterByteSpan {
                    start: 0,
                    end: body_start + offset + line.len(),
                },
                body: FrontmatterByteSpan {
                    start: body_start,
                    end: body_start + body.len(),
                },
                indent,
                dedented_body: dedent_frontmatter_body(body, indent),
                stripped,
            });
        }
        offset += line.len();
    }

    None
}

pub fn parse_frontmatter_yaml_fields(
    input: &str,
) -> std::result::Result<Map<String, Value>, String> {
    #[cfg(feature = "full-config")]
    {
        let parsed = crate::yaml_config::parse_yaml_value(input, MAX_CONFIG_NESTING_DEPTH)?;
        match parsed {
            Value::Object(map) => Ok(map),
            other => {
                crate::config::drop_value_nonrecursive(other);
                Ok(Map::new())
            }
        }
    }

    #[cfg(not(feature = "full-config"))]
    {
        let _ = input;
        Ok(Map::new())
    }
}

pub fn diagram_config_key_for_type(diagram_type: &str) -> &str {
    crate::family::config_namespace_for_diagram_type(diagram_type).unwrap_or(diagram_type)
}

fn frontmatter_indent_end(line: &str) -> usize {
    let mut end = 0usize;
    for (idx, ch) in line.char_indices() {
        if ch == '\n' || ch == '\r' || !ch.is_whitespace() {
            break;
        }
        end = idx + ch.len_utf8();
    }
    end
}

fn is_frontmatter_closing_line(line: &str, indent: &str) -> bool {
    let Some(after_indent) = line.strip_prefix(indent) else {
        return false;
    };
    after_indent.starts_with("---") && after_indent[3..].trim().is_empty()
}

fn dedent_frontmatter_body<'a>(body: &'a str, indent: &str) -> Cow<'a, str> {
    if indent.is_empty() {
        return Cow::Borrowed(body);
    }

    let mut out = String::with_capacity(body.len());
    for (idx, line) in body.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line.strip_prefix(indent).unwrap_or(line));
    }
    Cow::Owned(out)
}

#[cfg(feature = "full-config")]
fn frontmatter_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

#[cfg(feature = "full-config")]
fn frontmatter_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Null => "null".to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

#[cfg(feature = "full-config")]
fn merge_top_level_frontmatter_diagram_configs(
    parsed_obj: &serde_json::Map<String, Value>,
    config: &mut MermaidConfig,
) {
    // Mermaid upstream only consumes `config`, but users commonly read docs examples as allowing
    // diagram config namespaces at the YAML root. Keep this compatibility narrow and explicit.
    for fact in crate::family::frontmatter_config_aliases() {
        if let Some(value) = parsed_obj.get(fact.source) {
            config.set_value(
                fact.namespace,
                crate::config::clone_value_nonrecursive(value),
            );
        }
    }

    for &key in crate::family::frontmatter_config_namespaces() {
        if let Some(value) = parsed_obj.get(key) {
            config.set_value(key, crate::config::clone_value_nonrecursive(value));
        }
    }
}

fn process_directives(
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<(MermaidConfig, Vec<std::ops::Range<usize>>)> {
    let directives = detect_directives(input)?;
    if directives.is_empty() {
        return Ok((MermaidConfig::empty_object(), Vec::new()));
    }
    let init = detect_init(&directives, input, registry, diagram_type)?;
    let wrap = directives.iter().any(|d| d.ty == "wrap");

    let mut merged = init;
    if wrap {
        merged.set_value("wrap", Value::Bool(true));
    }

    Ok((merged, directive_removal_ranges(input)))
}

fn detect_init(
    directives: &[Directive],
    input: &str,
    registry: &DetectorRegistry,
    diagram_type: Option<&str>,
) -> Result<MermaidConfig> {
    let mut merged = MermaidConfig::empty_object();
    let mut config_for_detect = MermaidConfig::empty_object();

    for d in directives {
        if d.ty != "init" && d.ty != "initialize" {
            continue;
        }

        let mut args = match &d.args {
            Some(v) => crate::config::clone_value_nonrecursive(v),
            None => Value::Object(Default::default()),
        };
        let mut diagram_specific = args
            .as_object_mut()
            .and_then(|object| object.remove("config"));

        sanitize_directive(&mut args);

        // Mermaid moves a top-level `config` directive field into the diagram-type-specific config.
        if let Some(mut diagram_specific_value) = diagram_specific.take() {
            sanitize_directive(&mut diagram_specific_value);
            let detected = diagram_type.map(|t| t.to_string()).or_else(|| {
                registry
                    .detect_type(input, &mut config_for_detect)
                    .ok()
                    .map(ToString::to_string)
            });

            if let Some(ty) = detected {
                let key = diagram_config_key_for_type(&ty).to_string();
                if let Value::Object(obj) = &mut args
                    && let Some(old) = obj.insert(key, diagram_specific_value)
                {
                    crate::config::drop_value_nonrecursive(old);
                }
            } else {
                crate::config::drop_value_nonrecursive(diagram_specific_value);
            }
        }
        crate::config::mirror_legacy_font_family_into_theme_variables_value(&mut args);

        merged.deep_merge(&args);
    }

    Ok(merged)
}

#[derive(Debug, Clone)]
struct Directive {
    ty: String,
    args: Option<Value>,
}

fn detect_directives(input: &str) -> Result<Vec<Directive>> {
    let mut out = Vec::new();
    let mut pos = 0;
    let trimmed = input.trim();
    if !trimmed.contains("%%{") {
        return Ok(out);
    }

    // Mermaid's directive parser effectively treats single quotes as double quotes for JSON-like
    // directive bodies. Keep this behavior, but only pay the allocation when directives exist.
    let text = trimmed.replace('\'', "\"");

    while let Some(rel) = text[pos..].find("%%{") {
        let start = pos + rel;
        let content_start = start + 3;
        let Some(rel_end) = text[content_start..].find("}%%") else {
            break;
        };
        let content_end = content_start + rel_end;
        let raw = text[content_start..content_end].trim();

        if let Some(d) = parse_directive(raw)? {
            out.push(d);
        }

        pos = content_end + 3;
    }

    Ok(out)
}

#[derive(Clone)]
enum DirectiveValuePathSegment {
    Key(String),
    Index(usize),
}

#[derive(Clone, Copy)]
enum DirectiveDictionaryKind {
    NodeColors,
    IconReferences,
}

fn sanitize_directive(value: &mut Value) {
    let mut stack = vec![Vec::<DirectiveValuePathSegment>::new()];

    while let Some(path) = stack.pop() {
        let Some(current) = directive_value_at_path_mut(value, &path) else {
            continue;
        };

        match current {
            Value::Object(map) => {
                if let Some(old) = map.remove("secure") {
                    crate::config::drop_value_nonrecursive(old);
                }

                let blocked_keys = map
                    .iter()
                    .filter(|(key, value)| {
                        is_suspicious_directive_key(key)
                            || !crate::generated::is_default_config_key(key)
                            || value.is_null()
                    })
                    .map(|(key, _)| key)
                    .cloned()
                    .collect::<Vec<_>>();
                for key in blocked_keys {
                    if let Some(old) = map.remove(&key) {
                        crate::config::drop_value_nonrecursive(old);
                    }
                }

                let child_keys = map.keys().cloned().collect::<Vec<_>>();
                for key in child_keys.into_iter().rev() {
                    if let Some(kind) = directive_dictionary_kind(&key)
                        && map
                            .get_mut(&key)
                            .is_some_and(|child| sanitize_directive_dictionary(child, kind))
                    {
                        continue;
                    }

                    let mut child_path = path.clone();
                    child_path.push(DirectiveValuePathSegment::Key(key));
                    stack.push(child_path);
                }
            }
            Value::Array(arr) => {
                for idx in (0..arr.len()).rev() {
                    let mut child_path = path.clone();
                    child_path.push(DirectiveValuePathSegment::Index(idx));
                    stack.push(child_path);
                }
            }
            Value::String(s) => {
                if directive_path_is_css(&path) && !css_braces_are_balanced(s) {
                    *s = "{ /* ERROR: Unbalanced CSS */ }".to_string();
                }
                let blocked = s.contains('<') || s.contains('>') || s.contains("url(data:");
                if blocked {
                    s.clear();
                }
            }
            _ => {}
        }
    }
}

fn directive_path_is_css(path: &[DirectiveValuePathSegment]) -> bool {
    matches!(
        path.last(),
        Some(DirectiveValuePathSegment::Key(key))
            if ["themeCSS", "fontFamily", "altFontFamily"]
                .iter()
                .any(|css_key| key.contains(css_key))
    )
}

fn is_suspicious_directive_key(key: &str) -> bool {
    key.starts_with("__") || key.contains("proto") || key.contains("constr")
}

fn css_braces_are_balanced(css: &str) -> bool {
    let mut depth = 0usize;
    for ch in css.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                let Some(next) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next;
            }
            _ => {}
        }
    }
    depth == 0
}

fn directive_dictionary_kind(key: &str) -> Option<DirectiveDictionaryKind> {
    // Source: Mermaid 11.16 `sanitizeDirective.ts` DICTIONARY_CONFIG_PATTERNS.
    match key {
        "nodeColors" => Some(DirectiveDictionaryKind::NodeColors),
        "filenameIcons" | "extensionIcons" => Some(DirectiveDictionaryKind::IconReferences),
        _ => None,
    }
}

fn sanitize_directive_dictionary(value: &mut Value, kind: DirectiveDictionaryKind) -> bool {
    let is_valid_value = |value: &Value| {
        value.as_str().is_some_and(|value| match kind {
            DirectiveDictionaryKind::NodeColors => is_valid_node_color(value),
            DirectiveDictionaryKind::IconReferences => is_valid_icon_reference(value),
        })
    };

    match value {
        Value::Object(map) => {
            let blocked_keys = map
                .iter()
                .filter(|(key, value)| is_suspicious_dictionary_key(key) || !is_valid_value(value))
                .map(|(key, _)| key.clone())
                .collect::<Vec<_>>();

            for key in blocked_keys {
                if let Some(old) = map.remove(&key) {
                    crate::config::drop_value_nonrecursive(old);
                }
            }
            true
        }
        Value::Array(values) => {
            for value in values.iter_mut().filter(|value| !is_valid_value(value)) {
                let old = std::mem::replace(value, Value::Null);
                crate::config::drop_value_nonrecursive(old);
            }
            true
        }
        _ => false,
    }
}

fn is_suspicious_dictionary_key(key: &str) -> bool {
    is_suspicious_directive_key(key)
}

fn is_valid_icon_reference(value: &str) -> bool {
    let mut segments = value.split(':');
    let Some(first) = segments.next() else {
        return false;
    };
    let second = segments.next();

    is_valid_icon_reference_segment(first)
        && second.is_none_or(is_valid_icon_reference_segment)
        && segments.next().is_none()
}

fn is_valid_icon_reference_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn is_valid_node_color(value: &str) -> bool {
    if let Some(hex) = value.strip_prefix('#') {
        return (3..=8).contains(&hex.len()) && hex.bytes().all(|byte| byte.is_ascii_hexdigit());
    }

    if is_valid_node_color_function(value, "rgb(") || is_valid_node_color_function(value, "hsl(") {
        return true;
    }

    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_valid_node_color_function(value: &str, prefix: &str) -> bool {
    let Some(actual_prefix) = value.get(..prefix.len()) else {
        return false;
    };
    if !actual_prefix.eq_ignore_ascii_case(prefix) || !value.ends_with(')') {
        return false;
    }

    let inner = &value[prefix.len()..value.len() - 1];
    !inner.is_empty()
        && inner.chars().all(|ch| {
            ch.is_ascii_digit() || is_js_regex_whitespace(ch) || matches!(ch, '%' | ',' | '.')
        })
}

fn is_js_regex_whitespace(ch: char) -> bool {
    if ('\u{2000}'..='\u{200A}').contains(&ch) {
        return true;
    }

    matches!(
        ch,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

fn directive_value_at_path_mut<'a>(
    mut value: &'a mut Value,
    path: &[DirectiveValuePathSegment],
) -> Option<&'a mut Value> {
    for segment in path {
        match segment {
            DirectiveValuePathSegment::Key(key) => {
                value = value.as_object_mut()?.get_mut(key)?;
            }
            DirectiveValuePathSegment::Index(idx) => {
                value = value.as_array_mut()?.get_mut(*idx)?;
            }
        }
    }
    Some(value)
}

fn directive_removal_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut pos = 0;
    while let Some(rel) = text[pos..].find("%%{") {
        let start = pos + rel;
        let after_start = start + 3;
        if let Some(rel_end) = text[after_start..].find("}%%") {
            let end = after_start + rel_end + 3;
            ranges.push(start..end);
            pos = end;
        } else {
            ranges.push(start..text.len());
            return ranges;
        }
    }
    ranges
}

fn parse_directive(raw: &str) -> Result<Option<Directive>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }

    let mut chars = raw.chars().peekable();
    let mut ty = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ty.push(c);
            chars.next();
            continue;
        }
        break;
    }
    if ty.is_empty() {
        return Ok(None);
    }

    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }

    let args = if matches!(chars.peek(), Some(':')) {
        chars.next();
        while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
            chars.next();
        }
        let rest: String = chars.collect();
        let rest = rest.trim();
        if rest.is_empty() {
            None
        } else if rest.starts_with('{') || rest.starts_with('[') {
            if config_nesting_exceeds_limit(rest) {
                return Err(Error::InvalidDirectiveJson {
                    message: format!("config nesting exceeds {MAX_CONFIG_NESTING_DEPTH} levels"),
                });
            }
            Some(parse_directive_config_value(rest)?)
        } else {
            Some(Value::String(rest.to_string()))
        }
    } else {
        None
    };

    Ok(Some(Directive { ty, args }))
}

fn parse_directive_config_value(input: &str) -> Result<Value> {
    #[cfg(feature = "full-config")]
    {
        json5::from_str::<Value>(input).map_err(|e| Error::InvalidDirectiveJson {
            message: e.to_string(),
        })
    }

    #[cfg(not(feature = "full-config"))]
    {
        crate::inline_config::parse_inline_config_value(input)
            .map_err(|e| Error::InvalidDirectiveJson { message: e })
    }
}

fn config_nesting_exceeds_limit(text: &str) -> bool {
    max_flow_collection_depth(text) > MAX_CONFIG_NESTING_DEPTH
        || max_yaml_indent_depth(text) > MAX_CONFIG_NESTING_DEPTH
}

fn max_flow_collection_depth(text: &str) -> usize {
    let mut max_depth = 0usize;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    for ch in text.chars() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == q {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '{' | '[' => {
                depth = depth.saturating_add(1);
                max_depth = max_depth.max(depth);
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    max_depth
}

fn max_yaml_indent_depth(text: &str) -> usize {
    let mut indents = Vec::<usize>::new();
    let mut max_depth = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start_matches(' ').len();
        while indents.last().is_some_and(|prev| indent <= *prev) {
            indents.pop();
        }
        indents.push(indent);
        let inline_sequence_depth = yaml_inline_sequence_indicator_count(trimmed);
        max_depth = max_depth.max(indents.len() + inline_sequence_depth.saturating_sub(1));
    }

    max_depth
}

fn yaml_inline_sequence_indicator_count(mut text: &str) -> usize {
    let mut count = 0usize;
    loop {
        let Some(after_dash) = text.strip_prefix('-') else {
            return count;
        };
        if after_dash
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace())
        {
            return count;
        }
        count += 1;
        text = after_dash.trim_start();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};

    fn transformed(input: &str, transform: fn(&mut PreprocessedSource)) -> String {
        let mut source = PreprocessedSource::new(input);
        transform(&mut source);
        source.into_text()
    }

    #[test]
    fn normalize_crlf_matches_mermaid_line_ending_cleanup() {
        assert_eq!(
            transformed("flowchart TD\r\nA-->B\rC-->D\n", normalize_crlf),
            "flowchart TD\nA-->B\nC-->D\n"
        );
        assert_eq!(transformed("\r\r\n\n", normalize_crlf), "\n\n\n");
    }

    #[test]
    fn normalize_html_tag_attributes_matches_mermaid_cleanup_shape() {
        assert_eq!(
            transformed(
                r#"<span title="A" data-empty="">Label</span><br disabled="yes">"#,
                normalize_html_tag_attributes_like_upstream,
            ),
            r#"<span title='A' data-empty=''>Label</span><br disabled='yes'>"#
        );
        assert_eq!(
            transformed(
                r#"<é title="A"><_x value="B"><1 n="C">"#,
                normalize_html_tag_attributes_like_upstream,
            ),
            r#"<é title="A"><_x value='B'><1 n='C'>"#
        );
        assert_eq!(
            transformed(
                r#"<span a="x" title="A>B">"#,
                normalize_html_tag_attributes_like_upstream,
            ),
            r#"<span a='x' title="A>B">"#
        );
        assert_eq!(
            transformed(
                r#"<<span title="A">"#,
                normalize_html_tag_attributes_like_upstream,
            ),
            r#"<<span title='A'>"#
        );
    }

    #[test]
    fn normalize_html_attribute_quotes_keep_exact_unicode_source_spans() {
        let original = r#"flowchart TD
A["<span title="😀">Label</span>"]
"#;
        let mut source = PreprocessedSource::new(original);
        normalize_html_tag_attributes_like_upstream(&mut source);

        assert!(source.text().contains("title='😀'"));
        let emoji = source.text().find('😀').unwrap();
        let mapped = source
            .try_map_span(crate::SourceSpan::new(emoji, emoji + '😀'.len_utf8()))
            .expect("normalized attribute value span");
        assert_eq!(&original[mapped.start..mapped.end], "😀");
    }

    #[test]
    fn encode_entity_placeholders_matches_mermaid_ascii_word_shape() {
        assert_eq!(
            transformed(
                "Hello #there; #andHere;#77653;",
                encode_mermaid_entities_like_upstream,
            ),
            "Hello ﬂ°there¶ß ﬂ°andHere¶ßﬂ°°77653¶ß"
        );
        assert_eq!(
            transformed(
                "style this; is ; everything :something#not-nothing; and this too;",
                encode_mermaid_entities_like_upstream,
            ),
            "style this; is ; everything :something#not-nothing; and this too"
        );
        assert_eq!(
            transformed(
                "classDef this; is ; everything :something#not-nothing; and this too;",
                encode_mermaid_entities_like_upstream,
            ),
            "classDef this; is ; everything :something#not-nothing; and this too"
        );
        assert_eq!(
            transformed(
                "style a fill:#fff; style b fill:#000;",
                encode_mermaid_entities_like_upstream,
            ),
            "style a fill:ﬂ°fff¶ß style b fill:#000"
        );
        assert_eq!(
            transformed("style a fill: #fff;", encode_mermaid_entities_like_upstream,),
            "style a fill: ﬂ°fff¶ß"
        );
        assert_eq!(
            transformed(
                "#é; #+123; #has-dash;",
                encode_mermaid_entities_like_upstream,
            ),
            "#é; #+123; #has-dash;"
        );
    }

    #[test]
    fn sanitize_directive_handles_deep_values_with_small_stack() {
        const DEPTH: usize = 2_048;
        let mut value = deep_directive_value(DEPTH, Value::String("<blocked>".to_string()));

        let handle = std::thread::Builder::new()
            .name("preprocess-deep-directive-sanitize".to_string())
            .stack_size(64 * 1024)
            .spawn(move || {
                sanitize_directive(&mut value);
                assert_eq!(
                    deep_directive_leaf(&value, DEPTH).and_then(Value::as_str),
                    Some("")
                );
                crate::config::drop_value_nonrecursive(value);
            })
            .expect("spawn deep directive sanitizer test");
        handle
            .join()
            .expect("deep directive sanitizer should finish without stack overflow");
    }

    #[test]
    fn sanitize_directive_replaces_unbalanced_css_like_mermaid() {
        let mut value = json!({
            "themeCSS": "} * { background: red }",
            "flowchart": {
                "fontFamily": "valid { nested: value; }",
                "altFontFamily": "missing { close"
            }
        });

        sanitize_directive(&mut value);

        assert_eq!(
            value["themeCSS"],
            Value::String("{ /* ERROR: Unbalanced CSS */ }".to_string())
        );
        assert_eq!(value["flowchart"]["fontFamily"], "valid { nested: value; }");
        assert!(value["flowchart"].get("altFontFamily").is_none());
    }

    #[test]
    fn sanitize_directive_uses_generated_config_shape_for_all_value_kinds() {
        let mut value = json!({
            "notAConfigKey": "removed",
            "theme": null,
            "prototype": "removed",
            "constructor": "removed",
            "deterministicIDSeed": "accepted undefined key",
            "sequence": {
                "messageFont": "accepted function key",
                "unknownNestedKey": true
            },
            "secure": ["theme"],
            "flowchart": {
                "secure": ["htmlLabels"],
                "htmlLabels": false
            }
        });

        sanitize_directive(&mut value);

        assert_eq!(
            value,
            json!({
                "deterministicIDSeed": "accepted undefined key",
                "sequence": {
                    "messageFont": "accepted function key"
                },
                "flowchart": {
                    "htmlLabels": false
                }
            })
        );
    }

    #[test]
    fn sanitize_directive_preserves_valid_dictionary_entries() {
        let mut value = json!({
            "sankey": {
                "nodeColors": {
                    "shortHex": "#abc",
                    "alphaHex": "#12345678",
                    "rgb": "rgb(0, 10%, 255)",
                    "hsl": "hsl(120, 50%, 25.5%)",
                    "named": "rebeccapurple"
                }
            },
            "treeView": {
                "filenameIcons": {
                    "Makefile": "cmake",
                    "README.md": "fa:bell"
                },
                "extensionIcons": {
                    ".ts": "logos:typescript-icon",
                    ".txt": "none"
                }
            }
        });

        sanitize_directive(&mut value);

        assert_eq!(value["sankey"]["nodeColors"]["shortHex"], "#abc");
        assert_eq!(value["sankey"]["nodeColors"]["alphaHex"], "#12345678");
        assert_eq!(value["sankey"]["nodeColors"]["rgb"], "rgb(0, 10%, 255)");
        assert_eq!(value["sankey"]["nodeColors"]["hsl"], "hsl(120, 50%, 25.5%)");
        assert_eq!(value["sankey"]["nodeColors"]["named"], "rebeccapurple");
        assert_eq!(value["treeView"]["filenameIcons"]["README.md"], "fa:bell");
        assert_eq!(
            value["treeView"]["extensionIcons"][".ts"],
            "logos:typescript-icon"
        );
    }

    #[test]
    fn sanitize_directive_removes_invalid_dictionary_values() {
        let mut value = json!({
            "sankey": {
                "nodeColors": {
                    "valid": "#ff0000",
                    "short": "#12",
                    "function": "url(javascript:alert(1))",
                    "wrongType": 42
                }
            },
            "treeView": {
                "filenameIcons": {
                    "valid": "docker",
                    "markup": "<script>alert(1)</script>",
                    "wrongType": false
                },
                "extensionIcons": {
                    ".ts": "logos:typescript-icon",
                    ".css": "not a valid name",
                    ".json": "one:two:three"
                }
            }
        });

        sanitize_directive(&mut value);

        assert_eq!(value["sankey"]["nodeColors"], json!({ "valid": "#ff0000" }));
        assert_eq!(
            value["treeView"]["filenameIcons"],
            json!({ "valid": "docker" })
        );
        assert_eq!(
            value["treeView"]["extensionIcons"],
            json!({ ".ts": "logos:typescript-icon" })
        );
    }

    #[test]
    fn sanitize_directive_removes_suspicious_dictionary_keys() {
        let mut value = json!({
            "sankey": {
                "nodeColors": {
                    "__proto__hack": "red",
                    "prototype": "green",
                    "constructor.js": "blue",
                    "safe": "black"
                }
            },
            "treeView": {
                "filenameIcons": {
                    "__proto__hack": "docker",
                    "prototype.ts": "docker",
                    "constructor.js": "docker",
                    "a.ts": "docker"
                }
            }
        });

        sanitize_directive(&mut value);

        assert_eq!(value["sankey"]["nodeColors"], json!({ "safe": "black" }));
        assert_eq!(
            value["treeView"]["filenameIcons"],
            json!({ "a.ts": "docker" })
        );
    }

    #[test]
    fn sanitize_directive_validates_dictionary_arrays_like_javascript_objects() {
        let mut value = json!({
            "sankey": {
                "nodeColors": ["#ff0000", "url(javascript:alert(1))", 42]
            },
            "treeView": {
                "extensionIcons": ["logos:typescript-icon", "not a valid name", false]
            }
        });

        sanitize_directive(&mut value);

        assert_eq!(
            value["sankey"]["nodeColors"],
            json!(["#ff0000", null, null])
        );
        assert_eq!(
            value["treeView"]["extensionIcons"],
            json!(["logos:typescript-icon", null, null])
        );
    }

    #[test]
    fn config_nesting_counts_inline_yaml_sequence_indicators() {
        let yaml = format!(
            "config:\n  {}\"leaf\"",
            "- ".repeat(MAX_CONFIG_NESTING_DEPTH + 1)
        );
        assert!(config_nesting_exceeds_limit(&yaml));
    }

    fn deep_directive_value(depth: usize, leaf: Value) -> Value {
        let mut value = leaf;
        for _ in 0..depth {
            let mut map = Map::new();
            map.insert("flowchart".to_string(), value);
            value = Value::Object(map);
        }
        value
    }

    fn deep_directive_leaf(mut value: &Value, depth: usize) -> Option<&Value> {
        for _ in 0..depth {
            value = value.as_object()?.get("flowchart")?;
        }
        Some(value)
    }
}
