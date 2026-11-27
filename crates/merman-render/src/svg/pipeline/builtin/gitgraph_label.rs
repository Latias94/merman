use crate::Result;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};

use crate::DiagramFamilyId;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessExecution, SvgPostprocessor};
use merman_render_core::svg_output::{BoundedSvgOutput, SvgOutput};
use merman_render_core::svg_scan::{
    SvgTag, SvgTagScanner, extract_quoted_attr_with_checkpoints, find_with_checkpoints,
    next_svg_quoted_attr_with_checkpoints, start_tag_name,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct GitGraphBranchLabelBaselinePostprocessor;

impl SvgPostprocessor for GitGraphBranchLabelBaselinePostprocessor {
    fn name(&self) -> &'static str {
        "gitgraph-branch-label-baseline"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if ctx.family_id() != Some(DiagramFamilyId::GIT_GRAPH) {
            return Ok(svg);
        }
        center_gitgraph_branch_label_text(svg, ctx.execution())
    }
}

fn center_gitgraph_branch_label_text<'a>(
    svg: Cow<'a, str>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    let mut checkpoint = || execution.checkpoint();
    if find_with_checkpoints(&svg, "branch-label", &mut checkpoint)?.is_none() {
        return Ok(svg);
    }
    let mut centers = collect_branch_label_centers(&svg, &mut checkpoint)?;
    if centers.is_empty() {
        return Ok(svg);
    }

    let mut out = BoundedSvgOutput::new(execution.work_meter());
    let mut scanner = SvgTagScanner::new(&svg);
    let mut copied_until = 0;
    while let Some(group) = scanner.next_with_checkpoints(&mut checkpoint)? {
        if start_tag_name(group.raw()) != Some("g") {
            continue;
        }
        let Some(index) =
            extract_quoted_attr_with_checkpoints(group.raw(), "class", &mut checkpoint)?
                .and_then(branch_group_index)
        else {
            continue;
        };
        let Some(center_y) = centers.get_mut(&index).and_then(VecDeque::pop_front) else {
            continue;
        };
        let Some(group_end) = scanner.matching_end_with_checkpoints(group, &mut checkpoint)? else {
            break;
        };
        if group.is_self_closing() {
            continue;
        }
        let mut content = SvgTagScanner::new(&svg[..group_end.start()]);
        content.skip_to(group.end());
        let Some(text) = first_text_tag(&mut content, &mut checkpoint)? else {
            continue;
        };
        let Some(text_end) = content.matching_end_with_checkpoints(text, &mut checkpoint)? else {
            continue;
        };
        let group_y =
            extract_quoted_attr_with_checkpoints(group.raw(), "transform", &mut checkpoint)?
                .and_then(parse_translate_y)
                .unwrap_or(0.0);
        let text_y = fmt_number(center_y - group_y);
        out.push_str(&svg[copied_until..text.start()]);
        write_updated_tag(
            &mut out,
            text,
            [
                ("y", &text_y),
                ("dominant-baseline", "central"),
                ("alignment-baseline", "central"),
            ],
            &mut checkpoint,
        )?;
        let tspans = &svg[text.end()..text_end.start()];
        let (first_row, rows) = visible_tspan_rows(tspans, &mut checkpoint)?;
        if let Some(first_row) = first_row {
            let row_span = rows.saturating_sub(1);
            let dy = if row_span == 0 {
                "0".to_owned()
            } else {
                format!("{}em", fmt_number(-(row_span as f64) / 2.0))
            };
            out.push_str(&tspans[..first_row.start()]);
            write_updated_tag(&mut out, first_row, [("dy", &dy)], &mut checkpoint)?;
            out.push_str(&tspans[first_row.end()..]);
        } else {
            out.push_str(tspans);
        }
        out.checkpoint()?;
        copied_until = text_end.start();
    }
    if copied_until == 0 {
        return Ok(svg);
    }
    out.push_str(&svg[copied_until..]);
    let adjusted = out.finish()?;
    if adjusted == svg {
        Ok(svg)
    } else {
        Ok(Cow::Owned(adjusted))
    }
}

fn collect_branch_label_centers(
    svg: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<HashMap<usize, VecDeque<f64>>> {
    let mut centers = HashMap::new();
    let mut scanner = SvgTagScanner::new(svg);
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        if start_tag_name(tag.raw()) != Some("rect") {
            continue;
        }
        let Some(index) = extract_quoted_attr_with_checkpoints(tag.raw(), "class", checkpoint)?
            .and_then(branch_rect_index)
        else {
            continue;
        };
        let y =
            extract_quoted_attr_with_checkpoints(tag.raw(), "y", checkpoint)?.and_then(parse_f64);
        let height = extract_quoted_attr_with_checkpoints(tag.raw(), "height", checkpoint)?
            .and_then(parse_f64);
        if let (Some(y), Some(height)) = (y, height) {
            let translate_y =
                extract_quoted_attr_with_checkpoints(tag.raw(), "transform", checkpoint)?
                    .and_then(parse_translate_y)
                    .unwrap_or(0.0);
            centers
                .entry(index)
                .or_insert_with(VecDeque::new)
                .push_back(translate_y + y + height / 2.0);
        }
    }
    Ok(centers)
}

