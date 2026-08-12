use std::collections::{BTreeMap, BTreeSet};

use lopdf::{
    Document, Object, ObjectId, Stream,
    content::{Content, Operation},
};
use merman_theme_fixtures::{
    C6ArtifactAssertion, ReferenceSemanticRule, ReferenceThemeInput, ReferenceThemeMechanism,
};

use super::C6ObservedMechanismDisposition;

const MAX_PAGE_CONTENT_BYTES: usize = 1024 * 1024;
const MAX_FILTER_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
const PLACEMENT_TOLERANCE: f64 = 0.02;
const TRANSFORM_EPSILON: f64 = 1e-9;
const EFFECTIVELY_OPAQUE_ALPHA: u8 = 250;
const STATE_IDS: [&str; 4] = ["Ready", "Review", "Done", "Archive"];

pub(crate) fn prove_brutalist_state_pdf(
    input: &ReferenceThemeInput,
    sealed_svg: &str,
    bytes: &[u8],
    filter_scale: f32,
) -> super::C6TargetProof {
    let contract = BrutalistStatePdfContract::from_fixture(input, filter_scale);
    let geometry = StateSvgGeometry::from_sealed_svg(sealed_svg)
        .unwrap_or_else(|error| panic!("read the sealed C6 State geometry: {error}"));
    let document = Document::load_mem(bytes).expect("parse the final C6 PDF artifact");
    let pages = document.get_pages();
    assert_eq!(pages.len(), 1, "the C6 PDF must contain exactly one page");
    let page_id = *pages.values().next().expect("the C6 PDF page");
    let page_content = document
        .get_page_content_with_limit(page_id, MAX_PAGE_CONTENT_BYTES)
        .expect("decode the bounded C6 PDF page content");
    let operations = Content::decode_strict(&page_content)
        .expect("strictly decode the final C6 PDF page operators");

    let page_box = page_media_box(&document, page_id)
        .unwrap_or_else(|error| panic!("read the final C6 PDF page box: {error}"));
    let resources = PageDrawingResources::from_page(&document, page_id)
        .unwrap_or_else(|error| panic!("read the final C6 PDF drawing resources: {error}"));
    let placements = collect_drawn_page_images(
        &operations.operations,
        &resources.images,
        &resources.ext_gstates,
        page_box,
    )
    .unwrap_or_else(|error| panic!("validate the final C6 PDF image placements: {error}"));
    assert_eq!(
        placements.len(),
        contract.node_fills.len(),
        "each State node filter must become one drawn PDF image XObject"
    );

    let observed_images = placements
        .into_iter()
        .map(|placement| PlacedStateImage {
            image: assert_filtered_state_image(&document, placement.image_id, &contract),
            placement,
        })
        .collect::<Vec<_>>();
    let expected = expected_state_placements(&geometry, page_box, &contract)
        .unwrap_or_else(|error| panic!("project the sealed C6 State geometry into PDF: {error}"));
    validate_state_image_placements(&expected, &observed_images, page_box)
        .unwrap_or_else(|error| panic!("bind final C6 PDF images to State geometry: {error}"));
    assert_canvas_solid(
        &operations.operations,
        &resources.ext_gstates,
        page_box,
        contract.canvas,
    )
    .unwrap_or_else(|error| panic!("prove the final C6 PDF canvas paint: {error}"));
    let font_resource = assert_embedded_font(&document, page_id, contract.font_family);
    assert_state_text_labels(
        &document,
        page_id,
        &operations.operations,
        &resources.ext_gstates,
        &expected,
        contract.text,
        &font_resource,
    )
    .unwrap_or_else(|error| panic!("prove the final C6 PDF State labels: {error}"));

    super::C6TargetProof::verified(
        C6ArtifactAssertion::PdfDocument,
        bytes,
        applied_mechanisms(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    fn parse_hex(value: &str) -> Self {
        let hex = value
            .strip_prefix('#')
            .filter(|hex| hex.len() == 6)
            .unwrap_or_else(|| {
                panic!("the C6 PDF proof requires a six-digit hex color, got {value}")
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

    fn components(self) -> [f32; 3] {
        [
            f32::from(self.red) / 255.0,
            f32::from(self.green) / 255.0,
            f32::from(self.blue) / 255.0,
        ]
    }
}

struct BrutalistStatePdfContract<'a> {
    canvas: Rgb,
    text: Rgb,
    font_family: &'a str,
    node_fills: [Rgb; 4],
    quantized_shadow: Rgb,
    border_width_px: usize,
    corner_radius_px: usize,
    shadow_offset_x_px: usize,
    shadow_offset_y_px: usize,
    filter_scale: f64,
}

impl<'a> BrutalistStatePdfContract<'a> {
    fn from_fixture(input: &'a ReferenceThemeInput, filter_scale: f32) -> Self {
        assert!(filter_scale.is_finite() && filter_scale > 0.0);
        let tokens = input.tokens().expect("Brutalist requires typed tokens");
        let font_stack = input.font_stack().expect("Brutalist requires a font stack");
        assert_eq!(font_stack.families().len(), 1);
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
            _ => panic!("the C6 PDF proof requires one State ordinal palette"),
        };
        assert_eq!(palette.len(), 3);

        let scaled = |value: usize| (value as f32 * filter_scale).round() as usize;
        Self {
            canvas: Rgb::parse_hex(tokens.background()),
            text: Rgb::parse_hex(tokens.text()),
            font_family: &font_stack.families()[0],
            node_fills: [
                Rgb::parse_hex(&palette[0]).linear_rgb_roundtrip(),
                Rgb::parse_hex(&palette[1]).linear_rgb_roundtrip(),
                Rgb::parse_hex(&palette[2]).linear_rgb_roundtrip(),
                Rgb::parse_hex(&palette[0]).linear_rgb_roundtrip(),
            ],
            quantized_shadow: Rgb::parse_hex(shadow.color()).linear_rgb_roundtrip(),
            border_width_px: scaled(usize::from(border.width_px())),
            corner_radius_px: scaled(usize::from(
                node_style
                    .corner_radius_px()
                    .expect("Brutalist requires rounded State nodes"),
            )),
            shadow_offset_x_px: scaled(usize::from(shadow.offset_x_px().unsigned_abs())),
            shadow_offset_y_px: scaled(usize::from(shadow.offset_y_px().unsigned_abs())),
            filter_scale: f64::from(filter_scale),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Rect {
    fn from_xywh(x: f64, y: f64, width: f64, height: f64) -> Result<Self, String> {
        if ![x, y, width, height].into_iter().all(f64::is_finite) {
            return Err("rectangle coordinates must be finite".to_owned());
        }
        if width <= 0.0 || height <= 0.0 {
            return Err(format!(
                "rectangle dimensions must be positive, found {width} x {height}"
            ));
        }
        Ok(Self {
            min_x: x,
            min_y: y,
            max_x: x + width,
            max_y: y + height,
        })
    }

    fn from_corners(x0: f64, y0: f64, x1: f64, y1: f64) -> Result<Self, String> {
        Self::from_xywh(x0.min(x1), y0.min(y1), (x1 - x0).abs(), (y1 - y0).abs())
    }

    fn width(self) -> f64 {
        self.max_x - self.min_x
    }

    fn height(self) -> f64 {
        self.max_y - self.min_y
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let intersection = Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        };
        (intersection.width() > TRANSFORM_EPSILON && intersection.height() > TRANSFORM_EPSILON)
            .then_some(intersection)
    }

    fn contains(self, inner: Self, tolerance: f64) -> bool {
        self.min_x <= inner.min_x + tolerance
            && self.min_y <= inner.min_y + tolerance
            && self.max_x + tolerance >= inner.max_x
            && self.max_y + tolerance >= inner.max_y
    }

    fn contains_point(self, point: (f64, f64), tolerance: f64) -> bool {
        self.min_x <= point.0 + tolerance
            && self.min_y <= point.1 + tolerance
            && self.max_x + tolerance >= point.0
            && self.max_y + tolerance >= point.1
    }

    fn approximately_equals(self, other: Self, tolerance: f64) -> bool {
        (self.min_x - other.min_x).abs() <= tolerance
            && (self.min_y - other.min_y).abs() <= tolerance
            && (self.max_x - other.max_x).abs() <= tolerance
            && (self.max_y - other.max_y).abs() <= tolerance
    }
}

#[derive(Clone, Copy, Debug)]
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

    fn from_pdf(operation: &Operation) -> Result<Self, String> {
        if operation.operands.len() != 6 {
            return Err(format!(
                "PDF cm must contain six operands, found {}",
                operation.operands.len()
            ));
        }
        let mut values = [0.0; 6];
        for (index, value) in values.iter_mut().enumerate() {
            *value = pdf_number(operation, index)?;
        }
        let transform = Self {
            a: values[0],
            b: values[1],
            c: values[2],
            d: values[3],
            e: values[4],
            f: values[5],
        };
        transform.ensure_finite()?;
        Ok(transform)
    }

    fn ensure_finite(self) -> Result<(), String> {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .into_iter()
            .all(f64::is_finite)
            .then_some(())
            .ok_or_else(|| "PDF transformation matrix must be finite".to_owned())
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

    fn axis_aligned_rect(self, rect: Rect) -> Result<Rect, String> {
        self.ensure_axis_aligned()?;
        let (x0, y0) = self.apply(rect.min_x, rect.min_y);
        let (x1, y1) = self.apply(rect.max_x, rect.max_y);
        Rect::from_corners(x0, y0, x1, y1)
    }

    fn image_bbox(self) -> Result<Rect, String> {
        self.ensure_axis_aligned()?;
        if self.a <= TRANSFORM_EPSILON || self.d <= TRANSFORM_EPSILON {
            return Err(format!(
                "drawn PDF image must have positive non-zero scale, found [{}, {}]",
                self.a, self.d
            ));
        }
        Rect::from_xywh(self.e, self.f, self.a, self.d)
    }

    fn ensure_axis_aligned(self) -> Result<(), String> {
        self.ensure_finite()?;
        if self.b.abs() > TRANSFORM_EPSILON || self.c.abs() > TRANSFORM_EPSILON {
            return Err(format!(
                "C6 PDF image and clip transforms must be axis-aligned, found b={} c={}",
                self.b, self.c
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct StateSvgGeometry {
    view_box: Rect,
    nodes: Vec<StateSvgNode>,
}

impl StateSvgGeometry {
    fn from_sealed_svg(svg: &str) -> Result<Self, String> {
        let document = roxmltree::Document::parse(svg)
            .map_err(|error| format!("parse the sealed SVG: {error}"))?;
        let root = document.root_element();
        if !root.has_tag_name("svg") {
            return Err("sealed C6 document must have an SVG root".to_owned());
        }
        let view_box = parse_svg_view_box(
            root.attribute("viewBox")
                .ok_or_else(|| "sealed C6 SVG must declare a viewBox".to_owned())?,
        )?;

        let state_groups = document
            .descendants()
            .filter(|node| node.has_tag_name("g"))
            .filter_map(|node| {
                let id = node.attribute("id")?;
                STATE_IDS
                    .iter()
                    .copied()
                    .find(|state_id| id.contains(&format!("-state-{state_id}-")))
                    .map(|state_id| (state_id, node))
            })
            .collect::<Vec<_>>();
        let observed_order = state_groups
            .iter()
            .map(|(state_id, _)| *state_id)
            .collect::<Vec<_>>();
        if observed_order != STATE_IDS {
            return Err(format!(
                "sealed State ordinal order must be {STATE_IDS:?}, found {observed_order:?}"
            ));
        }

        let mut nodes = Vec::with_capacity(STATE_IDS.len());
        for (state_id, group) in state_groups {
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
            if rects.len() != 1 {
                return Err(format!(
                    "sealed State node {state_id} must contain one basic label rect, found {}",
                    rects.len()
                ));
            }
            let rect = rects[0];
            let source_bbox = transformed_svg_rect(rect)?;
            let filter_id = rect
                .attribute("filter")
                .and_then(|value| value.strip_prefix("url(#"))
                .and_then(|value| value.strip_suffix(')'))
                .ok_or_else(|| {
                    format!("sealed State node {state_id} must reference one local SVG filter")
                })?;
            let filters = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("filter") && node.attribute("id") == Some(filter_id)
                })
                .collect::<Vec<_>>();
            if filters.len() != 1 {
                return Err(format!(
                    "sealed State node {state_id} must resolve exactly one SVG filter, found {}",
                    filters.len()
                ));
            }
            let filter = filters[0];
            if filter.attribute("filterUnits") != Some("objectBoundingBox") {
                return Err(format!(
                    "sealed State node {state_id} filter must use objectBoundingBox units"
                ));
            }
            let filter_x = svg_ratio_attribute(filter, "x")?;
            let filter_y = svg_ratio_attribute(filter, "y")?;
            let filter_width = svg_ratio_attribute(filter, "width")?;
            let filter_height = svg_ratio_attribute(filter, "height")?;
            let filter_bbox = Rect::from_xywh(
                source_bbox.min_x + filter_x * source_bbox.width(),
                source_bbox.min_y + filter_y * source_bbox.height(),
                filter_width * source_bbox.width(),
                filter_height * source_bbox.height(),
            )?;
            if !filter_bbox.contains(source_bbox, PLACEMENT_TOLERANCE) {
                return Err(format!(
                    "sealed State node {state_id} filter region must contain its source rect"
                ));
            }
            nodes.push(StateSvgNode {
                state_id,
                source_bbox,
                filter_bbox,
            });
        }

        Ok(Self { view_box, nodes })
    }
}

#[derive(Clone, Debug)]
struct StateSvgNode {
    state_id: &'static str,
    source_bbox: Rect,
    filter_bbox: Rect,
}

fn transformed_svg_rect(node: roxmltree::Node<'_, '_>) -> Result<Rect, String> {
    let local = Rect::from_xywh(
        svg_number_attribute(node, "x")?,
        svg_number_attribute(node, "y")?,
        svg_number_attribute(node, "width")?,
        svg_number_attribute(node, "height")?,
    )?;
    let mut transform = AffineTransform::IDENTITY;
    let ancestors = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in ancestors.into_iter().rev() {
        let parsed = raw
            .parse::<svgtypes::Transform>()
            .map_err(|error| format!("invalid sealed SVG transform {raw:?}: {error}"))?;
        transform = transform.multiply(AffineTransform::from_svg(parsed));
    }
    if transform.a <= TRANSFORM_EPSILON || transform.d <= TRANSFORM_EPSILON {
        return Err("sealed State rect must retain positive scale".to_owned());
    }
    transform.axis_aligned_rect(local)
}

fn parse_svg_view_box(value: &str) -> Result<Rect, String> {
    let values = value
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>()
                .map_err(|error| format!("invalid SVG viewBox number {part:?}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 4 {
        return Err(format!(
            "sealed C6 SVG viewBox must have four numbers, found {}",
            values.len()
        ));
    }
    Rect::from_xywh(values[0], values[1], values[2], values[3])
}

fn svg_number_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> Result<f64, String> {
    let raw = node
        .attribute(name)
        .ok_or_else(|| format!("missing {name} on sealed C6 SVG geometry"))?;
    let value = raw
        .parse::<f64>()
        .map_err(|error| format!("invalid SVG {name} value {raw:?}: {error}"))?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| format!("SVG {name} value must be finite"))
}

fn svg_ratio_attribute(node: roxmltree::Node<'_, '_>, name: &str) -> Result<f64, String> {
    let raw = node
        .attribute(name)
        .ok_or_else(|| format!("missing {name} on sealed C6 SVG filter"))?;
    let (number, divisor) = raw
        .strip_suffix('%')
        .map_or((raw, 1.0), |percentage| (percentage, 100.0));
    let value = number
        .parse::<f64>()
        .map_err(|error| format!("invalid SVG filter {name} value {raw:?}: {error}"))?
        / divisor;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| format!("SVG filter {name} value must be finite"))
}

#[derive(Clone, Copy, Debug)]
struct ExtGraphicsState {
    fill_alpha: Option<f64>,
    has_soft_mask: Option<bool>,
}

#[derive(Debug)]
struct PageDrawingResources {
    images: BTreeMap<Vec<u8>, ObjectId>,
    ext_gstates: BTreeMap<Vec<u8>, ExtGraphicsState>,
}

impl PageDrawingResources {
    fn from_page(document: &Document, page_id: ObjectId) -> Result<Self, String> {
        let page = document
            .get_dictionary(page_id)
            .map_err(|error| format!("read PDF page dictionary: {error}"))?;
        let resources = page
            .get_deref(b"Resources", document)
            .and_then(Object::as_dict)
            .map_err(|error| format!("read PDF page Resources: {error}"))?;
        let xobjects = resources
            .get_deref(b"XObject", document)
            .and_then(Object::as_dict)
            .map_err(|error| format!("read PDF page XObject resources: {error}"))?;

        let mut images = BTreeMap::new();
        for (name, object) in xobjects.iter() {
            let (object_id, object) = document
                .dereference(object)
                .map_err(|error| format!("dereference PDF XObject {name:?}: {error}"))?;
            let stream = object
                .as_stream()
                .map_err(|error| format!("PDF XObject {name:?} must be a stream: {error}"))?;
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Image") {
                continue;
            }
            let object_id =
                object_id.ok_or_else(|| format!("PDF image XObject {name:?} must be indirect"))?;
            images.insert(name.to_vec(), object_id);
        }

        let mut ext_gstates = BTreeMap::new();
        if let Ok(states) = resources
            .get_deref(b"ExtGState", document)
            .and_then(Object::as_dict)
        {
            for (name, object) in states.iter() {
                let (_, object) = document
                    .dereference(object)
                    .map_err(|error| format!("dereference PDF ExtGState {name:?}: {error}"))?;
                let dictionary = object.as_dict().map_err(|error| {
                    format!("PDF ExtGState {name:?} must be a dictionary: {error}")
                })?;
                let fill_alpha = dictionary
                    .get(b"ca")
                    .ok()
                    .map(pdf_object_number)
                    .transpose()?;
                if fill_alpha.is_some_and(|alpha| !(0.0..=1.0).contains(&alpha)) {
                    return Err(format!(
                        "PDF ExtGState {name:?} fill alpha must be between zero and one"
                    ));
                }
                let has_soft_mask = dictionary.get(b"SMask").ok().map(
                    |mask| !matches!(mask, Object::Name(value) if value.as_slice() == b"None"),
                );
                ext_gstates.insert(
                    name.to_vec(),
                    ExtGraphicsState {
                        fill_alpha,
                        has_soft_mask,
                    },
                );
            }
        }

        Ok(Self {
            images,
            ext_gstates,
        })
    }
}

fn page_media_box(document: &Document, page_id: ObjectId) -> Result<Rect, String> {
    let mut current = page_id;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current) {
            return Err("PDF page parent chain contains a cycle".to_owned());
        }
        let dictionary = document
            .get_dictionary(current)
            .map_err(|error| format!("read PDF page ancestor {current:?}: {error}"))?;
        if let Ok(media_box) = dictionary.get_deref(b"MediaBox", document) {
            let values = media_box
                .as_array()
                .map_err(|error| format!("PDF MediaBox must be an array: {error}"))?;
            if values.len() != 4 {
                return Err(format!(
                    "PDF MediaBox must contain four numbers, found {}",
                    values.len()
                ));
            }
            return Rect::from_corners(
                pdf_object_number(&values[0])?,
                pdf_object_number(&values[1])?,
                pdf_object_number(&values[2])?,
                pdf_object_number(&values[3])?,
            );
        }
        current = dictionary
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(|error| format!("PDF page tree lacks an inherited MediaBox: {error}"))?;
    }
}

#[derive(Clone, Debug)]
struct PdfGraphicsState {
    ctm: AffineTransform,
    clip: PdfClip,
    fill_alpha: f64,
    has_soft_mask: bool,
}

#[derive(Clone, Copy, Debug)]
enum PdfClip {
    Rect(Rect),
    Empty,
    Unknown,
}

impl PdfClip {
    fn intersect(&mut self, path: Option<Rect>) {
        *self = match (&*self, path) {
            (Self::Empty, _) => Self::Empty,
            (Self::Unknown, _) | (_, None) => Self::Unknown,
            (Self::Rect(current), Some(path)) => {
                current.intersection(path).map_or(Self::Empty, Self::Rect)
            }
        };
    }
}

#[derive(Default)]
struct PdfPath {
    explicit_rect: Option<Rect>,
    points: Vec<(f64, f64)>,
    closed: bool,
    unsupported: bool,
    pending_clip: bool,
}

impl PdfPath {
    fn add_rect(&mut self, rect: Option<Rect>) {
        match (self.explicit_rect, rect) {
            (None, Some(rect)) if self.points.is_empty() && !self.unsupported => {
                self.explicit_rect = Some(rect);
            }
            _ => self.unsupported = true,
        }
    }

    fn move_to(&mut self, point: (f64, f64)) {
        if !self.points.is_empty() || self.explicit_rect.is_some() {
            self.unsupported = true;
        }
        self.points.push(point);
    }

    fn line_to(&mut self, point: (f64, f64)) {
        if self.points.is_empty() || self.closed || self.explicit_rect.is_some() {
            self.unsupported = true;
        }
        self.points.push(point);
    }

    fn close(&mut self) {
        if self.points.len() < 3 || self.explicit_rect.is_some() {
            self.unsupported = true;
        }
        self.closed = true;
    }

    fn mark_unsupported(&mut self) {
        self.unsupported = true;
    }

    fn finish(&mut self, clip: &mut PdfClip) -> Result<(), String> {
        if self.pending_clip {
            clip.intersect(self.axis_aligned_rect());
        }
        *self = Self::default();
        Ok(())
    }

    fn axis_aligned_rect(&self) -> Option<Rect> {
        if self.unsupported {
            return None;
        }
        if let Some(rect) = self.explicit_rect {
            return Some(rect);
        }
        if !self.closed {
            return None;
        }
        let mut points = self.points.as_slice();
        if points.len() == 5 && points_approximately_equal(points[0], points[4]) {
            points = &points[..4];
        }
        if points.len() != 4 {
            return None;
        }
        let min_x = points.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|(x, _)| *x)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = points.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
        let max_y = points
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::NEG_INFINITY, f64::max);
        let rect = Rect::from_corners(min_x, min_y, max_x, max_y).ok()?;
        let mut corners = BTreeSet::new();
        for &(x, y) in points {
            let x_side = approximate_side(x, min_x, max_x)?;
            let y_side = approximate_side(y, min_y, max_y)?;
            corners.insert((x_side, y_side));
        }
        (corners.len() == 4).then_some(rect)
    }
}

fn points_approximately_equal(left: (f64, f64), right: (f64, f64)) -> bool {
    (left.0 - right.0).abs() <= TRANSFORM_EPSILON && (left.1 - right.1).abs() <= TRANSFORM_EPSILON
}

fn approximate_side(value: f64, low: f64, high: f64) -> Option<u8> {
    if (value - low).abs() <= TRANSFORM_EPSILON {
        Some(0)
    } else if (value - high).abs() <= TRANSFORM_EPSILON {
        Some(1)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
struct PlacedPdfImage {
    name: Vec<u8>,
    image_id: ObjectId,
    bbox: Rect,
    clip: Rect,
}

fn collect_drawn_page_images(
    operations: &[Operation],
    image_ids: &BTreeMap<Vec<u8>, ObjectId>,
    ext_gstates: &BTreeMap<Vec<u8>, ExtGraphicsState>,
    page_box: Rect,
) -> Result<Vec<PlacedPdfImage>, String> {
    let mut state = PdfGraphicsState {
        ctm: AffineTransform::IDENTITY,
        clip: PdfClip::Rect(page_box),
        fill_alpha: 1.0,
        has_soft_mask: false,
    };
    let mut stack = Vec::new();
    let mut path = PdfPath::default();
    let mut placements = Vec::new();

    for operation in operations {
        match operation.operator.as_str() {
            "q" => {
                require_operand_count(operation, 0)?;
                stack.push(state.clone());
            }
            "Q" => {
                require_operand_count(operation, 0)?;
                state = stack.pop().ok_or_else(|| {
                    "PDF Q operator underflowed the graphics-state stack".to_owned()
                })?;
            }
            "cm" => {
                state.ctm = state.ctm.multiply(AffineTransform::from_pdf(operation)?);
                state.ctm.ensure_finite()?;
            }
            "gs" => {
                require_operand_count(operation, 1)?;
                let name = operation.operands[0]
                    .as_name()
                    .map_err(|error| format!("PDF gs operand must be a name: {error}"))?;
                let update = ext_gstates
                    .get(name)
                    .ok_or_else(|| format!("PDF gs references unknown ExtGState {name:?}"))?;
                if let Some(fill_alpha) = update.fill_alpha {
                    state.fill_alpha = fill_alpha;
                }
                if let Some(has_soft_mask) = update.has_soft_mask {
                    state.has_soft_mask = has_soft_mask;
                }
            }
            "re" => {
                require_operand_count(operation, 4)?;
                let local = Rect::from_xywh(
                    pdf_number(operation, 0)?,
                    pdf_number(operation, 1)?,
                    pdf_number(operation, 2)?,
                    pdf_number(operation, 3)?,
                );
                let transformed = local
                    .and_then(|rect| state.ctm.axis_aligned_rect(rect))
                    .ok();
                path.add_rect(transformed);
            }
            "m" => {
                require_operand_count(operation, 2)?;
                path.move_to(
                    state
                        .ctm
                        .apply(pdf_number(operation, 0)?, pdf_number(operation, 1)?),
                );
            }
            "l" => {
                require_operand_count(operation, 2)?;
                path.line_to(
                    state
                        .ctm
                        .apply(pdf_number(operation, 0)?, pdf_number(operation, 1)?),
                );
            }
            "h" => {
                require_operand_count(operation, 0)?;
                path.close();
            }
            "c" | "v" | "y" => path.mark_unsupported(),
            "W" | "W*" => {
                require_operand_count(operation, 0)?;
                path.pending_clip = true;
            }
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                path.finish(&mut state.clip)?;
            }
            "Do" => {
                require_operand_count(operation, 1)?;
                let name = operation.operands[0]
                    .as_name()
                    .map_err(|error| format!("PDF Do operand must be a name: {error}"))?;
                let Some(&image_id) = image_ids.get(name) else {
                    continue;
                };
                let bbox = state.ctm.image_bbox()?;
                let clip = match state.clip {
                    PdfClip::Rect(clip) => clip,
                    PdfClip::Empty => {
                        return Err(format!("PDF image {name:?} is fully clipped"));
                    }
                    PdfClip::Unknown => {
                        return Err(format!(
                            "PDF image {name:?} is drawn under an unverified non-rectangular clip"
                        ));
                    }
                };
                if bbox.intersection(clip).is_none() {
                    return Err(format!(
                        "PDF image {name:?} is outside the visible page clip"
                    ));
                }
                if state.fill_alpha < 1.0 - 1e-6 {
                    return Err(format!(
                        "PDF image {name:?} must be opaque, found alpha {}",
                        state.fill_alpha
                    ));
                }
                if state.has_soft_mask {
                    return Err(format!(
                        "PDF image {name:?} cannot inherit a graphics-state soft mask"
                    ));
                }
                placements.push(PlacedPdfImage {
                    name: name.to_vec(),
                    image_id,
                    bbox,
                    clip,
                });
            }
            _ => {}
        }
    }

    if path.pending_clip {
        return Err("PDF content ended before applying a pending clip".to_owned());
    }
    if !stack.is_empty() {
        return Err("PDF content ended with unbalanced q/Q graphics state".to_owned());
    }
    let drawn_counts = placements.iter().fold(
        BTreeMap::<Vec<u8>, usize>::new(),
        |mut counts, placement| {
            *counts.entry(placement.name.clone()).or_default() += 1;
            counts
        },
    );
    for name in image_ids.keys() {
        if drawn_counts.get(name) != Some(&1) {
            return Err(format!(
                "each page Image XObject must be drawn exactly once; {name:?} was drawn {} times",
                drawn_counts.get(name).copied().unwrap_or_default()
            ));
        }
    }
    if placements.len() != image_ids.len() {
        return Err(format!(
            "page drew {} Image XObjects but declared {} image resources",
            placements.len(),
            image_ids.len()
        ));
    }
    Ok(placements)
}

fn require_operand_count(operation: &Operation, expected: usize) -> Result<(), String> {
    (operation.operands.len() == expected)
        .then_some(())
        .ok_or_else(|| {
            format!(
                "PDF {} operator requires {expected} operands, found {}",
                operation.operator,
                operation.operands.len()
            )
        })
}

fn pdf_number(operation: &Operation, index: usize) -> Result<f64, String> {
    operation
        .operands
        .get(index)
        .ok_or_else(|| {
            format!(
                "PDF {} operator is missing operand {index}",
                operation.operator
            )
        })
        .and_then(pdf_object_number)
}

fn pdf_object_number(object: &Object) -> Result<f64, String> {
    let value = object
        .as_float()
        .map(f64::from)
        .map_err(|error| format!("PDF coordinate must be numeric: {error}"))?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| "PDF coordinate must be finite".to_owned())
}

#[derive(Clone, Debug)]
struct ExpectedStatePlacement {
    state_id: &'static str,
    source_bbox: Rect,
    image_bbox: Rect,
    fill: Rgb,
    image_width: usize,
    image_height: usize,
}

fn expected_state_placements(
    geometry: &StateSvgGeometry,
    page_box: Rect,
    contract: &BrutalistStatePdfContract<'_>,
) -> Result<Vec<ExpectedStatePlacement>, String> {
    if geometry.nodes.len() != contract.node_fills.len() {
        return Err(format!(
            "sealed SVG contains {} State nodes but the ordinal contract contains {} fills",
            geometry.nodes.len(),
            contract.node_fills.len()
        ));
    }
    let scale_x = page_box.width() / geometry.view_box.width();
    let scale_y = page_box.height() / geometry.view_box.height();
    if (scale_x - scale_y).abs() > 1e-6 * scale_x.max(scale_y).max(1.0) {
        return Err(format!(
            "PDF page must preserve the sealed SVG aspect ratio, found scales {scale_x} and {scale_y}"
        ));
    }

    geometry
        .nodes
        .iter()
        .zip(contract.node_fills)
        .map(|(node, fill)| {
            let source_bbox =
                project_svg_rect_to_pdf(node.source_bbox, geometry.view_box, page_box)?;
            let image_bbox =
                project_svg_rect_to_pdf(node.filter_bbox, geometry.view_box, page_box)?;
            if !page_box.contains(source_bbox, PLACEMENT_TOLERANCE) {
                return Err(format!(
                    "sealed State node {} source geometry lies outside the PDF page",
                    node.state_id
                ));
            }
            if !image_bbox.contains(source_bbox, PLACEMENT_TOLERANCE) {
                return Err(format!(
                    "sealed State node {} filter geometry does not contain its source",
                    node.state_id
                ));
            }
            Ok(ExpectedStatePlacement {
                state_id: node.state_id,
                source_bbox,
                image_bbox,
                fill,
                image_width: scaled_image_dimension(
                    node.filter_bbox.width(),
                    contract.filter_scale,
                )?,
                image_height: scaled_image_dimension(
                    node.filter_bbox.height(),
                    contract.filter_scale,
                )?,
            })
        })
        .collect()
}

fn project_svg_rect_to_pdf(svg_rect: Rect, view_box: Rect, page_box: Rect) -> Result<Rect, String> {
    let scale_x = page_box.width() / view_box.width();
    let scale_y = page_box.height() / view_box.height();
    Rect::from_xywh(
        page_box.min_x + (svg_rect.min_x - view_box.min_x) * scale_x,
        page_box.min_y + (view_box.max_y - svg_rect.max_y) * scale_y,
        svg_rect.width() * scale_x,
        svg_rect.height() * scale_y,
    )
}

fn scaled_image_dimension(points: f64, filter_scale: f64) -> Result<usize, String> {
    let pixels = points * filter_scale;
    if !pixels.is_finite() || pixels < 1.0 || pixels > usize::MAX as f64 {
        return Err(format!("invalid expected PDF filter image size {pixels}"));
    }
    Ok(pixels.round() as usize)
}

#[derive(Clone, Copy, Debug)]
struct FilteredStateImage {
    fill: Rgb,
    width: usize,
    height: usize,
    visible_bounds: PixelBounds,
}

#[derive(Clone, Debug)]
struct PlacedStateImage {
    placement: PlacedPdfImage,
    image: FilteredStateImage,
}

fn validate_state_image_placements(
    expected: &[ExpectedStatePlacement],
    observed: &[PlacedStateImage],
    page_box: Rect,
) -> Result<(), String> {
    if expected.len() != observed.len() {
        return Err(format!(
            "expected {} placed State images, found {}",
            expected.len(),
            observed.len()
        ));
    }
    let mut matched = BTreeSet::new();
    for expected_node in expected {
        let candidates = observed
            .iter()
            .enumerate()
            .filter(|(_, observed_image)| {
                observed_image
                    .placement
                    .bbox
                    .approximately_equals(expected_node.image_bbox, PLACEMENT_TOLERANCE)
            })
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(format!(
                "State node {} must match exactly one PDF image placement, found {}",
                expected_node.state_id,
                candidates.len()
            ));
        }
        let (index, observed_image) = candidates[0];
        if !matched.insert(index) {
            return Err(format!(
                "PDF image placement {index} was matched to more than one State node"
            ));
        }
        if observed_image.image.fill != expected_node.fill {
            return Err(format!(
                "State node {} placement contains ordinal fill {:?}, expected {:?}",
                expected_node.state_id, observed_image.image.fill, expected_node.fill
            ));
        }
        if observed_image
            .image
            .width
            .abs_diff(expected_node.image_width)
            > 1
            || observed_image
                .image
                .height
                .abs_diff(expected_node.image_height)
                > 1
        {
            return Err(format!(
                "State node {} image raster is {}x{}, expected {}x{}",
                expected_node.state_id,
                observed_image.image.width,
                observed_image.image.height,
                expected_node.image_width,
                expected_node.image_height
            ));
        }
        if !page_box.contains(expected_node.source_bbox, PLACEMENT_TOLERANCE) {
            return Err(format!(
                "State node {} source geometry is outside the PDF page",
                expected_node.state_id
            ));
        }
        let visible_bbox = observed_image
            .image
            .visible_pdf_bbox(observed_image.placement.bbox)?;
        if !observed_image
            .placement
            .clip
            .contains(visible_bbox, PLACEMENT_TOLERANCE)
        {
            return Err(format!(
                "State node {} visible filter pixels, including its hard shadow, are hidden by the active PDF clip: visible={visible_bbox:?}, clip={:?}",
                expected_node.state_id, observed_image.placement.clip
            ));
        }
    }
    if matched.len() != observed.len() {
        return Err("one or more drawn PDF images did not bind to a State node".to_owned());
    }
    Ok(())
}

fn assert_filtered_state_image(
    document: &Document,
    image_id: ObjectId,
    contract: &BrutalistStatePdfContract<'_>,
) -> FilteredStateImage {
    let image = document
        .get_object(image_id)
        .and_then(Object::as_stream)
        .expect("read a drawn C6 PDF image XObject");
    assert_image_dictionary(image, b"DeviceRGB");
    let width = positive_dimension(image, b"Width");
    let height = positive_dimension(image, b"Height");
    let pixel_count = width.checked_mul(height).expect("bounded PDF image area");
    let rgb_len = pixel_count
        .checked_mul(3)
        .expect("bounded PDF RGB image size");
    assert!(rgb_len <= MAX_FILTER_IMAGE_BYTES);
    let rgb = image
        .decompressed_content_with_limit(rgb_len)
        .expect("decode the bounded C6 PDF RGB filter image");
    assert_eq!(rgb.len(), rgb_len);

    let mask_id = image
        .dict
        .get(b"SMask")
        .and_then(Object::as_reference)
        .expect("each C6 PDF filter image must own a soft mask");
    let mask = document
        .get_object(mask_id)
        .and_then(Object::as_stream)
        .expect("read the C6 PDF filter image soft mask");
    assert_image_dictionary(mask, b"DeviceGray");
    assert_eq!(positive_dimension(mask, b"Width"), width);
    assert_eq!(positive_dimension(mask, b"Height"), height);
    let alpha = mask
        .decompressed_content_with_limit(pixel_count)
        .expect("decode the bounded C6 PDF alpha mask");
    assert_eq!(alpha.len(), pixel_count);
    assert!(alpha.contains(&0));
    assert!(alpha.contains(&u8::MAX));
    let visible_bounds = alpha_bounds(&alpha, width)
        .expect("the final PDF filter image must contain visible pixels");

    let (fill, fill_count) = contract
        .node_fills
        .iter()
        .copied()
        .map(|color| (color, count_color(&rgb, &alpha, color)))
        .max_by_key(|(_, count)| *count)
        .expect("the C6 PDF node palette is non-empty");
    assert!(
        fill_count >= pixel_count / 4,
        "one ordinal fill must dominate each localized State filter image"
    );
    for other in contract
        .node_fills
        .iter()
        .copied()
        .filter(|color| *color != fill)
    {
        assert_eq!(
            count_color(&rgb, &alpha, other),
            0,
            "a localized State image cannot contain another ordinal fill"
        );
    }

    let fill_bounds = color_bounds(&rgb, &alpha, width, fill)
        .expect("the dominant State fill must have a pixel extent");
    let shadow_bounds = color_bounds(&rgb, &alpha, width, contract.quantized_shadow)
        .expect("the final PDF image must contain the quantized hard shadow");
    assert!(
        count_color(&rgb, &alpha, contract.quantized_shadow) >= pixel_count / 20,
        "the final PDF image must contain a substantial hard-shadow region"
    );
    assert!(shadow_bounds.contains(fill_bounds));

    let left = fill_bounds.min_x - shadow_bounds.min_x;
    let top = fill_bounds.min_y - shadow_bounds.min_y;
    let right = shadow_bounds.max_x - fill_bounds.max_x;
    let bottom = shadow_bounds.max_y - fill_bounds.max_y;
    assert_near(left, contract.border_width_px, 2, "left border extent");
    assert_near(top, contract.border_width_px, 2, "top border extent");
    let directional_tolerance = contract.border_width_px.max(2) / 3 + 1;
    assert_near(
        right,
        contract.border_width_px + contract.shadow_offset_x_px,
        directional_tolerance,
        "right hard-shadow extent",
    );
    assert_near(
        bottom,
        contract.border_width_px + contract.shadow_offset_y_px,
        directional_tolerance,
        "bottom hard-shadow extent",
    );
    assert!(right > left && bottom > top);

    assert!(
        !pixel_is_effectively_opaque_color(
            &rgb,
            &alpha,
            width,
            fill_bounds.min_x,
            fill_bounds.min_y,
            fill,
        ),
        "the final PDF State fill must retain a rounded top-left corner"
    );
    let top_row_gap = (fill_bounds.min_x..=fill_bounds.max_x)
        .position(|x| {
            pixel_is_effectively_opaque_color(&rgb, &alpha, width, x, fill_bounds.min_y, fill)
        })
        .expect("the rounded State top edge must contain the ordinal fill");
    assert!(top_row_gap > 0);
    assert!(top_row_gap <= contract.corner_radius_px.max(1) + 1);

    FilteredStateImage {
        fill,
        width,
        height,
        visible_bounds,
    }
}

fn assert_image_dictionary(stream: &Stream, color_space: &[u8]) {
    assert_eq!(
        stream
            .dict
            .get(b"Type")
            .and_then(Object::as_name)
            .expect("PDF image XObject type"),
        b"XObject"
    );
    assert_eq!(
        stream
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name)
            .expect("PDF image XObject subtype"),
        b"Image"
    );
    assert_eq!(
        stream
            .dict
            .get(b"ColorSpace")
            .and_then(Object::as_name)
            .expect("PDF image color space"),
        color_space
    );
    assert_eq!(
        stream
            .dict
            .get(b"BitsPerComponent")
            .and_then(Object::as_i64)
            .expect("PDF image bits per component"),
        8
    );
    assert_eq!(
        stream
            .dict
            .get(b"Filter")
            .and_then(Object::as_name)
            .expect("PDF image stream filter"),
        b"FlateDecode"
    );
}

