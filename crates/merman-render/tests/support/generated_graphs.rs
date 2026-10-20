use std::fmt::Write as _;

pub const SEEDS: [u64; 6] = [1, 7, 42, 0x5eed, 0xdecafbad, u64::MAX];
pub const DIRECTIONS: [&str; 4] = ["TB", "BT", "LR", "RL"];

#[derive(Clone, Copy, Debug)]
pub enum Topology {
    Dag,
    NestedDag,
    ParallelDag,
    Cycle,
}

pub struct GeneratedGraph {
    pub source: String,
    pub leaves: Vec<(String, Option<String>)>,
    pub groups: Vec<(String, Option<String>)>,
    pub edges: Vec<(String, String)>,
    pub chain: Vec<String>,
}

struct Lcg(u64);

impl Lcg {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 32) as usize) % bound
    }
}

// This concrete subset deliberately excludes self/ancestor edges and uses only rectangular
// leaves. The last leaf stays disconnected; nested cases use three nonempty compounds.
pub fn generate(seed: u64, direction: &str, topology: Topology, swimlane: bool) -> GeneratedGraph {
    let mut rng = Lcg(seed);
    let count = 8 + rng.below(7);
    let nested = matches!(topology, Topology::NestedDag);
    let header = if swimlane {
        "swimlane-beta"
    } else {
        "flowchart"
    };
    let mut source = format!("{header} {direction}\n");
    let mut leaves = Vec::new();
    let mut groups = Vec::new();
    if nested {
        source.push_str("subgraph Outer[Outer]\nsubgraph Inner[Inner]\n");
        groups.extend([
            ("Outer".into(), None),
            ("Inner".into(), Some("Outer".into())),
            ("Other".into(), None),
        ]);
    } else if swimlane {
        source.push_str("subgraph First[First lane]\n");
        groups.extend([("First".into(), None), ("Second".into(), None)]);
    }
    for index in 0..count {
        if nested {
            if index == 3 {
                source.push_str("end\n");
            }
            if index == 5 {
                source.push_str("end\nsubgraph Other[Other]\n");
            }
            if index == count - 1 {
                source.push_str("end\n");
            }
        } else if swimlane {
            if index == count / 2 {
                source.push_str("end\nsubgraph Second[Second lane]\n");
            }
            if index == count - 1 {
                source.push_str("end\n");
            }
        }
        let id = format!("N{index}");
        writeln!(source, "{id}[Node {index}]").unwrap();
        let parent = if nested {
            match index {
                0..=2 => Some("Inner"),
                3..=4 => Some("Outer"),
                _ if index < count - 1 => Some("Other"),
                _ => None,
            }
        } else if swimlane {
            if index < count / 2 {
                Some("First")
            } else if index < count - 1 {
                Some("Second")
            } else {
                None
            }
        } else {
            None
        };
        leaves.push((id, parent.map(str::to_owned)));
    }
    let mut edges = (0..count - 2)
        .map(|index| (format!("N{index}"), format!("N{}", index + 1)))
        .collect::<Vec<_>>();
    for from in 0..count - 3 {
        if rng.below(2) == 0 {
            let to = from + 2 + rng.below(count - from - 3);
            edges.push((format!("N{from}"), format!("N{to}")));
        }
    }
    if matches!(topology, Topology::ParallelDag) {
        edges.extend([("N0".into(), "N1".into()), ("N0".into(), "N1".into())]);
    }
    if matches!(topology, Topology::Cycle) {
        edges.push((format!("N{}", count - 2), "N0".into()));
    }
    for (from, to) in &edges {
        writeln!(source, "{from} --> {to}").unwrap();
    }
    let chain = if matches!(topology, Topology::Dag | Topology::ParallelDag) && !swimlane {
        (0..count - 1).map(|index| format!("N{index}")).collect()
    } else if swimlane && !nested && !matches!(topology, Topology::Cycle) {
        // Same-lane ordering is guaranteed; cross-lane ranking is lane-owned.
        (0..count / 2).map(|index| format!("N{index}")).collect()
    } else {
        Vec::new()
    };
    GeneratedGraph {
        source,
        leaves,
        groups,
        edges,
        chain,
    }
}
