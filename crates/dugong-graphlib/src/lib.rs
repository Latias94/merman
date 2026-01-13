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

    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.iter().map(|n| n.id.clone()).collect()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn edges(&self) -> impl Iterator<Item = &EdgeKey> {
        self.edges.iter().map(|e| &e.key)
    }

    pub fn edge_keys(&self) -> Vec<EdgeKey> {
        self.edges.iter().map(|e| e.key.clone()).collect()
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

    pub fn set_path(&mut self, nodes: &[&str]) -> &mut Self {
        if nodes.len() < 2 {
            return self;
        }
        for pair in nodes.windows(2) {
            let v = pair[0];
            let w = pair[1];
            self.set_edge(v, w);
        }
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

    pub fn edge_by_key(&self, key: &EdgeKey) -> Option<&E> {
        self.edge_index.get(key).map(|&idx| &self.edges[idx].label)
    }

    pub fn edge_mut_by_key(&mut self, key: &EdgeKey) -> Option<&mut E> {
        self.edge_index
            .get(key)
            .copied()
            .map(move |idx| &mut self.edges[idx].label)
    }

    pub fn remove_edge_key(&mut self, key: &EdgeKey) -> bool {
        let Some(idx) = self.edge_index.remove(key) else {
            return false;
        };
        self.edges.remove(idx);
        self.edge_index.clear();
        for (i, e) in self.edges.iter().enumerate() {
            self.edge_index.insert(e.key.clone(), i);
        }
        true
    }

    pub fn remove_node(&mut self, id: &str) -> bool {
        let Some(idx) = self.node_index.remove(id) else {
            return false;
        };

        self.nodes.remove(idx);
        self.node_index.clear();
        for (i, n) in self.nodes.iter().enumerate() {
            self.node_index.insert(n.id.clone(), i);
        }

        // Remove incident edges.
        let mut removed_keys: Vec<EdgeKey> = Vec::new();
        for e in &self.edges {
            if e.key.v == id || e.key.w == id {
                removed_keys.push(e.key.clone());
            }
        }
        for k in removed_keys {
            let _ = self.remove_edge_key(&k);
        }

        // Remove parent links.
        if let Some(parent) = self.parent.remove(id) {
            if let Some(ch) = self.children.get_mut(&parent) {
                ch.retain(|c| c != id);
            }
        }
        // Remove children mappings.
        if let Some(ch) = self.children.remove(id) {
            for child in ch {
                self.parent.remove(&child);
            }
        }

        true
    }

    pub fn successors(&self, v: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.key.v == v)
            .map(|e| e.key.w.as_str())
            .collect()
    }

    pub fn predecessors(&self, v: &str) -> Vec<&str> {
        self.edges
            .iter()
            .filter(|e| e.key.w == v)
            .map(|e| e.key.v.as_str())
            .collect()
    }

    pub fn neighbors(&self, v: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for w in self.successors(v) {
            if !out.iter().any(|x| x == &w) {
                out.push(w);
            }
        }
        for u in self.predecessors(v) {
            if !out.iter().any(|x| x == &u) {
                out.push(u);
            }
        }
        out
    }

    pub fn out_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        self.edges
            .iter()
            .filter(|e| e.key.v == v && w.map_or(true, |w| e.key.w == w))
            .map(|e| e.key.clone())
            .collect()
    }

    pub fn in_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        self.edges
            .iter()
            .filter(|e| e.key.w == v && w.map_or(true, |w| e.key.v == w))
            .map(|e| e.key.clone())
            .collect()
    }

    pub fn set_edge_key(&mut self, key: EdgeKey, label: E) -> &mut Self {
        self.set_edge_named(key.v, key.w, key.name, Some(label))
    }

    pub fn set_parent(&mut self, child: impl Into<String>, parent: impl Into<String>) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        let child = child.into();
        let parent = parent.into();
        self.ensure_node(child.clone());
        self.ensure_node(parent.clone());
        if let Some(prev) = self.parent.insert(child.clone(), parent.clone()) {
            if let Some(ch) = self.children.get_mut(&prev) {
                ch.retain(|c| c != &child);
            }
        }
        let entry = self.children.entry(parent).or_default();
        if !entry.iter().any(|c| c == &child) {
            entry.push(child);
        }
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

    pub fn children_root(&self) -> Vec<&str> {
        if !self.options.compound {
            return self.nodes().collect();
        }
        self.nodes
            .iter()
            .filter(|n| !self.parent.contains_key(&n.id))
            .map(|n| n.id.as_str())
            .collect()
    }
}

