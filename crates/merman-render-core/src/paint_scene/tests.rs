use super::*;
use crate::resources::{RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase};
use crate::svg_css::SvgDiagramIdProjection;
use merman_model::OperationControl;

const INK: SceneColor = SceneColor {
    red: 14.0,
    green: 32.0,
    blue: 55.0,
    alpha: 1.0,
};
const CANVAS: SceneColor = SceneColor {
    red: 250.0,
    green: 250.0,
    blue: 255.0,
    alpha: 0.5,
};
const VIEW_BOX: SceneBox = SceneBox {
    x: -5.0,
    y: -10.0,
    width: 100.0,
    height: 70.0,
};
const LABEL: &str = "铅笔 <tag> &amp; ]]> \u{0}";

fn scope() -> SvgId<'static> {
    SvgId::new("seq-main").unwrap()
}

fn scene<'a>(markers: &'a [SceneMarker<'a>], groups: &'a [SceneGroup<'a>]) -> PaintScene<'a> {
    PaintScene {
        viewport: SceneViewport {
            view_box: VIEW_BOX,
            width: 150.0,
            height: 105.0,
            canvas: Some(CANVAS),
        },
        metadata: SceneMetadata {
            title: Some("A & B"),
            description: Some("Use < or >"),
        },
        groups,
        markers,
    }
}

fn with_sequence_paint<T>(run: impl FnOnce(&PaintScene<'_>) -> T) -> T {
    let arrow = [
        ScenePathCommand::MoveTo([0.0, 0.0]),
        ScenePathCommand::LineTo([10.0, 5.0]),
        ScenePathCommand::LineTo([0.0, 10.0]),
        ScenePathCommand::Close,
    ];
    let markers = [SceneMarker {
        view_box: None,
        reference: [7.9, 5.0],
        width: 12.0,
        height: 12.0,
        units: SceneMarkerUnits::UserSpaceOnUse,
        orient: SceneMarkerOrient::AutoStartReverse,
        commands: &arrow,
        paint: ScenePaint {
            fill: Some(INK),
            stroke: None,
        },
    }];
    let families = [
        SceneFontFamily::Named("Name \"A\" & B"),
        SceneFontFamily::Generic(GenericFontFamily::SansSerif),
    ];
    let node = [
        SceneItem::Rect(SceneRect {
            bounds: SceneBox {
                x: 0.0,
                y: 0.0,
                width: 32.0,
                height: 16.0,
            },
            radius: 2.0,
            paint: ScenePaint {
                fill: Some(CANVAS),
                stroke: None,
            },
        }),
        SceneItem::Text(SceneText {
            text: LABEL,
            position: [16.0, 8.0],
            dy: 0.0,
            anchor: SceneTextAnchor::Middle,
            baseline: SceneTextBaseline::Central,
            style: SceneTextStyle {
                families: &families,
                size: 14.0,
                weight: Some(600),
                style: FontStyle::Normal,
                fill: INK,
            },
        }),
    ];
    let points = [
        ScenePathCommand::MoveTo([16.0, 30.0]),
        ScenePathCommand::LineTo([80.0, 30.0]),
    ];
    let edge = [SceneItem::Path(ScenePath {
        commands: &points,
        paint: ScenePaint {
            fill: None,
            stroke: Some(SceneStroke {
                color: INK,
                width: 1.5,
                dash: Some([3.0, 3.0]),
            }),
        },
        start_marker: None,
        end_marker: Some(SceneMarkerRef(0)),
    })];
    let groups = [
        SceneGroup {
            id: SvgId::new("A"),
            role: SceneRole::Node,
            items: &node,
        },
        SceneGroup {
            id: None,
            role: SceneRole::EdgeLabel,
            items: &[],
        },
        SceneGroup {
            id: SvgId::new("A"),
            role: SceneRole::Edge,
            items: &edge,
        },
    ];
    run(&scene(&markers, &groups))
}

#[test]
fn compact_scene_serializes_only_typed_paint_and_escaped_text() {
    with_sequence_paint(|scene| {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::default());
        let svg = write_svg(scene, scope(), &meter).unwrap();
        let parsed = roxmltree::Document::parse(&svg).unwrap();
        let root = parsed.root_element();
        assert_eq!(root.attribute("viewBox"), Some("-5 -10 100 70"));
        assert_eq!(root.attribute("width"), Some("150"));
        assert_eq!(
            root.attribute("aria-labelledby"),
            Some("seq-main-scene-title")
        );
        assert_eq!(
            root.attribute("aria-describedby"),
            Some("seq-main-scene-description")
        );
        let children = root
            .children()
            .filter(roxmltree::Node::is_element)
            .collect::<Vec<_>>();
        assert_eq!(
            children
                .iter()
                .map(|node| node.tag_name().name())
                .collect::<Vec<_>>(),
            ["title", "desc", "rect", "defs", "g", "g"]
        );
        assert_eq!(children[0].text(), Some("A & B"));
        assert_eq!(children[1].text(), Some("Use < or >"));
        assert_eq!(children[2].attribute("fill-opacity"), Some("0.5"));
        assert_eq!(children[2].attribute("data-scene-role"), Some("canvas"));
        assert_ne!(children[4].attribute("id"), children[5].attribute("id"));
        let label = children[4]
            .children()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(label.text(), Some("铅笔 <tag> &amp; ]]> "));
        assert_eq!(
            label.attribute("font-family"),
            Some(r#""Name \"A\" & B", sans-serif"#)
        );
        let marker = children[3].first_element_child().unwrap();
        let shaft = children[5].first_element_child().unwrap();
        assert_eq!(marker.attribute("id"), Some("seq-main-scene-marker-0"));
        assert_eq!(marker.attribute("orient"), Some("auto-start-reverse"));
        assert_eq!(shaft.attribute("d"), Some("M16 30 L80 30"));
        assert_eq!(shaft.attribute("stroke-dasharray"), Some("3 3"));
        assert_eq!(
            shaft.attribute("marker-end"),
            Some("url(#seq-main-scene-marker-0)")
        );
        assert!(!svg.contains("<foreignObject"));
        assert!(!svg.contains("data-points"));
        assert!(!svg.contains(" style="));
    });
}

#[test]
fn compact_scene_admits_exact_bytes_and_keeps_the_first_terminal() {
    with_sequence_paint(|scene| {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let golden_meter = OperationWorkMeter::new(policy);
        let golden = write_svg(scene, scope(), &golden_meter).unwrap();
        let exact_meter = OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxSvgBytes, golden.len())
                .unwrap(),
        );
        assert_eq!(write_svg(scene, scope(), &exact_meter).unwrap(), golden);
        assert_eq!(exact_meter.projected_svg_bytes(), 0);

        let control = OperationControl::new();
        let short_meter = OperationWorkMeter::new_with_control(
            policy
                .with_limit(ResourceLimitId::MaxSvgBytes, golden.len() - 1)
                .unwrap(),
            control.clone(),
        );
        let mut writer = SceneSvgWriter {
            out: BoundedSvgOutput::new(&short_meter),
            meter: &short_meter,
            scope: scope(),
            marker_count: scene.markers.len(),
            elements: 0,
        };
        writer.out.observe_operation().unwrap();
        let Err(Error::ResourceLimitExceeded(first)) = writer.root(scene) else {
            panic!("expected the SVG byte ceiling");
        };
        assert_eq!(first.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(first.limit, "max_svg_bytes");
        assert_eq!((first.actual, first.max), (golden.len(), golden.len() - 1));
        control.cancel();
        assert!(matches!(
            writer.out.finish(),
            Err(Error::ResourceLimitExceeded(error)) if error == first
        ));
    });
}

#[test]
fn compact_scene_honors_cumulative_id_projection() {
    let scene = scene(&[], &[]);
    let policy = RenderResourcePolicy::default();
    let reference = write_svg(&scene, scope(), &OperationWorkMeter::new(policy)).unwrap();
    let limit = reference.len();
    let meter = OperationWorkMeter::new(
        policy
            .with_limit(ResourceLimitId::MaxSvgBytes, limit)
            .unwrap(),
    );
    let projection = SvgDiagramIdProjection::new(&meter);
    let id = scope().with_projection(&projection);
    for _ in 0..limit / id.semantic_str().len() {
        assert_eq!(id.to_string(), id.semantic_str());
    }

    assert!(matches!(
        write_svg(&scene, id, &meter),
        Err(Error::ResourceLimitExceeded(_))
    ));
    assert!(
        matches!(projection.finish(), Err(Error::ResourceLimitExceeded(error)) if error.limit == "max_svg_bytes")
    );
}

#[test]
fn compact_scene_work_and_elements_use_the_existing_meter() {
    with_sequence_paint(|scene| {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let reference_meter = OperationWorkMeter::new(policy);
        write_svg(scene, scope(), &reference_meter).unwrap();
        let emit_cost = reference_meter.used();
        let meter = OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, emit_cost)
                .unwrap(),
        );
        meter.charge(1).unwrap();
        assert!(matches!(
            write_svg(scene, scope(), &meter),
            Err(Error::ResourceLimitExceeded(error)) if error.limit == "max_layout_work_units"
        ));

        let meter = OperationWorkMeter::new(
            policy
                .with_limit(ResourceLimitId::MaxSvgElements, 11)
                .unwrap(),
        );
        assert!(matches!(
            write_svg(scene, scope(), &meter),
            Err(Error::ResourceLimitExceeded(error))
                if error.limit == "max_svg_elements" && error.actual == 12
        ));
    });
}

