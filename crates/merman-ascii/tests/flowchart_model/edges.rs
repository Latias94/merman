use super::*;

#[test]
fn flowchart_parser_open_edges_render_without_arrowhead() {
    let rendered = render_flowchart("flowchart LR\nA --- B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |-----| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_edge_length_modifiers_add_spacing() {
    let rendered =
        render_flowchart("flowchart LR\nA ----> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+         +---+\n|   |         |   |\n| A |-------->| B |\n|   |         |   |\n+---+         +---+\n"
    );
}

#[test]
fn flowchart_parser_self_loops_preserve_labels_in_all_directions() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let rendered = render_flowchart(
            &format!("flowchart {direction}\nA -->|loop-{direction}| A"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("{direction} self-loop should render: {error:?}"));

        assert!(rendered.contains('A'), "{direction}:\n{rendered}");
        assert!(
            rendered.contains(&format!("loop-{direction}")),
            "{direction} self-loop should preserve its label:\n{rendered}"
        );
        assert!(
            rendered
                .chars()
                .any(|ch| matches!(ch, '>' | '<' | '^' | 'v')),
            "{direction} self-loop should preserve its target marker:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_self_loops_preserve_independent_source_and_target_markers() {
    for direction in ["TD", "BT", "LR", "RL"] {
        for (options, circle_marker, cross_marker) in [
            (AsciiRenderOptions::ascii(), 'o', 'x'),
            (AsciiRenderOptions::unicode(), '○', '×'),
        ] {
            let rendered = render_flowchart(&format!("flowchart {direction}\nA o--x A"), &options)
                .unwrap_or_else(|error| {
                    panic!("{direction} double-ended self-loop should render: {error:?}")
                });

            let circle = rendered
                .lines()
                .enumerate()
                .find_map(|(y, line)| line.find(circle_marker).map(|x| (x, y)))
                .unwrap_or_else(|| panic!("missing source circle marker:\n{rendered}"));
            let cross = rendered
                .lines()
                .enumerate()
                .find_map(|(y, line)| line.find(cross_marker).map(|x| (x, y)))
                .unwrap_or_else(|| panic!("missing target cross marker:\n{rendered}"));
            assert_ne!(
                circle, cross,
                "{direction} self-loop endpoint markers need independent berths:\n{rendered}"
            );
            assert_rectangular_char_grid(&rendered);
        }
    }
}

#[test]
fn flowchart_parser_parallel_self_loops_keep_independent_lanes_in_all_directions() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let rendered = render_flowchart(
            &format!("flowchart {direction}\nA -->|alpha| A\nA -- beta --o A\nA -- gamma --x A"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("{direction} parallel self-loops should render: {error:?}"));

        for label in ["alpha", "beta", "gamma"] {
            assert_eq!(
                rendered.matches(label).count(),
                1,
                "{direction} should preserve self-loop label {label:?}:\n{rendered}"
            );
        }

        let marker_cells = rendered
            .lines()
            .enumerate()
            .flat_map(|(y, line)| {
                line.chars().enumerate().filter_map(move |(x, ch)| {
                    matches!(ch, '>' | '<' | '^' | 'v' | 'o' | 'x').then_some((x, y, ch))
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            marker_cells.len(),
            3,
            "{direction} should retain exactly one marker per self-loop:\n{rendered}"
        );
        assert!(
            marker_cells.iter().any(|(_, _, ch)| *ch == 'o'),
            "{direction} should retain the circle marker:\n{rendered}"
        );
        assert!(
            marker_cells.iter().any(|(_, _, ch)| *ch == 'x'),
            "{direction} should retain the cross marker:\n{rendered}"
        );
        assert!(
            marker_cells
                .iter()
                .any(|(_, _, ch)| matches!(*ch, '>' | '<' | '^' | 'v')),
            "{direction} should retain the point marker:\n{rendered}"
        );
        let mut marker_coords = marker_cells
            .iter()
            .map(|(x, y, _)| (*x, *y))
            .collect::<Vec<_>>();
        marker_coords.sort_unstable();
        marker_coords.dedup();
        assert_eq!(
            marker_coords.len(),
            3,
            "{direction} self-loop markers must occupy independent coordinates:\n{rendered}"
        );

        let mut label_coords = ["alpha", "beta", "gamma"]
            .into_iter()
            .map(|label| {
                rendered
                    .lines()
                    .enumerate()
                    .find_map(|(y, line)| line.find(label).map(|x| (x, y)))
                    .unwrap_or_else(|| panic!("missing {label:?} in rendered fixture:\n{rendered}"))
            })
            .collect::<Vec<_>>();
        label_coords.sort_unstable();
        label_coords.dedup();
        assert_eq!(
            label_coords.len(),
            3,
            "{direction} self-loop labels must occupy independent lanes:\n{rendered}"
        );
        assert_rectangular_char_grid(&rendered);
    }
}

#[test]
fn flowchart_parser_three_parallel_edges_keep_distinct_labels_and_markers() {
    for direction in ["TD", "LR"] {
        let rendered = render_flowchart(
            &format!("flowchart {direction}\nA -->|first| B\nA -->|second| B\nA -->|third| B"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("{direction} parallel edges should render: {error:?}"));

        for label in ["first", "second", "third"] {
            assert_eq!(
                rendered.matches(label).count(),
                1,
                "{direction} should preserve the parallel label {label:?}:\n{rendered}"
            );
        }
        let marker_coords = rendered
            .lines()
            .enumerate()
            .flat_map(|(y, line)| {
                line.chars().enumerate().filter_map(move |(x, ch)| {
                    matches!(ch, '>' | '<' | '^' | 'v').then_some((x, y))
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            marker_coords.len(),
            3,
            "{direction} should preserve one independently positioned target marker per parallel edge:\n{rendered}"
        );
        let mut unique_marker_coords = marker_coords.clone();
        unique_marker_coords.sort_unstable();
        unique_marker_coords.dedup();
        assert_eq!(
            unique_marker_coords.len(),
            3,
            "{direction} parallel markers should occupy independent coordinates:\n{rendered}"
        );
        let mut label_coords = ["first", "second", "third"]
            .into_iter()
            .map(|label| {
                rendered
                    .lines()
                    .enumerate()
                    .find_map(|(y, line)| line.find(label).map(|x| (x, y)))
                    .unwrap_or_else(|| panic!("missing {label:?} in rendered fixture:\n{rendered}"))
            })
            .collect::<Vec<_>>();
        label_coords.sort_unstable();
        label_coords.dedup();
        assert_eq!(
            label_coords.len(),
            3,
            "{direction} parallel labels should occupy independent lane coordinates:\n{rendered}"
        );
    }
}
