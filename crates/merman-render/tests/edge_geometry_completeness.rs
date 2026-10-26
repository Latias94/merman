#![cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]

use base64::Engine as _;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{environment::RenderEnvironment, family};
use std::collections::BTreeMap;

#[test]
fn geometry_contains_every_painted_edge_occurrence_for_each_flowchart_adapter() {
    let cases = [
        #[cfg(feature = "diagram-flowchart")]
        (
            "dagre nested",
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n---\nflowchart LR\nsubgraph Outer\n direction TB\n A((Circle)) -->|inner| B{Diamond}\nend\nB -->|outer| C[End]",
            2,
        ),
        #[cfg(all(feature = "diagram-flowchart", feature = "layout-elk"))]
        (
            "elk parallel",
            "---\nconfig:\n  layout: elk\n  htmlLabels: false\n---\nflowchart LR\nA((Circle)) -->|one| B{Diamond}\nA -->|two| B\nB --> C[End]",
            3,
        ),
        #[cfg(feature = "diagram-flowchart")]
        (
            "duplicate public IDs",
            "---\nconfig:\n  layout: dagre\n---\nflowchart LR\nX L_A_B_0@-->|first owner| Y\nA -->|second owner| B",
            2,
        ),
        #[cfg(feature = "diagram-swimlane")]
        (
            "swimlane nested",
            "---\nconfig:\n  htmlLabels: false\n---\nswimlane-beta LR\nsubgraph Outer\n subgraph Inner\n A((Circle)) -->|inner| B{Diamond}\n end\nend\nB -->|outer| C[End]",
            2,
        ),
        #[cfg(feature = "diagram-agentflow")]
        (
            "agentflow",
            "agentflow-beta\nA -->|first| B\nA -->|parallel| B\nB -->|last| C",
            3,
        ),
    ];
    for (case, source, expected_occurrences) in cases {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("{case}: {error}"))
            .expect("diagram");
        let prepare = || {
            family::prepare(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .unwrap()
        };
        let geometry = prepare().edge_geometry_json().unwrap();
        let svg = prepare()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(svg.svg()).unwrap();
        let paths = document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-points").is_some())
            .collect::<Vec<_>>();
        assert!(!paths.is_empty(), "{case}: actual SVG must paint edges");
        assert_eq!(
            paths.len(),
            expected_occurrences,
            "{case}: actual SVG must exercise every authored edge"
        );
        assert_eq!(
            geometry.len(),
            paths.len(),
            "{case}: missing or extra geometry occurrences"
        );
        let mut painted = BTreeMap::<String, Vec<usize>>::new();
        for path in paths {
            let id = path.attribute("data-id").expect("edge occurrence ID");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<serde_json::Value> =
                serde_json::from_slice(&bytes).expect("serialized SVG point list");
            assert!(points.len() >= 2, "{case}: {id} must paint a route");
            painted.entry(id.to_owned()).or_default().push(points.len());
        }
        let mut queried = BTreeMap::<String, Vec<usize>>::new();
        for edge in geometry {
            let points = edge.points.expect("every painted route has queried points");
            assert!(
                points
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite()),
                "{case}: {}",
                edge.id
            );
            queried.entry(edge.id).or_default().push(points.len());
        }
        // DOM traversal can visit recursive roots in a different order. Retain multiplicity,
        // including duplicate public IDs, instead of collapsing either consumer into a set.
        for occurrences in painted.values_mut().chain(queried.values_mut()) {
            occurrences.sort_unstable();
        }
        assert_eq!(
            queried, painted,
            "{case}: complete painted occurrence inventory"
        );
    }
}
