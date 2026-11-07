use crate::diagram_theme::{
    DiagramThemeCompiler, DiagramThemeSpec, FontStack, FontStyle, LineHeight, Specified, TextAlign,
    TextDecoration, TextTransform, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, ThemeTypographyProperty, ThemeVariant, ThemeWrapMode, TypographySpec,
    WhiteSpace,
};
use merman_model::family::DiagramFamilyId;
use merman_theme_contract::ThemeRecipeV1;
use serde_json::json;

#[test]
fn font_stack_quotes_names_that_are_not_valid_unquoted_css_identifiers() {
    let stack = FontStack::new([
        "123Radar",
        "-1Radar",
        "-",
        "Radar2",
        "-apple-system",
        "--radar",
    ])
    .expect("valid bounded font stack");

    assert_eq!(
        stack.as_css(),
        "\"123Radar\", \"-1Radar\", \"-\", Radar2, -apple-system, --radar"
    );
}

fn shared_typography() -> ThemeTextStyle {
    ThemeTextStyle::default()
        .with_font_stack(FontStack::single("serif").unwrap())
        .with_font_size_px(25.0)
        .unwrap()
        .with_font_weight(600)
        .unwrap()
        .with_font_style(FontStyle::Italic)
        .with_line_height(LineHeight::Multiplier(1.5))
        .unwrap()
        .with_letter_spacing_px(1.0)
        .unwrap()
        .with_word_spacing_px(2.0)
        .unwrap()
        .with_transform(TextTransform::Uppercase)
        .with_decoration(TextDecoration::Underline)
        .with_text_align(TextAlign::Center)
        .with_white_space(WhiteSpace::PreWrap)
        .with_wrap(ThemeWrapMode::Anywhere)
}

#[test]
fn family_typography_inherits_unspecified_defaults_independently_of_builder_order() {
    let shared = shared_typography();
    let family_style =
        ThemeTextStyle::default().with_font_stack(FontStack::single("monospace").unwrap());
    let expected = shared
        .clone()
        .with_font_stack(FontStack::single("monospace").unwrap());
    for &family in DiagramFamilyId::all() {
        for default_first in [true, false] {
            let typography = if default_first {
                TypographySpec::default()
                    .with_default(shared.clone())
                    .with_family_style(family, family_style.clone())
            } else {
                TypographySpec::default()
                    .with_family_style(family, family_style.clone())
                    .with_default(shared.clone())
            };
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_typography(typography))
                .unwrap();
            assert_eq!(theme.resolve(family).typography(), &expected, "{family}");
        }
    }
}

#[test]
fn family_typography_preserves_explicit_defaults_and_role_clear_restores_inherited_values() {
    let shared = shared_typography();
    let family_style = ThemeTextStyle::default()
        .with_font_size_px(16.0)
        .unwrap()
        .with_font_weight(400)
        .unwrap();
    let expected = shared
        .clone()
        .with_font_size_px(16.0)
        .unwrap()
        .with_font_weight(400)
        .unwrap();
    let mut patch = ThemeStylePatch::default();
    patch.typography.font_size_px = Specified::Clear;
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default()
                        .with_default(shared.clone())
                        .with_family_style(DiagramFamilyId::CLASS, family_style)
                        .with_family_style(DiagramFamilyId::SEQUENCE, ThemeTextStyle::default()),
                )
                .with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Text, patch)),
                ),
        )
        .unwrap();
    let class = theme.resolve(DiagramFamilyId::CLASS);
    assert_eq!(class.typography(), &expected);
    assert_eq!(
        class
            .style(ThemeTarget::Text, ThemeVariant::Default, None)
            .typography(),
        &expected
    );
    assert_eq!(
        theme.resolve(DiagramFamilyId::SEQUENCE).typography(),
        &shared
    );
    assert_eq!(
        theme.resolve(DiagramFamilyId::FLOWCHART).typography(),
        &shared
    );
}

#[test]
fn editable_and_complete_recipes_share_family_typography_inheritance() {
    let typography = json!({
        "default": {"font_size_px": 25, "font_weight": 600},
        "families": {"class": {"font_stack": ["monospace"]}}
    });
    for recipe in [
        json!({"schema_version": 1, "kind": "design", "design": {
            "typography": typography.clone()
        }}),
        json!({"schema_version": 1, "kind": "complete_spec", "complete_spec": {
            "typography": typography
        }}),
    ] {
        let recipe: ThemeRecipeV1 = serde_json::from_value(recipe).unwrap();
        let theme = DiagramThemeCompiler::new().compile_recipe(recipe).unwrap();
        let class = theme.resolve(DiagramFamilyId::CLASS);
        assert_eq!(class.typography().font_size_px(), 25.0);
        assert_eq!(class.typography().font_weight(), 600);
        assert_eq!(
            class.typography().font_stack().families(),
            &["monospace".to_owned()]
        );
        assert_eq!(
            theme
                .resolve(DiagramFamilyId::SEQUENCE)
                .typography()
                .font_size_px(),
            25.0
        );
    }
}

#[test]
fn family_null_removes_its_override_without_pinning_renderer_defaults() {
    for (family_style, expected_size) in [
        (json!({}), 25.0),
        (json!({"font_size_px": null}), 25.0),
        (json!({"font_size_px": 16}), 16.0),
    ] {
        let recipe = serde_json::from_value(json!({
            "schema_version": 1, "kind": "design", "design": {
                "typography": {
                    "default": {"font_size_px": 25, "font_weight": null},
                    "families": {"class": family_style}
                }
            }
        }))
        .unwrap();
        let theme = DiagramThemeCompiler::new().compile_recipe(recipe).unwrap();
        let class = theme.resolve(DiagramFamilyId::CLASS);
        assert_eq!(class.typography().font_size_px(), expected_size);
        assert!(
            !class
                .typography()
                .is_specified(ThemeTypographyProperty::FontWeight)
        );
    }
}

#[test]
fn role_clear_restores_the_shared_value_inherited_by_the_family() {
    let mut override_patch = ThemeStylePatch::default();
    override_patch.typography.font_size_px = Specified::Value(40.0);
    let mut clear_patch = ThemeStylePatch::default();
    clear_patch.typography.font_size_px = Specified::Clear;
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default()
                        .with_default(ThemeTextStyle::default().with_font_size_px(25.0).unwrap())
                        .with_family_style(
                            DiagramFamilyId::CLASS,
                            ThemeTextStyle::default()
                                .with_font_stack(FontStack::single("monospace").unwrap()),
                        ),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::Text, override_patch))
                        .with_rule(ThemeRule::new(ThemeTarget::Text, clear_patch)),
                ),
        )
        .unwrap();
    let class = theme.resolve(DiagramFamilyId::CLASS);
    assert_eq!(
        class
            .style(ThemeTarget::Text, ThemeVariant::Default, None)
            .typography()
            .font_size_px(),
        25.0,
    );
}
