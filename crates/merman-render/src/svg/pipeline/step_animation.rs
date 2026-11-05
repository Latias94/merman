use super::SvgPostprocessExecution;
use crate::xml_dom::{Document, Node, NodeId};
use crate::{Error, Result};
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

fn animation_id(value: &str) -> String {
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

fn validate_path(path: &str, parked: &str, execution: SvgPostprocessExecution<'_>) -> Result<()> {
    let mut first = None;
    for segment in svgtypes::PathParser::from(path) {
        execution.checkpoint()?;
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

pub(super) fn validate(
    document: &Document<'_>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<HashSet<NodeId>> {
    execution.checkpoint()?;
    let mut allowed = HashSet::new();
    let mut ids = HashMap::new();
    let root = document.root_element();
    let mut forbidden_containers = vec![false; document.descendants().len()];
    for node in document.descendants().filter(Node::is_element) {
        execution.checkpoint()?;
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
        execution.checkpoint()?;
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
        validate_path(path, parked, execution)?;
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
        execution.checkpoint()?;
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

pub(super) fn project_static<'a>(
    svg: Cow<'a, str>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    if !svg.contains("edge-step-") {
        return Ok(svg);
    }
    execution.checkpoint()?;
    let document = Document::parse(&svg).map_err(|_| reject())?;
    let allowed = validate(&document, execution)?;
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for node in document.descendants().filter(Node::is_element) {
        execution.checkpoint()?;
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
        execution.checkpoint()?;
        output.push_str(svg.get(cursor..range.start).ok_or_else(reject)?);
        cursor = range.end;
    }
    output.push_str(svg.get(cursor..).ok_or_else(reject)?);
    execution.checkpoint()?;
    Ok(Cow::Owned(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::SvgPipeline;

    fn fixture() -> String {
        let motion = animation_id("diagram-edge-step");
        let transform = format!(
            r#"<animateTransform attributeName="transform" type="scale" values="1;1" dur="0.25s" begin="{motion}.begin" fill="remove"/>"#
        );
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram"><g class="edgePaths"><path id="diagram-edge" data-edge="true" d="M0,0L100,0"/><path class="edge-step-highlight" aria-hidden="true" d="M0,0L100,0" fill="none" stroke="#333333" stroke-width="4" stroke-linecap="round" stroke-opacity="0.6" pointer-events="none" transform="translate(0,0) scale(0)" data-step="3">{transform}</path><circle class="edge-step-dot" r="4" aria-hidden="true" transform="translate(0,0) scale(0)" data-step="3" fill="#333333" stroke="none"><animateMotion id="{motion}" path="M0,0L100,0" dur="0.25s" begin="0.5s;{motion}.end+1s" fill="remove"/>{transform}</circle></g><g class="edge-step-badges"><g class="edge-step-badge"><circle r="9"/><text>3</text></g></g></svg>"##
        )
    }

    #[test]
    fn browser_admission_and_static_projection_are_distinct_closed_contracts() {
        let source = fixture();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let browser = SvgPipeline::parity()
            .with_browser_inline_contract("embed-step_bad")
            .process_to_string(&source, &session)
            .unwrap();
        assert!(browser.contains("animateMotion"));
        super::super::validate_static_inline_svg_admission(&browser, &session).unwrap();
        assert!(super::super::validate_static_inline_svg(&browser, &session).is_err());
        for pipeline in [
            SvgPipeline::resvg_safe(),
            SvgPipeline::parity().with_static_inline_contract("static"),
        ] {
            let output = pipeline.process_to_string(&source, &session).unwrap();
            assert!(!output.contains("animateMotion"));
            assert!(!output.contains("edge-step-highlight"));
            assert!(!output.contains("edge-step-dot"));
            assert!(output.contains("edge-step-badge"));
            assert!(output.contains("data-edge=\"true\""));
        }
    }

    #[test]
    fn static_step_projection_handles_nested_style_ranges_without_panicking() {
        let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style class="edge-step-style"><style class="edge-step-style"/></style></svg>"#;
        assert_eq!(source.len(), 117);
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for pipeline in [
            SvgPipeline::resvg_safe(),
            SvgPipeline::parity().with_static_inline_contract("static"),
        ] {
            let output = pipeline.process_to_string(source, &session).unwrap();
            assert!(!output.contains("edge-step-style"));
            super::super::validate_static_inline_svg(&output, &session).unwrap();
        }
    }

    #[test]
    fn static_step_projection_preserves_siblings_and_rejects_invalid_nested_content() {
        let styles = r#"<style class="edge-step-style"><style class="edge-step-style"/></style>"#;
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="before"/>{styles}<text>after</text></svg>"#
        );
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for pipeline in [
            SvgPipeline::resvg_safe(),
            SvgPipeline::parity().with_static_inline_contract("static"),
        ] {
            let output = pipeline.process_to_string(&source, &session).unwrap();
            assert!(output.contains("before"));
            assert!(output.contains(">after</text>"));
            assert!(!output.contains("edge-step-style"));
            for invalid in [
                source.replace("</svg>", ""),
                source.replace(
                    styles,
                    r##"<style class="edge-step-style"><animateMotion href="#before"/></style>"##,
                ),
            ] {
                assert!(pipeline.process_to_string(&invalid, &session).is_err());
            }
        }
    }

    #[test]
    fn browser_step_admission_handles_deep_parallel_edges_and_rejects_nested_svg() {
        let source = fixture();
        let edges = source
            .split_once("<g class=\"edgePaths\">")
            .unwrap()
            .1
            .split_once("</g>")
            .unwrap()
            .0;
        let motion = animation_id("diagram-edge-step");
        let mut parallel = String::new();
        for index in 0..128 {
            let edge_id = format!("diagram-edge{index}");
            parallel.push_str(
                &edges
                    .replace("diagram-edge", &edge_id)
                    .replace(&motion, &animation_id(&format!("{edge_id}-step"))),
            );
        }
        let nested = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram">{}<g class="edgePaths">{parallel}</g>{}</svg>"#,
            "<g>".repeat(2048),
            "</g>".repeat(2048)
        );
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        super::super::validate_static_inline_svg_admission(&nested, &session).unwrap();
        for (open, close) in [
            ("<svg>", "</svg>"),
            (
                r#"<s:svg xmlns:s="http://www.w3.org/2000/svg">"#,
                "</s:svg>",
            ),
            ("<foreignObject>", "</foreignObject>"),
        ] {
            let invalid = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram">{open}<g class="edgePaths">{parallel}</g>{close}</svg>"#
            );
            assert!(
                super::super::validate_static_inline_svg_admission(&invalid, &session).is_err()
            );
        }
    }

    #[test]
    fn step_decimal_timing_accepts_large_counts_and_rejects_fractional_cycles() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let pipeline = SvgPipeline::parity().with_browser_inline_contract("embed");
        for (step, begin) in [("1", "0s"), ("999999999", "99999999.80000001s")] {
            let source = fixture()
                .replace("data-step=\"3\"", &format!("data-step=\"{step}\""))
                .replace("dur=\"0.25s\"", "dur=\"0.1s\"")
                .replace("begin=\"0.5s;", &format!("begin=\"{begin};"))
                .replace(".end+1s", ".end+99999999.80000001s");
            pipeline.process_to_string(&source, &session).unwrap();
            let fractional = source.replace(".end+99999999.80000001s", ".end+99999999.825s");
            assert!(pipeline.process_to_string(&fractional, &session).is_err());
            if step == "999999999" {
                let early = source.replace(".end+99999999.80000001s", ".end+99999999.7s");
                assert!(pipeline.process_to_string(&early, &session).is_err());
            }
        }
        let distant = fixture()
            .replace("data-step=\"3\"", "data-step=\"10000000000000000\"")
            .replace("dur=\"0.25s\"", "dur=\"0.1s\"")
            .replace("begin=\"0.5s;", "begin=\"1000000000000000s;")
            .replace(".end+1s", ".end+1000000000000000s");
        pipeline.process_to_string(&distant, &session).unwrap();
        let early = distant.replace(".end+1000000000000000s", ".end+999999999999999.8s");
        assert!(pipeline.process_to_string(&early, &session).is_err());
    }

    #[test]
    fn step_rejects_matching_malformed_and_nonfinite_paths() {
        let source = fixture();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let pipeline = SvgPipeline::parity().with_browser_inline_contract("embed");
        pipeline.process_to_string(&source, &session).unwrap();
        for path in [
            "javascript:alert(1)",
            "M0,0L1e999,0",
            "M0,0L1",
            "M0,0A2,2,0,2,0,4,4",
        ] {
            let invalid = source.replace("M0,0L100,0", path);
            assert_ne!(invalid, source);
            assert!(
                pipeline.process_to_string(&invalid, &session).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn step_rejects_active_targeting_timing_and_path_variants() {
        let source = fixture();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for (from, to) in [
            ("attributeName=\"transform\"", "attributeName=\"href\""),
            ("type=\"scale\"", "type=\"translate\""),
            ("values=\"1;1\"", "values=\"1;100\""),
            ("<animateMotion ", "<animateMotion href=\"#other\" "),
            ("<animateMotion ", "<animateMotion onbegin=\"alert(1)\" "),
            (
                "<animateMotion ",
                "<animateMotion repeatCount=\"indefinite\" ",
            ),
            ("path=\"M0,0L100,0\"", "path=\"javascript:alert(1)\""),
            ("path=\"M0,0L100,0\"", "path=\"M0,0L1e999,0\""),
            ("begin=\"0.5s;", "begin=\"click;"),
            ("begin=\"0.5s;", "begin=\"0.6s;"),
            (".end+1s", ".end+0.1s"),
            (".begin\"", ".click\""),
            ("dur=\"0.25s\"", "dur=\"0s\""),
            ("fill=\"remove\"", "fill=\"freeze\""),
            ("id=\"diagram\"", "id=\"other\""),
            ("class=\"edgePaths\"", "class=\"other\""),
            ("</svg>", "<animateMotion path=\"M0,0L1,1\"/></svg>"),
            ("data-edge=\"true\"", "data-edge=\"false\""),
        ] {
            let attack = source.replacen(from, to, 1);
            assert!(
                SvgPipeline::parity()
                    .with_browser_inline_contract("embed")
                    .process_to_string(&attack, &session)
                    .is_err(),
                "{from} -> {to}"
            );
        }
    }
}
