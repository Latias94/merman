use crate::graphlib::Graph;
use std::collections::HashMap;

pub fn add_subgraph_constraints<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    cg: &mut Graph<CN, CE, CG>,
    vs: &[String],
) where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let mut prev: HashMap<String, String> = HashMap::new();
    let mut root_prev: Option<String> = None;

    for v in vs {
        let mut child = g.parent(v).map(|s| s.to_string());
        while let Some(c) = child.clone() {
            let parent = g.parent(&c).map(|s| s.to_string());

            let prev_child = if let Some(p) = parent.as_deref() {
                prev.insert(p.to_string(), c.clone())
            } else {
                root_prev.replace(c.clone())
            };

            if let Some(prev_child) = prev_child {
                if prev_child != c {
                    cg.set_edge(prev_child, c);
                    break;
                }
            }

            child = parent;
        }
    }
}
