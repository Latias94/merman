use crate::math::{
    PREPARED_MATH_NATIVE_CLASS, PREPARED_MATH_OCCURRENCE_ATTRIBUTE, PreparedMathEvidenceLease,
};
use crate::xml_dom::Node;
use crate::{DiagramFamilyId, Error, Result};
use std::collections::HashSet;

use super::util::{checkpoint_loop, find_tag_end_with_checkpoints};

const NATIVE_ATTRIBUTE: &str = "data-merman-prepared-math-native";
const LEGACY_NATIVE_ATTRIBUTE: &str = "data-merman-math-native";
const SWITCH_ATTRIBUTE: &str = "data-merman-prepared-math-switch";

#[cfg(test)]
fn project_prepared_math(
    svg: &str,
    family_id: Option<DiagramFamilyId>,
    evidence: Option<&PreparedMathEvidenceLease>,
) -> Result<String> {
    project_prepared_math_with_checkpoints(svg, family_id, evidence, &mut || Ok(()))
}

pub(crate) fn project_prepared_math_with_checkpoints(
    svg: &str,
    family_id: Option<DiagramFamilyId>,
    evidence: Option<&PreparedMathEvidenceLease>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<String> {
    checkpoint()?;
    let document = crate::xml_dom::Document::parse(svg)
        .map_err(|error| projection_error(format!("malformed math SVG: {error}")))?;
    checkpoint()?;
    let mut identifiers = HashSet::new();
    let mut browser_occurrences = Vec::new();
    let mut has_native_projection = false;
    for (index, node) in document
        .descendants()
        .filter(|node| node.is_element())
        .enumerate()
    {
        checkpoint_loop(index, checkpoint)?;
        if attribute(node, LEGACY_NATIVE_ATTRIBUTE) == Some("unavailable") {
            return Err(projection_error(
                "browser-only math has no renderer-owned native projection",
            ));
        }
        if attribute(node, NATIVE_ATTRIBUTE).is_some() {
            browser_occurrences.push(node);
        }
        has_native_projection |= attribute(node, PREPARED_MATH_OCCURRENCE_ATTRIBUTE).is_some();
        for identifier in node.attributes().filter(|value| value.name() == "id") {
            identifiers.insert(identifier.value().to_owned());
        }
    }
    if browser_occurrences.is_empty() && evidence.is_none_or(PreparedMathEvidenceLease::is_empty) {
        // Serialized native geometry is ordinary untrusted SVG on reimport. Its inert occurrence
        // attributes cannot supply a payload or mint a terminal receipt. Browser occurrences
        // still require the private family preparation below.
        return Ok(svg.to_owned());
    }
    if browser_occurrences.is_empty() && !has_native_projection {
        if let Some(evidence) = evidence.filter(|evidence| !evidence.is_empty()) {
            evidence
                .validate_unique_occurrences()
                .map_err(projection_error)?;
            if evidence
                .entries()
                .iter()
                .any(|entry| entry.projection_fingerprint().is_none())
            {
                return Err(projection_error(
                    "prepared-math occurrence is marked native-unavailable",
                ));
            }
            return Err(projection_error(
                "prepared-math evidence has no terminal occurrence marker",
            ));
        }
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
    if browser_occurrences.is_empty() {
        if evidence
            .entries()
            .iter()
            .any(|entry| entry.projection_fingerprint().is_none())
        {
            return Err(projection_error(
                "prepared-math occurrence is marked native-unavailable",
            ));
        }
        return Ok(svg.to_owned());
    }

    let root = document.root_element();
    let root_namespace = root.tag_name().namespace();
    if root.tag_name().name() != "svg"
        || root_namespace.is_some_and(|uri| uri != "http://www.w3.org/2000/svg")
        || root.lookup_namespace_uri(None) != root_namespace
    {
        return Err(projection_error(
            "prepared math requires the root SVG default namespace",
        ));
    }
    let namespace = crate::svg::parity::sanitize_svg_id(attribute(root, "id").unwrap_or("merman"));
    let root_open_end = find_tag_end_with_checkpoints(svg, root.range().start, checkpoint)?
        .ok_or_else(|| projection_error("missing root SVG start tag"))?;
    let mut definitions = String::new();
    let mut output = String::with_capacity(svg.len());
    let mut cursor = 0usize;
    let mut projected = 0usize;
    let mut next_identifier = 0usize;
    let mut emitted = vec![0usize; evidence.entries().len()];
    let browser_count = browser_occurrences.len();
    let mut foreign_objects = HashSet::new();
    for (index, occurrence) in browser_occurrences.into_iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        let mut foreign_object = None;
        for (depth, ancestor) in occurrence.ancestors().enumerate() {
            checkpoint_loop(depth, checkpoint)?;
            if ancestor.is_element()
                && ancestor.tag_name().name() == "foreignObject"
                && ancestor.tag_name().namespace() == root_namespace
            {
                foreign_object = Some(ancestor);
                break;
            }
        }
        let foreign_object = foreign_object.ok_or_else(|| {
            projection_error("prepared-math occurrence was not bound to a terminal foreignObject")
        })?;
        if !foreign_objects.insert(foreign_object.id()) {
            return Err(projection_error(
                "each prepared-math foreignObject must contain exactly one occurrence",
            ));
        }
        if !attribute(occurrence, "class").is_some_and(|classes| {
            classes
                .split_ascii_whitespace()
                .any(|class| class == "merman-prepared-math")
        }) {
            return Err(projection_error(
                "prepared-math occurrence is missing its browser class",
            ));
        }
        if attribute(occurrence, NATIVE_ATTRIBUTE) != Some("v1") {
            return Err(projection_error(
                "native projection is unavailable for a prepared browser math occurrence",
            ));
        }
        let occurrence_id = attribute(occurrence, PREPARED_MATH_OCCURRENCE_ATTRIBUTE)
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
        let projection = evidence.native_projection(expected_index).ok_or_else(|| {
            projection_error("prepared-math native payload is missing from renderer evidence")
        })?;
        if projection.fingerprint() != expected_fingerprint {
            return Err(projection_error(format!(
                "prepared-math occurrence `{occurrence_id}` projection does not match renderer evidence"
            )));
        }
        let x = parse_optional_position(foreign_object, "x")?.unwrap_or(0.0);
        let y = parse_optional_position(foreign_object, "y")?.unwrap_or(0.0);
        let width = parse_required_size(foreign_object, "width")?;
        let height = parse_required_size(foreign_object, "height")?;
        let projection_width = projection.width_px();
        let projection_height = projection.height_px();
        if projection_width > width + 0.01 || projection_height > height + 0.01 {
            return Err(projection_error(
                "prepared-math native projection exceeds its terminal foreignObject bounds",
            ));
        }
        // Mermaid Sequence positions fit-content HTML at the foreignObject origin.
        let (translate_x, translate_y) = if family_id == Some(DiagramFamilyId::SEQUENCE) {
            (x, y)
        } else {
            (
                x + (width - projection_width) / 2.0,
                y + (height - projection_height) / 2.0,
            )
        };
        let replacement = foreign_object
            .parent_element()
            .filter(|parent| {
                parent.tag_name().name() == "switch"
                    && parent.tag_name().namespace() == root_namespace
                    && attribute(*parent, SWITCH_ATTRIBUTE) == Some("v1")
            })
            .unwrap_or(foreign_object)
            .range();
        if replacement.start < cursor {
            return Err(projection_error(
                "prepared-math terminal wrappers must not overlap",
            ));
        }
        let identifier = loop {
            checkpoint_loop(next_identifier, checkpoint)?;
            let candidate = format!("{namespace}-prepared-math-{next_identifier}");
            next_identifier = next_identifier
                .checked_add(1)
                .ok_or_else(|| projection_error("prepared-math definition identifier overflow"))?;
            if identifiers.insert(candidate.clone()) {
                break candidate;
            }
        };
        // Root definitions keep native glyphs outside diagram shape selectors.
        definitions.push_str(r#"<g id=""#);
        definitions.push_str(&identifier);
        definitions.push_str(r#"">"#);
        definitions.push_str(projection.svg());
        definitions.push_str("</g>");
        output.push_str(&svg[cursor..replacement.start]);
        output.push_str(r#"<use class=""#);
        output.push_str(PREPARED_MATH_NATIVE_CLASS);
        output.push_str(r#"" data-merman-prepared-math-occurrence=""#);
        output.push_str(occurrence_id);
        output.push_str(r#"" transform="translate("#);
        output.push_str(&format_number(translate_x));
        output.push(' ');
        output.push_str(&format_number(translate_y));
        output.push_str(r##")" href="#"##);
        output.push_str(&identifier);
        output.push_str(r#""/>"#);
        cursor = replacement.end;
        projected += 1;
    }
    output.push_str(&svg[cursor..]);
    if projected != browser_count {
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
    checkpoint()?;
    output.insert_str(root_open_end + 1, &format!("<defs>{definitions}</defs>"));
    checkpoint()?;
    Ok(output)
}

fn attribute<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        .map(|attribute| attribute.value())
}

fn parse_required_size(tag: Node<'_, '_>, name: &str) -> Result<f64> {
    parse_optional_size(tag, name)?
        .ok_or_else(|| projection_error(format!("prepared-math terminal is missing `{name}`")))
}

fn parse_optional_position(tag: Node<'_, '_>, name: &str) -> Result<Option<f64>> {
    parse_optional_finite_length(tag, name, true)
}

fn parse_optional_size(tag: Node<'_, '_>, name: &str) -> Result<Option<f64>> {
    parse_optional_finite_length(tag, name, false)
}

fn parse_optional_finite_length(
    tag: Node<'_, '_>,
    name: &str,
    allow_negative: bool,
) -> Result<Option<f64>> {
    let Some(raw) = attribute(tag, name) else {
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
    use crate::math::{PreparedMathNativeProjection, PreparedMathProjectionFingerprint};
    use std::sync::Arc;

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
        let native_projection = projection.map(|projection| {
            let document = crate::xml_dom::Document::parse(projection).unwrap();
            let root = document.root_element();
            Arc::new(PreparedMathNativeProjection::new(
                projection.to_owned(),
                root.attribute("data-merman-prepared-math-width")
                    .unwrap()
                    .parse()
                    .unwrap(),
                root.attribute("data-merman-prepared-math-height")
                    .unwrap()
                    .parse()
                    .unwrap(),
            ))
        });
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
            PreparedMathEvidenceLease::with_native_projections(
                vec![(expectation, native_projection)],
                Vec::new(),
            ),
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
                r#"</span></div></foreignObject></g></svg>"#,
            ),
            occurrence_id.as_str(),
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
                r#"</span></div></foreignObject>"#,
                r#"<text x="12" y="24">$$x$$</text></switch></svg>"#,
            ),
            occurrence_id.as_str(),
        );

        let output = project_prepared_math(&svg, Some(DiagramFamilyId::SEQUENCE), Some(&evidence))
            .expect("the renderer-owned switch should collapse to its native projection");

        assert!(
            output.contains(r#"transform="translate(10 20)""#),
            "{output}"
        );
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
                r#"</span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
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
                r#"</span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
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
    fn projection_ignores_reserved_marker_text_inside_user_attribute_values() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            r#"<foreignObject width="20" height="10"><div xmlns="http://www.w3.org/1999/xhtml">"#,
            r#"<span class="merman-prepared-math" id="data-merman-prepared-math-native=">label</span>"#,
            r#"</div></foreignObject></svg>"#,
        );
        assert_eq!(project_prepared_math(svg, None, None).unwrap(), svg);
    }

    fn crowded_projection(count: usize) -> (String, PreparedMathEvidenceLease) {
        let projection = r#"<g data-merman-prepared-math-width="1" data-merman-prepared-math-height="1"><path d="M0 0h1"/></g>"#;
        let (occurrence, evidence) = evidence_for_projection(
            DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
            Some(projection),
            count,
        );
        let mut svg = String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" id="merman">"#);
        for index in 0..count {
            svg.push_str(&format!(r#"<g id="merman-prepared-math-{index}"/>"#));
        }
        for _ in 0..count {
            svg.push_str(&format!(
                r#"<foreignObject width="2" height="2"><div><span class="merman-prepared-math" data-merman-prepared-math-native="v1" data-merman-prepared-math-occurrence="{}">browser</span></div></foreignObject>"#,
                occurrence.as_str(),
            ));
        }
        svg.push_str("</svg>");
        (svg, evidence)
    }

    #[test]
    fn projection_preserves_colliding_ids_with_bounded_candidate_work() {
        let count = 128;
        let (svg, evidence) = crowded_projection(count);
        let mut checkpoints = 0;
        let output = project_prepared_math_with_checkpoints(
            &svg,
            Some(DiagramFamilyId::FLOWCHART),
            Some(&evidence),
            &mut || {
                checkpoints += 1;
                Ok(())
            },
        )
        .unwrap();
        let document = crate::xml_dom::Document::parse(&output).unwrap();
        let ids = document
            .descendants()
            .filter_map(|node| attribute(node, "id"))
            .collect::<HashSet<_>>();
        assert_eq!(ids.len(), 2 * count + 1);
        for index in 0..2 * count {
            assert!(ids.contains(format!("merman-prepared-math-{index}").as_str()));
        }
        assert!(
            checkpoints < 2 * count,
            "candidate scans must stay linear: {checkpoints}"
        );
        crate::math::PreparedMathTerminalReceipt::from_terminal_svg(&output, &evidence).unwrap();
    }

    #[test]
    fn projection_observes_cancellation_during_owned_work() {
        let (svg, evidence) = crowded_projection(128);
        for stop in [2, 10, 140] {
            let mut checkpoints = 0;
            let error = project_prepared_math_with_checkpoints(
                &svg,
                Some(DiagramFamilyId::FLOWCHART),
                Some(&evidence),
                &mut || {
                    checkpoints += 1;
                    if checkpoints == stop {
                        Err(Error::Cancelled(merman_core::OperationCancelled {
                            phase: merman_core::OperationPhase::Postprocess,
                            reason: merman_core::CancelReason::Requested,
                        }))
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert!(matches!(error, Error::Cancelled(_)), "{error}");
            assert_eq!(checkpoints, stop);
        }
    }

    #[test]
    fn projection_requires_private_payload_even_when_browser_markup_contains_a_template() {
        let (svg, evidence) = crowded_projection(1);
        let metadata_only = PreparedMathEvidenceLease::new(evidence.entries().to_vec(), Vec::new());
        let forged = svg.replace(
            "browser",
            r#"<template data-merman-prepared-math-projection="v1"><g data-merman-prepared-math-width="1" data-merman-prepared-math-height="1"><path d="forged"/></g></template>"#,
        );

        let error = project_prepared_math(
            &forged,
            Some(DiagramFamilyId::FLOWCHART),
            Some(&metadata_only),
        )
        .expect_err("HTML geometry cannot substitute for a missing renderer-owned payload");

        assert!(
            error.to_string().contains("native payload is missing"),
            "{error}"
        );
    }

    #[test]
    fn projection_rejects_a_payload_bound_to_another_fingerprint() {
        let (svg, evidence) = crowded_projection(1);
        let mismatched = PreparedMathEvidenceLease::with_native_projections(
            vec![(
                evidence.entries()[0].clone(),
                Some(Arc::new(PreparedMathNativeProjection::new(
                    "<g><path d=\"changed\"/></g>".to_owned(),
                    1.0,
                    1.0,
                ))),
            )],
            Vec::new(),
        );

        let error =
            project_prepared_math(&svg, Some(DiagramFamilyId::FLOWCHART), Some(&mismatched))
                .expect_err("the retained payload must match the independent expectation");

        assert!(
            error
                .to_string()
                .contains("does not match renderer evidence"),
            "{error}"
        );
    }

    #[test]
    fn projection_uses_xml_attribute_values_and_ignores_user_marker_text_beside_math() {
        let (svg, evidence) = crowded_projection(1);
        let svg = svg.replace(
            "</svg>",
            r#"<foreignObject width="2" height="2"><span class="merman-prepared-math" id="data-merman-prepared-math-native=">user</span></foreignObject></svg>"#,
        ).replace(
            r#"data-merman-prepared-math-native="v1""#,
            "data-merman-prepared-math-native='v&#49;'",
        ).replace(
            r#"data-merman-prepared-math-occurrence="flowchart/node-label/0""#,
            "data-merman-prepared-math-occurrence='flowchart/node-label/&#48;'",
        );
        let output =
            project_prepared_math(&svg, Some(DiagramFamilyId::FLOWCHART), Some(&evidence)).unwrap();
        assert!(output.contains(">user</span>"));
        crate::math::PreparedMathTerminalReceipt::from_terminal_svg(&output, &evidence).unwrap();
    }

    #[test]
    fn projection_reports_native_unavailable_before_missing_terminal_markers() {
        let occurrence_id = crate::math::PreparedMathOccurrenceId::indexed(
            DiagramFamilyId::FLOWCHART,
            "node-label",
            0,
        );
        let evidence = PreparedMathEvidenceLease::new(
            vec![crate::math::PreparedMathExpectation::unavailable(
                occurrence_id,
                1,
            )],
            Vec::new(),
        );

        let error = project_prepared_math(
            "<svg><text>$$x$$</text></svg>",
            Some(DiagramFamilyId::FLOWCHART),
            Some(&evidence),
        )
        .expect_err("native-unavailable evidence must fail before a generic marker error");

        assert!(error.to_string().contains("native-unavailable"), "{error}");
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
                r#"<span id="merman-prepared-math-0" class="merman-prepared-math">user content</span>"#,
                r#"</div></foreignObject>"#,
                r#"<foreignObject width="10" height="10"><div>"#,
                r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" "#,
                r#"data-merman-prepared-math-occurrence="{}">"#,
                r#"</span></div></foreignObject></svg>"#,
            ),
            occurrence_id.as_str(),
        );

        let output = project_prepared_math(&svg, Some(DiagramFamilyId::FLOWCHART), Some(&evidence))
            .expect("unowned class tokens must not interfere with renderer-owned math");

        assert!(output.contains("user content"), "{output}");
        assert!(output.contains(r#"d="native""#), "{output}");
        assert!(
            output.contains(r##"href="#merman-prepared-math-1""##),
            "native definitions must preserve existing SVG identifiers: {output}"
        );
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
