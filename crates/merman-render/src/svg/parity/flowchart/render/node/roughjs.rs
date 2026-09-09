//! RoughJS-compatible path generation helpers (via `roughr`).
//!
//! Mermaid uses RoughJS for "hand-drawn" flowchart node rendering, and in a few cases even when
//! roughness is zero. These helpers mirror Mermaid's RoughJS call ordering to keep SVG DOM parity.

use crate::resources::OperationWorkMeter;
use crate::svg::parity::roughjs_common::{ops_to_svg_path_d, parse_hex_color_to_srgba};
use merman_core::OperationPhase;
use roughr::core::RoughRandomness;

// Mermaid's `userNodeOverrides(...)` defaults for hand-drawn flowchart shapes.
// Keep these in one place so every object shape uses the same RoughJS contract.
const HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const HAND_DRAWN_FILL_WEIGHT: f32 = 4.0;
const HAND_DRAWN_HACHURE_GAP: f32 = 5.2;

// These estimates intentionally describe the allocations that RoughJS performs before the SVG
// sink can observe the result: points sampled along the path plus hachure scanlines. They are
// admission bounds, not output-size claims. The final bounded SVG sink remains authoritative.
const HAND_DRAWN_WORK_UNITS_PER_POINT: usize = 4;
const HAND_DRAWN_WORK_UNITS_PER_SCANLINE: usize = 8;
const HAND_DRAWN_SVG_BYTES_PER_POINT: usize = 32;
const HAND_DRAWN_SVG_BYTES_PER_SCANLINE: usize = 96;
const HAND_DRAWN_SVG_BYTES_OVERHEAD: usize = 512;

#[derive(Debug, Clone, Copy)]
struct HandDrawnPathBudget {
    work_units: usize,
    svg_bytes: usize,
}

fn ceil_to_usize(value: f64) -> Option<usize> {
    if !value.is_finite() || value < 0.0 || value > usize::MAX as f64 {
        return None;
    }
    Some(value.ceil() as usize)
}

fn hand_drawn_geometry_budget(
    path_length: f64,
    width: f64,
    height: f64,
    hachure_gap: f32,
    roughness: f32,
    path_bytes: usize,
) -> Option<HandDrawnPathBudget> {
    if !path_length.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || !roughness.is_finite()
    {
        return None;
    }

    // RoughR's path sampler uses `(1 + roughness) / 2` as its simplification distance. Use the
    // same lower bound here so the admission estimate does not undercount sampled points when a
    // caller selects a smoother path.
    let sample_distance = ((1.0 + f64::from(roughness.max(0.0))) / 2.0).max(0.1);
    let point_count = ceil_to_usize(path_length.max(1.0) / sample_distance)?.checked_add(1)?;
    let diagonal = width.abs().hypot(height.abs()).max(1.0);
    let gap = f64::from(hachure_gap.abs()).max(1.0);
    let scanline_count = ceil_to_usize(diagonal / gap)?.checked_add(2)?;

    let work_units = path_bytes
        .checked_add(point_count.checked_mul(HAND_DRAWN_WORK_UNITS_PER_POINT)?)?
        .checked_add(scanline_count.checked_mul(HAND_DRAWN_WORK_UNITS_PER_SCANLINE)?)?;
    let svg_bytes = HAND_DRAWN_SVG_BYTES_OVERHEAD
        .checked_add(path_bytes.checked_mul(4)?)?
        .checked_add(point_count.checked_mul(HAND_DRAWN_SVG_BYTES_PER_POINT)?)?
        .checked_add(scanline_count.checked_mul(HAND_DRAWN_SVG_BYTES_PER_SCANLINE)?)?;

    Some(HandDrawnPathBudget {
        work_units,
        svg_bytes,
    })
}

