//! Bounded Mermaid-compatible JSON projection of completed Treemap data.

#[cfg(test)]
use super::TreemapDiagramModel;
use super::{TreemapClassDef, TreemapDiagramRenderModel, TreemapNode, TreemapOutputPresence};
use crate::json::ManagedJson;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value, json};

// The compatibility shape contains both a nested `root` tree and a flattened `nodes` array. Keep a
// hard cap on the complete serialized shape so model limits cannot be bypassed by that duplication
// or by cloning the effective config into the output.
pub const TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES: usize = 8 * 1024 * 1024;

pub fn project(
    model: &TreemapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    project_controlled(model, meta, &OperationControl::new())
        .expect("a private compatibility projection control cannot be cancelled")
        .map(ManagedJson::into_unmanaged_value)
}

pub fn project_controlled(
    model: &TreemapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
    control.checkpoint()?;
    match PreparedProjection::new_controlled(model, meta, control)? {
        Ok(projection) => projection.project_controlled(control),
        Err(error) => Ok(Err(error)),
    }
}

/// Preflight-checked immutable inputs for one family projection.
///
/// Private fields bind validation to these borrowed values. Source adapters can prepare before
/// their editor-fact handoff and project afterward without rebuilding or repeating preflight.
#[derive(Debug)]
pub struct PreparedProjection<'a> {
    model: &'a TreemapDiagramRenderModel,
    metadata: CompatJsonMetadata<'a>,
}

impl<'a> PreparedProjection<'a> {
    pub fn new_controlled(
        model: &'a TreemapDiagramRenderModel,
        metadata: &CompatJsonMetadata<'a>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, CompatProjectionError>> {
        match treemap_compatibility_output_preflight(model, metadata, control)? {
            Ok(()) => Ok(Ok(Self {
                model,
                metadata: *metadata,
            })),
            Err(error) => Ok(Err(error)),
        }
    }

    pub fn project_controlled(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        project_prepared_controlled(self.model, &self.metadata, control)
    }
}

fn project_prepared_controlled(
    model: &TreemapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
    control.checkpoint()?;
    if model.output_presence() == TreemapOutputPresence::Empty {
        return Ok(Ok(ManagedJson::from_value(json!({}))));
    }
    let root = match project_root_controlled(model, control)? {
        Ok(root) => root,
        Err(message) => {
            return Ok(Err(CompatProjectionError::invalid_model(
                meta.diagram_type,
                message,
            )));
        }
    };
    let mut nodes = ManagedJson::from_value(Value::Array(Vec::with_capacity(model.nodes.len())));
    let mut stack = model
        .root
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .rev()
        .map(|&id| (id, 0_i64))
        .collect::<Vec<_>>();
    while let Some((id, level)) = stack.pop() {
        control.checkpoint()?;
        let node = &model.nodes[id];
        let mut value = render_node_to_json_controlled(
            node,
            model.compiled_styles(Some(id)),
            &model.classes,
            control,
        )?;
        value
            .as_value_mut()
            .as_object_mut()
            .expect("node projection is an object")
            .insert("level".to_string(), Value::Number(level.into()));
        nodes
            .as_value_mut()
            .as_array_mut()
            .expect("flat node projection is an array")
            .push(value.into_unmanaged_value());
        if let Some(children) = &node.children {
            for &child in children.iter().rev() {
                control.checkpoint()?;
                stack.push((child, level.saturating_add(1)));
            }
        }
    }
    let classes = render_classes_to_value(&model.classes, control)?;
    let config = ManagedJson::from_value(crate::json::clone_value_nonrecursive_with_control(
        meta.config.as_value(),
        control,
    )?);
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert("title".to_string(), json!(&model.title));
    out.insert("accTitle".to_string(), json!(&model.acc_title));
    out.insert("accDescr".to_string(), json!(&model.acc_descr));
    out.insert("root".to_string(), root.into_unmanaged_value());
    out.insert("nodes".to_string(), nodes.into_unmanaged_value());
    out.insert("classes".to_string(), classes);
    out.insert("config".to_string(), config.into_unmanaged_value());
    let out = ManagedJson::from_value(Value::Object(out));
    control.checkpoint()?;
    Ok(Ok(out))
}

