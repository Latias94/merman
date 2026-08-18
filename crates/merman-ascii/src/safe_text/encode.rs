use crate::Result;
use crate::color::AsciiColorMode;
use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, CheckedOutput, ResourceContext};

const ENCODE_CHECKPOINT_INTERVAL: usize = 64;

pub(super) fn encode_budgeted_lines_with_expected(
    lines: Vec<String>,
    color_mode: AsciiColorMode,
    resources: &ResourceContext,
    expected_len: usize,
) -> Result<String> {
    resources.transaction(|resources| {
        admit_and_validate_encoded_lines(&lines, color_mode, resources, expected_len)?;
        materialize_encoded_lines(lines, color_mode, resources)
    })
}

fn admit_and_validate_encoded_lines(
    lines: &[String],
    color_mode: AsciiColorMode,
    resources: &ResourceContext,
    expected_len: usize,
) -> Result<()> {
    let pass_work = resources.checked_work_add(lines.len(), expected_len)?;
    let encoder_work = resources.checked_work_mul(pass_work, 2)?;
    resources.charge_layout_work(encoder_work)?;

    let encoded_len = encoded_lines_len(lines, color_mode, resources)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, encoded_len)?;
    if expected_len != encoded_len {
        return Err(crate::error::AsciiError::UnsupportedFeature {
            diagram_type: "structured_text",
            feature: "encoded output byte accounting",
        });
    }
    Ok(())
}

fn materialize_encoded_lines(
    lines: Vec<String>,
    color_mode: AsciiColorMode,
    resources: &ResourceContext,
) -> Result<String> {
    resources.checkpoint()?;
    let mut output = CheckedOutput::new(resources.policy());
    for (index, line) in lines.into_iter().enumerate() {
        resources.checkpoint()?;
        if index > 0 {
            output.push_char('\n')?;
        }
        if color_mode == AsciiColorMode::Html {
            push_html_escaped_text_with_resources(&mut output, &line, resources)?;
        } else {
            push_text_with_resources(&mut output, &line, resources)?;
        }
    }
    Ok(output.finish())
}

fn encoded_lines_len(
    lines: &[String],
    color_mode: AsciiColorMode,
    resources: &ResourceContext,
) -> Result<usize> {
    let separators = lines.len().saturating_sub(1);
    lines.iter().try_fold(separators, |encoded_len, line| {
        resources.checkpoint()?;
        let line_len = encoded_text_len_with_resources(line, color_mode, resources)?;
        encoded_len
            .checked_add(line_len)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))
    })
}

pub(super) fn encoded_text_len_with_resources(
    value: &str,
    color_mode: AsciiColorMode,
    resources: &ResourceContext,
) -> Result<usize> {
    resources.checkpoint()?;
    if color_mode != AsciiColorMode::Html {
        return Ok(value.len());
    }

    let mut encoded_len = 0usize;
    visit_html_escaped_text_with_resources(value, resources, |fragment| {
        encoded_len = encoded_len
            .checked_add(fragment.len())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        Ok(())
    })?;
    Ok(encoded_len)
}

pub(super) fn encoded_text_len(
    value: &str,
    color_mode: AsciiColorMode,
    policy: AsciiResourcePolicy,
) -> Result<usize> {
    if color_mode != AsciiColorMode::Html {
        return Ok(value.len());
    }

    let mut encoded_len = 0usize;
    visit_html_escaped_text(value, |fragment| {
        encoded_len = encoded_len
            .checked_add(fragment.len())
            .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        Ok(())
    })?;
    Ok(encoded_len)
}

fn push_html_escaped_text_with_resources(
    output: &mut CheckedOutput,
    value: &str,
    resources: &ResourceContext,
) -> Result<()> {
    visit_html_escaped_text_with_resources(value, resources, |fragment| output.push_str(fragment))
}

fn push_text_with_resources(
    output: &mut CheckedOutput,
    value: &str,
    resources: &ResourceContext,
) -> Result<()> {
    let mut chunk_start = 0usize;
    let mut scalar_count = 0usize;
    for (byte_index, _) in value.char_indices() {
        if scalar_count == ENCODE_CHECKPOINT_INTERVAL {
            resources.checkpoint()?;
            output.push_str(&value[chunk_start..byte_index])?;
            chunk_start = byte_index;
            scalar_count = 0;
        }
        scalar_count += 1;
    }
    resources.checkpoint()?;
    output.push_str(&value[chunk_start..])
}

pub(crate) fn visit_html_escaped_text(
    value: &str,
    visit: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    visit_html_escaped_text_with_checkpoint(value, || Ok(()), visit)
}

pub(super) fn visit_html_escaped_text_with_checkpoint(
    value: &str,
    mut checkpoint: impl FnMut() -> Result<()>,
    mut visit: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    // HTML escaping classifies syntax scalars without participating in terminal layout.
    for ch in value.chars() {
        checkpoint()?;
        if let Some(escaped) = html_escape(ch) {
            visit(escaped)?;
        } else {
            let mut buffer = [0u8; 4];
            visit(ch.encode_utf8(&mut buffer))?;
        }
    }
    Ok(())
}

