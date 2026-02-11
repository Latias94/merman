//! Graph container APIs used by `dugong`.
//!
//! Baseline: `@dagrejs/graphlib` (see `docs/adr/0044-dugong-parity-and-testing.md`).
//!
//! This module contains the core `Graph` container plus a small set of helper algorithms
//! re-exported as `dugong_graphlib::alg` for Dagre compatibility.

use rustc_hash::FxBuildHasher;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};

type HashMap<K, V> = hashbrown::HashMap<K, V, FxBuildHasher>;
type HashSet<T> = hashbrown::HashSet<T, FxBuildHasher>;

#[derive(Debug, Clone)]
struct DirectedAdjCache {
    generation: u64,
    out: Vec<Vec<usize>>,
    in_: Vec<Vec<usize>>,
}

#[derive(Clone, Copy, Hash)]
struct EdgeKeyView<'a> {
    v: &'a str,
    w: &'a str,
    name: Option<&'a str>,
}

impl<'a> hashbrown::Equivalent<EdgeKey> for EdgeKeyView<'a> {
    fn equivalent(&self, key: &EdgeKey) -> bool {
        key.v == self.v && key.w == self.w && key.name.as_deref() == self.name
    }
}

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

    // Many Dagre algorithms call `predecessors` / `successors` / `in_edges` / `out_edges`
    // repeatedly. Scanning `self.edges` each time is O(E) per query and dominates runtime
    // for large graphs. We keep a lazily rebuilt adjacency cache for directed graphs.
    //
    // Note: This uses interior mutability to keep query APIs on `&self`.
    directed_adj_gen: u64,
    directed_adj_cache: RefCell<Option<DirectedAdjCache>>,
}