/// Estimates the complete compatibility JSON shape before allocating its output `Value` tree.
/// The estimate includes the nested and flattened node representations, class definitions, source
/// values, and effective config. It is deliberately conservative around JSON punctuation and
/// escaping so a later projection cannot bypass the hard output cap. Cancellation remains on the
/// outer [`OperationControlResult`] channel.
fn treemap_compatibility_output_preflight(
    model: &TreemapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<(), CompatProjectionError>> {
    control.checkpoint()?;
    if model.output_presence() == TreemapOutputPresence::Empty {
        return Ok(Ok(()));
    }

    if let Err(message) = model.validate_child_relationships_controlled(control)? {
        return Ok(Err(CompatProjectionError::invalid_model(
            meta.diagram_type,
            message,
        )));
    }
    let class_style_bytes = estimate_class_style_arrays(&model.classes, control)?;
    let estimated_bytes =
        estimate_compatibility_json_bytes(model, meta, &class_style_bytes, control)?;
    if estimated_bytes > TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES {
        return Ok(Err(treemap_compatibility_output_budget_error(
            meta,
            estimated_bytes,
        )));
    }
    Ok(Ok(()))
}

fn estimate_compatibility_json_bytes(
    model: &TreemapDiagramRenderModel,
    meta: &CompatJsonMetadata<'_>,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let mut total = 2usize; // top-level object braces
    let field_count = 8usize;

    total = total.saturating_add(json_field_bytes(
        "type",
        json_string_serialized_bytes(meta.diagram_type),
    ));
    total = total.saturating_add(json_field_bytes(
        "title",
        json_optional_string_bytes(model.title.as_deref()),
    ));
    total = total.saturating_add(json_field_bytes(
        "accTitle",
        json_optional_string_bytes(model.acc_title.as_deref()),
    ));
    total = total.saturating_add(json_field_bytes(
        "accDescr",
        json_optional_string_bytes(model.acc_descr.as_deref()),
    ));
    total = total.saturating_add(json_field_bytes(
        "root",
        estimate_nested_node_tree_bytes(model, class_style_bytes, control)?,
    ));
    total = total.saturating_add(json_field_bytes(
        "nodes",
        estimate_flat_node_array_bytes(model, class_style_bytes, control)?,
    ));
    total = total.saturating_add(json_field_bytes(
        "classes",
        estimate_classes_value_bytes(&model.classes, control)?,
    ));
    total = total.saturating_add(json_field_bytes(
        "config",
        json_value_serialized_bytes(meta.config.as_value(), control)?,
    ));
    total = total.saturating_add(field_count.saturating_sub(1));
    Ok(total)
}

fn estimate_nested_node_tree_bytes(
    model: &TreemapDiagramRenderModel,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let mut total = 0usize;
    let mut stack = vec![(&model.root, None)];
    let mut visited = 0usize;
    while let Some((node, id)) = stack.pop() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        total = total.saturating_add(estimate_node_map_bytes(
            node,
            model.compiled_styles(id),
            true,
            None,
            class_style_bytes,
            control,
        )?);
        if let Some(children) = node.children.as_ref() {
            stack.extend(
                children
                    .iter()
                    .rev()
                    .map(|&id| (&model.nodes[id], Some(id))),
            );
        }
    }
    control.checkpoint()?;
    Ok(total)
}

fn estimate_flat_node_array_bytes(
    model: &TreemapDiagramRenderModel,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let children = model.root.children.as_deref().unwrap_or_default();
    let mut total = 2usize; // array brackets; commas are added per emitted item
    let mut stack = children
        .iter()
        .rev()
        .map(|&id| (id, 0usize))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some((id, level)) = stack.pop() {
        let node = &model.nodes[id];
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        if visited > 1 {
            total = total.saturating_add(1);
        }
        total = total.saturating_add(estimate_node_map_bytes(
            node,
            model.compiled_styles(Some(id)),
            false,
            Some(level),
            class_style_bytes,
            control,
        )?);
        if let Some(children) = node.children.as_ref() {
            stack.extend(
                children
                    .iter()
                    .rev()
                    .map(|&id| (id, level.saturating_add(1))),
            );
        }
    }
    control.checkpoint()?;
    Ok(total)
}

