use std::borrow::Cow;
use std::collections::HashSet;

use crate::svg::fallback::PREPARED_TEXT_LABEL_DATA_ATTR;
use crate::svg::pipeline::builtin::util::{
    SvgTagScanner, find_with_checkpoints, next_svg_quoted_attr_with_checkpoints, start_tag_name,
};
use crate::text::{
    PREPARED_TEXT_LABEL_ID_PREFIX, PreparedTextLabelId, PreparedTextLabelLedgerEntry,
};
use crate::{Error, Result};

use super::context::SvgPostprocessExecution;

pub(crate) fn strip_prepared_text_label_ids<'a>(
    svg: &'a str,
    ledger: &[PreparedTextLabelLedgerEntry],
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    execution.checkpoint()?;
    if ledger.is_empty() && !contains_reserved_prepared_text_spelling(svg, execution)? {
        return Ok(Cow::Borrowed(svg));
    }
    strip_prepared_text_label_ids_scanned(svg, ledger, execution)
}

fn contains_reserved_prepared_text_spelling(
    svg: &str,
    execution: SvgPostprocessExecution<'_>,
) -> Result<bool> {
    let mut checkpoint = || execution.checkpoint();
    if find_with_checkpoints(svg, PREPARED_TEXT_LABEL_ID_PREFIX, &mut checkpoint)?.is_some() {
        return Ok(true);
    }
    // The current evidence attribute contains the ID prefix, so its absence is already proven.
    // Keep the exact fallback if the attribute spelling changes independently in the future.
    Ok(
        !PREPARED_TEXT_LABEL_DATA_ATTR.contains(PREPARED_TEXT_LABEL_ID_PREFIX)
            && find_with_checkpoints(svg, PREPARED_TEXT_LABEL_DATA_ATTR, &mut checkpoint)?
                .is_some(),
    )
}

fn strip_prepared_text_label_ids_scanned<'a>(
    svg: &'a str,
    ledger: &[PreparedTextLabelLedgerEntry],
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    execution.checkpoint()?;
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
    let mut checkpoint = || execution.checkpoint();

    while let Some(tag) = scanner.next_with_checkpoints(&mut checkpoint)? {
        let Some(element_name) = start_tag_name(tag.raw()) else {
            continue;
        };
        let stripped = strip_token_from_tag_with_checkpoints(
            tag.raw(),
            element_name,
            &expected,
            &mut seen_labels,
            &mut seen_svg_ids,
            &mut checkpoint,
        )?;
        let Cow::Owned(stripped) = stripped else {
            continue;
        };

        let output = out.get_or_insert_with(|| String::with_capacity(source.len()));
        output.push_str(&source[copied_until..tag.start()]);
        output.push_str(&stripped);
        copied_until = scanner.cursor();
    }
    checkpoint()?;

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
    execution: SvgPostprocessExecution<'_>,
) -> Result<(String, Option<String>)> {
    match strip_prepared_text_label_ids(&svg, ledger, execution)? {
        Cow::Borrowed(_) => Ok((svg, None)),
        Cow::Owned(public_svg) => Ok((public_svg, Some(svg))),
    }
}

