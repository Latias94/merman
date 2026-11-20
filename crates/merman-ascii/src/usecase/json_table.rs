//! Ordered JSON tables with explicit, unambiguous terminal paths.

use super::adapter::{allocation_failed, unsupported};
use super::validation::reserved_vec;
use crate::error::Result;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::{
    BudgetedTextDocument, terminal_line_display_width, visit_quoted_terminal_text,
};
use merman_model::families::usecase::{UsecaseJsonInfinity, UsecaseJsonNode, json_string};
use serde_json::Value;
use std::collections::HashSet;

pub(super) struct TableText {
    pub(super) title: String,
    pub(super) body: String,
}

pub(super) fn prepare(
    nodes: &[UsecaseJsonNode],
    profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<TableText>> {
    let mut tables = reserved_vec(nodes.len())?;
    let mut retained = 0;
    for node in nodes {
        let resources = resources
            .clone()
            .with_additional_retained_output(retained)?;
        let rows = Rows::new(node, &resources)?.collect()?;
        let row_bytes = rows.iter().try_fold(0, |total, row| {
            add_bytes(
                &resources,
                add_bytes(&resources, total, row.path.len())?,
                row.value.len(),
            )
        })?;
        let resources = resources.with_additional_retained_output(row_bytes)?;
        let options = AsciiRenderOptions::ascii().with_terminal_width_profile(profile);
        let mut title = BudgetedTextDocument::from_resources(resources.scoped(), &options);
        title.push_line_with(|line| {
            line.push_quoted_text(&node.id)?;
            line.push_str(" [JSON]")
        })?;
        let title = title.finish()?;
        let resources = resources.with_additional_retained_output(title.len())?;
        let mut key_width = "Path".len();
        for row in &rows {
            resources.charge_layout_work(row.path.len().max(1))?;
            key_width = key_width.max(terminal_line_display_width(&row.path, profile));
        }
        let mut body = BudgetedTextDocument::from_resources(resources.scoped(), &options);
        for (path, value) in std::iter::once(("Path", "Value")).chain(
            rows.iter()
                .map(|row| (row.path.as_str(), row.value.as_str())),
        ) {
            let width = terminal_line_display_width(path, profile);
            body.push_line_with(|line| {
                line.push_str(path)?;
                line.write_fmt(format_args!("{:padding$}", "", padding = key_width - width))?;
                line.push_str(" | ")?;
                line.push_str(value)
            })?;
        }
        let body = body.finish()?;
        retained = add_bytes(
            &resources,
            retained,
            add_bytes(&resources, title.len(), body.len())?,
        )?;
        tables.push(TableText { title, body });
    }
    Ok(tables)
}

struct Row {
    path: String,
    value: String,
}

struct Pending<'a> {
    value: &'a Value,
    /// Stored keys, rather than decoded display text, identify sidecar metadata.
    pointer: String,
    path: String,
    depth: usize,
}

enum Segment<'a> {
    Property(&'a str),
    Index(usize),
}

struct Rows<'a, 'resources> {
    node: &'a UsecaseJsonNode,
    resources: &'resources ResourceContext,
    pending: Vec<Pending<'a>>,
    rows: Vec<Row>,
    live_bytes: usize,
    orders: usize,
    infinities: usize,
}

impl<'a, 'resources> Rows<'a, 'resources> {
    fn new(node: &'a UsecaseJsonNode, resources: &'resources ResourceContext) -> Result<Self> {
        resources.checkpoint()?;
        if !node.value.is_object() {
            return Err(unsupported("JSON table roots must be objects"));
        }
        let mut path = String::new();
        append(&mut path, "$", resources)?;
        let mut pending = reserved_vec(1)?;
        pending.push(Pending {
            value: &node.value,
            pointer: String::new(),
            path,
            depth: 1,
        });
        Ok(Self {
            node,
            resources,
            pending,
            rows: Vec::new(),
            live_bytes: 1,
            orders: 0,
            infinities: 0,
        })
    }

