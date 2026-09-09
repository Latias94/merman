//! Bounded lowering of registry-owned SVG icon bodies, not rendered diagram SVG.
//!
//! This intentionally admits a small presentation-attribute subset. Referenced resources,
//! stylesheets, filters, text, and other unrepresented effects fail closed.

use super::builder::DrawingListBuilder;
use super::flowchart::{ellipse_path, elliptical_rounded_rect_path};
use super::support::{PortableStyleResolver, stroke};
use crate::environment::RenderSession;
use crate::family::RenderFamilyKind;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_display_list::{
    Color, DrawingCommand, FillRule, LineCap, LineJoin, Paint, PathSegment, PathStyle, Point,
    ResourceId, Transform,
};
use roxmltree::Node;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct AssetStructure {
    pub(crate) primitives: BTreeMap<String, PrimitiveGeometry>,
    pub(crate) dom_ids: BTreeMap<String, String>,
    pub(crate) scopes: BTreeMap<usize, AssetScope>,
}

impl AssetStructure {
    pub(crate) fn extend(&mut self, other: Self) {
        self.primitives.extend(other.primitives);
        self.dom_ids.extend(other.dom_ids);
        self.scopes.extend(other.scopes);
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AssetScope {
    pub(crate) end: usize,
    pub(crate) transform: Option<AssetTransform>,
    pub(crate) dom_id: Option<String>,
    pub(crate) kind: AssetScopeKind,
}

#[derive(Debug, Clone)]
pub(crate) enum AssetScopeKind {
    Group,
    EmptyPrimitive(PrimitiveGeometry),
}

#[derive(Debug, Clone)]
pub(crate) enum AssetTransform {
    Source(String),
    Alias(crate::svg::IconGeometryPlan),
}

impl AssetTransform {
    pub(crate) fn matching_prefix(
        &self,
        commands: &[DrawingCommand],
        session: &RenderSession,
    ) -> Result<Option<usize>> {
        match self {
            Self::Source(source) => matching_transform_prefix(source, commands, session),
            Self::Alias(plan) => {
                let mut count = 0;
                for expected in plan.transforms() {
                    session.checkpoint(OperationPhase::Emit)?;
                    if !matches!(commands.get(count), Some(DrawingCommand::ConcatTransform { transform }) if *transform == expected.matrix())
                    {
                        return Ok(None);
                    }
                    count += 1;
                }
                Ok(Some(count))
            }
        }
    }
}

impl std::fmt::Display for AssetTransform {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(source) => output.write_str(source),
            Self::Alias(plan) => {
                for (index, transform) in plan.transforms().enumerate() {
                    if index != 0 {
                        output.write_str(" ")?;
                    }
                    write!(output, "{transform}")?;
                }
                Ok(())
            }
        }
    }
}

impl AssetScope {
    fn capture(
        node: Node<'_, '_>,
        start: usize,
        dom_id: Option<String>,
        kind: AssetScopeKind,
        session: &RenderSession,
    ) -> Result<Self> {
        let transform = node
            .attribute("transform")
            .map(|source| -> Result<_> {
                session
                    .work_meter()
                    .charge_at(source.len(), OperationPhase::Emit)?;
                let mut owned = String::new();
                owned
                    .try_reserve_exact(source.len())
                    .map_err(|_| allocation("icon transform spelling"))?;
                owned.push_str(source);
                Ok(AssetTransform::Source(owned))
            })
            .transpose()?;
        Ok(Self {
            end: start,
            transform,
            dom_id,
            kind,
        })
    }
}

pub(crate) struct AssetIdentity<'a> {
    pub(crate) resource_prefix: &'a str,
    pub(crate) svg_scope: Option<crate::svg::IconIdScope>,
}

