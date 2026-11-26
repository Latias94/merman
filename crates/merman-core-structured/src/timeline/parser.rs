use super::{
    ParsedTimelineEvents, TimelineEventIssue, TimelinePeriodSuffix, TimelineSource,
    parse_acc_descr_block, parse_colon_directive, parse_space_directive, parse_timeline_events,
    parse_timeline_header, parse_timeline_period, starts_timeline_comment,
};
use merman_core_preprocess::scan::LineCursor;
use merman_model::families::timeline::{TimelineDiagramModel, TimelineDirection, TimelineTask};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

#[derive(Debug, Clone, Copy)]
pub enum TimelineSourceIssue<'source> {
    ExpectedHeader(SourceSpan),
    UnexpectedHeader(SourceSpan),
    EmptyAccTitle(usize),
    EmptyAccDescr(usize),
    UnterminatedAccDescr(SourceSpan),
    EventWithoutTask(SourceSpan),
    EventSyntax {
        statement: TimelineSource<'source>,
        issue: TimelineEventIssue,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum TimelineSourceFact<'source> {
    Title(TimelineSource<'source>),
    AccTitle(TimelineSource<'source>),
    AccDescr(TimelineSource<'source>),
    Section {
        selection: TimelineSource<'source>,
        statement: SourceSpan,
    },
    Task {
        selection: TimelineSource<'source>,
        statement: SourceSpan,
    },
    Event(TimelineSource<'source>),
}

#[derive(Debug, Clone, Copy)]
pub enum TimelineObservation<'source> {
    Issue(TimelineSourceIssue<'source>),
    Fact(TimelineSourceFact<'source>),
}

#[derive(Debug)]
pub struct TimelineParsedSource {
    pub model: TimelineDiagramModel,
    pub header_seen: bool,
}

#[derive(Default)]
struct TimelineDb {
    direction: TimelineDirection,
    title: String,
    acc_title: String,
    acc_descr: String,
    current_section: String,
    current_section_index: Option<usize>,
    sections: Vec<String>,
    tasks: Vec<TimelineTask>,
    next_id: i64,
}

impl TimelineDb {
    fn add_section(&mut self, text: &str) {
        self.current_section = text.to_string();
        self.current_section_index = Some(self.sections.len());
        self.sections.push(text.to_string());
    }

    fn add_task(&mut self, period: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(TimelineTask {
            id,
            section: self.current_section.clone(),
            section_index: self.current_section_index,
            task_type: self.current_section.clone(),
            task: period.to_string(),
            score: 0,
            events: Vec::new(),
        });
    }

    fn add_events<'source>(
        &mut self,
        parsed: ParsedTimelineEvents<'source>,
        statement: TimelineSource<'source>,
        capture_facts: bool,
        observe: &mut dyn FnMut(TimelineObservation<'source>),
    ) {
        for event in parsed.events {
            if let Some(last) = self.tasks.last_mut() {
                last.events.push(event.text.to_string());
                if capture_facts {
                    observe(TimelineObservation::Fact(TimelineSourceFact::Event(event)));
                }
            } else {
                observe(TimelineObservation::Issue(
                    TimelineSourceIssue::EventWithoutTask(event.span()),
                ));
            }
        }
        if let Some(issue) = parsed.issue {
            observe(TimelineObservation::Issue(
                TimelineSourceIssue::EventSyntax { statement, issue },
            ));
        }
    }
}

fn scan_timeline_events<'source>(
    input: TimelineSource<'source>,
    lines: &mut LineCursor<'source>,
    control: &OperationControl,
) -> OperationControlResult<ParsedTimelineEvents<'source>> {
    let source = TimelineSource::new(&lines.source()[input.start..], input.start);
    parse_timeline_events(source, |offset| lines.resume_at(offset), control)
}

/// Builds the model and reports borrowed observations in source order.
///
/// Facts are optional, but issues are always reported. On cancellation there is no model.
pub fn parse_timeline_semantic_source_controlled<'source>(
    code: &'source str,
    capture_facts: bool,
    observe: &mut dyn FnMut(TimelineObservation<'source>),
    control: &OperationControl,
) -> OperationControlResult<TimelineParsedSource> {
    control.checkpoint()?;
    let mut db = TimelineDb::default();
    let mut lines = LineCursor::new(code);
    let mut header_seen = false;

    while let Some((line, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let line = TimelineSource::new(line, line_start);
        let significant = line.trim();
        if significant.text.is_empty() {
            continue;
        }

        if starts_timeline_comment(significant.text) {
            continue;
        }
        if significant.text.starts_with('#') {
            continue;
        }

        if !header_seen {
            let Some((direction, remainder)) = parse_timeline_header(significant) else {
                observe(TimelineObservation::Issue(
                    TimelineSourceIssue::ExpectedHeader(significant.span()),
                ));
                continue;
            };

            header_seen = true;
            db.direction = direction;
            if !remainder.text.is_empty() {
                lines.resume_same_line_at(remainder.start);
            }
            continue;
        }

        let statement = line.trim_start();
        if let Some((_, remainder)) = parse_timeline_header(statement) {
            let span = SourceSpan::new(statement.start, remainder.start);
            observe(TimelineObservation::Issue(
                TimelineSourceIssue::UnexpectedHeader(span),
            ));
            continue;
        }
        let token_source = TimelineSource::new(&code[statement.start..], statement.start);
        if let Some(value) = parse_space_directive(token_source, "title", control)? {
            if capture_facts {
                observe(TimelineObservation::Fact(TimelineSourceFact::Title(value)));
            }
            db.title = value.text.to_string();
            lines.resume_at(value.end());
            continue;
        }

        if let Some(value) = parse_colon_directive(token_source, "accTitle", control)? {
            if capture_facts {
                observe(TimelineObservation::Fact(TimelineSourceFact::AccTitle(
                    value,
                )));
            }
            if value.text.is_empty() {
                observe(TimelineObservation::Issue(
                    TimelineSourceIssue::EmptyAccTitle(value.start),
                ));
            } else {
                db.acc_title = value.text.to_string();
            }
            lines.resume_at(value.end());
            continue;
        }

        if let Some(value) = parse_colon_directive(token_source, "accDescr", control)? {
            if capture_facts {
                observe(TimelineObservation::Fact(TimelineSourceFact::AccDescr(
                    value,
                )));
            }
            if value.text.is_empty() {
                observe(TimelineObservation::Issue(
                    TimelineSourceIssue::EmptyAccDescr(value.start),
                ));
            } else {
                db.acc_descr = value.text.to_string();
            }
            lines.resume_at(value.end());
            continue;
        }

        if let Some(parsed) =
            parse_acc_descr_block(token_source, |offset| lines.resume_at(offset), control)?
        {
            if capture_facts {
                observe(TimelineObservation::Fact(TimelineSourceFact::AccDescr(
                    parsed.value,
                )));
            }
            if parsed.closing.is_none() {
                let span = SourceSpan::new(parsed.keyword.start, code.len());
                observe(TimelineObservation::Issue(
                    TimelineSourceIssue::UnterminatedAccDescr(span),
                ));
            }
            if !parsed.value.text.is_empty() {
                db.acc_descr = parsed.value.text.to_string();
            }
            continue;
        }

        if let Some(value) = parse_space_directive(token_source, "section", control)? {
            if capture_facts {
                observe(TimelineObservation::Fact(TimelineSourceFact::Section {
                    selection: value.trim(),
                    statement: SourceSpan::new(statement.start, value.end()),
                }));
            }
            db.add_section(value.text);
            lines.resume_at(value.end());
            continue;
        }

        if statement.text.starts_with(':') {
            let parsed = scan_timeline_events(statement, &mut lines, control)?;
            db.add_events(parsed, statement, capture_facts, observe);
            continue;
        }

        let parsed = parse_timeline_period(statement);
        if parsed.task.text.is_empty() {
            continue;
        }

        if capture_facts {
            observe(TimelineObservation::Fact(TimelineSourceFact::Task {
                selection: parsed.task,
                statement: statement.span(),
            }));
        }
        db.add_task(parsed.period.text);

        if let TimelinePeriodSuffix::Events(events_source) = parsed.suffix {
            let events = scan_timeline_events(events_source, &mut lines, control)?;
            db.add_events(events, events_source, capture_facts, observe);
        }
    }

    control.checkpoint()?;
    let model = TimelineDiagramModel {
        direction: db.direction,
        title: (!db.title.is_empty()).then_some(db.title),
        acc_title: (!db.acc_title.is_empty()).then_some(db.acc_title),
        acc_descr: (!db.acc_descr.is_empty()).then_some(db.acc_descr),
        sections: db.sections,
        tasks: db.tasks,
    };
    Ok(TimelineParsedSource { model, header_seen })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_source_builds_model_and_reports_recovery_in_order() {
        let source = concat!(
            "invalid\r\ntimeline TD\r\n: orphan\r\nsection Plan\r\n",
            "β: Go\r\n:bad\r\nNext\r\n: done"
        );
        let mut trace = Vec::new();
        let parsed = parse_timeline_semantic_source_controlled(
            source,
            true,
            &mut |observation| trace.push(observation),
            &OperationControl::new(),
        )
        .unwrap();

        assert!(parsed.header_seen);
        assert_eq!(parsed.model.direction, TimelineDirection::TopDown);
        assert_eq!(parsed.model.sections, ["Plan"]);
        let tasks = parsed.model.tasks;
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].section_index, Some(0));
        assert_eq!(tasks[0].events, ["Go"]);
        assert_eq!(tasks[1].task, "Next");
        assert_eq!(tasks[1].events, ["done"]);
        let span_of = |text: &str| {
            let start = source.find(text).unwrap();
            SourceSpan::new(start, start + text.len())
        };
        assert!(
            matches!(
                trace.as_slice(),
                [
                    TimelineObservation::Issue(TimelineSourceIssue::ExpectedHeader(header)),
                    TimelineObservation::Issue(TimelineSourceIssue::EventWithoutTask(orphan)),
                    TimelineObservation::Fact(TimelineSourceFact::Section { selection: section, .. }),
                    TimelineObservation::Fact(TimelineSourceFact::Task { selection: first, .. }),
                    TimelineObservation::Fact(TimelineSourceFact::Event(go)),
                    TimelineObservation::Issue(TimelineSourceIssue::EventSyntax {
                        issue: TimelineEventIssue::MissingWhitespace(missing),
                        ..
                    }),
                    TimelineObservation::Fact(TimelineSourceFact::Task { selection: next, .. }),
                    TimelineObservation::Fact(TimelineSourceFact::Event(done)),
                ]
                if *header == span_of("invalid") && *orphan == span_of("orphan")
                    && section.span() == span_of("Plan") && first.span() == span_of("β")
                    && go.span() == span_of("Go") && *missing == source.find("bad").unwrap()
                    && next.span() == span_of("Next") && done.span() == span_of("done")
            ),
            "{trace:?}"
        );

        let mut lean_issues = 0;
        let lean = parse_timeline_semantic_source_controlled(
            source,
            false,
            &mut |observation| match observation {
                TimelineObservation::Issue(_) => lean_issues += 1,
                TimelineObservation::Fact(_) => panic!("lean parse reported a rich fact"),
            },
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(lean_issues, 3);
        assert_eq!(lean.model.tasks[0].events, ["Go"]);
    }

    #[test]
    fn timeline_source_final_checkpoint_precedes_model_publication() {
        for capture_facts in [false, true] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(2);
            assert!(
                parse_timeline_semantic_source_controlled(
                    "timeline",
                    capture_facts,
                    &mut |_| panic!("bare header has no observations"),
                    &control,
                )
                .is_err()
            );
        }
    }
}
