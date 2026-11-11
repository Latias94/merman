//! SVG emission for bounded shadow sequences; families own the painted terminals.

use std::borrow::Cow;

use super::{SvgOutput, escape_attr};
use crate::diagram_theme::{
    EffectColorSpace, EffectComposition, EffectInput, SvgFilterRegion, SvgFilterUnits,
    SvgShadowEffect, ThemeColorValue,
};

pub(super) fn write_theme_shadow_application(
    out: &mut impl SvgOutput,
    scoped_filter_id: &str,
    effect: &SvgShadowEffect,
    region: SvgFilterRegion,
) -> String {
    write_theme_shadow_filter(out, scoped_filter_id, effect, region, true)
}

/// Paint only body depth; the owning family paints SourceGraphic once above its edges.
pub(super) fn write_theme_shadow_depth_application(
    out: &mut impl SvgOutput,
    scoped_filter_id: &str,
    effect: &SvgShadowEffect,
    region: SvgFilterRegion,
) -> String {
    write_theme_shadow_filter(out, scoped_filter_id, effect, region, false)
}

fn write_theme_shadow_filter(
    out: &mut impl SvgOutput,
    scoped_filter_id: &str,
    effect: &SvgShadowEffect,
    region: SvgFilterRegion,
    include_source: bool,
) -> String {
    let [x, y, width, height] = region.as_array();
    let id = escape_attr(scoped_filter_id);
    let filter_units = match region.units() {
        SvgFilterUnits::ObjectBoundingBox => "objectBoundingBox",
        SvgFilterUnits::UserSpaceOnUse => "userSpaceOnUse",
    };
    let _ = write!(
        out,
        r#"<defs><filter id="{}" filterUnits="{}" x="{}" y="{}" width="{}" height="{}" color-interpolation-filters="{}">"#,
        id,
        filter_units,
        x,
        y,
        width,
        height,
        effect.color_space().svg_name()
    );
    if effect.composition() == EffectComposition::Independent {
        write_independent_shadows(out, effect, include_source);
    } else if !include_source {
        write_cascade_depth(out, effect);
    } else {
        for (index, stage) in effect.stages().iter().enumerate() {
            match effect.color_space() {
                EffectColorSpace::LinearRgb => {
                    let color = escape_attr(&stage.color.as_css());
                    let _ = write!(out, "<feDropShadow");
                    // Omitted SVG `in` selects the preceding primitive. `Previous` is not a keyword.
                    if stage.input == EffectInput::SourceGraphic {
                        let _ = write!(out, r#" in="SourceGraphic""#);
                    }
                    let _ = write!(
                        out,
                        r#" dx="{}" dy="{}" stdDeviation="{}" flood-color="{}"/>"#,
                        stage.offset_x, stage.offset_y, stage.std_deviation, color
                    );
                }
                EffectColorSpace::Srgb => {
                    // The standard DropShadow expansion keeps flood pixels in sRGB. The pinned
                    // native backend otherwise applies gamma conversion twice in feDropShadow.
                    let input = match stage.input {
                        EffectInput::SourceGraphic => "SourceGraphic".to_owned(),
                        EffectInput::Previous => format!("merman-shadow-{}-result", index - 1),
                    };
                    write_shadow_stage(
                        out,
                        index,
                        &input,
                        [stage.offset_x, stage.offset_y],
                        stage.std_deviation,
                        &stage.color,
                        effect.color_space(),
                    );
                    let _ = write!(
                        out,
                        r#"<feMerge result="merman-shadow-{index}-result"><feMergeNode in="merman-shadow-{index}-shadow"/><feMergeNode in="{input}"/></feMerge>"#,
                    );
                }
            }
        }
    }
    let _ = write!(out, "</filter></defs>");
    format!("url(#{scoped_filter_id})")
}

fn write_independent_shadows(
    out: &mut impl SvgOutput,
    effect: &SvgShadowEffect,
    include_source: bool,
) {
    for (index, stage) in effect.stages().iter().enumerate() {
        write_shadow_stage(
            out,
            index,
            "SourceAlpha",
            [stage.offset_x, stage.offset_y],
            stage.std_deviation,
            &stage.color,
            effect.color_space(),
        );
    }
    let _ = write!(out, r#"<feMerge result="merman-shadow-stack-result">"#);
    for index in 0..effect.stages().len() {
        let _ = write!(out, r#"<feMergeNode in="merman-shadow-{index}-shadow"/>"#);
    }
    if include_source {
        out.push_str(r#"<feMergeNode in="SourceGraphic"/>"#);
    }
    out.push_str("</feMerge>");
}

fn write_cascade_depth(out: &mut impl SvgOutput, effect: &SvgShadowEffect) {
    let mut first_live_shadow = 0;
    for (index, stage) in effect.stages().iter().enumerate() {
        let input = match stage.input {
            EffectInput::SourceGraphic => {
                first_live_shadow = index;
                "SourceGraphic".to_owned()
            }
            EffectInput::Previous => format!("merman-shadow-{}-result", index - 1),
        };
        write_shadow_stage(
            out,
            index,
            &input,
            [stage.offset_x, stage.offset_y],
            stage.std_deviation,
            &stage.color,
            effect.color_space(),
        );
        // Previous consumes the complete previous result, including the source's alpha.
        // Keep that intermediate off-screen; only the live shadows reach the final merge.
        if index + 1 < effect.stages().len() {
            let _ = write!(
                out,
                r#"<feMerge result="merman-shadow-{index}-result"><feMergeNode in="merman-shadow-{index}-shadow"/><feMergeNode in="{input}"/></feMerge>"#,
            );
        }
    }
    out.push_str(r#"<feMerge result="merman-shadow-stack-result">"#);
    // A drop shadow sits behind its input. A SourceGraphic stage resets the live stack.
    for index in (first_live_shadow..effect.stages().len()).rev() {
        let _ = write!(out, r#"<feMergeNode in="merman-shadow-{index}-shadow"/>"#);
    }
    out.push_str("</feMerge>");
}

fn write_shadow_stage(
    out: &mut impl SvgOutput,
    index: usize,
    input: &str,
    offset: [f32; 2],
    std_deviation: f32,
    color: &ThemeColorValue,
    color_space: EffectColorSpace,
) {
    // Zero sigma and zero translation are identities, including signed zero. Always name
    // the surviving input explicitly: the previous primitive may belong to another stage.
    let mut mask = Cow::Borrowed(input);
    if std_deviation != 0.0 {
        let _ = write!(
            out,
            r#"<feGaussianBlur in="{mask}" stdDeviation="{std_deviation}" result="merman-shadow-{index}-blur"/>"#,
        );
        mask = Cow::Owned(format!("merman-shadow-{index}-blur"));
    }
    if offset != [0.0, 0.0] {
        let _ = write!(
            out,
            r#"<feOffset in="{mask}" dx="{}" dy="{}" result="merman-shadow-{index}-offset"/>"#,
            offset[0], offset[1],
        );
        mask = Cow::Owned(format!("merman-shadow-{index}-offset"));
    }

    let color_css = color.as_css();
    if color_space == EffectColorSpace::Srgb
        && let Some([red, green, blue]) = opaque_srgb_channels(&color_css)
    {
        // An opaque flood masked with `in` is (constant RGB, mask alpha). A color
        // matrix expresses that in one primitive without painting the source. Use the
        // same parsed sRGB bytes as feFlood; keep translucent floods' premultiplication
        // and linearRGB's color conversion in the existing two-primitive expansion.
        let _ = write!(
            out,
            r#"<feColorMatrix in="{mask}" type="matrix" values="0 0 0 0 {red} 0 0 0 0 {green} 0 0 0 0 {blue} 0 0 0 1 0" result="merman-shadow-{index}-shadow"/>"#,
        );
        return;
    }

    let color = escape_attr(&color_css);
    let _ = write!(
        out,
        r#"<feFlood flood-color="{color}" result="merman-shadow-{index}-flood"/><feComposite in="merman-shadow-{index}-flood" in2="{mask}" operator="in" result="merman-shadow-{index}-shadow"/>"#,
    );
}

fn opaque_srgb_channels(css: &str) -> Option<[f64; 3]> {
    let source = merman_markup::theme_color::ThemeColor::parse(css)
        .ok()?
        .rgba_channels();
    let native = css.parse::<svgtypes::Color>().ok()?;
    // Native floods use byte colors. Require exact agreement with the CSS channels,
    // rather than rounding fractional RGB/HSL or nearly opaque alpha into this path.
    if source.alpha != 1.0
        || native.alpha != u8::MAX
        || source.red != f64::from(native.red)
        || source.green != f64::from(native.green)
        || source.blue != f64::from(native.blue)
    {
        return None;
    }
    Some([native.red, native.green, native.blue].map(|channel| f64::from(channel) / 255.0))
}
