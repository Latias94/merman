pub(crate) use merman_render_structured::timeline::{
    TimelineStyle, prepare_timeline_diagram, render_timeline_svg,
};

#[cfg(test)]
mod tests {
    use crate::environment::RenderEnvironment;
    use crate::model::TimelineDiagramLayout;
    use crate::{LayoutOptions, family};
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::families::timeline::TimelineDirection;
    use std::path::PathBuf;

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    fn layout_timeline(source: &str) -> TimelineDiagramLayout {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .expect("parse ok")
            .expect("diagram detected")
            .into_presentation();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let artifact =
            family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
        let projection = artifact.layout_json().expect("serialize Timeline layout");
        serde_json::from_value(projection["layout"]["TimelineDiagram"].clone())
            .expect("Timeline layout projection")
    }

    fn section_task_labels(layout: &TimelineDiagramLayout) -> Vec<Vec<&str>> {
        layout
            .sections
            .iter()
            .map(|section| {
                section
                    .tasks
                    .iter()
                    .map(|task| task.node.label.as_str())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn repeated_section_labels_reuse_matching_tasks_in_both_directions() {
        for direction in ["LR", "TD"] {
            for second_section in ["S", "T"] {
                let source = format!(
                    "timeline {direction}\nsection S\nA : X\nsection {second_section}\nB : Y\n"
                );
                let layout = layout_timeline(&source);
                let repeated = second_section == "S";
                let expected = if repeated {
                    vec![vec!["A ", "B "], vec!["A ", "B "]]
                } else {
                    vec![vec!["A "], vec!["B "]]
                };
                assert_eq!(section_task_labels(&layout), expected);
                if direction == "LR" {
                    let width = if repeated { 390.0 } else { 190.0 };
                    assert_eq!(layout.sections[0].node.width, width);
                    assert_eq!(layout.sections[1].node.width, width);
                    assert_eq!(
                        layout.sections[1].node.x - layout.sections[0].node.x,
                        if repeated { 400.0 } else { 200.0 }
                    );
                }
            }
        }
    }

    #[test]
    fn long_word_wrap_keeps_the_activity_line_aligned_with_content_bounds() {
        let path = workspace_root()
            .join("fixtures")
            .join("timeline")
            .join("upstream_long_word_wrap.mmd");
        let text = std::fs::read_to_string(&path).expect("fixture");

        let layout = layout_timeline(&text);

        let actual = layout.activity_line.x2;
        assert_eq!(
            actual,
            layout.pre_title_box_width + 3.0 * layout.left_margin
        );
        assert!(actual > layout.activity_line.x1);
        assert!(layout.bounds.expect("bounds").max_x >= actual);
    }

    #[test]
    fn empty_timeline_does_not_invent_pre_title_width() {
        let layout = layout_timeline("timeline");

        assert_eq!(layout.pre_title_box_width, 0.0);
        assert_eq!(layout.activity_line.x1, 150.0);
        assert_eq!(layout.activity_line.x2, 450.0);
        let bounds = layout.bounds.expect("bounds");
        assert_eq!(bounds.min_x, 100.0);
        assert_eq!(bounds.min_y, 50.0);
        assert_eq!(bounds.max_x, 500.0);
        assert_eq!(bounds.max_y, 150.0);
    }

    #[test]
    fn top_down_timeline_uses_vertical_axis_and_one_connector_per_event() {
        let layout = layout_timeline(concat!(
            "timeline TD\n",
            "section Delivery\n",
            "Plan : Event A : Event B\n",
            "Ship\n",
        ));

        assert_eq!(layout.direction, TimelineDirection::TopDown);
        assert_eq!(layout.activity_line.x1, layout.activity_line.x2);
        assert!(layout.activity_line.y2 > layout.activity_line.y1);
        let section = &layout.sections[0];
        assert_eq!(section.tasks.len(), 2);
        assert_eq!(section.tasks[0].connectors.len(), 2);
        assert!(section.tasks[1].connectors.is_empty());
        assert!(section.tasks[0].node.x < layout.activity_line.x1);
        assert!(
            section.tasks[0]
                .events
                .iter()
                .all(|event| event.x > layout.activity_line.x1)
        );
        assert!(section.tasks[1].node.y > section.tasks[0].node.y);
        for connector in &section.tasks[0].connectors {
            assert_eq!(connector.y1, connector.y2);
            assert_eq!(connector.x1, layout.activity_line.x1);
            assert!(connector.x2 > connector.x1);
        }
    }
}
