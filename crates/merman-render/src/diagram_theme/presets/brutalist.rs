//! Brutalist recipe: bold borders and hard shape shadows, with unfiltered text.

use merman_theme_contract::{
    DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeCanvasPaintWireV1, ThemeEffectEntryWireV1,
    ThemeEffectPrimitiveWireV1, ThemeMaterializationErrorV1, ThemeOrdinalCycleWireV1,
    ThemeOrdinalSelectorWireV1, ThemeRuleSetWireV1, ThemeStrokePatchWireV1, ThemeStylePatchWireV1,
    ThemeTextStylePatchWireV1,
};

use super::catalog::{PresetPalette, build_cross_family_recipe};
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeResourcePolicy, ThemeTarget, ThemeVariant};

const HARD_SHADOW: &str = "brutalist-hard-shadow";

pub(super) fn build_recipe(
    palette: PresetPalette,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1, ThemeMaterializationErrorV1> {
    let mut spec = build_cross_family_recipe(palette, resources)?;
    spec.effects = Some(vec![ThemeEffectEntryWireV1::Graph {
        id: HARD_SHADOW.to_owned(),
        color_space: Some("srgb".to_owned()),
        primitives: vec![ThemeEffectPrimitiveWireV1::DropShadow {
            input: Some("source-graphic".to_owned()),
            offset_x: 6.0,
            offset_y: 6.0,
            blur_radius: 0.0,
            spread: 0.0,
            color: "#000000".to_owned(),
        }],
    }]);
    let styles = spec.styles.get_or_insert_default();
    for (family, target, shadow) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node, true),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Edge, false),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Cluster, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Message, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Lifeline, false),
    ] {
        styles.push(rule(family, target, None, shape_style(3.0, shadow)));
    }
    for (family, target, weight) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::NodeLabel, 700),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::EdgeLabel, 700),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::ActorLabel, 700),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::NoteLabel, 700),
        (DiagramFamilyId::XY_CHART, ThemeTarget::Title, 900),
    ] {
        styles.push(rule(
            family,
            target,
            None,
            ThemeStylePatchWireV1 {
                typography: Some(ThemeTextStylePatchWireV1 {
                    font_weight: SpecifiedWireV1::Value(weight),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ));
    }
    for (variant, width, shadow) in [
        (ThemeVariant::Bar, 3.0, true),
        (ThemeVariant::Line, 4.0, false),
    ] {
        styles.push(rule(
            DiagramFamilyId::XY_CHART,
            ThemeTarget::ChartSeries,
            Some(variant),
            shape_style(width, shadow),
        ));
    }
    // Match the reference's 2n/3n/5n accents using zero-based cycle offsets.
    for (period, color) in [(2, "#FFE66D"), (3, "#4ECDC4"), (5, "#FF6B35")] {
        styles.push(ThemeRuleSetWireV1::Rule {
            target: ThemeTarget::Node.id().to_owned(),
            family: Some(DiagramFamilyId::FLOWCHART.as_str().to_owned()),
            variant: None,
            ordinal: Some(ThemeOrdinalSelectorWireV1::Cycle {
                cycle: ThemeOrdinalCycleWireV1 {
                    period,
                    offset: period - 1,
                },
            }),
            style: ThemeStylePatchWireV1 {
                fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(color.to_owned())),
                ..Default::default()
            },
        });
    }
    Ok(spec)
}

fn shape_style(width: f32, shadow: bool) -> ThemeStylePatchWireV1 {
    ThemeStylePatchWireV1 {
        stroke: Some(ThemeStrokePatchWireV1 {
            width: SpecifiedWireV1::Value(width),
            ..Default::default()
        }),
        effect: if shadow {
            SpecifiedWireV1::Value(HARD_SHADOW.to_owned())
        } else {
            SpecifiedWireV1::Unspecified
        },
        ..Default::default()
    }
}

fn rule(
    family: DiagramFamilyId,
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    style: ThemeStylePatchWireV1,
) -> ThemeRuleSetWireV1 {
    ThemeRuleSetWireV1::Rule {
        target: target.id().to_owned(),
        family: Some(family.as_str().to_owned()),
        variant: variant.map(|value| value.id().to_owned()),
        ordinal: None,
        style,
    }
}
