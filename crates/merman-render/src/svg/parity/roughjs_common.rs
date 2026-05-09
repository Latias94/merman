//! Shared RoughJS formatting helpers used by multiple parity renderers.

use std::fmt::Write as _;

pub(in crate::svg::parity) fn parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
    let s = s.trim();
    let hex = s.strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b)
        }
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some(roughr::Srgba::new(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    ))
}

pub(in crate::svg::parity) fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
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
    out.trim_end().to_string()
}
