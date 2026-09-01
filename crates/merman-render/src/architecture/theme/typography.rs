use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::config_theme_font_size_css_or_root_number_px_opt;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};

/// Resolves Architecture's inherited SVG typography without taking ownership of Cytoscape text.
///
/// Architecture has two intentionally separate font-size consumers. The base theme size applies
/// to SVG service/group/edge labels and CSS-inherited `iconText`; `architecture.fontSize` remains
/// the Cytoscape layout and group-icon-alignment owner. Keeping those values separate preserves
/// Mermaid's layout semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArchitectureTypographyTerminalInventory {
    native_svg_text_count: usize,
    icon_text_count: usize,
}

impl ArchitectureTypographyTerminalInventory {
    pub(crate) fn new(native_svg_text_count: usize, icon_text_count: usize) -> Option<Self> {
        native_svg_text_count
            .checked_add(icon_text_count)
            .map(|_| Self {
                native_svg_text_count,
                icon_text_count,
            })
    }

    const fn visible_text_terminal_count(self) -> usize {
        self.native_svg_text_count + self.icon_text_count
    }
}

/// CSS facts recorded by the successful Architecture stylesheet writer.
///
/// This is deliberately a small, writer-owned receipt rather than a stylesheet parser. The
/// terminal renderer consumes it together with its own DOM emission counts.
#[derive(Debug, Clone)]
pub(crate) struct ArchitectureTypographyCssEmission {
    diagram_root_font_family_css: Box<str>,
    nested_svg_font_family_css: Box<str>,
    diagram_root_font_size_css: Box<str>,
    nested_svg_font_size_css: Box<str>,
    root_variable_font_family_css: Box<str>,
}

impl ArchitectureTypographyCssEmission {
    pub(crate) fn new(
        diagram_root_font_family_css: &str,
        nested_svg_font_family_css: &str,
        diagram_root_font_size_css: &str,
        nested_svg_font_size_css: &str,
        root_variable_font_family_css: &str,
    ) -> Self {
        Self {
            diagram_root_font_family_css: diagram_root_font_family_css.into(),
            nested_svg_font_family_css: nested_svg_font_family_css.into(),
            diagram_root_font_size_css: diagram_root_font_size_css.into(),
            nested_svg_font_size_css: nested_svg_font_size_css.into(),
            root_variable_font_family_css: root_variable_font_family_css.into(),
        }
    }
}
#[derive(Debug)]
pub(crate) struct ArchitectureTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    font_size_px: f64,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    terminals: ArchitectureTypographyTerminalInventory,
    evidence: FamilyThemeEvidence,
    terminal_seal: OnceLock<ArchitectureTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct ArchitectureTypographyTerminalSeal {
    native_svg_text_count: usize,
    icon_text_count: usize,
}

/// Writer-owned proof for Architecture's inherited SVG typography.
#[derive(Debug, Clone)]
pub(crate) struct ArchitectureTypographyThemeReceipt {
    expected_font_family_css: Box<str>,
    expected_font_size_px: f64,
    expected_font_size_css: Box<str>,
    expected_terminals: ArchitectureTypographyTerminalInventory,
    css_emitted: bool,
    native_svg_text_count: usize,
    icon_text_count: usize,
    valid: bool,
}

impl ArchitectureTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        terminals: ArchitectureTypographyTerminalInventory,
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
        // Only the explicit theme-variable path owns this property. A top-level `fontSize` is a
        // Mermaid-compatible fallback and must not shadow an active typed base size.
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size_px = match (theme, typed_font_size_active) {
            (Some(theme), true) => f64::from(theme.typography().font_size_px()).max(1.0),
            _ => config_theme_font_size_css_or_root_number_px_opt(effective_config.as_value())
                .unwrap_or(16.0)
                .max(1.0),
        };
        let font_size_px = crate::number_format::canonicalize_number(font_size_px).max(1.0);
        let font_size_css =
            format!("{}px", crate::number_format::canonical_number(font_size_px)).into_boxed_str();

        Self {
            inherited_font_stack,
            font_size_px,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            terminals,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<ArchitectureTypographyThemeReceipt> {
        self.typography_requested()
            .then(|| ArchitectureTypographyThemeReceipt {
                expected_font_family_css: self.font_family_css().into(),
                expected_font_size_px: self.font_size_px,
                expected_font_size_css: self.font_size_css().into(),
                expected_terminals: self.terminals,
                css_emitted: false,
                native_svg_text_count: 0,
                icon_text_count: 0,
                valid: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: ArchitectureTypographyThemeReceipt) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_seal.set(seal).is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let has_visible_terminals = self.terminals.visible_text_terminal_count() != 0;
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, has_visible_terminals);

        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontStack,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.inherited_font_stack.typed_font_stack_active(),
        );
        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontSize,
            self.typed_font_size_requested,
            self.typed_font_size_active,
        );
        evidence
    }

    fn mark_typed_property_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        property: ThemeTypographyProperty,
        requested: bool,
        active: bool,
    ) {
        if !requested {
            return;
        }
        let key = FamilyThemeMechanismKey::Typography(property);
        if !active {
            evidence.mark_not_applicable(key);
            return;
        }

        let Some(seal) = self.terminal_seal.get() else {
            if self.terminals.visible_text_terminal_count() == 0 {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            return;
        };
        if seal.native_svg_text_count + seal.icon_text_count == 0 {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        }
    }
}

