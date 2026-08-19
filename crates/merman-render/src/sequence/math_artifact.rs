//! Operation-local prepared browser math for Sequence layout and SVG emission.

use std::cell::RefCell;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::math::{
    ConfiguredMathBackend, MathPreparationOutcome, MathPreparationUnavailable,
    PrepareMathLabelRequest, PreparedMathLabel,
};
use crate::resources::{OperationWorkError, OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::TextMeasurer;

use super::SequenceTerminalTextStyle;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum SequenceMathOccurrence {
    Actor(usize),
    Message(usize),
    Note(usize),
    BlockLabel(String),
}

impl SequenceMathOccurrence {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(match self {
            Self::BlockLabel(label) => label.len(),
            Self::Actor(_) | Self::Message(_) | Self::Note(_) => 0,
        })
    }
}

#[derive(Debug)]
struct SequencePreparedMathEntry {
    source: Arc<str>,
    foreground_provenance: super::SequenceTerminalForegroundProvenance,
    outcome: MathPreparationOutcome,
    _retained_reservation: PreparedTextRetainedReservation,
}

impl SequencePreparedMathEntry {
    fn owner_retained_bytes(occurrence: &SequenceMathOccurrence, source: &str) -> usize {
        std::mem::size_of::<Self>()
            .checked_add(source.len())
            .and_then(|bytes| bytes.checked_add(occurrence.retained_bytes()))
            .unwrap_or(usize::MAX)
    }

