//! Helper algorithms re-exported for Dagre compatibility.

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
