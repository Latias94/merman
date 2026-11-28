use super::postprocess::PostprocessWork;
use crate::Result;
use crate::resources::OperationWorkMeter;
use crate::svg_output::{BoundedSvgOutput, SvgOutput};
use crate::svg_scan::{
    CssXmlLocation, SvgTagScanner, XmlCssSource, next_svg_quoted_attr_with_checkpoints,
    start_tag_name, trim_with_checkpoints,
};
use cssparser::{Delimiter, ParseError, Parser};
use std::borrow::Cow;
use std::ops::Range;

/// Replaces or inserts the background declaration on the document root.
pub fn set_root_background_color<'a>(
    svg: Cow<'a, str>,
    background_color: &str,
    work_meter: &OperationWorkMeter,
) -> Result<Cow<'a, str>> {
    let execution = PostprocessWork(work_meter);
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

    let mut out = BoundedSvgOutput::new(work_meter);
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
    use crate::resources::RenderResourcePolicy;

    fn apply_root_background_edit(svg: &str, background_color: &str) -> String {
        let work = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        set_root_background_color(Cow::Borrowed(svg), background_color, &work)
            .unwrap()
            .into_owned()
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
}
