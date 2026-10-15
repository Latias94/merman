//! Spotless recipe: two-axis paper grid and manual-style typography, without filters.

use merman_theme_contract::{
    DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeCanvasLayerWireV1, ThemeCanvasPaintObjectWireV1,
    ThemeCanvasPaintWireV1, ThemeGradientStopWireV1, ThemeLinearGradientRepetitionWireV1,
    ThemeMaterializationErrorV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1,
    ThemeTextStylePatchWireV1,
};

use super::catalog::{PresetPalette, build_cross_family_recipe};
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeResourcePolicy, ThemeTarget};

pub(super) fn build_recipe(
    palette: PresetPalette,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1> {
    let mut spec = build_cross_family_recipe(palette, resources)?;
    spec.canvas
        .as_mut()
        .expect("preset tokens materialize a canvas")
        .layers = Some(vec![grid_layer(0.0), grid_layer(90.0)]);
    let styles = spec.styles.get_or_insert_default();
    for (family, target) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Edge),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Cluster),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Message),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation),
        (DiagramFamilyId::XY_CHART, ThemeTarget::ChartSeries),
    ] {
        styles.push(rule(
            family,
            target,
            ThemeStylePatchWireV1 {
                stroke: Some(ThemeStrokePatchWireV1 {
                    width: SpecifiedWireV1::Value(2.5),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ));
    }
    for (family, target, weight) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::NodeLabel, 700),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::EdgeLabel, 600),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::ActorLabel, 700),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::NoteLabel, 600),
        (DiagramFamilyId::XY_CHART, ThemeTarget::Title, 700),
    ] {
        styles.push(rule(
            family,
            target,
            ThemeStylePatchWireV1 {
                typography: Some(ThemeTextStylePatchWireV1 {
                    font_weight: SpecifiedWireV1::Value(weight),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ));
    }
    Ok(spec)
}

fn rule(
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

fn grid_layer(angle_degrees: f32) -> ThemeCanvasLayerWireV1 {
    ThemeCanvasLayerWireV1 {
        paint: ThemeCanvasPaintWireV1::Structured(ThemeCanvasPaintObjectWireV1::LinearGradient {
            angle_degrees,
            stops: vec![
                stop(0.0, "rgba(44, 36, 22, 0.02)"),
                stop(1.0 / 40.0, "rgba(44, 36, 22, 0.02)"),
                stop(1.0 / 40.0, "transparent"),
                stop(1.0, "transparent"),
            ],
            repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
                width_px: 40.0,
                height_px: 40.0,
            }),
        }),
        opacity: None,
        blend_mode: None,
        offset_x: None,
        offset_y: None,
    }
}

fn stop(offset: f32, color: &str) -> ThemeGradientStopWireV1 {
    ThemeGradientStopWireV1 {
        offset,
        color: color.to_owned(),
    }
}
