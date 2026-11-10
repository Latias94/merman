use std::collections::BTreeMap;
use std::sync::OnceLock;

use merman_model::presentation::PresentationConfig;

use crate::config::configured_theme_font_size_css_or_root_number_px;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};
use crate::mermaid_style::{
    CssFontFamilyOwnership, css_font_size_value_override, parse_style_declaration,
};
use crate::text::TextStyle;

mod font_size;
use font_size::C4ElementFontSize;

/// Resolves C4's inherited base typography for the diagram title and root stylesheet.
///
/// C4 roles keep their independent `c4.*Font*` owners. Inheriting font families resolve against
/// the root for measurement while retaining their configured CSS spelling for the writer.
#[derive(Debug)]
pub(crate) struct C4TypographyThemePlan {
    common_css: crate::svg::PreparedCommonCss,
    person_border: String,
    person_background: String,
    element_styles: BTreeMap<String, C4ElementStyle>,
    boundary_fonts: C4BoundaryFonts,
    message_font: C4PreparedFont,
    relationship_text_color: String,
    relationship_line_color: String,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_px: f64,
    font_size_known: bool,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    title: Option<Box<str>>,
    terminal_seal: OnceLock<C4TypographyTerminalSeal>,
}

#[derive(Debug)]
pub(crate) struct C4ElementStyle {
    pub(crate) font: C4PreparedFont,
    source_colors: Option<C4ElementColors>,
}

#[derive(Debug)]
pub(crate) struct C4PreparedFont {
    pub(crate) measurement_style: TextStyle,
    pub(crate) css_font_family: String,
    pub(crate) css_font_size_px: Option<f64>,
    family_ownership: CssFontFamilyOwnership,
    font_size_inherits_root: bool,
    element_font_size: Option<C4ElementFontSize>,
    root_size_known: bool,
}

impl C4PreparedFont {
    fn resolve(
        mut font: TextStyle,
        root_font_family: &str,
        root_font_size_px: f64,
        root_size_known: bool,
        emit_font_size: bool,
    ) -> Self {
        let css_font_family = font
            .font_family
            .clone()
            .unwrap_or_else(|| super::C4_DEFAULT_FONT_FAMILY.to_owned());
        let declaration = if css_font_family.is_empty() {
            // An empty property value is ignored by CSS, leaving inherited typography.
            String::new()
        } else {
            format!("font-family:{css_font_family}")
        };
        let family_ownership =
            crate::mermaid_style::css_font_family_ownership([declaration.as_str()]);
        if family_ownership == CssFontFamilyOwnership::Inherited {
            font.font_family = Some(root_font_family.to_owned());
        }
        // CSS rejects a negative font-size declaration. Legacy rows keep its raw spelling;
        // element CSSOM rules can omit it. Both inherit the diagram root's used size.
        let css_font_size_px = emit_font_size.then_some(font.font_size);
        let font_size_inherits_root = css_font_size_px.is_none_or(|size| size < 0.0);
        if font_size_inherits_root {
            font.font_size = root_font_size_px;
        }
        Self {
            measurement_style: font,
            css_font_family,
            css_font_size_px,
            family_ownership,
            font_size_inherits_root,
            element_font_size: None,
            root_size_known,
        }
    }

    fn resolve_element(
        font: TextStyle,
        root_font_family: &str,
        root_font_size_px: f64,
        root_size_known: bool,
        value: Option<&serde_json::Value>,
    ) -> Self {
        let size = C4ElementFontSize::resolve(value, root_size_known.then_some(root_font_size_px));
        let mut prepared = Self::resolve(
            font,
            root_font_family,
            root_font_size_px,
            root_size_known,
            false,
        );
        prepared.measurement_style.font_size = size.used_px.unwrap_or(root_font_size_px);
        prepared.font_size_inherits_root = size.inherits_root;
        prepared.element_font_size = Some(size);
        prepared
    }

    pub(crate) fn element_font_size_css(&self) -> Option<&str> {
        self.element_font_size.as_ref()?.css.as_deref()
    }

    pub(crate) fn font_size_is_known(&self) -> bool {
        self.element_font_size.as_ref().map_or(
            !self.font_size_inherits_root || self.root_size_known,
            |size| size.used_px.is_some(),
        )
    }

