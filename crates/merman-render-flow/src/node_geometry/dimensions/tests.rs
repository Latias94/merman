use super::*;

fn metrics() -> TextMetrics {
    TextMetrics {
        width: 100.0,
        height: 20.0,
        line_count: 1,
    }
}

#[test]
fn curved_neo_shapes_match_pinned_dimensions_and_sampling() {
    let stadium = StadiumGeometry::from_label(100.0, 20.0, 15.0, false);
    assert_eq!((stadium.width, stadium.height), (123.75, 35.0));
    let neo_stadium = StadiumGeometry::from_label(100.0, 20.0, 15.0, true);
    assert_eq!((neo_stadium.width, neo_stadium.height), (151.0, 44.0));
    assert_eq!(stadium.outline().len(), 103);
    assert_eq!(neo_stadium.outline().len(), 103);
    let delay = DelayGeometry::from_label(100.0, 20.0, 15.0, false);
    let neo_delay = DelayGeometry::from_label(100.0, 20.0, 15.0, true);
    assert!((delay.width - 134.174243).abs() < 1e-5 && delay.height == 50.0);
    assert!((neo_delay.width - 136.808164).abs() < 1e-5 && neo_delay.height == 44.0);
    let display = DisplayGeometry::from_label(100.0, 20.0, 15.0, false);
    let neo_display = DisplayGeometry::from_label(100.0, 20.0, 15.0, true);
    assert_eq!((display.width, display.height), (162.5, 50.0));
    assert_eq!((neo_display.width, neo_display.height), (165.0, 44.0));
    assert_eq!(display.points.len(), 55);
    assert_eq!(neo_display.points.len(), 55);
    for shape in [
        StadiumGeometry::from_label(0.0, 0.0, 0.0, false).outline(),
        DelayGeometry::from_label(0.0, 0.0, 0.0, false).points,
        DisplayGeometry::from_label(0.0, 0.0, 0.0, false).points,
    ] {
        assert!(shape.iter().all(|(x, y)| x.is_finite() && y.is_finite()));
    }
    for neo in [false, true] {
        assert_eq!(
            node_render_dimensions(Some("stadium"), metrics(), -2.0, neo),
            node_render_dimensions(Some("stadium"), metrics(), 0.0, neo)
        );
        assert_eq!(
            node_render_dimensions(Some("delay"), metrics(), -2.0, neo),
            node_render_dimensions(Some("delay"), metrics(), 0.0, neo)
        );
        assert_eq!(
            node_render_dimensions(Some("display"), metrics(), -2.0, neo),
            node_render_dimensions(Some("display"), metrics(), 0.0, neo)
        );
    }
}

