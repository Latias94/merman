#![cfg(any(feature = "diagram-flowchart", feature = "diagram-state"))]

mod support;

use merman_ascii::{AsciiLayoutProfile, AsciiRenderOptions, TerminalWidthProfile};
use support::{assert_rectangular_terminal_grid_with_profile, parse_model, render_model};
use unicode_width::UnicodeWidthStr;

fn display_width(text: &str, profile: TerminalWidthProfile) -> usize {
    match profile {
        TerminalWidthProfile::Cjk => UnicodeWidthStr::width_cjk(text),
        _ => UnicodeWidthStr::width(text),
    }
}

fn text_position(rendered: &str, text: &str, profile: TerminalWidthProfile) -> (usize, usize) {
    assert_eq!(
        rendered.matches(text).count(),
        1,
        "complete text {text:?} must survive exactly once:\n{rendered}"
    );
    rendered
        .lines()
        .enumerate()
        .find_map(|(y, row)| {
            row.find(text)
                .map(|offset| (display_width(&row[..offset], profile), y))
        })
        .unwrap_or_else(|| panic!("missing text {text:?}:\n{rendered}"))
}

fn render(source: &str, options: &AsciiRenderOptions) -> String {
    let rendered = render_model(&parse_model(source), options)
        .unwrap_or_else(|error| panic!("{source}\n{options:?}: {error}"));
    assert_rectangular_terminal_grid_with_profile(&rendered, options.terminal_width_profile);
    rendered
}

fn assert_title_block(
    rendered: &str,
    lines: &[&str],
    profile: TerminalWidthProfile,
    frame_delimits: bool,
) -> Vec<(usize, usize)> {
    let positions: Vec<_> = lines
        .iter()
        .map(|line| text_position(rendered, line, profile))
        .collect();
    let rows: Vec<_> = rendered.lines().collect();
    for (line, &(x, y)) in lines.iter().zip(&positions) {
        assert_eq!(
            x, positions[0].0,
            "title rows must share one visible left anchor:\n{rendered}"
        );
        let offset = rows[y].find(line).unwrap();
        let before = &rows[y][..offset];
        let after = &rows[y][offset + line.len()..];
        assert!(
            before.ends_with(' ') || (frame_delimits && before.ends_with(['|', '│'])),
            "title {line:?} needs clear reading space on its left:\n{rendered}"
        );
        assert!(
            after.starts_with(' ') || (frame_delimits && after.starts_with(['|', '│'])),
            "title {line:?} needs clear reading space on its right:\n{rendered}"
        );
    }
    assert!(
        positions.windows(2).all(|pair| pair[0].1 < pair[1].1),
        "title lines must stay upright in authored order:\n{rendered}"
    );
    positions
}

