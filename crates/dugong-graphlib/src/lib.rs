//! Graph container APIs used by `dugong`.
//!
//! Baseline: `@dagrejs/graphlib` (see `docs/adr/0044-dugong-parity-and-testing.md`).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy)]
pub struct GraphOptions {
    pub multigraph: bool,
    pub compound: bool,
    pub directed: bool,
}

impl Default for GraphOptions {
    fn default() -> Self {
        Self {
            multigraph: false,
            compound: false,
            directed: true,
        }
    }
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
    fn canonicalize_endpoints(&self, v: String, w: String) -> (String, String) {
        if self.options.directed || v <= w {
            (v, w)
        } else {
            (w, v)
        }
    }

    fn canonicalize_name(&self, name: Option<String>) -> Option<String> {
        if self.options.multigraph { name } else { None }
    }

    fn canonicalize_key(&self, mut key: EdgeKey) -> EdgeKey {
        if !self.options.directed && key.v > key.w {
            (key.v, key.w) = (key.w, key.v);
        }
        key.name = self.canonicalize_name(key.name);
        key
    }

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

    pub fn is_multigraph(&self) -> bool {
        self.options.multigraph
    }

    pub fn is_compound(&self) -> bool {
        self.options.compound
    }

    pub fn is_directed(&self) -> bool {
        self.options.directed
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
        let (v, w) = self.canonicalize_endpoints(v.into(), w.into());
        self.ensure_node(v.clone());
        self.ensure_node(w.clone());

        let name = self.canonicalize_name(name.map(Into::into));
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
        let (v, w) = self.canonicalize_endpoints(v.to_string(), w.to_string());
        let key = EdgeKey {
            v,
            w,
            name: self.canonicalize_name(name.map(|s| s.to_string())),
        };
        self.edge_index.contains_key(&key)
    }

    pub fn edge(&self, v: &str, w: &str, name: Option<&str>) -> Option<&E> {
        let (v, w) = self.canonicalize_endpoints(v.to_string(), w.to_string());
        let key = EdgeKey {
            v,
            w,
            name: self.canonicalize_name(name.map(|s| s.to_string())),
        };
        self.edge_index.get(&key).map(|&idx| &self.edges[idx].label)
    }

    pub fn edge_mut(&mut self, v: &str, w: &str, name: Option<&str>) -> Option<&mut E> {
        let (v, w) = self.canonicalize_endpoints(v.to_string(), w.to_string());
        let key = EdgeKey {
            v,
            w,
            name: self.canonicalize_name(name.map(|s| s.to_string())),
        };
        self.edge_index
            .get(&key)
            .copied()
            .map(move |idx| &mut self.edges[idx].label)
    }

    pub fn edge_by_key(&self, key: &EdgeKey) -> Option<&E> {
        let key = self.canonicalize_key(key.clone());
        self.edge_index.get(&key).map(|&idx| &self.edges[idx].label)
    }

    pub fn edge_mut_by_key(&mut self, key: &EdgeKey) -> Option<&mut E> {
        let key = self.canonicalize_key(key.clone());
        self.edge_index
            .get(&key)
            .copied()
            .map(move |idx| &mut self.edges[idx].label)
    }

    pub fn remove_edge_key(&mut self, key: &EdgeKey) -> bool {
        let key = self.canonicalize_key(key.clone());
        let Some(idx) = self.edge_index.remove(&key) else {
            return false;
        };
        self.edges.remove(idx);
        self.edge_index.clear();
        for (i, e) in self.edges.iter().enumerate() {
            self.edge_index.insert(e.key.clone(), i);
        }
        true
    }

    pub fn remove_edge(&mut self, v: &str, w: &str, name: Option<&str>) -> bool {
        let (v, w) = self.canonicalize_endpoints(v.to_string(), w.to_string());
        let key = EdgeKey {
            v,
            w,
            name: self.canonicalize_name(name.map(|s| s.to_string())),
        };
        self.remove_edge_key(&key)
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
        if !self.options.directed {
            return self.adjacent_nodes(v);
        }
        self.edges
            .iter()
            .filter(|e| e.key.v == v)
            .map(|e| e.key.w.as_str())
            .collect()
    }

    pub fn predecessors(&self, v: &str) -> Vec<&str> {
        if !self.options.directed {
            return self.adjacent_nodes(v);
        }
        self.edges
            .iter()
            .filter(|e| e.key.w == v)
            .map(|e| e.key.v.as_str())
            .collect()
    }

    pub fn neighbors(&self, v: &str) -> Vec<&str> {
        if !self.options.directed {
            return self.adjacent_nodes(v);
        }
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

    fn adjacent_nodes(&self, v: &str) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for e in &self.edges {
            if e.key.v == v {
                let w = e.key.w.as_str();
                if !out.iter().any(|x| x == &w) {
                    out.push(w);
                }
            } else if e.key.w == v {
                let u = e.key.v.as_str();
                if !out.iter().any(|x| x == &u) {
                    out.push(u);
                }
            }
        }
        out
    }

    pub fn out_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        if self.options.directed {
            return self
                .edges
                .iter()
                .filter(|e| e.key.v == v && w.is_none_or(|w| e.key.w == w))
                .map(|e| e.key.clone())
                .collect();
        }

