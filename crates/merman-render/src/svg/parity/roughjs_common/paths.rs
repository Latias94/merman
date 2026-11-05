//! RoughJS-compatible path generation helpers (via `roughr`).
//!
//! Mermaid uses RoughJS for "hand-drawn" node rendering, and in a few cases even when
//! roughness is zero. These helpers mirror Mermaid's RoughJS call ordering to keep SVG DOM parity.

use super::controlled;
use crate::resources::OperationWorkMeter;
use crate::svg::parity::roughjs_common::ops_to_svg_path_d;
use merman_model::OperationPhase;
use roughr::core::RoughRandomness;

#[cfg(test)]
#[path = "fixed_head_protocol.rs"]
pub(in crate::svg::parity) mod fixed_head_protocol;

#[derive(Clone, Copy)]
pub(in crate::svg::parity) enum RoughGeometryTarget {
    Svg,
    Query,
}

impl RoughGeometryTarget {
    const fn phase(self) -> OperationPhase {
        match self {
            Self::Svg => OperationPhase::Emit,
            Self::Query => OperationPhase::Layout,
        }
    }
}

pub(in crate::svg::parity) struct RoughPathPair {
    fill: roughr::core::OpSet<f64>,
    stroke: roughr::core::OpSet<f64>,
    reservation: Option<crate::resources::SvgByteReservation>,
}

impl RoughPathPair {
    fn new(fill: roughr::core::OpSet<f64>, stroke: roughr::core::OpSet<f64>) -> Self {
        Self {
            fill,
            stroke,
            reservation: None,
        }
    }

    pub(in crate::svg::parity) fn into_svg_paths(
        self,
        work_meter: &OperationWorkMeter,
    ) -> Option<(String, String)> {
        let paths = Some((
            ops_to_svg_path_d(&self.fill),
            ops_to_svg_path_d(&self.stroke),
        ));
        match self.reservation {
            Some(reservation) => reconcile_hand_drawn_path(work_meter, reservation, paths),
            None => paths,
        }
    }
}

