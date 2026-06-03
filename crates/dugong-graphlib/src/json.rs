//! Graphlib-compatible JSON read/write helpers.
//!
//! Upstream `graphlib.json.write/read` distinguishes omitted `value` fields (`undefined`) from an
//! explicit JSON `null`. The primary Rust seam preserves that shape exactly by operating on
//! `Graph<Option<N>, Option<E>, Option<G>>`.
//!
//! For callers that intentionally want to collapse missing values onto Rust defaults, the explicit
//! `*_with_defaults` helpers provide that fallback behavior.

use crate::{Graph, GraphOptions};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value as JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphJson {
    pub options: GraphOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<JsonValue>,
    pub nodes: Vec<GraphJsonNode>,
    pub edges: Vec<GraphJsonEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphJsonNode {
    pub v: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<JsonValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphJsonEdge {
    pub v: String,
    pub w: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<JsonValue>,
}

pub fn write<N, E, G>(
    graph: &Graph<Option<N>, Option<E>, Option<G>>,
) -> Result<GraphJson, serde_json::Error>
where
    N: Default + Serialize + 'static,
    E: Default + Serialize + 'static,
    G: Default + Serialize,
{
    let nodes = graph
        .node_ids()
        .into_iter()
        .map(|id| {
            let value = graph
                .node(&id)
                .expect("node_ids() should only yield live nodes");
            Ok(GraphJsonNode {
                v: id.clone(),
                value: option_label_to_json(value)?,
                parent: graph.parent(&id).map(str::to_string),
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;

    let edges = graph
        .edge_keys()
        .into_iter()
        .map(|key| {
            let value = graph
                .edge_by_key(&key)
                .expect("edge_keys() should only yield live edges");
            Ok(GraphJsonEdge {
                v: key.v,
                w: key.w,
                name: key.name,
                value: option_label_to_json(value)?,
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;

    Ok(GraphJson {
        options: graph.options(),
        value: option_label_to_json(graph.graph())?,
        nodes,
        edges,
    })
}

pub fn read<N, E, G>(
    json: &GraphJson,
) -> Result<Graph<Option<N>, Option<E>, Option<G>>, serde_json::Error>
where
    N: Default + DeserializeOwned + 'static,
    E: Default + DeserializeOwned + 'static,
    G: Default + DeserializeOwned,
{
    let mut graph = Graph::new(json.options);
    graph.set_graph(option_label_from_json(json.value.clone())?);

    for node in &json.nodes {
        graph.set_node(node.v.clone(), option_label_from_json(node.value.clone())?);
        if let Some(parent) = node.parent.as_deref().filter(|parent| !parent.is_empty()) {
            graph.set_parent_ref(&node.v, parent);
        }
    }

    for edge in &json.edges {
        graph.set_edge_named(
            edge.v.clone(),
            edge.w.clone(),
            edge.name.clone(),
            Some(option_label_from_json(edge.value.clone())?),
        );
    }

    Ok(graph)
}

pub fn write_with_defaults<N, E, G>(graph: &Graph<N, E, G>) -> Result<GraphJson, serde_json::Error>
where
    N: Default + Serialize + 'static,
    E: Default + Serialize + 'static,
    G: Default + Serialize,
{
    let nodes = graph
        .node_ids()
        .into_iter()
        .map(|id| {
            let value = graph
                .node(&id)
                .expect("node_ids() should only yield live nodes");
            Ok(GraphJsonNode {
                v: id.clone(),
                value: default_label_to_json(value)?,
                parent: graph.parent(&id).map(str::to_string),
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;

    let edges = graph
        .edge_keys()
        .into_iter()
        .map(|key| {
            let value = graph
                .edge_by_key(&key)
                .expect("edge_keys() should only yield live edges");
            Ok(GraphJsonEdge {
                v: key.v,
                w: key.w,
                name: key.name,
                value: default_label_to_json(value)?,
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;

    Ok(GraphJson {
        options: graph.options(),
        value: default_label_to_json(graph.graph())?,
        nodes,
        edges,
    })
}

pub fn read_with_defaults<N, E, G>(json: &GraphJson) -> Result<Graph<N, E, G>, serde_json::Error>
where
    N: Default + DeserializeOwned + 'static,
    E: Default + DeserializeOwned + 'static,
    G: Default + DeserializeOwned,
{
    let mut graph = Graph::new(json.options);
    graph.set_graph(default_label_from_json(json.value.clone())?);

    for node in &json.nodes {
        graph.set_node(node.v.clone(), default_label_from_json(node.value.clone())?);
        if let Some(parent) = node.parent.as_deref().filter(|parent| !parent.is_empty()) {
            graph.set_parent_ref(&node.v, parent);
        }
    }

    for edge in &json.edges {
        graph.set_edge_named(
            edge.v.clone(),
            edge.w.clone(),
            edge.name.clone(),
            Some(default_label_from_json(edge.value.clone())?),
        );
    }

    Ok(graph)
}

fn option_label_to_json<T>(value: &Option<T>) -> Result<Option<JsonValue>, serde_json::Error>
where
    T: Serialize,
{
    match value {
        Some(value) => Ok(Some(serde_json::to_value(value)?)),
        None => Ok(None),
    }
}

fn option_label_from_json<T>(value: Option<JsonValue>) -> Result<Option<T>, serde_json::Error>
where
    T: DeserializeOwned,
{
    match value {
        Some(value) => serde_json::from_value(value).map(Some),
        None => Ok(None),
    }
}

fn default_label_to_json<T>(value: &T) -> Result<Option<JsonValue>, serde_json::Error>
where
    T: Serialize,
{
    let value = serde_json::to_value(value)?;
    Ok((value != JsonValue::Null).then_some(value))
}

fn default_label_from_json<T>(value: Option<JsonValue>) -> Result<T, serde_json::Error>
where
    T: Default + DeserializeOwned,
{
    match value {
        Some(value) => serde_json::from_value(value),
        None => Ok(T::default()),
    }
}
