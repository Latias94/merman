use crate::graphlib::Graph;
use rustc_hash::FxHashMap as HashMap;

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
    let mut prev: HashMap<&str, &str> = HashMap::default();
    let mut root_prev: Option<&str> = None;

    for v in vs {
        let mut child = g.parent(v.as_str());
        while let Some(c) = child {
            let parent = g.parent(c);

            let prev_child = if let Some(p) = parent {
                prev.insert(p, c)
            } else {
                root_prev.replace(c)
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
