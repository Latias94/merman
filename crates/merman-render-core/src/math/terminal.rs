use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use roxmltree::{Document, Node};
use sha2::{Digest as _, Sha256};

use super::PreparedMathProjectionFingerprint;
use super::evidence::update_len_prefixed;
use super::{PREPARED_MATH_NATIVE_CLASS, PreparedMathEvidenceLease, PreparedMathExpectation};

#[cfg(test)]
const PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE: &str = r#"class="merman-prepared-math-native""#;

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedMathTerminalOccurrenceReceipt {
    expectation: PreparedMathExpectation,
    emitted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMathTerminalReceipt {
    artifact_digest: [u8; 32],
    occurrences: Arc<[PreparedMathTerminalOccurrenceReceipt]>,
}

impl PreparedMathTerminalReceipt {
    /// Checks that the final SVG still contains the prepared math geometry and its instances.
    pub fn from_terminal_svg(
        svg: &str,
        evidence: &PreparedMathEvidenceLease,
    ) -> Result<Option<Self>, String> {
        if evidence.is_empty() {
            return Ok(None);
        }
        evidence.validate_unique_occurrences()?;
        if evidence
            .entries()
            .iter()
            .any(|entry| entry.projection_fingerprint().is_none())
        {
            return Err("prepared-math occurrence is marked native-unavailable".to_owned());
        }

        let document = Document::parse(svg)
            .map_err(|error| format!("prepared-math terminal SVG is malformed: {error}"))?;
        let root = document.root_element();
        if !native_svg_element(root, "svg") {
            return Err("prepared-math terminal requires an SVG root".to_owned());
        }
        let namespace_context = root
            .namespaces()
            .map(|namespace| (namespace.name(), namespace.uri()))
            .collect::<HashMap<_, _>>();
        let owned_element = |node: Node<'_, '_>, name: &str| {
            node.tag_name().name() == name
                && node.tag_name().namespace() == root.tag_name().namespace()
        };
        let mut identifiers = HashMap::new();
        let mut references = HashMap::<&str, usize>::new();
        for node in document.descendants().filter(|node| node.is_element()) {
            if let Some(identifier) = node.attribute("id")
                && identifiers.insert(identifier, node).is_some()
            {
                return Err(format!("duplicate prepared-math SVG id `{identifier}`"));
            }
            if native_svg_element(node, "use")
                && let Some(target) = use_target(node)
            {
                *references.entry(target).or_default() += 1;
            }
        }
        let mut used_definitions = HashSet::new();
        let mut emitted = vec![0usize; evidence.entries().len()];
        let mut instance_indices = Vec::new();
        for (index, node) in document
            .descendants()
            .filter(|node| node.is_element())
            .enumerate()
        {
            let Some(occurrence_id) =
                unqualified_attribute(node, super::PREPARED_MATH_OCCURRENCE_ATTRIBUTE)
            else {
                // User classes do not establish renderer ownership. Missing owned instances are
                // rejected by the expected emission counts below.
                continue;
            };
            let (expected_index, expected) =
                evidence.expectation(occurrence_id)?.ok_or_else(|| {
                    format!("prepared-math terminal emitted unknown occurrence `{occurrence_id}`")
                })?;
            if !unqualified_attribute(node, "class").is_some_and(|class| {
                class
                    .split_ascii_whitespace()
                    .any(|token| token == PREPARED_MATH_NATIVE_CLASS)
            }) {
                return Err(format!(
                    "prepared-math terminal occurrence `{occurrence_id}` is missing its native class"
                ));
            }
            emitted[expected_index] = emitted[expected_index].saturating_add(1);
            if emitted[expected_index] > expected.expected_emissions() {
                return Err(format!(
                    "prepared-math terminal emitted duplicate occurrence `{occurrence_id}`"
                ));
            }
            let owner = {
                if !owned_element(node, "use") {
                    return Err(
                        "prepared-math instance must reference a root definition".to_owned()
                    );
                }
                let target = use_target(node).ok_or_else(|| {
                    "prepared-math instance requires a local definition".to_owned()
                })?;
                if references.get(target) != Some(&1) || !used_definitions.insert(target) {
                    return Err(format!(
                        "prepared-math definition `{target}` requires one instance"
                    ));
                }
                let definition = *identifiers
                    .get(target)
                    .ok_or_else(|| format!("prepared-math definition `{target}` is missing"))?;
                let root_definition = definition.parent_element().is_some_and(|parent| {
                    owned_element(parent, "defs") && parent.parent_element() == Some(root)
                });
                if !root_definition || !owned_element(definition, "g") {
                    return Err("prepared-math definition must be a root-owned group".to_owned());
                }
                if definition
                    .attributes()
                    .any(|attribute| attribute.namespace().is_some() || attribute.name() != "id")
                    || definition
                        .parent_element()
                        .is_some_and(|parent| parent.attributes().next().is_some())
                    || !same_namespace_context(definition, &namespace_context)
                    || definition
                        .parent_element()
                        .is_some_and(|parent| !same_namespace_context(parent, &namespace_context))
                {
                    return Err(
                        "prepared-math definition wrappers must preserve projection paint"
                            .to_owned(),
                    );
                }
                if node.children().any(|child| child.is_element())
                    || node
                        .ancestors()
                        .filter(|ancestor| ancestor.is_element())
                        .any(|ancestor| {
                            ancestor.tag_name().namespace() != root.tag_name().namespace()
                                || matches!(
                                    ancestor.tag_name().name(),
                                    "defs"
                                        | "symbol"
                                        | "clipPath"
                                        | "mask"
                                        | "marker"
                                        | "pattern"
                                        | "filter"
                                )
                        })
                {
                    return Err("prepared-math instance must be a visible leaf".to_owned());
                }
                definition
            };
            let mut children = owner.children().filter(|child| child.is_element());
            let projection = children.next().ok_or_else(|| {
                format!("prepared-math terminal occurrence `{occurrence_id}` has no projection")
            })?;
            if !owned_element(projection, "g") {
                return Err("prepared-math projection requires an SVG group".to_owned());
            }
            if children.next().is_some() {
                return Err(format!(
                    "prepared-math terminal occurrence `{occurrence_id}` has duplicate projections"
                ));
            }
            let actual =
                PreparedMathProjectionFingerprint::from_projection(&svg[projection.range()]);
            if Some(actual) != expected.projection_fingerprint() {
                return Err(format!(
                    "prepared-math terminal occurrence `{occurrence_id}` has a mismatched projection"
                ));
            }
            instance_indices.push(index);
        }

        if let Some((missing, _)) = evidence
            .entries()
            .iter()
            .zip(emitted.iter())
            .find(|(entry, emitted)| **emitted < entry.expected_emissions())
        {
            return Err(format!(
                "prepared-math terminal is missing occurrence `{}`",
                missing.occurrence_id().as_str()
            ));
        }

        let plan = crate::svg_validation::plan_svg_occurrences_for_receipt(svg)
            .map_err(|error| format!("prepared-math reference expansion is invalid: {error}"))?;
        if instance_indices
            .into_iter()
            .any(|index| plan.raw_element_occurrences().get(index) != Some(&1))
        {
            return Err("prepared-math occurrence must not be indirectly instantiated".to_owned());
        }

        let occurrences = evidence
            .entries()
            .iter()
            .cloned()
            .zip(emitted)
            .map(
                |(expectation, emitted)| PreparedMathTerminalOccurrenceReceipt {
                    expectation,
                    emitted,
                },
            )
            .collect::<Vec<_>>();
        Ok(Some(Self {
            artifact_digest: artifact_digest(svg),
            occurrences: occurrences.into(),
        }))
    }

    pub fn occurrence_count(&self) -> usize {
        self.occurrences.len()
    }

    pub fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }
}

