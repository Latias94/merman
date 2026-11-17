use crate::Result;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextDocument, charge_text_layout};
use crate::{AsciiColorMode, AsciiDiagram, AsciiProjection, AsciiSourceContext};
#[cfg(test)]
use merman_model::families::gantt::GanttTask;
use merman_model::families::gantt::GanttTerminalView;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

mod timeline;

pub(super) fn primary_projection(
    model: GanttTerminalView<'_>,
    execution: AsciiExecution<'_>,
) -> Result<AsciiProjection> {
    let resources = execution.new_resource_context(merman_model::OperationPhase::Semantic);
    let index =
        admit_then_materialize_gantt_structure(model, &resources, |admission, resources| {
            GanttTaskIndex::materialize(model, admission, resources)
        })?;
    Ok(
        if timeline::domain(model, index.as_ref(), &resources)?.is_some() {
            AsciiProjection::Diagrammatic
        } else {
            AsciiProjection::StructuredText
        },
    )
}

pub(super) fn render_gantt_diagram(
    model: GanttTerminalView<'_>,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_model::time::LocalTimeZone,
    source: AsciiSourceContext<'_>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    crate::output::preflight_semantic_model(&AsciiDiagram::Gantt(model), source, execution)?;
    let layout_resources = execution.new_resource_context(merman_model::OperationPhase::Layout);
    let task_index = admit_then_materialize_gantt_structure(
        model,
        &layout_resources,
        |admission, resources| GanttTaskIndex::materialize(model, admission, resources),
    )?;
    let topology =
        if let Some(domain) = timeline::domain(model, task_index.as_ref(), &layout_resources)? {
            let mut document = BudgetedTextDocument::from_resources(
                layout_resources.scoped(),
                &options.with_color_mode(AsciiColorMode::Plain),
            );
            timeline::write(
                model,
                task_index.as_ref(),
                domain,
                local_time_zone,
                options,
                &mut document,
                execution,
            )?;
            Some(document.finish()?)
        } else {
            None
        };
    crate::output::gantt::render_primary(
        &AsciiDiagram::Gantt(model),
        source,
        options,
        execution,
        topology.as_deref(),
    )
}

#[derive(Debug, Clone, Copy)]
struct GanttStructureAdmission {
    task_capacity: usize,
    section_lookup_capacity: usize,
    grouped: bool,
}

impl GanttStructureAdmission {
    fn preflight(model: GanttTerminalView<'_>, resources: &ResourceContext) -> Result<Self> {
        let metadata = model.metadata();
        let task_capacity = model.tasks().len();
        let grouped = !metadata.sections.is_empty();

        // The declared-section lookup contains exactly one entry per authored occurrence. Orphan
        // sections are owned separately by the per-task grouped index.
        let section_lookup_capacity = if grouped { metadata.sections.len() } else { 0 };
        let allocation_work = if grouped {
            let grouped_task_slots = resources.checked_work_mul(task_capacity, 3)?;
            let indexed_task_slots =
                resources.checked_work_add(task_capacity, grouped_task_slots)?;
            resources.checked_work_add(indexed_task_slots, section_lookup_capacity)?
        } else {
            task_capacity
        };
        resources.charge_layout_work(allocation_work)?;

        // Hashing borrowed identifiers is allocation-free, but preflight their terminal-safety
        // work before any input-sized container is allocated.
        for section in metadata.sections {
            charge_text_layout(resources, section)?;
        }
        for task in model.tasks() {
            let task = task.semantic;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, &task.id)?;
            charge_text_layout(resources, &task.section)?;
            if task.id.is_empty() {
                return Err(crate::error::AsciiError::UnsupportedFeature {
                    diagram_type: "gantt",
                    feature: "empty task ids",
                });
            }
            if let Some(section_index) = task.section_index
                && metadata.sections.get(section_index).map(String::as_str)
                    != Some(task.section.as_str())
            {
                return Err(invalid_task_section_occurrence());
            }
        }

