pub(crate) const SEQUENCE_MESSAGE_WRAP_PADDING_SIDES: f64 = 2.0;
pub(crate) const SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX: f64 = 60.0;
pub(crate) const SEQUENCE_FRAME_SIDE_PAD_PX: f64 = 11.0;
pub(crate) const SEQUENCE_FRAME_GEOM_PAD_PX: f64 = 10.0;
pub(crate) const SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT: f64 = 20.0;
pub(crate) const SEQUENCE_ACTOR_POPUP_ROW_HEIGHT: f64 = 30.0;

pub(crate) fn sequence_text_dimensions_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * (17.0 / 16.0)).round().max(1.0)
}

pub(crate) fn sequence_text_line_step_px(font_size_px: f64) -> f64 {
    font_size_px.max(1.0) * 1.1875
}

pub(crate) fn sequence_actor_popup_panel_height(link_count: usize) -> f64 {
    SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT + (link_count as f64) * SEQUENCE_ACTOR_POPUP_ROW_HEIGHT
}

pub(super) fn sequence_actor_visual_height(
    actor_type: &str,
    base_width: f64,
    base_height: f64,
    label_box_height: f64,
    has_prepared_math: bool,
) -> f64 {
    let legacy_height = match actor_type {
        // Mermaid derives these from the actor-type glyph bbox + label box height.
        // These heights are used by the footer actor rendering and affect the final SVG viewBox.
        "boundary" => (44.0 + label_box_height).max(1.0),
        // Mermaid's database actor updates the actor height from the cylinder bbox after render.
        // The cylinder uses `rect.width / 3`, then the label box height is added.
        "database" => ((base_width / 3.0) + label_box_height).max(1.0),
        "entity" => (44.0 + label_box_height).max(1.0),
        // Control uses an extra label-box height in Mermaid.
        "control" => (44.0 + 2.0 * label_box_height).max(1.0),
        _ => base_height.max(1.0),
    };
    if has_prepared_math {
        legacy_height.max(base_height).max(1.0)
    } else {
        legacy_height
    }
}

pub(super) fn sequence_actor_prepared_math_layout_height(
    actor_type: &str,
    base_width: f64,
    actor_height: f64,
    label_box_height: f64,
    prepared_height: f64,
) -> f64 {
    let legacy_height = sequence_actor_visual_height(
        actor_type,
        base_width,
        actor_height,
        label_box_height,
        false,
    );
    let half_prepared_height = prepared_height.max(1.0) / 2.0;
    let label_center_from_top = match actor_type {
        "actor" => actor_height + 2.5,
        "boundary" => {
            let top = 21.0 + 15.0 + actor_height / 2.0;
            let footer_height = 44.0 + label_box_height;
            let bottom = 21.0 + 15.0 + footer_height / 2.0;
            top.max(bottom)
        }
        "control" => {
            let top = 22.0 + 12.0 + actor_height / 2.0;
            let footer_height = 44.0 + 2.0 * label_box_height;
            let bottom = 22.0 + 5.0 + footer_height / 2.0;
            top.max(bottom)
        }
        "entity" => {
            let top = 6.0 + 30.0 + actor_height / 2.0;
            let footer_height = 44.0 + label_box_height;
            let bottom = 22.0 + 15.0 + footer_height / 2.0;
            top.max(bottom)
        }
        "database" => {
            let top = 35.0 + actor_height / 2.0;
            let footer_height = base_width / 3.0 + label_box_height;
            let bottom = 35.0 + footer_height / 2.0;
            top.max(bottom)
        }
        _ => return legacy_height.max(prepared_height).max(1.0),
    };
    legacy_height
        .max(label_center_from_top + half_prepared_height)
        .max(1.0)
}

pub(super) fn sequence_actor_lifeline_start_y(
    actor_type: &str,
    base_height: f64,
    box_text_margin: f64,
) -> f64 {
    match actor_type {
        // Hard-coded in Mermaid's sequence svgDraw.js for these actor types.
        // Prepared math may make the terminal actor footprint taller than the legacy glyph. Keep
        // the lifeline below the complete renderer-owned label projection in that case.
        "actor" | "boundary" => 80.0_f64.max(base_height),
        "control" | "entity" => 75.0_f64.max(base_height),
        // For database, Mermaid starts the lifeline slightly below the actor box.
        "database" => base_height + 2.0 * box_text_margin,
        _ => base_height,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sequence_text_and_frame_constants_match_mermaid() {
        assert_eq!(super::SEQUENCE_MESSAGE_WRAP_PADDING_SIDES, 2.0);
        assert_eq!(super::SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT, 20.0);
        assert_eq!(super::SEQUENCE_ACTOR_POPUP_ROW_HEIGHT, 30.0);
        assert_eq!(super::sequence_actor_popup_panel_height(0), 20.0);
        assert_eq!(super::sequence_actor_popup_panel_height(4), 140.0);
        assert_eq!(super::sequence_text_dimensions_height_px(16.0), 17.0);
        assert_eq!(super::sequence_text_dimensions_height_px(10.0), 11.0);
        assert_eq!(super::sequence_text_line_step_px(16.0), 19.0);
        assert_eq!(
            super::sequence_actor_visual_height("database", 150.0, 65.0, 20.0, false),
            70.0
        );
        assert_eq!(
            super::sequence_actor_visual_height("boundary", 150.0, 65.0, 20.0, false),
            64.0
        );
        assert_eq!(
            super::sequence_actor_visual_height("entity", 150.0, 65.0, 20.0, false),
            64.0
        );
        assert_eq!(
            super::sequence_actor_visual_height("control", 150.0, 65.0, 20.0, false),
            84.0
        );
        assert_eq!(super::SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX, 60.0);
        assert_eq!(super::SEQUENCE_FRAME_SIDE_PAD_PX, 11.0);
        assert_eq!(super::SEQUENCE_FRAME_GEOM_PAD_PX, 10.0);
    }

    #[test]
    fn prepared_actor_man_layout_contains_the_complete_label_projection() {
        let prepared_height = 80.0;
        for (actor_type, expected_minimum) in [
            ("actor", 107.5),
            ("boundary", 108.5),
            ("control", 109.0),
            ("entity", 109.0),
            ("database", 110.0),
        ] {
            let layout_height = super::sequence_actor_prepared_math_layout_height(
                actor_type,
                150.0,
                65.0,
                20.0,
                prepared_height,
            );
            assert_eq!(layout_height, expected_minimum, "{actor_type}");
            assert_eq!(
                super::sequence_actor_visual_height(actor_type, 150.0, layout_height, 20.0, true,),
                expected_minimum,
                "{actor_type}"
            );
            assert!(
                super::sequence_actor_lifeline_start_y(actor_type, layout_height, 5.0)
                    >= layout_height,
                "{actor_type} lifeline must start below the complete prepared label footprint"
            );
        }
    }
}
