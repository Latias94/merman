use super::*;
use crate::block::BlockAllocatedBounds;
use kurbo::{ParamCurve, Point};
use merman_model::{family::DiagramFamilyId, runtime::RuntimePolicy};
use merman_render_core::{
    resources::{OperationWorkMeter, RenderResourcePolicy},
    root_svg::RootViewportContext,
    rough_svg::operation_randomness,
    text::DeterministicTextMeasurer,
};

fn cylinder(id: &str) -> BlockShapeGeometry {
    BlockShapeGeometry {
        id: id.to_owned(),
        allocated: BlockAllocatedBounds {
            x: 0.0,
            y: 0.0,
            width: 80.0,
            height: 50.0,
        },
        boundary: BlockShapeBoundary::Cylinder {
            width: 80.0,
            body_height: 30.0,
            radius_x: 40.0,
            radius_y: 10.0,
        },
    }
}

fn hand_drawn_d(geometry: &BlockShapeGeometry, seed: f64) -> String {
    let work_meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let measurer = DeterministicTextMeasurer::default();
    let diagram_id = SvgId::new("block").unwrap();
    let context = BlockSvgContext {
        diagram_id,
        viewport: RootViewportContext::new(DiagramFamilyId::BLOCK, diagram_id),
        work_meter: &work_meter,
        text_measurer: &measurer,
        operation: &operation,
        canvas_background: None,
        shoal: true,
    };
    let randomness = operation_randomness(&operation, seed, "render.block.roughjs");
    hand_drawn_shape_path(geometry, &randomness, &context)
        .unwrap()
        .expect("hand-drawn cylinder path")
}

#[test]
fn hand_drawn_cylinder_strokes_a_closed_outline_and_an_open_rim() {
    let d = hand_drawn_d(&cylinder("store"), 43.0);
    let path = BezPath::from_svg(&d).expect("valid cylinder path data");
    for segment in path.segments() {
        let start = segment.start();
        let end = segment.end();
        let spans_cap = (start.x - end.x).abs() > 79.0
            && (start.y + 15.0).abs() < 0.01
            && (end.y + 15.0).abs() < 0.01;
        assert!(
            !spans_cap || (segment.eval(0.5).y + 15.0).abs() > 2.0,
            "unexpected stroke across the cylinder cap: {segment:?}"
        );
    }

    let mut starts = d.match_indices('M').map(|(index, _)| index);
    assert_eq!(starts.next(), Some(0));
    let rim_start = starts.next().expect("separate cylinder rim subpath");
    assert_eq!(starts.next(), None);
    let (outer, rim) = d.split_at(rim_start);
    assert_eq!(outer.matches('Z').count(), 1);
    assert!(outer.trim_end().ends_with('Z'));
    assert!(!rim.contains('Z'));

    let outer = BezPath::from_svg(outer).unwrap();
    let mut rim = BezPath::from_svg(rim).unwrap();
    rim.close_path(); // SVG fill implicitly closes open subpaths.
    let cap = Point::new(0.0, -10.0);
    assert_ne!(outer.winding(cap), 0);
    assert_eq!(rim.winding(cap), outer.winding(cap));
}

#[test]
fn hand_drawn_same_size_cylinders_have_distinct_reproducible_paths() {
    let left = cylinder("left");
    let right = cylinder("right");
    for seed in [0.0, 43.0] {
        let first = hand_drawn_d(&left, seed);
        assert_ne!(first, hand_drawn_d(&right, seed), "seed {seed}");
        assert_eq!(first, hand_drawn_d(&left, seed), "seed {seed}");
    }
    assert_ne!(hand_drawn_d(&left, 43.0), hand_drawn_d(&left, 44.0));
}

#[test]
fn cylinder_svg_preserves_the_mermaid_path() {
    assert_eq!(
        cylinder_path(80.0, 30.0, 40.0, 10.0),
        "M0,10 a40,10 0,0,0 80 0 a40,10 0,0,0 -80 0 l0,30 a40,10 0,0,0 80 0 l0,-30"
    );
}

#[test]
fn rough_subpath_omits_move_only_geometry() {
    let path = BezPath::from_svg("M12 34").unwrap();
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    let randomness = operation_randomness(&operation, 43.0, "render.block.roughjs");
    let mut options = OptionsBuilder::default()
        .randomness(randomness)
        .build()
        .unwrap();
    let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
    let d = controlled::generate(&meter, merman_model::OperationPhase::Emit, |control| {
        rough_subpath(&path, true, &mut options, control)
    })
    .unwrap();
    assert_eq!(d, None);
}
