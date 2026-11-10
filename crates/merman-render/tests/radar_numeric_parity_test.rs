#![cfg(feature = "diagram-radar")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{
    LayoutPoint, RadarCurveLayout, RadarDiagramLayout, RadarGraticuleShapeLayout,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn source(body: &str, graticule: &str) -> String {
    format!("radar-beta\naxis a,b,c\nticks 0\ngraticule {graticule}\n{body}\n")
}

fn render(source: &str) -> (RadarDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
    let projection = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["RadarDiagram"].clone()).unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .svg()
        .to_owned();
    (layout, svg)
}

fn curve_attribute(svg: &str, attribute: &str) -> String {
    let document = roxmltree::Document::parse(svg).unwrap();
    document
        .descendants()
        .find(|node| node.attribute("class") == Some("radarCurve-0"))
        .unwrap()
        .attribute(attribute)
        .unwrap()
        .to_owned()
}

#[test]
fn radar_finite_layout_numbers_keep_their_float_wire_shape() {
    let graticule = RadarGraticuleShapeLayout {
        kind: "circle".to_string(),
        r: Some(50.0),
        points: vec![LayoutPoint { x: 0.0, y: 50.0 }],
    };
    let curve = RadarCurveLayout {
        label: "Curve".to_string(),
        class_index: 0,
        points: vec![LayoutPoint { x: -0.0, y: 50.0 }],
        path_d: String::new(),
    };
    let wire = serde_json::to_value(&graticule).unwrap();
    assert!(wire["r"].is_f64());
    for value in [wire, serde_json::to_value(&curve).unwrap()] {
        assert!(value["points"][0]["x"].is_f64());
        assert!(value["points"][0]["y"].is_f64());
    }
    let restored: RadarCurveLayout =
        serde_json::from_value(serde_json::to_value(&curve).unwrap()).unwrap();
    assert_eq!(restored.points[0].x.to_bits(), (-0.0_f64).to_bits());
}

// Mermaid 12.3.0 (2086904e), radar/renderer.ts: Math.max retains Infinity;
// relativeRadius clips, multiplies, then divides; template strings preserve
// NaN/Infinity in polygon points and closedRoundCurve control points.
#[test]
fn radar_infinite_max_keeps_finite_entries_at_zero_radius() {
    let overflow = "9".repeat(309);
    for graticule in ["circle", "polygon"] {
        let (layout, svg) = render(&source(
            &format!("max {overflow}\ncurve c{{1,2,3}}"),
            graticule,
        ));
        assert!(
            layout.curves[0]
                .points
                .iter()
                .all(|point| point.x == 0.0 && point.y == 0.0)
        );
        if graticule == "polygon" {
            assert_eq!(curve_attribute(&svg, "points"), "0,0 0,0 0,0");
        } else {
            assert_eq!(
                curve_attribute(&svg, "d"),
                "M0,0 C0,0 0,0 0,0 C0,0 0,0 0,0 C0,0 0,0 0,0 Z"
            );
        }
    }
}

#[test]
fn radar_infinite_min_and_equal_range_keep_nan_geometry() {
    for body in [
        format!("min {}\nmax 3\ncurve c{{1,2,3}}", "9".repeat(309)),
        "min 3\nmax 3\ncurve c{1,2,3}".to_string(),
    ] {
        for graticule in ["circle", "polygon"] {
            let (layout, svg) = render(&source(&body, graticule));
            assert!(
                layout.curves[0]
                    .points
                    .iter()
                    .all(|point| point.x.is_nan() && point.y.is_nan())
            );
            if graticule == "polygon" {
                assert_eq!(curve_attribute(&svg, "points"), "NaN,NaN NaN,NaN NaN,NaN");
            } else {
                assert_eq!(
                    curve_attribute(&svg, "d"),
                    concat!(
                        "MNaN,NaN CNaN,NaN NaN,NaN NaN,NaN",
                        " CNaN,NaN NaN,NaN NaN,NaN CNaN,NaN NaN,NaN NaN,NaN Z",
                    )
                );
            }
        }
    }
}

