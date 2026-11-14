use super::*;

fn readability_options() -> [AsciiRenderOptions; 4] {
    [
        AsciiRenderOptions::ascii(),
        AsciiRenderOptions::unicode(),
        AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Compact),
        AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Compact),
    ]
}

fn assert_readable_target_heads(rendered: &str, expected: usize) {
    // These rectangular-node fixtures have no arrow characters in their labels.
    let markers: Vec<_> = rendered
        .lines()
        .enumerate()
        .flat_map(|(y, line)| {
            line.char_indices()
                .filter(|(_, ch)| matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼'))
                .map(move |(offset, ch)| (terminal_test_width(&line[..offset]), y, ch))
        })
        .collect();
    assert_eq!(
        markers.len(),
        expected,
        "every semantic edge must retain its target marker:\n{rendered}"
    );

    for &(x, y, ch) in &markers {
        let neighbor = if matches!(ch, '>' | '<' | '►' | '◄') {
            (x + 1, y, ch)
        } else {
            (x, y + 1, ch)
        };
        assert!(
            !markers.contains(&neighbor),
            "target markers must not stack along their pointing axis at ({x}, {y}):\n{rendered}"
        );
    }
    assert_rectangular_terminal_grid(rendered);
}

fn assert_labels_once(rendered: &str, labels: &[&str]) {
    for &label in labels {
        assert_eq!(
            rendered.matches(label).count(),
            1,
            "label {label:?} must remain readable exactly once:\n{rendered}"
        );
    }
}

fn assert_node_order(rendered: &str, direction: &str, labels: &[&str]) {
    let positions: Vec<_> = labels
        .iter()
        .map(|&label| {
            rendered
                .lines()
                .enumerate()
                .find_map(|(y, line)| {
                    line.split_once(label)
                        .map(|(prefix, _)| (terminal_test_width(prefix), y))
                })
                .unwrap_or_else(|| panic!("missing node {label:?}:\n{rendered}"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|pair| match direction {
            "LR" => pair[0].0 < pair[1].0,
            "RL" => pair[0].0 > pair[1].0,
            "TD" => pair[0].1 < pair[1].1,
            "BT" => pair[0].1 > pair[1].1,
            _ => panic!("unexpected test direction {direction}"),
        }),
        "node order {labels:?} must follow {direction}:\n{rendered}"
    );
}

#[test]
fn four_to_six_incoming_edges_keep_unstacked_target_heads() {
    for incoming in 4..=6 {
        for direction in ["TD", "BT", "LR", "RL"] {
            let mut source = format!("flowchart {direction}\n");
            for node in ['A', 'B', 'C', 'D', 'E', 'F'].into_iter().take(incoming) {
                source.push_str(&format!("{node} --> T\n"));
            }
            for options in readability_options() {
                let rendered = render_flowchart(&source, &options).unwrap_or_else(|error| {
                    panic!("{incoming} incoming edges must render ({direction}): {error}")
                });
                assert_readable_target_heads(&rendered, incoming);
                assert_labels_once(&rendered, &["T"]);
                for node in ["A", "B", "C", "D", "E", "F"].into_iter().take(incoming) {
                    assert_labels_once(&rendered, &[node]);
                    assert_node_order(&rendered, direction, &[node, "T"]);
                }
            }
        }
    }
}

#[test]
fn six_way_branch_and_merge_preserves_all_twelve_target_heads() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nS --> A & B & C & D & E & F\nA & B & C & D & E & F --> T\n"
        );
        for options in readability_options() {
            let rendered = render_flowchart(&source, &options)
                .expect("a six-way branch and merge must keep every target marker readable");
            assert_readable_target_heads(&rendered, 12);
            assert_labels_once(&rendered, &["S", "A", "B", "C", "D", "E", "F", "T"]);
            for node in ["A", "B", "C", "D", "E", "F"] {
                assert_node_order(&rendered, direction, &["S", node, "T"]);
            }
        }
    }
}

