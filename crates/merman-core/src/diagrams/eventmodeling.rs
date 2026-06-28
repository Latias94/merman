use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Value, json};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventModelingFrameRenderModel {
    pub name: String,
    #[serde(rename = "frameKind")]
    pub frame_kind: String,
    #[serde(rename = "modelEntityType")]
    pub model_entity_type: String,
    #[serde(rename = "entityIdentifier")]
    pub entity_identifier: String,
    #[serde(default, rename = "sourceFrames")]
    pub source_frames: Vec<String>,
    #[serde(default, rename = "dataInlineValue")]
    pub data_inline_value: Option<String>,
    #[serde(default, rename = "dataReference")]
    pub data_reference: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EventModelingDataEntityRenderModel {
    pub name: String,
    #[serde(rename = "dataBlockValue")]
    pub data_block_value: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EventModelingDiagramRenderModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub frames: Vec<EventModelingFrameRenderModel>,
    #[serde(default, rename = "dataEntities")]
    pub data_entities: Vec<EventModelingDataEntityRenderModel>,
}

impl EventModelingDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

pub fn parse_eventmodeling(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model = parse_eventmodeling_model_for_render(code, meta)?;
    Ok(json!({
        "type": meta.diagram_type,
        "title": model.title,
        "accTitle": model.acc_title,
        "accDescr": model.acc_descr,
        "frames": model.frames,
        "dataEntities": model.data_entities,
    }))
}

pub fn parse_eventmodeling_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<EventModelingDiagramRenderModel> {
    let body = strip_header(code, meta)?;
    let mut frames = Vec::new();
    let mut data_entities = Vec::new();
    let mut lines = body.lines().peekable();

    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }

        if starts_with_keyword(line, "data") {
            data_entities.push(parse_data_entity(line, &mut lines, meta)?);
            continue;
        }

        if let Some(frame) = parse_frame_line(line, meta)? {
            frames.push(frame);
        }
    }

    Ok(EventModelingDiagramRenderModel {
        frames,
        data_entities,
        ..Default::default()
    })
}

pub fn parse_eventmodeling_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut saw_header = false;

    while let Some(segment) = lines.next() {
        let line_start = offset;
        offset += segment.len();
        let line = segment.trim_end_matches(['\n', '\r']);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        if !saw_header {
            if !trimmed.starts_with("eventmodeling") {
                return facts;
            }
            saw_header = true;
            let rel = line.find("eventmodeling").unwrap_or(0);
            let span = SourceSpan::new(line_start + rel, line_start + rel + "eventmodeling".len());
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                span,
            ));
            facts.push_symbol(EditorSemanticSymbol::payload(
                "eventmodeling".to_string(),
                Some("eventmodeling header".to_string()),
                EditorSemanticKind::String,
                span,
                span,
            ));
            continue;
        }

        if let Some(frame) = parse_frame_facts(trimmed, line_start) {
            let name_span = frame.name_span;
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                name_span,
            ));
            facts.push_symbol(EditorSemanticSymbol::new(
                frame.name.clone(),
                Some("eventmodeling frame".to_string()),
                EditorSemanticKind::Namespace,
                frame.name_span,
                frame.name_span,
            ));
            facts.push_symbol(EditorSemanticSymbol::payload(
                frame.model_entity_type.clone(),
                Some("eventmodeling entity type".to_string()),
                EditorSemanticKind::String,
                frame.model_entity_type_span,
                frame.model_entity_type_span,
            ));
            facts.push_symbol(EditorSemanticSymbol::payload(
                frame.entity_identifier.clone(),
                Some("eventmodeling entity identifier".to_string()),
                EditorSemanticKind::String,
                frame.entity_identifier_span,
                frame.entity_identifier_span,
            ));
            for source in frame.source_frames {
                facts.push_symbol(EditorSemanticSymbol::new(
                    source.text.to_string(),
                    Some("eventmodeling source frame".to_string()),
                    EditorSemanticKind::Namespace,
                    source.span,
                    source.span,
                ));
            }
            if let Some(data_ref) = frame.data_reference {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    data_ref.text.to_string(),
                    Some("eventmodeling data reference".to_string()),
                    EditorSemanticKind::String,
                    data_ref.span,
                    data_ref.span,
                ));
            }
            if let Some(data_inline) = frame.data_inline_value {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    data_inline.text.to_string(),
                    Some("eventmodeling inline data".to_string()),
                    EditorSemanticKind::String,
                    data_inline.span,
                    data_inline.span,
                ));
            }
            continue;
        }

        if let Some(data_entity) = parse_data_entity_line(trimmed, line_start) {
            facts.push_symbol(EditorSemanticSymbol::new(
                data_entity.name.text.to_string(),
                Some("eventmodeling data entity".to_string()),
                EditorSemanticKind::Namespace,
                data_entity.name.span,
                data_entity.name.span,
            ));
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                data_entity.block_span,
            ));
            facts.push_symbol(EditorSemanticSymbol::payload(
                data_entity.block_text.to_string(),
                Some("eventmodeling data block".to_string()),
                EditorSemanticKind::String,
                data_entity.block_span,
                data_entity.block_span,
            ));
        }
    }

    facts
}

