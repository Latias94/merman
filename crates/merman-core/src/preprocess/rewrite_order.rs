use super::config_policy::sanitize_directive_controlled;
use crate::{MermaidConfig, OperationControl, OperationControlResult, SourceSpan};
use merman_core_preprocess::JSON_ESCAPE_EXPANSION;
pub(super) use merman_core_preprocess::MAX_CONFIG_REWRITE_DEPTH as MAX_REWRITE_DEPTH;
use merman_core_preprocess::directives::{
    MAX_DIRECTIVE_CONFIG_PARSE_BYTES, read_ordered_init_directive_controlled,
};
use merman_core_preprocess::frontmatter::{
    frontmatter_config_routes, parse_frontmatter_insert_order_json,
    read_ordered_frontmatter_controlled,
};
use merman_core_preprocess::yaml::YamlInsertOrder;
use merman_model::config::values_equal_controlled;
use merman_model::json::clone_value_nonrecursive_controlled;
use merman_model::json::ordered::OrderedJson;
use serde_json::{Map, Value};
use std::sync::Arc;

const MAX_REWRITE_DIRECTIVES: usize = 128;

#[derive(Debug, Clone, PartialEq)]
pub struct FrontmatterInsertOrder {
    compatibility: YamlInsertOrder,
    display_mode: Option<Arc<str>>,
}

impl FrontmatterInsertOrder {
    pub(super) fn from_yaml_capture(
        compatibility: YamlInsertOrder,
        display_mode: Option<&str>,
    ) -> Option<Self> {
        if display_mode.is_some_and(|mode| mode.len() > MAX_DIRECTIVE_CONFIG_PARSE_BYTES) {
            return None;
        }
        Some(Self {
            compatibility,
            display_mode: display_mode.map(Arc::from),
        })
    }

    pub fn estimated_owned_heap_bytes(&self) -> usize {
        let size = |text: &str| text.len().saturating_add(2 * std::mem::size_of::<usize>());
        let compatibility = match &self.compatibility {
            YamlInsertOrder::Captured(json) => size(json),
            YamlInsertOrder::NoRoutes | YamlInsertOrder::ExceededBudget => 0,
        };
        compatibility.saturating_add(self.display_mode.as_deref().map_or(0, size))
    }

    fn compatible_order_controlled(
        &self,
        config: &Value,
        max_bytes: usize,
        max_nesting_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<OrderedJson>> {
        let display_mode = self.display_mode.as_deref();
        if display_mode.is_some_and(|mode| {
            config.pointer("/gantt/displayMode").and_then(Value::as_str) != Some(mode)
        }) {
            return Ok(None);
        }

        let json = match &self.compatibility {
            YamlInsertOrder::NoRoutes => return Ok(Some(empty_order())),
            YamlInsertOrder::Captured(json) => json,
            YamlInsertOrder::ExceededBudget => return Ok(None),
        };
        if max_nesting_depth != MAX_REWRITE_DEPTH
            || json.len() > max_bytes.saturating_mul(JSON_ESCAPE_EXPANSION)
            || !supports_compatibility_replay(config)
        {
            return Ok(None);
        }
        let Some(order) = parse_frontmatter_insert_order_json(json) else {
            return Ok(None);
        };
        let Some(compatibility) = clone_value_nonrecursive_controlled(
            &order.value,
            max_bytes,
            max_nesting_depth,
            control,
        )?
        else {
            return Ok(None);
        };
        let mut replay = MermaidConfig::from_value(compatibility);
        replay.deep_merge(config);
        crate::config::mirror_legacy_font_family_into_theme_variables(&mut replay);
        if let Some(mode) = display_mode {
            replay.set_value("gantt.displayMode", Value::String(mode.to_owned()));
        }
        let equal = values_equal_controlled(replay.as_value(), config, Some(control))?;
        Ok(equal.then_some(order))
    }
}

/// Reject arrays at the two levels where a merge can append values or replay array updates.
fn supports_compatibility_replay(config: &Value) -> bool {
    config.as_object().is_some_and(|root| {
        root.values().all(|value| {
            !value.is_array()
                && value
                    .as_object()
                    .is_none_or(|members| members.values().all(|value| !value.is_array()))
        })
    })
}

pub enum FrontmatterRewrite<'a> {
    Insert {
        frontmatter: Option<&'a FrontmatterInsertOrder>,
        /// Authored overrides already bounded by the caller's migration budget.
        config: &'a Value,
    },
    Replace(SourceSpan),
}

/// Authored order hints for a frontmatter code action. The original parse owns the values.
pub struct SourceConfigRewriteOrder {
    pub frontmatter: Option<OrderedJson>,
    pub compatibility_config: OrderedJson,
    pub directives: Vec<(OrderedJson, Value)>,
}

/// Reads only the already-captured source constructs, using the same preprocessing and parsers.
pub fn source_config_rewrite_order_controlled(
    source: &str,
    frontmatter: FrontmatterRewrite<'_>,
    directives: &[SourceSpan],
    legacy_init_namespace: Option<&'static str>,
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
    let (frontmatter, mut compatibility_config) = match frontmatter {
        FrontmatterRewrite::Replace(span) => {
            let Some(text) = source
                .get(span.start..span.end)
                .filter(|s| s.len() <= max_bytes)
            else {
                return Ok(None);
            };
            let Some(fields) =
                read_ordered_frontmatter_controlled(text, max_bytes, max_nesting_depth, control)?
            else {
                return Ok(None);
            };
            (fields, empty_order())
        }
        FrontmatterRewrite::Insert {
            frontmatter,
            config,
        } => {
            let order = if let Some(frontmatter) = frontmatter {
                let Some(order) = frontmatter.compatible_order_controlled(
                    config,
                    max_bytes,
                    max_nesting_depth,
                    control,
                )?
                else {
                    return Ok(None);
                };
                order
            } else {
                empty_order()
            };
            (None, order)
        }
    };

    if let Some(fields) = frontmatter.as_ref() {
        let mut config = compatibility_config
            .object_mut()
            .expect("empty config object");
        for &(key, namespace) in frontmatter_config_routes() {
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
        let Some(mut order) =
            read_ordered_init_directive_controlled(text, max_nesting_depth, control)?
        else {
            return Ok(None);
        };
        if let Some(legacy) = order.view().get("config").map(|value| value.to_ordered()) {
            let mut object = order.object_mut().expect("config is an object member");
            object.retain(|key| key != "config");
            if directives.len() == 1
                && let Some(namespace) = legacy_init_namespace
            {
                object.insert(namespace.to_owned(), legacy);
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

fn empty_order() -> OrderedJson {
    OrderedJson::from(Value::Object(Map::new()))
}
