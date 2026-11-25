//! Admission and static projection for Mermaid Step edge animations.

use crate::{Error, Result};
use roxmltree::{Document, Node, NodeId};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

fn reject() -> Error {
    Error::svg_postprocess(
        "validate-edge-step-animation",
        "SVG contains an invalid Step animation contract",
    )
}

fn require(condition: bool) -> Result<()> {
    if condition { Ok(()) } else { Err(reject()) }
}

fn attribute<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str> {
    node.attribute(name).ok_or_else(reject)
}

fn attributes(node: Node<'_, '_>, names: &[&str]) -> Result<()> {
    require(
        node.attributes()
            .all(|attribute| attribute.namespace().is_none() && names.contains(&attribute.name())),
    )
}

fn is_svg(node: Node<'_, '_>, name: &str) -> bool {
    node.tag_name().name() == name
        && node
            .tag_name()
            .namespace()
            .is_none_or(|namespace| namespace == SVG_NAMESPACE)
}

fn number(value: &str) -> Result<f64> {
    require(
        value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || b".eE+-".contains(&byte)),
    )?;
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or_else(reject)
}

fn seconds(value: &str) -> Result<f64> {
    number(value.strip_suffix('s').ok_or_else(reject)?)
}

fn children<'a, 'input>(node: Node<'a, 'input>) -> Result<Vec<Node<'a, 'input>>> {
    let mut children = Vec::new();
    for child in node.children() {
        if child.is_element() {
            children.push(child);
        } else if child.is_text() {
            require(child.text().is_none_or(|text| text.trim().is_empty()))?;
        }
    }
    Ok(children)
}

fn validate_transform(node: Node<'_, '_>, motion: &str, duration: &str) -> Result<()> {
    require(is_svg(node, "animateTransform"))?;
    attributes(
        node,
        &["attributeName", "type", "values", "dur", "begin", "fill"],
    )?;
    require(
        attribute(node, "attributeName")? == "transform"
            && attribute(node, "type")? == "scale"
            && attribute(node, "values")? == "1;1"
            && attribute(node, "dur")? == duration
            && attribute(node, "begin")? == format!("{motion}.begin")
            && attribute(node, "fill")? == "remove"
            && children(node)?.is_empty(),
    )
}

/// Uses Mermaid's escaped ASCII identifier for one Step edge animation.
pub fn animation_id(value: &str) -> String {
    use std::fmt::Write;
    let mut output = String::from("step_");
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            output.push(character);
        } else {
            let _ = write!(output, "_{:x}_", character as u32);
        }
    }
    output
}

fn validate_path(
    path: &str,
    parked: &str,
    checkpoint: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    let mut first = None;
    for segment in svgtypes::PathParser::from(path) {
        checkpoint()?;
        let segment = segment.map_err(|_| reject())?;
        use svgtypes::PathSegment;
        let finite = match segment {
            PathSegment::MoveTo { x, y, .. }
            | PathSegment::LineTo { x, y, .. }
            | PathSegment::SmoothQuadratic { x, y, .. } => x.is_finite() && y.is_finite(),
            PathSegment::HorizontalLineTo { x, .. } => x.is_finite(),
            PathSegment::VerticalLineTo { y, .. } => y.is_finite(),
            PathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
                ..
            } => [x1, y1, x2, y2, x, y].iter().all(|value| value.is_finite()),
            PathSegment::SmoothCurveTo { x2, y2, x, y, .. } => {
                [x2, y2, x, y].iter().all(|value| value.is_finite())
            }
            PathSegment::Quadratic { x1, y1, x, y, .. } => {
                [x1, y1, x, y].iter().all(|value| value.is_finite())
            }
            PathSegment::EllipticalArc {
                rx,
                ry,
                x_axis_rotation,
                x,
                y,
                ..
            } => [rx, ry, x_axis_rotation, x, y]
                .iter()
                .all(|value| value.is_finite()),
            PathSegment::ClosePath { .. } => true,
        };
        require(finite)?;
        if first.is_none() {
            let PathSegment::MoveTo { x, y, .. } = segment else {
                return Err(reject());
            };
            first = Some((x, y));
        }
    }
    let (x, y) = first.ok_or_else(reject)?;
    if parked == "scale(0)" {
        return Ok(());
    }
    let translation = parked
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(") scale(0)"))
        .ok_or_else(reject)?;
    let (park_x, park_y) = translation.split_once(',').ok_or_else(reject)?;
    let coordinate = |value: &str| value.parse::<f64>().ok().filter(|value| value.is_finite());
    require(coordinate(park_x) == Some(x) && coordinate(park_y) == Some(y))
}

