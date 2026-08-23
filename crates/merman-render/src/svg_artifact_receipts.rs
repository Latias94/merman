//! Workspace-only renderer-owned observations for finalized SVG artifacts.
//!
//! These observations are deliberately feature-gated. The non-published acceptance harness may
//! evaluate its own fixture contract against these immutable facts, but it does not parse the
//! production SVG or reimplement the renderer's DOM/CSS interpretation.

use roxmltree::{Document, Node};
use sha2::{Digest as _, Sha256};

/// Renderer-owned, family-neutral observations of one finalized SVG artifact.
///
/// The acceptance harness receives this immutable projection instead of reparsing the terminal
/// XML/CSS. It may interpret semantic fixture contracts over these facts, but it cannot change the
/// observed artifact or mint a receipt for unrelated bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgArtifactReceipt {
    artifact_digest: [u8; 32],
    root_id: Option<Box<str>>,
    view_box_bits: [u64; 4],
    native_text: bool,
    has_foreign_object: bool,
    has_prepared_tokens: bool,
    elements: Box<[SvgElementObservation]>,
    stylesheets: Box<[SvgStylesheetObservation]>,
    digest: [u8; 32],
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

impl SvgArtifactReceipt {
    /// Observes the finalized public SVG and binds the projection to its exact byte digest.
    pub fn observe_svg(svg: &str, artifact_digest: [u8; 32]) -> Option<Self> {
        if artifact_digest == [0; 32]
            || Sha256::digest(svg.as_bytes()).as_slice() != artifact_digest
        {
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
        let mut receipt = Self {
            artifact_digest,
            root_id: root
                .attribute("id")
                .map(str::to_owned)
                .map(String::into_boxed_str),
            view_box_bits: view_box.map(f64::to_bits),
            native_text: document.descendants().any(|node| node.has_tag_name("text")),
            has_foreign_object: document
                .descendants()
                .any(|node| node.has_tag_name("foreignObject")),
            has_prepared_tokens: svg.contains("merman-prepared-"),
            elements: elements.into_boxed_slice(),
            stylesheets: stylesheets.into_boxed_slice(),
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

    pub const fn has_prepared_tokens(&self) -> bool {
        self.has_prepared_tokens
    }

    pub fn elements(&self) -> &[SvgElementObservation] {
        &self.elements
    }

    pub fn element(&self, index: usize) -> Option<&SvgElementObservation> {
        self.elements.get(index)
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

    fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"merman.svg-artifact-receipt.v1\0");
        hasher.update(self.artifact_digest);
        match &self.root_id {
            Some(root_id) => {
                hasher.update([1]);
                update_len_prefixed(&mut hasher, root_id.as_bytes());
            }
            None => hasher.update([0]),
        }
        for value in self.view_box_bits {
            hasher.update(value.to_be_bytes());
        }
        hasher.update([
            u8::from(self.native_text),
            u8::from(self.has_foreign_object),
            u8::from(self.has_prepared_tokens),
        ]);
        hasher.update((self.elements.len() as u64).to_be_bytes());
        for element in self.elements.iter() {
            hasher.update((element.index as u64).to_be_bytes());
            update_len_prefixed(&mut hasher, element.tag.as_bytes());
            update_optional_string(&mut hasher, element.id.as_deref());
            hasher.update((element.classes.len() as u64).to_be_bytes());
            for class in element.classes.iter() {
                update_len_prefixed(&mut hasher, class.as_bytes());
            }
            hasher.update((element.attributes.len() as u64).to_be_bytes());
            for attribute in element.attributes.iter() {
                update_len_prefixed(&mut hasher, attribute.name.as_bytes());
                update_len_prefixed(&mut hasher, attribute.value.as_bytes());
            }
            update_style_declarations(&mut hasher, &element.inline_styles);
            update_len_prefixed(&mut hasher, element.text.as_bytes());
            match element.parent_index {
                Some(parent) => {
                    hasher.update([1]);
                    hasher.update((parent as u64).to_be_bytes());
                }
                None => hasher.update([0]),
            }
            match element.bounds_bits {
                Some(bounds) => {
                    hasher.update([1]);
                    for value in bounds {
                        hasher.update(value.to_be_bytes());
                    }
                }
                None => hasher.update([0]),
            }
        }
        hasher.update((self.stylesheets.len() as u64).to_be_bytes());
        for stylesheet in self.stylesheets.iter() {
            hasher.update([
                u8::from(stylesheet.typed_font_marker),
                u8::from(stylesheet.parse_valid),
            ]);
            hasher.update((stylesheet.rules.len() as u64).to_be_bytes());
            for rule in stylesheet.rules.iter() {
                update_len_prefixed(&mut hasher, rule.selector.as_bytes());
                update_style_declarations(&mut hasher, &rule.declarations);
            }
            hasher.update((stylesheet.font_faces.len() as u64).to_be_bytes());
            for font_face in stylesheet.font_faces.iter() {
                update_style_declarations(&mut hasher, &font_face.declarations);
            }
        }
        hasher.finalize().into()
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartSvgArtifactReceipt {
    artifact_digest: [u8; 32],
    view_box_bits: [u64; 4],
    native_text: bool,
    has_foreign_object: bool,
    has_prepared_tokens: bool,
    canvas: Option<FlowchartSvgCanvasObservation>,
    nodes: Box<[FlowchartSvgNodeObservation]>,
    digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartSvgNodeObservation {
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
pub struct FlowchartSvgCanvasObservation {
    gradient_units: Box<str>,
    spread_method: Box<str>,
    x1_bits: u64,
    y1_bits: u64,
    x2_bits: u64,
    y2_bits: u64,
    stops: Box<[FlowchartSvgGradientStopObservation]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowchartSvgGradientStopObservation {
    offset_bits: u64,
    color: Box<str>,
}

impl FlowchartSvgArtifactReceipt {
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
            nodes.push(FlowchartSvgNodeObservation {
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

    pub fn node_observations(&self) -> &[FlowchartSvgNodeObservation] {
        &self.nodes
    }

    pub const fn has_native_text(&self) -> bool {
        self.native_text
    }

    pub const fn has_foreign_object(&self) -> bool {
        self.has_foreign_object
    }

    pub const fn has_prepared_tokens(&self) -> bool {
        self.has_prepared_tokens
    }

    pub fn canvas_observation(&self) -> Option<&FlowchartSvgCanvasObservation> {
        self.canvas.as_ref()
    }

    fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"merman.flowchart-svg-artifact-receipt.v1\0");
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

impl FlowchartSvgNodeObservation {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn region(&self) -> [f64; 4] {
        self.region_bits.map(f64::from_bits)
    }

    pub fn fill(&self) -> Option<&str> {
        self.fill.as_deref()
    }

    pub const fn stroke(&self) -> &str {
        &self.stroke
    }

    pub const fn stroke_width(&self) -> f64 {
        f64::from_bits(self.stroke_width_bits)
    }

    pub const fn radius_x(&self) -> f64 {
        f64::from_bits(self.radius_x_bits)
    }

    pub const fn radius_y(&self) -> f64 {
        f64::from_bits(self.radius_y_bits)
    }

    pub const fn dasharray(&self) -> &str {
        &self.dasharray
    }

    pub const fn font_family(&self) -> &str {
        &self.font_family
    }
}

impl FlowchartSvgCanvasObservation {
    pub const fn gradient_units(&self) -> &str {
        &self.gradient_units
    }

    pub const fn spread_method(&self) -> &str {
        &self.spread_method
    }

    pub const fn coordinates(&self) -> [f64; 4] {
        [
            f64::from_bits(self.x1_bits),
            f64::from_bits(self.y1_bits),
            f64::from_bits(self.x2_bits),
            f64::from_bits(self.y2_bits),
        ]
    }

    pub const fn stops(&self) -> &[FlowchartSvgGradientStopObservation] {
        &self.stops
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

impl FlowchartSvgGradientStopObservation {
    pub const fn offset(&self) -> f64 {
        f64::from_bits(self.offset_bits)
    }

    pub const fn color(&self) -> &str {
        &self.color
    }
}

fn update_len_prefixed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn update_optional_string(hasher: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            update_len_prefixed(hasher, value.as_bytes());
        }
        None => hasher.update([0]),
    }
}

fn update_style_declarations(hasher: &mut Sha256, declarations: &[SvgStyleDeclarationObservation]) {
    hasher.update((declarations.len() as u64).to_be_bytes());
    for declaration in declarations {
        update_len_prefixed(hasher, declaration.property.as_bytes());
        update_len_prefixed(hasher, declaration.value.as_bytes());
        hasher.update([u8::from(declaration.important)]);
    }
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

fn observe_canvas(document: &Document<'_>) -> Option<FlowchartSvgCanvasObservation> {
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
            Some(FlowchartSvgGradientStopObservation {
                offset_bits: percent_value(stop.attribute("offset")?)?.to_bits(),
                color: stop.attribute("stop-color")?.to_owned().into_boxed_str(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(FlowchartSvgCanvasObservation {
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
    fn receipt_binds_exact_svg_and_exposes_node_observations() {
        let svg = sample_svg(["#a", "#b", "#c"]);
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        let receipt = FlowchartSvgArtifactReceipt::observe_svg(&svg, digest).expect("seal receipt");

        assert!(receipt.proves_artifact(digest));
        assert!(receipt.has_native_text());
        assert!(!receipt.has_foreign_object());
        assert!(!receipt.has_prepared_tokens());
        let nodes = receipt.node_observations();
        assert_eq!(nodes.len(), 6);
        assert_eq!(nodes[0].id(), "A");
        assert_eq!(nodes[0].fill(), Some("#a"));
        assert_eq!(nodes[0].stroke(), "#111");
        assert_eq!(nodes[0].stroke_width(), 3.0);
        assert_eq!(nodes[0].radius_x(), 5.0);
        assert_eq!(nodes[0].radius_y(), 5.0);
    }

    #[test]
    fn receipt_rejects_forged_digest_or_changed_style() {
        let svg = sample_svg(["#a", "#b", "#c"]);
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        assert!(FlowchartSvgArtifactReceipt::observe_svg(&svg, [1; 32]).is_none());

        let changed = sample_svg(["#a", "#b", "#d"]);
        let changed_digest: [u8; 32] = Sha256::digest(changed.as_bytes()).into();
        let receipt = FlowchartSvgArtifactReceipt::observe_svg(&changed, changed_digest)
            .expect("seal changed");
        assert_eq!(receipt.node_observations()[2].fill(), Some("#d"));
        assert_ne!(receipt.digest(), digest);
    }

    #[test]
    fn receipt_observes_spotless_gradient_and_terminal_nodes() {
        let svg = sample_spotless_svg();
        let receipt = spotless_receipt(&svg);
        assert!(receipt.has_native_text());
        assert!(!receipt.has_foreign_object());
        assert!(!receipt.has_prepared_tokens());
        assert_eq!(receipt.node_observations().len(), 4);
        let canvas = receipt.canvas_observation().expect("canvas observation");
        assert_eq!(canvas.gradient_units(), "userSpaceOnUse");
        assert_eq!(canvas.spread_method(), "repeat");
        assert_eq!(
            canvas.coordinates(),
            [0.0, 0.0, 14.1421356237, -14.1421356237]
        );
        assert_eq!(canvas.stops().len(), 4);
        assert_eq!(canvas.stops()[2].offset(), 51.0);
        assert_eq!(canvas.stops()[2].color(), "#d2ccc0");
        assert_eq!(receipt.node_observations()[0].dasharray(), "6 4");
        assert_eq!(receipt.node_observations()[0].font_family(), "Excalifont");
    }

    #[test]
    fn receipt_rejects_mutated_spotless_gradient_or_terminal_style() {
        let valid = sample_spotless_svg();
        let valid_digest = spotless_receipt(&valid).digest();
        let mutations = [
            valid.replace("offset=\"50%\"", "offset=\"0%\""),
            valid.replace("url(#stripe)", "url(#missing-gradient)"),
            valid.replace("x2=\"14.1421356237\"", "x2=\"10\""),
            valid.replace("stroke-dasharray:6 4", "stroke-dasharray:4 2"),
            valid.replace("font-family:Excalifont", "font-family:Arial"),
        ];

        for mutated in mutations {
            let receipt = spotless_receipt(&mutated);
            assert_ne!(
                receipt.digest(),
                valid_digest,
                "mutation was not rebound: {mutated}"
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
            assert!(receipt.canvas_observation().is_none());
        }
    }

    #[test]
    fn generic_receipt_seals_dom_relationships_inline_styles_and_stylesheet_rules() {
        let svg = r##"<svg id="state" viewBox="0 0 40 30">
<style data-merman-typed-fonts="v1">@font-face{font-family:"Excalifont";src:url(data:font/ttf;base64,AA==)}#state .actor{fill:#123456!important;stroke:#654321}</style>
<g id="owner" class="actor"><rect x="2" y="3" width="10" height="8" style="fill:#abc;stroke:#111!important"/><text> Ready </text></g>
</svg>"##.to_owned();
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        let receipt = SvgArtifactReceipt::observe_svg(&svg, digest).expect("generic receipt");
        assert!(receipt.proves_artifact(digest));
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

    fn spotless_receipt(svg: &str) -> FlowchartSvgArtifactReceipt {
        let digest: [u8; 32] = Sha256::digest(svg.as_bytes()).into();
        FlowchartSvgArtifactReceipt::observe_svg(svg, digest).expect("seal receipt")
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