impl<N, E, G> Graph<N, E, G>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    fn invalidate_directed_adj(&mut self) {
        if !self.options.directed {
            return;
        }
        self.directed_adj_gen = self.directed_adj_gen.wrapping_add(1);
        *self.directed_adj_cache.get_mut() = None;
    }

    fn ensure_directed_adj<'a>(&'a self) -> std::cell::RefMut<'a, DirectedAdjCache> {
        debug_assert!(self.options.directed);
        let generation = self.directed_adj_gen;
        let mut cache = self.directed_adj_cache.borrow_mut();
        let stale = cache
            .as_ref()
            .map(|c| c.generation != generation)
            .unwrap_or(true);
        if stale {
            let mut out: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
            let mut in_: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
            for (edge_idx, e) in self.edges.iter().enumerate() {
                let Some(&v_idx) = self.node_index.get(&e.key.v) else {
                    continue;
                };
                let Some(&w_idx) = self.node_index.get(&e.key.w) else {
                    continue;
                };
                out[v_idx].push(edge_idx);
                in_[w_idx].push(edge_idx);
            }
            *cache = Some(DirectedAdjCache {
                generation,
                out,
                in_,
            });
        }
        std::cell::RefMut::map(cache, |c| {
            c.as_mut()
                .expect("directed adjacency cache should be present after ensure")
        })
    }

    fn edge_key_view<'a>(&self, v: &'a str, w: &'a str, name: Option<&'a str>) -> EdgeKeyView<'a> {
        let (v, w) = if self.options.directed || v <= w {
            (v, w)
        } else {
            (w, v)
        };
        let name = if self.options.multigraph { name } else { None };
        EdgeKeyView { v, w, name }
    }

    fn edge_key_view_from_key<'a>(&self, key: &'a EdgeKey) -> EdgeKeyView<'a> {
        let mut v = key.v.as_str();
        let mut w = key.w.as_str();
        if !self.options.directed && v > w {
            (v, w) = (w, v);
        }
        let name = if self.options.multigraph {
            key.name.as_deref()
        } else {
            None
        };
        EdgeKeyView { v, w, name }
    }

    fn edge_index_of_view(&self, view: EdgeKeyView<'_>) -> Option<usize> {
        self.edge_index.get(&view).copied()
    }

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
            node_index: HashMap::default(),
            edges: Vec::new(),
            edge_index: HashMap::default(),
            parent: HashMap::default(),
            children: HashMap::default(),
            directed_adj_gen: 0,
            directed_adj_cache: RefCell::new(None),
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
        self.invalidate_directed_adj();
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

    pub fn for_each_edge<F>(&self, mut f: F)
    where
        F: FnMut(&EdgeKey, &E),
    {
        for e in &self.edges {
            f(&e.key, &e.label);
        }
    }

    pub fn for_each_edge_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&EdgeKey, &mut E),
    {
        for e in &mut self.edges {
            f(&e.key, &mut e.label);
        }
    }

    pub fn for_each_node<F>(&self, mut f: F)
    where
        F: FnMut(&str, &N),
    {
        for n in &self.nodes {
            f(&n.id, &n.label);
        }
    }

    pub fn for_each_node_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&str, &mut N),
    {
        for n in &mut self.nodes {
            f(&n.id, &mut n.label);
        }
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

        self.invalidate_directed_adj();
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
        let view = self.edge_key_view(v, w, name);
        self.edge_index_of_view(view).is_some()
    }

    pub fn edge(&self, v: &str, w: &str, name: Option<&str>) -> Option<&E> {
        let view = self.edge_key_view(v, w, name);
        let idx = self.edge_index_of_view(view)?;
        Some(&self.edges[idx].label)
    }

    pub fn edge_mut(&mut self, v: &str, w: &str, name: Option<&str>) -> Option<&mut E> {
        let view = self.edge_key_view(v, w, name);
        let idx = self.edge_index_of_view(view)?;
        Some(&mut self.edges[idx].label)
    }

    pub fn edge_by_key(&self, key: &EdgeKey) -> Option<&E> {
        let view = self.edge_key_view_from_key(key);
        let idx = self.edge_index_of_view(view)?;
        Some(&self.edges[idx].label)
    }

    pub fn edge_mut_by_key(&mut self, key: &EdgeKey) -> Option<&mut E> {
        let view = self.edge_key_view_from_key(key);
        let idx = self.edge_index_of_view(view)?;
        Some(&mut self.edges[idx].label)
    }

    fn remove_edge_at_index(&mut self, idx: usize) {
        self.invalidate_directed_adj();
        let _ = self.edge_index.remove_entry(&self.edges[idx].key);
        self.edges.remove(idx);
        for i in idx..self.edges.len() {
            let k = &self.edges[i].key;
            if let Some(v) = self.edge_index.get_mut(k) {
                *v = i;
            }
        }
    }

    pub fn remove_edge_key(&mut self, key: &EdgeKey) -> bool {
        let view = self.edge_key_view_from_key(key);
        let Some(idx) = self.edge_index_of_view(view) else {
            return false;
        };
        self.remove_edge_at_index(idx);
        true
    }

    pub fn remove_edge(&mut self, v: &str, w: &str, name: Option<&str>) -> bool {
        let view = self.edge_key_view(v, w, name);
        let Some(idx) = self.edge_index_of_view(view) else {
            return false;
        };
        self.remove_edge_at_index(idx);
        true
    }

    pub fn remove_node(&mut self, id: &str) -> bool {
        let Some(idx) = self.node_index.remove(id) else {
            return false;
        };

        self.invalidate_directed_adj();
        self.nodes.remove(idx);
        for i in idx..self.nodes.len() {
            let node_id = self.nodes[i].id.as_str();
            if let Some(v) = self.node_index.get_mut(node_id) {
                *v = i;
            }
        }

        // Remove incident edges.
        let mut removed_any_edge = false;
        for e in &self.edges {
            if e.key.v == id || e.key.w == id {
                removed_any_edge = true;
                let _ = self.edge_index.remove_entry(&e.key);
            }
        }
        if removed_any_edge {
            self.edges.retain(|e| e.key.v != id && e.key.w != id);
            for (i, e) in self.edges.iter().enumerate() {
                if let Some(v) = self.edge_index.get_mut(&e.key) {
                    *v = i;
                }
            }
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
        let Some(&v_idx) = self.node_index.get(v) else {
            return Vec::new();
        };
        let cache = self.ensure_directed_adj();
        let out_edges = &cache.out[v_idx];
        let mut out: Vec<&str> = Vec::with_capacity(out_edges.len());
        for &edge_idx in out_edges {
            out.push(self.edges[edge_idx].key.w.as_str());
        }
        out
    }

    pub fn predecessors(&self, v: &str) -> Vec<&str> {
        if !self.options.directed {
            return self.adjacent_nodes(v);
        }
        let Some(&v_idx) = self.node_index.get(v) else {
            return Vec::new();
        };
        let cache = self.ensure_directed_adj();
        let in_edges = &cache.in_[v_idx];
        let mut out: Vec<&str> = Vec::with_capacity(in_edges.len());
        for &edge_idx in in_edges {
            out.push(self.edges[edge_idx].key.v.as_str());
        }
        out
    }

    pub fn first_successor<'a>(&'a self, v: &str) -> Option<&'a str> {
        if !self.options.directed {
            return self.adjacent_nodes(v).into_iter().next();
        }
        let &v_idx = self.node_index.get(v)?;
        let w = {
            let cache = self.ensure_directed_adj();
            let edge_idx = *cache.out[v_idx].first()?;
            self.edges[edge_idx].key.w.as_str()
        };
        Some(w)
    }

    pub fn first_predecessor<'a>(&'a self, v: &str) -> Option<&'a str> {
        if !self.options.directed {
            return self.adjacent_nodes(v).into_iter().next();
        }
        let &v_idx = self.node_index.get(v)?;
        let u = {
            let cache = self.ensure_directed_adj();
            let edge_idx = *cache.in_[v_idx].first()?;
            self.edges[edge_idx].key.v.as_str()
        };
        Some(u)
    }

    pub fn extend_successors<'a>(&'a self, v: &str, out: &mut Vec<&'a str>) {
        if !self.options.directed {
            out.extend(self.adjacent_nodes(v));
            return;
        }
        let Some(&v_idx) = self.node_index.get(v) else {
            return;
        };
        let cache = self.ensure_directed_adj();
        out.reserve(cache.out[v_idx].len());
        for &edge_idx in &cache.out[v_idx] {
            out.push(self.edges[edge_idx].key.w.as_str());
        }
    }

    pub fn extend_predecessors<'a>(&'a self, v: &str, out: &mut Vec<&'a str>) {
        if !self.options.directed {
            out.extend(self.adjacent_nodes(v));
            return;
        }
        let Some(&v_idx) = self.node_index.get(v) else {
            return;
        };
        let cache = self.ensure_directed_adj();
        out.reserve(cache.in_[v_idx].len());
        for &edge_idx in &cache.in_[v_idx] {
            out.push(self.edges[edge_idx].key.v.as_str());
        }
    }

    pub fn for_each_successor<'a, F>(&'a self, v: &str, mut f: F)
    where
        F: FnMut(&'a str),
    {
        if !self.options.directed {
            for w in self.adjacent_nodes(v) {
                f(w);
            }
            return;
        }
        let Some(&v_idx) = self.node_index.get(v) else {
            return;
        };
        let cache = self.ensure_directed_adj();
        for &edge_idx in &cache.out[v_idx] {
            f(self.edges[edge_idx].key.w.as_str());
        }
    }

    pub fn for_each_predecessor<'a, F>(&'a self, v: &str, mut f: F)
    where
        F: FnMut(&'a str),
    {
        if !self.options.directed {
            for u in self.adjacent_nodes(v) {
                f(u);
            }
            return;
        }
        let Some(&v_idx) = self.node_index.get(v) else {
            return;
        };
        let cache = self.ensure_directed_adj();
        for &edge_idx in &cache.in_[v_idx] {
            f(self.edges[edge_idx].key.v.as_str());
        }
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
            let Some(&v_idx) = self.node_index.get(v) else {
                return Vec::new();
            };
            let cache = self.ensure_directed_adj();
            let out_edges = &cache.out[v_idx];
            let mut out: Vec<EdgeKey> = Vec::with_capacity(out_edges.len());
            for &edge_idx in out_edges {
                let e = &self.edges[edge_idx];
                if w.is_none_or(|w| e.key.w == w) {
                    out.push(e.key.clone());
                }
            }
            return out;
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
            let Some(&v_idx) = self.node_index.get(v) else {
                return Vec::new();
            };
            let cache = self.ensure_directed_adj();
            let in_edges = &cache.in_[v_idx];
            let mut out: Vec<EdgeKey> = Vec::with_capacity(in_edges.len());
            for &edge_idx in in_edges {
                let e = &self.edges[edge_idx];
                if w.is_none_or(|w| e.key.v == w) {
                    out.push(e.key.clone());
                }
            }
            return out;
        }
        self.out_edges(v, w)
    }

    pub fn for_each_out_edge<F>(&self, v: &str, w: Option<&str>, mut f: F)
    where
        F: FnMut(&EdgeKey, &E),
    {
        if self.options.directed {
            let Some(&v_idx) = self.node_index.get(v) else {
                return;
            };
            let cache = self.ensure_directed_adj();
            for &edge_idx in &cache.out[v_idx] {
                let e = &self.edges[edge_idx];
                if w.is_none_or(|w| e.key.w == w) {
                    f(&e.key, &e.label);
                }
            }
            return;
        }

        for e in &self.edges {
            if e.key.v == v {
                if w.is_none_or(|w| e.key.w == w) {
                    f(&e.key, &e.label);
                }
            } else if e.key.w == v {
                if w.is_none_or(|w| e.key.v == w) {
                    f(&e.key, &e.label);
                }
            }
        }
    }

    pub fn for_each_in_edge<F>(&self, v: &str, w: Option<&str>, mut f: F)
    where
        F: FnMut(&EdgeKey, &E),
    {
        if self.options.directed {
            let Some(&v_idx) = self.node_index.get(v) else {
                return;
            };
            let cache = self.ensure_directed_adj();
            for &edge_idx in &cache.in_[v_idx] {
                let e = &self.edges[edge_idx];
                if w.is_none_or(|w| e.key.v == w) {
                    f(&e.key, &e.label);
                }
            }
            return;
        }

        self.for_each_out_edge(v, w, f);
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
        let mut seen: HashSet<EdgeKey> = HashSet::default();
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
                        let Some(v_low) = self.lowlink.get(v).copied() else {
                            debug_assert!(false, "tarjan lowlink missing for v");
                            continue;
                        };
                        let Some(w_low) = self.lowlink.get(w).copied() else {
                            debug_assert!(false, "tarjan lowlink missing for w");
                            continue;
                        };
                        self.lowlink.insert(v.to_string(), v_low.min(w_low));
                    } else if self.on_stack.contains(w) {
                        let Some(v_low) = self.lowlink.get(v).copied() else {
                            debug_assert!(false, "tarjan lowlink missing for v");
                            continue;
                        };
                        let Some(w_idx) = self.indices.get(w).copied() else {
                            debug_assert!(false, "tarjan index missing for w");
                            continue;
                        };
                        self.lowlink.insert(v.to_string(), v_low.min(w_idx));
                    }
                }

                let Some(v_low) = self.lowlink.get(v).copied() else {
                    debug_assert!(false, "tarjan lowlink missing for v");
                    return;
                };
                let Some(v_idx) = self.indices.get(v).copied() else {
                    debug_assert!(false, "tarjan index missing for v");
                    return;
                };
                if v_low == v_idx {
                    let mut scc: Vec<String> = Vec::new();
                    loop {
                        let Some(w) = self.stack.pop() else {
                            debug_assert!(false, "tarjan stack underflow");
                            break;
                        };
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
