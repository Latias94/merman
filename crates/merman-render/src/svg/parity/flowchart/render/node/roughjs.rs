//! RoughJS-compatible path generation helpers (via `roughr`).
//!
//! Mermaid uses RoughJS for "hand-drawn" flowchart node rendering, and in a few cases even when
//! roughness is zero. These helpers mirror Mermaid's RoughJS call ordering to keep SVG DOM parity.

use std::fmt::Write as _;

fn parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
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

pub(super) fn roughjs_paths_for_svg_path(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    seed: u64,
) -> Option<(String, String)> {
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let dash = stroke_dasharray.trim().replace(',', " ");
    let nums: Vec<f32> = dash
        .split_whitespace()
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    let (dash0, dash1) = match nums.as_slice() {
        [a] => (*a, *a),
        [a, b, ..] => (*a, *b),
        _ => (0.0, 0.0),
    };
    let base_options = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .bowing(1.0)
        .fill(fill)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0 as f64, dash1 as f64])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    // Rough.js' generator emits path data via `opsToPath(...)`, which uses `Number.toString()`
    // precision (not Mermaid's usual 3-decimal `fmt(...)` formatting). Avoid quantization here.
    fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
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

    // Rough.js `generator.path(...)`:
    // - `sets = pointsOnPath(d, 1, distance)`
    // - for solid fill, if `sets.length === 1`: fill path from `svgPath(...)` with
    //   `disableMultiStroke: true`, then drop subsequent `move` ops (`_mergedShape`).
    // - otherwise for solid fill: `solidFillPolygon(sets, o)`
    let distance = (1.0 + base_options.roughness.unwrap_or(1.0) as f64) / 2.0;
    let sets = roughr::points_on_path::points_on_path::<f64>(
        svg_path_data.to_string(),
        Some(1.0),
        Some(distance),
    );

    // Rough.js `generator.path(...)` builds the stroke opset first (`shape = svgPath(d, o)`),
    // which initializes and advances `o.randomizer`. For the solid-fill special-case
    // (`sets.length === 1`), it then calls `svgPath(d, Object.assign({}, o, ...))`, which
    // copies the *existing* `randomizer` by reference and therefore continues the PRNG stream.
    //
    // In headless Rust we model that by emitting the stroke opset first (advancing the
    // in-options PRNG state), then cloning the mutated options for the fill pass.
    let mut stroke_opts = base_options.clone();
    let stroke_opset =
        roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut stroke_opts);

    let fill_opset = if sets.len() == 1 {
        let mut fill_opts = stroke_opts.clone();
        fill_opts.disable_multi_stroke = Some(true);
        let base_rough = fill_opts.roughness.unwrap_or(1.0);
        fill_opts.roughness = Some(if base_rough != 0.0 {
            base_rough + 0.8
        } else {
            0.0
        });

        let mut opset =
            roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut fill_opts);
        opset.ops = opset
            .ops
            .iter()
            .cloned()
            .enumerate()
            .filter_map(|(idx, op)| {
                if idx != 0 && op.op == roughr::core::OpType::Move {
                    return None;
                }
                Some(op)
            })
            .collect();
        opset
    } else {
        let mut fill_opts = stroke_opts.clone();
        roughr::renderer::solid_fill_polygon(&sets, &mut fill_opts)
    };

    Some((
        ops_to_svg_path_d(&fill_opset),
        ops_to_svg_path_d(&stroke_opset),
    ))
}

pub(super) fn roughjs_stroke_path_for_svg_path(
    svg_path_data: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    seed: u64,
) -> Option<String> {
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let dash = stroke_dasharray.trim().replace(',', " ");
    let nums: Vec<f32> = dash
        .split_whitespace()
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    let (dash0, dash1) = match nums.as_slice() {
        [a] => (*a, *a),
        [a, b, ..] => (*a, *b),
        _ => (0.0, 0.0),
    };
    let mut options = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .bowing(1.0)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0 as f64, dash1 as f64])
        .stroke_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
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

    let opset = roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut options);
    Some(ops_to_svg_path_d(&opset))
}

pub(super) fn roughjs_circle_path_d(diameter: f64, seed: u64) -> Option<String> {
    // Port of Mermaid `stateEnd.ts`/`stateStart.ts` which use RoughJS even for classic look
    // (roughness=0). Use RoughJS `opsToPath(...)` formatting (no `fmt(...)` quantization).
    let mut opts = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;
    let opset = roughr::renderer::ellipse::<f64>(0.0, 0.0, diameter, diameter, &mut opts);
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
    Some(out.trim_end().to_string())
}

pub(super) fn roughjs_paths_for_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    seed: u64,
) -> Option<(String, String)> {
    // Port of Mermaid `forkJoin.ts` generation order: outline first (advancing PRNG), then fill;
    // SVG emission order is fill first, stroke second.
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let mut opts = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .fill(fill)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![0.0, 0.0])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let fill_poly = vec![vec![
        roughr::Point2D::new(x, y),
        roughr::Point2D::new(x + w, y),
        roughr::Point2D::new(x + w, y + h),
        roughr::Point2D::new(x, y + h),
    ]];
    let stroke_opset = roughr::renderer::rectangle::<f64>(x, y, w, h, &mut opts);
    let fill_opset = roughr::renderer::solid_fill_polygon(&fill_poly, &mut opts);

    fn ops_to_d(opset: &roughr::core::OpSet<f64>) -> String {
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

    Some((ops_to_d(&fill_opset), ops_to_d(&stroke_opset)))
}

pub(super) fn roughjs_paths_for_polygon(
    points: &[(f64, f64)],
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    seed: u64,
) -> Option<(String, String)> {
    // Mirror RoughJS `generator.polygon(...)` generation order: outline first, then fill, then
    // emit fill before outline.
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let mut opts = roughr::core::OptionsBuilder::default()
        .seed(seed)
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .fill(fill)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![0.0, 0.0])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let pts: Vec<_> = points
        .iter()
        .copied()
        .map(|(x, y)| roughr::Point2D::new(x, y))
        .collect();
    let outline_opset = roughr::renderer::polygon::<f64>(&pts, &mut opts);
    let fill_opset = roughr::renderer::solid_fill_polygon(&vec![pts.clone()], &mut opts);

    fn ops_to_d(opset: &roughr::core::OpSet<f64>) -> String {
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

    Some((ops_to_d(&fill_opset), ops_to_d(&outline_opset)))
}
