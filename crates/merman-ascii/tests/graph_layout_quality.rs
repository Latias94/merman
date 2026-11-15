#![cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-state",
    feature = "diagram-swimlane"
))]

mod support;

use merman_ascii::AsciiRenderOptions;
#[cfg(feature = "diagram-flowchart")]
use support::assert_rectangular_terminal_grid;
use support::{parse_model, render_model};
use unicode_width::UnicodeWidthStr;

fn render(source: &str, options: AsciiRenderOptions) -> String {
    render_model(&parse_model(source), &options)
        .unwrap_or_else(|error| panic!("characterization request should render: {error}\n{source}"))
}

fn label_position(rendered: &str, label: &str) -> (usize, usize) {
    rendered
        .lines()
        .enumerate()
        .find_map(|(y, line)| {
            line.split_once(label)
                .map(|(prefix, _)| (UnicodeWidthStr::width(prefix), y))
        })
        .unwrap_or_else(|| panic!("missing complete label {label:?}:\n{rendered}"))
}

fn assert_progresses(rendered: &str, direction: &str, labels: &[&str]) {
    let positions: Vec<_> = labels
        .iter()
        .map(|label| label_position(rendered, label))
        .collect();
    assert!(
        positions.windows(2).all(|pair| match direction {
            "LR" => pair[0].0 < pair[1].0,
            "RL" => pair[0].0 > pair[1].0,
            "TD" => pair[0].1 < pair[1].1,
            "BT" => pair[0].1 > pair[1].1,
            _ => panic!("unexpected test direction {direction}"),
        }),
        "endpoints and conditions {labels:?} must follow {direction}:\n{rendered}"
    );
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

impl Frame {
    #[cfg(any(feature = "diagram-state", feature = "diagram-swimlane"))]
    fn encloses(self, child: Self) -> bool {
        self.left < child.left
            && child.right < self.right
            && self.top < child.top
            && child.bottom < self.bottom
    }

    fn is_disjoint(self, other: Self) -> bool {
        self.right < other.left
            || other.right < self.left
            || self.bottom < other.top
            || other.bottom < self.top
    }
}

// Frame fixtures use ASCII labels, so character columns equal terminal cells even in Unicode art.
fn frame_for_label(rendered: &str, label: &str) -> Frame {
    let rows: Vec<Vec<char>> = rendered
        .lines()
        .map(|line| line.chars().collect())
        .collect();
    let (x, y) = label_position(rendered, label);
    let left = rows[y][..x]
        .iter()
        .rposition(|ch| matches!(ch, '|' | '│' | '├' | '┤' | '┼' | '┝' | '┥' | '┿' | '╂'))
        .unwrap_or_else(|| panic!("missing left frame for {label:?}:\n{rendered}"));
    let right = rows[y][x + label.len()..]
        .iter()
        .position(|ch| matches!(ch, '|' | '│' | '├' | '┤' | '┼' | '┝' | '┥' | '┿' | '╂'))
        .map(|offset| x + label.len() + offset)
        .unwrap_or_else(|| panic!("missing right frame for {label:?}:\n{rendered}"));
    let top = (0..y)
        .rev()
        .find(|&row| {
            matches!(rows[row][left], '+' | '┌' | '/' | '╭')
                && matches!(rows[row][right], '+' | '┐' | '\\' | '╮')
        })
        .unwrap_or_else(|| panic!("missing top frame for {label:?}:\n{rendered}"));
    let bottom = (y + 1..rows.len())
        .find(|&row| {
            matches!(rows[row][left], '+' | '└' | '\\' | '╰')
                && matches!(rows[row][right], '+' | '┘' | '/' | '╯')
        })
        .unwrap_or_else(|| panic!("missing bottom frame for {label:?}:\n{rendered}"));
    Frame {
        left,
        right,
        top,
        bottom,
    }
}

fn has_left_entry(rendered: &str, frame: Frame) -> bool {
    let Some(x) = frame.left.checked_sub(1) else {
        return false;
    };
    rendered
        .lines()
        .take(frame.bottom)
        .skip(frame.top + 1)
        .any(|line| matches!(line.chars().nth(x), Some('>' | '►')))
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_one_cell_text_source_keeps_its_branch_contact() {
    let source =
        "flowchart TD\nA@{ shape: text, label: \"Q\" }\nX[Start]\nB[Finish]\nA --> B\nX --> B\n";
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render(source, options);
        assert_progresses(&rendered, "TD", &["Q", "Finish"]);
        assert_progresses(&rendered, "TD", &["Start", "Finish"]);
        let (x, y) = label_position(&rendered, "Q");
        let rows: Vec<Vec<char>> = rendered
            .lines()
            .map(|line| line.chars().collect())
            .collect();
        let neighbors = [
            x.checked_sub(1).map(|x| (x, y)),
            Some((x + 1, y)),
            y.checked_sub(1).map(|y| (x, y)),
            Some((x, y + 1)),
        ];
        assert!(
            neighbors.into_iter().flatten().any(|(x, y)| {
                matches!(
                    rows.get(y).and_then(|row| row.get(x)),
                    Some('-' | '─' | '|' | '│' | '+' | '└' | '┌' | '┐' | '┘')
                )
            }),
            "the text source must remain connected: {rendered}"
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_merge_keeps_private_conditions_and_one_shared_arrow_in_all_directions() {
    for (direction, ascii_arrow, unicode_arrow) in [
        ("LR", '>', '►'),
        ("RL", '<', '◄'),
        ("TD", 'v', '▼'),
        ("BT", '^', '▲'),
    ] {
        let declarations = format!("flowchart {direction}\nA[Alpha]\nB[Beta]\nS[Sink]\n");
        for (options, arrow) in [
            (AsciiRenderOptions::ascii(), ascii_arrow),
            (AsciiRenderOptions::unicode(), unicode_arrow),
        ] {
            let mut expected = None;
            for edges in [
                "A -->|one| S\nB -->|two| S\n",
                "B -->|two| S\nA -->|one| S\n",
            ] {
                let rendered = render(&format!("{declarations}{edges}"), options);
                assert_progresses(&rendered, direction, &["Alpha", "one", "Sink"]);
                assert_progresses(&rendered, direction, &["Beta", "two", "Sink"]);
                assert_eq!(rendered.matches("one").count(), 1, "{rendered}");
                assert_eq!(rendered.matches("two").count(), 1, "{rendered}");
                assert_eq!(
                    rendered.chars().filter(|ch| *ch == arrow).count(),
                    1,
                    "{rendered}"
                );
                if let Some(expected) = &expected {
                    assert_eq!(&rendered, expected);
                } else {
                    expected = Some(rendered);
                }
            }
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_small_holdout_keeps_endpoints_and_conditions_in_authored_direction() {
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = format!(
            "flowchart {direction}\nD[Draft]\nR[Review]\nP[Publish]\nD -->|submit| R\nR -->|accept| P\n"
        );
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let rendered = render(&source, options);
            assert_progresses(
                &rendered,
                direction,
                &["Draft", "submit", "Review", "accept", "Publish"],
            );
            let frames =
                ["Draft", "Review", "Publish"].map(|label| frame_for_label(&rendered, label));
            assert!(
                frames.windows(2).all(|pair| pair[0].is_disjoint(pair[1])),
                "the small holdout must retain separate visible node frames:\n{rendered}"
            );
            if direction == "LR" {
                assert!(has_left_entry(&rendered, frames[1]), "{rendered}");
                assert!(has_left_entry(&rendered, frames[2]), "{rendered}");
                assert!(!has_left_entry(&rendered, frames[0]), "{rendered}");
            }
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_declared_chain_is_deterministic_under_semantic_edge_permutations() {
    let edges = [
        "A -->|read| B\n",
        "B -.->|checked| C\n",
        "C ==>|release| D\n",
    ];
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let mut expected = None;
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut source =
                String::from("flowchart LR\nA[Collect]\nB[Verify]\nC[Package]\nD[Ship]\n");
            for index in order {
                source.push_str(edges[index]);
            }
            let rendered = render(&source, options);
            assert_progresses(
                &rendered,
                "LR",
                &[
                    "Collect", "read", "Verify", "checked", "Package", "release", "Ship",
                ],
            );
            for (label, strokes) in [("checked", ['.', '┄']), ("release", ['=', '━'])] {
                let row = rendered.lines().find(|line| line.contains(label)).unwrap();
                let (before, after) = row.split_once(label).unwrap();
                assert!(
                    before
                        .chars()
                        .next_back()
                        .is_some_and(|ch| strokes.contains(&ch))
                        && after.chars().next().is_some_and(|ch| strokes.contains(&ch)),
                    "{label:?} must retain its authored edge type:\n{rendered}"
                );
            }
            if let Some(expected) = &expected {
                // Compare equivalent requests without fixing a stored layout snapshot.
                assert_eq!(
                    &rendered, expected,
                    "edge permutation {order:?} must not change a declared chain"
                );
            } else {
                expected = Some(rendered);
            }
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_keeps_cjk_conditions_and_complete_distinguishing_label_suffixes() {
    let source = concat!(
        "flowchart LR\n",
        "A[发布请求] -->|审批通过| B[artifact-release-PRODUCTION]\n",
        "B -->|验证完成| C[artifact-release-STAGING]\n",
    );
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render(source, options);
        assert_progresses(
            &rendered,
            "LR",
            &[
                "发布请求",
                "审批通过",
                "artifact-release-PRODUCTION",
                "验证完成",
                "artifact-release-STAGING",
            ],
        );
        assert_rectangular_terminal_grid(&rendered);
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_nested_composites_keep_members_with_their_actual_owner() {
    let source = concat!(
        "stateDiagram-v2\ndirection LR\n",
        "state Session {\n",
        "  state Worker {\nQueued --> Running: dispatch\n}\n",
        "  state Monitor {\nWatching --> Alerted: alarm\n}\n",
        "}\nRunning --> Offline: close\n",
    );
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render(source, options);
        let session = frame_for_label(&rendered, "Session");
        let worker = frame_for_label(&rendered, "Worker");
        let monitor = frame_for_label(&rendered, "Monitor");
        assert!(
            session.encloses(worker) && session.encloses(monitor),
            "{rendered}"
        );
        assert!(worker.is_disjoint(monitor), "{rendered}");
        for label in ["Queued", "Running"] {
            assert!(
                worker.encloses(frame_for_label(&rendered, label)),
                "{rendered}"
            );
        }
        for label in ["Watching", "Alerted"] {
            assert!(
                monitor.encloses(frame_for_label(&rendered, label)),
                "{rendered}"
            );
        }
        let offline = frame_for_label(&rendered, "Offline");
        assert!(session.is_disjoint(offline), "{rendered}");
        assert!(session.right < offline.left, "{rendered}");
        assert!(has_left_entry(&rendered, offline), "{rendered}");
        assert_progresses(&rendered, "LR", &["Queued", "dispatch", "Running"]);
        assert_progresses(&rendered, "LR", &["Watching", "alarm", "Alerted"]);
        assert_progresses(&rendered, "LR", &["Running", "close", "Offline"]);
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_concurrent_regions_keep_independent_members_and_transition_conditions() {
    let source = concat!(
        "stateDiagram-v2\ndirection LR\nstate Online {\n",
        "Idle --> Busy: work\n--\nQuiet --> Ringing: ping\n}\n",
    );
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render(source, options);
        let online = frame_for_label(&rendered, "Online");
        let upper = ["Idle", "Busy"].map(|label| frame_for_label(&rendered, label));
        let lower = ["Quiet", "Ringing"].map(|label| frame_for_label(&rendered, label));
        assert!(
            upper
                .into_iter()
                .chain(lower)
                .all(|frame| online.encloses(frame)),
            "{rendered}"
        );
        assert_progresses(&rendered, "LR", &["Idle", "work", "Busy"]);
        assert_progresses(&rendered, "LR", &["Quiet", "ping", "Ringing"]);
        let upper_bottom = upper.iter().map(|frame| frame.bottom).max().unwrap();
        let lower_top = lower.iter().map(|frame| frame.top).min().unwrap();
        assert!(upper_bottom < lower_top, "{rendered}");
        let rows: Vec<Vec<char>> = rendered
            .lines()
            .map(|line| line.chars().collect())
            .collect();
        let gap = &rows[upper_bottom + 1..lower_top];
        assert!(
            gap.iter().any(|row| {
                row[online.left + 1..online.right]
                    .iter()
                    .any(|ch| matches!(ch, '.' | '┄'))
            }),
            "concurrent lifecycles need a visible separating divider:\n{rendered}"
        );
        assert!(
            gap.iter().all(|row| {
                row[online.left + 1..online.right]
                    .iter()
                    .all(|ch| matches!(ch, ' ' | '.' | '┄'))
            }),
            "independent regions must not acquire a cross-region transition:\n{rendered}"
        );
        assert!(
            (online.top + 1..=upper_bottom).contains(&label_position(&rendered, "work").1)
                && (lower_top..online.bottom).contains(&label_position(&rendered, "ping").1),
            "each transition condition must stay with its own concurrent region:\n{rendered}"
        );
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_handoff_preserves_lane_membership_and_member_endpoints() {
    let source = concat!(
        "swimlane-beta LR\n",
        "subgraph Client[Client team]\nA[Request]\nend\n",
        "subgraph Operator[Operator team]\nB[Inspect]\nC[Resolve]\n",
        "B -->|diagnose| C\nend\nA -->|handoff| B\n",
    );
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render(source, options);
        let client = frame_for_label(&rendered, "Client team");
        let operator = frame_for_label(&rendered, "Operator team");
        let request = frame_for_label(&rendered, "Request");
        let inspect = frame_for_label(&rendered, "Inspect");
        let resolve = frame_for_label(&rendered, "Resolve");
        assert!(client.encloses(request), "{rendered}");
        assert!(
            operator.encloses(inspect) && operator.encloses(resolve),
            "{rendered}"
        );
        assert!(client.is_disjoint(operator), "{rendered}");
        assert!(has_left_entry(&rendered, inspect), "{rendered}");
        assert!(has_left_entry(&rendered, resolve), "{rendered}");
        assert!(!has_left_entry(&rendered, operator), "{rendered}");
        assert_progresses(
            &rendered,
            "LR",
            &["Request", "handoff", "Inspect", "diagnose", "Resolve"],
        );
    }
}

#[cfg(feature = "diagram-swimlane")]
#[test]
fn swimlane_group_target_does_not_turn_into_a_member_target() {
    for (target, targets_member) in [("Lane", false), ("C", true)] {
        let source = format!(
            "swimlane-beta LR\nsubgraph Lane[Review team]\nC[Check]\nend\nS[Submit]\nS -->|ready| {target}\n"
        );
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let rendered = render(&source, options);
            let lane = frame_for_label(&rendered, "Review team");
            let member = frame_for_label(&rendered, "Check");
            let submit = frame_for_label(&rendered, "Submit");
            assert!(lane.encloses(member), "{rendered}");
            assert!(lane.is_disjoint(submit), "{rendered}");
            assert_eq!(
                has_left_entry(&rendered, member),
                targets_member,
                "{rendered}"
            );
            assert_eq!(
                has_left_entry(&rendered, lane),
                !targets_member,
                "{rendered}"
            );
            label_position(&rendered, "ready");
        }
    }
}
