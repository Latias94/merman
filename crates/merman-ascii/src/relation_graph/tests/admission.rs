use super::*;

fn aggregate_test_regions<'a>(
    wide_top: &'a RelationGraphBox,
    wide_bottom: &'a RelationGraphBox,
    narrow_top: &'a RelationGraphBox,
    narrow_bottom: &'a RelationGraphBox,
    first_called: &'a Cell<bool>,
    second_called: &'a Cell<bool>,
    resources: &ResourceContext,
) -> Result<Vec<RelationRegionPlan<'a>>> {
    let first = RelationStackPlan::try_new(
        wide_top,
        wide_bottom,
        &[],
        resources,
        |center, resources| centered_row_blocks_extent(center, [(1, 1)], resources),
    )?;
    let second = RelationStackPlan::try_new(
        narrow_top,
        narrow_bottom,
        &[],
        resources,
        |_center, resources| resources.grid_extent(0, 0),
    )?;
    Ok(vec![
        RelationRegionPlan::Vertical {
            plan: first,
            rows: Box::new(move |center, resources| {
                first_called.set(true);
                Ok(vec![centered_text_line_with_role(
                    "|",
                    center,
                    AsciiColorRole::EdgeLine,
                    TerminalWidthProfile::Unicode,
                    resources,
                )?])
            }),
        },
        RelationRegionPlan::Vertical {
            plan: second,
            rows: Box::new(move |_center, _resources| {
                second_called.set(true);
                Ok(Vec::new())
            }),
        },
    ])
}

#[test]
fn render_plan_admits_aggregate_extent_before_materializing_regions() -> Result<()> {
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(policy_with_grid_limit(30));
    let wide_top = RelationGraphBox::new("a".to_string(), vec!["aaaaa".to_string()], 5);
    let wide_bottom = RelationGraphBox::new("b".to_string(), vec!["bbbbb".to_string()], 5);
    let narrow_top = RelationGraphBox::new("c".to_string(), vec!["ccc".to_string()], 3);
    let narrow_bottom = RelationGraphBox::new("d".to_string(), vec!["ddd".to_string()], 3);
    let first_called = Cell::new(false);
    let second_called = Cell::new(false);
    let regions = aggregate_test_regions(
        &wide_top,
        &wide_bottom,
        &narrow_top,
        &narrow_bottom,
        &first_called,
        &second_called,
        &resources,
    )?;

    let plan = RelationRenderPlan::try_new(regions, &mut resources)?;
    assert_eq!(plan.extent(), resources.grid_extent(5, 6)?);
    assert!(!first_called.get());
    assert!(!second_called.get());
    let lines = plan.materialize(&options, &mut resources)?;
    assert_eq!(
        relation_lines_extent(&lines, &resources)?,
        resources.grid_extent(5, 6)?
    );
    assert!(first_called.get());
    assert!(second_called.get());
    Ok(())
}

#[test]
fn render_plan_rejects_aggregate_n_minus_one_before_materializing_regions() -> Result<()> {
    let mut resources = test_resources(policy_with_grid_limit(29));
    let wide_top = RelationGraphBox::new("a".to_string(), vec!["aaaaa".to_string()], 5);
    let wide_bottom = RelationGraphBox::new("b".to_string(), vec!["bbbbb".to_string()], 5);
    let narrow_top = RelationGraphBox::new("c".to_string(), vec!["ccc".to_string()], 3);
    let narrow_bottom = RelationGraphBox::new("d".to_string(), vec!["ddd".to_string()], 3);
    let first_called = Cell::new(false);
    let second_called = Cell::new(false);
    let regions = aggregate_test_regions(
        &wide_top,
        &wide_bottom,
        &narrow_top,
        &narrow_bottom,
        &first_called,
        &second_called,
        &resources,
    )?;

    let error = match RelationRenderPlan::try_new(regions, &mut resources) {
        Ok(_) => panic!("aggregate N-1 must fail before painting any region"),
        Err(error) => error,
    };
    assert_grid_limit(error, 30, 29);
    assert!(!first_called.get());
    assert!(!second_called.get());
    Ok(())
}

fn parallel_test_lane_extents(resources: &ResourceContext) -> Vec<LogicalExtent> {
    vec![
        resources
            .grid_extent(1, 2)
            .expect("first lane extent should fit"),
        resources
            .grid_extent(1, 2)
            .expect("second lane extent should fit"),
    ]
}

