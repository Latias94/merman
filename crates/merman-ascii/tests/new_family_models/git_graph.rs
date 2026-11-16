use super::*;

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

#[test]
fn git_graph_linear_history_reads_newest_first_without_internal_field_dumps() {
    let mut root = history_commit("root", "main", 0, &[]);
    root.message = "initialize repository".to_string();
    let mut tip = history_commit("tip", "main", 1, &["root"]);
    tip.message = "ship".to_string();
    tip.tags = vec!["v1".to_string(), "stable".to_string()];

    let rendered = render(RenderSemanticModel::GitGraph(history_model(vec![
        root, tip,
    ])));

    assert_eq!(
        rendered,
        concat!(
            "Current branch: \"main\"\n",
            "Source direction: \"LR\"\n",
            "Branches: \"main\"\n",
            "\n",
            "Git history (newest first):\n",
            "* \"tip\" [\"main\"] - \"ship\"; tags: \"v1\", \"stable\"\n",
            "| Parent: \"root\"\n",
            "* \"root\" [\"main\"] (root) - \"initialize repository\"",
        )
    );
    for internal_field in ["bytes=", "seq=", "idSource=", "Details"] {
        assert!(!rendered.contains(internal_field), "{rendered}");
    }
}

#[test]
fn git_graph_parsed_branch_merge_retains_both_parents_in_lane_history() {
    let rendered = render_parsed(concat!(
        "gitGraph LR:\n",
        "commit id:\"A\" msg:\"initial\"\n",
        "branch feature\n",
        "commit id:\"F\" msg:\"work\"\n",
        "checkout main\n",
        "commit id:\"B\" msg:\"base\"\n",
        "merge feature id:\"M\" tag:\"v1\"\n",
    ));

    assert!(
        rendered.contains(concat!(
            "Git history (newest first):\n",
            "*   \"M\" [\"main\"; merge] - \"merged branch feature into main\"; tags: \"v1\"\n",
            "+-+ Parents: \"B\", \"F\"\n",
            "* | \"B\" [\"main\"] - \"base\"\n",
            "| | Parent: \"A\"\n",
            "| * \"F\" [\"feature\"] - \"work\"\n",
            "+-+ Parent: \"A\"\n",
            "* \"A\" [\"main\"] (root) - \"initial\"",
        )),
        "{rendered}"
    );
    assert!(rendered.contains("Details (commit occurrences in model order):\n  #4 \"M\": sequence 3; ID source explicit"), "{rendered}");
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

    assert!(rendered.contains("[\"main\"; cherry-pick]"), "{rendered}");
    assert!(
        rendered.contains("Parent: \"root\"; cherry-picked from: \"topic\""),
        "{rendered}"
    );
    assert!(
        rendered.contains("- \"cherry-picked fix into main\"; tags: \"chosen\""),
        "{rendered}"
    );
    assert!(
        rendered.contains("\"topic\" [\"feature\"] - \"fix\""),
        "{rendered}"
    );
}

#[test]
fn git_graph_repeated_ids_keep_all_occurrences_and_mark_ambiguous_parents() {
    let mut first = history_commit("same", "main", 0, &[]);
    first.message = "first occurrence".to_string();
    let mut second = history_commit("same", "feature", 1, &[]);
    second.message = "second occurrence".to_string();
    let child = history_commit("child", "main", 2, &["same"]);

    let rendered = render(RenderSemanticModel::GitGraph(history_model(vec![
        first, second, child,
    ])));

    assert!(
        rendered.contains("Git history (model order; explicit parent records):"),
        "{rendered}"
    );
    assert!(
        rendered.contains("* #1 \"same\" [\"main\"] (root) - \"first occurrence\""),
        "{rendered}"
    );
    assert!(
        rendered.contains("* #2 \"same\" [\"feature\"] (root) - \"second occurrence\""),
        "{rendered}"
    );
    assert!(
        rendered.contains("* #3 \"child\" [\"main\"]\n    Parent: \"same\" (ambiguous ID)"),
        "{rendered}"
    );
}

#[test]
fn git_graph_caller_histories_preserve_forward_external_and_repeated_references() {
    let first = history_commit("earlier", "main", 8, &["later"]);
    let second = history_commit("later", "main", -1, &["earlier"]);
    let third = history_commit("external child", "main", 3, &["outside", "outside"]);

    let rendered = render(RenderSemanticModel::GitGraph(history_model(vec![
        first, second, third,
    ])));

    assert!(rendered.contains("* #1 \"earlier\""), "{rendered}");
    assert!(rendered.contains("Parent: \"later\" (#2)"), "{rendered}");
    assert!(rendered.contains("Parent: \"earlier\" (#1)"), "{rendered}");
    assert!(
        rendered.contains("Parents: \"outside\" (external), \"outside\" (external)"),
        "{rendered}"
    );
    assert!(
        rendered.contains("#1 \"earlier\": sequence 8"),
        "{rendered}"
    );
    assert!(rendered.contains("#2 \"later\": sequence -1"), "{rendered}");
    assert!(
        rendered.contains("#3 \"external child\": sequence 3"),
        "{rendered}"
    );
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
    assert!(!ordinary.contains("Details"), "{ordinary}");
    assert!(generated.contains("Details (commit occurrences in model order):\n  #1 \"root\": sequence 0; type override normal; ID source generated"), "{generated}");
    assert!(explicit.contains("ID source explicit"), "{explicit}");
}