#[test]
fn neo_shape_dimensions_match_pinned_source() {
    // Executed Mermaid12 shape prefixes, with synthetic label bounds 100x20.
    for (
        neo,
        padding,
        lean_w,
        lean_h,
        inverted_w,
        inverted_h,
        odd_w,
        odd_h,
        hex_w,
        hex_h,
        circle_d,
    ) in [
        (
            false,
            0.0,
            120.0,
            20.0,
            120.0,
            20.0,
            105.0,
            20.0,
            110.0,
            20.0,
            111.9803902718557,
        ),
        (
            false,
            15.0,
            150.0,
            35.0,
            180.0,
            50.0,
            123.75,
            35.0,
            132.5,
            35.0,
            141.9803902718557,
        ),
        (
            false,
            31.0,
            182.0,
            51.0,
            244.0,
            82.0,
            143.75,
            51.0,
            156.5,
            51.0,
            173.9803902718557,
        ),
        (
            true,
            0.0,
            120.0,
            20.0,
            120.0,
            20.0,
            153.0,
            44.0,
            183.42857142857144,
            90.0,
            157.9803902718557,
        ),
        (
            true,
            15.0,
            165.0,
            35.0,
            210.0,
            50.0,
            153.0,
            44.0,
            183.42857142857144,
            90.0,
            157.9803902718557,
        ),
        (
            true,
            31.0,
            213.0,
            51.0,
            306.0,
            82.0,
            153.0,
            44.0,
            183.42857142857144,
            90.0,
            157.9803902718557,
        ),
    ] {
        for (shape, expected) in [
            ("lean_right", (lean_w, lean_h)),
            ("lean_left", (lean_w, lean_h)),
            ("trapezoid", (lean_w, lean_h)),
            ("inv_trapezoid", (inverted_w, inverted_h)),
            ("odd", (odd_w, odd_h)),
            ("hexagon", (hex_w, hex_h)),
            ("doublecircle", (circle_d, circle_d)),
        ] {
            let actual = node_render_dimensions(Some(shape), metrics(), padding, neo);
            assert!(
                (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
                "{shape}, neo={neo}, padding={padding}: {actual:?}, expected {expected:?}"
            );
        }
    }
}

#[test]
fn neo_shape_vertices_preserve_empty_and_tall_labels() {
    for (kind, expected) in [
        (
            LeanKind::Right,
            [(-17.5, 0.0), (130.0, 0.0), (147.5, -35.0), (0.0, -35.0)],
        ),
        (
            LeanKind::Left,
            [(0.0, 0.0), (147.5, 0.0), (130.0, -35.0), (-17.5, -35.0)],
        ),
        (
            LeanKind::Trapezoid,
            [(-17.5, 0.0), (147.5, 0.0), (130.0, -35.0), (0.0, -35.0)],
        ),
        (
            LeanKind::InvertedTrapezoid,
            [(0.0, 0.0), (160.0, 0.0), (185.0, -50.0), (-25.0, -50.0)],
        ),
    ] {
        assert_eq!(
            LeanGeometry::from_label(kind, 100.0, 20.0, 15.0, true).points,
            expected
        );
        let empty = LeanGeometry::from_label(kind, 0.0, 0.0, 0.0, false);
        assert_eq!((empty.width, empty.height), (0.0, 0.0));
        assert!(empty.points.iter().all(|&(x, y)| x == 0.0 && y == 0.0));
    }
    let odd = OddGeometry::from_label(100.0, 20.0, 15.0, true);
    assert_eq!(
        odd.points,
        [
            (-82.0, -22.0),
            (-71.0, 0.0),
            (-82.0, 22.0),
            (71.0, 22.0),
            (71.0, -22.0)
        ]
    );
    assert_eq!(odd.shift_x, 5.5);
    let empty = OddGeometry::from_label(0.0, 0.0, 0.0, true);
    assert_eq!(
        (empty.width, empty.height, empty.shift_x),
        (48.0, 24.0, 3.0)
    );
    let hex = HexagonGeometry::from_label(0.0, 0.0, 0.0, true);
    assert_eq!(
        (hex.width, hex.height, hex.points[0]),
        (72.0, 70.0, (20.0, 0.0))
    );
    let empty = HexagonGeometry::from_label(0.0, 0.0, 0.0, false);
    assert_eq!((empty.width, empty.height), (0.0, 0.0));
    let tall = HexagonGeometry::from_label(20.0, 160.0, 15.0, true);
    assert_eq!(tall.height, 230.0);
    assert!((tall.points[0].0 - 65.71428571428571).abs() < 1e-9);
    for (neo, inner, outer) in [(false, 40.0, 45.0), (true, 41.0, 53.0)] {
        let circle = DoubleCircleGeometry::from_label(40.0, 30.0, 15.0, neo);
        assert_eq!((circle.inner_radius, circle.outer_radius), (inner, outer));
        let painted = DoubleCircleGeometry::from_outer_diameter(2.0 * circle.outer_radius, neo);
        assert_eq!((painted.inner_radius, painted.outer_radius), (inner, outer));
        let tall = DoubleCircleGeometry::from_label(20.0, 160.0, 15.0, neo);
        let expected = if neo {
            108.62257748298549
        } else {
            100.62257748298549
        };
        assert!((tall.outer_radius - expected).abs() < 1e-9);
    }
    let empty = DoubleCircleGeometry::from_label(0.0, 0.0, 0.0, false);
    assert_eq!((empty.inner_radius, empty.outer_radius), (0.0, 5.0));
    let painted = DoubleCircleGeometry::from_outer_diameter(10.0, false);
    assert_eq!(painted.inner_radius, 0.0);
    let empty = DoubleCircleGeometry::from_label(0.0, 0.0, 0.0, true);
    assert_eq!((empty.inner_radius, empty.outer_radius), (16.0, 28.0));
    for shape in [
        "lean_right",
        "lean_left",
        "trapezoid",
        "inv_trapezoid",
        "odd",
        "hexagon",
        "doublecircle",
    ] {
        for neo in [false, true] {
            assert_eq!(
                node_render_dimensions(Some(shape), metrics(), -2.0, neo),
                node_render_dimensions(Some(shape), metrics(), 0.0, neo)
            );
        }
    }
}

#[test]
fn stacked_document_geometry_matches_pinned_neo_and_classic_source() {
    // Captured from the pinned TS outerPathPoints using its sine generator.
    for (neo, padding, width, height, bounds, label_dy, group_dy, last_wave_y) in [
        (
            true,
            15.0,
            100.0,
            20.0,
            (-76.0, -41.5, 76.0, 55.49005261696825),
            -4.0,
            -7.0,
            28.18520877186785,
        ),
        (
            false,
            15.0,
            100.0,
            20.0,
            (-75.0, -44.53125, 75.0, 52.65047696520479),
            1.875,
            -4.0625,
            36.80391580510188,
        ),
        (
            false,
            0.0,
            100.0,
            20.0,
            (-60.0, -20.625, 60.0, 23.123223681601473),
            7.5,
            -1.25,
            18.247358709262116,
        ),
        (
            true,
            0.0,
            0.0,
            0.0,
            (-26.0, -30.25, 26.0, 39.2436052537653),
            1.0,
            -4.5,
            21.690491353343617,
        ),
    ] {
        let geometry = flowchart_stacked_document_geometry(width, height, padding, neo);
        assert_eq!(geometry.outer_points.len(), 62);
        assert_eq!(geometry.inner_points.len(), 6);
        let actual = geometry.outer_points.iter().fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(min_x, min_y, max_x, max_y), &(x, y)| {
                (min_x.min(x), min_y.min(y), max_x.max(x), max_y.max(y))
            },
        );
        for (a, b) in [
            (actual.0, bounds.0),
            (actual.1, bounds.1),
            (actual.2, bounds.2),
            (actual.3, bounds.3),
        ] {
            assert!(
                (a - b).abs() < 1e-9,
                "neo={neo}, padding={padding}: {actual:?}"
            );
        }
        assert_eq!(geometry.label_dx, -10.0);
        assert_eq!(geometry.label_dy, label_dy);
        assert_eq!(geometry.group_dy, group_dy);
        assert!((geometry.outer_points[52].1 - last_wave_y).abs() < 1e-9);
        for shape in ["docs", "documents", "st-doc", "stacked-document"] {
            let size = node_render_dimensions(
                Some(shape),
                TextMetrics {
                    width,
                    height,
                    line_count: 1,
                },
                padding,
                neo,
            );
            assert!((size.0 - (bounds.2 - bounds.0)).abs() < 1e-9);
            assert!((size.1 - (bounds.3 - bounds.1)).abs() < 1e-9);
        }
    }
    let neo = flowchart_stacked_document_geometry(100.0, 20.0, 15.0, true);
    for padding in [0.0, -2.0, 100.0] {
        let other = flowchart_stacked_document_geometry(100.0, 20.0, padding, true);
        assert_eq!(other.outer_points, neo.outer_points);
        assert_eq!(other.label_dy, neo.label_dy);
    }
    // Preserve the established finite zero-span layout fallback, rather
    // than importing the TS sine helper's division-by-zero NaNs.
    for padding in [0.0, -2.0] {
        let empty = flowchart_stacked_document_geometry(0.0, 0.0, padding, false);
        assert!(
            empty
                .outer_points
                .iter()
                .chain(&empty.inner_points)
                .all(|(x, y)| x.is_finite() && y.is_finite())
        );
        assert_eq!(empty.outer_points[0], (-10.0, 10.0));
        assert_eq!(empty.label_dy, 10.0);
        let size = node_render_dimensions(
            Some("documents"),
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            },
            padding,
            false,
        );
        assert_eq!(size, (20.0, 20.0));
    }
}

