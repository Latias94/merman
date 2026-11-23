use super::*;

fn node(x0: f64, y0: f64, x1: f64, y1: f64) -> Body {
    Body::node(Rect::new(x0, y0, x1, y1))
}

fn point(x: f64, y: f64) -> Body {
    node(x, y, x, y)
}

fn label(owner: LabelOwner, x0: f64, y0: f64, x1: f64, y1: f64) -> Label {
    Label {
        owner,
        bounds: Rect::new(x0, y0, x1, y1),
    }
}

fn edge(from: usize, to: usize, d: &str) -> Edge<'_> {
    Edge::new(from, to, d, CrossAxis::X)
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-7,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn fork_join_midpoint_and_return_are_measured_on_the_requested_cross_axis() {
    let mut scene = Scene {
        bodies: vec![
            node(45., 0., 55., 10.),
            node(15., 30., 25., 40.),
            node(75., 30., 85., 40.),
            node(45., 60., 55., 70.),
        ],
        edges: vec![
            edge(0, 1, "M50 10L20 30"),
            edge(0, 2, "M50 10L80 30"),
            edge(1, 3, "M20 40L50 60"),
            edge(2, 3, "M80 40L50 60"),
        ],
        ..Scene::default()
    };
    let report = validate(&scene);
    assert!(report.hard.is_empty(), "{report:?}");
    assert!(report.crossings.is_empty());
    assert!(
        report
            .edges
            .iter()
            .all(|m| m.unwrap().sideways_return == 0.0)
    );
    assert_eq!(
        fork_join_offsets(&scene, 0, 3, &[1, 2], CrossAxis::X),
        Some((0., 0.))
    );
    assert_eq!(fork_join_offsets(&scene, 0, 3, &[1, 1], CrossAxis::X), None);
    assert_eq!(fork_join_offsets(&scene, 0, 3, &[1, 4], CrossAxis::X), None);
    // A third private branch near the left edge changes the average, but not the outer midpoint.
    scene.bodies.push(point(25., 35.));
    scene
        .edges
        .extend([edge(0, 4, "M50 10L25 35"), edge(4, 3, "M25 35L50 60")]);
    scene.bodies[0].bounds = scene.bodies[0].bounds + Vec2::new(9.625, 0.);
    scene.bodies[3].bounds = scene.bodies[3].bounds + Vec2::new(110.625, 0.);
    assert_eq!(
        fork_join_offsets(&scene, 0, 3, &[1, 4, 2], CrossAxis::X),
        Some((9.625, 110.625))
    );
    for body in &mut scene.bodies {
        let r = body.bounds;
        body.bounds = Rect::new(r.y0, r.x0, r.y1, r.x1);
    }
    assert_eq!(
        fork_join_offsets(&scene, 0, 3, &[1, 4, 2], CrossAxis::Y),
        Some((9.625, 110.625))
    );
}

#[test]
fn curved_shaft_enters_foreign_boxes_while_the_chord_and_boundary_touch_clear() {
    let mut scene = Scene {
        bodies: vec![point(0., 10.), point(40., 10.), node(12., 22., 16., 24.)],
        edges: vec![edge(0, 1, "m0 10q20 30 40 0"), edge(0, 1, "M0 10H40")],
        labels: vec![
            label(LabelOwner::Edge(0), 4., 17., 8., 20.),
            label(LabelOwner::Edge(1), 24., 23., 27., 25.),
            label(LabelOwner::Edge(1), 18., 25., 22., 26.),
            label(LabelOwner::Body(0), 0., 10., 1., 12.),
        ],
    };
    // The quadratic's maximum is y=25 and its endpoints lie on y=10.
    let report = validate(&scene);
    assert_eq!(
        report.hard,
        [
            Violation::ForeignNodeHit { edge: 0, node: 2 },
            Violation::ForeignLabelHit { edge: 0, label: 1 }
        ]
    );
    assert!(report.label_overlaps.is_empty());
    assert!(report.label_node_overlaps.is_empty());
    assert!(report.crossings.is_empty());
    scene.edges[0].d = "M0 10H40";
    let report = validate(&scene);
    assert!(report.hard.is_empty());
    assert_eq!((report.contacts_checked, report.markers_checked), (4, 0));
}

