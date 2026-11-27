use std::fmt::Write as _;

mod constrained;
pub mod controlled;

pub use constrained::constrained_rough_edge_path;
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
    let hex = s.trim().strip_prefix('#')?;
    // Validate ASCII hex digits before indexing bytes. Theme values are arbitrary CSS strings;
    // malformed or non-ASCII values must be rejected without slicing at a non-character boundary.
    if !matches!(hex.len(), 3 | 6) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |pair: &str| u8::from_str_radix(pair, 16).ok();
    let (r, g, b) = if hex.len() == 6 {
        (byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?)
    } else {
        (
            byte(&hex[0..1].repeat(2))?,
            byte(&hex[1..2].repeat(2))?,
            byte(&hex[2..3].repeat(2))?,
        )
    };
    Some(roughr::Srgba::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
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