fn assert_solid_outer_frame(rendered: &str, profile: TerminalWidthProfile) {
    let rows: Vec<_> = rendered.lines().collect();
    let width = display_width(rows[0], profile);
    let (top_left, top_right, bottom_left, bottom_right, horizontal, side) =
        if rendered.starts_with('┌') {
            ('┌', '┐', '└', '┘', '─', '│')
        } else {
            ('+', '+', '+', '+', '-', '|')
        };
    let span = horizontal.to_string().repeat(width - 2);
    assert_eq!(
        rows[0],
        format!("{top_left}{span}{top_right}"),
        "{rendered}"
    );
    assert_eq!(
        rows.last().copied().unwrap(),
        format!("{bottom_left}{span}{bottom_right}"),
        "{rendered}"
    );
    assert!(
        rows[1..rows.len() - 1]
            .iter()
            .all(|row| row.starts_with(side) && row.ends_with(side)),
        "the group must retain a closed solid contour:\n{rendered}"
    );
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn nested_multiline_titles_keep_their_block_anchor_clear_of_boundary_wires() {
    for direction in ["LR", "RL", "TD", "BT"] {
        let source = format!(
            "flowchart {direction}\n\
             Start --> Entry\n\
             subgraph outer [Outer Pipeline<br>OP]\n\
             direction LR\n\
             Entry[Entry]\n\
             subgraph inner [IO<br>内部工程]\n\
             direction TD\n\
             Validate[Validate] --> Persist[Persist]\n\
             end\n\
             Entry -.-> Validate\n\
             end\n\
             Persist --> Done\n"
        );
        for options in [
            AsciiRenderOptions::ascii(),
            AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Compact),
        ] {
            let rendered = render(&source, &options);
            let profile = options.terminal_width_profile;
            let outer = assert_title_block(&rendered, &["Outer Pipeline", "OP"], profile, false);
            let inner = assert_title_block(&rendered, &["IO", "内部工程"], profile, false);
            let [start, entry, validate, persist, done] =
                ["Start", "Entry", "Validate", "Persist", "Done"]
                    .map(|label| text_position(&rendered, label, profile));
            let before = |a: (usize, usize), b: (usize, usize)| match direction {
                "LR" => a.0 < b.0,
                "RL" => b.0 < a.0,
                "TD" => a.1 < b.1,
                "BT" => b.1 < a.1,
                _ => unreachable!(),
            };
            assert!(
                before(start, entry) && before(persist, done),
                "boundary entry and exit must preserve root {direction} ordering:\n{rendered}"
            );
            assert!(
                entry.0 < validate.0 && validate.1 < persist.1,
                "the outer LR and inner TD directions must survive boundary routing:\n{rendered}"
            );
            for (title, members) in [
                (&outer, &[entry, validate, persist][..]),
                (&inner, &[validate, persist][..]),
            ] {
                assert!(
                    members.iter().all(|member| if direction == "BT" {
                        member.1 < title[0].1
                    } else {
                        title.last().unwrap().1 < member.1
                    }),
                    "the title band must stay clear of its members, including BT's bottom band:\n{rendered}"
                );
            }
            assert_eq!(
                rendered
                    .chars()
                    .filter(|ch| matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼'))
                    .count(),
                4,
                "all four authored wires need their arrowheads:\n{rendered}"
            );
            assert!(
                rendered
                    .chars()
                    .any(|ch| matches!(ch, '.' | ':' | '┄' | '┆')),
                "the dotted inner-boundary edge must keep its authored stroke:\n{rendered}"
            );
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn narrow_empty_multiline_titles_keep_their_extent_and_widest_line_anchor() {
    for (direction, options) in [
        ("LR", AsciiRenderOptions::ascii()),
        (
            "RL",
            AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Compact),
        ),
        (
            "TD",
            AsciiRenderOptions::ascii()
                .with_layout_profile(AsciiLayoutProfile::Compact)
                .with_terminal_width_profile(TerminalWidthProfile::Cjk),
        ),
        (
            "BT",
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk),
        ),
    ] {
        let rendered = render(
            &format!("flowchart {direction}\nsubgraph empty [A<br>最·長<br>Z]\nend\n"),
            &options,
        );
        let profile = options.terminal_width_profile;
        let positions = assert_title_block(&rendered, &["A", "最·長", "Z"], profile, true);
        assert_solid_outer_frame(&rendered, profile);
        assert_eq!(
            (
                display_width(rendered.lines().next().unwrap(), profile),
                rendered.lines().count()
            ),
            (display_width("最·長", profile) + 2, 8),
            "title alignment must not expand an empty group's existing extent:\n{rendered}"
        );
        assert_eq!(
            (positions[0].1, positions[2].1),
            if direction == "BT" { (2, 6) } else { (1, 5) },
            "the existing title band and upright line spacing must survive projection:\n{rendered}"
        );
    }
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_concurrency_keeps_an_aligned_title_solid_frame_and_dotted_regions() {
    for (direction, options, dotted, side) in [
        ("RL", AsciiRenderOptions::unicode(), '┄', '│'),
        ("BT", AsciiRenderOptions::ascii(), '.', '|'),
    ] {
        let source = format!(
            "stateDiagram-v2\ndirection {direction}\n\
             state \"制御·系<br>OK\" as Running {{\nA --> B\n--\nC --> D\n}}\n"
        );
        let rendered = render(&source, &options);
        let profile = options.terminal_width_profile;
        let title = assert_title_block(&rendered, &["制御·系", "OK"], profile, false);
        assert_solid_outer_frame(&rendered, profile);
        let width = display_width(rendered.lines().next().unwrap(), profile);
        assert!(
            (title[0].0 - 1).abs_diff(width - 1 - title[0].0 - display_width("制御·系", profile))
                <= 2,
            "the widest title line must retain its centered position:\n{rendered}"
        );
        let separators: Vec<_> = rendered
            .lines()
            .filter(|row| row.contains(dotted))
            .collect();
        assert!(separators.len() >= 2, "{rendered}");
        for row in separators {
            assert!(
                row.strip_prefix(side)
                    .unwrap()
                    .strip_suffix(side)
                    .unwrap()
                    .chars()
                    .all(|ch| ch == dotted),
                "concurrent regions must remain dotted horizontal separators inside solid sides:\n{rendered}"
            );
        }
        for [from, to] in [["A", "B"], ["C", "D"]] {
            let from = text_position(&rendered, from, profile);
            let to = text_position(&rendered, to, profile);
            assert!(
                if direction == "RL" {
                    to.0 < from.0
                } else {
                    to.1 < from.1
                },
                "state transitions must preserve {direction} ordering:\n{rendered}"
            );
            assert!(
                [from, to].iter().all(|member| if direction == "BT" {
                    member.1 < title[0].1
                } else {
                    title.last().unwrap().1 < member.1
                }),
                "the composite's title band must remain separate from its regions:\n{rendered}"
            );
        }
        assert_eq!(
            rendered
                .chars()
                .filter(|ch| matches!(ch, '>' | '<' | '^' | 'v' | '►' | '◄' | '▲' | '▼'))
                .count(),
            2,
            "both state transitions need their arrowheads:\n{rendered}"
        );
        assert!(
            !rendered.contains("divider-id") && !rendered.contains("id-"),
            "{rendered}"
        );
    }
}
