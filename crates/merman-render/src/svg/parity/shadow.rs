//! SVG emission for bounded shadow sequences; families own the painted terminals.

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
) {
    let color = escape_attr(&color.as_css());
    let _ = write!(
        out,
        r#"<feGaussianBlur in="{input}" stdDeviation="{}" result="merman-shadow-{index}-blur"/>"#,
        std_deviation,
    );
    let mask = if offset == [0.0, 0.0] {
        "blur"
    } else {
        let _ = write!(
            out,
            r#"<feOffset in="merman-shadow-{index}-blur" dx="{}" dy="{}" result="merman-shadow-{index}-offset"/>"#,
            offset[0], offset[1],
        );
        "offset"
    };
    let _ = write!(
        out,
        r#"<feFlood flood-color="{color}" result="merman-shadow-{index}-flood"/><feComposite in="merman-shadow-{index}-flood" in2="merman-shadow-{index}-{mask}" operator="in" result="merman-shadow-{index}-shadow"/>"#,
    );
}
