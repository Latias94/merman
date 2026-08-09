use super::*;

#[test]
fn flowchart_local_semantic_fixture_covers_nested_direction_boundary_routes() {
    let input = local_semantic_input("flowchart/nested_direction_boundary.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic nested flowchart fixture should render");

    for expected in [
        "Start",
        "Outer Pipeline",
        "Inner Steps",
        "Entry",
        "Validate",
        "Persist",
        "Done",
    ] {
        assert!(
            rendered.contains(expected),
            "nested flowchart fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);

    assert!(
        line_index("Start") < line_index("Entry"),
        "root TD direction should keep Start above the outer group entry:\n{rendered}"
    );
    assert_eq!(
        line_index("Entry"),
        line_index("Validate"),
        "outer LR override should keep Entry and Validate on the same row:\n{rendered}"
    );
    assert!(
        line_index("Validate") < line_index("Persist"),
        "inner TD override should keep Validate above Persist:\n{rendered}"
    );
    assert!(
        line_index("Persist") < line_index("Done"),
        "cross-boundary exit edge should keep Done after Persist in root TD flow:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 10,
        "local semantic flowchart fixture should produce a non-trivial layout:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_multiple_boundary_routes() {
    let input = local_semantic_input("flowchart/multi_boundary_routes.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic multi-boundary flowchart fixture should render");

    for expected in [
        "Source", "Audit", "Pipeline", "Ingest", "Validate", "Publish", "Success", "Retry", "load",
        "check", "ok", "fail",
    ] {
        assert!(
            rendered.contains(expected),
            "multi-boundary flowchart fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);

    assert!(
        line_index("Source") < line_index("Ingest"),
        "first entering boundary edge should preserve root TD ordering:\n{rendered}"
    );
    assert!(
        line_index("Audit") < line_index("Validate"),
        "second entering boundary edge should preserve root TD ordering:\n{rendered}"
    );
    assert_eq!(
        line_index("Ingest"),
        line_index("Validate"),
        "subgraph LR override should keep Ingest and Validate on the same row:\n{rendered}"
    );
    assert_eq!(
        line_index("Validate"),
        line_index("Publish"),
        "subgraph LR override should keep Validate and Publish on the same row:\n{rendered}"
    );
    assert!(
        line_index("Publish") < line_index("Success"),
        "first leaving boundary edge should preserve root TD ordering:\n{rendered}"
    );
    assert!(
        line_index("Publish") < line_index("Retry"),
        "second leaving boundary edge should preserve root TD ordering:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 10,
        "multi-boundary flowchart fixture should produce a non-trivial layout:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_boundary_leaving_route_reserves_long_label_extent() {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "Start --> Ingest\n",
            "subgraph pipe [Pipeline]\n",
            "    direction LR\n",
            "    Ingest --> Publish\n",
            "end\n",
            "Publish -->|boundaryLabelWithEnoughWidth| Success[Success]\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("boundary-leaving flowchart route should render");

    assert!(
        rendered.contains("boundaryLabelWithEnoughWidth"),
        "boundary-leaving route should not clip its long label:\n{rendered}"
    );
    for expected in ["Pipeline", "Ingest", "Publish"] {
        assert!(
            rendered.contains(expected),
            "boundary-leaving route should keep {expected:?} visible:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_local_semantic_fixture_covers_boundary_label_lane() {
    let input = local_semantic_input("flowchart/boundary_label_lane.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic boundary-label lane fixture should render");

    for expected in [
        "Start",
        "Pipeline",
        "Ingest",
        "Publish",
        "Success",
        "boundaryLabelWithEnoughWidth",
    ] {
        assert!(
            rendered.contains(expected),
            "boundary-label lane fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    let label_line = rendered
        .lines()
        .find(|line| line.contains("boundaryLabelWithEnoughWidth"))
        .unwrap_or_else(|| panic!("boundary label should render:\n{rendered}"));
    assert!(
        label_line.contains("|boundaryLabelWithEnoughWidth"),
        "boundary label should attach to the planned vertical transit lane, not the target node row:\n{rendered}"
    );

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(
        line_index("Publish") < line_index("boundaryLabelWithEnoughWidth"),
        "boundary label should render after the source endpoint on the boundary lane:\n{rendered}"
    );
    assert!(
        line_index("boundaryLabelWithEnoughWidth") < line_index("Success"),
        "boundary label should render before the target endpoint on the boundary lane:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_multiline_edge_labels() {
    let input = local_semantic_input("flowchart/multiline_edge_label.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic multiline-edge fixture should render");

    for expected in ["A", "B", "north", "south"] {
        assert!(
            rendered.contains(expected),
            "multiline edge-label fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("<br>"),
        "multiline edge-label fixture should not leak HTML break markup:\n{rendered}"
    );

    let north = first_line_index_containing(&rendered, "north");
    let south = first_line_index_containing(&rendered, "south");
    assert_eq!(
        south,
        north + 1,
        "multiline edge-label fixture should stack label rows in order:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_back_edge_labels() {
    let input = local_semantic_input("flowchart/back_edge_labels.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic back-edge label fixture should render");

    for expected in ["A", "B", "C", "back to top", "back to middle"] {
        assert!(
            rendered.contains(expected),
            "back-edge label fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(
        line_index("A") < line_index("B"),
        "root TD flow should keep A above B:\n{rendered}"
    );
    assert!(
        line_index("B") < line_index("C"),
        "root TD flow should keep B above C:\n{rendered}"
    );
    assert_ne!(
        line_index("back to top"),
        line_index("back to middle"),
        "separate back-edge labels should not overwrite each other:\n{rendered}"
    );
    assert!(
        line_index("back to top") < line_index("back to middle"),
        "back-edge labels should retain their planned order without merging:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_sibling_group_boundary_routes() {
    let input = local_semantic_input("flowchart/sibling_boundary_routes.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic sibling-boundary flowchart fixture should render");

    for expected in [
        "Left Group",
        "Right Group",
        "Alpha",
        "Beta",
        "Gamma",
        "Delta",
        "handoff",
    ] {
        assert!(
            rendered.contains(expected),
            "sibling-boundary flowchart fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);

    assert!(
        line_index("Left Group") < line_index("Right Group"),
        "root TD direction should place the source sibling group before the target group:\n{rendered}"
    );
    assert!(
        line_index("Alpha") < line_index("Beta"),
        "source sibling group should preserve its internal TD chain:\n{rendered}"
    );
    assert!(
        line_index("Beta") < line_index("handoff"),
        "cross-boundary label should render after the source endpoint:\n{rendered}"
    );
    assert!(
        line_index("handoff") < line_index("Gamma"),
        "cross-boundary label should render before the target endpoint:\n{rendered}"
    );
    assert!(
        line_index("Gamma") < line_index("Delta"),
        "target sibling group should preserve its internal TD chain:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_disconnected_subgraphs() {
    let input = local_semantic_input("flowchart/disconnected_subgraphs.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic disconnected-subgraphs flowchart fixture should render");

    for expected in [
        "Today",
        "Next Wave",
        "Today AI",
        "Today Markdown",
        "Today Reads",
        "Next AI",
        "Next Widget",
        "Next Acts",
    ] {
        assert!(
            rendered.contains(expected),
            "disconnected-subgraphs fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    assert_rectangular_char_grid(&rendered);

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(
        line_index("Today") < line_index("Next Wave"),
        "LR disconnected subgraphs should stack into separate visible groups without overlap:\n{rendered}"
    );
    assert_eq!(
        line_index("Today AI"),
        line_index("Today Markdown"),
        "first disconnected group should keep its LR chain on one row:\n{rendered}"
    );
    assert_eq!(
        line_index("Today Markdown"),
        line_index("Today Reads"),
        "first disconnected group should keep its full LR chain on one row:\n{rendered}"
    );
    assert_eq!(
        line_index("Next AI"),
        line_index("Next Widget"),
        "second disconnected group should keep its LR chain on one row:\n{rendered}"
    );
    assert_eq!(
        line_index("Next Widget"),
        line_index("Next Acts"),
        "second disconnected group should keep its full LR chain on one row:\n{rendered}"
    );
    assert!(
        line_index("Today Reads") < line_index("Next AI"),
        "disconnected group content should not overlap or merge into one row:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_cjk_boundary_routes() {
    let input = local_semantic_input("flowchart/cjk_boundary_routes.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic CJK boundary-route flowchart fixture should render");

    for expected in [
        "入口",
        "流程中枢",
        "校验层",
        "检查",
        "解析",
        "验证",
        "发布",
        "完成",
    ] {
        assert!(
            rendered.contains(expected),
            "CJK boundary-route fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("pipe") && !rendered.contains("validate"),
        "CJK boundary-route fixture should not leak subgraph ids into ASCII output:\n{rendered}"
    );

    let line_index = |needle: &str| {
        rendered
            .lines()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
    };
    assert!(
        line_index("检查") <= line_index("解析"),
        "CJK entry chain should keep the first step before the second step:\n{rendered}"
    );
    assert!(
        line_index("解析") <= line_index("验证"),
        "CJK entry chain should keep the second step before the validation step:\n{rendered}"
    );
    assert!(
        line_index("验证") < line_index("发布"),
        "validation step should precede the publish step in the semantic fixture:\n{rendered}"
    );
    assert!(
        line_index("发布") < line_index("完成"),
        "success boundary route should keep the completion node after the publish step:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 10,
        "CJK boundary-route fixture should produce a non-trivial multi-line layout:\n{rendered}"
    );
}

#[test]
fn flowchart_local_semantic_fixture_covers_ampersand_fanin_and_fanout() {
    let input = local_semantic_input("flowchart/ampersand_fanin_fanout.mmd");
    let rendered = render_flowchart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic ampersand flowchart fixture should render");

    for expected in [
        "SourceA", "SourceB", "Merge", "Fanout", "TargetA", "TargetB",
    ] {
        assert!(
            rendered.contains(expected),
            "ampersand fan-in/fan-out fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert_eq!(
        line_index("SourceA"),
        line_index("SourceB"),
        "fan-in sources should stay on the same semantic rank:\n{rendered}"
    );
    assert!(
        line_index("SourceA") < line_index("Merge"),
        "fan-in target should render after both source nodes:\n{rendered}"
    );
    assert!(
        line_index("Fanout") < line_index("TargetA"),
        "fan-out source should render before target nodes:\n{rendered}"
    );
    assert_eq!(
        line_index("TargetA"),
        line_index("TargetB"),
        "fan-out targets should stay on the same semantic rank:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_explicit_td_sibling_group_preserves_local_order_after_external_edges() {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "subgraph left [Left Group]\n",
            "    direction LR\n",
            "    A[Alpha] --> B[Beta]\n",
            "end\n",
            "subgraph right [Right Group]\n",
            "    direction TD\n",
            "    C[Gamma] --> D[Delta]\n",
            "end\n",
            "B -- handoff --> C\n",
            "A -- audit --> D\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("explicit TD sibling group should render with external edges");

    for expected in [
        "Left Group",
        "Right Group",
        "Alpha",
        "Beta",
        "Gamma",
        "Delta",
        "handoff",
        "audit",
    ] {
        assert!(
            rendered.contains(expected),
            "mixed-direction sibling group output should keep {expected:?} visible:\n{rendered}"
        );
    }

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);

    assert_eq!(
        line_index("Alpha"),
        line_index("Beta"),
        "left sibling LR group should keep Alpha and Beta on the same row:\n{rendered}"
    );
    assert!(
        line_index("Gamma") < line_index("Delta"),
        "explicit right sibling TD group should keep Gamma above Delta despite external edges:\n{rendered}"
    );
}
