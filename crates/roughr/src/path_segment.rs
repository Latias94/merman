use svgtypes::PathSegment as CurrentPathSegment;
#[cfg(feature = "legacy-compat")]
use svgtypes_0_11::PathSegment as LegacyPathSegment;

/// Path-segment versions accepted by the public rough path helpers.
///
/// `roughr-merman` 0.12.3 keeps the `svgtypes` 0.11 input used by Merman 0.7 while retaining the
/// `svgtypes` 0.16 surface used by current Merman releases.
pub trait SvgPathSegment: Copy {
    #[doc(hidden)]
    fn into_current(self) -> CurrentPathSegment;

    #[doc(hidden)]
    fn from_current(segment: CurrentPathSegment) -> Self;
}

impl SvgPathSegment for CurrentPathSegment {
    fn into_current(self) -> CurrentPathSegment {
        self
    }

    fn from_current(segment: CurrentPathSegment) -> Self {
        segment
    }
}

#[cfg(feature = "legacy-compat")]
macro_rules! convert_path_segment {
    ($segment:expr, $source:ident, $target:ident) => {
        match $segment {
            $source::MoveTo { abs, x, y } => $target::MoveTo { abs, x, y },
            $source::LineTo { abs, x, y } => $target::LineTo { abs, x, y },
            $source::HorizontalLineTo { abs, x } => $target::HorizontalLineTo { abs, x },
            $source::VerticalLineTo { abs, y } => $target::VerticalLineTo { abs, y },
            $source::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => $target::CurveTo {
                abs,
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            },
            $source::SmoothCurveTo { abs, x2, y2, x, y } => {
                $target::SmoothCurveTo { abs, x2, y2, x, y }
            }
            $source::Quadratic { abs, x1, y1, x, y } => $target::Quadratic { abs, x1, y1, x, y },
            $source::SmoothQuadratic { abs, x, y } => $target::SmoothQuadratic { abs, x, y },
            $source::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            } => $target::EllipticalArc {
                abs,
                rx,
                ry,
                x_axis_rotation,
                large_arc,
                sweep,
                x,
                y,
            },
            $source::ClosePath { abs } => $target::ClosePath { abs },
        }
    };
}

#[cfg(feature = "legacy-compat")]
impl SvgPathSegment for LegacyPathSegment {
    fn into_current(self) -> CurrentPathSegment {
        convert_path_segment!(self, LegacyPathSegment, CurrentPathSegment)
    }

    fn from_current(segment: CurrentPathSegment) -> Self {
        convert_path_segment!(segment, CurrentPathSegment, LegacyPathSegment)
    }
}
