use crate::Result;
use crate::svg::pipeline::{SvgPostprocessContext, SvgPostprocessExecution, SvgPostprocessor};
use cssparser::{Delimiter, ParseError, Parser};
use merman_render_core::svg_output::{BoundedSvgOutput, SvgOutput};
use merman_render_core::svg_scan::{
    CssXmlLocation, SvgTagScanner, XmlCssSource, next_svg_quoted_attr_with_checkpoints,
    start_tag_name, trim_with_checkpoints,
};
use std::borrow::Cow;
use std::ops::Range;

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
        ctx: &SvgPostprocessContext<'_>,
    ) -> Result<Cow<'a, str>> {
        set_root_background_color(svg, &self.background_color, ctx.execution())
    }
}

fn set_root_background_color<'a>(
    svg: Cow<'a, str>,
    background_color: &str,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    let mut checkpoint = || execution.checkpoint();
    let background_color = trim_with_checkpoints(background_color, &mut checkpoint)?;
    if background_color.is_empty() {
        return Ok(svg);
    }
    let mut scanner = SvgTagScanner::new(&svg);
    let root = loop {
        let Some(tag) = scanner.next_with_checkpoints(&mut checkpoint)? else {
            return Ok(svg);
        };
        if let Some(name) = start_tag_name(tag.raw()) {
            if name.rsplit(':').next() != Some("svg") {
                return Ok(svg);
            }
            break tag;
        }
    };

    let mut out = BoundedSvgOutput::new(execution.work_meter());
    let mut attr_cursor = 0;
    while let Some(attr) =
        next_svg_quoted_attr_with_checkpoints(root.raw(), attr_cursor, &mut checkpoint)?
    {
        attr_cursor = attr.full_end;
        if !root.raw()[attr.name_start..attr.name_end].eq_ignore_ascii_case("style") {
            continue;
        }
        let start = root.start() + attr.value_start;
        let end = root.start() + attr.value_end;
        let Some(css) =
            XmlCssSource::new(&svg[start..end], CssXmlLocation::Attribute, &mut checkpoint)?
        else {
            return Ok(svg);
        };
        let declarations = background_declarations(css.logical(), &mut checkpoint)?;
        if declarations.is_empty() {
            out.push_str(&svg[..end]);
            let trimmed = css.logical().trim_end();
            if !trimmed.is_empty() && !trimmed.ends_with(';') {
                out.push(';');
            }
            out.push(' ');
            write_background(&mut out, background_color);
            out.push_str(&svg[end..]);
        } else {
            let mut cursor = 0;
            let mut wrote_background = false;
            css.visit_source_spans(&declarations, &mut checkpoint, |index, edit| {
                out.push_str(&svg[cursor..start + edit.start]);
                if index == 0 && !wrote_background {
                    write_background(&mut out, background_color);
                    wrote_background = true;
                }
                cursor = start + edit.end;
            })?;
            out.push_str(&svg[cursor..]);
        }
        return Ok(Cow::Owned(out.finish()?));
    }

    let insert_at = root.end() - if root.is_self_closing() { 2 } else { 1 };
    out.push_str(&svg[..insert_at]);
    out.push_str(" style=\"");
    write_background(&mut out, background_color);
    out.push('"');
    out.push_str(&svg[insert_at..]);
    Ok(Cow::Owned(out.finish()?))
}

fn write_background(out: &mut BoundedSvgOutput<'_>, color: &str) {
    out.push_str("background-color: ");
    crate::xml::escape_attr_into(out, color);
    out.push(';');
}

fn background_declarations(
    css: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Vec<Range<usize>>> {
    let mut parser = Parser::new(css);
    let mut declarations = Vec::new();
    while !parser.is_exhausted() {
        checkpoint()?;
        let start = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            declaration.skip_whitespace();
            let start = declaration.position().byte_index();
            declaration.expect_ident_matching("background-color")?;
            declaration.expect_colon()?;
            while declaration.next().is_ok() {}
            Ok::<_, ParseError<()>>(start)
        });
        if let Ok(start) = start {
            declarations.push(start..parser.position().byte_index());
        }
    }
    Ok(declarations)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::pipeline::SvgPipeline;

    fn apply_root_background_edit(svg: &str, background_color: &str) -> String {
        set_root_background_color(
            Cow::Borrowed(svg),
            background_color,
            SvgPostprocessExecution::new(&render_session()),
        )
        .unwrap()
        .into_owned()
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
    fn root_background_preserves_custom_font_entities() {
        let svg = r#"<svg style="--font-family: &quot;Open Sans&quot;, sans-serif;"><g/></svg>"#;
        let out = apply_root_background_edit(svg, "white");
        assert_eq!(
            out,
            r#"<svg style="--font-family: &quot;Open Sans&quot;, sans-serif; background-color: white;"><g/></svg>"#
        );
        let replaced = apply_root_background_edit(&out, "transparent");
        assert_eq!(
            replaced,
            r#"<svg style="--font-family: &quot;Open Sans&quot;, sans-serif; background-color: transparent;"><g/></svg>"#
        );
    }

    #[test]
    fn root_background_preserves_semicolons_in_css_values() {
        let svg = r#"<svg style="--font: &quot;A;B&quot;; background-color: white;"><g/></svg>"#;
        assert_eq!(
            apply_root_background_edit(svg, "#112233"),
            r##"<svg style="--font: &quot;A;B&quot;; background-color: #112233;"><g/></svg>"##
        );
    }

    #[test]
    fn root_background_uses_actual_root_and_css_declaration_boundaries() {
        let svg = concat!(
            r#"<?note <svg style='background-color: fake'>?>"#,
            r#"<!-- <svg/> --><svg title='a > b' "#,
            r#"style='--note: &quot;A; background-color: fake&quot;; /* canvas */ background-color: white;'>"#,
            r#"<svg style='background-color: blue;'/></svg>"#,
        );
        let out = apply_root_background_edit(svg, "transparent");
        assert_eq!(
            out,
            svg.replace("background-color: white", "background-color: transparent")
        );
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
    fn root_background_removes_duplicate_declarations_without_changing_other_values() {
        let svg = r#"<svg style="--note: 'A;B'; background-color: white!important; background-color: black; fill: green;"><g/></svg>"#;
        let out = apply_root_background_edit(svg, "transparent");
        assert_eq!(out.matches("background-color").count(), 1);
        assert!(out.contains("background-color: transparent;"));
        assert!(out.contains("--note: 'A;B';"));
        assert!(out.contains("fill: green;"));
    }

    #[test]
    fn root_background_obeys_the_serialized_svg_byte_ceiling() {
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};

        let expected = r#"<svg style="background-color: white;"/>"#;
        let pipeline =
            SvgPipeline::parity().with_postprocessor(RootBackgroundPostprocessor::new("white"));
        let session = |maximum| {
            crate::environment::RenderEnvironment::deterministic()
                .with_resource_policy(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
                        .unwrap(),
                )
                .begin_session()
                .unwrap()
        };
        assert_eq!(
            pipeline
                .process_to_string("<svg/>", &session(expected.len()))
                .unwrap(),
            expected
        );
        let error = pipeline
            .process_to_string("<svg/>", &session(expected.len() - 1))
            .unwrap_err();
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected a structured byte limit, got {error:?}");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(error.actual, expected.len());
        assert_eq!(error.max, expected.len() - 1);
    }
}
