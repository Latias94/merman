//! Fixture observations owned by the private qualification runner.
//!
//! Production output carries no acceptance sidecar. The runner observes the exact immutable SVG
//! it is checking; these bounded scenario helpers are not a general SVG/CSS validator.

use roxmltree::{Document, Node};

const MAX_OBSERVATION_TREE_DEPTH: usize = 256;

/// Family-neutral facts used only by the private scenario checkers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgArtifactObservation {
    root_id: Option<Box<str>>,
    view_box_bits: [u64; 4],
    native_text: bool,
    has_foreign_object: bool,
    elements: Box<[SvgElementObservation]>,
    stylesheets: Box<[SvgStylesheetObservation]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgElementObservation {
    index: usize,
    tag: Box<str>,
    id: Option<Box<str>>,
    classes: Box<[Box<str>]>,
    attributes: Box<[SvgAttributeObservation]>,
    inline_styles: Box<[SvgStyleDeclarationObservation]>,
    text: Box<str>,
    parent_index: Option<usize>,
    bounds_bits: Option<[u64; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgAttributeObservation {
    name: Box<str>,
    value: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgStyleDeclarationObservation {
    property: Box<str>,
    value: Box<str>,
    important: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgStylesheetObservation {
    typed_font_marker: bool,
    parse_valid: bool,
    rules: Box<[SvgStyleRuleObservation]>,
    font_faces: Box<[SvgFontFaceObservation]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgStyleRuleObservation {
    selector: Box<str>,
    declarations: Box<[SvgStyleDeclarationObservation]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgFontFaceObservation {
    declarations: Box<[SvgStyleDeclarationObservation]>,
}

impl SvgArtifactObservation {
    /// Observes the bytes supplied by the qualification runner or a scenario mutation test.
    pub fn observe(svg: &str) -> Option<Self> {
        let max_tree_depth = svg_max_tree_depth(svg)?;
        if max_tree_depth > MAX_OBSERVATION_TREE_DEPTH {
            return None;
        }
        let document = Document::parse(svg).ok()?;
        let root = document.root_element();
        let view_box = parse_view_box(root.attribute("viewBox")?)?;
        let document_nodes = document
            .descendants()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        let index_by_start = document_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.range().start, index))
            .collect::<std::collections::BTreeMap<_, _>>();
        let elements = document_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| observe_element(index, *node, &index_by_start))
            .collect::<Option<Vec<_>>>()?;
        let stylesheets = document
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .map(observe_stylesheet)
            .collect::<Vec<_>>();
        Some(Self {
            root_id: root
                .attribute("id")
                .map(str::to_owned)
                .map(String::into_boxed_str),
            view_box_bits: view_box.map(f64::to_bits),
            native_text: document.descendants().any(|node| node.has_tag_name("text")),
            has_foreign_object: document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            elements: elements.into_boxed_slice(),
            stylesheets: stylesheets.into_boxed_slice(),
        })
    }

    pub fn root_id(&self) -> Option<&str> {
        self.root_id.as_deref()
    }

    pub fn view_box(&self) -> [f64; 4] {
        self.view_box_bits.map(f64::from_bits)
    }

    pub const fn has_native_text(&self) -> bool {
        self.native_text
    }

    pub const fn has_foreign_object(&self) -> bool {
        self.has_foreign_object
    }

    pub fn elements(&self) -> &[SvgElementObservation] {
        &self.elements
    }

    pub fn element(&self, index: usize) -> Option<&SvgElementObservation> {
        self.elements.get(index)
    }

    pub fn root_element(&self) -> Option<&SvgElementObservation> {
        self.elements.first()
    }

    pub fn is_descendant_of(&self, element_index: usize, ancestor_index: usize) -> bool {
        let mut current = self
            .elements
            .get(element_index)
            .and_then(SvgElementObservation::parent_index);
        while let Some(index) = current {
            if index == ancestor_index {
                return true;
            }
            current = self
                .elements
                .get(index)
                .and_then(SvgElementObservation::parent_index);
        }
        false
    }

    pub fn descendants_of(
        &self,
        ancestor_index: usize,
    ) -> impl Iterator<Item = &SvgElementObservation> {
        self.elements
            .iter()
            .filter(move |element| self.is_descendant_of(element.index(), ancestor_index))
    }

    pub fn stylesheets(&self) -> &[SvgStylesheetObservation] {
        &self.stylesheets
    }
}

/// Performs the bounded, iterative XML depth check before handing the document to `roxmltree`.
///
/// `roxmltree` is useful for the fixture observation, but its recursive tree construction can
/// exhaust the default test/thread stack on an otherwise well-formed SVG that is already beyond
/// the runner's observation depth budget. Keep this preflight iterative and fail closed before
/// invoking the recursive parser.
fn svg_max_tree_depth(svg: &str) -> Option<usize> {
    let mut reader = quick_xml::Reader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut depth = 0usize;
    let mut max_tree_depth = 0usize;

    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(_)) => {
                depth = depth.checked_add(1)?;
                max_tree_depth = max_tree_depth.max(depth.saturating_sub(1));
            }
            Ok(quick_xml::events::Event::End(_)) => {
                depth = depth.checked_sub(1)?;
            }
            Ok(quick_xml::events::Event::Eof) => {
                return (depth == 0).then_some(max_tree_depth);
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

impl SvgElementObservation {
    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn tag_name(&self) -> &str {
        &self.tag
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn classes(&self) -> &[Box<str>] {
        &self.classes
    }

    pub fn has_class(&self, class_name: &str) -> bool {
        self.classes
            .iter()
            .any(|class| class.as_ref() == class_name)
    }

    pub fn attributes(&self) -> &[SvgAttributeObservation] {
        &self.attributes
    }

    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|attribute| attribute.name.as_ref() == name)
            .map(|attribute| attribute.value.as_ref())
    }

    pub fn numeric_attribute(&self, name: &str) -> Option<f64> {
        self.attribute(name)
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
    }

    pub fn inline_styles(&self) -> &[SvgStyleDeclarationObservation] {
        &self.inline_styles
    }

    pub fn style_value(&self, property: &str) -> Option<&str> {
        self.inline_styles
            .iter()
            .rev()
            .find(|declaration| declaration.property.as_ref() == property)
            .map(|declaration| declaration.value.as_ref())
    }

    pub fn style_is_important(&self, property: &str) -> bool {
        self.inline_styles
            .iter()
            .rev()
            .find(|declaration| declaration.property.as_ref() == property)
            .is_some_and(|declaration| declaration.important)
    }

    pub const fn text(&self) -> &str {
        &self.text
    }

    pub const fn parent_index(&self) -> Option<usize> {
        self.parent_index
    }

    pub fn bounds(&self) -> Option<[f64; 4]> {
        self.bounds_bits.map(|bounds| bounds.map(f64::from_bits))
    }
}

impl SvgAttributeObservation {
    pub const fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &str {
        &self.value
    }
}

impl SvgStyleDeclarationObservation {
    pub const fn property(&self) -> &str {
        &self.property
    }

    pub const fn value(&self) -> &str {
        &self.value
    }

    pub const fn important(&self) -> bool {
        self.important
    }
}

impl SvgStylesheetObservation {
    pub const fn typed_font_marker(&self) -> bool {
        self.typed_font_marker
    }

    pub const fn parse_valid(&self) -> bool {
        self.parse_valid
    }

    pub fn rules(&self) -> &[SvgStyleRuleObservation] {
        &self.rules
    }

    pub fn font_faces(&self) -> &[SvgFontFaceObservation] {
        &self.font_faces
    }
}

impl SvgStyleRuleObservation {
    pub const fn selector(&self) -> &str {
        &self.selector
    }

    pub fn declarations(&self) -> &[SvgStyleDeclarationObservation] {
        &self.declarations
    }

    pub fn declaration(&self, property: &str) -> Option<&SvgStyleDeclarationObservation> {
        self.declarations
            .iter()
            .rev()
            .find(|declaration| declaration.property.as_ref() == property)
    }
}

impl SvgFontFaceObservation {
    pub fn declarations(&self) -> &[SvgStyleDeclarationObservation] {
        &self.declarations
    }
}

fn observe_element(
    index: usize,
    node: Node<'_, '_>,
    index_by_start: &std::collections::BTreeMap<usize, usize>,
) -> Option<SvgElementObservation> {
    let parent_index = node
        .parent()
        .filter(|parent| parent.is_element())
        .and_then(|parent| index_by_start.get(&parent.range().start).copied());
    let attributes = node
        .attributes()
        .map(|attribute| SvgAttributeObservation {
            name: attribute.name().to_owned().into_boxed_str(),
            value: attribute.value().to_owned().into_boxed_str(),
        })
        .collect::<Vec<_>>();
    let classes = node
        .attribute("class")
        .unwrap_or_default()
        .split_ascii_whitespace()
        .filter(|class| !class.is_empty())
        .map(str::to_owned)
        .map(String::into_boxed_str)
        .collect::<Vec<_>>();
    let inline_styles = node
        .attribute("style")
        .map(parse_style_declarations)
        .unwrap_or_default();
    let text = node
        .descendants()
        .filter(|descendant| descendant.is_text())
        .filter_map(|descendant| descendant.text())
        .collect::<String>()
        .trim()
        .to_owned()
        .into_boxed_str();
    let bounds_bits = observe_geometry_bounds(node).map(|bounds| bounds.map(f64::to_bits));
    Some(SvgElementObservation {
        index,
        tag: node.tag_name().name().to_owned().into_boxed_str(),
        id: node
            .attribute("id")
            .map(str::to_owned)
            .map(String::into_boxed_str),
        classes: classes.into_boxed_slice(),
        attributes: attributes.into_boxed_slice(),
        inline_styles: inline_styles.into_boxed_slice(),
        text,
        parent_index,
        bounds_bits,
    })
}

fn observe_stylesheet(node: Node<'_, '_>) -> SvgStylesheetObservation {
    let css = node.text().unwrap_or_default();
    let (parse_valid, rules, font_faces) = parse_stylesheet(css);
    SvgStylesheetObservation {
        typed_font_marker: node.attribute("data-merman-typed-fonts") == Some("v1"),
        parse_valid,
        rules: rules.into_boxed_slice(),
        font_faces: font_faces.into_boxed_slice(),
    }
}

fn parse_style_declarations(raw: &str) -> Vec<SvgStyleDeclarationObservation> {
    split_css_statements(raw)
        .into_iter()
        .filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let property = property.trim();
            let mut value = value.trim();
            if property.is_empty() || value.is_empty() {
                return None;
            }
            let important = value
                .strip_suffix("!important")
                .map(|trimmed| {
                    value = trimmed.trim_end();
                    true
                })
                .unwrap_or(false);
            Some(SvgStyleDeclarationObservation {
                property: property.to_owned().into_boxed_str(),
                value: value.to_owned().into_boxed_str(),
                important,
            })
        })
        .collect()
}

fn parse_stylesheet(
    css: &str,
) -> (
    bool,
    Vec<SvgStyleRuleObservation>,
    Vec<SvgFontFaceObservation>,
) {
    let mut rules = Vec::new();
    let mut font_faces = Vec::new();
    let mut parse_valid = true;
    let mut cursor = 0usize;
    while let Some(open_offset) = find_css_delimiter(css, cursor, b'{') {
        let prelude = css[cursor..open_offset].trim();
        let Some(close_offset) = matching_css_brace(css, open_offset) else {
            parse_valid = false;
            break;
        };
        let body = &css[open_offset + 1..close_offset];
        if prelude.starts_with("@font-face") {
            font_faces.push(SvgFontFaceObservation {
                declarations: parse_style_declarations(body).into_boxed_slice(),
            });
        } else if !prelude.starts_with('@') && !prelude.is_empty() {
            rules.push(SvgStyleRuleObservation {
                selector: prelude.to_owned().into_boxed_str(),
                declarations: parse_style_declarations(body).into_boxed_slice(),
            });
        }
        cursor = close_offset + 1;
    }
    if css[cursor..]
        .chars()
        .any(|character| !character.is_whitespace())
    {
        parse_valid = false;
    }
    (parse_valid, rules, font_faces)
}

fn split_css_statements(raw: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (offset, character) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '(' | '[' => depth = depth.saturating_add(1),
            ')' | ']' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                statements.push(&raw[start..offset]);
                start = offset + character.len_utf8();
            }
            _ => {}
        }
    }
    statements.push(&raw[start..]);
    statements
}