#[derive(Debug, Clone)]
struct EventModelingFieldSpan {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct EventModelingFrameFacts {
    name: String,
    name_span: SourceSpan,
    model_entity_type: String,
    model_entity_type_span: SourceSpan,
    entity_identifier: String,
    entity_identifier_span: SourceSpan,
    source_frames: Vec<EventModelingFieldSpan>,
    data_reference: Option<EventModelingFieldSpan>,
    data_inline_value: Option<EventModelingFieldSpan>,
}

#[derive(Debug, Clone)]
struct EventModelingDataEntityFacts {
    name: EventModelingFieldSpan,
    block_text: String,
    block_span: SourceSpan,
}

fn parse_frame_facts(line: &str, line_start: usize) -> Option<EventModelingFrameFacts> {
    let (_frame_kind, rest) = if let Some(rest) = strip_keyword(line, "tf") {
        ("timeframe", rest)
    } else if let Some(rest) = strip_keyword(line, "timeframe") {
        ("timeframe", rest)
    } else if let Some(rest) = strip_keyword(line, "rf") {
        ("resetframe", rest)
    } else if let Some(rest) = strip_keyword(line, "resetframe") {
        ("resetframe", rest)
    } else {
        return None;
    };

    let rest = rest.trim_start();
    let mut parts = rest.splitn(4, char::is_whitespace);
    let name = parts.next()?.to_string();
    let model_entity_type = parts.next()?.to_string();
    let entity_identifier = parts.next()?.to_string();
    let tail = parts.next().unwrap_or("").trim();

    let name_rel = line.find(&name)?;
    let type_rel = line.find(&model_entity_type)?;
    let id_rel = line.find(&entity_identifier)?;

    let source_frames = parse_source_frames_spanned(tail, line_start, line);
    let data_reference = parse_data_reference_spanned(tail, line_start, line);
    let data_inline_value = parse_inline_data_spanned(tail, line_start, line);

    Some(EventModelingFrameFacts {
        name: name.clone(),
        name_span: SourceSpan::new(line_start + name_rel, line_start + name_rel + name.len()),
        model_entity_type: model_entity_type.clone(),
        model_entity_type_span: SourceSpan::new(
            line_start + type_rel,
            line_start + type_rel + model_entity_type.len(),
        ),
        entity_identifier: entity_identifier.clone(),
        entity_identifier_span: SourceSpan::new(
            line_start + id_rel,
            line_start + id_rel + entity_identifier.len(),
        ),
        source_frames,
        data_reference,
        data_inline_value,
    })
}

fn parse_source_frames_spanned(
    tail: &str,
    line_start: usize,
    line: &str,
) -> Vec<EventModelingFieldSpan> {
    let mut out = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = tail[search_from..].find("->>") {
        let arrow = search_from + rel;
        let after = tail[arrow + 3..].trim_start();
        let Some(name) = after.split_whitespace().next() else {
            break;
        };
        if name.is_empty() {
            break;
        }
        if let Some(line_rel) = line.find(name) {
            out.push(EventModelingFieldSpan {
                text: name.to_string(),
                span: SourceSpan::new(line_start + line_rel, line_start + line_rel + name.len()),
            });
        }
        search_from = arrow + 3;
    }
    out
}

fn parse_data_reference_spanned(
    tail: &str,
    line_start: usize,
    line: &str,
) -> Option<EventModelingFieldSpan> {
    let start = tail.find("[[")?;
    let rest = &tail[start + 2..];
    let end = rest.find("]]")?;
    let text = rest[..end].trim();
    let rel = line.find(text)?;
    Some(EventModelingFieldSpan {
        text: text.to_string(),
        span: SourceSpan::new(line_start + rel, line_start + rel + text.len()),
    })
}

fn parse_inline_data_spanned(
    tail: &str,
    line_start: usize,
    line: &str,
) -> Option<EventModelingFieldSpan> {
    let start = tail.find('{')?;
    let mut depth = 0usize;
    for (offset, ch) in tail[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let text = tail[start..start + offset + 1].trim();
                    let rel = line.find(text)?;
                    return Some(EventModelingFieldSpan {
                        text: text.to_string(),
                        span: SourceSpan::new(line_start + rel, line_start + rel + text.len()),
                    });
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_data_entity_line(line: &str, line_start: usize) -> Option<EventModelingDataEntityFacts> {
    let rest = strip_keyword(line, "data")?.trim_start();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts.next()?.trim();
    let first_tail = parts.next().unwrap_or("").trim();
    if !first_tail.starts_with('{') {
        return None;
    }
    let name_rel = line.find(name)?;
    let block_start = line.find('{')?;
    let block_text = first_tail.trim().to_string();
    let block_end = line_start + block_start + block_text.len();
    Some(EventModelingDataEntityFacts {
        name: EventModelingFieldSpan {
            text: name.to_string(),
            span: SourceSpan::new(line_start + name_rel, line_start + name_rel + name.len()),
        },
        block_text,
        block_span: SourceSpan::new(line_start + block_start, block_end),
    })
}

fn strip_header<'a>(code: &'a str, meta: &ParseMetadata) -> Result<&'a str> {
    let trimmed = code.trim_start();
    let Some(rest) = trimmed.strip_prefix("eventmodeling") else {
        return Err(parse_error(meta, "expected eventmodeling"));
    };
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(parse_error(meta, "expected eventmodeling"));
    }
    Ok(rest)
}

