//! Shared resource profiles, admission policy, and pure model complexity accounting.

mod data;
mod input;
#[cfg(feature = "diagram-block")]
pub use crate::families::block::model_complexity_from_block;
#[cfg(all(feature = "diagram-kanban", feature = "presentation-data"))]
pub use crate::families::kanban::model_complexity_from_kanban;
#[cfg(feature = "diagram-kanban")]
pub use crate::families::kanban::model_complexity_from_kanban_semantic;
#[cfg(feature = "diagram-treemap")]
pub use crate::families::treemap::TreemapComplexity;
#[cfg(feature = "diagram-ishikawa")]
pub use data::IshikawaComplexity;
#[cfg(feature = "diagram-radar")]
pub use data::model_complexity_from_radar;
pub use data::model_complexity_from_semantic_model;
pub use input::*;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow",
    feature = "diagram-class",
    feature = "diagram-zenuml",
    feature = "diagram-mindmap"
))]
mod family;
#[cfg(feature = "diagram-sequence")]
pub use crate::families::sequence::SequenceComplexity;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub use family::FlowchartComplexity;
#[cfg(feature = "diagram-mindmap")]
pub use family::MindmapComplexity;
#[cfg(feature = "diagram-zenuml")]
pub use family::ZenumlComplexity;
#[cfg(feature = "diagram-class")]
pub use family::{ClassComplexity, class_namespace_depth};

#[cfg(feature = "diagram-requirement")]
use crate::families::requirement::RequirementDiagramModel;
use serde::Serialize;
use serde::ser::{
    SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};

pub const RESOURCE_PROFILE_COUNT: usize = 4;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceProfile {
    Interactive,
    Constrained,
    TrustedNative,
    UnboundedForTrustedInput,
}

impl ResourceProfile {
    pub const ALL: [Self; RESOURCE_PROFILE_COUNT] = [
        Self::Interactive,
        Self::Constrained,
        Self::TrustedNative,
        Self::UnboundedForTrustedInput,
    ];

    pub const fn descriptor(self) -> &'static ResourceProfileDescriptor {
        &RESOURCE_PROFILE_DESCRIPTORS[self as usize]
    }

    pub const fn id(self) -> &'static str {
        self.descriptor().id
    }

    pub fn from_id(id: &str) -> Option<Self> {
        RESOURCE_PROFILE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.id == id)
            .map(|descriptor| descriptor.profile)
    }

    /// Returns whether this profile is no looser than a host-owned ceiling.
    ///
    /// Numeric overrides cover only the resource IDs exposed by a concrete artifact. This
    /// relation also protects resource owners that are not part of that generic vocabulary, such
    /// as theme compilation.
    pub const fn is_no_looser_than(self, ceiling: Self) -> bool {
        match ceiling {
            Self::Constrained => matches!(self, Self::Constrained),
            Self::Interactive => matches!(self, Self::Constrained | Self::Interactive),
            Self::TrustedNative => !matches!(self, Self::UnboundedForTrustedInput),
            Self::UnboundedForTrustedInput => true,
        }
    }
}

impl std::fmt::Display for ResourceProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

