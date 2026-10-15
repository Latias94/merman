//! Caller-owned HTML parsing, serialization, and operation controls.

use std::cell::Cell;

use merman_core::OperationPhase;
use merman_core::sanitize::{SanitizeFailure, SanitizeOutputSink};

use crate::resources::{OperationWorkError, OperationWorkMeter, ResourceLimitPhase};

#[derive(Debug, Clone)]
pub(crate) enum HtmlOutputError {
    Limit { attempted: usize },
    Allocation,
    Operation(OperationWorkError),
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HtmlSanitizeSink<'a> {
    max: usize,
    work_meter: &'a OperationWorkMeter,
    parse_work: &'a Cell<usize>,
    phase: OperationPhase,
}

impl<'a> HtmlSanitizeSink<'a> {
    pub(crate) const fn new(
        max: usize,
        work_meter: &'a OperationWorkMeter,
        parse_work: &'a Cell<usize>,
        phase: OperationPhase,
    ) -> Self {
        Self {
            max,
            work_meter,
            parse_work,
            phase,
        }
    }

    pub(crate) fn checked_len(
        &self,
        current: usize,
        additional: usize,
    ) -> Result<usize, HtmlOutputError> {
        let attempted = current
            .checked_add(additional)
            .ok_or(HtmlOutputError::Limit {
                attempted: usize::MAX,
            })?;
        if attempted > self.max {
            return Err(HtmlOutputError::Limit { attempted });
        }
        Ok(attempted)
    }
}

impl SanitizeOutputSink for HtmlSanitizeSink<'_> {
    type Error = HtmlOutputError;

    fn checkpoint(&self) -> Result<(), Self::Error> {
        self.work_meter
            .checkpoint(self.phase)
            .map_err(HtmlOutputError::Operation)
    }

    fn check_parse_storage(
        &self,
        input_bytes: usize,
        nodes: usize,
        retained_bytes: usize,
    ) -> Result<(), Self::Error> {
        const BYTES_PER_WORK_UNIT: usize = 256;
        if nodes == 0 {
            self.parse_work.set(0);
        }
        // Charge the parser's input, nodes, and transient storage before tree growth. Temporary
        // tree storage does not belong to the final prepared-artifact retained-byte ledger.
        let work = input_bytes
            .div_ceil(BYTES_PER_WORK_UNIT)
            .checked_add(nodes)
            .and_then(|work| work.checked_add(retained_bytes.div_ceil(BYTES_PER_WORK_UNIT)))
            .ok_or_else(|| HtmlOutputError::Operation(self.work_meter.arithmetic_overflow()))?;
        let additional = work.saturating_sub(self.parse_work.get());
        match self.phase {
            OperationPhase::Layout => self.work_meter.charge(additional),
            _ => self.work_meter.charge_emit_work(additional),
        }
        .map_err(HtmlOutputError::Operation)?;
        self.parse_work.set(work);
        Ok(())
    }

    fn checked_output_len(&self, current: usize, additional: usize) -> Result<usize, Self::Error> {
        self.checked_len(current, additional)
    }

    fn string_with_capacity(&self, capacity: usize) -> Result<String, Self::Error> {
        self.checked_len(0, capacity)?;
        let mut output = String::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| HtmlOutputError::Allocation)?;
        Ok(output)
    }

    fn output_buffer(&self, input_len: usize) -> Result<Vec<u8>, Self::Error> {
        self.checked_len(0, input_len)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(input_len)
            .map_err(|_| HtmlOutputError::Allocation)?;
        Ok(output)
    }

    fn push_output_chunk(&self, output: &mut Vec<u8>, chunk: &[u8]) -> Result<(), Self::Error> {
        self.checked_len(output.len(), chunk.len())?;
        output
            .try_reserve(chunk.len())
            .map_err(|_| HtmlOutputError::Allocation)?;
        output.extend_from_slice(chunk);
        Ok(())
    }
}

