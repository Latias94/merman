//! Warm paper, botanical bodies, and independent category surfaces and chart inks.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    // Paper swatches are labeled surfaces. Fine chart marks need their own darker pigments.
    let chart_inks = [
        "#556F50", "#3E7183", "#97603F", "#825C7A", "#876E24", "#4E6C69",
    ];
    for (index, color) in chart_inks.iter().enumerate() {
        let id = format!("chart-ink.{index}");
        color_token(
            &mut design,
            &id,
            &format!("Chart ink {}", index + 1),
            "Chart inks",
            "Botanical pigment for fine chart marks, independent of pale category surfaces.",
            color,
        );
        // Sankey labels sit outside their narrow nodes. Preserve its distinct node/link domain.
        let mut entry = rule(
            Some(DiagramFamilyId::SANKEY),
            ThemeTarget::Node,
            None,
            style(Some(&id), None),
        );
        entry.ordinal = Some(ThemeOrdinalSelectorWireV1::Cycle {
            cycle: ThemeOrdinalCycleWireV1 {
                period: chart_inks.len() as u32,
                offset: index as u32,
            },
        });
        design.rules.push(entry);
    }
    token(
        &mut design,
        "chart-inks",
        "Chart inks",
        "Chart inks",
        "Ordered colors for chart strokes and swatches; repeating after the authored six pigments.",
        ThemeDesignTokenValueV1::Palette(literal(
            (0..chart_inks.len())
                .map(|index| reference(&format!("chart-ink.{index}")))
                .collect(),
        )),
    );
    design.palettes.insert(
        ThemeTarget::ChartSeries.id().to_owned(),
        reference("chart-inks"),
    );
    color_token(
        &mut design,
        "paper-texture",
        "Paper texture",
        "Effects",
        "Color of the fine, low-opacity diagonal fibers on the exported canvas.",
        "#A59676",
    );
    token(
        &mut design,
        "body-font",
        "Font family",
        "Typography",
        "Font fallback order; fonts are resolved on the receiving host, not embedded.",
        ThemeDesignTokenValueV1::FontStack(literal(
            ["Open Sans", "Noto Sans SC", "sans-serif"]
                .map(str::to_owned)
                .to_vec(),
        )),
    );
    design
        .canvas
        .as_mut()
        .expect("shared recipe owns a canvas")
        .layers = vec![texture_layer()];
    append_diagram_bodies(&mut design);
    append_xy_chart(&mut design);

    for (id, label, size, target) in [
        (
            "pie-percentage-size",
            "Pie percentage size",
            17.0,
            ThemeTarget::Text,
        ),
        ("pie-title-size", "Pie title size", 25.0, ThemeTarget::Title),
        (
            "pie-legend-size",
            "Pie legend size",
            12.0,
            ThemeTarget::Legend,
        ),
    ] {
        length_token(
            &mut design,
            id,
            label,
            "Typography",
            "Font size of this Pie text role, in pixels.",
            size,
        );
        design.rules.push(rule(
            Some(DiagramFamilyId::PIE),
            target,
            None,
            ThemeDesignStyleV1 {
                typography: Some(literal(ThemeDesignTextStyleV1 {
                    font_size_px: SpecifiedWireV1::Value(reference(id)),
                    ..Default::default()
                })),
                ..Default::default()
            },
        ));
    }
    design
}