fn admit_hand_drawn_geometry(
    path_length: f64,
    width: f64,
    height: f64,
    hachure_gap: f32,
    roughness: f32,
    path_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> Option<crate::resources::SvgByteReservation> {
    work_meter.checkpoint(OperationPhase::Emit).ok()?;
    let budget = hand_drawn_geometry_budget(
        path_length,
        width,
        height,
        hachure_gap,
        roughness,
        path_bytes,
    )?;
    work_meter.charge_emit_work(budget.work_units).ok()?;

    let reservation = work_meter.reserve_svg_bytes_up_to(budget.svg_bytes).ok()?;
    if let Some(error) = reservation.limit_error {
        let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
        let _ = work_meter.terminate_absolute_resource_error(error, OperationPhase::Emit);
        return None;
    }
    Some(reservation)
}

fn admit_hand_drawn_path(
    path_data: &str,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
) -> Option<crate::resources::SvgByteReservation> {
    let bounds = crate::svg::parity::path_bounds::svg_path_bounds_from_d(path_data)?;
    let path_length = crate::svg::parity::path_bounds::svg_path_length_from_d(path_data)?;
    admit_hand_drawn_geometry(
        path_length,
        bounds.max_x - bounds.min_x,
        bounds.max_y - bounds.min_y,
        hachure_gap,
        roughness,
        path_data.len(),
        work_meter,
    )
}

fn admit_hand_drawn_rect(
    width: f64,
    height: f64,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
) -> Option<crate::resources::SvgByteReservation> {
    admit_hand_drawn_geometry(
        2.0 * (width.abs() + height.abs()),
        width,
        height,
        hachure_gap,
        roughness,
        64,
        work_meter,
    )
}

fn admit_hand_drawn_circle(
    diameter: f64,
    hachure_gap: f32,
    work_meter: &OperationWorkMeter,
) -> Option<crate::resources::SvgByteReservation> {
    admit_hand_drawn_geometry(
        std::f64::consts::PI * diameter.abs(),
        diameter,
        diameter,
        hachure_gap,
        HAND_DRAWN_ROUGHNESS,
        32,
        work_meter,
    )
}

fn reconcile_hand_drawn_path(
    work_meter: &OperationWorkMeter,
    reservation: crate::resources::SvgByteReservation,
    paths: Option<(String, String)>,
) -> Option<(String, String)> {
    let Some((fill_d, stroke_d)) = paths else {
        let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
        return None;
    };
    let actual_bytes = fill_d.len().checked_add(stroke_d.len())?;
    if work_meter
        .reconcile_svg_bytes(reservation.additional_bytes, actual_bytes)
        .is_err()
    {
        return None;
    }
    (!fill_d.is_empty() && !stroke_d.is_empty()).then_some((fill_d, stroke_d))
}

pub(in crate::svg::parity) use crate::svg::parity::roughjs_common::{
    RoughRectSpec, roughjs_circle_path_d, roughjs_paths_for_rect,
};

fn parse_stroke_dash_pair(stroke_dasharray: &str) -> (f64, f64) {
    let dash = stroke_dasharray.trim().replace(',', " ");
    let mut nums = dash
        .split_whitespace()
        .filter_map(|t| t.parse::<f32>().ok());
    match (nums.next(), nums.next()) {
        (Some(a), Some(b)) => (a as f64, b as f64),
        (Some(a), None) => (a as f64, a as f64),
        _ => (0.0, 0.0),
    }
}

pub(in crate::svg::parity) fn roughjs_paths_for_svg_path(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let base_options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .bowing(1.0)
        .fill(fill)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

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

pub(in crate::svg::parity) fn roughjs_paths_for_svg_path_single_set(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    // Variant of `roughjs_paths_for_svg_path(...)` that always takes RoughJS' `sets.length === 1`
    // branch (fill path via `svgPath(...)` with `disableMultiStroke=true`), avoiding the
    // `pointsOnPath(...)` step which can overflow on complex paths.
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let base_options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .bowing(1.0)
        .fill(fill)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    // Keep call ordering consistent with RoughJS: stroke pass advances the PRNG before fill.
    let mut stroke_opts = base_options.clone();
    let stroke_opset =
        roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut stroke_opts);

    let mut fill_opts = stroke_opts.clone();
    fill_opts.disable_multi_stroke = Some(true);
    let base_rough = fill_opts.roughness.unwrap_or(1.0);
    fill_opts.roughness = Some(if base_rough != 0.0 {
        base_rough + 0.8
    } else {
        0.0
    });
    let mut fill_opset =
        roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut fill_opts);
    fill_opset.ops = fill_opset
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

    Some((
        ops_to_svg_path_d(&fill_opset),
        ops_to_svg_path_d(&stroke_opset),
    ))
}

pub(in crate::svg::parity) fn roughjs_stroke_path_for_svg_path(
    svg_path_data: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    randomness: &RoughRandomness,
) -> Option<String> {
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .bowing(1.0)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    let opset = roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut options);
    Some(ops_to_svg_path_d(&opset))
}

