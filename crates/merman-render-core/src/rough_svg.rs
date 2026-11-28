use crate::number_format::canonical_number as number;
use std::fmt::Write as _;

mod canonical_edge;
pub use canonical_edge::constrained_rough_edge_path;
#[cfg(feature = "layout-shoal")]
pub use canonical_edge::constrained_rough_edge_with_geometry;
pub mod controlled;
#[cfg(feature = "svg-edge-parity")]
#[doc(hidden)]
pub mod paths;
#[doc(hidden)]
pub use roughr::core::RoughRandomness;

/// Selects the RoughJS numeric seed and the operation's dedicated fallback random stream.
pub fn operation_randomness(
    operation: &merman_model::runtime::OperationContext,
    configured_seed: f64,
    owner_domain: &str,
) -> RoughRandomness {
    use roughr::core::{RoughJsSeed, RoughMathRandom};

    let resolved_seed = if configured_seed == 0.0 {
        operation.derive_nonzero_u64("render.root", 0).get() as f64
    } else {
        configured_seed
    };
    RoughRandomness::new(
        RoughJsSeed::new(resolved_seed),
        RoughMathRandom::new(operation.derive_u64(owner_domain, 0)),
    )
}
mod geometry;

pub use geometry::{
    RoughRectSpec, append_minor_circular_arc, closed_path_d_from_points, mermaid_arc_points,
    roughjs_circle_operations, roughjs_circle_path_d, roughjs_paths_for_rect,
    roughjs_rect_operations,
};

pub fn parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
    let paint = s.trim();
    let rgba = if paint.eq_ignore_ascii_case("none") {
        return Some(roughr::Srgba::new(0.0, 0.0, 0.0, 0.0));
    } else {
        merman_model::css_color::parse(paint)?.rgba
    };
    Some(roughr::Srgba::new(
        (rgba.red / 255.0) as f32,
        (rgba.green / 255.0) as f32,
        (rgba.blue / 255.0) as f32,
        rgba.alpha as f32,
    ))
}

pub fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
    let mut out = String::new();
    for op in &opset.ops {
        match op.op {
            roughr::core::OpType::Move => {
                let _ = write!(&mut out, "M{} {} ", number(op.data[0]), number(op.data[1]));
            }
            roughr::core::OpType::BCurveTo => {
                let _ = write!(
                    &mut out,
                    "C{} {}, {} {}, {} {} ",
                    number(op.data[0]),
                    number(op.data[1]),
                    number(op.data[2]),
                    number(op.data[3]),
                    number(op.data[4]),
                    number(op.data[5]),
                );
            }
            roughr::core::OpType::LineTo => {
                let _ = write!(&mut out, "L{} {} ", number(op.data[0]), number(op.data[1]));
            }
        }
    }
    // Each operation appends one trailing ASCII space.
    out.pop();
    out
}

/// Serialize native rough operations to hundredths of a pixel.
#[cfg(feature = "svg-edge-parity")]
pub fn ops_to_compact_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
    use crate::svg_edge_parity::path_numbers::fmt_path_into;
    use roughr::core::OpType;

    let mut out = String::new();
    for op in &opset.ops {
        let command = match op.op {
            OpType::Move => 'M',
            OpType::BCurveTo => 'C',
            OpType::LineTo => 'L',
        };
        out.push(command);
        for (index, &coordinate) in op.data.iter().enumerate() {
            if index != 0 {
                out.push(' ');
            }
            if coordinate.abs() < i64::MAX as f64 / 1000.0 {
                fmt_path_into(&mut out, (coordinate * 100.0).round() / 100.0);
            } else {
                let _ = write!(out, "{}", number(coordinate));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use roughr::core::{Op, OpSet, OpSetType, OpType};

    #[test]
    fn rough_path_coordinates_share_canonical_svg_number_format() {
        let operations = OpSet {
            op_set_type: OpSetType::Path,
            ops: vec![
                Op {
                    op: OpType::Move,
                    data: vec![-0.0, 2.0 + 1e-8],
                },
                Op {
                    op: OpType::BCurveTo,
                    data: vec![1e-10, 2.5, -3.0, 4.0 - 1e-8, 5.123456, 6.0],
                },
                Op {
                    op: OpType::LineTo,
                    data: vec![7.0 + 1e-8, 8.0],
                },
            ],
            size: None,
            path: None,
        };
        assert_eq!(
            ops_to_svg_path_d(&operations),
            "M0 2 C0 2.5, -3 4, 5.123456 6 L7 8"
        );
        #[cfg(feature = "svg-edge-parity")]
        assert_eq!(
            ops_to_compact_svg_path_d(&operations),
            "M0 2C0 2.5 -3 4 5.12 6L7 8"
        );
    }
}
