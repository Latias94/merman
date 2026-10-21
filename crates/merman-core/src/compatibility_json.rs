use crate::json_value::{
    clone_value_nonrecursive, clone_value_nonrecursive_with_control, drop_value_nonrecursive,
};
use crate::{OperationControl, OperationControlResult};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::fmt;
use std::io::Write;
use std::ops::Deref;

/// Mermaid-compatible JSON with iterative ownership and export.
///
/// Ordinary clone, equality, debug formatting, and disposal do not recurse through containers.
/// Generic serde serialization supports at most 128 nested JSON containers; use
/// [`Self::write_json`] for deeper output. Borrowed raw values and explicitly extracted unmanaged
/// values retain serde_json's own lifecycle and serialization behavior.
#[derive(Default)]
pub struct ManagedSemanticJson {
    value: Value,
}

impl ManagedSemanticJson {
    /// Takes ownership of an existing JSON value without changing its shape or object order.
    pub fn from_value(value: Value) -> Self {
        Self { value }
    }

    /// Borrows the raw payload. Cloning or serializing this borrow bypasses managed guarantees.
    pub fn as_value(&self) -> &Value {
        &self.value
    }

    pub(crate) fn as_value_mut(&mut self) -> &mut Value {
        &mut self.value
    }

    /// Transfers the payload into an unmanaged value.
    ///
    /// The caller assumes responsibility for deep clone, serialization, and disposal afterward.
    pub fn into_unmanaged_value(mut self) -> Value {
        std::mem::take(&mut self.value)
    }

    /// Clones while observing the operation control and safely disposing of partial clones.
    pub fn clone_controlled(&self, control: &OperationControl) -> OperationControlResult<Self> {
        clone_value_nonrecursive_with_control(&self.value, control).map(Self::from_value)
    }

    /// Writes compact JSON without a container-depth limit.
    pub fn write_json(&self, writer: impl Write) -> serde_json::Result<()> {
        self.write_json_controlled(writer, &OperationControl::new())
            .expect("a private operation control cannot be cancelled")
    }

    /// Writes compact JSON while observing cancellation between traversal steps.
    ///
    /// An already observed cancellation retains its phase and precedes a later writer error.
    /// Otherwise, writer errors remain distinct from cancellation requests. The caller owns any
    /// partial output.
    pub fn write_json_controlled(
        &self,
        writer: impl Write,
        control: &OperationControl,
    ) -> OperationControlResult<serde_json::Result<()>> {
        self.write_with_formatter(writer, control, serde_json::ser::CompactFormatter)
    }

    /// Writes indented JSON with serde_json's standard whitespace and no container-depth limit.
    pub fn write_json_pretty(&self, writer: impl Write) -> serde_json::Result<()> {
        self.write_json_pretty_controlled(writer, &OperationControl::new())
            .expect("a private operation control cannot be cancelled")
    }

    /// Writes indented JSON while observing the original operation control.
    pub fn write_json_pretty_controlled(
        &self,
        writer: impl Write,
        control: &OperationControl,
    ) -> OperationControlResult<serde_json::Result<()>> {
        self.write_with_formatter(writer, control, serde_json::ser::PrettyFormatter::new())
    }

