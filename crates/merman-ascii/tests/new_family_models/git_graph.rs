use super::*;
use merman_ascii::{AsciiOutputOutcome, AsciiProjection, AsciiViewportPolicy, OverflowPolicy};

fn history_commit(id: &str, branch: &str, seq: i64, parents: &[&str]) -> GitGraphCommit {
    GitGraphCommit {
        id: id.to_string(),
        message: String::new(),
        seq,
        commit_type: 0,
        tags: Vec::new(),
        parents: parents.iter().map(|parent| (*parent).to_string()).collect(),
        branch: branch.to_string(),
        custom_type: None,
        custom_id: None,
    }
}

fn history_model(commits: Vec<GitGraphCommit>) -> GitGraphDiagramModel {
    GitGraphDiagramModel {
        diagram_type: "gitGraph".to_string(),
        commits,
        branches: vec![GitGraphBranch {
            name: "main".to_string(),
        }],
        current_branch: "main".to_string(),
        direction: "LR".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    }
}

fn unwrapped_records(rendered: &str) -> Vec<String> {
    let mut records: Vec<String> = Vec::new();
    for line in rendered.lines() {
        if let Some(continuation) = line.strip_prefix("  ")
            && let Some(record) = records.last_mut()
        {
            record.push_str(continuation);
        } else {
            records.push(line.to_string());
        }
    }
    records
}

fn record<'a>(records: &'a [String], prefix: &str) -> &'a str {
    records
        .iter()
        .find(|record| record.starts_with(prefix))
        .map(String::as_str)
        .unwrap_or_else(|| panic!("missing {prefix}:\n{}", records.join("\n")))
}

fn render_primary(model: GitGraphDiagramModel, projection: AsciiProjection) -> String {
    let report = support::render_model_report(
        &RenderSemanticModel::GitGraph(model),
        &AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::unrestricted(),
    )
    .expect("GitGraph primary should retain complete records");
    assert_eq!(
        report.outcome,
        AsciiOutputOutcome::Primary,
        "{}",
        report.text
    );
    assert_eq!(report.projection, projection, "{}", report.text);
    assert!(!report.fallback.attempted, "{}", report.text);
    assert_eq!(
        report
            .text
            .lines()
            .filter(|line| *line == "GitGraph records")
            .count(),
        1,
        "{}",
        report.text
    );
    report.text
}

