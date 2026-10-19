//! Two-axis paper grid and manual-style typography, without filters.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::ThemeTarget;

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    color_token(&mut design, "grid-color", "Paper grid", "#2c2416");
    length_token(&mut design, "border-width", "Border width", 2.5);
    number_token(&mut design, "label-weight", "Label weight", 700.0);
    number_token(
        &mut design,
        "secondary-label-weight",
        "Secondary label weight",
        600.0,
    );
    design
        .canvas
        .as_mut()
        .expect("shared design owns a canvas")
        .layers = vec![grid_layer(0.0), grid_layer(90.0)];
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
        design.rules.push(rule(
            Some(family),
            target,
            None,
            ThemeDesignStyleV1 {
                stroke: Some(literal(ThemeDesignStrokeV1 {
                    width: SpecifiedWireV1::Value(reference("border-width")),
                    ..Default::default()
                })),
                ..Default::default()
            },
        ));
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
            ThemeTarget::NoteLabel,
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Title,
            "label-weight",
        ),
    ] {
        design
            .rules
            .push(rule(Some(family), target, None, weighted(weight)));
    }
    design
}

fn grid_layer(angle_degrees: f32) -> ThemeDesignCanvasLayerV1 {
    ThemeDesignCanvasLayerV1 {
        paint: literal(ThemeDesignPaintV1::Structured(
            ThemeDesignPaintObjectV1::LinearGradient {
                angle_degrees: literal(angle_degrees),
                stops: vec![
                    stop(0.0, "grid-color", 0.02),
                    stop(1.0 / 40.0, "grid-color", 0.02),
                    stop(1.0 / 40.0, "grid-color", 0.0),
                    stop(1.0, "grid-color", 0.0),
                ],
                repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
                    width_px: 40.0,
                    height_px: 40.0,
                }),
            },
        )),
        opacity: None,
        blend_mode: None,
        offset_x: None,
        offset_y: None,
    }
}
