//! Workspace-only receipts for native C6a target observations.
//!
//! These receipts are deliberately feature-gated.  The acceptance crate receives only the
//! semantic predicates and region observations needed by its raster checks; it does not own an
//! SVG/XML/CSS interpreter for the production artifact.

use roxmltree::{Document, Node};
use sha2::{Digest as _, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartC6SvgReceipt {
    artifact_digest: [u8; 32],
    view_box_bits: [u64; 4],
    native_text: bool,
    has_foreign_object: bool,
    has_prepared_tokens: bool,
    canvas: Option<FlowchartC6CanvasReceipt>,
    nodes: Box<[FlowchartC6NodeReceipt]>,
    digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartC6NodeReceipt {
    id: Box<str>,
    fill: Option<Box<str>>,
    stroke: Box<str>,
    stroke_width_bits: u64,
    radius_x_bits: u64,
    radius_y_bits: u64,
    dasharray: Box<str>,
    font_family: Box<str>,
    region_bits: [u64; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartC6CanvasReceipt {
    gradient_units: Box<str>,
    spread_method: Box<str>,
    x1_bits: u64,
    y1_bits: u64,
    x2_bits: u64,
    y2_bits: u64,
    stops: Box<[FlowchartC6GradientStopReceipt]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartC6GradientStopReceipt {
    offset_bits: u64,
    color: Box<str>,
}

impl FlowchartC6SvgReceipt {
    /// Observes the finalized public SVG and binds the observation to its exact artifact digest.
    pub fn observe_svg(svg: &str, artifact_digest: [u8; 32]) -> Option<Self> {
        if artifact_digest == [0; 32]
            || Sha256::digest(svg.as_bytes()).as_slice() != artifact_digest
        {
            return None;
        }
        let document = Document::parse(svg).ok()?;
        let root = document.root_element();
        let view_box = parse_view_box(root.attribute("viewBox")?)?;
        let wrappers = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-et") == Some("node")
                    && node.attribute("data-id").is_some()
            })
            .collect::<Vec<_>>();
        if wrappers.is_empty() {
            return None;
        }
        let mut nodes = Vec::with_capacity(wrappers.len());
        for wrapper in wrappers {
            let id = wrapper.attribute("data-id")?;
            let rects = wrapper
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && class_contains(*node, "basic")
                        && class_contains(*node, "label-container")
                })
                .collect::<Vec<_>>();
            if rects.len() != 1 {
                return None;
            }
            let rect = rects[0];
            let fill = style_or_attribute_fill(rect)
                .map(str::to_owned)
                .map(String::into_boxed_str);
            let stroke = style_value(rect, "stroke")?.to_owned().into_boxed_str();
            let stroke_width = style_value(rect, "stroke-width")?
                .trim()
                .strip_suffix("px")
                .unwrap_or(style_value(rect, "stroke-width")?.trim())
                .parse::<f64>()
                .ok()?;
            let radius_x = rect.attribute("rx")?.parse::<f64>().ok()?;
            let radius_y = rect.attribute("ry")?.parse::<f64>().ok()?;
            let region = transformed_rect(rect)?;
            if ![stroke_width, radius_x, radius_y]
                .into_iter()
                .all(f64::is_finite)
            {
                return None;
            }
            let dasharray = style_value(rect, "stroke-dasharray")
                .unwrap_or_default()
                .to_owned()
                .into_boxed_str();
            let font_family = wrapper
                .descendants()
                .find_map(|node| style_value(node, "font-family"))
                .unwrap_or_default()
                .to_owned()
                .into_boxed_str();
            nodes.push(FlowchartC6NodeReceipt {
                id: id.to_owned().into_boxed_str(),
                fill,
                stroke,
                stroke_width_bits: stroke_width.to_bits(),
                radius_x_bits: radius_x.to_bits(),
                radius_y_bits: radius_y.to_bits(),
                dasharray,
                font_family,
                region_bits: region.map(f64::to_bits),
            });
        }
        let mut receipt = Self {
            artifact_digest,
            view_box_bits: view_box.map(f64::to_bits),
            native_text: document.descendants().any(|node| node.has_tag_name("text")),
            has_foreign_object: document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            has_prepared_tokens: svg.contains("merman-prepared-"),
            canvas: observe_canvas(&document),
            nodes: nodes.into_boxed_slice(),
            digest: [0; 32],
        };
        receipt.digest = receipt.canonical_digest();
        Some(receipt)
    }

    pub const fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub fn proves_artifact(&self, artifact_digest: [u8; 32]) -> bool {
        artifact_digest != [0; 32]
            && self.artifact_digest == artifact_digest
            && self.digest == self.canonical_digest()
    }

    pub fn view_box(&self) -> [f64; 4] {
        self.view_box_bits.map(f64::from_bits)
    }

    pub fn node_regions(&self) -> &[FlowchartC6NodeReceipt] {
        &self.nodes
    }

    /// Proves the bounded Brutalist Flowchart SVG contract without exposing the parsed DOM.
    pub fn proves_flowchart_node_contract(
        &self,
        node_ids: &[&str],
        palette: &[&str],
        border: &str,
        stroke_width: f64,
        radius: f64,
    ) -> bool {
        node_ids.len() == 6
            && palette.len() == 3
            && self.native_text
            && !self.has_foreign_object
            && !self.has_prepared_tokens
            && self.nodes.len() == 6
            && self.nodes.iter().enumerate().all(|(ordinal, node)| {
                node.id.as_ref() == node_ids[ordinal]
                    && node.fill.as_deref() == Some(palette[ordinal % palette.len()])
                    && node.stroke.as_ref() == border
                    && approx_eq(node.stroke_width(), stroke_width)
                    && approx_eq(node.radius_x(), radius)
                    && approx_eq(node.radius_y(), radius)
            })
    }

    pub fn proves_spotless_flowchart_contract(
        &self,
        node_ids: &[&str],
        stripe_colors: &[&str],
        stop_offsets: &[f64],
        period_px: f64,
        stroke: &str,
        stroke_width: f64,
        dasharray: &str,
        radius: f64,
        font_family: &str,
    ) -> bool {
        node_ids.len() == 4
            && stripe_colors.len() == 2
            && stop_offsets.len() == 4
            && self.native_text
            && !self.has_foreign_object
            && !self.has_prepared_tokens
            && self.nodes.len() == 4
            && self.canvas.as_ref().is_some_and(|canvas| {
                canvas.matches_repeating_gradient(stripe_colors, stop_offsets, period_px)
            })
            && self.nodes.iter().enumerate().all(|(ordinal, node)| {
                node.id.as_ref() == node_ids[ordinal]
                    && node.stroke.as_ref() == stroke
                    && approx_eq(node.stroke_width(), stroke_width)
                    && node.dasharray.as_ref() == dasharray
                    && approx_eq(node.radius_x(), radius)
                    && approx_eq(node.radius_y(), radius)
                    && node.font_family.contains(font_family)
            })
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"merman.flowchart-c6-svg-receipt.v2\0");
        hasher.update(self.artifact_digest);
        for value in self.view_box_bits {
            hasher.update(value.to_be_bytes());
        }
        hasher.update([
            u8::from(self.native_text),
            u8::from(self.has_foreign_object),
            u8::from(self.has_prepared_tokens),
        ]);
        hasher.update((self.nodes.len() as u64).to_be_bytes());
        for node in self.nodes.iter() {
            update_len_prefixed(&mut hasher, node.id.as_bytes());
            match &node.fill {
                Some(fill) => {
                    hasher.update([1]);
                    update_len_prefixed(&mut hasher, fill.as_bytes());
                }
                None => hasher.update([0]),
            }
            update_len_prefixed(&mut hasher, node.stroke.as_bytes());
            hasher.update(node.stroke_width_bits.to_be_bytes());
            hasher.update(node.radius_x_bits.to_be_bytes());
            hasher.update(node.radius_y_bits.to_be_bytes());
            update_len_prefixed(&mut hasher, node.dasharray.as_bytes());
            update_len_prefixed(&mut hasher, node.font_family.as_bytes());
            for value in node.region_bits {
                hasher.update(value.to_be_bytes());
            }
        }
        match &self.canvas {
            Some(canvas) => {
                hasher.update([1]);
                canvas.update_digest(&mut hasher);
            }
            None => hasher.update([0]),
        }
        hasher.finalize().into()
    }
}

