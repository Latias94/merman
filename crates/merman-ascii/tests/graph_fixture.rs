mod support;

use merman_ascii::AsciiRenderOptions;
use merman_core::diagram::RenderSemanticModel;
use merman_core::{Engine, ParseOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use support::render_model;

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
    graph_fixture("ascii", "graph_tb_direction.txt"),
    graph_fixture("ascii", "preserve_order_of_definition.txt"),
    graph_fixture("ascii", "subgraph_complex_nested.txt"),
    graph_fixture("ascii", "subgraph_complex_mixed.txt"),
    graph_fixture("ascii", "subgraph_empty.txt"),
    graph_fixture("ascii", "subgraph_explicit_title.txt"),
    graph_fixture("ascii", "subgraph_mixed_nodes_td.txt"),
    graph_fixture("ascii", "subgraph_multiple_edges.txt"),
    graph_fixture("ascii", "subgraph_multiple_nodes.txt"),
    graph_fixture("ascii", "subgraph_node_outside_lr.txt"),
    graph_fixture("ascii", "subgraph_td_direction.txt"),
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

// These copied outputs erase group borders at legal route crossings. Their bytes remain immutable;
// named corrections use strict Merman snapshots and independently verify the compound geometry.
const GRAPH_FIXTURE_CORRECTIONS: &[GraphFixture] = &[
    graph_fixture("ascii", "subgraph_mixed_nodes.txt"),
    graph_fixture("ascii", "subgraph_nested_with_external.txt"),
    graph_fixture("ascii", "subgraph_standalone_labeled_node.txt"),
    graph_fixture("ascii", "subgraph_td_multiple_paddingy.txt"),
    graph_fixture("ascii", "subgraph_td_multiple.txt"),
    graph_fixture("ascii", "subgraph_three_separate.txt"),
    graph_fixture("ascii", "subgraph_two_separate.txt"),
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
    let corrections = graph_fixture_keys(GRAPH_FIXTURE_CORRECTIONS);
    let mut exact_count = 0;
    let mut mismatches = Vec::new();
    for fixture in GRAPH_FIXTURE_CORPUS {
        if gaps.contains(&fixture.key()) || corrections.contains(&fixture.key()) {
            continue;
        }
        exact_count += 1;
        let path = fixture_path(*fixture);
        let (input, expected) = split_fixture(&path);
        match render_flowchart(&input, &fixture.options()) {
            Ok(rendered) if rendered == expected => {}
            Ok(_) => mismatches.push(format!("{}: output differs", fixture.key())),
            Err(error) => mismatches.push(format!("{}: {error}", fixture.key())),
        }
    }

    assert_eq!(
        exact_count, 33,
        "the immutable exact subset must remain explicit"
    );
    assert!(
        mismatches.is_empty(),
        "copied graph fixtures no longer match exactly:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn graph_fixture_named_corrections_preserve_closed_compound_frames() {
    for fixture in GRAPH_FIXTURE_CORRECTIONS {
        let (input, copied) = split_fixture(&fixture_path(*fixture));
        let snapshot = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/graph_fixture/corrected-fixtures")
            .join(fixture.directory)
            .join(fixture.name);
        let expected = std::fs::read_to_string(&snapshot).unwrap_or_else(|error| {
            panic!("read corrected snapshot {}: {error}", snapshot.display())
        });
        assert_ne!(
            expected,
            copied,
            "{} must retain a verified correction",
            fixture.key()
        );
        assert_eq!(
            expected.len(),
            copied.len(),
            "these corrections preserve the copied geometry"
        );
        assert!(
            copied
                .chars()
                .zip(expected.chars())
                .all(|(original, corrected)| {
                    original == corrected || (matches!(original, '-' | '|') && corrected == '+')
                }),
            "{} must only preserve the frame directions at route crossings",
            fixture.key()
        );
        let rendered = render_flowchart(&input, &fixture.options())
            .unwrap_or_else(|error| panic!("{}: {error}", fixture.key()));
        assert_eq!(
            rendered,
            expected,
            "{} corrected output changed",
            fixture.key()
        );
        assert_eq!(
            visible_text_token_counts(&rendered),
            visible_text_token_counts(&copied),
            "{} must preserve every copied label exactly once",
            fixture.key()
        );
        assert_corrected_compound_geometry(&input, &rendered);
    }
}

fn unique_ascii_label_position(rows: &[Vec<char>], label: &str) -> (usize, usize) {
    assert!(
        !label.is_empty() && label.is_ascii(),
        "these named labels are plain ASCII"
    );
    let is_word = |glyph: char| glyph.is_ascii_alphanumeric() || glyph == '_';
    let positions = rows
        .iter()
        .enumerate()
        .flat_map(|(row, glyphs)| {
            glyphs
                .iter()
                .collect::<String>()
                .match_indices(label)
                .filter_map(|(column, _)| {
                    let before = column.checked_sub(1).and_then(|index| glyphs.get(index));
                    let after = glyphs.get(column + label.len());
                    (!before.is_some_and(|glyph| is_word(*glyph))
                        && !after.is_some_and(|glyph| is_word(*glyph)))
                    .then_some((row, column))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        positions.len(),
        1,
        "authored label {label:?} must occur exactly once"
    );
    positions[0]
}

fn assert_corrected_compound_geometry(input: &str, rendered: &str) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("corrected fixture parses")
        .expect("corrected fixture is detected");
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("a corrected graph fixture must have the Flowchart model");
    };
    let rows = rendered
        .lines()
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut frames = BTreeMap::new();
    let mut crossings = 0;
    for group in &model.subgraphs {
        let (title_row, title_column) = unique_ascii_label_position(&rows, &group.title);
        let top = title_row
            .checked_sub(1)
            .expect("group title has a top frame");
        let candidates = (0..title_column)
            .filter(|left| rows[top][*left] == '+')
            .flat_map(|left| {
                let rows = &rows;
                (title_column + group.title.len()..rows[top].len())
                    .filter(move |right| rows[top][*right] == '+')
                    .filter_map(move |right| {
                        let horizontal_frame = |row: usize| {
                            rows[row].get(left) == Some(&'+')
                                && rows[row].get(right) == Some(&'+')
                                && rows[row][left + 1..right]
                                    .iter()
                                    .all(|glyph| matches!(glyph, '-' | '+'))
                        };
                        if !horizontal_frame(top) {
                            return None;
                        }
                        (title_row + 1..rows.len())
                            .find(|bottom| {
                                horizontal_frame(*bottom)
                                    && rows[top + 1..*bottom].iter().all(|row| {
                                        matches!(row.get(left), Some('|' | '+'))
                                            && matches!(row.get(right), Some('|' | '+'))
                                    })
                            })
                            .map(|bottom| (left, right, bottom))
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            candidates.len(),
            1,
            "group {:?} must have one closed frame around its title:\n{rendered}",
            group.title
        );
        let (left, right, bottom) = candidates[0];
        for row in [top, bottom] {
            assert_eq!(rows[row][left], '+', "group corner must remain visible");
            assert_eq!(rows[row][right], '+', "group corner must remain visible");
            assert!(
                rows[row][left + 1..right]
                    .iter()
                    .all(|glyph| matches!(glyph, '-' | '+')),
                "horizontal group frame must remain continuous:\n{rendered}"
            );
            crossings += rows[row][left + 1..right]
                .iter()
                .filter(|glyph| **glyph == '+')
                .count();
        }
        for row in &rows[top + 1..bottom] {
            for column in [left, right] {
                assert!(
                    matches!(row[column], '|' | '+'),
                    "vertical group frame must remain continuous at {column}:\n{rendered}"
                );
                crossings += usize::from(row[column] == '+');
            }
        }
        frames.insert(group.id.as_str(), (left, top, right, bottom));
    }
    assert!(
        crossings > 0,
        "a named correction must retain an actual frame/route junction"
    );

    // The first declaration owns repeated membership, including API in the two TD fixtures.
    let mut owners = BTreeMap::new();
    for group in &model.subgraphs {
        for member in &group.nodes {
            owners.entry(member.as_str()).or_insert(group.id.as_str());
        }
    }
    let contains = |outer: (usize, usize, usize, usize), inner: (usize, usize, usize, usize)| {
        outer.0 < inner.0 && outer.1 < inner.1 && inner.2 < outer.2 && inner.3 < outer.3
    };
    let mut node_frames = BTreeMap::new();
    for node in model.nodes.iter().filter(|node| !node.is_subgraph_anchor()) {
        let label = node.label.as_deref().unwrap_or(&node.id);
        let (row, column) = unique_ascii_label_position(&rows, label);
        let left = rows[row][..column]
            .iter()
            .rposition(|glyph| *glyph == '|')
            .expect("node left frame remains visible");
        let right = rows[row][column + label.len()..]
            .iter()
            .position(|glyph| *glyph == '|')
            .map(|offset| column + label.len() + offset)
            .expect("node right frame remains visible");
        let top = row
            .checked_sub(2)
            .expect("default node frame has two rows above its label");
        let bottom = row + 2;
        for corner in [(left, top), (right, top), (left, bottom), (right, bottom)] {
            assert_eq!(
                rows[corner.1][corner.0], '+',
                "node frame corner must remain visible"
            );
        }
        node_frames.insert(node.id.as_str(), (left, top, right, bottom));
        if let Some(owner) = owners.get(node.id.as_str()) {
            assert!(
                contains(frames[owner], (left, top, right, bottom)),
                "the full node frame must remain inside its declared group:\n{rendered}"
            );
        }
    }
    let mut attached_heads = BTreeSet::new();
    for edge in &model.edges {
        let &(left, top, right, bottom) = node_frames
            .get(edge.to.as_str())
            .expect("these copied edges target authored nodes");
        let center_x = left + (right - left).div_ceil(2);
        let center_y = top + (bottom - top).div_ceil(2);
        let mut heads = Vec::new();
        if let Some(x) = left.checked_sub(1) {
            heads.push((x, center_y, '>'));
        }
        if let Some(y) = top.checked_sub(1) {
            heads.push((center_x, y, 'v'));
        }
        heads.extend([(right + 1, center_y, '<'), (center_x, bottom + 1, '^')]);
        let heads = heads
            .into_iter()
            .filter(|(x, y, glyph)| rows.get(*y).and_then(|row| row.get(*x)) == Some(glyph))
            .collect::<Vec<_>>();
        assert_eq!(
            heads.len(),
            1,
            "edge {} -> {} must enter its actual target with one arrowhead:\n{rendered}",
            edge.from,
            edge.to
        );
        assert!(
            attached_heads.insert(heads[0]),
            "distinct edges must retain distinct target heads"
        );
    }
    assert_eq!(
        attached_heads.len(),
        model.edges.len(),
        "every authored edge must keep its arrowhead"
    );
    for (id, frame) in &frames {
        if let Some(owner) = owners.get(id) {
            assert!(
                contains(frames[owner], *frame),
                "the full nested group frame must remain inside its parent:\n{rendered}"
            );
        }
    }
}

#[test]
fn graph_fixture_named_gaps_preserve_visible_text_and_render() {
    let mut failures = Vec::new();
    for fixture in GRAPH_FIXTURE_GAPS {
        let path = fixture_path(*fixture);
        let (input, expected) = split_fixture(&path);
        match render_flowchart(&input, &fixture.options()) {
            Ok(rendered) if !rendered.trim().is_empty() => {
                let expected_tokens = visible_text_token_counts(&expected);
                let rendered_tokens = visible_text_tokens(&rendered);
                for (token, expected_count) in expected_tokens {
                    let actual_count = visible_token_occurrences(&rendered_tokens, &token);
                    if actual_count < expected_count {
                        failures.push(format!(
                            "{}: visible token {token:?} occurs {actual_count} times, expected at least {expected_count}\n{rendered}",
                            fixture.key(),
                        ));
                    }
                }
            }
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

fn visible_text_token_counts(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for token in visible_text_tokens(text) {
        *counts.entry(token).or_default() += 1;
    }
    counts
}

fn visible_text_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let flush = |token: &mut String, tokens: &mut Vec<String>| {
        // The copied ASCII renderer uses a standalone `v` as a downward arrowhead.
        if !token.is_empty() && token != "v" {
            tokens.push(std::mem::take(token));
        } else {
            token.clear();
        }
    };

    for character in text.chars() {
        if character.is_alphanumeric() || character == '_' {
            token.push(character);
        } else {
            flush(&mut token, &mut tokens);
        }
    }
    flush(&mut token, &mut tokens);
    tokens
}

fn visible_token_occurrences(tokens: &[String], expected: &str) -> usize {
    let mut occurrences = 0usize;
    for start in 0..tokens.len() {
        let mut joined = String::new();
        for token in &tokens[start..] {
            joined.push_str(token);
            if joined == expected {
                occurrences += 1;
                break;
            }
            if joined.len() >= expected.len() || !expected.starts_with(&joined) {
                break;
            }
        }
    }
    occurrences
}

#[test]
fn graph_fixture_gap_inventory_covers_all_graph_fixtures() {
    let corpus = graph_fixture_keys(GRAPH_FIXTURE_CORPUS);
    let gaps = graph_fixture_keys(GRAPH_FIXTURE_GAPS);
    let corrections = graph_fixture_keys(GRAPH_FIXTURE_CORRECTIONS);
    assert_eq!(corpus.len(), GRAPH_FIXTURE_CORPUS.len());
    assert_eq!(gaps.len(), GRAPH_FIXTURE_GAPS.len());
    assert_eq!(corrections.len(), GRAPH_FIXTURE_CORRECTIONS.len());
    assert_eq!((corpus.len(), gaps.len(), corrections.len()), (79, 39, 7));
    assert!(
        corrections.is_subset(&corpus),
        "every correction must belong to the copied corpus"
    );
    assert!(
        corrections.is_disjoint(&gaps),
        "a corrected oracle must never become a tolerated layout gap"
    );
    let corrected_directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/graph_fixture/corrected-fixtures/ascii");
    let corrected_files = std::fs::read_dir(&corrected_directory)
        .expect("corrected snapshot directory is readable")
        .map(|entry| {
            let path = entry.expect("corrected snapshot entry is readable").path();
            assert!(
                path.is_file() && path.extension().is_some_and(|extension| extension == "txt"),
                "only named corrected text snapshots belong in {}",
                corrected_directory.display()
            );
            format!(
                "ascii/{}",
                path.file_name()
                    .expect("snapshot has a filename")
                    .to_str()
                    .expect("snapshot filename is UTF-8")
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        corrected_files, corrections,
        "corrected snapshots must cover their explicit dispositions exactly"
    );
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
