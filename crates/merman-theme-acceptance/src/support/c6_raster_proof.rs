use std::collections::BTreeMap;

use image::ImageFormat;
use merman_theme_fixtures::{ReferenceSemanticRule, ReferenceThemeInput, ReferenceThemeMechanism};

use super::C6ObservedMechanismDisposition;

const STATE_IDS: [&str; 4] = ["Ready", "Review", "Done", "Archive"];
const MIN_PNG_CANVAS_COVERAGE: f64 = 0.78;
const MIN_JPEG_CANVAS_COVERAGE: f64 = 0.70;
const MIN_PNG_FILL_COVERAGE: f64 = 0.86;
const MIN_JPEG_FILL_COVERAGE: f64 = 0.76;
const MIN_PNG_STROKE_COVERAGE: f64 = 0.78;
const MIN_JPEG_STROKE_COVERAGE: f64 = 0.66;
const MIN_PNG_CORNER_COVERAGE: f64 = 0.82;
const MIN_JPEG_CORNER_COVERAGE: f64 = 0.68;
const MIN_PNG_SHADOW_COVERAGE: f64 = 0.82;
const MIN_JPEG_SHADOW_COVERAGE: f64 = 0.68;
const MIN_PNG_SHADOW_CONTROL_COVERAGE: f64 = 0.72;
const MIN_JPEG_SHADOW_CONTROL_COVERAGE: f64 = 0.62;
const MIN_LABEL_INK_PIXELS: usize = 48;
const MIN_JPEG_PSNR_DB: f64 = 28.0;
const MAX_JPEG_P99_CHANNEL_DELTA: u8 = 48;
const MIN_JPEG_MASK_PRECISION: f64 = 0.72;
const MIN_JPEG_MASK_RECALL: f64 = 0.76;
const MIN_JPEG_MASK_IOU: f64 = 0.62;

pub(crate) struct PngArtifactProof {
    geometry: StateRasterGeometry,
    raster: RasterImage,
    mechanisms: BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition>,
}

impl PngArtifactProof {
    pub(crate) fn target_proof(&self, bytes: &[u8]) -> super::C6TargetProof {
        super::C6TargetProof::brutalist_state_png(bytes, self.mechanisms.clone())
    }
}

pub(crate) fn prove_brutalist_state_png(
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    bytes: &[u8],
) -> PngArtifactProof {
    let contract = BrutalistStateVisualContract::from_fixture(input);
    let geometry = StateRasterGeometry::from_sealed_svg(sealed_svg);
    let raster = RasterImage::decode_png(bytes);
    assert_eq!(geometry.nodes.len(), contract.nodes.len());
    assert_brutalist_state_raster(&contract, &geometry, &raster, 0);

    PngArtifactProof {
        geometry,
        raster,
        mechanisms: applied_mechanisms(),
    }
}

pub(crate) fn prove_brutalist_state_jpeg(
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    bytes: &[u8],
    png: &PngArtifactProof,
) -> super::C6TargetProof {
    let contract = BrutalistStateVisualContract::from_fixture(input);
    let geometry = StateRasterGeometry::from_sealed_svg(sealed_svg);
    assert_eq!(geometry, png.geometry);
    let raster = RasterImage::decode_jpeg(bytes);
    assert_eq!(raster.width, png.raster.width);
    assert_eq!(raster.height, png.raster.height);
    assert_brutalist_state_raster(&contract, &geometry, &raster, 18);
    assert_jpeg_tracks_png(&raster, &png.raster, &contract, &geometry);
    super::C6TargetProof::brutalist_state_jpeg(bytes, applied_mechanisms())
}

