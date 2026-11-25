use super::color::{
    dot, luminance_gradient, luminance_ratio, normalize_channel, relative_luminance,
};
use super::{Appearance, Color, GradientStop, Paint};

pub(super) const TEXT_CONTRAST: f64 = 4.5;
pub(super) const DARK_INK: Color = Color::rgb(15, 23, 42);
pub(super) const BLACK: Color = Color::rgb(0, 0, 0);
const LIGHT_INK: Color = Color::rgb(248, 250, 252);
const WHITE: Color = Color::rgb(255, 255, 255);

const CONTRAST_ACCURACY: f64 = 0.001;
const MAX_SUBDIVISIONS_PER_CELL: usize = 64;
// Covers the rounding of the sRGB transfer function's splice point and floating-point arithmetic.
const LUMINANCE_EPSILON: f64 = 1e-8;

type Rgb = [f64; 3];

impl Paint {
    /// Estimates the minimum WCAG contrast of this paint over `background`.
    ///
    /// Use paints with validated gradient stops. The background is composited at the given
    /// opacity over the optional canvas, with a white or black backdrop for light or dark
    /// appearances. The calculation follows each SVG sRGB gradient segment and allows
    /// independently positioned gradients to overlap at any phase. For gradients it returns a
    /// lower bound, refined to within 0.001 of an observed ratio when the budget permits.
    pub fn minimum_contrast_over(
        &self,
        background: &Self,
        background_opacity: f64,
        canvas: Option<&Self>,
        appearance: Appearance,
    ) -> f64 {
        self.minimum_contrast_over_backdrop(
            background,
            background_opacity,
            canvas,
            backdrop(appearance),
        )
    }

    fn minimum_contrast_over_backdrop(
        &self,
        background: &Self,
        background_opacity: f64,
        canvas: Option<&Self>,
        backdrop: Color,
    ) -> f64 {
        let opacity = background_opacity.clamp(0.0, 1.0);
        let transparent = Self::Solid(Color::TRANSPARENT);
        let canvas = canvas
            .filter(|_| !is_opaque(background, opacity))
            .unwrap_or(&transparent);
        if let (Self::Solid(foreground), Self::Solid(background), Self::Solid(canvas)) =
            (self, background, canvas)
        {
            let background = background.composite(canvas.composite(backdrop, 1.0), opacity);
            return foreground.composite(background, 1.0).contrast(background);
        }

        let backdrop = Rgba::new(backdrop, 1.0).rgb;
        let mut minimum: f64 = 21.0;
        for canvas_segment in segments(canvas, 1.0) {
            for background_segment in segments(background, opacity) {
                for foreground_segment in segments(self, 1.0) {
                    let cell = [canvas_segment, background_segment, foreground_segment];
                    let mut search = ContrastSearch::new(backdrop);
                    search.measure_endpoints(cell);
                    let observed = search.observed_minimum();
                    if observed == 1.0 {
                        return observed;
                    }
                    if cell.iter().all(|span| span.0 == span.1) {
                        minimum = minimum.min(observed);
                        continue;
                    }
                    let lower = search.measure_bounds(cell);
                    if lower < minimum {
                        minimum = minimum.min(search.refine(cell, lower));
                    }
                    if minimum == 1.0 {
                        return minimum;
                    }
                }
            }
        }
        minimum
    }
}

pub(super) const fn backdrop(appearance: Appearance) -> Color {
    match appearance {
        Appearance::Light => WHITE,
        Appearance::Dark => BLACK,
    }
}

pub(super) fn automatic_fill_ink_over(
    fill: Color,
    canvas: &Paint,
    opacity: f64,
    backdrop: Color,
) -> Color {
    let fill = Paint::Solid(fill);
    select_fill_ink(|ink| {
        Paint::Solid(ink).minimum_contrast_over_backdrop(&fill, opacity, Some(canvas), backdrop)
    })
}

fn select_fill_ink(contrast: impl Fn(Color) -> f64) -> Color {
    let dark = contrast(DARK_INK);
    let light = contrast(LIGHT_INK);
    if dark.max(light) >= TEXT_CONTRAST {
        return if dark >= light { DARK_INK } else { LIGHT_INK };
    }
    if contrast(BLACK) >= contrast(WHITE) {
        BLACK
    } else {
        WHITE
    }
}

