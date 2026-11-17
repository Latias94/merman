use super::*;
use crate::AsciiRenderOptions;
use crate::graph::model::{GraphDirection, GraphEdgeMarker, GraphNodeShape};
use crate::resource::AsciiResourcePolicy;

#[test]
fn compound_return_preparation_keeps_an_already_routable_viewport() {
    use crate::graph::layout::graph_canvas_extent;
    use crate::{AsciiViewportPolicy, OverflowPolicy};

    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A wide rectangular source node");
    graph.add_node("b", "A wide rectangular target node");
    graph.add_edge("a", "b");
    graph.add_edge("b", "a");
    graph.add_group_with_style(
        "scope",
        "Scope",
        Some(GraphDirection::TopDown),
        vec!["a".into(), "b".into()],
        Default::default(),
    );
    let options = AsciiRenderOptions::ascii();
    let policy = options.flowchart_layout().graph_policy();
    let charset = GraphCharset::for_options(&options);
    let limits = AsciiResourcePolicy::unbounded();
    let execution = AsciiExecution::for_test(&limits);
    let mut resources = ResourceContext::new(limits);
    let initial = layout_graph_with_feedback(
        &graph,
        &policy,
        &LayoutFeedback::default(),
        &mut resources,
        execution,
    )
    .unwrap();
    let routes = routing::prepare_route_scene_with_execution(
        &graph,
        &initial.layout,
        &graph.edges,
        &charset,
        &policy,
        &mut resources,
        execution,
    )
    .unwrap();
    let (edge_width, edge_height) = routes.canvas_extent();
    let extent = graph_canvas_extent(
        &initial.layout.nodes,
        &initial.layout.groups,
        edge_width,
        edge_height,
        &resources,
    )
    .unwrap();
    let rendered = crate::graph::draw::render_graph_with_execution(
        &graph,
        &options,
        execution.with_viewport(
            AsciiViewportPolicy::with_max_width(extent.width()).overflow(OverflowPolicy::Error),
        ),
    )
    .unwrap();
    assert_eq!(
        rendered.matches("A wide rectangular source node").count(),
        1
    );
    assert_eq!(
        rendered.matches("A wide rectangular target node").count(),
        1
    );
}

#[test]
fn existing_exterior_envelopes_keep_self_loops_within_their_original_viewport() {
    use crate::graph::model::GraphEdgeAttrs;
    use crate::{AsciiViewportPolicy, OverflowPolicy};

    for direction in [
        GraphDirection::TopDown,
        GraphDirection::BottomTop,
        GraphDirection::LeftRight,
        GraphDirection::RightLeft,
    ] {
        for grouped in [false, true] {
            let mut graph = AsciiGraph::new(direction);
            graph.add_node("a", "Node");
            graph.add_edge_with_attrs(
                "a",
                "a",
                GraphEdgeAttrs {
                    label: Some("1234567890123456789012345678901234567890".into()),
                    ..Default::default()
                },
            );
            if grouped {
                graph.add_group_with_style(
                    "scope",
                    "Scope",
                    Some(direction),
                    vec!["a".into()],
                    Default::default(),
                );
            }
            let options = AsciiRenderOptions::ascii();
            let policy = options.flowchart_layout().graph_policy();
            let charset = GraphCharset::for_options(&options);
            let limits = AsciiResourcePolicy::default();
            let execution = AsciiExecution::for_test(&limits);
            let mut resources = ResourceContext::new(limits);
            let initial = layout_graph_with_feedback(
                &graph,
                &policy,
                &LayoutFeedback::default(),
                &mut resources,
                execution,
            )
            .unwrap();
            routing::prepare_route_scene_with_execution(
                &graph,
                &initial.layout,
                &graph.edges,
                &charset,
                &policy,
                &mut resources,
                execution,
            )
            .expect("the existing self-loop envelope must already be routable");
            let planned = plan_graph(&graph, &policy, &charset, &mut resources, execution).unwrap();
            assert_eq!(planned.layout, initial.layout, "{direction:?}/{grouped}");
            let unbounded =
                crate::graph::draw::render_graph_with_execution(&graph, &options, execution)
                    .unwrap();
            let original_width = unbounded.lines().map(str::len).max().unwrap();
            if direction == GraphDirection::TopDown && !grouped {
                assert!(
                    original_width <= 64,
                    "the reported 64-column regression must stay admitted"
                );
            }
            let rendered = crate::graph::draw::render_graph_with_execution(
                &graph,
                &options,
                execution.with_viewport(
                    AsciiViewportPolicy::with_max_width(original_width)
                        .overflow(OverflowPolicy::Error),
                ),
            )
            .unwrap();
            assert_eq!(rendered, unbounded);
            assert_eq!(
                rendered
                    .matches("1234567890123456789012345678901234567890")
                    .count(),
                1
            );
            assert!(
                rendered.lines().all(|row| row.len() <= original_width),
                "{rendered}"
            );
        }
    }
}