pub(in crate::svg::parity) fn roughjs_hand_drawn_stroke_path_for_svg_path(
    svg_path_data: &str,
    roughness: f32,
    randomness: &RoughRandomness,
) -> Option<String> {
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    let opset = roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut options);
    Some(ops_to_svg_path_d(&opset))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::svg::parity) fn roughjs_hachure_paths_for_svg_path(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    fill_weight: f32,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    let fill =
        parse_hex_color_to_srgba(fill).unwrap_or_else(|| roughr::Srgba::new(0.0, 0.0, 0.0, 1.0));
    let stroke =
        parse_hex_color_to_srgba(stroke).unwrap_or_else(|| roughr::Srgba::new(0.0, 0.0, 0.0, 1.0));
    let reservation = admit_hand_drawn_path(svg_path_data, hachure_gap, roughness, work_meter)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .fill(fill)
        .fill_style(roughr::core::FillStyle::Hachure)
        .fill_weight(fill_weight)
        .hachure_gap(hachure_gap)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generator = roughr::generator::Generator::default();
    let drawable = generator.path::<f64>(svg_path_data.to_string(), &Some(options));
    let mut fill_d = None;
    let mut stroke_d = None;

    for set in drawable.sets {
        let d = ops_to_svg_path_d(&set);
        match set.op_set_type {
            roughr::core::OpSetType::FillPath | roughr::core::OpSetType::FillSketch => {
                fill_d = Some(d);
            }
            roughr::core::OpSetType::Path => {
                stroke_d = Some(d);
            }
        }
    }

    let paths = match (fill_d, stroke_d) {
        (Some(fill_d), Some(stroke_d)) => Some((fill_d, stroke_d)),
        _ => None,
    };
    reconcile_hand_drawn_path(work_meter, reservation, paths)
}

/// Render a path with Mermaid's default hand-drawn node options.
pub(in crate::svg::parity) fn roughjs_paths_for_hand_drawn_svg_path(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    // Object-shape renderers use this helper alongside the hand-drawn circle path. Keep both
    // branches on the same fallback boundary when a CSS color is not representable by roughr.
    parse_hex_color_to_srgba(fill)?;
    parse_hex_color_to_srgba(stroke)?;
    roughjs_hachure_paths_for_svg_path(
        svg_path_data,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
        HAND_DRAWN_FILL_WEIGHT,
        HAND_DRAWN_HACHURE_GAP,
        HAND_DRAWN_ROUGHNESS,
        work_meter,
        randomness,
    )
}

/// Generate a hand-drawn path whose fill remains solid.
///
/// Mermaid uses this variant for small decorations (for example the tag on a tagged rectangle):
/// the outer shape is hatched, while the decoration is a rough solid fill.  Keeping the option
/// choice here avoids making individual shape renderers emulate RoughJS configuration.
pub(in crate::svg::parity) fn roughjs_paths_for_hand_drawn_solid_svg_path(
    svg_path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let reservation = admit_hand_drawn_path(
        svg_path_data,
        HAND_DRAWN_HACHURE_GAP,
        HAND_DRAWN_ROUGHNESS,
        work_meter,
    )?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(HAND_DRAWN_ROUGHNESS)
        .fill(fill)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generator = roughr::generator::Generator::default();
    let drawable = generator.path::<f64>(svg_path_data.to_string(), &Some(options));
    let mut fill_d = None;
    let mut stroke_d = None;

    for set in drawable.sets {
        let d = ops_to_svg_path_d(&set);
        match set.op_set_type {
            roughr::core::OpSetType::FillPath | roughr::core::OpSetType::FillSketch => {
                fill_d = Some(d);
            }
            roughr::core::OpSetType::Path => {
                stroke_d = Some(d);
            }
        }
    }

    let paths = match (fill_d, stroke_d) {
        (Some(fill_d), Some(stroke_d)) => Some((fill_d, stroke_d)),
        _ => None,
    };
    reconcile_hand_drawn_path(work_meter, reservation, paths)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::svg::parity) fn roughjs_hachure_paths_for_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    fill_weight: f32,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    let fill =
        parse_hex_color_to_srgba(fill).unwrap_or_else(|| roughr::Srgba::new(0.0, 0.0, 0.0, 1.0));
    let stroke =
        parse_hex_color_to_srgba(stroke).unwrap_or_else(|| roughr::Srgba::new(0.0, 0.0, 0.0, 1.0));
    let reservation = admit_hand_drawn_rect(w, h, hachure_gap, roughness, work_meter)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .fill(fill)
        .fill_style(roughr::core::FillStyle::Hachure)
        .fill_weight(fill_weight)
        .hachure_gap(hachure_gap)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generator = roughr::generator::Generator::default();
    let drawable = generator.rectangle::<f64>(x, y, w, h, &Some(options));
    let mut fill_d = None;
    let mut stroke_d = None;

    for set in drawable.sets {
        let d = ops_to_svg_path_d(&set);
        match set.op_set_type {
            roughr::core::OpSetType::FillPath | roughr::core::OpSetType::FillSketch => {
                fill_d = Some(d);
            }
            roughr::core::OpSetType::Path => {
                stroke_d = Some(d);
            }
        }
    }

    reconcile_hand_drawn_path(work_meter, reservation, Some((fill_d?, stroke_d?)))
}

