use crate::svg::fallback::PREPARED_TEXT_LABEL_DATA_ATTR;
use crate::svg::pipeline::builtin::util::{
    SvgTagScanner, find_with_checkpoints, next_svg_quoted_attr_with_checkpoints, start_tag_name,
};
use crate::text::{PREPARED_TEXT_LABEL_ID_PREFIX, PreparedTextLabelId};
use crate::{Error, Result};

use super::context::SvgPostprocessExecution;

pub(crate) fn reject_reserved_prepared_text_tokens(
    svg: &str,
    execution: SvgPostprocessExecution<'_>,
) -> Result<()> {
    execution.checkpoint()?;
    // The reserved data attribute also contains this prefix. Ordinary SVG needs no tag scan.
    if find_with_checkpoints(svg, PREPARED_TEXT_LABEL_ID_PREFIX, &mut || {
        execution.checkpoint()
    })?
    .is_none()
    {
        return Ok(());
    }
    reject_reserved_prepared_text_tokens_scanned(svg, execution)
}

fn reject_reserved_prepared_text_tokens_scanned(
    svg: &str,
    execution: SvgPostprocessExecution<'_>,
) -> Result<()> {
    execution.checkpoint()?;
    let mut scanner = SvgTagScanner::new(svg);
    let mut checkpoint = || execution.checkpoint();
    while let Some(tag) = scanner.next_with_checkpoints(&mut checkpoint)? {
        let Some(element_name) = start_tag_name(tag.raw()) else {
            continue;
        };
        reject_reserved_token_in_tag(tag.raw(), element_name, &mut checkpoint)?;
    }
    checkpoint()
}

fn reject_reserved_token_in_tag(
    tag: &str,
    element_name: &str,
    checkpoint: &mut impl FnMut() -> Result<()>,
) -> Result<()> {
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
        if PreparedTextLabelId::from_svg_id(value).is_none() {
            if is_foreign_object_token || PreparedTextLabelId::is_svg_id_candidate(value) {
                return Err(token_error(format!(
                    "prepared-text SVG token {value:?} is malformed"
                )));
            }
            continue;
        }
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
        if is_foreign_object_token && value.contains("-line-") {
            return Err(token_error(format!(
                "prepared-text foreignObject token {value:?} must use the base label id"
            )));
        }
        return Err(token_error(format!(
            "prepared-text SVG token {value:?} has no matching ledger entry"
        )));
    }
    Ok(())
}

fn token_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess("prepared-text-token", message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_token_observation_admits_ordinary_svg() {
        let svg = r#"<svg><text id="ordinary">label</text></svg>"#;
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        reject_reserved_prepared_text_tokens(svg, SvgPostprocessExecution::new(&session)).unwrap();
    }

    #[test]
    fn reserved_token_observation_rejects_reserved_tokens() {
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
            (
                r#"<svg><foreignObject data-merman-prepared-text-label='merman-prepared-state-0-line-1'/></svg>"#,
                "must use the base label id",
            ),
        ] {
            let error =
                reject_reserved_prepared_text_tokens(svg, SvgPostprocessExecution::new(&session))
                    .expect_err("reserved SVG tokens must not be admitted");
            assert!(error.to_string().contains(message), "{svg}: {error}");
        }
    }

    #[test]
    fn reserved_token_observation_ignores_spelling_outside_token_attributes() {
        let environment = crate::environment::RenderEnvironment::deterministic();
        let session = environment.begin_session().unwrap();
        for svg in [
            "<svg><text>merman-prepared-state-0</text></svg>",
            "<svg><text>data-merman-prepared-text-label</text></svg>",
            r#"<svg><!-- <text id="merman-prepared-state-0"/> --></svg>"#,
            r#"<svg><![CDATA[<text id="merman-prepared-state-0"/>]]></svg>"#,
            r#"<svg><text title="merman-prepared-state-0">label</text></svg>"#,
        ] {
            reject_reserved_prepared_text_tokens(svg, SvgPostprocessExecution::new(&session))
                .unwrap();
        }
    }

    #[test]
    fn reserved_token_observation_preserves_nonvalidating_xml_behavior() {
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
            reject_reserved_prepared_text_tokens(svg, SvgPostprocessExecution::new(&session))
                .unwrap();
        }
    }

    #[test]
    fn reserved_token_observation_still_observes_operation_cancellation() {
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
                reject_reserved_prepared_text_tokens(svg, SvgPostprocessExecution::new(&session))
                    .expect_err("cancelled operations must stop before token inspection");
            let Error::Cancelled(cancelled) = error else {
                panic!("expected structured cancellation for {svg}");
            };
            assert_eq!(cancelled.phase, merman_core::OperationPhase::Postprocess);
            assert_eq!(cancelled.reason, merman_core::CancelReason::Requested);
        }
    }
}
