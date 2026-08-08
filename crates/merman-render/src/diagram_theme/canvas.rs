use merman_core::theme_color::{ColorChannel, ThemeColor};

use super::ThemeCompileValidationError;

const MAX_GRADIENT_STOPS: usize = 64;
const MAX_CANVAS_LAYERS: usize = 32;

/// A parsed, canonical color owned by a typed diagram theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColorValue(ThemeColor);

impl ThemeColorValue {
    pub fn parse(value: &str) -> Result<Self, ThemeCompileValidationError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(ThemeCompileValidationError::EmptyValue { field: "color" });
        }
        ThemeColor::parse(value)
            .map(Self)
            .map_err(|_| ThemeCompileValidationError::InvalidColor {
                value: value.to_string(),
            })
    }

    pub fn as_css(&self) -> String {
        self.0.stringify()
    }

    pub fn alpha(&self) -> f64 {
        self.0.channel(ColorChannel::Alpha)
    }

    pub fn is_transparent(&self) -> bool {
        self.alpha() == 0.0
    }
}

impl TryFrom<&str> for ThemeColorValue {
    type Error = ThemeCompileValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

/// A bounded length used by theme geometry and canvas placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThemeLength {
    Px(f32),
    Percent(f32),
}

impl ThemeLength {
    pub const fn px(value: f32) -> Self {
        Self::Px(value)
    }

    pub const fn percent(value: f32) -> Self {
        Self::Percent(value)
    }

    pub(crate) fn validate(self, field: &'static str) -> Result<(), ThemeCompileValidationError> {
        let value = match self {
            Self::Px(value) | Self::Percent(value) => value,
        };
        if !value.is_finite() || value < 0.0 {
            return Err(ThemeCompileValidationError::InvalidNumber { field });
        }
        if matches!(self, Self::Percent(_)) && value > 100.0 {
            return Err(ThemeCompileValidationError::InvalidNumber { field });
        }
        Ok(())
    }
}

/// Four-sided content or paint inset. Unlike a single edge-label padding scalar, this type keeps
/// layout content padding distinct from route and mask clearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsetsPx {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl InsetsPx {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub(crate) fn validate(self, field: &'static str) -> Result<(), ThemeCompileValidationError> {
        if [self.top, self.right, self.bottom, self.left]
            .into_iter()
            .any(|value| !value.is_finite() || value < 0.0)
        {
            return Err(ThemeCompileValidationError::InvalidNumber { field });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Difference,
}

impl BlendMode {
    pub const fn as_svg(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Multiply => "multiply",
            Self::Screen => "screen",
            Self::Overlay => "overlay",
            Self::Darken => "darken",
            Self::Lighten => "lighten",
            Self::Difference => "difference",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    offset: f32,
    color: ThemeColorValue,
}

impl GradientStop {
    pub fn new(offset: f32, color: ThemeColorValue) -> Result<Self, ThemeCompileValidationError> {
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "gradient.stop.offset",
            });
        }
        Ok(Self { offset, color })
    }

    pub const fn offset(&self) -> f32 {
        self.offset
    }

    pub const fn color(&self) -> &ThemeColorValue {
        &self.color
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    angle_degrees: f32,
    stops: Vec<GradientStop>,
}

impl LinearGradient {
    pub fn new(
        angle_degrees: f32,
        stops: impl IntoIterator<Item = GradientStop>,
    ) -> Result<Self, ThemeCompileValidationError> {
        if !angle_degrees.is_finite() {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "canvas.linear_gradient.angle_degrees",
            });
        }
        let stops = stops.into_iter().collect::<Vec<_>>();
        validate_gradient_stops(&stops, "canvas.linear_gradient.stops")?;
        Ok(Self {
            angle_degrees,
            stops,
        })
    }

    pub const fn angle_degrees(&self) -> f32 {
        self.angle_degrees
    }

    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    center_x: ThemeLength,
    center_y: ThemeLength,
    radius: ThemeLength,
    stops: Vec<GradientStop>,
}