    fn collect(mut self) -> Result<Vec<Row>> {
        while let Some(current) = self.pending.pop() {
            self.resources.check_nesting_depth(current.depth)?;
            self.resources
                .charge_layout_work(current.pointer.len().max(1))?;
            if self.node.non_finite_numbers.contains_key(&current.pointer) {
                if !current.value.is_null() {
                    return Err(unsupported(
                        "JSON infinity metadata must identify null values",
                    ));
                }
                self.infinities += 1;
            }
            let children = match current.value {
                Value::Object(values) => values.len(),
                Value::Array(values) => values.len(),
                _ => 0,
            };
            if children > 0 {
                self.resources
                    .check_nesting_depth(self.resources.checked_work_add(current.depth, 1)?)?;
                // Every pending subtree produces at least one leaf. Admit that lower bound
                // before reserving the traversal queue or an object's metadata index.
                self.reserve_children(children)?;
            }
            let order = self.node.property_order.get(&current.pointer);
            if let Some(order) = order {
                let Some(values) = current.value.as_object() else {
                    return Err(unsupported("JSON property order must identify objects"));
                };
                if order.len() != values.len() {
                    return Err(unsupported(
                        "JSON property order must contain every key exactly once",
                    ));
                }
                self.resources.charge_layout_work(order.len())?;
                let mut seen = HashSet::new();
                seen.try_reserve(order.len())
                    .map_err(|_| allocation_failed())?;
                for key in order {
                    self.resources.charge_layout_work(key.len().max(1))?;
                    if !values.contains_key(key) || !seen.insert(key.as_str()) {
                        return Err(unsupported(
                            "JSON property order must contain every key exactly once",
                        ));
                    }
                }
                self.orders += 1;
            }
            match current.value {
                Value::Object(values) if !values.is_empty() => {
                    if let Some(order) = order {
                        for key in order.iter().rev() {
                            let value = values
                                .get(key)
                                .ok_or_else(|| unsupported("invalid JSON property order"))?;
                            self.child(&current, Segment::Property(key), value)?;
                        }
                    } else {
                        // A borrowed model without source metadata uses its typed map order.
                        for (key, value) in values.iter().rev() {
                            self.child(&current, Segment::Property(key), value)?;
                        }
                    }
                }
                Value::Array(values) if !values.is_empty() => {
                    for (index, value) in values.iter().enumerate().rev() {
                        self.child(&current, Segment::Index(index), value)?;
                    }
                }
                value => {
                    let resources = self
                        .resources
                        .clone()
                        .with_additional_retained_output(self.live_bytes)?;
                    let value = scalar(self.node, value, &current.pointer, &resources)?;
                    self.live_bytes = add_bytes(self.resources, self.live_bytes, value.len())?;
                    self.live_bytes -= current.pointer.len();
                    self.resources
                        .check_usage(0, self.rows.len().saturating_add(1))?;
                    self.rows.try_reserve(1).map_err(|_| allocation_failed())?;
                    self.rows.push(Row {
                        path: current.path,
                        value,
                    });
                    continue;
                }
            }
            self.live_bytes -= current.path.len() + current.pointer.len();
        }
        if self.orders != self.node.property_order.len()
            || self.infinities != self.node.non_finite_numbers.len()
        {
            return Err(unsupported("unreachable JSON table metadata"));
        }
        Ok(self.rows)
    }

