use std::fmt::Write as _;

pub mod controlled;

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
