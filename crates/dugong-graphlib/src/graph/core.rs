//! Core graph container implementation.

use rustc_hash::FxBuildHasher;
use std::{cell::RefCell, collections::BTreeMap, error::Error, fmt};

use super::adj_cache::{DirectedAdjCache, UndirectedAdjCache};
use super::edge_key::{EdgeKey, EdgeKeyView};
use super::entries::{EdgeEntry, NodeEntry};
use super::options::GraphOptions;

type HashMap<K, V> = hashbrown::HashMap<K, V, FxBuildHasher>;
type HashSet<T> = hashbrown::HashSet<T, FxBuildHasher>;
type DefaultNodeLabel<N> = Box<dyn Fn(&str) -> N + Send + Sync>;
type DefaultEdgeLabel<E> = Box<dyn Fn(&str, &str, Option<&str>) -> E + Send + Sync>;

fn javascript_array_index(value: &str) -> Option<u32> {
    let bytes = value.as_bytes();
    if bytes.is_empty() || (bytes.len() > 1 && bytes[0] == b'0') {
        return None;
    }

    let mut parsed = 0u32;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        parsed = parsed
            .checked_mul(10)?
            .checked_add(u32::from(byte - b'0'))?;
    }
    (parsed != u32::MAX).then_some(parsed)
}

/// Returns whether `value` is an ECMAScript array-index property key.
///
/// Pinned Graphlib stores nodes and compound children in ordinary JavaScript objects, so
/// `Object.keys` enumerates canonical decimal keys in `0..=2^32 - 2` numerically before ordinary
/// strings. Keeping the classifier here gives layout owners one source of truth when they meter
/// the ordered-map work required by those keys.
pub fn is_javascript_array_index(value: &str) -> bool {
    javascript_array_index(value).is_some()
}

const ORDINARY_KEY_TOMBSTONE: usize = usize::MAX;
const ORDINARY_KEY_COMPACT_MIN_SLOTS: usize = 16;

#[derive(Default)]
struct ObjectKeyOrder {
    // Every compound node owns one child bucket, while most buckets never see an array-index ID.
    // Boxing keeps the empty per-node bucket footprint small and pays the extra allocation only
    // for buckets that need numeric ordering.
    #[allow(clippy::box_collection)]
    numeric: Option<Box<BTreeMap<u32, usize>>>,
    ordinary: Vec<usize>,
    ordinary_len: usize,
}

impl ObjectKeyOrder {
    fn len(&self) -> usize {
        self.numeric.as_deref().map_or(0, BTreeMap::len) + self.ordinary_len
    }

    fn numeric_len(&self) -> usize {
        self.numeric.as_deref().map_or(0, BTreeMap::len)
    }

    fn iteration_slot_count(&self) -> usize {
        self.numeric_len() + self.ordinary.len()
    }

    fn reserve_ordinary(&mut self, additional: usize) {
        self.ordinary.reserve(additional);
    }

    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.numeric
            .as_deref()
            .into_iter()
            .flat_map(|numeric| numeric.values().copied())
            .chain(
                self.ordinary
                    .iter()
                    .copied()
                    .filter(|&child_ix| child_ix != ORDINARY_KEY_TOMBSTONE),
            )
    }

    fn insert(
        &mut self,
        child_ix: usize,
        array_index: Option<u32>,
        ordinary_positions: &mut [usize],
    ) {
        if let Some(array_index) = array_index {
            let previous = self
                .numeric
                .get_or_insert_with(|| Box::new(BTreeMap::new()))
                .insert(array_index, child_ix);
            debug_assert!(previous.is_none(), "node IDs are unique graph keys");
            return;
        }

        let position = self.ordinary.len();
        self.ordinary.push(child_ix);
        self.ordinary_len += 1;
        ordinary_positions[child_ix] = position;
    }

    fn remove(
        &mut self,
        child_ix: usize,
        array_index: Option<u32>,
        ordinary_positions: &mut [usize],
    ) -> bool {
        if let Some(array_index) = array_index {
            let Some(numeric) = self.numeric.as_deref_mut() else {
                return false;
            };
            let removed = numeric.remove(&array_index).is_some();
            if numeric.is_empty() {
                self.numeric = None;
            }
            return removed;
        }

        let Some(position) = ordinary_positions.get_mut(child_ix) else {
            return false;
        };
        if *position == ORDINARY_KEY_TOMBSTONE
            || self.ordinary.get(*position).copied() != Some(child_ix)
        {
            return false;
        }
        self.ordinary[*position] = ORDINARY_KEY_TOMBSTONE;
        *position = ORDINARY_KEY_TOMBSTONE;
        self.ordinary_len -= 1;
        self.compact_ordinary_if_sparse(ordinary_positions);
        true
    }

    fn take_ordered(&mut self, ordinary_positions: &mut [usize]) -> Vec<usize> {
        let mut ordered = Vec::with_capacity(self.len());
        if let Some(numeric) = self.numeric.take() {
            ordered.extend(numeric.into_values());
        }
        for child_ix in self.ordinary.drain(..) {
            if child_ix == ORDINARY_KEY_TOMBSTONE {
                continue;
            }
            ordinary_positions[child_ix] = ORDINARY_KEY_TOMBSTONE;
            ordered.push(child_ix);
        }
        self.ordinary_len = 0;
        ordered
    }

    fn compact_ordinary_if_sparse(&mut self, ordinary_positions: &mut [usize]) {
        if self.ordinary.len() < ORDINARY_KEY_COMPACT_MIN_SLOTS
            || self.ordinary.len() <= self.ordinary_len.saturating_mul(2)
        {
            return;
        }
        if self.ordinary_len == 0 {
            self.ordinary.clear();
            return;
        }

        let mut write = 0usize;
        for read in 0..self.ordinary.len() {
            let child_ix = self.ordinary[read];
            if child_ix == ORDINARY_KEY_TOMBSTONE {
                continue;
            }
            self.ordinary[write] = child_ix;
            ordinary_positions[child_ix] = write;
            write += 1;
        }
        self.ordinary.truncate(write);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphError {
    NamedEdgeInNonMultigraph,
    ParentCycle { child_ix: usize, parent_ix: usize },
    ParentBatchRequiresUnparentedChild { child_ix: usize },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedEdgeInNonMultigraph => {
                f.write_str("Cannot set a named edge when is_multigraph = false")
            }
            Self::ParentCycle {
                child_ix,
                parent_ix,
            } => write!(
                f,
                "Setting node index {parent_ix} as parent of node index {child_ix} would create a cycle"
            ),
            Self::ParentBatchRequiresUnparentedChild { child_ix } => write!(
                f,
                "Batch parent assignment requires node index {child_ix} to be unparented and assigned only once"
            ),
        }
    }
}

impl Error for GraphError {}

pub struct Graph<N, E, G>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    options: GraphOptions,

    graph_label: G,
    default_node_label: DefaultNodeLabel<N>,
    default_edge_label: DefaultEdgeLabel<E>,

    nodes: Vec<Option<NodeEntry<N>>>,
    node_len: usize,
    node_index: HashMap<String, usize>,
    node_order: ObjectKeyOrder,
    node_ordinary_positions: Vec<usize>,

    edges: Vec<Option<EdgeEntry<E>>>,
    edge_len: usize,
    edge_index: HashMap<EdgeKey, usize>,

    // Compound graph parent/children relationships.
    //
    // Dagre-style algorithms touch `parent(...)` and `children_iter(...)` frequently; storing
    // these relationships by node index avoids repeated string hashing and avoids allocating
    // duplicate `String`s when setting parent links between already-present nodes.
    parent_ix: Vec<Option<usize>>,
    children_ix: Vec<ObjectKeyOrder>,
    root_children: ObjectKeyOrder,
    child_ordinary_positions: Vec<usize>,

    // Many Dagre algorithms call `predecessors` / `successors` / `in_edges` / `out_edges`
    // repeatedly. Scanning `self.edges` each time is O(E) per query and dominates runtime
    // for large graphs. We keep a lazily rebuilt adjacency cache for directed graphs.
    //
    // Note: This uses interior mutability to keep query APIs on `&self`.
    directed_adj_gen: u64,
    directed_adj_cache: RefCell<Option<DirectedAdjCache>>,

    // Some Dagre helpers (especially `network-simplex`) use undirected trees. Make adjacency
    // queries for undirected graphs fast as well.
    undirected_adj_gen: u64,
    undirected_adj_cache: RefCell<Option<UndirectedAdjCache>>,
}