/// Source transform spelling is usable only for the exact current command prefix.
fn matching_transform_prefix(
    source: &str,
    commands: &[DrawingCommand],
    session: &RenderSession,
) -> Result<Option<usize>> {
    session
        .work_meter()
        .charge_at(source.len(), OperationPhase::Emit)?;
    let context = Context {
        session,
        family: RenderFamilyKind::TreeView,
    };
    let mut count = 0;
    for token in svgtypes::TransformListParser::from(source) {
        session.checkpoint(OperationPhase::Emit)?;
        let Ok(token) = token else { return Ok(None) };
        let Ok(expected) = context.transform(token) else {
            return Ok(None);
        };
        if !matches!(commands.get(count), Some(DrawingCommand::ConcatTransform { transform }) if *transform == expected)
        {
            return Ok(None);
        }
        count += 1;
    }
    Ok(Some(count))
}

/// Source spelling only. A serializer must check it against the current public path.
#[derive(Debug, Clone)]
pub(crate) struct PrimitiveGeometry {
    pub(crate) tag: &'static str,
    pub(crate) attributes: Vec<(&'static str, String)>,
}

impl PrimitiveGeometry {
    fn capture(node: Node<'_, '_>, session: &RenderSession) -> Result<Self> {
        let (tag, names): (&str, &[&str]) = match node.tag_name().name() {
            "path" => ("path", &["d"]),
            "rect" => ("rect", &["x", "y", "width", "height", "rx", "ry"]),
            "circle" => ("circle", &["cx", "cy", "r"]),
            "ellipse" => ("ellipse", &["cx", "cy", "rx", "ry"]),
            "line" => ("line", &["x1", "y1", "x2", "y2"]),
            "polygon" => ("polygon", &["points"]),
            "polyline" => ("polyline", &["points"]),
            _ => unreachable!("only admitted primitive elements have geometry"),
        };
        let mut attributes = Vec::new();
        attributes
            .try_reserve_exact(names.len())
            .map_err(|_| allocation("icon geometry attributes"))?;
        for &name in names {
            if let Some(value) = node.attribute(name) {
                session
                    .work_meter()
                    .charge_at(value.len(), OperationPhase::Emit)?;
                let mut owned = String::new();
                owned
                    .try_reserve_exact(value.len())
                    .map_err(|_| allocation("icon geometry spelling"))?;
                owned.push_str(value);
                attributes.push((name, owned));
            }
        }
        Ok(Self { tag, attributes })
    }