fn strip_token_from_tag_with_checkpoints<'a>(
    tag: &'a str,
    element_name: &str,
    expected: &HashSet<PreparedTextLabelId>,
    seen_labels: &mut HashSet<PreparedTextLabelId>,
    seen_svg_ids: &mut HashSet<String>,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<Cow<'a, str>> {
    let mut out = None::<String>;
    let mut copied_until = 0usize;
    let mut cursor = 0usize;

    while let Some(attribute) = next_svg_quoted_attr_with_checkpoints(tag, cursor, checkpoint)? {
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

#[cfg(test)]
fn strip_token_from_tag<'a>(
    tag: &'a str,
    element_name: &str,
    expected: &HashSet<PreparedTextLabelId>,
    seen_labels: &mut HashSet<PreparedTextLabelId>,
    seen_svg_ids: &mut HashSet<String>,
) -> Result<Cow<'a, str>> {
    strip_token_from_tag_with_checkpoints(
        tag,
        element_name,
        expected,
        seen_labels,
        seen_svg_ids,
        &mut || Ok(()),
    )
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
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        let stripped =
            strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                .unwrap();

        assert!(matches!(&stripped, Cow::Borrowed(_)));
        assert_eq!(stripped, svg);
    }

    #[test]
    fn empty_ledger_rejects_reserved_tokens() {
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for (svg, message) in [
            (
                r#"<svg><text id="merman-prepared-state-00">label</text></svg>"#,
                "is malformed",
            ),
            (
                r#"<svg><text id='merman-prepared-unknown-0'>label</text></svg>"#,
                "is malformed",
            ),
            (
                r#"<svg><g id="merman-prepared-state-0"/></svg>"#,
                "instead of <text>",
            ),
            (
                r#"<svg><text id="merman-prepared-state-0">label</text></svg>"#,
                "has no matching ledger entry",
            ),
            (
                r#"<svg><foreignObject data-merman-prepared-text-label="ordinary"/></svg>"#,
                "is malformed",
            ),
            (
                r#"<svg><g data-merman-prepared-text-label="merman-prepared-state-0"/></svg>"#,
                "instead of <foreignObject>",
            ),
            (
                r#"<svg><foreignObject data-merman-prepared-text-label='merman-prepared-state-0'/></svg>"#,
                "has no matching ledger entry",
            ),
        ] {
            let error =
                strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                    .expect_err("empty evidence must not admit a reserved SVG token");
            assert!(error.to_string().contains(message), "{svg}: {error}");
        }
    }

    #[test]
    fn empty_ledger_keeps_reserved_spelling_outside_token_attributes() {
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for svg in [
            "<svg><text>merman-prepared-state-0</text></svg>",
            "<svg><text>data-merman-prepared-text-label</text></svg>",
            r#"<svg><!-- <text id="merman-prepared-state-0"/> --></svg>"#,
            r#"<svg><![CDATA[<text id="merman-prepared-state-0"/>]]></svg>"#,
            r#"<svg><text title="merman-prepared-state-0">label</text></svg>"#,
        ] {
            let stripped =
                strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                    .unwrap();
            assert!(matches!(&stripped, Cow::Borrowed(_)), "{svg}");
            assert_eq!(stripped, svg);
        }
    }

    #[test]
    fn empty_ledger_preserves_the_scanners_nonvalidating_xml_behavior() {
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for svg in [
            "",
            "plain text",
            "<svg><text>",
            "<svg><!-- unclosed",
            r#"<svg><text id="unclosed"#,
            "<svg><text id=ordinary>label</svg>",
        ] {
            let stripped =
                strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                    .unwrap();
            assert!(matches!(&stripped, Cow::Borrowed(_)), "{svg}");
            assert_eq!(stripped, svg);
        }
    }

    #[test]
    fn empty_ledger_still_observes_operation_cancellation() {
        for svg in [
            "<svg><text>ordinary</text></svg>",
            "<svg><text>merman-prepared-state-0</text></svg>",
            r#"<svg><foreignObject data-merman-prepared-text-label="ordinary"/></svg>"#,
        ] {
            let control = merman_core::OperationControl::new();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_control(control.clone())
                .unwrap();
            control.cancel();
            let error =
                strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                    .expect_err("cancelled operations must stop before token inspection");
            let Error::Cancelled(cancelled) = error else {
                panic!("expected structured cancellation for {svg}");
            };
            assert_eq!(cancelled.phase, merman_core::OperationPhase::Postprocess);
            assert_eq!(cancelled.reason, merman_core::CancelReason::Requested);
        }
    }

    #[test]
    fn empty_ledger_fast_path_matches_scanner_when_reserved_spelling_is_absent() {
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for svg in [
            "",
            "plain text",
            "<svg><text>ordinary</text></svg>",
            "<svg><text title=\"ordinary\">label</text></svg>",
            "<svg><text title=\"你好 > 😀\">关系 é</text></svg>",
            "<svg><!-- ordinary --></svg>",
            "<svg><![CDATA[ordinary]]></svg>",
            "<svg><text id=ordinary>label</svg>",
        ] {
            let fast =
                strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                    .unwrap();
            let scanned = strip_prepared_text_label_ids_scanned(
                svg,
                &[],
                SvgPostprocessExecution::new(&session),
            )
            .unwrap();
            assert_eq!(fast, scanned, "{svg}");
            assert!(matches!(&fast, Cow::Borrowed(_)), "{svg}");
        }
    }

    #[test]
    fn empty_ledger_reserved_spelling_always_falls_back_to_scanner() {
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for svg in [
            r#"<svg><text id="merman-prepared-state-00">label</text></svg>"#,
            r#"<svg><text>merman-prepared-state-0</text></svg>"#,
            r#"<svg><foreignObject data-merman-prepared-text-label="ordinary"/></svg>"#,
        ] {
            let actual =
                strip_prepared_text_label_ids(svg, &[], SvgPostprocessExecution::new(&session))
                    .map_err(|error| error.to_string());
            let scanned = strip_prepared_text_label_ids_scanned(
                svg,
                &[],
                SvgPostprocessExecution::new(&session),
            )
            .map_err(|error| error.to_string());
            assert_eq!(actual, scanned, "{svg}");
        }
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