#[cfg(feature = "compile")]
pub(super) fn best_ink(background: Color, first: Color, second: Color) -> Color {
    if first.contrast(background) >= second.contrast(background) {
        first
    } else {
        second
    }
}

pub(super) fn is_opaque(paint: &Paint, opacity: f64) -> bool {
    opacity == 1.0
        && match paint {
            Paint::Solid(color) => color.channels()[3] == 255,
            Paint::LinearGradient(gradient) => {
                !gradient.stops.is_empty()
                    && gradient
                        .stops
                        .iter()
                        .all(|stop| stop.opacity == 1.0 && stop.color.channels()[3] == 255)
            }
        }
}

#[derive(Clone, Copy, PartialEq)]
struct Rgba {
    rgb: Rgb,
    alpha: f64,
}

impl Rgba {
    fn new(color: Color, opacity: f64) -> Self {
        let [r, g, b, a] = color.channels().map(normalize_channel);
        Self {
            rgb: [r, g, b],
            alpha: a * opacity,
        }
    }

    fn over(self, background: Rgb) -> Rgb {
        std::array::from_fn(|index| {
            self.rgb[index] * self.alpha + background[index] * (1.0 - self.alpha)
        })
    }

    fn midpoint(self, other: Self) -> Self {
        Self {
            rgb: midpoint(self.rgb, other.rgb),
            alpha: self.alpha.midpoint(other.alpha),
        }
    }
}

fn midpoint(a: Rgb, b: Rgb) -> Rgb {
    std::array::from_fn(|index| a[index].midpoint(b[index]))
}

#[derive(Clone, Copy)]
struct Segment(Rgba, Rgba);

impl Segment {
    fn solid(color: Rgba) -> Self {
        Self(color, color)
    }

    fn midpoint(self) -> Rgba {
        self.0.midpoint(self.1)
    }

    fn variation(self) -> f64 {
        self.0
            .rgb
            .into_iter()
            .zip(self.1.rgb)
            .map(|(a, b)| (a - b).abs())
            .fold((self.0.alpha - self.1.alpha).abs(), f64::max)
    }

    fn endpoints(self) -> impl Iterator<Item = Rgba> {
        std::iter::once(self.0).chain((self.0 != self.1).then_some(self.1))
    }

    fn control_colors(self, background: Rgb) -> impl Iterator<Item = Rgb> {
        let Self(start, end) = self;
        // SVG interpolates straight sRGB and opacity independently. Compositing multiplies the
        // two linear functions, so the resulting RGB follows a quadratic Bézier curve.
        let rest = (start != end).then(|| {
            let cross_start = Rgba {
                alpha: end.alpha,
                ..start
            };
            let cross_end = Rgba {
                alpha: start.alpha,
                ..end
            };
            [
                midpoint(cross_start.over(background), cross_end.over(background)),
                end.over(background),
            ]
        });
        std::iter::once(start.over(background)).chain(rest.into_iter().flatten())
    }
}

fn segments(paint: &Paint, opacity: f64) -> impl Iterator<Item = Segment> + '_ {
    let rgba = move |stop: &GradientStop| Rgba::new(stop.color, stop.opacity * opacity);
    let (first, stops, last) = match paint {
        Paint::Solid(color) => (Rgba::new(*color, opacity), &[][..], None),
        Paint::LinearGradient(gradient) => (
            gradient
                .stops
                .first()
                .map_or(Rgba::new(Color::TRANSPARENT, 1.0), rgba),
            gradient.stops.as_slice(),
            gradient.stops.last().map(rgba),
        ),
    };
    // Pad with the end colors and skip zero-width intervals at hard stops.
    std::iter::once(Segment::solid(first))
        .chain(
            stops
                .windows(2)
                .filter(|pair| pair[0].offset < pair[1].offset)
                .map(move |pair| Segment(rgba(&pair[0]), rgba(&pair[1]))),
        )
        .chain(last.map(Segment::solid))
}

struct LuminanceBounds {
    slope: Rgb,
    intercept: f64,
    minimum: f64,
    maximum: f64,
}