    pub(crate) fn matches_path(
        &self,
        segments: &[PathSegment],
        session: &RenderSession,
    ) -> Result<bool> {
        for (_, value) in &self.attributes {
            session
                .work_meter()
                .charge_at(value.len(), OperationPhase::Emit)?;
        }
        let context = Context {
            session,
            family: RenderFamilyKind::TreeView,
        };
        let mut index = 0;
        let mut equal = true;
        let result = context.geometry(
            self.tag,
            |name| {
                self.attributes
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.as_str())
            },
            &mut |segment| {
                session.checkpoint(OperationPhase::Emit)?;
                equal &= segments.get(index) == Some(&segment);
                index += 1;
                Ok(())
            },
        );
        match result {
            Ok(()) => Ok(equal && index == segments.len()),
            // A stale representation hint must not override or reject valid public geometry.
            Err(Error::InvalidModel { .. } | Error::DrawingListUnavailable { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Copy)]
enum IconPaint {
    None,
    CurrentColor,
    Color(Color),
}

#[derive(Clone, Copy)]
struct Style<'a> {
    color: Color,
    fill: IconPaint,
    stroke: IconPaint,
    fill_opacity: f64,
    stroke_opacity: f64,
    width: f64,
    fill_rule: FillRule,
    cap: LineCap,
    join: LineJoin,
    miter: f64,
    dash: Option<&'a str>,
    dash_offset: f64,
}

impl Style<'_> {
    fn inherited(color: Color, fill: Color) -> Self {
        Self {
            color,
            fill: IconPaint::Color(fill),
            stroke: IconPaint::None,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            width: 1.0,
            fill_rule: FillRule::NonZero,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter: 4.0,
            dash: None,
            dash_offset: 0.0,
        }
    }
}

/// The caller owns viewport clipping and alias transforms. The icon root's inherited
/// `color` and `fill` are independent: only an explicit currentColor paint joins them.
pub(crate) fn lower_icon_asset(
    body: &str,
    color: Color,
    inherited_fill: Color,
    identity: AssetIdentity<'_>,
    builder: &mut DrawingListBuilder<'_>,
    session: &RenderSession,
    family: RenderFamilyKind,
) -> Result<AssetStructure> {
    let context = Context { session, family };
    let bytes = body
        .len()
        .checked_add(46)
        .ok_or_else(|| context.unsupported("XML size overflow"))?;
    // XML parsing and its temporary source copy are separate from the final path footprint.
    session
        .work_meter()
        .charge_at(bytes, OperationPhase::Emit)?;
    let mut xml = String::new();
    xml.try_reserve_exact(bytes)
        .map_err(|_| allocation("icon XML"))?;
    xml.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\">");
    xml.push_str(body);
    xml.push_str("</svg>");
    let document = roxmltree::Document::parse(&xml)
        .map_err(|error| context.unsupported(format!("invalid icon XML: {error}")))?;
    let root = document.root_element();
    let mut stack = Vec::new();
    let mut state = Style::inherited(color, inherited_fill);
    let mut index = 0usize;
    let mut structure = AssetStructure::default();
    let mut id_index = 0;
    let mut cursor = root.first_child().map(|node| (node, true));
    while let Some((node, entering)) = cursor {
        session.checkpoint(OperationPhase::Emit)?;
        cursor = if entering {
            Some(
                node.first_child()
                    .map_or((node, false), |child| (child, true)),
            )
        } else {
            node.next_sibling()
                .map(|sibling| (sibling, true))
                .or_else(|| {
                    node.parent()
                        .filter(|parent| *parent != root)
                        .map(|parent| (parent, false))
                })
        };
        if !node.is_element() {
            if node.is_text() && node.text().is_some_and(|text| !text.trim().is_empty()) {
                return Err(context.unsupported("text content"));
            }
            if node.is_pi() {
                return Err(context.unsupported("XML processing instruction"));
            }
            continue;
        }
        match entering {
            true => {
                let tag = node.tag_name().name();
                if node.tag_name().namespace() != Some("http://www.w3.org/2000/svg")
                    || !matches!(
                        tag,
                        "g" | "path"
                            | "rect"
                            | "circle"
                            | "ellipse"
                            | "line"
                            | "polygon"
                            | "polyline"
                    )
                {
                    return Err(context.unsupported(format!("element <{tag}>")));
                }
                if tag != "g" && node.children().any(|child| child.is_element()) {
                    return Err(context.unsupported(format!("nested content in <{tag}>")));
                }
                let next = context.style(node, state)?;
                let mut dom_id = if node.attribute("id").is_some() {
                    let id = identity
                        .svg_scope
                        .map(|scope| scope.scoped_id(id_index))
                        .transpose()?;
                    id_index += 1;
                    id
                } else {
                    None
                };
                let start = builder.command_count();
                builder.push_control(DrawingCommand::Save)?;
                stack
                    .try_reserve(1)
                    .map_err(|_| allocation("icon scopes"))?;
                stack.push((state, start));
                state = next;
                if tag == "g" {
                    structure.scopes.insert(
                        start,
                        AssetScope::capture(
                            node,
                            start,
                            dom_id.take(),
                            AssetScopeKind::Group,
                            session,
                        )?,
                    );
                }
                if let Some(value) = node.attribute("transform") {
                    for token in svgtypes::TransformListParser::from(value) {
                        session.checkpoint(OperationPhase::Emit)?;
                        let token = token
                            .map_err(|error| context.unsupported(format!("transform: {error}")))?;
                        builder.push_control(DrawingCommand::ConcatTransform {
                            transform: context.transform(token)?,
                        })?;
                    }
                }
                if tag != "g" {
                    let style = context.path_style(state)?;
                    let id = format!("{}.shape.{index}", identity.resource_prefix);
                    if builder.draw_optional_path_with(
                        ResourceId::new(id.clone()),
                        style,
                        |emit| context.geometry(tag, |name| node.attribute(name), emit),
                    )? {
                        if let Some(dom_id) = dom_id {
                            structure.dom_ids.insert(id.clone(), dom_id);
                        }
                        structure
                            .primitives
                            .insert(id, PrimitiveGeometry::capture(node, session)?);
                    } else {
                        let empty = PrimitiveGeometry::capture(node, session)?;
                        structure.scopes.insert(
                            start,
                            AssetScope::capture(
                                node,
                                start,
                                dom_id.take(),
                                AssetScopeKind::EmptyPrimitive(empty),
                                session,
                            )?,
                        );
                    }
                    index += 1;
                }
            }
            false => {
                let (previous, start) = stack
                    .pop()
                    .ok_or_else(|| context.unsupported("unbalanced XML scopes"))?;
                if let Some(group) = structure.scopes.get_mut(&start) {
                    group.end = builder.command_count();
                }
                builder.push_control(DrawingCommand::Restore)?;
                state = previous;
            }
        }
    }
    Ok(structure)
}