fn applied_mechanisms() -> BTreeMap<ReferenceThemeMechanism, C6ObservedMechanismDisposition> {
    BTreeMap::from([
        (
            ReferenceThemeMechanism::CanvasSolid,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::CssFilter,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::FontStack,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::NthChildSelector,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::RoundedCorners,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::StrokeStyling,
            C6ObservedMechanismDisposition::Applied,
        ),
        (
            ReferenceThemeMechanism::ThemeVariables,
            C6ObservedMechanismDisposition::Applied,
        ),
    ])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    const WHITE: Self = Self {
        red: u8::MAX,
        green: u8::MAX,
        blue: u8::MAX,
    };

    fn parse_hex(value: &str) -> Self {
        let hex = value
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .unwrap_or_else(|| {
                panic!("the C6 raster proof requires a six-digit hex color, got {value}")
            });
        Self {
            red: u8::from_str_radix(&hex[0..2], 16).expect("valid red channel"),
            green: u8::from_str_radix(&hex[2..4], 16).expect("valid green channel"),
            blue: u8::from_str_radix(&hex[4..6], 16).expect("valid blue channel"),
        }
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
    fn from_fixture(input: &ReferenceThemeInput) -> Self {
        let tokens = input.tokens().expect("Brutalist requires typed tokens");
        let node_style = input
            .node_style()
            .expect("Brutalist requires a typed State node style");
        let border = node_style
            .border()
            .expect("Brutalist requires a node border");
        let shadow = node_style
            .shadow()
            .expect("Brutalist requires a hard shadow");
        assert_eq!(shadow.blur_px(), 0);
        assert_eq!(shadow.spread_px(), 0);
        assert!(shadow.offset_x_px() > 0 && shadow.offset_y_px() > 0);
        let palette = match input.semantic_rules() {
            [ReferenceSemanticRule::OrdinalPalette { colors, .. }] => colors,
            _ => panic!("the C6 raster proof requires one State ordinal palette"),
        };
        assert_eq!(palette.len(), 3);
        let shadow = Rgb::parse_hex(shadow.color());
        let quantized_shadow = shadow.linear_rgb_roundtrip();
        assert_ne!(shadow, quantized_shadow);
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
            assert_ne!(Rgb::parse_hex(other), quantized_shadow);
        }

        Self {
            canvas: Rgb::parse_hex(tokens.background()),
            text: Rgb::parse_hex(tokens.text()),
            border: Rgb::parse_hex(border.color()),
            border_width: f64::from(border.width_px()),
            radius: f64::from(
                node_style
                    .corner_radius_px()
                    .expect("Brutalist requires rounded State nodes"),
            ),
            shadow,
            quantized_shadow,
            shadow_offset_x: f64::from(shadow_offset_x(input)),
            shadow_offset_y: f64::from(shadow_offset_y(input)),
            nodes: vec![
                NodeVisualContract::new(STATE_IDS[0], &palette[0]),
                NodeVisualContract::new(STATE_IDS[1], &palette[1]),
                NodeVisualContract::new(STATE_IDS[2], &palette[2]),
                NodeVisualContract::new(STATE_IDS[3], &palette[0]),
            ],
        }
    }
}

fn shadow_offset_x(input: &ReferenceThemeInput) -> i16 {
    input
        .node_style()
        .and_then(|style| style.shadow())
        .expect("Brutalist hard shadow")
        .offset_x_px()
}

fn shadow_offset_y(input: &ReferenceThemeInput) -> i16 {
    input
        .node_style()
        .and_then(|style| style.shadow())
        .expect("Brutalist hard shadow")
        .offset_y_px()
}

#[derive(Debug)]
struct NodeVisualContract {
    id: &'static str,
    fill: Rgb,
}

