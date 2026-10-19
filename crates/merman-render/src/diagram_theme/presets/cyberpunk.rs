//! Authored neon colors, layered canvas, and scoped cascade glows.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

const SHAPE_GLOW: &str = "shape-glow";
const EDGE_GLOW: &str = "edge-glow";
const NOTE_GLOW: &str = "note-glow";
const LOOP_TEXT_GLOW: &str = "loop-text-glow";
const SOFT_CYAN_GLOW: &str = "soft-cyan-glow";
const KEYWORD_GLOW: &str = "keyword-glow";
const NOTE_TEXT_GLOW: &str = "note-text-glow";

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    color_token(&mut design, "neon-cyan", "Neon cyan", "#00f2ff");
    color_token(&mut design, "neon-magenta", "Neon magenta", "#ff00ff");
    // These bindings are source intent, not an equality-based inference over output colors.
    for id in [
        "text",
        "subtle-text",
        "border",
        "line",
        "cluster-border",
        "actor-border",
        "actor-text",
        "activation-border",
    ] {
        design.tokens.get_mut(id).expect("shared color role").value =
            ThemeDesignTokenValueV1::Color(reference("neon-cyan"));
    }
    for id in ["accent", "note-border", "note-text"] {
        design.tokens.get_mut(id).expect("shared color role").value =
            ThemeDesignTokenValueV1::Color(reference("neon-magenta"));
    }
    length_token(&mut design, "body-border-width", "Body border width", 3.0);
    length_token(
        &mut design,
        "detail-border-width",
        "Detail border width",
        2.0,
    );
    length_token(&mut design, "body-radius", "Body radius", 10.0);
    number_token(&mut design, "label-weight", "Label weight", 600.0);
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
            stops: vec![stop(0.0, "neon-cyan", 0.05), stop(0.7, "neon-cyan", 0.0)],
            repetition: None,
        }),
        screen_layer(grid_line(90.0)),
        screen_layer(grid_line(180.0)),
    ];
    for (id, label, color, sigma, opacity) in [
        (SOFT_CYAN_GLOW, "Soft cyan glow", "neon-cyan", 4.0, 0.5),
        (KEYWORD_GLOW, "Keyword glow", "neon-cyan", 6.0, 0.4),
        (EDGE_GLOW, "Edge glow", "neon-cyan", 6.0, 0.6),
        (NOTE_GLOW, "Note body glow", "neon-magenta", 8.0, 0.4),
        (LOOP_TEXT_GLOW, "Text glow", "neon-cyan", 5.0, 0.5),
        (NOTE_TEXT_GLOW, "Note text glow", "neon-magenta", 4.0, 0.4),
    ] {
        effect_token(
            &mut design,
            id,
            label,
            vec![shadow(color, 0.0, sigma, opacity)],
        );
    }
    effect_token(
        &mut design,
        SHAPE_GLOW,
        "Body glow",
        vec![
            shadow("neon-cyan", 0.0, 8.0, 0.5),
            shadow("neon-cyan", 0.0, 16.0, 0.3),
        ],
    );
    for (family, target, effect, width, radius, label_weight, paint) in [
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Loop,
            Some(SOFT_CYAN_GLOW),
            Some("detail-border-width"),
            false,
            false,
            true,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabelBackground,
            Some(KEYWORD_GLOW),
            Some("detail-border-width"),
            false,
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
            false,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            Some(EDGE_GLOW),
            Some("detail-border-width"),
            false,
            false,
            false,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::NodeLabel,
            Some(LOOP_TEXT_GLOW),
            None,
            false,
            true,
            false,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::EdgeLabel,
            Some(LOOP_TEXT_GLOW),
            None,
            false,
            true,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Actor,
            Some(SHAPE_GLOW),
            Some("body-border-width"),
            true,
            false,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::ActorLabel,
            Some(SHAPE_GLOW),
            None,
            false,
            false,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Message,
            Some(EDGE_GLOW),
            Some("detail-border-width"),
            false,
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
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            Some(EDGE_GLOW),
            Some("detail-border-width"),
            false,
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
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabel,
            Some(LOOP_TEXT_GLOW),
            None,
            false,
            false,
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::NoteLabel,
            Some(NOTE_TEXT_GLOW),
            None,
            false,
            false,
            false,
        ),
    ] {
        let mut style = if label_weight {
            weighted("label-weight")
        } else {
            ThemeDesignStyleV1::default()
        };
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
    append_xy_series(&mut design);
    append_xy_text(&mut design);
    let mut tick = style(None, Some("neon-cyan"));
    tick.opacity = SpecifiedWireV1::Value(literal(0.3));
    design.rules.push(rule(
        Some(DiagramFamilyId::XY_CHART),
        ThemeTarget::AxisTick,
        None,
        tick,
    ));
    design
}

fn append_xy_text(design: &mut ThemeDesignV1) {
    let mut legend = style(Some("neon-cyan"), None);
    legend.effect = SpecifiedWireV1::Value(reference(SOFT_CYAN_GLOW));
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
    // CSS text-shadow blur is twice the SVG Gaussian standard deviation.
    for (target, size, weight, sigma, opacity) in [
        (ThemeTarget::Title, Some(18.0), Some(700.0), 7.5, 0.8),
        (ThemeTarget::AxisTitle, Some(13.0), None, 5.0, 0.6),
        (ThemeTarget::AxisLabel, None, Some(600.0), 5.0, 0.5),
    ] {
        let effect_id = format!("xy-{}-glow", target.id());
        effect_token(
            design,
            &effect_id,
            &format!("Chart {} glow", target.id()),
            vec![shadow("neon-cyan", 0.0, sigma, opacity)],
        );
        let mut patch = style(Some("neon-cyan"), None);
        patch.effect = SpecifiedWireV1::Value(reference(&effect_id));
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
    for (offset, color) in ["#6cc6cb", "#c77dff", "#7ce38b"].into_iter().enumerate() {
        let color_id = format!("xy-series.{offset}");
        color_token(
            design,
            &color_id,
            &format!("Chart series {}", offset + 1),
            color,
        );
        for (kind, width, sigma, opacity) in [
            (ThemeVariant::Bar, 2.0, 8.0, 0.4),
            (ThemeVariant::Line, 3.0, 6.0, 0.5),
        ] {
            let effect_id = format!("xy-{}-{offset}-glow", kind.id());
            effect_token(
                design,
                &effect_id,
                &format!("Chart {} {} glow", kind.id(), offset + 1),
                vec![shadow(&color_id, 0.0, sigma, opacity)],
            );
            let mut patch = style(
                (kind == ThemeVariant::Bar).then_some(color_id.as_str()),
                Some(&color_id),
            );
            if kind == ThemeVariant::Bar {
                patch.fill_opacity = SpecifiedWireV1::Value(literal(0.2));
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
                    period: 3,
                    offset: offset as u32,
                },
            });
            design.rules.push(entry);
        }
    }
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
            stop(0.0, "neon-cyan", 0.03),
            stop(1.0 / 40.0, "neon-cyan", 0.03),
            stop(1.0 / 40.0, "neon-cyan", 0.0),
            stop(1.0, "neon-cyan", 0.0),
        ],
        repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
            width_px: 40.0,
            height_px: 40.0,
        }),
    }
}
