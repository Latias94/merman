use super::{
    RoutedTextMeasurer, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementReport,
    TextMeasurementSession,
};
use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

pub(in crate::text) struct TestMeasurementSession {
    measurement: TextMeasurementSession,
    work: OperationWorkMeter,
}

impl TestMeasurementSession {
    pub(in crate::text) fn new(policy: TextMeasurementPolicy) -> Self {
        Self {
            measurement: TextMeasurementSession::new(policy),
            work: OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        }
    }

    pub(in crate::text) fn deterministic() -> Self {
        Self::new(TextMeasurementPolicy::deterministic())
    }

    pub(in crate::text) fn text_measurer(
        &self,
        phase: TextMeasurementPhase,
    ) -> RoutedTextMeasurer<'_> {
        self.measurement.routed(&self.work, phase, None)
    }

    pub(in crate::text) fn text_measurement_report(&self) -> TextMeasurementReport {
        self.measurement.report()
    }
}
