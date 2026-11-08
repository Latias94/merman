//! Operation-owned final Note rows and their exact built-in measurement binding.

use std::sync::Arc;

use merman_model::OperationPhase;
use merman_model::families::sequence::{SequenceDiagramModel, SequenceMessage};

use crate::Result;
use crate::environment::{BuiltinTextMeasurementOperationCarrier, TextMeasurementOperation};
use crate::resources::{
    OperationWorkMeter, PreparedTextRetainedReservation, ResourceLimitCause, ResourceLimitExceeded,
    ResourceLimitId, ResourceLimitOverride, ResourceLimitPhase,
};
use crate::text::TextStyle;

use super::message_metrics::SequenceMessageMeasurementBinding;
use super::notes::SequenceWrappedSideNoteLabelContext;

#[derive(Debug)]
struct SequenceNoteMeasurementBinding {
    wrapping: SequenceMessageMeasurementBinding,
    terminal_style: TextStyle,
    terminal_carriers: [BuiltinTextMeasurementOperationCarrier; 2],
}

impl SequenceNoteMeasurementBinding {
    fn for_context(ctx: &SequenceWrappedSideNoteLabelContext<'_>) -> Option<Self> {
        Some(Self {
            wrapping: SequenceMessageMeasurementBinding::for_measurer(
                ctx.measurement_style,
                ctx.measurer,
            )?,
            terminal_style: super::sequence_drawn_text_style(ctx.terminal_style, ctx.config),
            terminal_carriers: [
                ctx.measurer
                    .builtin_operation_carrier(TextMeasurementOperation::TspanBBoxWidth)?,
                ctx.measurer
                    .builtin_operation_carrier(TextMeasurementOperation::TspanBBoxHeight)?,
            ],
        })
    }

    fn matches(&self, ctx: &SequenceWrappedSideNoteLabelContext<'_>) -> bool {
        if !self.wrapping.matches(ctx.measurement_style, ctx.measurer)
            || ctx
                .measurer
                .builtin_operation_carrier(TextMeasurementOperation::TspanBBoxWidth)
                != Some(self.terminal_carriers[0])
            || ctx
                .measurer
                .builtin_operation_carrier(TextMeasurementOperation::TspanBBoxHeight)
                != Some(self.terminal_carriers[1])
        {
            return false;
        }
        let terminal = super::sequence_drawn_text_style(ctx.terminal_style, ctx.config);
        self.terminal_style.font_family == terminal.font_family
            && self.terminal_style.font_size.to_bits() == terminal.font_size.to_bits()
            && self.terminal_style.font_weight == terminal.font_weight
            && self.terminal_style.font_style == terminal.font_style
    }

    fn retained_heap_bytes(&self) -> Option<usize> {
        self.wrapping
            .retained_heap_bytes()?
            .checked_add(
                self.terminal_style
                    .font_family
                    .as_ref()
                    .map_or(0, String::capacity),
            )?
            .checked_add(
                self.terminal_style
                    .font_weight
                    .as_ref()
                    .map_or(0, String::capacity),
            )?
            .checked_add(
                self.terminal_style
                    .font_style
                    .as_ref()
                    .map_or(0, String::capacity),
            )
    }
}

/// Retain layout's resolved numbers; SVG's legacy numeric reader has different coercions.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct SequenceNoteWrapWidths {
    pub(crate) default_width: f64,
    pub(crate) note_margin: f64,
    pub(crate) wrap_padding: f64,
}

impl SequenceNoteWrapWidths {
    fn for_context(ctx: &SequenceWrappedSideNoteLabelContext<'_>) -> Self {
        Self {
            default_width: ctx.default_width,
            note_margin: ctx.note_margin,
            wrap_padding: ctx.wrap_padding,
        }
    }

    fn bits(self) -> [u64; 3] {
        [self.default_width, self.note_margin, self.wrap_padding].map(f64::to_bits)
    }
}

