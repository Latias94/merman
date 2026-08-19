use crate::math::{
    PREPARED_MATH_CLASS_ATTRIBUTE, PREPARED_MATH_NATIVE_AVAILABLE_ATTRIBUTE,
    PREPARED_MATH_PROJECTION_TEMPLATE_OPEN,
};
use crate::{DiagramFamilyId, Error, Result};

use super::util::{extract_quoted_attr, find_tag_end};

pub(crate) fn project_prepared_math(
    svg: &str,
    family_id: Option<DiagramFamilyId>,
) -> Result<String> {
    if !svg.contains(PREPARED_MATH_CLASS_ATTRIBUTE) {
        return Ok(svg.to_owned());
    }
    if !matches!(
        family_id,
        Some(DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE | DiagramFamilyId::SEQUENCE)
    ) {
        return Err(projection_error(
            "prepared math requires renderer-owned family metadata",
        ));
    }

    let mut output = String::with_capacity(svg.len());
    let mut cursor = 0usize;
    let mut projected = 0usize;
    while let Some(relative_start) = svg[cursor..].find("<foreignObject") {
        let start = cursor + relative_start;
        let open_end = find_tag_end(svg, start)
            .ok_or_else(|| projection_error("malformed prepared-math foreignObject start tag"))?;
        let close_start = open_end
            + 1
            + svg[open_end + 1..]
                .find("</foreignObject>")
                .ok_or_else(|| projection_error("unclosed prepared-math foreignObject"))?;
        let end = close_start + "</foreignObject>".len();
        let foreign_object = &svg[start..end];
        if !foreign_object.contains(PREPARED_MATH_CLASS_ATTRIBUTE) {
            output.push_str(&svg[cursor..end]);
            cursor = end;
            continue;
        }
        if foreign_object
            .matches(PREPARED_MATH_CLASS_ATTRIBUTE)
            .count()
            != 1
        {
            return Err(projection_error(
                "each prepared-math foreignObject must contain exactly one occurrence",
            ));
        }
        if !foreign_object.contains(PREPARED_MATH_NATIVE_AVAILABLE_ATTRIBUTE) {
            return Err(projection_error(
                "native projection is unavailable for a prepared browser math occurrence",
            ));
        }

        let template_start = foreign_object
            .find(PREPARED_MATH_PROJECTION_TEMPLATE_OPEN)
            .ok_or_else(|| projection_error("prepared-math native projection is missing"))?
            + PREPARED_MATH_PROJECTION_TEMPLATE_OPEN.len();
        let template_end = template_start
            + foreign_object[template_start..]
                .find("</template>")
                .ok_or_else(|| projection_error("prepared-math native projection is unclosed"))?;
        if foreign_object[template_end + "</template>".len()..]
            .contains(PREPARED_MATH_PROJECTION_TEMPLATE_OPEN)
        {
            return Err(projection_error(
                "prepared-math occurrence contains duplicate native projections",
            ));
        }
        let projection = &foreign_object[template_start..template_end];
        let projection_open_end = find_tag_end(projection, 0)
            .ok_or_else(|| projection_error("prepared-math native projection is malformed"))?;
        let projection_tag = &projection[..=projection_open_end];
        if !projection_tag.starts_with("<g") || !projection.ends_with("</g>") {
            return Err(projection_error(
                "prepared-math native projection must be one SVG group",
            ));
        }

        let foreign_object_tag = &foreign_object[..=open_end - start];
        let x = parse_optional_length(foreign_object_tag, "x")?.unwrap_or(0.0);
        let y = parse_optional_length(foreign_object_tag, "y")?.unwrap_or(0.0);
        let width = parse_required_length(foreign_object_tag, "width")?;
        let height = parse_required_length(foreign_object_tag, "height")?;
        let projection_width =
            parse_required_length(projection_tag, "data-merman-prepared-math-width")?;
        let projection_height =
            parse_required_length(projection_tag, "data-merman-prepared-math-height")?;
        if projection_width > width + 0.01 || projection_height > height + 0.01 {
            return Err(projection_error(
                "prepared-math native projection exceeds its terminal foreignObject bounds",
            ));
        }
        let translate_x = x + (width - projection_width) / 2.0;
        let translate_y = y + (height - projection_height) / 2.0;

        output.push_str(&svg[cursor..start]);
        output.push_str(r#"<g class="merman-prepared-math-native" transform="translate("#);
        output.push_str(&format_number(translate_x));
        output.push(' ');
        output.push_str(&format_number(translate_y));
        output.push_str(r#")">"#);
        output.push_str(projection);
        output.push_str("</g>");
        cursor = end;
        projected = projected.saturating_add(1);
    }
    output.push_str(&svg[cursor..]);

    if projected == 0 || output.contains(PREPARED_MATH_CLASS_ATTRIBUTE) {
        return Err(projection_error(
            "prepared-math occurrence was not bound to a terminal foreignObject",
        ));
    }
    Ok(output)
}

fn parse_required_length(tag: &str, name: &str) -> Result<f64> {
    parse_optional_length(tag, name)?
        .ok_or_else(|| projection_error(format!("prepared-math terminal is missing `{name}`")))
}

fn parse_optional_length(tag: &str, name: &str) -> Result<Option<f64>> {
    let Some(raw) = extract_quoted_attr(tag, name) else {
        return Ok(None);
    };
    let raw = raw.strip_suffix("px").unwrap_or(raw);
    let value = raw.parse::<f64>().map_err(|_| {
        projection_error(format!(
            "prepared-math terminal `{name}` must be a finite pixel length"
        ))
    })?;
    if !value.is_finite() || value < 0.0 {
        return Err(projection_error(format!(
            "prepared-math terminal `{name}` must be a finite non-negative length"
        )));
    }
    Ok(Some(value))
}

fn format_number(value: f64) -> String {
    let value = format!("{value:.6}");
    let value = value.trim_end_matches('0').trim_end_matches('.');
    if value.is_empty() || value == "-0" {
        "0".to_owned()
    } else {
        value.to_owned()
    }
}

fn projection_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess("prepared-math-projection", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_replaces_one_terminal_foreign_object_and_centers_native_geometry() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(5 6)">"#,
            r#"<foreignObject x="10" y="20" width="24" height="18"><div>"#,
            r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1">"#,
            r#"<svg><path d="browser"/></svg>"#,
            r#"<template data-merman-prepared-math-projection="v1">"#,
            r#"<g data-merman-prepared-math-width="20" data-merman-prepared-math-height="10">"#,
            r#"<svg width="20" height="10"><path d="native"/></svg></g></template>"#,
            r#"</span></div></foreignObject></g></svg>"#,
        );

        let output = project_prepared_math(svg, Some(DiagramFamilyId::FLOWCHART)).unwrap();

        assert!(!output.contains("<foreignObject"), "{output}");
        assert!(!output.contains("browser"), "{output}");
        assert!(
            output.contains(r#"transform="translate(12 24)""#),
            "{output}"
        );
        assert!(output.contains(r#"d="native""#), "{output}");
        assert!(output.contains(r#"transform="translate(5 6)""#), "{output}");
    }

    #[test]
    fn projection_rejects_browser_only_and_unowned_occurrences() {
        let browser_only = concat!(
            r#"<svg><foreignObject width="10" height="10"><div>"#,
            r#"<span class="merman-prepared-math" data-merman-prepared-math-native="unavailable"/>"#,
            r#"</div></foreignObject></svg>"#,
        );
        let error = project_prepared_math(browser_only, Some(DiagramFamilyId::SEQUENCE))
            .expect_err("browser-only math must fail closed");
        assert!(
            error
                .to_string()
                .contains("native projection is unavailable")
        );

        let forged = browser_only.replace(
            r#"data-merman-prepared-math-native="unavailable""#,
            r#"data-merman-prepared-math-native="v1""#,
        );
        let error = project_prepared_math(&forged, None)
            .expect_err("raw SVG cannot mint prepared-math capability");
        assert!(error.to_string().contains("renderer-owned family metadata"));
    }
}
