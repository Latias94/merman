pub(crate) const SEQUENCE_MESSAGE_WRAP_PADDING_SIDES: f64 = 2.0;
pub(crate) const SEQUENCE_SELF_MESSAGE_FRAME_EXTRA_Y_PX: f64 = 60.0;
pub(crate) const SEQUENCE_FRAME_SIDE_PAD_PX: f64 = 11.0;
pub(crate) const SEQUENCE_FRAME_GEOM_PAD_PX: f64 = 10.0;
pub(crate) const SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT: f64 = 20.0;
pub(crate) const SEQUENCE_ACTOR_POPUP_ROW_HEIGHT: f64 = 30.0;

pub(crate) fn sequence_text_dimensions_height_px(font_size_px: f64) -> f64 {
    (font_size_px.max(1.0) * (17.0 / 16.0)).round().max(1.0)
}

pub(crate) fn sequence_actor_popup_rect_height(
    actor_type: &str,
    visual_height: f64,
    base_height: f64,
    is_neo: bool,
    mirror_actors: bool,
) -> f64 {
    // drawPopup inherits the last drawActor call's rectData. Top drawing keeps the pre-shape
    // actor height for collection, queue, and database shapes; mirrored drawing runs after shape mutation and uses
    // the footer's visual node height. Neo collections shorten both rectData copies by six pixels.
    let height = if mirror_actors || !matches!(actor_type, "collections" | "queue" | "database") {
        visual_height
    } else {
        base_height
    };
    if actor_type == "collections" && is_neo {
        (height - 6.0).max(0.0)
    } else {
        height
    }
}

pub(super) fn sequence_actor_visual_height(
    actor_type: &str,
    base_width: f64,
    base_height: f64,
    label_box_height: f64,
    label_footprint: Option<&SequenceActorLabelFootprint>,
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
    if label_footprint.is_some() {
        legacy_height.max(base_height).max(1.0)
    } else {
        legacy_height
    }
}

/// The measurement owner remains distinct even when its resulting geometry is shared.
#[derive(Debug, Clone, Copy)]
pub(super) enum SequenceActorLabelKind {
    TypedText,
    PreparedMath,
}

/// Classic actor label geometry, relative to the participant's top, shared by layout and SVG.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SequenceActorLabelFootprint {
    pub(super) layout_height: f64,
    glyph_label_centers: Option<(f64, f64)>,
}

impl SequenceActorLabelFootprint {
    pub(super) fn resolve(
        kind: SequenceActorLabelKind,
        actor_type: &str,
        base_width: f64,
        actor_height: f64,
        label_box_height: f64,
        text_height: f64,
    ) -> Self {
        let legacy_height = sequence_actor_visual_height(
            actor_type,
            base_width,
            actor_height,
            label_box_height,
            None,
        );
        let half_height = text_height.max(1.0) / 2.0;
        // These are the existing classic writer's translated top/footer label centers and
        // glyph bottoms. Keep math's legacy centers; typed text needs a separate label band.
        let glyph = match actor_type {
            "actor" => {
                let center = match kind {
                    SequenceActorLabelKind::PreparedMath => actor_height + 2.5,
                    SequenceActorLabelKind::TypedText => actor_height / 2.0 + 35.0,
                };
                Some((center, center, 60.0, 60.0))
            }
            "boundary" => Some((
                21.0 + 15.0 + actor_height / 2.0,
                21.0 + 15.0 + (44.0 + label_box_height) / 2.0,
                55.0,
                55.0,
            )),
            "control" => Some((
                34.0 + actor_height / 2.0,
                27.0 + (44.0 + 2.0 * label_box_height) / 2.0,
                54.0,
                54.0,
            )),
            "entity" => Some((
                6.0 + 30.0 + actor_height / 2.0,
                22.0 + 15.0 + (44.0 + label_box_height) / 2.0,
                53.0,
                54.0,
            )),
            "database" => {
                let cylinder_height = base_width / 3.0;
                let ry = cylinder_height / 2.0 / (2.5 + cylinder_height / 50.0);
                let glyph_bottom = cylinder_height + ry;
                Some((
                    35.0 + actor_height / 2.0,
                    35.0 + (cylinder_height + label_box_height) / 2.0,
                    glyph_bottom,
                    glyph_bottom,
                ))
            }
            _ => None,
        };
        let mut layout_height =
            if glyph.is_some() && matches!(kind, SequenceActorLabelKind::PreparedMath) {
                legacy_height
            } else {
                legacy_height.max(text_height).max(1.0)
            };
        let glyph_label_centers = glyph.map(|(top, footer, top_bottom, footer_bottom)| {
            let (top, footer) = match kind {
                SequenceActorLabelKind::PreparedMath => (top, footer),
                SequenceActorLabelKind::TypedText => (
                    top.max(top_bottom + SEQUENCE_ACTOR_LABEL_GAP + half_height),
                    footer.max(footer_bottom + SEQUENCE_ACTOR_LABEL_GAP + half_height),
                ),
            };
            layout_height = layout_height.max(top.max(footer) + half_height);
            (top, footer)
        });
        if actor_type == "collections" && matches!(kind, SequenceActorLabelKind::TypedText) {
            // The classic collection label is translated 6px down from its rectangle center.
            layout_height = layout_height.max(text_height + 2.0 * SEQUENCE_ACTOR_LABEL_GAP);
        }
        if matches!(kind, SequenceActorLabelKind::TypedText) {
            // Reserve a whole output pixel so serialized label coordinates cannot round outside
            // their own row. Math keeps its existing exact projection dimensions.
            layout_height = layout_height.ceil();
        }
        Self {
            layout_height,
            // Prepared math retains its existing writer placement and browser/native projection.
            glyph_label_centers: matches!(kind, SequenceActorLabelKind::TypedText)
                .then_some(glyph_label_centers)
                .flatten(),
        }
    }

