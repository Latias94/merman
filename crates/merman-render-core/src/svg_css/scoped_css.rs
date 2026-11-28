use super::css_override::{CssOverridePolicy, strip_css_important};
use super::postprocess::PostprocessWork;
use crate::resources::OperationWorkMeter;
use crate::svg_scan::{
    checkpoint_loop, find_tag_end_with_checkpoints, find_with_checkpoints, rfind_with_checkpoints,
    trim_with_checkpoints,
};
use crate::{Error, Result};

use std::borrow::Cow;

mod rewrite;

const STYLE_OPEN: &str = r#"<style data-merman-postprocess="scoped-css">"#;
const STYLE_CLOSE: &str = "</style>";

/// Scopes and inserts CSS using the original SVG document and operation meter.
/// The caller owns final XML and resource admission.
pub fn apply_scoped_css<'a>(
    svg: Cow<'a, str>,
    css: &str,
    svg_id: Option<&str>,
    override_policy: CssOverridePolicy,
    merge_into_existing_style: bool,
    work_meter: &OperationWorkMeter,
) -> Result<Cow<'a, str>> {
    let execution = PostprocessWork(work_meter);
    execution.checkpoint()?;
    let mut checkpoint = || execution.checkpoint();
    if trim_with_checkpoints(css, &mut checkpoint)?.is_empty() {
        return Ok(svg);
    }

    let base = match override_policy {
        CssOverridePolicy::Preserve => svg,
        CssOverridePolicy::StripExistingImportant => strip_css_important(svg, work_meter)?,
    };
    let css = decode_mermaid_css_hash_placeholders(css, execution)?;
    let scope = css_scope(svg_id, execution)?;
    let plan = InjectionPlan::locate(base.as_ref(), merge_into_existing_style, execution)?;

    let projected_css_bytes =
        rewrite::projected_css_bytes(css.as_ref(), scope.as_deref(), execution)?;
    let projected_svg_bytes = plan
        .projected_svg_bytes(base.len(), projected_css_bytes)
        .ok_or_else(|| execution.svg_byte_count_overflow())?;
    execution.preflight_svg_byte_count(projected_svg_bytes)?;
    execution.checkpoint()?;

    let mut output = String::new();
    output
        .try_reserve_exact(projected_svg_bytes)
        .map_err(|error| {
            Error::svg_postprocess(
                "scoped-css",
                format!("failed to allocate scoped SVG: {error}"),
            )
        })?;
    output.push_str(&base[..plan.insertion]);
    if plan.wrapped {
        output.push_str(STYLE_OPEN);
    }
    rewrite::materialize_css(css.as_ref(), scope.as_deref(), &mut output, execution)?;
    if plan.wrapped {
        output.push_str(STYLE_CLOSE);
    }
    output.push_str(&base[plan.insertion..]);
    execution.checkpoint()?;
    if output.len() != projected_svg_bytes {
        return Err(Error::svg_postprocess(
            "scoped-css",
            "scoped SVG byte projection changed during materialization",
        ));
    }
    Ok(Cow::Owned(output))
}

fn decode_mermaid_css_hash_placeholders<'a>(
    css: &'a str,
    execution: PostprocessWork<'_>,
) -> Result<Cow<'a, str>> {
    let mut checkpoint = || execution.checkpoint();
    let mut has_placeholder = false;
    for (iteration, character) in css.chars().enumerate() {
        checkpoint_loop(iteration, &mut checkpoint)?;
        if matches!(character, 'ﬂ' | '¶') {
            has_placeholder = true;
            break;
        }
    }
    checkpoint()?;
    if !has_placeholder {
        return Ok(Cow::Borrowed(css));
    }

    let mut decoded = String::new();
    decoded.try_reserve_exact(css.len()).map_err(|error| {
        Error::svg_postprocess(
            "scoped-css",
            format!("failed to allocate decoded scoped CSS: {error}"),
        )
    })?;
    let mut cursor = 0usize;
    let mut iteration = 0usize;
    while cursor < css.len() {
        checkpoint_loop(iteration, &mut checkpoint)?;
        iteration = iteration.wrapping_add(1);
        let tail = &css[cursor..];
        if tail.starts_with("ﬂ°°") {
            decoded.push('#');
            cursor += "ﬂ°°".len();
        } else if tail.starts_with("ﬂ°") {
            decoded.push('#');
            cursor += "ﬂ°".len();
        } else if tail.starts_with("¶ß") {
            decoded.push(';');
            cursor += "¶ß".len();
        } else {
            let character = tail
                .chars()
                .next()
                .expect("cursor remains on a UTF-8 character boundary");
            decoded.push(character);
            cursor += character.len_utf8();
        }
    }
    checkpoint()?;
    Ok(Cow::Owned(decoded))
}

