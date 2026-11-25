use krilla::blend::BlendMode;
use krilla::color::{luma, rgb};
use krilla::geom::{Path, PathBuilder, Rect};
use krilla::mask::MaskType;
use krilla::num::NormalizedF32;
use krilla::paint::{
    Fill, FillRule, LineCap, LineJoin, LinearGradient, Paint, Pattern, RadialGradient,
    SpreadMethod, Stop, Stroke, StrokeDash,
};
use krilla::stream::StreamBuilder;
use tiny_skia::PathSegment;
use usvg::tiny_skia_path::Transform;

use crate::{group, ProcessContext};

/// Convert a usvg `SpreadMethod` into a krilla `SpreadMethod`.
pub(crate) fn convert_spread_method(spread_method: &usvg::SpreadMethod) -> SpreadMethod {
    match spread_method {
        usvg::SpreadMethod::Pad => SpreadMethod::Pad,
        usvg::SpreadMethod::Reflect => SpreadMethod::Reflect,
        usvg::SpreadMethod::Repeat => SpreadMethod::Repeat,
    }
}

/// Convert a usvg `Stop` into a krilla `Stop`.
pub(crate) fn convert_stop(stop: &usvg::Stop) -> Stop {
    Stop {
        offset: NormalizedF32::new(stop.offset().get()).unwrap(),
        color: rgb::Color::new(stop.color().red, stop.color().green, stop.color().blue).into(),
        opacity: NormalizedF32::new(stop.opacity().get()).unwrap(),
    }
}

/// Convert a usvg `Paint` into a krilla `Paint`.
pub(crate) fn convert_paint(
    paint: &usvg::Paint,
    mut stream_builder: StreamBuilder,
    process_context: &mut ProcessContext,
    // The additional transform is needed because in krilla, a transform to a shape will also apply
    // the transform to the paint server. However, in the case of SVG glyphs, we don't want the transform
    // to be shifted for each glyph we draw (since we draw them separately instead of in a glyph run),
    // so we need to apply an additional inverse transform to counter that effect.
    additional_transform: Transform,
) -> Paint {
    match paint {
        usvg::Paint::Color(c) => {
            if c.red == c.green && c.green == c.blue {
                // Use gray-scale color if possible.
                luma::Color::new(c.red).into()
            } else {
                rgb::Color::new(c.red, c.green, c.blue).into()
            }
        }
        usvg::Paint::LinearGradient(lg) => LinearGradient {
            x1: lg.x1(),
            y1: lg.y1(),
            x2: lg.x2(),
            y2: lg.y2(),
            transform: additional_transform.pre_concat(lg.transform()).to_krilla(),
            spread_method: convert_spread_method(&lg.spread_method()),
            stops: lg.stops().iter().map(convert_stop).collect::<Vec<_>>(),
            anti_alias: false,
        }
        .into(),
        usvg::Paint::RadialGradient(rg) => RadialGradient {
            cx: rg.cx(),
            cy: rg.cy(),
            cr: rg.r().get(),
            fx: rg.fx(),
            fy: rg.fy(),
            fr: 0.0,
            transform: additional_transform.pre_concat(rg.transform()).to_krilla(),
            spread_method: convert_spread_method(&rg.spread_method()),
            stops: rg.stops().iter().map(convert_stop).collect::<Vec<_>>(),
            anti_alias: false,
        }
        .into(),
        usvg::Paint::Pattern(pat) => {
            let mut surface = stream_builder.surface();
            group::render(pat.root(), &mut surface, process_context);
            surface.finish();
            let stream = stream_builder.finish();

            Pattern {
                stream,
                transform: additional_transform
                    .pre_concat(pat.transform())
                    .pre_concat(Transform::from_translate(pat.rect().x(), pat.rect().y()))
                    .to_krilla(),
                width: pat.rect().width(),
                height: pat.rect().height(),
            }
            .into()
        }
    }
}

/// Convert a usvg `LineCap` into a krilla `LineCap`.
pub(crate) fn convert_line_cap(line_cap: &usvg::LineCap) -> LineCap {
    match line_cap {
        usvg::LineCap::Butt => LineCap::Butt,
        usvg::LineCap::Round => LineCap::Round,
        usvg::LineCap::Square => LineCap::Square,
    }
}

/// Convert a usvg `LineJoin` into a krilla `LineJoin`.
pub(crate) fn convert_line_join(line_join: &usvg::LineJoin) -> LineJoin {
    match line_join {
        usvg::LineJoin::Miter => LineJoin::Miter,
        usvg::LineJoin::MiterClip => LineJoin::Miter,
        usvg::LineJoin::Round => LineJoin::Round,
        usvg::LineJoin::Bevel => LineJoin::Bevel,
    }
}

/// Convert a usvg `FillRule` into a krilla `FillRule`.
pub(crate) fn convert_fill_rule(fill_rule: &usvg::FillRule) -> FillRule {
    match fill_rule {
        usvg::FillRule::NonZero => FillRule::NonZero,
        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
    }
}

