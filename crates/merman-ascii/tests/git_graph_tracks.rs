#![cfg(feature = "diagram-git-graph")]

mod support;

use merman_ascii::{
    AsciiColorMode, AsciiDiagram, AsciiError, AsciiOutput, AsciiOutputOutcome, AsciiProjection,
    AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
    AsciiViewportPolicy, OverflowPolicy, TerminalWidthProfile,
};
use merman_core::SemanticParser;
use merman_model::families::git_graph::GitGraphDiagramModel;
use merman_model::runtime::RuntimePolicy;
use merman_model::{OperationControl, ParsedSemanticDiagram, SemanticDiagramModel};
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

const MERGE: &str = concat!(
    "gitGraph LR:\ncommit id:\"A\"\nbranch feature\ncommit id:\"F\"\n",
    "checkout main\ncommit id:\"B\"\nmerge feature id:\"M\" tag:\"v1\"\n",
);

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .with_runtime_policy(RuntimePolicy::deterministic())
        .parse_diagram_sync(source)
        .unwrap()
}

fn model(parsed: &ParsedSemanticDiagram) -> &GitGraphDiagramModel {
    let SemanticDiagramModel::GitGraph(model) = &parsed.model else {
        panic!("expected GitGraph")
    };
    model
}

fn render(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    width: usize,
    overflow: OverflowPolicy,
) -> merman_ascii::Result<AsciiOutput> {
    AsciiRenderer::new(options)?.render_parsed_report(
        parsed,
        AsciiViewportPolicy::with_max_width(width).overflow(overflow),
        &OperationControl::new(),
        &RuntimePolicy::deterministic().begin_operation().unwrap(),
        AsciiResourcePolicy::default(),
    )
}

