use std::borrow::Cow;
use std::collections::HashSet;

use crate::svg::fallback::PREPARED_TEXT_LABEL_DATA_ATTR;
use crate::svg::pipeline::builtin::util::{SvgTagScanner, next_svg_quoted_attr, start_tag_name};
use crate::text::{PreparedTextLabelId, PreparedTextLabelLedgerEntry};
use crate::{Error, Result};

pub(crate) fn strip_prepared_text_label_ids<'a>(
    svg: &'a str,
    ledger: &[PreparedTextLabelLedgerEntry],
) -> Result<Cow<'a, str>> {
    let expected = ledger
        .iter()
        .map(PreparedTextLabelLedgerEntry::id)
        .collect::<HashSet<_>>();
    if expected.len() != ledger.len() {
        return Err(token_error(
            "prepared-text ledger contains duplicate label ids",
        ));
    }

    let mut seen_labels = HashSet::with_capacity(expected.len());
    let mut seen_svg_ids = HashSet::with_capacity(expected.len());
    let source = svg;
    let mut scanner = SvgTagScanner::new(source);
    let mut out = None::<String>;
    let mut copied_until = 0usize;

    while let Some(tag) = scanner.next() {
        let Some(element_name) = start_tag_name(tag.raw()) else {
            continue;
        };
        let stripped = strip_token_from_tag(
            tag.raw(),
            element_name,
            &expected,
            &mut seen_labels,
            &mut seen_svg_ids,
        )?;
        let Cow::Owned(stripped) = stripped else {
            continue;
        };

        let output = out.get_or_insert_with(|| String::with_capacity(source.len()));
        output.push_str(&source[copied_until..tag.start()]);
        output.push_str(&stripped);
        copied_until = scanner.cursor();
    }

    if seen_labels != expected {
        let missing = expected.len().saturating_sub(seen_labels.len());
        return Err(token_error(format!(
            "prepared-text SVG is missing {missing} ledger label token(s)"
        )));
    }

    if let Some(mut out) = out {
        out.push_str(&source[copied_until..]);
        Ok(Cow::Owned(out))
    } else if ledger.is_empty() {
        Ok(Cow::Borrowed(source))
    } else {
        Err(token_error(
            "prepared-text ledger is non-empty but the SVG contains no label tokens",
        ))
    }
}

pub(crate) fn partition_prepared_text_label_ids(
    svg: String,
    ledger: &[PreparedTextLabelLedgerEntry],
) -> Result<(String, Option<String>)> {
    match strip_prepared_text_label_ids(&svg, ledger)? {
        Cow::Borrowed(_) => Ok((svg, None)),
        Cow::Owned(public_svg) => Ok((public_svg, Some(svg))),
    }
}

fn strip_token_from_tag<'a>(
    tag: &'a str,
    element_name: &str,
    expected: &HashSet<PreparedTextLabelId>,
    seen_labels: &mut HashSet<PreparedTextLabelId>,
    seen_svg_ids: &mut HashSet<String>,
) -> Result<Cow<'a, str>> {
    let mut out = None::<String>;
    let mut copied_until = 0usize;
    let mut cursor = 0usize;

    while let Some(attribute) = next_svg_quoted_attr(tag, cursor) {
        cursor = attribute.full_end;
        let attribute_name = &tag[attribute.name_start..attribute.name_end];
        let is_text_id = attribute_name == "id";
        let is_foreign_object_token = attribute_name == PREPARED_TEXT_LABEL_DATA_ATTR;
        if !is_text_id && !is_foreign_object_token {
            continue;
        }
        let value = &tag[attribute.value_start..attribute.value_end];
        let Some(label_id) = PreparedTextLabelId::from_svg_id(value) else {
            if is_foreign_object_token || PreparedTextLabelId::is_svg_id_candidate(value) {
                return Err(token_error(format!(
                    "prepared-text SVG token {value:?} is malformed"
                )));
            }
            continue;
        };

        if is_text_id && element_name != "text" {
            return Err(token_error(format!(
                "prepared-text token {value:?} is attached to <{element_name}> instead of <text>"
            )));
        }
        if is_foreign_object_token && element_name != "foreignObject" {
            return Err(token_error(format!(
                "prepared-text token {value:?} is attached to <{element_name}> instead of <foreignObject>"
            )));
        }
        if is_foreign_object_token && value != label_id.as_svg_id() {
            return Err(token_error(format!(
                "prepared-text foreignObject token {value:?} must use the base label id"
            )));
        }
        if !expected.contains(&label_id) {
            return Err(token_error(format!(
                "prepared-text SVG token {value:?} has no matching ledger entry"
            )));
        }
        if !seen_svg_ids.insert(value.to_string()) {
            return Err(token_error(format!(
                "prepared-text SVG token {value:?} is duplicated"
            )));
        }
        seen_labels.insert(label_id);

        let output = out.get_or_insert_with(|| String::with_capacity(tag.len()));
        output.push_str(&tag[copied_until..attribute.full_start]);
        copied_until = attribute.full_end;
    }

    if let Some(mut out) = out {
        out.push_str(&tag[copied_until..]);
        Ok(Cow::Owned(out))
    } else {
        Ok(Cow::Borrowed(tag))
    }
}