fn css_scope(svg_id: Option<&str>, execution: PostprocessWork<'_>) -> Result<Option<String>> {
    let Some(svg_id) = svg_id else {
        return Ok(None);
    };
    let mut checkpoint = || execution.checkpoint();
    let svg_id = trim_with_checkpoints(svg_id, &mut checkpoint)?;
    if svg_id.is_empty() {
        return Ok(None);
    }

    let mut checkpoint = || execution.checkpoint();
    let mut escaped_bytes = 1usize;
    for (iteration, character) in svg_id.chars().enumerate() {
        checkpoint_loop(iteration, &mut checkpoint)?;
        escaped_bytes = escaped_bytes
            .checked_add(if is_css_identifier_character(character) {
                character.len_utf8()
            } else {
                1 + character.len_utf8()
            })
            .ok_or_else(|| execution.svg_byte_count_overflow())?;
    }
    checkpoint()?;
    let mut scope = String::new();
    scope.try_reserve_exact(escaped_bytes).map_err(|error| {
        Error::svg_postprocess(
            "scoped-css",
            format!("failed to allocate scoped CSS root selector: {error}"),
        )
    })?;
    scope.push('#');
    for (iteration, character) in svg_id.chars().enumerate() {
        checkpoint_loop(iteration, &mut checkpoint)?;
        if !is_css_identifier_character(character) {
            scope.push('\\');
        }
        scope.push(character);
    }
    checkpoint()?;
    Ok(Some(scope))
}

fn is_css_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '-' || character == '_'
}

#[derive(Debug, Clone, Copy)]
struct InjectionPlan {
    insertion: usize,
    wrapped: bool,
}

impl InjectionPlan {
    fn locate(
        svg: &str,
        merge_into_existing_style: bool,
        execution: PostprocessWork<'_>,
    ) -> Result<Self> {
        let mut checkpoint = || execution.checkpoint();
        if merge_into_existing_style
            && let Some(insertion) = find_with_checkpoints(svg, "</style", &mut checkpoint)?
        {
            return Ok(Self {
                insertion,
                wrapped: false,
            });
        }

        if let Some(root_start) = find_with_checkpoints(svg, "<svg", &mut checkpoint)?
            && let Some(root_end) = find_tag_end_with_checkpoints(svg, root_start, &mut checkpoint)?
        {
            if let Some(style_start) = rfind_with_checkpoints(svg, "</style", &mut checkpoint)?
                && let Some(style_end) =
                    find_tag_end_with_checkpoints(svg, style_start, &mut checkpoint)?
            {
                return Ok(Self {
                    insertion: style_end + 1,
                    wrapped: true,
                });
            }
            return Ok(Self {
                insertion: root_end + 1,
                wrapped: true,
            });
        }

        Ok(Self {
            insertion: svg.len(),
            wrapped: true,
        })
    }