/// Rows are indexed by immutable model occurrence, never by user-visible message IDs.
/// The artifact owns both the allocations and their reservation; views only borrow them.
#[derive(Debug, Default)]
pub(crate) struct SequencePreparedNoteLabels {
    model_messages_identity: usize,
    model_messages_len: usize,
    wrap_widths: SequenceNoteWrapWidths,
    binding: Option<SequenceNoteMeasurementBinding>,
    entries: Vec<(usize, String)>,
    _retained: Option<PreparedTextRetainedReservation>,
}

impl SequencePreparedNoteLabels {
    pub(super) fn new(
        model: &SequenceDiagramModel,
        entries: Vec<(usize, String)>,
        ctx: &SequenceWrappedSideNoteLabelContext<'_>,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        work_meter.checkpoint(OperationPhase::Layout)?;
        if entries.is_empty() {
            return Ok(Self::default());
        }
        work_meter.preflight_prepared_text_retained_bytes(std::mem::size_of::<Self>())?;
        let binding = SequenceNoteMeasurementBinding::for_context(ctx);
        let overflow = || retained_size_overflow(work_meter);
        let mut retained_bytes = entries
            .capacity()
            .checked_mul(std::mem::size_of::<(usize, String)>())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(overflow)?;
        if let Some(binding) = &binding {
            retained_bytes = binding
                .retained_heap_bytes()
                .and_then(|bytes| retained_bytes.checked_add(bytes))
                .ok_or_else(overflow)?;
        }
        for (index, (_, text)) in entries.iter().enumerate() {
            if index.is_multiple_of(super::SEQUENCE_CHECKPOINT_INTERVAL) {
                work_meter.checkpoint(OperationPhase::Layout)?;
            }
            retained_bytes = retained_bytes
                .checked_add(text.capacity())
                .ok_or_else(overflow)?;
        }
        let retained = work_meter.reserve_prepared_text_retained_bytes(retained_bytes)?;
        work_meter.checkpoint(OperationPhase::Layout)?;
        Ok(Self {
            model_messages_identity: model.messages.as_ptr() as usize,
            model_messages_len: model.messages.len(),
            wrap_widths: SequenceNoteWrapWidths::for_context(ctx),
            binding,
            entries,
            _retained: Some(retained),
        })
    }

    pub(crate) const fn wrap_widths(&self) -> SequenceNoteWrapWidths {
        self.wrap_widths
    }

    pub(crate) fn view<'a>(
        &'a self,
        model: &'a SequenceDiagramModel,
        ctx: &SequenceWrappedSideNoteLabelContext<'_>,
    ) -> SequencePreparedNoteLabelView<'a> {
        if self.model_messages_identity == model.messages.as_ptr() as usize
            && self.model_messages_len == model.messages.len()
            && self.wrap_widths.bits() == SequenceNoteWrapWidths::for_context(ctx).bits()
            && self
                .binding
                .as_ref()
                .is_some_and(|binding| binding.matches(ctx))
        {
            SequencePreparedNoteLabelView {
                model_messages: &model.messages,
                entries: &self.entries,
            }
        } else {
            SequencePreparedNoteLabelView::empty()
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SequencePreparedNoteLabelView<'a> {
    model_messages: &'a [SequenceMessage],
    entries: &'a [(usize, String)],
}

impl SequencePreparedNoteLabelView<'_> {
    pub(crate) const fn empty() -> Self {
        Self {
            model_messages: &[],
            entries: &[],
        }
    }
}

impl<'a> SequencePreparedNoteLabelView<'a> {
    pub(crate) fn get(self, index: usize, message: &SequenceMessage) -> Option<&'a str> {
        if !std::ptr::eq(self.model_messages.get(index)?, message) {
            return None;
        }
        let entry = self
            .entries
            .binary_search_by_key(&index, |(owner, _)| *owner)
            .ok()?;
        Some(self.entries[entry].1.as_str())
    }
}