/// Convert a usvg `Fill` into a krilla `Fill`.
pub(crate) fn convert_fill(
    fill: &usvg::Fill,
    stream_builder: StreamBuilder,
    process_context: &mut ProcessContext,
    additional_transform: Transform,
) -> Fill {
    Fill {
        paint: convert_paint(
            fill.paint(),
            stream_builder,
            process_context,
            additional_transform,
        ),
        opacity: NormalizedF32::new(fill.opacity().get()).unwrap(),
        rule: convert_fill_rule(&fill.rule()),
    }
}

/// Convert a usvg `Stroke` into a krilla `Stroke`.
pub(crate) fn convert_stroke(
    stroke: &usvg::Stroke,
    stream_builder: StreamBuilder,
    process_context: &mut ProcessContext,
    additional_transform: Transform,
) -> Stroke {
    let dash = stroke.dasharray().map(|dash_array| StrokeDash {
        offset: stroke.dashoffset(),
        array: dash_array.to_vec(),
    });

    Stroke {
        paint: convert_paint(
            stroke.paint(),
            stream_builder,
            process_context,
            additional_transform,
        ),
        width: stroke.width().get(),
        miter_limit: stroke.miterlimit().get(),
        line_cap: convert_line_cap(&stroke.linecap()),
        line_join: convert_line_join(&stroke.linejoin()),
        opacity: NormalizedF32::new(stroke.opacity().get()).unwrap(),
        dash,
    }
}

/// Convert a usvg `BlendMode` into a krilla `BlendMode`.
pub(crate) fn convert_blend_mode(blend_mode: &usvg::BlendMode) -> BlendMode {
    match blend_mode {
        usvg::BlendMode::Normal => BlendMode::Normal,
        usvg::BlendMode::Multiply => BlendMode::Multiply,
        usvg::BlendMode::Screen => BlendMode::Screen,
        usvg::BlendMode::Overlay => BlendMode::Overlay,
        usvg::BlendMode::Darken => BlendMode::Darken,
        usvg::BlendMode::Lighten => BlendMode::Lighten,
        usvg::BlendMode::ColorDodge => BlendMode::ColorDodge,
        usvg::BlendMode::ColorBurn => BlendMode::ColorBurn,
        usvg::BlendMode::HardLight => BlendMode::HardLight,
        usvg::BlendMode::SoftLight => BlendMode::SoftLight,
        usvg::BlendMode::Difference => BlendMode::Difference,
        usvg::BlendMode::Exclusion => BlendMode::Exclusion,
        usvg::BlendMode::Hue => BlendMode::Hue,
        usvg::BlendMode::Saturation => BlendMode::Saturation,
        usvg::BlendMode::Color => BlendMode::Color,
        usvg::BlendMode::Luminosity => BlendMode::Luminosity,
        // PDF has no additive blend mode. Screen approximates its brightening effect.
        usvg::BlendMode::PlusLighter => BlendMode::Screen,
    }
}

/// Convert a usvg `MaskType` into a krilla `MaskType`.
pub(crate) fn convert_mask_type(mask_type: &usvg::MaskType) -> MaskType {
    match mask_type {
        usvg::MaskType::Luminance => MaskType::Luminosity,
        usvg::MaskType::Alpha => MaskType::Alpha,
    }
}

pub(crate) trait RectExt {
    fn to_clip_path(&self) -> Path;
}

impl RectExt for Rect {
    fn to_clip_path(&self) -> Path {
        let mut path_builder = PathBuilder::new();
        path_builder.move_to(self.left(), self.top());
        path_builder.line_to(self.right(), self.top());
        path_builder.line_to(self.right(), self.bottom());
        path_builder.line_to(self.left(), self.bottom());
        path_builder.close();
        path_builder.finish().unwrap()
    }
}

pub(crate) trait KrillaTransformExt {
    fn to_usvg(&self) -> Transform;
}

impl KrillaTransformExt for krilla::geom::Transform {
    fn to_usvg(&self) -> Transform {
        Transform::from_row(
            self.sx(),
            self.ky(),
            self.kx(),
            self.sy(),
            self.tx(),
            self.ty(),
        )
    }
}

pub(crate) trait UsvgTransformExt {
    fn to_krilla(&self) -> krilla::geom::Transform;
}

impl UsvgTransformExt for Transform {
    fn to_krilla(&self) -> krilla::geom::Transform {
        krilla::geom::Transform::from_row(self.sx, self.ky, self.kx, self.sy, self.tx, self.ty)
    }
}

pub(crate) trait PathExt {
    fn to_krilla(&self) -> Path;
}

impl PathExt for usvg::Path {
    fn to_krilla(&self) -> Path {
        let mut pb = PathBuilder::new();

        for seg in self.data().segments() {
            match seg {
                PathSegment::MoveTo(p) => pb.move_to(p.x, p.y),
                PathSegment::LineTo(p) => pb.line_to(p.x, p.y),
                PathSegment::QuadTo(p1, p2) => pb.quad_to(p1.x, p1.y, p2.x, p2.y),
                PathSegment::CubicTo(p1, p2, p3) => pb.cubic_to(p1.x, p1.y, p2.x, p2.y, p3.x, p3.y),
                PathSegment::Close => pb.close(),
            }
        }

        pb.finish().unwrap()
    }
}
