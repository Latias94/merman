#[cfg(test)]
mod tests {
    use merman_render_core::svg_edge_parity::svg_path_length_from_d;

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    #[test]
    fn rough_operation_bounds_match_serialized_path_bounds() {
        use merman_render_core::rough_svg::{
            ops_to_svg_path_d, paths::svg_path_bounds_from_rough_ops,
        };
        use merman_render_core::svg_bounds::{SvgPathBounds, svg_path_bounds_from_d};
        use roughr::core::{Op, OpSet, OpSetType, OpType};

        let opset = OpSet {
            op_set_type: OpSetType::Path,
            ops: vec![
                Op {
                    op: OpType::Move,
                    data: vec![-0.0, 0.1],
                },
                Op {
                    op: OpType::BCurveTo,
                    data: vec![0.6, 19.0, 3.0, -15.0, 1.1, 2.0],
                },
                Op {
                    op: OpType::Move,
                    data: vec![1.2, 2.2250738585072014e-308],
                },
                Op {
                    op: OpType::LineTo,
                    data: vec![2.1, 0.5],
                },
            ],
            size: None,
            path: None,
        };
        let expected = svg_path_bounds_from_d(&ops_to_svg_path_d(&opset)).unwrap();
        let actual = svg_path_bounds_from_rough_ops(&opset).unwrap();
        let coordinates = |bounds: SvgPathBounds| {
            [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y].map(f64::to_bits)
        };
        assert_eq!(coordinates(actual), coordinates(expected));
    }

    #[test]
    fn total_length_uses_svg_dom_float_precision() {
        assert_eq!(
            svg_path_length_from_d("M0,0L0.1,0"),
            Some(0.10000000149011612)
        );
        assert_eq!(svg_path_length_from_d("M0,0L1,1"), Some(1.4142135381698608));
    }

    #[test]
    fn neo_marker_mask_length_is_stable_across_platforms() {
        // The 12.1 new-shapes fixture's n11 -> n22 edge differs by one f64 ULP on Linux/Windows.
        let d = "M214.9,268.158L214.9,287.23L214.9,300.159Q214.9,307.23 221.971,307.23L293.662,307.23Q300.733,307.23 300.733,314.301L300.733,343.162Q300.733,347.23 304.801,347.23L308.87,347.23";
        let length = svg_path_length_from_d(d).unwrap();
        assert_eq!(length, 166.18093872070312);
        let mut style = String::new();
        merman_render_core::svg_edge_parity::edge_path::write_neo_edge_mask(
            &mut style,
            length,
            None,
            Some("arrow_point"),
            false,
            false,
        );
        assert_eq!(
            style,
            "stroke-dasharray: 0 0 162.18093872070313 4; stroke-dashoffset: 0;"
        );
    }
}
