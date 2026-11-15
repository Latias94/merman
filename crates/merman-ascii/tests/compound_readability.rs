#![cfg(feature = "diagram-flowchart")]

mod support;

use merman_ascii::{AsciiLayoutProfile, AsciiRenderOptions};
use support::{assert_rectangular_terminal_grid, parse_model, render_model};

#[derive(Clone, Copy, Debug)]
struct Frame {
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

fn label_position(rows: &[Vec<char>], label: &str, rendered: &str) -> (usize, usize) {
    let needle: Vec<_> = label.chars().collect();
    let positions: Vec<_> = rows
        .iter()
        .enumerate()
        .flat_map(|(y, row)| {
            let needle = &needle;
            row.windows(needle.len())
                .enumerate()
                .filter(move |(_, part)| *part == needle.as_slice())
                .map(move |(x, _)| (x, y))
        })
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "label {label:?} must survive once:\n{rendered}"
    );
    positions[0]
}

fn node_frame(rows: &[Vec<char>], label: char, rendered: &str) -> Frame {
    let (x, y) = label_position(rows, &label.to_string(), rendered);
    let side = |ch| matches!(ch, '|' | '│' | '├' | '┤' | '+');
    let left = (0..x)
        .rev()
        .find(|x| side(rows[y][*x]))
        .unwrap_or_else(|| panic!("node {label} must have a left contour:\n{rendered}"));
    let right = (x + 1..rows[y].len())
        .find(|x| side(rows[y][*x]))
        .unwrap_or_else(|| panic!("node {label} must have a right contour:\n{rendered}"));
    let corners = |y: usize, l, r| {
        (rows[y].get(left) == Some(&'+') || rows[y].get(left) == Some(&l))
            && (rows[y].get(right) == Some(&'+') || rows[y].get(right) == Some(&r))
    };
    let top = (0..y)
        .rev()
        .find(|y| corners(*y, '┌', '┐'))
        .unwrap_or_else(|| panic!("node {label} must have a top contour:\n{rendered}"));
    let bottom = (y + 1..rows.len())
        .find(|y| corners(*y, '└', '┘'))
        .unwrap_or_else(|| panic!("node {label} must have a bottom contour:\n{rendered}"));
    Frame {
        left,
        right,
        top,
        bottom,
    }
}

fn route_directions(ch: char) -> u8 {
    match ch {
        '-' | '─' | '>' | '<' | '►' | '◄' => 2 | 8,
        '|' | '│' | '^' | 'v' | '▲' | '▼' => 1 | 4,
        '┌' => 2 | 4,
        '┐' => 8 | 4,
        '└' => 1 | 2,
        '┘' => 1 | 8,
        '├' => 1 | 2 | 4,
        '┤' => 1 | 4 | 8,
        '┬' => 2 | 4 | 8,
        '┴' => 1 | 2 | 8,
        '+' | '┼' => 1 | 2 | 4 | 8,
        _ => 0,
    }
}

fn group_frame(rows: &[Vec<char>], title: &str, rendered: &str) -> Frame {
    let (x, y) = label_position(rows, title, rendered);
    // Direction projection can place the title beside the top or bottom frame edge.
    let (left, right, top, bottom) = [(y - 1, '┌', '┐'), (y + 1, '└', '┘')]
        .into_iter()
        .find_map(|(border_y, left_corner, right_corner)| {
            let row = rows.get(border_y)?;
            let left = row[..x].iter().rposition(|ch| *ch == left_corner)?;
            let right = (x + title.len()..row.len()).find(|x| row[*x] == right_corner)?;
            let corners =
                |y: usize, l, r| rows[y].get(left) == Some(&l) && rows[y].get(right) == Some(&r);
            if border_y < y {
                let bottom = (y + 1..rows.len()).find(|y| corners(*y, '└', '┘'))?;
                Some((left, right, border_y, bottom))
            } else {
                let top = (0..y).rev().find(|y| corners(*y, '┌', '┐'))?;
                Some((left, right, top, border_y))
            }
        })
        .unwrap_or_else(|| panic!("title {title:?} must belong to a complete frame:\n{rendered}"));
    Frame {
        left,
        right,
        top,
        bottom,
    }
}

