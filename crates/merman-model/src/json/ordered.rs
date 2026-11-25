use serde::de::{DeserializeSeed, IntoDeserializer, MapAccess, SeqAccess, Visitor, value};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, fmt};

/// A JSON value with member order tracked for caller-selected subtrees.
#[derive(Debug, Clone)]
pub struct OrderedJson {
    /// Use the ordered mutation methods when changing tracked object keys or array structure.
    pub value: Value,
    order: MemberOrder,
}

#[derive(Debug, Clone, Default)]
enum MemberOrder {
    #[default]
    Untracked,
    Object(Box<ObjectOrder>),
    Array(Box<[(usize, MemberOrder)]>),
}

#[derive(Debug, Clone, Default)]
struct ObjectOrder {
    keys: Vec<String>,
    children: BTreeMap<String, MemberOrder>,
}

impl ObjectOrder {
    fn set_child(&mut self, key: &str, order: MemberOrder) {
        match order {
            MemberOrder::Untracked => {
                self.children.remove(key);
            }
            order => {
                self.children.insert(key.to_owned(), order);
            }
        }
    }
}

impl From<Value> for OrderedJson {
    fn from(value: Value) -> Self {
        Self {
            value,
            order: MemberOrder::Untracked,
        }
    }
}

/// Selects the child subtrees whose member order is tracked during deserialization.
/// A child without a scope is deserialized directly as a JSON value.
pub trait JsonOrderScope: Copy {
    fn member(self, key: &str) -> Option<Self>;
    fn array_item(self) -> Option<Self>;
}

#[derive(Clone, Copy)]
struct OrderSeed<S>(Option<S>);

impl<'de, S: JsonOrderScope> DeserializeSeed<'de> for OrderSeed<S> {
    type Value = OrderedJson;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        match self.0 {
            Some(scope) => deserializer.deserialize_any(OrderVisitor(scope)),
            None => Value::deserialize(deserializer).map(OrderedJson::from),
        }
    }
}

struct OrderVisitor<S>(S);

impl<'de, S: JsonOrderScope> Visitor<'de> for OrderVisitor<S> {
    type Value = OrderedJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("any valid JSON value")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = Map::new();
        let mut order = ObjectOrder::default();
        while let Some(key) = map.next_key::<String>()? {
            let member = map.next_value_seed(OrderSeed(self.0.member(&key)))?;
            // Exact duplicates keep the first slot and the last value, as in the JSON map.
            if !values.contains_key(&key) {
                order.keys.push(key.clone());
            }
            order.set_child(&key, member.order);
            values.insert(key, member.value);
        }
        let order = if order.keys.is_empty() {
            MemberOrder::Untracked
        } else {
            MemberOrder::Object(Box::new(order))
        };
        Ok(OrderedJson {
            value: Value::Object(values),
            order,
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut values = Vec::new();
        let mut order = Vec::new();
        let item = OrderSeed(self.0.array_item());
        while let Some(item) = seq.next_element_seed(item)? {
            if !matches!(item.order, MemberOrder::Untracked) {
                order.push((values.len(), item.order));
            }
            values.push(item.value);
        }
        let order = if order.is_empty() {
            MemberOrder::Untracked
        } else {
            MemberOrder::Array(order.into_boxed_slice())
        };
        Ok(OrderedJson {
            value: Value::Array(values),
            order,
        })
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(Value::Null.into())
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::from(value).into())
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::from(value).into())
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::from(value).into())
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
        Ok(Value::from(value).into())
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
        Ok(Value::from(value).into())
    }
}

impl OrderedJson {
    /// Deserializes through the caller's deserializer with selective member-order tracking.
    pub fn deserialize_with_scope<'de, D: Deserializer<'de>>(
        deserializer: D,
        scope: impl JsonOrderScope,
    ) -> Result<Self, D::Error> {
        OrderSeed(Some(scope)).deserialize(deserializer)
    }

    pub fn view(&self) -> OrderedJsonRef<'_> {
        OrderedJsonRef {
            value: &self.value,
            order: &self.order,
        }
    }

    pub fn object_mut(&mut self) -> Option<OrderedJsonObjectMut<'_>> {
        Some(OrderedJsonObjectMut {
            map: self.value.as_object_mut()?,
            order: &mut self.order,
        })
    }

    pub fn merge(&mut self, request: Self) {
        merge(&mut self.value, &mut self.order, request);
    }
}