impl ArchitectureTypographyThemeReceipt {
    pub(crate) fn record_css_emission(&mut self, emitted: &ArchitectureTypographyCssEmission) {
        self.valid &= !self.css_emitted
            && emitted.diagram_root_font_family_css == self.expected_font_family_css
            && emitted.nested_svg_font_family_css == self.expected_font_family_css
            && emitted.root_variable_font_family_css == self.expected_font_family_css
            && emitted.diagram_root_font_size_css == self.expected_font_size_css
            && emitted.nested_svg_font_size_css == self.expected_font_size_css;
        self.css_emitted = true;
    }

    pub(crate) fn record_native_svg_text_terminal(
        &mut self,
        font_family_css: &str,
        font_size_px: f64,
    ) {
        self.valid &= self.native_svg_text_count < self.expected_terminals.native_svg_text_count
            && font_family_css == self.expected_font_family_css.as_ref()
            && font_size_px == self.expected_font_size_px;
        self.native_svg_text_count = self.native_svg_text_count.saturating_add(1);
    }

    pub(crate) fn record_icon_text_terminal(&mut self) {
        self.valid &= self.icon_text_count < self.expected_terminals.icon_text_count;
        self.icon_text_count = self.icon_text_count.saturating_add(1);
    }

    fn seal(self) -> Option<ArchitectureTypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.native_svg_text_count == self.expected_terminals.native_svg_text_count
            && self.icon_text_count == self.expected_terminals.icon_text_count)
            .then_some(ArchitectureTypographyTerminalSeal {
                native_svg_text_count: self.native_svg_text_count,
                icon_text_count: self.icon_text_count,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArchitectureTypographyCssEmission, ArchitectureTypographyTerminalInventory,
        ArchitectureTypographyThemePlan, ArchitectureTypographyThemeReceipt,
    };
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use merman_core::MermaidConfig;

    #[test]
    fn typed_values_are_resolved_for_svg_labels_only() {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Inter").expect("font stack"))
            .with_font_size_px(18.0)
            .expect("font size");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default()
                        .with_family_style(DiagramFamilyId::ARCHITECTURE, typography),
                ),
            )
            .expect("compile Architecture theme")
            .resolve(DiagramFamilyId::ARCHITECTURE);
        let plan = ArchitectureTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::empty_object(),
            ArchitectureTypographyTerminalInventory::new(2, 1).expect("terminal inventory"),
        );

        assert_eq!(plan.font_family_css(), "Inter");
        assert_eq!(plan.font_size_px(), 18.0);
        assert!(plan.begin_terminal_receipt().is_some());
    }

    #[test]
    fn receipt_requires_all_real_svg_text_terminals() {
        let mut receipt = ArchitectureTypographyThemeReceipt {
            expected_font_family_css: "Inter".into(),
            expected_font_size_px: 18.0,
            expected_font_size_css: "18px".into(),
            expected_terminals: ArchitectureTypographyTerminalInventory::new(2, 1)
                .expect("terminal inventory"),
            css_emitted: false,
            native_svg_text_count: 0,
            icon_text_count: 0,
            valid: true,
        };
        receipt.record_css_emission(&ArchitectureTypographyCssEmission::new(
            "Inter", "Inter", "18px", "18px", "Inter",
        ));
        receipt.record_native_svg_text_terminal("Inter", 18.0);
        assert!(receipt.clone().seal().is_none());
        receipt.record_native_svg_text_terminal("Inter", 18.0);
        receipt.record_icon_text_terminal();
        assert!(receipt.seal().is_some());
    }

    #[test]
    fn raw_config_without_ownership_keeps_the_typed_route() {
        let typography = ThemeTextStyle::default()
            .with_font_size_px(18.0)
            .expect("font size");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default()
                        .with_family_style(DiagramFamilyId::ARCHITECTURE, typography),
                ),
            )
            .expect("compile Architecture theme")
            .resolve(DiagramFamilyId::ARCHITECTURE);
        let config = MermaidConfig::from_value(serde_json::json!({
            "themeVariables": { "fontSize": "22px" }
        }));
        let plan = ArchitectureTypographyThemePlan::resolve(
            Some(&theme),
            &config,
            ArchitectureTypographyTerminalInventory::new(1, 0).expect("terminal inventory"),
        );

        // `from_value` intentionally carries no explicit-path provenance. The real parse
        // pipeline supplies that metadata when site/source config owns the value.
        assert_eq!(plan.font_size_px(), 18.0);
        assert!(plan.typed_font_size_active);
    }
}
