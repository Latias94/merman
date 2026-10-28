use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sha2::{Digest as _, Sha256};

use crate::DiagramFamilyId;
use crate::resources::PreparedTextRetainedReservation;

const OCCURRENCE_KEY_DOMAIN: &[u8] = b"merman-prepared-math-occurrence-v1";
const PROJECTION_DOMAIN: &[u8] = b"merman-prepared-math-native-projection-v1";
pub(crate) const PREPARED_MATH_NATIVE_CLASS: &str = "merman-prepared-math-native";
#[cfg(test)]
const PREPARED_MATH_NATIVE_CLASS_ATTRIBUTE: &str = r#"class="merman-prepared-math-native""#;

/// Renderer-owned identity for one semantic math-label occurrence.
///
/// The value is safe to emit as an XML attribute and is never recovered from user-authored SVG.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PreparedMathOccurrenceId(Arc<str>);

impl PreparedMathOccurrenceId {
    pub(crate) fn indexed(family_id: DiagramFamilyId, role: &'static str, index: usize) -> Self {
        Self(Arc::from(format!("{}/{role}/{index}", family_id.as_str())))
    }

    pub(crate) fn keyed(family_id: DiagramFamilyId, role: &'static str, key: &str) -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, OCCURRENCE_KEY_DOMAIN);
        update_len_prefixed(&mut hasher, family_id.as_str().as_bytes());
        update_len_prefixed(&mut hasher, role.as_bytes());
        update_len_prefixed(&mut hasher, key.as_bytes());
        let digest = hasher.finalize();
        Self(Arc::from(format!(
            "{}/{role}/sha256:{}",
            family_id.as_str(),
            data_encoding::HEXLOWER.encode_display(&digest)
        )))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PreparedMathProjectionFingerprint([u8; 32]);

impl PreparedMathProjectionFingerprint {
    pub(crate) fn from_projection(projection: &str) -> Self {
        let mut hasher = Sha256::new();
        update_len_prefixed(&mut hasher, PROJECTION_DOMAIN);
        update_len_prefixed(&mut hasher, projection.as_bytes());
        Self(hasher.finalize().into())
    }
}

/// Style facts the prepared-math backend can independently attest for one browser emission.
///
/// The compiled RaTeX backend owns both the wrapper and the default glyph paint, so it can attest
/// the requested foreground and font size. Compatibility HTML backends return opaque markup and
/// therefore remain unverified even though Merman still wraps their result in the requested CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathStyleAssurance {
    requested_foreground: Arc<str>,
    requested_font_size_bits: u64,
    default_foreground_verified: bool,
    font_size_verified: bool,
}

impl PreparedMathStyleAssurance {
    pub(crate) fn compiled_ratex(requested_foreground: &str, requested_font_size_px: f64) -> Self {
        Self {
            requested_foreground: Arc::from(requested_foreground),
            requested_font_size_bits: requested_font_size_px.to_bits(),
            default_foreground_verified: true,
            font_size_verified: true,
        }
    }

    #[cfg(any(test, merman_internal_theme_acceptance))]
    pub(crate) fn opaque_html(requested_foreground: &str, requested_font_size_px: f64) -> Self {
        Self {
            requested_foreground: Arc::from(requested_foreground),
            requested_font_size_bits: requested_font_size_px.to_bits(),
            default_foreground_verified: false,
            font_size_verified: false,
        }
    }

    pub(crate) fn proves_default_foreground(&self, expected: &str) -> bool {
        self.default_foreground_verified && self.requested_foreground.as_ref() == expected
    }

    pub(crate) fn proves_font_size_px(&self, expected: f64) -> bool {
        self.font_size_verified && self.requested_font_size_bits == expected.to_bits()
    }

