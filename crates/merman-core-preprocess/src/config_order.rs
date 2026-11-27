//! Bounded source construct replay for editor order evidence.

use crate::MappedSource;
use crate::text::{PreprocessMarkers, cleanup_text_controlled};
use merman_model::json::ordered::{JsonOrderScope, OrderedJson};
use merman_model::{OperationControl, OperationControlResult};

pub const MAX_CONFIG_REWRITE_DEPTH: usize = 64;
pub const JSON_ESCAPE_EXPANSION: usize = 6;

pub(crate) fn ordered_yaml_json(json: &str) -> Option<OrderedJson> {
    let mut parser = serde_json::Deserializer::from_str(json);
    OrderedJson::deserialize_with_scope(&mut parser, EveryMember).ok()
}

pub(crate) fn clean_construct(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<MappedSource> {
    let mut source = MappedSource::new_controlled(text, false, control)?;
    let markers = PreprocessMarkers::scan(text, control)?;
    cleanup_text_controlled(&mut source, markers, control)?;
    Ok(source)
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

#[cfg(feature = "directive-config")]
pub(crate) struct AllOrderedJson(pub(crate) OrderedJson);

#[cfg(feature = "directive-config")]
impl<'de> serde::Deserialize<'de> for AllOrderedJson {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        OrderedJson::deserialize_with_scope(deserializer, EveryMember).map(Self)
    }
}
