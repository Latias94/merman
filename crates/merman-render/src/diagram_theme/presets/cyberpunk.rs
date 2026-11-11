//! Neutral ink on dark panels, with neon contours and restrained body and edge glows.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

const SHAPE_GLOW: &str = "shape-glow";
const EDGE_GLOW: &str = "edge-glow";
const NOTE_GLOW: &str = "note-glow";
const SOFT_CYAN_GLOW: &str = "soft-cyan-glow";

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    // Git tags keep Mermaid's pale tag backing; branch and commit labels have other surfaces.
    design.rules.push(rule(
        Some(DiagramFamilyId::GIT_GRAPH),
        ThemeTarget::NodeLabel,
        None,
        style(Some("category-text"), None),
    ));
    color_token(
        &mut design,
        "neon-cyan",
        "Neon cyan",
        "Core colors",
        "Shared cyan source for outlines, chart marks, canvas gradients, and glows; label ink is independent.",
        palette.border,
    );
    color_token(
        &mut design,
        "neon-magenta",
        "Neon magenta",
        "Core colors",
        "Shared magenta source for accents, note outlines, chart marks, and glows; label ink is independent.",
        palette.accent,
    );
    color_token(
        &mut design,
        "chart-amber",
        "Chart amber",
        "Series palette",
        "Warm third chart ink, independent of the pastel filled-category palette.",
        "#E8C078",
    );
    // These bindings are source intent, not an equality-based inference over output colors.
    for id in [
        "border",
        "line",
        "cluster-border",
        "actor-border",
        "activation-border",
    ] {
        design.tokens.get_mut(id).expect("shared color role").value =
            ThemeDesignTokenValueV1::Color(reference("neon-cyan"));
    }
    for id in ["accent", "note-border"] {
        design.tokens.get_mut(id).expect("shared color role").value =
            ThemeDesignTokenValueV1::Color(reference("neon-magenta"));
    }
    length_token(
        &mut design,
        "body-border-width",
        "Body border width",
        "Geometry",
        "Outline thickness for Flowchart nodes and sequence actors and activations.",
        1.5,
    );
    length_token(
        &mut design,
        "detail-border-width",
        "Detail border width",
        "Geometry",
        "Outline thickness for selected connectors and sequence details.",
        1.25,
    );
    length_token(
        &mut design,
        "body-radius",
        "Body radius",
        "Geometry",
        "Corner radius for Flowchart nodes and sequence actors and notes.",
        4.0,
    );
    number_token(
        &mut design,
        "label-weight",
        "Label weight",
        "Font weight for Flowchart node labels and sequence actor labels.",
        600.0,
    );
    number_token(
        &mut design,
        "secondary-label-weight",
        "Secondary label weight",
        "Font weight for connector, message, loop, and note labels.",
        500.0,
    );
    design
        .canvas
        .as_mut()
        .expect("shared design owns a canvas")
        .layers = vec![
        screen_layer(ThemeDesignPaintObjectV1::RadialGradient {
            center_x: literal(ThemeLengthWireV1::Percent { percent: 50.0 }),
            center_y: literal(ThemeLengthWireV1::Percent { percent: 50.0 }),
            // CSS farthest-corner radius, converted to SVG's normalized diagonal radius.
            radius: literal(ThemeLengthWireV1::Percent {
                percent: 100.0 / std::f32::consts::SQRT_2,
            }),
            stops: vec![stop(0.0, "neon-cyan", 0.025), stop(0.7, "neon-cyan", 0.0)],
            repetition: None,
        }),
        screen_layer(grid_line(90.0)),
        screen_layer(grid_line(180.0)),
    ];
    for (id, label, description, color, sigma, opacity) in [
        (
            SOFT_CYAN_GLOW,
            "Soft cyan glow",
            "Restrained glow for sequence loop outlines.",
            "neon-cyan",
            1.5,
            0.12,
        ),
        (
            EDGE_GLOW,
            "Edge glow",
            "Glow for Flowchart edges and sequence messages and lifelines.",
            "neon-cyan",
            1.5,
            0.16,
        ),
        (
            NOTE_GLOW,
            "Note body glow",
            "Magenta glow around sequence note bodies.",
            "neon-magenta",
            2.5,
            0.12,
        ),
    ] {
        effect_token(
            &mut design,
            id,
            label,
            description,
            vec![shadow(color, 0.0, sigma, opacity)],
        );
    }
    effect_token(
        &mut design,
        SHAPE_GLOW,
        "Body glow",
        "Restrained layered cyan glow for Flowchart bodies and sequence actor bodies.",
        vec![
            shadow("neon-cyan", 0.0, 2.0, 0.2),
            shadow("neon-cyan", 0.0, 5.0, 0.1),
        ],
    );
    for (family, target, effect, width, radius, paint) in [
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Loop,
            Some(SOFT_CYAN_GLOW),
            Some("detail-border-width"),
            false,
            true,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabelBackground,
            None,
            Some("detail-border-width"),
            false,
            false,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Node,
            Some(SHAPE_GLOW),
            Some("body-border-width"),
            true,
            false,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            Some(EDGE_GLOW),
            Some("detail-border-width"),
            false,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Actor,
            Some(SHAPE_GLOW),
            Some("body-border-width"),
            true,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Message,
            Some(EDGE_GLOW),
            Some("detail-border-width"),
            false,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Note,
            Some(NOTE_GLOW),
            Some("detail-border-width"),
            true,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            Some(EDGE_GLOW),
            Some("detail-border-width"),
            false,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Activation,
            None,
            Some("body-border-width"),
            false,
            false,
        ),
    ] {
        let mut style = ThemeDesignStyleV1::default();
        if let Some(id) = effect {
            style.effect = SpecifiedWireV1::Value(reference(id));
        }
        if radius {
            style.radius = SpecifiedWireV1::Value(reference("body-radius"));
        }
        if let Some(id) = width {
            style.stroke = Some(literal(ThemeDesignStrokeV1 {
                width: SpecifiedWireV1::Value(reference(id)),
                paint: if paint {
                    SpecifiedWireV1::Value(super::design::paint("neon-cyan"))
                } else {
                    SpecifiedWireV1::Unspecified
                },
                ..Default::default()
            }));
        }
        design.rules.push(rule(Some(family), target, None, style));
    }
    for (family, target, ink, weight) in [
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::NodeLabel,
            "text",
            "label-weight",
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::EdgeLabel,
            "text",
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::ActorLabel,
            "actor-text",
            "label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::MessageLabel,
            "text",
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabel,
            "text",
            "secondary-label-weight",
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::NoteLabel,
            "note-text",
            "secondary-label-weight",
        ),
    ] {
        let mut patch = weighted(weight);
        patch.fill = SpecifiedWireV1::Value(super::design::paint(ink));
        design.rules.push(rule(Some(family), target, None, patch));
    }
    append_xy_series(&mut design);
    append_xy_text(&mut design);
    let mut tick = style(None, Some("subtle-text"));
    tick.opacity = SpecifiedWireV1::Value(literal(0.35));
    design.rules.push(rule(
        Some(DiagramFamilyId::XY_CHART),
        ThemeTarget::AxisTick,
        None,
        tick,
    ));
    design
}

