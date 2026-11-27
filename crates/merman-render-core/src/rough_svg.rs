use std::fmt::Write as _;

mod constrained;
pub mod controlled;

pub use constrained::constrained_rough_edge_path;
#[cfg(feature = "layout-shoal")]
pub use constrained::constrained_rough_edge_with_geometry;
#[cfg(feature = "svg-edge-parity")]
#[doc(hidden)]
pub mod paths;

/// Selects the RoughJS numeric seed and the operation's dedicated fallback random stream.
pub fn operation_randomness(
    operation: &merman_model::runtime::OperationContext,
    configured_seed: f64,
    owner_domain: &str,
) -> roughr::core::RoughRandomness {
    use roughr::core::{RoughJsSeed, RoughMathRandom, RoughRandomness};

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
    RoughRectSpec, closed_path_d_from_points, mermaid_arc_points, roughjs_circle_operations,
    roughjs_circle_path_d, roughjs_paths_for_rect, roughjs_rect_operations,
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
                let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
            }
            roughr::core::OpType::BCurveTo => {
                let _ = write!(
                    &mut out,
                    "C{} {}, {} {}, {} {} ",
                    op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                );
            }
            roughr::core::OpType::LineTo => {
                let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
            }
        }
    }
    // Each operation appends one trailing ASCII space.
    out.pop();
    out
}