/// Mutates an object together with its tracked member order.
pub struct OrderedJsonObjectMut<'a> {
    map: &'a mut Map<String, Value>,
    order: &'a mut MemberOrder,
}

impl OrderedJsonObjectMut<'_> {
    pub fn child_object(&mut self, key: &str) -> Option<OrderedJsonObjectMut<'_>> {
        let order = object_order(self.map, self.order);
        let order = order.children.entry(key.to_owned()).or_default();
        let map = self.map.get_mut(key)?.as_object_mut()?;
        Some(OrderedJsonObjectMut { map, order })
    }

    /// Removes a member and moves the last tracked key into its slot.
    pub fn swap_remove(&mut self, key: &str) -> Option<OrderedJson> {
        let value = self.map.remove(key)?;
        let mut order = MemberOrder::Untracked;
        if let MemberOrder::Object(object) = self.order {
            let index = object
                .keys
                .iter()
                .position(|member| member == key)
                .expect("tracked JSON key");
            object.keys.swap_remove(index);
            order = object.children.remove(key).unwrap_or_default();
        }
        Some(OrderedJson { value, order })
    }

    pub fn insert(&mut self, key: String, member: OrderedJson) {
        let novel = !self.map.contains_key(&key);
        let order = object_order(self.map, self.order);
        if novel {
            order.keys.push(key.clone());
        }
        order.set_child(&key, member.order);
        self.map.insert(key, member.value);
    }

    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.map.retain(|key, _| keep(key));
        if let MemberOrder::Object(order) = self.order {
            order.keys.retain(|key| keep(key));
            order.children.retain(|key, _| keep(key));
        }
    }

    fn merge_member(&mut self, key: String, request: OrderedJson) {
        if let Some(value) = self.map.get_mut(&key) {
            if let MemberOrder::Object(object) = self.order {
                let mut order = object.children.remove(&key).unwrap_or_default();
                merge(value, &mut order, request);
                object.set_child(&key, order);
            } else {
                merge(value, &mut MemberOrder::Untracked, request);
            }
        } else if matches!(self.order, MemberOrder::Object(_)) {
            self.insert(key, request);
        } else {
            self.map.insert(key, request.value);
        }
    }
}

fn object_order<'a>(map: &Map<String, Value>, order: &'a mut MemberOrder) -> &'a mut ObjectOrder {
    if !matches!(order, MemberOrder::Object(_)) {
        *order = MemberOrder::Object(Box::new(ObjectOrder {
            keys: map.keys().cloned().collect(),
            children: BTreeMap::new(),
        }));
    }
    let MemberOrder::Object(order) = order else {
        unreachable!();
    };
    order
}

fn merge(base: &mut Value, order: &mut MemberOrder, request: OrderedJson) {
    match (base, request.value) {
        (Value::Object(base), Value::Object(mut request_map)) => {
            let mut base = OrderedJsonObjectMut { map: base, order };
            if let MemberOrder::Object(request_order) = request.order {
                object_order(base.map, base.order);
                let ObjectOrder { keys, mut children } = *request_order;
                for key in keys {
                    let value = request_map.remove(&key).expect("tracked JSON key");
                    let order = children.remove(&key).unwrap_or_default();
                    base.merge_member(key, OrderedJson { value, order });
                }
            } else {
                for (key, value) in request_map {
                    base.merge_member(key, value.into());
                }
            }
        }
        (base, request_value) => {
            *base = request_value;
            *order = request.order;
        }
    }
}

/// A borrowed JSON value and its tracked member order.
#[derive(Clone, Copy)]
pub struct OrderedJsonRef<'a> {
    pub value: &'a Value,
    order: &'a MemberOrder,
}

impl<'a> From<&'a Value> for OrderedJsonRef<'a> {
    fn from(value: &'a Value) -> Self {
        Self {
            value,
            order: &MemberOrder::Untracked,
        }
    }
}

impl<'a> OrderedJsonRef<'a> {
    /// Clones this subtree with its tracked member order.
    pub fn to_ordered(self) -> OrderedJson {
        OrderedJson {
            value: self.value.clone(),
            order: self.order.clone(),
        }
    }