fn visit_html_escaped_text_with_resources(
    value: &str,
    resources: &ResourceContext,
    visit: impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let mut scalars_until_checkpoint = 0usize;
    visit_html_escaped_text_with_checkpoint(
        value,
        || {
            if scalars_until_checkpoint == 0 {
                scalars_until_checkpoint = ENCODE_CHECKPOINT_INTERVAL;
                resources.checkpoint()?;
            }
            scalars_until_checkpoint -= 1;
            Ok(())
        },
        visit,
    )
}

fn html_escape(ch: char) -> Option<&'static str> {
    match ch {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '>' => Some("&gt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#39;"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiError;
    use crate::resource::{AsciiResourcePolicy, ResourceContext};
    use merman_core::{OperationControl, OperationPhase};

    #[test]
    fn structured_text_counting_cancellation_rolls_back_the_ledger() {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(5);
        let resources = ResourceContext::new(AsciiResourcePolicy::default())
            .controlled(control, OperationPhase::Emit);
        let line = "<&".repeat(128);

        let error =
            encode_budgeted_lines_with_expected(vec![line], AsciiColorMode::Html, &resources, 0)
                .expect_err("HTML counting should observe scheduled emit cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(details) if details.phase == OperationPhase::Emit
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn structured_text_materialization_observes_emit_cancellation() {
        let control = OperationControl::new();
        let shared_resources = ResourceContext::new(AsciiResourcePolicy::default());
        shared_resources
            .charge_usage(3, 2)
            .expect("prior structured-text usage should fit");
        let resources = shared_resources.controlled(control.clone(), OperationPhase::Emit);
        let line = "payload".repeat(128);
        let expected_len = line.len();

        let error = resources
            .transaction(|resources| {
                admit_and_validate_encoded_lines(
                    std::slice::from_ref(&line),
                    AsciiColorMode::Plain,
                    resources,
                    expected_len,
                )?;
                control.cancel();
                materialize_encoded_lines(vec![line], AsciiColorMode::Plain, resources)
            })
            .expect_err("plain materialization should observe emit cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(details) if details.phase == OperationPhase::Emit
        ));
        assert_eq!(shared_resources.layout_work_used(), 3);
        assert_eq!(shared_resources.document_cells_used(), 2);
    }

    #[test]
    fn structured_text_encoder_work_has_an_exact_transactional_boundary() {
        const EXPECTED_ENCODER_WORK: usize = 4;

        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                EXPECTED_ENCODER_WORK,
            )
            .expect("the exact encoder work limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let output = encode_budgeted_lines_with_expected(
            vec!["A".to_owned()],
            AsciiColorMode::Plain,
            &exact_resources,
            1,
        )
        .expect("one line plus one encoded byte across two passes should fit exactly");
        assert_eq!(output, "A");
        assert_eq!(exact_resources.layout_work_used(), EXPECTED_ENCODER_WORK);

        let below_policy = AsciiResourcePolicy::default()
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                EXPECTED_ENCODER_WORK - 1,
            )
            .expect("the below-exact encoder work limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let error = encode_budgeted_lines_with_expected(
            vec!["A".to_owned()],
            AsciiColorMode::Plain,
            &below_resources,
            1,
        )
        .expect_err("limit-minus-one should reject transactionally");

        assert_eq!(below_resources.layout_work_used(), 0);
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == EXPECTED_ENCODER_WORK
                    && details.max == EXPECTED_ENCODER_WORK - 1
        ));

        const EXPECTED_HTML_WORK: usize = 20;
        let html_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_HTML_WORK)
            .expect("the exact HTML encoder work limit should be valid");
        let html_resources = ResourceContext::new(html_policy);
        let output = encode_budgeted_lines_with_expected(
            vec!["<&".to_owned()],
            AsciiColorMode::Html,
            &html_resources,
            9,
        )
        .expect("one line plus nine encoded bytes across two passes should fit exactly");
        assert_eq!(output, "&lt;&amp;");
        assert_eq!(html_resources.layout_work_used(), EXPECTED_HTML_WORK);

        let below_html_policy = AsciiResourcePolicy::default()
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                EXPECTED_HTML_WORK - 1,
            )
            .expect("the below-exact HTML encoder work limit should be valid");
        let below_html_resources = ResourceContext::new(below_html_policy);
        let error = encode_budgeted_lines_with_expected(
            vec!["<&".to_owned()],
            AsciiColorMode::Html,
            &below_html_resources,
            9,
        )
        .expect_err("HTML limit-minus-one should reject transactionally");
        assert_eq!(below_html_resources.layout_work_used(), 0);
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == EXPECTED_HTML_WORK
                    && details.max == EXPECTED_HTML_WORK - 1
        ));
    }
}