fn append_xy_text(design: &mut ThemeDesignV1) {
    let mut legend = style(Some("subtle-text"), None);
    legend.typography = Some(literal(ThemeDesignTextStyleV1 {
        font_size_px: SpecifiedWireV1::Value(px(12.0)),
        ..Default::default()
    }));
    design.rules.push(rule(
        Some(DiagramFamilyId::XY_CHART),
        ThemeTarget::Legend,
        None,
        legend,
    ));
    for (target, ink, size, weight) in [
        (ThemeTarget::Title, "text", Some(18.0), Some(600.0)),
        (
            ThemeTarget::AxisTitle,
            "subtle-text",
            Some(13.0),
            Some(500.0),
        ),
        (ThemeTarget::AxisLabel, "subtle-text", None, Some(500.0)),
    ] {
        let mut patch = style(Some(ink), None);
        patch.typography = Some(literal(ThemeDesignTextStyleV1 {
            font_size_px: size.map_or(SpecifiedWireV1::Unspecified, |value| {
                SpecifiedWireV1::Value(px(value))
            }),
            font_weight: weight.map_or(SpecifiedWireV1::Unspecified, |value| {
                SpecifiedWireV1::Value(literal(value))
            }),
            ..Default::default()
        }));
        design
            .rules
            .push(rule(Some(DiagramFamilyId::XY_CHART), target, None, patch));
    }
}

