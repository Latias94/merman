use merman_ascii::{AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GraphFixture {
    directory: &'static str,
    name: &'static str,
}

impl GraphFixture {
    fn key(self) -> String {
        format!("{}/{}", self.directory, self.name)
    }

    fn options(self) -> AsciiRenderOptions {
        let mut options = match self.directory {
            "ascii" => AsciiRenderOptions::ascii(),
            "extended-chars" => AsciiRenderOptions::unicode(),
            other => panic!("unsupported graph fixture directory: {other}"),
        };

        let (graph_padding_x, graph_padding_y) = self.graph_padding();
        options.graph_padding_x = graph_padding_x;
        options.graph_padding_y = graph_padding_y;
        options
    }

    fn graph_padding(self) -> (usize, usize) {
        match self.name {
            "backlink_with_short_y_padding.txt" => (5, 3),
            "custom_padding.txt" => (2, 1),
            "subgraph_td_multiple_paddingy.txt" => (3, 3),
            _ => (5, 5),
        }
    }
}

const fn graph_fixture(directory: &'static str, name: &'static str) -> GraphFixture {
    GraphFixture { directory, name }
}

const GRAPH_FIXTURE_CORPUS: &[GraphFixture] = &[
    GraphFixture {
        directory: "ascii",
        name: "ampersand_lhs.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "ampersand_lhs_and_rhs.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "ampersand_rhs.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "ampersand_without_edge.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "back_reference_from_child.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "backlink_from_bottom.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "backlink_from_top.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "backlink_with_short_y_padding.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "back_edges_two_labels_td.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "bidirectional_edge_labels_lr.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "bidirectional_edge_labels_td.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "comments.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "custom_padding.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "duplicate_edge_labels.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "explicit_label_after_bare_reference.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "flowchart_tb_simple.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "graph_tb_direction.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "multiline_single_node.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "preserve_order_of_definition.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "single_node_longer_name.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "single_node.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "self_reference.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "self_reference_with_edge.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_complex_mixed.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_complex_nested.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_empty.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_explicit_title.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_mixed_nodes_td.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_mixed_nodes.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_multiple_edges.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_multiple_nodes.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_nested_with_external.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_nested.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_node_outside_lr.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_single_node.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_standalone_labeled_node.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_td_direction.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_td_multiple_paddingy.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_td_multiple.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_three_levels_nested.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_three_separate.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_two_separate.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "subgraph_with_labels.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "three_nodes_single_line.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "three_nodes.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "tight_arrow.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "tight_arrow_mixed.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_layer_single_graph.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_layer_single_graph_longer_names.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_nodes_linked.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_nodes_longer_names.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_root_nodes_longer_names.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_root_nodes.txt",
    },
    GraphFixture {
        directory: "ascii",
        name: "two_single_root_nodes.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "ampersand_lhs.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "ampersand_lhs_and_rhs.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "ampersand_rhs.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "ampersand_without_edge.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "back_reference_from_child.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "backlink_from_bottom.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "backlink_from_top.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "back_edges_two_labels_td.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "comments.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "preserve_order_of_definition.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "self_reference.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "self_reference_with_edge.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "single_node_longer_name.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "single_node.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "three_nodes_single_line.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "three_nodes.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "tight_arrow.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "tight_arrow_mixed.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_layer_single_graph_longer_names.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_layer_single_graph.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_nodes_linked.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_nodes_longer_names.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_root_nodes_longer_names.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_root_nodes.txt",
    },
    GraphFixture {
        directory: "extended-chars",
        name: "two_single_root_nodes.txt",
    },
];

const GRAPH_FIXTURE_GAPS: &[GraphFixture] = &[
    graph_fixture("ascii", "ampersand_lhs.txt"),
    graph_fixture("ascii", "ampersand_lhs_and_rhs.txt"),
    graph_fixture("ascii", "ampersand_rhs.txt"),
    graph_fixture("ascii", "back_reference_from_child.txt"),
    graph_fixture("ascii", "backlink_from_bottom.txt"),
    graph_fixture("ascii", "backlink_from_top.txt"),
    graph_fixture("ascii", "backlink_with_short_y_padding.txt"),
    graph_fixture("ascii", "back_edges_two_labels_td.txt"),
    graph_fixture("ascii", "bidirectional_edge_labels_lr.txt"),
    graph_fixture("ascii", "bidirectional_edge_labels_td.txt"),
    graph_fixture("ascii", "comments.txt"),
    graph_fixture("ascii", "duplicate_edge_labels.txt"),
    graph_fixture("ascii", "preserve_order_of_definition.txt"),
    graph_fixture("ascii", "subgraph_complex_mixed.txt"),
    graph_fixture("ascii", "subgraph_empty.txt"),
    graph_fixture("ascii", "subgraph_mixed_nodes_td.txt"),
    graph_fixture("ascii", "subgraph_multiple_edges.txt"),
    graph_fixture("ascii", "subgraph_node_outside_lr.txt"),
    graph_fixture("ascii", "subgraph_with_labels.txt"),
    graph_fixture("ascii", "tight_arrow_mixed.txt"),
    graph_fixture("ascii", "two_layer_single_graph.txt"),
    graph_fixture("ascii", "two_layer_single_graph_longer_names.txt"),
    graph_fixture("extended-chars", "ampersand_lhs.txt"),
    graph_fixture("extended-chars", "ampersand_lhs_and_rhs.txt"),
    graph_fixture("extended-chars", "ampersand_rhs.txt"),
    graph_fixture("extended-chars", "back_reference_from_child.txt"),
    graph_fixture("extended-chars", "backlink_from_bottom.txt"),
    graph_fixture("extended-chars", "backlink_from_top.txt"),
    graph_fixture("extended-chars", "back_edges_two_labels_td.txt"),
    graph_fixture("extended-chars", "comments.txt"),
    graph_fixture("extended-chars", "preserve_order_of_definition.txt"),
    graph_fixture("extended-chars", "tight_arrow_mixed.txt"),
    graph_fixture("extended-chars", "two_layer_single_graph_longer_names.txt"),
    graph_fixture("extended-chars", "two_layer_single_graph.txt"),
];

fn render_flowchart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("flowchart should parse")
        .expect("flowchart should be detected");

    render_model(parsed.model(), options)
}

