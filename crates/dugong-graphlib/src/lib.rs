//! Graph container APIs used by `dugong`.
//!
//! Baseline: `@dagrejs/graphlib` (see `docs/adr/0044-dugong-parity-and-testing.md`).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, Default)]
pub struct GraphOptions {
    pub multigraph: bool,
    pub compound: bool,
}

#[derive(Debug, Clone)]
pub struct EdgeKey {
    pub v: String,
    pub w: String,
    pub name: Option<String>,
}

impl EdgeKey {
    pub fn new(
        v: impl Into<String>,
        w: impl Into<String>,
        name: Option<impl Into<String>>,
    ) -> Self {
        Self {
            v: v.into(),
            w: w.into(),
            name: name.map(Into::into),
        }
    }
}

impl PartialEq for EdgeKey {
    fn eq(&self, other: &Self) -> bool {
        self.v == other.v && self.w == other.w && self.name == other.name
    }
}

impl Eq for EdgeKey {}

impl Hash for EdgeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.v.hash(state);
        self.w.hash(state);
        self.name.hash(state);
    }
}

#[derive(Debug, Clone)]
struct NodeEntry<N> {
    id: String,
    label: N,
}

#[derive(Debug, Clone)]
struct EdgeEntry<E> {
    key: EdgeKey,
    label: E,
}

pub struct Graph<N, E, G>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    options: GraphOptions,

    graph_label: G,
    default_node_label: Box<dyn Fn() -> N + Send + Sync>,
    default_edge_label: Box<dyn Fn() -> E + Send + Sync>,

    nodes: Vec<NodeEntry<N>>,
    node_index: HashMap<String, usize>,

    edges: Vec<EdgeEntry<E>>,
    edge_index: HashMap<EdgeKey, usize>,

    parent: HashMap<String, String>,
    children: HashMap<String, Vec<String>>,
}

impl<N, E, G> Graph<N, E, G>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    pub fn new(options: GraphOptions) -> Self {
        Self {
            options,
            graph_label: G::default(),
            default_node_label: Box::new(N::default),
            default_edge_label: Box::new(E::default),
            nodes: Vec::new(),
            node_index: HashMap::new(),
            edges: Vec::new(),
            edge_index: HashMap::new(),
            parent: HashMap::new(),
            children: HashMap::new(),
        }
    }

    pub fn options(&self) -> GraphOptions {
        self.options
    }

    pub fn set_graph(&mut self, label: G) -> &mut Self {
        self.graph_label = label;
        self
    }

    pub fn graph(&self) -> &G {
        &self.graph_label
    }

    pub fn graph_mut(&mut self) -> &mut G {
        &mut self.graph_label
    }

    pub fn set_default_node_label<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() -> N + Send + Sync + 'static,
    {
        self.default_node_label = Box::new(f);
        self
    }

    pub fn set_default_edge_label<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() -> E + Send + Sync + 'static,
    {
        self.default_edge_label = Box::new(f);
        self
    }

    pub fn has_node(&self, id: &str) -> bool {
        self.node_index.contains_key(id)
    }

    pub fn set_node(&mut self, id: impl Into<String>, label: N) -> &mut Self {
        let id = id.into();
        if let Some(&idx) = self.node_index.get(&id) {
            self.nodes[idx].label = label;
            return self;
        }
        let idx = self.nodes.len();
        self.nodes.push(NodeEntry {
            id: id.clone(),
            label,
        });
        self.node_index.insert(id, idx);
        self
    }

    pub fn ensure_node(&mut self, id: impl Into<String>) -> &mut Self {
        let id = id.into();
        if self.node_index.contains_key(&id) {
            return self;
        }
        let label = (self.default_node_label)();
        self.set_node(id, label)
    }

    pub fn node(&self, id: &str) -> Option<&N> {
        self.node_index.get(id).map(|&idx| &self.nodes[idx].label)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut N> {
        self.node_index
            .get(id)
            .copied()
            .map(move |idx| &mut self.nodes[idx].label)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|n| n.id.as_str())
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn edges(&self) -> impl Iterator<Item = &EdgeKey> {
        self.edges.iter().map(|e| &e.key)
    }

    pub fn set_edge(&mut self, v: impl Into<String>, w: impl Into<String>) -> &mut Self {
        self.set_edge_named(v, w, None::<String>, None)
    }

    pub fn set_edge_with_label(
        &mut self,
        v: impl Into<String>,
        w: impl Into<String>,
        label: E,
    ) -> &mut Self {
        self.set_edge_named(v, w, None::<String>, Some(label))
    }

    pub fn set_edge_named(
        &mut self,
        v: impl Into<String>,
        w: impl Into<String>,
        name: Option<impl Into<String>>,
        label: Option<E>,
    ) -> &mut Self {
        let v = v.into();
        let w = w.into();
        self.ensure_node(v.clone());
        self.ensure_node(w.clone());

        let name = if self.options.multigraph {
            name.map(Into::into)
        } else {
            None
        };
        let key = EdgeKey { v, w, name };

        if let Some(&idx) = self.edge_index.get(&key) {
            if let Some(label) = label {
                self.edges[idx].label = label;
            }
            return self;
        }

        let idx = self.edges.len();
        self.edges.push(EdgeEntry {
            key: key.clone(),
            label: label.unwrap_or_else(|| (self.default_edge_label)()),
        });
        self.edge_index.insert(key, idx);
        self
    }

    pub fn has_edge(&self, v: &str, w: &str, name: Option<&str>) -> bool {
        let key = EdgeKey {
            v: v.to_string(),
            w: w.to_string(),
            name: if self.options.multigraph {
                name.map(|s| s.to_string())
            } else {
                None
            },
        };
        self.edge_index.contains_key(&key)
    }

    pub fn edge(&self, v: &str, w: &str, name: Option<&str>) -> Option<&E> {
        let key = EdgeKey {
            v: v.to_string(),
            w: w.to_string(),
            name: if self.options.multigraph {
                name.map(|s| s.to_string())
            } else {
                None
            },
        };
        self.edge_index.get(&key).map(|&idx| &self.edges[idx].label)
    }

    pub fn edge_mut(&mut self, v: &str, w: &str, name: Option<&str>) -> Option<&mut E> {
        let key = EdgeKey {
            v: v.to_string(),
            w: w.to_string(),
            name: if self.options.multigraph {
                name.map(|s| s.to_string())
            } else {
                None
            },
        };
        self.edge_index
            .get(&key)
            .copied()
            .map(move |idx| &mut self.edges[idx].label)
    }

    pub fn set_parent(&mut self, child: impl Into<String>, parent: impl Into<String>) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        let child = child.into();
        let parent = parent.into();
        self.ensure_node(child.clone());
        self.ensure_node(parent.clone());
        self.parent.insert(child.clone(), parent.clone());
        self.children.entry(parent).or_default().push(child);
        self
    }

    pub fn parent(&self, child: &str) -> Option<&str> {
        self.parent.get(child).map(|s| s.as_str())
    }

    pub fn children(&self, parent: &str) -> Vec<&str> {
        self.children
            .get(parent)
            .map(|v| v.iter().map(|s| s.as_str()).collect::<Vec<_>>())
            .unwrap_or_default()
    }
}