fn materialize_parallel_test_lanes(
    resources: &ResourceContext,
) -> Result<Vec<Vec<RelationGraphLine>>> {
    let width_profile = TerminalWidthProfile::Unicode;
    Ok(vec![
        vec![
            RelationGraphLine::try_plain("^", width_profile, resources)?,
            RelationGraphLine::try_plain("|", width_profile, resources)?,
        ],
        vec![
            RelationGraphLine::try_plain("^", width_profile, resources)?,
            RelationGraphLine::try_plain("|", width_profile, resources)?,
        ],
    ])
}

fn materialize_stack_test_rows(
    center: usize,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    Ok(vec![marker_line_with_role(
        '|',
        center,
        AsciiColorRole::EdgeLine,
        TerminalWidthProfile::Unicode,
        resources,
    )?])
}

fn self_loop_test_metrics() -> Vec<RelationSelfLoopMetrics> {
    vec![
        RelationSelfLoopMetrics::new(1, 1, 1, 1, None, '-', '|'),
        RelationSelfLoopMetrics::new(1, 2, 1, 1, Some(1), '-', '|'),
    ]
}

fn materialize_self_loop_test_rows(
    resources: &ResourceContext,
) -> Result<Vec<RelationSelfLoopRows>> {
    let width_profile = TerminalWidthProfile::Unicode;
    let marker = |text, resources: &ResourceContext| {
        RelationGraphLine::try_with_role(text, AsciiColorRole::EdgeArrow, width_profile, resources)
    };
    Ok(vec![
        RelationSelfLoopRows::new(
            marker("^", resources)?,
            vec![RelationGraphLine::try_plain("x", width_profile, resources)?],
            marker("v", resources)?,
            '-',
            '|',
        ),
        RelationSelfLoopRows::new(
            marker("^", resources)?,
            vec![RelationGraphLine::try_plain(
                "yy",
                width_profile,
                resources,
            )?],
            marker("v", resources)?,
            '-',
            '|',
        )
        .with_tail_prefix(marker(">", resources)?),
    ])
}

#[test]
fn self_loop_plan_admits_exact_extent_before_materializing() {
    let boxes = admission_test_boxes();
    let mut resources = test_resources(policy_with_grid_limit(42));
    let plan = RelationSelfLoopPlan::try_new(&boxes[0], self_loop_test_metrics(), &resources)
        .expect("self-loop descriptor should fit the exact aggregate limit");
    assert_eq!(
        plan.extent(),
        resources
            .grid_extent(7, 6)
            .expect("7 by 6 should fit the exact limit")
    );

    let materialized = Cell::new(false);
    let lines = plan
        .render_lines(&mut resources, |resources| {
            materialized.set(true);
            materialize_self_loop_test_rows(resources)
        })
        .expect("7 by 6 self-loop layout should fit 42 cells");

    assert!(materialized.get());
    assert_eq!(lines.len(), 6);
    assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(7));
}

#[test]
fn self_loop_plan_rejects_n_minus_one_before_materializing() {
    let boxes = admission_test_boxes();
    let mut resources = test_resources(policy_with_grid_limit(41));
    let materialized = Cell::new(false);

    let error = RelationSelfLoopPlan::try_new(&boxes[0], self_loop_test_metrics(), &resources)
        .and_then(|plan| {
            plan.render_lines(&mut resources, |resources| {
                materialized.set(true);
                materialize_self_loop_test_rows(resources)
            })
        })
        .expect_err("7 by 6 self-loop layout must not fit 41 cells");

    assert_grid_limit(error, 42, 41);
    assert!(!materialized.get());
}

#[test]
fn self_loop_plan_rejects_materialized_descriptor_mismatch() {
    let boxes = admission_test_boxes();
    let mut resources = test_resources(policy_with_grid_limit(42));
    let plan = RelationSelfLoopPlan::try_new(&boxes[0], self_loop_test_metrics(), &resources)
        .expect("self-loop descriptor should fit before row validation");
    let error = plan
        .render_lines(&mut resources, |resources| {
            let mut rows = materialize_self_loop_test_rows(resources)?;
            rows[0].label_lines[0] =
                RelationGraphLine::try_plain("xx", TerminalWidthProfile::Unicode, resources)?;
            Ok(rows)
        })
        .expect_err("materialized label width must match its admitted descriptor");

    assert_grid_limit(error, usize::MAX, 42);
}