impl FlowchartC6NodeReceipt {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn region(&self) -> [f64; 4] {
        self.region_bits.map(f64::from_bits)
    }

    fn stroke_width(&self) -> f64 {
        f64::from_bits(self.stroke_width_bits)
    }

    fn radius_x(&self) -> f64 {
        f64::from_bits(self.radius_x_bits)
    }

    fn radius_y(&self) -> f64 {
        f64::from_bits(self.radius_y_bits)
    }
}

impl FlowchartC6CanvasReceipt {
    fn matches_repeating_gradient(
        &self,
        stripe_colors: &[&str],
        stop_offsets: &[f64],
        period_px: f64,
    ) -> bool {
        if stripe_colors.len() != 2 || stop_offsets.len() != 4 {
            return false;
        }
        let x1 = f64::from_bits(self.x1_bits);
        let y1 = f64::from_bits(self.y1_bits);
        let x2 = f64::from_bits(self.x2_bits);
        let y2 = f64::from_bits(self.y2_bits);
        let expected_colors = [
            stripe_colors[0],
            stripe_colors[0],
            stripe_colors[1],
            stripe_colors[1],
        ];
        let vector_x = x2 - x1;
        let vector_y = y2 - y1;
        self.gradient_units.as_ref() == "userSpaceOnUse"
            && self.spread_method.as_ref() == "repeat"
            && approx_eq(x1, 0.0)
            && approx_eq(y1, 0.0)
            && vector_x > 0.0
            && vector_y < 0.0
            && approx_eq(vector_x, -vector_y)
            && period_px.is_finite()
            && period_px > 0.0
            && approx_eq(vector_x.hypot(vector_y), period_px)
            && self.stops.len() == stop_offsets.len()
            && self
                .stops
                .iter()
                .zip(stop_offsets.iter().zip(expected_colors))
                .all(|(stop, (offset, color))| {
                    approx_eq(stop.offset(), *offset) && stop.color.as_ref() == color
                })
    }

