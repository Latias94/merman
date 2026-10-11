mod support;

use merman_ascii::{AsciiLayoutProfile, AsciiRenderOptions};
use support::{parse_model, render_model};

#[derive(Clone, Copy, Debug)]
struct Frame {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

fn frame_for_label(rows: &[Vec<char>], label: &str, context: &str) -> Frame {
    let needle = label.chars().collect::<Vec<_>>();
    let matches = rows
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            let needle = &needle;
            row.windows(needle.len())
                .enumerate()
                .filter_map(move |(x, part)| (part == needle).then_some((x, y)))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{label} must survive exactly once: {context}"
    );
    let (label_x, label_y) = matches[0];
    for top in (0..label_y).rev() {
        for left in (0..=label_x).rev() {
            if rows[top].get(left) != Some(&'┌') {
                continue;
            }
            for right in label_x + needle.len()..rows[top].len() {
                if rows[top][right] != '┐' {
                    continue;
                }
                for bottom in label_y + 1..rows.len() {
                    if rows[bottom].get(left) == Some(&'└') && rows[bottom].get(right) == Some(&'┘')
                    {
                        // Four unrelated corner glyphs can line up across nested frames. Only
                        // accept one connected perimeter, then let the independent containment
                        // assertions reject any damaged child that fell back to its ancestor.
                        let horizontal_closed = [top, bottom].into_iter().all(|border| {
                            rows[border][left + 1..right].iter().all(|ch| *ch == '─')
                        });
                        let vertical_closed = rows[top + 1..bottom]
                            .iter()
                            .all(|row| row.get(left) == Some(&'│') && row.get(right) == Some(&'│'));
                        if horizontal_closed && vertical_closed {
                            return Frame {
                                left,
                                top,
                                right,
                                bottom,
                            };
                        }
                    }
                }
            }
        }
    }
    panic!("complete closed frame enclosing {label} must survive: {context}");
}

fn assert_contains(parent: Frame, child: Frame, context: &str) {
    assert!(
        parent.left < child.left
            && child.right < parent.right
            && parent.top < child.top
            && child.bottom < parent.bottom,
        "{context}: a complete child frame must stay strictly inside its parent: {parent:?}, {child:?}"
    );
}

fn assert_gutter(left: Frame, right: Frame, context: &str) {
    assert!(
        left.right + 1 < right.left
            || right.right + 1 < left.left
            || left.bottom + 1 < right.top
            || right.bottom + 1 < left.top,
        "{context}: unrelated complete frames need an empty row or column: {left:?}, {right:?}"
    );
}

#[test]
fn separate_empty_subtrees_keep_all_titles_and_complete_frames_in_every_scope() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for reverse in [false, true] {
            for deep in [false, true] {
                let nested = if deep { "subgraph D1\nend\n" } else { "" };
                let first = format!("subgraph P1\nsubgraph E1\n{nested}end\nend\n");
                let second = "subgraph P2\nsubgraph E2\nend\nend\n";
                for external in [false, true] {
                    let groups = if reverse {
                        format!("{second}{first}")
                    } else {
                        format!("{first}{second}")
                    };
                    let outside = if external { "C[CCCC]\n" } else { "" };
                    let source = format!("flowchart {direction}\n{groups}{outside}");
                    let model = parse_model(&source);
                    for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                        for zero_padding in [false, true] {
                            let mut options =
                                AsciiRenderOptions::unicode().with_layout_profile(profile);
                            if zero_padding {
                                options = options
                                    .with_node_padding_x(0)
                                    .with_node_padding_y(0)
                                    .with_graph_padding_x(0)
                                    .with_graph_padding_y(0);
                            }
                            let text = render_model(&model, &options)
                                .expect("empty compound subtrees must render");
                            let rows = text
                                .lines()
                                .map(|line| line.chars().collect())
                                .collect::<Vec<Vec<char>>>();
                            let context = format!(
                                "{direction} {profile:?}, reverse={reverse}, deep={deep}, external={external}, zero={zero_padding}\n{text}"
                            );
                            let p1 = frame_for_label(&rows, "P1", &context);
                            let e1 = frame_for_label(&rows, "E1", &context);
                            let p2 = frame_for_label(&rows, "P2", &context);
                            let e2 = frame_for_label(&rows, "E2", &context);
                            assert_contains(p1, e1, &context);
                            assert_contains(p2, e2, &context);
                            assert_gutter(p1, p2, &context);
                            if deep {
                                assert_contains(
                                    e1,
                                    frame_for_label(&rows, "D1", &context),
                                    &context,
                                );
                            }
                            if external {
                                let node = frame_for_label(&rows, "CCCC", &context);
                                assert_gutter(p1, node, &context);
                                assert_gutter(p2, node, &context);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mixed_empty_member_subtrees_do_not_follow_foreign_sibling_nodes() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for reverse in [false, true] {
            for empty_first in [false, true] {
                for deep in [false, true] {
                    let nested = if deep { "subgraph Deep\nend\n" } else { "" };
                    let empty = format!("subgraph Empty\n{nested}end\n");
                    let members = if empty_first {
                        format!("{empty}A[AAA]\n")
                    } else {
                        format!("A[AAA]\n{empty}")
                    };
                    let left = format!("subgraph Left\n{members}end\n");
                    let right = "subgraph Right\nB[BBB]\nend\n";
                    let groups = if reverse {
                        format!("{right}{left}")
                    } else {
                        format!("{left}{right}")
                    };
                    let source = format!("flowchart {direction}\n{groups}C[CCCC]\n");
                    let model = parse_model(&source);
                    for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                        for zero_padding in [false, true] {
                            let mut options =
                                AsciiRenderOptions::unicode().with_layout_profile(profile);
                            if zero_padding {
                                options = options
                                    .with_node_padding_x(0)
                                    .with_node_padding_y(0)
                                    .with_graph_padding_x(0)
                                    .with_graph_padding_y(0);
                            }
                            let text = render_model(&model, &options)
                                .expect("mixed compound siblings must render");
                            let rows = text
                                .lines()
                                .map(|line| line.chars().collect())
                                .collect::<Vec<Vec<char>>>();
                            let context = format!(
                                "{direction} {profile:?}, reverse={reverse}, empty_first={empty_first}, deep={deep}, zero={zero_padding}\n{text}"
                            );
                            let left = frame_for_label(&rows, "Left", &context);
                            let right = frame_for_label(&rows, "Right", &context);
                            let empty = frame_for_label(&rows, "Empty", &context);
                            let a = frame_for_label(&rows, "AAA", &context);
                            let b = frame_for_label(&rows, "BBB", &context);
                            let c = frame_for_label(&rows, "CCCC", &context);
                            assert_contains(left, empty, &context);
                            assert_contains(left, a, &context);
                            assert_contains(right, b, &context);
                            assert_gutter(left, right, &context);
                            assert_gutter(left, c, &context);
                            assert_gutter(right, c, &context);
                            assert_gutter(empty, a, &context);
                            if deep {
                                assert_contains(
                                    empty,
                                    frame_for_label(&rows, "Deep", &context),
                                    &context,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
