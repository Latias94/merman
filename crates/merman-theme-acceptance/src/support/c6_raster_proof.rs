//! Bounded PNG decoding and pixel observers for host-dependent preset qualification.

use super::{C6ProofError, C6ProofResult};
use merman_export::{DEFAULT_MAX_RASTER_PIXELS, DEFAULT_MAX_RASTER_SIDE_LENGTH, RasterPlan};
use std::io::Cursor;

const MAX_RASTER_ARTIFACT_OVERHEAD_BYTES: usize = 1024 * 1024;

pub(crate) fn decode_bounded_png_artifact(
    bytes: &[u8],
    raster_plan: RasterPlan,
) -> C6ProofResult<RasterImage> {
    let raster = RasterImage::decode_png(bytes, raster_plan)?;
    c6_ensure!(
        "png-artifact",
        raster.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0),
        "decoded PNG artifact is fully transparent"
    );
    Ok(raster)
}

pub(crate) fn parse_c6_hex_rgb(value: &str) -> C6ProofResult<[u8; 3]> {
    let color = Rgb::parse_hex(value)?;
    Ok([color.red, color.green, color.blue])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    fn parse_hex(value: &str) -> C6ProofResult<Self> {
        let Some(hex) = value.strip_prefix('#') else {
            return Err(C6ProofError::new(
                "raster-contract",
                format!("expected a six-digit hex color, got {value}"),
            ));
        };
        let bytes = hex.as_bytes();
        let [
            red_high,
            red_low,
            green_high,
            green_low,
            blue_high,
            blue_low,
        ] = bytes
        else {
            return Err(C6ProofError::new(
                "raster-contract",
                format!("expected a six-digit hex color, got {value}"),
            ));
        };
        let parse_channel = |high: u8, low: u8, name| {
            let digits = [high, low];
            let digits = std::str::from_utf8(&digits).map_err(|error| {
                C6ProofError::new(
                    "raster-contract",
                    format!("invalid {name} channel in {value}: {error}"),
                )
            })?;
            u8::from_str_radix(digits, 16).map_err(|error| {
                C6ProofError::new(
                    "raster-contract",
                    format!("invalid {name} channel in {value}: {error}"),
                )
            })
        };
        Ok(Self {
            red: parse_channel(*red_high, *red_low, "red")?,
            green: parse_channel(*green_high, *green_low, "green")?,
            blue: parse_channel(*blue_high, *blue_low, "blue")?,
        })
    }

    fn linear_rgb_roundtrip(self) -> Self {
        Self {
            red: srgb_u8_linear_roundtrip(self.red),
            green: srgb_u8_linear_roundtrip(self.green),
            blue: srgb_u8_linear_roundtrip(self.blue),
        }
    }

    fn max_channel_delta(self, other: Self) -> u8 {
        self.red
            .abs_diff(other.red)
            .max(self.green.abs_diff(other.green))
            .max(self.blue.abs_diff(other.blue))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

impl Rect {
    fn right(self) -> f64 {
        self.left + self.width
    }

    fn bottom(self) -> f64 {
        self.top + self.height
    }

    fn center_x(self) -> f64 {
        self.left + self.width / 2.0
    }

    fn center_y(self) -> f64 {
        self.top + self.height / 2.0
    }

    fn expanded(self, amount: f64) -> Self {
        Self {
            left: self.left - amount,
            top: self.top - amount,
            width: self.width + amount * 2.0,
            height: self.height + amount * 2.0,
        }
    }

    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.left && x <= self.right() && y >= self.top && y <= self.bottom()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RasterImage {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl RasterImage {
    pub(super) fn decode_png(bytes: &[u8], plan: RasterPlan) -> C6ProofResult<Self> {
        let planned = PlannedRaster::new(plan, 4, "png-decode")?;
        planned.require_bounded_artifact(bytes, "png-decode")?;
        let decoder_limit = planned
            .decoded_bytes
            .checked_add(MAX_RASTER_ARTIFACT_OVERHEAD_BYTES)
            .ok_or_else(|| C6ProofError::new("png-decode", "PNG decoder limit overflowed"))?;
        let decoder = png::Decoder::new_with_limits(
            Cursor::new(bytes),
            png::Limits {
                bytes: decoder_limit,
            },
        );
        let mut reader = decoder.read_info().map_err(|error| {
            C6ProofError::new(
                "png-decode",
                format!("failed to decode C6 PNG header: {error}"),
            )
        })?;
        let header = reader.info();
        planned.require_dimensions(header.width, header.height, "png-decode")?;
        c6_ensure!(
            "png-decode",
            header.color_type == png::ColorType::Rgba && header.bit_depth == png::BitDepth::Eight,
            "C6 PNG header must describe 8-bit RGBA, got {:?} {:?}",
            header.color_type,
            header.bit_depth
        );
        let output_buffer_size = reader.output_buffer_size().ok_or_else(|| {
            C6ProofError::new("png-decode", "C6 PNG output buffer size is unbounded")
        })?;
        c6_ensure!(
            "png-decode",
            output_buffer_size == planned.decoded_bytes,
            "C6 PNG output buffer does not match its frozen raster plan; decoded={output_buffer_size}, planned={}",
            planned.decoded_bytes
        );
        let mut buffer = zeroed_bytes(planned.decoded_bytes, "png-decode")?;
        let frame = reader.next_frame(&mut buffer).map_err(|error| {
            C6ProofError::new(
                "png-decode",
                format!("failed to decode C6 PNG frame: {error}"),
            )
        })?;
        planned.require_dimensions(frame.width, frame.height, "png-decode")?;
        c6_ensure!(
            "png-decode",
            frame.color_type == png::ColorType::Rgba && frame.bit_depth == png::BitDepth::Eight,
            "C6 PNG must decode as 8-bit RGBA, got {:?} {:?}",
            frame.color_type,
            frame.bit_depth
        );
        let frame_buffer_size = frame.buffer_size();
        c6_ensure!(
            "png-decode",
            frame_buffer_size == planned.decoded_bytes,
            "decoded C6 PNG frame does not match its frozen raster plan; frame={frame_buffer_size}, planned={}",
            planned.decoded_bytes
        );
        Ok(Self {
            width: frame.width,
            height: frame.height,
            pixels: buffer,
        })
    }

    fn transform(&self, view_box: Rect) -> RasterTransform {
        RasterTransform {
            view_box,
            width: self.width,
            height: self.height,
        }
    }

    fn rgb(&self, x: i32, y: i32) -> Option<Rgb> {
        let x = u32::try_from(x).ok()?;
        let y = u32::try_from(y).ok()?;
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = usize::try_from(u64::from(y) * u64::from(self.width) + u64::from(x)).ok()?;
        let pixel = self.rgba_pixel(index)?;
        (pixel[3] == u8::MAX).then_some(Rgb {
            red: pixel[0],
            green: pixel[1],
            blue: pixel[2],
        })
    }

    fn rgba_pixel(&self, index: usize) -> Option<[u8; 4]> {
        let start = index.checked_mul(4)?;
        let pixel = self.pixels.get(start..start.checked_add(4)?)?;
        Some([pixel[0], pixel[1], pixel[2], pixel[3]])
    }

    pub(crate) const fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[cfg(test)]
    pub(crate) fn solid_for_test(width: u32, height: u32, color: [u8; 3]) -> Self {
        Self {
            width,
            height,
            pixels: [color[0], color[1], color[2], u8::MAX]
                .repeat(width as usize * height as usize),
        }
    }

    #[cfg(test)]
    pub(crate) fn set_rgb_for_test(&mut self, x: u32, y: u32, color: [u8; 3]) {
        assert!(x < self.width && y < self.height);
        let start = ((y * self.width + x) * 4) as usize;
        self.pixels[start..start + 4].copy_from_slice(&[color[0], color[1], color[2], u8::MAX]);
    }

    pub(crate) fn count_opaque_pixels_near(&self, expected: [u8; 3], tolerance: u8) -> usize {
        self.pixels
            .chunks_exact(4)
            .filter(|pixel| {
                pixel[3] != 0
                    && pixel[0].abs_diff(expected[0]) <= tolerance
                    && pixel[1].abs_diff(expected[1]) <= tolerance
                    && pixel[2].abs_diff(expected[2]) <= tolerance
            })
            .count()
    }

    pub(crate) fn count_opaque_pixels_near_in_svg_rect(
        &self,
        view_box: [f64; 4],
        rect: [f64; 4],
        expected: [u8; 3],
        tolerance: u8,
    ) -> Option<usize> {
        let [view_left, view_top, view_width, view_height] = view_box;
        let [rect_left, rect_top, rect_width, rect_height] = rect;
        if !view_box.into_iter().chain(rect).all(f64::is_finite)
            || view_width <= 0.0
            || view_height <= 0.0
            || rect_width <= 0.0
            || rect_height <= 0.0
            || self.width == 0
            || self.height == 0
        {
            return None;
        }

        let rect_right = rect_left + rect_width;
        let rect_bottom = rect_top + rect_height;
        let width = f64::from(self.width);
        let height = f64::from(self.height);
        let mut count = 0usize;
        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            if pixel[3] == 0
                || pixel[0].abs_diff(expected[0]) > tolerance
                || pixel[1].abs_diff(expected[1]) > tolerance
                || pixel[2].abs_diff(expected[2]) > tolerance
            {
                continue;
            }
            let index = u64::try_from(index).ok()?;
            let x = u32::try_from(index % u64::from(self.width)).ok()?;
            let y = u32::try_from(index / u64::from(self.width)).ok()?;
            let svg_x = view_left + (f64::from(x) + 0.5) * view_width / width;
            let svg_y = view_top + (f64::from(y) + 0.5) * view_height / height;
            if svg_x >= rect_left
                && svg_x <= rect_right
                && svg_y >= rect_top
                && svg_y <= rect_bottom
            {
                count += 1;
            }
        }
        Some(count)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_opaque_color_coverage_in_svg_rect(
        &self,
        view_box: [f64; 4],
        rect: [f64; 4],
        expected: [u8; 3],
        tolerance: u8,
        minimum_coverage: f64,
        stage: &'static str,
        label: &str,
    ) -> C6ProofResult<()> {
        let [view_left, view_top, view_width, view_height] = view_box;
        let [left, top, width, height] = rect;
        c6_ensure!(
            stage,
            view_box.into_iter().chain(rect).all(f64::is_finite)
                && view_width > 0.0
                && view_height > 0.0
                && width > 0.0
                && height > 0.0
                && self.width > 0
                && self.height > 0,
            "invalid {label} C6 coverage region"
        );
        let transform = self.transform(Rect {
            left: view_left,
            top: view_top,
            width: view_width,
            height: view_height,
        });
        prove_color_region_coverage(
            stage,
            self,
            transform,
            Rect {
                left,
                top,
                width,
                height,
            },
            |_, _| true,
            &[Rgb {
                red: expected[0],
                green: expected[1],
                blue: expected[2],
            }],
            tolerance,
            minimum_coverage,
            label,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prove_opaque_color_difference_coverage_in_svg_rect(
        &self,
        view_box: [f64; 4],
        rect: [f64; 4],
        reference: [u8; 3],
        minimum_difference: u8,
        minimum_coverage: f64,
        stage: &'static str,
        label: &str,
    ) -> C6ProofResult<()> {
        let [view_left, view_top, view_width, view_height] = view_box;
        let [left, top, width, height] = rect;
        c6_ensure!(
            stage,
            view_box.into_iter().chain(rect).all(f64::is_finite)
                && view_width > 0.0
                && view_height > 0.0
                && width > 0.0
                && height > 0.0
                && minimum_difference > 0
                && minimum_coverage > 0.0
                && minimum_coverage <= 1.0
                && self.width > 0
                && self.height > 0,
            "invalid {label} C6 difference region"
        );
        let transform = self.transform(Rect {
            left: view_left,
            top: view_top,
            width: view_width,
            height: view_height,
        });
        let bounds = Rect {
            left,
            top,
            width,
            height,
        };
        let mut matching = 0usize;
        let mut total = 0usize;
        let pixel_bounds = transform.pixel_bounds(bounds);
        for y in pixel_bounds.top..pixel_bounds.bottom {
            for x in pixel_bounds.left..pixel_bounds.right {
                let (user_x, user_y) = transform.user_point(x, y);
                if !bounds.contains(user_x, user_y) {
                    continue;
                }
                total += 1;
                let Some(actual) = self.rgb(x, y) else {
                    continue;
                };
                let difference = actual
                    .red
                    .abs_diff(reference[0])
                    .max(actual.green.abs_diff(reference[1]))
                    .max(actual.blue.abs_diff(reference[2]));
                matching += usize::from(difference >= minimum_difference);
            }
        }
        c6_ensure!(stage, total > 0, "{label} C6 region must contain pixels");
        let coverage = matching as f64 / total as f64;
        c6_ensure!(
            stage,
            coverage >= minimum_coverage,
            "{label} must visibly differ from its reference color across at least {:.0}% of its C6 region; coverage={coverage:.3}, matching={matching}, total={total}, raster={}x{}",
            minimum_coverage * 100.0,
            self.width,
            self.height,
        );
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn vertical_opaque_color_run_at_svg_point(
        &self,
        view_box: [f64; 4],
        point: [f64; 2],
        expected: [u8; 3],
        tolerance: u8,
        radius: usize,
        stage: &'static str,
        label: &str,
    ) -> C6ProofResult<usize> {
        let [view_left, view_top, view_width, view_height] = view_box;
        c6_ensure!(
            stage,
            view_box.into_iter().chain(point).all(f64::is_finite)
                && view_width > 0.0
                && view_height > 0.0
                && self.width > 0
                && self.height > 0,
            "invalid {label} C6 color-run projection"
        );
        let transform = self.transform(Rect {
            left: view_left,
            top: view_top,
            width: view_width,
            height: view_height,
        });
        contiguous_filtered_color_run(
            self,
            transform.point(point[0], point[1]),
            radius,
            Rgb {
                red: expected[0],
                green: expected[1],
                blue: expected[2],
            },
            tolerance,
            stage,
            label,
        )
    }

    pub(crate) fn rounded_corner_coverage_in_svg_rect(
        &self,
        view_box: [f64; 4],
        rect: [f64; 4],
        radius: f64,
        stroke_width: f64,
        canvas: [u8; 3],
        tolerance: u8,
    ) -> Option<RoundedCornerCoverage> {
        let [view_left, view_top, view_width, view_height] = view_box;
        let [left, top, width, height] = rect;
        if !view_box
            .into_iter()
            .chain(rect)
            .chain([radius, stroke_width])
            .all(f64::is_finite)
            || view_width <= 0.0
            || view_height <= 0.0
            || width <= 0.0
            || height <= 0.0
            || radius <= 0.0
            || stroke_width < 0.0
        {
            return None;
        }

        let transform = self.transform(Rect {
            left: view_left,
            top: view_top,
            width: view_width,
            height: view_height,
        });
        let rounded_rect = Rect {
            left,
            top,
            width,
            height,
        }
        .expanded(stroke_width / 2.0);
        let outer_radius = radius + stroke_width / 2.0;
        let patch_size = outer_radius + 2.0;
        Some(rounded_corner_coverage(
            self,
            transform,
            Rect {
                left: rounded_rect.left,
                top: rounded_rect.top,
                width: patch_size,
                height: patch_size,
            },
            rounded_rect,
            outer_radius,
            Rgb {
                red: canvas[0],
                green: canvas[1],
                blue: canvas[2],
            },
            tolerance,
        ))
    }
}

#[derive(Clone, Copy)]
struct PlannedRaster {
    width: u32,
    height: u32,
    decoded_bytes: usize,
}

impl PlannedRaster {
    fn new(plan: RasterPlan, channels: usize, stage: &'static str) -> C6ProofResult<Self> {
        c6_ensure!(
            stage,
            plan.width_px > 0 && plan.height_px > 0,
            "the frozen raster plan must have non-zero dimensions"
        );
        c6_ensure!(
            stage,
            plan.width_px <= DEFAULT_MAX_RASTER_SIDE_LENGTH
                && plan.height_px <= DEFAULT_MAX_RASTER_SIDE_LENGTH,
            "the frozen raster plan exceeds the default side limit; plan={}x{}, max={DEFAULT_MAX_RASTER_SIDE_LENGTH}",
            plan.width_px,
            plan.height_px
        );
        let pixel_count = u64::from(plan.width_px)
            .checked_mul(u64::from(plan.height_px))
            .ok_or_else(|| C6ProofError::new(stage, "raster pixel count overflowed"))?;
        c6_ensure!(
            stage,
            pixel_count <= DEFAULT_MAX_RASTER_PIXELS,
            "the frozen raster plan exceeds the default pixel limit; plan={pixel_count}, max={DEFAULT_MAX_RASTER_PIXELS}"
        );
        let pixel_count = usize::try_from(pixel_count)
            .map_err(|error| C6ProofError::new(stage, error.to_string()))?;
        let decoded_bytes = pixel_count
            .checked_mul(channels)
            .ok_or_else(|| C6ProofError::new(stage, "decoded raster byte count overflowed"))?;
        Ok(Self {
            width: plan.width_px,
            height: plan.height_px,
            decoded_bytes,
        })
    }

    fn require_dimensions(self, width: u32, height: u32, stage: &'static str) -> C6ProofResult<()> {
        c6_ensure!(
            stage,
            width == self.width && height == self.height,
            "artifact dimensions do not match the frozen raster plan; artifact={width}x{height}, plan={}x{}",
            self.width,
            self.height
        );
        Ok(())
    }

    fn require_bounded_artifact(self, bytes: &[u8], stage: &'static str) -> C6ProofResult<()> {
        let max_artifact_bytes = self
            .decoded_bytes
            .checked_add(MAX_RASTER_ARTIFACT_OVERHEAD_BYTES)
            .ok_or_else(|| C6ProofError::new(stage, "raster artifact limit overflowed"))?;
        c6_ensure!(
            stage,
            bytes.len() <= max_artifact_bytes,
            "raster artifact exceeds the plan-bound byte limit; actual={}, max={max_artifact_bytes}",
            bytes.len()
        );
        Ok(())
    }
}

fn zeroed_bytes(len: usize, stage: &'static str) -> C6ProofResult<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(|error| {
        C6ProofError::new(
            stage,
            format!("reserve the bounded raster output buffer: {error}"),
        )
    })?;
    bytes.resize(len, 0);
    Ok(bytes)
}

#[derive(Clone, Copy)]
struct RasterTransform {
    view_box: Rect,
    width: u32,
    height: u32,
}

impl RasterTransform {
    fn scale_x(self) -> f64 {
        f64::from(self.width) / self.view_box.width
    }

    fn scale_y(self) -> f64 {
        f64::from(self.height) / self.view_box.height
    }

    fn point(self, x: f64, y: f64) -> (i32, i32) {
        let x = ((x - self.view_box.left) * self.scale_x()).floor();
        let y = ((y - self.view_box.top) * self.scale_y()).floor();
        (
            x.clamp(0.0, f64::from(self.width.saturating_sub(1))) as i32,
            y.clamp(0.0, f64::from(self.height.saturating_sub(1))) as i32,
        )
    }

    fn user_point(self, x: i32, y: i32) -> (f64, f64) {
        (
            self.view_box.left + (f64::from(x) + 0.5) / self.scale_x(),
            self.view_box.top + (f64::from(y) + 0.5) / self.scale_y(),
        )
    }

    fn pixel_bounds(self, bounds: Rect) -> PixelBounds {
        let left = ((bounds.left - self.view_box.left) * self.scale_x()).floor();
        let top = ((bounds.top - self.view_box.top) * self.scale_y()).floor();
        let right = ((bounds.right() - self.view_box.left) * self.scale_x()).ceil();
        let bottom = ((bounds.bottom() - self.view_box.top) * self.scale_y()).ceil();
        PixelBounds {
            left: left.clamp(0.0, f64::from(self.width)) as i32,
            top: top.clamp(0.0, f64::from(self.height)) as i32,
            right: right.clamp(0.0, f64::from(self.width)) as i32,
            bottom: bottom.clamp(0.0, f64::from(self.height)) as i32,
        }
    }
}

#[derive(Clone, Copy)]
struct PixelBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn prove_color_region_coverage(
    stage: &'static str,
    raster: &RasterImage,
    transform: RasterTransform,
    bounds: Rect,
    include: impl Fn(f64, f64) -> bool,
    expected: &[Rgb],
    tolerance: u8,
    minimum_coverage: f64,
    label: &str,
) -> C6ProofResult<()> {
    let mut matching = 0usize;
    let mut total = 0usize;
    let pixel_bounds = transform.pixel_bounds(bounds);
    for y in pixel_bounds.top..pixel_bounds.bottom {
        for x in pixel_bounds.left..pixel_bounds.right {
            let (user_x, user_y) = transform.user_point(x, y);
            if !bounds.contains(user_x, user_y) || !include(user_x, user_y) {
                continue;
            }
            total += 1;
            if matches_any_color(raster.rgb(x, y), expected, tolerance) {
                matching += 1;
            }
        }
    }
    c6_ensure!(stage, total > 0, "{label} C6 region must contain pixels");
    let coverage = matching as f64 / total as f64;
    c6_ensure!(
        stage,
        coverage >= minimum_coverage,
        "{label} must cover at least {:.0}% of its C6 region; coverage={coverage:.3}, matching={matching}, total={total}, raster={}x{}",
        minimum_coverage * 100.0,
        raster.width,
        raster.height,
    );
    Ok(())
}

#[derive(Default)]
pub(crate) struct RoundedCornerCoverage {
    inside_matching: usize,
    inside_total: usize,
    outside_matching: usize,
    outside_total: usize,
}

impl RoundedCornerCoverage {
    pub(crate) const fn inside_total(&self) -> usize {
        self.inside_total
    }

    pub(crate) const fn outside_total(&self) -> usize {
        self.outside_total
    }

    pub(crate) fn inside_ratio(&self) -> f64 {
        self.inside_matching as f64 / self.inside_total as f64
    }

    pub(crate) fn outside_ratio(&self) -> f64 {
        self.outside_matching as f64 / self.outside_total as f64
    }
}

fn rounded_corner_coverage(
    raster: &RasterImage,
    transform: RasterTransform,
    patch: Rect,
    rounded_rect: Rect,
    radius: f64,
    canvas: Rgb,
    tolerance: u8,
) -> RoundedCornerCoverage {
    let mut coverage = RoundedCornerCoverage::default();
    let pixel_bounds = transform.pixel_bounds(patch);
    // Filtered groups are rasterized offscreen and composited, so exclude the
    // pixel footprint plus the bounded resampling fringe from arc evidence.
    let pixel_boundary_guard = (1.0 / transform.scale_x()).hypot(1.0 / transform.scale_y());
    for y in pixel_bounds.top..pixel_bounds.bottom {
        for x in pixel_bounds.left..pixel_bounds.right {
            let (user_x, user_y) = transform.user_point(x, y);
            let distance = rounded_rect_signed_distance(user_x, user_y, rounded_rect, radius);
            if distance <= -pixel_boundary_guard {
                coverage.inside_total += 1;
                if !matches_any_color(raster.rgb(x, y), &[canvas], tolerance) {
                    coverage.inside_matching += 1;
                }
            } else if distance >= pixel_boundary_guard {
                coverage.outside_total += 1;
                if matches_any_color(raster.rgb(x, y), &[canvas], tolerance) {
                    coverage.outside_matching += 1;
                }
            }
        }
    }
    coverage
}

fn rounded_rect_signed_distance(x: f64, y: f64, rect: Rect, radius: f64) -> f64 {
    let radius = radius.max(0.0).min(rect.width / 2.0).min(rect.height / 2.0);
    let half_width = rect.width / 2.0;
    let half_height = rect.height / 2.0;
    let local_x = (x - rect.center_x()).abs() - (half_width - radius);
    let local_y = (y - rect.center_y()).abs() - (half_height - radius);
    let outside_x = local_x.max(0.0);
    let outside_y = local_y.max(0.0);
    outside_x.hypot(outside_y) + local_x.max(local_y).min(0.0) - radius
}

fn contiguous_filtered_color_run(
    raster: &RasterImage,
    center: (i32, i32),
    radius: usize,
    expected: Rgb,
    tolerance: u8,
    stage: &'static str,
    label: &str,
) -> C6ProofResult<usize> {
    let expected = filtered_color_candidates(expected);
    let radius = i32::try_from(radius).map_err(|error| {
        C6ProofError::new(
            stage,
            format!("{label} stroke scan radius exceeds i32: {error}"),
        )
    })?;
    let color_at = |offset: i32| {
        let point = (center.0, center.1 + offset);
        matches_any_color(raster.rgb(point.0, point.1), &expected, tolerance)
    };
    c6_ensure!(
        stage,
        color_at(0),
        "{label} stroke scan center must match the expected border color"
    );
    let mut first = 0;
    while first > -radius && color_at(first - 1) {
        first -= 1;
    }
    let mut last = 0;
    while last < radius && color_at(last + 1) {
        last += 1;
    }
    usize::try_from(last - first + 1).map_err(|error| {
        C6ProofError::new(
            stage,
            format!("{label} stroke color run is invalid: {error}"),
        )
    })
}

fn matches_any_color(actual: Option<Rgb>, expected: &[Rgb], tolerance: u8) -> bool {
    actual.is_some_and(|actual| {
        expected
            .iter()
            .any(|expected| actual.max_channel_delta(*expected) <= tolerance)
    })
}

fn filtered_color_candidates(color: Rgb) -> [Rgb; 2] {
    [color, color.linear_rgb_roundtrip()]
}

fn srgb_u8_linear_roundtrip(channel: u8) -> u8 {
    let srgb = f64::from(channel) / 255.0;
    let linear = if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    };
    let quantized_linear = (linear * 255.0).round().clamp(0.0, 255.0) / 255.0;
    let roundtrip = if quantized_linear <= 0.0031308 {
        quantized_linear * 12.92
    } else {
        1.055 * quantized_linear.powf(1.0 / 2.4) - 0.055
    };
    (roundtrip * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_rect_color_count_is_local_to_the_projected_region() {
        let image = RasterImage {
            width: 4,
            height: 2,
            pixels: vec![
                0x33, 0x33, 0x33, 0xff, 0x33, 0x33, 0x33, 0xff, 0x33, 0x33, 0x33, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                0xff, 0xff, 0xff, 0xff,
            ],
        };

        assert_eq!(
            image.count_opaque_pixels_near_in_svg_rect(
                [0.0, 0.0, 40.0, 20.0],
                [0.0, 0.0, 20.0, 10.0],
                [0x33, 0x33, 0x33],
                0,
            ),
            Some(2)
        );
        assert_eq!(
            image.count_opaque_pixels_near_in_svg_rect(
                [0.0, 0.0, 40.0, 20.0],
                [20.0, 0.0, 20.0, 10.0],
                [0x33, 0x33, 0x33],
                0,
            ),
            Some(1)
        );
        assert_eq!(
            image.count_opaque_pixels_near_in_svg_rect(
                [0.0, 0.0, 0.0, 20.0],
                [0.0, 0.0, 20.0, 10.0],
                [0x33, 0x33, 0x33],
                0,
            ),
            None
        );
    }
}