impl NodeVisualContract {
    fn new(id: &'static str, fill: &str) -> Self {
        Self {
            id,
            fill: Rgb::parse_hex(fill),
        }
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

    fn inset(self, amount: f64) -> Self {
        assert!(self.width > amount * 2.0 && self.height > amount * 2.0);
        Self {
            left: self.left + amount,
            top: self.top + amount,
            width: self.width - amount * 2.0,
            height: self.height - amount * 2.0,
        }
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
    fn from_sealed_svg(svg: &str) -> Self {
        let document = roxmltree::Document::parse(svg).expect("parse the sealed C6 SVG geometry");
        let root = document.root_element();
        let view_box = parse_view_box(root.attribute("viewBox").expect("C6 SVG viewBox"));
        assert_eq!(root.attribute("preserveAspectRatio"), None);

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
                assert_eq!(groups.len(), 1, "expected one State group for {state_id}");
                let rects = groups[0]
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
                assert_eq!(rects.len(), 1, "expected one State rect for {state_id}");
                NodeGeometry {
                    id: state_id,
                    rect: transformed_rect(rects[0]),
                }
            })
            .collect();
        Self { view_box, nodes }
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

fn transformed_rect(node: roxmltree::Node<'_, '_>) -> Rect {
    let x = number_attribute(node, "x");
    let y = number_attribute(node, "y");
    let width = number_attribute(node, "width");
    let height = number_attribute(node, "height");
    assert!(width > 0.0 && height > 0.0);

    let mut transform = AffineTransform::IDENTITY;
    let ancestors = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in ancestors.into_iter().rev() {
        let parsed = raw
            .parse::<svgtypes::Transform>()
            .unwrap_or_else(|error| panic!("invalid C6 geometry transform {raw:?}: {error}"));
        transform = transform.multiply(AffineTransform::from_svg(parsed));
    }
    assert!(transform.b.abs() <= 1e-9 && transform.c.abs() <= 1e-9);
    assert!(transform.a > 0.0 && transform.d > 0.0);
    let (left, top) = transform.apply(x, y);
    let (right, bottom) = transform.apply(x + width, y + height);
    Rect {
        left,
        top,
        width: right - left,
        height: bottom - top,
    }
}

fn parse_view_box(value: &str) -> Rect {
    let values = value
        .split_ascii_whitespace()
        .map(|part| part.parse::<f64>().expect("finite C6 viewBox number"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4);
    assert!(values.iter().all(|value| value.is_finite()));
    assert!(values[2] > 0.0 && values[3] > 0.0);
    Rect {
        left: values[0],
        top: values[1],
        width: values[2],
        height: values[3],
    }
}

fn number_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> f64 {
    node.attribute(name)
        .unwrap_or_else(|| panic!("missing {name} on C6 geometry node"))
        .parse()
        .unwrap_or_else(|error| panic!("invalid {name} on C6 geometry node: {error}"))
}

#[derive(Clone)]
struct RasterImage {
    width: u32,
    height: u32,
    pixels: Vec<[u8; 4]>,
}

impl RasterImage {
    fn decode_png(bytes: &[u8]) -> Self {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().expect("decode C6 PNG header");
        let mut buffer = vec![
            0;
            reader
                .output_buffer_size()
                .expect("bounded C6 PNG output buffer")
        ];
        let frame = reader.next_frame(&mut buffer).expect("decode C6 PNG frame");
        assert_eq!(frame.color_type, png::ColorType::Rgba);
        assert_eq!(frame.bit_depth, png::BitDepth::Eight);
        let pixels = buffer[..frame.buffer_size()]
            .chunks_exact(4)
            .map(|pixel| [pixel[0], pixel[1], pixel[2], pixel[3]])
            .collect();
        Self {
            width: frame.width,
            height: frame.height,
            pixels,
        }
    }

    fn decode_jpeg(bytes: &[u8]) -> Self {
        let image = image::load_from_memory_with_format(bytes, ImageFormat::Jpeg)
            .expect("decode final C6 JPEG")
            .to_rgb8();
        let (width, height) = image.dimensions();
        let pixels = image
            .pixels()
            .map(|pixel| [pixel[0], pixel[1], pixel[2], u8::MAX])
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }

    fn transform(&self, view_box: Rect) -> RasterTransform {
        RasterTransform {
            view_box,
            width: self.width,
            height: self.height,
        }
    }

    fn pixel(&self, x: i32, y: i32) -> [u8; 4] {
        assert!(x >= 0 && y >= 0);
        let x = u32::try_from(x).expect("non-negative pixel x");
        let y = u32::try_from(y).expect("non-negative pixel y");
        assert!(x < self.width && y < self.height);
        self.pixels[(y * self.width + x) as usize]
    }

    fn rgb(&self, x: i32, y: i32) -> Option<Rgb> {
        let pixel = self.pixel(x, y);
        (pixel[3] == u8::MAX).then_some(Rgb {
            red: pixel[0],
            green: pixel[1],
            blue: pixel[2],
        })
    }

    #[cfg(test)]
    fn set_rgb(&mut self, x: i32, y: i32, color: Rgb) {
        assert!(x >= 0 && y >= 0);
        let x = u32::try_from(x).expect("non-negative pixel x");
        let y = u32::try_from(y).expect("non-negative pixel y");
        assert!(x < self.width && y < self.height);
        self.pixels[(y * self.width + x) as usize] = [color.red, color.green, color.blue, u8::MAX];
    }
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

fn assert_brutalist_state_raster(
    contract: &BrutalistStateVisualContract,
    geometry: &StateRasterGeometry,
    raster: &RasterImage,
    tolerance: u8,
) {
    let transform = raster.transform(geometry.view_box);
    assert!(transform.scale_x() >= 1.9 && transform.scale_y() >= 1.9);
    assert!((transform.scale_x() - transform.scale_y()).abs() <= 0.02);
    assert_canvas(contract, geometry, transform, raster, tolerance);

    for (expected, actual) in contract.nodes.iter().zip(&geometry.nodes) {
        assert_eq!(expected.id, actual.id);
        assert_node_fill(
            contract,
            expected,
            actual.rect,
            transform,
            raster,
            tolerance,
        );
        assert_node_stroke(contract, actual.rect, transform, raster, tolerance);
        assert_node_rounding(contract, actual.rect, transform, raster, tolerance);
        assert_node_shadow(contract, actual.rect, transform, raster, tolerance);
        assert_node_label_ink(contract, actual.rect, transform, raster, tolerance);
    }
}

fn assert_canvas(
    contract: &BrutalistStateVisualContract,
    geometry: &StateRasterGeometry,
    transform: RasterTransform,
    raster: &RasterImage,
    tolerance: u8,
) {
    let exclusion =
        contract.border_width / 2.0 + contract.shadow_offset_x.max(contract.shadow_offset_y) + 1.0;
    let expanded_nodes = geometry
        .nodes
        .iter()
        .map(|node| node.rect.expanded(exclusion))
        .collect::<Vec<_>>();
    assert_color_region_coverage(
        raster,
        transform,
        geometry.view_box,
        |x, y| !expanded_nodes.iter().any(|rect| rect.contains(x, y)),
        &[contract.canvas],
        tolerance,
        coverage_threshold(tolerance, MIN_PNG_CANVAS_COVERAGE, MIN_JPEG_CANVAS_COVERAGE),
        "canvas outside diagram nodes",
    );
}

fn assert_node_fill(
    contract: &BrutalistStateVisualContract,
    expected: &NodeVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
    tolerance: u8,
) {
    let inset = contract.border_width / 2.0 + 0.75 / transform.scale_x();
    let interior = rect.inset(inset);
    let fill_candidates = filtered_color_candidates(expected.fill);
    assert_color_region_coverage(
        raster,
        transform,
        interior,
        |_, y| {
            let relative_y = (y - rect.top) / rect.height;
            relative_y <= 0.34 || relative_y >= 0.68
        },
        &fill_candidates,
        tolerance,
        coverage_threshold(tolerance, MIN_PNG_FILL_COVERAGE, MIN_JPEG_FILL_COVERAGE),
        &format!("{} ordinal fill interior", expected.id),
    );
}

fn assert_node_stroke(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
    tolerance: u8,
) {
    let clearance = contract.radius + contract.border_width + 1.0;
    let band_half_width = contract.border_width * 0.25;
    let horizontal_width = rect.width - clearance * 2.0;
    let vertical_height = rect.height - clearance * 2.0;
    assert!(horizontal_width > 0.0 && vertical_height > 0.0);
    let border_candidates = filtered_color_candidates(contract.border);
    for (label, region) in stroke_regions(rect, clearance, band_half_width) {
        assert_color_region_coverage(
            raster,
            transform,
            region,
            |_, _| true,
            &border_candidates,
            tolerance,
            coverage_threshold(tolerance, MIN_PNG_STROKE_COVERAGE, MIN_JPEG_STROKE_COVERAGE),
            label,
        );
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
                tolerance,
            ),
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
                tolerance,
            ),
            expected_horizontal_width,
        ));
    }
    let allowed_error = if tolerance == 0 { 2 } else { 3 };
    for (actual, expected) in runs {
        assert!(
            actual.abs_diff(expected) <= allowed_error,
            "State stroke width must remain within {allowed_error}px of {expected}px, found {actual}px"
        );
    }
}

