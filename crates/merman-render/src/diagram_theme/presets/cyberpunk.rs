//! Complete Cyberpunk recipe data. Drawing and support remain owned by each family writer.

use merman_theme_contract::{
    DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeCanvasLayerWireV1, ThemeCanvasPaintObjectWireV1,
    ThemeCanvasPaintWireV1, ThemeEffectEntryWireV1, ThemeEffectPrimitiveWireV1,
    ThemeGradientStopWireV1, ThemeLengthWireV1, ThemeLinearGradientRepetitionWireV1,
    ThemeMaterializationErrorV1, ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1, ThemeTextStylePatchWireV1,
};

use super::catalog::{PresetPalette, build_cross_family_recipe};
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeResourcePolicy, ThemeTarget, ThemeVariant};

const SHAPE_GLOW: &str = "cyberpunk-shape-glow";
const EDGE_GLOW: &str = "cyberpunk-edge-glow";
const NOTE_GLOW: &str = "cyberpunk-note-glow";
const LOOP_TEXT_GLOW: &str = "cyberpunk-loop-text-glow";
const NOTE_TEXT_GLOW: &str = "cyberpunk-note-text-glow";

pub(super) fn build_recipe(
    palette: PresetPalette,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1> {
    let mut spec = build_cross_family_recipe(palette, resources)?;
    // CSS backgrounds are listed front to back; canvas layers paint back to front.
    // The centered CSS circle's farthest-corner radius is half the diagonal, whereas
    // SVG percentage radii use the diagonal divided by sqrt(2).
    spec.canvas
        .as_mut()
        .expect("tokens materialize a canvas")
        .layers = Some(vec![
        screen_layer(ThemeCanvasPaintObjectWireV1::RadialGradient {
            center_x: ThemeLengthWireV1::Percent { percent: 50.0 },
            center_y: ThemeLengthWireV1::Percent { percent: 50.0 },
            radius: ThemeLengthWireV1::Percent {
                percent: 100.0 / std::f32::consts::SQRT_2,
            },
            stops: vec![
                stop(0.0, "rgba(0, 242, 255, 0.05)"),
                stop(0.7, "rgba(0, 242, 255, 0)"),
            ],
            repetition: None,
        }),
        screen_layer(grid_line(90.0)),
        screen_layer(grid_line(180.0)),
    ]);
    spec.effects = Some(vec![
        ThemeEffectEntryWireV1::Graph {
            id: SHAPE_GLOW.to_owned(),
            color_space: Some("srgb".to_owned()),
            primitives: vec![
                shadow("source-graphic", 8.0, "rgba(0, 242, 255, 0.5)"),
                shadow("previous", 16.0, "rgba(0, 242, 255, 0.3)"),
            ],
        },
        ThemeEffectEntryWireV1::Graph {
            id: EDGE_GLOW.to_owned(),
            color_space: Some("srgb".to_owned()),
            primitives: vec![shadow("source-graphic", 6.0, "rgba(0, 242, 255, 0.6)")],
        },
        ThemeEffectEntryWireV1::Graph {
            id: NOTE_GLOW.to_owned(),
            color_space: Some("srgb".to_owned()),
            primitives: vec![shadow("source-graphic", 8.0, "rgba(255, 0, 255, 0.4)")],
        },
        ThemeEffectEntryWireV1::Graph {
            id: LOOP_TEXT_GLOW.to_owned(),
            color_space: Some("srgb".to_owned()),
            // The reference's 10px CSS text-shadow blur lowers to sigma 5.
            primitives: vec![shadow("source-graphic", 5.0, "rgba(0, 242, 255, 0.5)")],
        },
        ThemeEffectEntryWireV1::Graph {
            id: NOTE_TEXT_GLOW.to_owned(),
            color_space: Some("srgb".to_owned()),
            // CSS text-shadow blur 8px corresponds to Gaussian sigma 4px.
            primitives: vec![shadow("source-graphic", 4.0, "rgba(255, 0, 255, 0.4)")],
        },
    ]);
    // Use scoped rules, not global Node/Actor bindings. A different family receives
    // the shared palette without inheriting another family's effect requirement.
    // Each writer must account for its actual terminals; selecting a preset does not
    // grant visual qualification or imply that every shape consumes the graph.
    spec.styles.get_or_insert_default().extend([
        family_rule(
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Node,
            ThemeStylePatchWireV1 {
                effect: SpecifiedWireV1::Value(SHAPE_GLOW.to_owned()),
                radius: SpecifiedWireV1::Value(10.0),
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(3.0),
                    ..ThemeStrokePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            ThemeStylePatchWireV1 {
                effect: SpecifiedWireV1::Value(EDGE_GLOW.to_owned()),
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(2.0),
                    ..ThemeStrokePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::NodeLabel,
            ThemeStylePatchWireV1 {
                typography: Some(ThemeTextStylePatchWireV1 {
                    font_weight: SpecifiedWireV1::Value(600),
                    ..ThemeTextStylePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::EdgeLabel,
            ThemeStylePatchWireV1 {
                typography: Some(ThemeTextStylePatchWireV1 {
                    font_weight: SpecifiedWireV1::Value(600),
                    ..ThemeTextStylePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Actor,
            ThemeStylePatchWireV1 {
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(3.0),
                    ..ThemeStrokePatchWireV1::default()
                }),
                radius: SpecifiedWireV1::Value(10.0),
                effect: SpecifiedWireV1::Value(SHAPE_GLOW.to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Message,
            ThemeStylePatchWireV1 {
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(2.0),
                    ..ThemeStrokePatchWireV1::default()
                }),
                effect: SpecifiedWireV1::Value(EDGE_GLOW.to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Note,
            ThemeStylePatchWireV1 {
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(2.0),
                    ..ThemeStrokePatchWireV1::default()
                }),
                radius: SpecifiedWireV1::Value(10.0),
                effect: SpecifiedWireV1::Value(NOTE_GLOW.to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabel,
            ThemeStylePatchWireV1 {
                effect: SpecifiedWireV1::Value(LOOP_TEXT_GLOW.to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::NoteLabel,
            ThemeStylePatchWireV1 {
                effect: SpecifiedWireV1::Value(NOTE_TEXT_GLOW.to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        ),
    ]);
    append_xy_series(&mut spec);
    append_xy_text(&mut spec);
    spec.styles.get_or_insert_default().push(family_rule(
        DiagramFamilyId::XY_CHART,
        ThemeTarget::AxisTick,
        ThemeStylePatchWireV1 {
            stroke: Some(ThemeStrokePatchWireV1 {
                paint: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color("#00f2ff".to_owned())),
                ..ThemeStrokePatchWireV1::default()
            }),
            opacity: SpecifiedWireV1::Value(0.3),
            ..ThemeStylePatchWireV1::default()
        },
    ));
    Ok(spec)
}

fn append_xy_text(spec: &mut DiagramThemeSpecWireV1) {
    // CSS text-shadow blur radii map to half as much SVG Gaussian sigma.
    for (target, size, weight, sigma, color) in [
        (
            ThemeTarget::Title,
            Some(18.0),
            Some(700),
            7.5,
            "rgba(0, 242, 255, 0.8)",
        ),
        (
            ThemeTarget::AxisTitle,
            Some(13.0),
            None,
            5.0,
            "rgba(0, 242, 255, 0.6)",
        ),
        (
            ThemeTarget::Legend,
            Some(12.0),
            None,
            4.0,
            "rgba(0, 242, 255, 0.5)",
        ),
        (
            ThemeTarget::AxisLabel,
            None,
            Some(600),
            5.0,
            "rgba(0, 242, 255, 0.5)",
        ),
    ] {
        let effect_id = format!("cyberpunk-xy-{}-glow", target.id());
        spec.effects
            .get_or_insert_default()
            .push(ThemeEffectEntryWireV1::Graph {
                id: effect_id.clone(),
                color_space: Some("srgb".to_owned()),
                primitives: vec![shadow("source-graphic", sigma, color)],
            });
        spec.styles.get_or_insert_default().push(family_rule(
            DiagramFamilyId::XY_CHART,
            target,
            ThemeStylePatchWireV1 {
                fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color("#00f2ff".to_owned())),
                effect: SpecifiedWireV1::Value(effect_id),
                typography: Some(ThemeTextStylePatchWireV1 {
                    font_size_px: size.map_or(SpecifiedWireV1::Unspecified, SpecifiedWireV1::Value),
                    font_weight: weight
                        .map_or(SpecifiedWireV1::Unspecified, SpecifiedWireV1::Value),
                    ..ThemeTextStylePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ));
    }
}

fn append_xy_series(spec: &mut DiagramThemeSpecWireV1) {
    let colors = [
        ("#6cc6cb", "108, 198, 203"),
        ("#c77dff", "199, 125, 255"),
        ("#7ce38b", "124, 227, 139"),
    ];
    for (offset, (color, rgb)) in colors.into_iter().enumerate() {
        for (kind, width, blur, alpha) in [
            (ThemeVariant::Bar, 2.0, 8.0, 0.4),
            (ThemeVariant::Line, 3.0, 6.0, 0.5),
        ] {
            let effect_id = format!("cyberpunk-xy-{}-{offset}-glow", kind.id());
            spec.effects
                .get_or_insert_default()
                .push(ThemeEffectEntryWireV1::Graph {
                    id: effect_id.clone(),
                    color_space: Some("srgb".to_owned()),
                    primitives: vec![shadow(
                        "source-graphic",
                        blur,
                        &format!("rgba({rgb}, {alpha})"),
                    )],
                });
            spec.styles
                .get_or_insert_default()
                .push(ThemeRuleSetWireV1::Rule {
                    target: ThemeTarget::ChartSeries.id().to_owned(),
                    family: Some(DiagramFamilyId::XY_CHART.as_str().to_owned()),
                    variant: Some(kind.id().to_owned()),
                    ordinal: Some(ThemeOrdinalSelectorWireV1::Cycle {
                        cycle: ThemeOrdinalCycleWireV1 {
                            period: 3,
                            offset: offset as u32,
                        },
                    }),
                    style: ThemeStylePatchWireV1 {
                        fill: if kind == ThemeVariant::Bar {
                            SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(color.to_owned()))
                        } else {
                            SpecifiedWireV1::Unspecified
                        },
                        fill_opacity: if kind == ThemeVariant::Bar {
                            SpecifiedWireV1::Value(0.2)
                        } else {
                            SpecifiedWireV1::Unspecified
                        },
                        stroke: Some(ThemeStrokePatchWireV1 {
                            paint: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(
                                color.to_owned(),
                            )),
                            width: SpecifiedWireV1::Value(width),
                            ..ThemeStrokePatchWireV1::default()
                        }),
                        effect: SpecifiedWireV1::Value(effect_id),
                        ..ThemeStylePatchWireV1::default()
                    },
                });
        }
    }
}

fn screen_layer(paint: ThemeCanvasPaintObjectWireV1) -> ThemeCanvasLayerWireV1 {
    ThemeCanvasLayerWireV1 {
        paint: ThemeCanvasPaintWireV1::Structured(paint),
        opacity: None,
        blend_mode: Some("screen".to_owned()),
        offset_x: None,
        offset_y: None,
    }
}

fn grid_line(angle_degrees: f32) -> ThemeCanvasPaintObjectWireV1 {
    ThemeCanvasPaintObjectWireV1::LinearGradient {
        angle_degrees,
        stops: vec![
            stop(0.0, "rgba(0, 242, 255, 0.03)"),
            stop(1.0 / 40.0, "rgba(0, 242, 255, 0.03)"),
            stop(1.0 / 40.0, "transparent"),
            stop(1.0, "transparent"),
        ],
        repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
            width_px: 40.0,
            height_px: 40.0,
        }),
    }
}

fn stop(offset: f32, color: &str) -> ThemeGradientStopWireV1 {
    ThemeGradientStopWireV1 {
        offset,
        color: color.to_owned(),
    }
}

fn shadow(input: &str, blur_radius: f32, color: &str) -> ThemeEffectPrimitiveWireV1 {
    ThemeEffectPrimitiveWireV1::DropShadow {
        input: Some(input.to_owned()),
        offset_x: 0.0,
        offset_y: 0.0,
        blur_radius,
        spread: 0.0,
        color: color.to_owned(),
    }
}

fn family_rule(
    family: DiagramFamilyId,
    target: ThemeTarget,
    style: ThemeStylePatchWireV1,
) -> ThemeRuleSetWireV1 {
    ThemeRuleSetWireV1::Rule {
        target: target.id().to_owned(),
        family: Some(family.as_str().to_owned()),
        variant: None,
        ordinal: None,
        style,
    }
}