#[test]
fn brace_layout_uses_pinned_neo_axis_padding_and_preserves_zero() {
    let empty = TextMetrics {
        width: 0.0,
        height: 0.0,
        line_count: 1,
    };
    for (shape, zero_width, neo_width, neo_height) in [
        ("brace", 15.0, 129.8, 42.0),
        ("brace-l", 15.0, 129.8, 42.0),
        ("comment", 15.0, 129.8, 42.0),
        ("brace-r", 10.0, 146.0, 54.0),
        ("braces", 12.5, 148.5, 54.0),
    ] {
        for padding in [0.0, -2.0] {
            let zero = node_render_dimensions(Some(shape), empty, padding, false);
            assert!((zero.0 - zero_width).abs() < 1e-9, "{shape}: {zero:?}");
            assert!((zero.1 - 10.0).abs() < 1e-9, "{shape}: {zero:?}");
        }
        // Neo owns its axis padding, independent of configured padding.
        for padding in [0.0, 15.0, 40.0] {
            let neo = node_render_dimensions(Some(shape), metrics(), padding, true);
            assert!((neo.0 - neo_width).abs() < 1e-9, "{shape}: {neo:?}");
            assert!((neo.1 - neo_height).abs() < 1e-9, "{shape}: {neo:?}");
        }
    }
}