fn detached(model: &GitGraphDiagramModel) -> AsciiOutput {
    let config = merman_model::MermaidConfig::empty_object();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_model_report(
            &AsciiDiagram::GitGraph(model),
            merman_ascii::AsciiSourceContext {
                diagram_type: "gitGraph",
                config: &config,
                semantic_config: &config,
                title: None,
            },
            AsciiViewportPolicy::unrestricted(),
            &OperationControl::new(),
            &RuntimePolicy::deterministic().begin_operation().unwrap(),
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn records(output: &AsciiOutput) -> String {
    output
        .text
        .split_once("GitGraph records")
        .unwrap()
        .1
        .split_whitespace()
        .collect()
}

/// Trace rendered connections backwards along the event axis, stopping at actual commit
/// markers. At x/╳, continue straight rather than joining the crossing route. This reads the
/// picture independently of its planner and catches lost edges and invented ancestry.
fn assert_picture_parents(parsed: &ParsedSemanticDiagram, output: &AsciiOutput) {
    assert_eq!(
        output.projection,
        AsciiProjection::Diagrammatic,
        "{}",
        output.text
    );
    let model = model(parsed);
    let spatial = output.text.split_once("GitGraph records").unwrap().0;
    let mut rows: Vec<Vec<char>> = spatial
        .lines()
        .skip_while(|line| !line.starts_with("Git branch tracks"))
        .skip(1)
        .take_while(|line| !line.starts_with("o normal;"))
        .map(|line| line.chars().collect())
        .collect();
    let width = rows.iter().map(Vec::len).max().unwrap();
    for row in &mut rows {
        row.resize(width, ' ');
    }
    let mut nodes = HashMap::new();
    for index in 0..model.commits.len() {
        let key: Vec<char> = format!("C{}", index + 1).chars().collect();
        let mut found = None;
        for (y, row) in rows.iter().enumerate() {
            for (x, window) in row.windows(key.len()).enumerate() {
                if window == key && !row.get(x + key.len()).is_some_and(char::is_ascii_digit) {
                    let point = if model.direction == "LR" {
                        (x - 1, y - 1)
                    } else {
                        (x - 1, y - 2)
                    };
                    assert!(
                        found.replace(point).is_none(),
                        "duplicate key in picture: C{}",
                        index + 1
                    );
                }
            }
        }
        nodes.insert(
            found.unwrap_or_else(|| panic!("missing key C{}:\n{spatial}", index + 1)),
            index,
        );
    }
    let ports = |ch: char| -> u8 {
        match ch {
            '-' | '─' | '<' | '>' => 3,
            '|' | '│' | '^' | 'v' => 12,
            '+' | '┼' | 'x' | '╳' => 15,
            '┘' => 5,
            '└' => 6,
            '┐' => 9,
            '┌' => 10,
            '┴' => 7,
            '┬' => 11,
            '┤' => 13,
            '├' => 14,
            'o' | 'X' | '*' | 'M' | 'c' => 15,
            _ => 0,
        }
    };
    let directions = [
        (1, -1isize, 0isize, 2),
        (2, 1, 0, 1),
        (4, 0, -1, 8),
        (8, 0, 1, 4),
    ];
    for (&start, &child) in &nodes {
        let mut queue = VecDeque::from([(start, 0u8)]);
        let mut seen = HashSet::new();
        let mut actual = BTreeSet::new();
        while let Some(((x, y), incoming)) = queue.pop_front() {
            if !seen.insert(((x, y), incoming)) {
                continue;
            }
            if incoming != 0
                && let Some(&parent) = nodes.get(&(x, y))
            {
                if parent != child {
                    actual.insert(parent);
                }
                continue;
            }
            let mut mask = ports(rows[y][x]);
            let arrow = match rows[y][x] {
                '<' => 1,
                '>' => 2,
                '^' => 4,
                'v' => 8,
                _ => 0,
            };
            if arrow != 0 && incoming != arrow {
                continue;
            }
            if matches!(rows[y][x], 'x' | '╳') && incoming != 0 {
                mask &= if incoming & 3 != 0 { 3 } else { 12 };
            }
            for (port, dx, dy, opposite) in directions {
                if mask & port == 0 {
                    continue;
                }
                // Every admitted path is monotone in time. Horizontal branch movement in
                // TB/BT and vertical branch movement in LR can go either way.
                if (model.direction == "LR" && dx > 0)
                    || (model.direction == "TB" && dy > 0)
                    || (model.direction == "BT" && dy < 0)
                {
                    continue;
                }
                let (Some(nx), Some(ny)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                if ny < rows.len() && nx < width && ports(rows[ny][nx]) & opposite != 0 {
                    queue.push_back(((nx, ny), opposite));
                }
            }
        }
        let commit = &model.commits[child];
        let expected: BTreeSet<_> = commit
            .parents
            .iter()
            .take(if commit.commit_type == 4 {
                1
            } else {
                usize::MAX
            })
            .map(|id| {
                model
                    .commits
                    .iter()
                    .position(|commit| &commit.id == id)
                    .unwrap()
            })
            .collect();
        assert_eq!(
            actual,
            expected,
            "wrong picture parents for C{}:\n{spatial}",
            child + 1
        );
    }
}

#[test]
fn fixed_branches_and_merge_parents_are_traceable_in_every_direction_and_charset() {
    for direction in ["LR", "TB", "BT"] {
        let parsed = parse(&MERGE.replace("LR:", &format!("{direction}:")));
        for width in [80, 100, 120] {
            for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
                for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                    let options = options.with_terminal_width_profile(profile);
                    let output = render(&parsed, options, width, OverflowPolicy::Error).unwrap();
                    assert_picture_parents(&parsed, &output);
                    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
                    assert!(!output.fallback.attempted);
                    assert_eq!(
                        support::terminal_extent_with_profile(&output.text, profile),
                        (output.emitted_extent.width, output.emitted_extent.height)
                    );
                    let text = records(&output);
                    for fact in [
                        "B1[branch]name=\"main\"",
                        "B2[branch]name=\"feature\"",
                        "C4[commit]id=\"M\"branch=\"main\"kind=mergemessage=",
                        "tags=[\"v1\"]parents=[\"B\",\"F\"]",
                    ] {
                        assert!(text.contains(fact), "missing {fact}:\n{}", output.text);
                    }
                }
            }
        }
    }
}

#[test]
fn branch_order_empty_lanes_delayed_forks_and_crossings_keep_actual_ancestry() {
    let source = concat!(
        "---\nconfig:\n  gitGraph:\n    mainBranchOrder: 2\n---\n",
        "gitGraph LR:\ncommit id:\"R\"\nbranch z order:4\nbranch unused order:3\n",
        "checkout main\ncommit id:\"A\"\nbranch x order:1\ncommit id:\"X\"\n",
        "checkout z\ncommit id:\"Z\"\ncheckout main\nmerge z id:\"M\"\n",
        "merge x id:\"N\"\n",
    );
    for direction in ["LR", "TB", "BT"] {
        let parsed = parse(&source.replace("LR:", &format!("{direction}:")));
        assert_eq!(
            model(&parsed)
                .branches
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            ["x", "main", "unused", "z"]
        );
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let output = render(&parsed, options, 100, OverflowPolicy::Error).unwrap();
            assert_picture_parents(&parsed, &output);
            assert!(records(&output).contains("B3[branch]name=\"unused\""));
        }
    }
}