fn branch_rect_index(classes: &str) -> Option<usize> {
    if !classes
        .split_whitespace()
        .any(|class| class == "branchLabelBkg")
    {
        return None;
    }
    label_class_index(classes)
}

fn branch_group_index(classes: &str) -> Option<usize> {
    if !classes.split_whitespace().any(|class| class == "label") {
        return None;
    }
    classes
        .split_whitespace()
        .find_map(|class| class.strip_prefix("branch-label"))
        .and_then(|value| value.parse::<usize>().ok())
}

fn label_class_index(classes: &str) -> Option<usize> {
    classes
        .split_whitespace()
        .find_map(|class| class.strip_prefix("label"))
        .and_then(|value| value.parse::<usize>().ok())
}

fn first_text_tag<'a>(
    scanner: &mut SvgTagScanner<'a>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Option<SvgTag<'a>>> {
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        if start_tag_name(tag.raw()) == Some("text") && !tag.is_self_closing() {
            return Ok(Some(tag));
        }
    }
    Ok(None)
}

fn visible_tspan_rows<'a>(
    content: &'a str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<(Option<SvgTag<'a>>, usize)> {
    let mut scanner = SvgTagScanner::new(content);
    let mut first = None;
    let mut rows = 0;
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        if start_tag_name(tag.raw()) != Some("tspan") || tag.is_self_closing() {
            continue;
        }
        let Some(close) = scanner.matching_end_with_checkpoints(tag, checkpoint)? else {
            break;
        };
        // Preserve authored spaces, including rows with xml:space="preserve".
        if has_text_content(&content[tag.end()..close.start()], checkpoint)? {
            first.get_or_insert(tag);
            rows += 1;
        }
    }
    Ok((first, rows))
}

fn has_text_content(content: &str, checkpoint: &mut impl FnMut() -> Result<()>) -> Result<bool> {
    let mut scanner = SvgTagScanner::new(content);
    let mut cursor = 0;
    while let Some(tag) = scanner.next_with_checkpoints(checkpoint)? {
        if tag.start() > cursor
            || tag
                .raw()
                .strip_prefix("<![CDATA[")
                .and_then(|body| body.strip_suffix("]]>"))
                .is_some_and(|body| !body.is_empty())
        {
            return Ok(true);
        }
        cursor = tag.end();
    }
    Ok(cursor < content.len())
}

fn write_updated_tag<const N: usize>(
    out: &mut BoundedSvgOutput<'_>,
    tag: SvgTag<'_>,
    attributes: [(&str, &str); N],
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
    let raw = tag.raw();
    let mut written = [false; N];
    let mut cursor = 0;
    let mut copied = 0;
    while let Some(attr) = next_svg_quoted_attr_with_checkpoints(raw, cursor, checkpoint)? {
        cursor = attr.full_end;
        let name = &raw[attr.name_start..attr.name_end];
        if let Some(index) = attributes
            .iter()
            .position(|(key, _)| name.eq_ignore_ascii_case(key))
        {
            out.push_str(&raw[copied..attr.value_start]);
            out.push_str(attributes[index].1);
            copied = attr.value_end;
            written[index] = true;
        }
    }
    let end = raw.len() - 1;
    out.push_str(&raw[copied..end]);
    for ((name, value), written) in attributes.into_iter().zip(written) {
        if !written {
            out.push(' ');
            out.push_str(name);
            out.push_str("=\"");
            out.push_str(value);
            out.push('"');
        }
    }
    out.push('>');
    out.checkpoint()
}

fn parse_translate_y(transform: &str) -> Option<f64> {
    let lower = transform.to_ascii_lowercase();
    let start = lower.find("translate(")? + "translate(".len();
    let end = transform[start..].find(')')? + start;
    let args = &transform[start..end];
    let mut parts = args
        .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
        .filter(|part| !part.trim().is_empty());
    let _x = parts.next()?;
    parts.next().and_then(parse_f64).or(Some(0.0))
}