fn positive_dimension(stream: &Stream, key: &[u8]) -> usize {
    let value = stream
        .dict
        .get(key)
        .and_then(Object::as_i64)
        .unwrap_or_else(|error| panic!("invalid PDF image dimension {key:?}: {error}"));
    usize::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .expect("PDF image dimensions must be positive")
}

#[derive(Clone, Copy, Debug)]
struct PixelBounds {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

impl PixelBounds {
    fn contains(self, inner: Self) -> bool {
        self.min_x <= inner.min_x
            && self.min_y <= inner.min_y
            && self.max_x >= inner.max_x
            && self.max_y >= inner.max_y
    }
}

impl FilteredStateImage {
    fn visible_pdf_bbox(&self, placement: Rect) -> Result<Rect, String> {
        if self.visible_bounds.max_x >= self.width || self.visible_bounds.max_y >= self.height {
            return Err("PDF filter alpha bounds exceed the image dimensions".to_owned());
        }
        let x_scale = placement.width() / self.width as f64;
        let y_scale = placement.height() / self.height as f64;
        Rect::from_corners(
            placement.min_x + self.visible_bounds.min_x as f64 * x_scale,
            placement.max_y - (self.visible_bounds.max_y + 1) as f64 * y_scale,
            placement.min_x + (self.visible_bounds.max_x + 1) as f64 * x_scale,
            placement.max_y - self.visible_bounds.min_y as f64 * y_scale,
        )
    }
}

fn alpha_bounds(alpha: &[u8], width: usize) -> Option<PixelBounds> {
    if width == 0 || !alpha.len().is_multiple_of(width) {
        return None;
    }
    let mut bounds: Option<PixelBounds> = None;
    for (index, value) in alpha.iter().copied().enumerate() {
        if value == 0 {
            continue;
        }
        let x = index % width;
        let y = index / width;
        bounds = Some(match bounds {
            Some(bounds) => PixelBounds {
                min_x: bounds.min_x.min(x),
                min_y: bounds.min_y.min(y),
                max_x: bounds.max_x.max(x),
                max_y: bounds.max_y.max(y),
            },
            None => PixelBounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
        });
    }
    bounds
}

fn color_bounds(rgb: &[u8], alpha: &[u8], width: usize, color: Rgb) -> Option<PixelBounds> {
    let mut bounds: Option<PixelBounds> = None;
    for (index, pixel) in rgb.chunks_exact(3).enumerate() {
        if alpha[index] < EFFECTIVELY_OPAQUE_ALPHA || rgb_from_slice(pixel) != color {
            continue;
        }
        let x = index % width;
        let y = index / width;
        bounds = Some(match bounds {
            Some(bounds) => PixelBounds {
                min_x: bounds.min_x.min(x),
                min_y: bounds.min_y.min(y),
                max_x: bounds.max_x.max(x),
                max_y: bounds.max_y.max(y),
            },
            None => PixelBounds {
                min_x: x,
                min_y: y,
                max_x: x,
                max_y: y,
            },
        });
    }
    bounds
}

fn count_color(rgb: &[u8], alpha: &[u8], color: Rgb) -> usize {
    rgb.chunks_exact(3)
        .zip(alpha)
        .filter(|(pixel, alpha)| {
            **alpha >= EFFECTIVELY_OPAQUE_ALPHA && rgb_from_slice(pixel) == color
        })
        .count()
}

fn pixel_is_effectively_opaque_color(
    rgb: &[u8],
    alpha: &[u8],
    width: usize,
    x: usize,
    y: usize,
    color: Rgb,
) -> bool {
    let pixel_index = y * width + x;
    let rgb_offset = pixel_index * 3;
    alpha[pixel_index] >= EFFECTIVELY_OPAQUE_ALPHA
        && rgb_from_slice(&rgb[rgb_offset..rgb_offset + 3]) == color
}

fn rgb_from_slice(pixel: &[u8]) -> Rgb {
    Rgb {
        red: pixel[0],
        green: pixel[1],
        blue: pixel[2],
    }
}

fn assert_near(actual: usize, expected: usize, tolerance: usize, label: &str) {
    assert!(
        actual.abs_diff(expected) <= tolerance,
        "{label} must be {expected}px +/- {tolerance}px, found {actual}px"
    );
}

#[derive(Clone, Debug)]
struct PdfPagePaintState {
    ctm: AffineTransform,
    clip: PdfClip,
    fill: Option<Rgb>,
    fill_alpha: f64,
    has_soft_mask: bool,
}

fn assert_canvas_solid(
    operations: &[Operation],
    ext_gstates: &BTreeMap<Vec<u8>, ExtGraphicsState>,
    page_box: Rect,
    canvas: Rgb,
) -> Result<(), String> {
    let mut state = PdfPagePaintState {
        ctm: AffineTransform::IDENTITY,
        clip: PdfClip::Rect(page_box),
        fill: None,
        fill_alpha: 1.0,
        has_soft_mask: false,
    };
    let mut stack = Vec::new();
    let mut path = PdfPath::default();
    let mut painted = false;

    for operation in operations {
        match operation.operator.as_str() {
            "q" => {
                require_operand_count(operation, 0)?;
                stack.push(state.clone());
            }
            "Q" => {
                require_operand_count(operation, 0)?;
                state = stack.pop().ok_or_else(|| {
                    "PDF Q operator underflowed the canvas graphics-state stack".to_owned()
                })?;
            }
            "cm" => {
                state.ctm = state.ctm.multiply(AffineTransform::from_pdf(operation)?);
                state.ctm.ensure_finite()?;
            }
            "gs" => apply_page_ext_gstate(operation, ext_gstates, &mut state)?,
            "rg" | "g" => state.fill = Some(pdf_fill_color(operation)?),
            "k" | "cs" | "sc" | "scn" => state.fill = None,
            "re" => {
                require_operand_count(operation, 4)?;
                let transformed = Rect::from_xywh(
                    pdf_number(operation, 0)?,
                    pdf_number(operation, 1)?,
                    pdf_number(operation, 2)?,
                    pdf_number(operation, 3)?,
                )
                .and_then(|rect| state.ctm.axis_aligned_rect(rect))
                .ok();
                path.add_rect(transformed);
            }
            "m" => {
                require_operand_count(operation, 2)?;
                path.move_to(
                    state
                        .ctm
                        .apply(pdf_number(operation, 0)?, pdf_number(operation, 1)?),
                );
            }
            "l" => {
                require_operand_count(operation, 2)?;
                path.line_to(
                    state
                        .ctm
                        .apply(pdf_number(operation, 0)?, pdf_number(operation, 1)?),
                );
            }
            "h" => {
                require_operand_count(operation, 0)?;
                path.close();
            }
            "c" | "v" | "y" => path.mark_unsupported(),
            "W" | "W*" => {
                require_operand_count(operation, 0)?;
                path.pending_clip = true;
            }
            "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                if let (Some(fill), Some(paint_bbox), PdfClip::Rect(clip)) =
                    (state.fill, path.axis_aligned_rect(), state.clip)
                    && fill == canvas
                    && state.fill_alpha >= 1.0 - 1e-6
                    && !state.has_soft_mask
                    && paint_bbox.contains(page_box, PLACEMENT_TOLERANCE)
                    && clip.contains(page_box, PLACEMENT_TOLERANCE)
                {
                    painted = true;
                }
                path.finish(&mut state.clip)?;
            }
            "n" | "S" | "s" => path.finish(&mut state.clip)?,
            _ => {}
        }
    }