#[test]
fn exhausted_initial_capacity_analysis_keeps_the_unmodified_feasible_layout() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    graph.add_group_with_style(
        "scope",
        "Scope",
        Some(GraphDirection::TopDown),
        vec!["a".into(), "b".into()],
        Default::default(),
    );
    let options = AsciiRenderOptions::ascii();
    let policy = options.flowchart_layout().graph_policy();
    let charset = GraphCharset::for_options(&options);
    let limits = AsciiResourcePolicy::default();
    let mut resources = ResourceContext::new(limits);
    let budget = GraphPlanningBudget::default();
    budget.reflow_work.set(1);
    let execution = AsciiExecution::for_test(&limits).with_graph_planning(&budget);
    let input = LayoutInput::try_new(&graph, &policy, &mut resources, execution).unwrap();
    let initial = input
        .generate(&LayoutFeedback::default(), &mut resources, execution)
        .unwrap();
    let before = resources.layout_work_used();
    let InitialGraph { measured, .. } =
        initial_layout(&graph, &input, &charset, &policy, &mut resources, execution).unwrap();
    assert_eq!(measured.layout, initial.layout);
    assert_eq!(budget.reflow_work.get(), 0);
    assert!(
        resources.layout_work_used() > before,
        "discarded analysis still consumes work"
    );
    let routes = routing::prepare_route_scene_with_execution(
        &graph,
        &measured.layout,
        &graph.edges,
        &charset,
        &policy,
        &mut resources,
        execution,
    );
    assert!(
        routes.is_ok(),
        "the unchanged directly routable geometry remains available"
    );
}