struct Context<'a> {
    session: &'a RenderSession,
    family: RenderFamilyKind,
}

impl Context<'_> {
    fn unsupported(&self, effect: impl std::fmt::Display) -> Error {
        Error::DrawingListUnavailable {
            family: self.family.as_str().to_owned(),
            reason: format!("icon asset cannot represent {effect}"),
        }
    }

    fn resolver(&self) -> PortableStyleResolver {
        PortableStyleResolver::new(self.family.as_str())
    }

    fn paint(&self, value: &str) -> Result<IconPaint> {
        if value.eq_ignore_ascii_case("currentColor") {
            return Ok(IconPaint::CurrentColor);
        }
        Ok(match self.resolver().optional_color("icon paint", value)? {
            Some(color) => IconPaint::Color(color),
            None => IconPaint::None,
        })
    }

    fn number(&self, value: &str) -> Result<f64> {
        let value = value.trim();
        let number = value
            .strip_suffix("px")
            .unwrap_or(value)
            .parse::<f64>()
            .map_err(|_| self.unsupported(format!("length `{value}`")))?;
        if !number.is_finite() {
            return Err(self.unsupported("non-finite number"));
        }
        Ok(number)
    }

    fn style<'a>(&self, node: Node<'a, '_>, inherited: Style<'a>) -> Result<Style<'a>> {
        let mut style = inherited;
        for attribute in node.attributes() {
            self.session.checkpoint(OperationPhase::Emit)?;
            if attribute.namespace().is_some() {
                return Err(self.unsupported("namespaced attribute"));
            }
            let key = attribute.name();
            if matches!(key, "style" | "transform" | "id") {
                continue;
            }
            let geometry = match node.tag_name().name() {
                "path" => key == "d",
                "rect" => matches!(key, "x" | "y" | "width" | "height" | "rx" | "ry"),
                "circle" => matches!(key, "cx" | "cy" | "r"),
                "ellipse" => matches!(key, "cx" | "cy" | "rx" | "ry"),
                "line" => matches!(key, "x1" | "x2" | "y1" | "y2"),
                "polygon" | "polyline" => key == "points",
                _ => false,
            };
            if !geometry {
                self.property(&mut style, key, attribute.value(), inherited)?;
            }
        }
        if let Some(inline) = node.attribute("style") {
            // This subset contains no strings, escaped tokens, or semicolon-bearing functions.
            // Do not approximate a stylesheet parser when a declaration is outside it.
            for declaration in inline
                .split(';')
                .map(str::trim)
                .filter(|part| !part.is_empty())
            {
                self.session.checkpoint(OperationPhase::Emit)?;
                let (key, value) = declaration
                    .split_once(':')
                    .ok_or_else(|| self.unsupported("malformed style declaration"))?;
                self.property(&mut style, key.trim(), value.trim(), inherited)?;
            }
        }
        Ok(style)
    }

    fn property<'a>(
        &self,
        style: &mut Style<'a>,
        key: &str,
        value: &'a str,
        inherited: Style<'a>,
    ) -> Result<()> {
        let value = value.trim();
        if value.contains('!') || value.contains('\\') {
            return Err(self.unsupported(format!("style `{key}: {value}`")));
        }
        if value == "inherit" {
            match key {
                "color" => style.color = inherited.color,
                "fill" => style.fill = inherited.fill,
                "stroke" => style.stroke = inherited.stroke,
                "fill-opacity" => style.fill_opacity = inherited.fill_opacity,
                "stroke-opacity" => style.stroke_opacity = inherited.stroke_opacity,
                "stroke-width" => style.width = inherited.width,
                "fill-rule" => style.fill_rule = inherited.fill_rule,
                "stroke-linecap" => style.cap = inherited.cap,
                "stroke-linejoin" => style.join = inherited.join,
                "stroke-miterlimit" => style.miter = inherited.miter,
                "stroke-dasharray" => style.dash = inherited.dash,
                "stroke-dashoffset" => style.dash_offset = inherited.dash_offset,
                _ => return Err(self.unsupported(format!("inherited property `{key}`"))),
            }
            return Ok(());
        }
        match key {
            "color" => {
                style.color = if value.eq_ignore_ascii_case("currentColor") {
                    inherited.color
                } else {
                    self.resolver().color(key, value)?
                }
            }
            "fill" => style.fill = self.paint(value)?,
            "stroke" => style.stroke = self.paint(value)?,
            "fill-opacity" => style.fill_opacity = self.resolver().opacity(key, value)?,
            "stroke-opacity" => style.stroke_opacity = self.resolver().opacity(key, value)?,
            "opacity" if self.resolver().opacity(key, value)? == 1.0 => {}
            "stroke-width" => style.width = self.resolver().length(key, value)?,
            "stroke-dashoffset" => style.dash_offset = self.number(value)?,
            "stroke-dasharray" => style.dash = (value != "none").then_some(value),
            "stroke-miterlimit" => {
                style.miter = self.number(value)?;
                if style.miter < 1.0 {
                    return Err(self.unsupported("stroke-miterlimit below one"));
                }
            }
            "fill-rule" => {
                style.fill_rule = match value {
                    "evenodd" => FillRule::EvenOdd,
                    "nonzero" => FillRule::NonZero,
                    _ => return Err(self.unsupported("fill-rule")),
                }
            }
            "stroke-linecap" => {
                style.cap = match value {
                    "butt" => LineCap::Butt,
                    "round" => LineCap::Round,
                    "square" => LineCap::Square,
                    _ => return Err(self.unsupported("stroke-linecap")),
                }
            }
            "stroke-linejoin" => {
                style.join = match value {
                    "miter" => LineJoin::Miter,
                    "round" => LineJoin::Round,
                    "bevel" => LineJoin::Bevel,
                    _ => return Err(self.unsupported("stroke-linejoin")),
                }
            }
            _ => return Err(self.unsupported(format!("property `{key}: {value}`"))),
        }
        Ok(())
    }

    fn path_style(&self, style: Style<'_>) -> Result<PathStyle> {
        let color = |paint, opacity| match paint {
            IconPaint::None => None,
            IconPaint::CurrentColor => Some(with_alpha(style.color, opacity)),
            IconPaint::Color(color) => Some(with_alpha(color, opacity)),
        };
        let mut outline = color(style.stroke, style.stroke_opacity)
            .filter(|_| style.width > 0.0)
            .map(|color| stroke(color, style.width));
        if let Some(outline) = &mut outline {
            outline.line_cap = style.cap;
            outline.line_join = style.join;
            outline.miter_limit = style.miter;
            outline.dash_offset = style.dash_offset;
            if let Some(dash) = style.dash {
                // Fixed scratch storage keeps asset parsing bounded before the builder owns it.
                // More elaborate patterns remain an explicit unsupported effect in this subset.
                let mut values = [0.0; 32];
                let mut count = 0;
                for number in svgtypes::NumberListParser::from(dash) {
                    self.session.checkpoint(OperationPhase::Emit)?;
                    let number = number.map_err(|_| self.unsupported("stroke-dasharray number"))?;
                    if count == values.len() || !number.is_finite() || number < 0.0 {
                        return Err(
                            self.unsupported("stroke-dasharray (at most 32 nonnegative entries)")
                        );
                    }
                    values[count] = number;
                    count += 1;
                }
                if count == 0 {
                    return Err(self.unsupported("empty stroke-dasharray"));
                }
                if values[..count].iter().any(|value| *value != 0.0) {
                    outline
                        .dash_array
                        .try_reserve_exact(count)
                        .map_err(|_| allocation("icon stroke dash"))?;
                    outline.dash_array.extend_from_slice(&values[..count]);
                }
            }
        }
        Ok(PathStyle {
            fill_rule: style.fill_rule,
            fill: color(style.fill, style.fill_opacity).map(Paint::solid),
            stroke: outline,
        })
    }

    fn transform(&self, token: svgtypes::TransformListToken) -> Result<Transform> {
        use svgtypes::TransformListToken as T;
        let mut matrix = Transform::IDENTITY;
        match token {
            T::Matrix { a, b, c, d, e, f } => matrix = Transform { a, b, c, d, e, f },
            T::Translate { tx, ty } => {
                matrix.e = tx;
                matrix.f = ty;
            }
            T::Scale { sx, sy } => {
                matrix.a = sx;
                matrix.d = sy;
            }
            T::Rotate { angle } => {
                let (sin, cos) = angle.to_radians().sin_cos();
                matrix.a = cos;
                matrix.b = sin;
                matrix.c = -sin;
                matrix.d = cos;
            }
            T::SkewX { angle } => matrix.c = angle.to_radians().tan(),
            T::SkewY { angle } => matrix.b = angle.to_radians().tan(),
        }
        if ![matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f]
            .iter()
            .all(|number| number.is_finite())
        {
            return Err(self.unsupported("non-finite transform"));
        }
        Ok(matrix)
    }

    fn geometry<'a>(
        &self,
        tag: &str,
        attribute: impl Fn(&str) -> Option<&'a str>,
        emit: &mut dyn FnMut(PathSegment) -> Result<()>,
    ) -> Result<()> {
        let length =
            |name, default| attribute(name).map_or(Ok(default), |value| self.number(value));
        let nonnegative = |name, default| {
            let value = length(name, default)?;
            if value < 0.0 {
                return Err(self.unsupported(format!("negative `{name}`")));
            }
            Ok(value)
        };
        match tag {
            "path" => super::write_svg_path(attribute("d").unwrap_or_default(), emit),
            "rect" => {
                let (x, y, w, h) = (
                    length("x", 0.0)?,
                    length("y", 0.0)?,
                    nonnegative("width", 0.0)?,
                    nonnegative("height", 0.0)?,
                );
                if w == 0.0 || h == 0.0 {
                    return Ok(());
                }
                let rx = nonnegative("rx", attribute("ry").map_or(Ok(0.0), |v| self.number(v))?)?;
                let ry = nonnegative("ry", rx)?;
                for segment in elliptical_rounded_rect_path(x + w / 2.0, y + h / 2.0, w, h, rx, ry)
                {
                    emit(segment)?;
                }
                Ok(())
            }
            "circle" | "ellipse" => {
                let x = length("cx", 0.0)?;
                let y = length("cy", 0.0)?;
                let rx = nonnegative(if tag == "circle" { "r" } else { "rx" }, 0.0)?;
                let ry = if tag == "circle" {
                    rx
                } else {
                    nonnegative("ry", 0.0)?
                };
                if rx == 0.0 || ry == 0.0 {
                    return Ok(());
                }
                for segment in ellipse_path(x, y, rx, ry) {
                    emit(segment)?;
                }
                Ok(())
            }
            "line" => {
                emit(PathSegment::MoveTo {
                    to: Point::new(length("x1", 0.0)?, length("y1", 0.0)?),
                })?;
                emit(PathSegment::LineTo {
                    to: Point::new(length("x2", 0.0)?, length("y2", 0.0)?),
                })
            }
            "polygon" | "polyline" => {
                let mut numbers =
                    svgtypes::NumberListParser::from(attribute("points").unwrap_or_default());
                let mut first = true;
                while let Some(x) = numbers.next() {
                    let x = x.map_err(|_| self.unsupported("points number"))?;
                    let y = numbers
                        .next()
                        .ok_or_else(|| self.unsupported("odd points count"))?
                        .map_err(|_| self.unsupported("points number"))?;
                    if !x.is_finite() || !y.is_finite() {
                        return Err(self.unsupported("non-finite point"));
                    }
                    let to = Point::new(x, y);
                    emit(if first {
                        PathSegment::MoveTo { to }
                    } else {
                        PathSegment::LineTo { to }
                    })?;
                    first = false;
                }
                if !first && tag == "polygon" {
                    emit(PathSegment::Close)?;
                }
                Ok(())
            }
            _ => Err(self.unsupported(format!("geometry <{tag}>"))),
        }
    }
}