#[test]
fn mixed_stroke_fork_keeps_each_branch_style_and_target_head() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!("flowchart {direction}\nS --> A\nS -.-> B\nS ==> C\n");
        for (options, dotted, thick) in [
            (AsciiRenderOptions::ascii(), ['.', ':'], ['=', '#']),
            (AsciiRenderOptions::unicode(), ['┄', '┆'], ['━', '┃']),
        ] {
            let rendered = render_flowchart(&source, &options)
                .expect("a three-way fork must route normal, dotted, and thick branches together");
            assert_readable_target_heads(&rendered, 3);
            assert_labels_once(&rendered, &["S", "A", "B", "C"]);
            assert!(
                rendered.chars().any(|ch| dotted.contains(&ch)),
                "the dotted branch must retain a visible dotted segment:\n{rendered}"
            );
            assert!(
                rendered.chars().any(|ch| thick.contains(&ch)),
                "the thick branch must retain a visible thick segment:\n{rendered}"
            );
            for node in ["A", "B", "C"] {
                assert_node_order(&rendered, direction, &["S", node]);
            }
        }
    }
}

#[test]
fn mixed_stroke_six_way_merge_keeps_its_exit_and_incoming_heads() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nA --> T\nB -.-> T\nC ==> T\nD --> T\nE -.-> T\nF ==> T\nT --> Z\n"
        );
        for options in readability_options() {
            let rendered = render_flowchart(&source, &options)
                .expect("six mixed-stroke incoming edges must not prevent routing the exit");
            assert_readable_target_heads(&rendered, 7);
            assert_labels_once(&rendered, &["A", "B", "C", "D", "E", "F", "T", "Z"]);
            for node in ["A", "B", "C", "D", "E", "F"] {
                assert_node_order(&rendered, direction, &[node, "T", "Z"]);
            }
        }
    }
}

#[test]
fn mixed_stroke_skip_and_merge_routes_independently_of_edge_order() {
    let edges = [
        "A --> B\n",
        "A -.-> D\n",
        "B ==> C\n",
        "B --> D\n",
        "C -.-> D\n",
        "D --> E\n",
    ];
    for direction in ["TD", "BT", "LR", "RL"] {
        for order in [[0, 1, 2, 3, 4, 5], [5, 4, 3, 2, 1, 0], [1, 4, 3, 2, 0, 5]] {
            let mut source = format!("flowchart {direction}\nA\nB\nC\nD\nE\n");
            for index in order {
                source.push_str(edges[index]);
            }
            for options in readability_options() {
                let rendered = render_flowchart(&source, &options).unwrap_or_else(|error| {
                    panic!("mixed skip edges must render ({direction}, {order:?}): {error}")
                });
                assert_readable_target_heads(&rendered, 6);
                assert_labels_once(&rendered, &["A", "B", "C", "D", "E"]);
                assert_node_order(&rendered, direction, &["A", "B", "C", "D", "E"]);
            }
        }
    }
}

#[test]
fn wide_labeled_feedback_and_merge_keep_unstacked_target_heads() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nS[開始] --> A[確認待ち]\nA -->|承認済み| B[保存先]\nA -->|差し戻し| C[修正待ち]\nC -->|再試行| A\nB --> T[完了]\nC --> T\n"
        );
        for options in readability_options() {
            let rendered = render_flowchart(&source, &options)
                .expect("wide labels must leave independent heads for feedback and merge edges");
            assert_readable_target_heads(&rendered, 6);
            assert_labels_once(
                &rendered,
                &[
                    "開始",
                    "確認待ち",
                    "承認済み",
                    "保存先",
                    "差し戻し",
                    "修正待ち",
                    "再試行",
                    "完了",
                ],
            );
            assert_node_order(&rendered, direction, &["開始", "確認待ち"]);
            for branch in ["保存先", "修正待ち"] {
                assert_node_order(&rendered, direction, &[branch, "完了"]);
            }
        }
    }
}

#[test]
fn parallel_self_loop_labels_leave_the_following_exit_clear() {
    for direction in ["TD", "BT", "LR", "RL"] {
        for exit in ["A --> B", "A -->|passed| B"] {
            let source = format!(
                "flowchart {direction}\nA -->|alpha| A\nA -- beta --o A\nA -- gamma --x A\n{exit}\n"
            );
            for options in readability_options() {
                let rendered = render_flowchart(&source, &options).expect(
                    "parallel self-loop labels must leave the following exit a valid route",
                );
                assert_labels_once(&rendered, &["alpha", "beta", "gamma", "A", "B"]);
                if exit.contains('|') {
                    assert_labels_once(&rendered, &["passed"]);
                }
                assert_readable_target_heads(&rendered, 2);
                assert_eq!(
                    rendered
                        .chars()
                        .filter(|ch| matches!(ch, 'o' | '○'))
                        .count(),
                    1,
                    "{rendered}"
                );
                assert_eq!(
                    rendered
                        .chars()
                        .filter(|ch| matches!(ch, 'x' | '×'))
                        .count(),
                    1,
                    "{rendered}"
                );
            }
        }
    }
}

