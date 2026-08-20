use crate::math::{
    BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE, PREPARED_MATH_CLASS_ATTRIBUTE,
    PREPARED_MATH_NATIVE_AVAILABLE_ATTRIBUTE, PREPARED_MATH_NATIVE_CLASS,
    PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE, PREPARED_MATH_OCCURRENCE_ATTRIBUTE,
    PREPARED_MATH_PROJECTION_TEMPLATE_OPEN, PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE,
    PreparedMathEvidenceLease, PreparedMathProjectionFingerprint,
};
use crate::{DiagramFamilyId, Error, Result};

use super::util::{extract_quoted_attr, find_tag_end};

const PREPARED_MATH_NATIVE_MARKER_PREFIX: &str = "data-merman-prepared-math-native=";

pub(crate) fn project_prepared_math(
    svg: &str,
    family_id: Option<DiagramFamilyId>,
    evidence: Option<&PreparedMathEvidenceLease>,
) -> Result<String> {
    if svg.contains(BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE) {
        return Err(projection_error(
            "browser-only math has no renderer-owned native projection",
        ));
    }
    let has_browser_projection = svg.contains(PREPARED_MATH_CLASS_ATTRIBUTE)
        && svg.contains(PREPARED_MATH_NATIVE_MARKER_PREFIX);
    let has_native_projection = svg.contains(PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE)
        && svg.contains(PREPARED_MATH_OCCURRENCE_ATTRIBUTE);
    if !has_browser_projection && !has_native_projection {
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
    let evidence = evidence
        .filter(|evidence| !evidence.is_empty())
        .ok_or_else(|| {
            projection_error("prepared math requires renderer-owned occurrence evidence")
        })?;
    evidence
        .validate_unique_occurrences()
        .map_err(projection_error)?;
    if !has_browser_projection {
        return Ok(svg.to_owned());
    }

    let mut output = String::with_capacity(svg.len());
    let mut cursor = 0usize;
    let mut projected = 0usize;
    let mut emitted = vec![0usize; evidence.entries().len()];
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
        if !foreign_object.contains(PREPARED_MATH_CLASS_ATTRIBUTE)
            || !foreign_object.contains(PREPARED_MATH_NATIVE_MARKER_PREFIX)
        {
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
        let occurrence_id = extract_quoted_attr(foreign_object, PREPARED_MATH_OCCURRENCE_ATTRIBUTE)
            .ok_or_else(|| projection_error("prepared-math occurrence identity is missing"))?;
        let (expected_index, expected) = evidence
            .expectation(occurrence_id)
            .map_err(projection_error)?
            .ok_or_else(|| {
                projection_error(format!(
                    "prepared-math occurrence `{occurrence_id}` is not renderer-owned"
                ))
            })?;
        emitted[expected_index] = emitted[expected_index].saturating_add(1);
        if emitted[expected_index] > expected.expected_emissions() {
            return Err(projection_error(format!(
                "prepared-math occurrence {occurrence_id} was emitted more times than expected"
            )));
        }
        let expected_fingerprint = expected.projection_fingerprint().ok_or_else(|| {
            projection_error(format!(
                "native projection is unavailable for prepared-math occurrence `{occurrence_id}`"
            ))
        })?;

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
        if PreparedMathProjectionFingerprint::from_projection(projection) != expected_fingerprint {
            return Err(projection_error(format!(
                "prepared-math occurrence `{occurrence_id}` projection does not match renderer evidence"
            )));
        }
        let projection_open_end = find_tag_end(projection, 0)
            .ok_or_else(|| projection_error("prepared-math native projection is malformed"))?;
        let projection_tag = &projection[..=projection_open_end];
        if !projection_tag.starts_with("<g") || !projection.ends_with("</g>") {
            return Err(projection_error(
                "prepared-math native projection must be one SVG group",
            ));
        }

        let foreign_object_tag = &foreign_object[..=open_end - start];
        let x = parse_optional_position(foreign_object_tag, "x")?.unwrap_or(0.0);
        let y = parse_optional_position(foreign_object_tag, "y")?.unwrap_or(0.0);
        let width = parse_required_size(foreign_object_tag, "width")?;
        let height = parse_required_size(foreign_object_tag, "height")?;
        let projection_width =
            parse_required_size(projection_tag, "data-merman-prepared-math-width")?;
        let projection_height =
            parse_required_size(projection_tag, "data-merman-prepared-math-height")?;
        if projection_width > width + 0.01 || projection_height > height + 0.01 {
            return Err(projection_error(
                "prepared-math native projection exceeds its terminal foreignObject bounds",
            ));
        }
        let translate_x = x + (width - projection_width) / 2.0;
        let translate_y = y + (height - projection_height) / 2.0;

        let (replacement_start, replacement_end) =
            prepared_math_switch_wrapper(svg, cursor, start, end).unwrap_or((start, end));
        output.push_str(&svg[cursor..replacement_start]);
        output.push_str(r#"<g class=""#);
        output.push_str(PREPARED_MATH_NATIVE_CLASS);
        output.push_str(r#"" data-merman-prepared-math-occurrence=""#);
        output.push_str(occurrence_id);
        output.push_str(r#"" transform="translate("#);
        output.push_str(&format_number(translate_x));
        output.push(' ');
        output.push_str(&format_number(translate_y));
        output.push_str(r#")">"#);
        output.push_str(projection);
        output.push_str("</g>");
        cursor = replacement_end;
        projected = projected.saturating_add(1);
    }
    output.push_str(&svg[cursor..]);

    if projected == 0
        || (output.contains(PREPARED_MATH_CLASS_ATTRIBUTE)
            && output.contains(PREPARED_MATH_NATIVE_MARKER_PREFIX))
    {
        return Err(projection_error(
            "prepared-math occurrence was not bound to a terminal foreignObject",
        ));
    }
    if let Some((missing, _)) = evidence
        .entries()
        .iter()
        .zip(emitted.iter())
        .find(|(entry, emitted)| **emitted < entry.expected_emissions())
    {
        return Err(projection_error(format!(
            "prepared-math terminal is missing occurrence {}",
            missing.occurrence_id().as_str()
        )));
    }
    Ok(output)
}

fn prepared_math_switch_wrapper(
    svg: &str,
    cursor: usize,
    foreign_object_start: usize,
    foreign_object_end: usize,
) -> Option<(usize, usize)> {
    let switch_start = svg[cursor..foreign_object_start]
        .rfind("<switch")
        .map(|offset| cursor + offset)?;
    let switch_open_end = find_tag_end(svg, switch_start)?;
    if switch_open_end >= foreign_object_start
        || !svg[switch_start..=switch_open_end].contains(PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE)
        || svg[switch_open_end + 1..foreign_object_start].contains("</switch>")
    {
        return None;
    }
    let switch_close_start = foreign_object_end + svg[foreign_object_end..].find("</switch>")?;
    Some((switch_start, switch_close_start + "</switch>".len()))
}

fn parse_required_size(tag: &str, name: &str) -> Result<f64> {
    parse_optional_size(tag, name)?
        .ok_or_else(|| projection_error(format!("prepared-math terminal is missing `{name}`")))
}

fn parse_optional_position(tag: &str, name: &str) -> Result<Option<f64>> {
    parse_optional_finite_length(tag, name, true)
}

fn parse_optional_size(tag: &str, name: &str) -> Result<Option<f64>> {
    parse_optional_finite_length(tag, name, false)
}

fn parse_optional_finite_length(
    tag: &str,
    name: &str,
    allow_negative: bool,
) -> Result<Option<f64>> {
    let Some(raw) = extract_quoted_attr(tag, name) else {
        return Ok(None);
    };
    let raw = raw.strip_suffix("px").unwrap_or(raw);
    let value = raw.parse::<f64>().map_err(|_| {
        projection_error(format!(
            "prepared-math terminal `{name}` must be a finite pixel length"
        ))
    })?;
    if !value.is_finite() || (!allow_negative && value < 0.0) {
        let constraint = if allow_negative {
            "finite"
        } else {
            "finite non-negative"
        };
        return Err(projection_error(format!(
            "prepared-math terminal `{name}` must be a {constraint} length"
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

    fn evidence_for_projection(
        family_id: DiagramFamilyId,
        role: &'static str,
        index: usize,
        projection: Option<&str>,
        expected_emissions: usize,
    ) -> (
        crate::math::PreparedMathOccurrenceId,
        PreparedMathEvidenceLease,
    ) {
        let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(family_id, role, index);
        let expectation = projection.map_or_else(
            || {
                crate::math::PreparedMathExpectation::unavailable(
                    occurrence_id.clone(),
                    expected_emissions,
                )
            },
            |projection| {
                crate::math::PreparedMathExpectation::available(
                    occurrence_id.clone(),
                    PreparedMathProjectionFingerprint::from_projection(projection),
                    expected_emissions,
                )
            },
        );
        (
            occurrence_id,
            PreparedMathEvidenceLease::new(vec![expectation], Vec::new()),
        )
    }

    #[test]
    fn projection_replaces_one_terminal_foreign_object_and_centers_native_geometry() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="20" data-merman-prepared-math-height="10">"#,
            r#"<svg width="20" height="10"><path d="native"/></svg></g>"#,
        );
        let (occurrence_id, evidence) = evidence_for_projection(
            DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
            Some(projection),
            1,
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg"><g transform="translate(5 6)">"#,
                r#"<foreignObject x="10" y="20" width="24" height="18"><div>"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"<svg><path d="browser"/></svg>"#,
                r#"<template data-merman-prepared-math-projection="v1">{}"#,
                r#"</template></span></div></foreignObject></g></svg>"#,
            ),
            occurrence_id.as_str(),
            projection,
        );

        let output =
            project_prepared_math(&svg, Some(DiagramFamilyId::FLOWCHART), Some(&evidence)).unwrap();

        assert!(!output.contains("<foreignObject"), "{output}");
        assert!(!output.contains("browser"), "{output}");
        assert!(
            output.contains(r#"transform="translate(12 24)""#),
            "{output}"
        );
        assert!(output.contains(r#"d="native""#), "{output}");
        assert!(output.contains(r#"transform="translate(5 6)""#), "{output}");
        assert!(
            output.contains(&format!(
                r#"data-merman-prepared-math-occurrence="{}""#,
                occurrence_id.as_str()
            )),
            "{output}"
        );
    }

    #[test]
    fn projection_replaces_the_owned_switch_and_discards_raw_math_fallback() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="20" data-merman-prepared-math-height="10">"#,
            r#"<svg width="20" height="10"><path d="native"/></svg></g>"#,
        );
        let (occurrence_id, evidence) = evidence_for_projection(
            DiagramFamilyId::SEQUENCE,
            "actor-label",
            0,
            Some(projection),
            1,
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<switch data-merman-prepared-math-switch="v1">"#,
                r#"<foreignObject x="10" y="20" width="24" height="18"><div>"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"<template data-merman-prepared-math-projection="v1">{}"#,
                r#"</template></span></div></foreignObject>"#,
                r#"<text x="12" y="24">$$x$$</text></switch></svg>"#,
            ),
            occurrence_id.as_str(),
            projection,
        );

        let output = project_prepared_math(&svg, Some(DiagramFamilyId::SEQUENCE), Some(&evidence))
            .expect("the renderer-owned switch should collapse to its native projection");

        assert!(!output.contains("<switch"), "{output}");
        assert!(!output.contains("$$x$$"), "{output}");
        assert!(!output.contains("<foreignObject"), "{output}");
        assert!(output.contains(r#"d="native""#), "{output}");
    }

    #[test]
    fn projection_accepts_signed_finite_foreign_object_positions() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="20" data-merman-prepared-math-height="10">"#,
            r#"<svg width="20" height="10"><path d="native"/></svg></g>"#,
        );
        let (occurrence_id, evidence) = evidence_for_projection(
            DiagramFamilyId::FLOWCHART,
            "edge-label",
            0,
            Some(projection),
            1,
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<foreignObject x="-3.5" y="-4" width="24" height="18"><div>"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"<template data-merman-prepared-math-projection="v1">{}"#,
                r#"</template></span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
            projection,
        );

        let output = project_prepared_math(&svg, Some(DiagramFamilyId::FLOWCHART), Some(&evidence))
            .expect("negative x/y positions are valid SVG geometry");

        assert!(
            output.contains(r#"transform="translate(-1.5 0)""#),
            "{output}"
        );
    }

    #[test]
    fn projection_accepts_ceiled_sequence_bounds_for_fractional_native_geometry() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="9.14448" "#,
            r#"data-merman-prepared-math-height="18.28896">"#,
            r#"<svg width="9.14448" height="18.28896"><path d="native"/></svg></g>"#,
        );
        let (occurrence_id, evidence) = evidence_for_projection(
            DiagramFamilyId::SEQUENCE,
            "message-label",
            0,
            Some(projection),
            1,
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<foreignObject width="10" height="19"><div>"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"<template data-merman-prepared-math-projection="v1">{}"#,
                r#"</template></span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
            projection,
        );

        let output = project_prepared_math(&svg, Some(DiagramFamilyId::SEQUENCE), Some(&evidence))
            .expect("ceiled Sequence bounds must contain exact native projection geometry");

        assert!(!output.contains("<foreignObject"), "{output}");
        assert!(output.contains(r#"d="native""#), "{output}");
    }

    #[test]
    fn projection_rejects_browser_only_and_unowned_occurrences() {
        let (occurrence_id, evidence) =
            evidence_for_projection(DiagramFamilyId::SEQUENCE, "message-label", 0, None, 1);
        let browser_only = format!(
            concat!(
                r#"<svg><foreignObject width="10" height="10"><div>"#,
                r#"<span class="merman-prepared-math" "#,
                r#"data-merman-prepared-math-native="unavailable" "#,
                r#"data-merman-prepared-math-occurrence="{}"/>"#,
                r#"</div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
        );
        let error = project_prepared_math(
            &browser_only,
            Some(DiagramFamilyId::SEQUENCE),
            Some(&evidence),
        )
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
        let error = project_prepared_math(&forged, None, None)
            .expect_err("raw SVG cannot mint prepared-math capability");
        assert!(error.to_string().contains("renderer-owned family metadata"));

        let (_, other_evidence) = evidence_for_projection(
            DiagramFamilyId::SEQUENCE,
            "note-label",
            1,
            Some(
                r#"<g data-merman-prepared-math-width="1" data-merman-prepared-math-height="1"/>"#,
            ),
            1,
        );
        let error = project_prepared_math(
            &forged,
            Some(DiagramFamilyId::SEQUENCE),
            Some(&other_evidence),
        )
        .expect_err("one renderer occurrence cannot claim another occurrence's projection");
        assert!(error.to_string().contains("is not renderer-owned"));
    }

    #[test]
    fn projection_ignores_an_unowned_user_class_without_reserved_markers() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<foreignObject width="20" height="10"><div xmlns="http://www.w3.org/1999/xhtml">"#,
            r#"<span class="merman-prepared-math">user content</span>"#,
            r#"</div></foreignObject></svg>"#,
        );

        let output = project_prepared_math(svg, None, None)
            .expect("an unowned class name must not mint or deny renderer capability");

        assert_eq!(output, svg);
    }

    #[test]
    fn projection_preserves_an_unowned_user_class_beside_owned_math() {
        let projection = concat!(
            r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9">"#,
            r#"<path d="native"/></g>"#,
        );
        let (occurrence_id, evidence) = evidence_for_projection(
            DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
            Some(projection),
            1,
        );
        let svg = format!(
            concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<foreignObject width="20" height="10"><div>"#,
                r#"<span class="merman-prepared-math">user content</span>"#,
                r#"</div></foreignObject>"#,
                r#"<foreignObject width="10" height="10"><div>"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"<template data-merman-prepared-math-projection="v1">{}"#,
                r#"</template></span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
            projection,
        );

        let output = project_prepared_math(&svg, Some(DiagramFamilyId::FLOWCHART), Some(&evidence))
            .expect("unowned class tokens must not interfere with renderer-owned math");

        assert!(output.contains("user content"), "{output}");
        assert!(output.contains(r#"d="native""#), "{output}");
    }

    #[test]
    fn projection_rejects_legacy_browser_only_math_before_foreign_object_stripping() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<foreignObject width="20" height="10"><div xmlns="http://www.w3.org/1999/xhtml">"#,
            r#"<span data-merman-math-native="unavailable"><svg><path d="browser-only"/></svg></span>"#,
            r#"</div></foreignObject></svg>"#,
        );

        let error = project_prepared_math(svg, Some(DiagramFamilyId::CLASS), None)
            .expect_err("unprojected Class/Mindmap math must not be silently stripped");

        assert!(
            error
                .to_string()
                .contains("browser-only math has no renderer-owned native projection"),
            "{error}"
        );
    }
}