fn reserve_for_target(
    target: RoughGeometryTarget,
    svg_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> Option<Option<crate::resources::SvgByteReservation>> {
    match target {
        RoughGeometryTarget::Svg => reserve_hand_drawn_svg_bytes(svg_bytes, work_meter).map(Some),
        RoughGeometryTarget::Query => Some(None),
    }
}

// Mermaid's `userNodeOverrides(...)` defaults for hand-drawn flowchart shapes.
// Keep these in one place so every object shape uses the same RoughJS contract.
const HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const HAND_DRAWN_FILL_WEIGHT: f32 = 4.0;
const HAND_DRAWN_HACHURE_GAP: f32 = 5.2;
const CIRCLE_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const CIRCLE_HACHURE_GAP: f32 = 1.5;
const CIRCLE_CURVE_STEP_COUNT: f32 = 9.0;
const CIRCLE_CURVE_FITTING: f32 = 0.95;

const HAND_DRAWN_SVG_BYTES_PER_POINT: usize = 32;
const HAND_DRAWN_SVG_BYTES_PER_SCANLINE: usize = 96;
const HAND_DRAWN_SVG_BYTES_OVERHEAD: usize = 512;

#[derive(Debug, Clone, Copy)]
struct HandDrawnPathBudget {
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

    let svg_bytes = HAND_DRAWN_SVG_BYTES_OVERHEAD
        .checked_add(path_bytes.checked_mul(4)?)?
        .checked_add(point_count.checked_mul(HAND_DRAWN_SVG_BYTES_PER_POINT)?)?
        .checked_add(scanline_count.checked_mul(HAND_DRAWN_SVG_BYTES_PER_SCANLINE)?)?;

    Some(HandDrawnPathBudget { svg_bytes })
}

#[allow(
    clippy::too_many_arguments,
    reason = "Geometry admission keeps upstream bounds and sampling inputs explicit with target-specific accounting."
)]
fn admit_hand_drawn_geometry(
    target: RoughGeometryTarget,
    path_length: f64,
    width: f64,
    height: f64,
    hachure_gap: f32,
    roughness: f32,
    path_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> Option<Option<crate::resources::SvgByteReservation>> {
    work_meter.checkpoint(target.phase()).ok()?;
    let budget = hand_drawn_geometry_budget(
        path_length,
        width,
        height,
        hachure_gap,
        roughness,
        path_bytes,
    )?;

    reserve_for_target(target, budget.svg_bytes, work_meter)
}

fn reserve_hand_drawn_svg_bytes(
    svg_bytes: usize,
    work_meter: &OperationWorkMeter,
) -> Option<crate::resources::SvgByteReservation> {
    let reservation = work_meter.reserve_svg_bytes_up_to(svg_bytes).ok()?;
    if let Some(error) = reservation.limit_error {
        let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
        let _ = work_meter.terminate_absolute_resource_error(error, OperationPhase::Emit);
        return None;
    }
    Some(reservation)
}

fn admit_hand_drawn_path(
    target: RoughGeometryTarget,
    path_data: &str,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
) -> Option<Option<crate::resources::SvgByteReservation>> {
    let bounds = crate::svg::parity::path_bounds::svg_path_bounds_from_d(path_data)?;
    let path_length = crate::svg::parity::path_bounds::svg_path_length_from_d(path_data)?;
    admit_hand_drawn_geometry(
        target,
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
    target: RoughGeometryTarget,
    width: f64,
    height: f64,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
) -> Option<Option<crate::resources::SvgByteReservation>> {
    admit_hand_drawn_geometry(
        target,
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
    target: RoughGeometryTarget,
    diameter: f64,
    roughness: f32,
    work_meter: &OperationWorkMeter,
) -> Option<Option<crate::resources::SvgByteReservation>> {
    work_meter.checkpoint(target.phase()).ok()?;
    let budget = hand_drawn_geometry_budget(
        std::f64::consts::PI * diameter.abs(),
        diameter,
        diameter,
        CIRCLE_HACHURE_GAP,
        roughness,
        32,
    )
    .or_else(|| {
        let _ = work_meter.arithmetic_overflow();
        None
    })?;
    reserve_for_target(target, budget.svg_bytes, work_meter)
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
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_svg_path(
        RoughGeometryTarget::Svg,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

pub(in crate::svg::parity) fn roughjs_prepare_paths_for_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    prepare_solid_svg_path(
        target,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
        false,
    )
}

fn prepare_solid_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
    force_single_set: bool,
) -> Option<RoughPathPair> {
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .bowing(1.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, target.phase(), |control| {
        let segments =
            roughr::points_on_path::normalized_path_with_control(svg_path_data, control)?;
        let stroke = roughr::renderer::svg_normalized_segments_with_control::<f64>(
            &segments,
            &mut options,
            control,
        )?;
        let sets = if force_single_set {
            Vec::new()
        } else {
            roughr::points_on_path::points_on_normalized_segments_with_control::<f64>(
                &segments,
                Some(1.0),
                Some(0.5),
                control,
            )?
        };
        let mut fill_options = options.clone();
        let fill = if force_single_set || sets.len() == 1 {
            fill_options.disable_multi_stroke = Some(true);
            let mut fill = roughr::renderer::svg_normalized_segments_with_control::<f64>(
                &segments,
                &mut fill_options,
                control,
            )?;
            let mut retained = 0;
            for index in 0..fill.ops.len() {
                roughr::work::WorkControl::charge(control, 1)?;
                if index == 0 || fill.ops[index].op != roughr::core::OpType::Move {
                    fill.ops.swap(retained, index);
                    retained += 1;
                }
            }
            fill.ops.truncate(retained);
            Ok(fill)
        } else {
            roughr::renderer::solid_fill_polygon_with_control(&sets, &mut fill_options, control)
        }?;
        Ok(RoughPathPair::new(fill, stroke))
    })
    .ok()
}

pub(in crate::svg::parity) fn roughjs_paths_for_svg_path_single_set(
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_svg_path_single_set(
        RoughGeometryTarget::Svg,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

pub(in crate::svg::parity) fn roughjs_prepare_paths_for_svg_path_single_set(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    prepare_solid_svg_path(
        target,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
        true,
    )
}

pub(in crate::svg::parity) fn roughjs_stroke_path_for_svg_path(
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<String> {
    roughjs_prepare_stroke_path_for_svg_path(
        RoughGeometryTarget::Svg,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .map(|opset| ops_to_svg_path_d(&opset))
}

pub(in crate::svg::parity) fn roughjs_prepare_stroke_path_for_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<roughr::core::OpSet<f64>> {
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .bowing(1.0)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, target.phase(), |control| {
        let segments =
            roughr::points_on_path::normalized_path_with_control(svg_path_data, control)?;
        roughr::renderer::svg_normalized_segments_with_control::<f64>(
            &segments,
            &mut options,
            control,
        )
    })
    .ok()
}

pub(in crate::svg::parity) fn roughjs_hand_drawn_stroke_path_for_svg_path(
    svg_path_data: &str,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<String> {
    roughjs_prepare_hand_drawn_stroke_path_for_svg_path(
        RoughGeometryTarget::Svg,
        svg_path_data,
        roughness,
        work_meter,
        randomness,
    )
    .map(|opset| ops_to_svg_path_d(&opset))
}

pub(in crate::svg::parity) fn roughjs_prepare_hand_drawn_stroke_path_for_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<roughr::core::OpSet<f64>> {
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, target.phase(), |control| {
        let segments =
            roughr::points_on_path::normalized_path_with_control(svg_path_data, control)?;
        roughr::renderer::svg_normalized_segments_with_control::<f64>(
            &segments,
            &mut options,
            control,
        )
    })
    .ok()
}

#[allow(clippy::too_many_arguments)]
pub(in crate::svg::parity) fn roughjs_hachure_paths_for_svg_path(
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    fill_weight: f32,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_hachure_paths_for_svg_path(
        RoughGeometryTarget::Svg,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        fill_weight,
        hachure_gap,
        roughness,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

#[allow(
    clippy::too_many_arguments,
    reason = "The RoughJS adapter keeps upstream geometry, stroke, accounting, and randomness explicit."
)]
pub(in crate::svg::parity) fn roughjs_prepare_hachure_paths_for_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    fill_weight: f32,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    let reservation =
        admit_hand_drawn_path(target, svg_path_data, hachure_gap, roughness, work_meter)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .fill_style(roughr::core::FillStyle::Hachure)
        .fill_weight(fill_weight)
        .hachure_gap(hachure_gap)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generated = controlled::generate(work_meter, target.phase(), |control| {
        let segments =
            roughr::points_on_path::normalized_path_with_control(svg_path_data, control)?;
        let mut sets = roughr::points_on_path::points_on_normalized_segments_with_control::<f64>(
            &segments,
            Some(1.0),
            Some((1.0 + roughness as f64) / 2.0),
            control,
        )?;
        let fill =
            roughr::renderer::hachure_fill_polygons_with_control(&mut sets, &mut options, control)?;
        let stroke = roughr::renderer::svg_normalized_segments_with_control(
            &segments,
            &mut options,
            control,
        )?;
        Ok((fill, stroke))
    });
    let (fill, stroke) = match generated {
        Ok(paths) => paths,
        Err(_) => {
            if let Some(reservation) = reservation {
                let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
            }
            return None;
        }
    };
    Some(RoughPathPair {
        fill,
        stroke,
        reservation,
    })
}

/// Render a path with Mermaid's default hand-drawn node options.
pub(in crate::svg::parity) fn roughjs_paths_for_hand_drawn_svg_path(
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_hand_drawn_svg_path(
        RoughGeometryTarget::Svg,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

pub(in crate::svg::parity) fn roughjs_prepare_paths_for_hand_drawn_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    roughjs_prepare_hachure_paths_for_svg_path(
        target,
        svg_path_data,
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
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_hand_drawn_solid_svg_path(
        RoughGeometryTarget::Svg,
        svg_path_data,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

pub(in crate::svg::parity) fn roughjs_prepare_paths_for_hand_drawn_solid_svg_path(
    target: RoughGeometryTarget,
    svg_path_data: &str,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    // Generator.path uses paint presence to select operation sets. Actual CSS paint belongs to
    // SVG emission, so neutral colors activate both sets without constraining source colors.
    let geometry_paint = roughr::Srgba::new(0.0, 0.0, 0.0, 1.0);
    let reservation = admit_hand_drawn_path(
        target,
        svg_path_data,
        HAND_DRAWN_HACHURE_GAP,
        HAND_DRAWN_ROUGHNESS,
        work_meter,
    )?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(HAND_DRAWN_ROUGHNESS)
        .fill(geometry_paint)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke(geometry_paint)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let drawable = match controlled::generate(work_meter, target.phase(), |control| {
        roughr::generator::Generator::default().path_with_control::<f64>(
            svg_path_data,
            &Some(options),
            control,
        )
    }) {
        Ok(drawable) => drawable,
        Err(_) => {
            if let Some(reservation) = reservation {
                let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
            }
            return None;
        }
    };
    let mut fill_d = None;
    let mut stroke_d = None;

    for set in drawable.sets {
        match set.op_set_type {
            roughr::core::OpSetType::FillPath | roughr::core::OpSetType::FillSketch => {
                fill_d = Some(set);
            }
            roughr::core::OpSetType::Path => {
                stroke_d = Some(set);
            }
        }
    }

    match (fill_d, stroke_d) {
        (Some(fill), Some(stroke)) => Some(RoughPathPair {
            fill,
            stroke,
            reservation,
        }),
        _ => {
            if let Some(reservation) = reservation {
                let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
            }
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::svg::parity) fn roughjs_hachure_paths_for_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    fill_weight: f32,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_hachure_paths_for_rect(
        RoughGeometryTarget::Svg,
        x,
        y,
        w,
        h,
        stroke_width,
        stroke_dasharray,
        fill_weight,
        hachure_gap,
        roughness,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

#[allow(
    clippy::too_many_arguments,
    reason = "The RoughJS adapter keeps upstream geometry, stroke, accounting, and randomness explicit."
)]
pub(in crate::svg::parity) fn roughjs_prepare_hachure_paths_for_rect(
    target: RoughGeometryTarget,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    fill_weight: f32,
    hachure_gap: f32,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    let reservation = admit_hand_drawn_rect(target, w, h, hachure_gap, roughness, work_meter)?;
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .fill_style(roughr::core::FillStyle::Hachure)
        .fill_weight(fill_weight)
        .hachure_gap(hachure_gap)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generated = controlled::generate(work_meter, target.phase(), |control| {
        let stroke =
            roughr::renderer::rectangle_with_control::<f64>(x, y, w, h, &mut options, control)?;
        let points = vec![
            roughr::Point2D::new(x, y),
            roughr::Point2D::new(x + w, y),
            roughr::Point2D::new(x + w, y + h),
            roughr::Point2D::new(x, y + h),
        ];
        let fill = roughr::renderer::hachure_fill_polygons_with_control(
            &mut [points],
            &mut options,
            control,
        )?;
        Ok((fill, stroke))
    });
    let (fill, stroke) = match generated {
        Ok(paths) => paths,
        Err(_) => {
            if let Some(reservation) = reservation {
                let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
            }
            return None;
        }
    };
    Some(RoughPathPair {
        fill,
        stroke,
        reservation,
    })
}

/// Render a rectangle with Mermaid's default hand-drawn fill and stroke options.
#[allow(
    clippy::too_many_arguments,
    reason = "The RoughJS rectangle adapter preserves the upstream geometry and stroke parameters alongside deterministic randomness."
)]
pub(in crate::svg::parity) fn roughjs_paths_for_hand_drawn_rect(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_hand_drawn_rect(
        RoughGeometryTarget::Svg,
        x,
        y,
        w,
        h,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

#[allow(
    clippy::too_many_arguments,
    reason = "The RoughJS adapter keeps upstream geometry, stroke, accounting, and randomness explicit."
)]
pub(in crate::svg::parity) fn roughjs_prepare_paths_for_hand_drawn_rect(
    target: RoughGeometryTarget,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    roughjs_prepare_hachure_paths_for_rect(
        target,
        x,
        y,
        w,
        h,
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
    start: (f64, f64),
    end: (f64, f64),
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<String> {
    roughjs_prepare_hand_drawn_line_path(
        RoughGeometryTarget::Svg,
        start,
        end,
        stroke_width,
        stroke_dasharray,
        work_meter,
        randomness,
    )
    .map(|opset| ops_to_svg_path_d(&opset))
}

pub(in crate::svg::parity) fn roughjs_prepare_hand_drawn_line_path(
    target: RoughGeometryTarget,
    start: (f64, f64),
    end: (f64, f64),
    stroke_width: f32,
    stroke_dasharray: &str,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<roughr::core::OpSet<f64>> {
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(HAND_DRAWN_ROUGHNESS)
        .bowing(1.0)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, target.phase(), |control| {
        roughr::renderer::line_with_control::<f64>(
            start.0,
            start.1,
            end.0,
            end.1,
            &mut options,
            control,
        )
    })
    .ok()
}

pub(in crate::svg::parity) fn roughjs_paths_for_polygon(
    points: &[(f64, f64)],
    stroke_width: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_polygon(
        RoughGeometryTarget::Svg,
        points,
        stroke_width,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

pub(in crate::svg::parity) fn roughjs_prepare_paths_for_polygon(
    target: RoughGeometryTarget,
    points: &[(f64, f64)],
    stroke_width: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    // Mirror RoughJS `generator.polygon(...)` generation order: outline first, then fill, then
    // emit fill before outline.
    let mut opts = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(0.0)
        .fill_style(roughr::core::FillStyle::Solid)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![0.0, 0.0])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    controlled::generate(work_meter, target.phase(), |control| {
        let mut polygon = Vec::new();
        for &(x, y) in points {
            roughr::work::WorkControl::charge(control, 1)?;
            polygon.push(roughr::Point2D::new(x, y));
        }
        let stroke =
            roughr::renderer::linear_path_with_control(&polygon, true, &mut opts, control)?;
        let fill =
            roughr::renderer::solid_fill_polygon_with_control(&[polygon], &mut opts, control)?;
        Ok(RoughPathPair::new(fill, stroke))
    })
    .ok()
}

pub(in crate::svg::parity) fn roughjs_paths_for_circle(
    diameter: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn: bool,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_circle(
        RoughGeometryTarget::Svg,
        diameter,
        stroke_width,
        stroke_dasharray,
        hand_drawn,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

pub(in crate::svg::parity) fn roughjs_prepare_paths_for_circle(
    target: RoughGeometryTarget,
    diameter: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn: bool,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    roughjs_prepare_paths_for_circle_with_roughness(
        target,
        diameter,
        stroke_width,
        stroke_dasharray,
        hand_drawn,
        if hand_drawn {
            HAND_DRAWN_ROUGHNESS
        } else {
            0.0
        },
        work_meter,
        randomness,
    )
}

pub(in crate::svg::parity) fn roughjs_paths_for_circle_with_roughness(
    diameter: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn: bool,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<(String, String)> {
    roughjs_prepare_paths_for_circle_with_roughness(
        RoughGeometryTarget::Svg,
        diameter,
        stroke_width,
        stroke_dasharray,
        hand_drawn,
        roughness,
        work_meter,
        randomness,
    )
    .and_then(|paths| paths.into_svg_paths(work_meter))
}

#[allow(
    clippy::too_many_arguments,
    reason = "The RoughJS adapter keeps upstream geometry, stroke, accounting, and randomness explicit."
)]
pub(in crate::svg::parity) fn roughjs_prepare_paths_for_circle_with_roughness(
    target: RoughGeometryTarget,
    diameter: f64,
    stroke_width: f32,
    stroke_dasharray: &str,
    hand_drawn: bool,
    roughness: f32,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Option<RoughPathPair> {
    let reservation = if hand_drawn {
        admit_hand_drawn_circle(target, diameter, roughness, work_meter)?
    } else {
        None
    };
    let (dash0, dash1) = parse_stroke_dash_pair(stroke_dasharray);
    let mut options = roughr::core::OptionsBuilder::default()
        .randomness(randomness.clone())
        .roughness(roughness)
        .curve_step_count(CIRCLE_CURVE_STEP_COUNT)
        .curve_fitting(CIRCLE_CURVE_FITTING)
        .fill_style(if hand_drawn {
            roughr::core::FillStyle::Hachure
        } else {
            roughr::core::FillStyle::Solid
        })
        .fill_weight(CIRCLE_HAND_DRAWN_FILL_WEIGHT)
        .hachure_gap(CIRCLE_HACHURE_GAP)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![dash0, dash1])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false)
        .build()
        .ok()?;

    let generated = controlled::generate(work_meter, target.phase(), |control| {
        let params = roughr::renderer::generate_ellipse_params_with_control(
            diameter,
            diameter,
            &mut options,
            control,
        )?;
        let stroke = roughr::renderer::ellipse_with_params_with_control(
            0.0,
            0.0,
            &mut options,
            &params,
            control,
        )?;
        let fill = if hand_drawn {
            roughr::renderer::hachure_fill_polygons_with_control(
                &mut [stroke.estimated_points],
                &mut options,
                control,
            )?
        } else {
            roughr::renderer::ellipse_with_params_with_control(
                0.0,
                0.0,
                &mut options,
                &params,
                control,
            )?
            .opset
        };
        Ok((fill, stroke.opset))
    });
    let (fill, stroke) = match generated {
        Ok(paths) => paths,
        Err(_) => {
            if let Some(reservation) = reservation {
                let _ = work_meter.reconcile_svg_bytes(reservation.additional_bytes, 0);
            }
            return None;
        }
    };
    Some(RoughPathPair {
        fill,
        stroke,
        reservation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{
        OperationWorkError, RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase,
    };
    use merman_model::OperationControl;

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
    fn circle_budget_overflow_is_terminal_for_svg_and_query() {
        for target in [RoughGeometryTarget::Svg, RoughGeometryTarget::Query] {
            for diameter in [1e18, f64::MAX] {
                let control = OperationControl::new();
                let meter = OperationWorkMeter::new_with_control(
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                    control.clone(),
                );
                assert!(
                    roughjs_prepare_paths_for_circle_with_roughness(
                        target,
                        diameter,
                        1.3,
                        "0 0",
                        true,
                        0.7,
                        &meter,
                        &randomness(),
                    )
                    .is_none()
                );
                for phase in [OperationPhase::Emit, OperationPhase::Layout] {
                    let OperationWorkError::ResourceLimitExceeded(error) = meter
                        .checkpoint(phase)
                        .expect_err("overflow must remain terminal")
                    else {
                        panic!("expected arithmetic overflow");
                    };
                    assert_eq!(error.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
                    assert_eq!(
                        error.cause,
                        crate::resources::ResourceLimitCause::ArithmeticOverflow
                    );
                }
                control.cancel();
                assert!(matches!(meter.checkpoint(target.phase()),
                    Err(OperationWorkError::ResourceLimitExceeded(error))
                        if error.cause == crate::resources::ResourceLimitCause::ArithmeticOverflow));
                assert!(
                    roughjs_prepare_paths_for_circle_with_roughness(
                        target,
                        80.0,
                        1.3,
                        "0 0",
                        true,
                        0.7,
                        &meter,
                        &randomness(),
                    )
                    .is_none()
                );
                assert!(matches!(meter.checkpoint(target.phase()),
                    Err(OperationWorkError::ResourceLimitExceeded(error))
                        if error.cause == crate::resources::ResourceLimitCause::ArithmeticOverflow));
                assert_eq!(meter.used(), 0);
                assert_eq!(meter.projected_svg_bytes(), 0);
            }
        }
    }

    #[test]
    fn hand_drawn_hachure_stops_inside_roughr_at_work_ceiling() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 256)
            .expect("valid work ceiling");
        let meter = OperationWorkMeter::new(policy);

        assert!(
            roughjs_paths_for_hand_drawn_svg_path(path(), 1.0, "0 0", &meter, &randomness(),)
                .is_none()
        );
        assert_limit(
            meter
                .checkpoint(OperationPhase::Emit)
                .expect_err("work ceiling must become terminal"),
            ResourceLimitId::MaxLayoutWorkUnits.as_str(),
            ResourceLimitPhase::LayoutModel,
        );
        assert!(meter.used() > 0 && meter.used() <= 256);
    }

    #[test]
    fn query_geometry_keeps_work_and_cancellation_without_reserving_svg() {
        let generate = |shape, meter: &OperationWorkMeter| match shape {
            0 => roughjs_prepare_paths_for_hand_drawn_svg_path(
                RoughGeometryTarget::Query,
                "M0 0 H80 V40 H0 Z",
                1.0,
                "0 0",
                meter,
                &randomness(),
            ),
            1 => roughjs_prepare_paths_for_hand_drawn_rect(
                RoughGeometryTarget::Query,
                0.0,
                0.0,
                80.0,
                40.0,
                1.0,
                "0 0",
                meter,
                &randomness(),
            ),
            _ => roughjs_prepare_paths_for_circle(
                RoughGeometryTarget::Query,
                80.0,
                1.0,
                "0 0",
                true,
                meter,
                &randomness(),
            ),
        };
        for shape in 0..3 {
            let tiny_svg = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                .unwrap();
            let meter = OperationWorkMeter::new(tiny_svg);
            assert!(generate(shape, &meter).is_some());
            assert!(meter.used() > 0);
            assert_eq!(meter.projected_svg_bytes(), 0);
            meter.checkpoint(OperationPhase::Emit).unwrap();

            let meter = OperationWorkMeter::new(
                tiny_svg
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                    .unwrap(),
            );
            assert!(generate(shape, &meter).is_none());
            assert_limit(
                meter.checkpoint(OperationPhase::Emit).unwrap_err(),
                ResourceLimitId::MaxLayoutWorkUnits.as_str(),
                ResourceLimitPhase::LayoutModel,
            );
            assert!(matches!(
                meter.control().terminal_checkpoint_at(OperationPhase::Emit),
                Err(merman_model::OperationLedgerError::ResourceLimitExceeded(error))
                    if error.phase == OperationPhase::Layout
            ));
            assert_eq!(meter.projected_svg_bytes(), 0);

            let control = OperationControl::new();
            control.cancel();
            let meter = OperationWorkMeter::new_with_control(tiny_svg, control);
            assert!(generate(shape, &meter).is_none());
            assert!(matches!(
                meter.checkpoint(OperationPhase::Emit),
                Err(OperationWorkError::Cancelled(error)) if error.phase == OperationPhase::Layout
            ));
            assert_eq!(meter.projected_svg_bytes(), 0);
        }
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
            roughjs_paths_for_hand_drawn_svg_path(path(), 1.0, "0 0", &meter, &randomness(),)
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
    fn hand_drawn_circle_none_preserves_work_svg_and_cancellation_terminals() {
        let diameter = 108.0;
        let budget = hand_drawn_geometry_budget(
            std::f64::consts::PI * diameter,
            diameter,
            diameter,
            CIRCLE_HACHURE_GAP,
            HAND_DRAWN_ROUGHNESS,
            32,
        )
        .unwrap();
        for (id, max, phase) in [
            (
                ResourceLimitId::MaxLayoutWorkUnits,
                1,
                ResourceLimitPhase::LayoutModel,
            ),
            (
                ResourceLimitId::MaxSvgBytes,
                budget.svg_bytes - 1,
                ResourceLimitPhase::SvgOutput,
            ),
        ] {
            let meter = OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(id, max)
                    .unwrap(),
            );
            assert!(
                roughjs_paths_for_circle(diameter, 1.3, "0 0", true, &meter, &randomness())
                    .is_none()
            );
            let error = meter
                .checkpoint(OperationPhase::Emit)
                .expect_err("circle admission retains the terminal cause");
            assert_limit(error, id.as_str(), phase);
        }
        let control = OperationControl::new();
        control.cancel();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        assert!(
            roughjs_paths_for_circle(diameter, 1.3, "0 0", true, &meter, &randomness()).is_none()
        );
        assert!(matches!(
            meter.checkpoint(OperationPhase::Emit),
            Err(OperationWorkError::Cancelled(_))
        ));
    }

    #[test]
    fn hand_drawn_circle_counts_generated_work_and_retains_the_selected_roughness() {
        let diameter = 108.0;
        for roughness in [0.2, HAND_DRAWN_ROUGHNESS] {
            let probe =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            let expected = roughjs_paths_for_circle_with_roughness(
                diameter,
                1.3,
                "0 0",
                true,
                roughness,
                &probe,
                &randomness(),
            )
            .unwrap();
            let units = probe.used();
            assert!(units > 0);
            let meter = OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, units)
                    .unwrap(),
            );
            let (fill, stroke) = roughjs_paths_for_circle_with_roughness(
                diameter,
                1.3,
                "0 0",
                true,
                roughness,
                &meter,
                &randomness(),
            )
            .expect("one circle must fit its exact geometry-work budget");
            assert_eq!(meter.used(), units);
            assert_eq!(
                (fill.as_str(), stroke.as_str()),
                (expected.0.as_str(), expected.1.as_str())
            );
            assert_eq!(meter.projected_svg_bytes(), fill.len() + stroke.len());
            meter.checkpoint(OperationPhase::Emit).unwrap();
            let below = OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, units - 1)
                    .unwrap(),
            );
            assert!(
                roughjs_paths_for_circle_with_roughness(
                    diameter,
                    1.3,
                    "0 0",
                    true,
                    roughness,
                    &below,
                    &randomness(),
                )
                .is_none()
            );
            assert_limit(
                below.checkpoint(OperationPhase::Emit).unwrap_err(),
                ResourceLimitId::MaxLayoutWorkUnits.as_str(),
                ResourceLimitPhase::LayoutModel,
            );
        }
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
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 20_000)
            .unwrap()
            .with_limit(ResourceLimitId::MaxSvgBytes, budget.svg_bytes)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);

        let (fill_d, stroke_d) =
            roughjs_paths_for_hand_drawn_svg_path(path, 1.0, "0 0", &meter, &randomness())
                .expect("bounded ordinary path must remain renderable");
        assert!(!fill_d.is_empty());
        assert!(!stroke_d.is_empty());
        assert!(meter.projected_svg_bytes() <= budget.svg_bytes);
        meter
            .checkpoint(OperationPhase::Emit)
            .expect("successful path must not poison the operation");
    }
}