#[test]
fn parallel_long_self_loop_labels_leave_the_following_exit_clear() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nA[ENTRY] -->|first retry after check| A\nA -->|second retry after check| A\nA -->|third retry after check| A\nA -->|accept after checks| B[DONE]\n"
        );
        for options in readability_options() {
            let rendered = render_flowchart(&source, &options).unwrap_or_else(|error| {
                panic!("long parallel self-loop labels must leave an exit ({direction}): {error}")
            });
            assert_labels_once(
                &rendered,
                &[
                    "ENTRY",
                    "DONE",
                    "first retry after check",
                    "second retry after check",
                    "third retry after check",
                    "accept after checks",
                ],
            );
            assert_readable_target_heads(&rendered, 4);
        }
    }
}

#[test]
fn multiline_parallel_feedback_keeps_labels_and_heads_with_zero_preferred_padding() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nA[ENTRY] -->|initial<br/>screening| B[CHECK]\nB -->|first retry<br/>after check| A\nB -->|second retry<br/>after check| A\nB -->|third retry<br/>after check| A\nB -->|accept<br/>after checks| T[DONE]\n"
        );
        for options in readability_options() {
            let options = options.with_graph_padding_x(0).with_graph_padding_y(0);
            let rendered = render_flowchart(&source, &options).unwrap_or_else(|error| {
                panic!(
                    "multiline parallel feedback must retain readable lanes ({direction}): {error}"
                )
            });
            assert_labels_once(
                &rendered,
                &[
                    "ENTRY",
                    "CHECK",
                    "DONE",
                    "initial",
                    "screening",
                    "first retry",
                    "second retry",
                    "third retry",
                    "accept",
                ],
            );
            assert_readable_target_heads(&rendered, 5);
        }
    }
}

#[test]
fn labeled_parallel_reverse_edges_keep_independent_target_heads() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nA[ENTRY] -->|send primary request| B[CHECK]\nB -->|first failure response| A\nA -->|send second request| B\nB -->|second failure response| A\n"
        );
        for options in readability_options() {
            let rendered = render_flowchart(&source, &options).unwrap_or_else(|error| {
                panic!("labeled parallel reverse edges must keep independent heads ({direction}): {error}")
            });
            assert_labels_once(
                &rendered,
                &[
                    "ENTRY",
                    "CHECK",
                    "send primary request",
                    "send second request",
                    "first failure response",
                    "second failure response",
                ],
            );
            assert_readable_target_heads(&rendered, 4);
        }
    }
}

#[test]
fn long_labeled_mixed_feedback_keeps_every_label_and_target_head() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            concat!(
                "flowchart {}\n",
                "A[ENTRY] -->|accepted after mandatory checks| B[CHECK]\n",
                "A -.->|pending additional checks| C[RETRY]\n",
                "B ==>|ready after checks| T[DONE]\n",
                "C -->|needs additional checks| T\n",
                "B -->|repeat after failed checks| A\n",
            ),
            direction
        );
        for options in readability_options() {
            let rendered = render_flowchart(&source, &options)
                .expect("long labeled mixed-stroke branches must render with their feedback edge");
            assert_readable_target_heads(&rendered, 5);
            assert_labels_once(
                &rendered,
                &[
                    "ENTRY",
                    "CHECK",
                    "RETRY",
                    "DONE",
                    "accepted after mandatory checks",
                    "pending additional checks",
                    "ready after checks",
                    "needs additional checks",
                    "repeat after failed checks",
                ],
            );
            for branch in ["ENTRY", "CHECK", "RETRY"] {
                assert_node_order(&rendered, direction, &[branch, "DONE"]);
            }
        }
    }
}