    fn retained_bytes_without_reservation(
        occurrence: &SequenceMathOccurrence,
        source: &str,
        outcome: &MathPreparationOutcome,
    ) -> usize {
        let owner_bytes = Self::owner_retained_bytes(occurrence, source);
        outcome.prepared().map_or(owner_bytes, |artifact| {
            owner_bytes
                .checked_add(artifact.retained_bytes())
                .unwrap_or(usize::MAX)
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct SequenceMathSidecar {
    entries: FxHashMap<SequenceMathOccurrence, SequencePreparedMathEntry>,
}

impl SequenceMathSidecar {
    pub(crate) fn get_for_occurrence(
        &self,
        occurrence: &SequenceMathOccurrence,
    ) -> Option<&PreparedMathLabel> {
        self.entries
            .get(occurrence)
            .and_then(|entry| entry.outcome.prepared())
    }

    pub(crate) fn get(
        &self,
        occurrence: &SequenceMathOccurrence,
        source: &str,
    ) -> Option<&PreparedMathLabel> {
        self.entries
            .get(occurrence)
            .filter(|entry| entry.source.as_ref() == source)
            .and_then(|entry| entry.outcome.prepared())
    }

    #[cfg(test)]
    pub(crate) fn get_arc(
        &self,
        occurrence: &SequenceMathOccurrence,
        source: &str,
    ) -> Option<&Arc<PreparedMathLabel>> {
        let entry = self
            .entries
            .get(occurrence)
            .filter(|entry| entry.source.as_ref() == source)?;
        match &entry.outcome {
            MathPreparationOutcome::Prepared(artifact) => Some(artifact),
            MathPreparationOutcome::Unavailable(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn unavailable_reason(
        &self,
        occurrence: &SequenceMathOccurrence,
    ) -> Option<MathPreparationUnavailable> {
        self.entries
            .get(occurrence)
            .and_then(|entry| entry.outcome.unavailable_reason())
    }

    #[cfg(test)]
    pub(crate) fn foreground_provenance(
        &self,
        occurrence: &SequenceMathOccurrence,
    ) -> Option<super::SequenceTerminalForegroundProvenance> {
        self.entries
            .get(occurrence)
            .map(|entry| entry.foreground_provenance)
    }
}

pub(crate) trait SequenceMathArtifactStore {
    fn prepare_or_get(
        &self,
        occurrence: SequenceMathOccurrence,
        source: &str,
        terminal: SequenceTerminalTextStyle<'_>,
    ) -> Option<Arc<PreparedMathLabel>>;
}

impl SequenceMathArtifactStore for SequenceMathSidecar {
    fn prepare_or_get(
        &self,
        occurrence: SequenceMathOccurrence,
        source: &str,
        _terminal: SequenceTerminalTextStyle<'_>,
    ) -> Option<Arc<PreparedMathLabel>> {
        let entry = self
            .entries
            .get(&occurrence)
            .filter(|entry| entry.source.as_ref() == source)?;
        match &entry.outcome {
            MathPreparationOutcome::Prepared(artifact) => Some(Arc::clone(artifact)),
            MathPreparationOutcome::Unavailable(_) => None,
        }
    }
}

pub(crate) struct SequenceMathSidecarBuilder<'a> {
    backend: Option<ConfiguredMathBackend>,
    config: merman_core::MermaidConfig,
    text_measurer: &'a dyn TextMeasurer,
    work_meter: Arc<OperationWorkMeter>,
    entries: RefCell<FxHashMap<SequenceMathOccurrence, SequencePreparedMathEntry>>,
    error: RefCell<Option<OperationWorkError>>,
}

impl<'a> SequenceMathSidecarBuilder<'a> {
    pub(crate) fn new(
        backend: Option<&ConfiguredMathBackend>,
        config: &merman_core::MermaidConfig,
        text_measurer: &'a dyn TextMeasurer,
        work_meter: Arc<OperationWorkMeter>,
    ) -> Self {
        Self {
            backend: backend.cloned(),
            config: config.clone(),
            text_measurer,
            work_meter,
            entries: RefCell::new(FxHashMap::default()),
            error: RefCell::new(None),
        }
    }

    pub(crate) fn prepare(
        &self,
        occurrence: SequenceMathOccurrence,
        source: &str,
        terminal: SequenceTerminalTextStyle<'_>,
    ) -> Option<Arc<PreparedMathLabel>> {
        if self.error.borrow().is_some() || !source.contains("$$") {
            return None;
        }
        if let Some(entry) = self.entries.borrow().get(&occurrence) {
            debug_assert_eq!(
                entry.source.as_ref(),
                source,
                "one Sequence math occurrence must bind one effective source"
            );
            debug_assert_eq!(entry.foreground_provenance, terminal.foreground_provenance);
            return match &entry.outcome {
                MathPreparationOutcome::Prepared(artifact) => Some(Arc::clone(artifact)),
                MathPreparationOutcome::Unavailable(_) => None,
            };
        }

        let owner_retained_bytes =
            SequencePreparedMathEntry::owner_retained_bytes(&occurrence, source);
        let outcome = match self.backend.as_ref() {
            Some(backend) => match backend.prepare(
                PrepareMathLabelRequest::sequence(
                    source,
                    &self.config,
                    terminal.text_style,
                    terminal.foreground,
                )
                .with_text_measurer(self.text_measurer)
                .with_owner_retained_bytes(owner_retained_bytes),
                &self.work_meter,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    self.record_error(error);
                    return None;
                }
            },
            None => {
                MathPreparationOutcome::Unavailable(MathPreparationUnavailable::BackendUnavailable)
            }
        };
        let retained_bytes = SequencePreparedMathEntry::retained_bytes_without_reservation(
            &occurrence,
            source,
            &outcome,
        );
        let reservation = match self
            .work_meter
            .reserve_prepared_text_retained_bytes(retained_bytes)
        {
            Ok(reservation) => reservation,
            Err(error) => {
                self.record_error(OperationWorkError::ResourceLimitExceeded(error));
                return None;
            }
        };
        let prepared = match &outcome {
            MathPreparationOutcome::Prepared(artifact) => Some(Arc::clone(artifact)),
            MathPreparationOutcome::Unavailable(_) => None,
        };
        self.entries.borrow_mut().insert(
            occurrence,
            SequencePreparedMathEntry {
                source: Arc::from(source),
                foreground_provenance: terminal.foreground_provenance,
                outcome,
                _retained_reservation: reservation,
            },
        );
        prepared
    }

    pub(crate) fn get(
        &self,
        occurrence: &SequenceMathOccurrence,
        source: &str,
    ) -> Option<Arc<PreparedMathLabel>> {
        let entries = self.entries.borrow();
        let entry = entries
            .get(occurrence)
            .filter(|entry| entry.source.as_ref() == source)?;
        match &entry.outcome {
            MathPreparationOutcome::Prepared(artifact) => Some(Arc::clone(artifact)),
            MathPreparationOutcome::Unavailable(_) => None,
        }
    }

    pub(crate) fn finish(self) -> Result<SequenceMathSidecar, OperationWorkError> {
        if let Some(error) = self.error.into_inner() {
            return Err(error);
        }
        Ok(SequenceMathSidecar {
            entries: self.entries.into_inner(),
        })
    }

    fn record_error(&self, error: OperationWorkError) {
        let mut recorded = self.error.borrow_mut();
        if recorded.is_none() {
            *recorded = Some(error);
        }
    }

    #[cfg(test)]
    fn retained_bytes(&self) -> usize {
        self.entries
            .borrow()
            .values()
            .map(|entry| entry._retained_reservation.retained_bytes())
            .sum()
    }
}

impl SequenceMathArtifactStore for SequenceMathSidecarBuilder<'_> {
    fn prepare_or_get(
        &self,
        occurrence: SequenceMathOccurrence,
        source: &str,
        terminal: SequenceTerminalTextStyle<'_>,
    ) -> Option<Arc<PreparedMathLabel>> {
        self.prepare(occurrence, source, terminal)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::math::{MathRenderer, PreparedMathHtmlLabel};
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::text::{DeterministicTextMeasurer, TextMetrics, TextStyle};

    use super::*;

    #[derive(Debug)]
    struct CountingSequenceMathRenderer {
        prepare_calls: Arc<AtomicUsize>,
    }

    impl MathRenderer for CountingSequenceMathRenderer {
        fn render_html_label(
            &self,
            _text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            panic!("combined Sequence preparation must be used")
        }

        fn prepare_sequence_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
            _style: &TextStyle,
        ) -> Option<PreparedMathHtmlLabel> {
            self.prepare_calls.fetch_add(1, Ordering::SeqCst);
            Some(PreparedMathHtmlLabel::with_max_line_height(
                format!("<span>{text}</span>"),
                TextMetrics {
                    width: 42.0,
                    height: 53.0,
                    line_count: 2,
                },
                41.0,
            ))
        }
    }

    fn terminal(style: &TextStyle) -> SequenceTerminalTextStyle<'_> {
        SequenceTerminalTextStyle {
            text_style: style,
            foreground: "#67e8f9",
            foreground_provenance:
                super::super::SequenceTerminalForegroundProvenance::MermaidConfig,
        }
    }

    #[test]
    fn one_occurrence_prepares_once_and_layout_writer_share_the_same_arc() {
        let calls = Arc::new(AtomicUsize::new(0));
        let backend = ConfiguredMathBackend::external(Arc::new(CountingSequenceMathRenderer {
            prepare_calls: Arc::clone(&calls),
        }));
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let text_measurer = DeterministicTextMeasurer::default();
        let builder = SequenceMathSidecarBuilder::new(
            Some(&backend),
            &merman_core::MermaidConfig::default(),
            &text_measurer,
            Arc::clone(&meter),
        );
        let style = TextStyle {
            font_size: 27.0,
            ..TextStyle::default()
        };
        let occurrence = SequenceMathOccurrence::Message(3);

        let layout = builder
            .prepare(occurrence.clone(), "$$x$$<br>$$y$$", terminal(&style))
            .unwrap();
        let repeated = builder
            .prepare(occurrence.clone(), "$$x$$<br>$$y$$", terminal(&style))
            .unwrap();
        assert!(Arc::ptr_eq(&layout, &repeated));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(layout.max_line_height_px(), 41.0);
        assert_eq!(
            builder
                .entries
                .borrow()
                .get(&occurrence)
                .expect("prepared occurrence")
                .foreground_provenance,
            super::super::SequenceTerminalForegroundProvenance::MermaidConfig
        );
        assert!(layout.browser_xhtml().contains("font-size:27px"));
        assert!(layout.browser_xhtml().contains("color:#67e8f9"));
        assert_eq!(
            crate::sequence::measure_prepared_sequence_math_label(
                Some(layout.as_ref()),
                &style,
                crate::sequence::SequenceMathHeightMode::Draw,
            )
            .expect("prepared multiline math dimensions")
            .1,
            43.0,
            "the tallest row must win over total-height averaging"
        );

        let sidecar = builder.finish().unwrap();
        let writer = sidecar.get_arc(&occurrence, "$$x$$<br>$$y$$").unwrap();
        assert!(Arc::ptr_eq(&layout, writer));
        assert_eq!(
            sidecar.foreground_provenance(&occurrence),
            Some(super::super::SequenceTerminalForegroundProvenance::MermaidConfig)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        drop(sidecar);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn exact_retained_ceiling_succeeds_and_one_byte_short_releases_everything() {
        let calls = Arc::new(AtomicUsize::new(0));
        let backend = ConfiguredMathBackend::external(Arc::new(CountingSequenceMathRenderer {
            prepare_calls: calls,
        }));
        let config = merman_core::MermaidConfig::default();
        let style = TextStyle::default();
        let text_measurer = DeterministicTextMeasurer::default();
        let measuring_meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let measuring = SequenceMathSidecarBuilder::new(
            Some(&backend),
            &config,
            &text_measurer,
            Arc::clone(&measuring_meter),
        );
        let occurrence = SequenceMathOccurrence::BlockLabel("block-".repeat(256));
        measuring.prepare(occurrence.clone(), "$$x$$", terminal(&style));
        let exact_bytes = measuring.retained_bytes();
        assert!(
            exact_bytes
                >= std::mem::size_of::<SequenceMathOccurrence>()
                    + "block-".len() * 256
                    + "$$x$$".len()
        );
        drop(measuring);
        assert_eq!(measuring_meter.prepared_text_retained_bytes(), 0);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, exact_bytes)
            .unwrap();
        let exact_meter = Arc::new(OperationWorkMeter::new(exact_policy));
        let exact = SequenceMathSidecarBuilder::new(
            Some(&backend),
            &config,
            &text_measurer,
            Arc::clone(&exact_meter),
        );
        assert!(
            exact
                .prepare(occurrence.clone(), "$$x$$", terminal(&style),)
                .is_some()
        );
        drop(exact);
        assert_eq!(exact_meter.prepared_text_retained_bytes(), 0);

        let short_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                ResourceLimitId::MaxPreparedTextRetainedBytes,
                exact_bytes - 1,
            )
            .unwrap();
        let short_meter = Arc::new(OperationWorkMeter::new(short_policy));
        let short = SequenceMathSidecarBuilder::new(
            Some(&backend),
            &config,
            &text_measurer,
            Arc::clone(&short_meter),
        );
        assert!(
            short
                .prepare(occurrence, "$$x$$", terminal(&style),)
                .is_none()
        );
        assert!(matches!(
            short.finish(),
            Err(OperationWorkError::ResourceLimitExceeded(_))
        ));
        assert_eq!(short_meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn plain_sequence_occurrences_do_not_call_or_retain_the_math_backend() {
        let calls = Arc::new(AtomicUsize::new(0));
        let backend = ConfiguredMathBackend::external(Arc::new(CountingSequenceMathRenderer {
            prepare_calls: Arc::clone(&calls),
        }));
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let text_measurer = DeterministicTextMeasurer::default();
        let builder = SequenceMathSidecarBuilder::new(
            Some(&backend),
            &merman_core::MermaidConfig::default(),
            &text_measurer,
            Arc::clone(&meter),
        );

        for index in 0..128 {
            assert!(
                builder
                    .prepare(
                        SequenceMathOccurrence::Message(index),
                        "ordinary message text",
                        terminal(&TextStyle::default()),
                    )
                    .is_none()
            );
        }

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(builder.retained_bytes(), 0);
        drop(builder.finish().expect("plain sidecar"));
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn declined_sequence_math_retains_its_reason_without_writer_reentry() {
        let backend = ConfiguredMathBackend::external(Arc::new(crate::math::NoopMathRenderer));
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let text_measurer = DeterministicTextMeasurer::default();
        let builder = SequenceMathSidecarBuilder::new(
            Some(&backend),
            &merman_core::MermaidConfig::default(),
            &text_measurer,
            Arc::clone(&meter),
        );

        assert!(
            builder
                .prepare(
                    SequenceMathOccurrence::Message(0),
                    "$$x$$",
                    terminal(&TextStyle::default()),
                )
                .is_none()
        );
        assert!(builder.retained_bytes() > 0);
        let sidecar = builder.finish().expect("declined sidecar");
        assert_eq!(
            sidecar.unavailable_reason(&SequenceMathOccurrence::Message(0)),
            Some(MathPreparationUnavailable::BackendDeclined)
        );
        assert!(
            sidecar
                .get_for_occurrence(&SequenceMathOccurrence::Message(0))
                .is_none()
        );
        drop(sidecar);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }
}