fn estimate_node_map_bytes(
    node: &TreemapNode,
    compiled_styles: Option<&[String]>,
    include_children: bool,
    level: Option<usize>,
    class_style_bytes: &std::collections::HashMap<&str, usize>,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let mut total = 2usize; // object braces
    let mut fields = 1usize; // name
    total = total.saturating_add(json_field_bytes(
        "name",
        json_string_serialized_bytes(&node.name),
    ));

    if include_children && let Some(children) = node.children.as_ref() {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "children",
            json_array_overhead(children.len()),
        ));
    }
    if let Some(value) = node.value.as_ref() {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "value",
            json_value_serialized_bytes(value, control)?,
        ));
    }
    if let Some(class_selector) = node.class_selector.as_deref() {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "classSelector",
            json_string_serialized_bytes(class_selector),
        ));
    }
    if let Some(styles) = compiled_styles {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes(
            "cssCompiledStyles",
            estimate_string_array_bytes(styles, control)?,
        ));
    } else if let Some(selector) = node.class_selector.as_deref()
        && let Some(&styles_bytes) = class_style_bytes.get(selector)
    {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes("cssCompiledStyles", styles_bytes));
    }
    if let Some(level) = level {
        fields = fields.saturating_add(1);
        total = total.saturating_add(json_field_bytes("level", level.to_string().len()));
    }

    Ok(total.saturating_add(fields.saturating_sub(1)))
}

fn estimate_class_style_arrays<'a>(
    classes: &'a std::collections::BTreeMap<String, TreemapClassDef>,
    control: &OperationControl,
) -> OperationControlResult<std::collections::HashMap<&'a str, usize>> {
    let mut class_style_bytes = std::collections::HashMap::with_capacity(classes.len());
    for (index, (id, class_def)) in classes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if !class_def.styles.is_empty() {
            class_style_bytes.insert(
                id.as_str(),
                json_array_overhead(1).saturating_add(
                    joined_style_json_content_bytes(&class_def.styles, control)?.saturating_add(2),
                ),
            );
        }
    }
    Ok(class_style_bytes)
}

fn estimate_classes_value_bytes(
    classes: &std::collections::BTreeMap<String, TreemapClassDef>,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let mut total = json_object_overhead(classes.len());
    for (index, (name, class_def)) in classes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let mut class_total = 2usize; // class definition object braces
        class_total = class_total.saturating_add(json_field_bytes(
            "id",
            json_string_serialized_bytes(&class_def.id),
        ));
        class_total = class_total.saturating_add(json_field_bytes(
            "styles",
            estimate_string_array_bytes(&class_def.styles, control)?,
        ));
        class_total = class_total.saturating_add(json_field_bytes(
            "textStyles",
            estimate_string_array_bytes(&class_def.text_styles, control)?,
        ));
        class_total = class_total.saturating_add(2); // three class fields
        total = total.saturating_add(json_string_serialized_bytes(name));
        total = total.saturating_add(1); // map entry colon
        total = total.saturating_add(class_total);
    }
    control.checkpoint()?;
    Ok(total)
}

fn estimate_string_array_bytes(
    values: &[String],
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let mut total = json_array_overhead(values.len());
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        total = total.saturating_add(json_string_serialized_bytes(value));
    }
    Ok(total)
}

fn joined_style_json_content_bytes(
    styles: &[String],
    control: &OperationControl,
) -> OperationControlResult<usize> {
    let mut total = 0usize;
    for (index, style) in styles.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if index != 0 {
            total = total.saturating_add(json_string_content_bytes(";"));
        }
        total = total.saturating_add(json_string_content_bytes(style));
    }
    Ok(total)
}

fn json_value_serialized_bytes(
    value: &Value,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    enum Task<'a> {
        Value(&'a Value),
        Array {
            values: &'a [Value],
            index: usize,
        },
        Object {
            entries: serde_json::map::Iter<'a>,
            remaining: usize,
        },
    }

    let mut total = 0usize;
    let mut tasks = vec![Task::Value(value)];
    let mut visited = 0usize;
    while let Some(task) = tasks.pop() {
        if visited.is_multiple_of(128) {
            control.checkpoint()?;
        }
        visited = visited.saturating_add(1);
        match task {
            Task::Value(value) => match value {
                Value::Null => total = total.saturating_add(4),
                Value::Bool(value) => {
                    total = total.saturating_add(if *value { 4 } else { 5 });
                }
                Value::Number(value) => total = total.saturating_add(value.to_string().len()),
                Value::String(value) => {
                    total = total.saturating_add(json_string_serialized_bytes(value));
                }
                Value::Array(values) => {
                    total = total.saturating_add(json_array_overhead(values.len()));
                    tasks.push(Task::Array { values, index: 0 });
                }
                Value::Object(values) => {
                    total = total.saturating_add(json_object_overhead(values.len()));
                    tasks.push(Task::Object {
                        entries: values.iter(),
                        remaining: values.len(),
                    });
                }
            },
            Task::Array { values, index } => {
                if let Some(value) = values.get(index) {
                    tasks.push(Task::Array {
                        values,
                        index: index.saturating_add(1),
                    });
                    tasks.push(Task::Value(value));
                }
            }
            Task::Object {
                mut entries,
                remaining,
            } => {
                if remaining != 0
                    && let Some((key, value)) = entries.next()
                {
                    total = total
                        .saturating_add(json_string_serialized_bytes(key))
                        .saturating_add(1);
                    tasks.push(Task::Object {
                        entries,
                        remaining: remaining.saturating_sub(1),
                    });
                    tasks.push(Task::Value(value));
                }
            }
        }
    }
    Ok(total)
}