fn assert_node_rounding(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
    tolerance: u8,
) {
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
    let threshold =
        coverage_threshold(tolerance, MIN_PNG_CORNER_COVERAGE, MIN_JPEG_CORNER_COVERAGE);
    for (label, patch) in corners {
        let coverage = rounded_corner_coverage(
            raster,
            transform,
            patch,
            outer,
            outer_radius,
            contract.canvas,
            tolerance,
        );
        assert!(
            coverage.inside_total >= 3 && coverage.outside_total >= 3,
            "{label} must retain enough classified pixels; inside={}, outside={}",
            coverage.inside_total,
            coverage.outside_total
        );
        assert!(
            coverage.inside_ratio() >= threshold,
            "{label} must retain the expected occupied arc; coverage={:.3} ({}/{})",
            coverage.inside_ratio(),
            coverage.inside_matching,
            coverage.inside_total
        );
        assert!(
            coverage.outside_ratio() >= threshold,
            "{label} must retain the expected canvas cutout; coverage={:.3} ({}/{})",
            coverage.outside_ratio(),
            coverage.outside_matching,
            coverage.outside_total
        );
    }
}

fn assert_node_shadow(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
    tolerance: u8,
) {
    let accepted_shadow = [contract.shadow, contract.quantized_shadow];
    let guard = (1.25 / transform.scale_x()).max(0.6);
    assert!(contract.shadow_offset_x > guard * 2.0);
    assert!(contract.shadow_offset_y > guard * 2.0);
    let source = rect.expanded(contract.border_width / 2.0);
    let clearance = contract.radius + contract.border_width / 2.0 + guard;
    let vertical_span = source.height - clearance * 2.0;
    let horizontal_span = source.width - clearance * 2.0;
    assert!(vertical_span > 0.0 && horizontal_span > 0.0);
    let [(right_label, right_band), (bottom_label, bottom_band)] =
        shadow_bands(contract, rect, transform);
    for (label, region) in [(right_label, right_band), (bottom_label, bottom_band)] {
        assert_color_region_coverage(
            raster,
            transform,
            region,
            |_, _| true,
            &accepted_shadow,
            tolerance,
            coverage_threshold(tolerance, MIN_PNG_SHADOW_COVERAGE, MIN_JPEG_SHADOW_COVERAGE),
            label,
        );
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
        assert_color_region_coverage(
            raster,
            transform,
            region,
            |_, _| true,
            &[contract.canvas],
            tolerance,
            coverage_threshold(
                tolerance,
                MIN_PNG_SHADOW_CONTROL_COVERAGE,
                MIN_JPEG_SHADOW_CONTROL_COVERAGE,
            ),
            label,
        );
    }
}