    if path.pending_clip {
        return Err("PDF content ended before applying a pending canvas clip".to_owned());
    }
    if !stack.is_empty() {
        return Err("PDF content ended with unbalanced canvas q/Q graphics state".to_owned());
    }
    painted.then_some(()).ok_or_else(|| {
        "the canvas color must be used by an opaque path fill covering the complete PDF page"
            .to_owned()
    })
}

fn apply_page_ext_gstate(
    operation: &Operation,
    ext_gstates: &BTreeMap<Vec<u8>, ExtGraphicsState>,
    state: &mut PdfPagePaintState,
) -> Result<(), String> {
    require_operand_count(operation, 1)?;
    let name = operation.operands[0]
        .as_name()
        .map_err(|error| format!("PDF gs operand must be a name: {error}"))?;
    let update = ext_gstates
        .get(name)
        .ok_or_else(|| format!("PDF gs references unknown ExtGState {name:?}"))?;
    if let Some(fill_alpha) = update.fill_alpha {
        state.fill_alpha = fill_alpha;
    }
    if let Some(has_soft_mask) = update.has_soft_mask {
        state.has_soft_mask = has_soft_mask;
    }
    Ok(())
}

fn pdf_fill_color(operation: &Operation) -> Result<Rgb, String> {
    let components = match operation.operator.as_str() {
        "rg" => {
            require_operand_count(operation, 3)?;
            [
                pdf_number(operation, 0)?,
                pdf_number(operation, 1)?,
                pdf_number(operation, 2)?,
            ]
        }
        "g" => {
            require_operand_count(operation, 1)?;
            let gray = pdf_number(operation, 0)?;
            [gray; 3]
        }
        operator => return Err(format!("unsupported PDF fill color operator {operator}")),
    };
    if !components
        .iter()
        .all(|component| (0.0..=1.0).contains(component))
    {
        return Err(format!(
            "PDF fill color components must be between zero and one, found {components:?}"
        ));
    }
    Ok(Rgb {
        red: (components[0] * 255.0).round() as u8,
        green: (components[1] * 255.0).round() as u8,
        blue: (components[2] * 255.0).round() as u8,
    })
}