pub(crate) fn resolve_svg_fragment(
    result: Result<String, SanitizeFailure<HtmlOutputError>>,
    work_meter: &OperationWorkMeter,
) -> crate::Result<String> {
    match result {
        Ok(fragment) => Ok(fragment),
        Err(SanitizeFailure::Output(HtmlOutputError::Operation(error))) => Err(error.into()),
        Err(SanitizeFailure::Output(HtmlOutputError::Limit { attempted })) => {
            work_meter.preflight_svg_byte_count(
                attempted,
                ResourceLimitPhase::SvgOutput,
                OperationPhase::Emit,
            )?;
            Ok(String::new())
        }
        // Preserve the compatibility string wrappers' fail-closed allocation/input behavior.
        Err(_) => Ok(String::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    fn loose_config() -> merman_core::MermaidConfig {
        merman_core::MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))
    }

    #[test]
    fn svg_fragment_parser_reports_emit_work_rejection() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let parse_work = Cell::new(0);
        let sink = HtmlSanitizeSink::new(4096, &meter, &parse_work, OperationPhase::Emit);
        let result =
            merman_core::sanitize::sanitize_xhtml_with_sink("<b>label</b>", &loose_config(), &sink);
        let error = resolve_svg_fragment(result, &meter)
            .expect_err("tree allocation must obey the operation work ceiling");
        assert!(matches!(error, crate::Error::ResourceLimitExceeded(_)));
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn svg_context_parser_preserves_emit_cancellation() {
        struct CancelOnSecondParse<'a> {
            inner: HtmlSanitizeSink<'a>,
            control: &'a merman_core::OperationControl,
            epochs: Cell<usize>,
        }
        impl SanitizeOutputSink for CancelOnSecondParse<'_> {
            type Error = HtmlOutputError;
            fn checkpoint(&self) -> Result<(), Self::Error> {
                self.inner.checkpoint()
            }
            fn check_parse_storage(
                &self,
                input: usize,
                nodes: usize,
                retained: usize,
            ) -> Result<(), Self::Error> {
                if nodes == 0 && retained == 0 {
                    self.epochs.set(self.epochs.get() + 1);
                    if self.epochs.get() == 2 {
                        self.control.cancel();
                    }
                }
                self.inner.check_parse_storage(input, nodes, retained)
            }
            fn checked_output_len(
                &self,
                current: usize,
                additional: usize,
            ) -> Result<usize, Self::Error> {
                self.inner.checked_output_len(current, additional)
            }
            fn string_with_capacity(&self, capacity: usize) -> Result<String, Self::Error> {
                self.inner.string_with_capacity(capacity)
            }
            fn output_buffer(&self, len: usize) -> Result<Vec<u8>, Self::Error> {
                self.inner.output_buffer(len)
            }
            fn push_output_chunk(
                &self,
                output: &mut Vec<u8>,
                chunk: &[u8],
            ) -> Result<(), Self::Error> {
                self.inner.push_output_chunk(output, chunk)
            }
        }
        let control = merman_core::OperationControl::new();
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        );
        let parse_work = Cell::new(0);
        let sink = CancelOnSecondParse {
            inner: HtmlSanitizeSink::new(usize::MAX, &meter, &parse_work, OperationPhase::Emit),
            control: &control,
            epochs: Cell::new(0),
        };
        let result = merman_core::sanitize::sanitize_xhtml_in_svg_context_with_sink(
            &"<span>label</span>".repeat(64),
            &loose_config(),
            &sink,
        );
        let error = resolve_svg_fragment(result, &meter)
            .expect_err("both Architecture parsing stages share the operation controls");
        assert_eq!(
            sink.epochs.get(),
            2,
            "HTML sanitization completed before SVG parsing was canceled"
        );
        let crate::Error::Cancelled(cancelled) = error else {
            panic!("expected parser cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Emit);
        assert_eq!(
            meter.checkpoint(OperationPhase::Layout),
            Err(OperationWorkError::Cancelled(cancelled)),
        );
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn repeated_parse_epochs_charge_work_without_reserving_svg_bytes() {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let parse_work = Cell::new(0);
        let sink = HtmlSanitizeSink::new(4096, &meter, &parse_work, OperationPhase::Emit);
        let config = loose_config();
        let input = "<b>label</b>";
        merman_core::sanitize::sanitize_xhtml_with_sink(input, &config, &sink).unwrap();
        let first = meter.used();
        merman_core::sanitize::sanitize_xhtml_with_sink(input, &config, &sink).unwrap();
        assert!(first > 0);
        assert_eq!(meter.used(), first * 2);
        assert_eq!(meter.projected_svg_bytes(), 0);
    }

    #[test]
    fn local_output_limit_maps_to_existing_svg_terminal() {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let parse_work = Cell::new(0);
        let sink = HtmlSanitizeSink::new(1, &meter, &parse_work, OperationPhase::Emit);
        let result =
            merman_core::sanitize::sanitize_xhtml_with_sink("<b>label</b>", &loose_config(), &sink);
        let error = resolve_svg_fragment(result, &meter)
            .expect_err("local fragment growth must preserve SVG limit classification");
        let crate::Error::ResourceLimitExceeded(error) = error else {
            panic!("expected SVG resource limit");
        };
        assert_eq!(error.limit, "max_svg_bytes");
        assert_eq!(meter.projected_svg_bytes(), 0);
    }
}
