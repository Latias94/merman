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

impl Frame {
    fn contains(self, x: usize, y: usize) -> bool {
        self.left < x && x < self.right && self.top < y && y < self.bottom
    }

    fn encloses(self, other: Self) -> bool {
        self.contains(other.left, other.top) && self.contains(other.right, other.bottom)
    }

    fn disjoint(self, other: Self) -> bool {
        self.right < other.left
            || other.right < self.left
            || self.bottom < other.top
            || other.bottom < self.top
    }

    fn touches(self, x: usize, y: usize, dx: isize, dy: isize) -> bool {
        let (Some(x), Some(y)) = (x.checked_add_signed(dx), y.checked_add_signed(dy)) else {
            return false;
        };
        (dy == 0 && (x == self.left || x == self.right) && self.top < y && y < self.bottom)
            || (dx == 0 && (y == self.top || y == self.bottom) && self.left < x && x < self.right)
    }
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
                .filter(move |(x, part)| {
                    *part == needle.as_slice()
                        && (*x == 0 || !row[*x - 1].is_alphanumeric())
                        && row
                            .get(*x + needle.len())
                            .is_none_or(|ch| !ch.is_alphanumeric())
                })
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

fn frame_around(rows: &[Vec<char>], label: &str, rendered: &str) -> Frame {
    let (x, y) = label_position(rows, label, rendered);
    let horizontal = |ch: &char| {
        matches!(
            ch,
            '-' | '─' | '+' | '┬' | '┴' | '┼' | '┰' | '┸' | '┿' | '╂'
        )
    };
    let vertical = |ch: char| {
        matches!(
            ch,
            '|' | '│' | '+' | '├' | '┤' | '┼' | '┝' | '┥' | '┿' | '╂'
        )
    };
    for top in (0..y).rev() {
        for left in (0..x).rev() {
            if !matches!(rows[top][left], '+' | '┌') {
                continue;
            }
            for right in x + label.len()..rows[top].len() {
                if !matches!(rows[top][right], '+' | '┐')
                    || !rows[top][left + 1..right].iter().all(horizontal)
                {
                    continue;
                }
                for bottom in y + 1..rows.len() {
                    if matches!(rows[bottom][left], '+' | '└')
                        && matches!(rows[bottom][right], '+' | '┘')
                        && rows[bottom][left + 1..right].iter().all(horizontal)
                        && (top + 1..bottom)
                            .all(|y| vertical(rows[y][left]) && vertical(rows[y][right]))
                    {
                        return Frame {
                            left,
                            right,
                            top,
                            bottom,
                        };
                    }
                }
            }
        }
    }
    panic!("label {label:?} must belong to an intact rectangular frame:\n{rendered}");
}

fn assert_scene<'a>(
    rendered: &str,
    labels: &[&'a str],
    expected_targets: &[(&str, bool)],
) -> Vec<(&'a str, Frame)> {
    assert_rectangular_terminal_grid(rendered);
    let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
    let frames: Vec<_> = labels
        .iter()
        .map(|label| (*label, frame_around(&rows, label, rendered)))
        .collect();
    let mut targets = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, head) in row.iter().enumerate() {
            let (dx, dy) = match head {
                '>' | '►' => (1, 0),
                '<' | '◄' => (-1, 0),
                '^' | '▲' => (0, -1),
                'v' | '▼' => (0, 1),
                _ => continue,
            };
            let contacts: Vec<_> = frames
                .iter()
                .filter(|(_, frame)| frame.touches(x, y, dx, dy))
                .collect();
            assert_eq!(
                contacts.len(),
                1,
                "head ({x}, {y}) must touch its actual target frame without stacking:\n{rendered}"
            );
            let (label, frame) = contacts[0];
            targets.push((*label, frame.contains(x, y)));
        }
    }
    targets.sort_unstable();
    let mut expected = expected_targets.to_vec();
    expected.sort_unstable();
    assert_eq!(
        targets, expected,
        "each edge needs its own target contact on the correct side of the frame:\n{rendered}"
    );
    frames
}

fn frame(frames: &[(&str, Frame)], label: &str) -> Frame {
    frames.iter().find(|(name, _)| *name == label).unwrap().1
}

fn assert_isolated_member(rendered: &str, group: Frame, member: Frame, title: &str) {
    assert!(
        group.encloses(member),
        "{title} must contain its member:\n{rendered}"
    );
    let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
    let (title_x, title_y) = label_position(&rows, title, rendered);
    for (y, row) in rows
        .iter()
        .enumerate()
        .take(group.bottom)
        .skip(group.top + 1)
    {
        for (x, ch) in row
            .iter()
            .enumerate()
            .take(group.right)
            .skip(group.left + 1)
        {
            if ((member.left..=member.right).contains(&x)
                && (member.top..=member.bottom).contains(&y))
                || (y == title_y && (title_x..title_x + title.len()).contains(&x))
            {
                continue;
            }
            assert_eq!(
                *ch, ' ',
                "an exterior group endpoint must not route through {title}'s member or interior:\n{rendered}"
            );
        }
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
fn group_fanin_uses_three_distinct_exterior_frame_contacts() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source =
            format!("graph {direction}\nsubgraph g[BUFFER]\nA\nend\nB --> g\nC --> g\nD --> g\n");
        let model = parse_model(&source);
        for options in options() {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            let frames = assert_scene(
                &rendered,
                &["BUFFER", "A", "B", "C", "D"],
                &[("BUFFER", false); 3],
            );
            assert_isolated_member(
                &rendered,
                frame(&frames, "BUFFER"),
                frame(&frames, "A"),
                "BUFFER",
            );
        }
    }
}