#[test]
fn radar_infinite_numeric_and_reference_entries_keep_inferred_max() {
    let overflow = "9".repeat(309);
    for entries in [
        format!("{overflow},2,3"),
        format!("c: 3, a: {overflow}, b: 2"),
    ] {
        for graticule in ["circle", "polygon"] {
            let (layout, svg) = render(&source(&format!("curve c{{{entries}}}"), graticule));
            let points = &layout.curves[0].points;
            assert!(points[0].x.is_nan() && points[0].y.is_nan());
            assert!(
                points[1..]
                    .iter()
                    .all(|point| point.x == 0.0 && point.y == 0.0)
            );
            if graticule == "polygon" {
                assert_eq!(curve_attribute(&svg, "points"), "NaN,NaN 0,0 0,0");
            } else {
                assert_eq!(
                    curve_attribute(&svg, "d"),
                    concat!(
                        "MNaN,NaN CNaN,NaN NaN,NaN 0,0",
                        " CNaN,NaN NaN,NaN 0,0 CNaN,NaN NaN,NaN NaN,NaN Z",
                    )
                );
            }
        }
    }
    let (layout, _) = render(&source(
        &format!("curve skipped{{{overflow}}}, drawable{{1,2,3}}"),
        "polygon",
    ));
    assert_eq!(layout.curves.len(), 1);
    assert_eq!(layout.curves[0].class_index, 1);
    assert!(
        layout.curves[0]
            .points
            .iter()
            .all(|point| point.x == 0.0 && point.y == 0.0)
    );

    let (layout, _) = render(&source(
        &format!("max 3\ncurve c{{{overflow},2,3}}"),
        "polygon",
    ));
    assert_eq!(layout.curves[0].points[0].y, -300.0);
}

#[test]
fn radar_finite_multiply_overflow_is_preserved_in_points_and_path() {
    let huge = format!("1{}", "0".repeat(308));
    for graticule in ["circle", "polygon"] {
        let (layout, svg) = render(&source(
            &format!("max {huge}\ncurve c{{{huge},1,2}}"),
            graticule,
        ));
        let points = &layout.curves[0].points;
        assert_eq!(points[0].x, f64::INFINITY);
        assert_eq!(points[0].y, f64::NEG_INFINITY);
        assert!(points[1].x.is_finite() && points[1].x > 0.0);
        assert!(points[1].y.is_finite() && points[1].y > 0.0);
        if graticule == "polygon" {
            assert!(curve_attribute(&svg, "points").starts_with("Infinity,-Infinity "));
        } else {
            assert!(curve_attribute(&svg, "d").starts_with("MInfinity,-Infinity C"));
        }
    }
}

#[test]
fn radar_reversed_and_subnormal_ranges_use_the_original_denominator() {
    let tiny = format!("0.{}5", "0".repeat(323));
    for body in [
        "min 3\nmax 1\ncurve c{1,2,3}".to_string(),
        format!("max {tiny}\ncurve c{{{tiny},0,0}}"),
    ] {
        for graticule in ["circle", "polygon"] {
            let (layout, _) = render(&source(&body, graticule));
            assert_eq!(layout.curves[0].points[0].y, -300.0);
        }
    }
}

#[test]
fn radar_subnormal_ticks_preserve_nonfinite_graticule_output() {
    let tiny = format!("0.{}5", "0".repeat(323));
    for graticule in ["circle", "polygon"] {
        let (layout, svg) = render(&source(
            &format!("ticks {tiny}\ncurve c{{1,2,3}}"),
            graticule,
        ));
        assert_eq!(layout.graticules.len(), 1);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("class") == Some("radarGraticule"))
            .unwrap();
        if graticule == "circle" {
            assert_eq!(layout.graticules[0].r, Some(f64::INFINITY));
            assert_eq!(node.attribute("r"), Some("Infinity"));
        } else {
            assert_eq!(layout.graticules[0].points[0].x, f64::INFINITY);
            assert_eq!(layout.graticules[0].points[0].y, f64::NEG_INFINITY);
            assert_eq!(
                node.attribute("points"),
                Some("Infinity,-Infinity Infinity,Infinity -Infinity,Infinity")
            );
        }
    }
}

#[test]
fn radar_single_point_curve_and_adjacent_statements_keep_upstream_shape() {
    let (_, svg) = render(&format!(
        "radar-beta\naxis a\ncurve c{{{}}}\nticks 0\n",
        "9".repeat(309)
    ));
    assert_eq!(
        curve_attribute(&svg, "d"),
        "MNaN,NaN CNaN,NaN NaN,NaN NaN,NaN Z"
    );

    let (layout, _) = render("radar-beta\nticks 1.5axis b\ncurve c{1}\n");
    assert_eq!(layout.axes.len(), 1);
    assert_eq!(layout.graticules.len(), 2);
    assert_eq!(layout.graticules[0].r, Some(200.0));
    assert_eq!(layout.graticules[1].r, Some(400.0));
}

#[test]
fn radar_overflowing_source_ticks_are_clamped_before_layout_work() {
    let (layout, _) = render(&source(
        &format!("ticks {}\ncurve c{{1,2,3}}", "9".repeat(309)),
        "circle",
    ));
    assert_eq!(layout.graticules.len(), 32);
    assert_eq!(layout.graticules[31].r, Some(300.0));
}