#[test]
fn merge_custom_appearance_does_not_change_parents_or_reverse_time() {
    for (kind, marker) in [("NORMAL", 'o'), ("REVERSE", 'X'), ("HIGHLIGHT", '*')] {
        let parsed = parse(&MERGE.replace("tag:\"v1\"", &format!("tag:\"v1\" type:{kind}")));
        let output = render(
            &parsed,
            AsciiRenderOptions::ascii(),
            80,
            OverflowPolicy::Error,
        )
        .unwrap();
        assert_picture_parents(&parsed, &output);
        let rows: Vec<_> = output.text.lines().collect();
        let label_row = rows
            .iter()
            .position(|line| line.contains("C4") && !line.contains("[commit]"))
            .unwrap();
        let column = rows[label_row].find("C4").unwrap();
        assert_eq!(rows[label_row - 1].chars().nth(column - 1), Some(marker));
        assert!(records(&output).contains("kind=merge"));
    }
}

#[test]
fn cherry_pick_source_is_not_an_ancestry_line_even_when_both_ids_match() {
    for source in [
        "gitGraph\ncommit id:\"R\"\nbranch feature\ncherry-pick id:\"R\"\n",
        "gitGraph\ncommit id:\"R\"\nbranch feature\ncommit id:\"F\"\ncheckout main\ncherry-pick id:\"F\" tag:\"chosen\"\n",
    ] {
        let parsed = parse(source);
        let output = render(
            &parsed,
            AsciiRenderOptions::unicode(),
            80,
            OverflowPolicy::Error,
        )
        .unwrap();
        assert_picture_parents(&parsed, &output);
        assert!(records(&output).contains("cherry-pickedfrom="));
    }
}

#[test]
fn non_spatial_config_and_legal_branch_alias_merges_are_complete_record_primaries() {
    for source in [
        &format!("---\nconfig:\n  gitGraph:\n    parallelCommits: true\n---\n{MERGE}"),
        "gitGraph\ncommit id:\"R\"\nbranch hold\ncheckout main\ncommit id:\"A\"\nmerge hold id:\"M\"\n",
        "gitGraph\ncommit id:\"same\"\ncommit id:\"same\"\n",
        "gitGraph\nbranch unused\n",
    ] {
        let parsed = parse(source);
        let output = render(
            &parsed,
            AsciiRenderOptions::ascii(),
            80,
            OverflowPolicy::Error,
        )
        .unwrap();
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "{}",
            output.text
        );
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert!(!output.fallback.attempted);
        assert_eq!(
            records(&output).matches("[commit]").count(),
            model(&parsed).commits.len()
        );
    }
}

#[test]
fn detached_ambiguities_preserve_occurrences_and_all_parent_references() {
    let parsed = parse(MERGE);
    let base = model(&parsed);
    for mutation in 0..7 {
        let mut model = base.clone();
        match mutation {
            0 => model.commits[1].id = model.commits[0].id.clone(),
            1 => model.commits[1].parents.push("outside".to_string()),
            2 => model.commits[0].parents.push("M".to_string()),
            3 => model.commits[1].seq = model.commits[0].seq,
            4 => model.branches.push(model.branches[0].clone()),
            5 => model.commits[1].branch = "unknown".to_string(),
            _ => model.commits[3].custom_type = Some(77),
        }
        let output = detached(&model);
        assert_eq!(
            output.projection,
            AsciiProjection::StructuredText,
            "mutation {mutation}:\n{}",
            output.text
        );
        assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
        assert_eq!(records(&output).matches("[commit]").count(), 4);
    }
    let mut gaps = base.clone();
    for (index, commit) in gaps.commits.iter_mut().enumerate() {
        commit.seq = 10 + index as i64 * 7;
    }
    assert_eq!(detached(&gaps).projection, AsciiProjection::Diagrammatic);
}