#[test]
fn compact_scene_observes_cancellation_before_validation_and_inside_a_path() {
    let control = OperationControl::new();
    control.cancel();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let meter = OperationWorkMeter::new_with_control(policy, control);
    let mut invalid_view = scene(&[], &[]);
    invalid_view.viewport.view_box.width = f64::NAN;
    assert!(matches!(
        write_svg(&invalid_view, scope(), &meter),
        Err(Error::Cancelled(error)) if error.phase == OperationPhase::Emit
    ));
    assert_eq!(meter.used(), 0);

    let control = OperationControl::new();
    let meter = OperationWorkMeter::new_with_control(policy, control.clone());
    let mut writer = SceneSvgWriter {
        out: BoundedSvgOutput::new(&meter),
        meter: &meter,
        scope: scope(),
        marker_count: 0,
        elements: 0,
    };
    writer.emit(format_args!("<svg>")).unwrap();
    let mut commands = [ScenePathCommand::LineTo([1.0, 1.0]); 130];
    commands[0] = ScenePathCommand::MoveTo([0.0, 0.0]);
    control.cancel_after_checkpoints(2);
    assert!(matches!(
        writer.path_open(&commands),
        Err(Error::Cancelled(error)) if error.phase == OperationPhase::Emit
    ));
    assert_eq!(writer.out.as_str().matches('L').count(), 63);
}