#[test]
fn edge_label_box_overlaps_are_hard_even_when_every_shaft_is_clear() {
    let mut scene = Scene {
        bodies: vec![
            node(0., 5., 4., 15.),
            node(46., 5., 50., 15.),
            node(13., 24., 19., 30.),
            Body::group(Rect::new(21., 20., 42., 45.)),
        ],
        edges: vec![edge(0, 1, "M4 10H46"), edge(0, 1, "M4 12H46")],
        labels: vec![
            label(LabelOwner::Edge(0), 19., 26., 21., 28.),
            label(LabelOwner::Body(3), 24., 22., 30., 26.),
            label(LabelOwner::Edge(1), 26., 30., 34., 34.),
            label(LabelOwner::Body(0), 0.5, 5.5, 3.5, 6.5),
        ],
    };
    // Edge label 0 touches node 2's boundary only. The group title and node label are inside their owners.
    assert!(validate(&scene).hard.is_empty());
    for (bounds, violation) in [
        (
            Rect::new(14., 25., 18., 28.),
            Violation::LabelNodeOverlap { label: 0, node: 2 },
        ),
        (
            Rect::new(25., 22., 29., 25.),
            Violation::LabelOverlap {
                first: 0,
                second: 1,
            },
        ),
        (
            Rect::new(26., 30., 29., 33.),
            Violation::LabelOverlap {
                first: 0,
                second: 2,
            },
        ),
        (
            Rect::new(0.5, 12., 3., 14.),
            Violation::LabelNodeOverlap { label: 0, node: 0 },
        ),
    ] {
        scene.labels[0].bounds = bounds;
        let report = validate(&scene);
        match violation {
            Violation::LabelNodeOverlap { label, node } => {
                assert_eq!(report.label_node_overlaps, [(label, node)])
            }
            Violation::LabelOverlap { first, second } => {
                assert_eq!(report.label_overlaps, [(first, second)])
            }
            _ => unreachable!(),
        }
        assert_eq!(report.hard, [violation]);
    }
}

#[test]
fn cubic_extrema_measure_return_between_vertices_and_ignore_the_other_axis() {
    let mut scene = Scene {
        bodies: vec![point(10., 0.), point(10., 30.)],
        edges: vec![edge(0, 1, "M10 0C30 10 30 20 10 30")],
        ..Scene::default()
    };
    let report = validate(&scene);
    assert!(report.hard.is_empty(), "{report:?}");
    let metrics = report.edges[0].unwrap();
    close(metrics.sideways_return, 15.0);
    assert!(metrics.length > 30.0);
    scene.edges[0].cross_axis = CrossAxis::Y;
    close(validate(&scene).edges[0].unwrap().sideways_return, 0.0);
    scene.edges[0].transform = Affine::new([0., 1., 1., 0., 0., 0.]);
    scene.bodies = vec![point(0., 10.), point(30., 10.)];
    let report = validate(&scene);
    assert!(report.hard.is_empty(), "{report:?}");
    close(report.edges[0].unwrap().sideways_return, 15.0);
}

#[test]
fn markers_sample_one_depth_on_the_curve_at_either_end_and_contour_undoes_setback() {
    let view = Rect::new(0., 0., 12., 10.);
    let viewport = Size::new(12., 10.);
    let mut scene = Scene {
        bodies: vec![node(0., 8., 10., 12.), node(40., 8., 50., 12.)],
        edges: vec![edge(0, 1, "M12 10h26")],
        ..Scene::default()
    };
    let e = &mut scene.edges[0];
    e.start.setback = 2.;
    e.end.setback = 2.;
    e.start.marker = Some(PointMarker::from_view_box(End::Start, view, viewport, 2.).unwrap());
    e.end.marker = Some(PointMarker::from_view_box(End::End, view, viewport, 10.).unwrap());
    assert_eq!(e.end.marker.unwrap().depth, 10.);
    let report = validate(&scene);
    assert!(report.hard.is_empty());
    assert_eq!((report.contacts_checked, report.markers_checked), (2, 2));

    // The terminal tangents and shape contacts stay horizontal. A short straight stub at each
    // end hides two immediate hairpin turns from a tangent-only marker check.
    scene.edges[0].d = "M12 10h0.2q0 10 8 10h9.6q8 0 8 -10h0.2";
    let report = validate(&scene);
    assert_eq!(report.markers_checked, 2);
    assert_eq!(report.hard.len(), 2);
    for (violation, side) in report.hard.iter().zip([End::Start, End::End]) {
        let Violation::MarkerAngle {
            edge: 0,
            end,
            sample_angle_deg: Some(angle),
            half_opening_deg,
        } = violation
        else {
            panic!("{violation:?}")
        };
        assert_eq!(*end, side);
        assert!(angle > half_opening_deg);
    }
    let scaled = PointMarker::from_view_box(
        End::End,
        Rect::new(5., 3., 17., 13.),
        Size::new(24., 30.),
        15.,
    )
    .unwrap();
    assert_eq!((scaled.depth, scaled.half_width), (20., 10.));
    assert!(PointMarker::from_view_box(End::End, view, viewport, 0.).is_none());
}

