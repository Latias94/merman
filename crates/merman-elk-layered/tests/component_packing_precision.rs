use merman_elk_layered::NoopWorkControl;
use merman_elk_layered::algorithms::{box_layout, force, mrtree, stress};
use merman_elk_layered::{
    ElkDirection, ElkInputGraph, ElkInputNode, HierarchyHandling, LayeredOptions, NodeFlexibility,
    NodeLabelPlacement, execute_ported_processors, import_graph,
};

#[test]
fn layered_components_keep_worker_double_precision_at_row_boundaries() {
    let mut options = LayeredOptions {
        direction: ElkDirection::Right,
        hierarchy_handling: HierarchyHandling::SeparateChildren,
        aspect_ratio: 1.6_f32 as f64,
        ..Default::default()
    };
    options.spacing.component_component = 20.0;
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".into(),
        options,
        nodes: ["a", "b"]
            .into_iter()
            .map(|id| ElkInputNode {
                id: id.into(),
                width: 40.0000009,
                height: 48.828125,
                parent: None,
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                port_constraints: None,
                port_alignment: None,
                node_label_placement: NodeLabelPlacement::Fixed,
                node_flexibility: NodeFlexibility::None,
                ports_surrounding: None,
                nested_options: None,
                label: None,
            })
            .collect(),
        edges: vec![],
    })
    .unwrap();
    execute_ported_processors(&mut graph).unwrap();
    let size = graph.exported_root_size();
    assert_eq!(size.width, 124.0000018);
    assert_eq!(size.height, 72.828125);
    assert_eq!(
        graph.layerless_nodes[0].position.y,
        graph.layerless_nodes[1].position.y
    );
}

#[test]
fn force_and_stress_keep_worker_double_precision_at_component_row_boundaries() {
    for width in [40.0000005, 40.0000015] {
        let nodes = [force::Node {
            width,
            height: 125.0,
            ..Default::default()
        }; 2];
        let force = force::layout(&nodes, &[], &Default::default(), &mut NoopWorkControl)
            .expect("valid disconnected components");
        let stress_nodes = nodes.map(|geometry| stress::Node {
            geometry,
            fixed: false,
        });
        let stress = stress::layout(
            &stress_nodes,
            &[],
            &Default::default(),
            &mut NoopWorkControl,
        )
        .expect("valid disconnected components");
        for output in [force, stress] {
            assert_eq!(output.height, 225.0);
            assert_eq!(output.width, 180.0 + 2.0 * width);
            assert_eq!(output.nodes[0].x, 50.0);
            assert_eq!(output.nodes[1].x, 130.0 + width);
            assert_eq!(output.nodes[0].y, 50.0);
            assert_eq!(output.nodes[1].y, 50.0);
        }
    }
}

#[test]
fn mrtree_keeps_worker_double_precision_at_component_row_boundaries() {
    for (width, height) in [
        (40.0000005, 125.0),
        (40.0000015, 125.0),
        (40.0000009, 48.828125),
    ] {
        let nodes = vec![
            mrtree::Node {
                width,
                height,
                ..Default::default()
            };
            2
        ];
        let output = mrtree::layout(&nodes, &[], &Default::default(), &mut NoopWorkControl)
            .expect("valid disconnected components");
        assert_eq!(output.height, 40.0 + height);
        assert_eq!(output.width, 60.0 + 2.0 * width);
        assert_eq!(output.nodes[0].x, 20.0);
        assert!((output.nodes[1].x - (40.0 + width)).abs() < 1e-12);
        assert_eq!(output.nodes[0].y, 20.0);
        assert_eq!(output.nodes[1].y, 20.0);
    }
}

#[test]
fn box_default_preserves_the_worker_float_literal_without_rounding_runtime_values() {
    assert_eq!(box_layout::Options::default().aspect_ratio, 1.3_f32 as f64);
    let height = 86.77884615384616;
    let rectangles = [box_layout::Rectangle {
        width: 40.0,
        height,
        ..Default::default()
    }; 2];
    for (aspect_ratio, expected_width, expected_height) in [
        (
            box_layout::Options::default().aspect_ratio,
            40.0,
            2.0 * height + 15.0,
        ),
        (1.3, 95.0, height),
    ] {
        let options = box_layout::Options {
            aspect_ratio,
            padding: box_layout::Padding {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            ..Default::default()
        };
        let output = box_layout::layout(&rectangles, &options, &mut NoopWorkControl).unwrap();
        assert_eq!(output.width, expected_width);
        assert_eq!(output.height, expected_height);
    }
}
