//! SVG emission for bounded shadow sequences; families own the painted terminals.

use super::{SvgOutput, escape_attr};
use crate::diagram_theme::{
    EffectColorSpace, EffectInput, SvgFilterRegion, SvgFilterUnits, SvgShadowEffect,
};

pub(super) fn write_theme_shadow_application(
    out: &mut impl SvgOutput,
    scoped_filter_id: &str,
    effect: &SvgShadowEffect,
    region: SvgFilterRegion,
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
    for (index, stage) in effect.stages().iter().enumerate() {
        let color = escape_attr(&stage.color.as_css());
        match effect.color_space() {
            EffectColorSpace::LinearRgb => {
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
                let _ = write!(
                    out,
                    r#"<feGaussianBlur in="{input}" stdDeviation="{}" result="merman-shadow-{index}-blur"/>"#,
                    stage.std_deviation
                );
                let mask = if stage.offset_x == 0.0 && stage.offset_y == 0.0 {
                    "blur"
                } else {
                    let _ = write!(
                        out,
                        r#"<feOffset in="merman-shadow-{index}-blur" dx="{}" dy="{}" result="merman-shadow-{index}-offset"/>"#,
                        stage.offset_x, stage.offset_y
                    );
                    "offset"
                };
                let _ = write!(
                    out,
                    concat!(
                        r#"<feFlood flood-color="{color}" result="merman-shadow-{index}-flood"/>"#,
                        r#"<feComposite in="merman-shadow-{index}-flood" in2="merman-shadow-{index}-{mask}" operator="in" result="merman-shadow-{index}-shadow"/>"#,
                        r#"<feMerge result="merman-shadow-{index}-result"><feMergeNode in="merman-shadow-{index}-shadow"/><feMergeNode in="{input}"/></feMerge>"#
                    ),
                    input = input,
                    index = index,
                    mask = mask,
                    color = color
                );
            }
        }
    }
    let _ = write!(out, "</filter></defs>");
    format!("url(#{scoped_filter_id})")
}
