//! Renderer-neutral Rough.js geometry shared by document adapters and SVG renderers.

use crate::environment::RenderSession;
use crate::{Error, Result};
use merman_display_list::{Color, PathSegment, Point};
use roughr::core::{FillStyle, OpSet, OpType, Options, OptionsBuilder, RoughRandomness};

pub(crate) struct RoughRectangleSpec<'a> {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) fill: Option<roughr::Srgba>,
    pub(crate) stroke: Option<roughr::Srgba>,
    pub(crate) stroke_width: f32,
    pub(crate) randomness: &'a RoughRandomness,
}

pub(crate) struct RoughRectangleOpSets {
    pub(crate) fill: Option<OpSet<f64>>,
    pub(crate) stroke: Option<OpSet<f64>>,
}

pub(crate) fn operation_randomness(
    session: &RenderSession,
    configured_seed: f64,
    owner_domain: &str,
) -> RoughRandomness {
    let resolved_seed = if configured_seed == 0.0 {
        session.render_seed().get() as f64
    } else {
        configured_seed
    };
    let operation = session.operation_context();
    RoughRandomness::new(
        roughr::core::RoughJsSeed::new(resolved_seed),
        roughr::core::RoughMathRandom::new(operation.derive_u64(owner_domain, 0)),
    )
}

pub(crate) fn display_color_to_srgba(color: Color) -> roughr::Srgba {
    roughr::Srgba::new(
        color.red as f32 / 255.0,
        color.green as f32 / 255.0,
        color.blue as f32 / 255.0,
        color.alpha as f32 / 255.0,
    )
}

pub(crate) fn rough_rectangle_opsets(spec: RoughRectangleSpec<'_>) -> Result<RoughRectangleOpSets> {
    let RoughRectangleSpec {
        x,
        y,
        width,
        height,
        fill,
        stroke,
        stroke_width,
        randomness,
    } = spec;
    let mut options = rough_options(randomness, stroke_width)?;
    options.fill = fill;
    options.stroke = stroke;

    // Rough.js advances the rectangle outline first, then emits the optional fill before the
    // outline. Preserve that generation order so explicit seeds have the same visible geometry.
    let outline = roughr::renderer::rectangle::<f64>(x, y, width, height, &mut options);
    let fill = options.fill.map(|_| {
        let polygons = vec![vec![
            roughr::Point2D::new(x, y),
            roughr::Point2D::new(x + width, y),
            roughr::Point2D::new(x + width, y + height),
            roughr::Point2D::new(x, y + height),
        ]];
        roughr::renderer::solid_fill_polygon(&polygons, &mut options)
    });
    let stroke = options.stroke.map(|_| outline);

    Ok(RoughRectangleOpSets { fill, stroke })
}

pub(crate) fn rough_line_opset(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    stroke: roughr::Srgba,
    stroke_width: f32,
    randomness: &RoughRandomness,
) -> Result<OpSet<f64>> {
    let mut options = rough_options(randomness, stroke_width)?;
    options.fill = None;
    options.stroke = Some(stroke);
    Ok(roughr::renderer::line::<f64>(x1, y1, x2, y2, &mut options))
}

pub(crate) fn opset_to_path_segments(opset: &OpSet<f64>) -> Result<Vec<PathSegment>> {
    opset.ops.iter().map(op_to_path_segment).collect()
}

pub(crate) fn op_to_path_segment(op: &roughr::core::Op<f64>) -> Result<PathSegment> {
    match (&op.op, op.data.as_slice()) {
        (OpType::Move, [x, y]) => Ok(PathSegment::MoveTo {
            to: Point::new(*x, *y),
        }),
        (OpType::LineTo, [x, y]) => Ok(PathSegment::LineTo {
            to: Point::new(*x, *y),
        }),
        (OpType::BCurveTo, [x1, y1, x2, y2, x, y]) => Ok(PathSegment::CubicTo {
            control1: Point::new(*x1, *y1),
            control2: Point::new(*x2, *y2),
            to: Point::new(*x, *y),
        }),
        _ => Err(Error::InvalidModel {
            message: format!(
                "Rough.js operation {:?} has an invalid coordinate arity {}",
                op.op,
                op.data.len()
            ),
        }),
    }
}

fn rough_options(randomness: &RoughRandomness, stroke_width: f32) -> Result<Options> {
    let mut builder = OptionsBuilder::default();
    builder
        .randomness(randomness.clone())
        .roughness(0.0)
        .fill_style(FillStyle::Solid)
        .stroke_width(stroke_width)
        .stroke_line_dash(vec![0.0, 0.0])
        .stroke_line_dash_offset(0.0)
        .fill_line_dash(vec![0.0, 0.0])
        .fill_line_dash_offset(0.0)
        .disable_multi_stroke(false)
        .disable_multi_stroke_fill(false);
    builder.build().map_err(|error| Error::InvalidModel {
        message: format!("failed to construct Rough.js options: {error}"),
    })
}
