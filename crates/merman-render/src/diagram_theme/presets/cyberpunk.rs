//! Warning-yellow lettering, cyan signals and red registration edges on dark technical panels.

use merman_theme_contract::*;

use super::catalog::{PresetPalette, build_cross_family_recipe};
use super::design::*;
use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

const PANEL_DEPTH: &str = "panel-depth";
const EDGE_GLOW: &str = "edge-glow";
const NOTE_GLOW: &str = "note-glow";
const LOOP_GLOW: &str = "loop-glow";

pub(super) fn build_recipe(palette: PresetPalette) -> ThemeDesignV1 {
    let mut design = build_cross_family_recipe(palette);
    // Git tags keep Mermaid's pale backing; other Git labels have separate surfaces.
    design.rules.push(rule(
        Some(DiagramFamilyId::GIT_GRAPH),
        ThemeTarget::NodeLabel,
        None,
        style(Some("category-text"), None),
    ));
    for (id, label, description, color) in [
        (
            "warning-yellow",
            "Warning yellow",
            "Panel names and outlines, chart marks and the canvas light field; readable on panels and exterior labels.",
            palette.border,
        ),
        (
            "neon-cyan",
            "Signal cyan",
            "Connections, lifelines, activations, registration depth, chart marks and fine grid lines.",
            palette.line,
        ),
        (
            "alert-red",
            "Alert red",
            "Note and loop outlines, red registration depth, chart marks and status accents.",
            palette.accent,
        ),
    ] {
        color_token(&mut design, id, label, "Signal colors", description, color);
    }
    for (id, source) in [
        ("border", "warning-yellow"),
        ("line", "neon-cyan"),
        ("accent", "alert-red"),
        ("actor-background", "surface"),
        ("actor-border", "warning-yellow"),
        ("actor-text", "warning-yellow"),
        ("activation-background", "neon-cyan"),
        ("activation-border", "neon-cyan"),
        ("note-border", "alert-red"),
    ] {
        design.tokens.get_mut(id).expect("shared color role").value =
            ThemeDesignTokenValueV1::Color(reference(source));
    }
    let actor_border = design
        .tokens
        .get_mut("actor-border")
        .expect("shared actor border");
    actor_border.description = Some(
        "Actor panel and glyph outlines; signal lifelines use the independent line role."
            .to_owned(),
    );
    for (id, label, description, value) in [
        (
            "body-border-width",
            "Panel border width",
            "Fine yellow contours on Flowchart nodes and sequence actors; cyan activation outlines.",
            1.5,
        ),
        (
            "detail-border-width",
            "Signal line width",
            "Line weight for connectors, lifelines, notes and loop details.",
            1.5,
        ),
        (
            "body-radius",
            "Panel radius",
            "Square supported Flowchart and sequence panels without changing source shape identity.",
            0.0,
        ),
    ] {
        length_token(&mut design, id, label, "Geometry", description, value);
    }
    number_token(
        &mut design,
        "label-weight",
        "Panel label weight",
        "Strong names on Flowchart and actor panels; lettering stays free of effects.",
        700.0,
    );
    number_token(
        &mut design,
        "secondary-label-weight",
        "Annotation weight",
        "Readable connector, message, loop and note labels on their independent backings.",
        500.0,
    );
    design.canvas.as_mut().expect("shared canvas").layers = vec![
        screen_layer(ThemeDesignPaintObjectV1::RadialGradient {
            center_x: literal(ThemeLengthWireV1::Percent { percent: 50.0 }),
            center_y: literal(ThemeLengthWireV1::Percent { percent: 50.0 }),
            radius: literal(ThemeLengthWireV1::Percent {
                percent: 100.0 / std::f32::consts::SQRT_2,
            }),
            stops: vec![
                stop(0.0, "warning-yellow", 0.04),
                stop(0.7, "warning-yellow", 0.0),
            ],
            repetition: None,
        }),
        screen_layer(grid_line(90.0)),
        screen_layer(grid_line(180.0)),
    ];
    signal_effect(
        &mut design,
        PANEL_DEPTH,
        "Signal registration",
        "Independent hard red left edge and cyan lower backing; the panel face is painted once.",
        vec![
            ThemeDesignShadowV1 {
                offset_x: px(-3.0),
                offset_y: px(0.0),
                ..shadow("alert-red", 0.0, 0.0, 0.8)
            },
            shadow("neon-cyan", 4.0, 0.0, 0.85),
        ],
    );
    for (id, label, description, color, sigma, opacity) in [
        (
            EDGE_GLOW,
            "Signal glow",
            "Fine cyan connection halo.",
            "neon-cyan",
            1.2,
            0.25,
        ),
        (
            NOTE_GLOW,
            "Note contour",
            "Red halo around the note body, independent of its lettering.",
            "alert-red",
            1.5,
            0.18,
        ),
        (
            LOOP_GLOW,
            "Loop contour",
            "Red loop boundary, independent of the loop's label backing.",
            "alert-red",
            1.0,
            0.14,
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
    for (family, target, fill, stroke, effect, width, radius) in [
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Node,
            Some("surface"),
            Some("border"),
            Some(PANEL_DEPTH),
            "body-border-width",
            true,
        ),
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::Edge,
            None,
            Some("line"),
            Some(EDGE_GLOW),
            "detail-border-width",
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Actor,
            Some("actor-background"),
            Some("actor-border"),
            Some(PANEL_DEPTH),
            "body-border-width",
            true,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Message,
            None,
            Some("line"),
            Some(EDGE_GLOW),
            "detail-border-width",
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            Some("line"),
            Some("line"),
            Some(EDGE_GLOW),
            "detail-border-width",
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Note,
            None,
            Some("note-border"),
            Some(NOTE_GLOW),
            "detail-border-width",
            true,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Loop,
            None,
            Some("alert-red"),
            Some(LOOP_GLOW),
            "detail-border-width",
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabelBackground,
            None,
            Some("border"),
            None,
            "detail-border-width",
            false,
        ),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Activation,
            None,
            Some("activation-border"),
            None,
            "body-border-width",
            false,
        ),
    ] {
        let mut patch = style(fill, stroke);
        if let Some(id) = effect {
            patch.effect = SpecifiedWireV1::Value(reference(id));
        }
        if radius {
            patch.radius = SpecifiedWireV1::Value(reference("body-radius"));
        }
        if let Some(ThemeValueV1::Literal(stroke)) = &mut patch.stroke {
            stroke.width = SpecifiedWireV1::Value(reference(width));
        }
        design.rules.push(rule(Some(family), target, None, patch));
    }
    for (family, target, ink, weight) in [
        (
            DiagramFamilyId::FLOWCHART,
            ThemeTarget::NodeLabel,
            "warning-yellow",
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
    design.rules.push(rule(
        Some(DiagramFamilyId::ZENUML),
        ThemeTarget::Lifeline,
        None,
        style(Some("line"), Some("line")),
    ));
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
    for (target, ink, size, weight) in [
        (
            ThemeTarget::Title,
            "warning-yellow",
            Some(18.0),
            Some(700.0),
        ),
        (
            ThemeTarget::AxisTitle,
            "subtle-text",
            Some(13.0),
            Some(500.0),
        ),
        (ThemeTarget::AxisLabel, "subtle-text", None, Some(500.0)),
        (ThemeTarget::Legend, "subtle-text", Some(12.0), None),
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
    let colors = ["warning-yellow", "neon-cyan", "alert-red"];
    for (offset, color) in colors.into_iter().enumerate() {
        let color_id = format!("xy-series.{offset}");
        token(
            design,
            &color_id,
            &format!("Chart series {}", offset + 1),
            "Series palette",
            "Repeating signal ink for chart marks and matching legend swatches.",
            ThemeDesignTokenValueV1::Color(reference(color)),
        );
        for (kind, width) in [(ThemeVariant::Bar, 1.25), (ThemeVariant::Line, 2.0)] {
            let effect_id = format!("xy-{}-{offset}-glow", kind.id());
            effect_token(
                design,
                &effect_id,
                &format!("Chart {} {} signal", kind.id(), offset + 1),
                "Fine series halo, independent of chart text.",
                vec![shadow(&color_id, 0.0, 1.0, 0.18)],
            );
            let mut patch = style(
                (kind == ThemeVariant::Bar).then_some(color_id.as_str()),
                Some(&color_id),
            );
            if kind == ThemeVariant::Bar {
                patch.fill_opacity = SpecifiedWireV1::Value(literal(0.85));
            }
            if let Some(ThemeValueV1::Literal(stroke)) = &mut patch.stroke {
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
        "Chart signals",
        "Series palette",
        "Three repeating chart signals, independent of the twelve readable filled categories and Pie sectors.",
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

fn signal_effect(
    design: &mut ThemeDesignV1,
    id: &str,
    label: &str,
    description: &str,
    shadows: Vec<ThemeDesignShadowV1>,
) {
    token(
        design,
        id,
        label,
        "Effects",
        description,
        ThemeDesignTokenValueV1::Effect(literal(ThemeDesignEffectV1::ShadowStack {
            composition: ThemeShadowCompositionV1::Independent,
            color_space: Some("srgb".to_owned()),
            shadows,
        })),
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
            stop(0.0, "neon-cyan", 0.035),
            stop(1.0 / 40.0, "neon-cyan", 0.035),
            stop(1.0 / 40.0, "neon-cyan", 0.0),
            stop(1.0, "neon-cyan", 0.0),
        ],
        repetition: Some(ThemeLinearGradientRepetitionWireV1::Tiled {
            width_px: 40.0,
            height_px: 40.0,
        }),
    }
}
