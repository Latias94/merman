use super::*;

#[test]
fn flowchart_parser_simple_subgraph_renders_group_box() {
    let rendered = render_flowchart(
        "flowchart TB\nsubgraph one\nA --> B\nend",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        fixture_expected("ascii", "graph_tb_direction.txt")
    );
}

#[test]
fn flowchart_parser_cross_subgraph_routes_follow_compound_parent_topology() {
    for (name, input, expected_nodes) in [
        (
            "directionless sibling groups",
            "flowchart LR\nsubgraph left\nA\nend\nsubgraph right\nB\nend\nA --> B",
            &["A", "B"][..],
        ),
        (
            "first parent for a repeated node reference",
            "flowchart TD\nsubgraph first\nA --> B\nend\nsubgraph second\nB --> C\nend",
            &["A", "B", "C"][..],
        ),
    ] {
        let rendered =
            render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap_or_else(|error| {
                panic!("{name} should route across owned group borders: {error}")
            });

        for node in expected_nodes.iter().copied() {
            assert!(
                rendered.contains(node),
                "{name} should preserve node {node:?}:\n{rendered}"
            );
        }
        assert_rectangular_char_grid(&rendered);
    }
}

#[test]
fn flowchart_parser_multiline_subgraph_title_renders_centered_rows() {
    let rendered = render_flowchart(
        "flowchart TB\nsubgraph cluster [Line<br>Two]\nA\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("subgraph titles with Mermaid break syntax should render as multiline title rows");

    assert_eq!(
        rendered,
        concat!(
            "+-------+\n",
            "| Line  |\n",
            "|       |\n",
            "|  Two  |\n",
            "|       |\n",
            "|       |\n",
            "| +---+ |\n",
            "| |   | |\n",
            "| | A | |\n",
            "| |   | |\n",
            "| +---+ |\n",
            "|       |\n",
            "+-------+\n",
        )
    );
}

#[test]
fn flowchart_parser_long_subgraph_title_wraps_to_multiple_rows() {
    let rendered = render_flowchart(
        "flowchart LR\nsubgraph cluster [Wrap this title nicely]\nA --> B\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("long subgraph titles should wrap inside the existing group box");

    assert_eq!(
        rendered,
        concat!(
            "+-----------------+\n",
            "| Wrap this title |\n",
            "|                 |\n",
            "|     nicely      |\n",
            "|                 |\n",
            "|                 |\n",
            "| +---+     +---+ |\n",
            "| |   |     |   | |\n",
            "| | A |---->| B | |\n",
            "| |   |     |   | |\n",
            "| +---+     +---+ |\n",
            "|                 |\n",
            "+-----------------+\n",
        )
    );
}

#[test]
fn render_model_subgraph_direction_override_renders_local_left_right_layout_without_cross_boundary_edges()
 {
    let model = merman_core::diagrams::flowchart::FlowchartModel {
        keyword: "graph".to_string(),
        acc_descr: None,
        acc_title: None,
        class_defs: Default::default(),
        direction: Some("TD".to_string()),
        edge_defaults: None,
        vertex_calls: Vec::new(),
        nodes: vec![
            merman_core::diagrams::flowchart::FlowNode {
                id: "A".to_string(),
                label: Some("A".to_string()),
                label_type: None,
                layout_shape: None,
                shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            },
            merman_core::diagrams::flowchart::FlowNode {
                id: "B".to_string(),
                label: Some("B".to_string()),
                label_type: None,
                layout_shape: None,
                shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            },
        ],
        edges: vec![merman_core::diagrams::flowchart::FlowEdge {
            id: "L-A-B".to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some("arrow_point".to_string()),
            arrow: "-->".to_string(),
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::Point,
            is_user_defined_id: false,
            stroke: Some("normal".to_string()),
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }],
        subgraphs: vec![merman_core::diagrams::flowchart::FlowSubgraph {
            id: "one".to_string(),
            title: "LR Group".to_string(),
            dir: Some("LR".to_string()),
            has_explicit_dir: true,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string(), "B".to_string()],
        }],
        tooltips: Default::default(),
        warning_facts: Vec::new(),
    };
    let rendered = render_model(
        &merman_core::RenderSemanticModel::Flowchart(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect("subgraph direction override should render a local LR layout inside a TD graph");

    assert_eq!(
        rendered,
        concat!(
            "+-----------------+\n",
            "|    LR Group     |\n",
            "|                 |\n",
            "|                 |\n",
            "| +---+     +---+ |\n",
            "| |   |     |   | |\n",
            "| | A |---->| B | |\n",
            "| |   |     |   | |\n",
            "| +---+     +---+ |\n",
            "|                 |\n",
            "+-----------------+\n",
        )
    );
}

#[test]
fn flowchart_parser_subgraph_reverse_directions_mirror_local_coordinates() {
    for (direction, expected_axis) in [("RL", "row"), ("BT", "column")] {
        let rendered = render_flowchart(
            &format!(
                "flowchart TB\nsubgraph reverse\n    direction {direction}\n    A --> B\nend\n"
            ),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("local {direction} subgraph should render: {error:?}"));

        let a = rendered
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.find(" A ").map(|x| (x, y)))
            .unwrap_or_else(|| panic!("missing A in {direction} output:\n{rendered}"));
        let b = rendered
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.find(" B ").map(|x| (x, y)))
            .unwrap_or_else(|| panic!("missing B in {direction} output:\n{rendered}"));

        if expected_axis == "row" {
            assert_eq!(
                a.1, b.1,
                "RL local direction should keep the edge horizontal:\n{rendered}"
            );
            assert!(
                b.0 < a.0,
                "RL local direction should place B before A:\n{rendered}"
            );
        } else {
            assert_eq!(
                a.0, b.0,
                "BT local direction should keep the edge vertical:\n{rendered}"
            );
            assert!(
                b.1 < a.1,
                "BT local direction should place B above A:\n{rendered}"
            );
        }
        assert_rectangular_char_grid(&rendered);
    }
}

#[test]
fn flowchart_root_and_local_reverse_directions_compose_once() {
    for (root_direction, local_direction, expected_axis, target_before_source) in [
        ("RL", "RL", "row", true),
        ("RL", "LR", "row", false),
        ("BT", "BT", "column", true),
        ("BT", "TB", "column", false),
    ] {
        let rendered = render_flowchart(
            &format!(
                "flowchart {root_direction}\nsubgraph reverse\n    direction {local_direction}\n    A --> B\nend\n"
            ),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| {
            panic!(
                "root {root_direction} with local {local_direction} should render: {error:?}"
            )
        });

        let a = rendered
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.find(" A ").map(|x| (x, y)))
            .unwrap_or_else(|| panic!("missing A in output:\n{rendered}"));
        let b = rendered
            .lines()
            .enumerate()
            .find_map(|(y, line)| line.find(" B ").map(|x| (x, y)))
            .unwrap_or_else(|| panic!("missing B in output:\n{rendered}"));

        if expected_axis == "row" {
            assert_eq!(
                a.1, b.1,
                "local direction must stay horizontal:\n{rendered}"
            );
            assert_eq!(
                b.0 < a.0,
                target_before_source,
                "root/local horizontal mirrors must compose exactly once:\n{rendered}"
            );
        } else {
            assert_eq!(a.0, b.0, "local direction must stay vertical:\n{rendered}");
            assert_eq!(
                b.1 < a.1,
                target_before_source,
                "root/local vertical mirrors must compose exactly once:\n{rendered}"
            );
        }
        assert_rectangular_char_grid(&rendered);
    }
}