fn assert_node_label_ink(
    contract: &BrutalistStateVisualContract,
    rect: Rect,
    transform: RasterTransform,
    raster: &RasterImage,
    tolerance: u8,
) {
    let mask = label_ink_mask(contract, rect, transform, raster, tolerance);
    let ink_pixels = mask.true_count();
    let minimum_ink =
        ((MIN_LABEL_INK_PIXELS as f64 / 4.0) * transform.scale_x() * transform.scale_y()).round()
            as usize;
    assert!(
        ink_pixels >= minimum_ink,
        "each final native artifact must retain structured embedded-font label ink; found {ink_pixels} pixels, expected at least {minimum_ink}"
    );
    let bounds = mask.tight_bounds().expect("label ink bounds");
    let minimum_width = (8.0 * transform.scale_x()).round() as usize;
    let minimum_height = (4.0 * transform.scale_y()).round() as usize;
    assert!(
        bounds.width >= minimum_width && bounds.height >= minimum_height,
        "label ink must span at least {minimum_width}x{minimum_height}px; found {}x{}px",
        bounds.width,
        bounds.height,
    );
    let density = ink_pixels as f64 / (bounds.width * bounds.height) as f64;
    assert!(
        (0.06..=0.78).contains(&density),
        "label ink density must remain glyph-like; density={density:.3}"
    );
    assert!(
        mask.occupied_column_count(&bounds) >= (6.0 * transform.scale_x()).round() as usize,
        "label ink must span enough occupied columns"
    );
    assert!(
        mask.occupied_row_count(&bounds) >= (3.0 * transform.scale_y()).round() as usize,
        "label ink must span enough occupied rows"
    );
}

