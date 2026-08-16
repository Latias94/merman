use crate::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitExceeded, ResourceLimitId,
    ResourceLimitOverride, ResourceLimitPhase,
};
use crate::{Error, Result};
use cssparser::{Delimiter, Parser, ParserInput};
use std::borrow::Cow;

use super::css_override::{CssOverridePolicy, strip_css_important};
use super::util::{escape_xml_attr, find_matching_brace, find_tag_end};
use crate::svg::escape_css_identifier;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

const SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT: usize = 64;
const SCOPED_CSS_GROUPING_DEPTH_HARD_CAP_ID: &str = "max_scoped_css_grouping_depth_hard_cap";
const SCOPED_STYLE_OPEN: &str = r#"<style data-merman-postprocess="scoped-css">"#;
const SCOPED_STYLE_CLOSE: &str = "</style>";
const XML_STYLE_END_ESCAPE: &str = "&lt;/style";

#[derive(Debug, Clone)]
pub struct ScopedCssPostprocessor {
    css: String,
    override_policy: CssOverridePolicy,
    merge_into_existing_style: bool,
}

impl ScopedCssPostprocessor {
    pub fn new(css: impl Into<String>) -> Self {
        Self {
            css: css.into(),
            override_policy: CssOverridePolicy::Preserve,
            merge_into_existing_style: false,
        }
    }

    pub fn with_override_policy(mut self, policy: CssOverridePolicy) -> Self {
        self.override_policy = policy;
        self
    }

    pub fn css(&self) -> &str {
        &self.css
    }

    pub fn override_policy(&self) -> CssOverridePolicy {
        self.override_policy
    }

    pub fn with_existing_style_merge(mut self) -> Self {
        self.merge_into_existing_style = true;
        self
    }
}

impl SvgPostprocessor for ScopedCssPostprocessor {
    fn name(&self) -> &'static str {
        "scoped-css"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        if self.css.trim().is_empty() {
            return Ok(svg);
        }

        let base = match self.override_policy {
            CssOverridePolicy::Preserve => svg.into_owned(),
            CssOverridePolicy::StripExistingImportant => strip_css_important(svg.as_ref()),
        };
        let css = decode_mermaid_css_hash_placeholders(&self.css);
        let injection = StyleInjection::for_svg(&base, self.merge_into_existing_style);
        let scoped_css = scope_css(
            css.as_ref(),
            ctx.svg_id(),
            ctx.resource_policy(),
            base.len(),
            injection.wrapper_bytes(),
        )?;
        Ok(Cow::Owned(inject_style(
            base,
            &scoped_css,
            injection,
            ctx.resource_policy(),
        )?))
    }
}

#[derive(Debug, Clone, Copy)]
struct StyleInjection {
    index: usize,
    wrap: bool,
}

impl StyleInjection {
    fn for_svg(svg: &str, merge_into_existing_style: bool) -> Self {
        if merge_into_existing_style && let Some(index) = svg.find("</style") {
            return Self { index, wrap: false };
        }

        let index = if let Some(start) = svg.find("<svg")
            && let Some(root_end) = find_tag_end(svg, start)
        {
            svg.rfind("</style")
                .and_then(|style_start| find_tag_end(svg, style_start))
                .map_or(root_end + 1, |style_end| style_end + 1)
        } else {
            svg.len()
        };
        Self { index, wrap: true }
    }

    const fn wrapper_bytes(self) -> usize {
        if self.wrap {
            SCOPED_STYLE_OPEN.len() + SCOPED_STYLE_CLOSE.len()
        } else {
            0
        }
    }
}

fn inject_style(
    svg: String,
    css: &str,
    injection: StyleInjection,
    resource_policy: RenderResourcePolicy,
) -> Result<String> {
    let escaped_css_bytes = css.len().saturating_add(
        css.match_indices("</style")
            .count()
            .saturating_mul(XML_STYLE_END_ESCAPE.len() - "</style".len()),
    );
    let projected_svg_bytes = svg
        .len()
        .saturating_add(injection.wrapper_bytes())
        .saturating_add(escaped_css_bytes);
    resource_policy
        .check_svg_byte_count(projected_svg_bytes, ResourceLimitPhase::SvgPostprocess)?;

    let mut out = String::with_capacity(projected_svg_bytes);
    out.push_str(&svg[..injection.index]);
    if injection.wrap {
        out.push_str(SCOPED_STYLE_OPEN);
    }
    push_style_safe_css(&mut out, css);
    if injection.wrap {
        out.push_str(SCOPED_STYLE_CLOSE);
    }
    out.push_str(&svg[injection.index..]);
    Ok(out)
}