    pub fn get(self, key: &str) -> Option<Self> {
        let value = self.value.get(key)?;
        let order = match self.order {
            MemberOrder::Object(order) => order.children.get(key),
            _ => None,
        };
        Some(Self {
            value,
            order: order.unwrap_or(&MemberOrder::Untracked),
        })
    }

    pub fn entries(self) -> impl Iterator<Item = (&'a str, Self)> {
        let object = self.value.as_object();
        let (ordered, fallback) = match self.order {
            MemberOrder::Object(order) => (Some(order.as_ref()), None),
            _ => (None, object),
        };
        ordered
            .into_iter()
            .flat_map(|order| &order.keys)
            .map(move |key| (key.as_str(), self.get(key).expect("tracked JSON key")))
            .chain(
                fallback
                    .into_iter()
                    .flat_map(Map::iter)
                    .map(|(key, value)| (key.as_str(), Self::from(value))),
            )
    }

    pub fn items(self, array: &'a [Value]) -> impl Iterator<Item = Self> {
        let ordered_items = match self.order {
            MemberOrder::Array(order) => order.as_ref(),
            _ => &[],
        };
        let mut ordered_items = ordered_items.iter().peekable();
        array.iter().enumerate().map(move |(index, value)| Self {
            value,
            order: ordered_items
                .next_if(|(child_index, _)| *child_index == index)
                .map(|(_, order)| order)
                .unwrap_or(&MemberOrder::Untracked),
        })
    }
}

impl Serialize for OrderedJsonRef<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match (self.order, self.value) {
            (MemberOrder::Object(_), Value::Object(object)) => {
                let mut map = serializer.serialize_map(Some(object.len()))?;
                for (key, member) in self.entries() {
                    map.serialize_entry(key, &member)?;
                }
                map.end()
            }
            (MemberOrder::Array(_), Value::Array(items)) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for item in self.items(items) {
                    seq.serialize_element(&item)?;
                }
                seq.end()
            }
            _ => self.value.serialize(serializer),
        }
    }
}

impl<'de> IntoDeserializer<'de, serde_json::Error> for OrderedJsonRef<'de> {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self {
        self
    }
}

macro_rules! delegate_to_value {
    ($($method:ident),* $(,)?) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
                self.value.$method(visitor)
            }
        )*
    };
}

impl<'de> Deserializer<'de> for OrderedJsonRef<'de> {
    type Error = serde_json::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.order {
            MemberOrder::Object(_) => {
                value::MapDeserializer::new(self.entries()).deserialize_any(visitor)
            }
            MemberOrder::Array(_) => {
                let array = self.value.as_array().expect("tracked JSON array");
                let mut sequence = value::SeqDeserializer::<_, Self::Error>::new(self.items(array));
                let decoded = visitor.visit_seq(&mut sequence)?;
                if sequence.end().is_ok() {
                    Ok(decoded)
                } else {
                    Err(serde::de::Error::invalid_length(
                        array.len(),
                        &"fewer elements in array",
                    ))
                }
            }
            MemberOrder::Untracked => self.value.deserialize_any(visitor),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.value.is_null() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if matches!(self.order, MemberOrder::Object(_)) {
            self.deserialize_any(visitor)
        } else {
            self.value.deserialize_map(visitor)
        }
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if matches!(self.order, MemberOrder::Array(_)) {
            self.deserialize_any(visitor)
        } else {
            self.value.deserialize_seq(visitor)
        }
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        match self.order {
            MemberOrder::Untracked => self.value.deserialize_struct(name, fields, visitor),
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        // In particular, RawValue uses serde_json's own newtype protocol.
        self.value.deserialize_newtype_struct(name, visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.value.deserialize_enum(name, variants, visitor)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.value.deserialize_unit_struct(name, visitor)
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.value.deserialize_tuple(len, visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.value.deserialize_tuple_struct(name, len, visitor)
    }

    delegate_to_value! {
        deserialize_bool, deserialize_i8, deserialize_i16, deserialize_i32, deserialize_i64,
        deserialize_i128, deserialize_u8, deserialize_u16, deserialize_u32, deserialize_u64,
        deserialize_u128, deserialize_f32, deserialize_f64, deserialize_char, deserialize_str,
        deserialize_string, deserialize_bytes, deserialize_byte_buf, deserialize_unit,
        deserialize_identifier, deserialize_ignored_any,
    }
}
