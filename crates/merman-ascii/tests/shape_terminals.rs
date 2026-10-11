#![cfg(feature = "diagram-flowchart")]

use merman_ascii::{AsciiLayoutProfile, AsciiRenderOptions, AsciiRenderer, AsciiResourcePolicy};
use merman_core::{Engine, OperationControl, ParseOptions};

fn render(source: &str, options: AsciiRenderOptions) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let context = merman_core::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    AsciiRenderer::new(options)
        .unwrap()
        .render_parsed(
            &parsed,
            &OperationControl::new(),
            &context,
            AsciiResourcePolicy::default(),
        )
        .unwrap_or_else(|error| panic!("{source} with {options:?}: {error}"))
}

fn assert_heads_touch_contours(rendered: &str, expected: usize) {
    let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
    let mut heads = 0;
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.iter().copied().enumerate() {
            let target = match ch {
                '>' | '►' => Some((x + 1, y)),
                '<' | '◄' => x.checked_sub(1).map(|x| (x, y)),
                '^' | '▲' => y.checked_sub(1).map(|y| (x, y)),
                'v' | '▼' => Some((x, y + 1)),
                _ => None,
            };
            let Some((target_x, target_y)) = target else {
                continue;
            };
            heads += 1;
            let contact = rows
                .get(target_y)
                .and_then(|row| row.get(target_x))
                .copied()
                .unwrap_or(' ');
            assert!(
                matches!(
                    contact,
                    '/' | '\\' | '-' | '─' | '|' | '│' | '+' | '┬' | '┴' | '├' | '┤'
                ),
                "head ({x},{y}) must point at its adjacent shape contour, got {contact:?}:\n{rendered}"
            );
        }
    }
    assert_eq!(
        heads, expected,
        "every authored head must remain visible:\n{rendered}"
    );
}