fn append_diagram_bodies(design: &mut ThemeDesignV1) {
    for (id, label, description, value) in [
        (
            "body-radius",
            "Body radius",
            "Corner radius for Flowchart nodes and sequence actors and notes.",
            7.0,
        ),
        (
            "body-border-width",
            "Body border width",
            "Outline thickness for Flowchart nodes and sequence actors and notes.",
            1.5,
        ),
        (
            "detail-border-width",
            "Detail border width",
            "Line thickness for Flowchart edges and groups and sequence details.",
            1.1,
        ),
    ] {
        length_token(design, id, label, "Geometry", description, value);
    }
    for (id, label, description, value) in [
        (
            "label-weight",
            "Label weight",
            "Font weight for Flowchart node labels and sequence actor labels.",
            600.0,
        ),
        (
            "secondary-label-weight",
            "Secondary label weight",
            "Font weight for connector, message, loop, and note labels.",
            500.0,
        ),
    ] {
        number_token(design, id, label, description, value);
    }
    for (family, target, rounded) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node, true),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Edge, false),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Cluster, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Message, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Lifeline, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Loop, false),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabelBackground,
            false,
        ),
    ] {
        let mut patch = ThemeDesignStyleV1 {
            stroke: Some(literal(ThemeDesignStrokeV1 {
                width: SpecifiedWireV1::Value(reference(if rounded {
                    "body-border-width"
                } else {
                    "detail-border-width"
                })),
                paint: if family == DiagramFamilyId::SEQUENCE && target == ThemeTarget::Loop {
                    SpecifiedWireV1::Value(paint("border"))
                } else {
                    SpecifiedWireV1::Unspecified
                },
                ..Default::default()
            })),
            ..Default::default()
        };
        if rounded {
            patch.radius = SpecifiedWireV1::Value(reference("body-radius"));
        }
        if family == DiagramFamilyId::FLOWCHART && target == ThemeTarget::Node {
            patch.effect = SpecifiedWireV1::Clear;
        }
        design.rules.push(rule(Some(family), target, None, patch));
    }
    for (family, target, weight) in [
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::NodeLabel,
            "label-weight",
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::EdgeLabel,
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::ActorLabel,
            "label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::MessageLabel,
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::NoteLabel,
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabel,
            "secondary-label-weight",
        ),
    ] {
        design
            .rules
            .push(rule(Some(family), target, None, weighted(weight)));
    }
    for (id, label, color) in [
        ("node-oat", "Oat node", "#F6EBD6"),
        ("node-mist", "Mist node", "#E6EEE7"),
    ] {
        color_token(
            design,
            id,
            label,
            "Diagram details",
            "Muted Flowchart body tone; independent of Pie sectors and categorical surfaces.",
            color,
        );
    }
    for (offset, color) in ["surface", "node-oat", "node-mist"].into_iter().enumerate() {
        let mut entry = rule(
            Some(DiagramFamilyId::FLOWCHART),
            ThemeTarget::Node,
            None,
            style(Some(color), None),
        );
        entry.ordinal = Some(ThemeOrdinalSelectorWireV1::Cycle {
            cycle: ThemeOrdinalCycleWireV1 {
                period: 3,
                offset: offset as u32,
            },
        });
        design.rules.push(entry);
    }
}

fn append_xy_chart(design: &mut ThemeDesignV1) {
    for (target, ink, size, weight) in [
        (ThemeTarget::Title, "text", Some(20.0), 600.0),
        (ThemeTarget::AxisTitle, "subtle-text", Some(13.0), 500.0),
        (ThemeTarget::AxisLabel, "subtle-text", None, 500.0),
        (ThemeTarget::Legend, "subtle-text", Some(12.0), 500.0),
    ] {
        let mut patch = style(Some(ink), None);
        patch.typography = Some(literal(ThemeDesignTextStyleV1 {
            font_size_px: size.map_or(SpecifiedWireV1::Unspecified, |value| {
                SpecifiedWireV1::Value(px(value))
            }),
            font_weight: SpecifiedWireV1::Value(literal(weight)),
            ..Default::default()
        }));
        design
            .rules
            .push(rule(Some(DiagramFamilyId::XY_CHART), target, None, patch));
    }
    let mut tick = style(None, Some("line"));
    tick.opacity = SpecifiedWireV1::Value(literal(0.35));
    design.rules.push(rule(
        Some(DiagramFamilyId::XY_CHART),
        ThemeTarget::AxisTick,
        None,
        tick,
    ));
    for (variant, width) in [(ThemeVariant::Bar, 1.0), (ThemeVariant::Line, 2.25)] {
        design.rules.push(rule(
            Some(DiagramFamilyId::XY_CHART),
            ThemeTarget::ChartSeries,
            Some(variant),
            ThemeDesignStyleV1 {
                stroke: Some(literal(ThemeDesignStrokeV1 {
                    width: SpecifiedWireV1::Value(px(width)),
                    ..Default::default()
                })),
                fill_opacity: if variant == ThemeVariant::Bar {
                    SpecifiedWireV1::Value(literal(0.85))
                } else {
                    SpecifiedWireV1::Unspecified
                },
                ..Default::default()
            },
        ));
    }
}

fn texture_layer() -> ThemeDesignCanvasLayerV1 {
    ThemeDesignCanvasLayerV1 {
        paint: literal(ThemeDesignPaintV1::Structured(
            ThemeDesignPaintObjectV1::LinearGradient {
                angle_degrees: literal(28.0),
                stops: vec![
                    stop(0.0, "paper-texture", 0.025),
                    stop(0.0625, "paper-texture", 0.025),
                    stop(0.0625, "paper-texture", 0.0),
                    stop(1.0, "paper-texture", 0.0),
                ],
                repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
                    width_px: 8.0,
                    height_px: 8.0,
                }),
            },
        )),
        opacity: None,
        blend_mode: None,
        offset_x: None,
        offset_y: None,
    }
}