    fn update_digest(&self, hasher: &mut Sha256) {
        update_len_prefixed(hasher, self.gradient_units.as_bytes());
        update_len_prefixed(hasher, self.spread_method.as_bytes());
        for value in [self.x1_bits, self.y1_bits, self.x2_bits, self.y2_bits] {
            hasher.update(value.to_be_bytes());
        }
        hasher.update((self.stops.len() as u64).to_be_bytes());
        for stop in self.stops.iter() {
            hasher.update(stop.offset_bits.to_be_bytes());
            update_len_prefixed(hasher, stop.color.as_bytes());
        }
    }
}

impl FlowchartC6GradientStopReceipt {
    fn offset(&self) -> f64 {
        f64::from_bits(self.offset_bits)
    }
}

fn update_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn parse_view_box(raw: &str) -> Option<[f64; 4]> {
    let values = raw
        .split_ascii_whitespace()
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let [left, top, width, height] = values.as_slice() else {
        return None;
    };
    if !values.iter().all(|value| value.is_finite()) || *width <= 0.0 || *height <= 0.0 {
        return None;
    }
    Some([*left, *top, *width, *height])
}

fn observe_canvas(document: &Document<'_>) -> Option<FlowchartC6CanvasReceipt> {
    let bases = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect") && node.attribute("data-merman-theme-canvas") == Some("base")
        })
        .collect::<Vec<_>>();
    if bases.len() != 1
        || document
            .descendants()
            .any(|node| node.attribute("data-merman-theme-canvas-layer").is_some())
    {
        return None;
    }
    let base = bases[0];
    let gradient_id = style_or_attribute_fill(base).and_then(local_url_id)?;
    let gradients = document
        .descendants()
        .filter(|node| node.attribute("id") == Some(gradient_id))
        .collect::<Vec<_>>();
    if gradients.len() != 1 || !gradients[0].has_tag_name("linearGradient") {
        return None;
    }
    let gradient = gradients[0];
    let x1 = numeric_attribute(gradient, "x1")?;
    let y1 = numeric_attribute(gradient, "y1")?;
    let x2 = numeric_attribute(gradient, "x2")?;
    let y2 = numeric_attribute(gradient, "y2")?;
    let elements = gradient
        .children()
        .filter(|node| node.is_element())
        .collect::<Vec<_>>();
    if elements.is_empty() || elements.iter().any(|node| !node.has_tag_name("stop")) {
        return None;
    }
    let stops = elements
        .into_iter()
        .map(|stop| {
            Some(FlowchartC6GradientStopReceipt {
                offset_bits: percent_value(stop.attribute("offset")?)?.to_bits(),
                color: stop.attribute("stop-color")?.to_owned().into_boxed_str(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(FlowchartC6CanvasReceipt {
        gradient_units: gradient
            .attribute("gradientUnits")
            .unwrap_or_default()
            .to_owned()
            .into_boxed_str(),
        spread_method: gradient
            .attribute("spreadMethod")
            .unwrap_or_default()
            .to_owned()
            .into_boxed_str(),
        x1_bits: x1.to_bits(),
        y1_bits: y1.to_bits(),
        x2_bits: x2.to_bits(),
        y2_bits: y2.to_bits(),
        stops: stops.into_boxed_slice(),
    })
}

fn style_or_attribute_fill<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    style_value(node, "fill").or_else(|| node.attribute("fill"))
}

fn local_url_id(value: &str) -> Option<&str> {
    value
        .strip_prefix("url(#")
        .and_then(|value| value.strip_suffix(')'))
}

fn percent_value(value: &str) -> Option<f64> {
    value
        .strip_suffix('%')?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn numeric_attribute(node: Node<'_, '_>, name: &str) -> Option<f64> {
    node.attribute(name)?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn class_contains(node: Node<'_, '_>, class_name: &str) -> bool {
    node.attribute("class").is_some_and(|classes| {
        classes
            .split_ascii_whitespace()
            .any(|class| class == class_name)
    })
}

fn style_value<'a>(node: Node<'a, '_>, property: &str) -> Option<&'a str> {
    node.attribute("style")?.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        (name.trim() == property).then(|| {
            value
                .trim()
                .strip_suffix("!important")
                .unwrap_or(value.trim())
                .trim()
        })
    })
}

fn transformed_rect(node: Node<'_, '_>) -> Option<[f64; 4]> {
    let x = node.attribute("x")?.parse::<f64>().ok()?;
    let y = node.attribute("y")?.parse::<f64>().ok()?;
    let width = node.attribute("width")?.parse::<f64>().ok()?;
    let height = node.attribute("height")?.parse::<f64>().ok()?;
    if ![x, y, width, height].into_iter().all(f64::is_finite) || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let mut transform = AffineTransform::IDENTITY;
    let ancestors = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in ancestors.into_iter().rev() {
        let parsed = raw.parse::<svgtypes::Transform>().ok()?;
        transform = transform.multiply(AffineTransform::from_svg(parsed));
    }
    if transform.b.abs() > 1e-9
        || transform.c.abs() > 1e-9
        || transform.a <= 0.0
        || transform.d <= 0.0
    {
        return None;
    }
    let (left, top) = transform.apply(x, y);
    let (right, bottom) = transform.apply(x + width, y + height);
    let region = [left, top, right - left, bottom - top];
    region.into_iter().all(f64::is_finite).then_some(region)
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

fn approx_eq(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn sample_svg(fill: [&str; 3]) -> String {
        let mut svg = String::from(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 40"><text>x</text>"#,
        );
        for (index, id) in ["A", "B", "C", "D", "E", "F"].into_iter().enumerate() {
            writeln!(
                svg,
                r#"<g data-id="{id}" data-et="node"><rect class="basic label-container" x="{}" y="5" width="30" height="20" rx="5" ry="5" style="fill:{};stroke:#111;stroke-width:3px"/></g>"#,
                index * 35,
                fill[index % fill.len()]
            )
            .expect("write sample SVG");
        }
        svg.push_str("</svg>");
        svg
    }

    #[test]
    fn receipt_binds_exact_svg_and_flowchart_contract() {
        let svg = sample_svg(["#a", "#b", "#c"]);
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        let receipt = FlowchartC6SvgReceipt::observe_svg(&svg, digest).expect("seal receipt");

        assert!(receipt.proves_artifact(digest));
        assert!(receipt.proves_flowchart_node_contract(
            &["A", "B", "C", "D", "E", "F"],
            &["#a", "#b", "#c"],
            "#111",
            3.0,
            5.0,
        ));
        assert_eq!(receipt.node_regions().len(), 6);
    }

    #[test]
    fn receipt_rejects_forged_digest_or_changed_style() {
        let svg = sample_svg(["#a", "#b", "#c"]);
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        assert!(FlowchartC6SvgReceipt::observe_svg(&svg, [1; 32]).is_none());

        let changed = sample_svg(["#a", "#b", "#d"]);
        let changed_digest: [u8; 32] = Sha256::digest(changed.as_bytes()).into();
        let receipt =
            FlowchartC6SvgReceipt::observe_svg(&changed, changed_digest).expect("seal changed");
        assert!(!receipt.proves_flowchart_node_contract(
            &["A", "B", "C", "D", "E", "F"],
            &["#a", "#b", "#c"],
            "#111",
            3.0,
            5.0,
        ));
        assert_ne!(receipt.digest(), digest);
    }

    #[test]
    fn receipt_proves_spotless_flowchart_gradient_and_terminal_nodes() {
        let svg = sample_spotless_svg();
        assert_spotless_contract(&spotless_receipt(&svg));
    }

    #[test]
    fn receipt_rejects_mutated_spotless_gradient_or_terminal_style() {
        let valid = sample_spotless_svg();
        let mutations = [
            valid.replace("offset=\"50%\"", "offset=\"0%\""),
            valid.replace("url(#stripe)", "url(#missing-gradient)"),
            valid.replace("x2=\"14.1421356237\"", "x2=\"10\""),
            valid.replace("stroke-dasharray:6 4", "stroke-dasharray:4 2"),
            valid.replace("font-family:Excalifont", "font-family:Arial"),
        ];

        for mutated in mutations {
            assert!(
                !spotless_receipt(&mutated).proves_spotless_flowchart_contract(
                    &["A", "B", "C", "D"],
                    &["#e7e2d6", "#d2ccc0"],
                    &[0.0, 50.0, 51.0, 100.0],
                    20.0,
                    "#2c2416",
                    2.0,
                    "6 4",
                    4.0,
                    "Excalifont",
                ),
                "mutation unexpectedly retained the Spotless contract: {mutated}"
            );
        }
    }

    #[test]
    fn receipt_rejects_ambiguous_spotless_canvas_structure() {
        let valid = sample_spotless_svg();
        let mutations = [
            valid.replace(
                "</svg>",
                "<rect data-merman-theme-canvas=\"base\" fill=\"url(#stripe)\"/></svg>",
            ),
            valid.replace("</svg>", "<g data-merman-theme-canvas-layer=\"0\"/></svg>"),
            valid.replace("</linearGradient>", "<g/></linearGradient>"),
        ];

        for mutated in mutations {
            let receipt = spotless_receipt(&mutated);
            assert!(
                !receipt.proves_spotless_flowchart_contract(
                    &["A", "B", "C", "D"],
                    &["#e7e2d6", "#d2ccc0"],
                    &[0.0, 50.0, 51.0, 100.0],
                    20.0,
                    "#2c2416",
                    2.0,
                    "6 4",
                    4.0,
                    "Excalifont",
                ),
                "ambiguous canvas unexpectedly retained the Spotless contract: {mutated}"
            );
        }
    }

    fn spotless_receipt(svg: &str) -> FlowchartC6SvgReceipt {
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        FlowchartC6SvgReceipt::observe_svg(svg, digest).expect("seal receipt")
    }

    fn assert_spotless_contract(receipt: &FlowchartC6SvgReceipt) {
        assert!(receipt.proves_spotless_flowchart_contract(
            &["A", "B", "C", "D"],
            &["#e7e2d6", "#d2ccc0"],
            &[0.0, 50.0, 51.0, 100.0],
            20.0,
            "#2c2416",
            2.0,
            "6 4",
            4.0,
            "Excalifont",
        ));
    }

    fn sample_spotless_svg() -> String {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 180 80">
<defs><linearGradient id="stripe" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="14.1421356237" y2="-14.1421356237" spreadMethod="repeat"><stop offset="0%" stop-color="#e7e2d6"/><stop offset="50%" stop-color="#e7e2d6"/><stop offset="51%" stop-color="#d2ccc0"/><stop offset="100%" stop-color="#d2ccc0"/></linearGradient></defs>
<rect data-merman-theme-canvas="base" fill="url(#stripe)" x="0" y="0" width="180" height="80"/>
<text x="0" y="10">native</text>
<g data-id="A" data-et="node"><rect class="basic label-container" x="10" y="25" width="30" height="20" rx="4" ry="4" style="fill:#fff;stroke:#2c2416;stroke-width:2px;stroke-dasharray:6 4"/><text style="font-family:Excalifont">A</text></g>
<g data-id="B" data-et="node"><rect class="basic label-container" x="55" y="25" width="30" height="20" rx="4" ry="4" style="fill:#fff;stroke:#2c2416;stroke-width:2px;stroke-dasharray:6 4"/><text style="font-family:Excalifont">B</text></g>
<g data-id="C" data-et="node"><rect class="basic label-container" x="100" y="25" width="30" height="20" rx="4" ry="4" style="fill:#fff;stroke:#2c2416;stroke-width:2px;stroke-dasharray:6 4"/><text style="font-family:Excalifont">C</text></g>
<g data-id="D" data-et="node"><rect class="basic label-container" x="145" y="25" width="30" height="20" rx="4" ry="4" style="fill:#fff;stroke:#2c2416;stroke-width:2px;stroke-dasharray:6 4"/><text style="font-family:Excalifont">D</text></g>
</svg>"##.to_owned()
    }
}