fn parse_f64(raw: &str) -> Option<f64> {
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn fmt_number(value: f64) -> String {
    let value = if value.abs() < 1e-9 { 0.0 } else { value };
    if (value.fract()).abs() < 1e-9 {
        return format!("{}", value as i64);
    }

    let mut out = format!("{value:.6}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::{SvgPipeline, SvgPostprocessMetadata};

    fn render_session() -> crate::environment::RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn gitgraph_branch_label_text_uses_rect_center_baseline() {
        let svg = r#"<svg id="g"><g><rect class="branchLabelBkg label0" rx="4" ry="4" x="-69" y="-1.5" width="53" height="21" transform="translate(-19, -8.5)"/><g class="branchLabel"><g class="label branch-label0" transform="translate(-79, -9.5)"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">main</tspan></text></g></g></g></svg>"#;
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(DiagramFamilyId::GIT_GRAPH);
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(GitGraphBranchLabelBaselinePostprocessor)
            .process_to_string_with_metadata(svg, &metadata, &session)
            .unwrap();

        assert!(out.contains(r#"<text"#), "{out}");
        assert!(out.contains(r#"y="10""#), "{out}");
        assert!(out.contains(r#"dominant-baseline="central""#), "{out}");
        assert!(out.contains(r#"alignment-baseline="central""#), "{out}");
        assert!(
            out.contains(r#"<tspan xml:space="preserve" dy="0" x="0" class="row">main</tspan>"#)
        );
    }

    #[test]
    fn gitgraph_multiline_branch_label_centering_is_idempotent_with_empty_rows() {
        let svg = r#"<svg id="g"><g><rect class="branchLabelBkg label0" y="0" height="40"/><g class="label branch-label0"><text><tspan dy="1em"></tspan><tspan dy="1em">first</tspan><tspan dy="1em"></tspan><tspan dy="1em">last</tspan><tspan dy="1em"></tspan></text></g></g></svg>"#;
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(DiagramFamilyId::GIT_GRAPH);
        let session = render_session();
        let pipeline =
            SvgPipeline::parity().with_postprocessor(GitGraphBranchLabelBaselinePostprocessor);
        let centered = pipeline
            .process_to_string_with_metadata(svg, &metadata, &session)
            .unwrap();
        assert_eq!(centered.matches(r#"dy="-0.5em""#).count(), 1);
        assert_eq!(
            pipeline
                .process_to_string_with_metadata(&centered, &metadata, &session)
                .unwrap(),
            centered,
        );
    }

    #[test]
    fn gitgraph_branch_label_uses_real_nested_group_and_tspan_boundaries() {
        let svg = concat!(
            r#"<svg><!-- <rect class='branchLabelBkg label0' y='0' height='100'/> -->"#,
            r#"<rect class='branchLabelBkg label0' y='0' height='40'/>"#,
            r#"<g class='label branch-label0' data-note='a > b'><g/>"#,
            r#"<g><!-- </g><text>fake</text> --></g >"#,
            r#"<text><tspan dy='1em'/><!-- <tspan>fake</tspan> -->"#,
            r#"<tspan dy='1em'>first</tspan ><tspan dy='1em'>last</tspan></text ></g ></svg>"#,
        );
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(DiagramFamilyId::GIT_GRAPH);
        let out = SvgPipeline::parity()
            .with_postprocessor(GitGraphBranchLabelBaselinePostprocessor)
            .process_to_string_with_metadata(svg, &metadata, &render_session())
            .unwrap();
        let doc = crate::xml_dom::Document::parse(&out).unwrap();
        let text = doc
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("y"), Some("20"));
        let rows: Vec<_> = text
            .children()
            .filter(|node| node.has_tag_name("tspan"))
            .map(|node| node.attribute("dy"))
            .collect();
        assert_eq!(rows, [Some("1em"), Some("-0.5em"), Some("1em")]);
        assert!(out.contains("<!-- </g><text>fake</text> -->"));
    }

    #[test]
    fn gitgraph_branch_label_postprocessor_ignores_other_diagrams() {
        let svg = r#"<svg id="g"><g><rect class="branchLabelBkg label0" y="-1.5" height="21"/><g class="label branch-label0"><text><tspan dy="1em">main</tspan></text></g></g></svg>"#;
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(DiagramFamilyId::FLOWCHART);
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(GitGraphBranchLabelBaselinePostprocessor)
            .process_to_string_with_metadata(svg, &metadata, &session)
            .unwrap();

        assert_eq!(out, svg);
    }

    #[test]
    fn gitgraph_branch_label_uses_shared_quoted_attr_scanner() {
        let svg = r#"<svg id="g"><g><rect class = 'branchLabelBkg label0' rx="4" ry="4" x="-69" y='-1.5' width="53" height='21' transform = 'translate(-19, -8.5)'/><g class="branchLabel"><g class = 'label branch-label0' transform = 'translate(-79, -9.5)'><text y='0'><tspan xml:space="preserve" dy='1em' x="0" class="row">main</tspan></text></g></g></g></svg>"#;
        let metadata =
            SvgPostprocessMetadata::from_svg(svg).with_family_id(DiagramFamilyId::GIT_GRAPH);
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(GitGraphBranchLabelBaselinePostprocessor)
            .process_to_string_with_metadata(svg, &metadata, &session)
            .unwrap();

        assert!(out.contains(r#"<text y='10'"#), "{out}");
        assert!(out.contains(r#"dy='0'"#), "{out}");
        assert!(out.contains(r#"dominant-baseline="central""#), "{out}");
    }
}