fn push_style_safe_css(out: &mut String, css: &str) {
    let mut cursor = 0;
    while let Some(relative_start) = css[cursor..].find("</style") {
        let start = cursor + relative_start;
        out.push_str(&css[cursor..start]);
        out.push_str(XML_STYLE_END_ESCAPE);
        cursor = start + "</style".len();
    }
    out.push_str(&css[cursor..]);
}

fn scope_css(
    css: &str,
    svg_id: Option<&str>,
    resource_policy: RenderResourcePolicy,
    base_svg_bytes: usize,
    wrapper_bytes: usize,
) -> Result<String> {
    let mut out = BoundedCssOutput::new(css.len(), resource_policy, base_svg_bytes, wrapper_bytes);
    let Some(svg_id) = svg_id.filter(|id| !id.trim().is_empty()) else {
        out.push_str(css)?;
        return Ok(out.finish());
    };
    let scope = format!("#{}", escape_css_identifier(svg_id));
    let first_unclosed_grouping = first_unclosed_grouping_open(css, resource_policy)?;
    scope_css_block(css, &scope, first_unclosed_grouping, &mut out)?;
    Ok(out.finish())
}

fn decode_mermaid_css_hash_placeholders(css: &str) -> Cow<'_, str> {
    if !css.contains('ﬂ') && !css.contains('¶') {
        return Cow::Borrowed(css);
    }

    let mut decoded = String::with_capacity(css.len());
    let mut cursor = 0;
    while cursor < css.len() {
        let remainder = &css[cursor..];
        if remainder.starts_with("ﬂ°°") {
            decoded.push('#');
            cursor += "ﬂ°°".len();
        } else if remainder.starts_with("ﬂ°") {
            decoded.push('#');
            cursor += "ﬂ°".len();
        } else if remainder.starts_with("¶ß") {
            decoded.push(';');
            cursor += "¶ß".len();
        } else {
            let ch = remainder
                .chars()
                .next()
                .expect("a non-empty CSS remainder must contain one character");
            decoded.push(ch);
            cursor += ch.len_utf8();
        }
    }
    Cow::Owned(decoded)
}

struct BoundedCssOutput {
    value: String,
    resource_policy: RenderResourcePolicy,
    base_svg_bytes: usize,
    wrapper_bytes: usize,
}

impl BoundedCssOutput {
    fn new(
        estimated_css_bytes: usize,
        resource_policy: RenderResourcePolicy,
        base_svg_bytes: usize,
        wrapper_bytes: usize,
    ) -> Self {
        let available = resource_policy
            .value(ResourceLimitId::MaxSvgBytes)
            .map_or(estimated_css_bytes, |limit| {
                limit.saturating_sub(base_svg_bytes.saturating_add(wrapper_bytes))
            });
        Self {
            value: String::with_capacity(estimated_css_bytes.min(available)),
            resource_policy,
            base_svg_bytes,
            wrapper_bytes,
        }
    }

    fn ensure_additional(&self, additional: usize) -> Result<()> {
        let css_bytes = self.value.len().saturating_add(additional);
        let projected_svg_bytes = self
            .base_svg_bytes
            .saturating_add(self.wrapper_bytes)
            .saturating_add(css_bytes);
        self.resource_policy
            .check_svg_byte_count(projected_svg_bytes, ResourceLimitPhase::SvgPostprocess)?;
        Ok(())
    }

    fn remaining_capacity_hint(&self) -> usize {
        self.resource_policy
            .value(ResourceLimitId::MaxSvgBytes)
            .map_or(usize::MAX, |limit| {
                limit.saturating_sub(
                    self.base_svg_bytes
                        .saturating_add(self.wrapper_bytes)
                        .saturating_add(self.value.len()),
                )
            })
    }

    fn len(&self) -> usize {
        self.value.len()
    }

    fn truncate(&mut self, len: usize) {
        self.value.truncate(len);
    }

    fn push_str(&mut self, value: &str) -> Result<()> {
        self.ensure_additional(value.len())?;
        self.value.push_str(value);
        Ok(())
    }

    fn push_char(&mut self, value: char) -> Result<()> {
        self.ensure_additional(value.len_utf8())?;
        self.value.push(value);
        Ok(())
    }