/// Render a rectangle with Mermaid's default hand-drawn fill and stroke options.
pub(in crate::svg::parity) fn roughjs_paths_for_hand_drawn_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    // Keep the hand-drawn path and classic fallback on the same admission boundary. The generic
    // rectangle helper intentionally substitutes black for unsupported colors for older callers;
    // a hand-drawn object shape must instead fall back as a whole when either color is not
    // representable by roughr.
    parse_hex_color_to_srgba(fill)?;
    parse_hex_color_to_srgba(stroke)?;
    roughjs_hachure_paths_for_rect(
        x,
        y,
        w,
        h,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
        HAND_DRAWN_FILL_WEIGHT,
        HAND_DRAWN_HACHURE_GAP,
        HAND_DRAWN_ROUGHNESS,
        work_meter,
        randomness,
    )
}

pub(in crate::svg::parity) fn roughjs_hand_drawn_line_path(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    randomness: &RoughRandomness,
) -> Option<String> {
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(HAND_DRAWN_ROUGHNESS)
        .bowing(1.0)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    Some(ops_to_svg_path_d(&roughr::renderer::line::<f64>(
        x1,
        y1,
        x2,
        y2,
        &mut options,
    )))
}

pub(in crate::svg::parity) fn roughjs_paths_for_polygon(
    points: &[(f64, f64)],
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    // Mirror RoughJS `generator.polygon(...)` generation order: outline first, then fill, then
    // emit fill before outline.
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let mut opts = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
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

    Some((
        ops_to_svg_path_d(&fill_opset),
        ops_to_svg_path_d(&outline_opset),
    ))
}