#[test]
fn shaped_contact_checks_the_outer_contour_instead_of_the_measured_box() {
    let mut diamond = node(30., 20., 50., 40.);
    diamond.contour = Some(BezPath::from_svg("M40 20L50 30L40 40L30 30Z").unwrap());
    let mut scene = Scene {
        bodies: vec![node(0., 23., 10., 27.), diamond],
        edges: vec![edge(0, 1, "M10 25C10 25 30 25 33 25")],
        ..Scene::default()
    };
    scene.edges[0].end.setback = 2.;
    assert!(validate(&scene).hard.is_empty());
    // The same marker tip now lands on the enclosing rectangle, 5/sqrt(2) outside the diamond.
    scene.edges[0].d = "M10 25H28";
    let report = validate(&scene);
    let [
        Violation::EndpointContour {
            edge: 0,
            end: End::End,
            gap,
        },
    ] = report.hard.as_slice()
    else {
        panic!("{report:?}")
    };
    close(*gap, 5.0 / 2.0_f64.sqrt());
}

#[test]
fn actual_contour_contact_accepts_a_quarter_pixel_inclusively() {
    let mut circle = node(10., 10., 30., 30.);
    circle.contour = Some(kurbo::Circle::new((20., 20.), 10.).to_path(0.01));
    let mut scene = Scene {
        bodies: vec![circle, node(60., 15., 70., 25.)],
        edges: vec![edge(0, 1, "M30.25 20H59.75")],
        ..Scene::default()
    };
    // Test both sides of the outline as well as the inclusive acceptance boundary.
    for d in ["M30.25 20H59.75", "M29.751 20H60.249"] {
        scene.edges[0].d = d;
        let report = validate(&scene);
        assert_eq!(report.contacts_checked, 2);
        assert!(report.hard.is_empty(), "{report:?}");
    }
    scene.edges[0].d = "M30.251 20H59.749";
    let report = validate(&scene);
    assert_eq!(report.hard.len(), 2);
    for (violation, side) in report.hard.iter().zip([End::Start, End::End]) {
        let Violation::EndpointContour { edge: 0, end, gap } = violation else {
            panic!("{violation:?}")
        };
        assert_eq!(*end, side);
        close(*gap, 0.251);
    }
}

#[test]
fn unmarked_shafts_must_follow_both_declared_outward_normals_at_exact_contacts() {
    let mut scene = Scene {
        bodies: vec![node(0., 10., 10., 25.), node(40., 10., 50., 25.)],
        edges: vec![edge(0, 1, "M10 15C10 15 25 15 25 15C25 15 40 15 40 15")],
        ..Scene::default()
    };
    scene.edges[0].start.outward_normal = Some(Vec2::new(4., 0.));
    scene.edges[0].end.outward_normal = Some(Vec2::new(-9., 0.));
    let report = validate(&scene);
    assert_eq!(
        (
            report.contacts_checked,
            report.normals_checked,
            report.markers_checked
        ),
        (2, 2, 0)
    );
    assert!(report.hard.is_empty(), "{report:?}");
    for (d, angle) in [
        ("M10 15C18 23 32 23 40 15", 45.),
        ("M10 15C4 15 46 15 40 15", 180.),
    ] {
        scene.edges[0].d = d;
        let report = validate(&scene);
        assert_eq!(report.hard.len(), 2);
        for (v, side) in report.hard.iter().zip([End::Start, End::End]) {
            let Violation::EndpointNormal {
                edge: 0,
                end,
                deviation_deg,
            } = v
            else {
                panic!("{v:?}")
            };
            assert_eq!(*end, side);
            close(*deviation_deg, angle);
        }
    }
    scene.edges[0].d = "M10 15H40";
    for invalid in [
        Vec2::ZERO,
        Vec2::new(f64::NAN, 1.),
        Vec2::new(f64::INFINITY, 0.),
    ] {
        scene.edges[0].start.outward_normal = Some(invalid);
        assert_eq!(
            validate(&scene).hard,
            [Violation::InvalidNormal {
                edge: 0,
                end: End::Start
            }]
        );
    }
}