#[derive(Clone, Debug)]
struct PdfTextGraphicsState {
    ctm: AffineTransform,
    clip: PdfClip,
    fill: Option<Rgb>,
    fill_alpha: f64,
    has_soft_mask: bool,
}

#[derive(Clone, Debug)]
struct PdfTextObject {
    matrix: AffineTransform,
    font: Option<Vec<u8>>,
    font_size: Option<f64>,
    rendering_mode: i64,
}

#[derive(Clone, Debug)]
struct PdfTextShow {
    bytes: Vec<u8>,
    origin: (f64, f64),
    font: Vec<u8>,
    fill: Option<Rgb>,
    fill_alpha: f64,
    has_soft_mask: bool,
    rendering_mode: i64,
    clip: PdfClip,
}

fn assert_state_text_labels(
    document: &Document,
    page_id: ObjectId,
    operations: &[Operation],
    ext_gstates: &BTreeMap<Vec<u8>, ExtGraphicsState>,
    expected: &[ExpectedStatePlacement],
    text: Rgb,
    admitted_font: &[u8],
) -> Result<(), String> {
    let fonts = document
        .get_page_fonts(page_id)
        .map_err(|error| format!("read the final C6 PDF page fonts: {error}"))?;
    let font = fonts.get(admitted_font).ok_or_else(|| {
        format!("the admitted PDF font resource {admitted_font:?} is not available to the page")
    })?;
    let encoding = font
        .get_font_encoding_with_limit(document, MAX_FONT_BYTES)
        .map_err(|error| format!("read the admitted PDF font encoding: {error}"))?;
    assert_state_text_labels_with_decoder(
        operations,
        ext_gstates,
        expected,
        text,
        admitted_font,
        |bytes| {
            Document::decode_text(&encoding, bytes)
                .map_err(|error| format!("decode a C6 PDF Tj string: {error}"))
        },
    )
}

