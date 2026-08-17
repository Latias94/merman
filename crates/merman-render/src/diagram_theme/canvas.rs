use std::borrow::Cow;

use merman_core::theme_color::{ColorChannel, ColorSourceFormat, ThemeColor};

use super::ThemeCompileValidationError;

pub(crate) const MAX_GRADIENT_STOPS: usize = 64;
const MAX_CANVAS_LAYERS: usize = 32;
const MIN_GRADIENT_GEOMETRY_PX: f32 = 1.0;
const MAX_GRADIENT_PERIOD_PX: f32 = 65_536.0;
const MAX_GRADIENT_TILE_EDGE_PX: f32 = 4_096.0;

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

    pub(crate) fn as_css_cow(&self) -> Cow<'_, str> {
        match self.0.source_format() {
            ColorSourceFormat::Hex | ColorSourceFormat::Rgb | ColorSourceFormat::Hsl => {
                Cow::Borrowed(
                    self.0
                        .raw()
                        .expect("parsed source-backed theme color must retain its input"),
                )
            }
            ColorSourceFormat::Keyword | ColorSourceFormat::ConstructedRgb => {
                Cow::Owned(self.0.stringify())
            }
        }
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
    repetition: GradientRepetition,
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
            repetition: GradientRepetition::None,
        })
    }

    /// Repeats the gradient along its authored angle using a 1..=65,536px user-space period.
    pub fn with_repeating_period_px(
        mut self,
        period_px: f32,
    ) -> Result<Self, ThemeCompileValidationError> {
        validate_gradient_period_px(period_px, "canvas.linear_gradient.repeating_period_px")?;
        self.repetition = GradientRepetition::Repeating {
            period_px: Some(period_px),
        };
        Ok(self)
    }

    /// Repeats one complete gradient tile with bounded 1..=4,096px edges.
    pub fn with_tile_px(
        mut self,
        width_px: f32,
        height_px: f32,
    ) -> Result<Self, ThemeCompileValidationError> {
        validate_gradient_tile_px(width_px, height_px, "canvas.linear_gradient.tile")?;
        self.repetition = GradientRepetition::Tiled {
            width_px,
            height_px,
        };
        Ok(self)
    }

    pub const fn angle_degrees(&self) -> f32 {
        self.angle_degrees
    }

    pub fn stops(&self) -> &[GradientStop] {
        &self.stops
    }

    pub const fn repeating_period_px(&self) -> Option<f32> {
        match self.repetition {
            GradientRepetition::Repeating {
                period_px: Some(period_px),
            } => Some(period_px),
            GradientRepetition::None
            | GradientRepetition::Repeating { period_px: None }
            | GradientRepetition::Tiled { .. } => None,
        }
    }

    pub const fn tile_size_px(&self) -> Option<(f32, f32)> {
        self.repetition.tile_size_px()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialGradient {
    center_x: ThemeLength,
    center_y: ThemeLength,
    radius: ThemeLength,
    stops: Vec<GradientStop>,
    repetition: GradientRepetition,
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
            repetition: GradientRepetition::None,
        })
    }

    /// Repeats the radial stops using the authored radius as the period.
    ///
    /// Pixel radii are bounded immediately. Percentage radii are resolved against terminal paint
    /// bounds and receive portable evidence only when the resulting period is 1..=65,536px.
    pub fn with_repeating(mut self) -> Result<Self, ThemeCompileValidationError> {
        match self.radius {
            ThemeLength::Px(radius_px) => {
                validate_gradient_period_px(radius_px, "canvas.radial_gradient.repeating_radius")?
            }
            ThemeLength::Percent(radius_percent) => {
                self.radius
                    .validate("canvas.radial_gradient.repeating_radius")?;
                if radius_percent <= 0.0 {
                    return Err(ThemeCompileValidationError::InvalidNumber {
                        field: "canvas.radial_gradient.repeating_radius",
                    });
                }
            }
        }
        self.repetition = GradientRepetition::Repeating { period_px: None };
        Ok(self)
    }

    /// Repeats one complete radial-gradient tile with bounded 1..=4,096px edges.
    pub fn with_tile_px(
        mut self,
        width_px: f32,
        height_px: f32,
    ) -> Result<Self, ThemeCompileValidationError> {
        validate_gradient_tile_px(width_px, height_px, "canvas.radial_gradient.tile")?;
        self.repetition = GradientRepetition::Tiled {
            width_px,
            height_px,
        };
        Ok(self)
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

    pub const fn is_repeating(&self) -> bool {
        matches!(
            self.repetition,
            GradientRepetition::Repeating { period_px: None }
        )
    }

    pub const fn tile_size_px(&self) -> Option<(f32, f32)> {
        self.repetition.tile_size_px()
    }

    pub(crate) fn resolved_repeating_radius_is_bounded(&self, radius_px: f64) -> bool {
        !self.is_repeating()
            || (radius_px.is_finite()
                && (f64::from(MIN_GRADIENT_GEOMETRY_PX)..=f64::from(MAX_GRADIENT_PERIOD_PX))
                    .contains(&radius_px))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GradientRepetition {
    None,
    Repeating { period_px: Option<f32> },
    Tiled { width_px: f32, height_px: f32 },
}

impl GradientRepetition {
    const fn tile_size_px(self) -> Option<(f32, f32)> {
        match self {
            Self::Tiled {
                width_px,
                height_px,
            } => Some((width_px, height_px)),
            Self::None | Self::Repeating { .. } => None,
        }
    }

    fn validate(self, field: &'static str) -> Result<(), ThemeCompileValidationError> {
        match self {
            Self::None | Self::Repeating { period_px: None } => Ok(()),
            Self::Repeating {
                period_px: Some(period_px),
            } => validate_gradient_period_px(period_px, field),
            Self::Tiled {
                width_px,
                height_px,
            } => validate_gradient_tile_px(width_px, height_px, field),
        }
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

    pub(crate) const fn requires_pattern_capability(&self) -> bool {
        match self {
            Self::LinearGradient(gradient) => {
                gradient.repeating_period_px().is_some() || gradient.tile_size_px().is_some()
            }
            Self::RadialGradient(gradient) => {
                gradient.is_repeating() || gradient.tile_size_px().is_some()
            }
            Self::Transparent | Self::Solid(_) | Self::Pattern(_) => false,
        }
    }

    pub(crate) fn validate(&self, field: &'static str) -> Result<(), ThemeCompileValidationError> {
        match self {
            Self::Transparent | Self::Solid(_) => Ok(()),
            Self::LinearGradient(gradient) => {
                validate_gradient_stops(gradient.stops(), field)?;
                gradient.repetition.validate(field)
            }
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
                validate_gradient_stops(gradient.stops(), field)?;
                gradient.repetition.validate(field)
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
    base_explicit: bool,
    layers: Vec<CanvasLayer>,
    bleed: InsetsPx,
}

impl Default for CanvasSpec {
    fn default() -> Self {
        Self {
            base: CanvasPaint::Transparent,
            base_explicit: false,
            layers: Vec::new(),
            bleed: InsetsPx::ZERO,
        }
    }
}

impl CanvasSpec {
    pub fn transparent() -> Self {
        Self {
            base_explicit: true,
            ..Self::default()
        }
    }

    pub fn solid(value: impl AsRef<str>) -> Result<Self, ThemeCompileValidationError> {
        Ok(Self::default().with_base(CanvasPaint::solid(value)?))
    }

    pub fn with_base(mut self, base: CanvasPaint) -> Self {
        self.base = base;
        self.base_explicit = true;
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

    /// Returns whether the recipe explicitly selected a base paint.
    ///
    /// A default canvas remains unspecified so Mermaid's historical root background can be kept;
    /// `CanvasSpec::transparent()` and `with_base(CanvasPaint::Transparent)` are explicit clear
    /// requests.
    pub const fn has_explicit_base(&self) -> bool {
        self.base_explicit
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

fn validate_gradient_period_px(
    value: f32,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    if !value.is_finite() || !(MIN_GRADIENT_GEOMETRY_PX..=MAX_GRADIENT_PERIOD_PX).contains(&value) {
        return Err(ThemeCompileValidationError::InvalidNumber { field });
    }
    Ok(())
}

fn validate_gradient_tile_px(
    width_px: f32,
    height_px: f32,
    field: &'static str,
) -> Result<(), ThemeCompileValidationError> {
    for value in [width_px, height_px] {
        if !value.is_finite()
            || !(MIN_GRADIENT_GEOMETRY_PX..=MAX_GRADIENT_TILE_EDGE_PX).contains(&value)
        {
            return Err(ThemeCompileValidationError::InvalidNumber { field });
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn color(value: &str) -> ThemeColorValue {
        ThemeColorValue::parse(value).expect("valid test color")
    }

    fn stops() -> [GradientStop; 2] {
        [
            GradientStop::new(0.0, color("#0f172a")).unwrap(),
            GradientStop::new(1.0, color("#f8fafc")).unwrap(),
        ]
    }

    #[test]
    fn source_backed_color_css_is_borrowed_for_svg_preflight() {
        let rgb = color("rgb(1,   2, 3)");
        assert!(matches!(rgb.as_css_cow(), Cow::Borrowed("rgb(1,   2, 3)")));

        let keyword = color("transparent");
        assert!(matches!(keyword.as_css_cow(), Cow::Owned(_)));
        assert_eq!(keyword.as_css_cow(), "#00000000");
    }

    #[test]
    fn repeating_linear_gradient_requires_a_positive_bounded_period() {
        let ordinary = LinearGradient::new(135.0, stops()).unwrap();
        assert_eq!(ordinary.repeating_period_px(), None);
        assert_eq!(ordinary.tile_size_px(), None);

        let repeated = ordinary
            .clone()
            .with_repeating_period_px(MAX_GRADIENT_PERIOD_PX)
            .unwrap();
        assert_eq!(repeated.repeating_period_px(), Some(MAX_GRADIENT_PERIOD_PX));
        assert_eq!(repeated.tile_size_px(), None);
        assert!(
            ordinary
                .clone()
                .with_repeating_period_px(MIN_GRADIENT_GEOMETRY_PX)
                .is_ok()
        );

        for period in [
            0.0,
            MIN_GRADIENT_GEOMETRY_PX / 2.0,
            -1.0,
            f32::NAN,
            f32::INFINITY,
            MAX_GRADIENT_PERIOD_PX + 1.0,
        ] {
            assert!(
                ordinary.clone().with_repeating_period_px(period).is_err(),
                "period {period:?} must fail closed"
            );
        }
    }

    #[test]
    fn tiled_gradient_geometry_is_positive_bounded_and_mutually_exclusive() {
        let linear = LinearGradient::new(90.0, stops())
            .unwrap()
            .with_repeating_period_px(16.0)
            .unwrap()
            .with_tile_px(24.0, 32.0)
            .unwrap();
        assert_eq!(linear.repeating_period_px(), None);
        assert_eq!(linear.tile_size_px(), Some((24.0, 32.0)));

        let radial = RadialGradient::new(
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            stops(),
        )
        .unwrap()
        .with_repeating()
        .unwrap()
        .with_tile_px(20.0, 20.0)
        .unwrap();
        assert!(!radial.is_repeating());
        assert_eq!(radial.tile_size_px(), Some((20.0, 20.0)));
        assert!(
            LinearGradient::new(90.0, stops())
                .unwrap()
                .with_tile_px(MAX_GRADIENT_TILE_EDGE_PX, MAX_GRADIENT_TILE_EDGE_PX)
                .is_ok()
        );

        for (width, height) in [
            (0.0, 16.0),
            (MIN_GRADIENT_GEOMETRY_PX / 2.0, 16.0),
            (16.0, 0.0),
            (-1.0, 16.0),
            (16.0, f32::NAN),
            (MAX_GRADIENT_TILE_EDGE_PX + 1.0, 16.0),
            (16.0, MAX_GRADIENT_TILE_EDGE_PX + 1.0),
        ] {
            assert!(
                LinearGradient::new(90.0, stops())
                    .unwrap()
                    .with_tile_px(width, height)
                    .is_err(),
                "tile {width:?}x{height:?} must fail closed"
            );
        }

        let zero_radius = RadialGradient::new(
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            ThemeLength::px(0.0),
            stops(),
        )
        .unwrap();
        assert!(zero_radius.with_repeating().is_err());

        let subpercent_radius = RadialGradient::new(
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            ThemeLength::percent(0.5),
            stops(),
        )
        .unwrap();
        assert!(subpercent_radius.with_repeating().is_ok());

        let unbounded_radius = RadialGradient::new(
            ThemeLength::percent(50.0),
            ThemeLength::percent(50.0),
            ThemeLength::px(MAX_GRADIENT_PERIOD_PX + 1.0),
            stops(),
        )
        .unwrap();
        assert!(unbounded_radius.with_repeating().is_err());
    }

    #[test]
    fn repeating_or_tiled_gradients_require_pattern_capability() {
        let ordinary = CanvasPaint::LinearGradient(LinearGradient::new(90.0, stops()).unwrap());
        let repeated = CanvasPaint::LinearGradient(
            LinearGradient::new(90.0, stops())
                .unwrap()
                .with_repeating_period_px(16.0)
                .unwrap(),
        );
        let tiled = CanvasPaint::RadialGradient(
            RadialGradient::new(
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                ThemeLength::percent(50.0),
                stops(),
            )
            .unwrap()
            .with_tile_px(20.0, 20.0)
            .unwrap(),
        );

        assert!(!ordinary.requires_pattern_capability());
        assert!(repeated.requires_pattern_capability());
        assert!(tiled.requires_pattern_capability());
    }
}