    fn finish(self) -> String {
        self.value
    }
}

#[derive(Debug, Clone, Copy)]
struct GroupingFrame {
    source_cursor: usize,
    output_checkpoint: usize,
}

fn first_unclosed_grouping_open(
    css: &str,
    resource_policy: RenderResourcePolicy,
) -> Result<Option<usize>> {
    let mut grouping_opens = Vec::with_capacity(SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT);
    let mut cursor = 0;
    loop {
        let Some((relative_brace, brace)) =
            next_scope_brace(&css[cursor..], !grouping_opens.is_empty())
        else {
            return Ok(grouping_opens.first().copied());
        };
        if brace == '}' {
            cursor += relative_brace + 1;
            grouping_opens.pop();
            continue;
        }

        let open = cursor + relative_brace;
        let selector_start = css[cursor..open]
            .rfind(';')
            .map(|rel| cursor + rel + 1)
            .unwrap_or(cursor);
        let selector = &css[selector_start..open];
        if selector.trim_start().starts_with('@')
            && is_css_grouping_rule(css_at_rule_name(selector))
        {
            check_grouping_depth(resource_policy, grouping_opens.len().saturating_add(1))?;
            grouping_opens.push(open);
            cursor = open + 1;
            continue;
        }

        let Some(close) = find_matching_brace(css, open) else {
            return Ok(grouping_opens.first().copied());
        };
        cursor = close + 1;
    }
}

fn scope_css_block(
    css: &str,
    scope: &str,
    first_unclosed_grouping: Option<usize>,
    out: &mut BoundedCssOutput,
) -> Result<()> {
    // Only grouping at-rules recursively scope their bodies. Retaining their continuation state
    // here keeps emission to one traversal and avoids one call-stack frame per nesting level.
    let mut grouping_frames: Vec<GroupingFrame> =
        Vec::with_capacity(SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT);
    let mut cursor = 0;

    loop {
        let Some((relative_brace, brace)) =
            next_scope_brace(&css[cursor..], !grouping_frames.is_empty())
        else {
            preserve_unclosed_grouping(css, cursor, &grouping_frames, out)?;
            return Ok(());
        };

        if brace == '}' {
            let close = cursor + relative_brace;
            out.push_str(&css[cursor..close])?;
            out.push_char('}')?;
            cursor = close + 1;
            grouping_frames.pop();
            continue;
        }

        let open = cursor + relative_brace;
        let selector_start = css[cursor..open]
            .rfind(';')
            .map(|rel| cursor + rel + 1)
            .unwrap_or(cursor);
        let output_checkpoint = out.len();
        push_scope_css_statement_prefix(out, &css[cursor..selector_start])?;
        let selector = &css[selector_start..open];
        if selector.trim_start().starts_with('@') {
            let name = css_at_rule_name(selector);
            if is_css_grouping_rule(name) {
                if first_unclosed_grouping == Some(open) {
                    out.push_str(&css[cursor..])?;
                    return Ok(());
                }
                check_grouping_depth(out.resource_policy, grouping_frames.len().saturating_add(1))?;
                out.push_str(selector)?;
                out.push_char('{')?;
                grouping_frames.push(GroupingFrame {
                    source_cursor: cursor,
                    output_checkpoint,
                });
                cursor = open + 1;
                continue;
            }

            let Some(close) = find_matching_brace(css, open) else {
                preserve_unclosed_grouping(css, cursor, &grouping_frames, out)?;
                return Ok(());
            };
            if is_css_keyframes_rule(name) {
                out.push_str(selector)?;
                out.push_str(&css[open..=close])?;
            }
            cursor = close + 1;
        } else {
            let Some(close) = find_matching_brace(css, open) else {
                preserve_unclosed_grouping(css, cursor, &grouping_frames, out)?;
                return Ok(());
            };
            let body = &css[open + 1..close];
            write_scope_selector(out, selector, body, scope)?;
            out.push_char(' ')?;
            out.push_str(&css[open..=close])?;
            cursor = close + 1;
        }
    }
}

fn next_scope_brace(css: &str, accept_group_close: bool) -> Option<(usize, char)> {
    css.char_indices()
        .find(|(_, ch)| *ch == '{' || (accept_group_close && *ch == '}'))
}

