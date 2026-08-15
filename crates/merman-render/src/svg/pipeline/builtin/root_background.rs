use crate::Result;
use std::borrow::Cow;
use std::ops::Range;

use super::util::{escape_xml_attr, find_quoted_attr_value_span, find_tag_end};
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessor};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootBackgroundEdit {
    range: Range<usize>,
    replacement: String,
}

impl RootBackgroundEdit {
    fn new(range: Range<usize>, replacement: String) -> Self {
        Self { range, replacement }
    }

    pub(crate) fn additional_len(&self) -> usize {
        self.replacement.len().saturating_sub(self.range.len())
    }

    pub(crate) fn adjusted_end(&self, end: usize) -> Option<usize> {
        if self.range.end > end {
            return None;
        }
        end.checked_sub(self.range.len())?
            .checked_add(self.replacement.len())
    }

    pub(crate) fn apply(self, svg: &mut String) {
        svg.replace_range(self.range, &self.replacement);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBackgroundPostprocessor {
    background_color: String,
}

impl RootBackgroundPostprocessor {
    pub fn new(background_color: impl Into<String>) -> Self {
        Self {
            background_color: background_color.into(),
        }
    }

    pub fn background_color(&self) -> &str {
        &self.background_color
    }
}

impl SvgPostprocessor for RootBackgroundPostprocessor {
    fn name(&self) -> &'static str {
        "root-background"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        let background_color = self.background_color.trim();
        if background_color.is_empty() || !svg.contains("<svg") {
            return Ok(svg);
        }

        let Some(edit) = set_root_background_color(svg.as_ref(), background_color) else {
            return Ok(svg);
        };
        let mut out = svg.into_owned();
        edit.apply(&mut out);
        Ok(Cow::Owned(out))
    }
}

pub(crate) fn set_root_background_color(
    svg: &str,
    background_color: &str,
) -> Option<RootBackgroundEdit> {
    let svg_start = svg.find("<svg")?;
    let svg_end = find_tag_end(svg, svg_start)?;

    let tag = &svg[svg_start..=svg_end];
    let escaped_color = escape_xml_attr(background_color.trim());

    if let Some((style_value_start, style_value_end)) = find_quoted_attr_value_span(tag, "style") {
        let style = &tag[style_value_start..style_value_end];
        let rewritten = set_background_in_style_attr(style, &escaped_color);
        let absolute_value_start = svg_start + style_value_start;
        let absolute_value_end = svg_start + style_value_end;

        return Some(RootBackgroundEdit::new(
            absolute_value_start..absolute_value_end,
            rewritten,
        ));
    }

    let insert_at = if svg.as_bytes().get(svg_end.saturating_sub(1)) == Some(&b'/') {
        svg_end - 1
    } else {
        svg_end
    };

    Some(RootBackgroundEdit::new(
        insert_at..insert_at,
        format!(r#" style="background-color: {escaped_color};""#),
    ))
}

fn set_background_in_style_attr(style: &str, background_color: &str) -> String {
    let mut declarations = Vec::new();
    let mut replaced = false;

    for declaration in style.split(';') {
        let trimmed = declaration.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((property, _value)) = trimmed.split_once(':') else {
            declarations.push(trimmed.to_string());
            continue;
        };

        if property.trim().eq_ignore_ascii_case("background-color") {
            if !replaced {
                declarations.push(format!("background-color: {background_color}"));
                replaced = true;
            }
        } else {
            declarations.push(trimmed.to_string());
        }
    }

    if !replaced {
        declarations.push(format!("background-color: {background_color}"));
    }

    format!("{};", declarations.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    fn apply_root_background_edit(svg: &str, background_color: &str) -> String {
        let mut out = svg.to_string();
        set_root_background_color(&out, background_color)
            .expect("valid SVG root background edit")
            .apply(&mut out);
        out
    }

    fn render_session() -> crate::environment::RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    #[test]
    fn root_background_rewrites_existing_background_color() {
        let svg =
            r#"<svg id="diagram" style="max-width: 400px; background-color: white;"><g/></svg>"#;
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(RootBackgroundPostprocessor::new("#111827"))
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(
            out,
            r#"<svg id="diagram" style="max-width: 400px; background-color: #111827;"><g/></svg>"#
        );
    }

    #[test]
    fn root_background_adds_missing_style_property() {
        let svg = r#"<svg id="diagram" width="100%"><g/></svg>"#;
        let session = render_session();

        let out = SvgPipeline::parity()
            .with_postprocessor(RootBackgroundPostprocessor::new("transparent"))
            .process_to_string(svg, &session)
            .unwrap();

        assert_eq!(
            out,
            r#"<svg id="diagram" width="100%" style="background-color: transparent;"><g/></svg>"#
        );
    }

    #[test]
    fn root_background_escapes_xml_attribute_value() {
        let svg = r#"<svg id="diagram" style="max-width: 400px;"><g/></svg>"#;

        let out = apply_root_background_edit(svg, "rgb(1, 2, 3)&");

        assert!(out.contains("background-color: rgb(1, 2, 3)&amp;;"));
    }

    #[test]
    fn root_background_rewrites_single_quoted_style_attr() {
        let svg =
            r#"<svg id="diagram" style='max-width: 400px; background-color: white;'><g/></svg>"#;

        let out = apply_root_background_edit(svg, "#111827");

        assert_eq!(
            out,
            r##"<svg id="diagram" style='max-width: 400px; background-color: #111827;'><g/></svg>"##
        );
    }

    #[test]
    fn root_background_edit_describes_only_the_root_attribute_replacement() {
        let svg = r#"<svg id="diagram" style="background-color: white;"><g/></svg>"#;
        let edit =
            set_root_background_color(svg, "transparent").expect("valid SVG root background edit");

        assert_eq!(&svg[edit.range.clone()], "background-color: white;");
        assert_eq!(edit.replacement, "background-color: transparent;");
        assert_eq!(edit.additional_len(), "transparent".len() - "white".len());
    }
}