fn token_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess("prepared-text-token", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::PreparedTextLabelFamily;

    #[test]
    fn empty_ledger_leaves_ordinary_svg_unchanged() {
        let svg = r#"<svg><text id="ordinary">label</text></svg>"#;
        assert_eq!(strip_prepared_text_label_ids(svg, &[]).unwrap(), svg);
    }

    #[test]
    fn token_parser_rejects_non_text_owners_before_stripping() {
        let id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 0);
        let expected = HashSet::from([id]);
        let mut seen_labels = HashSet::new();
        let mut seen_svg_ids = HashSet::new();
        let error = strip_token_from_tag(
            r#"<g id="merman-prepared-state-0">"#,
            "g",
            &expected,
            &mut seen_labels,
            &mut seen_svg_ids,
        )
        .unwrap_err();
        assert!(error.to_string().contains("instead of <text>"));
    }

    #[test]
    fn token_parser_rejects_malformed_reserved_ids() {
        let mut seen_labels = HashSet::new();
        let mut seen_svg_ids = HashSet::new();
        let error = strip_token_from_tag(
            r#"<text id="merman-prepared-state-00">"#,
            "text",
            &HashSet::new(),
            &mut seen_labels,
            &mut seen_svg_ids,
        )
        .expect_err("reserved malformed ids must fail closed");

        assert!(error.to_string().contains("is malformed"));
    }

    #[test]
    fn foreign_object_token_is_stripped_and_satisfies_the_expected_label() {
        let id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 7);
        let expected = HashSet::from([id]);
        let mut seen_labels = HashSet::new();
        let mut seen_svg_ids = HashSet::new();
        let stripped = strip_token_from_tag(
            r#"<foreignObject data-merman-prepared-text-label="merman-prepared-state-7" width="48">"#,
            "foreignObject",
            &expected,
            &mut seen_labels,
            &mut seen_svg_ids,
        )
        .unwrap();

        assert_eq!(stripped, r#"<foreignObject width="48">"#);
        assert_eq!(seen_labels, expected);
        assert_eq!(seen_svg_ids, HashSet::from([id.as_svg_id()]));
    }

    #[test]
    fn foreign_object_token_rejects_the_wrong_element() {
        let id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 7);
        let mut seen_labels = HashSet::new();
        let mut seen_svg_ids = HashSet::new();
        let error = strip_token_from_tag(
            r#"<g data-merman-prepared-text-label="merman-prepared-state-7">"#,
            "g",
            &HashSet::from([id]),
            &mut seen_labels,
            &mut seen_svg_ids,
        )
        .expect_err("foreignObject evidence must stay on its owning element");

        assert!(error.to_string().contains("instead of <foreignObject>"));
    }

    #[test]
    fn foreign_object_token_rejects_line_scoped_ids() {
        let id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 7);
        let mut seen_labels = HashSet::new();
        let mut seen_svg_ids = HashSet::new();
        let error = strip_token_from_tag(
            r#"<foreignObject data-merman-prepared-text-label="merman-prepared-state-7-line-0">"#,
            "foreignObject",
            &HashSet::from([id]),
            &mut seen_labels,
            &mut seen_svg_ids,
        )
        .expect_err("foreignObject evidence must use a base label id");

        assert!(error.to_string().contains("must use the base label id"));
    }

    #[test]
    fn foreign_object_token_rejects_malformed_values() {
        let mut seen_labels = HashSet::new();
        let mut seen_svg_ids = HashSet::new();
        let error = strip_token_from_tag(
            r#"<foreignObject data-merman-prepared-text-label="state-7">"#,
            "foreignObject",
            &HashSet::new(),
            &mut seen_labels,
            &mut seen_svg_ids,
        )
        .expect_err("the internal evidence attribute must fail closed");

        assert!(error.to_string().contains("is malformed"));
    }
}