        Ok(Self {
            task_capacity,
            section_lookup_capacity,
            grouped,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GanttSectionKey<'model> {
    Declared(usize),
    Orphan(&'model str),
}

#[derive(Debug, Clone, Copy)]
enum DeclaredSectionMatch {
    Unique(usize),
    Ambiguous,
}

struct GanttTaskIndex<'model> {
    orphan_section_order: Vec<&'model str>,
    tasks_by_section: HashMap<GanttSectionKey<'model>, Vec<usize>>,
}

impl<'model> GanttTaskIndex<'model> {
    fn materialize(
        model: GanttTerminalView<'model>,
        admission: GanttStructureAdmission,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        let metadata = model.metadata();
        resources.checkpoint()?;
        let mut task_ids = HashSet::new();
        task_ids
            .try_reserve(admission.task_capacity)
            .map_err(|_| layout_allocation_error())?;

        let mut grouped_index = if admission.grouped {
            resources.checkpoint()?;
            let mut orphan_section_order = Vec::new();
            let mut tasks_by_section = HashMap::new();
            orphan_section_order
                .try_reserve(admission.task_capacity)
                .map_err(|_| layout_allocation_error())?;
            tasks_by_section
                .try_reserve(admission.task_capacity)
                .map_err(|_| layout_allocation_error())?;
            Some(Self {
                orphan_section_order,
                tasks_by_section,
            })
        } else {
            None
        };

        let mut declared_sections = HashMap::new();
        if admission.grouped {
            resources.checkpoint()?;
            declared_sections
                .try_reserve(admission.section_lookup_capacity)
                .map_err(|_| layout_allocation_error())?;
            for (section_index, section) in metadata.sections.iter().enumerate() {
                resources.checkpoint()?;
                match declared_sections.entry(section.as_str()) {
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() = DeclaredSectionMatch::Ambiguous;
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(DeclaredSectionMatch::Unique(section_index));
                    }
                }
            }
        }

        for (task_index, task) in model.tasks().enumerate() {
            let semantic = task.semantic;
            resources.checkpoint()?;
            if !task_ids.insert(semantic.id.as_str()) {
                return Err(crate::error::AsciiError::UnsupportedFeature {
                    diagram_type: "gantt",
                    feature: "duplicate task ids",
                });
            }
            let Some(index) = grouped_index.as_mut() else {
                if semantic.section_index.is_some() {
                    return Err(invalid_task_section_occurrence());
                }
                continue;
            };
            let section_key = match semantic.section_index {
                Some(section_index)
                    if metadata.sections.get(section_index).map(String::as_str)
                        == Some(semantic.section.as_str()) =>
                {
                    GanttSectionKey::Declared(section_index)
                }
                Some(_) => return Err(invalid_task_section_occurrence()),
                None => match declared_sections.get(semantic.section.as_str()) {
                    Some(DeclaredSectionMatch::Unique(section_index)) => {
                        GanttSectionKey::Declared(*section_index)
                    }
                    Some(DeclaredSectionMatch::Ambiguous) => {
                        return Err(ambiguous_task_section_occurrence());
                    }
                    None => GanttSectionKey::Orphan(semantic.section.as_str()),
                },
            };
            let tasks = match index.tasks_by_section.entry(section_key) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    if let GanttSectionKey::Orphan(section) = section_key {
                        index.orphan_section_order.push(section);
                    }
                    entry.insert(Vec::new())
                }
            };
            resources.checkpoint()?;
            tasks
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            tasks.push(task_index);
        }

        resources.checkpoint()?;
        Ok(grouped_index)
    }
}

fn invalid_task_section_occurrence() -> crate::error::AsciiError {
    crate::error::AsciiError::UnsupportedFeature {
        diagram_type: "gantt",
        feature: "invalid task section occurrence",
    }
}

