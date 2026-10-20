//! Explicit authoring helpers for built-in designs. Roles are declared before use, never inferred
//! from equal resolved colors or reverse-engineered from compiled specifications.

use merman_theme_contract::*;

use crate::DiagramFamilyId;
use crate::diagram_theme::{ThemeTarget, ThemeVariant};

pub(super) fn literal<T>(value: T) -> ThemeValueV1<T> {
    ThemeValueV1::Literal(value)
}

pub(super) fn reference<T>(id: &str) -> ThemeValueV1<T> {
    ThemeValueV1::Ref(ThemeReferenceV1 {
        r#ref: id.to_owned(),
    })
}

pub(super) fn token(
    design: &mut ThemeDesignV1,
    id: &str,
    label: &str,
    group: &str,
    description: &str,
    value: ThemeDesignTokenValueV1,
) {
    design.tokens.insert(
        id.to_owned(),
        ThemeDesignTokenV1 {
            label: Some(label.to_owned()),
            description: Some(description.to_owned()),
            group: Some(group.to_owned()),
            value,
        },
    );
}

pub(super) fn color_token(
    design: &mut ThemeDesignV1,
    id: &str,
    label: &str,
    group: &str,
    description: &str,
    value: &str,
) {
    token(
        design,
        id,
        label,
        group,
        description,
        ThemeDesignTokenValueV1::Color(literal(value.to_owned())),
    );
}

pub(super) fn length_token(
    design: &mut ThemeDesignV1,
    id: &str,
    label: &str,
    group: &str,
    description: &str,
    value: f32,
) {
    token(
        design,
        id,
        label,
        group,
        description,
        ThemeDesignTokenValueV1::Length(px(value)),
    );
}

pub(super) fn number_token(
    design: &mut ThemeDesignV1,
    id: &str,
    label: &str,
    description: &str,
    value: f32,
) {
    token(
        design,
        id,
        label,
        "Typography",
        description,
        ThemeDesignTokenValueV1::Number(literal(value)),
    );
}

pub(super) fn px(value: f32) -> ThemeValueV1<ThemeLengthWireV1> {
    literal(ThemeLengthWireV1::PxValue(value))
}

pub(super) fn paint(id: &str) -> ThemeValueV1<ThemeDesignPaintV1> {
    literal(ThemeDesignPaintV1::Structured(
        ThemeDesignPaintObjectV1::Solid {
            color: reference(id),
        },
    ))
}

pub(super) fn style(fill: Option<&str>, stroke: Option<&str>) -> ThemeDesignStyleV1 {
    ThemeDesignStyleV1 {
        fill: fill.map_or(SpecifiedWireV1::Unspecified, |id| {
            SpecifiedWireV1::Value(paint(id))
        }),
        stroke: stroke.map(|id| {
            literal(ThemeDesignStrokeV1 {
                paint: SpecifiedWireV1::Value(paint(id)),
                ..Default::default()
            })
        }),
        ..Default::default()
    }
}

pub(super) fn rule(
    family: Option<DiagramFamilyId>,
    target: ThemeTarget,
    variant: Option<ThemeVariant>,
    style: ThemeDesignStyleV1,
) -> ThemeDesignRuleV1 {
    ThemeDesignRuleV1 {
        target: target.id().to_owned(),
        family: family.map(|value| value.as_str().to_owned()),
        variant: variant.map(|value| value.id().to_owned()),
        ordinal: None,
        style,
    }
}

pub(super) fn weighted(id: &str) -> ThemeDesignStyleV1 {
    ThemeDesignStyleV1 {
        typography: Some(literal(ThemeDesignTextStyleV1 {
            font_weight: SpecifiedWireV1::Value(reference(id)),
            ..Default::default()
        })),
        ..Default::default()
    }
}

pub(super) fn shadow(color: &str, offset: f32, sigma: f32, opacity: f32) -> ThemeDesignShadowV1 {
    ThemeDesignShadowV1 {
        offset_x: px(offset),
        offset_y: px(offset),
        std_deviation: px(sigma),
        color: reference(color),
        opacity: Some(literal(opacity)),
    }
}

pub(super) fn effect_token(
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
            composition: ThemeShadowCompositionV1::Cascade,
            color_space: Some("srgb".to_owned()),
            shadows,
        })),
    );
}

pub(super) fn stop(offset: f32, color: &str, opacity: f32) -> ThemeDesignGradientStopV1 {
    ThemeDesignGradientStopV1 {
        offset: literal(offset),
        color: reference(color),
        opacity: Some(literal(opacity)),
    }
}