fn assert_jpeg_tracks_png(
    jpeg: &RasterImage,
    png: &RasterImage,
    contract: &BrutalistStateVisualContract,
    geometry: &StateRasterGeometry,
) {
    let mut squared_error = 0.0;
    let mut delta_histogram = [0usize; 256];
    for (jpeg_pixel, png_pixel) in jpeg.pixels.iter().zip(&png.pixels) {
        let png_composited = composite_rgba_over(*png_pixel, Rgb::WHITE);
        for (&jpeg_channel, &png_channel) in jpeg_pixel[..3].iter().zip(&png_composited) {
            let delta = jpeg_channel.abs_diff(png_channel);
            delta_histogram[usize::from(delta)] += 1;
            squared_error += f64::from(delta) * f64::from(delta);
        }
    }
    let channel_count = usize::try_from(jpeg.width)
        .expect("bounded JPEG width")
        .saturating_mul(usize::try_from(jpeg.height).expect("bounded JPEG height"))
        .saturating_mul(3);
    let mean_squared_error = squared_error / channel_count as f64;
    let psnr = 10.0 * ((255.0 * 255.0) / mean_squared_error).log10();
    assert!(
        psnr >= MIN_JPEG_PSNR_DB,
        "C6 JPEG must track the proved PNG; PSNR={psnr:.3}dB"
    );
    let p99_target = (channel_count * 99).div_ceil(100);
    let mut cumulative = 0usize;
    let mut p99_delta = 0u8;
    for (delta, count) in delta_histogram.into_iter().enumerate() {
        cumulative += count;
        if cumulative >= p99_target {
            p99_delta = u8::try_from(delta).expect("channel delta fits in u8");
            break;
        }
    }
    assert!(
        p99_delta <= MAX_JPEG_P99_CHANNEL_DELTA,
        "C6 JPEG p99 channel delta must remain bounded; p99={p99_delta}"
    );

    let transform = jpeg.transform(geometry.view_box);
    assert_color_mask_tracks_png(
        jpeg,
        png,
        transform,
        geometry.view_box,
        &[contract.canvas],
        0,
        18,
        "canvas",
    );
    for (expected, node) in contract.nodes.iter().zip(&geometry.nodes) {
        let fill = filtered_color_candidates(expected.fill);
        assert_color_mask_tracks_png(
            jpeg,
            png,
            transform,
            node.rect.inset(1.0),
            &fill,
            0,
            18,
            &format!("{} ordinal fill", node.id),
        );
        let clearance = contract.radius + contract.border_width + 1.0;
        let band_half_width = contract.border_width * 0.32;
        for (label, region) in stroke_regions(node.rect, clearance, band_half_width) {
            assert_color_mask_tracks_png(
                jpeg,
                png,
                transform,
                region,
                &filtered_color_candidates(contract.border),
                4,
                18,
                &format!("{} {label}", node.id),
            );
        }
        for (label, region) in shadow_bands(contract, node.rect, transform) {
            assert_color_mask_tracks_png(
                jpeg,
                png,
                transform,
                region,
                &[contract.shadow, contract.quantized_shadow],
                4,
                18,
                &format!("{} {label}", node.id),
            );
        }
        let label_region = label_region(contract, node.rect);
        assert_color_mask_tracks_png(
            jpeg,
            png,
            transform,
            label_region,
            &filtered_color_candidates(contract.text),
            8,
            26,
            &format!("{} label ink", node.id),
        );
    }
}

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