fn json_string_content_bytes(value: &str) -> usize {
    value.bytes().fold(0usize, |total, byte| {
        total.saturating_add(match byte {
            b'"' | b'\\' | 0x08 | 0x09 | 0x0a | 0x0c | 0x0d => 2,
            0x00..=0x1f => 6,
            _ => 1,
        })
    })
}

fn json_string_serialized_bytes(value: &str) -> usize {
    json_string_content_bytes(value).saturating_add(2)
}

fn json_optional_string_bytes(value: Option<&str>) -> usize {
    value.map_or(4, json_string_serialized_bytes)
}

fn json_field_bytes(key: &str, value_bytes: usize) -> usize {
    json_string_serialized_bytes(key)
        .saturating_add(1)
        .saturating_add(value_bytes)
}

fn json_array_overhead(item_count: usize) -> usize {
    2usize.saturating_add(item_count.saturating_sub(1))
}

fn json_object_overhead(field_count: usize) -> usize {
    2usize.saturating_add(field_count.saturating_sub(1))
}

fn treemap_compatibility_output_budget_error(
    meta: &CompatJsonMetadata<'_>,
    estimated_bytes: usize,
) -> CompatProjectionError {
    CompatProjectionError::invalid_model(
        meta.diagram_type.to_owned(),
        format!(
            "treemap compatibility JSON output budget exceeded: estimated compatibility JSON requires {estimated_bytes} bytes, maximum is {TREEMAP_COMPATIBILITY_OUTPUT_BUDGET_BYTES} bytes"
        ),
    )
}

fn render_node_to_json_controlled(
    node: &TreemapNode,
    compiled_styles: Option<&[String]>,
    classes: &std::collections::BTreeMap<String, TreemapClassDef>,
    control: &OperationControl,
) -> OperationControlResult<ManagedJson> {
    control.checkpoint()?;
    let mut out = ManagedJson::from_value(Value::Object(Map::new()));
    let fields = out.as_value_mut().as_object_mut().expect("node object");
    fields.insert("name".to_string(), Value::String(node.name.clone()));
    if let Some(value) = &node.value {
        fields.insert(
            "value".to_string(),
            crate::json::clone_value_nonrecursive_with_control(value, control)?,
        );
    }
    if let Some(class_selector) = &node.class_selector {
        fields.insert(
            "classSelector".to_string(),
            Value::String(class_selector.clone()),
        );
    }
    if let Some(styles) = compiled_styles {
        fields.insert(
            "cssCompiledStyles".to_string(),
            Value::Array(clone_string_array_with_control(styles, control)?),
        );
    } else if let Some(selector) = node.class_selector.as_deref()
        && let Some(class_def) = classes.get(selector)
    {
        let styles = join_styles_with_control(&class_def.styles, control)?;
        if !styles.is_empty() {
            fields.insert(
                "cssCompiledStyles".to_string(),
                Value::Array(vec![Value::String(styles)]),
            );
        }
    }
    Ok(out)
}