#[test]
fn vertical_branch_headers_fit_when_branch_keys_have_more_digits_than_commit_keys() {
    let mut data = model(&parse("gitGraph\ncommit id:\"root\"\n")).clone();
    for branches in [999, 1000] {
        data.branches = (0..branches)
            .map(|index| merman_model::families::git_graph::GitGraphBranch {
                name: if index == 0 {
                    "main".into()
                } else {
                    format!("unused{index}")
                },
            })
            .collect();
        for direction in ["TB", "BT"] {
            data.direction = direction.to_string();
            let output = detached(&data);
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            let header = output.text.lines().nth(1).unwrap();
            let keys: Vec<_> = header.split_whitespace().collect();
            assert_eq!(keys.len(), branches);
            assert_eq!(keys.last().unwrap(), &format!("B{branches}"));
            assert_eq!(records(&output).matches("[branch]").count(), branches);
        }
    }
}

#[test]
fn width_policies_measure_tracks_and_never_reflow_spatial_connections() {
    let parsed = parse(MERGE);
    let allowed = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        20,
        OverflowPolicy::Allow,
    )
    .unwrap();
    assert_eq!(allowed.projection, AsciiProjection::Diagrammatic);
    assert!(allowed.emitted_extent.width > 20);
    assert!(matches!(
        render(
            &parsed,
            AsciiRenderOptions::ascii(),
            20,
            OverflowPolicy::Error
        ),
        Err(AsciiError::WidthOverflow { .. })
    ));
    let fallback = render(
        &parsed,
        AsciiRenderOptions::ascii(),
        20,
        OverflowPolicy::Fallback,
    )
    .unwrap();
    assert_eq!(fallback.projection, AsciiProjection::StructuredText);
    assert_eq!(fallback.outcome, AsciiOutputOutcome::Fallback);
    assert!(fallback.emitted_extent.width <= 20);
    assert_eq!(records(&fallback).matches("[commit]").count(), 4);
    assert!(records(&fallback).contains("parents=[\"B\",\"F\"]"));
}

#[test]
fn unsafe_cjk_labels_and_all_encodings_keep_logical_extent_and_safe_data() {
    let mut data = model(&parse(MERGE)).clone();
    data.branches[0].name = "主線".to_string();
    for commit in &mut data.commits {
        if commit.branch == "main" {
            commit.branch = "主線".to_string();
        }
    }
    data.commits[0].message = "中文 Cafe\u{301} 👩‍💻 <v1>& \u{1b}[31m".to_string();
    let config = merman_model::MermaidConfig::empty_object();
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let mut extent = None;
        for color in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let output = AsciiRenderer::new(
                AsciiRenderOptions::unicode()
                    .with_terminal_width_profile(profile)
                    .with_color_mode(color),
            )
            .unwrap()
            .render_model_report(
                &AsciiDiagram::GitGraph(&data),
                merman_ascii::AsciiSourceContext {
                    diagram_type: "gitGraph",
                    config: &config,
                    semantic_config: &config,
                    title: None,
                },
                AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Error),
                &OperationControl::new(),
                &RuntimePolicy::deterministic().begin_operation().unwrap(),
                AsciiResourcePolicy::default(),
            )
            .unwrap();
            assert_eq!(output.projection, AsciiProjection::Diagrammatic);
            assert!(!output.text.contains('\u{1b}'));
            assert!(output.text.contains("\\u{1B}"));
            assert!(output.text.contains("中文 Cafe\u{301} 👩‍💻"));
            if color == AsciiColorMode::Html {
                assert!(output.text.contains("&lt;v1&gt;&amp;"));
            }
            if let Some(expected) = extent {
                assert_eq!(output.emitted_extent, expected);
            }
            extent = Some(output.emitted_extent);
        }
    }
}