fn native_svg_element(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node
            .tag_name()
            .namespace()
            .is_none_or(|uri| uri == "http://www.w3.org/2000/svg")
}

fn use_target<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    // Match usvg and the terminal reference plan: SVG 2 href takes precedence over XLink.
    let href = unqualified_attribute(node, "href")
        .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))?;
    svgtypes::IRI::from_str(href).ok().map(|iri| iri.0)
}

fn unqualified_attribute<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        .map(|attribute| attribute.value())
}

fn same_namespace_context(node: Node<'_, '_>, expected: &HashMap<Option<&str>, &str>) -> bool {
    node.namespaces().len() == expected.len()
        && node
            .namespaces()
            .all(|namespace| expected.get(&namespace.name()) == Some(&namespace.uri()))
}

fn artifact_digest(svg: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman-prepared-math-terminal-artifact-v1");
    update_len_prefixed(&mut hasher, svg.as_bytes());
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::PreparedMathOccurrenceId;
    use merman_model::family::DiagramFamilyId;

    fn available_evidence(
        occurrence_id: PreparedMathOccurrenceId,
        projection: &str,
        expected_emissions: usize,
    ) -> PreparedMathEvidenceLease {
        PreparedMathEvidenceLease::new(
            vec![PreparedMathExpectation::available(
                occurrence_id,
                PreparedMathProjectionFingerprint::from_projection(projection),
                expected_emissions,
            )],
            Vec::new(),
        )
    }

    fn terminal_occurrence(
        occurrence_id: &PreparedMathOccurrenceId,
        projection: &str,
        index: usize,
    ) -> String {
        format!(
            concat!(
                r#"<defs><g id="math-{index}">{projection}</g></defs>"#,
                r##"<use class="merman-prepared-math-native" data-merman-prepared-math-occurrence="{}" href="#math-{index}"/>"##,
            ),
            occurrence_id.as_str(),
            index = index,
            projection = projection,
        )
    }

    #[test]
    fn terminal_receipt_binds_exact_artifact_and_expected_emission_count() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::SEQUENCE, "actor-label", 0);
        let projection = r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 2);
        let occurrence = terminal_occurrence(&occurrence_id, projection, 0);
        let repeated = terminal_occurrence(&occurrence_id, projection, 1);
        let svg = format!(r#"<svg>{occurrence}{repeated}</svg>"#);

        let receipt = PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence)
            .expect("valid terminal evidence")
            .expect("non-empty terminal receipt");

        assert_eq!(receipt.occurrence_count(), 1);
        assert_ne!(receipt.artifact_digest(), [0; 32]);
        let changed_svg = format!(r#"<svg data-version="2">{occurrence}{repeated}</svg>"#);
        let changed = PreparedMathTerminalReceipt::from_terminal_svg(&changed_svg, &evidence)
            .expect("same occurrence evidence in a different exact artifact")
            .expect("non-empty terminal receipt");
        assert_ne!(receipt.artifact_digest(), changed.artifact_digest());
    }

    #[test]
    fn terminal_receipt_ignores_user_native_classes_without_owned_occurrences() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 1);
        let occurrence = terminal_occurrence(&occurrence_id, projection, 0);
        let svg = format!(
            concat!(
                r#"<svg><g class="node merman-prepared-math-native"><path d="M0 0h2"/></g>"#,
                r#"<defs><path id="user-shape" d="M0 0h3"/></defs>"#,
                r##"<use class="merman-prepared-math-native" href="#user-shape"/>"##,
                "{occurrence}</svg>",
            ),
            occurrence = occurrence,
        );

        let receipt = PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence)
            .expect("ordinary user classes must not claim renderer-owned occurrences")
            .expect("the owned math occurrence still requires a receipt");

        assert_eq!(receipt.occurrence_count(), 1);
        assert_eq!(receipt.occurrences[0].emitted, 1);
    }

    #[test]
    fn terminal_receipt_rejects_owned_occurrences_without_native_classes() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 1);
        let occurrence = terminal_occurrence(&occurrence_id, projection, 0);
        for replacement in ["", r#"class="ordinary-user-class""#] {
            let svg = format!(
                "<svg>{}</svg>",
                occurrence.replace(PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE, replacement)
            );
            let error = PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence)
                .expect_err("an occurrence identity must not bypass native class validation");

            assert!(error.contains("missing its native class"), "{error}");
        }

        let unmarked = terminal_occurrence(&occurrence_id, projection, 1)
            .replace(PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE, "");
        let error = PreparedMathTerminalReceipt::from_terminal_svg(
            &format!("<svg>{occurrence}{unmarked}</svg>"),
            &evidence,
        )
        .expect_err("an additional owned instance cannot hide by dropping its class");
        assert!(error.contains("missing its native class"), "{error}");
    }

    #[test]
    fn terminal_receipt_rejects_missing_owned_identities_beside_user_native_classes() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 1);
        let occurrence = terminal_occurrence(&occurrence_id, projection, 0);
        let identity = format!(
            r#" data-merman-prepared-math-occurrence="{}""#,
            occurrence_id.as_str()
        );
        for missing in [
            occurrence.replace(&identity, ""),
            occurrence
                .replace(&identity, "")
                .replace(PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE, ""),
            String::new(),
        ] {
            let svg = format!(r#"<svg><g class="merman-prepared-math-native"/>{missing}</svg>"#);
            let error = PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence)
                .expect_err("user classes cannot replace a missing renderer-owned occurrence");

            assert!(error.contains("missing occurrence"), "{error}");
        }
    }

    #[test]
    fn terminal_receipt_rejects_missing_duplicate_unavailable_and_mismatched_occurrences() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 1);

        let error = PreparedMathTerminalReceipt::from_terminal_svg("<svg/>", &evidence)
            .expect_err("missing occurrence must fail closed");
        assert!(error.contains("missing occurrence"), "{error}");

        let occurrence = terminal_occurrence(&occurrence_id, projection, 0);
        let repeated = terminal_occurrence(&occurrence_id, projection, 1);
        let duplicated = format!(r#"<svg>{occurrence}{repeated}</svg>"#);
        let error = PreparedMathTerminalReceipt::from_terminal_svg(&duplicated, &evidence)
            .expect_err("unexpected duplicate occurrence must fail closed");
        assert!(error.contains("duplicate occurrence"), "{error}");

        let unavailable = PreparedMathEvidenceLease::new(
            vec![PreparedMathExpectation::unavailable(
                occurrence_id.clone(),
                1,
            )],
            Vec::new(),
        );
        let error = PreparedMathTerminalReceipt::from_terminal_svg("<svg/>", &unavailable)
            .expect_err("native-unavailable occurrence must fail closed");
        assert!(error.contains("native-unavailable"), "{error}");

        let mismatched = terminal_occurrence(
            &occurrence_id,
            r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><rect/></g>"#,
            0,
        );
        let error = PreparedMathTerminalReceipt::from_terminal_svg(
            &format!("<svg>{mismatched}</svg>"),
            &evidence,
        )
        .expect_err("mismatched projection must fail closed");
        assert!(error.contains("mismatched projection"), "{error}");
    }

    #[test]
    fn terminal_receipt_requires_one_root_definition_per_visible_instance() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence_id.clone(), projection, 1);
        let valid = format!(
            "<svg>{}</svg>",
            terminal_occurrence(&occurrence_id, projection, 0)
        );
        PreparedMathTerminalReceipt::from_terminal_svg(&valid, &evidence).unwrap();
        for invalid in [
            valid.replace(r##"href="#math-0""##, r##"href="#missing""##),
            valid
                .replace("<defs>", "<g><defs>")
                .replace("</defs>", "</defs></g>"),
            valid.replace("</svg>", r##"<use href="#math-0"/></svg>"##),
            valid.replace(
                "</svg>",
                r##"<use xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="#math-0"/></svg>"##,
            ),
            valid.replace("</svg>", r#"<g id="math-0"/></svg>"#),
            valid.replace(r#"id="math-0""#, r#"id="math-0" transform="scale(0)""#),
            valid.replace("<defs>", r#"<defs opacity="0">"#),
            valid
                .replace("<use ", "<defs><use ")
                .replace("/></svg>", "/></defs></svg>"),
            format!(
                r#"<svg><g class="merman-prepared-math-native" data-merman-prepared-math-occurrence="{}">{projection}</g></svg>"#,
                occurrence_id.as_str(),
            ),
        ] {
            assert!(
                PreparedMathTerminalReceipt::from_terminal_svg(&invalid, &evidence).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn terminal_receipt_follows_svg_use_target_precedence() {
        let occurrence =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence.clone(), projection, 1);
        let valid = format!(
            "<svg xmlns:xlink=\"http://www.w3.org/1999/xlink\">{}</svg>",
            terminal_occurrence(&occurrence, projection, 0)
        );
        for href in [
            r##"href="#math-0" xlink:href="#missing""##,
            r##"xlink:href="#missing" href="#math-0""##,
            r##"xml:href="#missing" href="#math-0""##,
            r##"xlink:href="#math-0""##,
        ] {
            let svg = valid.replace(r##"href="#math-0""##, href);
            PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence).unwrap();
        }
        for href in [
            r##"href="#missing" xlink:href="#math-0""##,
            r##"xml:href="#math-0""##,
            r##"xmlns:s="http://www.w3.org/2000/svg" s:href="#math-0""##,
            r##"xmlns:q="urn:other" q:href="#math-0""##,
        ] {
            let svg = valid.replace(r##"href="#math-0""##, href);
            assert!(
                PreparedMathTerminalReceipt::from_terminal_svg(&svg, &evidence).is_err(),
                "{href}"
            );
        }
    }

    #[test]
    fn terminal_receipt_rejects_namespace_context_mutations_without_changed_bytes() {
        let occurrence =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence.clone(), projection, 1);
        let valid = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">{}</svg>"#,
            terminal_occurrence(&occurrence, projection, 0),
        );
        PreparedMathTerminalReceipt::from_terminal_svg(&valid, &evidence).unwrap();
        for changed in [
            valid.replace("<defs>", r#"<defs xmlns="urn:other">"#),
            valid.replace(r#"id="math-0""#, r#"id="math-0" xmlns="urn:other""#),
            valid.replace("<use ", r#"<use xmlns="urn:other" "#),
            valid.replace("<defs>", r#"<defs xmlns:q="urn:other">"#),
        ] {
            assert!(
                PreparedMathTerminalReceipt::from_terminal_svg(&changed, &evidence).is_err(),
                "{changed}",
            );
        }
    }

    #[test]
    fn terminal_receipt_rejects_indirect_copies_of_owned_instances() {
        let occurrence =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence.clone(), projection, 1);
        let valid = format!(
            "<svg>{}</svg>",
            terminal_occurrence(&occurrence, projection, 0)
        );
        let carrier = valid
            .replace("<use ", r#"<g id="carrier"><use "#)
            .replace("/></svg>", "/></g></svg>");
        PreparedMathTerminalReceipt::from_terminal_svg(&carrier, &evidence).unwrap();
        for href in [
            r##"href="#carrier""##,
            r##"xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="#carrier""##,
        ] {
            let copied = carrier.replace("</svg>", &format!("<use {href} x=\"20\"/></svg>"));
            assert!(
                PreparedMathTerminalReceipt::from_terminal_svg(&copied, &evidence).is_err(),
                "{copied}",
            );
        }
        let ignored = carrier.replace("</svg>", r##"<use xml:href="#carrier"/></svg>"##);
        PreparedMathTerminalReceipt::from_terminal_svg(&ignored, &evidence).unwrap();
        let identified = valid.replace("<use ", r#"<use id="instance" "#);
        let copied = identified.replace("</svg>", r##"<use href="#instance"/></svg>"##);
        assert!(PreparedMathTerminalReceipt::from_terminal_svg(&copied, &evidence).is_err());
    }

    #[test]
    fn terminal_receipt_rejects_duplicate_renderer_occurrence_identities() {
        let occurrence_id =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g data-merman-prepared-math-width="8" data-merman-prepared-math-height="9"><path d="M0 0h1"/></g>"#;
        let fingerprint = PreparedMathProjectionFingerprint::from_projection(projection);
        let evidence = PreparedMathEvidenceLease::new(
            vec![
                PreparedMathExpectation::available(occurrence_id.clone(), fingerprint, 1),
                PreparedMathExpectation::available(occurrence_id, fingerprint, 1),
            ],
            Vec::new(),
        );

        let error = PreparedMathTerminalReceipt::from_terminal_svg("<svg/>", &evidence)
            .expect_err("duplicate renderer occurrence identities must fail closed");

        assert!(error.contains("duplicate occurrence identity"), "{error}");
    }
}