impl<N, E, G> Graph<N, E, G>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    fn insert_node_entry(&mut self, id: String, label: N) -> usize {
        self.invalidate_adj();
        let idx = self.nodes.len();
        let array_index = javascript_array_index(&id);
        self.nodes.push(Some(NodeEntry {
            id: id.clone(),
            label,
        }));
        self.node_len += 1;
        self.node_index.insert(id, idx);
        self.node_ordinary_positions.push(ORDINARY_KEY_TOMBSTONE);
        self.node_order
            .insert(idx, array_index, &mut self.node_ordinary_positions);
        self.parent_ix.push(None);
        self.children_ix.push(ObjectKeyOrder::default());
        self.child_ordinary_positions.push(ORDINARY_KEY_TOMBSTONE);
        if self.options.compound {
            self.insert_child_in_object_key_order(None, idx);
        }
        idx
    }

    fn remove_node_from_object_key_order(&mut self, node_ix: usize) {
        let array_index = self
            .nodes
            .get(node_ix)
            .and_then(Option::as_ref)
            .and_then(|node| javascript_array_index(&node.id));
        self.node_order
            .remove(node_ix, array_index, &mut self.node_ordinary_positions);
    }

    fn child_array_index(&self, child_ix: usize) -> Option<u32> {
        self.node_id_by_ix(child_ix)
            .and_then(javascript_array_index)
    }

    fn insert_child_in_object_key_order(&mut self, parent_ix: Option<usize>, child_ix: usize) {
        let array_index = self.child_array_index(child_ix);
        let positions = &mut self.child_ordinary_positions;
        match parent_ix {
            Some(parent_ix) => {
                self.children_ix[parent_ix].insert(child_ix, array_index, positions);
            }
            None => self.root_children.insert(child_ix, array_index, positions),
        }
    }

    fn remove_child_from_object(&mut self, parent_ix: Option<usize>, child_ix: usize) {
        let array_index = self.child_array_index(child_ix);
        let positions = &mut self.child_ordinary_positions;
        match parent_ix {
            Some(parent_ix) => {
                self.children_ix[parent_ix].remove(child_ix, array_index, positions);
            }
            None => {
                self.root_children.remove(child_ix, array_index, positions);
            }
        }
    }

    fn take_children_in_object_key_order(&mut self, parent_ix: usize) -> Vec<usize> {
        self.children_ix[parent_ix].take_ordered(&mut self.child_ordinary_positions)
    }

    fn trim_trailing_node_tombstones(&mut self) {
        while matches!(self.nodes.last(), Some(None)) {
            self.nodes.pop();
            self.node_ordinary_positions.pop();
            self.parent_ix.pop();
            self.children_ix.pop();
            self.child_ordinary_positions.pop();
        }
    }

    fn trim_trailing_edge_tombstones(&mut self) {
        while matches!(self.edges.last(), Some(None)) {
            self.edges.pop();
        }
    }

    pub fn compact_if_sparse(&mut self, max_capacity_factor: f64) -> bool {
        // Note: `nodes.len()` / `edges.len()` are slot capacities; `node_len` / `edge_len` are
        // live counts. When a graph is built once and then repeatedly mutated (layout pipelines
        // add/remove dummy nodes), tombstones can accumulate. Callers that reuse graphs can
        // periodically compact to keep memory and cache rebuild costs bounded.
        let nodes_sparse = if self.node_len == 0 {
            !self.nodes.is_empty()
        } else {
            max_capacity_factor > 1.0
                && (self.nodes.len() as f64) > (self.node_len as f64) * max_capacity_factor
        };
        let edges_sparse = if self.edge_len == 0 {
            !self.edges.is_empty()
        } else {
            max_capacity_factor > 1.0
                && (self.edges.len() as f64) > (self.edge_len as f64) * max_capacity_factor
        };

        if !(nodes_sparse || edges_sparse) {
            return false;
        }

        self.compact();
        true
    }

    fn compact(&mut self) {
        self.invalidate_adj();

        if self.node_len == 0 {
            self.nodes.clear();
            self.node_index.clear();
            self.node_order = ObjectKeyOrder::default();
            self.node_ordinary_positions.clear();
            self.node_len = 0;

            self.edges.clear();
            self.edge_index.clear();
            self.edge_len = 0;

            self.parent_ix.clear();
            self.children_ix.clear();
            self.root_children = ObjectKeyOrder::default();
            self.child_ordinary_positions.clear();
            return;
        }

        let old_nodes = std::mem::take(&mut self.nodes);
        let mut old_children_ix = std::mem::take(&mut self.children_ix);
        let mut old_root_children = std::mem::take(&mut self.root_children);
        let mut old_child_ordinary_positions = std::mem::take(&mut self.child_ordinary_positions);
        let old_root_order = old_root_children.take_ordered(&mut old_child_ordinary_positions);
        let old_child_orders = old_children_ix
            .iter_mut()
            .map(|children| children.take_ordered(&mut old_child_ordinary_positions))
            .collect::<Vec<_>>();
        let mut node_remap: Vec<Option<usize>> = vec![None; old_nodes.len()];

        let mut new_nodes: Vec<Option<NodeEntry<N>>> = Vec::with_capacity(self.node_len);
        let mut new_node_index: HashMap<String, usize> = HashMap::default();
        let mut new_node_order = ObjectKeyOrder::default();
        new_node_order.reserve_ordinary(self.node_len);
        let mut new_node_ordinary_positions = Vec::with_capacity(self.node_len);
        for (old_ix, slot) in old_nodes.into_iter().enumerate() {
            let Some(node) = slot else {
                continue;
            };
            let new_ix = new_nodes.len();
            new_node_index.insert(node.id.clone(), new_ix);
            let array_index = javascript_array_index(&node.id);
            new_node_ordinary_positions.push(ORDINARY_KEY_TOMBSTONE);
            new_node_order.insert(new_ix, array_index, &mut new_node_ordinary_positions);
            node_remap[old_ix] = Some(new_ix);
            new_nodes.push(Some(node));
        }

        self.nodes = new_nodes;
        self.node_index = new_node_index;
        self.node_order = new_node_order;
        self.node_ordinary_positions = new_node_ordinary_positions;
        self.node_len = self.nodes.len();

        self.parent_ix = vec![None; self.nodes.len()];
        self.children_ix = (0..self.nodes.len())
            .map(|_| ObjectKeyOrder::default())
            .collect();
        self.root_children = ObjectKeyOrder::default();
        self.child_ordinary_positions = vec![ORDINARY_KEY_TOMBSTONE; self.nodes.len()];
        if self.options.compound {
            for old_child in old_root_order {
                let Some(new_child) = node_remap.get(old_child).copied().flatten() else {
                    continue;
                };
                self.insert_child_in_object_key_order(None, new_child);
            }
            for (old_parent, old_children) in old_child_orders.into_iter().enumerate() {
                let Some(new_parent) = node_remap.get(old_parent).copied().flatten() else {
                    continue;
                };
                for old_child in old_children {
                    let Some(new_child) = node_remap.get(old_child).copied().flatten() else {
                        continue;
                    };
                    self.parent_ix[new_child] = Some(new_parent);
                    self.insert_child_in_object_key_order(Some(new_parent), new_child);
                }
            }
        }

        let old_edges = std::mem::take(&mut self.edges);
        let mut new_edges: Vec<Option<EdgeEntry<E>>> = Vec::with_capacity(self.edge_len);
        let mut new_edge_index: HashMap<EdgeKey, usize> = HashMap::default();
        let mut new_edge_len: usize = 0;

        for slot in old_edges.into_iter() {
            let Some(mut edge) = slot else {
                continue;
            };
            let Some(v_ix) = node_remap.get(edge.v_ix).copied().flatten() else {
                continue;
            };
            let Some(w_ix) = node_remap.get(edge.w_ix).copied().flatten() else {
                continue;
            };
            edge.v_ix = v_ix;
            edge.w_ix = w_ix;

            let new_ix = new_edges.len();
            new_edge_index.insert(edge.key.clone(), new_ix);
            new_edges.push(Some(edge));
            new_edge_len += 1;
        }

        self.edges = new_edges;
        self.edge_index = new_edge_index;
        self.edge_len = new_edge_len;
    }

    fn invalidate_directed_adj(&mut self) {
        if !self.options.directed {
            return;
        }
        self.directed_adj_gen = self.directed_adj_gen.wrapping_add(1);
        *self.directed_adj_cache.get_mut() = None;
    }

    fn invalidate_undirected_adj(&mut self) {
        if self.options.directed {
            return;
        }
        self.undirected_adj_gen = self.undirected_adj_gen.wrapping_add(1);
        *self.undirected_adj_cache.get_mut() = None;
    }

    fn invalidate_adj(&mut self) {
        self.invalidate_directed_adj();
        self.invalidate_undirected_adj();
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
            // Build a CSR-like adjacency index to avoid allocating many small `Vec`s
            // and to keep adjacency queries cache-friendly.
            let node_slots = self.nodes.len();
            let mut out_offsets: Vec<usize> = vec![0; node_slots + 1];
            let mut in_offsets: Vec<usize> = vec![0; node_slots + 1];

            for e in self.edges.iter().filter_map(|e| e.as_ref()) {
                out_offsets[e.v_ix + 1] += 1;
                in_offsets[e.w_ix + 1] += 1;
            }

            for i in 1..=node_slots {
                out_offsets[i] += out_offsets[i - 1];
                in_offsets[i] += in_offsets[i - 1];
            }

            let mut out_edges: Vec<usize> = vec![0; out_offsets[node_slots]];
            let mut in_edges: Vec<usize> = vec![0; in_offsets[node_slots]];
            let mut out_cursors = out_offsets.clone();
            let mut in_cursors = in_offsets.clone();

            for (edge_idx, e) in self.edges.iter().enumerate() {
                let Some(e) = e.as_ref() else {
                    continue;
                };
                let out_pos = out_cursors[e.v_ix];
                out_edges[out_pos] = edge_idx;
                out_cursors[e.v_ix] += 1;

                let in_pos = in_cursors[e.w_ix];
                in_edges[in_pos] = edge_idx;
                in_cursors[e.w_ix] += 1;
            }

            *cache = Some(DirectedAdjCache {
                generation,
                out_offsets,
                out_edges,
                in_offsets,
                in_edges,
            });
        }
        std::cell::RefMut::map(cache, |c| {
            c.get_or_insert_with(|| DirectedAdjCache {
                generation,
                out_offsets: vec![0; self.nodes.len() + 1],
                out_edges: Vec::new(),
                in_offsets: vec![0; self.nodes.len() + 1],
                in_edges: Vec::new(),
            })
        })
    }

    fn ensure_undirected_adj<'a>(&'a self) -> std::cell::RefMut<'a, UndirectedAdjCache> {
        debug_assert!(!self.options.directed);
        let generation = self.undirected_adj_gen;
        let mut cache = self.undirected_adj_cache.borrow_mut();
        let stale = cache
            .as_ref()
            .map(|c| c.generation != generation)
            .unwrap_or(true);
        if stale {
            let node_slots = self.nodes.len();
            let mut offsets: Vec<usize> = vec![0; node_slots + 1];

            for e in self.edges.iter().filter_map(|e| e.as_ref()) {
                offsets[e.v_ix + 1] += 1;
                offsets[e.w_ix + 1] += 1;
            }

            for i in 1..=node_slots {
                offsets[i] += offsets[i - 1];
            }

            let mut edges: Vec<usize> = vec![0; offsets[node_slots]];
            let mut cursors = offsets.clone();
            for (edge_idx, e) in self.edges.iter().enumerate() {
                let Some(e) = e.as_ref() else {
                    continue;
                };
                let v_pos = cursors[e.v_ix];
                edges[v_pos] = edge_idx;
                cursors[e.v_ix] += 1;

                let w_pos = cursors[e.w_ix];
                edges[w_pos] = edge_idx;
                cursors[e.w_ix] += 1;
            }

            *cache = Some(UndirectedAdjCache {
                generation,
                offsets,
                edges,
            });
        }

        std::cell::RefMut::map(cache, |c| {
            c.get_or_insert_with(|| UndirectedAdjCache {
                generation,
                offsets: vec![0; self.nodes.len() + 1],
                edges: Vec::new(),
            })
        })
    }

    fn edge_key_view<'a>(&self, v: &'a str, w: &'a str, name: Option<&'a str>) -> EdgeKeyView<'a> {
        let (v, w) = if self.options.directed || v <= w {
            (v, w)
        } else {
            (w, v)
        };
        EdgeKeyView { v, w, name }
    }

    fn edge_key_view_from_key<'a>(&self, key: &'a EdgeKey) -> EdgeKeyView<'a> {
        let mut v = key.v.as_str();
        let mut w = key.w.as_str();
        if !self.options.directed && v > w {
            (v, w) = (w, v);
        }
        let name = key.name.as_deref();
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

    fn canonicalize_name(&self, name: Option<String>) -> Result<Option<String>, GraphError> {
        if name.is_some() && !self.options.multigraph {
            return Err(GraphError::NamedEdgeInNonMultigraph);
        }
        Ok(name)
    }

    fn canonicalize_key(&self, mut key: EdgeKey) -> Result<EdgeKey, GraphError> {
        if !self.options.directed && key.v > key.w {
            (key.v, key.w) = (key.w, key.v);
        }
        key.name = self.canonicalize_name(key.name)?;
        Ok(key)
    }

    pub fn new(options: GraphOptions) -> Self {
        Self {
            options,
            graph_label: G::default(),
            default_node_label: Box::new(|_| N::default()),
            default_edge_label: Box::new(|_, _, _| E::default()),
            nodes: Vec::new(),
            node_len: 0,
            node_index: HashMap::default(),
            node_order: ObjectKeyOrder::default(),
            node_ordinary_positions: Vec::new(),
            edges: Vec::new(),
            edge_len: 0,
            edge_index: HashMap::default(),
            parent_ix: Vec::new(),
            children_ix: Vec::new(),
            root_children: ObjectKeyOrder::default(),
            child_ordinary_positions: Vec::new(),
            directed_adj_gen: 0,
            directed_adj_cache: RefCell::new(None),
            undirected_adj_gen: 0,
            undirected_adj_cache: RefCell::new(None),
        }
    }

    pub fn with_capacity(
        options: GraphOptions,
        node_capacity: usize,
        edge_capacity: usize,
    ) -> Self {
        let mut g = Self::new(options);
        g.nodes.reserve(node_capacity);
        g.edges.reserve(edge_capacity);
        g.node_index.reserve(node_capacity);
        g.node_order.reserve_ordinary(node_capacity);
        g.node_ordinary_positions.reserve(node_capacity);
        g.edge_index.reserve(edge_capacity);
        g.parent_ix.reserve(node_capacity);
        g.children_ix.reserve(node_capacity);
        if options.compound {
            g.root_children.reserve_ordinary(node_capacity);
        }
        g.child_ordinary_positions.reserve(node_capacity);
        g
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
        self.default_node_label = Box::new(move |_| f());
        self
    }

    pub fn set_default_node_label_with_id<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&str) -> N + Send + Sync + 'static,
    {
        self.default_node_label = Box::new(f);
        self
    }

    pub fn set_default_edge_label<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn() -> E + Send + Sync + 'static,
    {
        self.default_edge_label = Box::new(move |_, _, _| f());
        self
    }

    pub fn set_default_edge_label_with_endpoints<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(&str, &str, Option<&str>) -> E + Send + Sync + 'static,
    {
        self.default_edge_label = Box::new(f);
        self
    }

    pub fn has_node(&self, id: &str) -> bool {
        self.node_index.contains_key(id)
    }

    pub fn node_ix(&self, id: &str) -> Option<usize> {
        self.node_index.get(id).copied()
    }

    pub fn node_id_by_ix(&self, ix: usize) -> Option<&str> {
        self.nodes
            .get(ix)
            .and_then(|n| n.as_ref())
            .map(|n| n.id.as_str())
    }

    pub fn node_label_by_ix(&self, ix: usize) -> Option<&N> {
        self.nodes
            .get(ix)
            .and_then(|n| n.as_ref())
            .map(|n| &n.label)
    }

    pub fn node_label_mut_by_ix(&mut self, ix: usize) -> Option<&mut N> {
        self.nodes
            .get_mut(ix)
            .and_then(|n| n.as_mut())
            .map(|n| &mut n.label)
    }

    pub fn has_edge_ix(&self, v_ix: usize, w_ix: usize) -> bool {
        self.edge_by_endpoints_ix(v_ix, w_ix).is_some()
    }

    pub fn edge_by_endpoints_ix(&self, v_ix: usize, w_ix: usize) -> Option<&E> {
        if self.options.directed {
            let cache = self.ensure_directed_adj();
            for &edge_idx in cache.out_edges(v_ix) {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
                debug_assert_eq!(e.v_ix, v_ix);
                if e.w_ix == w_ix {
                    return Some(&e.label);
                }
            }
            return None;
        }

        for e in self.edges.iter().filter_map(|e| e.as_ref()) {
            if (e.v_ix == v_ix && e.w_ix == w_ix) || (e.v_ix == w_ix && e.w_ix == v_ix) {
                return Some(&e.label);
            }
        }
        None
    }

    pub fn set_node(&mut self, id: impl Into<String>, label: N) -> &mut Self {
        let id = id.into();
        if let Some(&idx) = self.node_index.get(&id) {
            if let Some(node) = self.nodes.get_mut(idx).and_then(|n| n.as_mut()) {
                node.label = label;
            }
            return self;
        }
        let _ = self.insert_node_entry(id, label);
        self
    }

    pub fn set_nodes(&mut self, ids: &[&str]) -> &mut Self {
        for id in ids {
            self.ensure_node_ref(id);
        }
        self
    }

    pub fn set_nodes_with_label(&mut self, ids: &[&str], label: N) -> &mut Self
    where
        N: Clone,
    {
        for id in ids {
            self.set_node(*id, label.clone());
        }
        self
    }

    pub fn ensure_node(&mut self, id: impl Into<String>) -> &mut Self {
        let id = id.into();
        if self.node_index.contains_key(&id) {
            return self;
        }
        let label = (self.default_node_label)(&id);
        self.set_node(id, label)
    }

    pub fn ensure_node_ref(&mut self, id: &str) -> &mut Self {
        if self.node_index.contains_key(id) {
            return self;
        }
        let label = (self.default_node_label)(id);
        self.set_node(id.to_string(), label)
    }

    pub fn node(&self, id: &str) -> Option<&N> {
        let idx = *self.node_index.get(id)?;
        self.nodes
            .get(idx)
            .and_then(|n| n.as_ref())
            .map(|n| &n.label)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut N> {
        let idx = *self.node_index.get(id)?;
        self.nodes
            .get_mut(idx)
            .and_then(|n| n.as_mut())
            .map(|n| &mut n.label)
    }

    pub fn node_count(&self) -> usize {
        self.node_len
    }

    /// Returns the number of index-addressable node slots, including tombstones.
    ///
    /// Unlike [`Self::node_count`], this includes interior tombstones left by removals. Algorithms
    /// that scan index-backed state such as parent arrays should meter this value. Iteration through
    /// [`Self::nodes`] uses the separate object-key order and is metered by
    /// [`Self::node_order_slot_count`].
    pub fn node_slot_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the exact number of ordered entries inspected by one `nodes()` traversal.
    ///
    /// Numeric keys occupy the ordered-map prefix. Ordinary keys use a compacting creation-order
    /// vector whose tombstones remain visible to the iterator until a bounded compaction. This
    /// count lets layout owners charge the actual traversal representation instead of assuming
    /// that live-node or index-slot cardinality is equivalent.
    pub fn node_order_slot_count(&self) -> usize {
        self.node_order.iteration_slot_count()
    }

    /// Returns the number of live node IDs classified as JavaScript array-index keys.
    pub fn array_index_node_count(&self) -> usize {
        self.node_order.numeric_len()
    }

    fn node_indices_in_object_key_order(&self) -> impl Iterator<Item = usize> + '_ {
        self.node_order.iter()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &str> {
        self.node_indices_in_object_key_order()
            .filter_map(|node_ix| self.node_id_by_ix(node_ix))
    }

    pub fn node_ids(&self) -> Vec<String> {
        self.nodes().map(str::to_owned).collect()
    }

    pub fn edge_count(&self) -> usize {
        self.edge_len
    }

    /// Returns the number of indexed edge slots visited by graph-wide edge scans.
    ///
    /// Unlike [`Self::edge_count`], this includes interior tombstones left by removals. Callers
    /// that meter `edge_keys`, `edges`, or another slot-backed traversal should charge this value
    /// rather than the number of live edges.
    pub fn edge_slot_count(&self) -> usize {
        self.edges.len()
    }

    pub fn edge_key_by_ix(&self, edge_ix: usize) -> Option<&EdgeKey> {
        self.edges
            .get(edge_ix)
            .and_then(|e| e.as_ref())
            .map(|e| &e.key)
    }

    pub fn edges(&self) -> impl Iterator<Item = &EdgeKey> {
        self.edges.iter().filter_map(|e| e.as_ref().map(|e| &e.key))
    }

    pub fn for_each_edge<F>(&self, mut f: F)
    where
        F: FnMut(&EdgeKey, &E),
    {
        for e in &self.edges {
            let Some(e) = e.as_ref() else {
                continue;
            };
            f(&e.key, &e.label);
        }
    }

    pub fn for_each_edge_ix<F>(&self, mut f: F)
    where
        F: FnMut(usize, usize, &EdgeKey, &E),
    {
        for e in &self.edges {
            let Some(e) = e.as_ref() else {
                continue;
            };
            f(e.v_ix, e.w_ix, &e.key, &e.label);
        }
    }

    pub fn for_each_edge_entry_ix<F>(&self, mut f: F)
    where
        F: FnMut(usize, usize, usize, &EdgeKey, &E),
    {
        for (edge_ix, e) in self.edges.iter().enumerate() {
            let Some(e) = e.as_ref() else {
                continue;
            };
            f(edge_ix, e.v_ix, e.w_ix, &e.key, &e.label);
        }
    }

    pub fn for_each_edge_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&EdgeKey, &mut E),
    {
        for e in &mut self.edges {
            let Some(e) = e.as_mut() else {
                continue;
            };
            f(&e.key, &mut e.label);
        }
    }

    pub fn for_each_node<F>(&self, mut f: F)
    where
        F: FnMut(&str, &N),
    {
        for node_ix in self.node_indices_in_object_key_order() {
            let Some(node) = self.nodes.get(node_ix).and_then(Option::as_ref) else {
                continue;
            };
            f(&node.id, &node.label);
        }
    }

    pub fn for_each_node_ix<F>(&self, mut f: F)
    where
        F: FnMut(usize, &str, &N),
    {
        for node_ix in self.node_indices_in_object_key_order() {
            let Some(node) = self.nodes.get(node_ix).and_then(Option::as_ref) else {
                continue;
            };
            f(node_ix, &node.id, &node.label);
        }
    }

    pub fn for_each_node_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&str, &mut N),
    {
        let node_order = &self.node_order;
        let nodes = &mut self.nodes;
        for node_ix in node_order.iter() {
            let Some(node) = nodes.get_mut(node_ix).and_then(Option::as_mut) else {
                continue;
            };
            f(&node.id, &mut node.label);
        }
    }

    pub fn edge_keys(&self) -> Vec<EdgeKey> {
        self.edges
            .iter()
            .filter_map(|e| e.as_ref().map(|e| e.key.clone()))
            .collect()
    }

    pub fn filter_nodes<F>(&self, mut filter: F) -> Self
    where
        N: Clone,
        E: Clone,
        G: Clone,
        F: FnMut(&str) -> bool,
    {
        let mut copy = Self::new(self.options);
        copy.set_graph(self.graph_label.clone());

        for node_ix in self.node_indices_in_object_key_order() {
            let node = self.nodes[node_ix]
                .as_ref()
                .expect("the node order contains only live nodes");
            if filter(&node.id) {
                copy.set_node(node.id.clone(), node.label.clone());
            }
        }

        for edge in self.edges.iter().filter_map(|e| e.as_ref()) {
            if copy.has_node(&edge.key.v) && copy.has_node(&edge.key.w) {
                copy.set_edge_named(
                    edge.key.v.clone(),
                    edge.key.w.clone(),
                    edge.key.name.clone(),
                    Some(edge.label.clone()),
                );
            }
        }

        if self.options.compound {
            let copied_ids = copy.nodes().map(str::to_owned).collect::<Vec<_>>();
            for id in copied_ids {
                let mut parent = self.parent(&id);
                while let Some(parent_id) = parent {
                    if copy.has_node(parent_id) {
                        copy.set_parent_ref(&id, parent_id);
                        break;
                    }
                    parent = self.parent(parent_id);
                }
            }
        }

        copy
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
        let _ = self.try_set_edge_named(v, w, name, label);
        self
    }

    pub fn try_set_edge_named(
        &mut self,
        v: impl Into<String>,
        w: impl Into<String>,
        name: Option<impl Into<String>>,
        label: Option<E>,
    ) -> Result<&mut Self, GraphError> {
        let (v, w) = self.canonicalize_endpoints(v.into(), w.into());
        let name = self.canonicalize_name(name.map(Into::into))?;
        let key = EdgeKey { v, w, name };

        Ok(self.set_edge_canonical(key, label))
    }

    fn set_edge_canonical(&mut self, key: EdgeKey, label: Option<E>) -> &mut Self {
        let v = key.v.clone();
        let w = key.w.clone();
        self.ensure_node(v.clone());
        self.ensure_node(w.clone());

        if let Some(&idx) = self.edge_index.get(&key) {
            if let Some(label) = label
                && let Some(edge) = self.edges.get_mut(idx).and_then(|e| e.as_mut())
            {
                edge.label = label;
            }
            return self;
        }

        let Some(&v_ix) = self.node_index.get(&key.v) else {
            return self;
        };
        let Some(&w_ix) = self.node_index.get(&key.w) else {
            return self;
        };

        self.invalidate_adj();
        let idx = self.edges.len();
        self.edges.push(Some(EdgeEntry {
            key: key.clone(),
            v_ix,
            w_ix,
            label: label.unwrap_or_else(|| {
                (self.default_edge_label)(key.v.as_str(), key.w.as_str(), key.name.as_deref())
            }),
        }));
        self.edge_len += 1;
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

    pub fn set_path_with_label(&mut self, nodes: &[&str], label: E) -> &mut Self
    where
        E: Clone,
    {
        if nodes.len() < 2 {
            return self;
        }
        for pair in nodes.windows(2) {
            let v = pair[0];
            let w = pair[1];
            self.set_edge_with_label(v, w, label.clone());
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
        self.edges
            .get(idx)
            .and_then(|e| e.as_ref())
            .map(|e| &e.label)
    }

    pub fn edge_mut(&mut self, v: &str, w: &str, name: Option<&str>) -> Option<&mut E> {
        let view = self.edge_key_view(v, w, name);
        let idx = self.edge_index_of_view(view)?;
        self.edges
            .get_mut(idx)
            .and_then(|e| e.as_mut())
            .map(|e| &mut e.label)
    }

    pub fn edge_by_key(&self, key: &EdgeKey) -> Option<&E> {
        let view = self.edge_key_view_from_key(key);
        let idx = self.edge_index_of_view(view)?;
        self.edges
            .get(idx)
            .and_then(|e| e.as_ref())
            .map(|e| &e.label)
    }

    pub fn edge_mut_by_key(&mut self, key: &EdgeKey) -> Option<&mut E> {
        let view = self.edge_key_view_from_key(key);
        let idx = self.edge_index_of_view(view)?;
        self.edges
            .get_mut(idx)
            .and_then(|e| e.as_mut())
            .map(|e| &mut e.label)
    }

    fn remove_edge_at_index(&mut self, idx: usize) {
        self.invalidate_adj();
        let Some(edge) = self.edges.get(idx).and_then(|e| e.as_ref()) else {
            return;
        };
        let _ = self.edge_index.remove_entry(&edge.key);
        self.edges[idx] = None;
        self.edge_len = self.edge_len.saturating_sub(1);
        self.trim_trailing_edge_tombstones();
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
        let Some(&idx) = self.node_index.get(id) else {
            return false;
        };

        let mut incident_edges: Vec<usize> = if self.options.directed {
            let cache = self.ensure_directed_adj();
            let out_edges = cache.out_edges(idx);
            let in_edges = cache.in_edges(idx);
            let mut edge_indices = Vec::with_capacity(out_edges.len() + in_edges.len());
            edge_indices.extend_from_slice(out_edges);
            edge_indices.extend_from_slice(in_edges);
            edge_indices
        } else {
            let cache = self.ensure_undirected_adj();
            cache.edges(idx).to_vec()
        };
        if incident_edges.len() > 1 {
            incident_edges.sort_unstable();
            incident_edges.dedup();
        }

        if self.options.compound {
            let previous_parent = self.parent_ix.get(idx).copied().flatten();
            self.remove_child_from_object(previous_parent, idx);

            let promoted_children = self.take_children_in_object_key_order(idx);
            for &child_ix in &promoted_children {
                if self.parent_ix.get(child_ix).copied().flatten() == Some(idx) {
                    self.parent_ix[child_ix] = None;
                }
                self.insert_child_in_object_key_order(None, child_ix);
            }
            self.parent_ix[idx] = None;
        }

        self.remove_node_from_object_key_order(idx);
        let _ = self.node_index.remove(id);
        self.invalidate_adj();
        if let Some(slot) = self.nodes.get_mut(idx)
            && slot.is_some()
        {
            *slot = None;
            self.node_len = self.node_len.saturating_sub(1);
        }

        // Remove incident edges.
        for edge_idx in incident_edges {
            let Some(slot) = self.edges.get_mut(edge_idx) else {
                continue;
            };
            let Some(edge) = slot.as_ref() else {
                continue;
            };
            let key = edge.key.clone();
            let _ = self.edge_index.remove_entry(&key);
            *slot = None;
            self.edge_len = self.edge_len.saturating_sub(1);
        }

        self.trim_trailing_edge_tombstones();
        self.trim_trailing_node_tombstones();

        true
    }

    /// Removes a set of nodes with one graph-wide incident-edge pass.
    ///
    /// Missing and duplicate IDs are ignored, and the return value is the number of unique live
    /// nodes removed. Surviving nodes, edges, and compound children retain their relative order;
    /// surviving children of a removed parent become roots. The target iterator is consumed before
    /// the mutation is committed, so an iterator panic cannot leave a partially updated graph.
    ///
    /// For `T` requested IDs, this operation takes amortized `O(T + V + E)` work for ordinary
    /// string IDs and `O((T + V) log V + E)` in the all-array-index case, with `O(V)` temporary
    /// space. It does not allocate graph-sized storage when no live target exists. Sequential
    /// `remove_node` calls remain preferable for isolated mutations because they use the adjacency
    /// cache to touch only incident edges after at most one cache build. Layout phases that retire
    /// many temporary nodes should use this method to avoid rebuilding that cache between removals.
    pub fn remove_nodes<'a, I>(&mut self, ids: I) -> usize
    where
        I: IntoIterator<Item = &'a str>,
    {
        if self.node_len == 0 {
            return 0;
        }

        let mut removed_indices: HashSet<usize> = HashSet::default();
        let mut removed_order = Vec::new();
        for id in ids {
            let Some(&idx) = self.node_index.get(id) else {
                continue;
            };
            if removed_indices.insert(idx) {
                removed_order.push(idx);
            }
        }
        let removed_count = removed_indices.len();
        if removed_count == 0 {
            return 0;
        }

        let mut removed_by_ix = vec![false; self.nodes.len()];
        for idx in removed_indices {
            removed_by_ix[idx] = true;
        }

        if self.options.compound {
            // Replay the requested removal order at the compound-object boundary. This matches
            // graphlib's observable ordinary-key recreation order while each bucket mutation is
            // O(1) amortized for ordinary IDs and O(log V) for array-index IDs.
            for &node_ix in &removed_order {
                let previous_parent = self.parent_ix[node_ix];
                self.remove_child_from_object(previous_parent, node_ix);
                let promoted_children = self.take_children_in_object_key_order(node_ix);
                self.parent_ix[node_ix] = None;
                for child_ix in promoted_children {
                    if self.parent_ix[child_ix] == Some(node_ix) {
                        self.parent_ix[child_ix] = None;
                    }
                    self.insert_child_in_object_key_order(None, child_ix);
                }
            }
        }

        // Consume the caller's iterator completely before committing any mutation. This keeps the
        // graph internally consistent even if a custom iterator panics and the caller catches it.
        self.node_index.retain(|_, idx| !removed_by_ix[*idx]);
        self.invalidate_adj();
        for (idx, removed) in removed_by_ix.iter().copied().enumerate() {
            if !removed {
                continue;
            }
            self.remove_node_from_object_key_order(idx);
            if self.nodes.get_mut(idx).and_then(Option::take).is_some() {
                self.node_len = self.node_len.saturating_sub(1);
            }
        }

        for slot in &mut self.edges {
            let should_remove = slot
                .as_ref()
                .is_some_and(|edge| removed_by_ix[edge.v_ix] || removed_by_ix[edge.w_ix]);
            if !should_remove {
                continue;
            }
            let Some(edge) = slot.take() else {
                continue;
            };
            let _ = self.edge_index.remove_entry(&edge.key);
            self.edge_len = self.edge_len.saturating_sub(1);
        }

        self.trim_trailing_edge_tombstones();
        self.trim_trailing_node_tombstones();
        removed_count
    }

    pub fn successors(&self, v: &str) -> Vec<&str> {
        if !self.options.directed {
            return self.adjacent_nodes(v);
        }
        let Some(&v_idx) = self.node_index.get(v) else {
            return Vec::new();
        };
        let cache = self.ensure_directed_adj();
        let out_edges = cache.out_edges(v_idx);
        let mut out: Vec<&str> = Vec::with_capacity(out_edges.len());
        for &edge_idx in out_edges {
            let Some(edge) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            out.push(edge.key.w.as_str());
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
        let in_edges = cache.in_edges(v_idx);
        let mut out: Vec<&str> = Vec::with_capacity(in_edges.len());
        for &edge_idx in in_edges {
            let Some(edge) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            out.push(edge.key.v.as_str());
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
            let edge_idx = *cache.out_edges(v_idx).first()?;
            self.edges.get(edge_idx)?.as_ref()?.key.w.as_str()
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
            let edge_idx = *cache.in_edges(v_idx).first()?;
            self.edges.get(edge_idx)?.as_ref()?.key.v.as_str()
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
        let out_edges = cache.out_edges(v_idx);
        out.reserve(out_edges.len());
        for &edge_idx in out_edges {
            let Some(edge) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            out.push(edge.key.w.as_str());
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
        let in_edges = cache.in_edges(v_idx);
        out.reserve(in_edges.len());
        for &edge_idx in in_edges {
            let Some(edge) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            out.push(edge.key.v.as_str());
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
        for &edge_idx in cache.out_edges(v_idx) {
            let Some(edge) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            f(edge.key.w.as_str());
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
        for &edge_idx in cache.in_edges(v_idx) {
            let Some(edge) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            f(edge.key.v.as_str());
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
        debug_assert!(!self.options.directed);
        let Some(&v_ix) = self.node_index.get(v) else {
            return Vec::new();
        };
        let cache = self.ensure_undirected_adj();
        let mut seen: HashSet<usize> = HashSet::default();
        let mut out: Vec<&str> = Vec::new();
        for &edge_idx in cache.edges(v_ix) {
            let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            let other_ix = if e.v_ix == v_ix { e.w_ix } else { e.v_ix };
            if !seen.insert(other_ix) {
                continue;
            }
            let Some(other) = self.node_id_by_ix(other_ix) else {
                continue;
            };
            out.push(other);
        }
        out
    }

    pub fn out_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        if self.options.directed {
            let Some(&v_idx) = self.node_index.get(v) else {
                return Vec::new();
            };
            let cache = self.ensure_directed_adj();
            let out_edges = cache.out_edges(v_idx);
            let mut out: Vec<EdgeKey> = Vec::with_capacity(out_edges.len());
            for &edge_idx in out_edges {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
                if w.is_none_or(|w| e.key.w == w) {
                    out.push(e.key.clone());
                }
            }
            return out;
        }

        let Some(&v_ix) = self.node_index.get(v) else {
            return Vec::new();
        };
        let cache = self.ensure_undirected_adj();
        let mut out: Vec<EdgeKey> = Vec::new();
        for &edge_idx in cache.edges(v_ix) {
            let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            if let Some(w) = w {
                let other_ix = if e.v_ix == v_ix { e.w_ix } else { e.v_ix };
                if self.node_id_by_ix(other_ix).is_some_and(|id| id == w) {
                    out.push(e.key.clone());
                }
            } else {
                out.push(e.key.clone());
            }
        }
        out
    }

    pub fn in_edges(&self, v: &str, w: Option<&str>) -> Vec<EdgeKey> {
        if self.options.directed {
            let Some(&v_idx) = self.node_index.get(v) else {
                return Vec::new();
            };
            let cache = self.ensure_directed_adj();
            let in_edges = cache.in_edges(v_idx);
            let mut out: Vec<EdgeKey> = Vec::with_capacity(in_edges.len());
            for &edge_idx in in_edges {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
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
            for &edge_idx in cache.out_edges(v_idx) {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
                if w.is_none_or(|w| e.key.w == w) {
                    f(&e.key, &e.label);
                }
            }
            return;
        }

        let Some(&v_ix) = self.node_index.get(v) else {
            return;
        };
        let cache = self.ensure_undirected_adj();
        for &edge_idx in cache.edges(v_ix) {
            let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            if let Some(w) = w {
                let other_ix = if e.v_ix == v_ix { e.w_ix } else { e.v_ix };
                if self.node_id_by_ix(other_ix).is_some_and(|id| id == w) {
                    f(&e.key, &e.label);
                }
            } else {
                f(&e.key, &e.label);
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
            for &edge_idx in cache.in_edges(v_idx) {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
                if w.is_none_or(|w| e.key.v == w) {
                    f(&e.key, &e.label);
                }
            }
            return;
        }

        self.for_each_out_edge(v, w, f);
    }

    pub fn set_edge_key(&mut self, key: EdgeKey, label: E) -> &mut Self {
        let _ = self.try_set_edge_key(key, label);
        self
    }

    pub fn try_set_edge_key(&mut self, key: EdgeKey, label: E) -> Result<&mut Self, GraphError> {
        let key = self.canonicalize_key(key)?;
        Ok(self.set_edge_canonical(key, Some(label)))
    }

    pub fn for_each_out_edge_ix<F>(&self, v_ix: usize, w_ix: Option<usize>, mut f: F)
    where
        F: FnMut(usize, usize, &EdgeKey, &E),
    {
        if !self.options.directed {
            return;
        }
        let cache = self.ensure_directed_adj();
        for &edge_idx in cache.out_edges(v_ix) {
            let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            debug_assert_eq!(e.v_ix, v_ix);
            if w_ix.is_none_or(|w_ix| e.w_ix == w_ix) {
                f(e.v_ix, e.w_ix, &e.key, &e.label);
            }
        }
    }

    /// Returns the number of directed outgoing edge entries for an indexed node.
    ///
    /// This is an allocation-free cardinality query over the shared adjacency cache. It is useful
    /// to budget an owner-local edge scan before executing that scan.
    pub fn out_edge_count_ix(&self, v_ix: usize) -> usize {
        if !self.options.directed {
            return 0;
        }
        self.ensure_directed_adj().out_edges(v_ix).len()
    }

    pub fn for_each_out_edge_entry_ix<F>(&self, v_ix: usize, w_ix: Option<usize>, mut f: F)
    where
        F: FnMut(usize, usize, usize, &EdgeKey, &E),
    {
        if !self.options.directed {
            return;
        }
        let cache = self.ensure_directed_adj();
        for &edge_ix in cache.out_edges(v_ix) {
            let Some(e) = self.edges.get(edge_ix).and_then(|e| e.as_ref()) else {
                continue;
            };
            debug_assert_eq!(e.v_ix, v_ix);
            if w_ix.is_none_or(|w_ix| e.w_ix == w_ix) {
                f(edge_ix, e.v_ix, e.w_ix, &e.key, &e.label);
            }
        }
    }

    pub fn for_each_neighbor_ix<F>(&self, v_ix: usize, mut f: F)
    where
        F: FnMut(usize),
    {
        if self.options.directed {
            let cache = self.ensure_directed_adj();
            for &edge_idx in cache.out_edges(v_ix) {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
                debug_assert_eq!(e.v_ix, v_ix);
                f(e.w_ix);
            }
            for &edge_idx in cache.in_edges(v_ix) {
                let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                    continue;
                };
                debug_assert_eq!(e.w_ix, v_ix);
                f(e.v_ix);
            }
            return;
        }

        let cache = self.ensure_undirected_adj();
        for &edge_idx in cache.edges(v_ix) {
            let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            let other_ix = if e.v_ix == v_ix { e.w_ix } else { e.v_ix };
            f(other_ix);
        }
    }

    pub fn for_each_in_edge_ix<F>(&self, v_ix: usize, w_ix: Option<usize>, mut f: F)
    where
        F: FnMut(usize, usize, &EdgeKey, &E),
    {
        if !self.options.directed {
            return;
        }
        let cache = self.ensure_directed_adj();
        for &edge_idx in cache.in_edges(v_ix) {
            let Some(e) = self.edges.get(edge_idx).and_then(|e| e.as_ref()) else {
                continue;
            };
            debug_assert_eq!(e.w_ix, v_ix);
            if w_ix.is_none_or(|w_ix| e.v_ix == w_ix) {
                f(e.v_ix, e.w_ix, &e.key, &e.label);
            }
        }
    }

    pub fn for_each_in_edge_entry_ix<F>(&self, v_ix: usize, w_ix: Option<usize>, mut f: F)
    where
        F: FnMut(usize, usize, usize, &EdgeKey, &E),
    {
        if !self.options.directed {
            return;
        }
        let cache = self.ensure_directed_adj();
        for &edge_ix in cache.in_edges(v_ix) {
            let Some(e) = self.edges.get(edge_ix).and_then(|e| e.as_ref()) else {
                continue;
            };
            debug_assert_eq!(e.w_ix, v_ix);
            if w_ix.is_none_or(|w_ix| e.v_ix == w_ix) {
                f(edge_ix, e.v_ix, e.w_ix, &e.key, &e.label);
            }
        }
    }

    pub fn set_parent(&mut self, child: impl Into<String>, parent: impl Into<String>) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        let child = child.into();
        let parent = parent.into();
        self.ensure_node(child.clone());
        self.ensure_node(parent.clone());
        let Some(&child_ix) = self.node_index.get(&child) else {
            return self;
        };
        let Some(&parent_ix) = self.node_index.get(&parent) else {
            return self;
        };
        self.set_parent_ix(child_ix, parent_ix);
        self
    }

    pub fn set_parent_ref(&mut self, child: &str, parent: &str) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        self.ensure_node_ref(child);
        self.ensure_node_ref(parent);
        let Some(&child_ix) = self.node_index.get(child) else {
            return self;
        };
        let Some(&parent_ix) = self.node_index.get(parent) else {
            return self;
        };
        self.set_parent_ix(child_ix, parent_ix);
        self
    }

    pub fn set_parent_ix(&mut self, child_ix: usize, parent_ix: usize) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        if self.nodes.get(child_ix).and_then(Option::as_ref).is_none()
            || self.nodes.get(parent_ix).and_then(Option::as_ref).is_none()
        {
            return self;
        }
        assert!(
            !self.would_create_parent_cycle(child_ix, parent_ix),
            "set_parent would create a cycle"
        );

        let prev = self.parent_ix.get(child_ix).copied().flatten();
        if prev == Some(parent_ix) && self.child_array_index(child_ix).is_some() {
            // Graphlib deletes and recreates the property in its `_children` object. JavaScript
            // still enumerates array-index keys numerically, so that mutation cannot change this
            // child's observable position. Preserve the visible result without a sibling scan.
            return self;
        }
        self.remove_child_from_object(prev, child_ix);

        if let Some(slot) = self.parent_ix.get_mut(child_ix) {
            *slot = Some(parent_ix);
        }
        // For ordinary string keys, dagre-d3-es deletes and recreates the property even when the
        // parent is unchanged, moving it to the end of the ordinary-key segment. Array-index keys
        // remain in numeric order regardless of property creation time.
        self.insert_child_in_object_key_order(Some(parent_ix), child_ix);
        self
    }

    /// Atomically assigns parents to currently unparented children in caller order.
    ///
    /// Each pair is `(child_ix, parent_ix)`. Removed or out-of-range node slots are ignored, as in
    /// [`Self::set_parent_ix`]. Every live child must have no current parent and may occur at most
    /// once in the batch. Violating that narrow construction-only contract returns
    /// [`GraphError::ParentBatchRequiresUnparentedChild`] without mutation; reparenting and repeated
    /// assignments must use [`Self::set_parent_ix`] so dagre-d3-es deletion/reinsertion order stays
    /// observable.
    ///
    /// Existing forest links seed an incremental union-find check. New links are checked in caller
    /// order, so [`GraphError::ParentCycle`] identifies the same first cycle-closing assignment as
    /// sequential dagre-d3-es `setParent` for this unparented-child domain. Validation completes
    /// before mutation, so an error leaves the graph unchanged.
    ///
    /// For `S = node_slot_count()` and `P = assignments.len()`, this takes amortized
    /// `O((S + P) * alpha(S))` time for ordinary string IDs and
    /// `O((S + P) * alpha(S) + P log S)` in the all-array-index case, with `O(S)` temporary space.
    /// In particular, it performs neither one ancestor walk nor one sibling scan per assignment.
    /// Ordinary-key creation order is updated in amortized constant time, while the comparatively
    /// rare array-index keys use the same ordered map semantics as JavaScript `Object.keys`.
    pub fn try_set_unparented_parents_ix(
        &mut self,
        assignments: &[(usize, usize)],
    ) -> Result<&mut Self, GraphError> {
        if !self.options.compound || assignments.is_empty() {
            return Ok(self);
        }

        fn find(parents: &mut [usize], mut index: usize) -> usize {
            let mut root = index;
            while parents[root] != root {
                root = parents[root];
            }
            while parents[index] != index {
                let next = parents[index];
                parents[index] = root;
                index = next;
            }
            root
        }

        fn union(parents: &mut [usize], ranks: &mut [u8], left: usize, right: usize) {
            let left_root = find(parents, left);
            let right_root = find(parents, right);
            if left_root == right_root {
                return;
            }
            if ranks[left_root] < ranks[right_root] {
                parents[left_root] = right_root;
            } else {
                parents[right_root] = left_root;
                if ranks[left_root] == ranks[right_root] {
                    ranks[left_root] = ranks[left_root].saturating_add(1);
                }
            }
        }

        let slot_count = self.nodes.len();
        let mut component_parents = (0..slot_count).collect::<Vec<_>>();
        let mut component_ranks = vec![0u8; slot_count];
        for (child_ix, parent_ix) in self.parent_ix.iter().copied().enumerate() {
            let Some(parent_ix) = parent_ix else {
                continue;
            };
            if self.nodes.get(child_ix).and_then(Option::as_ref).is_some()
                && self.nodes.get(parent_ix).and_then(Option::as_ref).is_some()
            {
                union(
                    &mut component_parents,
                    &mut component_ranks,
                    child_ix,
                    parent_ix,
                );
            }
        }

        let mut assigned = vec![false; slot_count];
        let mut valid_assignments = Vec::with_capacity(assignments.len().min(slot_count));
        for &(child_ix, parent_ix) in assignments {
            if self.nodes.get(child_ix).and_then(Option::as_ref).is_none()
                || self.nodes.get(parent_ix).and_then(Option::as_ref).is_none()
            {
                continue;
            }
            if self.parent_ix[child_ix].is_some() || assigned[child_ix] {
                return Err(GraphError::ParentBatchRequiresUnparentedChild { child_ix });
            }

            let child_root = find(&mut component_parents, child_ix);
            let parent_root = find(&mut component_parents, parent_ix);
            if child_root == parent_root {
                return Err(GraphError::ParentCycle {
                    child_ix,
                    parent_ix,
                });
            }
            union(
                &mut component_parents,
                &mut component_ranks,
                child_root,
                parent_root,
            );
            assigned[child_ix] = true;
            valid_assignments.push((child_ix, parent_ix));
        }

        if valid_assignments.is_empty() {
            return Ok(self);
        }

        for (child_ix, parent_ix) in valid_assignments {
            self.remove_child_from_object(None, child_ix);
            self.parent_ix[child_ix] = Some(parent_ix);
            self.insert_child_in_object_key_order(Some(parent_ix), child_ix);
        }
        Ok(self)
    }

    fn would_create_parent_cycle(&self, child_ix: usize, parent_ix: usize) -> bool {
        let mut seen: HashSet<usize> = HashSet::default();
        let mut current = Some(parent_ix);
        while let Some(ix) = current {
            if ix == child_ix {
                return true;
            }
            if !seen.insert(ix) {
                return true;
            }
            current = self.parent_ix.get(ix).copied().flatten();
        }
        false
    }

    pub fn clear_parent(&mut self, child: &str) -> &mut Self {
        if !self.options.compound {
            return self;
        }
        let Some(&child_ix) = self.node_index.get(child) else {
            return self;
        };
        let prev_parent_ix = self.parent_ix.get(child_ix).copied().flatten();
        self.remove_child_from_object(prev_parent_ix, child_ix);
        self.parent_ix[child_ix] = None;
        self.insert_child_in_object_key_order(None, child_ix);
        self
    }

    pub fn parent(&self, child: &str) -> Option<&str> {
        if !self.options.compound {
            return None;
        }
        let &child_ix = self.node_index.get(child)?;
        let parent_ix = self.parent_ix.get(child_ix).copied().flatten()?;
        self.node_id_by_ix(parent_ix)
    }

    pub fn children_iter<'a>(&'a self, parent: &str) -> impl Iterator<Item = &'a str> + 'a {
        let children = if self.options.compound {
            self.node_index
                .get(parent)
                .and_then(|&p_ix| self.children_ix.get(p_ix))
        } else {
            None
        };
        children
            .into_iter()
            .flat_map(ObjectKeyOrder::iter)
            .filter_map(|child_ix| self.node_id_by_ix(child_ix))
    }

    /// Returns the number of direct compound children without allocating a snapshot.
    ///
    /// Missing nodes and non-compound graphs have no direct compound children. Public graph
    /// mutations keep the indexed child list free of removed or duplicate entries, so this is an
    /// `O(1)` query equivalent to `children(parent).len()`.
    pub fn child_count(&self, parent: &str) -> usize {
        if !self.options.compound {
            return 0;
        }
        self.node_index
            .get(parent)
            .and_then(|&parent_ix| self.children_ix.get(parent_ix))
            .map_or(0, ObjectKeyOrder::len)
    }

    /// Returns the exact number of ordered entries inspected by `children_iter(parent)`.
    pub fn child_order_slot_count(&self, parent: &str) -> usize {
        if !self.options.compound {
            return 0;
        }
        self.node_index
            .get(parent)
            .and_then(|&parent_ix| self.children_ix.get(parent_ix))
            .map_or(0, ObjectKeyOrder::iteration_slot_count)
    }

    /// Returns the number of direct children whose IDs are JavaScript array-index keys.
    pub fn array_index_child_count(&self, parent: &str) -> usize {
        if !self.options.compound {
            return 0;
        }
        self.node_index
            .get(parent)
            .and_then(|&parent_ix| self.children_ix.get(parent_ix))
            .map_or(0, ObjectKeyOrder::numeric_len)
    }

    /// Returns the exact number of ordered entries inspected by `children_root()`.
    pub fn root_child_order_slot_count(&self) -> usize {
        if self.options.compound {
            self.root_children.iteration_slot_count()
        } else {
            self.node_order_slot_count()
        }
    }

    /// Returns the number of root children whose IDs are JavaScript array-index keys.
    pub fn root_array_index_child_count(&self) -> usize {
        if self.options.compound {
            self.root_children.numeric_len()
        } else {
            self.array_index_node_count()
        }
    }

    pub fn children(&self, parent: &str) -> Vec<&str> {
        self.children_iter(parent).collect()
    }

    pub fn children_opt(&self, parent: &str) -> Option<Vec<&str>> {
        if !self.options.compound {
            return self.has_node(parent).then(Vec::new);
        }

        let &parent_ix = self.node_index.get(parent)?;
        let children = self.children_ix.get(parent_ix)?;
        let mut out = Vec::with_capacity(children.len());
        for child_ix in children.iter() {
            if let Some(id) = self.node_id_by_ix(child_ix) {
                out.push(id);
            }
        }
        Some(out)
    }

    pub fn children_root(&self) -> Vec<&str> {
        if !self.options.compound {
            return self.nodes().collect();
        }
        self.root_children
            .iter()
            .filter_map(|child_ix| self.node_id_by_ix(child_ix))
            .collect()
    }

    pub fn sources(&self) -> Vec<&str> {
        if !self.options.directed {
            return self.nodes().collect();
        }
        self.node_indices_in_object_key_order()
            .filter_map(|node_ix| self.nodes[node_ix].as_ref())
            .filter(|node| self.in_edges(&node.id, None).is_empty())
            .map(|node| node.id.as_str())
            .collect()
    }

    pub fn sinks(&self) -> Vec<&str> {
        if !self.options.directed {
            return self.nodes().collect();
        }
        self.node_indices_in_object_key_order()
            .filter_map(|node_ix| self.nodes[node_ix].as_ref())
            .filter(|node| self.out_edges(&node.id, None).is_empty())
            .map(|node| node.id.as_str())
            .collect()
    }

    pub fn is_leaf(&self, v: &str) -> bool {
        if !self.has_node(v) {
            return false;
        }
        if self.options.directed {
            return self.successors(v).is_empty();
        }
        self.neighbors(v).is_empty()
    }

    pub fn node_edges(&self, v: &str) -> Vec<EdgeKey> {
        let mut out: Vec<EdgeKey> = Vec::new();
        let mut seen: HashSet<EdgeKey> = HashSet::default();
        for e in &self.edges {
            let Some(e) = e.as_ref() else {
                continue;
            };
            if (e.key.v == v || e.key.w == v) && seen.insert(e.key.clone()) {
                out.push(e.key.clone());
            }
        }
        out
    }

    pub fn node_edges_between(&self, v: &str, w: &str) -> Vec<EdgeKey> {
        let mut out: Vec<EdgeKey> = Vec::new();
        let mut seen: HashSet<EdgeKey> = HashSet::default();
        for e in &self.edges {
            let Some(e) = e.as_ref() else {
                continue;
            };
            let touches_both = (e.key.v == v && e.key.w == w) || (e.key.v == w && e.key.w == v);
            if touches_both && seen.insert(e.key.clone()) {
                out.push(e.key.clone());
            }
        }
        out
    }
}
