use std::io::Cursor;

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use image::{ColorType, ImageDecoder, ImageFormat, ImageReader, Limits};
use merman_export::{DEFAULT_MAX_RASTER_PIXELS, DEFAULT_MAX_RASTER_SIDE_LENGTH, RasterPlan};
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
use std::cmp::Ordering;

use super::{BrutalistStateFixtureContract, C6ProofError, C6ProofResult};

const STATE_IDS: [&str; 4] = ["Ready", "Review", "Done", "Archive"];
const MIN_PNG_CANVAS_COVERAGE: f64 = 0.78;
const MIN_PNG_FILL_COVERAGE: f64 = 0.86;
const MIN_PNG_STROKE_COVERAGE: f64 = 0.78;
const MIN_PNG_CORNER_COVERAGE: f64 = 0.82;
const MIN_PNG_SHADOW_COVERAGE: f64 = 0.82;
const MIN_PNG_SHADOW_CONTROL_COVERAGE: f64 = 0.72;
const MIN_LABEL_INK_PIXELS: usize = 48;
#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
const MIN_JPEG_PSNR_DB: f64 = 28.0;
const PNG_COLOR_TOLERANCE: u8 = 0;
const PNG_LABEL_TOLERANCE: u8 = 8;
const PNG_STROKE_WIDTH_ERROR_PX: usize = 2;
const MAX_RASTER_ARTIFACT_OVERHEAD_BYTES: usize = 1024 * 1024;

pub(crate) struct PngArtifactProof {
    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    raster: RasterImage,
}

pub(crate) fn prove_brutalist_state_png(
    fixture: &BrutalistStateFixtureContract<'_>,
    sealed_svg: &str,
    bytes: &[u8],
    raster_plan: RasterPlan,
) -> C6ProofResult<PngArtifactProof> {
    let contract = BrutalistStateVisualContract::from_fixture(fixture)?;
    let geometry = StateRasterGeometry::from_sealed_svg(sealed_svg)?;
    let raster = RasterImage::decode_png(bytes, raster_plan)?;
    c6_ensure!(
        "raster-geometry",
        geometry.nodes.len() == contract.nodes.len(),
        "State geometry node count must match the visual contract; geometry={}, contract={}",
        geometry.nodes.len(),
        contract.nodes.len()
    );
    prove_brutalist_state_raster(&contract, &geometry, &raster)?;

    Ok(PngArtifactProof {
        #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
        raster,
    })
}

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

pub(crate) fn parse_c6_svg_view_box(value: &str) -> C6ProofResult<[f64; 4]> {
    let rect = parse_view_box(value)?;
    Ok([rect.left, rect.top, rect.width, rect.height])
}

pub(crate) fn transformed_c6_svg_rect(node: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
    let rect = transformed_rect(node)?;
    Ok([rect.left, rect.top, rect.width, rect.height])
}

pub(crate) fn parse_c6_hex_rgb(value: &str) -> C6ProofResult<[u8; 3]> {
    let color = Rgb::parse_hex(value)?;
    Ok([color.red, color.green, color.blue])
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) fn prove_brutalist_state_jpeg(
    bytes: &[u8],
    raster_plan: RasterPlan,
    png: &PngArtifactProof,
) -> C6ProofResult<()> {
    let raster = RasterImage::decode_jpeg(bytes, raster_plan)?;
    c6_ensure!(
        "raster-geometry",
        raster.width == png.raster.width && raster.height == png.raster.height,
        "JPEG and PNG dimensions must match; jpeg={}x{}, png={}x{}",
        raster.width,
        raster.height,
        png.raster.width,
        png.raster.height
    );
    prove_jpeg_tracks_png(&raster, &png.raster)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    #[cfg(any(test, all(feature = "png", feature = "jpeg", feature = "pdf")))]
    const WHITE: Self = Self {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    };

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

#[derive(Debug)]
struct BrutalistStateVisualContract {
    canvas: Rgb,
    text: Rgb,
    border: Rgb,
    border_width: f64,
    radius: f64,
    shadow: Rgb,
    quantized_shadow: Rgb,
    shadow_offset_x: f64,
    shadow_offset_y: f64,
    nodes: Vec<NodeVisualContract>,
}

impl BrutalistStateVisualContract {
    fn from_fixture(fixture: &BrutalistStateFixtureContract<'_>) -> C6ProofResult<Self> {
        let tokens = fixture.tokens;
        let border = fixture.border;
        let shadow = fixture.shadow;
        c6_ensure!(
            "raster-contract",
            shadow.offset_x_px() > 0 && shadow.offset_y_px() > 0,
            "Brutalist raster proof requires positive hard-shadow offsets; x={}, y={}",
            shadow.offset_x_px(),
            shadow.offset_y_px()
        );
        let palette = fixture.palette_colors;
        let shadow_offset_x = f64::from(shadow.offset_x_px());
        let shadow_offset_y = f64::from(shadow.offset_y_px());
        let shadow = Rgb::parse_hex(shadow.color())?;
        let quantized_shadow = shadow.linear_rgb_roundtrip();
        c6_ensure!(
            "raster-contract",
            shadow != quantized_shadow,
            "hard-shadow color must produce a distinct filtered quantization candidate"
        );
        for other in [
            tokens.background(),
            tokens.surface(),
            tokens.primary(),
            tokens.text(),
            border.color(),
        ]
        .into_iter()
        .chain(palette.iter().map(String::as_str))
        {
            c6_ensure!(
                "raster-contract",
                Rgb::parse_hex(other)? != quantized_shadow,
                "hard-shadow quantization candidate must not collide with {other}"
            );
        }

        Ok(Self {
            canvas: Rgb::parse_hex(tokens.background())?,
            text: Rgb::parse_hex(tokens.text())?,
            border: Rgb::parse_hex(border.color())?,
            border_width: f64::from(border.width_px()),
            radius: f64::from(fixture.radius_px),
            shadow,
            quantized_shadow,
            shadow_offset_x,
            shadow_offset_y,
            nodes: vec![
                NodeVisualContract::new(STATE_IDS[0], &palette[0])?,
                NodeVisualContract::new(STATE_IDS[1], &palette[1])?,
                NodeVisualContract::new(STATE_IDS[2], &palette[2])?,
                NodeVisualContract::new(STATE_IDS[3], &palette[0])?,
            ],
        })
    }
}

#[derive(Debug)]
struct NodeVisualContract {
    id: &'static str,
    fill: Rgb,
}

impl NodeVisualContract {
    fn new(id: &'static str, fill: &str) -> C6ProofResult<Self> {
        Ok(Self {
            id,
            fill: Rgb::parse_hex(fill)?,
        })
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

    fn inset(self, amount: f64) -> C6ProofResult<Self> {
        c6_ensure!(
            "raster-geometry",
            amount.is_finite()
                && amount >= 0.0
                && self.width > amount * 2.0
                && self.height > amount * 2.0,
            "cannot inset rect {}x{} by {amount}",
            self.width,
            self.height
        );
        Ok(Self {
            left: self.left + amount,
            top: self.top + amount,
            width: self.width - amount * 2.0,
            height: self.height - amount * 2.0,
        })
    }

    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.left && x <= self.right() && y >= self.top && y <= self.bottom()
    }
}

#[derive(Clone, Debug, PartialEq)]
struct StateRasterGeometry {
    view_box: Rect,
    nodes: Vec<NodeGeometry>,
}

impl StateRasterGeometry {
    fn from_sealed_svg(svg: &str) -> C6ProofResult<Self> {
        let document = roxmltree::Document::parse(svg).map_err(|error| {
            C6ProofError::new(
                "raster-geometry",
                format!("failed to parse sealed C6 SVG geometry: {error}"),
            )
        })?;
        let root = document.root_element();
        let view_box = parse_view_box(root.attribute("viewBox").ok_or_else(|| {
            C6ProofError::new("raster-geometry", "sealed C6 SVG is missing viewBox")
        })?)?;
        c6_ensure!(
            "raster-geometry",
            root.attribute("preserveAspectRatio").is_none(),
            "sealed C6 SVG must not declare preserveAspectRatio"
        );

        let nodes = STATE_IDS
            .into_iter()
            .map(|state_id| {
                let fragment = format!("-state-{state_id}-");
                let groups = document
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("g")
                            && node
                                .attribute("id")
                                .is_some_and(|id| id.contains(&fragment))
                    })
                    .collect::<Vec<_>>();
                c6_ensure!(
                    "raster-geometry",
                    groups.len() == 1,
                    "expected one State group for {state_id}, found {}",
                    groups.len()
                );
                let group = groups.into_iter().next().ok_or_else(|| {
                    C6ProofError::new(
                        "raster-geometry",
                        format!("expected one State group for {state_id}"),
                    )
                })?;
                let rects = group
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("class").is_some_and(|classes| {
                                classes
                                    .split_ascii_whitespace()
                                    .any(|class| class == "basic")
                                    && classes
                                        .split_ascii_whitespace()
                                        .any(|class| class == "label-container")
                            })
                    })
                    .collect::<Vec<_>>();
                c6_ensure!(
                    "raster-geometry",
                    rects.len() == 1,
                    "expected one State rect for {state_id}, found {}",
                    rects.len()
                );
                let rect = rects.into_iter().next().ok_or_else(|| {
                    C6ProofError::new(
                        "raster-geometry",
                        format!("expected one State rect for {state_id}"),
                    )
                })?;
                Ok(NodeGeometry {
                    id: state_id,
                    rect: transformed_rect(rect)?,
                })
            })
            .collect::<C6ProofResult<Vec<_>>>()?;
        Ok(Self { view_box, nodes })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NodeGeometry {
    id: &'static str,
    rect: Rect,
}