impl RadialGradient {
    pub fn new(
        center_x: ThemeLength,
        center_y: ThemeLength,
        radius: ThemeLength,
        stops: impl IntoIterator<Item = GradientStop>,
    ) -> Result<Self, ThemeCompileValidationError> {
        center_x.validate("canvas.radial_gradient.center_x")?;
        center_y.validate("canvas.radial_gradient.center_y")?;
        radius.validate("canvas.radial_gradient.radius")?;
        let stops = stops.into_iter().collect::<Vec<_>>();
        validate_gradient_stops(&stops, "canvas.radial_gradient.stops")?;
        Ok(Self {
            center_x,
            center_y,
            radius,
            stops,
        })
    }

    pub const fn center_x(&self) -> ThemeLength {
        self.center_x
    }

    pub const fn center_y(&self) -> ThemeLength {
        self.center_y
    }

    pub const fn radius(&self) -> ThemeLength {
        self.radius
    }

    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternKind {
    Dots,
    Grid,
    Stripes,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatternSpec {
    kind: PatternKind,
    cell_width: f32,
    cell_height: f32,
    foreground: ThemeColorValue,
    background: Option<ThemeColorValue>,
    angle_degrees: f32,
}

impl PatternSpec {
    pub fn new(
        kind: PatternKind,
        cell_width: f32,
        cell_height: f32,
        foreground: ThemeColorValue,
    ) -> Result<Self, ThemeCompileValidationError> {
        validate_positive(cell_width, "canvas.pattern.cell_width")?;
        validate_positive(cell_height, "canvas.pattern.cell_height")?;
        Ok(Self {
            kind,
            cell_width,
            cell_height,
            foreground,
            background: None,
            angle_degrees: 0.0,
        })
    }

    pub fn with_background(mut self, background: ThemeColorValue) -> Self {
        self.background = Some(background);
        self
    }

    pub fn with_angle_degrees(
        mut self,
        angle_degrees: f32,
    ) -> Result<Self, ThemeCompileValidationError> {
        if !angle_degrees.is_finite() {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "canvas.pattern.angle_degrees",
            });
        }
        self.angle_degrees = angle_degrees;
        Ok(self)
    }

    pub const fn kind(&self) -> PatternKind {
        self.kind
    }

    pub const fn cell_width(&self) -> f32 {
        self.cell_width
    }

    pub const fn cell_height(&self) -> f32 {
        self.cell_height
    }

    pub const fn foreground(&self) -> &ThemeColorValue {
        &self.foreground
    }

    pub const fn background(&self) -> Option<&ThemeColorValue> {
        self.background.as_ref()
    }