pub fn validate(
    document: &Document<'_>,
    checkpoint: &mut dyn FnMut() -> Result<()>,
) -> Result<HashSet<NodeId>> {
    checkpoint()?;
    let mut allowed = HashSet::new();
    let mut ids = HashMap::new();
    let root = document.root_element();
    let mut forbidden_containers = vec![false; document.descendants().len()];
    for node in document.descendants().filter(Node::is_element) {
        checkpoint()?;
        let inherited = node
            .parent()
            .is_some_and(|parent| forbidden_containers[parent.id().get_usize()]);
        forbidden_containers[node.id().get_usize()] =
            inherited || (node != root && (is_svg(node, "svg") || is_svg(node, "foreignObject")));
        if let Some(id) = node.attribute("id") {
            *ids.entry(id).or_insert(0usize) += 1;
        }
    }
    let mut common_timing = None;
    for dot in document
        .descendants()
        .filter(|node| node.attribute("class") == Some("edge-step-dot"))
    {
        checkpoint()?;
        require(is_svg(dot, "circle"))?;
        attributes(
            dot,
            &[
                "class",
                "r",
                "aria-hidden",
                "transform",
                "data-step",
                "fill",
                "stroke",
            ],
        )?;
        require(attribute(dot, "r")? == "4" && attribute(dot, "aria-hidden")? == "true")?;
        let parent = dot.parent_element().ok_or_else(reject)?;
        require(
            is_svg(parent, "g")
                && parent.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "edgePaths")
                }),
        )?;
        require(!forbidden_containers[dot.id().get_usize()])?;
        let highlight = dot.prev_sibling_element().ok_or_else(reject)?;
        require(
            is_svg(highlight, "path")
                && highlight.attribute("class") == Some("edge-step-highlight"),
        )?;
        attributes(
            highlight,
            &[
                "class",
                "aria-hidden",
                "d",
                "fill",
                "stroke",
                "stroke-width",
                "stroke-linecap",
                "stroke-opacity",
                "pointer-events",
                "transform",
                "data-step",
            ],
        )?;
        require(
            attribute(highlight, "aria-hidden")? == "true"
                && attribute(highlight, "fill")? == "none"
                && attribute(highlight, "stroke-width")? == "4"
                && attribute(highlight, "stroke-linecap")? == "round"
                && attribute(highlight, "stroke-opacity")? == "0.6"
                && attribute(highlight, "pointer-events")? == "none",
        )?;
        let edge = highlight.prev_sibling_element().ok_or_else(reject)?;
        require(
            is_svg(edge, "path")
                && edge.attribute("data-edge") == Some("true")
                && edge.attribute("id").is_some(),
        )?;
        let step = number(attribute(dot, "data-step")?)?;
        require(
            step > 0.0
                && step.fract() == 0.0
                && attribute(highlight, "data-step")? == attribute(dot, "data-step")?,
        )?;
        let dot_children = children(dot)?;
        let highlight_children = children(highlight)?;
        require(dot_children.len() == 2 && highlight_children.len() == 1)?;
        let motion = dot_children[0];
        require(is_svg(motion, "animateMotion"))?;
        attributes(motion, &["id", "path", "dur", "begin", "fill"])?;
        let motion_id = attribute(motion, "id")?;
        require(
            motion_id
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
                && motion_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
                && ids.get(motion_id) == Some(&1)
                && children(motion)?.is_empty()
                && attribute(motion, "fill")? == "remove",
        )?;
        let prefix = &motion_id[..motion_id.rfind('-').map_or(0, |index| index + 1)];
        let edge_id = attribute(edge, "id")?;
        let original_edge = edge_id.strip_prefix(prefix).ok_or_else(reject)?;
        require(
            motion_id == format!("{prefix}{}", animation_id(&format!("{original_edge}-step"))),
        )?;
        let root_id = attribute(root, "id")?;
        require(
            edge_id.starts_with(&format!("{root_id}-"))
                && ids.get(edge_id) == Some(&1)
                && ids.get(root_id) == Some(&1),
        )?;
        let duration_text = attribute(motion, "dur")?;
        let duration = seconds(duration_text)?;
        require(duration > 0.0)?;
        let timing = attribute(motion, "begin")?;
        let (begin, gap) = timing
            .split_once(&format!(";{motion_id}.end+"))
            .ok_or_else(reject)?;
        let begin = seconds(begin)?;
        let gap = seconds(gap)?;
        let count = gap / duration + 1.0;
        let count_tolerance = 4.0 * f64::EPSILON * count;
        require(
            begin == (step - 1.0) * duration
                && count.is_finite()
                && (gap + duration).is_finite()
                && gap >= begin
                && (count - count.round()).abs() <= count_tolerance,
        )?;
        if let Some((common_duration, common_gap)) = common_timing {
            require((duration, gap) == (common_duration, common_gap))?;
        }
        common_timing = Some((duration, gap));
        let path = attribute(motion, "path")?;
        let parked = attribute(dot, "transform")?;
        require(
            attribute(highlight, "d")? == path && attribute(highlight, "transform")? == parked,
        )?;
        if let Some(fill) = dot.attribute("fill") {
            require(
                dot.attribute("stroke") == Some("none")
                    && highlight.attribute("stroke") == Some(fill),
            )?;
        } else {
            require(
                dot.attribute("stroke").is_none()
                    && highlight.attribute("stroke") == Some("currentColor"),
            )?;
        }
        validate_path(path, parked, checkpoint)?;
        validate_transform(dot_children[1], motion_id, duration_text)?;
        validate_transform(highlight_children[0], motion_id, duration_text)?;
        allowed.extend([
            dot.id(),
            highlight.id(),
            motion.id(),
            dot_children[1].id(),
            highlight_children[0].id(),
        ]);
    }
    for node in document.descendants().filter(Node::is_element) {
        checkpoint()?;
        let name = node.tag_name().name();
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "animatemotion" | "animatetransform"
        ) || node
            .attribute("class")
            .is_some_and(|class| matches!(class, "edge-step-dot" | "edge-step-highlight"))
        {
            require(allowed.contains(&node.id()))?;
        }
    }
    Ok(allowed)
}