#[test]
fn neo_uses_pinned_shape_specific_content_padding() {
    assert_eq!(
        node_render_dimensions(Some("squareRect"), metrics(), 15.0, true),
        (132.0, 44.0)
    );
    assert_eq!(
        node_render_dimensions(Some("stadium"), metrics(), 15.0, true),
        (151.0, 44.0)
    );
    assert_eq!(
        node_render_dimensions(Some("subroutine"), metrics(), 15.0, true),
        (144.0, 32.0)
    );

    let (cylinder_w, cylinder_h) = node_render_dimensions(Some("cylinder"), metrics(), 15.0, true);
    let expected_ry = 62.0 / (2.5 + 124.0 / 50.0);
    assert_eq!(cylinder_w, 124.0);
    assert!((cylinder_h - (44.0 + 3.0 * expected_ry)).abs() < 1e-9);

    // `linedCylinder.ts` uses independent fixed x/y padding in Neo mode.
    for (neo, padding_x, padding_y) in [(false, 15.0, 15.0), (true, 16.0, 24.0)] {
        let (width, height) = node_render_dimensions(Some("lined-cylinder"), metrics(), 15.0, neo);
        let expected_width = 100.0 + 2.0 * padding_x;
        let rx = expected_width / 2.0;
        let ry = rx / (2.5 + expected_width / 50.0);
        let expected_height = 20.0 + 2.0 * padding_y + 3.0 * ry;
        assert!(
            (width - expected_width).abs() < 1e-9,
            "{neo}: width={width}"
        );
        assert!(
            (height - expected_height).abs() < 1e-9,
            "{neo}: height={height}"
        );
    }
    assert_eq!(
        node_render_dimensions(Some("triangle"), metrics(), 15.0, true),
        (150.0, 150.0)
    );
    assert_eq!(
        node_render_dimensions(Some("flipped-triangle"), metrics(), 15.0, true),
        (150.0, 150.0)
    );
    assert_eq!(
        node_render_dimensions(Some("triangle"), metrics(), 15.0, false),
        (135.0, 135.0)
    );
    assert_eq!(
        node_render_dimensions(Some("flipped-triangle"), metrics(), 15.0, false),
        (135.0, 135.0)
    );
    let classic_h_cyl = node_render_dimensions(Some("h-cyl"), metrics(), 15.0, false);
    let classic_h = 20.0 + 7.5;
    let classic_ry = classic_h / 2.0;
    let classic_rx = classic_ry / (2.5 + classic_h / 50.0);
    assert!((classic_h_cyl.0 - (100.0 + 7.5 + 3.0 * classic_rx)).abs() < 1e-9);
    assert!((classic_h_cyl.1 - classic_h).abs() < 1e-9);
    let neo_h_cyl = node_render_dimensions(Some("h-cyl"), metrics(), 15.0, true);
    let neo_h = 20.0 + 12.0;
    let neo_ry = neo_h / 2.0;
    let neo_rx = neo_ry / (2.5 + neo_h / 50.0);
    assert!((neo_h_cyl.0 - (100.0 + 12.0 + 3.0 * neo_rx)).abs() < 1e-9);
    assert!((neo_h_cyl.1 - neo_h).abs() < 1e-9);
    assert_eq!(
        node_render_dimensions(Some("document"), metrics(), 15.0, false),
        (130.0, 62.49555920400368)
    );
    assert_eq!(
        node_render_dimensions(Some("document"), metrics(), 15.0, true),
        (132.0, 65.99218419904648)
    );
    assert_eq!(
        node_render_dimensions(Some("sloped-rectangle"), metrics(), 15.0, false),
        (130.0, 75.0)
    );
    assert_eq!(
        node_render_dimensions(Some("sloped-rectangle"), metrics(), 15.0, true),
        (132.0, 66.0)
    );
    assert_eq!(
        node_render_dimensions(Some("card"), metrics(), 15.0, false),
        (127.0, 35.0)
    );
    assert_eq!(
        node_render_dimensions(Some("card"), metrics(), 15.0, true),
        (156.0, 68.0)
    );

    for (neo, padding_x, padding_y) in [(false, 15.0, 15.0), (true, 16.0, 20.0)] {
        let (width, height) = node_render_dimensions(Some("paper-tape"), metrics(), 15.0, neo);
        let base_w = 100.0 + 2.0 * padding_x;
        let base_h = 20.0 + padding_y;
        let amplitude = base_h / 8.0;
        let final_h = base_h + 2.0 * amplitude;
        let sampled_peak = (0..=50)
            .map(|i| ((i as f64) / 50.0 * std::f64::consts::TAU).sin().abs())
            .fold(0.0, f64::max);
        let expected_h = final_h + 2.0 * amplitude * sampled_peak;
        assert!(
            (width - base_w).abs() < 1e-9,
            "paper tape neo={neo}: {width}"
        );
        assert!(
            (height - expected_h).abs() < 1e-9,
            "paper tape neo={neo}: {height}"
        );
    }

    for (neo, padding_x, padding_y) in [(false, 15.0, 15.0), (true, 16.0, 12.0)] {
        let (width, height) = node_render_dimensions(Some("tag-rect"), metrics(), 15.0, neo);
        let base_h = 20.0 + 2.0 * padding_y;
        let expected_width = 100.0 + 2.0 * padding_x + 0.2 * base_h;
        assert!(
            (width - expected_width).abs() < 1e-9,
            "tag rect neo={neo}: {width}"
        );
        assert!(
            (height - base_h).abs() < 1e-9,
            "tag rect neo={neo}: {height}"
        );
    }

    for (neo, padding_x, padding_y, offset) in [(false, 15.0, 15.0, 5.0), (true, 16.0, 12.0, 10.0)]
    {
        let actual = node_render_dimensions(Some("stacked-rectangle"), metrics(), 15.0, neo);
        let expected = (
            100.0 + 2.0 * padding_x + 2.0 * offset,
            20.0 + 2.0 * padding_y + 2.0 * offset,
        );
        assert!(
            (actual.0 - expected.0).abs() < 1e-9,
            "stacked rectangle neo={neo}: {actual:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() < 1e-9,
            "stacked rectangle neo={neo}: {actual:?}"
        );
    }
    // The trapezoidal pentagon has no automatic minimum; 15x5 is only applied
    // by Mermaid when an explicit node width or height is supplied.
    assert_eq!(
        node_render_dimensions(Some("notched-pentagon"), metrics(), 15.0, false),
        (130.0, 50.0)
    );
    assert_eq!(
        node_render_dimensions(Some("notched-pentagon"), metrics(), 15.0, true),
        (132.0, 44.0)
    );
    assert_eq!(
        node_render_dimensions(
            Some("notched-pentagon"),
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            },
            0.0,
            false,
        ),
        (0.0, 0.0)
    );
    assert_eq!(
        node_render_dimensions(
            Some("notched-pentagon"),
            TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            },
            0.0,
            true,
        ),
        (32.0, 24.0)
    );

    assert_eq!(
        node_render_dimensions(Some("shaded-process"), metrics(), 15.0, false),
        (146.0, 50.0)
    );
    assert_eq!(
        node_render_dimensions(Some("shaded-process"), metrics(), 15.0, true),
        (140.0, 44.0)
    );

    // Stored data uses Neo's 16px/12px axis padding before the sampled arc bbox.
    let neo_bow = node_render_dimensions(Some("bow-tie-rectangle"), metrics(), 15.0, true);
    assert!((neo_bow.0 - 137.07813754398302).abs() < 1e-9);
    assert!((neo_bow.1 - 32.0).abs() < 1e-9);
    let classic_bow = node_render_dimensions(Some("bow-tie-rectangle"), metrics(), 15.0, false);
    assert!((classic_bow.0 - 135.4500714461302).abs() < 1e-9);
    assert!((classic_bow.1 - 35.0).abs() < 1e-9);

    // Window pane: Neo uses 16px horizontal and 12px vertical padding before the
    // fixed 10px frame offset; Classic keeps the configured padding on both axes.
    assert_eq!(
        node_render_dimensions(Some("window-pane"), metrics(), 15.0, false),
        (140.0, 60.0)
    );
    assert_eq!(
        node_render_dimensions(Some("window-pane"), metrics(), 15.0, true),
        (142.0, 54.0)
    );

    // Divided rectangle: the source adds one 16px Neo padding term to each label
    // axis, then the polygon's 20% top band expands only the rendered height.
    assert_eq!(
        node_render_dimensions(Some("divided-rectangle"), metrics(), 15.0, false),
        (115.0, 42.0)
    );
    let divided_neo = node_render_dimensions(Some("divided-rectangle"), metrics(), 15.0, true);
    assert!((divided_neo.0 - 116.0).abs() < 1e-9);
    assert!((divided_neo.1 - 43.2).abs() < 1e-9);
}

