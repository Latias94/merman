//! Static label typography shared by layout, source resolution, and terminal writers.

use std::borrow::Cow;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet, FontStack,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeTarget, ThemeTypographyProperty,
    ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::{TextStyle, resolve_css_font_weight};

#[derive(Debug, Clone, Default)]
struct LabelTypography {
    font_stack: Option<FontStack>,
    font_size: Option<f64>,
    font_weight: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FlowchartLabelTypography {
    node: LabelTypography,
    edge: LabelTypography,
    invalid_config_weight: bool,
}

impl FlowchartLabelTypography {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        meter: Option<&OperationWorkMeter>,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let routes = theme.family_mechanism_routes();
        if let Some(meter) = meter {
            meter.charge(routes.len())?;
        }
        if !routes.iter().any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::NodeLabel | ThemeTarget::EdgeLabel,
                        facet: FamilyThemeRuleFacet::Typography(
                            ThemeTypographyProperty::FontStack
                                | ThemeTypographyProperty::FontSize
                                | ThemeTypographyProperty::FontWeight
                        ),
                        ..
                    }
                )
        }) {
            return Ok(Self::default());
        }
        let typography = |target| -> Result<LabelTypography, OperationWorkError> {
            let style = match meter {
                Some(meter) => {
                    theme.style_with_work_meter(target, ThemeVariant::Default, None, meter)?
                }
                None => theme.style(target, ThemeVariant::Default, None),
            };
            let selected = |requested| {
                style.winner_rule_properties().any(|(property, origin)| {
                    property == ResolvedStyleProperty::Typography(requested)
                        && theme.rule_facet_disposition(
                            origin.rule_index(),
                            FamilyThemeRuleFacet::Typography(requested),
                        ) == Some(FamilyThemeDisposition::TypedAdapter)
                })
            };
            let patch = style.typography_resolution().patch();
            // Swimlane resolves its labelRect source styles in a separate layout module. Until
            // that module installs these parents, retain its existing unverified font path.
            let prepared_fonts = matches!(
                theme.family_id(),
                crate::DiagramFamilyId::FLOWCHART | crate::DiagramFamilyId::AGENTFLOW
            );
            Ok(LabelTypography {
                font_stack: (prepared_fonts
                    && selected(ThemeTypographyProperty::FontStack)
                    && matches!(patch.font_stack, Specified::Value(_)))
                .then(|| style.typography().font_stack().clone()),
                font_size: (prepared_fonts
                    && selected(ThemeTypographyProperty::FontSize)
                    && matches!(patch.font_size_px, Specified::Value(_)))
                .then(|| f64::from(style.typography().font_size_px())),
                font_weight: selected(ThemeTypographyProperty::FontWeight)
                    .then(|| style.typography().font_weight()),
            })
        };
        Ok(Self {
            node: typography(ThemeTarget::NodeLabel)?,
            edge: typography(ThemeTarget::EdgeLabel)?,
            invalid_config_weight: false,
        })
    }

    pub(crate) fn with_config(
        mut self,
        ownership: super::FlowchartTypographyConfigOwnership,
    ) -> Self {
        for style in [&mut self.node, &mut self.edge] {
            if ownership.font_stack {
                style.font_stack = None;
            }
            if ownership.font_size {
                style.font_size = None;
            }
            if let Some(weight) = ownership.font_weight {
                style.font_weight = style.font_weight.map(|_| weight);
            }
        }
        self.invalid_config_weight = ownership.invalid_font_weight;
        self
    }

    pub(crate) fn config_is_verified(&self) -> bool {
        !self.invalid_config_weight
    }

    fn style(&self, target: ThemeTarget) -> Option<&LabelTypography> {
        match target {
            ThemeTarget::NodeLabel => Some(&self.node),
            ThemeTarget::EdgeLabel => Some(&self.edge),
            _ => None,
        }
    }

    pub(crate) fn get(&self, target: ThemeTarget) -> Option<u16> {
        self.style(target)?.font_weight
    }

    pub(crate) fn has_fonts(&self, target: ThemeTarget) -> bool {
        self.style(target)
            .is_some_and(|style| style.font_stack.is_some() || style.font_size.is_some())
    }

    pub(crate) fn requested(&self, target: ThemeTarget) -> bool {
        self.has_fonts(target) || self.get(target).is_some()
    }

    pub(crate) fn parent_style<'a>(
        &self,
        target: ThemeTarget,
        parent: &'a TextStyle,
    ) -> Cow<'a, TextStyle> {
        self.parent_style_with_source(target, parent, false, false)
    }

    pub(crate) fn source_parent_style<'a>(
        &self,
        target: ThemeTarget,
        parent: &'a TextStyle,
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        classes: &[String],
        inline_styles: &[String],
        node_defaults: bool,
    ) -> Cow<'a, TextStyle> {
        if !self.has_fonts(target) {
            return self.parent_style(target, parent);
        }
        let [stack, size] = super::style::flowchart_declared_font_facets(
            class_defs,
            classes,
            inline_styles,
            node_defaults,
        );
        self.parent_style_with_source(target, parent, stack, size)
    }

    pub(crate) fn node_source_parent_style<'a>(
        &self,
        parent: &'a TextStyle,
        class_defs: &indexmap::IndexMap<String, Vec<String>>,
        source: (&[String], &[String], bool),
        structural: &super::FlowchartStructuralLabelTypography,
    ) -> Cow<'a, TextStyle> {
        let [stack, size] =
            super::style::flowchart_declared_font_facets(class_defs, source.0, source.1, source.2);
        let parent = self.parent_style_with_source(
            ThemeTarget::NodeLabel,
            parent,
            stack || !structural.font_stack_status().is_absent(),
            size || !structural.font_size_status().is_absent(),
        );
        structural.apply_to_parent(parent)
    }

    pub(crate) fn parent_style_with_source<'a>(
        &self,
        target: ThemeTarget,
        parent: &'a TextStyle,
        source_font_stack: bool,
        source_font_size: bool,
    ) -> Cow<'a, TextStyle> {
        let Some(style) = self.style(target) else {
            return Cow::Borrowed(parent);
        };
        if !self.requested(target) {
            return Cow::Borrowed(parent);
        }
        // A source winner keeps the original CSS parent, so em/inherit do not inherit a
        // competing theme declaration that the browser's cascade would discard.
        let mut parent = parent.clone();
        if !source_font_stack && let Some(stack) = &style.font_stack {
            parent.font_family = Some(stack.as_css());
        }
        if !source_font_size && let Some(size) = style.font_size {
            parent.font_size = size;
        }
        if let Some(weight) = style.font_weight {
            parent.font_weight = Some(weight.to_string());
        }
        Cow::Owned(parent)
    }

    pub(crate) fn apply_cow<'a>(
        &self,
        target: ThemeTarget,
        mut source: Cow<'a, TextStyle>,
    ) -> Cow<'a, TextStyle> {
        let Some(base) = self.get(target) else {
            return source;
        };
        let weight = match source.font_weight.as_deref() {
            Some(value) => match resolve_css_font_weight(value, base) {
                Some(weight) => weight,
                // Preserve invalid source input for the existing source admission boundary.
                None => return source,
            },
            None => base,
        };
        source.to_mut().font_weight = Some(weight.to_string());
        source
    }

    pub(crate) fn append_style(
        &self,
        target: ThemeTarget,
        effective: &TextStyle,
        source_font_stack: super::FlowchartSourceFacetStatus,
        source_font_size: super::FlowchartSourceFacetStatus,
        style: &mut String,
    ) {
        let Some(selected) = self.style(target) else {
            return;
        };
        use std::fmt::Write as _;
        if selected.font_stack.is_some()
            && source_font_stack.is_absent()
            && let Some(stack) = &effective.font_family
        {
            let _ = write!(style, ";font-family:{stack} !important;");
        }
        if selected.font_size.is_some() && source_font_size.is_absent() {
            let _ = write!(style, ";font-size:{}px !important;", effective.font_size);
        }
        if selected.font_weight.is_some()
            && let Some(weight) = effective
                .font_weight
                .as_deref()
                .and_then(|s| s.parse::<u16>().ok())
                .filter(|weight| (1..=1000).contains(weight))
        {
            let _ = write!(style, ";font-weight:{weight} !important;");
        }
    }

    pub(crate) fn apply<'a>(
        &self,
        target: ThemeTarget,
        source: &'a TextStyle,
    ) -> Cow<'a, TextStyle> {
        self.apply_cow(target, Cow::Borrowed(source))
    }
}
