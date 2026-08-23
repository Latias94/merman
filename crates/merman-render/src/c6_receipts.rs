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
    nodes: Box<[FlowchartC6NodeReceipt]>,
    digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartC6NodeReceipt {
    id: Box<str>,
    fill: Box<str>,
    stroke: Box<str>,
    stroke_width_bits: u64,
    radius_x_bits: u64,
    radius_y_bits: u64,
    region_bits: [u64; 4],
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
            let fill = style_value(rect, "fill")?.to_owned().into_boxed_str();
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
            nodes.push(FlowchartC6NodeReceipt {
                id: id.to_owned().into_boxed_str(),
                fill,
                stroke,
                stroke_width_bits: stroke_width.to_bits(),
                radius_x_bits: radius_x.to_bits(),
                radius_y_bits: radius_y.to_bits(),
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
                    && node.fill.as_ref() == palette[ordinal % palette.len()]
                    && node.stroke.as_ref() == border
                    && approx_eq(node.stroke_width(), stroke_width)
                    && approx_eq(node.radius_x(), radius)
                    && approx_eq(node.radius_y(), radius)
            })
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"merman.flowchart-c6-svg-receipt.v1\0");
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
            update_len_prefixed(&mut hasher, node.fill.as_bytes());
            update_len_prefixed(&mut hasher, node.stroke.as_bytes());
            hasher.update(node.stroke_width_bits.to_be_bytes());
            hasher.update(node.radius_x_bits.to_be_bytes());
            hasher.update(node.radius_y_bits.to_be_bytes());
            for value in node.region_bits {
                hasher.update(value.to_be_bytes());
            }
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
}