#[test]
fn initial_capacity_preparation_stays_within_its_allowance_and_preserves_hard_errors() {
    use crate::graph::model::GraphEdgeAttrs;
    use crate::resource::AsciiResourceLimitId;

    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    graph.add_edge_with_attrs(
        "b",
        "a",
        GraphEdgeAttrs {
            label: Some("a complete return condition".into()),
            ..Default::default()
        },
    );
    graph.add_group_with_style(
        "scope",
        "Scope",
        Some(GraphDirection::TopDown),
        vec!["a".into(), "b".into()],
        Default::default(),
    );
    let policy = AsciiRenderOptions::ascii()
        .flowchart_layout()
        .graph_policy();
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let limits = AsciiResourcePolicy::unbounded();
    let mut preparation = ResourceContext::new(limits);
    let execution = AsciiExecution::for_test(&limits);
    let input = LayoutInput::try_new(&graph, &policy, &mut preparation, execution).unwrap();
    let mut original_resources = ResourceContext::new(limits);
    let original = input
        .generate(
            &LayoutFeedback::default(),
            &mut original_resources,
            execution,
        )
        .unwrap();
    let initial_work = original_resources.layout_work_used();
    let budget = GraphPlanningBudget::default();
    let mut resources = ResourceContext::new(limits);
    let prepared = initial_layout(
        &graph,
        &input,
        &charset,
        &policy,
        &mut resources,
        execution.with_graph_planning(&budget),
    )
    .unwrap();
    let required = GraphPlanningBudget::default().reflow_work.get() - budget.reflow_work.get();
    assert_eq!(prepared.measured.layout, original.layout);
    assert!(
        prepared.incumbent.is_some(),
        "a complete original scene must be retained"
    );
    for allowance in [1, required / 2, required] {
        let budget = GraphPlanningBudget::default();
        budget.reflow_work.set(allowance);
        let mut resources = ResourceContext::new(limits);
        let candidate = initial_layout(
            &graph,
            &input,
            &charset,
            &policy,
            &mut resources,
            execution.with_graph_planning(&budget),
        )
        .unwrap();
        let spent = allowance - budget.reflow_work.get();
        assert!(spent <= allowance);
        assert!(resources.layout_work_used() >= initial_work + spent);
        if allowance == 1 {
            assert_eq!(candidate.measured.layout, original.layout);
            assert_eq!(budget.reflow_work.get(), 0);
            assert!(
                resources.layout_work_used() > initial_work,
                "discarded rebuild work must remain charged"
            );
        }
    }
    let hard_limits = limits
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, initial_work)
        .unwrap();
    let mut resources = ResourceContext::new(hard_limits);
    let budget = GraphPlanningBudget::default();
    budget.reflow_work.set(1);
    assert!(
        matches!(
            initial_layout(
                &graph,
                &input,
                &charset,
                &policy,
                &mut resources,
                AsciiExecution::for_test(&hard_limits).with_graph_planning(&budget)
            ),
            Err(AsciiError::ResourceLimitExceeded(_))
        ),
        "public work exhaustion must not be swallowed by optional preflight"
    );
}

#[test]
#[cfg(feature = "diagram-flowchart")]
fn large_compound_chains_keep_the_existing_strict_benchmark_admission() {
    use std::fmt::Write as _;
    let mut source = String::from("flowchart LR\n");
    for segment in 0..10 {
        writeln!(source, "subgraph S{}[Segment {}]", segment + 1, segment + 1).unwrap();
        for offset in 1..=20 {
            let node = segment * 20 + offset;
            writeln!(source, "L{node}[Node {node}]").unwrap();
            if offset > 1 {
                writeln!(source, "L{} --> L{node}", node - 1).unwrap();
            }
        }
        source.push_str("end\n");
    }
    for segment in 0..9 {
        writeln!(
            source,
            "L{} --> L{}",
            segment * 20 + 1,
            (segment + 1) * 20 + 1
        )
        .unwrap();
    }
    let parsed = merman_core::SemanticParser::new()
        .parse_diagram_sync(&source)
        .unwrap();
    let context = merman_model::runtime::RuntimePolicy::deterministic()
        .begin_operation()
        .unwrap();
    let output = crate::AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_parsed_report(
            &parsed,
            crate::AsciiViewportPolicy::unrestricted(),
            &merman_model::OperationControl::new(),
            &context,
            AsciiResourcePolicy::unbounded(),
        )
        .expect("finite search allowances must retain directly routable strict chains");
    assert_eq!(output.projection, crate::AsciiProjection::Diagrammatic);
    for node in 1..=200 {
        assert!(output.text.contains(&format!("Node {node}")));
    }
}