#[test]
fn groups_exempt_only_endpoint_ancestors_and_keep_titles_as_obstacles() {
    let mut scene = Scene {
        bodies: vec![
            Body::group(Rect::new(0., 0., 100., 90.)),
            Body::group(Rect::new(10., 25., 50., 75.)),
            node(15., 45., 25., 55.),
            node(75., 45., 85., 55.),
            Body::group(Rect::new(52., 60., 70., 75.)),
            node(54., 64., 58., 68.),
        ],
        labels: vec![
            label(LabelOwner::Body(0), 35., 3., 65., 9.),
            label(LabelOwner::Body(1), 15., 28., 28., 34.),
            label(LabelOwner::Edge(0), 38., 48., 42., 52.),
        ],
        edges: vec![edge(2, 3, "M25 50H75"), edge(1, 2, "M10 50H15")],
    };
    for (id, parent) in [(1, 0), (2, 1), (3, 0), (4, 0), (5, 4)] {
        scene.bodies[id].parent = Some(parent);
    }
    assert!(validate(&scene).hard.is_empty());
    // Cross one unrelated group twice, enter its child, and cut through a directly attached group's title.
    scene.edges[0].d = "M25 50H51L56 66H64V57L68 67L75 50";
    scene.edges[1].d = "M10 31H23L20 45";
    assert_eq!(
        validate(&scene).hard,
        [
            Violation::ForeignGroupHit { edge: 0, group: 4 },
            Violation::ForeignNodeHit { edge: 0, node: 5 },
            Violation::ForeignLabelHit { edge: 1, label: 1 },
        ]
    );
    scene.edges[1].d = "M10 50L2 50L20 45";
    scene.bodies[5].bounds = Rect::new(68., 64., 74., 68.);
    scene.labels[1].bounds = Rect::new(8., 28., 18., 34.);
    let report = validate(&scene);
    for item in [Item::Edge(1), Item::Label(1)] {
        assert!(
            report
                .hard
                .contains(&Violation::OutsideGroup { item, group: 1 }),
            "{report:?}"
        );
    }
    assert!(report.hard.contains(&Violation::OutsideGroup {
        item: Item::Body(5),
        group: 4
    }));
}

#[test]
fn sibling_overlap_excludes_containment_and_zero_area_contact() {
    let mut scene = Scene {
        bodies: vec![
            Body::group(Rect::new(0., 0., 30., 30.)),
            node(0., 0., 10., 10.),
            node(10., 0., 20., 10.),
        ],
        ..Scene::default()
    };
    scene.bodies[1].parent = Some(0);
    scene.bodies[2].parent = Some(0);
    assert!(validate(&scene).hard.is_empty());
    scene.bodies[2].bounds.x0 = 9.;
    assert_eq!(
        validate(&scene).hard,
        [Violation::SiblingOverlap {
            first: 1,
            second: 2
        }]
    );
}