fn with_alpha(mut color: Color, opacity: f64) -> Color {
    color.alpha = (f64::from(color.alpha) * opacity).round() as u8;
    color
}

fn allocation(collection: &'static str) -> Error {
    Error::DrawingListAllocationFailed { collection }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use merman_display_list::{
        DrawingListDocument, DrawingListLimits, DrawingListPolicy, DrawingResource, Rect, Viewport,
    };
    use std::collections::BTreeMap;

    fn lower(body: &str, limits: DrawingListLimits) -> Result<DrawingListDocument> {
        let session = RenderEnvironment::default().begin_session().unwrap();
        let mut builder = DrawingListBuilder::new(DrawingListPolicy::VectorOnly, limits, &session);
        lower_icon_asset(
            body,
            Color::rgba(12, 34, 56, 255),
            Color::rgba(51, 51, 51, 255),
            AssetIdentity {
                resource_prefix: "asset",
                svg_scope: None,
            },
            &mut builder,
            &session,
            RenderFamilyKind::Architecture,
        )?;
        builder.finish(
            Viewport::new(Rect::new(0.0, 0.0, 24.0, 24.0)),
            BTreeMap::new(),
        )
    }

    #[test]
    fn primitive_asset_streams_paths_and_omits_zero_extent_rect() {
        let document = lower(r#"<g><path d="M0 0L2 2Z"/><rect x="1" width="3" height="4" rx="1"/><circle r="2"/><ellipse rx="2" ry="3"/><line x2="4"/><polygon points="0 0 1 1 2 0"/><polyline points="0 0 1 1"/><rect width="0" height="10"/></g>"#, DrawingListLimits::default()).unwrap();
        assert_eq!(document.resources.len(), 7);
        assert_eq!(
            document
                .commands
                .iter()
                .filter(|command| matches!(command, DrawingCommand::DrawPath { .. }))
                .count(),
            7
        );
        assert!(
            document
                .resources
                .iter()
                .all(|resource| matches!(resource, DrawingResource::Path(_)))
        );
    }

    #[test]
    fn icon_style_inherits_and_resolves_currentcolor_after_local_color() {
        let document = lower(r##"<g color="#ff0000" fill-opacity="0.5" stroke="currentColor" stroke-linecap="round"><path fill="currentColor" style="stroke-width:2;fill-rule:evenodd;color:#0000ff;stroke-dasharray:2,3" d="M0 0L5 5"/></g>"##, DrawingListLimits::default()).unwrap();
        let style = document
            .commands
            .iter()
            .find_map(|command| match command {
                DrawingCommand::DrawPath { style, .. } => Some(style),
                _ => None,
            })
            .unwrap();
        assert_eq!(style.fill, Some(Paint::solid(Color::rgba(0, 0, 255, 128))));
        assert_eq!(style.fill_rule, FillRule::EvenOdd);
        let outline = style.stroke.as_ref().unwrap();
        assert_eq!(outline.paint, Paint::solid(Color::rgba(0, 0, 255, 255)));
        assert_eq!(outline.width, 2.0);
        assert_eq!(outline.line_cap, LineCap::Round);
        assert_eq!(outline.dash_array, [2.0, 3.0]);
    }

    #[test]
    fn inherited_fill_is_independent_of_icon_color() {
        let document = lower(
            r#"<path d="M0 0L1 1"/><path fill="currentColor" d="M0 0L1 1"/>"#,
            DrawingListLimits::default(),
        )
        .unwrap();
        let fills: Vec<_> = document
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawingCommand::DrawPath { style, .. } => style.fill.clone(),
                _ => None,
            })
            .collect();
        assert_eq!(
            fills,
            [
                Paint::solid(Color::rgba(51, 51, 51, 255)),
                Paint::solid(Color::rgba(12, 34, 56, 255))
            ]
        );
    }

    #[test]
    fn icon_transform_preserves_svg_list_order_and_rotation_pivot() {
        let document=lower(r#"<g transform="translate(3,4) scale(2)"><path transform="rotate(90,1,2)" d="M0 0L1 1"/></g>"#,DrawingListLimits::default()).unwrap();
        let matrices: Vec<_> = document
            .commands
            .iter()
            .filter_map(|command| match command {
                DrawingCommand::ConcatTransform { transform } => Some(*transform),
                _ => None,
            })
            .collect();
        assert_eq!(matrices.len(), 5);
        assert_eq!((matrices[0].e, matrices[0].f), (3.0, 4.0));
        assert_eq!((matrices[1].a, matrices[1].d), (2.0, 2.0));
        assert_eq!((matrices[2].e, matrices[2].f), (1.0, 2.0));
        assert!((matrices[3].b - 1.0).abs() < 1e-12);
        assert_eq!((matrices[4].e, matrices[4].f), (-1.0, -2.0));
    }

    #[test]
    fn unrepresented_icon_effects_fail_closed() {
        for body in [
            r#"<g opacity="0.5"><circle r="2"/></g>"#,
            r#"<path filter="url(#blur)" d="M0 0L1 1"/>"#,
            r#"<path style="mix-blend-mode:multiply" d="M0 0L1 1"/>"#,
            r#"<path class="themed" d="M0 0L1 1"/>"#,
            "<title>Icon title</title>",
            "<desc>Icon description</desc>",
            "<text>label</text>",
            "<use href='#other'/>",
        ] {
            assert!(
                matches!(lower(body,DrawingListLimits::default()),Err(Error::DrawingListUnavailable {family,reason}) if family == "architecture" && reason.contains("icon asset")),
                "{body}"
            );
        }
        assert!(matches!(
            lower(
                r#"<path stroke="red" stroke-width="-1" d="M0 0L1 1"/>"#,
                DrawingListLimits::default()
            ),
            Err(Error::DrawingListUnavailable { .. })
        ));
    }

    #[test]
    fn icon_geometry_obeys_incremental_path_budget() {
        let error = lower(
            r#"<path d="M0 0L1 1L2 2L3 3"/>"#,
            DrawingListLimits {
                max_path_segments: 2,
                ..DrawingListLimits::default()
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            Error::DrawingListContract(merman_display_list::DrawingListError::ResourceLimit {
                resource: "path_segments",
                ..
            })
        ));
    }
}
