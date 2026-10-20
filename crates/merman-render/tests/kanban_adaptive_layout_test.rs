#![cfg(feature = "diagram-kanban")]

use merman_core::{Engine, OperationControl, ParseOptions, ParsedDiagramRender};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::KanbanDiagramLayout;
use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle};
use merman_render::{LayoutAdaptation, LayoutAdaptationObservation, LayoutOptions};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = "---\nconfig:\n  mindmap:\n    padding: 8\n  kanban:\n    ticketBaseUrl: https://example.invalid/ticket/#TICKET#\n---\nkanban\n  todo[待办]\n    a[\"**第一项**<br/>第二行\"]@{ticket: T-1, assigned: Alice}\n    aa[Another]\n  doing[Doing]\n    b[Second]\n  done[Done]\n    c[Third]";

fn parse(source: &str) -> ParsedDiagramRender {
    Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
}

fn render(
    parsed: &ParsedDiagramRender,
    options: &LayoutOptions,
    environment: RenderEnvironment,
) -> (
    KanbanDiagramLayout,
    Option<LayoutAdaptationObservation>,
    String,
    Value,
) {
    let artifact = family::prepare(parsed, options, environment.begin_session().unwrap()).unwrap();
    let observation = artifact.layout_adaptation();
    let projection = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(projection["layout"]["KanbanDiagram"].clone()).unwrap();
    let semantic = projection["semantic"].clone();
    let completion = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .into_completion();
    assert_eq!(completion.layout_adaptation(), observation);
    (
        layout,
        observation,
        completion.into_output_and_report().0,
        semantic,
    )
}

fn fit(width: f64) -> LayoutOptions {
    LayoutOptions::default()
        .with_adaptation(LayoutAdaptation::FitWidth)
        .with_container_size(width, 600.0)
}