#[derive(Clone, Copy)]
struct AffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl AffineTransform {
    const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    fn from_svg(transform: svgtypes::Transform) -> Self {
        Self {
            a: transform.a,
            b: transform.b,
            c: transform.c,
            d: transform.d,
            e: transform.e,
            f: transform.f,
        }
    }

    fn multiply(self, child: Self) -> Self {
        Self {
            a: self.a * child.a + self.c * child.b,
            b: self.b * child.a + self.d * child.b,
            c: self.a * child.c + self.c * child.d,
            d: self.b * child.c + self.d * child.d,
            e: self.a * child.e + self.c * child.f + self.e,
            f: self.b * child.e + self.d * child.f + self.f,
        }
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }
}

fn transformed_rect(node: roxmltree::Node<'_, '_>) -> C6ProofResult<Rect> {
    let x = number_attribute(node, "x")?;
    let y = number_attribute(node, "y")?;
    let width = number_attribute(node, "width")?;
    let height = number_attribute(node, "height")?;
    c6_ensure!(
        "raster-geometry",
        width > 0.0 && height > 0.0,
        "State geometry rect dimensions must be positive; width={width}, height={height}"
    );

    let mut transform = AffineTransform::IDENTITY;
    let ancestors = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in ancestors.into_iter().rev() {
        let parsed = raw.parse::<svgtypes::Transform>().map_err(|error| {
            C6ProofError::new(
                "raster-geometry",
                format!("invalid C6 geometry transform {raw:?}: {error}"),
            )
        })?;
        transform = transform.multiply(AffineTransform::from_svg(parsed));
    }
    c6_ensure!(
        "raster-geometry",
        transform.b.abs() <= 1e-9 && transform.c.abs() <= 1e-9,
        "State geometry transform may not skew or rotate; b={}, c={}",
        transform.b,
        transform.c
    );
    c6_ensure!(
        "raster-geometry",
        transform.a > 0.0 && transform.d > 0.0,
        "State geometry transform scale must be positive; a={}, d={}",
        transform.a,
        transform.d
    );
    let (left, top) = transform.apply(x, y);
    let (right, bottom) = transform.apply(x + width, y + height);
    Ok(Rect {
        left,
        top,
        width: right - left,
        height: bottom - top,
    })
}