fn assert_group_frame(
    rows: &[Vec<char>],
    title: &str,
    members: &str,
    closed: bool,
    nodes: &[(char, Frame)],
    rendered: &str,
) {
    let (x, y) = label_position(rows, title, rendered);
    // Unicode corners identify frame ownership without interpreting ambiguous ASCII '+' cells.
    let frame = rendered
        .contains('┌')
        .then(|| group_frame(rows, title, rendered));
    let cramped = frame.is_some_and(|frame| frame.right - frame.left - 1 < title.len() + 2);
    let reading_cell = |column: usize| {
        rows[y].get(column) == Some(&' ')
            || (cramped
                && frame.is_some_and(|frame| column == frame.left || column == frame.right)
                && rows[y].get(column) == Some(&'│'))
    };
    assert!(
        x > 0 && reading_cell(x - 1) && reading_cell(x + title.len()),
        "title {title:?} needs a reading cell on each side with available frame space:\n{rendered}"
    );
    let Some(Frame {
        left,
        right,
        top,
        bottom,
    }) = frame
    else {
        return;
    };
    for (label, node) in nodes.iter().filter(|(label, _)| members.contains(*label)) {
        assert!(
            left < node.left && node.right < right && top < node.top && node.bottom < bottom,
            "member {label} must remain inside {title}:\n{rendered}"
        );
    }
    let horizontal = [top, bottom]
        .into_iter()
        .flat_map(|y| (left + 1..right).map(move |x| (x, y, true)));
    let vertical = [left, right]
        .into_iter()
        .flat_map(|x| (top + 1..bottom).map(move |y| (x, y, false)));
    for (x, y, horizontal) in horizontal.chain(vertical) {
        let ch = rows[y][x];
        if ch == if horizontal { '─' } else { '│' } {
            continue;
        }
        assert!(
            !closed && ch == '┼',
            "{title} border must stay intact, with only transverse crossings at ({x}, {y}):\n{rendered}"
        );
        let (dx, dy, before, after) = if horizontal {
            (0, 1, 4, 1)
        } else {
            (1, 0, 2, 8)
        };
        let continues = |forward: bool, direction| {
            let at = |distance: usize| {
                let next_y = if forward {
                    y.checked_add(dy * distance)
                } else {
                    y.checked_sub(dy * distance)
                }?;
                let next_x = if forward {
                    x.checked_add(dx * distance)
                } else {
                    x.checked_sub(dx * distance)
                }?;
                rows.get(next_y)?.get(next_x).copied()
            };
            let Some(first) = at(1) else { return false };
            // A horizontal overpass leaves exactly one cell of a vertical route
            // unpainted. Require its immediate continuation beyond that crossing.
            route_directions(first) & direction != 0
                || (horizontal
                    && first == '─'
                    && at(2).is_some_and(|second| route_directions(second) & direction != 0))
        };
        assert!(
            continues(false, before) && continues(true, after),
            "a route crossing {title} at ({x}, {y}) must continue straight on both sides of its frame:\n{rendered}"
        );
    }
}

fn assert_compound_readable(
    rendered: &str,
    expected_targets: &[char],
    groups: &[(&str, &str, bool)],
) {
    assert_rectangular_terminal_grid(rendered);
    let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
    let nodes = ['A', 'B', 'C', 'D'].map(|label| (label, node_frame(&rows, label, rendered)));
    let mut targets = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, head) in row.iter().copied().enumerate() {
            let (dx, dy) = match head {
                '>' | '►' => (1, 0),
                '<' | '◄' => (-1, 0),
                '^' | '▲' => (0, -1),
                'v' | '▼' => (0, 1),
                _ => continue,
            };
            let target_x = x.checked_add_signed(dx).unwrap();
            let target_y = y.checked_add_signed(dy).unwrap();
            let target = nodes.iter().find(|(_, frame)| {
                ((target_x == frame.left || target_x == frame.right)
                    && (frame.top + 1..frame.bottom).contains(&target_y))
                    || ((target_y == frame.top || target_y == frame.bottom)
                        && (frame.left + 1..frame.right).contains(&target_x))
            });
            targets.push(
                target
                    .unwrap_or_else(|| {
                        panic!(
                            "head ({x}, {y}) must touch its actual node, without stacking or stopping at a group frame:\n{rendered}"
                        )
                    })
                    .0,
            );
        }
    }
    targets.sort_unstable();
    assert_eq!(
        targets, expected_targets,
        "each authored incoming edge needs its own head at the correct target:\n{rendered}"
    );
    for &(title, members, closed) in groups {
        assert_group_frame(&rows, title, members, closed, &nodes, rendered);
    }
}

fn options() -> [AsciiRenderOptions; 4] {
    [
        AsciiRenderOptions::ascii(),
        AsciiRenderOptions::unicode(),
        AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Compact),
        AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Compact),
    ]
}