pub fn project_static<'a>(
    svg: Cow<'a, str>,
    checkpoint: &mut dyn FnMut() -> Result<()>,
) -> Result<Cow<'a, str>> {
    if !svg.contains("edge-step-") {
        return Ok(svg);
    }
    checkpoint()?;
    let document = Document::parse(&svg).map_err(|_| reject())?;
    let allowed = validate(&document, checkpoint)?;
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for node in document.descendants().filter(Node::is_element) {
        checkpoint()?;
        if (allowed.contains(&node.id())
            && node
                .attribute("class")
                .is_some_and(|class| matches!(class, "edge-step-dot" | "edge-step-highlight")))
            || (is_svg(node, "style") && node.attribute("class") == Some("edge-step-style"))
        {
            let range = node.range();
            require(range.start < range.end && svg.get(range.clone()).is_some())?;
            if let Some(previous) = ranges.last() {
                require(range.start >= previous.start)?;
                if range.start < previous.end {
                    require(range.end <= previous.end)?;
                    continue;
                }
            }
            ranges.push(range);
        }
    }
    if ranges.is_empty() {
        return Ok(svg);
    }
    let mut output = String::with_capacity(svg.len());
    let mut cursor = 0;
    for range in ranges {
        checkpoint()?;
        output.push_str(svg.get(cursor..range.start).ok_or_else(reject)?);
        cursor = range.end;
    }
    output.push_str(svg.get(cursor..).ok_or_else(reject)?);
    checkpoint()?;
    Ok(Cow::Owned(output))
}
