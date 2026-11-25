use merman_core::preprocess::SourceConfigRewriteOrder;
use merman_model::json::ordered::OrderedJsonRef;
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

/// Emits the values from repair evidence in authored order. Source inputs only supply order.
pub(super) struct OrderedYaml<'a> {
    value: &'a Value,
    sources: Vec<SourceOrder<'a>>,
    merge_depth: Option<u8>,
}

impl<'a> OrderedYaml<'a> {
    pub(super) fn config(value: &'a Value, order: &'a SourceConfigRewriteOrder) -> Self {
        let sources = std::iter::once(SourceOrder::from(order.compatibility_config.view()))
            .chain(
                order
                    .frontmatter
                    .as_ref()
                    .and_then(|fields| fields.view().get("config"))
                    .map(SourceOrder::from),
            )
            .chain(
                order
                    .directives
                    .iter()
                    .map(|(authored, admitted)| SourceOrder {
                        authored: authored.view(),
                        admitted,
                    }),
            )
            .collect();
        Self {
            value,
            sources,
            merge_depth: Some(2),
        }
    }

    pub(super) fn field(value: &'a Value, source: OrderedJsonRef<'a>) -> Self {
        Self {
            value,
            sources: vec![SourceOrder::from(source)],
            merge_depth: None,
        }
    }

    fn member(&self, value: &'a Value, mut sources: Vec<SourceOrder<'a>>) -> Self {
        // Mermaid assigns whole members at its shallow merge boundary. An earlier object's
        // internal key order does not carry over to a member replaced at that boundary.
        if self.merge_depth == Some(0)
            && let Some(last) = sources.pop()
        {
            sources.clear();
            sources.push(last);
        }
        Self {
            value,
            sources,
            merge_depth: self.merge_depth.and_then(|depth| depth.checked_sub(1)),
        }
    }
}

impl Serialize for OrderedYaml<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.value {
            Value::Object(object) if !self.sources.is_empty() => {
                let mut keys = Vec::new();
                let mut members = BTreeMap::<&str, Vec<_>>::new();
                for source in &self.sources {
                    source.visit_members(&mut |key, source| {
                        if !object.contains_key(key)
                            || (self.merge_depth.is_some_and(|depth| depth > 0)
                                && source.admitted.is_null())
                        {
                            return;
                        }
                        members
                            .entry(key)
                            .or_insert_with(|| {
                                keys.push(key);
                                Vec::new()
                            })
                            .push(source);
                    });
                }

                let mut map = serializer.serialize_map(Some(object.len()))?;
                for key in keys {
                    let sources = members.get_mut(key).map(std::mem::take).unwrap_or_default();
                    map.serialize_entry(key, &self.member(&object[key], sources))?;
                }
                for (key, value) in object {
                    if !members.contains_key(key.as_str()) {
                        map.serialize_entry(key, value)?;
                    }
                }
                map.end()
            }
            Value::Array(items) if !self.sources.is_empty() => {
                // Mermaid appends compound array items, even when equal, and deduplicates only
                // primitive values. Their final positions and all values come from the admitted
                // array. Match the remaining compound items in encounter order.
                let compound = |value: &Value| value.is_object() || value.is_array();
                let sources: Vec<_> = self
                    .sources
                    .iter()
                    .flat_map(|source| source.items())
                    .filter(|source| compound(source.admitted))
                    .collect();
                let count = items.iter().filter(|item| compound(item)).count();
                let mut orders = sources
                    .iter()
                    .skip(sources.len().saturating_sub(count))
                    .copied();
                let mut unmatched = count.saturating_sub(sources.len());
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    let source = if compound(item) && unmatched == 0 {
                        orders.next()
                    } else {
                        unmatched = unmatched.saturating_sub(usize::from(compound(item)));
                        None
                    };
                    if let Some(source) = source {
                        seq.serialize_element(&Self {
                            value: item,
                            sources: vec![source],
                            merge_depth: None,
                        })?;
                    } else {
                        seq.serialize_element(item)?;
                    }
                }
                seq.end()
            }
            _ => self.value.serialize(serializer),
        }
    }
}

#[derive(Clone, Copy)]
struct SourceOrder<'a> {
    authored: OrderedJsonRef<'a>,
    admitted: &'a Value,
}

impl<'a> From<OrderedJsonRef<'a>> for SourceOrder<'a> {
    fn from(authored: OrderedJsonRef<'a>) -> Self {
        Self {
            admitted: authored.value,
            authored,
        }
    }
}

impl<'a> SourceOrder<'a> {
    fn visit_members(self, visit: &mut impl FnMut(&'a str, Self)) {
        if let Value::Object(object) = self.admitted {
            for (key, authored) in self.authored.entries() {
                if let Some(admitted) = object.get(key) {
                    visit(key, Self { authored, admitted });
                }
            }
        } else {
            // Mermaid can treat an array as successive object updates to a mapping.
            for item in self.items() {
                item.visit_members(visit);
            }
        }
    }

    fn items(self) -> impl Iterator<Item = Self> + 'a {
        self.authored
            .value
            .as_array()
            .into_iter()
            .flat_map(move |items| self.authored.items(items))
            .zip(self.admitted.as_array().into_iter().flatten())
            .map(|(authored, admitted)| Self { authored, admitted })
    }
}