fn ambiguous_task_section_occurrence() -> crate::error::AsciiError {
    crate::error::AsciiError::UnsupportedFeature {
        diagram_type: "gantt",
        feature: "ambiguous task section occurrence",
    }
}

fn admit_then_materialize_gantt_structure<T>(
    model: GanttTerminalView<'_>,
    resources: &ResourceContext,
    materialize: impl FnOnce(GanttStructureAdmission, &ResourceContext) -> Result<T>,
) -> Result<T> {
    resources.transaction(|resources| {
        let admission = GanttStructureAdmission::preflight(model, resources)?;
        materialize(admission, resources)
    })
}

pub(crate) fn format_date(ms: i64, local_time_zone: &merman_model::time::LocalTimeZone) -> String {
    local_time_zone
        .at_instant(ms)
        .map(|local| {
            format_resolved_date(
                ms,
                local,
                local_time_zone.resolve_local(local.local_datetime()),
            )
        })
        .unwrap_or_else(|| ms.to_string())
}

fn format_resolved_date(
    ms: i64,
    local: merman_model::time::OffsetDateTime,
    compatible: Option<merman_model::time::OffsetDateTime>,
) -> String {
    let datetime = local.local_datetime();
    let mut formatted = if datetime.hour() == 0
        && datetime.minute() == 0
        && datetime.second() == 0
        && datetime.millisecond() == 0
    {
        local.date().to_string()
    } else {
        datetime.to_string()
    };
    if compatible.is_some_and(|compatible| compatible.timestamp_millis() != ms) {
        formatted.push_str(&local.offset().to_string());
    }
    formatted
}