    fn reserve_children(&mut self, count: usize) -> Result<()> {
        let minimum_rows = self.resources.checked_work_add(
            self.resources
                .checked_work_add(self.rows.len(), self.pending.len())?,
            count,
        )?;
        self.resources.check_usage(0, minimum_rows)?;
        self.resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            add_bytes(self.resources, self.live_bytes, count)?,
        )?;
        self.resources.charge_layout_work(count)?;
        self.pending
            .try_reserve(count)
            .map_err(|_| allocation_failed())
    }

    fn child(
        &mut self,
        parent: &Pending<'a>,
        segment: Segment<'a>,
        value: &'a Value,
    ) -> Result<()> {
        let resources = self
            .resources
            .clone()
            .with_additional_retained_output(self.live_bytes)?;
        let mut pointer = String::new();
        append(&mut pointer, &parent.pointer, &resources)?;
        append(&mut pointer, "/", &resources)?;
        match segment {
            Segment::Property(key) => {
                for ch in key.chars() {
                    let mut bytes = [0; 4];
                    append(
                        &mut pointer,
                        match ch {
                            '~' => "~0",
                            '/' => "~1",
                            _ => ch.encode_utf8(&mut bytes),
                        },
                        &resources,
                    )?;
                }
            }
            Segment::Index(index) => append(&mut pointer, &index.to_string(), &resources)?,
        }
        let resources = resources.with_additional_retained_output(pointer.len())?;
        let mut path = String::new();
        append(&mut path, &parent.path, &resources)?;
        append(&mut path, "[", &resources)?;
        match segment {
            Segment::Property(key) => string(self.node, key, &mut path, &resources)?,
            Segment::Index(index) => append(&mut path, &index.to_string(), &resources)?,
        }
        append(&mut path, "]", &resources)?;
        self.live_bytes = add_bytes(
            self.resources,
            self.live_bytes,
            add_bytes(self.resources, path.len(), pointer.len())?,
        )?;
        self.pending.push(Pending {
            value,
            pointer,
            path,
            depth: self.resources.checked_work_add(parent.depth, 1)?,
        });
        Ok(())
    }
}

fn scalar(
    node: &UsecaseJsonNode,
    value: &Value,
    pointer: &str,
    resources: &ResourceContext,
) -> Result<String> {
    let mut output = String::new();
    match value {
        Value::String(value) => string(node, value, &mut output, resources)?,
        Value::Number(value) => {
            let value = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| unsupported("invalid JSON number"))?;
            append(&mut output, ryu_js::Buffer::new().format(value), resources)?;
        }
        Value::Null => append(
            &mut output,
            match node.non_finite_numbers.get(pointer) {
                Some(UsecaseJsonInfinity::Positive) => "Infinity",
                Some(UsecaseJsonInfinity::Negative) => "-Infinity",
                None => "null",
            },
            resources,
        )?,
        Value::Bool(value) => append(
            &mut output,
            if *value { "true" } else { "false" },
            resources,
        )?,
        Value::Array(_) => append(&mut output, "[]", resources)?,
        Value::Object(_) => append(&mut output, "{}", resources)?,
    }
    Ok(output)
}

fn string(
    node: &UsecaseJsonNode,
    value: &str,
    output: &mut String,
    resources: &ResourceContext,
) -> Result<()> {
    if node.string_encoding.is_some() {
        resources.charge_layout_work_product(value.len(), 2)?;
        // The model decoder can retain two bytes of UTF-16 and three bytes of UTF-8 per
        // source byte. Admit that scratch space before calling its allocating helper.
        let scratch = value
            .len()
            .checked_mul(5)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            add_bytes(resources, output.len(), scratch)?,
        )?;
        if json_string::has_unpaired_surrogates(value)
            .map_err(|_| unsupported("invalid JSON UTF-16 strings"))?
        {
            append(output, "utf16:", resources)?;
            return append(output, value.trim(), resources);
        }
    }
    let decoded = node
        .display_string(value)
        .map_err(|_| unsupported("invalid JSON UTF-16 strings"))?;
    let retained = if matches!(decoded, std::borrow::Cow::Owned(_)) {
        decoded.len()
    } else {
        0
    };
    let resources = resources
        .clone()
        .with_additional_retained_output(retained)?;
    visit_quoted_terminal_text(&decoded, &resources, |fragment| {
        append(output, fragment, &resources)
    })
}

fn add_bytes(resources: &ResourceContext, left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))
}

fn append(output: &mut String, value: &str, resources: &ResourceContext) -> Result<()> {
    resources.charge_layout_work(value.len().max(1))?;
    resources.check(
        AsciiResourceLimitId::MaxOutputBytes,
        add_bytes(resources, output.len(), value.len())?,
    )?;
    output
        .try_reserve(value.len())
        .map_err(|_| allocation_failed())?;
    output.push_str(value);
    Ok(())
}
