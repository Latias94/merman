use merman_model::presentation::PresentationConfig;

use super::{
    RailroadStyle, is_valid_color_value, parse_number_value, railroad_config_value,
    sanitize_font_family_value, theme_config_value, trim_ecmascript_whitespace,
};
use crate::theme::{Color, FamilyTheme, font_family_css};

#[derive(Clone, Debug)]
pub(crate) struct RailroadNativeTypography {
    pub(crate) terminal_font_family: Option<String>,
    pub(crate) rule_name_font_family: Option<String>,
    pub(crate) title_weight: u16,
}

pub(super) fn apply(style: &mut RailroadStyle, theme: &FamilyTheme, config: &PresentationConfig) {
    let roles = &theme.roles;
    let border = roles.stroke_border.first_color();
    let connector = roles.stroke_connector.first_color();
    let node = roles.surface_node.first_color();
    let primary = roles.text_primary;
    for (target, key, variables, color) in [
        (
            &mut style.terminal_fill,
            "terminalFill",
            &["secondBkg", "secondaryColor"][..],
            roles.surface_emphasis.first_color(),
        ),
        (
            &mut style.terminal_stroke,
            "terminalStroke",
            &["secondaryBorderColor", "lineColor"][..],
            border,
        ),
        (
            &mut style.terminal_text_color,
            "terminalTextColor",
            &["secondaryTextColor", "textColor"][..],
            roles.text_on_emphasis,
        ),
        (
            &mut style.non_terminal_fill,
            "nonTerminalFill",
            &["mainBkg", "background"][..],
            node,
        ),
        (
            &mut style.non_terminal_stroke,
            "nonTerminalStroke",
            &["primaryBorderColor", "lineColor"][..],
            border,
        ),
        (
            &mut style.non_terminal_text_color,
            "nonTerminalTextColor",
            &["primaryTextColor", "textColor"][..],
            primary,
        ),
        (
            &mut style.line_color,
            "lineColor",
            &["lineColor"][..],
            connector,
        ),
        (
            &mut style.marker_fill,
            "markerFill",
            &["lineColor"][..],
            connector,
        ),
        (
            &mut style.comment_fill,
            "commentFill",
            &["labelBackground", "tertiaryColor"][..],
            node,
        ),
        (
            &mut style.comment_stroke,
            "commentStroke",
            &["tertiaryBorderColor", "lineColor"][..],
            border,
        ),
        (
            &mut style.comment_text_color,
            "commentTextColor",
            &["tertiaryTextColor", "textColor"][..],
            roles.text_muted,
        ),
        (
            &mut style.special_fill,
            "specialFill",
            &["tertiaryColor", "secondaryColor"][..],
            node,
        ),
        (
            &mut style.special_stroke,
            "specialStroke",
            &["tertiaryBorderColor", "secondaryBorderColor"][..],
            border,
        ),
        (
            &mut style.rule_name_color,
            "ruleNameColor",
            &["titleColor", "textColor"][..],
            roles.text_title,
        ),
    ] {
        *target = resolve_color(config, key, variables, color);
    }

    let typography = &theme.typography;
    let raw = config.as_value();
    let authored_family = [
        (
            "railroad.fontFamily",
            railroad_config_value(raw, "fontFamily"),
        ),
        (
            "themeVariables.fontFamily",
            theme_config_value(raw, "fontFamily", None),
        ),
        ("fontFamily", config.get("fontFamily")),
    ]
    .into_iter()
    .filter(|(path, _)| config.explicit_config_owns_path(path))
    .map(|(_, value)| sanitize_font_family_value(value, ""))
    .find(|font| !font.is_empty());
    let use_native_stacks = authored_family.is_none();
    style.font_family = authored_family
        .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)));
    style.font_size = [
        (
            "railroad.fontSize",
            railroad_config_value(raw, "fontSize"),
            true,
        ),
        (
            "themeVariables.fontSize",
            theme_config_value(raw, "fontSize", None),
            false,
        ),
        ("fontSize", config.get("fontSize"), false),
    ]
    .into_iter()
    .filter(|(path, _, _)| config.explicit_config_owns_path(path))
    .find_map(|(_, value, zero_is_valid)| {
        parse_number_value(value)
            .filter(|size| size.is_finite() && (*size > 0.0 || (zero_is_valid && *size == 0.0)))
    })
    .unwrap_or(typography.base_size_px);
    style.native_typography = Some(RailroadNativeTypography {
        terminal_font_family: use_native_stacks
            .then(|| font_family_css(typography.stack(typography.code_family))),
        rule_name_font_family: use_native_stacks
            .then(|| font_family_css(typography.stack(typography.title_family))),
        title_weight: typography.title_weight,
    });

    style.terminal_radius = theme.form.radius;
    let width_is_authored = config.explicit_config_owns_path("railroad.strokeWidth")
        && parse_number_value(railroad_config_value(raw, "strokeWidth"))
            .is_some_and(|width| width.is_finite() && width >= 0.0);
    if !width_is_authored {
        style.stroke_width = theme.form.stroke_width;
        style.connector_width = Some(theme.form.connector_width);
    }
}

fn resolve_color(
    config: &PresentationConfig,
    key: &str,
    variables: &[&str],
    fallback: Color,
) -> String {
    let authored_color = |path: &str| {
        config
            .get_str(path)
            .filter(|_| config.explicit_config_owns_path(path))
            .map(trim_ecmascript_whitespace)
            .filter(|value| is_valid_color_value(value))
    };
    authored_color(&format!("railroad.{key}"))
        .or_else(|| {
            variables
                .iter()
                .find_map(|variable| authored_color(&format!("themeVariables.{variable}")))
        })
        .map_or_else(|| fallback.to_string(), str::to_owned)
}