    pub(crate) fn potentially_visible(&self) -> bool {
        !self.font_size_is_known() || self.measurement_style.font_size > 0.0
    }
}

#[derive(Debug)]
pub(crate) struct C4BoundaryFonts {
    pub(crate) name: C4PreparedFont,
    pub(crate) ty: C4PreparedFont,
    pub(crate) descr: C4PreparedFont,
}

impl C4BoundaryFonts {
    fn resolve(
        font: TextStyle,
        root_font_family: &str,
        root_font_size_px: f64,
        root_size_known: bool,
    ) -> Self {
        // Apply Mermaid's row arithmetic to the configured size before CSS validation. Resolving
        // a negative base first would incorrectly turn a -1px name into root + 2px, not 1px.
        let mut name = font.clone();
        name.font_size += 2.0;
        name.font_weight = Some("bold".to_owned());
        let mut descr = font.clone();
        descr.font_size -= 2.0;
        Self {
            name: C4PreparedFont::resolve(
                name,
                root_font_family,
                root_font_size_px,
                root_size_known,
                true,
            ),
            ty: C4PreparedFont::resolve(
                font,
                root_font_family,
                root_font_size_px,
                root_size_known,
                true,
            ),
            descr: C4PreparedFont::resolve(
                descr,
                root_font_family,
                root_font_size_px,
                root_size_known,
                true,
            ),
        }
    }
}

#[derive(Debug)]
pub(super) struct C4ElementColors {
    pub(super) background: String,
    pub(super) border: String,
    pub(super) text: String,
}

#[derive(Debug, Clone, Copy)]
struct C4TypographyTerminalSeal {
    has_visible_title: bool,
    has_visible_inherited_font_family: bool,
    has_visible_inherited_font_size: bool,
    has_unverified_visible_font_family: bool,
    has_unverified_visible_font_size: bool,
}

/// Writer-owned proof for C4's stylesheet and actual visible inherited text consumers.
#[derive(Debug)]
pub(crate) struct C4TypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_font_size_css: &'a str,
    expected_title: Option<&'a str>,
    expected_font_size_px: f64,
    root_size_known: bool,
    css_emitted: bool,
    css_matches: bool,
    title_seen: bool,
    has_visible_inherited_font_family: bool,
    has_visible_inherited_font_size: bool,
    has_unverified_visible_font_family: bool,
    has_unverified_visible_font_size: bool,
    valid: bool,
}

