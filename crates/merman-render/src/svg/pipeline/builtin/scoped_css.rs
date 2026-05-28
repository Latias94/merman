use crate::Result;
use std::borrow::Cow;

use super::css_override::{CssOverridePolicy, strip_css_important};
use super::util::{escape_xml_attr, find_matching_brace, find_tag_end};
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone)]
pub struct ScopedCssPostprocessor {
    css: String,
    override_policy: CssOverridePolicy,
}

impl ScopedCssPostprocessor {
    pub fn new(css: impl Into<String>) -> Self {
        Self {
            css: css.into(),
            override_policy: CssOverridePolicy::Preserve,
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

        let mut base = match self.override_policy {
            CssOverridePolicy::Preserve => svg.into_owned(),
            CssOverridePolicy::StripExistingImportant => strip_css_important(svg.as_ref()),
        };
        let scoped_css = scope_css(&self.css, ctx.svg_id());
        inject_style(&mut base, &scoped_css);
        Ok(Cow::Owned(base))
    }
}

fn inject_style(svg: &mut String, css: &str) {
    let css = css.replace("</style", "<\\/style");
    let style = format!(
        r#"<style data-merman-postprocess="scoped-css">{}</style>"#,
        css
    );

    if let Some(start) = svg.find("<svg") {
        if let Some(end) = find_tag_end(svg, start) {
            if let Some(style_close_start) = svg.rfind("</style") {
                if let Some(style_close_end) = find_tag_end(svg, style_close_start) {
                    svg.insert_str(style_close_end + 1, &style);
                    return;
                }
            }
            svg.insert_str(end + 1, &style);
            return;
        }
    }

    svg.push_str(&style);
}

fn scope_css(css: &str, svg_id: Option<&str>) -> String {
    let Some(svg_id) = svg_id.filter(|id| !id.trim().is_empty()) else {
        return css.to_string();
    };
    let scope = format!("#{}", css_escape_id(svg_id));
    let mut out = String::with_capacity(css.len() + scope.len() * 4);
    let mut cursor = 0;

    while let Some(rel_open) = css[cursor..].find('{') {
        let open = cursor + rel_open;
        let selector = &css[cursor..open];
        let Some(close) = find_matching_brace(css, open) else {
            out.push_str(&css[cursor..]);
            return out;
        };

        if selector.trim_start().starts_with('@') {
            out.push_str(&css[cursor..=close]);
        } else {
            out.push_str(&scope_selector(selector, &scope));
            out.push(' ');
            out.push_str(&css[open..=close]);
        }
        cursor = close + 1;
    }

    out.push_str(&css[cursor..]);
    out
}

fn scope_selector(selector: &str, scope: &str) -> String {
    selector
        .split(',')
        .map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                String::new()
            } else if trimmed.starts_with(scope) {
                trimmed.to_string()
            } else if trimmed == ":root" || trimmed == "svg" {
                scope.to_string()
            } else {
                format!("{scope} {trimmed}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn css_escape_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        let ok = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_';
        if ok {
            out.push(ch);
        } else {
            out.push('\\');
            out.push(ch);
        }
    }
    out
}

#[allow(dead_code)]
fn scoped_attr_selector(id: &str) -> String {
    format!(r#"svg[id="{}"]"#, escape_xml_attr(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    #[test]
    fn scoped_css_injects_after_root_svg_tag_when_no_style_exists() {
        let svg = r#"<svg id="diagram"><rect class="node"/></svg>"#;
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(
                ".node rect, text.label { fill: red; }",
            ))
            .process_to_string(svg)
            .unwrap();

        assert!(out.starts_with(r#"<svg id="diagram"><style"#));
        assert!(out.contains("#diagram .node rect, #diagram text.label { fill: red; }"));
    }

    #[test]
    fn scoped_css_injects_after_existing_style_for_cascade_order() {
        let svg =
            r#"<svg id="diagram"><style>#diagram .node rect { fill: red; }</style><g/></svg>"#;
        let out = SvgPipeline::parity()
            .with_postprocessor(ScopedCssPostprocessor::new(".node rect { fill: green; }"))
            .process_to_string(svg)
            .unwrap();

        let existing = out.find("fill: red").unwrap();
        let injected = out.find("fill: green").unwrap();
        assert!(
            existing < injected,
            "injected CSS should follow Mermaid CSS for cascade order: {out}"
        );
    }

    #[test]
    fn scoped_css_can_strip_existing_important_before_injection() {
        let svg = r#"<svg id="diagram"><style>.node{fill:red !important;}</style></svg>"#;
        let out = SvgPipeline::parity()
            .with_postprocessor(
                ScopedCssPostprocessor::new(".node { fill: green; }")
                    .with_override_policy(CssOverridePolicy::StripExistingImportant),
            )
            .process_to_string(svg)
            .unwrap();

        assert!(!out.contains("!important"));
        assert!(out.contains("#diagram .node { fill: green; }"));
    }
}