#[test]
fn group_fanout_starts_at_the_group_and_reaches_each_exterior_node() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!("graph {direction}\nsubgraph g[BUFFER]\nA\nend\ng --> B & C & D\n");
        let model = parse_model(&source);
        for options in options() {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            let frames = assert_scene(
                &rendered,
                &["BUFFER", "A", "B", "C", "D"],
                &[("B", false), ("C", false), ("D", false)],
            );
            assert_isolated_member(
                &rendered,
                frame(&frames, "BUFFER"),
                frame(&frames, "A"),
                "BUFFER",
            );
        }
    }
}

#[test]
fn parallel_labeled_group_edges_preserve_both_labels_and_sink_contacts() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph g[SOURCE]\nA\nend\nsubgraph h[SINK]\nB\nend\ng -->|fast| h\ng -->|safe| h\n"
        );
        let model = parse_model(&source);
        for options in [options()[1], options()[2]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            for label in ["fast", "safe"] {
                assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
            }
            let frames = assert_scene(
                &rendered,
                &["SOURCE", "SINK", "A", "B"],
                &[("SINK", false); 2],
            );
            assert!(
                frame(&frames, "SOURCE").disjoint(frame(&frames, "SINK")),
                "{rendered}"
            );
            for (title, member) in [("SOURCE", "A"), ("SINK", "B")] {
                assert_isolated_member(
                    &rendered,
                    frame(&frames, title),
                    frame(&frames, member),
                    title,
                );
            }
        }
    }
}

#[test]
fn two_labeled_feedback_edges_return_to_the_group_without_touching_its_member() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph g[BUFFER]\nA\nend\ng -->|send| B\nB --> C\nC -->|retry| g\nB -->|again| g\n"
        );
        let model = parse_model(&source);
        for options in [options()[0], options()[3]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            for label in ["send", "retry", "again"] {
                assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
            }
            let frames = assert_scene(
                &rendered,
                &["BUFFER", "A", "B", "C"],
                &[
                    ("BUFFER", false),
                    ("BUFFER", false),
                    ("B", false),
                    ("C", false),
                ],
            );
            assert_isolated_member(
                &rendered,
                frame(&frames, "BUFFER"),
                frame(&frames, "A"),
                "BUFFER",
            );
        }
    }
}

#[test]
fn nested_group_endpoint_stops_at_the_inner_frame_and_keeps_parent_members_inside() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph p[PIPELINE]\nsubgraph g[BUFFER]\nA\nend\nB\nend\nC -->|in| g\ng -->|out| B\n"
        );
        let model = parse_model(&source);
        for options in [options()[1], options()[2]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            for label in ["in", "out"] {
                assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
            }
            let frames = assert_scene(
                &rendered,
                &["PIPELINE", "BUFFER", "A", "B", "C"],
                &[("BUFFER", false), ("B", false)],
            );
            let parent = frame(&frames, "PIPELINE");
            let group = frame(&frames, "BUFFER");
            assert!(parent.encloses(group), "{rendered}");
            assert!(parent.encloses(frame(&frames, "B")), "{rendered}");
            assert!(group.disjoint(frame(&frames, "B")), "{rendered}");
            assert!(parent.disjoint(frame(&frames, "C")), "{rendered}");
            assert_isolated_member(&rendered, group, frame(&frames, "A"), "BUFFER");
        }
    }
}