impl C4TypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &PresentationConfig,
        title: Option<&str>,
        model: &merman_model::families::c4::C4DiagramModel,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        // The theme-variable path is the typed base-size owner. A top-level `fontSize` remains a
        // Mermaid-compatible fallback and must not shadow an active typed value.
        let configured_font_size =
            configured_theme_font_size_css_or_root_number_px(effective_config.as_value());
        let configured_css = effective_config
            .get_str("themeVariables.fontSize")
            .filter(|raw| {
                // Root CSS accepts a trailing declaration terminator; element CSSOM does not.
                parse_style_declaration(&format!("font-size:{raw}")).is_some_and(|declaration| {
                    css_font_size_value_override(declaration.source_value()).is_some()
                })
            });
        let config_owns_font_size = (configured_css.is_some()
            || configured_font_size.is_some_and(|(_, path)| path == "themeVariables.fontSize"))
            && effective_config.config_path_overrides_typed_default("themeVariables.fontSize");
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size_known = typed_font_size_active
            || configured_css.is_none()
            || configured_font_size.is_some_and(|(_, path)| path == "themeVariables.fontSize");
        let font_size_px = match (theme, typed_font_size_active) {
            (Some(theme), true) => f64::from(theme.typography().font_size_px()),
            _ if !font_size_known => 16.0,
            _ => configured_font_size.map_or(16.0, |(size, _)| size),
        };
        let font_size_css = if font_size_known {
            format!("{font_size_px}px").into_boxed_str()
        } else {
            configured_css
                .expect("unknown root size retains its CSS")
                .trim()
                .into()
        };

        let config = effective_config.as_value();
        let view = super::C4ConfigView::new(config);
        let element_styles = super::C4_ELEMENT_TYPES
            .iter()
            .copied()
            .chain(
                model
                    .shapes
                    .iter()
                    .map(|shape| shape.type_c4_shape.as_str()),
            )
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|name| {
                let font = C4PreparedFont::resolve_element(
                    view.shape_font(name),
                    inherited_font_stack.font_family_css(),
                    font_size_px,
                    font_size_known,
                    view.shape_font_size_value(name),
                );
                (
                    name.to_owned(),
                    C4ElementStyle {
                        font,
                        source_colors: None,
                    },
                )
            })
            .collect();
        let common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            config,
            inherited_font_stack.font_family_css().to_owned(),
            font_size_css.to_string(),
        );
        Self {
            common_css,
            person_border: crate::config::config_string(
                config,
                &["themeVariables", "personBorder"],
            )
            .unwrap_or_else(|| "hsl(240, 60%, 86.2745098039%)".to_owned()),
            person_background: crate::config::config_string(
                config,
                &["themeVariables", "personBkg"],
            )
            .unwrap_or_else(|| "#ECECFF".to_owned()),
            element_styles,
            boundary_fonts: C4BoundaryFonts::resolve(
                view.boundary_font(),
                inherited_font_stack.font_family_css(),
                font_size_px,
                font_size_known,
            ),
            message_font: C4PreparedFont::resolve(
                view.message_font(),
                inherited_font_stack.font_family_css(),
                font_size_px,
                font_size_known,
                true,
            ),
            relationship_text_color: view.color("relationship_text_color", "#444444"),
            relationship_line_color: view.color("relationship_line_color", "#444444"),
            inherited_font_stack,
            font_size_px,
            font_size_known,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            title: normalize_title(title),
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }
    pub(crate) fn person_border(&self) -> &str {
        &self.person_border
    }
    pub(crate) fn person_background(&self) -> &str {
        &self.person_background
    }
    pub(crate) fn element_style(&self, name: &str) -> Option<&C4ElementStyle> {
        self.element_styles.get(name)
    }

    pub(super) fn prepare_element_colors(
        &mut self,
        name: &str,
        config: &super::C4ConfigView<'_>,
    ) -> Option<&C4ElementColors> {
        let style = self.element_styles.get_mut(name)?;
        Some(style.source_colors.get_or_insert_with(|| {
            let (background, border) = if name.starts_with("external_") {
                ("#999999", "#8A8A8A")
            } else {
                ("#08427B", "#073B6F")
            };
            C4ElementColors {
                background: config.color(&format!("{name}_bg_color"), background),
                border: config.color(&format!("{name}_border_color"), border),
                text: config.color(&format!("{name}_font_color"), "#FFFFFF"),
            }
        }))
    }
    pub(crate) fn boundary_fonts(&self) -> &C4BoundaryFonts {
        &self.boundary_fonts
    }
    pub(crate) fn message_font(&self) -> &C4PreparedFont {
        &self.message_font
    }

    pub(crate) fn relationship_text_color(&self) -> &str {
        &self.relationship_text_color
    }
    pub(crate) fn relationship_line_color(&self) -> &str {
        &self.relationship_line_color
    }

    #[cfg(test)]
    pub(crate) fn use_config_root_css_for_test(&mut self, config: &serde_json::Value) {
        self.common_css = crate::svg::PreparedCommonCss::new(config, None);
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) fn font_size_is_known(&self) -> bool {
        self.font_size_known
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<C4TypographyThemeReceipt<'_>> {
        self.typography_requested()
            .then(|| C4TypographyThemeReceipt {
                expected_font_family_css: self.font_family_css(),
                expected_font_size_css: self.font_size_css(),
                expected_title: self.title.as_deref(),
                expected_font_size_px: self.font_size_px,
                root_size_known: self.font_size_known,
                css_emitted: false,
                css_matches: false,
                title_seen: false,
                has_visible_inherited_font_family: false,
                has_visible_inherited_font_size: false,
                has_unverified_visible_font_family: false,
                has_unverified_visible_font_size: false,
                valid: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: C4TypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_seal.set(seal).is_ok())
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.typography_requested() {
            return;
        }

        let Some(seal) = self.terminal_seal.get() else {
            self.mark_typed_property_evidence(
                evidence,
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
                false,
                false,
            );
            self.mark_typed_property_evidence(
                evidence,
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
                false,
                false,
            );
            return;
        };

        self.mark_typed_property_evidence(
            evidence,
            ThemeTypographyProperty::FontStack,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.inherited_font_stack.typed_font_stack_active(),
            seal.has_visible_title || seal.has_visible_inherited_font_family,
            seal.has_unverified_visible_font_family,
        );
        self.mark_typed_property_evidence(
            evidence,
            ThemeTypographyProperty::FontSize,
            self.typed_font_size_requested,
            self.typed_font_size_active,
            seal.has_visible_title || seal.has_visible_inherited_font_size,
            seal.has_unverified_visible_font_size,
        );
    }

    fn mark_typed_property_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        property: ThemeTypographyProperty,
        requested: bool,
        active: bool,
        has_visible_consumer: bool,
        has_unverified_consumer: bool,
    ) {
        if !requested {
            return;
        }
        let key = FamilyThemeMechanismKey::Typography(property);
        if !active {
            evidence.mark_not_applicable(key);
        } else if self.terminal_seal.get().is_none() || has_unverified_consumer {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
        } else if !has_visible_consumer {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        }
    }
}

