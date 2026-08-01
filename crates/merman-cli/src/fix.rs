use merman_analysis::{AnalysisDiagnostic, DiagnosticFixEdit};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

const FIX_ID_PREFIX: &str = "mfix-v1:";
const FIX_ID_DOMAIN: &[u8] = b"merman-cli/fix-id/v1\0";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FixId(String);

impl FixId {
    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn from_edits(edits: &[CanonicalEdit]) -> Self {
        let mut hasher = Sha256::new();
        // This framing is part of the CLI contract. Keep it independent of serializers and
        // diagnostic metadata so titles, rule origins, and payload ordering cannot change an ID.
        hasher.update(FIX_ID_DOMAIN);
        hash_usize(&mut hasher, edits.len());
        for edit in edits {
            hash_usize(&mut hasher, edit.start);
            hash_usize(&mut hasher, edit.end);
            hash_usize(&mut hasher, edit.replacement.len());
            hasher.update(edit.replacement.as_bytes());
        }

        let digest = hasher.finalize();
        let mut id = String::with_capacity(FIX_ID_PREFIX.len() + digest.len() * 2);
        id.push_str(FIX_ID_PREFIX);
        for byte in digest {
            id.push(hex_digit(byte >> 4));
            id.push(hex_digit(byte & 0x0f));
        }
        Self(id)
    }
}

impl fmt::Display for FixId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for FixId {
    type Err = FixIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(digest) = value.strip_prefix(FIX_ID_PREFIX) else {
            return Err(FixIdParseError);
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(FixIdParseError);
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("fix id must use the form `mfix-v1:<64 lowercase hexadecimal characters>`")]
pub(crate) struct FixIdParseError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CanonicalEdit {
    start: usize,
    end: usize,
    replacement: String,
}

impl CanonicalEdit {
    #[cfg(test)]
    pub(crate) fn replacement(&self) -> &str {
        &self.replacement
    }