#[test]
fn cancellation_deadlines_and_work_or_grid_limits_remain_errors() {
    let parsed = parse(MERGE);
    let renderer = AsciiRenderer::new(AsciiRenderOptions::ascii()).unwrap();
    let runtime = RuntimePolicy::deterministic().begin_operation().unwrap();
    let cancelled = OperationControl::new();
    cancelled.cancel();
    for (control, reason) in [
        (cancelled, merman_model::CancelReason::Requested),
        (
            OperationControl::new().with_deadline(std::time::Duration::ZERO),
            merman_model::CancelReason::DeadlineExceeded,
        ),
    ] {
        assert!(
            matches!(renderer.render_parsed_report(&parsed, AsciiViewportPolicy::unrestricted(), &control, &runtime, AsciiResourcePolicy::default()), Err(AsciiError::Cancelled(error)) if error.reason == reason)
        );
    }
    for id in [
        AsciiResourceLimitId::MaxLayoutWorkUnits,
        AsciiResourceLimitId::MaxGridCells,
    ] {
        let resources = AsciiResourcePolicy::unbounded().with_limit(id, 1).unwrap();
        assert!(
            matches!(renderer.render_parsed_report(&parsed, AsciiViewportPolicy::unrestricted(), &OperationControl::new(), &runtime, resources), Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == id)
        );
    }
}

fn generated_history(mut seed: u32) -> String {
    let mut source = String::from("gitGraph LR:\ncommit id:\"R\"\n");
    let names = ["main", "alpha", "beta", "gamma"];
    for name in &names[1..] {
        source.push_str(&format!(
            "checkout main\nbranch {name}\ncommit id:\"{name}0\"\n"
        ));
    }
    for index in 0..8 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let target = (seed >> 16) as usize % names.len();
        let other = (target + 1 + (seed >> 24) as usize % 3) % names.len();
        source.push_str(&format!("checkout {}\n", names[target]));
        if seed & 4 != 0 {
            source.push_str(&format!("merge {} id:\"event{index}\"\n", names[other]));
        } else {
            source.push_str(&format!("commit id:\"event{index}\"\n"));
        }
    }
    source
}

#[test]
fn generated_histories_check_rendered_topology_and_complete_width_fallback() {
    let mut spatial = 0;
    for seed in 0..16 {
        for direction in ["LR", "TB", "BT"] {
            let source = generated_history(seed).replace("LR:", &format!("{direction}:"));
            let parsed = parse(&source);
            let output = render(
                &parsed,
                AsciiRenderOptions::unicode(),
                100,
                OverflowPolicy::Allow,
            )
            .unwrap();
            if output.projection == AsciiProjection::Diagrammatic {
                spatial += 1;
                assert_picture_parents(&parsed, &output);
            }
            for width in [80, 100, 120] {
                let output = render(
                    &parsed,
                    AsciiRenderOptions::ascii(),
                    width,
                    OverflowPolicy::Fallback,
                )
                .unwrap();
                assert!(output.emitted_extent.width <= width);
                let records = records(&output);
                assert_eq!(records.matches("[commit]").count(), 12);
                for (index, commit) in model(&parsed).commits.iter().enumerate() {
                    assert!(records.contains(&format!(
                        "C{}[commit]id=\"{}\"",
                        index + 1,
                        commit.id
                    )));
                }
            }
        }
    }
    assert!(
        spatial > 0,
        "generated histories must exercise the spatial planner"
    );
    eprintln!("GitGraph generated spatial admission: {spatial}/48; complete outputs: 192/192");
}

#[test]
fn canonical_and_presentation_models_share_the_complete_projection() {
    use merman_core::ParseOptions;
    use merman_core::presentation::MermaidEngine;
    for source in [
        MERGE,
        "---\ntitle: Release\nconfig:\n  gitGraph:\n    parallelCommits: true\n---\ngitGraph\ncommit id:\"root\"\n",
    ] {
        let parsed = parse(source);
        let canonical = render(
            &parsed,
            AsciiRenderOptions::ascii(),
            80,
            OverflowPolicy::Error,
        )
        .unwrap();
        let full = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let (metadata, model) = full.into_parts();
        let output = AsciiRenderer::new(AsciiRenderOptions::ascii())
            .unwrap()
            .render_model_report(
                &support::full_model::view(&model),
                support::full_model::source(&metadata),
                AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Error),
                &OperationControl::new(),
                &RuntimePolicy::deterministic().begin_operation().unwrap(),
                AsciiResourcePolicy::default(),
            )
            .unwrap();
        assert_eq!(canonical.text, output.text);
        assert_eq!(canonical.projection, output.projection);
        assert_eq!(canonical.emitted_extent, output.emitted_extent);
    }
}