pub(in crate::svg::parity) fn roughjs_paths_for_circle(
    diameter: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn: bool,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    let fill = parse_hex_color_to_srgba(fill)?;
    let stroke = parse_hex_color_to_srgba(stroke)?;
    let reservation = hand_drawn
        .then(|| admit_hand_drawn_circle(diameter, HAND_DRAWN_HACHURE_GAP, work_meter))
        .flatten();
    if hand_drawn && reservation.is_none() {
        return None;
    }
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(if hand_drawn {
            HAND_DRAWN_ROUGHNESS
        } else {
            0.0
        })
        .fill(fill)
        .fill_style(if hand_drawn {
            roughr::core::FillStyle::Hachure
        } else {
            roughr::core::FillStyle::Solid
        })
        .fill_weight(HAND_DRAWN_FILL_WEIGHT)
        .hachure_gap(HAND_DRAWN_HACHURE_GAP)
        .stroke(stroke)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generator = roughr::generator::Generator::default();
    let drawable = generator.circle::<f64>(0.0, 0.0, diameter, &Some(options.clone()));
    let mut fill_d = None;
    let mut stroke_d = None;

    for set in drawable.sets {
        let d = ops_to_svg_path_d(&set);
        match set.op_set_type {
            roughr::core::OpSetType::FillPath | roughr::core::OpSetType::FillSketch => {
                fill_d = Some(d);
            }
            roughr::core::OpSetType::Path => {
                stroke_d = Some(d);
            }
        }
    }

    let paths = match (fill_d, stroke_d) {
        (Some(fill_d), Some(stroke_d)) => Some((fill_d, stroke_d)),
        _ => None,
    };
    match (hand_drawn, reservation) {
        (true, Some(reservation)) => reconcile_hand_drawn_path(work_meter, reservation, paths),
        (false, None) => paths,
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        OperationWorkError, RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase,
    };
    use merman_core::OperationControl;

    fn hand_drawn_path_budget(
        path_data: &str,
        hachure_gap: f32,
        roughness: f32,
    ) -> Option<HandDrawnPathBudget> {
        let bounds = crate::svg::parity::path_bounds::svg_path_bounds_from_d(path_data)?;
        let path_length = crate::svg::parity::path_bounds::svg_path_length_from_d(path_data)?;
        let width = (bounds.max_x - bounds.min_x).abs();
        let height = (bounds.max_y - bounds.min_y).abs();
        hand_drawn_geometry_budget(
            path_length,
            width,
            height,
            hachure_gap,
            roughness,
            path_data.len(),
        )
    }

    fn randomness() -> RoughRandomness {
        RoughRandomness::new(
            roughr::core::RoughJsSeed::new(1.0),
            roughr::core::RoughMathRandom::new(7),
        )
    }

    fn path() -> &'static str {
        "M0 0 H1000000 V1000000 H0 Z"
    }

    fn assert_limit(error: OperationWorkError, limit: &str, phase: ResourceLimitPhase) {
        let OperationWorkError::ResourceLimitExceeded(details) = error else {
            panic!("expected resource rejection");
        };
        assert_eq!(details.limit, limit);
        assert_eq!(details.phase, phase);
    }

    #[test]
    fn hand_drawn_hachure_rejects_before_roughr_at_work_ceiling() {
        let budget = hand_drawn_path_budget(path(), HAND_DRAWN_HACHURE_GAP, HAND_DRAWN_ROUGHNESS)
            .expect("fixture path must have a finite budget");
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, budget.work_units - 1)
            .expect("valid work ceiling");
        let meter = OperationWorkMeter::new(policy);

        assert!(
            roughjs_paths_for_hand_drawn_svg_path(
                path(),
                "#ffffff",
                "#000000",
                1.0,
                "0 0",
                &meter,
                &randomness(),
            )
            .is_none()
        );
        assert_limit(
            meter
                .checkpoint(OperationPhase::Emit)
                .expect_err("work ceiling must become terminal"),
            ResourceLimitId::MaxLayoutWorkUnits.as_str(),
            ResourceLimitPhase::LayoutModel,
        );
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn hand_drawn_hachure_rejects_before_roughr_at_svg_ceiling() {
        let budget = hand_drawn_path_budget(path(), HAND_DRAWN_HACHURE_GAP, HAND_DRAWN_ROUGHNESS)
            .expect("fixture path must have a finite budget");
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, budget.svg_bytes - 1)
            .expect("valid SVG ceiling");
        let meter = OperationWorkMeter::new(policy);

        assert!(
            roughjs_paths_for_hand_drawn_svg_path(
                path(),
                "#ffffff",
                "#000000",
                1.0,
                "0 0",
                &meter,
                &randomness(),
            )
            .is_none()
        );
        assert_limit(
            meter
                .checkpoint(OperationPhase::Emit)
                .expect_err("SVG ceiling must become terminal"),
            ResourceLimitId::MaxSvgBytes.as_str(),
            ResourceLimitPhase::SvgOutput,
        );
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn hand_drawn_hachure_replays_cancellation_before_generation() {
        let control = OperationControl::new();
        control.cancel();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );

        assert!(
            roughjs_paths_for_hand_drawn_svg_path(
                "M0 0 H80 V40 H0 Z",
                "#ffffff",
                "#000000",
                1.0,
                "0 0",
                &meter,
                &randomness(),
            )
            .is_none()
        );
        assert!(matches!(
            meter.checkpoint(OperationPhase::Emit),
            Err(OperationWorkError::Cancelled(_))
        ));
    }

    #[test]
    fn hand_drawn_hachure_reconciles_a_bounded_success() {
        let path = "M0 0 H80 V40 H0 Z";
        let budget = hand_drawn_path_budget(path, HAND_DRAWN_HACHURE_GAP, HAND_DRAWN_ROUGHNESS)
            .expect("fixture path must have a finite budget");
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, budget.work_units)
            .unwrap()
            .with_limit(ResourceLimitId::MaxSvgBytes, budget.svg_bytes)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);

        let (fill_d, stroke_d) = roughjs_paths_for_hand_drawn_svg_path(
            path,
            "#ffffff",
            "#000000",
            1.0,
            "0 0",
            &meter,
            &randomness(),
        )
        .expect("bounded ordinary path must remain renderable");
        assert!(!fill_d.is_empty());
        assert!(!stroke_d.is_empty());
        assert!(meter.projected_svg_bytes() <= budget.svg_bytes);
        meter
            .checkpoint(OperationPhase::Emit)
            .expect("successful path must not poison the operation");
    }
}