    fn from_diagnostic(edit: &DiagnosticFixEdit) -> Self {
        Self {
            start: edit.span.byte_start,
            end: edit.span.byte_end,
            replacement: edit.replacement.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixOrigin {
    diagnostic_index: usize,
    rule_id: String,
    titles: Vec<String>,
    preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixCandidate {
    id: FixId,
    edits: Vec<CanonicalEdit>,
    origins: Vec<FixOrigin>,
}

impl FixCandidate {
    #[cfg(test)]
    pub(crate) fn id(&self) -> &FixId {
        &self.id
    }

    #[cfg(test)]
    pub(crate) fn edits(&self) -> &[CanonicalEdit] {
        &self.edits
    }

    #[cfg(test)]
    pub(crate) fn origins(&self) -> &[FixOrigin] {
        &self.origins
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FixSelection {
    pub(crate) rule_ids: BTreeSet<String>,
    pub(crate) fix_ids: BTreeSet<FixId>,
}

impl FixSelection {
    #[cfg(test)]
    pub(crate) fn rules(rule_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            rule_ids: rule_ids.into_iter().collect(),
            fix_ids: BTreeSet::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn exact(fix_ids: impl IntoIterator<Item = FixId>) -> Self {
        Self {
            rule_ids: BTreeSet::new(),
            fix_ids: fix_ids.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticChoice {
    rule_id: String,
    candidates: Vec<DiagnosticCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagnosticCandidate {
    id: FixId,
    preferred: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixCatalog {
    candidates: BTreeMap<FixId, FixCandidate>,
    diagnostics: Vec<DiagnosticChoice>,
}

impl FixCatalog {
    pub(crate) fn build(
        source: &str,
        diagnostics: &[AnalysisDiagnostic],
    ) -> Result<Self, FixPlanError> {
        let mut candidates = BTreeMap::<FixId, FixCandidate>::new();
        let mut choices = Vec::new();

        for (diagnostic_index, diagnostic) in diagnostics.iter().enumerate() {
            let mut diagnostic_candidates = BTreeMap::<FixId, bool>::new();
            for fix in &diagnostic.fixes {
                let edits = canonicalize_edits(source, &diagnostic.id, &fix.title, &fix.edits)?;
                let id = FixId::from_edits(&edits);

                let candidate = candidates
                    .entry(id.clone())
                    .or_insert_with(|| FixCandidate {
                        id: id.clone(),
                        edits: edits.clone(),
                        origins: Vec::new(),
                    });
                if candidate.edits != edits {
                    return Err(FixPlanError::IdentifierCollision { id });
                }
                merge_origin(
                    candidate,
                    diagnostic_index,
                    &diagnostic.id,
                    &fix.title,
                    fix.is_preferred,
                );
                diagnostic_candidates
                    .entry(id)
                    .and_modify(|preferred| *preferred |= fix.is_preferred)
                    .or_insert(fix.is_preferred);
            }

            if !diagnostic_candidates.is_empty() {
                choices.push(DiagnosticChoice {
                    rule_id: diagnostic.id.clone(),
                    candidates: diagnostic_candidates
                        .into_iter()
                        .map(|(id, preferred)| DiagnosticCandidate { id, preferred })
                        .collect(),
                });
            }
        }

        for candidate in candidates.values_mut() {
            candidate.origins.sort_by(|left, right| {
                (left.diagnostic_index, left.rule_id.as_str())
                    .cmp(&(right.diagnostic_index, right.rule_id.as_str()))
            });
        }

        Ok(Self {
            candidates,
            diagnostics: choices,
        })
    }

    #[cfg(test)]
    pub(crate) fn candidates(&self) -> impl Iterator<Item = &FixCandidate> {
        self.candidates.values()
    }

    pub(crate) fn plan(&self, selection: &FixSelection) -> Result<FixPlan, FixPlanError> {
        if selection.fix_ids.is_empty() {
            Ok(self.plan_defaults(&selection.rule_ids))
        } else {
            self.plan_exact(selection)
        }
    }

    fn plan_defaults(&self, rule_ids: &BTreeSet<String>) -> FixPlan {
        let mut selected = Vec::<FixId>::new();
        let mut selected_set = BTreeSet::<FixId>::new();
        let mut skipped_conflicts = Vec::new();

        for diagnostic in self
            .diagnostics
            .iter()
            .filter(|diagnostic| rule_ids.is_empty() || rule_ids.contains(&diagnostic.rule_id))
        {
            if diagnostic
                .candidates
                .iter()
                .any(|candidate| selected_set.contains(&candidate.id))
            {
                continue;
            }

            let mut ranked = diagnostic.candidates.iter().collect::<Vec<_>>();
            ranked.sort_by(|left, right| {
                right
                    .preferred
                    .cmp(&left.preferred)
                    .then_with(|| left.id.cmp(&right.id))
            });

            for candidate in ranked {
                let conflict = selected.iter().find(|selected_id| {
                    candidates_conflict(
                        &self.candidates[&candidate.id],
                        &self.candidates[*selected_id],
                    )
                });
                if let Some(conflicting_fix_id) = conflict {
                    skipped_conflicts.push(SkippedFixConflict {
                        rule_id: diagnostic.rule_id.clone(),
                        fix_id: candidate.id.clone(),
                        conflicting_fix_id: conflicting_fix_id.clone(),
                    });
                    continue;
                }

                selected_set.insert(candidate.id.clone());
                selected.push(candidate.id.clone());
                break;
            }
        }

        self.finish_plan(selected, skipped_conflicts)
    }

    fn plan_exact(&self, selection: &FixSelection) -> Result<FixPlan, FixPlanError> {
        let mut selected = Vec::with_capacity(selection.fix_ids.len());
        for id in &selection.fix_ids {
            let Some(candidate) = self.candidates.get(id) else {
                return Err(FixPlanError::UnknownFix { id: id.clone() });
            };
            if !selection.rule_ids.is_empty()
                && !candidate
                    .origins
                    .iter()
                    .any(|origin| selection.rule_ids.contains(&origin.rule_id))
            {
                return Err(FixPlanError::IneligibleFix {
                    id: id.clone(),
                    rule_ids: selection.rule_ids.iter().cloned().collect(),
                });
            }
            selected.push(id.clone());
        }

        for (index, left_id) in selected.iter().enumerate() {
            let left = &self.candidates[left_id];
            for right_id in selected.iter().skip(index + 1) {
                let right = &self.candidates[right_id];
                if let Some((diagnostic_index, rule_id)) =
                    shared_eligible_diagnostic(left, right, &selection.rule_ids)
                {
                    return Err(FixPlanError::SelectedAlternatives {
                        diagnostic_index,
                        rule_id,
                        first: left_id.clone(),
                        second: right_id.clone(),
                    });
                }
                if candidates_conflict(left, right) {
                    return Err(FixPlanError::ConflictingExactFixes {
                        first: left_id.clone(),
                        second: right_id.clone(),
                    });
                }
            }
        }

        Ok(self.finish_plan(selected, Vec::new()))
    }

    fn finish_plan(
        &self,
        mut selected_fix_ids: Vec<FixId>,
        skipped_conflicts: Vec<SkippedFixConflict>,
    ) -> FixPlan {
        selected_fix_ids.sort();
        selected_fix_ids.dedup();

        let edits = selected_fix_ids
            .iter()
            .flat_map(|id| self.candidates[id].edits.iter().cloned())
            .collect::<Vec<_>>();
        let edits = canonicalize_selected_edits(edits);

        FixPlan {
            selected_fix_ids,
            edits,
            skipped_conflicts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SkippedFixConflict {
    pub(crate) rule_id: String,
    pub(crate) fix_id: FixId,
    pub(crate) conflicting_fix_id: FixId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixPlan {
    selected_fix_ids: Vec<FixId>,
    edits: Vec<CanonicalEdit>,
    skipped_conflicts: Vec<SkippedFixConflict>,
}

impl FixPlan {
    #[cfg(test)]
    pub(crate) fn selected_fix_ids(&self) -> &[FixId] {
        &self.selected_fix_ids
    }

    #[cfg(test)]
    pub(crate) fn edits(&self) -> &[CanonicalEdit] {
        &self.edits
    }

    pub(crate) fn skipped_conflicts(&self) -> &[SkippedFixConflict] {
        &self.skipped_conflicts
    }

    pub(crate) fn apply(&self, source: &str) -> Result<String, FixPlanError> {
        validate_canonical_edits(source, &self.edits).map_err(|reason| {
            FixPlanError::IncompatibleApplicationSource {
                reason: reason.to_string(),
            }
        })?;

        let mut result = String::with_capacity(source.len());
        let mut cursor = 0;
        for edit in &self.edits {
            result.push_str(&source[cursor..edit.start]);
            result.push_str(&edit.replacement);
            cursor = edit.end;
        }
        result.push_str(&source[cursor..]);
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum FixPlanError {
    #[error("diagnostic `{rule_id}` fix `{title}` has no edits")]
    EmptyCandidate { rule_id: String, title: String },
    #[error(
        "diagnostic `{rule_id}` fix `{title}` has invalid byte range {start}..{end} for a {source_len}-byte source"
    )]
    InvalidRange {
        rule_id: String,
        title: String,
        start: usize,
        end: usize,
        source_len: usize,
    },
    #[error(
        "diagnostic `{rule_id}` fix `{title}` byte range {start}..{end} is not on UTF-8 character boundaries"
    )]
    InvalidUtf8Boundary {
        rule_id: String,
        title: String,
        start: usize,
        end: usize,
    },
    #[error(
        "diagnostic `{rule_id}` fix `{title}` contains overlapping edits {first_start}..{first_end} and {second_start}..{second_end}"
    )]
    OverlappingCandidateEdits {
        rule_id: String,
        title: String,
        first_start: usize,
        first_end: usize,
        second_start: usize,
        second_end: usize,
    },
    #[error("stable fix identifier collision for `{id}`")]
    IdentifierCollision { id: FixId },
    #[error("unknown fix id `{id}`")]
    UnknownFix { id: FixId },
    #[error("fix `{id}` is not eligible for selected rules: {rule_ids:?}")]
    IneligibleFix { id: FixId, rule_ids: Vec<String> },
    #[error(
        "fixes `{first}` and `{second}` are alternatives for diagnostic #{diagnostic_index} (`{rule_id}`)"
    )]
    SelectedAlternatives {
        diagnostic_index: usize,
        rule_id: String,
        first: FixId,
        second: FixId,
    },
    #[error("selected fixes `{first}` and `{second}` contain conflicting edits")]
    ConflictingExactFixes { first: FixId, second: FixId },
    #[error("planned fixes cannot be applied to this source: {reason}")]
    IncompatibleApplicationSource { reason: String },
}

impl FixPlanError {
    pub(crate) const fn is_selection_error(&self) -> bool {
        matches!(
            self,
            Self::UnknownFix { .. }
                | Self::IneligibleFix { .. }
                | Self::SelectedAlternatives { .. }
                | Self::ConflictingExactFixes { .. }
        )
    }
}

fn canonicalize_edits(
    source: &str,
    rule_id: &str,
    title: &str,
    edits: &[DiagnosticFixEdit],
) -> Result<Vec<CanonicalEdit>, FixPlanError> {
    if edits.is_empty() {
        return Err(FixPlanError::EmptyCandidate {
            rule_id: rule_id.to_owned(),
            title: title.to_owned(),
        });
    }

    let mut edits = edits
        .iter()
        .map(CanonicalEdit::from_diagnostic)
        .collect::<Vec<_>>();
    edits.sort();
    edits.dedup();

    for edit in &edits {
        if edit.start > edit.end || edit.end > source.len() {
            return Err(FixPlanError::InvalidRange {
                rule_id: rule_id.to_owned(),
                title: title.to_owned(),
                start: edit.start,
                end: edit.end,
                source_len: source.len(),
            });
        }
        if !source.is_char_boundary(edit.start) || !source.is_char_boundary(edit.end) {
            return Err(FixPlanError::InvalidUtf8Boundary {
                rule_id: rule_id.to_owned(),
                title: title.to_owned(),
                start: edit.start,
                end: edit.end,
            });
        }
    }

    for (index, left) in edits.iter().enumerate() {
        for right in edits.iter().skip(index + 1) {
            if edits_conflict(left, right) {
                return Err(FixPlanError::OverlappingCandidateEdits {
                    rule_id: rule_id.to_owned(),
                    title: title.to_owned(),
                    first_start: left.start,
                    first_end: left.end,
                    second_start: right.start,
                    second_end: right.end,
                });
            }
        }
    }

    Ok(edits)
}

fn validate_canonical_edits(source: &str, edits: &[CanonicalEdit]) -> Result<(), &'static str> {
    for edit in edits {
        if edit.start > edit.end || edit.end > source.len() {
            return Err("an edit range is outside the source");
        }
        if !source.is_char_boundary(edit.start) || !source.is_char_boundary(edit.end) {
            return Err("an edit range is not on UTF-8 character boundaries");
        }
    }
    for (index, left) in edits.iter().enumerate() {
        for right in edits.iter().skip(index + 1) {
            if edits_conflict(left, right) {
                return Err("planned edits overlap");
            }
        }
    }
    Ok(())
}

fn merge_origin(
    candidate: &mut FixCandidate,
    diagnostic_index: usize,
    rule_id: &str,
    title: &str,
    preferred: bool,
) {
    if let Some(origin) = candidate
        .origins
        .iter_mut()
        .find(|origin| origin.diagnostic_index == diagnostic_index)
    {
        origin.preferred |= preferred;
        if !origin.titles.iter().any(|existing| existing == title) {
            origin.titles.push(title.to_owned());
            origin.titles.sort();
        }
        return;
    }

    candidate.origins.push(FixOrigin {
        diagnostic_index,
        rule_id: rule_id.to_owned(),
        titles: vec![title.to_owned()],
        preferred,
    });
}

fn shared_eligible_diagnostic(
    left: &FixCandidate,
    right: &FixCandidate,
    rule_ids: &BTreeSet<String>,
) -> Option<(usize, String)> {
    left.origins.iter().find_map(|left_origin| {
        if !rule_ids.is_empty() && !rule_ids.contains(&left_origin.rule_id) {
            return None;
        }
        right
            .origins
            .iter()
            .find(|right_origin| right_origin.diagnostic_index == left_origin.diagnostic_index)
            .map(|_| (left_origin.diagnostic_index, left_origin.rule_id.clone()))
    })
}

fn candidates_conflict(left: &FixCandidate, right: &FixCandidate) -> bool {
    left.edits.iter().any(|left_edit| {
        right
            .edits
            .iter()
            .any(|right_edit| left_edit != right_edit && edits_conflict(left_edit, right_edit))
    })
}

fn edits_conflict(left: &CanonicalEdit, right: &CanonicalEdit) -> bool {
    let left_empty = left.start == left.end;
    let right_empty = right.start == right.end;
    match (left_empty, right_empty) {
        (true, true) => false,
        (true, false) => left.start > right.start && left.start < right.end,
        (false, true) => right.start > left.start && right.start < left.end,
        (false, false) => left.start < right.end && right.start < left.end,
    }
}

fn canonicalize_selected_edits(mut edits: Vec<CanonicalEdit>) -> Vec<CanonicalEdit> {
    edits.sort();
    edits.dedup();
    edits
}

fn hash_usize(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_be_bytes());
}

const fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + nibble - 10) as char,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_analysis::{
        DiagnosticCategory, DiagnosticFix, DiagnosticSpan, LspRange, Utf16Position,
    };

    fn span(start: usize, end: usize) -> DiagnosticSpan {
        DiagnosticSpan {
            byte_start: start,
            byte_end: end,
            line: 1,
            column: start + 1,
            end_line: 1,
            end_column: end + 1,
            lsp_range: LspRange::new(
                Utf16Position {
                    line: 0,
                    character: start,
                },
                Utf16Position {
                    line: 0,
                    character: end,
                },
            ),
        }
    }

    fn edit(start: usize, end: usize, replacement: &str) -> DiagnosticFixEdit {
        DiagnosticFixEdit::new(span(start, end), replacement)
    }

    fn diagnostic(
        rule_id: &str,
        fixes: impl IntoIterator<Item = DiagnosticFix>,
    ) -> AnalysisDiagnostic {
        AnalysisDiagnostic::error(rule_id, DiagnosticCategory::Config, "test").with_fixes(fixes)
    }

    fn candidate_id(catalog: &FixCatalog, replacement: &str) -> FixId {
        catalog
            .candidates()
            .find(|candidate| {
                candidate
                    .edits()
                    .iter()
                    .any(|edit| edit.replacement() == replacement)
            })
            .expect("candidate")
            .id()
            .clone()
    }

    #[test]
    fn fix_id_parser_requires_the_versioned_lowercase_shape() {
        let valid = format!("{FIX_ID_PREFIX}{}", "a".repeat(64));
        assert_eq!(valid.parse::<FixId>().expect("valid").as_str(), valid);
        assert!("a".repeat(64).parse::<FixId>().is_err());
        assert!(
            format!("{FIX_ID_PREFIX}{}", "A".repeat(64))
                .parse::<FixId>()
                .is_err()
        );
        assert!(
            format!("{FIX_ID_PREFIX}{}", "a".repeat(63))
                .parse::<FixId>()
                .is_err()
        );
    }

    #[test]
    fn canonicalization_validates_ranges_and_utf8_boundaries() {
        let reversed = diagnostic(
            "rule",
            [DiagnosticFix::new("reversed", vec![edit(2, 1, "x")])],
        );
        assert!(matches!(
            FixCatalog::build("abc", &[reversed]),
            Err(FixPlanError::InvalidRange { .. })
        ));

        let outside = diagnostic(
            "rule",
            [DiagnosticFix::new("outside", vec![edit(0, 4, "x")])],
        );
        assert!(matches!(
            FixCatalog::build("abc", &[outside]),
            Err(FixPlanError::InvalidRange { .. })
        ));

        let split_codepoint = diagnostic(
            "rule",
            [DiagnosticFix::new("unicode", vec![edit(1, 2, "x")])],
        );
        assert!(matches!(
            FixCatalog::build("a中b", &[split_codepoint]),
            Err(FixPlanError::InvalidUtf8Boundary { .. })
        ));
    }

    #[test]
    fn canonicalization_sorts_deduplicates_and_allows_boundaries_and_insertions() {
        let fixes = [DiagnosticFix::new(
            "edits",
            vec![
                edit(4, 5, "B"),
                edit(1, 1, "y"),
                edit(0, 1, "A"),
                edit(1, 1, "x"),
                edit(0, 1, "A"),
            ],
        )];
        let catalog = FixCatalog::build("a中b", &[diagnostic("rule", fixes)]).expect("catalog");
        let plan = catalog.plan(&FixSelection::default()).expect("plan");

        assert_eq!(plan.edits().len(), 4);
        assert_eq!(plan.apply("a中b").expect("apply"), "Axy中B");
    }

    #[test]
    fn adjacent_replacements_do_not_conflict() {
        let fixes = [DiagnosticFix::new(
            "adjacent",
            vec![edit(0, 1, "A"), edit(1, 4, "middle"), edit(4, 5, "B")],
        )];
        let catalog = FixCatalog::build("a中b", &[diagnostic("rule", fixes)]).expect("catalog");
        let plan = catalog.plan(&FixSelection::default()).expect("plan");

        assert_eq!(plan.apply("a中b").expect("apply"), "AmiddleB");
    }

    #[test]
    fn canonicalization_rejects_overlap_and_insertions_inside_replacements() {
        let overlap = diagnostic(
            "rule",
            [DiagnosticFix::new(
                "overlap",
                vec![edit(0, 2, "x"), edit(1, 3, "y")],
            )],
        );
        assert!(matches!(
            FixCatalog::build("abcd", &[overlap]),
            Err(FixPlanError::OverlappingCandidateEdits { .. })
        ));

        let insertion = diagnostic(
            "rule",
            [DiagnosticFix::new(
                "insertion",
                vec![edit(0, 3, "x"), edit(1, 1, "y")],
            )],
        );
        assert!(matches!(
            FixCatalog::build("abcd", &[insertion]),
            Err(FixPlanError::OverlappingCandidateEdits { .. })
        ));
    }

    #[test]
    fn stable_id_depends_only_on_the_canonical_edit_set() {
        let first = diagnostic(
            "first-rule",
            [DiagnosticFix::new(
                "first title",
                vec![edit(2, 3, "z"), edit(0, 1, "x"), edit(0, 1, "x")],
            )],
        );
        let second = diagnostic(
            "second-rule",
            [DiagnosticFix::new(
                "second title",
                vec![edit(0, 1, "x"), edit(2, 3, "z")],
            )],
        );
        let catalog = FixCatalog::build("abc", &[first, second]).expect("catalog");
        let candidates = catalog.candidates().collect::<Vec<_>>();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].origins().len(), 2);
        assert_eq!(candidates[0].origins()[0].rule_id, "first-rule");
        assert_eq!(candidates[0].origins()[0].titles, ["first title"]);
        assert!(!candidates[0].origins()[0].preferred);
        assert_eq!(candidates[0].origins()[1].rule_id, "second-rule");
        assert_eq!(candidates[0].origins()[1].titles, ["second title"]);
        assert_eq!(
            candidates[0].id().as_str(),
            "mfix-v1:d12d33ff8f9a2c5b19bec239de711f8fa878100cde059545663c36269af866a2"
        );
    }

    #[test]
    fn repeated_diagnostic_candidates_are_selected_and_applied_once() {
        let repeated = DiagnosticFix::new("rewrite", vec![edit(0, 3, "new")]).preferred();
        let diagnostics = [
            diagnostic("rule-a", [repeated.clone()]),
            diagnostic("rule-b", [repeated]),
        ];
        let catalog = FixCatalog::build("old", &diagnostics).expect("catalog");
        let plan = catalog.plan(&FixSelection::default()).expect("plan");

        assert_eq!(catalog.candidates().count(), 1);
        assert_eq!(plan.selected_fix_ids().len(), 1);
        assert_eq!(plan.edits().len(), 1);
        assert_eq!(plan.apply("old").expect("apply"), "new");
    }

    #[test]
    fn default_selection_chooses_the_preferred_alternative() {
        let alternatives = diagnostic(
            "rule",
            [
                DiagnosticFix::new("fallback", vec![edit(0, 1, "fallback")]),
                DiagnosticFix::new("preferred", vec![edit(0, 1, "preferred")]).preferred(),
            ],
        );
        let catalog = FixCatalog::build("a", &[alternatives]).expect("catalog");
        let plan = catalog.plan(&FixSelection::default()).expect("plan");

        assert_eq!(plan.selected_fix_ids().len(), 1);
        assert_eq!(plan.apply("a").expect("apply"), "preferred");
    }

    #[test]
    fn preferred_ties_are_broken_by_stable_fix_id() {
        let alternatives = diagnostic(
            "rule",
            [
                DiagnosticFix::new("one", vec![edit(0, 1, "one")]).preferred(),
                DiagnosticFix::new("two", vec![edit(0, 1, "two")]).preferred(),
            ],
        );
        let catalog = FixCatalog::build("a", &[alternatives]).expect("catalog");
        let expected = catalog
            .candidates()
            .map(|candidate| candidate.id().clone())
            .min()
            .expect("candidate");
        let plan = catalog.plan(&FixSelection::default()).expect("plan");

        assert_eq!(plan.selected_fix_ids(), &[expected]);
    }

    #[test]
    fn default_selection_reports_conflicts_and_tries_the_next_alternative() {
        let diagnostics = [
            diagnostic(
                "rule-a",
                [DiagnosticFix::new("first", vec![edit(0, 2, "A")]).preferred()],
            ),
            diagnostic(
                "rule-b",
                [
                    DiagnosticFix::new("conflict", vec![edit(1, 3, "B")]).preferred(),
                    DiagnosticFix::new("fallback", vec![edit(3, 4, "C")]),
                ],
            ),
        ];
        let catalog = FixCatalog::build("abcd", &diagnostics).expect("catalog");
        let conflicting_id = candidate_id(&catalog, "B");
        let plan = catalog.plan(&FixSelection::default()).expect("plan");

        assert_eq!(plan.selected_fix_ids().len(), 2);
        assert_eq!(plan.skipped_conflicts().len(), 1);
        assert_eq!(plan.skipped_conflicts()[0].rule_id, "rule-b");
        assert_eq!(plan.skipped_conflicts()[0].fix_id, conflicting_id);
        assert_eq!(plan.apply("abcd").expect("apply"), "AcC");
    }

    #[test]
    fn rule_selection_limits_default_candidates() {
        let diagnostics = [
            diagnostic("rule-a", [DiagnosticFix::new("a", vec![edit(0, 1, "A")])]),
            diagnostic("rule-b", [DiagnosticFix::new("b", vec![edit(1, 2, "B")])]),
        ];
        let catalog = FixCatalog::build("ab", &diagnostics).expect("catalog");
        let plan = catalog
            .plan(&FixSelection::rules(["rule-b".to_owned()]))
            .expect("plan");

        assert_eq!(plan.selected_fix_ids().len(), 1);
        assert_eq!(plan.apply("ab").expect("apply"), "aB");
    }

    #[test]
    fn exact_selection_rejects_two_alternatives_from_one_diagnostic() {
        let alternatives = diagnostic(
            "rule",
            [
                DiagnosticFix::new("left", vec![edit(0, 1, "L")]),
                DiagnosticFix::new("right", vec![edit(1, 2, "R")]),
            ],
        );
        let catalog = FixCatalog::build("ab", &[alternatives]).expect("catalog");
        let ids = catalog
            .candidates()
            .map(|candidate| candidate.id().clone())
            .collect::<Vec<_>>();
        let error = catalog
            .plan(&FixSelection::exact(ids))
            .expect_err("alternatives");

        assert!(matches!(error, FixPlanError::SelectedAlternatives { .. }));
    }

    #[test]
    fn exact_selection_rejects_cross_diagnostic_conflicts() {
        let diagnostics = [
            diagnostic(
                "rule-a",
                [DiagnosticFix::new("left", vec![edit(0, 2, "L")])],
            ),
            diagnostic(
                "rule-b",
                [DiagnosticFix::new("right", vec![edit(1, 3, "R")])],
            ),
        ];
        let catalog = FixCatalog::build("abc", &diagnostics).expect("catalog");
        let ids = catalog
            .candidates()
            .map(|candidate| candidate.id().clone())
            .collect::<Vec<_>>();
        let error = catalog
            .plan(&FixSelection::exact(ids))
            .expect_err("conflict");

        assert!(matches!(error, FixPlanError::ConflictingExactFixes { .. }));
    }

    #[test]
    fn exact_selection_must_be_eligible_for_selected_rules() {
        let catalog = FixCatalog::build(
            "a",
            &[diagnostic(
                "rule-a",
                [DiagnosticFix::new("fix", vec![edit(0, 1, "A")])],
            )],
        )
        .expect("catalog");
        let id = catalog.candidates().next().expect("candidate").id().clone();
        let selection = FixSelection {
            rule_ids: ["rule-b".to_owned()].into_iter().collect(),
            fix_ids: [id].into_iter().collect(),
        };

        assert!(matches!(
            catalog.plan(&selection),
            Err(FixPlanError::IneligibleFix { .. })
        ));
    }

    #[test]
    fn exact_selection_rejects_an_unknown_well_formed_id() {
        let catalog = FixCatalog::build("a", &[]).expect("catalog");
        let id = format!("{FIX_ID_PREFIX}{}", "0".repeat(64))
            .parse::<FixId>()
            .expect("well-formed id");

        assert!(matches!(
            catalog.plan(&FixSelection::exact([id])),
            Err(FixPlanError::UnknownFix { .. })
        ));
    }
}