fn parse_frame_line(
    line: &str,
    meta: &ParseMetadata,
) -> Result<Option<EventModelingFrameRenderModel>> {
    let (frame_kind, rest) = if let Some(rest) = strip_keyword(line, "tf") {
        ("timeframe", rest)
    } else if let Some(rest) = strip_keyword(line, "timeframe") {
        ("timeframe", rest)
    } else if let Some(rest) = strip_keyword(line, "rf") {
        ("resetframe", rest)
    } else if let Some(rest) = strip_keyword(line, "resetframe") {
        ("resetframe", rest)
    } else {
        return Ok(None);
    };

    let mut parts = rest.trim_start().splitn(4, char::is_whitespace);
    let name = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| parse_error(meta, "expected eventmodeling frame name"))?;
    let model_entity_type = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| parse_error(meta, "expected eventmodeling entity type"))?;
    let entity_identifier = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| parse_error(meta, "expected eventmodeling entity identifier"))?;
    let tail = parts.next().unwrap_or("").trim();

    let source_frames = parse_source_frames(tail);
    let data_reference = parse_data_reference(tail);
    let data_inline_value =
        parse_inline_data(tail).map(|s| sanitize_text(&s, &meta.effective_config));

    Ok(Some(EventModelingFrameRenderModel {
        name: name.to_string(),
        frame_kind: frame_kind.to_string(),
        model_entity_type: model_entity_type.to_string(),
        entity_identifier: sanitize_text(entity_identifier, &meta.effective_config),
        source_frames,
        data_inline_value,
        data_reference,
    }))
}

fn parse_source_frames(tail: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut iter = tail.split_whitespace();
    while let Some(token) = iter.next() {
        if token == "->>"
            && let Some(name) = iter.next()
        {
            out.push(trim_trailing_syntax(name).to_string());
        }
    }
    out
}

fn parse_data_reference(tail: &str) -> Option<String> {
    let start = tail.find("[[")?;
    let rest = &tail[start + 2..];
    let end = rest.find("]]")?;
    Some(rest[..end].trim().to_string())
}

fn parse_inline_data(tail: &str) -> Option<String> {
    let start = tail.find('{')?;
    let mut depth = 0usize;
    for (offset, ch) in tail[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(tail[start..start + offset + 1].trim().to_string());
                }
            }
            _ => {}
        }
    }
    Some(tail[start..].trim().to_string())
}

