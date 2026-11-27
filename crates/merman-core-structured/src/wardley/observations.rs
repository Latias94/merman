use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_model::SourceSpan;
use merman_model::families::wardley::WardleySourceStrategy;

use super::{SpannedNumber, SpannedText, WardleyComponentAst, WardleyPipelineComponentAst};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardleyFieldKind {
    Definition { quoted: bool },
    Reference { quoted: bool },
    Outline,
    Text,
    Number,
}

/// Observations borrowed from the active statement in preprocessed source coordinates.
pub enum WardleyObservation<'a> {
    /// The syntax pass has cleared its initial cancellation checkpoint.
    Started,
    Directive(&'static str),
    Common(&'a LangiumCommonFact),
    Field {
        kind: WardleyFieldKind,
        text: &'a str,
        detail: &'static str,
        span: SourceSpan,
        selection: SourceSpan,
    },
}

pub(super) type WardleyObserver<'a> = dyn for<'source> FnMut(WardleyObservation<'source>) + 'a;

fn observe_text(
    observe: &mut WardleyObserver<'_>,
    value: &SpannedText,
    kind: WardleyFieldKind,
    detail: &'static str,
) {
    observe(WardleyObservation::Field {
        kind,
        text: &value.text,
        detail,
        span: value.span,
        selection: value.selection,
    });
}

pub(super) fn observe_entity(
    observe: &mut WardleyObserver<'_>,
    value: &SpannedText,
    detail: &'static str,
) {
    observe_text(
        observe,
        value,
        WardleyFieldKind::Definition {
            quoted: value.quoted,
        },
        detail,
    );
}

pub(super) fn observe_reference(
    observe: &mut WardleyObserver<'_>,
    value: &SpannedText,
    detail: &'static str,
) {
    observe_text(
        observe,
        value,
        WardleyFieldKind::Reference {
            quoted: value.quoted,
        },
        detail,
    );
}

pub(super) fn observe_outline(
    observe: &mut WardleyObserver<'_>,
    value: &SpannedText,
    detail: &'static str,
) {
    observe_text(observe, value, WardleyFieldKind::Outline, detail);
}

pub(super) fn observe_payload(
    observe: &mut WardleyObserver<'_>,
    value: &SpannedText,
    detail: &'static str,
) {
    observe_text(observe, value, WardleyFieldKind::Text, detail);
}

pub(super) fn observe_number(
    observe: &mut WardleyObserver<'_>,
    value: SpannedNumber,
    detail: &'static str,
) {
    observe(WardleyObservation::Field {
        kind: WardleyFieldKind::Number,
        text: ryu_js::Buffer::new().format(value.value),
        detail,
        span: value.span,
        selection: value.span,
    });
}

pub(super) fn observe_component(
    observe: &mut WardleyObserver<'_>,
    component: &WardleyComponentAst,
) {
    observe_entity(observe, &component.positioned.name, "wardley component");
    observe_number(
        observe,
        component.positioned.visibility,
        "wardley component visibility",
    );
    observe_number(
        observe,
        component.positioned.evolution,
        "wardley component evolution",
    );
    if let Some(label) = &component.label {
        observe(WardleyObservation::Directive("label"));
        observe_number(observe, label.offset_x, "wardley label X offset");
        observe_number(observe, label.offset_y, "wardley label Y offset");
    }
    if let (Some(strategy), Some(span)) =
        (component.source_strategy, component.source_strategy_span)
    {
        let text = match strategy {
            WardleySourceStrategy::Build => "build",
            WardleySourceStrategy::Buy => "buy",
            WardleySourceStrategy::Outsource => "outsource",
            WardleySourceStrategy::Market => "market",
        };
        observe(WardleyObservation::Field {
            kind: WardleyFieldKind::Text,
            text,
            detail: "wardley source strategy",
            span,
            selection: span,
        });
    }
    if let Some(span) = component.inertia_span {
        observe(WardleyObservation::Directive("inertia"));
        observe(WardleyObservation::Field {
            kind: WardleyFieldKind::Text,
            text: "inertia",
            detail: "wardley inertia",
            span,
            selection: span,
        });
    }
}

pub(super) fn observe_pipeline_component(
    observe: &mut WardleyObserver<'_>,
    component: &WardleyPipelineComponentAst,
) {
    observe_entity(observe, &component.name, "wardley pipeline component");
    observe_number(
        observe,
        component.evolution,
        "wardley pipeline component evolution",
    );
    if let Some(label) = &component.label {
        observe(WardleyObservation::Directive("label"));
        observe_number(observe, label.offset_x, "wardley label X offset");
        observe_number(observe, label.offset_y, "wardley label Y offset");
    }
}