#[test]
fn flowchart_parser_subgraph_direction_override_with_cross_boundary_edges_records_boundary_aware_baseline()
 {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "subgraph one [LR Group]\n",
            "    direction LR\n",
            "    A --> B\n",
            "end\n",
            "X --> A\n",
            "B --> Y\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect(
        "cross-boundary mixed-direction subgraph should render through the boundary-aware seam",
    );

    for expected in ["LR Group", "A", "B", "X", "Y"] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?}:\n{rendered}"
        );
    }
    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(
        line_index("X") < line_index("A"),
        "root TD direction should keep the external source above its group entry:\n{rendered}"
    );
    assert_eq!(
        line_index("A"),
        line_index("B"),
        "the local LR override should keep A and B on the same row:\n{rendered}"
    );
    assert!(
        line_index("B") < line_index("Y"),
        "root TD direction should keep the external target below its group exit:\n{rendered}"
    );
    assert_rectangular_char_grid(&rendered);
}

#[test]
fn flowchart_parser_nested_subgraph_direction_override_keeps_child_group_as_a_movable_block() {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "subgraph outer\n",
            "    direction LR\n",
            "    A\n",
            "    subgraph inner\n",
            "        direction TD\n",
            "        B --> C\n",
            "    end\n",
            "    A --> B\n",
            "end\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("nested subgraph direction override should render as a movable child block");

    assert_eq!(
        normalize_ascii_art(&rendered),
        normalize_ascii_art(concat!(
            "+-------------------+\n",
            "|       outer       |\n",
            "|                   |\n",
            "|                   |\n",
            "|         +-------+ |\n",
            "|         | inner | |\n",
            "|         |       | |\n",
            "|         |       | |\n",
            "| +---+   | +---+ | |\n",
            "| |   |   | |   | | |\n",
            "| | A |---->| B | | |\n",
            "| |   |   | |   | | |\n",
            "| +---+   | +---+ | |\n",
            "|         |   |   | |\n",
            "|         |   |   | |\n",
            "|         |   |   | |\n",
            "|         |   |   | |\n",
            "|         |   v   | |\n",
            "|         | +---+ | |\n",
            "|         | |   | | |\n",
            "|         | | C | | |\n",
            "|         | |   | | |\n",
            "|         | +---+ | |\n",
            "|         |       | |\n",
            "|         +-------+ |\n",
            "|                   |\n",
            "+-------------------+\n",
        ))
    );
}