fn find_css_delimiter(raw: &str, start: usize, delimiter: u8) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut index = start;
    let mut quote = None;
    let mut comment = false;
    while index < bytes.len() {
        if comment {
            if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            comment = true;
            index += 2;
            continue;
        }
        if let Some(active_quote) = quote {
            if bytes[index] == active_quote && (index == 0 || bytes[index - 1] != b'\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        if bytes[index] == b'\'' || bytes[index] == b'"' {
            quote = Some(bytes[index]);
        } else if bytes[index] == delimiter {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn matching_css_brace(raw: &str, open_offset: usize) -> Option<usize> {
    let bytes = raw.as_bytes();
    let mut depth = 0usize;
    let mut index = open_offset;
    let mut quote = None;
    let mut comment = false;
    while index < bytes.len() {
        if comment {
            if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            comment = true;
            index += 2;
            continue;
        }
        if let Some(active_quote) = quote {
            if bytes[index] == active_quote && (index == 0 || bytes[index - 1] != b'\\') {
                quote = None;
            }
            index += 1;
            continue;
        }
        match bytes[index] {
            b'\'' | b'"' => quote = Some(bytes[index]),
            b'{' => depth = depth.saturating_add(1),
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn observe_geometry_bounds(node: Node<'_, '_>) -> Option<[f64; 4]> {
    match node.tag_name().name() {
        "rect" => transformed_rect(node),
        "line" => transformed_line(node),
        "circle" => transformed_circle(node, false),
        "ellipse" => transformed_circle(node, true),
        _ => None,
    }
}

fn transformed_line(node: Node<'_, '_>) -> Option<[f64; 4]> {
    let x1 = numeric_attribute(node, "x1")?;
    let y1 = numeric_attribute(node, "y1")?;
    let x2 = numeric_attribute(node, "x2")?;
    let y2 = numeric_attribute(node, "y2")?;
    let transform = accumulated_transform(node)?;
    let (x1, y1) = transform.apply(x1, y1);
    let (x2, y2) = transform.apply(x2, y2);
    let left = x1.min(x2);
    let top = y1.min(y2);
    let right = x1.max(x2);
    let bottom = y1.max(y2);
    [left, top, right - left, bottom - top]
        .into_iter()
        .all(f64::is_finite)
        .then_some([left, top, right - left, bottom - top])
}

fn transformed_circle(node: Node<'_, '_>, ellipse: bool) -> Option<[f64; 4]> {
    let (cx, cy) = (
        numeric_attribute(node, "cx")?,
        numeric_attribute(node, "cy")?,
    );
    let rx = numeric_attribute(node, if ellipse { "rx" } else { "r" })?;
    let ry = numeric_attribute(node, if ellipse { "ry" } else { "r" })?;
    if rx < 0.0 || ry < 0.0 {
        return None;
    }
    let transform = accumulated_transform(node)?;
    let corners = [
        transform.apply(cx - rx, cy - ry),
        transform.apply(cx + rx, cy - ry),
        transform.apply(cx - rx, cy + ry),
        transform.apply(cx + rx, cy + ry),
    ];
    let left = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::INFINITY, f64::min);
    let top = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::INFINITY, f64::min);
    let right = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);
    [left, top, right - left, bottom - top]
        .into_iter()
        .all(f64::is_finite)
        .then_some([left, top, right - left, bottom - top])
}

fn accumulated_transform(node: Node<'_, '_>) -> Option<AffineTransform> {
    let mut transform = AffineTransform::IDENTITY;
    let transforms = node
        .ancestors()
        .filter_map(|ancestor| ancestor.attribute("transform"))
        .collect::<Vec<_>>();
    for raw in transforms.into_iter().rev() {
        let parsed = raw.parse::<svgtypes::Transform>().ok()?;
        transform = transform.multiply(AffineTransform::from_svg(parsed));
    }
    Some(transform)
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

fn numeric_attribute(node: Node<'_, '_>, name: &str) -> Option<f64> {
    node.attribute(name)?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observes_dom_relationships_inline_styles_and_stylesheet_rules() {
        let svg = r##"<svg id="state" viewBox="0 0 40 30">
<style data-merman-typed-fonts="v1">@font-face{font-family:"Excalifont";src:url(data:font/ttf;base64,AA==)}#state .actor{fill:#123456!important;stroke:#654321}</style>
<g id="owner" class="actor"><rect x="2" y="3" width="10" height="8" style="fill:#abc;stroke:#111!important"/><text> Ready </text></g>
</svg>"##.to_owned();
        let receipt = SvgArtifactObservation::observe(&svg).expect("SVG observation");
        assert_eq!(receipt.root_id(), Some("state"));
        assert_eq!(receipt.view_box(), [0.0, 0.0, 40.0, 30.0]);
        assert_eq!(receipt.elements().len(), 5);
        let wrapper = receipt
            .elements()
            .iter()
            .find(|element| element.id() == Some("owner"))
            .expect("owner wrapper");
        let rect = receipt
            .elements()
            .iter()
            .find(|element| element.tag_name() == "rect")
            .expect("rect");
        assert_eq!(rect.parent_index(), Some(wrapper.index()));
        assert!(receipt.is_descendant_of(rect.index(), wrapper.index()));
        assert_eq!(rect.style_value("fill"), Some("#abc"));
        assert!(rect.style_is_important("stroke"));
        assert_eq!(rect.bounds(), Some([2.0, 3.0, 10.0, 8.0]));
        let stylesheet = &receipt.stylesheets()[0];
        assert!(stylesheet.typed_font_marker());
        assert!(stylesheet.parse_valid());
        assert_eq!(stylesheet.rules()[0].selector(), "#state .actor");
        assert_eq!(
            stylesheet.rules()[0]
                .declaration("fill")
                .map(SvgStyleDeclarationObservation::value),
            Some("#123456")
        );
        assert_eq!(stylesheet.font_faces().len(), 1);
    }
}