fn route_directions(ch: char) -> u8 {
    match ch {
        '-' | '─' => 2 | 8,
        '|' | '│' => 1 | 4,
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

fn node_frame(rows: &[Vec<char>], label: char) -> (isize, isize, isize, isize) {
    let cell = |x: isize, y: isize| {
        usize::try_from(y)
            .ok()
            .and_then(|y| rows.get(y))
            .and_then(|row| usize::try_from(x).ok().and_then(|x| row.get(x)))
            .copied()
            .unwrap_or(' ')
    };
    let (label_y, label_x) = rows
        .iter()
        .enumerate()
        .find_map(|(y, row)| row.iter().position(|ch| *ch == label).map(|x| (y, x)))
        .unwrap();
    let left = (0..label_x)
        .rev()
        .find(|x| matches!(rows[label_y][*x], '|' | '│' | '├' | '┤'))
        .unwrap();
    let right = (label_x + 1..rows[label_y].len())
        .find(|x| matches!(rows[label_y][*x], '|' | '│' | '├' | '┤'))
        .unwrap();
    let top = (0..label_y)
        .rev()
        .find(|y| {
            matches!(cell(left as isize, *y as isize), '+' | '┌')
                && matches!(cell(right as isize, *y as isize), '+' | '┐')
        })
        .unwrap();
    let bottom = (label_y + 1..rows.len())
        .find(|y| {
            matches!(cell(left as isize, *y as isize), '+' | '└')
                && matches!(cell(right as isize, *y as isize), '+' | '┘')
        })
        .unwrap();
    (left as isize, right as isize, top as isize, bottom as isize)
}

fn assert_terminals_reach_authored_nodes(rendered: &str) {
    let rows: Vec<Vec<char>> = rendered.lines().map(|row| row.chars().collect()).collect();
    let cell = |x: isize, y: isize| {
        usize::try_from(y)
            .ok()
            .and_then(|y| rows.get(y))
            .and_then(|row| usize::try_from(x).ok().and_then(|x| row.get(x)))
            .copied()
            .unwrap_or(' ')
    };
    let boxes = ['A', 'B'].map(|label| node_frame(&rows, label));
    let is_node_contact = |x: isize, y: isize| {
        boxes.iter().any(|(left, right, top, bottom)| {
            ((*left == x || *right == x) && (*top..=*bottom).contains(&y))
                || ((*top == y || *bottom == y) && (*left..=*right).contains(&x))
        })
    };
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.iter().copied().enumerate() {
            let (dx, dy, inward, outward) = match ch {
                '>' | '►' => (1, 0, 2, 8),
                '<' | '◄' => (-1, 0, 8, 2),
                '^' | '▲' => (0, -1, 1, 4),
                'v' | '▼' => (0, 1, 4, 1),
                _ => continue,
            };
            let (x, y) = (x as isize, y as isize);
            assert!(
                is_node_contact(x + dx, y + dy),
                "a group frame cannot replace the authored endpoint:\n{rendered}"
            );
            let (mut path_x, mut path_y) = (x - dx, y - dy);
            let mut reached_end = false;
            for _ in 0..rows.len() + row.len() {
                if is_node_contact(path_x, path_y) {
                    reached_end = true;
                    break;
                }
                let directions = route_directions(cell(path_x, path_y));
                assert_ne!(
                    directions & inward,
                    0,
                    "terminal cell must connect toward its head:\n{rendered}"
                );
                let next_x = path_x - dx;
                let next_y = path_y - dy;
                if directions & outward != 0
                    && (is_node_contact(next_x, next_y)
                        || route_directions(cell(next_x, next_y)) & inward != 0)
                {
                    path_x = next_x;
                    path_y = next_y;
                    continue;
                }
                let transverse = if dx == 0 { 2 | 8 } else { 1 | 4 };
                assert_ne!(
                    directions & transverse,
                    0,
                    "terminal must continue straight or reach a connected bend:\n{rendered}"
                );
                reached_end = true;
                break;
            }
            assert!(
                reached_end,
                "terminal must reach a node or bend within the finite canvas:\n{rendered}"
            );
        }
    }
}

#[test]
fn leaning_terminals_and_multiline_labels_share_the_actual_contour() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for unicode in [false, true] {
            for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                for padding in [None, Some((0, 0)), Some((1, 0))] {
                    for shape in ["lean-r", "lean-l"] {
                        for leaning_source in [false, true] {
                            for edge in ["-->", "---", "-.->", "==>"] {
                                let mut options = if unicode {
                                    AsciiRenderOptions::unicode()
                                } else {
                                    AsciiRenderOptions::ascii()
                                };
                                options.layout_profile = profile;
                                if let Some((x, y)) = padding {
                                    options.node_padding_x = Some(x);
                                    options.node_padding_y = Some(y);
                                }
                                let lean = format!(
                                    "A@{{ shape: {shape}, label: \"aaa<br/>bbb<br/>ccc\" }}"
                                );
                                let source = if leaning_source {
                                    format!("flowchart {direction}\n{lean} {edge} B[B]")
                                } else {
                                    format!("flowchart {direction}\nB[B] {edge} {lean}")
                                };
                                let rendered = render(&source, options);
                                for text in ["aaa", "bbb", "ccc"] {
                                    let row = rendered
                                        .lines()
                                        .find(|row| row.contains(text))
                                        .unwrap_or_else(|| {
                                            panic!("{text} must survive:\n{rendered}")
                                        });
                                    let text_x = row.find(text).unwrap();
                                    assert!(
                                        row[..text_x].contains(['/', '\\'])
                                            && row[text_x + text.len()..].contains(['/', '\\']),
                                        "each multiline label must stay between its own row's sloping sides:\n{rendered}"
                                    );
                                }
                                assert_heads_touch_contours(&rendered, usize::from(edge != "---"));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn cross_group_return_heads_enter_the_authored_member_contour() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for unicode in [false, true] {
            for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                for nesting in [1, 2] {
                    let mut options = if unicode {
                        AsciiRenderOptions::unicode()
                    } else {
                        AsciiRenderOptions::ascii()
                    };
                    options.layout_profile = profile;
                    let groups = if nesting == 1 {
                        "subgraph Outer\nA[A]\nend"
                    } else {
                        "subgraph Outer\nsubgraph Inner\nA[A]\nend\nend"
                    };
                    let source = format!("flowchart {direction}\n{groups}\nA --> B[B]\nB --> A");
                    let rendered = render(&source, options);
                    assert_heads_touch_contours(&rendered, 2);
                    assert_terminals_reach_authored_nodes(&rendered);
                }
            }
        }
    }
}

#[test]
fn zero_horizontal_padding_preserves_subroutine_inner_decoration() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for unicode in [false, true] {
            let mut options = if unicode {
                AsciiRenderOptions::unicode()
            } else {
                AsciiRenderOptions::ascii()
            };
            options.node_padding_x = Some(0);
            options.node_padding_y = Some(0);
            let rendered = render(&format!("flowchart {direction}\nA[[AB]]"), options);
            let row = rendered.lines().find(|row| row.contains("AB")).unwrap();
            assert_eq!(
                row.chars().filter(|ch| matches!(ch, '|' | '│')).count(),
                4,
                "intrinsic inner bars must survive independently of configurable whitespace:\n{rendered}"
            );
        }
    }
}

#[test]
fn fan_in_routes_keep_five_independent_heads_and_continuous_target_terminals() {
    for direction in ["LR", "RL", "TD", "BT"] {
        for unicode in [false, true] {
            for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
                for padding_y in [3, 5] {
                    let mut options = if unicode {
                        AsciiRenderOptions::unicode()
                    } else {
                        AsciiRenderOptions::ascii()
                    };
                    options.layout_profile = profile;
                    options.graph_padding_x = 5;
                    options.graph_padding_y = padding_y;
                    let source = format!(
                        "flowchart {direction}\nA --> B\nB --> C\nA --> C\nB --> D\nD --> C"
                    );
                    let rendered = render(&source, options);
                    let rows: Vec<Vec<char>> =
                        rendered.lines().map(|row| row.chars().collect()).collect();
                    let cell = |x: isize, y: isize| {
                        usize::try_from(y)
                            .ok()
                            .and_then(|y| rows.get(y))
                            .and_then(|row| usize::try_from(x).ok().and_then(|x| row.get(x)))
                            .copied()
                            .unwrap_or(' ')
                    };
                    let boxes = ['A', 'B', 'C', 'D'].map(|label| {
                        let (left, right, top, bottom) = node_frame(&rows, label);
                        (label, left, right, top, bottom)
                    });
                    let mut targets = Vec::new();
                    for (y, row) in rows.iter().enumerate() {
                        for (x, head) in row.iter().copied().enumerate() {
                            let (dx, dy, axis) = match head {
                                '>' | '►' => (1, 0, 2 | 8),
                                '<' | '◄' => (-1, 0, 2 | 8),
                                '^' | '▲' => (0, -1, 1 | 4),
                                'v' | '▼' => (0, 1, 1 | 4),
                                _ => continue,
                            };
                            let (mut terminal_x, mut terminal_y) = (x as isize, y as isize);
                            let mut reached = None;
                            for _ in 0..rows.len() + row.len() {
                                terminal_x += dx;
                                terminal_y += dy;
                                if let Some((label, _, _, _, _)) =
                                    boxes.iter().find(|(_, left, right, top, bottom)| {
                                        ((*left == terminal_x || *right == terminal_x)
                                            && (*top..=*bottom).contains(&terminal_y))
                                            || ((*top == terminal_y || *bottom == terminal_y)
                                                && (*left..=*right).contains(&terminal_x))
                                    })
                                {
                                    reached = Some(*label);
                                    break;
                                }
                                let next = cell(terminal_x, terminal_y);
                                assert!(
                                    route_directions(next) & axis == axis || next == head,
                                    "each allocated head must retain the straight shared terminal into its target:\n{rendered}"
                                );
                            }
                            targets.push(reached.unwrap_or_else(|| {
                                panic!("head must reach an authored node:\n{rendered}")
                            }));
                        }
                    }
                    targets.sort_unstable();
                    assert_eq!(
                        targets,
                        ['B', 'C', 'C', 'C', 'D'],
                        "all five authored heads need independent berths into the correct targets:\n{rendered}"
                    );
                }
            }
        }
    }
}