impl<'a> C4TypographyThemeReceipt<'a> {
    pub(crate) fn record_css_emission(
        &mut self,
        emitted_font_family_css: &str,
        emitted_font_size_css: &str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
    ) {
        self.valid &= !self.css_emitted
            && base_typography_emitted
            && root_typography_emitted
            && emitted_font_family_css == self.expected_font_family_css
            && emitted_font_size_css == self.expected_font_size_css;
        self.css_emitted = true;
        self.css_matches = emitted_font_family_css == self.expected_font_family_css
            && emitted_font_size_css == self.expected_font_size_css;
    }

    pub(crate) fn record_title_text(&mut self, text: &str) {
        self.valid &= !self.title_seen && self.expected_title == Some(text);
        self.title_seen = true;
        self.has_unverified_visible_font_family |= !self.root_size_known;
        self.has_unverified_visible_font_size |= !self.root_size_known;
    }

    pub(crate) fn record_visible_font(&mut self, font: &C4PreparedFont) {
        self.valid &= self.css_emitted && self.css_matches && font.potentially_visible();
        let size_known = font.font_size_is_known();
        self.has_unverified_visible_font_size |= !size_known;
        match font.family_ownership {
            CssFontFamilyOwnership::Inherited => {
                self.valid &= font.measurement_style.font_family.as_deref()
                    == Some(self.expected_font_family_css);
                self.has_visible_inherited_font_family |= size_known;
                self.has_unverified_visible_font_family |= !size_known;
            }
            CssFontFamilyOwnership::SourceOwned => {}
            CssFontFamilyOwnership::Unverified => {
                self.has_unverified_visible_font_family = true;
            }
        }
        if font.font_size_inherits_root {
            self.valid &= font.measurement_style.font_size == self.expected_font_size_px;
            self.has_visible_inherited_font_size = true;
        }
    }

    fn seal(self) -> Option<C4TypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.css_matches
            && self.title_seen == self.expected_title.is_some())
        .then_some(C4TypographyTerminalSeal {
            has_visible_title: self.title_seen
                && self.root_size_known
                && self.expected_font_size_px > 0.0,
            has_visible_inherited_font_family: self.has_visible_inherited_font_family,
            has_visible_inherited_font_size: self.has_visible_inherited_font_size,
            has_unverified_visible_font_family: self.has_unverified_visible_font_family,
            has_unverified_visible_font_size: self.has_unverified_visible_font_size,
        })
    }
}