fn preserve_unclosed_grouping(
    css: &str,
    cursor: usize,
    grouping_frames: &[GroupingFrame],
    out: &mut BoundedCssOutput,
) -> Result<()> {
    if let Some(outermost) = grouping_frames.first().copied() {
        out.truncate(outermost.output_checkpoint);
        out.push_str(&css[outermost.source_cursor..])
    } else {
        out.push_str(&css[cursor..])
    }
}

fn push_scope_css_statement_prefix(out: &mut BoundedCssOutput, prefix: &str) -> Result<()> {
    let trimmed = prefix.trim_start();
    if trimmed.starts_with("@import")
        || trimmed.starts_with("@namespace")
        || trimmed.starts_with("@charset")
    {
        return Ok(());
    }
    out.push_str(prefix)
}

fn css_at_rule_name(selector: &str) -> &str {
    selector
        .trim_start()
        .split(|ch: char| ch.is_whitespace() || ch == '{')
        .next()
        .unwrap_or("")
}

fn is_css_keyframes_rule(name: &str) -> bool {
    name.eq_ignore_ascii_case("@keyframes") || name.eq_ignore_ascii_case("@-webkit-keyframes")
}

fn is_css_grouping_rule(name: &str) -> bool {
    [
        "@media",
        "@supports",
        "@layer",
        "@scope",
        "@container",
        "@starting-style",
    ]
    .iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn check_grouping_depth(resource_policy: RenderResourcePolicy, depth: usize) -> Result<()> {
    if depth <= SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT {
        return Ok(());
    }
    Err(Error::ResourceLimitExceeded(ResourceLimitExceeded {
        cause: ResourceLimitCause::Ceiling,
        phase: ResourceLimitPhase::SvgPostprocess,
        limit: SCOPED_CSS_GROUPING_DEPTH_HARD_CAP_ID,
        actual: depth,
        max: SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT,
        profile: resource_policy.profile(),
        explicit_overrides: resource_policy
            .explicit_overrides()
            .map(|(id, value)| ResourceLimitOverride { id, value })
            .collect(),
    }))
}

fn write_scope_selector(
    out: &mut BoundedCssOutput,
    selector: &str,
    body: &str,
    scope: &str,
) -> Result<()> {
    let safe_root_declarations = selector
        .split(',')
        .any(|part| matches!(part.trim(), "&") || part.trim() == scope)
        && has_only_safe_root_declarations(body);
    for (index, part) in selector.split(',').enumerate() {
        if index != 0 {
            out.push_str(", ")?;
        }
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let expanded = expand_selector(trimmed, scope, out)?;
        if (expanded == scope && safe_root_declarations)
            || is_already_namespaced(expanded.as_ref(), scope)
        {
            out.push_str(expanded.as_ref())?;
        } else if let Some(suffix) = safe_root_selector_suffix(expanded.as_ref(), ":root") {
            out.push_str(scope)?;
            out.push_str(suffix)?;
        } else if let Some(suffix) = safe_root_selector_suffix(expanded.as_ref(), "svg") {
            out.push_str(scope)?;
            out.push_str(suffix)?;
        } else {
            out.push_str(scope)?;
            out.push_char(' ')?;
            out.push_str(expanded.as_ref())?;
        }
    }
    Ok(())
}

fn expand_selector<'a>(
    selector: &'a str,
    scope: &str,
    out: &BoundedCssOutput,
) -> Result<Cow<'a, str>> {
    if !selector.contains('&') {
        return Ok(Cow::Borrowed(selector));
    }

    let mut expanded = String::with_capacity(selector.len().min(out.remaining_capacity_hint()));
    let mut parts = selector.split('&').peekable();
    while let Some(part) = parts.next() {
        let next_len = expanded.len().saturating_add(part.len());
        out.ensure_additional(next_len)?;
        expanded.push_str(part);
        if parts.peek().is_some() {
            let next_len = expanded.len().saturating_add(scope.len());
            out.ensure_additional(next_len)?;
            expanded.push_str(scope);
        }
    }
    Ok(Cow::Owned(expanded))
}

fn safe_root_selector_suffix<'a>(selector: &'a str, root: &str) -> Option<&'a str> {
    let suffix = selector.strip_prefix(root)?;
    if !is_safe_root_suffix(suffix, true) {
        return None;
    }
    Some(suffix)
}

fn is_safely_scoped_selector(selector: &str, scope: &str) -> bool {
    let Some(suffix) = selector.strip_prefix(scope) else {
        return false;
    };
    is_safe_root_suffix(suffix, false)
}

