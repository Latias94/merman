//! Bold borders and hard body shadows, with independently styled labels.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

const HARD_SHADOW: &str = "hard-shadow";

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    color_token(
        &mut design,
        "shadow-color",
        "Shadow color",
        "Effects",
        "Color of the hard body shadow.",
        "#000000",
    );
    length_token(
        &mut design,
        "shadow-offset",
        "Shadow offset",
        "Effects",
        "Shared horizontal and vertical displacement of the hard shadow, in pixels.",
        6.0,
    );
    let mut hard_shadow = shadow("shadow-color", 6.0, 0.0, 1.0);
    hard_shadow.offset_x = reference("shadow-offset");
    hard_shadow.offset_y = reference("shadow-offset");
    effect_token(
        &mut design,
        HARD_SHADOW,
        "Hard shadow",
        "Sharp offset shadow for selected diagram bodies and chart bars.",
        vec![hard_shadow],
    );
    length_token(
        &mut design,
        "border-width",
        "Border width",
        "Geometry",
        "Outline thickness for selected diagram bodies, connectors, and chart bars.",
        3.0,
    );
    length_token(
        &mut design,
        "chart-line-width",
        "Chart line width",
        "Geometry",
        "Stroke thickness for line-chart series.",
        4.0,
    );
    number_token(
        &mut design,
        "label-weight",
        "Label weight",
        "Font weight for selected node, edge, actor, and note labels.",
        700.0,
    );
    number_token(
        &mut design,
        "title-weight",
        "Title weight",
        "Font weight for XY chart titles.",
        900.0,
    );
    for (family, target, has_shadow) in [
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Node, true),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Edge, false),
        (DiagramFamilyId::FLOWCHART, ThemeTarget::Cluster, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Message, false),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation, true),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Lifeline, false),
    ] {
        design.rules.push(rule(
            Some(family),
            target,
            None,
            shape_style("border-width", has_shadow),
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
            "label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::ActorLabel,
            "label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::NoteLabel,
            "label-weight",
        ),
        (
            DiagramFamilyId::XY_CHART,
            ThemeTarget::Title,
            "title-weight",
        ),
    ] {
        design
            .rules
            .push(rule(Some(family), target, None, weighted(weight)));
    }
    for (variant, width, has_shadow) in [
        (ThemeVariant::Bar, "border-width", true),
        (ThemeVariant::Line, "chart-line-width", false),
    ] {
        design.rules.push(rule(
            Some(DiagramFamilyId::XY_CHART),
            ThemeTarget::ChartSeries,
            Some(variant),
            shape_style(width, has_shadow),
        ));
    }
    // The distinct zero-based 2n/3n/5n accent roles retain authored order and precedence.
    for (period, id, label, color) in [
        (2, "accent-even", "Every second node", "#FFE66D"),
        (3, "accent-third", "Every third node", "#4ECDC4"),
        (5, "accent-fifth", "Every fifth node", "#FF6B35"),
    ] {
        color_token(
            &mut design,
            id,
            label,
            "Diagram details",
            "Repeating Flowchart node fill; later matching rules take precedence.",
            color,
        );
        let mut entry = rule(
            Some(DiagramFamilyId::FLOWCHART),
            ThemeTarget::Node,
            None,
            style(Some(id), None),
        );
        entry.ordinal = Some(ThemeOrdinalSelectorWireV1::Cycle {
            cycle: ThemeOrdinalCycleWireV1 {
                period,
                offset: period - 1,
            },
        });
        design.rules.push(entry);
    }
    design
}

fn shape_style(width: &str, has_shadow: bool) -> ThemeDesignStyleV1 {
    ThemeDesignStyleV1 {
        stroke: Some(literal(ThemeDesignStrokeV1 {
            width: SpecifiedWireV1::Value(reference(width)),
            ..Default::default()
        })),
        effect: if has_shadow {
            SpecifiedWireV1::Value(reference(HARD_SHADOW))
        } else {
            SpecifiedWireV1::Unspecified
        },
        ..Default::default()
    }
}
