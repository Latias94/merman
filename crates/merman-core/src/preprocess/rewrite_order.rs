use super::*;
use merman_model::json::ordered::{JsonOrderScope, OrderedJson};
use serde::Deserialize;

const MAX_REWRITE_DEPTH: usize = 64;
const MAX_REWRITE_DIRECTIVES: usize = 128;

/// Authored order hints for a frontmatter code action. The original parse owns the values.
pub struct SourceConfigRewriteOrder {
    pub frontmatter: Option<OrderedJson>,
    pub compatibility_config: OrderedJson,
    pub directives: Vec<(OrderedJson, Value)>,
}

/// Reads only the already-captured source constructs, using the same preprocessing and parsers.
pub fn source_config_rewrite_order_controlled(
    source: &str,
    frontmatter: Option<SourceSpan>,
    directives: &[SourceSpan],
    max_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<SourceConfigRewriteOrder>> {
    control.checkpoint()?;
    let max_bytes = max_bytes.min(MAX_DIRECTIVE_CONFIG_PARSE_BYTES);
    let max_nesting_depth = max_nesting_depth.min(MAX_REWRITE_DEPTH);
    if directives.len() > MAX_REWRITE_DIRECTIVES {
        return Ok(None);
    }
    let frontmatter = match frontmatter {
        Some(span) => {
            let Some(text) = source
                .get(span.start..span.end)
                .filter(|s| s.len() <= max_bytes)
            else {
                return Ok(None);
            };
            let cleaned = clean_construct(text, control)?;
            let Some(block) = split_frontmatter_block_controlled(cleaned.text(), control)? else {
                return Ok(None);
            };
            let Some(json) = crate::yaml_config::parse_yaml_ordered_document_controlled(
                &block.dedented_body,
                max_bytes,
                max_nesting_depth,
                control,
            )?
            else {
                return Ok(None);
            };
            if json.is_empty() {
                None
            } else {
                let mut parser = serde_json::Deserializer::from_str(&json);
                let Ok(ordered) = OrderedJson::deserialize_with_scope(&mut parser, EveryMember)
                else {
                    return Ok(None);
                };
                Some(ordered)
            }
        }
        None => None,
    };

    let mut compatibility_config = OrderedJson::from(Value::Object(Map::new()));
    if let Some(fields) = frontmatter.as_ref() {
        let mut config = compatibility_config
            .object_mut()
            .expect("empty config object");
        let aliases = crate::family::frontmatter_config_aliases()
            .iter()
            .map(|fact| (fact.source, fact.namespace));
        let namespaces = crate::family::frontmatter_config_namespaces()
            .iter()
            .map(|&key| (key, key));
        for (key, namespace) in aliases.chain(namespaces) {
            if let Some(value) = fields.view().get(key) {
                config.insert(namespace.to_owned(), value.to_ordered());
            }
        }
    }

    let mut remaining_bytes = max_bytes;
    let mut directive_order = Vec::with_capacity(directives.len());
    for span in directives {
        control.checkpoint()?;
        let Some(text) = source
            .get(span.start..span.end)
            .filter(|s| s.len() <= remaining_bytes)
        else {
            return Ok(None);
        };
        remaining_bytes -= text.len();
        let Some(mut order) = parse_init_order(text, max_nesting_depth, control)? else {
            return Ok(None);
        };
        if let Some(legacy) = order.view().get("config").map(|value| value.to_ordered()) {
            let mut object = order.object_mut().expect("config is an object member");
            object.retain(|key| key != "config");
            if directives.len() == 1 {
                let cleaned = clean_construct(source, control)?;
                if let Ok(fact) = crate::detect::detect_builtin_controlled(cleaned.text(), control)?
                {
                    let name = if fact.id == "flowchart-v2" {
                        "flowchart"
                    } else {
                        fact.id
                    };
                    object.insert(name.to_owned(), legacy);
                }
            }
        }
        // Keep track of the entries that can contribute order, including array items that
        // directive sanitization turns into null. The action still uses the captured values.
        let mut admitted = order.value.clone();
        sanitize_directive_controlled(&mut admitted, control)?;
        directive_order.push((order, admitted));
    }
    control.checkpoint()?;
    Ok(Some(SourceConfigRewriteOrder {
        frontmatter,
        compatibility_config,
        directives: directive_order,
    }))
}

fn clean_construct(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<PreprocessedSource> {
    let mut source = PreprocessedSource::new_controlled(text, false, control)?;
    cleanup_text_controlled(
        &mut source,
        PreprocessMarkers::scan(text, control)?,
        control,
    )?;
    Ok(source)
}

fn parse_init_order(
    text: &str,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<OrderedJson>> {
    let cleaned = clean_construct(text, control)?;
    let Some(raw) = cleaned
        .text()
        .strip_prefix("%%{")
        .and_then(|s| s.strip_suffix("}%%"))
    else {
        return Ok(None);
    };
    let normalized = normalize_directive_quotes_controlled(raw, control)?;
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let Some((_, argument)) = normalized.split_once(':') else {
        return Ok(Some(Value::Null.into()));
    };
    let (start, end) = trim_whitespace_bounds_controlled(argument, &mut checkpoints)?;
    let argument = &argument[start..end];
    if argument.is_empty() {
        return Ok(Some(Value::Null.into()));
    }
    if max_flow_collection_depth_controlled::<true>(argument, &mut checkpoints)? > max_nesting_depth
    {
        return Ok(None);
    }
    checkpoints.finish()?;
    let order = json5::from_str::<AllOrderedJson>(argument)
        .ok()
        .map(|value| value.0);
    control.checkpoint()?;
    Ok(order)
}

#[derive(Clone, Copy)]
struct EveryMember;

impl JsonOrderScope for EveryMember {
    fn member(self, _key: &str) -> Option<Self> {
        Some(self)
    }

    fn array_item(self) -> Option<Self> {
        Some(self)
    }
}

struct AllOrderedJson(OrderedJson);

impl<'de> Deserialize<'de> for AllOrderedJson {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        OrderedJson::deserialize_with_scope(deserializer, EveryMember).map(Self)
    }
}
