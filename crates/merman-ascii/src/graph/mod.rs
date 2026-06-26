mod adapter;
mod charset;
mod draw;
mod label;
mod layout;
mod model;
mod routing;
pub(crate) mod style;
mod topology;

pub(crate) use adapter::from_flowchart_model;
pub(crate) use draw::render_graph;
pub(crate) use model::{
    AsciiGraph, GraphDirection, GraphEdgeArrow, GraphEdgeAttrs, GraphGroupKind, GraphGroupStyle,
    GraphNodeShape, GraphNodeStyle,
};

#[cfg(test)]
mod graph_golden {
    use super::model::{AsciiGraph, GraphDirection};
    use super::*;
    use crate::AsciiRenderOptions;
    use std::path::Path;

    fn fixture_expected(directory: &str, name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/mermaid-ascii")
            .join(directory)
            .join(name);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
            .replace("\r\n", "\n");
        let (_, expected) = content
            .split_once("\n---\n")
            .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
        expected.to_string()
    }

    #[test]
    fn single_node_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "single_node.txt"));
    }

    #[test]
    fn single_node_unicode_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");

        let actual = render_graph(&graph, &AsciiRenderOptions::unicode()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("extended-chars", "single_node.txt")
        );
    }

    #[test]
    fn two_nodes_linked_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_edge("A", "B");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "two_nodes_linked.txt"));
    }

    #[test]
    fn two_nodes_linked_unicode_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_edge("A", "B");

        let actual = render_graph(&graph, &AsciiRenderOptions::unicode()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("extended-chars", "two_nodes_linked.txt")
        );
    }

    #[test]
    fn long_node_labels_ascii_match_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("LongerName1", "LongerName1");
        graph.add_node("LongerName2", "LongerName2");
        graph.add_edge("LongerName1", "LongerName2");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            actual,
            fixture_expected("ascii", "two_nodes_longer_names.txt")
        );
    }

    #[test]
    fn top_down_chain_ascii_matches_upstream_golden() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node("A", "A");
        graph.add_node("B", "B");
        graph.add_node("C", "C");
        graph.add_edge("A", "B");
        graph.add_edge("B", "C");

        let actual = render_graph(&graph, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(actual, fixture_expected("ascii", "flowchart_tb_simple.txt"));
    }
}