fn layout_allocation_error() -> crate::error::AsciiError {
    crate::error::AsciiError::AllocationFailed {
        phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AsciiError;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::families::gantt::{GanttDiagramRenderModel, GanttRenderTask};
    use merman_model::resources::ResourceProfile;
    use merman_model::time::{OffsetDateTime, UtcOffset};
    use merman_model::{CancelReason, OperationControl, OperationPhase};
    use std::cell::Cell;

    fn direct_model() -> GanttDiagramRenderModel {
        let mut model = GanttDiagramRenderModel::default();
        model.sections = vec!["Build".to_string(), "Ship".to_string()];
        model.tasks = vec![
            GanttRenderTask {
                semantic: GanttTask {
                    id: "a".to_string(),
                    section: "Build".to_string(),
                    ..GanttTask::default()
                },
                ..GanttRenderTask::default()
            },
            GanttRenderTask {
                semantic: GanttTask {
                    id: "b".to_string(),
                    section: "Orphan".to_string(),
                    ..GanttTask::default()
                },
                ..GanttRenderTask::default()
            },
        ];
        model
    }

    #[test]
    fn canonical_gantt_tasks_match_full_terminal_dates_sections_and_constraints() {
        use merman_core::presentation::SourcePresentationConfig;
        use merman_core::{MermaidEngine, ParseOptions, SemanticParser};
        use merman_model::presentation::RenderSemanticModel;
        use merman_model::{MermaidConfig, SemanticDiagramModel};

        let runtime = merman_model::runtime::RuntimePolicy::deterministic()
            .with_local_time_zone(merman_model::time::LocalTimeZone::utc());
        let config = serde_json::json!({});
        let parser = SemanticParser::new()
            .with_runtime_policy(runtime.clone())
            .with_site_config(MermaidConfig::from_value(config.clone()));
        let engine = MermaidEngine::new()
            .with_runtime_policy(runtime)
            .with_site_config(SourcePresentationConfig::from_value(config));
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        for source in [
            "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection Delivery\nTask: task,2019-02-01,1d\nsection Delivery\nNext: next,after task,2d\n",
            "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nincludes 2019-02-02\nsection Delivery\nTask: task,2019-02-01,1d\nFixed: fixed,2019-02-02,2019-02-04\n",
            "gantt\nsection Empty\nsection Empty\n",
            "---\nconfig:\n  gantt:\n    displayMode: compact\n---\ngantt\ndateFormat YYYY-MM-DD\nTask: a,2026-01-01,1d\n",
            "gantt\ndateFormat YYYY-MM-DD\nTask: a,2026-01-01,1d\nNext: b,after a,1d\nLast: c,after b,1d\nclick c href \"https://example.com/c\"\nclick a href \"https://example.com/a\"\nclick b href \"https://example.com/b\"\n",
        ] {
            let canonical = parser
                .parse_diagram_sync(source)
                .expect("canonical Gantt parse");
            let SemanticDiagramModel::Gantt(model) = &canonical.model else {
                panic!("expected canonical Gantt");
            };
            let full = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .expect("full Gantt parse")
                .expect("detected Gantt");
            let RenderSemanticModel::Gantt(full) = full.model() else {
                panic!("expected full Gantt");
            };
            for (canonical, full) in model.tasks.iter().zip(&full.tasks) {
                assert_eq!(canonical.render_end_ms, full.render_end_ms);
                assert_eq!(canonical.render_end_ms, full.semantic.render_end_ms);
            }
            let actual = render_gantt_diagram(
                GanttTerminalView::from_model(model, &canonical.metadata.semantic_config),
                &options,
                &merman_model::time::LocalTimeZone::utc(),
                AsciiSourceContext::from(&canonical.metadata),
                AsciiExecution::for_test(&policy),
            )
            .expect("canonical Gantt terminal output");
            let expected = render_gantt_diagram(
                GanttTerminalView::from_render(full),
                &options,
                &merman_model::time::LocalTimeZone::utc(),
                AsciiSourceContext::from(&canonical.metadata),
                AsciiExecution::for_test(&policy),
            )
            .expect("full Gantt terminal output");
            assert_eq!(actual, expected, "{source}");
        }
    }

    #[test]
    fn repeated_local_time_discloses_the_later_absolute_instant() {
        let daylight_offset = UtcOffset::from_minutes(-4 * 60).expect("valid EDT offset");
        let standard_offset = UtcOffset::from_minutes(-5 * 60).expect("valid EST offset");
        let earlier_ms = 1_793_511_000_000;
        let later_ms = 1_793_514_600_000;
        let earlier = OffsetDateTime::from_unix_millis(earlier_ms, daylight_offset);
        let later = OffsetDateTime::from_unix_millis(later_ms, standard_offset);

        assert_eq!(earlier.local_datetime(), later.local_datetime());
        assert_eq!(
            format_resolved_date(earlier_ms, earlier, Some(earlier)),
            "2026-11-01T01:30:00.000"
        );
        assert_eq!(
            format_resolved_date(later_ms, later, Some(earlier)),
            "2026-11-01T01:30:00.000-05:00"
        );
    }

    #[test]
    fn timeline_and_records_keep_exact_resource_boundaries_and_mid_render_cancellation() {
        use merman_core::SemanticParser;
        use merman_model::SemanticDiagramModel;

        let parsed = SemanticParser::new()
            .parse_diagram_sync("gantt\ndateFormat YYYY-MM-DD\nsection Build\n设计 👩‍💻: a,2026-01-01,1d\nShip: b,after a,1d\n")
            .unwrap();
        let SemanticDiagramModel::Gantt(model) = &parsed.model else {
            panic!("Gantt model")
        };
        let policy = AsciiResourcePolicy::unbounded();
        let ledger = ResourceContext::new(policy);
        let render = |execution: AsciiExecution<'_>| {
            let render_ledger = execution.new_resource_context(OperationPhase::Semantic);
            render_gantt_diagram(
                GanttTerminalView::from_model(model, &parsed.metadata.semantic_config),
                &AsciiRenderOptions::unicode(),
                &merman_model::time::LocalTimeZone::utc(),
                AsciiSourceContext::from(&parsed.metadata),
                execution.with_render_ledger(&render_ledger),
            )
        };
        let text = render(AsciiExecution::for_test(&policy).with_render_ledger(&ledger)).unwrap();
        for (limit, exact) in [
            (
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                ledger.layout_work_used(),
            ),
            (
                AsciiResourceLimitId::MaxDocumentCells,
                text.lines()
                    .map(|line| {
                        crate::text::display_width_with_profile(
                            line,
                            crate::TerminalWidthProfile::Unicode,
                        )
                    })
                    .sum(),
            ),
            (AsciiResourceLimitId::MaxGridCells, 80 * 3),
            (AsciiResourceLimitId::MaxGraphemeBytes, "👩‍💻".len()),
        ] {
            let exact_policy = policy.with_limit(limit, exact).unwrap();
            render(AsciiExecution::for_test(&exact_policy)).unwrap();
            let short = policy.with_limit(limit, exact - 1).unwrap();
            let result = render(AsciiExecution::for_test(&short));
            assert!(
                matches!(&result, Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit),
                "{limit:?} at {exact}-1: {result:?}"
            );
        }
        for after in [20, 80, 500] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(after);
            assert!(matches!(
                render(AsciiExecution::new(&control, &policy)),
                Err(AsciiError::Cancelled(_))
            ));
        }
    }

    #[test]
    fn gantt_admission_accepts_exact_work_before_materializing_index() {
        const STRUCTURE_WORK: usize = 40;
        let model = direct_model();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, STRUCTURE_WORK)
            .expect("exact Gantt admission work limit should be valid");
        let resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);
        let index = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
            },
        )
        .expect("exact Gantt admission work should permit materialization");

        assert!(materialized.get());
        let index = index.expect("declared sections require a grouped task index");
        assert_eq!(index.tasks_by_section.len(), 2);
        assert_eq!(resources.layout_work_used(), STRUCTURE_WORK);
    }

    #[test]
    fn gantt_admission_rejects_max_minus_one_before_materializing_index() {
        const STRUCTURE_WORK: usize = 40;
        let model = direct_model();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, STRUCTURE_WORK - 1)
            .expect("max-minus-one Gantt admission work limit should be valid");
        let resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);
        let error = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
                .map(|_| ())
            },
        )
        .expect_err("max-minus-one Gantt admission work should fail before materialization");

        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == STRUCTURE_WORK
                    && details.max == STRUCTURE_WORK - 1
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn gantt_index_failure_rolls_back_structure_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let mut model = direct_model();
        let duplicate_id = model.tasks[0].id.clone();
        model.tasks[1].id = duplicate_id;
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");

        let error = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
                .map(|_| ())
            },
        )
        .expect_err("duplicate ids should reject index materialization");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "gantt",
                feature: "duplicate task ids",
            }
        );
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }

    #[test]
    fn gantt_rejects_explicit_section_occurrence_without_declared_sections() {
        let mut model = GanttDiagramRenderModel::default();
        model.tasks.push(GanttRenderTask {
            semantic: GanttTask {
                id: "task".to_string(),
                section: "missing".to_string(),
                section_index: Some(0),
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        });
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let materialized = Cell::new(false);

        let error = match admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
            },
        ) {
            Ok(_) => panic!("an explicit occurrence requires a declared section"),
            Err(error) => error,
        };

        assert_eq!(error, invalid_task_section_occurrence());
        assert!(!materialized.get());
    }

    #[test]
    fn gantt_index_cancellation_rolls_back_admitted_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let model = direct_model();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");
        let control = OperationControl::new();
        let controlled = resources.controlled(control.clone(), OperationPhase::Layout);

        let error = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &controlled,
            |admission, resources| {
                control.cancel();
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
                .map(|_| ())
            },
        )
        .expect_err("materialization should observe cancellation at its first checkpoint");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }
}
