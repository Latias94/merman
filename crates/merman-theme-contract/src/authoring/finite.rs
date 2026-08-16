use super::*;

pub(super) trait ContainsNonFiniteNumber {
    fn contains_non_finite_number(&self) -> bool;
}

impl ContainsNonFiniteNumber for f32 {
    fn contains_non_finite_number(&self) -> bool {
        !self.is_finite()
    }
}

impl<T> ContainsNonFiniteNumber for Option<T>
where
    T: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        self.as_ref()
            .is_some_and(ContainsNonFiniteNumber::contains_non_finite_number)
    }
}

impl<T> ContainsNonFiniteNumber for Vec<T>
where
    T: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        self.iter()
            .any(ContainsNonFiniteNumber::contains_non_finite_number)
    }
}

impl<T> ContainsNonFiniteNumber for SpecifiedWireV1<T>
where
    T: ContainsNonFiniteNumber,
{
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Value(value) => value.contains_non_finite_number(),
            Self::Unspecified | Self::Clear => false,
        }
    }
}

impl ContainsNonFiniteNumber for ThemeAuthoringTypographyV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.font_size_px.contains_non_finite_number()
            || self.line_height.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeRuleSetWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Rule { style, .. } => style.contains_non_finite_number(),
            Self::OrdinalPalette { .. } => false,
        }
    }
}

impl ContainsNonFiniteNumber for ThemeStylePatchWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.fill.contains_non_finite_number()
            || self.opacity.contains_non_finite_number()
            || self.fill_opacity.contains_non_finite_number()
            || self.stroke.contains_non_finite_number()
            || self.radius.contains_non_finite_number()
            || self.padding.contains_non_finite_number()
            || self.typography.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeStrokePatchWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.paint.contains_non_finite_number()
            || self.width.contains_non_finite_number()
            || self.dasharray.contains_non_finite_number()
            || self.opacity.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeTextStylePatchWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.font_size_px.contains_non_finite_number()
            || self.line_height.contains_non_finite_number()
            || self.letter_spacing_px.contains_non_finite_number()
            || self.word_spacing_px.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeCanvasPaintWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Color(_) => false,
            Self::Structured(paint) => paint.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeCanvasPaintObjectWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Transparent | Self::Solid { .. } => false,
            Self::LinearGradient {
                angle_degrees,
                stops,
                repetition,
            } => {
                angle_degrees.contains_non_finite_number()
                    || stops.contains_non_finite_number()
                    || repetition.contains_non_finite_number()
            }
            Self::RadialGradient {
                center_x,
                center_y,
                radius,
                stops,
                repetition,
            } => {
                center_x.contains_non_finite_number()
                    || center_y.contains_non_finite_number()
                    || radius.contains_non_finite_number()
                    || stops.contains_non_finite_number()
                    || repetition.contains_non_finite_number()
            }
            Self::Pattern {
                cell_width,
                cell_height,
                angle_degrees,
                ..
            } => {
                cell_width.contains_non_finite_number()
                    || cell_height.contains_non_finite_number()
                    || angle_degrees.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeLinearGradientRepetitionWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Repeating { period_px } => period_px.contains_non_finite_number(),
            Self::Tiled {
                width_px,
                height_px,
            } => width_px.contains_non_finite_number() || height_px.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeRadialGradientRepetitionWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Repeating => false,
            Self::Tiled {
                width_px,
                height_px,
            } => width_px.contains_non_finite_number() || height_px.contains_non_finite_number(),
        }
    }
}

impl ContainsNonFiniteNumber for ThemeGradientStopWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.offset.contains_non_finite_number()
    }
}

impl ContainsNonFiniteNumber for ThemeLengthWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::PxValue(value) | Self::Px { px: value } | Self::Percent { percent: value } => {
                value.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeInsetsWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::All(value) => value.contains_non_finite_number(),
            Self::Sides {
                top,
                right,
                bottom,
                left,
            } => {
                top.contains_non_finite_number()
                    || right.contains_non_finite_number()
                    || bottom.contains_non_finite_number()
                    || left.contains_non_finite_number()
            }
        }
    }
}

impl ContainsNonFiniteNumber for ThemeLineHeightWireV1 {
    fn contains_non_finite_number(&self) -> bool {
        match self {
            Self::Keyword(_) => false,
            Self::Multiplier(value) | Self::Px { px: value } => value.contains_non_finite_number(),
        }
    }
}
