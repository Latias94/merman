//! Iterative JSON value ownership used by configuration and explicit projections.

mod managed;
pub use managed::ManagedJson;

use crate::{OperationControl, OperationControlResult};
use serde_json::Number;
use serde_json::{Map, Value};
use std::mem::size_of;

/// Base charge for a JSON value or object header in configuration budgets.
///
/// The original accounting used the insertion-ordered JSON representation, which
/// reserved seven pointer-sized words and two 64-bit hash seeds on supported targets.
/// Keep that charge independent of the backing map selected by Cargo features.
#[doc(hidden)]
pub const JSON_VALUE_CHARGE_BYTES: usize = size_of::<([usize; 7], [u64; 2])>();

/// Charge for an object entry before accounting for its key bytes and child value.
#[doc(hidden)]
pub const JSON_OBJECT_ENTRY_CHARGE_BYTES: usize =
    size_of::<String>() + JSON_VALUE_CHARGE_BYTES + 4 * size_of::<usize>();

pub fn number_value(value: f64) -> Value {
    if value.is_finite()
        && value.fract() == 0.0
        && value >= i64::MIN as f64
        && value < i64::MAX as f64
    {
        Value::Number(Number::from(value as i64))
    } else {
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

pub fn clone_value_nonrecursive(value: &Value) -> Value {
    let control = OperationControl::new();
    clone_value_nonrecursive_with_control(value, &control)
        .expect("a private operation control cannot be cancelled")
}

pub fn clone_value_nonrecursive_with_control(
    value: &Value,
    control: &OperationControl,
) -> OperationControlResult<Value> {
    clone_value_nonrecursive_controlled(value, usize::MAX, usize::MAX, control)
        .map(|value| value.expect("unbounded config cloning cannot exceed its budget"))
}

pub fn clone_value_nonrecursive_controlled(
    value: &Value,
    max_retained_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<Value>> {
    let mut cloner = ValueCloner::new(max_retained_bytes, max_nesting_depth, control);
    if !cloner.clone_tree(value, 0)? {
        return Ok(None);
    }
    control.checkpoint()?;
    Ok(cloner.cloned.pop().map(ManagedJson::into_unmanaged_value))
}

/// Clones a root object whose members are borrowed from ordered storage sections.
pub(crate) fn clone_object_nonrecursive_controlled<'a>(
    len: usize,
    entries: impl Iterator<Item = (&'a String, &'a Value)> + Clone,
    max_retained_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<Value>> {
    let mut cloner = ValueCloner::new(max_retained_bytes, max_nesting_depth, control);
    cloner.visit()?;
    if len > 0 && max_nesting_depth == 0 {
        return Ok(None);
    }
    if cloner.bounded()
        && !cloner.charge(object_structural_weight(
            len,
            entries.clone().map(|(key, _)| key),
        ))
    {
        return Ok(None);
    }
    for (_, value) in entries.clone() {
        if !cloner.clone_tree(value, 1)? {
            return Ok(None);
        }
    }
    cloner.visit()?;
    if !cloner.charge(JSON_VALUE_CHARGE_BYTES) {
        return Ok(None);
    }
    let object = ManagedJson::from_value(cloner.collect_object(len, entries.map(|(key, _)| key))?);
    control.checkpoint()?;
    Ok(Some(object.into_unmanaged_value()))
}

struct ValueCloner<'a> {
    stack: Vec<(&'a Value, bool, usize)>,
    cloned: Vec<ManagedJson>,
    retained_bytes: usize,
    visited_nodes: usize,
    max_retained_bytes: usize,
    max_nesting_depth: usize,
    control: &'a OperationControl,
}

impl<'a> ValueCloner<'a> {
    fn new(
        max_retained_bytes: usize,
        max_nesting_depth: usize,
        control: &'a OperationControl,
    ) -> Self {
        Self {
            stack: Vec::with_capacity(1),
            cloned: Vec::new(),
            retained_bytes: 0,
            visited_nodes: 0,
            max_retained_bytes,
            max_nesting_depth,
            control,
        }
    }

    fn visit(&mut self) -> OperationControlResult<()> {
        if self.visited_nodes.is_multiple_of(64) {
            self.control.checkpoint()?;
        }
        self.visited_nodes = self.visited_nodes.saturating_add(1);
        Ok(())
    }

    fn bounded(&self) -> bool {
        self.max_retained_bytes != usize::MAX
    }

    fn charge(&mut self, weight: usize) -> bool {
        self.retained_bytes = self.retained_bytes.saturating_add(weight);
        self.retained_bytes <= self.max_retained_bytes
    }

    fn clone_tree(&mut self, value: &'a Value, depth: usize) -> OperationControlResult<bool> {
        self.stack.push((value, false, depth));
        while let Some((current, visited, depth)) = self.stack.pop() {
            self.visit()?;
            if visited {
                if self.bounded() && !self.charge(value_clone_weight(current)) {
                    return Ok(false);
                }
                let value = match current {
                    Value::Null => Value::Null,
                    Value::Bool(v) => Value::Bool(*v),
                    Value::Number(v) => Value::Number(v.clone()),
                    Value::String(v) => Value::String(v.clone()),
                    Value::Array(items) => {
                        let mut out = Vec::with_capacity(items.len());
                        let children_start = self.cloned.len() - items.len();
                        for (index, item) in self.cloned.drain(children_start..).enumerate() {
                            if index.is_multiple_of(64)
                                && let Err(cancelled) = self.control.checkpoint()
                            {
                                drop_value_nonrecursive(Value::Array(out));
                                return Err(cancelled);
                            }
                            out.push(item.into_unmanaged_value());
                        }
                        Value::Array(out)
                    }
                    Value::Object(entries) => self.collect_object(entries.len(), entries.keys())?,
                };
                self.cloned.push(ManagedJson::from_value(value));
            } else {
                let has_children = matches!(current, Value::Array(items) if !items.is_empty())
                    || matches!(current, Value::Object(entries) if !entries.is_empty());
                if has_children && depth >= self.max_nesting_depth {
                    return Ok(false);
                }
                if self.bounded() && !self.charge(value_structural_weight(current)) {
                    return Ok(false);
                }
                self.stack.push((current, true, depth));
                let child_depth = depth.saturating_add(1);
                match current {
                    Value::Array(items) => {
                        self.stack
                            .extend(items.iter().rev().map(|item| (item, false, child_depth)));
                    }
                    Value::Object(entries) => {
                        self.stack.extend(
                            entries
                                .values()
                                .rev()
                                .map(|child| (child, false, child_depth)),
                        );
                    }
                    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
                }
            }
        }
        Ok(true)
    }

    fn collect_object(
        &mut self,
        len: usize,
        keys: impl Iterator<Item = &'a String>,
    ) -> OperationControlResult<Value> {
        let mut out = Map::with_capacity(len);
        let children_start = self.cloned.len() - len;
        for (index, (key, child)) in keys.zip(self.cloned.drain(children_start..)).enumerate() {
            if index.is_multiple_of(64)
                && let Err(cancelled) = self.control.checkpoint()
            {
                drop_value_nonrecursive(Value::Object(out));
                return Err(cancelled);
            }
            out.insert(key.clone(), child.into_unmanaged_value());
        }
        Ok(Value::Object(out))
    }
}

fn object_structural_weight<'a>(len: usize, keys: impl Iterator<Item = &'a String>) -> usize {
    let entries = len.saturating_mul(JSON_OBJECT_ENTRY_CHARGE_BYTES);
    keys.fold(entries, |weight, key| weight.saturating_add(key.len()))
}

fn value_structural_weight(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len().saturating_mul(JSON_VALUE_CHARGE_BYTES),
        Value::Object(entries) => object_structural_weight(entries.len(), entries.keys()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => 0,
    }
}

fn value_clone_weight(value: &Value) -> usize {
    JSON_VALUE_CHARGE_BYTES.saturating_add(match value {
        Value::String(value) => value.len(),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_) => 0,
    })
}

/// Estimates the allocation weight of an owned JSON value without recursive traversal.
pub fn estimated_value_owned_heap_bytes(value: &Value) -> usize {
    let mut retained_bytes = 0usize;
    let mut stack = vec![value];
    while let Some(current) = stack.pop() {
        retained_bytes = retained_bytes
            .saturating_add(value_structural_weight(current))
            .saturating_add(value_clone_weight(current));
        match current {
            Value::Array(items) => stack.extend(items),
            Value::Object(entries) => stack.extend(entries.values()),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    retained_bytes
}

pub fn drop_value_nonrecursive(mut value: Value) {
    let mut pending = Vec::new();
    loop {
        let mut next = None;
        let visit = |child| {
            if matches!(child, Value::Array(_) | Value::Object(_))
                && let Some(previous) = next.replace(child)
            {
                pending.push(previous);
            }
        };
        match value {
            Value::Array(items) => items.into_iter().for_each(visit),
            Value::Object(entries) => entries.into_values().for_each(visit),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
        let Some(child) = next.or_else(|| pending.pop()) else {
            break;
        };
        value = child;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_projection_does_not_saturate_at_the_rounded_i64_upper_bound() {
        let upper_bound = i64::MAX as f64;
        let projected = number_value(upper_bound);
        assert_eq!(projected.as_f64(), Some(upper_bound));
        assert_ne!(projected, Value::Number(Number::from(i64::MAX)));
        assert_eq!(
            number_value(i64::MIN as f64),
            Value::Number(Number::from(i64::MIN))
        );
        assert_eq!(number_value(-0.0), Value::Number(Number::from(0)));
        assert_eq!(number_value(f64::INFINITY), Value::Null);
    }

    #[test]
    fn mixed_json_disposal_completes_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(128 * 1024)
            .spawn(|| {
                let mut value = Value::String("leaf".into());
                for depth in 0..10_000 {
                    value = if depth % 2 == 0 {
                        Value::Array(vec![Value::Null, value, Value::String("sibling".into())])
                    } else {
                        Value::Object(Map::from_iter([
                            ("child".into(), value),
                            ("number".into(), Value::Number(Number::from(depth))),
                            ("text".into(), Value::String("sibling".into())),
                        ]))
                    };
                }
                drop_value_nonrecursive(value);
            })
            .expect("spawn small-stack JSON disposal worker")
            .join()
            .expect("dispose deeply nested mixed JSON without recursion");
    }
}