fn append_xy_series(design: &mut ThemeDesignV1) {
    let colors = ["neon-cyan", "neon-magenta", "chart-amber"];
    for (offset, color) in colors.into_iter().enumerate() {
        let color_id = format!("xy-series.{offset}");
        token(
            design,
            &color_id,
            &format!("Chart series {}", offset + 1),
            "Series palette",
            "Chart ink for marks and legend swatches, including the matching XY glow.",
            ThemeDesignTokenValueV1::Color(reference(color)),
        );
        for (kind, width, sigma, opacity) in [
            (ThemeVariant::Bar, 1.25, 2.0, 0.12),
            (ThemeVariant::Line, 2.0, 1.5, 0.16),
        ] {
            let effect_id = format!("xy-{}-{offset}-glow", kind.id());
            effect_token(
                design,
                &effect_id,
                &format!("Chart {} {} glow", kind.id(), offset + 1),
                "Glow for this repeating XY chart series and variant.",
                vec![shadow(&color_id, 0.0, sigma, opacity)],
            );
            let mut patch = style(
                (kind == ThemeVariant::Bar).then_some(color_id.as_str()),
                Some(&color_id),
            );
            if kind == ThemeVariant::Bar {
                patch.fill_opacity = SpecifiedWireV1::Value(literal(0.35));
            }
            if let Some(ThemeValueV1::Literal(stroke)) = patch.stroke.as_mut() {
                stroke.width = SpecifiedWireV1::Value(px(width));
            }
            patch.effect = SpecifiedWireV1::Value(reference(&effect_id));
            let mut entry = rule(
                Some(DiagramFamilyId::XY_CHART),
                ThemeTarget::ChartSeries,
                Some(kind),
                patch,
            );
            entry.ordinal = Some(ThemeOrdinalSelectorWireV1::Cycle {
                cycle: ThemeOrdinalCycleWireV1 {
                    period: colors.len() as u32,
                    offset: offset as u32,
                },
            });
            design.rules.push(entry);
        }
    }
    token(
        design,
        "xy-series",
        "Chart inks",
        "Series palette",
        "Three repeating chart inks, independent of the twelve pastel filled categories and Pie sectors.",
        ThemeDesignTokenValueV1::Palette(literal(
            (0..colors.len())
                .map(|index| reference(&format!("xy-series.{index}")))
                .collect(),
        )),
    );
    design.palettes.insert(
        ThemeTarget::ChartSeries.id().to_owned(),
        reference("xy-series"),
    );
}

fn screen_layer(paint: ThemeDesignPaintObjectV1) -> ThemeDesignCanvasLayerV1 {
    ThemeDesignCanvasLayerV1 {
        paint: literal(ThemeDesignPaintV1::Structured(paint)),
        opacity: None,
        blend_mode: Some("screen".to_owned()),
        offset_x: None,
        offset_y: None,
    }
}

fn grid_line(angle_degrees: f32) -> ThemeDesignPaintObjectV1 {
    ThemeDesignPaintObjectV1::LinearGradient {
        angle_degrees: literal(angle_degrees),
        stops: vec![
            stop(0.0, "neon-cyan", 0.018),
            stop(1.0 / 40.0, "neon-cyan", 0.018),
            stop(1.0 / 40.0, "neon-cyan", 0.0),
            stop(1.0, "neon-cyan", 0.0),
        ],
        repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
            width_px: 40.0,
            height_px: 40.0,
        }),
    }
}