fn is_safe_root_suffix(suffix: &str, require_attribute_qualifier: bool) -> bool {
    if suffix.is_empty() {
        return true;
    }
    if suffix.starts_with('>') {
        return !require_attribute_qualifier;
    }
    if suffix.chars().next().is_some_and(char::is_whitespace) {
        if require_attribute_qualifier {
            return false;
        }
        return !starts_with_sibling_or_comment(suffix.trim_start());
    }
    if !suffix.starts_with('[') {
        return false;
    }

    let Some(rest) = consume_attribute_qualifiers(suffix) else {
        return false;
    };
    if rest.is_empty() || rest.starts_with('>') {
        return true;
    }
    if rest.chars().next().is_some_and(char::is_whitespace) {
        return !starts_with_sibling_or_comment(rest.trim_start());
    }
    false
}

fn starts_with_sibling_or_comment(selector: &str) -> bool {
    matches!(selector.chars().next(), Some('+' | '~' | '/'))
}

fn consume_attribute_qualifiers(mut selector: &str) -> Option<&str> {
    while selector.starts_with('[') {
        let mut quote = None;
        let mut escaped = false;
        let mut end = None;
        let mut chars = selector.char_indices().skip(1).peekable();
        while let Some((index, ch)) = chars.next() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if let Some(open_quote) = quote {
                if ch == open_quote {
                    quote = None;
                }
                continue;
            }
            if matches!(ch, '\'' | '"') {
                quote = Some(ch);
            } else if ch == '/' && chars.peek().is_some_and(|(_, next)| *next == '*') {
                return None;
            } else if ch == ']' {
                end = Some(index + ch.len_utf8());
                break;
            }
        }
        selector = &selector[end?..];
    }
    Some(selector)
}

fn has_only_safe_root_declarations(body: &str) -> bool {
    let mut input = ParserInput::new(body);
    let mut parser = Parser::new(&mut input);

    while !parser.is_exhausted() {
        if parser
            .try_parse(|declaration| declaration.expect_semicolon())
            .is_ok()
        {
            continue;
        }

        let allowed = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;
            while declaration.next_including_whitespace().is_ok() {}

            Ok::<_, cssparser::ParseError<'_, ()>>(matches!(
                property.as_ref(),
                "font-family" | "font-size" | "fill"
            ))
        });
        if !matches!(allowed, Ok(true)) {
            return false;
        }
    }

    true
}

fn is_already_namespaced(selector: &str, scope: &str) -> bool {
    let Some(suffix) = selector.strip_prefix(scope) else {
        return false;
    };
    if suffix.starts_with('>') {
        return true;
    }

    let Some(first) = suffix.chars().next() else {
        return false;
    };
    if !is_css_whitespace(first) {
        return false;
    }

    let descendant = suffix.trim_start_matches(is_css_whitespace);
    !descendant.is_empty()
        && !descendant.starts_with('+')
        && !descendant.starts_with('~')
        && !descendant.starts_with("||")
}

fn is_css_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\n' | '\r' | '\t' | '\u{000C}')
}