    pub(crate) fn label_center_y(&self, footer: bool) -> Option<f64> {
        self.glyph_label_centers
            .map(|(top, bottom)| if footer { bottom } else { top })
    }
}

pub(super) fn sequence_actor_lifeline_start_y(
    actor_type: &str,
    base_height: f64,
    box_text_margin: f64,
) -> f64 {
    match actor_type {
        // Hard-coded in Mermaid's sequence svgDraw.js for these actor types.
        // Prepared math and typed text may have a taller terminal footprint than the glyph.
        "actor" | "boundary" => 80.0_f64.max(base_height),
        "control" | "entity" => 75.0_f64.max(base_height),
        // For database, Mermaid starts the lifeline slightly below the actor box.
        "database" => base_height + 2.0 * box_text_margin,
        _ => base_height,
    }
}

/// Mermaid's Neo participant row: glyph, gap, measured text, gap to the lifeline.
pub(crate) const SEQUENCE_GLYPH_BAND_HEIGHT: f64 = 44.0;
const SEQUENCE_ACTOR_LABEL_GAP: f64 = 6.0;
pub(crate) fn sequence_actor_stack_height(text_height: f64) -> f64 {
    SEQUENCE_GLYPH_BAND_HEIGHT + SEQUENCE_ACTOR_LABEL_GAP + text_height + 6.0
}

pub(crate) struct SequenceActorBands {
    pub(crate) glyph_bottom_y: f64,
    pub(crate) label_center_y: f64,
}

impl SequenceActorBands {
    pub(crate) fn new(actor_y: f64, row_height: f64, text_height: f64, footer: bool) -> Self {
        if footer {
            let glyph_bottom_y = actor_y + 10.0 + SEQUENCE_GLYPH_BAND_HEIGHT;
            Self {
                glyph_bottom_y,
                label_center_y: glyph_bottom_y + 6.0 + text_height / 2.0,
            }
        } else {
            let datum = actor_y + row_height;
            Self {
                glyph_bottom_y: datum - 6.0 - text_height - 6.0,
                label_center_y: datum - 3.0 - text_height / 2.0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn sequence_text_and_frame_constants_match_mermaid() {
        assert_eq!(super::SEQUENCE_MESSAGE_WRAP_PADDING_SIDES, 2.0);
        assert_eq!(super::SEQUENCE_ACTOR_POPUP_PANEL_BASE_HEIGHT, 20.0);
        assert_eq!(super::SEQUENCE_ACTOR_POPUP_ROW_HEIGHT, 30.0);
        assert_eq!(
            super::sequence_actor_popup_rect_height("queue", 74.0, 65.0, false, false),
            65.0
        );
        assert_eq!(
            super::sequence_actor_popup_rect_height("queue", 74.0, 65.0, false, true),
            74.0
        );
        assert_eq!(
            super::sequence_actor_popup_rect_height("collections", 80.0, 80.0, true, false),
            74.0
        );
        assert_eq!(super::sequence_text_dimensions_height_px(16.0), 17.0);
        assert_eq!(super::sequence_text_dimensions_height_px(10.0), 11.0);
        assert_eq!(crate::text::sequence_text_line_step_px(16.0), 19.0);
        assert_eq!(
            super::sequence_actor_visual_height("database", 150.0, 65.0, 20.0, None),
            70.0
        );
        assert_eq!(
            super::sequence_actor_visual_height("boundary", 150.0, 65.0, 20.0, None),
            64.0
        );
        assert_eq!(
            super::sequence_actor_visual_height("entity", 150.0, 65.0, 20.0, None),
            64.0
        );
        assert_eq!(
            super::sequence_actor_visual_height("control", 150.0, 65.0, 20.0, None),
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
            let footprint = super::SequenceActorLabelFootprint::resolve(
                super::SequenceActorLabelKind::PreparedMath,
                actor_type,
                150.0,
                65.0,
                20.0,
                prepared_height,
            );
            let layout_height = footprint.layout_height;
            assert_eq!(layout_height, expected_minimum, "{actor_type}");
            assert_eq!(
                super::sequence_actor_visual_height(
                    actor_type,
                    150.0,
                    layout_height,
                    20.0,
                    Some(&footprint),
                ),
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

    #[test]
    fn classic_typed_text_footprints_keep_separate_glyph_and_label_bands() {
        for (actor_type, glyph_bottom) in [
            ("actor", 60.0),
            ("boundary", 55.0),
            ("control", 54.0),
            ("entity", 54.0),
            ("database", 50.0 + 25.0 / 3.5),
        ] {
            let text = super::SequenceActorLabelFootprint::resolve(
                super::SequenceActorLabelKind::TypedText,
                actor_type,
                150.0,
                65.0,
                20.0,
                125.0,
            );
            let math = super::SequenceActorLabelFootprint::resolve(
                super::SequenceActorLabelKind::PreparedMath,
                actor_type,
                150.0,
                65.0,
                20.0,
                125.0,
            );
            for footer in [false, true] {
                let center = text.label_center_y(footer).unwrap();
                // Entity's top glyph is one pixel shorter than its footer glyph.
                let bottom = if actor_type == "entity" && !footer {
                    53.0
                } else {
                    glyph_bottom
                };
                assert!(center - 62.5 > bottom, "{actor_type}/footer={footer}");
                assert!(center + 62.5 <= text.layout_height);
                assert!(
                    math.label_center_y(footer).is_none(),
                    "math retains its writer placement"
                );
            }
        }
    }
}