fn parse_data_entity<'a, I>(
    line: &str,
    lines: &mut std::iter::Peekable<I>,
    meta: &ParseMetadata,
) -> Result<EventModelingDataEntityRenderModel>
where
    I: Iterator<Item = &'a str>,
{
    let rest = strip_keyword(line, "data")
        .ok_or_else(|| parse_error(meta, "expected eventmodeling data block"))?
        .trim_start();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let name = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| parse_error(meta, "expected eventmodeling data name"))?;
    let first_tail = parts.next().unwrap_or("");

    let mut block = String::new();
    if !first_tail.trim().is_empty() {
        block.push_str(first_tail.trim());
    }

    let mut depth = brace_delta(&block);
    while depth <= 0 || !block.contains('{') {
        let Some(next) = lines.peek().copied() else {
            break;
        };
        if !block.is_empty() {
            block.push('\n');
        }
        block.push_str(next);
        depth += brace_delta(next);
        lines.next();
        if block.contains('{') && depth <= 0 {
            break;
        }
    }

    while depth > 0 {
        let Some(next) = lines.next() else {
            break;
        };
        if !block.is_empty() {
            block.push('\n');
        }
        block.push_str(next);
        depth += brace_delta(next);
    }

    Ok(EventModelingDataEntityRenderModel {
        name: name.to_string(),
        data_block_value: sanitize_text(block.trim(), &meta.effective_config),
    })
}

fn brace_delta(text: &str) -> isize {
    text.chars().fold(0, |acc, ch| match ch {
        '{' => acc + 1,
        '}' => acc - 1,
        _ => acc,
    })
}

fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    rest.chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace())
        .then_some(rest)
}

fn starts_with_keyword(line: &str, keyword: &str) -> bool {
    strip_keyword(line, keyword).is_some()
}

fn trim_trailing_syntax(value: &str) -> &str {
    value.trim_matches(|ch: char| ch == ',' || ch == ';')
}

fn parse_error(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::DiagramParse {
        diagram_type: meta.diagram_type.clone(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, MermaidConfig, ParseMetadata, ParseOptions};

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "eventmodeling".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn parses_simple_model_with_full_syntax() {
        let model = parse_eventmodeling_model_for_render(
            "eventmodeling\ntimeframe 01 event Start\n",
            &meta(),
        )
        .unwrap();

        assert_eq!(model.frames.len(), 1);
        let frame = &model.frames[0];
        assert_eq!(frame.name, "01");
        assert_eq!(frame.model_entity_type, "event");
        assert_eq!(frame.entity_identifier, "Start");
    }

    #[test]
    fn parses_reset_frames_qualified_names_and_sources() {
        let model = parse_eventmodeling_model_for_render(
            r#"eventmodeling
tf 02 ui UI
resetframe 01 evt Product.PriceChanged
tf 03 evt Cart.ItemAdded ->> 01 ->> 02
"#,
            &meta(),
        )
        .unwrap();

        assert_eq!(model.frames.len(), 3);
        assert_eq!(model.frames[1].frame_kind, "resetframe");
        assert_eq!(model.frames[1].entity_identifier, "Product.PriceChanged");
        assert_eq!(model.frames[2].source_frames, ["01", "02"]);
    }

    #[test]
    fn captures_inline_data_and_data_blocks() {
        let model = parse_eventmodeling_model_for_render(
            r#"eventmodeling
tf 01 cmd AddItem { productId: 7 }
tf 02 evt ItemAdded [[ItemAddedData]]

data ItemAddedData {
  productId: 7
}
"#,
            &meta(),
        )
        .unwrap();

        assert_eq!(
            model.frames[0].data_inline_value.as_deref(),
            Some("{ productId: 7 }")
        );
        assert_eq!(
            model.frames[1].data_reference.as_deref(),
            Some("ItemAddedData")
        );
        assert_eq!(model.data_entities.len(), 1);
        assert!(
            model.data_entities[0]
                .data_block_value
                .contains("productId")
        );
    }

    #[test]
    fn parse_eventmodeling_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let text = r#"eventmodeling
tf 01 cmd AddItem { productId: 7 }
tf 02 evt ItemAdded [[ItemAddedData]] ->> 01

data ItemAddedData {
  productId: 7
}
"#;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(
                "eventmodeling",
                text,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "01"));
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "ItemAddedData")
        );
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "ItemAdded")
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "AddItem"));

        let frame_start = text.find("01").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(frame_start, frame_start + "01".len())
        }));
    }
}