fn assert_state_text_labels_with_decoder(
    operations: &[Operation],
    ext_gstates: &BTreeMap<Vec<u8>, ExtGraphicsState>,
    expected: &[ExpectedStatePlacement],
    text: Rgb,
    admitted_font: &[u8],
    mut decode: impl FnMut(&[u8]) -> Result<String, String>,
) -> Result<(), String> {
    let shows = collect_pdf_text_shows(operations, ext_gstates)?;
    for state_id in STATE_IDS {
        let expected_node = expected
            .iter()
            .find(|node| node.state_id == state_id)
            .ok_or_else(|| format!("missing expected State geometry for {state_id}"))?;
        let mut node_shows = shows
            .iter()
            .filter(|show| {
                expected_node
                    .source_bbox
                    .contains_point(show.origin, PLACEMENT_TOLERANCE)
            })
            .collect::<Vec<_>>();
        node_shows.sort_by(|left, right| {
            left.origin
                .0
                .total_cmp(&right.origin.0)
                .then_with(|| left.origin.1.total_cmp(&right.origin.1))
        });
        if node_shows.is_empty() {
            return Err(format!(
                "State node {state_id} must contain text shown by PDF Tj operators"
            ));
        }

        let mut observed = String::new();
        for show in node_shows {
            if show.font != admitted_font {
                return Err(format!(
                    "State node {state_id} uses PDF font resource {:?}, expected the admitted resource {admitted_font:?}",
                    show.font
                ));
            }
            if show.fill != Some(text) {
                return Err(format!(
                    "State node {state_id} Tj uses fill {:?}, expected {:?}",
                    show.fill, text
                ));
            }
            if show.fill_alpha < 1.0 - 1e-6 || show.has_soft_mask {
                return Err(format!(
                    "State node {state_id} text must be shown opaquely without a soft mask"
                ));
            }
            if !matches!(show.rendering_mode, 0 | 2 | 4 | 6) {
                return Err(format!(
                    "State node {state_id} Tj rendering mode {} does not fill glyphs",
                    show.rendering_mode
                ));
            }
            match show.clip {
                PdfClip::Rect(clip)
                    if clip.contains(expected_node.source_bbox, PLACEMENT_TOLERANCE) => {}
                _ => {
                    return Err(format!(
                        "State node {state_id} text is hidden by an unverified PDF clip"
                    ));
                }
            }
            observed.push_str(&decode(&show.bytes)?);
        }
        if observed != state_id {
            return Err(format!(
                "State node {state_id} must decode from its positioned Tj operators, found {observed:?}"
            ));
        }
    }
    Ok(())
}