#[allow(dead_code)]
fn scoped_attr_selector(id: &str) -> String {
    format!(r#"svg[id="{}"]"#, escape_xml_attr(id))
}

#[cfg(test)]
fn scope_selector(selector: &str, body: &str, scope: &str) -> String {
    let mut out = BoundedCssOutput::new(
        selector.len(),
        RenderResourcePolicy::unbounded_for_trusted_input(),
        0,
        0,
    );
    write_scope_selector(&mut out, selector, body, scope).expect("unbounded selector scoping");
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    fn render_session() -> crate::environment::RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn scoped_css_injects_after_root_svg_tag_when_no_style_exists() {
        let svg = r#"<svg id="diagram"><rect class="node"/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                ".node rect, text.label { fill: red; }",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.starts_with(r#"<svg id="diagram"><style"#));
        assert!(out.contains("#diagram .node rect, #diagram text.label { fill: red; }"));
    }

    #[test]
    fn scoped_css_injects_after_existing_style_for_cascade_order() {
        let svg =
            r#"<svg id="diagram"><style>#diagram .node rect { fill: red; }</style><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(".node rect { fill: green; }"))
            .process_to_string(svg, &session)
            .unwrap();

        let existing = out.find("fill: red").unwrap();
        let injected = out.find("fill: green").unwrap();
        assert!(
            existing < injected,
            "injected CSS should follow Mermaid CSS for cascade order: {out}"
        );
    }

    #[test]
    fn scoped_css_can_merge_into_mermaid_generated_stylesheet() {
        let svg = r#"<svg id="diagram"><style>#diagram{fill:red;}</style><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }").with_existing_style_merge(),
            )
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(out.matches("<style").count(), 1);
        assert!(out.contains("#diagram{fill:red;}#diagram .node { fill: green; }</style>"));
        assert!(!out.contains("data-merman-postprocess"));
    }

    #[test]
    fn scoped_css_merge_targets_mermaids_first_global_stylesheet() {
        let svg = r#"<svg id="diagram"><style>.global{fill:red;}</style><g><style>.nested{fill:blue;}</style></g></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }").with_existing_style_merge(),
            )
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains(".global{fill:red;}#diagram .node { fill: green; }</style>"));
        assert!(out.contains("<style>.nested{fill:blue;}</style>"));
    }

    #[test]
    fn scoped_css_can_strip_existing_important_before_injection() {
        let svg = r#"<svg id="diagram"><style>.node{fill:red !important;}</style></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }")
                    .with_override_policy(CssOverridePolicy::StripExistingImportant),
            )
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("!important"));
        assert!(out.contains("#diagram .node { fill: green; }"));
    }

    #[test]
    fn scoped_css_matches_mermaid_ampersand_selector_namespace() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                ":not(&){background:green !important}",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("#diagram :not(#diagram) {background:green !important}"));
    }

    #[test]
    fn scoped_css_preserves_root_svg_selector_qualifiers() {
        assert_eq!(
            scope_selector(
                r#"svg[aria-roledescription="classDiagram"] g.classGroup rect, svg > g, :root, svg-icon"#,
                "color: red;",
                "#diagram",
            ),
            r#"#diagram[aria-roledescription="classDiagram"] g.classGroup rect, #diagram svg > g, #diagram, #diagram svg-icon"#
        );
    }

    #[test]
    fn scoped_css_does_not_escape_through_root_siblings_or_id_prefixes() {
        assert_eq!(
            scope_selector(
                "svg + .outside, svg ~ .outside, svg:has(+ .outside), #diagram-other .node",
                "color: red;",
                "#diagram",
            ),
            "#diagram svg + .outside, #diagram svg ~ .outside, #diagram svg:has(+ .outside), #diagram #diagram-other .node"
        );
    }

    #[test]
    fn scoped_css_parses_repeated_and_escaped_root_attributes() {
        assert_eq!(
            scope_selector(
                r#"svg[data-label="a]b"][data-path="a\"b"] .node"#,
                "color: red;",
                "#diagram",
            ),
            r#"#diagram[data-label="a]b"][data-path="a\"b"] .node"#
        );
    }

    #[test]
    fn scoped_css_keeps_ambiguous_root_suffixes_inside_the_scope() {
        assert_eq!(
            scope_selector(
                "svg[data-x] + .outside, svg[data-x]~.outside, svg[data-x] /* guard */ + .outside, svg[data-x",
                "color: red;",
                "#diagram",
            ),
            "#diagram svg[data-x] + .outside, #diagram svg[data-x]~.outside, #diagram svg[data-x] /* guard */ + .outside, #diagram svg[data-x"
        );
    }

    #[test]
    fn scoped_css_fails_closed_for_attribute_comments_and_sibling_suffixes() {
        assert_eq!(
            scope_selector(
                "svg[data-x/* ] */] + .outside, svg[data-x]/*guard*/+.outside, svg[a][b] + .outside, svg[a][b] ~ .outside",
                "color: red;",
                "#diagram",
            ),
            "#diagram svg[data-x/* ] */] + .outside, #diagram svg[data-x]/*guard*/+.outside, #diagram svg[a][b] + .outside, #diagram svg[a][b] ~ .outside"
        );
    }

    #[test]
    fn scoped_css_handles_escaped_brackets_and_unclosed_repeated_attributes() {
        assert_eq!(
            scope_selector(
                r"svg[data-label=a\]b] .node, svg[a][b",
                "color: red;",
                "#diagram",
            ),
            r"#diagram[data-label=a\]b] .node, #diagram svg[a][b"
        );
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
            ("& > *", "color: red;", "#diagram > *"),
            ("& *", "color: red;", "#diagram *"),
        ];

        for (selector, body, expected) in cases {
            assert_eq!(
                scope_selector(selector, body, "#diagram"),
                expected,
                "selector: {selector:?}"
            );
        }
    }

    #[test]
    fn scoped_css_scopes_nested_grouping_at_rules_and_drops_unsupported_rules() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                "@import url('https://example.test/styles.css'); @media (max-width: 600px) { * { fill: red; } } @supports selector(h2 > p) { h2 > p { color: red; } }",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(!out.contains("@import"));
        assert!(out.contains("@media (max-width: 600px) {"));
        assert!(out.contains("#diagram * { fill: red; }"));
        assert!(out.contains("@supports selector(h2 > p) {"));
        assert!(out.contains("#diagram h2 > p { color: red; }"));
    }

    #[test]
    fn scoped_css_grouping_depth_is_iterative_and_hard_bounded() {
        fn nested_grouping(depth: usize) -> String {
            format!(
                "{}.node{{fill:red}}{}",
                "@media all{".repeat(depth),
                "}".repeat(depth)
            )
        }

        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let exact = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(nested_grouping(
                SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT,
            )))
            .process_to_string(svg, &session)
            .expect("the inclusive grouping depth boundary must remain supported");
        assert_eq!(
            exact.matches("@media all{").count(),
            SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT
        );
        assert!(exact.contains("#diagram .node {fill:red}"), "{exact}");

        let error = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(nested_grouping(
                SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT + 1,
            )))
            .process_to_string(svg, &session)
            .expect_err("one grouping level past the hard bound must be rejected");
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected a resource-limit error, got {error}");
        };
        assert_eq!(error.limit, SCOPED_CSS_GROUPING_DEPTH_HARD_CAP_ID);
        assert_eq!(error.actual, SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT + 1);
        assert_eq!(error.max, SCOPED_CSS_GROUPING_DEPTH_HARD_LIMIT);
    }

    #[test]
    fn scoped_css_preserves_unclosed_grouping_rules_without_partial_rewrite() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let css = "@media all {.node{fill:red}";
        let session = render_session();
        let expected = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(expected.contains(css), "{expected}");
        assert!(!expected.contains("#diagram .node"), "{expected}");

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len())
            .unwrap();
        let exact_session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(exact_policy)
            .begin_session()
            .unwrap();
        let exact = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &exact_session)
            .expect("an unclosed grouping must not exceed the exact raw fallback boundary");
        assert_eq!(exact, expected);
    }

    #[test]
    fn scoped_css_honors_the_exact_postprocess_svg_byte_boundary() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let css = ".node { fill: red; }";
        let expected = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &render_session())
            .unwrap();
        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len())
            .unwrap();
        let exact_session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(exact_policy)
            .begin_session()
            .unwrap();
        let exact = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &exact_session)
            .expect("the exact final SVG byte boundary must pass");
        assert_eq!(exact, expected);

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, expected.len() - 1)
            .unwrap();
        let short_session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(short_policy)
            .begin_session()
            .unwrap();
        let error = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(css))
            .process_to_string(svg, &short_session)
            .expect_err("one byte below the final SVG size must fail during postprocess");
        let Error::ResourceLimitExceeded(error) = error else {
            panic!("expected a resource-limit error, got {error}");
        };
        assert_eq!(error.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(error.phase, ResourceLimitPhase::SvgPostprocess);
    }

    #[test]
    fn scoped_css_escapes_style_end_tokens_during_the_single_svg_rebuild() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                r#".node::after { content: "</style"; }"#,
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains(r#"content: "&lt;/style"#), "{out}");
        assert_eq!(out.matches("</style>").count(), 1, "{out}");
    }

    #[test]
    fn scoped_css_keeps_keyframes_unscoped_like_mermaid() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                "@keyframes dash { to { stroke-dashoffset: 1000; } } .edge { animation: dash 1s; }",
            ))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("@keyframes dash { to { stroke-dashoffset: 1000; } }"));
        assert!(out.contains("#diagram .edge { animation: dash 1s; }"));
    }

    #[test]
    fn scoped_css_decodes_mermaid_hash_placeholders_as_css_hashes() {
        let svg = r#"<svg id="diagram"><g/></svg>"#;
        let session = render_session();
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(".node { fill: ﬂ°°123456¶ß }"))
            .process_to_string(svg, &session)
            .unwrap();

        assert!(out.contains("#diagram .node { fill: #123456; }"));
    }
}