#[test]
fn git_graph_linear_history_preserves_model_order_and_complete_records() {
    let mut root = history_commit("root", "main", 0, &[]);
    root.message = "initialize repository".to_string();
    let mut tip = history_commit("tip", "main", 1, &["root"]);
    tip.message = "ship".to_string();
    tip.tags = vec!["v1".to_string(), "stable".to_string()];

    let rendered = render_primary(
        history_model(vec![root, tip]),
        AsciiProjection::Diagrammatic,
    );
    let records = unwrapped_records(&rendered);
    let commits = records
        .iter()
        .filter(|record| record.contains(" [commit] "))
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        commits,
        [
            "C1 [commit] id=\"root\" branch=\"main\" kind=normal message=\"initialize repository\" tags=[] parents=[]",
            "C2 [commit] id=\"tip\" branch=\"main\" kind=normal message=\"ship\" tags=[\"v1\", \"stable\"] parents=[\"root\"]",
        ],
        "{rendered}"
    );
    assert_eq!(
        record(&records, "B1 [branch] "),
        "B1 [branch] name=\"main\""
    );
    for (key, sequence) in [("C1 details:", "seq:0"), ("C2 details:", "seq:1")] {
        let details = record(&records, key);
        assert!(details.contains(sequence), "{rendered}");
        assert!(details.contains("type:0"), "{rendered}");
    }
    let details = record(&records, "gitGraph details:");
    for value in [
        "type:\"gitGraph\"",
        "currentBranch:\"main\"",
        "direction:\"LR\"",
        "title:null",
        "accTitle:null",
        "accDescr:null",
        "sourceConfig:{}",
        "sourceTitle:null",
    ] {
        assert!(details.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn git_graph_records_retain_source_title_and_explicit_config_false() {
    let rendered = render_parsed(concat!(
        "---\n",
        "title: Source history\n",
        "config:\n",
        "  gitGraph:\n",
        "    showBranches: false\n",
        "---\n",
        "gitGraph\n",
        "commit id:\"root\"\n",
    ));
    let records = unwrapped_records(&rendered);
    let details = record(&records, "gitGraph details:");
    for value in [
        "sourceTitle:\"Source history\"",
        "sourceConfig:",
        "\"showBranches\":false",
    ] {
        assert!(details.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn git_graph_parsed_branch_merge_retains_both_parents_in_model_order() {
    let rendered = render_parsed(concat!(
        "gitGraph LR:\n",
        "commit id:\"A\" msg:\"initial\"\n",
        "branch feature\n",
        "commit id:\"F\" msg:\"work\"\n",
        "checkout main\n",
        "commit id:\"B\" msg:\"base\"\n",
        "merge feature id:\"M\" tag:\"v1\"\n",
    ));

    let records = unwrapped_records(&rendered);
    for expected in [
        "B1 [branch] name=\"main\"",
        "B2 [branch] name=\"feature\"",
        "C1 [commit] id=\"A\" branch=\"main\" kind=normal message=\"initial\" tags=[] parents=[]",
        "C2 [commit] id=\"F\" branch=\"feature\" kind=normal message=\"work\" tags=[] parents=[\"A\"]",
        "C3 [commit] id=\"B\" branch=\"main\" kind=normal message=\"base\" tags=[] parents=[\"A\"]",
        "C4 [commit] id=\"M\" branch=\"main\" kind=merge message=\"merged branch feature into main\" tags=[\"v1\"] parents=[\"B\", \"F\"]",
    ] {
        assert!(
            records.iter().any(|record| record == expected),
            "{rendered}"
        );
    }
    let details = record(&records, "C4 details:");
    for value in ["seq:3", "type:3", "customId:true"] {
        assert!(details.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn git_graph_cherry_pick_distinguishes_ancestry_from_picked_source() {
    let rendered = render_parsed(concat!(
        "gitGraph\n",
        "commit id:\"root\"\n",
        "branch feature\n",
        "commit id:\"topic\" msg:\"fix\"\n",
        "checkout main\n",
        "cherry-pick id:\"topic\" tag:\"chosen\"\n",
    ));

    let records = unwrapped_records(&rendered);
    let picked = record(&records, "C3 [commit] ");
    assert!(picked.starts_with("C3 [commit] id=\""), "{rendered}");
    assert!(
        picked.ends_with(concat!(
            " branch=\"main\" kind=cherry-pick message=\"cherry-picked fix into main\"",
            " tags=[\"chosen\"] parents=[\"root\", \"topic\"] cherry-picked from=\"topic\"",
        )),
        "{rendered}"
    );
    assert_eq!(
        record(&records, "C2 [commit] "),
        "C2 [commit] id=\"topic\" branch=\"feature\" kind=normal message=\"fix\" tags=[] parents=[\"root\"]",
        "{rendered}"
    );
    let details = record(&records, "C3 details:");
    assert!(details.contains("seq:2"), "{rendered}");
    assert!(details.contains("type:4"), "{rendered}");
}

#[test]
fn git_graph_repeated_ids_keep_occurrence_keys_and_literal_parents_in_structured_primary() {
    let mut first = history_commit("same", "main", 0, &[]);
    first.message = "first occurrence".to_string();
    let mut second = history_commit("same", "feature", 1, &[]);
    second.message = "second occurrence".to_string();
    let child = history_commit("child", "main", 2, &["same"]);

    let mut model = history_model(vec![first, second, child]);
    model.branches.push(GitGraphBranch {
        name: "feature".to_string(),
    });
    let rendered = render_primary(model, AsciiProjection::StructuredText);
    let records = unwrapped_records(&rendered);
    let commits = records
        .iter()
        .filter(|record| record.contains(" [commit] "))
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        commits,
        [
            "C1 [commit] id=\"same\" branch=\"main\" kind=normal message=\"first occurrence\" tags=[] parents=[]",
            "C2 [commit] id=\"same\" branch=\"feature\" kind=normal message=\"second occurrence\" tags=[] parents=[]",
            "C3 [commit] id=\"child\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[\"same\"]",
        ],
        "{rendered}"
    );
}

#[test]
fn git_graph_caller_histories_preserve_forward_external_and_repeated_references() {
    let first = history_commit("earlier", "main", 0, &["later"]);
    let second = history_commit("later", "main", 1, &["earlier"]);
    let third = history_commit("external child", "main", 2, &["outside", "outside"]);

    let rendered = render_primary(
        history_model(vec![first, second, third]),
        AsciiProjection::StructuredText,
    );
    let records = unwrapped_records(&rendered);
    for (key, expected) in [
        (
            "C1 [commit] ",
            "C1 [commit] id=\"earlier\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[\"later\"]",
        ),
        (
            "C2 [commit] ",
            "C2 [commit] id=\"later\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[\"earlier\"]",
        ),
        (
            "C3 [commit] ",
            "C3 [commit] id=\"external child\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[\"outside\", \"outside\"]",
        ),
    ] {
        assert_eq!(record(&records, key), expected, "{rendered}");
    }
}

#[test]
fn git_graph_noncanonical_sequences_preserve_model_order_and_literal_values() {
    let rendered = render_primary(
        history_model(vec![
            history_commit("earlier", "main", 8, &[]),
            history_commit("later", "main", -1, &["earlier"]),
            history_commit("last", "main", 3, &["later"]),
        ]),
        AsciiProjection::StructuredText,
    );
    let records = unwrapped_records(&rendered);
    for (key, id, sequence) in [(1, "earlier", 8), (2, "later", -1), (3, "last", 3)] {
        let identity = record(&records, &format!("C{key} [commit] "));
        assert!(identity.contains(&format!("id=\"{id}\"")), "{rendered}");
        let details = record(&records, &format!("C{key} details:"));
        assert!(details.contains(&format!("seq:{sequence}")), "{rendered}");
    }
}

#[test]
fn git_graph_undeclared_commit_branch_retains_literal_identity_in_structured_primary() {
    let rendered = render_primary(
        history_model(vec![
            history_commit("root", "main", 0, &[]),
            history_commit("topic", "feature", 1, &["root"]),
        ]),
        AsciiProjection::StructuredText,
    );
    let records = unwrapped_records(&rendered);
    assert_eq!(
        record(&records, "C2 [commit] "),
        "C2 [commit] id=\"topic\" branch=\"feature\" kind=normal message=\"\" tags=[] parents=[\"root\"]",
        "{rendered}"
    );
    assert_eq!(
        record(&records, "B1 [branch] "),
        "B1 [branch] name=\"main\""
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.contains(" [branch] "))
            .count(),
        1,
        "an undeclared branch must not be invented:\n{rendered}"
    );
}

#[test]
fn git_graph_repeated_branch_names_preserve_branch_occurrences_in_structured_primary() {
    let mut model = history_model(vec![history_commit("root", "main", 0, &[])]);
    model
        .branches
        .extend(["feature", "main"].map(|name| GitGraphBranch {
            name: name.to_string(),
        }));
    let rendered = render_primary(model, AsciiProjection::StructuredText);
    let records = unwrapped_records(&rendered);
    let branches = records
        .iter()
        .filter(|record| record.contains(" [branch] "))
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(
        branches,
        [
            "B1 [branch] name=\"main\"",
            "B2 [branch] name=\"feature\"",
            "B3 [branch] name=\"main\"",
        ],
        "{rendered}"
    );
    assert_eq!(
        record(&records, "C1 [commit] "),
        "C1 [commit] id=\"root\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[]"
    );
}

#[test]
fn git_graph_tracks_follow_lr_tb_and_bt_with_fixed_canonical_branches() {
    let mut merge = history_commit("merge", "main", 3, &["base", "topic"]);
    merge.commit_type = 3;
    let mut model = history_model(vec![
        history_commit("root", "main", 0, &[]),
        history_commit("topic", "feature", 1, &["root"]),
        history_commit("base", "main", 2, &["root"]),
        merge,
    ]);
    model.branches.push(GitGraphBranch {
        name: "feature".to_string(),
    });
    for direction in ["LR", "TB", "BT"] {
        model.direction = direction.to_string();
        let rendered = render_primary(model.clone(), AsciiProjection::Diagrammatic);
        let (topology, _) = rendered
            .split_once("GitGraph records")
            .expect("tracks should be followed by complete records");
        let position = |key: &str| {
            topology
                .lines()
                .enumerate()
                .find_map(|(row, line)| line.find(key).map(|column| (row, column)))
                .unwrap_or_else(|| panic!("missing track key {key}:\n{rendered}"))
        };
        let [root, topic, base, merge] = ["C1", "C2", "C3", "C4"].map(position);
        let [main, feature] = ["B1", "B2"].map(position);
        if direction == "LR" {
            assert!(
                root.1 < topic.1 && topic.1 < base.1 && base.1 < merge.1,
                "{rendered}"
            );
            assert_eq!(root.0, base.0, "{rendered}");
            assert_eq!(base.0, merge.0, "{rendered}");
            assert!(root.0 < topic.0 && main.0 < feature.0, "{rendered}");
        } else {
            assert_eq!(root.1, base.1, "{rendered}");
            assert_eq!(base.1, merge.1, "{rendered}");
            assert!(root.1 < topic.1 && main.1 < feature.1, "{rendered}");
            if direction == "TB" {
                assert!(
                    root.0 < topic.0 && topic.0 < base.0 && base.0 < merge.0,
                    "{rendered}"
                );
            } else {
                assert!(
                    root.0 > topic.0 && topic.0 > base.0 && base.0 > merge.0,
                    "{rendered}"
                );
            }
        }
        let records = unwrapped_records(&rendered);
        assert_eq!(
            record(&records, "B1 [branch] "),
            "B1 [branch] name=\"main\""
        );
        assert_eq!(
            record(&records, "B2 [branch] "),
            "B2 [branch] name=\"feature\""
        );
        assert!(
            record(&records, "C4 [commit] ").ends_with("parents=[\"base\", \"topic\"]"),
            "{rendered}"
        );
    }
}

#[test]
fn git_graph_extra_metadata_is_separate_and_preserves_explicit_false() {
    let root = history_commit("root", "main", 0, &[]);
    let mut overridden = root.clone();
    overridden.custom_type = Some(0);
    overridden.custom_id = Some(false);
    let ordinary = render(RenderSemanticModel::GitGraph(history_model(vec![root])));
    let generated = render(RenderSemanticModel::GitGraph(history_model(vec![
        overridden.clone(),
    ])));
    overridden.custom_id = Some(true);
    let explicit = render(RenderSemanticModel::GitGraph(history_model(vec![
        overridden,
    ])));

    assert_ne!(ordinary, generated);
    assert_ne!(generated, explicit);
    let ordinary_records = unwrapped_records(&ordinary);
    let generated_records = unwrapped_records(&generated);
    let explicit_records = unwrapped_records(&explicit);
    assert_eq!(
        record(&ordinary_records, "C1 [commit] "),
        record(&generated_records, "C1 [commit] ")
    );
    assert_eq!(
        record(&generated_records, "C1 [commit] "),
        record(&explicit_records, "C1 [commit] ")
    );
    let ordinary_details = record(&ordinary_records, "C1 details:");
    assert!(ordinary_details.contains("seq:0"), "{ordinary}");
    assert!(ordinary_details.contains("type:0"), "{ordinary}");
    assert!(!ordinary_details.contains("customType:"), "{ordinary}");
    assert!(!ordinary_details.contains("customId:"), "{ordinary}");
    for (records, expected) in [
        (&generated_records, "customId:false"),
        (&explicit_records, "customId:true"),
    ] {
        let details = record(records, "C1 details:");
        assert!(details.contains("customType:0"), "{details}");
        assert!(details.contains(expected), "{details}");
    }
}

#[test]
fn git_graph_wide_histories_retain_every_branch_commit_and_parent_record() {
    let mut commits = vec![history_commit("root", "main", 0, &[])];
    for index in 1..=10 {
        commits.push(history_commit(
            &format!("base {index}"),
            &format!("branch {index}"),
            index,
            &["root"],
        ));
    }
    for index in 1..=10 {
        commits.push(history_commit(
            &format!("head {index}"),
            &format!("branch {index}"),
            index + 10,
            &[&format!("base {index}")],
        ));
    }

    let mut model = history_model(commits);
    model.branches.extend((1..=10).map(|index| GitGraphBranch {
        name: format!("branch {index}"),
    }));
    let rendered = render_primary(model, AsciiProjection::Diagrammatic);
    let records = unwrapped_records(&rendered);
    assert_eq!(
        records
            .iter()
            .filter(|record| record.contains(" [commit] "))
            .count(),
        21,
        "{rendered}"
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| record.contains(" [branch] "))
            .count(),
        11,
        "{rendered}"
    );
    assert_eq!(
        record(&records, "B1 [branch] "),
        "B1 [branch] name=\"main\""
    );
    assert_eq!(
        record(&records, "C1 [commit] "),
        "C1 [commit] id=\"root\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[]"
    );
    for index in 1..=10 {
        let branch_key = index + 1;
        let head_key = index + 11;
        assert_eq!(
            record(&records, &format!("B{branch_key} [branch] ")),
            format!("B{branch_key} [branch] name=\"branch {index}\"")
        );
        assert_eq!(
            record(&records, &format!("C{branch_key} [commit] ")),
            format!(
                "C{branch_key} [commit] id=\"base {index}\" branch=\"branch {index}\" kind=normal message=\"\" tags=[] parents=[\"root\"]"
            )
        );
        assert_eq!(
            record(&records, &format!("C{head_key} [commit] ")),
            format!(
                "C{head_key} [commit] id=\"head {index}\" branch=\"branch {index}\" kind=normal message=\"\" tags=[] parents=[\"base {index}\"]"
            )
        );
        assert!(
            record(&records, &format!("C{head_key} details:"))
                .contains(&format!("seq:{}", index + 10)),
            "{rendered}"
        );
    }
}

#[test]
fn git_graph_history_safely_preserves_cjk_graphemes_controls_and_html() {
    let mut root = history_commit("root\u{1b}", "主線", 0, &[]);
    root.message = "初期化 Cafe\u{301} 👩‍💻".to_string();
    root.tags = vec!["<v1>&\"".to_string()];
    let tip = history_commit("tip", "主線", 1, &["root\u{1b}"]);
    let mut model = history_model(vec![root, tip]);
    model.branches[0].name = "主線".to_string();
    model.current_branch = "主線".to_string();
    let model = RenderSemanticModel::GitGraph(model);
    for profile in [
        merman_ascii::TerminalWidthProfile::Unicode,
        merman_ascii::TerminalWidthProfile::Cjk,
    ] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let options = options.with_terminal_width_profile(profile);
            let text = render_with_options(model.clone(), &options);
            let records = unwrapped_records(&text);
            assert!(!text.contains('\u{1b}'), "{text:?}");
            assert!(
                record(&records, "C2 [commit] ").ends_with("parents=[\"root\\u{1B}\"]"),
                "{text}"
            );
            let root = record(&records, "C1 [commit] ");
            for value in [
                "id=\"root\\u{1B}\"",
                "branch=\"主線\"",
                "message=\"初期化 Cafe\u{301} 👩‍💻\"",
                r#"tags=["<v1>&\""]"#,
            ] {
                assert!(root.contains(value), "missing {value}:\n{text}");
            }
            let html = render_with_options(
                model.clone(),
                &options.with_color_mode(merman_ascii::AsciiColorMode::Html),
            );
            assert!(!html.contains("<v1>"), "{html}");
            assert!(html.contains("&lt;v1&gt;&amp;"), "{html}");
            assert!(!html.contains('\u{1b}'), "{html:?}");
        }
    }
}

#[test]
fn git_graph_narrow_viewport_retains_complete_family_records_within_width() {
    let mut root = history_commit("root", "main", 0, &[]);
    root.message = "initialize repository".to_string();
    let mut tip = history_commit("tip", "main", 1, &["root"]);
    tip.message = "a complete authored message that exceeds the requested viewport".to_string();
    tip.tags = vec!["v1".to_string(), "stable".to_string()];
    tip.custom_type = Some(2);
    tip.custom_id = Some(false);
    let mut model = history_model(vec![
        root,
        tip,
        history_commit("release", "main", 2, &["tip"]),
        history_commit("tail", "main", 3, &["release"]),
    ]);
    model.branches.push(GitGraphBranch {
        name: "unused feature".to_string(),
    });
    model.title = Some("Repository history".to_string());
    model.acc_title = Some("Git title".to_string());
    model.acc_descr = Some("Git description".to_string());
    model.warning_facts.push(DiagramWarningFact::new(
        "test.git_graph.warning",
        "retain authored warning",
    ));
    let model = RenderSemanticModel::GitGraph(model);
    let report = support::render_model_report(
        &model,
        &AsciiRenderOptions::ascii(),
        AsciiViewportPolicy::with_max_width(20).overflow(OverflowPolicy::Fallback),
    )
    .expect("narrow GitGraph output should retain complete semantic fallback");

    assert_eq!(report.outcome, AsciiOutputOutcome::Fallback);
    assert_eq!(report.projection, AsciiProjection::StructuredText);
    assert!(report.fallback.attempted);
    assert!(!report.emitted_overflowed, "{}", report.text);
    assert!(report.emitted_extent.width <= 20, "{}", report.text);
    assert!(
        support::terminal_extent(&report.text).0 <= 20,
        "{}",
        report.text
    );
    let records = unwrapped_records(&report.text);
    assert_eq!(
        records
            .iter()
            .filter(|record| *record == "GitGraph records")
            .count(),
        1,
        "{}",
        report.text
    );
    let flattened = records.join("");
    for value in [
        "B1 [branch] name=\"main\"",
        "B2 [branch] name=\"unused feature\"",
        "C1 [commit] id=\"root\" branch=\"main\" kind=normal message=\"initialize repository\" tags=[] parents=[]",
        "C2 [commit] id=\"tip\" branch=\"main\" kind=normal message=\"a complete authored message that exceeds the requested viewport\" tags=[\"v1\", \"stable\"] parents=[\"root\"]",
        "C3 [commit] id=\"release\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[\"tip\"]",
        "C4 [commit] id=\"tail\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[\"release\"]",
        "currentBranch:\"main\"",
        "direction:\"LR\"",
        "type:\"gitGraph\"",
        "title:\"Repository history\"",
        "accTitle:\"Git title\"",
        "accDescr:\"Git description\"",
        "sourceConfig:{}",
        "sourceTitle:null",
        "warningFacts:[{",
        "\"ruleId\":\"test.git_graph.warning\"",
        "\"message\":\"retain authored warning\"",
    ] {
        assert!(
            flattened.contains(value),
            "missing {value}:\n{}",
            report.text
        );
    }
    for sequence in 0..4 {
        let details = record(&records, &format!("C{} details:", sequence + 1));
        assert!(
            details.contains(&format!("seq:{sequence}")),
            "{}",
            report.text
        );
        assert!(details.contains("type:0"), "{}", report.text);
    }
    let details = record(&records, "C2 details:");
    assert!(details.contains("customType:2"), "{}", report.text);
    assert!(details.contains("customId:false"), "{}", report.text);
    assert!(!flattened.contains("model.commits["), "{}", report.text);
}

#[test]
fn git_graph_render_model_renders_branches_commits_and_warnings() {
    let model = GitGraphDiagramModel {
        diagram_type: "caller-git-history".to_string(),
        commits: vec![GitGraphCommit {
            id: "c0".to_string(),
            message: "init".to_string(),
            seq: 0,
            commit_type: 2,
            tags: vec!["v1".to_string()],
            parents: vec!["seed".to_string()],
            branch: "main".to_string(),
            custom_type: Some(7),
            custom_id: Some(true),
        }],
        branches: vec![
            GitGraphBranch {
                name: "main".to_string(),
            },
            GitGraphBranch {
                name: "feature".to_string(),
            },
        ],
        current_branch: "main".to_string(),
        direction: "TB".to_string(),
        title: Some("Repository history".to_string()),
        acc_title: Some("Git title".to_string()),
        acc_descr: Some("Git description".to_string()),
        warning_facts: vec![DiagramWarningFact::new(
            GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID,
            "duplicate head",
        )],
    };

    let rendered = render_primary(model, AsciiProjection::StructuredText);
    let records = unwrapped_records(&rendered);
    assert_eq!(
        record(&records, "B1 [branch] "),
        "B1 [branch] name=\"main\""
    );
    assert_eq!(
        record(&records, "B2 [branch] "),
        "B2 [branch] name=\"feature\""
    );
    assert_eq!(
        record(&records, "C1 [commit] "),
        "C1 [commit] id=\"c0\" branch=\"main\" kind=highlight message=\"init\" tags=[\"v1\"] parents=[\"seed\"]"
    );
    let commit_details = record(&records, "C1 details:");
    for value in ["seq:0", "type:2", "customType:7", "customId:true"] {
        assert!(
            commit_details.contains(value),
            "missing {value}:\n{rendered}"
        );
    }
    let root_details = record(&records, "gitGraph details:");
    for value in [
        "currentBranch:\"main\"",
        "direction:\"TB\"",
        "title:\"Repository history\"",
        "accTitle:\"Git title\"",
        "accDescr:\"Git description\"",
        "type:\"caller-git-history\"",
        "sourceConfig:{}",
        "sourceTitle:null",
        "warningFacts:[{",
        "\"message\":\"duplicate head\"",
    ] {
        assert!(root_details.contains(value), "missing {value}:\n{rendered}");
    }
    assert!(
        root_details.contains(&format!(
            "\"ruleId\":\"{GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID}\""
        )),
        "{rendered}"
    );
}

#[test]
fn git_graph_commit_message_and_metadata_are_quoted_without_collisions() {
    let base = GitGraphDiagramModel {
        diagram_type: "gitGraph".to_string(),
        commits: Vec::new(),
        branches: Vec::new(),
        current_branch: String::new(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };
    let mut message = base.clone();
    message.commits.push(GitGraphCommit {
        id: "c0".to_string(),
        message: "init tags=[v1]".to_string(),
        seq: 0,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    });
    let mut metadata = base;
    metadata.commits.push(GitGraphCommit {
        id: "c0".to_string(),
        message: "init".to_string(),
        seq: 0,
        commit_type: 0,
        tags: vec!["v1".to_string()],
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    });

    let message_output = render(RenderSemanticModel::GitGraph(message));
    let metadata_output = render(RenderSemanticModel::GitGraph(metadata));
    assert_ne!(message_output, metadata_output);
    let message_records = unwrapped_records(&message_output);
    let metadata_records = unwrapped_records(&metadata_output);
    assert_eq!(
        record(&message_records, "C1 [commit] "),
        "C1 [commit] id=\"c0\" branch=\"main\" kind=normal message=\"init tags=[v1]\" tags=[] parents=[]"
    );
    assert_eq!(
        record(&metadata_records, "C1 [commit] "),
        "C1 [commit] id=\"c0\" branch=\"main\" kind=normal message=\"init\" tags=[\"v1\"] parents=[]"
    );

    let mut comma_message = GitGraphDiagramModel {
        diagram_type: "gitGraph".to_string(),
        commits: vec![GitGraphCommit {
            id: "c0".to_string(),
            message: "x".to_string(),
            seq: 0,
            commit_type: 0,
            tags: vec!["a, b".to_string()],
            parents: Vec::new(),
            branch: "main".to_string(),
            custom_type: None,
            custom_id: None,
        }],
        branches: Vec::new(),
        current_branch: String::new(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };
    let mut comma_tags = comma_message.clone();
    let joined_tags = render(RenderSemanticModel::GitGraph(comma_message.clone()));
    comma_message.commits[0].tags.clear();
    comma_message.commits[0].message = "x tags=[a, b]".to_string();
    comma_tags.commits[0].tags = vec!["a".to_string(), "b".to_string()];
    let split_tags = render(RenderSemanticModel::GitGraph(comma_tags.clone()));
    assert_ne!(
        joined_tags, split_tags,
        "one tag must remain distinct from two tags"
    );
    assert!(record(&unwrapped_records(&joined_tags), "C1 [commit] ").contains("tags=[\"a, b\"]"));
    assert!(
        record(&unwrapped_records(&split_tags), "C1 [commit] ").contains("tags=[\"a\", \"b\"]")
    );
    assert_ne!(
        render(RenderSemanticModel::GitGraph(comma_message)),
        split_tags,
        "quoted fields must distinguish embedded delimiters from list structure"
    );

    let mut leading = comma_tags.clone();
    leading.commits[0].message = " init".to_string();
    leading.commits[0].tags.clear();
    let mut trailing = comma_tags;
    trailing.commits[0].message = "init ".to_string();
    trailing.commits[0].tags.clear();
    assert_ne!(
        render(RenderSemanticModel::GitGraph(leading)),
        render(RenderSemanticModel::GitGraph(trailing)),
        "quoted fields must preserve equal-length leading and trailing whitespace"
    );
}

#[test]
fn git_graph_render_model_rejects_unknown_commit_types() {
    for commit_type in [98, 99] {
        let model = GitGraphDiagramModel {
            diagram_type: "gitGraph".to_string(),
            commits: vec![GitGraphCommit {
                id: "c0".to_string(),
                message: "unknown".to_string(),
                seq: 0,
                commit_type,
                tags: Vec::new(),
                parents: Vec::new(),
                branch: "main".to_string(),
                custom_type: None,
                custom_id: None,
            }],
            branches: Vec::new(),
            current_branch: "main".to_string(),
            direction: "TB".to_string(),
            title: None,
            acc_title: None,
            acc_descr: None,
            warning_facts: Vec::new(),
        };

        let error = render_model(
            &RenderSemanticModel::GitGraph(model),
            &AsciiRenderOptions::ascii(),
        )
        .expect_err("unknown direct-model commit types must not disappear");
        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "gitGraph",
                feature: "unknown commit types",
            }
        );
    }
}

#[test]
fn git_graph_branch_and_commit_identity_fields_are_quoted() {
    let base = GitGraphDiagramModel {
        diagram_type: "gitGraph".to_string(),
        commits: Vec::new(),
        branches: Vec::new(),
        current_branch: String::new(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };
    let mut joined_branches = base.clone();
    joined_branches.branches = vec![GitGraphBranch {
        name: "main, feature".to_string(),
    }];
    let mut split_branches = base.clone();
    split_branches.branches = vec![
        GitGraphBranch {
            name: "main".to_string(),
        },
        GitGraphBranch {
            name: "feature".to_string(),
        },
    ];
    let joined_output = render(RenderSemanticModel::GitGraph(joined_branches));
    let split_output = render(RenderSemanticModel::GitGraph(split_branches));
    assert_ne!(
        joined_output, split_output,
        "branch-list delimiters must not be forgeable by authored branch names"
    );
    let joined_records = unwrapped_records(&joined_output);
    let split_records = unwrapped_records(&split_output);
    assert_eq!(
        record(&joined_records, "B1 [branch] "),
        "B1 [branch] name=\"main, feature\""
    );
    assert_eq!(
        record(&split_records, "B1 [branch] "),
        "B1 [branch] name=\"main\""
    );
    assert_eq!(
        record(&split_records, "B2 [branch] "),
        "B2 [branch] name=\"feature\""
    );

    let mut joined_identity = base.clone();
    joined_identity.commits.push(GitGraphCommit {
        id: "c0 x".to_string(),
        message: String::new(),
        seq: 0,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main".to_string(),
        custom_type: None,
        custom_id: None,
    });
    let mut split_identity = base;
    split_identity.commits.push(GitGraphCommit {
        id: "x".to_string(),
        message: String::new(),
        seq: 0,
        commit_type: 0,
        tags: Vec::new(),
        parents: Vec::new(),
        branch: "main c0".to_string(),
        custom_type: None,
        custom_id: None,
    });
    let joined_output = render(RenderSemanticModel::GitGraph(joined_identity));
    let split_output = render(RenderSemanticModel::GitGraph(split_identity));
    assert_ne!(
        joined_output, split_output,
        "commit branch and id ownership must remain distinguishable"
    );
    let joined_records = unwrapped_records(&joined_output);
    let split_records = unwrapped_records(&split_output);
    assert_eq!(
        record(&joined_records, "C1 [commit] "),
        "C1 [commit] id=\"c0 x\" branch=\"main\" kind=normal message=\"\" tags=[] parents=[]"
    );
    assert_eq!(
        record(&split_records, "C1 [commit] "),
        "C1 [commit] id=\"x\" branch=\"main c0\" kind=normal message=\"\" tags=[] parents=[]"
    );
}

#[test]
fn git_graph_rendering_observes_operation_cancellation() {
    let model = GitGraphDiagramModel {
        diagram_type: "gitGraph".to_string(),
        commits: (0..32)
            .map(|seq| GitGraphCommit {
                id: format!("c{seq}"),
                message: format!("commit {seq}"),
                seq,
                commit_type: 0,
                tags: Vec::new(),
                parents: Vec::new(),
                branch: "main".to_string(),
                custom_type: None,
                custom_id: None,
            })
            .collect(),
        branches: vec![GitGraphBranch {
            name: "main".to_string(),
        }],
        current_branch: "main".to_string(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };

    let error = render_with_scheduled_cancellation(RenderSemanticModel::GitGraph(model), 1);
    assert!(matches!(error, AsciiError::Cancelled(_)));
}