fn collect_pdf_text_shows(
    operations: &[Operation],
    ext_gstates: &BTreeMap<Vec<u8>, ExtGraphicsState>,
) -> Result<Vec<PdfTextShow>, String> {
    let unbounded_clip = Rect::from_xywh(-1.0e12, -1.0e12, 2.0e12, 2.0e12)?;
    let mut state = PdfTextGraphicsState {
        ctm: AffineTransform::IDENTITY,
        clip: PdfClip::Rect(unbounded_clip),
        fill: None,
        fill_alpha: 1.0,
        has_soft_mask: false,
    };
    let mut stack = Vec::new();
    let mut path = PdfPath::default();
    let mut text_object: Option<PdfTextObject> = None;
    let mut shows = Vec::new();

    for operation in operations {
        match operation.operator.as_str() {
            "q" => {
                require_operand_count(operation, 0)?;
                stack.push(state.clone());
            }
            "Q" => {
                require_operand_count(operation, 0)?;
                state = stack.pop().ok_or_else(|| {
                    "PDF Q operator underflowed the text graphics-state stack".to_owned()
                })?;
            }
            "cm" => {
                state.ctm = state.ctm.multiply(AffineTransform::from_pdf(operation)?);
                state.ctm.ensure_finite()?;
            }
            "gs" => {
                let mut page_state = PdfPagePaintState {
                    ctm: state.ctm,
                    clip: state.clip,
                    fill: state.fill,
                    fill_alpha: state.fill_alpha,
                    has_soft_mask: state.has_soft_mask,
                };
                apply_page_ext_gstate(operation, ext_gstates, &mut page_state)?;
                state.fill_alpha = page_state.fill_alpha;
                state.has_soft_mask = page_state.has_soft_mask;
            }
            "rg" | "g" => state.fill = Some(pdf_fill_color(operation)?),
            "k" | "cs" | "sc" | "scn" => state.fill = None,
            "re" => {
                require_operand_count(operation, 4)?;
                let transformed = Rect::from_xywh(
                    pdf_number(operation, 0)?,
                    pdf_number(operation, 1)?,
                    pdf_number(operation, 2)?,
                    pdf_number(operation, 3)?,
                )
                .and_then(|rect| state.ctm.axis_aligned_rect(rect))
                .ok();
                path.add_rect(transformed);
            }
            "m" => {
                require_operand_count(operation, 2)?;
                path.move_to(
                    state
                        .ctm
                        .apply(pdf_number(operation, 0)?, pdf_number(operation, 1)?),
                );
            }
            "l" => {
                require_operand_count(operation, 2)?;
                path.line_to(
                    state
                        .ctm
                        .apply(pdf_number(operation, 0)?, pdf_number(operation, 1)?),
                );
            }
            "h" => {
                require_operand_count(operation, 0)?;
                path.close();
            }
            "c" | "v" | "y" => path.mark_unsupported(),
            "W" | "W*" => {
                require_operand_count(operation, 0)?;
                path.pending_clip = true;
            }
            "n" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                path.finish(&mut state.clip)?;
            }
            "BT" => {
                require_operand_count(operation, 0)?;
                if text_object.is_some() {
                    return Err("PDF BT cannot nest inside an active text object".to_owned());
                }
                text_object = Some(PdfTextObject {
                    matrix: AffineTransform::IDENTITY,
                    font: None,
                    font_size: None,
                    rendering_mode: 0,
                });
            }
            "ET" => {
                require_operand_count(operation, 0)?;
                text_object
                    .take()
                    .ok_or_else(|| "PDF ET must close an active text object".to_owned())?;
            }
            "Tf" => {
                require_operand_count(operation, 2)?;
                let current = text_object
                    .as_mut()
                    .ok_or_else(|| "PDF Tf must occur inside BT/ET".to_owned())?;
                let font = operation.operands[0]
                    .as_name()
                    .map_err(|error| format!("PDF Tf font must be a name: {error}"))?;
                let size = pdf_number(operation, 1)?;
                if size <= 0.0 {
                    return Err(format!("PDF Tf font size must be positive, found {size}"));
                }
                current.font = Some(font.to_vec());
                current.font_size = Some(size);
            }
            "Tm" => {
                let current = text_object
                    .as_mut()
                    .ok_or_else(|| "PDF Tm must occur inside BT/ET".to_owned())?;
                current.matrix = AffineTransform::from_pdf(operation)?;
            }
            "Tr" => {
                require_operand_count(operation, 1)?;
                let current = text_object
                    .as_mut()
                    .ok_or_else(|| "PDF Tr must occur inside BT/ET".to_owned())?;
                let mode = operation.operands[0]
                    .as_i64()
                    .map_err(|error| format!("PDF Tr operand must be an integer: {error}"))?;
                if !(0..=7).contains(&mode) {
                    return Err(format!(
                        "PDF Tr mode must be between zero and seven, found {mode}"
                    ));
                }
                current.rendering_mode = mode;
            }
            "Tj" => {
                require_operand_count(operation, 1)?;
                let current = text_object
                    .as_ref()
                    .ok_or_else(|| "PDF Tj must occur inside BT/ET".to_owned())?;
                let font = current.font.clone().ok_or_else(|| {
                    "PDF Tj must be preceded by Tf in the same text object".to_owned()
                })?;
                if current.font_size.is_none() {
                    return Err("PDF Tj lacks an active positive font size".to_owned());
                }
                let bytes = operation.operands[0]
                    .as_str()
                    .map_err(|error| format!("PDF Tj operand must be a string: {error}"))?;
                let origin = state.ctm.multiply(current.matrix).apply(0.0, 0.0);
                if !origin.0.is_finite() || !origin.1.is_finite() {
                    return Err("PDF Tj origin must be finite".to_owned());
                }
                shows.push(PdfTextShow {
                    bytes: bytes.to_vec(),
                    origin,
                    font,
                    fill: state.fill,
                    fill_alpha: state.fill_alpha,
                    has_soft_mask: state.has_soft_mask,
                    rendering_mode: current.rendering_mode,
                    clip: state.clip,
                });
            }
            _ => {}
        }
    }

    if text_object.is_some() {
        return Err("PDF content ended before ET closed the active text object".to_owned());
    }
    if path.pending_clip {
        return Err("PDF content ended before applying a pending text clip".to_owned());
    }
    if !stack.is_empty() {
        return Err("PDF content ended with unbalanced text q/Q graphics state".to_owned());
    }
    Ok(shows)
}