impl std::str::FromStr for ResourceProfile {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::from_id(value).ok_or_else(|| {
            let supported = RESOURCE_PROFILE_DESCRIPTORS
                .iter()
                .map(|descriptor| descriptor.id)
                .collect::<Vec<_>>()
                .join(", ");
            format!("unsupported resource profile `{value}`; expected one of: {supported}")
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ResourceProfileDescriptor {
    pub profile: ResourceProfile,
    pub id: &'static str,
    pub purpose: &'static str,
    pub trust_assumption: &'static str,
    pub recommended_binding_default: bool,
}

pub static RESOURCE_PROFILE_DESCRIPTORS: [ResourceProfileDescriptor; RESOURCE_PROFILE_COUNT] = [
    ResourceProfileDescriptor {
        profile: ResourceProfile::Interactive,
        id: "interactive",
        purpose: "General interactive applications and public binding surfaces",
        trust_assumption: "Cooperative user-authored input; not a hostile or multi-tenant isolation boundary",
        recommended_binding_default: true,
    },
    ResourceProfileDescriptor {
        profile: ResourceProfile::Constrained,
        id: "constrained",
        purpose: "Constrained rendering for untrusted or publicly submitted documents",
        trust_assumption: "The host must provide timeout, memory, concurrency, and preemption controls",
        recommended_binding_default: false,
    },
    ResourceProfileDescriptor {
        profile: ResourceProfile::TrustedNative,
        id: "trusted-native",
        purpose: "Local CLI and controlled native batch rendering",
        trust_assumption: "Input is trusted and the native host controls the workload",
        recommended_binding_default: false,
    },
    ResourceProfileDescriptor {
        profile: ResourceProfile::UnboundedForTrustedInput,
        id: "unbounded-for-trusted-input",
        purpose: "Explicitly disable policy budgets while retaining hard backend capabilities",
        trust_assumption: "Input is fully trusted and the host provides outer isolation",
        recommended_binding_default: false,
    },
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModelComplexity {
    pub items: usize,
    pub text_bytes: usize,
    pub nesting_depth: usize,
}

impl ModelComplexity {
    pub const fn new(items: usize, text_bytes: usize, nesting_depth: usize) -> Self {
        Self {
            items,
            text_bytes,
            nesting_depth,
        }
    }

    /// Counts the stable serialized model shape used by JSON-returning transport operations.
    ///
    /// A model whose serializer refuses the generic depth boundary receives saturated complexity
    /// so resource policy rejects it without panicking. Use [`Self::from_json`] for an exact count
    /// of a managed JSON value at arbitrary depth.
    pub fn from_serializable<T: Serialize + ?Sized>(model: &T) -> Self {
        let mut counter = ModelComplexitySerializer::default();
        if model.serialize(&mut counter).is_err() {
            return Self::new(usize::MAX, usize::MAX, usize::MAX);
        }
        counter.finish()
    }

    /// Counts JSON's serialized shape without invoking a recursive serializer.
    pub fn from_json(model: &serde_json::Value) -> Self {
        let complexity = json_value_complexity(model, 0);
        Self::new(
            complexity.items.max(1),
            complexity.text_bytes,
            complexity.container_depth,
        )
    }

    #[cfg(feature = "diagram-requirement")]
    pub fn from_requirement(model: &RequirementDiagramModel) -> Self {
        let common_text_bytes = [
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            Some(model.direction.as_str()),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let requirement_text_bytes = model.requirements.iter().fold(0usize, |total, node| {
            let total = [
                node.name.as_str(),
                node.node_type.as_str(),
                node.requirement_id.as_str(),
                node.text.as_str(),
                node.risk.as_str(),
                node.verify_method.as_str(),
            ]
            .into_iter()
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
            node.css_styles
                .iter()
                .chain(&node.classes)
                .fold(total, |subtotal, value| {
                    subtotal.saturating_add(value.len())
                })
        });
        let element_text_bytes = model.elements.iter().fold(0usize, |total, node| {
            let total = [
                node.name.as_str(),
                node.element_type.as_str(),
                node.doc_ref.as_str(),
            ]
            .into_iter()
            .fold(total, |subtotal, value| {
                subtotal.saturating_add(value.len())
            });
            node.css_styles
                .iter()
                .chain(&node.classes)
                .fold(total, |subtotal, value| {
                    subtotal.saturating_add(value.len())
                })
        });
        let relationship_text_bytes =
            model
                .relationships
                .iter()
                .fold(0usize, |total, relationship| {
                    [
                        relationship.rel_type.as_str(),
                        relationship.src.as_str(),
                        relationship.dst.as_str(),
                    ]
                    .into_iter()
                    .fold(total, |subtotal, value| {
                        subtotal.saturating_add(value.len())
                    })
                });
        let class_text_bytes = model.classes.iter().fold(0usize, |total, (key, class)| {
            class.styles.iter().chain(&class.text_styles).fold(
                total
                    .saturating_add(key.len())
                    .saturating_add(class.id.len()),
                |subtotal, value| subtotal.saturating_add(value.len()),
            )
        });
        Self::new(
            model
                .requirements
                .len()
                .saturating_add(model.elements.len())
                .saturating_add(model.relationships.len())
                .saturating_add(model.classes.len()),
            common_text_bytes
                .saturating_add(requirement_text_bytes)
                .saturating_add(element_text_bytes)
                .saturating_add(relationship_text_bytes)
                .saturating_add(class_text_bytes),
            0,
        )
    }
}

#[derive(Debug, Default)]
struct ModelComplexitySerializer {
    items: usize,
    text_bytes: usize,
    current_depth: usize,
    max_depth: usize,
}

impl ModelComplexitySerializer {
    fn enter(&mut self) {
        self.current_depth = self.current_depth.saturating_add(1);
        self.max_depth = self.max_depth.max(self.current_depth);
    }

    fn leave(&mut self) {
        self.current_depth = self.current_depth.saturating_sub(1);
    }

    fn finish(self) -> ModelComplexity {
        ModelComplexity::new(
            self.items.max(1),
            self.text_bytes,
            self.max_depth.saturating_sub(1),
        )
    }
}

#[derive(Debug)]
struct ModelComplexitySerializationError;

impl std::fmt::Display for ModelComplexitySerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("model complexity serialization failed")
    }
}

impl std::error::Error for ModelComplexitySerializationError {}

impl serde::ser::Error for ModelComplexitySerializationError {
    fn custom<T: std::fmt::Display>(_message: T) -> Self {
        Self
    }
}

struct ModelCompound<'a> {
    counter: &'a mut ModelComplexitySerializer,
    count_items: bool,
}

impl<'a> ModelCompound<'a> {
    fn new(counter: &'a mut ModelComplexitySerializer, count_items: bool) -> Self {
        counter.enter();
        Self {
            counter,
            count_items,
        }
    }

    fn serialize_value<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), ModelComplexitySerializationError> {
        if self.count_items {
            self.counter.items = self.counter.items.saturating_add(1);
        }
        value.serialize(&mut *self.counter)
    }
}

impl Drop for ModelCompound<'_> {
    fn drop(&mut self) {
        self.counter.leave();
    }
}