    fn projected_svg_bytes(self, svg_bytes: usize, css_bytes: usize) -> Option<usize> {
        let wrapper_bytes = if self.wrapped {
            STYLE_OPEN.len() + STYLE_CLOSE.len()
        } else {
            0
        };
        svg_bytes.checked_add(css_bytes)?.checked_add(wrapper_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use merman_model::{CancelReason, OperationControl, OperationPhase};

    fn scoped_selector(selector: &str) -> String {
        let work = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        let execution = PostprocessWork(&work);
        let mut output = String::new();
        rewrite::materialize_css(
            &format!("{selector}{{color:red;}}"),
            Some("#diagram"),
            &mut output,
            execution,
        )
        .expect("valid scoped selector");
        output
            .split_once(" {")
            .map(|(selector, _)| selector.to_string())
            .expect("materialized selector and declaration block")
    }

    #[test]
    fn scoped_css_matches_mermaid_namespace_boundary_rules() {
        let cases = [
            ("& ~ *", "color: red;", "#diagram #diagram ~ *"),
            (
                "& \n\t \r \u{000C} \r\n + *",
                "color: red;",
                "#diagram #diagram \n\t \r \u{000C} \r\n + *",
            ),
            ("& || *", "color: red;", "#diagram #diagram || *"),
            ("&", "color: red;", "#diagram #diagram"),
            (
                "&",
                "font-family: serif; font-size: 12px; fill: red;",
                "#diagram",
            ),
            ("#diagram", "color: red;", "#diagram #diagram"),
            (
                "#diagram",
                "font-family: serif; font-size: 12px; fill: red;",
                "#diagram",
            ),
            ("& > *", "color: red;", "#diagram > *"),
            ("& *", "color: red;", "#diagram *"),
        ];

        for (selector, body, expected) in cases {
            let css = format!("{selector}{{{body}}}");
            let work = OperationWorkMeter::new(RenderResourcePolicy::interactive());
            let mut out = String::new();
            rewrite::materialize_css(&css, Some("#diagram"), &mut out, PostprocessWork(&work))
                .expect("CSS namespace rewriting admits all CSS whitespace");
            assert_eq!(
                out,
                format!("{expected} {{{body}}}"),
                "selector: {selector:?}"
            );
        }
    }

    #[test]
    fn scoped_css_preserves_root_svg_selector_qualifiers() {
        assert_eq!(
            scoped_selector(
                r#"svg[aria-roledescription="classDiagram"] g.classGroup rect, svg > g, :root, svg-icon"#,
            ),
            r#"#diagram[aria-roledescription="classDiagram"] g.classGroup rect, #diagram svg > g, #diagram, #diagram svg-icon"#
        );
    }

    #[test]
    fn scoped_css_does_not_escape_through_root_siblings_or_id_prefixes() {
        assert_eq!(
            scoped_selector(
                "svg + .outside, svg ~ .outside, svg:has(+ .outside), #diagram-other .node",
            ),
            "#diagram svg + .outside, #diagram svg ~ .outside, #diagram svg:has(+ .outside), #diagram #diagram-other .node"
        );
    }

    #[test]
    fn scoped_css_parses_repeated_and_escaped_root_attributes() {
        assert_eq!(
            scoped_selector(r#"svg[data-label="a]b"][data-path="a\"b"] .node"#),
            r#"#diagram[data-label="a]b"][data-path="a\"b"] .node"#
        );
    }

    #[test]
    fn scoped_css_keeps_ambiguous_root_suffixes_inside_the_scope() {
        assert_eq!(
            scoped_selector(
                "svg[data-x] + .outside, svg[data-x]~.outside, svg[data-x] /* guard */ + .outside",
            ),
            "#diagram svg[data-x] + .outside, #diagram svg[data-x]~.outside, #diagram svg[data-x] /* guard */ + .outside"
        );
    }

    #[test]
    fn scoped_css_fails_closed_for_attribute_comments_and_sibling_suffixes() {
        assert_eq!(
            scoped_selector(
                "svg[data-x/* ] */] + .outside, svg[data-x]/*guard*/+.outside, svg[a][b] + .outside, svg[a][b] ~ .outside",
            ),
            "#diagram svg[data-x/* ] */] + .outside, #diagram svg[data-x]/*guard*/+.outside, #diagram svg[a][b] + .outside, #diagram svg[a][b] ~ .outside"
        );
    }

    #[test]
    fn scoped_css_handles_escaped_brackets_and_rejects_unclosed_attributes() {
        assert_eq!(
            scoped_selector(r"svg[data-label=a\]b] .node"),
            r"#diagram[data-label=a\]b] .node"
        );

        let work = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        let execution = PostprocessWork(&work);
        let mut output = String::new();
        let error = rewrite::materialize_css(
            "svg[a][b{color:red;}",
            Some("#diagram"),
            &mut output,
            execution,
        )
        .expect_err("unclosed root attributes must fail before CSS injection");
        assert!(error.to_string().contains("invalid scoped CSS"));
        assert!(output.is_empty());
    }

    #[test]
    fn scoped_css_token_walk_observes_mid_stream_cancellation() {
        let control = OperationControl::new();
        let work = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let css = ".node { fill: red; }".repeat(512);
        control.cancel_after_checkpoints(2);

        let error = rewrite::projected_css_bytes(&css, Some("#diagram"), PostprocessWork(&work))
            .expect_err("the CSS token walk must observe cancellation before completion");
        let Error::Cancelled(cancelled) = error else {
            panic!("expected structured cancellation, got {error}");
        };
        assert_eq!(cancelled.phase, OperationPhase::Postprocess);
        assert_eq!(cancelled.reason, CancelReason::Requested);
    }
}