fn coverage_threshold(tolerance: u8, lossless: f64, lossy: f64) -> f64 {
    if tolerance == 0 { lossless } else { lossy }
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
    tolerance: u8,
) -> BinaryMask {
    color_mask(
        raster,
        transform,
        label_region(contract, rect),
        &filtered_color_candidates(contract.text),
        tolerance.saturating_add(8),
    )
}

#[allow(clippy::too_many_arguments)]
fn assert_color_region_coverage(
    raster: &RasterImage,
    transform: RasterTransform,
    bounds: Rect,
    include: impl Fn(f64, f64) -> bool,
    expected: &[Rgb],
    tolerance: u8,
    minimum_coverage: f64,
    label: &str,
) {
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
    assert!(total > 0, "{label} C6 region must contain pixels");
    let coverage = matching as f64 / total as f64;
    assert!(
        coverage >= minimum_coverage,
        "{label} must cover at least {:.0}% of its C6 region; coverage={coverage:.3}, matching={matching}, total={total}, raster={}x{}",
        minimum_coverage * 100.0,
        raster.width,
        raster.height,
    );
}

fn color_mask(
    raster: &RasterImage,
    transform: RasterTransform,
    bounds: Rect,
    expected: &[Rgb],
    tolerance: u8,
) -> BinaryMask {
    let pixel_bounds = transform.pixel_bounds(bounds);
    let width =
        usize::try_from(pixel_bounds.right - pixel_bounds.left).expect("positive mask width");
    let height =
        usize::try_from(pixel_bounds.bottom - pixel_bounds.top).expect("positive mask height");
    assert!(width > 0 && height > 0);
    let mut bits = Vec::with_capacity(width * height);
    for y in pixel_bounds.top..pixel_bounds.bottom {
        for x in pixel_bounds.left..pixel_bounds.right {
            bits.push(matches_any_color(raster.rgb(x, y), expected, tolerance));
        }
    }
    BinaryMask {
        width,
        height,
        bits,
    }
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
struct RoundedCornerCoverage {
    inside_matching: usize,
    inside_total: usize,
    outside_matching: usize,
    outside_total: usize,
}

impl RoundedCornerCoverage {
    fn inside_ratio(&self) -> f64 {
        self.inside_matching as f64 / self.inside_total as f64
    }

    fn outside_ratio(&self) -> f64 {
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
) -> usize {
    let expected = filtered_color_candidates(expected);
    let radius = i32::try_from(radius).expect("bounded stroke scan radius");
    let color_at = |offset: i32| {
        let point = match axis {
            ScanAxis::Horizontal => (center.0 + offset, center.1),
            ScanAxis::Vertical => (center.0, center.1 + offset),
        };
        matches_any_color(raster.rgb(point.0, point.1), &expected, tolerance)
    };
    assert!(color_at(0));
    let mut first = 0;
    while first > -radius && color_at(first - 1) {
        first -= 1;
    }
    let mut last = 0;
    while last < radius && color_at(last + 1) {
        last += 1;
    }
    usize::try_from(last - first + 1).expect("positive color run")
}

#[allow(clippy::too_many_arguments)]
fn assert_color_mask_tracks_png(
    jpeg: &RasterImage,
    png: &RasterImage,
    transform: RasterTransform,
    bounds: Rect,
    expected: &[Rgb],
    png_tolerance: u8,
    jpeg_tolerance: u8,
    label: &str,
) {
    let pixel_bounds = transform.pixel_bounds(bounds);
    let mut true_positive = 0usize;
    let mut false_positive = 0usize;
    let mut false_negative = 0usize;
    let mut reference_positive = 0usize;
    for y in pixel_bounds.top..pixel_bounds.bottom {
        for x in pixel_bounds.left..pixel_bounds.right {
            let expected_in_png = matches_any_color(png.rgb(x, y), expected, png_tolerance);
            let observed_in_jpeg = matches_any_color(jpeg.rgb(x, y), expected, jpeg_tolerance);
            reference_positive += expected_in_png as usize;
            match (expected_in_png, observed_in_jpeg) {
                (true, true) => true_positive += 1,
                (false, true) => false_positive += 1,
                (true, false) => false_negative += 1,
                (false, false) => {}
            }
        }
    }
    assert!(
        reference_positive > 0,
        "{label} PNG mask must contain evidence"
    );
    let precision = true_positive as f64 / (true_positive + false_positive).max(1) as f64;
    let recall = true_positive as f64 / (true_positive + false_negative).max(1) as f64;
    let iou =
        true_positive as f64 / (true_positive + false_positive + false_negative).max(1) as f64;
    assert!(
        precision >= MIN_JPEG_MASK_PRECISION
            && recall >= MIN_JPEG_MASK_RECALL
            && iou >= MIN_JPEG_MASK_IOU,
        "{label} JPEG mask must track PNG locally; precision={precision:.3}, recall={recall:.3}, iou={iou:.3}"
    );
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

    fn contract() -> BrutalistStateVisualContract {
        let shadow = Rgb::parse_hex("#111111");
        BrutalistStateVisualContract {
            canvas: Rgb::parse_hex("#f7f3e8"),
            text: Rgb::parse_hex("#111111"),
            border: Rgb::parse_hex("#111111"),
            border_width: 3.0,
            radius: 2.0,
            shadow,
            quantized_shadow: shadow.linear_rgb_roundtrip(),
            shadow_offset_x: 6.0,
            shadow_offset_y: 6.0,
            nodes: vec![NodeVisualContract::new("Ready", "#facc15")],
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
            pixels: vec![
                [
                    contract.canvas.red,
                    contract.canvas.green,
                    contract.canvas.blue,
                    u8::MAX,
                ];
                240 * 180
            ],
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

    fn assert_rejected(expected_message: &str, action: impl FnOnce() + std::panic::UnwindSafe) {
        let panic = std::panic::catch_unwind(action)
            .expect_err("the raster proof accepted a deliberately incomplete mechanism");
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic payload");
        assert!(
            message.contains(expected_message),
            "expected rejection containing {expected_message:?}, got {message:?}"
        );
    }

    #[test]
    fn corner_only_canvas_is_rejected() {
        let contract = contract();
        let geometry = geometry();
        let mut raster = RasterImage {
            width: 240,
            height: 180,
            pixels: vec![[255, 255, 255, u8::MAX]; 240 * 180],
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

        assert_rejected("canvas outside diagram nodes", || {
            assert_canvas(
                &contract,
                &geometry,
                raster.transform(geometry.view_box),
                &raster,
                0,
            );
        });
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

        assert_rejected("ordinal fill interior", || {
            assert_node_fill(&contract, &contract.nodes[0], rect, transform, &raster, 0);
        });
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

        assert_rejected("left border", || {
            assert_node_stroke(&contract, rect, transform, &raster, 0);
        });
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

        assert_rejected("top-left rounded corner", || {
            assert_node_rounding(&contract, rect, transform, &raster, 0);
        });
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

        assert_rejected("right hard-shadow band", || {
            assert_node_shadow(&contract, rect, transform, &raster, 0);
        });
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

        assert_rejected("structured embedded-font label ink", || {
            assert_node_label_ink(&contract, rect, transform, &raster, 0);
        });
    }

    #[test]
    fn jpeg_local_mask_rejects_background_dominated_feature_loss() {
        let contract = contract();
        let geometry = geometry();
        let mut png = raster();
        let mut jpeg = raster();
        let transform = png.transform(geometry.view_box);
        let region = Rect {
            left: 35.0,
            top: 35.0,
            width: 40.0,
            height: 10.0,
        };
        let pixel_bounds = transform.pixel_bounds(region);
        let mut painted = 0usize;
        for y in pixel_bounds.top..pixel_bounds.bottom {
            for x in pixel_bounds.left..pixel_bounds.right {
                if painted < 120 {
                    png.set_rgb(x, y, contract.text);
                    if painted < 20 {
                        jpeg.set_rgb(x, y, contract.text);
                    }
                    painted += 1;
                }
            }
        }
        assert_eq!(painted, 120);

        assert_rejected("JPEG mask must track PNG locally", || {
            assert_color_mask_tracks_png(
                &jpeg,
                &png,
                transform,
                region,
                &[contract.text],
                0,
                0,
                "synthetic label",
            );
        });
    }
}