#[test]
fn ancestor_and_member_edges_use_the_interior_side_of_the_group() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for (edge, label, target) in [
            ("g -->|in| A", "in", ("A", false)),
            ("A -->|out| g", "out", ("BUFFER", true)),
        ] {
            let source = format!("graph {direction}\nsubgraph g[BUFFER]\nA\nend\n{edge}\n");
            let model = parse_model(&source);
            for options in [options()[0], options()[3]] {
                let rendered = render_model(&model, &options)
                    .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
                assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
                let frames = assert_scene(&rendered, &["BUFFER", "A"], &[target]);
                let group = frame(&frames, "BUFFER");
                assert!(group.encloses(frame(&frames, "A")), "{rendered}");
                for (y, row) in rendered.lines().enumerate() {
                    for (x, ch) in row.chars().enumerate() {
                        if x < group.left || x > group.right || y < group.top || y > group.bottom {
                            assert_eq!(
                                ch, ' ',
                                "ancestor/member routes must remain inside the group:\n{rendered}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn ancestor_member_bidirectional_edge_uses_two_distinct_marker_berths() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!("graph {direction}\nsubgraph g[BUFFER]\nA\nend\ng <--> A\n");
        let model = parse_model(&source);
        for options in [options()[1], options()[2]] {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            let frames = assert_scene(
                &rendered,
                &["BUFFER", "A"],
                &[("BUFFER", true), ("A", false)],
            );
            let group = frame(&frames, "BUFFER");
            assert!(group.encloses(frame(&frames, "A")), "{rendered}");
        }
    }
}

#[test]
fn labeled_group_self_loops_keep_distinct_contacts_beside_an_exit() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for loops in [1, 3] {
            let mut source = format!("graph {direction}\nsubgraph g[BUFFER]\nA\nend\n");
            let labels = ["first", "second", "third"];
            for label in &labels[..loops] {
                source.push_str(&format!("g -->|{label}| g\n"));
            }
            source.push_str("g --> Z\n");
            let model = parse_model(&source);
            for options in options() {
                let rendered = render_model(&model, &options)
                    .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
                let mut targets = vec![("BUFFER", false); loops];
                targets.push(("Z", false));
                let frames = assert_scene(&rendered, &["BUFFER", "A", "Z"], &targets);
                let group = frame(&frames, "BUFFER");
                assert_isolated_member(&rendered, group, frame(&frames, "A"), "BUFFER");
                let rows: Vec<Vec<char>> =
                    rendered.lines().map(|row| row.chars().collect()).collect();
                for label in &labels[..loops] {
                    let (x, y) = label_position(&rows, label, &rendered);
                    assert!(!group.contains(x, y), "{rendered}");
                }
            }
        }
    }
}

#[test]
fn nested_group_self_loop_stays_outside_its_frame_and_inside_its_parent() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph p[PIPELINE]\nsubgraph g[BUFFER]\nA\nend\ng -->|retry| g\nend\ng --> Z\n"
        );
        let model = parse_model(&source);
        for options in options() {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            let frames = assert_scene(
                &rendered,
                &["PIPELINE", "BUFFER", "A", "Z"],
                &[("BUFFER", false), ("Z", false)],
            );
            let parent = frame(&frames, "PIPELINE");
            let group = frame(&frames, "BUFFER");
            assert!(parent.encloses(group), "{rendered}");
            assert!(parent.disjoint(frame(&frames, "Z")), "{rendered}");
            assert_isolated_member(&rendered, group, frame(&frames, "A"), "BUFFER");
            let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
            let (x, y) = label_position(&rows, "retry", &rendered);
            assert!(parent.contains(x, y), "{rendered}");
            assert!(!group.contains(x, y), "{rendered}");
        }
    }
}

#[test]
fn group_self_loops_preserve_two_ended_markers_and_distinct_strokes() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph g[BUFFER]\nA\nend\ng <-->|retry| g\ng -.->|poll| g\ng ==>|flush| g\ng --> Z\n"
        );
        let model = parse_model(&source);
        for options in options() {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            let frames = assert_scene(
                &rendered,
                &["BUFFER", "A", "Z"],
                &[
                    ("BUFFER", false),
                    ("BUFFER", false),
                    ("BUFFER", false),
                    ("BUFFER", false),
                    ("Z", false),
                ],
            );
            assert_isolated_member(
                &rendered,
                frame(&frames, "BUFFER"),
                frame(&frames, "A"),
                "BUFFER",
            );
            for label in ["retry", "poll", "flush"] {
                assert_eq!(rendered.matches(label).count(), 1, "{rendered}");
            }
        }
    }
}

#[test]
fn deeply_nested_parallel_group_self_loops_reserve_label_space() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "graph {direction}\nsubgraph p[PIPELINE]\nsubgraph q[MIDDLE]\nsubgraph g[BUFFER]\nA\nend\ng -->|first| g\ng -->|second| g\nend\nend\ng --> Z\n"
        );
        let model = parse_model(&source);
        for options in options() {
            let rendered = render_model(&model, &options)
                .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
            let frames = assert_scene(
                &rendered,
                &["PIPELINE", "MIDDLE", "BUFFER", "A", "Z"],
                &[("BUFFER", false), ("BUFFER", false), ("Z", false)],
            );
            let parent = frame(&frames, "MIDDLE");
            let group = frame(&frames, "BUFFER");
            assert!(frame(&frames, "PIPELINE").encloses(parent), "{rendered}");
            assert!(parent.encloses(group), "{rendered}");
            assert_isolated_member(&rendered, group, frame(&frames, "A"), "BUFFER");
            let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
            for label in ["first", "second"] {
                let (x, y) = label_position(&rows, label, &rendered);
                assert!(parent.contains(x, y), "{rendered}");
                assert!(!group.contains(x, y), "{rendered}");
            }
        }
    }
}