    pub(crate) fn retained_heap_bytes(&self) -> usize {
        self.requested_foreground.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathExpectation {
    occurrence_id: PreparedMathOccurrenceId,
    projection_fingerprint: Option<PreparedMathProjectionFingerprint>,
    expected_emissions: usize,
}

impl PreparedMathExpectation {
    pub(crate) fn available(
        occurrence_id: PreparedMathOccurrenceId,
        projection_fingerprint: PreparedMathProjectionFingerprint,
        expected_emissions: usize,
    ) -> Self {
        Self {
            occurrence_id,
            projection_fingerprint: Some(projection_fingerprint),
            expected_emissions,
        }
    }

    pub(crate) fn unavailable(
        occurrence_id: PreparedMathOccurrenceId,
        expected_emissions: usize,
    ) -> Self {
        Self {
            occurrence_id,
            projection_fingerprint: None,
            expected_emissions,
        }
    }

    pub(crate) fn occurrence_id(&self) -> &PreparedMathOccurrenceId {
        &self.occurrence_id
    }

    pub(crate) const fn projection_fingerprint(&self) -> Option<PreparedMathProjectionFingerprint> {
        self.projection_fingerprint
    }

    pub(crate) const fn expected_emissions(&self) -> usize {
        self.expected_emissions
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.occurrence_id.as_str().len())
    }
}

/// Immutable renderer-owned expectations and native payloads for one rendered family artifact.
#[derive(Debug, Clone, Default)]
pub(crate) struct PreparedMathEvidenceLease {
    entries: Arc<[PreparedMathExpectation]>,
    native_projections: Arc<[Option<Arc<super::PreparedMathNativeProjection>>]>,
    duplicate_occurrence_id: Option<PreparedMathOccurrenceId>,
    _retained_reservations: Arc<[PreparedTextRetainedReservation]>,
}

impl PartialEq for PreparedMathEvidenceLease {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}

impl Eq for PreparedMathEvidenceLease {}

impl PreparedMathEvidenceLease {
    #[cfg(test)]
    pub(crate) fn new(
        entries: Vec<PreparedMathExpectation>,
        retained_reservations: Vec<PreparedTextRetainedReservation>,
    ) -> Self {
        Self::with_native_projections(
            entries.into_iter().map(|entry| (entry, None)).collect(),
            retained_reservations,
        )
    }

    pub(crate) fn with_native_projections(
        mut entries: Vec<(
            PreparedMathExpectation,
            Option<Arc<super::PreparedMathNativeProjection>>,
        )>,
        retained_reservations: Vec<PreparedTextRetainedReservation>,
    ) -> Self {
        entries.sort_unstable_by(|left, right| left.0.occurrence_id().cmp(right.0.occurrence_id()));
        let duplicate_occurrence_id = entries
            .windows(2)
            .find(|entries| entries[0].0.occurrence_id() == entries[1].0.occurrence_id())
            .map(|entries| entries[0].0.occurrence_id().clone());
        let (entries, native_projections): (Vec<_>, Vec<_>) = entries.into_iter().unzip();
        Self {
            entries: entries.into(),
            native_projections: native_projections.into(),
            duplicate_occurrence_id,
            _retained_reservations: retained_reservations.into(),
        }
    }

    pub(crate) fn entries(&self) -> &[PreparedMathExpectation] {
        &self.entries
    }

    pub(crate) fn native_projection(
        &self,
        index: usize,
    ) -> Option<&super::PreparedMathNativeProjection> {
        self.native_projections.get(index)?.as_deref()
    }