#[test]
fn stack_plan_admits_exact_extent_before_materializing() {
    let boxes = admission_test_boxes();
    let mut resources = test_resources(policy_with_grid_limit(24));
    let plan = RelationStackPlan::try_new(
        &boxes[0],
        &boxes[1],
        &[],
        &resources,
        |center, resources| centered_row_blocks_extent(center, [(1, 1)], resources),
    )
    .expect("relation row descriptor should fit before aggregate admission");
    assert_eq!(
        plan.extent(),
        resources
            .grid_extent(4, 6)
            .expect("4 by 6 should fit the exact limit")
    );

    let materialized = Cell::new(false);
    let lines = plan
        .render_lines(&mut resources, |center, resources| {
            materialized.set(true);
            materialize_stack_test_rows(center, resources)
        })
        .expect("4 by 6 relation stack should fit 24 cells");

    assert!(materialized.get());
    assert_eq!(lines.len(), 6);
    assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(4));
}

#[test]
fn stack_plan_rejects_n_minus_one_before_materializing() {
    let boxes = admission_test_boxes();
    let mut resources = test_resources(policy_with_grid_limit(23));
    let materialized = Cell::new(false);

    let error = RelationStackPlan::try_new(
        &boxes[0],
        &boxes[1],
        &[],
        &resources,
        |center, resources| centered_row_blocks_extent(center, [(1, 1)], resources),
    )
    .and_then(|plan| {
        plan.render_lines(&mut resources, |center, resources| {
            materialized.set(true);
            materialize_stack_test_rows(center, resources)
        })
    })
    .expect_err("4 by 6 relation stack must not fit 23 cells");

    assert_grid_limit(error, 24, 23);
    assert!(!materialized.get());
}

#[test]
fn parallel_plan_admits_odd_endpoint_extent_at_exact_limit_before_materializing() {
    let top = RelationGraphBox::new("top".to_string(), vec!["abcde".to_string()], 5);
    let bottom = RelationGraphBox::new("bottom".to_string(), vec!["vwxyz".to_string()], 5);
    let mut default_resources = test_resources(AsciiResourcePolicy::default());
    let plan = RelationParallelPlan::new(
        &top,
        &bottom,
        parallel_test_lane_extents(&default_resources),
        2,
        &mut default_resources,
    )
    .expect("parallel geometry should plan from lane extents");
    assert!(
        plan.ports_fit(&default_resources)
            .expect("wide endpoints should accept both ports")
    );
    let planned = plan.extent();
    assert_eq!(
        (planned.width(), planned.height(), planned.cells()),
        (5, 4, 20)
    );

    let mut resources = test_resources(policy_with_grid_limit(planned.cells()));
    let plan = RelationParallelPlan::new(
        &top,
        &bottom,
        parallel_test_lane_extents(&resources),
        2,
        &mut resources,
    )
    .expect("exact-limit parallel geometry should plan");
    assert!(
        plan.ports_fit(&resources)
            .expect("wide endpoints should accept both ports")
    );
    let materialized = Cell::new(false);
    let lines = plan
        .render_lines(&mut resources, |resources| {
            materialized.set(true);
            materialize_parallel_test_lanes(resources)
        })
        .expect("5 by 4 parallel document should fit 20 cells");

    assert!(materialized.get());
    assert_eq!(lines.len(), 4);
    assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(5));
}

#[test]
fn parallel_plan_rejects_odd_endpoint_extent_at_n_minus_one_before_materializing() {
    let top = RelationGraphBox::new("top".to_string(), vec!["abcde".to_string()], 5);
    let bottom = RelationGraphBox::new("bottom".to_string(), vec!["vwxyz".to_string()], 5);
    let mut resources = test_resources(policy_with_grid_limit(19));
    let materialized = Cell::new(false);
    let error = RelationParallelPlan::new(
        &top,
        &bottom,
        parallel_test_lane_extents(&resources),
        2,
        &mut resources,
    )
    .and_then(|plan| {
        plan.render_lines(&mut resources, |resources| {
            materialized.set(true);
            materialize_parallel_test_lanes(resources)
        })
    })
    .expect_err("5 by 4 parallel document must not fit 19 cells");

    assert_grid_limit(error, 20, 19);
    assert!(!materialized.get());
}