#[test]
fn narrow_then_wide_preserves_columns_content_links_and_default_svg_bytes() {
    let parsed = parse(SOURCE);
    let (natural, none, svg, semantics) = render(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic(),
    );
    assert!(none.is_none());
    let (_, explicit_none, explicit_svg, _) = render(
        &parsed,
        &LayoutOptions::default().with_container_size(20.0, 600.0),
        RenderEnvironment::deterministic(),
    );
    assert!(explicit_none.is_none());
    assert_eq!(svg, explicit_svg);
    let (narrow, observation, narrow_svg, narrow_semantics) =
        render(&parsed, &fit(421.0), RenderEnvironment::deterministic());
    assert_eq!(semantics, narrow_semantics);
    assert_eq!(narrow.sections.len(), 3);
    assert_eq!(narrow.items.len(), natural.items.len());
    assert_eq!(narrow.sections[0].rect_y, narrow.sections[1].rect_y);
    let row_bottom = narrow.sections[..2]
        .iter()
        .map(|column| column.rect_y + column.rect_height)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(narrow.sections[2].rect_y > row_bottom);
    for (before, after) in natural.items.iter().zip(&narrow.items) {
        assert_eq!(
            (&before.id, &before.label, &before.ticket, &before.assigned),
            (&after.id, &after.label, &after.ticket, &after.assigned)
        );
        assert_eq!((before.width, before.height), (after.width, after.height));
    }
    let observation = observation.unwrap();
    assert_eq!(
        (
            observation.requested_width(),
            observation.actual_width(),
            observation.overflowed()
        ),
        (421.0, 421.0, false)
    );
    let before = roxmltree::Document::parse(&svg).unwrap();
    let after = roxmltree::Document::parse(&narrow_svg).unwrap();
    let links = |document: &roxmltree::Document<'_>| {
        document
            .descendants()
            .filter(|node| node.has_tag_name("a"))
            .map(|node| node.attribute("href").unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
    };
    assert!(!links(&before).is_empty());
    assert_eq!(links(&before), links(&after));
    let clusters = after
        .descendants()
        .filter(|node| {
            node.attribute("class")
                .is_some_and(|class| class.starts_with("cluster undefined section-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(clusters.len(), narrow.sections.len());
    for (cluster, column) in clusters.into_iter().zip(&narrow.sections) {
        let rect = cluster
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(
            rect.attribute("y").unwrap().parse::<f64>().unwrap(),
            column.rect_y
        );
        let label = cluster
            .children()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .unwrap();
        let coordinates = label
            .attribute("transform")
            .unwrap()
            .trim_start_matches("translate(")
            .trim_end_matches(')')
            .split(',')
            .map(|value| value.trim().parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(coordinates[1], column.rect_y);
    }
    assert_eq!(
        before
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .map(|n| n.text())
            .collect::<Vec<_>>(),
        after
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .map(|n| n.text())
            .collect::<Vec<_>>()
    );
    let (wide, _, wide_svg, wide_semantics) =
        render(&parsed, &fit(800.0), RenderEnvironment::deterministic());
    assert_eq!(wide_svg, svg);
    assert_eq!(wide_semantics, semantics);
    assert_eq!(
        serde_json::to_value(wide).unwrap(),
        serde_json::to_value(natural).unwrap()
    );
}

#[test]
fn exact_thresholds_follow_effective_padding_and_report_single_column_overflow() {
    for (source, single, double) in [
        (SOURCE, 216.0, 421.0),
        (
            "kanban\n  one[One]\n    a[A]\n  two[Two]\n    b[B]",
            220.0,
            425.0,
        ),
    ] {
        let parsed = parse(source);
        for (width, same_row, overflow) in [
            (double, true, false),
            (double - 1.0, false, false),
            (single, false, false),
            (single - 1.0, false, true),
        ] {
            let (layout, observation, _, _) =
                render(&parsed, &fit(width), RenderEnvironment::deterministic());
            assert_eq!(
                layout.sections[0].rect_y == layout.sections[1].rect_y,
                same_row
            );
            let observation = observation.unwrap();
            assert_eq!(observation.overflowed(), overflow);
            assert_eq!(
                observation.actual_width(),
                if same_row { double } else { single }
            );
        }
    }
}

#[test]
fn empty_single_and_duplicate_id_columns_keep_occurrence_ownership() {
    for source in [
        "kanban",
        "kanban\n  same[Only]",
        "kanban\n  same[First]\n    task[A]\n  same[Second]\n    task[B]\n  third[Third]\n    task[C]",
    ] {
        let parsed = parse(source);
        let (layout, observation, svg, _) =
            render(&parsed, &fit(220.0), RenderEnvironment::deterministic());
        assert_eq!(
            observation.unwrap().actual_width(),
            if layout.sections.is_empty() {
                100.0
            } else {
                220.0
            }
        );
        roxmltree::Document::parse(&svg).unwrap();
        if layout.sections.len() == 3 {
            let (natural, _, _, semantics) = render(
                &parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic(),
            );
            let (_, _, _, adapted_semantics) =
                render(&parsed, &fit(220.0), RenderEnvironment::deterministic());
            assert_eq!(semantics, adapted_semantics);
            assert_eq!(
                natural
                    .items
                    .iter()
                    .map(|item| &item.label)
                    .collect::<Vec<_>>(),
                layout
                    .items
                    .iter()
                    .map(|item| &item.label)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                layout
                    .items
                    .iter()
                    .map(|item| item.label.as_str())
                    .collect::<Vec<_>>(),
                ["A", "B", "A", "B", "C"]
            );
            // Canonical parsing repeats the two same-ID columns' matching tasks; placement
            // must preserve those five occurrences and keep each contiguous column intact.
            for (column, range) in layout.sections.iter().zip([0..2, 2..4, 4..5]) {
                for task in &layout.items[range] {
                    assert_eq!(column.center_x, task.center_x);
                    assert!(task.center_y - task.height / 2.0 >= column.rect_y);
                    assert!(
                        task.center_y + task.height / 2.0 <= column.rect_y + column.rect_height
                    );
                }
            }
            for pair in layout.sections.windows(2) {
                assert!(pair[1].rect_y > pair[0].rect_y + pair[0].rect_height);
            }
        }
    }
}

struct CountingMeasurer(Arc<AtomicUsize>);
impl TextMeasurer for CountingMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.0.fetch_add(1, Ordering::Relaxed);
        DeterministicTextMeasurer::default().measure(text, style)
    }
}

#[test]
fn placement_reuses_prepared_labels_and_checks_width_at_the_public_owner() {
    let parsed = parse(SOURCE);
    let calls = Arc::new(AtomicUsize::new(0));
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::uniform(TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("counting").unwrap(),
                "1",
            )
            .unwrap(),
            Arc::new(CountingMeasurer(calls.clone())),
        )),
    );
    let _ = render(&parsed, &LayoutOptions::default(), environment.clone());
    let baseline = calls.swap(0, Ordering::Relaxed);
    let _ = render(&parsed, &fit(216.0), environment);
    assert!(baseline > 0);
    assert_eq!(calls.load(Ordering::Relaxed), baseline);
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            family::prepare(
                &parsed,
                &fit(width),
                RenderEnvironment::deterministic().begin_session().unwrap()
            ),
            Err(merman_render::Error::InvalidAdaptationWidth)
        ));
    }
}

#[test]
fn adapted_width_observation_rejects_nonfinite_bounds_without_panicking() {
    let parsed =
        parse("---\nconfig:\n  mindmap:\n    padding: 1.7976931348623157e308\n---\nkanban\n  one");
    assert!(matches!(
        family::prepare(
            &parsed,
            &fit(800.0),
            RenderEnvironment::deterministic().begin_session().unwrap()
        ),
        Err(merman_render::Error::InvalidModel { .. })
    ));
}

#[test]
fn adapted_placement_uses_the_existing_operation_work_and_cancellation() {
    let parsed = parse(SOURCE);
    let environment = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input());
    let artifact =
        family::prepare(&parsed, &fit(216.0), environment.begin_session().unwrap()).unwrap();
    let completion = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap()
        .into_completion();
    let used = completion.report().session_report().layout_work_units();
    let limited = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, used - 1)
        .unwrap();
    assert!(matches!(
        family::prepare(
            &parsed,
            &fit(216.0),
            RenderEnvironment::deterministic()
                .with_resource_policy(limited)
                .begin_session()
                .unwrap()
        ),
        Err(merman_render::Error::ResourceLimitExceeded(_))
    ));
    let control = OperationControl::new();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_control(control.clone())
        .unwrap();
    control.cancel();
    assert!(matches!(
        family::prepare(&parsed, &fit(216.0), session),
        Err(merman_render::Error::Cancelled(_))
    ));
}