fn fixture_cases(directory: &str) -> Vec<PathBuf> {
    let root = fixture_root().join(directory);
    let mut cases = std::fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", root.display()))
        .map(|entry| entry.expect("fixture entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect::<Vec<_>>();
    cases.sort();
    cases
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/testdata/mermaid-ascii")
}

fn fixture_path(fixture: GraphFixture) -> PathBuf {
    fixture_root().join(fixture.directory).join(fixture.name)
}

fn split_fixture(path: &Path) -> (String, String) {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .replace("\r\n", "\n");
    let (input, expected) = content
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
    let mut expected = expected.to_string();
    if !expected.ends_with('\n') {
        expected.push('\n');
    }
    (input.to_string(), expected)
}

fn graph_fixture_keys(fixtures: &[GraphFixture]) -> BTreeSet<String> {
    fixtures.iter().map(|fixture| fixture.key()).collect()
}

#[test]
fn graph_fixture_exact_subset_matches_upstream() {
    let gaps = graph_fixture_keys(GRAPH_FIXTURE_GAPS);
    let mut mismatches = Vec::new();
    for fixture in GRAPH_FIXTURE_CORPUS {
        if gaps.contains(&fixture.key()) {
            continue;
        }
        let path = fixture_path(*fixture);
        let (input, expected) = split_fixture(&path);
        match render_flowchart(&input, &fixture.options()) {
            Ok(rendered) if rendered == expected => {}
            Ok(_) => mismatches.push(format!("{}: output differs", fixture.key())),
            Err(error) => mismatches.push(format!("{}: {error}", fixture.key())),
        }
    }

    assert!(
        mismatches.is_empty(),
        "copied graph fixtures no longer match exactly:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn graph_fixture_named_gaps_remain_renderable() {
    let mut failures = Vec::new();
    for fixture in GRAPH_FIXTURE_GAPS {
        let path = fixture_path(*fixture);
        let (input, _) = split_fixture(&path);
        match render_flowchart(&input, &fixture.options()) {
            Ok(rendered) if !rendered.is_empty() => {}
            Ok(_) => failures.push(format!("{}: empty output", fixture.key())),
            Err(error) => failures.push(format!("{}: {error}", fixture.key())),
        }
    }

    assert!(
        failures.is_empty(),
        "named graph fixture gaps must remain renderable:\n{}",
        failures.join("\n")
    );
}

#[test]
fn graph_fixture_gap_inventory_covers_all_graph_fixtures() {
    let corpus = graph_fixture_keys(GRAPH_FIXTURE_CORPUS);
    let gaps = graph_fixture_keys(GRAPH_FIXTURE_GAPS);
    assert_eq!(corpus.len(), GRAPH_FIXTURE_CORPUS.len());
    assert_eq!(gaps.len(), GRAPH_FIXTURE_GAPS.len());
    assert!(
        gaps.is_subset(&corpus),
        "every named graph gap must belong to the copied corpus"
    );

    let mut discovered = BTreeSet::new();
    for directory in ["ascii", "extended-chars"] {
        for path in fixture_cases(directory) {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_else(|| panic!("fixture path is not UTF-8: {}", path.display()));
            discovered.insert(format!("{directory}/{name}"));
        }
    }

    assert_eq!(
        discovered, corpus,
        "the copied graph corpus inventory must cover every fixture exactly once"
    );
}