    fn write_with_formatter(
        &self,
        mut writer: impl Write,
        control: &OperationControl,
        mut formatter: impl serde_json::ser::Formatter,
    ) -> OperationControlResult<serde_json::Result<()>> {
        enum Task<'a> {
            Value(&'a Value),
            Key(&'a str),
            ArrayItems(std::slice::Iter<'a, Value>, bool),
            ObjectEntries(serde_json::map::Iter<'a>, bool),
            ArrayValue(bool),
            ArrayValueEnd,
            ArrayEnd,
            ObjectKey(bool),
            ObjectValue,
            ObjectValueEnd,
            ObjectEnd,
        }

        let mut stack = vec![Task::Value(&self.value)];
        let mut steps = 0usize;
        while let Some(task) = stack.pop() {
            if steps.is_multiple_of(64) {
                control.checkpoint()?;
            }
            steps = steps.saturating_add(1);
            let result = match task {
                Task::ArrayItems(mut items, first) => {
                    if let Some(item) = items.next() {
                        stack.push(Task::ArrayItems(items, false));
                        stack.push(Task::ArrayValueEnd);
                        stack.push(Task::Value(item));
                        stack.push(Task::ArrayValue(first));
                    }
                    Ok(())
                }
                Task::ObjectEntries(mut entries, first) => {
                    if let Some((key, value)) = entries.next() {
                        stack.push(Task::ObjectEntries(entries, false));
                        stack.push(Task::ObjectValueEnd);
                        stack.push(Task::Value(value));
                        stack.push(Task::ObjectValue);
                        stack.push(Task::Key(key));
                        stack.push(Task::ObjectKey(first));
                    }
                    Ok(())
                }
                Task::ArrayValue(first) => formatter
                    .begin_array_value(&mut writer, first)
                    .map_err(serde_json::Error::io),
                Task::ArrayValueEnd => formatter
                    .end_array_value(&mut writer)
                    .map_err(serde_json::Error::io),
                Task::ArrayEnd => formatter
                    .end_array(&mut writer)
                    .map_err(serde_json::Error::io),
                Task::ObjectKey(first) => formatter
                    .begin_object_key(&mut writer, first)
                    .map_err(serde_json::Error::io),
                Task::ObjectValue => formatter
                    .begin_object_value(&mut writer)
                    .map_err(serde_json::Error::io),
                Task::ObjectValueEnd => formatter
                    .end_object_value(&mut writer)
                    .map_err(serde_json::Error::io),
                Task::ObjectEnd => formatter
                    .end_object(&mut writer)
                    .map_err(serde_json::Error::io),
                Task::Key(key) => serde_json::to_writer(&mut writer, key),
                Task::Value(Value::Array(items)) => {
                    stack.push(Task::ArrayEnd);
                    stack.push(Task::ArrayItems(items.iter(), true));
                    formatter
                        .begin_array(&mut writer)
                        .map_err(serde_json::Error::io)
                }
                Task::Value(Value::Object(entries)) => {
                    stack.push(Task::ObjectEnd);
                    stack.push(Task::ObjectEntries(entries.iter(), true));
                    formatter
                        .begin_object(&mut writer)
                        .map_err(serde_json::Error::io)
                }
                Task::Value(scalar) => serde_json::to_writer(&mut writer, scalar),
            };
            if let Err(error) = result {
                if let Some(cancelled) = control.observed_cancellation() {
                    return Err(cancelled);
                }
                return Ok(Err(error));
            }
        }
        control.checkpoint()?;
        Ok(Ok(()))
    }
}

impl From<Value> for ManagedSemanticJson {
    fn from(value: Value) -> Self {
        Self::from_value(value)
    }
}

impl From<&Value> for ManagedSemanticJson {
    fn from(value: &Value) -> Self {
        Self::from_value(clone_value_nonrecursive(value))
    }
}

impl Deref for ManagedSemanticJson {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        self.as_value()
    }
}

impl AsRef<Value> for ManagedSemanticJson {
    fn as_ref(&self) -> &Value {
        self.as_value()
    }
}

impl Clone for ManagedSemanticJson {
    fn clone(&self) -> Self {
        Self::from(&self.value)
    }
}

impl Drop for ManagedSemanticJson {
    fn drop(&mut self) {
        drop_value_nonrecursive(std::mem::take(&mut self.value));
    }
}

fn equal_values(left: &Value, right: &Value) -> bool {
    let mut stack = vec![(left, right)];
    while let Some((left, right)) = stack.pop() {
        match (left, right) {
            (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
                stack.extend(left.iter().zip(right));
            }
            (Value::Object(left), Value::Object(right)) if left.len() == right.len() => {
                for (key, value) in left {
                    let Some(other) = right.get(key) else {
                        return false;
                    };
                    stack.push((value, other));
                }
            }
            (Value::Null, Value::Null) => {}
            (Value::Bool(left), Value::Bool(right)) if left == right => {}
            (Value::Number(left), Value::Number(right)) if left == right => {}
            (Value::String(left), Value::String(right)) if left == right => {}
            _ => return false,
        }
    }
    true
}

impl PartialEq for ManagedSemanticJson {
    fn eq(&self, other: &Self) -> bool {
        equal_values(&self.value, &other.value)
    }
}

impl Eq for ManagedSemanticJson {}

impl PartialEq<Value> for ManagedSemanticJson {
    fn eq(&self, other: &Value) -> bool {
        equal_values(&self.value, other)
    }
}

impl PartialEq<ManagedSemanticJson> for Value {
    fn eq(&self, other: &ManagedSemanticJson) -> bool {
        equal_values(self, &other.value)
    }
}

impl fmt::Debug for ManagedSemanticJson {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = Vec::new();
        self.write_json(&mut bytes).map_err(|_| fmt::Error)?;
        formatter.write_str(std::str::from_utf8(&bytes).map_err(|_| fmt::Error)?)
    }
}

impl Serialize for ManagedSemanticJson {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut stack = vec![(&self.value, 0usize)];
        while let Some((value, depth)) = stack.pop() {
            if !matches!(value, Value::Array(_) | Value::Object(_)) {
                continue;
            }
            let container_depth = depth.saturating_add(1);
            if container_depth > 128 {
                return Err(serde::ser::Error::custom(
                    "semantic JSON exceeds generic serde's 128-container depth; use write_json",
                ));
            }
            match value {
                Value::Array(items) => {
                    stack.extend(
                        items
                            .iter()
                            .filter(|child| matches!(child, Value::Array(_) | Value::Object(_)))
                            .map(|child| (child, container_depth)),
                    );
                }
                Value::Object(entries) => {
                    stack.extend(
                        entries
                            .values()
                            .filter(|child| matches!(child, Value::Array(_) | Value::Object(_)))
                            .map(|child| (child, container_depth)),
                    );
                }
                _ => unreachable!("container kind was checked above"),
            }
        }
        self.value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ManagedSemanticJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Value::deserialize(deserializer).map(Self::from_value)
    }
}
