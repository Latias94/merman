use merman_model::SourceSpan;

use super::date::relative_ref_ids_range;
use super::parse::{ClickStatementParts, SpannedText, gantt_statement_span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GanttSymbolKind {
    DateFormat,
    AxisFormat,
    TickInterval,
    Includes,
    Excludes,
    TodayMarker,
    Weekday,
    Weekend,
    Title,
    Section,
    AccTitle,
    AccDescr,
    Task,
    Dependency,
    ClickTarget,
    ClickHref,
    ClickCallback,
    ClickCallbackArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GanttExpectedSyntax {
    Payload,
    InteractionAction,
    NodeIdentifier,
}

/// An observation borrowed from the active statement in preprocessed source coordinates.
#[derive(Debug, Clone, Copy)]
pub enum GanttObservation<'a> {
    DirectivePrefix(&'static str),
    ExpectedSyntax {
        kind: GanttExpectedSyntax,
        span: SourceSpan,
    },
    Symbol {
        kind: GanttSymbolKind,
        text: &'a str,
        statement: SourceSpan,
        selection: SourceSpan,
    },
}

pub(super) type GanttObserver<'observer> =
    dyn for<'source> FnMut(GanttObservation<'source>) + 'observer;

pub(super) fn collect_gantt_click_symbols(
    line: &str,
    line_start: usize,
    click: ClickStatementParts<'_>,
    facts: &mut GanttObserver<'_>,
) {
    let statement_span = gantt_statement_span(line, line_start);
    push_gantt_delimited_id_symbols(
        click.ids,
        ',',
        GanttSymbolKind::ClickTarget,
        statement_span,
        facts,
    );

    for action in [click.href_keyword, click.call_keyword]
        .into_iter()
        .flatten()
    {
        facts(GanttObservation::ExpectedSyntax {
            kind: GanttExpectedSyntax::InteractionAction,
            span: action,
        });
    }

    if let Some(href) = click.href {
        push_gantt_payload_symbol(line, line_start, href, GanttSymbolKind::ClickHref, facts);
    }

    if let Some(call) = click.call {
        push_gantt_payload_symbol(
            line,
            line_start,
            call.name,
            GanttSymbolKind::ClickCallback,
            facts,
        );
        if let Some(args) = call.args {
            push_gantt_payload_symbol(
                line,
                line_start,
                args,
                GanttSymbolKind::ClickCallbackArgs,
                facts,
            );
        }
    }
}

pub(super) fn collect_gantt_section_symbol(
    line: &str,
    line_start: usize,
    section: SpannedText<'_>,
    facts: &mut GanttObserver<'_>,
) {
    let Some(section) = section.trim() else {
        return;
    };
    facts(GanttObservation::Symbol {
        kind: GanttSymbolKind::Section,
        text: section.text,
        statement: gantt_statement_span(line, line_start),
        selection: section.span(),
    });
}

pub(super) fn push_gantt_payload_symbol(
    line: &str,
    line_start: usize,
    field: SpannedText<'_>,
    kind: GanttSymbolKind,
    facts: &mut GanttObserver<'_>,
) {
    let Some(field) = field.trim() else {
        return;
    };
    facts(GanttObservation::ExpectedSyntax {
        kind: GanttExpectedSyntax::Payload,
        span: field.span(),
    });
    facts(GanttObservation::Symbol {
        kind,
        text: field.text,
        statement: gantt_statement_span(line, line_start),
        selection: field.span(),
    });
}

pub(super) fn collect_gantt_task_field_symbols(
    fields: &[SpannedText<'_>],
    statement_span: SourceSpan,
    facts: &mut GanttObserver<'_>,
) {
    let mut field_start = 0usize;
    while fields
        .get(field_start)
        .is_some_and(|field| is_gantt_task_tag(field.text))
    {
        field_start += 1;
    }

    let fields = &fields[field_start..];
    match fields {
        [end_data] => push_gantt_relative_ref_symbols(end_data, statement_span, facts),
        [start_data, end_data] => {
            push_gantt_relative_ref_symbols(start_data, statement_span, facts);
            push_gantt_relative_ref_symbols(end_data, statement_span, facts);
        }
        [id, start_data, end_data] => {
            push_gantt_id_symbol(*id, GanttSymbolKind::Task, statement_span, facts);
            push_gantt_relative_ref_symbols(start_data, statement_span, facts);
            push_gantt_relative_ref_symbols(end_data, statement_span, facts);
        }
        _ => {}
    }
}

fn is_gantt_task_tag(text: &str) -> bool {
    matches!(text, "active" | "done" | "crit" | "milestone" | "vert")
}

fn push_gantt_relative_ref_symbols(
    field: &SpannedText<'_>,
    statement_span: SourceSpan,
    facts: &mut GanttObserver<'_>,
) {
    for keyword in ["after", "until"] {
        let Some(range) = relative_ref_ids_range(field.text, keyword) else {
            continue;
        };
        push_gantt_delimited_id_symbols(
            SpannedText {
                text: &field.text[range.clone()],
                start: field.start + range.start,
                end: field.start + range.end,
            },
            ' ',
            GanttSymbolKind::Dependency,
            statement_span,
            facts,
        );
    }
}

fn push_gantt_delimited_id_symbols(
    field: SpannedText<'_>,
    delimiter: char,
    kind: GanttSymbolKind,
    statement_span: SourceSpan,
    facts: &mut GanttObserver<'_>,
) {
    let mut segment_start = 0usize;
    for (idx, ch) in field.text.char_indices() {
        if ch == delimiter {
            push_gantt_id_symbol(
                SpannedText {
                    text: &field.text[segment_start..idx],
                    start: field.start + segment_start,
                    end: field.start + idx,
                },
                kind,
                statement_span,
                facts,
            );
            segment_start = idx + ch.len_utf8();
        }
    }

    push_gantt_id_symbol(
        SpannedText {
            text: &field.text[segment_start..],
            start: field.start + segment_start,
            end: field.end,
        },
        kind,
        statement_span,
        facts,
    );
}

fn push_gantt_id_symbol(
    field: SpannedText<'_>,
    kind: GanttSymbolKind,
    statement_span: SourceSpan,
    facts: &mut GanttObserver<'_>,
) {
    let Some(field) = field.trim() else {
        return;
    };
    facts(GanttObservation::ExpectedSyntax {
        kind: GanttExpectedSyntax::NodeIdentifier,
        span: field.span(),
    });
    facts(GanttObservation::Symbol {
        kind,
        text: field.text,
        statement: statement_span,
        selection: field.span(),
    });
}