#[test]
fn crossings_count_pairs_once_and_exclude_touches_and_collinear_runs() {
    let scene = Scene {
        bodies: vec![
            point(0., 10.),
            point(40., 10.),
            point(0., 0.),
            point(40., 0.),
            point(30., 10.),
        ],
        edges: vec![
            edge(0, 1, "M0 10H40"),
            edge(2, 3, "M0 0L10 20L20 0L30 20L40 0"),
            edge(0, 4, "M0 10H2M20 10H30"),
        ],
        ..Scene::default()
    };
    let report = validate(&scene);
    assert!(report.hard.is_empty(), "{report:?}");
    assert_eq!(report.crossings, [(0, 1), (1, 2)]);
    close(report.edges[2].unwrap().length, 12.0);
    // Two incident edges may meet properly away from their shared endpoint.
    let scene = Scene {
        bodies: vec![point(0., 10.), point(40., 10.), point(40., 20.)],
        edges: vec![edge(0, 1, "M0 10H40"), edge(0, 2, "M0 10L10 0L40 20")],
        ..Scene::default()
    };
    assert_eq!(validate(&scene).crossings, [(0, 1)]);
}

#[test]
fn relative_arcs_and_disconnected_subpaths_only_hit_painted_spans() {
    let scene = Scene {
        bodies: vec![
            point(0., 20.),
            point(60., 20.),
            node(9., 9., 11., 11.),
            node(29., 19., 31., 21.),
        ],
        edges: vec![edge(0, 1, "m0 20a10 10 0 0 1 20 0m20 0h20")],
        ..Scene::default()
    };
    let report = validate(&scene);
    assert_eq!(
        report.hard,
        [Violation::ForeignNodeHit { edge: 0, node: 2 }]
    );
    assert!((report.edges[0].unwrap().length - (std::f64::consts::PI * 10. + 20.)).abs() < 0.02);
}

#[test]
fn a_straight_bezier_can_reverse_beyond_both_endpoints() {
    let scene = Scene {
        bodies: vec![point(10., 10.), node(20., 9., 21., 11.), point(0., 0.)],
        edges: vec![
            edge(0, 0, "M10 10Q40 10 10 10"),
            edge(2, 2, "M0 0L23 9V12L25 0Z"),
        ],
        ..Scene::default()
    };
    let report = validate(&scene);
    assert_eq!(
        report.hard,
        [Violation::ForeignNodeHit { edge: 0, node: 1 }]
    );
    assert_eq!(report.crossings, [(0, 1)]);
    close(report.edges[0].unwrap().sideways_return, 15.0);
}

#[test]
fn invalid_paint_coordinates_and_ownership_cannot_produce_a_clean_report() {
    let mut scene = Scene {
        bodies: vec![
            Body::group(Rect::new(0., 0., 40., 40.)),
            point(0., 10.),
            point(30., 10.),
            node(4., 4., 1., 6.),
        ],
        labels: vec![label(LabelOwner::Edge(99), f64::NAN, 0., 9., 1.)],
        edges: vec![
            edge(1, 2, ""),
            edge(1, 2, "M0 10 L30 10 ?"),
            edge(1, 2, "M0 10 h0"),
            edge(1, 2, "M0 10Q15 -40 30 10"),
            edge(1, 99, "M0 10H30"),
            edge(1, 2, "M0 10H30"),
            edge(1, 2, "M1e400 1 H30"),
        ],
    };
    scene.bodies[0].parent = Some(0);
    scene.bodies[3].parent = Some(99);
    scene.edges[5].end.marker = Some(PointMarker {
        depth: 0.,
        half_width: 4.,
    });
    let report = validate(&scene);
    for id in [0, 1, 2, 6] {
        assert!(report.edges[id].is_none());
        assert!(
            report
                .hard
                .iter()
                .any(|v| matches!(v, Violation::InvalidPath { edge, .. } if *edge == id))
        );
    }
    for item in [Item::Body(0), Item::Body(3), Item::Edge(4), Item::Label(0)] {
        assert!(
            report.hard.contains(&Violation::InvalidOwner { item }),
            "{report:?}"
        );
    }
    for item in [Item::Body(3), Item::Label(0), Item::Edge(3), Item::Edge(5)] {
        assert!(
            report.hard.contains(&Violation::InvalidGeometry { item }),
            "{report:?}"
        );
    }
    // Negative control coordinates and negative relative deltas are legal when the ink stays inside the frame.
    let scene = Scene {
        bodies: vec![point(0., 10.), point(30., 10.)],
        edges: vec![edge(0, 1, "M0 10q15 -12 30 0")],
        ..Scene::default()
    };
    assert!(validate(&scene).hard.is_empty());
}