#[test]
fn stack_and_horizontal_strip_admit_exact_grid_extent() {
    let boxes = admission_test_boxes();
    let options = AsciiRenderOptions::ascii();

    let mut stack_resources = test_resources(policy_with_grid_limit(24));
    let stack = stacked_box_lines_ordered(
        &boxes,
        options.terminal_width_profile,
        true,
        &mut stack_resources,
    )
    .expect("4 by 6 reversed stack should fit 24 cells");
    assert_eq!(stack.len(), 6);
    assert_eq!(stack[0].width(), 4);
    assert_eq!(stack[4].width(), 3);

    let horizontal_resources = test_resources(policy_with_grid_limit(27));
    let strip = render_horizontal_box_strip_lines(
        &boxes,
        RelationGraphHorizontalDirection::LeftRight,
        2,
        options.terminal_width_profile,
        &horizontal_resources,
    )
    .expect("9 by 3 horizontal strip should fit 27 cells");
    assert_eq!(strip.len(), 3);
    assert!(strip.iter().all(|line| line.width() == 9));
}

#[test]
fn stack_and_horizontal_strip_reject_grid_extent_at_n_minus_one() {
    let boxes = admission_test_boxes();
    let options = AsciiRenderOptions::ascii();

    let mut stack_resources = test_resources(policy_with_grid_limit(23));
    let error = stacked_box_lines_ordered(
        &boxes,
        options.terminal_width_profile,
        true,
        &mut stack_resources,
    )
    .expect_err("4 by 6 reversed stack must not fit 23 cells");
    assert_grid_limit(error, 24, 23);

    let horizontal_resources = test_resources(policy_with_grid_limit(26));
    let error = render_horizontal_box_strip_lines(
        &boxes,
        RelationGraphHorizontalDirection::LeftRight,
        2,
        options.terminal_width_profile,
        &horizontal_resources,
    )
    .expect_err("9 by 3 horizontal strip must not fit 26 cells");
    assert_grid_limit(error, 27, 26);
}

#[test]
fn relation_document_admits_exact_extent_before_materializing() {
    let boxes = admission_test_boxes();
    let default_options = AsciiRenderOptions::ascii();
    let default_resources = test_resources(AsciiResourcePolicy::default());
    let mut deferred = DeferredTextRegistry::new();
    let rows = vec![
        test_summary_row(
            "A",
            "-->",
            "B",
            None,
            default_options.terminal_width_profile,
            &mut deferred,
            &default_resources,
        )
        .expect("summary row should plan"),
    ];
    let base_extent = stacked_box_extent(&boxes, &default_resources)
        .expect("base stack should have a checked extent");
    let summary_extent = relation_summary_extent(&rows, None, &default_options, &default_resources)
        .expect("summary should have a checked extent");
    let planned =
        RelationDocumentPlan::new(base_extent, Some(summary_extent), 10, &default_resources)
            .expect("aggregate document should have a checked extent")
            .extent();
    assert_eq!(
        (planned.width(), planned.height(), planned.cells()),
        (10, 9, 90)
    );

    let exact = planned.cells();
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(policy_with_grid_limit(exact));
    let base_extent =
        stacked_box_extent(&boxes, &resources).expect("base stack should fit the aggregate limit");
    let materialized = Cell::new(false);

    let lines = render_relation_document_with_summary(
        base_extent,
        &rows,
        None,
        &options,
        &mut resources,
        |resources| {
            materialized.set(true);
            stacked_box_lines_ordered(&boxes, options.terminal_width_profile, true, resources)
        },
    )
    .expect("10 by 9 aggregate document should fit 90 cells");

    assert!(materialized.get());
    assert_eq!(lines.len(), 9);
    assert_eq!(lines.iter().map(RelationGraphLine::width).max(), Some(10));
}

#[test]
fn relation_document_rejects_n_minus_one_before_materializing() {
    let boxes = admission_test_boxes();
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(policy_with_grid_limit(89));
    let mut deferred = DeferredTextRegistry::new();
    let rows = vec![
        test_summary_row(
            "A",
            "-->",
            "B",
            None,
            options.terminal_width_profile,
            &mut deferred,
            &resources,
        )
        .expect("summary row should plan"),
    ];
    let base_extent = stacked_box_extent(&boxes, &resources)
        .expect("base stack should fit before aggregate admission");
    let materialized = Cell::new(false);

    let error = render_relation_document_with_summary(
        base_extent,
        &rows,
        None,
        &options,
        &mut resources,
        |resources| {
            materialized.set(true);
            stacked_box_lines_ordered(&boxes, options.terminal_width_profile, true, resources)
        },
    )
    .expect_err("10 by 9 aggregate document must not fit 89 cells");

    assert_grid_limit(error, 90, 89);
    assert!(!materialized.get());
}