#[test]
fn capacity_feedback_finishes_a_strict_diagram_with_every_independent_marker() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node_with_shape_and_style(
        "source",
        "",
        GraphNodeShape::ForkJoinHorizontal,
        Default::default(),
    );
    graph.add_node("target", "Target");
    for _ in 0..97 {
        graph.add_edge("source", "target");
        let edge = graph.edges.last_mut().unwrap();
        edge.start_marker = GraphEdgeMarker::Circle;
        edge.end_marker = GraphEdgeMarker::Open;
    }
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let policy = options.flowchart_layout().graph_policy();
    let resources_policy = AsciiResourcePolicy::default();
    let mut resources = ResourceContext::new(resources_policy);
    let execution = AsciiExecution::for_test(&resources_policy);
    let initial = layout_graph_with_feedback(
        &graph,
        &policy,
        &LayoutFeedback::default(),
        &mut resources,
        execution,
    )
    .unwrap();
    assert_eq!(initial.layout.nodes[0].width, 7);
    let operation_budget = GraphPlanningBudget::default();
    let execution = execution.with_graph_planning(&operation_budget);
    let input = routing::SceneRoutingInput::try_new(&graph, &graph.edges, &charset, &mut resources)
        .unwrap();
    let work_before = resources.layout_work_used();
    let credit_before = operation_budget.feasibility.remaining().1;
    assert!(matches!(
        routing::route_scene_with_input(
            &graph,
            &initial.layout,
            &input,
            &charset,
            &policy,
            &mut resources,
            execution,
        ),
        Err(AsciiError::PortCapacityExceeded {
            required: 97,
            available: 96,
            ..
        })
    ));
    assert_eq!(
        credit_before - operation_budget.feasibility.remaining().1,
        resources.layout_work_used() - work_before,
        "a capacity miss must settle all work before another geometry generation"
    );
    let planned = plan_graph(&graph, &policy, &charset, &mut resources, execution)
        .expect("measured capacity drives a complete fresh strict scene");
    assert_eq!(
        planned.layout.nodes[0].shape,
        GraphNodeShape::ForkJoinHorizontal
    );
    assert_eq!(planned.layout.nodes[0].width, 293);
    assert!(planned.layout.nodes[1].width >= 293);
    let diagram = crate::graph::draw::render_graph(&graph, &options, resources_policy)
        .expect("the accepted strict scene must materialize without a text projection");
    assert_eq!(
        diagram
            .chars()
            .filter(|character| *character == 'o')
            .count(),
        97
    );
    assert_eq!(diagram.matches("Target").count(), 1);
}

#[test]
fn strict_reflow_readback_recovers_every_private_marked_relationship() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node_with_shape_and_style(
        "source",
        "",
        GraphNodeShape::ForkJoinHorizontal,
        Default::default(),
    );
    graph.add_node("target", "Target");
    for _ in 0..97 {
        graph.add_edge("source", "target");
        let edge = graph.edges.last_mut().unwrap();
        edge.start_marker = GraphEdgeMarker::Circle;
        edge.end_marker = GraphEdgeMarker::Open;
    }
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(crate::AsciiColorMode::Plain)
        .with_graph_presentation(crate::AsciiGraphPresentation::Strict);
    assert_eq!(GraphCharset::for_options(&options).thick_horizontal, '=');
    let diagram =
        crate::graph::draw::render_graph(&graph, &options, AsciiResourcePolicy::default())
            .expect("strict reflow must paint all independent relationships");
    assert_strict_reflow_ascii_readback(&diagram, 97);
}

fn strict_reflow_ascii_wire_connects(glyph: u8, delta_x: isize, delta_y: isize) -> bool {
    match glyph {
        b'-' => delta_y == 0,
        b'|' => delta_x == 0,
        b'+' | b'o' => true,
        _ => false,
    }
}