fn assert_embedded_font(document: &Document, page_id: ObjectId, expected_family: &str) -> Vec<u8> {
    let fonts = document
        .get_page_fonts(page_id)
        .expect("read the final C6 PDF page fonts");
    assert_eq!(fonts.len(), 1, "the C6 PDF must use one admitted font face");
    let (font_resource, font) = fonts.iter().next().expect("the C6 PDF font");
    assert_eq!(
        font.get(b"Subtype")
            .and_then(Object::as_name)
            .expect("the C6 PDF font subtype"),
        b"Type0"
    );
    let base_font = font
        .get(b"BaseFont")
        .and_then(Object::as_name)
        .expect("the C6 PDF Type0 BaseFont");
    assert!(
        String::from_utf8_lossy(base_font).contains(expected_family),
        "the final PDF BaseFont must identify the admitted fixture family"
    );

    let descendants = font
        .get_deref(b"DescendantFonts", document)
        .and_then(Object::as_array)
        .expect("the C6 PDF descendant font array");
    assert_eq!(descendants.len(), 1);
    let (_, descendant) = document
        .dereference(&descendants[0])
        .expect("dereference the C6 PDF descendant font");
    let descriptor = descendant
        .as_dict()
        .and_then(|font| font.get_deref(b"FontDescriptor", document))
        .and_then(Object::as_dict)
        .expect("read the C6 PDF font descriptor");
    let font_file = descriptor
        .get_deref(b"FontFile2", document)
        .and_then(Object::as_stream)
        .expect("the final C6 PDF must embed its TrueType font subset");
    let embedded_font = font_file
        .decompressed_content_with_limit(MAX_FONT_BYTES)
        .expect("decode the bounded C6 PDF font subset");
    assert!(embedded_font.len() > 1024);
    font_resource.clone()
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

    const RED: Rgb = Rgb {
        red: 255,
        green: 0,
        blue: 0,
    };
    const BLUE: Rgb = Rgb {
        red: 0,
        green: 0,
        blue: 255,
    };
    const BLACK: Rgb = Rgb {
        red: 0,
        green: 0,
        blue: 0,
    };

    #[test]
    fn state_placement_binding_rejects_clip_that_crops_filter_shadow() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let expected = ExpectedStatePlacement {
            state_id: "Ready",
            source_bbox: rect(12.0, 12.0, 16.0, 16.0),
            image_bbox: rect(10.0, 10.0, 20.0, 20.0),
            fill: RED,
            image_width: 20,
            image_height: 20,
        };
        let observed = PlacedStateImage {
            placement: PlacedPdfImage {
                name: b"red".to_vec(),
                image_id: (1, 0),
                bbox: expected.image_bbox,
                clip: expected.source_bbox,
            },
            image: FilteredStateImage {
                fill: RED,
                width: 20,
                height: 20,
                visible_bounds: PixelBounds {
                    min_x: 2,
                    min_y: 1,
                    max_x: 18,
                    max_y: 17,
                },
            },
        };

        let error = validate_state_image_placements(&[expected], &[observed], page)
            .expect_err("a source-only clip must not hide the hard-shadow filter extent");

        assert!(error.contains("hard shadow"), "unexpected error: {error}");
    }

    #[test]
    fn state_placement_binding_allows_clipped_transparent_filter_padding() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let expected = ExpectedStatePlacement {
            state_id: "Ready",
            source_bbox: rect(3.0, 14.0, 12.0, 12.0),
            image_bbox: rect(-1.0, 10.0, 20.0, 20.0),
            fill: RED,
            image_width: 20,
            image_height: 20,
        };
        let observed = PlacedStateImage {
            placement: PlacedPdfImage {
                name: b"red".to_vec(),
                image_id: (1, 0),
                bbox: expected.image_bbox,
                clip: page,
            },
            image: FilteredStateImage {
                fill: RED,
                width: 20,
                height: 20,
                visible_bounds: PixelBounds {
                    min_x: 4,
                    min_y: 4,
                    max_x: 18,
                    max_y: 18,
                },
            },
        };

        validate_state_image_placements(&[expected], &[observed], page)
            .expect("page clipping may discard only transparent filter padding");
    }

    #[test]
    fn soft_mask_color_evidence_ignores_barely_visible_pixels() {
        let rgb = [
            RED.red, RED.green, RED.blue, BLUE.red, BLUE.green, BLUE.blue,
        ];
        let alpha = [1, u8::MAX];

        assert_eq!(count_color(&rgb, &alpha, RED), 0);
        assert!(color_bounds(&rgb, &alpha, 2, RED).is_none());
        assert!(!pixel_is_effectively_opaque_color(
            &rgb, &alpha, 2, 0, 0, RED
        ));
        assert_eq!(count_color(&rgb, &alpha, BLUE), 1);
    }

    #[test]
    fn canvas_proof_rejects_unused_color_operator() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let operations = [
            set_rgb(RED),
            set_gray(BLACK),
            Operation::new("Tj", vec![Object::string_literal("one")]),
            Operation::new("Tj", vec![Object::string_literal("two")]),
            Operation::new("Tj", vec![Object::string_literal("three")]),
            Operation::new("Tj", vec![Object::string_literal("four")]),
        ];

        let error = assert_canvas_solid(&operations, &BTreeMap::new(), page, RED)
            .expect_err("declaring the canvas color without painting it must fail");

        assert!(error.contains("path fill"), "unexpected error: {error}");
    }

    #[test]
    fn canvas_proof_accepts_opaque_page_covering_path_fill() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let operations = [
            set_rgb(RED),
            Operation::new(
                "re",
                vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(100),
                    Object::Integer(100),
                ],
            ),
            Operation::new("f", vec![]),
        ];

        assert_canvas_solid(&operations, &BTreeMap::new(), page, RED)
            .expect("an opaque page-covering path must prove the solid canvas");
    }

    #[test]
    fn state_text_proof_rejects_tj_without_local_tf() {
        let operations = [
            Operation::new("BT", vec![]),
            text_matrix(10.0, 10.0),
            Operation::new("Tj", vec![Object::string_literal("Ready")]),
            Operation::new("ET", vec![]),
        ];

        let error = collect_pdf_text_shows(&operations, &BTreeMap::new())
            .expect_err("Tj must use a Tf selected in the same BT/ET object");

        assert!(
            error.contains("preceded by Tf"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn state_text_proof_rejects_unused_admitted_font() {
        let mut operations = vec![
            Operation::new("BT", vec![]),
            set_font(b"F0"),
            Operation::new("ET", vec![]),
            set_gray(BLACK),
        ];
        operations.extend(state_label_operations(b"F1", STATE_IDS));

        let error = assert_state_text_labels_with_decoder(
            &operations,
            &BTreeMap::new(),
            &expected_state_text_placements(),
            BLACK,
            b"F0",
            decode_ascii,
        )
        .expect_err("an embedded but unused admitted font must not prove FontStack");

        assert!(
            error.contains("expected the admitted resource"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn state_text_proof_rejects_arbitrary_positioned_tj_text() {
        let mut operations = vec![set_gray(BLACK)];
        operations.extend(state_label_operations(
            b"F0",
            ["Wrong", "Labels", "Do", "NotMatch"],
        ));

        let error = assert_state_text_labels_with_decoder(
            &operations,
            &BTreeMap::new(),
            &expected_state_text_placements(),
            BLACK,
            b"F0",
            decode_ascii,
        )
        .expect_err("arbitrary Tj strings at State positions must not prove the labels");

        assert!(error.contains("must decode"), "unexpected error: {error}");
    }

    #[test]
    fn state_text_proof_rejects_wrong_current_fill_color() {
        let mut operations = vec![set_gray(BLACK), set_rgb(RED)];
        operations.extend(state_label_operations(b"F0", STATE_IDS));

        let error = assert_state_text_labels_with_decoder(
            &operations,
            &BTreeMap::new(),
            &expected_state_text_placements(),
            BLACK,
            b"F0",
            decode_ascii,
        )
        .expect_err("State labels must use the expected current non-stroking color");

        assert!(error.contains("Tj uses fill"), "unexpected error: {error}");
    }

    #[test]
    fn state_text_proof_rejects_labels_at_the_wrong_node_positions() {
        let mut label_operations = state_label_operations(b"F0", STATE_IDS);
        for operation in &mut label_operations {
            if operation.operator == "Tm" {
                operation.operands[4] = Object::Real(5.0);
            }
        }
        let mut operations = vec![set_gray(BLACK)];
        operations.extend(label_operations);

        let error = assert_state_text_labels_with_decoder(
            &operations,
            &BTreeMap::new(),
            &expected_state_text_placements(),
            BLACK,
            b"F0",
            decode_ascii,
        )
        .expect_err("correct labels at the wrong State positions must fail");

        assert!(error.contains("must decode"), "unexpected error: {error}");
    }

    #[test]
    fn state_placement_binding_ignores_do_order_but_preserves_identity() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let image_ids = image_ids();
        let operations = [
            Operation::new("q", vec![]),
            cm(20.0, 20.0, 60.0, 10.0),
            draw(b"blue"),
            Operation::new("Q", vec![]),
            Operation::new("q", vec![]),
            cm(20.0, 20.0, 10.0, 10.0),
            draw(b"red"),
            Operation::new("Q", vec![]),
        ];
        let placements = collect_drawn_page_images(&operations, &image_ids, &BTreeMap::new(), page)
            .expect("parse reversed but correctly placed image draws");
        let observed = observed_images(placements);

        validate_state_image_placements(&expected_placements(), &observed, page)
            .expect("spatial binding must not depend on PDF Do order");
    }

    #[test]
    fn state_placement_binding_rejects_swapped_matrices() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let image_ids = image_ids();
        let operations = [
            Operation::new("q", vec![]),
            cm(20.0, 20.0, 60.0, 10.0),
            draw(b"red"),
            Operation::new("Q", vec![]),
            Operation::new("q", vec![]),
            cm(20.0, 20.0, 10.0, 10.0),
            draw(b"blue"),
            Operation::new("Q", vec![]),
        ];
        let placements = collect_drawn_page_images(&operations, &image_ids, &BTreeMap::new(), page)
            .expect("parse swapped image draws");
        let error = validate_state_image_placements(
            &expected_placements(),
            &observed_images(placements),
            page,
        )
        .expect_err("swapping image matrices must break ordinal-to-geometry identity");

        assert!(error.contains("ordinal fill"), "unexpected error: {error}");
    }

    #[test]
    fn page_image_parser_rejects_off_page_placement() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let image_ids = BTreeMap::from([(b"red".to_vec(), (1, 0))]);
        let operations = [
            Operation::new("q", vec![]),
            cm(20.0, 20.0, 120.0, 120.0),
            draw(b"red"),
            Operation::new("Q", vec![]),
        ];
        let error = collect_drawn_page_images(&operations, &image_ids, &BTreeMap::new(), page)
            .expect_err("an off-page image must be rejected");

        assert!(error.contains("outside"), "unexpected error: {error}");
    }

    #[test]
    fn page_image_parser_rejects_zero_scale_placement() {
        let page = rect(0.0, 0.0, 100.0, 100.0);
        let image_ids = BTreeMap::from([(b"red".to_vec(), (1, 0))]);
        let operations = [
            Operation::new("q", vec![]),
            cm(0.0, 20.0, 10.0, 10.0),
            draw(b"red"),
            Operation::new("Q", vec![]),
        ];
        let error = collect_drawn_page_images(&operations, &image_ids, &BTreeMap::new(), page)
            .expect_err("a zero-scale image must be rejected");

        assert!(
            error.contains("positive non-zero scale"),
            "unexpected error: {error}"
        );
    }

    fn image_ids() -> BTreeMap<Vec<u8>, ObjectId> {
        BTreeMap::from([(b"red".to_vec(), (1, 0)), (b"blue".to_vec(), (2, 0))])
    }

    fn expected_placements() -> [ExpectedStatePlacement; 2] {
        [
            ExpectedStatePlacement {
                state_id: "Red",
                source_bbox: rect(12.0, 12.0, 16.0, 16.0),
                image_bbox: rect(10.0, 10.0, 20.0, 20.0),
                fill: RED,
                image_width: 20,
                image_height: 20,
            },
            ExpectedStatePlacement {
                state_id: "Blue",
                source_bbox: rect(62.0, 12.0, 16.0, 16.0),
                image_bbox: rect(60.0, 10.0, 20.0, 20.0),
                fill: BLUE,
                image_width: 20,
                image_height: 20,
            },
        ]
    }

    fn observed_images(placements: Vec<PlacedPdfImage>) -> Vec<PlacedStateImage> {
        placements
            .into_iter()
            .map(|placement| {
                let fill = match placement.image_id {
                    (1, 0) => RED,
                    (2, 0) => BLUE,
                    image_id => panic!("unexpected synthetic image id {image_id:?}"),
                };
                PlacedStateImage {
                    placement,
                    image: FilteredStateImage {
                        fill,
                        width: 20,
                        height: 20,
                        visible_bounds: PixelBounds {
                            min_x: 0,
                            min_y: 0,
                            max_x: 19,
                            max_y: 19,
                        },
                    },
                }
            })
            .collect()
    }

    fn cm(width: f32, height: f32, x: f32, y: f32) -> Operation {
        Operation::new(
            "cm",
            vec![
                Object::Real(width),
                Object::Integer(0),
                Object::Integer(0),
                Object::Real(height),
                Object::Real(x),
                Object::Real(y),
            ],
        )
    }

    fn draw(name: &[u8]) -> Operation {
        Operation::new("Do", vec![Object::Name(name.to_vec())])
    }

    fn set_rgb(color: Rgb) -> Operation {
        Operation::new(
            "rg",
            color.components().into_iter().map(Object::Real).collect(),
        )
    }

    fn set_gray(color: Rgb) -> Operation {
        assert_eq!(color.red, color.green);
        assert_eq!(color.green, color.blue);
        Operation::new("g", vec![Object::Real(color.components()[0])])
    }

    fn set_font(name: &[u8]) -> Operation {
        Operation::new("Tf", vec![Object::Name(name.to_vec()), Object::Real(12.0)])
    }

    fn text_matrix(x: f32, y: f32) -> Operation {
        Operation::new(
            "Tm",
            vec![
                Object::Integer(1),
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(1),
                Object::Real(x),
                Object::Real(y),
            ],
        )
    }

    fn state_label_operations<const N: usize>(font: &[u8], labels: [&str; N]) -> Vec<Operation> {
        labels
            .into_iter()
            .enumerate()
            .flat_map(|(index, label)| {
                [
                    Operation::new("BT", vec![]),
                    set_font(font),
                    text_matrix(5.0 + index as f32 * 25.0, 10.0),
                    Operation::new("Tj", vec![Object::string_literal(label)]),
                    Operation::new("ET", vec![]),
                ]
            })
            .collect()
    }

    fn expected_state_text_placements() -> [ExpectedStatePlacement; 4] {
        STATE_IDS.map(|state_id| {
            let index = STATE_IDS
                .iter()
                .position(|candidate| *candidate == state_id)
                .expect("known State id");
            ExpectedStatePlacement {
                state_id,
                source_bbox: rect(index as f64 * 25.0, 0.0, 20.0, 20.0),
                image_bbox: rect(index as f64 * 25.0, 0.0, 20.0, 20.0),
                fill: RED,
                image_width: 20,
                image_height: 20,
            }
        })
    }

    fn decode_ascii(bytes: &[u8]) -> Result<String, String> {
        String::from_utf8(bytes.to_vec()).map_err(|error| error.to_string())
    }

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect::from_xywh(x, y, width, height).expect("valid synthetic rectangle")
    }
}