fn project_root_controlled(
    model: &TreemapDiagramRenderModel,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<ManagedJson, &'static str>> {
    let root_id = model.nodes.len();
    let mut completed = vec![None::<ManagedJson>; root_id + 1];
    let mut seen = vec![false; root_id + 1];
    let mut stack = vec![(root_id, false)];
    while let Some((id, expanded)) = stack.pop() {
        control.checkpoint()?;
        let node = if id == root_id {
            &model.root
        } else {
            &model.nodes[id]
        };
        if expanded {
            let mut projected = render_node_to_json_controlled(
                node,
                model.compiled_styles((id != root_id).then_some(id)),
                &model.classes,
                control,
            )?;
            let mut children = node.children.as_ref().map(|_| Vec::new());
            if let Some(ids) = &node.children {
                for &child in ids {
                    control.checkpoint()?;
                    let Some(value) = completed[child].take() else {
                        return Ok(Err("invalid treemap child relationship"));
                    };
                    children.as_mut().expect("children slot exists").push(value);
                }
            }
            if let Some(children) = children {
                projected
                    .as_value_mut()
                    .as_object_mut()
                    .expect("node projection is an object")
                    .insert(
                        "children".to_string(),
                        Value::Array(
                            children
                                .into_iter()
                                .map(ManagedJson::into_unmanaged_value)
                                .collect(),
                        ),
                    );
            }
            completed[id] = Some(projected);
        } else {
            if std::mem::replace(&mut seen[id], true) {
                return Ok(Err("cyclic or repeated treemap child relationship"));
            }
            stack.push((id, true));
            if let Some(children) = &node.children {
                for &child in children.iter().rev() {
                    control.checkpoint()?;
                    if child >= root_id {
                        return Ok(Err("treemap child ID is out of range"));
                    }
                    stack.push((child, false));
                }
            }
        }
    }
    control.checkpoint()?;
    Ok(Ok(completed[root_id]
        .take()
        .expect("root projection completed")))
}

fn render_classes_to_value(
    classes: &std::collections::BTreeMap<String, TreemapClassDef>,
    control: &OperationControl,
) -> OperationControlResult<Value> {
    let mut out = Map::new();
    for (index, (name, class_def)) in classes.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        let mut class = Map::new();
        class.insert("id".to_string(), Value::String(class_def.id.clone()));
        class.insert(
            "styles".to_string(),
            Value::Array(clone_string_array_with_control(&class_def.styles, control)?),
        );
        class.insert(
            "textStyles".to_string(),
            Value::Array(clone_string_array_with_control(
                &class_def.text_styles,
                control,
            )?),
        );
        out.insert(name.clone(), Value::Object(class));
    }
    control.checkpoint()?;
    Ok(Value::Object(out))
}

fn clone_string_array_with_control(
    values: &[String],
    control: &OperationControl,
) -> OperationControlResult<Vec<Value>> {
    let mut cloned = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        cloned.push(Value::String(value.clone()));
    }
    Ok(cloned)
}

fn join_styles_with_control(
    styles: &[String],
    control: &OperationControl,
) -> OperationControlResult<String> {
    let mut joined = String::new();
    for (index, style) in styles.iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if index != 0 {
            joined.push(';');
        }
        joined.push_str(style);
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MermaidConfig;
    #[test]
    fn treemap_compatibility_preflight_estimate_covers_serialized_output() {
        let model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            title: Some("title \"with\" controls\n".to_string()),
            acc_title: Some("acc".to_string()),
            acc_descr: Some("description".to_string()),
            root: TreemapNode {
                name: "root".to_string(),
                children: Some(vec![0]),
                value: None,
                class_selector: None,
            },
            nodes: vec![TreemapNode {
                name: "leaf \"\u{0001}".to_string(),
                children: None,
                value: Some(json!({"nested": ["value", true, null]}).into()),
                class_selector: Some("shared".to_string()),
            }],
            classes: std::collections::BTreeMap::from([(
                "shared".to_string(),
                TreemapClassDef {
                    id: "shared".to_string(),
                    styles: vec!["fill:\"quoted\"".to_string(), "stroke:#123".to_string()],
                    text_styles: vec!["font-size:12px".to_string()],
                },
            )]),
        });
        let config = MermaidConfig::empty_object();
        let meta = CompatJsonMetadata {
            diagram_type: "treemap",
            config: &config,
            authored_config: &config,
            title: None,
            diagram_id: None,
        };
        let control = OperationControl::new();
        let class_style_bytes = estimate_class_style_arrays(&model.classes, &control).unwrap();
        let estimate =
            estimate_compatibility_json_bytes(&model, &meta, &class_style_bytes, &control).unwrap();
        let output = project(&model, &meta).unwrap();
        let actual = serde_json::to_vec(&output).unwrap().len();

        assert!(
            estimate >= actual,
            "preflight estimate {estimate} must cover serialized output {actual}"
        );
    }

    #[test]
    fn treemap_style_byte_preflight_observes_cancellation_inside_one_class() {
        let styles = (0..512)
            .map(|index| format!("fill:{index}"))
            .collect::<Vec<_>>();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        assert!(matches!(
            joined_style_json_content_bytes(&styles, &control),
            Err(crate::OperationCancelled { .. })
        ));
    }
}