fn assert_strict_reflow_ascii_readback(diagram: &str, expected_relationships: usize) {
    const MAX_READBACK_CELLS: usize = 64 * 1024;
    const STEPS: [(isize, isize); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

    assert!(diagram.is_ascii(), "readback requires Plain ASCII output");
    assert!(
        diagram.len() <= MAX_READBACK_CELLS,
        "this two-node readback must stay bounded"
    );
    let mut grid: Vec<Vec<u8>> = diagram
        .lines()
        .map(|line| line.as_bytes().to_vec())
        .collect();
    let width = grid.iter().map(Vec::len).max().expect("a painted grid");
    let cell_count = width.checked_mul(grid.len()).expect("bounded grid area");
    assert!(width > 0 && cell_count <= MAX_READBACK_CELLS);
    for row in &mut grid {
        row.resize(width, b' ');
    }

    let source_rows: Vec<_> = grid
        .iter()
        .enumerate()
        .filter_map(|(row, cells)| cells.contains(&b'=').then_some(row))
        .collect();
    assert_eq!(source_rows.len(), 1, "one horizontal fork/join bar");
    let source_row = source_rows[0];
    let source_left = grid[source_row]
        .iter()
        .position(|cell| *cell == b'=')
        .unwrap();
    let source_right = grid[source_row]
        .iter()
        .rposition(|cell| *cell == b'=')
        .unwrap();
    assert!(source_left < source_right);
    assert!(
        grid[source_row][source_left..=source_right]
            .iter()
            .all(|cell| *cell == b'='),
        "the source must retain a continuous thick horizontal contour"
    );
    // The horizontal fork/join has its '=' stroke on the middle row and '-' contacts
    // on the following row. Require each contact to be backed immediately by '='.
    let source_contact_row = source_row + 1;

    let label = b"Target";
    let mut labels = Vec::new();
    for (row, cells) in grid.iter().enumerate() {
        for (column, text) in cells.windows(label.len()).enumerate() {
            if text == label {
                labels.push((column, row));
            }
        }
    }
    assert_eq!(labels.len(), 1, "one identifiable target");
    let (label_column, label_row) = labels[0];
    let target_left = grid[label_row][..label_column]
        .iter()
        .rposition(|cell| *cell == b'|')
        .expect("a rectangle to the left of Target");
    let after_label = label_column + label.len();
    let target_right = after_label
        + grid[label_row][after_label..]
            .iter()
            .position(|cell| *cell == b'|')
            .expect("a rectangle to the right of Target");
    let target_top = (0..label_row)
        .rev()
        .find(|row| grid[*row][target_left] == b'+' && grid[*row][target_right] == b'+')
        .expect("the target's upper corners");
    let target_bottom = (label_row + 1..grid.len())
        .find(|row| grid[*row][target_left] == b'+' && grid[*row][target_right] == b'+')
        .expect("the target's lower corners");
    assert!(
        source_contact_row + 1 < target_top,
        "TopDown relationships must leave the source above Target"
    );
    for row in [target_top, target_bottom] {
        assert!(
            grid[row][target_left + 1..target_right]
                .iter()
                .all(|cell| *cell == b'-'),
            "Target must have an intact horizontal rectangle contour"
        );
    }
    for (row, cells) in grid
        .iter()
        .enumerate()
        .take(target_bottom)
        .skip(target_top + 1)
    {
        assert_eq!(cells[target_left], b'|');
        assert_eq!(cells[target_right], b'|');
        for (column, cell) in cells
            .iter()
            .enumerate()
            .take(target_right)
            .skip(target_left + 1)
        {
            let expected = if row == label_row && (label_column..after_label).contains(&column) {
                label[column - label_column]
            } else {
                b' '
            };
            assert_eq!(
                *cell, expected,
                "only Target's label may occupy its interior"
            );
        }
    }
    let source_contacts: Vec<_> = (source_left..=source_right)
        .filter(|column| {
            let glyph = grid[source_contact_row][*column];
            assert!(
                matches!(glyph, b' ' | b'-'),
                "only separate normal-stroke contacts may sit below the source bar"
            );
            glyph == b'-'
        })
        .collect();
    assert_eq!(source_contacts.len(), expected_relationships);
    for column in &source_contacts {
        assert!(source_left < *column && *column < source_right);
        assert_eq!(grid[source_row][*column], b'=');
        assert_eq!(
            grid[source_contact_row + 1][*column],
            b'o',
            "each source contact must have its own outward circle marker"
        );
    }

    // Cut out both nodes before traversal: their shared contours cannot be used to
    // connect otherwise separate wires. No production plan or validator is consulted.
    let in_source = |column, row| {
        (source_left..=source_right).contains(&column)
            && (source_row..=source_contact_row).contains(&row)
    };
    let in_target = |column, row| {
        (target_left..=target_right).contains(&column)
            && (target_top..=target_bottom).contains(&row)
    };
    let mut wire = vec![false; cell_count];
    let mut markers = Vec::new();
    for (row, cells) in grid.iter().enumerate() {
        for (column, glyph) in cells.iter().copied().enumerate() {
            if in_source(column, row) || in_target(column, row) || glyph == b' ' {
                continue;
            }
            assert!(
                matches!(glyph, b'-' | b'|' | b'+' | b'o'),
                "unexpected relationship glyph at ({column}, {row}): {glyph:?}"
            );
            wire[row * width + column] = true;
            if glyph == b'o' {
                markers.push((column, row));
            }
        }
    }
    assert_eq!(markers.len(), expected_relationships);

    let mut visited = vec![false; cell_count];
    let mut used_sources = vec![false; width];
    let mut used_targets = vec![false; width];
    let mut connections = Vec::new();
    for marker in markers {
        let marker_index = marker.1 * width + marker.0;
        assert!(
            !visited[marker_index],
            "markers must have private wire components"
        );
        visited[marker_index] = true;
        let mut pending = vec![marker];
        let mut source_contact = None;
        let mut target_contact = None;
        let mut component_markers = 0;
        while let Some((column, row)) = pending.pop() {
            let glyph = grid[row][column];
            component_markers += usize::from(glyph == b'o');
            let mut degree = 0;
            for (delta_x, delta_y) in STEPS {
                if !strict_reflow_ascii_wire_connects(glyph, delta_x, delta_y) {
                    continue;
                }
                let (Some(next_column), Some(next_row)) = (
                    column.checked_add_signed(delta_x),
                    row.checked_add_signed(delta_y),
                ) else {
                    continue;
                };
                if next_column >= width || next_row >= grid.len() {
                    continue;
                }
                let next_glyph = grid[next_row][next_column];
                let next_index = next_row * width + next_column;
                if wire[next_index] {
                    if !strict_reflow_ascii_wire_connects(next_glyph, -delta_x, -delta_y) {
                        continue;
                    }
                    degree += 1;
                    if !visited[next_index] {
                        visited[next_index] = true;
                        pending.push((next_column, next_row));
                    }
                } else if in_source(next_column, next_row) && next_glyph != b' ' {
                    assert!(
                        delta_x == 0
                            && delta_y == -1
                            && next_row == source_contact_row
                            && source_left < next_column
                            && next_column < source_right
                            && next_glyph == b'-',
                        "source contacts must leave the lower contour normally"
                    );
                    assert_eq!(
                        (column, row),
                        marker,
                        "the circle must be at the source end of its wire"
                    );
                    assert!(
                        source_contact.replace(next_column).is_none(),
                        "a private wire must touch the source exactly once"
                    );
                    degree += 1;
                } else if in_target(next_column, next_row) && next_glyph != b' ' {
                    assert!(
                        delta_x == 0
                            && delta_y == 1
                            && next_row == target_top
                            && target_left < next_column
                            && next_column < target_right
                            && next_glyph == b'-',
                        "target contacts must enter the upper rectangle contour normally"
                    );
                    assert!(
                        target_contact.replace(next_column).is_none(),
                        "a private wire must touch Target exactly once"
                    );
                    degree += 1;
                }
            }
            assert_eq!(
                degree, 2,
                "each wire cell must have two reciprocal continuations or normal contacts; \
                 found a gap or branch at ({column}, {row})"
            );
        }
        assert_eq!(
            component_markers, 1,
            "a wire may not contain another circle"
        );
        let source_contact = source_contact.expect("the circle's wire must reach the source bar");
        let target_contact = target_contact.expect("the circle's wire must reach Target");
        assert!(
            !used_sources[source_contact],
            "source contacts must be private"
        );
        assert!(
            !used_targets[target_contact],
            "target contacts must be private"
        );
        used_sources[source_contact] = true;
        used_targets[target_contact] = true;
        connections.push((marker, source_contact, target_contact));
    }
    assert_eq!(connections.len(), expected_relationships);
    assert_eq!(
        used_sources.iter().filter(|used| **used).count(),
        source_contacts.len(),
        "every painted source contact must own one relationship"
    );
    assert!(
        wire.iter().zip(&visited).all(|(cell, seen)| !cell || *seen),
        "every painted wire cell must belong to a recovered marked relationship"
    );
}