#[test]
fn grouped_skip_and_merge_keeps_all_five_heads_inside_its_frame() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph Group\ndirection {direction}\nA --> B & C\nB --> C & D\nD --> C\nend\n"
        );
        let model = parse_model(&source);
        for options in options() {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            assert_compound_readable(
                &rendered,
                &['B', 'C', 'C', 'C', 'D'],
                &[("Group", "ABCD", true)],
            );
        }
    }
}

#[test]
fn nested_skip_and_merge_respects_local_direction_without_crossing_ancestor_frames() {
    for (direction, local) in [("LR", "TD"), ("RL", "BT"), ("TD", "LR"), ("BT", "RL")] {
        let source = format!(
            "graph {direction}\nsubgraph outer[OUTER]\ndirection {direction}\nsubgraph inner[INNER]\ndirection {local}\nA --> B & C\nB --> C & D\nD --> C\nend\nend\n"
        );
        let model = parse_model(&source);
        for options in [options()[1], options()[2]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            assert_compound_readable(
                &rendered,
                &['B', 'C', 'C', 'C', 'D'],
                &[("OUTER", "ABCD", true), ("INNER", "ABCD", true)],
            );
            let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
            let positions = ['A', 'B', 'D', 'C']
                .map(|label| label_position(&rows, &label.to_string(), &rendered));
            assert!(
                positions.windows(2).all(|pair| match local {
                    "LR" => pair[0].0 < pair[1].0,
                    "RL" => pair[0].0 > pair[1].0,
                    "TD" => pair[0].1 < pair[1].1,
                    "BT" => pair[0].1 > pair[1].1,
                    _ => unreachable!(),
                }),
                "member order must follow the explicit inner {local} direction:\n{rendered}"
            );
        }
    }
}

#[test]
fn sibling_skip_and_merge_crosses_frames_without_stacking_three_incoming_heads() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph outer[OUTER]\nsubgraph left[LEFT]\nA\nB\nend\nsubgraph right[RIGHT]\nC\nD\nend\nA --> B & C\nB --> C & D\nD --> C\nend\n"
        );
        let model = parse_model(&source);
        for options in [options()[0], options()[3]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            assert_compound_readable(
                &rendered,
                &['B', 'C', 'C', 'C', 'D'],
                &[
                    ("OUTER", "ABCD", true),
                    ("LEFT", "AB", false),
                    ("RIGHT", "CD", false),
                ],
            );
        }
    }
}

#[test]
fn sibling_feedback_keeps_its_label_and_six_independent_target_contacts() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph left[LEFT]\ndirection {direction}\nA --> B\nend\nsubgraph right[RIGHT]\ndirection {direction}\nC --> D\nend\nA --> C\nB --> C & D\nD -->|retry| B\n"
        );
        let model = parse_model(&source);
        for options in [options()[1], options()[2]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            assert_eq!(rendered.matches("retry").count(), 1, "{rendered}");
            assert_compound_readable(
                &rendered,
                &['B', 'B', 'C', 'C', 'D', 'D'],
                &[("LEFT", "AB", false), ("RIGHT", "CD", false)],
            );
        }
    }
}

#[test]
fn nested_sibling_chain_keeps_complete_sibling_frames_disjoint() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph p[Pipeline]\nsubgraph left[Source]\ndirection LR\nA --> B\nend\nsubgraph right[Sink]\ndirection LR\nD --> C\nend\nB --> D\nend\n"
        );
        let model = parse_model(&source);
        for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
            let options = AsciiRenderOptions::unicode().with_layout_profile(profile);
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            assert_compound_readable(
                &rendered,
                &['B', 'C', 'D'],
                &[
                    ("Pipeline", "ABCD", true),
                    ("Source", "AB", false),
                    ("Sink", "CD", false),
                ],
            );
            let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
            let pipeline = group_frame(&rows, "Pipeline", &rendered);
            let source_frame = group_frame(&rows, "Source", &rendered);
            let sink_frame = group_frame(&rows, "Sink", &rendered);
            for child in [source_frame, sink_frame] {
                assert!(
                    pipeline.left < child.left
                        && child.right < pipeline.right
                        && pipeline.top < child.top
                        && child.bottom < pipeline.bottom,
                    "each complete sibling frame must remain inside Pipeline:\n{rendered}"
                );
            }
            assert!(
                source_frame.right < sink_frame.left
                    || sink_frame.right < source_frame.left
                    || source_frame.bottom < sink_frame.top
                    || sink_frame.bottom < source_frame.top,
                "Source and Sink must have disjoint complete frames:\n{rendered}"
            );
        }
    }
}