fn retained_size_overflow(work_meter: &OperationWorkMeter) -> crate::Error {
    let policy = work_meter.policy();
    let limit = ResourceLimitId::MaxPreparedTextRetainedBytes;
    ResourceLimitExceeded {
        cause: ResourceLimitCause::ArithmeticOverflow,
        phase: ResourceLimitPhase::LayoutModel,
        limit: limit.as_str(),
        actual: usize::MAX,
        max: policy.value(limit).unwrap_or(usize::MAX),
        profile: policy.profile(),
        explicit_overrides: policy
            .explicit_overrides()
            .map(|(id, value)| ResourceLimitOverride { id, value })
            .collect(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{RenderEnvironment, TextMeasurementPhase};
    use crate::resources::RenderResourcePolicy;
    use crate::text::DeterministicTextMeasurer;
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::presentation::{PresentationConfig, RenderSemanticModel};

    fn note_model() -> SequenceDiagramModel {
        let presentation = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig: {sequence: {wrap: true}}\n---\nsequenceDiagram\nparticipant A\nNote right of A: first\nNote right of A: second",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Sequence(model) = presentation.model() else {
            panic!("expected Sequence model");
        };
        model.clone()
    }

    #[test]
    fn final_note_rows_bind_occurrences_styles_dimensions_and_every_planner_operation() {
        struct OpaqueTerminal<'a>(&'a dyn crate::text::TextMeasurer);
        impl crate::text::TextMeasurer for OpaqueTerminal<'_> {
            fn measure(&self, text: &str, style: &TextStyle) -> crate::text::TextMetrics {
                self.0.measure(text, style)
            }
            #[allow(private_interfaces)]
            fn builtin_operation_carrier(
                &self,
                operation: TextMeasurementOperation,
            ) -> Option<BuiltinTextMeasurementOperationCarrier> {
                (operation == TextMeasurementOperation::MermaidCalculateTextDimensions)
                    .then(|| self.0.builtin_operation_carrier(operation))
                    .flatten()
            }
        }

        let mut model = note_model();
        assert_eq!(model.messages.len(), 2);
        model.messages[1].id = model.messages[0].id.clone();
        let style = TextStyle {
            font_family: Some("Georgia".to_string()),
            font_size: 40.0,
            font_weight: Some("700".to_string()),
            font_style: Some("italic".to_string()),
        };
        let config = PresentationConfig::default();
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let ctx = SequenceWrappedSideNoteLabelContext {
            measurer: &measurer,
            measurement_style: &style,
            terminal_style: &style,
            config: &config,
            default_width: 150.0,
            note_margin: 10.0,
            wrap_padding: 10.0,
            checkpoints: super::super::SequenceTextCheckpoints::for_phase(
                &meter,
                OperationPhase::Layout,
            ),
        };
        let labels = SequencePreparedNoteLabels::new(
            &model,
            vec![(0, "first".to_string()), (1, "second".to_string())],
            &ctx,
            &meter,
        )
        .unwrap();
        let view = labels.view(&model, &ctx);
        assert_eq!(view.get(0, &model.messages[0]), Some("first"));
        assert_eq!(view.get(1, &model.messages[1]), Some("second"));
        assert_eq!(view.get(0, &model.messages[1]), None);
        assert_eq!(view.get(0, &model.messages[0].clone()), None);
        let equal_model = model.clone();
        assert_eq!(
            labels
                .view(&equal_model, &ctx)
                .get(0, &equal_model.messages[0]),
            None
        );

        for changed in [
            TextStyle {
                font_family: Some("monospace".to_string()),
                ..style.clone()
            },
            TextStyle {
                font_size: 41.0,
                ..style.clone()
            },
            TextStyle {
                font_weight: Some("400".to_string()),
                ..style.clone()
            },
            TextStyle {
                font_style: Some("oblique".to_string()),
                ..style.clone()
            },
        ] {
            let measurement_ctx = SequenceWrappedSideNoteLabelContext {
                measurement_style: &changed,
                ..ctx
            };
            assert_eq!(
                labels
                    .view(&model, &measurement_ctx)
                    .get(0, &model.messages[0]),
                None
            );
            let terminal_ctx = SequenceWrappedSideNoteLabelContext {
                terminal_style: &changed,
                ..ctx
            };
            assert_eq!(
                labels
                    .view(&model, &terminal_ctx)
                    .get(0, &model.messages[0]),
                None
            );
        }
        for (width, margin, padding) in [
            (151.0, 10.0, 10.0),
            (150.0, 11.0, 10.0),
            (150.0, 10.0, 11.0),
        ] {
            let changed_ctx = SequenceWrappedSideNoteLabelContext {
                default_width: width,
                note_margin: margin,
                wrap_padding: padding,
                ..ctx
            };
            assert_eq!(
                labels.view(&model, &changed_ctx).get(0, &model.messages[0]),
                None
            );
        }
        let opaque_terminal = OpaqueTerminal(&measurer);
        let custom = DeterministicTextMeasurer::default();
        for opaque in [&opaque_terminal as &dyn crate::text::TextMeasurer, &custom] {
            let opaque_ctx = SequenceWrappedSideNoteLabelContext {
                measurer: opaque,
                ..ctx
            };
            assert_eq!(
                labels.view(&model, &opaque_ctx).get(0, &model.messages[0]),
                None
            );
            let opaque_labels = SequencePreparedNoteLabels::new(
                &model,
                vec![(0, "first".to_string())],
                &opaque_ctx,
                &meter,
            )
            .unwrap();
            assert_eq!(
                opaque_labels.view(&model, &ctx).get(0, &model.messages[0]),
                None
            );
        }
    }

    #[test]
    fn retained_note_capacity_has_an_exact_ceiling_and_releases_after_cancellation() {
        let model = note_model();
        let style = TextStyle {
            font_family: Some("Georgia".to_string()),
            font_weight: Some("700".to_string()),
            font_style: Some("italic".to_string()),
            ..TextStyle::default()
        };
        let config = PresentationConfig::default();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let entries = || {
            let mut text = String::with_capacity(128);
            text.push_str("first");
            let mut entries = Vec::with_capacity(8);
            entries.push((0, text));
            entries
        };
        let prepare = |meter: &Arc<OperationWorkMeter>| {
            SequencePreparedNoteLabels::new(
                &model,
                entries(),
                &SequenceWrappedSideNoteLabelContext {
                    measurer: &measurer,
                    measurement_style: &style,
                    terminal_style: &style,
                    config: &config,
                    default_width: 150.0,
                    note_margin: 10.0,
                    wrap_padding: 10.0,
                    checkpoints: super::super::SequenceTextCheckpoints::for_phase(
                        meter,
                        OperationPhase::Layout,
                    ),
                },
                meter,
            )
        };
        let measuring = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let labels = prepare(&measuring).unwrap();
        let exact_bytes = measuring.prepared_text_retained_bytes();
        assert!(
            exact_bytes
                > std::mem::size_of::<SequencePreparedNoteLabels>()
                    + 8 * std::mem::size_of::<(usize, String)>()
                    + 128,
            "retained accounting includes spare entry/text capacity and both style bindings"
        );
        drop(labels);
        assert_eq!(measuring.prepared_text_retained_bytes(), 0);

        for ceiling in [1, exact_bytes - 1, exact_bytes] {
            let meter = Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, ceiling)
                    .unwrap(),
            ));
            if ceiling == exact_bytes {
                let labels = prepare(&meter).expect("the exact retained capacity is admitted");
                assert_eq!(meter.prepared_text_retained_bytes(), exact_bytes);
                meter.control().cancel();
                drop(labels);
            } else {
                let crate::Error::ResourceLimitExceeded(error) = prepare(&meter).unwrap_err()
                else {
                    panic!("expected the retained Note byte ceiling");
                };
                assert_eq!(
                    error.limit,
                    ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
                );
                assert_eq!(error.cause, ResourceLimitCause::Ceiling);
                assert_eq!(error.max, ceiling);
                assert!(error.actual > ceiling);
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }
}
