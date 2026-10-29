//! Warm paper and muted Pie colors, with separately authored text roles.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::ThemeTarget;

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    color_token(
        &mut design,
        "paper-texture",
        "Paper texture",
        "Effects",
        "Color of the faint diagonal fibers on the exported canvas.",
        "#D2B48C",
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
        .layers = [45.0, -45.0]
        .into_iter()
        .flat_map(|angle| [texture_layer(angle, false), texture_layer(angle, true)])
        .collect();

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

fn texture_layer(angle: f32, trailing: bool) -> ThemeDesignCanvasLayerV1 {
    let stops = if trailing {
        vec![
            stop(0.0, "paper-texture", 0.0),
            stop(0.75, "paper-texture", 0.0),
            stop(0.75, "paper-texture", 0.03),
            stop(1.0, "paper-texture", 0.03),
        ]
    } else {
        vec![
            stop(0.0, "paper-texture", 0.03),
            stop(0.25, "paper-texture", 0.03),
            stop(0.25, "paper-texture", 0.0),
            stop(1.0, "paper-texture", 0.0),
        ]
    };
    ThemeDesignCanvasLayerV1 {
        paint: literal(ThemeDesignPaintV1::Structured(
            ThemeDesignPaintObjectV1::LinearGradient {
                angle_degrees: literal(angle),
                stops,
                repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
                    width_px: 20.0,
                    height_px: 20.0,
                }),
            },
        )),
        opacity: None,
        blend_mode: None,
        offset_x: None,
        offset_y: None,
    }
}