impl SerializeSeq for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeTuple for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeTupleStruct for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeTupleVariant for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeMap for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<Self::Ok, Self::Error> {
        self.serialize_value(key)
    }

    fn serialize_value<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(&mut *self.counter)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeStruct for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(&mut *self.counter)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl SerializeStructVariant for ModelCompound<'_> {
    type Ok = ();
    type Error = ModelComplexitySerializationError;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(&mut *self.counter)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }
}

impl<'a> serde::Serializer for &'a mut ModelComplexitySerializer {
    type Ok = ();
    type Error = ModelComplexitySerializationError;
    type SerializeSeq = ModelCompound<'a>;
    type SerializeTuple = ModelCompound<'a>;
    type SerializeTupleStruct = ModelCompound<'a>;
    type SerializeTupleVariant = ModelCompound<'a>;
    type SerializeMap = ModelCompound<'a>;
    type SerializeStruct = ModelCompound<'a>;
    type SerializeStructVariant = ModelCompound<'a>;

    fn serialize_bool(self, _value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i8(self, _value: i8) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i16(self, _value: i16) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i32(self, _value: i32) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i64(self, _value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_i128(self, _value: i128) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u8(self, _value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u16(self, _value: u16) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u32(self, _value: u32) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u64(self, _value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_u128(self, _value: u128) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_f32(self, _value: f32) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_f64(self, _value: f64) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        self.text_bytes = self.text_bytes.saturating_add(value.len_utf8());
        Ok(())
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        self.text_bytes = self.text_bytes.saturating_add(value.len());
        Ok(())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.items = self.items.saturating_add(value.len());
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_seq(self, _length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ModelCompound::new(self, true))
    }

    fn serialize_tuple(self, _length: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(ModelCompound::new(self, true))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(ModelCompound::new(self, true))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(ModelCompound::new(self, true))
    }

    fn serialize_map(self, _length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(ModelCompound::new(self, true))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(ModelCompound::new(self, false))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _length: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(ModelCompound::new(self, false))
    }

    fn collect_str<T: std::fmt::Display + ?Sized>(
        self,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        self.text_bytes = self.text_bytes.saturating_add(value.to_string().len());
        Ok(())
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

/// Raw counts for an arbitrary JSON subtree, before model-level minimum cardinality.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JsonValueComplexity {
    pub items: usize,
    pub text_bytes: usize,
    pub nesting_depth: usize,
    pub container_depth: usize,
}

impl JsonValueComplexity {
    pub fn merge(&mut self, other: Self) {
        self.items = self.items.saturating_add(other.items);
        self.text_bytes = self.text_bytes.saturating_add(other.text_bytes);
        self.nesting_depth = self.nesting_depth.max(other.nesting_depth);
        self.container_depth = self.container_depth.max(other.container_depth);
    }
}

enum JsonTraversalFrame<'a> {
    Value(&'a serde_json::Value, usize),
    Array(std::slice::Iter<'a, serde_json::Value>, usize),
    Object(serde_json::map::Iter<'a>, usize),
}

/// Counts an arbitrary JSON subtree without recursive calls or width-proportional scratch space.
pub fn json_value_complexity(
    value: &serde_json::Value,
    initial_depth: usize,
) -> JsonValueComplexity {
    let mut complexity = JsonValueComplexity::default();
    let mut pending = vec![JsonTraversalFrame::Value(value, initial_depth)];
    while let Some(frame) = pending.pop() {
        match frame {
            JsonTraversalFrame::Value(value, depth) => {
                complexity.nesting_depth = complexity.nesting_depth.max(depth);
                match value {
                    serde_json::Value::String(value) => {
                        complexity.text_bytes = complexity.text_bytes.saturating_add(value.len());
                    }
                    serde_json::Value::Array(values) => {
                        complexity.container_depth = complexity.container_depth.max(depth);
                        complexity.items = complexity.items.saturating_add(values.len());
                        pending.push(JsonTraversalFrame::Array(
                            values.iter(),
                            depth.saturating_add(1),
                        ));
                    }
                    serde_json::Value::Object(values) => {
                        complexity.container_depth = complexity.container_depth.max(depth);
                        complexity.items = complexity.items.saturating_add(values.len());
                        pending.push(JsonTraversalFrame::Object(
                            values.iter(),
                            depth.saturating_add(1),
                        ));
                    }
                    serde_json::Value::Null
                    | serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_) => {}
                }
            }
            JsonTraversalFrame::Array(mut values, depth) => {
                if let Some(value) = values.next() {
                    pending.push(JsonTraversalFrame::Array(values, depth));
                    pending.push(JsonTraversalFrame::Value(value, depth));
                }
            }
            JsonTraversalFrame::Object(mut values, depth) => {
                if let Some((key, value)) = values.next() {
                    complexity.text_bytes = complexity.text_bytes.saturating_add(key.len());
                    pending.push(JsonTraversalFrame::Object(values, depth));
                    pending.push(JsonTraversalFrame::Value(value, depth));
                }
            }
        }
    }
    complexity
}
