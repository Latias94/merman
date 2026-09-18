//! Complete Cyberpunk recipe data. Drawing and support remain owned by each family writer.

use merman_theme_contract::{
    DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeCanvasLayerWireV1, ThemeCanvasPaintObjectWireV1,
    ThemeCanvasPaintWireV1, ThemeEffectEntryWireV1, ThemeEffectPrimitiveWireV1,
    ThemeGradientStopWireV1, ThemeLengthWireV1, ThemeLinearGradientRepetitionWireV1,
    ThemeMaterializationErrorV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1,
};

use super::catalog::{PresetPalette, build_cross_family_recipe};
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeResourcePolicy, ThemeTarget};

const SHAPE_GLOW: &str = "cyberpunk-shape-glow";

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
    spec.effects = Some(vec![ThemeEffectEntryWireV1::Graph {
        id: SHAPE_GLOW.to_owned(),
        color_space: Some("srgb".to_owned()),
        primitives: vec![
            shadow("source-graphic", 8.0, "rgba(0, 242, 255, 0.5)"),
            shadow("previous", 16.0, "rgba(0, 242, 255, 0.3)"),
        ],
    }]);
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
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(2.0),
                    ..ThemeStrokePatchWireV1::default()
                }),
                ..ThemeStylePatchWireV1::default()
            },
        ),
        family_rule(
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Actor,
            ThemeStylePatchWireV1 {
                effect: SpecifiedWireV1::Value(SHAPE_GLOW.to_owned()),
                ..ThemeStylePatchWireV1::default()
            },
        ),
    ]);
    Ok(spec)
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