#[test]
fn git_graph_wide_histories_use_complete_relations_instead_of_dropping_lanes() {
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

    let rendered = render(RenderSemanticModel::GitGraph(history_model(commits)));

    assert!(
        rendered.contains("Connections exceed the readable lane layout"),
        "{rendered}"
    );
    assert_eq!(
        rendered
            .lines()
            .filter(|line| line.starts_with("* #"))
            .count(),
        21,
        "{rendered}"
    );
    for index in 1..=10 {
        assert!(
            rendered.contains(&format!("Parent: \"base {index}\" (#{})", index + 1)),
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
    let model = RenderSemanticModel::GitGraph(history_model(vec![root, tip]));
    for profile in [
        merman_ascii::TerminalWidthProfile::Unicode,
        merman_ascii::TerminalWidthProfile::Cjk,
    ] {
        for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
            let options = options.with_terminal_width_profile(profile);
            let text = render_with_options(model.clone(), &options);
            assert!(!text.contains('\u{1b}'), "{text:?}");
            assert!(text.contains("Parent: \"root\\u{1B}\""), "{text}");
            assert!(text.contains("[\"主線\"]"), "{text}");
            assert!(text.contains("初期化 Cafe\u{301} 👩‍💻"), "{text}");
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
fn git_graph_narrow_viewport_retains_the_existing_complete_structured_fallback() {
    let root = history_commit("root", "main", 0, &[]);
    let mut tip = history_commit("tip", "main", 1, &["root"]);
    tip.message = "a complete authored message that exceeds the requested viewport".to_string();
    tip.custom_id = Some(false);
    let model = RenderSemanticModel::GitGraph(history_model(vec![root, tip]));
    let report = support::render_model_report(
        &model,
        &AsciiRenderOptions::ascii(),
        merman_ascii::AsciiViewportPolicy::with_max_width(20)
            .overflow(merman_ascii::OverflowPolicy::Fallback),
    )
    .expect("narrow GitGraph output should retain complete semantic fallback");

    assert_eq!(report.outcome, merman_ascii::AsciiOutputOutcome::Fallback);
    assert_eq!(
        report.projection,
        merman_ascii::AsciiProjection::StructuredText
    );
    assert!(report.fallback.attempted);
    assert!(report.emitted_extent.width <= 20, "{}", report.text);
    let flattened = report.text.replace('\n', "");
    assert!(
        flattened.contains("model.commits[1].parents[0]: \"root\""),
        "{}",
        report.text
    );
    assert!(
        flattened.contains("model.commits[1].customId: false"),
        "{}",
        report.text
    );
    assert!(
        flattened.contains("a complete authored message that exceeds the requested viewport"),
        "{}",
        report.text
    );
}

#[test]
fn git_graph_render_model_renders_branches_commits_and_warnings() {
    let model = GitGraphDiagramModel {
        diagram_type: "gitGraph".to_string(),
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

    let rendered = render(RenderSemanticModel::GitGraph(model));

    assert_eq!(
        rendered,
        concat!(
            "Current branch: \"main\"\n",
            "Source direction: \"TB\"\n",
            "Title: \"Repository history\"\n",
            "Accessible title: \"Git title\"\n",
            "Accessible description: \"Git description\"\n",
            "Branches: \"main\", \"feature\"\n",
            "\n",
            "Git history (model order; explicit parent records):\n",
            "Some parents are external or out of order; explicit references are preserved.\n",
            "* #1 \"c0\" [\"main\"; highlight] - \"init\"; tags: \"v1\"\n",
            "    Parent: \"seed\" (external)\n",
            "\n",
            "Details (commit occurrences in model order):\n",
            "  #1 \"c0\": sequence 0; type override 7; ID source explicit\n",
            "\n",
            "Warnings:\n",
            "  - \"duplicate head\"",
        )
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
    assert!(message_output.contains("- \"init tags=[v1]\"") && !message_output.contains("; tags:"));
    assert!(metadata_output.contains("- \"init\"; tags: \"v1\""));

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
    comma_message.commits[0].tags.clear();
    comma_message.commits[0].message = "x tags=[a, b]".to_string();
    comma_tags.commits[0].tags = vec!["a".to_string(), "b".to_string()];
    assert_ne!(
        render(RenderSemanticModel::GitGraph(comma_message)),
        render(RenderSemanticModel::GitGraph(comma_tags.clone())),
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
    assert_ne!(
        render(RenderSemanticModel::GitGraph(joined_branches)),
        render(RenderSemanticModel::GitGraph(split_branches)),
        "branch-list delimiters must not be forgeable by authored branch names"
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
    assert_ne!(
        render(RenderSemanticModel::GitGraph(joined_identity)),
        render(RenderSemanticModel::GitGraph(split_identity)),
        "commit branch and id ownership must remain distinguishable"
    );
}

#[test]
fn git_graph_summary_loop_observes_operation_cancellation() {
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

    // Commit-type validation now checks each commit before the summary is emitted. Advance past
    // those semantic checks so this fixture continues to exercise the Emit loop itself.
    let successful_checkpoints = model.commits.len() + 4;
    let error = render_with_scheduled_cancellation(
        RenderSemanticModel::GitGraph(model),
        successful_checkpoints,
    );
    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == merman_model::OperationPhase::Emit
    ));
}