fn normalize_title(title: Option<&str>) -> Option<Box<str>> {
    title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{C4TypographyThemePlan, C4TypographyThemeReceipt};
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use merman_model::presentation::PresentationConfig;

    #[test]
    fn title_receipt_requires_matching_css_and_optional_title() {
        let mut receipt = C4TypographyThemeReceipt {
            expected_font_family_css: "Inter",
            expected_font_size_css: "18px",
            expected_title: Some("Context"),
            expected_font_size_px: 18.0,
            root_size_known: true,
            css_emitted: false,
            css_matches: false,
            title_seen: false,
            has_visible_inherited_font_family: false,
            has_visible_inherited_font_size: false,
            has_unverified_visible_font_family: false,
            has_unverified_visible_font_size: false,
            valid: true,
        };
        receipt.record_css_emission("Inter", "18px", true, true);
        receipt.record_title_text("Context");
        assert!(receipt.seal().is_some());

        let mut missing_title = C4TypographyThemeReceipt {
            expected_font_family_css: "Inter",
            expected_font_size_css: "18px",
            expected_title: Some("Context"),
            expected_font_size_px: 18.0,
            root_size_known: true,
            css_emitted: false,
            css_matches: false,
            title_seen: false,
            has_visible_inherited_font_family: false,
            has_visible_inherited_font_size: false,
            has_unverified_visible_font_family: false,
            has_unverified_visible_font_size: false,
            valid: true,
        };
        missing_title.record_css_emission("Inter", "18px", true, true);
        assert!(missing_title.seal().is_none());
    }

    #[test]
    fn typed_values_are_resolved_for_c4_title_surface() {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Inter").expect("font stack"))
            .with_font_size_px(18.0)
            .expect("font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(crate::DiagramFamilyId::C4, typography),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(spec)
            .expect("compile C4 theme")
            .resolve(crate::DiagramFamilyId::C4);
        let plan = C4TypographyThemePlan::resolve(
            Some(&theme),
            &PresentationConfig::empty_object(),
            Some("Context"),
            &serde_json::from_value(serde_json::json!({})).expect("empty C4 fixture"),
        );

        assert_eq!(plan.font_family_css(), "Inter");
        assert_eq!(plan.font_size_css(), "18px");
        assert!(plan.begin_terminal_receipt().is_some());
    }

    #[test]
    fn prepared_element_styles_keep_actual_types_and_independent_owners() {
        let model = serde_json::from_value(serde_json::json!({
            "shapes": [
                {"alias": "custom", "typeC4Shape": "custom_type"},
                {"alias": "empty"}
            ]
        }))
        .expect("C4 actual-type fixture");
        let config = crate::test_support::presentation_config(serde_json::json!({"c4": {
            "custom_typeFontFamily": "  Custom Sans ;  ", "custom_typeFontSize": "19",
            "custom_typeFontWeight": 600, "custom_type_bg_color": "var(--card)",
            "custom_type_border_color": "", "custom_type_font_color": 42,
            "boundaryFontFamily": "", "boundaryFontSize": 21,
            "messageFontFamily": "Message Sans", "messageFontSize": 17
        }}));
        let mut plan = C4TypographyThemePlan::resolve(None, &config, None, &model);
        let actual = plan
            .element_style("custom_type")
            .expect("actual type bound");
        assert_eq!(
            actual.font.measurement_style.font_family.as_deref(),
            Some(plan.font_family_css())
        );
        assert_eq!(
            actual.font.css_font_family, "",
            "CSSOM rejects the raw trailing semicolon"
        );
        // A nonzero unitless string is rejected by the element CSSOM and inherits the root.
        assert_eq!(actual.font.measurement_style.font_size, 16.0);
        assert_eq!(
            actual.font.measurement_style.font_weight.as_deref(),
            Some("600")
        );
        assert!(
            plan.element_style("").is_some(),
            "default model type is bound"
        );
        assert_eq!(plan.boundary_fonts().ty.css_font_family, "");
        assert_eq!(
            plan.boundary_fonts()
                .ty
                .measurement_style
                .font_family
                .as_deref(),
            Some(plan.font_family_css())
        );
        assert_eq!(plan.boundary_fonts().ty.measurement_style.font_size, 21.0);
        assert_eq!(plan.message_font().measurement_style.font_size, 17.0);
        let view = super::super::C4ConfigView::new(config.as_value());
        let colors = plan
            .prepare_element_colors("custom_type", &view)
            .expect("actual type colors");
        assert_eq!(colors.background, "var(--card)");
        assert_eq!(colors.border, "");
        assert_eq!(colors.text, "#FFFFFF");
        let colors = plan
            .prepare_element_colors("external_person", &view)
            .expect("external type colors");
        assert_eq!(colors.background, "#999999");
        assert_eq!(colors.border, "#8A8A8A");
        assert_eq!(colors.text, "#FFFFFF");
    }
}