impl LuminanceBounds {
    fn new(rgb: Rgb, luminance: f64) -> Self {
        let slope = luminance_gradient(rgb);
        Self {
            slope,
            intercept: luminance - dot(rgb, slope),
            minimum: luminance,
            maximum: luminance,
        }
    }

    fn include(&mut self, control: Rgb) {
        // Composited RGB stays inside the control colors' convex hull, including when the
        // underlay varies. Luminance is convex, so a tangent bounds it below and the brightest
        // control color bounds it above.
        self.minimum = self.minimum.min(self.intercept + dot(control, self.slope));
        self.maximum = self.maximum.max(relative_luminance(control));
    }
}

struct ContrastSearch {
    backdrop: Rgb,
    smallest_ratio: f64,
    greatest_ratio: f64,
    splits_left: usize,
}

impl ContrastSearch {
    fn new(backdrop: Rgb) -> Self {
        Self {
            backdrop,
            smallest_ratio: 21.0,
            greatest_ratio: 1.0 / 21.0,
            splits_left: MAX_SUBDIVISIONS_PER_CELL,
        }
    }

    fn observe(&mut self, foreground: f64, background: f64) {
        let ratio = luminance_ratio(foreground, background);
        self.smallest_ratio = self.smallest_ratio.min(ratio);
        self.greatest_ratio = self.greatest_ratio.max(ratio);
    }

    fn observed_minimum(&self) -> f64 {
        // If the lighter paint changes inside this continuous cell, the luminances coincide.
        self.smallest_ratio
            .max(self.greatest_ratio.recip())
            .max(1.0)
    }

    fn measure_endpoints(&mut self, [canvas, background, foreground]: [Segment; 3]) {
        for canvas in canvas.endpoints() {
            for background in background.endpoints() {
                let background = background.over(canvas.over(self.backdrop));
                let background_luminance = relative_luminance(background);
                for foreground in foreground.endpoints() {
                    self.observe(
                        relative_luminance(foreground.over(background)),
                        background_luminance,
                    );
                }
            }
        }
    }

    fn measure_bounds(&mut self, [canvas, background, foreground]: [Segment; 3]) -> f64 {
        let bg = background
            .midpoint()
            .over(canvas.midpoint().over(self.backdrop));
        let fg = foreground.midpoint().over(bg);
        let bg_luminance = relative_luminance(bg);
        let fg_luminance = relative_luminance(fg);
        self.observe(fg_luminance, bg_luminance);
        let mut bg_bounds = LuminanceBounds::new(bg, bg_luminance);
        let mut fg_bounds = LuminanceBounds::new(fg, fg_luminance);
        for canvas in canvas.control_colors(self.backdrop) {
            for background in background.control_colors(canvas) {
                bg_bounds.include(background);
                for foreground in foreground.control_colors(background) {
                    fg_bounds.include(foreground);
                }
            }
        }
        let bound = |lighter: &LuminanceBounds, darker: &LuminanceBounds| {
            luminance_ratio(
                (lighter.minimum - LUMINANCE_EPSILON).max(0.0),
                darker.maximum + LUMINANCE_EPSILON,
            )
        };
        bound(&fg_bounds, &bg_bounds)
            .max(bound(&bg_bounds, &fg_bounds))
            .max(1.0)
    }

    fn refine(&mut self, cell: [Segment; 3], lower: f64) -> f64 {
        let observed = self.observed_minimum();
        if observed == 1.0 {
            return observed;
        }
        if lower + CONTRAST_ACCURACY >= observed || self.splits_left == 0 {
            return lower.min(observed);
        }

        let Some((axis, span)) = cell
            .into_iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.variation().total_cmp(&b.variation()))
        else {
            return lower;
        };
        self.splits_left -= 1;
        let middle = span.midpoint();
        let mut first = cell;
        let mut second = cell;
        first[axis] = Segment(span.0, middle);
        second[axis] = Segment(middle, span.1);
        let mut first = (first, self.measure_bounds(first));
        let mut second = (second, self.measure_bounds(second));
        if first.1 > second.1 {
            std::mem::swap(&mut first, &mut second);
        }
        let refined = self
            .refine(first.0, first.1)
            .min(self.refine(second.0, second.1));
        lower.max(refined).min(self.observed_minimum())
    }
}