pub mod alg {
    use super::Graph;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    pub fn components<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
    where
        N: Default + 'static,
        E: Default + 'static,
        G: Default,
    {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let ids = g.node_ids();
        let mut out: Vec<Vec<String>> = Vec::new();

        for start in ids {
            if !seen.insert(start.clone()) {
                continue;
            }
            let mut comp: Vec<String> = Vec::new();
            let mut q: VecDeque<String> = VecDeque::new();
            q.push_back(start);
            while let Some(v) = q.pop_front() {
                comp.push(v.clone());
                for n in g.successors(&v) {
                    if seen.insert(n.to_string()) {
                        q.push_back(n.to_string());
                    }
                }
                for n in g.predecessors(&v) {
                    if seen.insert(n.to_string()) {
                        q.push_back(n.to_string());
                    }
                }
            }
            out.push(comp);
        }

        out
    }

    pub fn find_cycles<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
    where
        N: Default + 'static,
        E: Default + 'static,
        G: Default,
    {
        // Strongly connected components (Tarjan). Report SCCs with size > 1, or self-loops.
        let node_ids = g.node_ids();
        let mut index: usize = 0;
        let mut stack: Vec<String> = Vec::new();
        let mut on_stack: BTreeSet<String> = BTreeSet::new();
        let mut indices: BTreeMap<String, usize> = BTreeMap::new();
        let mut lowlink: BTreeMap<String, usize> = BTreeMap::new();
        let mut sccs: Vec<Vec<String>> = Vec::new();

        fn strongconnect<N, E, G>(
            g: &Graph<N, E, G>,
            v: &str,
            index: &mut usize,
            stack: &mut Vec<String>,
            on_stack: &mut BTreeSet<String>,
            indices: &mut BTreeMap<String, usize>,
            lowlink: &mut BTreeMap<String, usize>,
            sccs: &mut Vec<Vec<String>>,
        ) where
            N: Default + 'static,
            E: Default + 'static,
            G: Default,
        {
            indices.insert(v.to_string(), *index);
            lowlink.insert(v.to_string(), *index);
            *index += 1;
            stack.push(v.to_string());
            on_stack.insert(v.to_string());

            for w in g.successors(v) {
                if !indices.contains_key(w) {
                    strongconnect(g, w, index, stack, on_stack, indices, lowlink, sccs);
                    let v_low = lowlink[v];
                    let w_low = lowlink[w];
                    lowlink.insert(v.to_string(), v_low.min(w_low));
                } else if on_stack.contains(w) {
                    let v_low = lowlink[v];
                    let w_idx = indices[w];
                    lowlink.insert(v.to_string(), v_low.min(w_idx));
                }
            }

            if lowlink[v] == indices[v] {
                let mut scc: Vec<String> = Vec::new();
                loop {
                    let w = stack.pop().expect("tarjan stack underflow");
                    on_stack.remove(&w);
                    scc.push(w.clone());
                    if w == v {
                        break;
                    }
                }
                sccs.push(scc);
            }
        }

        for v in &node_ids {
            if !indices.contains_key(v) {
                strongconnect(
                    g,
                    v,
                    &mut index,
                    &mut stack,
                    &mut on_stack,
                    &mut indices,
                    &mut lowlink,
                    &mut sccs,
                );
            }
        }

        let mut cycles: Vec<Vec<String>> = Vec::new();
        for mut scc in sccs {
            if scc.len() > 1 {
                // Deterministic node order: use original insertion order.
                let order: BTreeMap<String, usize> = node_ids
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, v)| (v, i))
                    .collect();
                scc.sort_by_key(|v| order.get(v).copied().unwrap_or(usize::MAX));
                cycles.push(scc);
            } else {
                let v = &scc[0];
                if g.has_edge(v, v, None) || !g.out_edges(v, Some(v)).is_empty() {
                    cycles.push(vec![v.clone()]);
                }
            }
        }

        cycles.sort_by(|a, b| a.first().cmp(&b.first()));
        cycles
    }
}