    pub const fn angle_degrees(&self) -> f32 {
        self.angle_degrees
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasPaint {
    Transparent,
    Solid(ThemeColorValue),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
    Pattern(PatternSpec),
}

impl CanvasPaint {
    pub fn solid(value: impl AsRef<str>) -> Result<Self, ThemeCompileValidationError> {
        Ok(Self::Solid(ThemeColorValue::parse(value.as_ref())?))
    }

    pub const fn is_transparent(&self) -> bool {
        matches!(self, Self::Transparent)
    }

    pub(crate) fn validate(&self, field: &'static str) -> Result<(), ThemeCompileValidationError> {
        match self {
            Self::Transparent | Self::Solid(_) => Ok(()),
            Self::LinearGradient(gradient) => validate_gradient_stops(gradient.stops(), field),
            Self::RadialGradient(gradient) => {
                gradient
                    .center_x()
                    .validate("canvas.radial_gradient.center_x")?;
                gradient
                    .center_y()
                    .validate("canvas.radial_gradient.center_y")?;
                gradient
                    .radius()
                    .validate("canvas.radial_gradient.radius")?;
                validate_gradient_stops(gradient.stops(), field)
            }
            Self::Pattern(pattern) => {
                validate_positive(pattern.cell_width(), field)?;
                validate_positive(pattern.cell_height(), field)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasLayer {
    paint: CanvasPaint,
    opacity: f32,
    blend_mode: BlendMode,
    offset_x: f32,
    offset_y: f32,
}

impl CanvasLayer {
    pub fn new(paint: CanvasPaint) -> Self {
        Self {
            paint,
            opacity: 1.0,
            blend_mode: BlendMode::Normal,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    pub fn with_opacity(mut self, opacity: f32) -> Result<Self, ThemeCompileValidationError> {
        validate_unit_interval(opacity, "canvas.layer.opacity")?;
        self.opacity = opacity;
        Ok(self)
    }

    pub fn with_blend_mode(mut self, blend_mode: BlendMode) -> Self {
        self.blend_mode = blend_mode;
        self
    }

    pub fn with_offset(
        mut self,
        offset_x: f32,
        offset_y: f32,
    ) -> Result<Self, ThemeCompileValidationError> {
        if !offset_x.is_finite() || !offset_y.is_finite() {
            return Err(ThemeCompileValidationError::InvalidNumber {
                field: "canvas.layer.offset",
            });
        }
        self.offset_x = offset_x;
        self.offset_y = offset_y;
        Ok(self)
    }

    pub const fn paint(&self) -> &CanvasPaint {
        &self.paint
    }

    pub const fn opacity(&self) -> f32 {
        self.opacity
    }

    pub const fn blend_mode(&self) -> BlendMode {
        self.blend_mode
    }

    pub const fn offset(&self) -> (f32, f32) {
        (self.offset_x, self.offset_y)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasSpec {
    base: CanvasPaint,
    layers: Vec<CanvasLayer>,
    bleed: InsetsPx,
}

impl Default for CanvasSpec {
    fn default() -> Self {
        Self {
            base: CanvasPaint::Transparent,
            layers: Vec::new(),
            bleed: InsetsPx::ZERO,
        }
    }
}

impl CanvasSpec {
    pub fn transparent() -> Self {
        Self::default()
    }

    pub fn solid(value: impl AsRef<str>) -> Result<Self, ThemeCompileValidationError> {
        Ok(Self {
            base: CanvasPaint::solid(value)?,
            ..Self::default()
        })
    }

    pub fn with_base(mut self, base: CanvasPaint) -> Self {
        self.base = base;
        self
    }

    pub fn with_layer(mut self, layer: CanvasLayer) -> Result<Self, ThemeCompileValidationError> {
        if self.layers.len() >= MAX_CANVAS_LAYERS {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "canvas.layers",
            });
        }
        layer.paint.validate("canvas.layer.paint")?;
        self.layers.push(layer);
        Ok(self)
    }

    pub fn with_bleed(mut self, bleed: InsetsPx) -> Result<Self, ThemeCompileValidationError> {
        bleed.validate("canvas.bleed")?;
        self.bleed = bleed;
        Ok(self)
    }

    pub const fn base(&self) -> &CanvasPaint {
        &self.base
    }

    pub fn layers(&self) -> &[CanvasLayer] {
        &self.layers
    }

    pub const fn bleed(&self) -> InsetsPx {
        self.bleed
    }

    pub(crate) fn validate(&self) -> Result<(), ThemeCompileValidationError> {
        self.base.validate("canvas.base")?;
        if self.layers.len() > MAX_CANVAS_LAYERS {
            return Err(ThemeCompileValidationError::LimitExceeded {
                field: "canvas.layers",
            });
        }
        for layer in &self.layers {
            layer.paint.validate("canvas.layer.paint")?;
            validate_unit_interval(layer.opacity, "canvas.layer.opacity")?;
        }
        self.bleed.validate("canvas.bleed")
    }
}

fn validate_gradient_stops(
    stops: &[GradientStop],
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if stops.len() < 2 || stops.len() > MAX_GRADIENT_STOPS {
        return Err(ThemeCompileValidationError::InvalidCollection { field });
    }
    if stops.windows(2).any(|pair| pair[0].offset > pair[1].offset) {
        return Err(ThemeCompileValidationError::InvalidCollection { field });
    }
    Ok(())
}

fn validate_positive(value: f32, field: &'static str) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

fn validate_unit_interval(
    value: f32,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}
