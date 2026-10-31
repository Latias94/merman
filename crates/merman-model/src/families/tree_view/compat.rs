//! Pure Mermaid-compatible JSON projection of declared TreeView data.

use super::{TreeViewDiagramModel, TreeViewNode};
use crate::json::ManagedJson;
use crate::presentation::compat::{CompatJsonMetadata, CompatProjectionError};
use crate::{OperationControl, OperationControlResult};
use serde_json::{Map, Value, json};

pub fn project(
    model: &TreeViewDiagramModel,
    meta: &CompatJsonMetadata<'_>,
) -> Result<Value, CompatProjectionError> {
    project_controlled(model, meta, &OperationControl::new())
        .expect("a private projection control cannot be cancelled")
}

pub fn project_controlled(
    model: &TreeViewDiagramModel,
    meta: &CompatJsonMetadata<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, CompatProjectionError>> {
    control.checkpoint()?;
    let root = project_node_controlled(&model.root, control)?;
    let mut nodes = Vec::new();
    let mut pending = vec![&model.root];
    while let Some(node) = pending.pop() {
        control.checkpoint()?;
        nodes.push(flat_node_to_value(node));
        for child in node.children.iter().rev() {
            control.checkpoint()?;
            pending.push(child);
        }
    }
    let mut out = Map::new();
    out.insert(
        "type".to_owned(),
        Value::String(meta.diagram_type.to_owned()),
    );
    out.insert("title".to_owned(), json!(&model.title));
    out.insert("accTitle".to_owned(), json!(&model.acc_title));
    out.insert("accDescr".to_owned(), json!(&model.acc_descr));
    out.insert("root".to_owned(), root.into_unmanaged_value());
    out.insert("nodes".to_owned(), Value::Array(nodes));
    let out = ManagedJson::from_value(Value::Object(out));
    control.checkpoint()?;
    Ok(Ok(out.into_unmanaged_value()))
}

fn project_node_controlled(
    root: &TreeViewNode,
    control: &OperationControl,
) -> OperationControlResult<ManagedJson> {
    struct Frame<'a> {
        node: &'a TreeViewNode,
        next: usize,
        children: Vec<ManagedJson>,
    }
    let mut stack = vec![Frame {
        node: root,
        next: 0,
        children: Vec::new(),
    }];
    loop {
        control.checkpoint()?;
        let frame = stack.last_mut().expect("root projection frame exists");
        if let Some(child) = frame.node.children.get(frame.next) {
            frame.next += 1;
            stack.push(Frame {
                node: child,
                next: 0,
                children: Vec::new(),
            });
            continue;
        }
        let frame = stack.pop().expect("completed frame exists");
        let mut fields = flat_node_fields(frame.node);
        fields.insert(
            "children".to_owned(),
            Value::Array(
                frame
                    .children
                    .into_iter()
                    .map(ManagedJson::into_unmanaged_value)
                    .collect(),
            ),
        );
        let value = ManagedJson::from_value(Value::Object(fields));
        if let Some(parent) = stack.last_mut() {
            parent.children.push(value);
        } else {
            control.checkpoint()?;
            return Ok(value);
        }
    }
}

fn flat_node_to_value(node: &TreeViewNode) -> Value {
    Value::Object(flat_node_fields(node))
}

fn flat_node_fields(node: &TreeViewNode) -> Map<String, Value> {
    let mut value = Map::new();
    value.insert("id".to_owned(), json!(node.id));
    value.insert("level".to_owned(), json!(node.level));
    value.insert("name".to_owned(), json!(node.name));
    value.insert("nodeType".to_owned(), json!(node.node_type));
    if let Some(css_class) = &node.css_class {
        value.insert("cssClass".to_owned(), json!(css_class));
    }
    if let Some(icon) = &node.icon {
        value.insert("icon".to_owned(), json!(icon));
    }
    if let Some(description) = &node.description {
        value.insert("description".to_owned(), json!(description));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MermaidConfig;

    #[test]
    fn nested_and_flat_projections_keep_each_occurrence_and_optional_fields() {
        let child = TreeViewNode {
            id: 1,
            level: 0,
            name: "leaf".to_owned(),
            css_class: Some("accent".to_owned()),
            icon: Some("file".to_owned()),
            description: Some("description".to_owned()),
            ..TreeViewNode::default()
        };
        let model = TreeViewDiagramModel {
            root: TreeViewNode {
                children: vec![child.clone(), child],
                ..TreeViewNode::default()
            },
            ..TreeViewDiagramModel::default()
        };
        let config = MermaidConfig::empty_object();
        let metadata = CompatJsonMetadata {
            diagram_type: "treeView",
            config: &config,
            authored_config: &config,
            title: None,
            diagram_id: None,
        };
        let value = project(&model, &metadata).unwrap();
        assert_eq!(value["root"], serde_json::to_value(&model.root).unwrap());
        assert_eq!(value["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(value["nodes"][1], value["nodes"][2]);
        assert!(value["nodes"][1].get("children").is_none());
        assert_eq!(value["nodes"][1]["cssClass"], "accent");
    }

    #[test]
    fn cancellation_releases_a_completed_nested_child_and_preserves_the_model() {
        let mut chain = TreeViewNode::default();
        for _ in 0..64 {
            chain = TreeViewNode {
                children: vec![chain],
                ..TreeViewNode::default()
            };
        }
        let model = TreeViewDiagramModel {
            root: TreeViewNode {
                children: vec![chain, TreeViewNode::default()],
                ..TreeViewNode::default()
            },
            ..TreeViewDiagramModel::default()
        };
        let config = MermaidConfig::empty_object();
        let metadata = CompatJsonMetadata {
            diagram_type: "treeView",
            config: &config,
            authored_config: &config,
            title: None,
            diagram_id: None,
        };
        let control = OperationControl::new();
        // The first child has completed and is managed by its parent's frame at this checkpoint.
        control.cancel_after_checkpoints(132);
        assert!(project_controlled(&model, &metadata, &control).is_err());
        let value = ManagedJson::from_value(project(&model, &metadata).unwrap());
        assert_eq!(value.as_value()["nodes"].as_array().unwrap().len(), 67);
    }
}
