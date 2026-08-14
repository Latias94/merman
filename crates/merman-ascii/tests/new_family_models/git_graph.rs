use super::*;

#[test]
fn git_graph_render_model_renders_branches_commits_and_warnings() {
    let model = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: vec![GitGraphCommitRenderModel {
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
            GitGraphBranchRenderModel {
                name: "main".to_string(),
            },
            GitGraphBranchRenderModel {
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
            "gitGraph direction(bytes=2)=\"TB\" current(bytes=4)=\"main\"\n",
            "title(bytes=18)=\"Repository history\"\n",
            "accTitle(bytes=9)=\"Git title\"\n",
            "accDescr(bytes=15)=\"Git description\"\n",
            "branches=[bytes=4 \"main\", bytes=7 \"feature\"]\n",
            "  - seq=0 branch(bytes=4)=\"main\" id(bytes=2)=\"c0\" kind=highlight message(bytes=4)=\"init\" tags=[bytes=2 \"v1\"] parents=[bytes=4 \"seed\"] typeOverride=7 idSource=explicit\n",
            "warnings:\n",
            "  - message(bytes=14)=\"duplicate head\"",
        )
    );
}

#[test]
fn git_graph_commit_message_and_metadata_are_framed_without_collisions() {
    let base = GitGraphRenderModel {
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
    message.commits.push(GitGraphCommitRenderModel {
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
    metadata.commits.push(GitGraphCommitRenderModel {
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
    assert!(
        message_output.contains("message(bytes=14)=")
            && message_output.contains("init")
            && message_output.contains("tags=[v1]")
    );
    assert!(
        metadata_output.contains("message(bytes=4)=\"init\"")
            && metadata_output.contains("tags=[bytes=2 \"v1\"]")
    );

    let mut comma_message = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: vec![GitGraphCommitRenderModel {
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
        "length-framed fields must distinguish embedded delimiters from list structure"
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
        let model = GitGraphRenderModel {
            diagram_type: "gitGraph".to_string(),
            commits: vec![GitGraphCommitRenderModel {
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
fn git_graph_branch_and_commit_identity_fields_are_length_framed() {
    let base = GitGraphRenderModel {
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
    joined_branches.branches = vec![GitGraphBranchRenderModel {
        name: "main, feature".to_string(),
    }];
    let mut split_branches = base.clone();
    split_branches.branches = vec![
        GitGraphBranchRenderModel {
            name: "main".to_string(),
        },
        GitGraphBranchRenderModel {
            name: "feature".to_string(),
        },
    ];
    assert_ne!(
        render(RenderSemanticModel::GitGraph(joined_branches)),
        render(RenderSemanticModel::GitGraph(split_branches)),
        "branch-list delimiters must not be forgeable by authored branch names"
    );

    let mut joined_identity = base.clone();
    joined_identity.commits.push(GitGraphCommitRenderModel {
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
    split_identity.commits.push(GitGraphCommitRenderModel {
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
    let model = GitGraphRenderModel {
        diagram_type: "gitGraph".to_string(),
        commits: (0..32)
            .map(|seq| GitGraphCommitRenderModel {
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
        branches: vec![GitGraphBranchRenderModel {
            name: "main".to_string(),
        }],
        current_branch: "main".to_string(),
        direction: "TB".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        warning_facts: Vec::new(),
    };

    let error = render_with_scheduled_cancellation(RenderSemanticModel::GitGraph(model), 4);
    assert!(matches!(
        error,
        AsciiError::Cancelled(cancelled)
            if cancelled.phase == merman_core::OperationPhase::Emit
    ));
}