fn parse_view_box(value: &str) -> C6ProofResult<Rect> {
    let values = value
        .split_ascii_whitespace()
        .map(|part| {
            part.parse::<f64>().map_err(|error| {
                C6ProofError::new(
                    "raster-geometry",
                    format!("invalid C6 viewBox number {part:?}: {error}"),
                )
            })
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let [left, top, width, height] = values.as_slice() else {
        return Err(C6ProofError::new(
            "raster-geometry",
            format!(
                "C6 viewBox must contain four numbers, found {}",
                values.len()
            ),
        ));
    };
    c6_ensure!(
        "raster-geometry",
        values.iter().all(|value| value.is_finite()),
        "C6 viewBox numbers must be finite"
    );
    c6_ensure!(
        "raster-geometry",
        *width > 0.0 && *height > 0.0,
        "C6 viewBox dimensions must be positive; width={}, height={}",
        width,
        height
    );
    Ok(Rect {
        left: *left,
        top: *top,
        width: *width,
        height: *height,
    })
}

fn number_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> C6ProofResult<f64> {
    let raw = node.attribute(name).ok_or_else(|| {
        C6ProofError::new(
            "raster-geometry",
            format!("missing {name} on C6 geometry node"),
        )
    })?;
    let value = raw.parse::<f64>().map_err(|error| {
        C6ProofError::new(
            "raster-geometry",
            format!("invalid {name} on C6 geometry node: {error}"),
        )
    })?;
    c6_ensure!(
        "raster-geometry",
        value.is_finite(),
        "{name} on C6 geometry node must be finite"
    );
    Ok(value)
}

#[derive(Clone, Debug)]
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

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    pub(super) fn decode_jpeg(bytes: &[u8], plan: RasterPlan) -> C6ProofResult<Self> {
        let planned_rgb = PlannedRaster::new(plan, 3, "jpeg-decode")?;
        let planned_rgba = PlannedRaster::new(plan, 4, "jpeg-decode")?;
        planned_rgba.require_bounded_artifact(bytes, "jpeg-decode")?;
        c6_ensure!(
            "jpeg-decode",
            bytes.starts_with(&[0xff, 0xd8, 0xff]) && bytes.ends_with(&[0xff, 0xd9]),
            "C6 JPEG must retain its SOI and EOI envelope"
        );

        let mut limits = Limits::default();
        limits.max_image_width = Some(DEFAULT_MAX_RASTER_SIDE_LENGTH);
        limits.max_image_height = Some(DEFAULT_MAX_RASTER_SIDE_LENGTH);
        limits.max_alloc = Some(
            u64::try_from(planned_rgba.decoded_bytes)
                .map_err(|error| C6ProofError::new("jpeg-decode", error.to_string()))?,
        );
        let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg);
        reader.limits(limits);
        let decoder = reader.into_decoder().map_err(|error| {
            C6ProofError::new(
                "jpeg-decode",
                format!("failed to decode C6 JPEG header: {error}"),
            )
        })?;
        let (width, height) = decoder.dimensions();
        planned_rgb.require_dimensions(width, height, "jpeg-decode")?;
        c6_ensure!(
            "jpeg-decode",
            decoder.color_type() == ColorType::Rgb8,
            "C6 JPEG must decode as 8-bit RGB, got {:?}",
            decoder.color_type()
        );
        c6_ensure!(
            "jpeg-decode",
            decoder.total_bytes()
                == u64::try_from(planned_rgb.decoded_bytes)
                    .map_err(|error| C6ProofError::new("jpeg-decode", error.to_string()))?,
            "C6 JPEG decoded byte count does not match its frozen raster plan"
        );

        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(planned_rgba.decoded_bytes)
            .map_err(|error| {
                C6ProofError::new(
                    "jpeg-decode",
                    format!("reserve the bounded C6 JPEG output buffer: {error}"),
                )
            })?;
        pixels.resize(planned_rgb.decoded_bytes, 0);
        decoder.read_image(&mut pixels).map_err(|error| {
            C6ProofError::new(
                "jpeg-decode",
                format!("failed to decode final C6 JPEG: {error}"),
            )
        })?;
        pixels.resize(planned_rgba.decoded_bytes, 0);
        for pixel_index in (0..planned_rgb.pixel_count).rev() {
            let source = pixel_index * 3;
            let target = pixel_index * 4;
            let red = pixels[source];
            let green = pixels[source + 1];
            let blue = pixels[source + 2];
            pixels[target] = red;
            pixels[target + 1] = green;
            pixels[target + 2] = blue;
            pixels[target + 3] = u8::MAX;
        }
        Ok(Self {
            width,
            height,
            pixels,
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

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn pixel_count(&self) -> usize {
        self.pixels.len() / 4
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
            ScanAxis::Vertical,
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

    #[cfg(test)]
    pub(crate) fn control_mask_background_counts(
        &self,
        control: &Self,
        expected: [u8; 3],
        tolerance: u8,
        background: [u8; 4],
        background_tolerance: u8,
    ) -> Option<(usize, usize)> {
        if self.dimensions() != control.dimensions() {
            return None;
        }

        let mut control_pixels = 0usize;
        let mut non_background_pixels = 0usize;
        for (actual, control) in self
            .pixels
            .chunks_exact(4)
            .zip(control.pixels.chunks_exact(4))
        {
            let matches_control = control[3] != 0
                && control[0].abs_diff(expected[0]) <= tolerance
                && control[1].abs_diff(expected[1]) <= tolerance
                && control[2].abs_diff(expected[2]) <= tolerance;
            if matches_control {
                control_pixels += 1;
                if actual[0].abs_diff(background[0]) > background_tolerance
                    || actual[1].abs_diff(background[1]) > background_tolerance
                    || actual[2].abs_diff(background[2]) > background_tolerance
                    || actual[3].abs_diff(background[3]) > background_tolerance
                {
                    non_background_pixels += 1;
                }
            }
        }
        Some((control_pixels, non_background_pixels))
    }

    pub(crate) fn control_mask_alpha_counts_in_svg_rect(
        &self,
        control: &Self,
        view_box: [f64; 4],
        rect: [f64; 4],
        expected: [u8; 3],
        color_tolerance: u8,
        transparent_alpha_tolerance: u8,
        excluded_rects: &[[f64; 4]],
    ) -> Option<(usize, usize)> {
        if self.dimensions() != control.dimensions() {
            return None;
        }
        let [view_left, view_top, view_width, view_height] = view_box;
        let [rect_left, rect_top, rect_width, rect_height] = rect;
        if !view_box.into_iter().chain(rect).all(f64::is_finite)
            || view_width <= 0.0
            || view_height <= 0.0
            || rect_width <= 0.0
            || rect_height <= 0.0
            || self.width == 0
            || self.height == 0
            || excluded_rects.iter().any(|rect| {
                !rect.iter().all(|value| value.is_finite()) || rect[2] < 0.0 || rect[3] < 0.0
            })
        {
            return None;
        }

        let rect_right = rect_left + rect_width;
        let rect_bottom = rect_top + rect_height;
        let width = f64::from(self.width);
        let height = f64::from(self.height);
        let mut control_pixels = 0usize;
        let mut non_transparent_pixels = 0usize;
        for (index, (actual, control)) in self
            .pixels
            .chunks_exact(4)
            .zip(control.pixels.chunks_exact(4))
            .enumerate()
        {
            let matches_control = control[3] != 0
                && control[0].abs_diff(expected[0]) <= color_tolerance
                && control[1].abs_diff(expected[1]) <= color_tolerance
                && control[2].abs_diff(expected[2]) <= color_tolerance;
            if !matches_control {
                continue;
            }
            let index = u64::try_from(index).ok()?;
            let x = u32::try_from(index % u64::from(self.width)).ok()?;
            let y = u32::try_from(index / u64::from(self.width)).ok()?;
            let svg_x = view_left + (f64::from(x) + 0.5) * view_width / width;
            let svg_y = view_top + (f64::from(y) + 0.5) * view_height / height;
            if excluded_rects.iter().any(|excluded| {
                svg_x >= excluded[0]
                    && svg_x <= excluded[0] + excluded[2]
                    && svg_y >= excluded[1]
                    && svg_y <= excluded[1] + excluded[3]
            }) {
                continue;
            }
            if svg_x >= rect_left
                && svg_x <= rect_right
                && svg_y >= rect_top
                && svg_y <= rect_bottom
            {
                control_pixels += 1;
                if actual[3] > transparent_alpha_tolerance {
                    non_transparent_pixels += 1;
                }
            }
        }
        Some((control_pixels, non_transparent_pixels))
    }

    pub(crate) fn control_mask_colors_counts_in_svg_rect(
        &self,
        control: &Self,
        view_box: [f64; 4],
        rect: [f64; 4],
        control_match: ([u8; 3], u8),
        expected_matches: &[([u8; 3], u8)],
        excluded_rects: &[[f64; 4]],
    ) -> Option<(usize, usize)> {
        if self.dimensions() != control.dimensions() {
            return None;
        }
        let [view_left, view_top, view_width, view_height] = view_box;
        let [rect_left, rect_top, rect_width, rect_height] = rect;
        if !view_box.into_iter().chain(rect).all(f64::is_finite)
            || view_width <= 0.0
            || view_height <= 0.0
            || rect_width <= 0.0
            || rect_height <= 0.0
            || self.width == 0
            || self.height == 0
            || excluded_rects.iter().any(|rect| {
                !rect.iter().all(|value| value.is_finite()) || rect[2] < 0.0 || rect[3] < 0.0
            })
        {
            return None;
        }

        let rect_right = rect_left + rect_width;
        let rect_bottom = rect_top + rect_height;
        let width = f64::from(self.width);
        let height = f64::from(self.height);
        let (control_color, control_tolerance) = control_match;
        let mut control_pixels = 0usize;
        let mut expected_pixels = 0usize;
        for (index, (actual, control)) in self
            .pixels
            .chunks_exact(4)
            .zip(control.pixels.chunks_exact(4))
            .enumerate()
        {
            let matches_control = control[3] != 0
                && control[0].abs_diff(control_color[0]) <= control_tolerance
                && control[1].abs_diff(control_color[1]) <= control_tolerance
                && control[2].abs_diff(control_color[2]) <= control_tolerance;
            if !matches_control {
                continue;
            }
            let index = u64::try_from(index).ok()?;
            let x = u32::try_from(index % u64::from(self.width)).ok()?;
            let y = u32::try_from(index / u64::from(self.width)).ok()?;
            let svg_x = view_left + (f64::from(x) + 0.5) * view_width / width;
            let svg_y = view_top + (f64::from(y) + 0.5) * view_height / height;
            if excluded_rects.iter().any(|excluded| {
                svg_x >= excluded[0]
                    && svg_x <= excluded[0] + excluded[2]
                    && svg_y >= excluded[1]
                    && svg_y <= excluded[1] + excluded[3]
            }) {
                continue;
            }
            if svg_x < rect_left || svg_x > rect_right || svg_y < rect_top || svg_y > rect_bottom {
                continue;
            }
            control_pixels += 1;
            if actual[3] > 2 && rgb_matches_palette_or_blend(actual, expected_matches) {
                expected_pixels += 1;
            }
        }
        Some((control_pixels, expected_pixels))
    }
}

fn rgb_matches_palette_or_blend(actual: &[u8], expected: &[([u8; 3], u8)]) -> bool {
    if expected.iter().any(|(color, tolerance)| {
        actual[0].abs_diff(color[0]) <= *tolerance
            && actual[1].abs_diff(color[1]) <= *tolerance
            && actual[2].abs_diff(color[2]) <= *tolerance
    }) {
        return true;
    }
    expected.iter().enumerate().any(|(left_index, left)| {
        expected
            .iter()
            .skip(left_index + 1)
            .any(|right| rgb_near_segment(actual, *left, *right))
    })
}

fn rgb_near_segment(actual: &[u8], left: ([u8; 3], u8), right: ([u8; 3], u8)) -> bool {
    let start = left.0.map(f64::from);
    let end = right.0.map(f64::from);
    let pixel = [
        f64::from(actual[0]),
        f64::from(actual[1]),
        f64::from(actual[2]),
    ];
    let direction = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let length_squared = direction.iter().map(|value| value * value).sum::<f64>();
    if length_squared == 0.0 {
        return false;
    }
    let offset = [
        pixel[0] - start[0],
        pixel[1] - start[1],
        pixel[2] - start[2],
    ];
    let interpolation = offset
        .iter()
        .zip(direction)
        .map(|(value, direction)| value * direction)
        .sum::<f64>()
        / length_squared;
    if !(0.0..=1.0).contains(&interpolation) {
        return false;
    }
    let distance_squared = pixel
        .iter()
        .zip(start)
        .zip(direction)
        .map(|((value, start), direction)| {
            let distance = value - (start + interpolation * direction);
            distance * distance
        })
        .sum::<f64>();
    let tolerance = f64::from(left.1.max(right.1));
    distance_squared <= 3.0 * tolerance * tolerance
}

#[derive(Clone, Copy)]
struct PlannedRaster {
    width: u32,
    height: u32,
    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    pixel_count: usize,
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
            #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
            pixel_count,
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

fn prove_brutalist_state_raster(
    contract: &BrutalistStateVisualContract,
    geometry: &StateRasterGeometry,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let transform = raster.transform(geometry.view_box);
    c6_ensure!(
        "raster-geometry",
        transform.scale_x() >= 1.9 && transform.scale_y() >= 1.9,
        "C6 raster scale must be at least 1.9x; x={:.3}, y={:.3}",
        transform.scale_x(),
        transform.scale_y()
    );
    c6_ensure!(
        "raster-geometry",
        (transform.scale_x() - transform.scale_y()).abs() <= 0.02,
        "C6 raster scale must remain uniform; x={:.3}, y={:.3}",
        transform.scale_x(),
        transform.scale_y()
    );
    prove_canvas(contract, geometry, transform, raster)?;

    for (expected, actual) in contract.nodes.iter().zip(&geometry.nodes) {
        c6_ensure!(
            "raster-geometry",
            expected.id == actual.id,
            "State node order must match the visual contract; expected={}, actual={}",
            expected.id,
            actual.id
        );
        prove_node_fill(contract, expected, actual.rect, transform, raster)?;
        prove_node_stroke(contract, actual.rect, transform, raster)?;
        prove_node_rounding(contract, actual.rect, transform, raster)?;
        prove_node_shadow(contract, actual.rect, transform, raster)?;
        prove_node_label_ink(contract, actual.rect, transform, raster)?;
    }
    Ok(())
}

fn prove_canvas(
    contract: &BrutalistStateVisualContract,
    geometry: &StateRasterGeometry,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let exclusion =
        contract.border_width / 2.0 + contract.shadow_offset_x.max(contract.shadow_offset_y) + 1.0;
    let expanded_nodes = geometry
        .nodes
        .iter()
        .map(|node| node.rect.expanded(exclusion))
        .collect::<Vec<_>>();
    prove_color_region_coverage(
        "raster-canvas",
        raster,
        transform,
        geometry.view_box,
        |x, y| !expanded_nodes.iter().any(|rect| rect.contains(x, y)),
        &[contract.canvas],
        PNG_COLOR_TOLERANCE,
        MIN_PNG_CANVAS_COVERAGE,
        "canvas outside diagram nodes",
    )
}

fn prove_node_fill(
    contract: &BrutalistStateVisualContract,
    expected: &NodeVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let inset = contract.border_width / 2.0 + 0.75 / transform.scale_x();
    let interior = rect.inset(inset)?;
    let fill_candidates = filtered_color_candidates(expected.fill);
    prove_color_region_coverage(
        "raster-node-fill",
        raster,
        transform,
        interior,
        |_, y| {
            let relative_y = (y - rect.top) / rect.height;
            relative_y <= 0.34 || relative_y >= 0.68
        },
        &fill_candidates,
        PNG_COLOR_TOLERANCE,
        MIN_PNG_FILL_COVERAGE,
        &format!("{} ordinal fill interior", expected.id),
    )
}

fn prove_node_stroke(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let clearance = contract.radius + contract.border_width + 1.0;
    let band_half_width = contract.border_width * 0.25;
    let horizontal_width = rect.width - clearance * 2.0;
    let vertical_height = rect.height - clearance * 2.0;
    c6_ensure!(
        "raster-node-stroke",
        horizontal_width > 0.0 && vertical_height > 0.0,
        "State stroke regions must have positive spans; horizontal={horizontal_width}, vertical={vertical_height}"
    );
    let border_candidates = filtered_color_candidates(contract.border);
    for (label, region) in stroke_regions(rect, clearance, band_half_width) {
        prove_color_region_coverage(
            "raster-node-stroke",
            raster,
            transform,
            region,
            |_, _| true,
            &border_candidates,
            PNG_COLOR_TOLERANCE,
            MIN_PNG_STROKE_COVERAGE,
            label,
        )?;
    }

    let expected_vertical_width = (contract.border_width * transform.scale_y()).round() as usize;
    let expected_horizontal_width = (contract.border_width * transform.scale_x()).round() as usize;
    let mut runs = Vec::new();
    for position in [0.25, 0.5, 0.75] {
        let top = transform.point(rect.left + rect.width * position, rect.top);
        runs.push((
            contiguous_filtered_color_run(
                raster,
                top,
                ScanAxis::Vertical,
                expected_vertical_width + 4,
                contract.border,
                PNG_COLOR_TOLERANCE,
                "raster-node-stroke",
                "State",
            )?,
            expected_vertical_width,
        ));
        let left = transform.point(rect.left, rect.top + rect.height * position);
        runs.push((
            contiguous_filtered_color_run(
                raster,
                left,
                ScanAxis::Horizontal,
                expected_horizontal_width + 4,
                contract.border,
                PNG_COLOR_TOLERANCE,
                "raster-node-stroke",
                "State",
            )?,
            expected_horizontal_width,
        ));
    }
    for (actual, expected) in runs {
        c6_ensure!(
            "raster-node-stroke",
            actual.abs_diff(expected) <= PNG_STROKE_WIDTH_ERROR_PX,
            "State stroke width must remain within {PNG_STROKE_WIDTH_ERROR_PX}px of {expected}px, found {actual}px"
        );
    }
    Ok(())
}

fn prove_node_rounding(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let outer = rect.expanded(contract.border_width / 2.0);
    let outer_radius = contract.radius + contract.border_width / 2.0;
    let patch_size = outer_radius + 2.0;
    let corners = [
        (
            "top-left rounded corner",
            Rect {
                left: outer.left,
                top: outer.top,
                width: patch_size,
                height: patch_size,
            },
        ),
        (
            "top-right rounded corner",
            Rect {
                left: outer.right() - patch_size,
                top: outer.top,
                width: patch_size,
                height: patch_size,
            },
        ),
    ];
    let threshold = MIN_PNG_CORNER_COVERAGE;
    for (label, patch) in corners {
        let coverage = rounded_corner_coverage(
            raster,
            transform,
            patch,
            outer,
            outer_radius,
            contract.canvas,
            PNG_COLOR_TOLERANCE,
        );
        c6_ensure!(
            "raster-node-rounding",
            coverage.inside_total >= 3 && coverage.outside_total >= 3,
            "{label} must retain enough classified pixels; inside={}, outside={}",
            coverage.inside_total,
            coverage.outside_total
        );
        c6_ensure!(
            "raster-node-rounding",
            coverage.inside_ratio() >= threshold,
            "{label} must retain the expected occupied arc; coverage={:.3} ({}/{})",
            coverage.inside_ratio(),
            coverage.inside_matching,
            coverage.inside_total
        );
        c6_ensure!(
            "raster-node-rounding",
            coverage.outside_ratio() >= threshold,
            "{label} must retain the expected canvas cutout; coverage={:.3} ({}/{})",
            coverage.outside_ratio(),
            coverage.outside_matching,
            coverage.outside_total
        );
    }
    Ok(())
}

fn prove_node_shadow(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let accepted_shadow = [contract.shadow, contract.quantized_shadow];
    let guard = (1.25 / transform.scale_x()).max(0.6);
    c6_ensure!(
        "raster-node-shadow",
        contract.shadow_offset_x > guard * 2.0 && contract.shadow_offset_y > guard * 2.0,
        "hard-shadow offsets must exceed the sampling guard; x={}, y={}, guard={guard}",
        contract.shadow_offset_x,
        contract.shadow_offset_y
    );
    let source = rect.expanded(contract.border_width / 2.0);
    let clearance = contract.radius + contract.border_width / 2.0 + guard;
    let vertical_span = source.height - clearance * 2.0;
    let horizontal_span = source.width - clearance * 2.0;
    c6_ensure!(
        "raster-node-shadow",
        vertical_span > 0.0 && horizontal_span > 0.0,
        "hard-shadow bands must have positive spans; vertical={vertical_span}, horizontal={horizontal_span}"
    );
    let [(right_label, right_band), (bottom_label, bottom_band)] =
        shadow_bands(contract, rect, transform);
    for (label, region) in [(right_label, right_band), (bottom_label, bottom_band)] {
        prove_color_region_coverage(
            "raster-node-shadow",
            raster,
            transform,
            region,
            |_, _| true,
            &accepted_shadow,
            PNG_COLOR_TOLERANCE,
            MIN_PNG_SHADOW_COVERAGE,
            label,
        )?;
    }

    let controls = [
        (
            "right shadow extent control",
            Rect {
                left: source.right() + contract.shadow_offset_x + guard,
                top: right_band.top,
                width: guard * 2.0,
                height: right_band.height,
            },
        ),
        (
            "bottom shadow extent control",
            Rect {
                left: bottom_band.left,
                top: source.bottom() + contract.shadow_offset_y + guard,
                width: bottom_band.width,
                height: guard * 2.0,
            },
        ),
        (
            "left shadow direction control",
            Rect {
                left: source.left - contract.shadow_offset_x + guard,
                top: source.top + clearance,
                width: contract.shadow_offset_x - guard * 2.0,
                height: vertical_span,
            },
        ),
        (
            "top shadow direction control",
            Rect {
                left: source.left + clearance,
                top: source.top - contract.shadow_offset_y + guard,
                width: horizontal_span,
                height: contract.shadow_offset_y - guard * 2.0,
            },
        ),
    ];
    for (label, region) in controls {
        prove_color_region_coverage(
            "raster-node-shadow",
            raster,
            transform,
            region,
            |_, _| true,
            &[contract.canvas],
            PNG_COLOR_TOLERANCE,
            MIN_PNG_SHADOW_CONTROL_COVERAGE,
            label,
        )?;
    }
    Ok(())
}

fn prove_node_label_ink(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<()> {
    let mask = label_ink_mask(contract, rect, transform, raster)?;
    let ink_pixels = mask.true_count();
    let minimum_ink =
        ((MIN_LABEL_INK_PIXELS as f64 / 4.0) * transform.scale_x() * transform.scale_y()).round()
            as usize;
    c6_ensure!(
        "raster-node-label",
        ink_pixels >= minimum_ink,
        "each final native artifact must retain structured embedded-font label ink; found {ink_pixels} pixels, expected at least {minimum_ink}"
    );
    let bounds = mask
        .tight_bounds()
        .ok_or_else(|| C6ProofError::new("raster-node-label", "label ink mask is empty"))?;
    let minimum_width = (8.0 * transform.scale_x()).round() as usize;
    let minimum_height = (4.0 * transform.scale_y()).round() as usize;
    c6_ensure!(
        "raster-node-label",
        bounds.width >= minimum_width && bounds.height >= minimum_height,
        "label ink must span at least {minimum_width}x{minimum_height}px; found {}x{}px",
        bounds.width,
        bounds.height,
    );
    let density = ink_pixels as f64 / (bounds.width * bounds.height) as f64;
    c6_ensure!(
        "raster-node-label",
        (0.06..=0.78).contains(&density),
        "label ink density must remain glyph-like; density={density:.3}"
    );
    c6_ensure!(
        "raster-node-label",
        mask.occupied_column_count(&bounds) >= (6.0 * transform.scale_x()).round() as usize,
        "label ink must span enough occupied columns"
    );
    c6_ensure!(
        "raster-node-label",
        mask.occupied_row_count(&bounds) >= (3.0 * transform.scale_y()).round() as usize,
        "label ink must span enough occupied rows"
    );
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn prove_jpeg_tracks_png(jpeg: &RasterImage, png: &RasterImage) -> C6ProofResult<()> {
    c6_ensure!(
        "jpeg-control",
        jpeg.pixel_count() == png.pixel_count(),
        "JPEG and PNG pixel counts must match; jpeg={}, png={}",
        jpeg.pixel_count(),
        png.pixel_count()
    );
    c6_ensure!(
        "jpeg-control",
        jpeg.pixel_count() > 0,
        "JPEG and PNG control images must contain pixels"
    );
    let mut squared_error = 0u128;
    for pixel_index in 0..jpeg.pixel_count() {
        let jpeg_pixel = jpeg
            .rgba_pixel(pixel_index)
            .ok_or_else(|| C6ProofError::new("jpeg-control", "JPEG pixel buffer is incomplete"))?;
        let png_pixel = png
            .rgba_pixel(pixel_index)
            .ok_or_else(|| C6ProofError::new("jpeg-control", "PNG pixel buffer is incomplete"))?;
        let png_composited = composite_rgba_over(png_pixel, Rgb::WHITE);
        for (&jpeg_channel, &png_channel) in jpeg_pixel[..3].iter().zip(&png_composited) {
            let delta = u128::from(jpeg_channel.abs_diff(png_channel));
            squared_error += delta * delta;
        }
    }
    let channel_count = jpeg
        .pixel_count()
        .checked_mul(3)
        .ok_or_else(|| C6ProofError::new("jpeg-control", "JPEG channel count exceeds usize"))?;
    let mean_squared_error = squared_error as f64 / channel_count as f64;
    let psnr = if mean_squared_error == 0.0 {
        f64::INFINITY
    } else {
        10.0 * ((255.0 * 255.0) / mean_squared_error).log10()
    };
    c6_ensure!(
        "jpeg-control",
        matches!(
            psnr.partial_cmp(&MIN_JPEG_PSNR_DB),
            Some(Ordering::Equal | Ordering::Greater)
        ),
        "C6 JPEG must remain globally similar to the same document's proved PNG; MSE={mean_squared_error:.3}, PSNR={psnr:.3}dB"
    );
    Ok(())
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
fn composite_rgba_over(pixel: [u8; 4], matte: Rgb) -> [u8; 3] {
    let alpha = u32::from(pixel[3]);
    let inverse_alpha = u32::from(u8::MAX) - alpha;
    let composite = |source: u8, background: u8| {
        ((u32::from(source) * alpha + u32::from(background) * inverse_alpha + 127) / 255) as u8
    };
    [
        composite(pixel[0], matte.red),
        composite(pixel[1], matte.green),
        composite(pixel[2], matte.blue),
    ]
}

fn stroke_regions(rect: Rect, clearance: f64, band_half_width: f64) -> [(&'static str, Rect); 4] {
    let horizontal_width = rect.width - clearance * 2.0;
    let vertical_height = rect.height - clearance * 2.0;
    [
        (
            "top border",
            Rect {
                left: rect.left + clearance,
                top: rect.top - band_half_width,
                width: horizontal_width,
                height: band_half_width * 2.0,
            },
        ),
        (
            "bottom border",
            Rect {
                left: rect.left + clearance,
                top: rect.bottom() - band_half_width,
                width: horizontal_width,
                height: band_half_width * 2.0,
            },
        ),
        (
            "left border",
            Rect {
                left: rect.left - band_half_width,
                top: rect.top + clearance,
                width: band_half_width * 2.0,
                height: vertical_height,
            },
        ),
        (
            "right border",
            Rect {
                left: rect.right() - band_half_width,
                top: rect.top + clearance,
                width: band_half_width * 2.0,
                height: vertical_height,
            },
        ),
    ]
}

fn shadow_bands(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
) -> [(&'static str, Rect); 2] {
    let guard = (1.25 / transform.scale_x()).max(0.6);
    let horizontal_guard = contract.shadow_offset_x * 0.25;
    let vertical_guard = contract.shadow_offset_y * 0.25;
    let source = rect.expanded(contract.border_width / 2.0);
    let clearance = contract.radius + contract.border_width / 2.0 + guard;
    [
        (
            "right hard-shadow band",
            Rect {
                left: source.right() + horizontal_guard,
                top: source.top + contract.shadow_offset_y + clearance,
                width: contract.shadow_offset_x - horizontal_guard * 2.0,
                height: source.height - clearance * 2.0,
            },
        ),
        (
            "bottom hard-shadow band",
            Rect {
                left: source.left + contract.shadow_offset_x + clearance,
                top: source.bottom() + vertical_guard,
                width: source.width - clearance * 2.0,
                height: contract.shadow_offset_y - vertical_guard * 2.0,
            },
        ),
    ]
}

fn label_region(contract: &BrutalistStateVisualContract, rect: Rect) -> Rect {
    Rect {
        left: rect.left + contract.border_width * 2.0,
        top: rect.top + rect.height * 0.22,
        width: rect.width - contract.border_width * 4.0,
        height: rect.height * 0.60,
    }
}

fn label_ink_mask(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
) -> C6ProofResult<BinaryMask> {
    color_mask(
        raster,
        transform,
        label_region(contract, rect),
        &filtered_color_candidates(contract.text),
        PNG_LABEL_TOLERANCE,
    )
}

#[allow(clippy::too_many_arguments)]
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

fn color_mask(
    raster: &RasterImage,
    transform: RasterTransform,
    bounds: Rect,
    expected: &[Rgb],
    tolerance: u8,
) -> C6ProofResult<BinaryMask> {
    let pixel_bounds = transform.pixel_bounds(bounds);
    let width = usize::try_from(pixel_bounds.right - pixel_bounds.left).map_err(|error| {
        C6ProofError::new(
            "raster-node-label",
            format!("label mask width is invalid: {error}"),
        )
    })?;
    let height = usize::try_from(pixel_bounds.bottom - pixel_bounds.top).map_err(|error| {
        C6ProofError::new(
            "raster-node-label",
            format!("label mask height is invalid: {error}"),
        )
    })?;
    c6_ensure!(
        "raster-node-label",
        width > 0 && height > 0,
        "label mask dimensions must be positive; width={width}, height={height}"
    );
    let capacity = width.checked_mul(height).ok_or_else(|| {
        C6ProofError::new("raster-node-label", "label mask dimensions exceed usize")
    })?;
    let mut bits = Vec::with_capacity(capacity);
    for y in pixel_bounds.top..pixel_bounds.bottom {
        for x in pixel_bounds.left..pixel_bounds.right {
            bits.push(matches_any_color(raster.rgb(x, y), expected, tolerance));
        }
    }
    Ok(BinaryMask {
        width,
        height,
        bits,
    })
}

#[derive(Clone)]
struct BinaryMask {
    width: usize,
    height: usize,
    bits: Vec<bool>,
}

impl BinaryMask {
    fn true_count(&self) -> usize {
        self.bits.iter().filter(|bit| **bit).count()
    }

    fn tight_bounds(&self) -> Option<MaskBounds> {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0usize;
        let mut max_y = 0usize;
        let mut found = false;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.bits[y * self.width + x] {
                    found = true;
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
            }
        }
        found.then_some(MaskBounds {
            left: min_x,
            top: min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        })
    }

    fn occupied_column_count(&self, bounds: &MaskBounds) -> usize {
        (bounds.left..bounds.left + bounds.width)
            .filter(|&x| {
                (bounds.top..bounds.top + bounds.height).any(|y| self.bits[y * self.width + x])
            })
            .count()
    }

    fn occupied_row_count(&self, bounds: &MaskBounds) -> usize {
        (bounds.top..bounds.top + bounds.height)
            .filter(|&y| {
                (bounds.left..bounds.left + bounds.width).any(|x| self.bits[y * self.width + x])
            })
            .count()
    }
}

struct MaskBounds {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
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

#[derive(Clone, Copy)]
enum ScanAxis {
    Horizontal,
    Vertical,
}

fn contiguous_filtered_color_run(
    raster: &RasterImage,
    center: (i32, i32),
    axis: ScanAxis,
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
        let point = match axis {
            ScanAxis::Horizontal => (center.0 + offset, center.1),
            ScanAxis::Vertical => (center.0, center.1 + offset),
        };
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

    impl RasterImage {
        fn set_rgb(&mut self, x: i32, y: i32, color: Rgb) {
            assert!(x >= 0 && y >= 0);
            let x = u32::try_from(x).expect("non-negative pixel x");
            let y = u32::try_from(y).expect("non-negative pixel y");
            assert!(x < self.width && y < self.height);
            let start = usize::try_from((y * self.width + x) * 4).expect("pixel offset");
            self.pixels[start..start + 4].copy_from_slice(&[
                color.red,
                color.green,
                color.blue,
                u8::MAX,
            ]);
        }
    }

    fn solid_raster_pixels(color: Rgb, pixel_count: usize) -> Vec<u8> {
        [color.red, color.green, color.blue, u8::MAX].repeat(pixel_count)
    }

    #[test]
    fn control_mask_detects_non_background_replacements() {
        let control = RasterImage {
            width: 3,
            height: 1,
            pixels: vec![
                0xdc,
                0x26,
                0x26,
                u8::MAX,
                0xdc,
                0x26,
                0x26,
                u8::MAX,
                0,
                0,
                0,
                0,
            ],
        };
        let transparent = RasterImage {
            width: 3,
            height: 1,
            pixels: vec![
                0xff,
                0xff,
                0xff,
                u8::MAX,
                0,
                0,
                0,
                1,
                0xff,
                0xff,
                0xff,
                u8::MAX,
            ],
        };

        assert_eq!(
            transparent.control_mask_background_counts(
                &control,
                [0xdc, 0x26, 0x26],
                0,
                [0xff, 0xff, 0xff, u8::MAX],
                0,
            ),
            Some((2, 1))
        );
    }

    #[test]
    fn local_control_mask_rejects_opaque_transparency_substitutes() {
        let control = RasterImage {
            width: 3,
            height: 1,
            pixels: vec![
                0xdc,
                0x26,
                0x26,
                u8::MAX,
                0xdc,
                0x26,
                0x26,
                u8::MAX,
                0,
                0,
                0,
                0,
            ],
        };
        let transparent = RasterImage {
            width: 3,
            height: 1,
            pixels: vec![0xfb, 0xbf, 0x24, u8::MAX, 0, 0, 0, 1, 0, 0, 0, 0],
        };

        assert_eq!(
            transparent.control_mask_alpha_counts_in_svg_rect(
                &control,
                [0.0, 0.0, 3.0, 1.0],
                [0.0, 0.0, 2.0, 1.0],
                [0xdc, 0x26, 0x26],
                0,
                2,
                &[],
            ),
            Some((2, 1))
        );

        assert_eq!(
            transparent.control_mask_alpha_counts_in_svg_rect(
                &control,
                [0.0, 0.0, 3.0, 1.0],
                [0.0, 0.0, 2.0, 1.0],
                [0xdc, 0x26, 0x26],
                0,
                2,
                &[[0.0, 0.0, 1.0, 1.0]],
            ),
            Some((1, 0))
        );
    }

    #[test]
    fn underlay_palette_accepts_antialiased_blends_but_rejects_third_party_colors() {
        let underlays = [([0xec, 0xec, 0xff], 6), ([0xff, 0xff, 0xde], 6)];

        assert!(rgb_matches_palette_or_blend(
            &[0xf6, 0xf6, 0xef, u8::MAX],
            &underlays,
        ));
        assert!(!rgb_matches_palette_or_blend(
            &[0x11, 0x11, 0x11, u8::MAX],
            &underlays,
        ));
    }

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

    fn contract() -> BrutalistStateVisualContract {
        let shadow = Rgb::parse_hex("#111111").expect("test color");
        BrutalistStateVisualContract {
            canvas: Rgb::parse_hex("#f7f3e8").expect("test color"),
            text: Rgb::parse_hex("#111111").expect("test color"),
            border: Rgb::parse_hex("#111111").expect("test color"),
            border_width: 3.0,
            radius: 2.0,
            shadow,
            quantized_shadow: shadow.linear_rgb_roundtrip(),
            shadow_offset_x: 6.0,
            shadow_offset_y: 6.0,
            nodes: vec![NodeVisualContract::new("Ready", "#facc15").expect("test node")],
        }
    }

    fn geometry() -> StateRasterGeometry {
        StateRasterGeometry {
            view_box: Rect {
                left: 0.0,
                top: 0.0,
                width: 120.0,
                height: 90.0,
            },
            nodes: vec![NodeGeometry {
                id: "Ready",
                rect: Rect {
                    left: 30.0,
                    top: 25.0,
                    width: 60.0,
                    height: 40.0,
                },
            }],
        }
    }

    fn raster() -> RasterImage {
        let contract = contract();
        RasterImage {
            width: 240,
            height: 180,
            pixels: solid_raster_pixels(contract.canvas, 240 * 180),
        }
    }

    fn paint_window(raster: &mut RasterImage, center: (i32, i32), radius: i32, color: Rgb) {
        for y in center.1 - radius..=center.1 + radius {
            for x in center.0 - radius..=center.0 + radius {
                raster.set_rgb(x, y, color);
            }
        }
    }

    fn paint_user_rect(raster: &mut RasterImage, view_box: Rect, rect: Rect, color: Rgb) {
        let transform = raster.transform(view_box);
        let left = transform.point(rect.left, rect.top).0;
        let top = transform.point(rect.left, rect.top).1;
        let right = transform.point(rect.right(), rect.bottom()).0;
        let bottom = transform.point(rect.right(), rect.bottom()).1;
        for y in top..=bottom {
            for x in left..=right {
                raster.set_rgb(x, y, color);
            }
        }
    }

    fn assert_rejected(
        expected_stage: &'static str,
        expected_message: &str,
        result: C6ProofResult<()>,
    ) {
        let error = result.expect_err("the raster proof accepted an incomplete mechanism");
        assert!(
            error.to_string().starts_with(expected_stage),
            "expected rejection stage {expected_stage:?}, got {error}"
        );
        assert!(
            error.to_string().contains(expected_message),
            "expected rejection containing {expected_message:?}, got {error}"
        );
    }

    #[test]
    fn corner_only_canvas_is_rejected() {
        let contract = contract();
        let geometry = geometry();
        let mut raster = RasterImage {
            width: 240,
            height: 180,
            pixels: solid_raster_pixels(Rgb::WHITE, 240 * 180),
        };
        let margin = 4i32;
        for (x, y) in [
            (margin, margin),
            (raster.width as i32 - margin - 1, margin),
            (margin, raster.height as i32 - margin - 1),
            (
                raster.width as i32 - margin - 1,
                raster.height as i32 - margin - 1,
            ),
        ] {
            raster.set_rgb(x, y, contract.canvas);
        }

        assert_rejected(
            "raster-canvas",
            "canvas outside diagram nodes",
            prove_canvas(
                &contract,
                &geometry,
                raster.transform(geometry.view_box),
                &raster,
            ),
        );
    }

    #[test]
    fn probe_only_ordinal_fill_is_rejected() {
        let contract = contract();
        let geometry = geometry();
        let rect = geometry.nodes[0].rect;
        let mut raster = raster();
        let transform = raster.transform(geometry.view_box);
        for y in [rect.top + rect.height * 0.22, rect.top + rect.height * 0.78] {
            paint_window(
                &mut raster,
                transform.point(rect.center_x(), y),
                2,
                contract.nodes[0].fill,
            );
        }

        assert_rejected(
            "raster-node-fill",
            "ordinal fill interior",
            prove_node_fill(&contract, &contract.nodes[0], rect, transform, &raster),
        );
    }

    #[test]
    fn missing_left_and_right_strokes_are_rejected() {
        let contract = contract();
        let geometry = geometry();
        let rect = geometry.nodes[0].rect;
        let mut raster = raster();
        let transform = raster.transform(geometry.view_box);
        let half_width = contract.border_width / 2.0;
        for edge in [rect.top, rect.bottom()] {
            paint_user_rect(
                &mut raster,
                geometry.view_box,
                Rect {
                    left: rect.left,
                    top: edge - half_width,
                    width: rect.width,
                    height: contract.border_width,
                },
                contract.border,
            );
        }

        assert_rejected(
            "raster-node-stroke",
            "left border",
            prove_node_stroke(&contract, rect, transform, &raster),
        );
    }

    #[test]
    fn square_corner_with_probe_patch_is_rejected() {
        let contract = contract();
        let geometry = geometry();
        let rect = geometry.nodes[0].rect;
        let mut raster = raster();
        let transform = raster.transform(geometry.view_box);
        paint_user_rect(&mut raster, geometry.view_box, rect, contract.nodes[0].fill);
        let half_width = contract.border_width / 2.0;
        paint_user_rect(
            &mut raster,
            geometry.view_box,
            Rect {
                left: rect.left - half_width,
                top: rect.top - half_width,
                width: rect.width + contract.border_width,
                height: contract.border_width,
            },
            contract.border,
        );
        paint_user_rect(
            &mut raster,
            geometry.view_box,
            Rect {
                left: rect.left - half_width,
                top: rect.top - half_width,
                width: contract.border_width,
                height: rect.height + contract.border_width,
            },
            contract.border,
        );
        let old_probe = transform.point(
            rect.left - contract.border_width * 0.45,
            rect.top - contract.border_width * 0.45,
        );
        raster.set_rgb(old_probe.0, old_probe.1, contract.canvas);

        assert_rejected(
            "raster-node-rounding",
            "top-left rounded corner",
            prove_node_rounding(&contract, rect, transform, &raster),
        );
    }

    #[test]
    fn isolated_shadow_probe_patches_are_rejected() {
        let contract = contract();
        let geometry = geometry();
        let rect = geometry.nodes[0].rect;
        let mut raster = raster();
        let transform = raster.transform(geometry.view_box);
        let right = transform.point(
            rect.right() + contract.shadow_offset_x * 0.55,
            rect.center_y() + contract.shadow_offset_y,
        );
        let bottom = transform.point(
            rect.center_x() + contract.shadow_offset_x,
            rect.bottom() + contract.shadow_offset_y * 0.55,
        );
        paint_window(&mut raster, right, 2, contract.quantized_shadow);
        paint_window(&mut raster, bottom, 2, contract.quantized_shadow);

        assert_rejected(
            "raster-node-shadow",
            "right hard-shadow band",
            prove_node_shadow(&contract, rect, transform, &raster),
        );
    }

    #[test]
    fn twenty_dark_dots_are_not_label_evidence() {
        let contract = contract();
        let geometry = geometry();
        let rect = geometry.nodes[0].rect;
        let mut raster = raster();
        let transform = raster.transform(geometry.view_box);
        let origin = transform.point(rect.center_x() - 10.0, rect.center_y() - 2.0);
        for index in 0..20 {
            raster.set_rgb(origin.0 + index, origin.1 + index % 2, contract.text);
        }

        assert_rejected(
            "raster-node-label",
            "structured embedded-font label ink",
            prove_node_label_ink(&contract, rect, transform, &raster),
        );
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn jpeg_global_similarity_accepts_bounded_loss() {
        let png = RasterImage {
            width: 2,
            height: 1,
            pixels: vec![10, 20, 30, u8::MAX, 40, 50, 60, u8::MAX],
        };
        let jpeg = RasterImage {
            width: 2,
            height: 1,
            pixels: vec![11, 19, 31, u8::MAX, 39, 51, 59, u8::MAX],
        };

        prove_jpeg_tracks_png(&jpeg, &png)
            .expect("small whole-image loss must satisfy the JPEG smoke");
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn jpeg_global_similarity_rejects_whole_image_drift() {
        let png = RasterImage {
            width: 2,
            height: 1,
            pixels: vec![0, 0, 0, u8::MAX, 0, 0, 0, u8::MAX],
        };
        let jpeg = RasterImage {
            width: 2,
            height: 1,
            pixels: vec![u8::MAX; 8],
        };

        assert_rejected(
            "jpeg-control",
            "globally similar",
            prove_jpeg_tracks_png(&jpeg, &png),
        );
    }
}