    pub(crate) fn retained_entry_bytes(
        expectation: &PreparedMathExpectation,
        projection: Option<&super::PreparedMathNativeProjection>,
    ) -> usize {
        expectation
            .retained_bytes()
            .saturating_add(std::mem::size_of::<
                Option<Arc<super::PreparedMathNativeProjection>>,
            >())
            .saturating_add(
                projection.map_or(0, super::PreparedMathNativeProjection::retained_bytes),
            )
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn validate_unique_occurrences(&self) -> Result<(), String> {
        let Some(duplicate) = &self.duplicate_occurrence_id else {
            return Ok(());
        };
        Err(format!(
            "prepared-math evidence contains duplicate occurrence identity `{}`",
            duplicate.as_str()
        ))
    }

    pub(crate) fn expectation(
        &self,
        occurrence_id: &str,
    ) -> Result<Option<(usize, &PreparedMathExpectation)>, String> {
        self.validate_unique_occurrences()?;
        Ok(self
            .entries
            .binary_search_by(|entry| entry.occurrence_id().as_str().cmp(occurrence_id))
            .ok()
            .map(|index| (index, &self.entries[index])))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathTerminalOccurrenceReceipt {
    expectation: PreparedMathExpectation,
    emitted: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedMathTerminalReceipt {
    artifact_digest: [u8; 32],
    occurrences: Arc<[PreparedMathTerminalOccurrenceReceipt]>,
}

impl PreparedMathTerminalReceipt {
    pub(crate) fn from_terminal_svg(
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

        let document = crate::xml_dom::Document::parse(svg)
            .map_err(|error| format!("prepared-math terminal SVG is malformed: {error}"))?;
        let root = document.root_element();
        if !native_svg_element(root, "svg") {
            return Err("prepared-math terminal requires an SVG root".to_owned());
        }
        let namespace_context = root
            .namespaces()
            .map(|namespace| (namespace.name(), namespace.uri()))
            .collect::<HashMap<_, _>>();
        let owned_element = |node: crate::xml_dom::Node<'_, '_>, name: &str| {
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
                && let Some(target) = node
                    .attribute(("http://www.w3.org/1999/xlink", "href"))
                    .or_else(|| node.attribute("href"))
                    .and_then(|href| svgtypes::IRI::from_str(href).ok().map(|iri| iri.0))
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
                let target = node
                    .attribute(("http://www.w3.org/1999/xlink", "href"))
                    .or_else(|| node.attribute("href"))
                    .and_then(|href| svgtypes::IRI::from_str(href).ok().map(|iri| iri.0))
                    .ok_or_else(|| {
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

        let plan = crate::svg::SvgReferencePlan::for_occurrence_receipt(svg)
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

    pub(crate) fn occurrence_count(&self) -> usize {
        self.occurrences.len()
    }

    pub(crate) fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }
}

fn native_svg_element(node: crate::xml_dom::Node<'_, '_>, name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == name
        && node
            .tag_name()
            .namespace()
            .is_none_or(|uri| uri == "http://www.w3.org/2000/svg")
}

fn unqualified_attribute<'a>(node: crate::xml_dom::Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        .map(|attribute| attribute.value())
}

fn same_namespace_context(
    node: crate::xml_dom::Node<'_, '_>,
    expected: &HashMap<Option<&str>, &str>,
) -> bool {
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

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn semantic_occurrence_ids_are_stable_and_attribute_safe() {
        let indexed =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "edge-label", 17);
        let keyed = PreparedMathOccurrenceId::keyed(
            DiagramFamilyId::SEQUENCE,
            "block-label",
            "critical \0 λ",
        );

        assert_eq!(indexed.as_str(), "flowchart/edge-label/17");
        assert!(keyed.as_str().starts_with("sequence/block-label/sha256:"));
        assert!(
            keyed
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"/-:".contains(&byte))
        );
    }

    #[test]
    fn projection_fingerprint_is_domain_separated_and_content_sensitive() {
        assert_ne!(
            PreparedMathProjectionFingerprint::from_projection("<g><path/></g>"),
            PreparedMathProjectionFingerprint::from_projection("<g><rect/></g>")
        );
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
    fn terminal_receipt_follows_native_xlink_target_precedence() {
        let occurrence =
            PreparedMathOccurrenceId::indexed(DiagramFamilyId::FLOWCHART, "node-label", 0);
        let projection = r#"<g><path d="M0 0h1"/></g>"#;
        let evidence = available_evidence(occurrence.clone(), projection, 1);
        let valid = format!(
            "<svg xmlns:xlink=\"http://www.w3.org/1999/xlink\">{}</svg>",
            terminal_occurrence(&occurrence, projection, 0)
        );
        let redirected = valid.replace(
            r##"href="#math-0""##,
            r##"href="#math-0" xlink:href="#missing""##,
        );
        assert!(PreparedMathTerminalReceipt::from_terminal_svg(&redirected, &evidence).is_err());
        let native_target = valid.replace(
            r##"href="#math-0""##,
            r##"href="#missing" xlink:href="#math-0""##,
        );
        PreparedMathTerminalReceipt::from_terminal_svg(&native_target, &evidence).unwrap();
        let first_fallback = valid.replace(
            r##"href="#math-0""##,
            r##"xml:href="#missing" href="#math-0""##,
        );
        assert!(
            PreparedMathTerminalReceipt::from_terminal_svg(&first_fallback, &evidence).is_err()
        );
        for (namespace, name) in [("http://www.w3.org/2000/svg", "s"), ("urn:other", "q")] {
            let aliased = valid.replace(
                r##"href="#math-0""##,
                &format!(r##"xmlns:{name}="{namespace}" {name}:href="#math-0""##),
            );
            PreparedMathTerminalReceipt::from_terminal_svg(&aliased, &evidence).unwrap();
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
        for href in [r##"href="#carrier""##, r##"xml:href="#carrier""##] {
            let copied = carrier.replace("</svg>", &format!("<use {href} x=\"20\"/></svg>"));
            assert!(
                PreparedMathTerminalReceipt::from_terminal_svg(&copied, &evidence).is_err(),
                "{copied}",
            );
        }
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