        self.edges
            .iter()
            .filter(|e| {
                if e.key.v == v {
                    w.is_none_or(|w| e.key.w == w)
                } else if e.key.w == v {
                    w.is_none_or(|w| e.key.v == w)
                } else {
                    false
                }
            })
            .map(|e| e.key.clone())
            .collect()
    }

    pub fn in_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        if self.options.directed {
            return self
                .edges
                .iter()
                .filter(|e| e.key.w == v && w.is_none_or(|w| e.key.v == w))
                .map(|e| e.key.clone())
                .collect();
        }
        self.out_edges(v, w)
    }

    pub fn set_edge_key(&mut self, key: EdgeKey, label: E) -> &mut Self {
        let key = self.canonicalize_key(key);
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

    pub fn clear_parent(&mut self, child: &str) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        if let Some(prev) = self.parent.remove(child) {
            if let Some(ch) = self.children.get_mut(&prev) {
                ch.retain(|c| c != child);
            }
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

    pub fn sources(&self) -> Vec<&str> {
        if !self.options.directed {
            return self.nodes().collect();
        }
        self.nodes
            .iter()
            .filter(|n| self.in_edges(&n.id, None).is_empty())
            .map(|n| n.id.as_str())
            .collect()
    }

    pub fn node_edges(&self, v: &str) -> Vec<EdgeKey> {
        let mut out: Vec<EdgeKey> = Vec::new();
        let mut seen: std::collections::HashSet<EdgeKey> = std::collections::HashSet::new();
        for e in &self.edges {
            if (e.key.v == v || e.key.w == v) && seen.insert(e.key.clone()) {
                out.push(e.key.clone());
            }
        }
        out
    }
}

pub mod alg {
    use super::Graph;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    pub fn preorder<N, E, G>(g: &Graph<N, E, G>, roots: &[&str]) -> Vec<String>
    where
        N: Default + 'static,
        E: Default + 'static,
        G: Default,
    {
        fn dfs<N, E, G>(
            g: &Graph<N, E, G>,
            v: &str,
            visited: &mut BTreeSet<String>,
            out: &mut Vec<String>,
        ) where
            N: Default + 'static,
            E: Default + 'static,
            G: Default,
        {
            if !visited.insert(v.to_string()) {
                return;
            }
            out.push(v.to_string());
            for w in g.successors(v) {
                dfs(g, w, visited, out);
            }
        }

        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut out: Vec<String> = Vec::new();
        for r in roots {
            dfs(g, r, &mut visited, &mut out);
        }
        out
    }

    pub fn postorder<N, E, G>(g: &Graph<N, E, G>, roots: &[&str]) -> Vec<String>
    where
        N: Default + 'static,
        E: Default + 'static,
        G: Default,
    {
        fn dfs<N, E, G>(
            g: &Graph<N, E, G>,
            v: &str,
            visited: &mut BTreeSet<String>,
            out: &mut Vec<String>,
        ) where
            N: Default + 'static,
            E: Default + 'static,
            G: Default,
        {
            if !visited.insert(v.to_string()) {
                return;
            }
            for w in g.successors(v) {
                dfs(g, w, visited, out);
            }
            out.push(v.to_string());
        }

        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut out: Vec<String> = Vec::new();
        for r in roots {
            dfs(g, r, &mut visited, &mut out);
        }
        out
    }

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
        struct Tarjan<'a, N, E, G>
        where
            N: Default + 'static,
            E: Default + 'static,
            G: Default,
        {
            g: &'a Graph<N, E, G>,
            index: usize,
            stack: Vec<String>,
            on_stack: BTreeSet<String>,
            indices: BTreeMap<String, usize>,
            lowlink: BTreeMap<String, usize>,
            sccs: Vec<Vec<String>>,
        }

        impl<N, E, G> Tarjan<'_, N, E, G>
        where
            N: Default + 'static,
            E: Default + 'static,
            G: Default,
        {
            fn strongconnect(&mut self, v: &str) {
                self.indices.insert(v.to_string(), self.index);
                self.lowlink.insert(v.to_string(), self.index);
                self.index += 1;
                self.stack.push(v.to_string());
                self.on_stack.insert(v.to_string());

                for w in self.g.successors(v) {
                    if !self.indices.contains_key(w) {
                        self.strongconnect(w);
                        let v_low = self
                            .lowlink
                            .get(v)
                            .copied()
                            .expect("tarjan lowlink missing for v");
                        let w_low = self
                            .lowlink
                            .get(w)
                            .copied()
                            .expect("tarjan lowlink missing for w");
                        self.lowlink.insert(v.to_string(), v_low.min(w_low));
                    } else if self.on_stack.contains(w) {
                        let v_low = self
                            .lowlink
                            .get(v)
                            .copied()
                            .expect("tarjan lowlink missing for v");
                        let w_idx = self
                            .indices
                            .get(w)
                            .copied()
                            .expect("tarjan index missing for w");
                        self.lowlink.insert(v.to_string(), v_low.min(w_idx));
                    }
                }

                let v_low = self
                    .lowlink
                    .get(v)
                    .copied()
                    .expect("tarjan lowlink missing for v");
                let v_idx = self
                    .indices
                    .get(v)
                    .copied()
                    .expect("tarjan index missing for v");
                if v_low == v_idx {
                    let mut scc: Vec<String> = Vec::new();
                    loop {
                        let w = self.stack.pop().expect("tarjan stack underflow");
                        self.on_stack.remove(&w);
                        scc.push(w.clone());
                        if w == v {
                            break;
                        }
                    }
                    self.sccs.push(scc);
                }
            }
        }

        let mut tarjan = Tarjan {
            g,
            index: 0,
            stack: Vec::new(),
            on_stack: BTreeSet::new(),
            indices: BTreeMap::new(),
            lowlink: BTreeMap::new(),
            sccs: Vec::new(),
        };

        for v in &node_ids {
            if !tarjan.indices.contains_key(v) {
                tarjan.strongconnect(v);
            }
        }

        let mut cycles: Vec<Vec<String>> = Vec::new();
        for mut scc in tarjan.sccs {
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