#[test]
fn circle_uses_label_diagonal_and_neo_padding() {
    let empty = TextMetrics {
        width: 0.0,
        height: 0.0,
        line_count: 1,
    };
    assert_eq!(
        node_render_dimensions(Some("circle"), empty, 15.0, true),
        (32.0, 32.0)
    );

    let value = node_render_dimensions(Some("circle"), metrics(), 15.0, true);
    let expected = 100.0_f64.hypot(20.0) + 32.0;
    assert!((value.0 - expected).abs() < 1e-9);
    assert_eq!(value.0, value.1);

    let classic = node_render_dimensions(Some("circle"), metrics(), 15.0, false);
    let classic_expected = 100.0_f64.hypot(20.0) + 15.0;
    assert!((classic.0 - classic_expected).abs() < 1e-9);
    assert_eq!(
        node_render_dimensions(Some("circle"), empty, 24.0, false),
        (24.0, 24.0)
    );
}

#[test]
fn mermaid_1172_object_shapes_use_pinned_dimensions() {
    let (person_w, person_h) = node_render_dimensions(Some("person"), metrics(), 15.0, false);
    let person_width = 100.0 + 2.0 * 15.0;
    let head_radius = person_width * 0.23;
    assert_eq!(person_w, person_width);
    assert!((person_h - (50.0 + 2.0 * head_radius - head_radius * 0.27)).abs() < 1e-9);
    assert_eq!(
        node_render_dimensions(Some("bucket"), metrics(), 15.0, false),
        (130.0, 60.4)
    );
    assert_eq!(
        node_render_dimensions(Some("console"), metrics(), 15.0, false),
        (130.0, 70.0)
    );
    assert_eq!(
        node_render_dimensions(Some("browser"), metrics(), 15.0, false),
        (130.0, 68.0)
    );
}

#[test]
fn classic_shape_padding_remains_unchanged() {
    assert_eq!(
        node_render_dimensions(Some("squareRect"), metrics(), 15.0, false),
        (160.0, 50.0)
    );
    assert_eq!(
        node_render_dimensions(Some("stadium"), metrics(), 15.0, false),
        (123.75, 35.0)
    );
    assert_eq!(
        node_render_dimensions(Some("subroutine"), metrics(), 15.0, false),
        (131.0, 35.0)
    );
}
