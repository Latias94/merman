//! Static label weights shared by layout and terminal writers.

use std::borrow::Cow;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet, ResolvedDiagramTheme,
    ResolvedStyleProperty, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::{TextStyle, resolve_css_font_weight};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FlowchartLabelWeights {
    node: Option<u16>,
    edge: Option<u16>,
    invalid_config_weight: bool,
}

impl FlowchartLabelWeights {
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
                            ThemeTypographyProperty::FontWeight
                        ),
                        ..
                    }
                )
        }) {
            return Ok(Self::default());
        }
        let weight = |target| -> Result<Option<u16>, OperationWorkError> {
            let style = match meter {
                Some(meter) => {
                    theme.style_with_work_meter(target, ThemeVariant::Default, None, meter)?
                }
                None => theme.style(target, ThemeVariant::Default, None),
            };
            let selected = style.winner_rule_properties().any(|(property, origin)| {
                property == ResolvedStyleProperty::Typography(ThemeTypographyProperty::FontWeight)
                    && theme.rule_facet_disposition(
                        origin.rule_index(),
                        FamilyThemeRuleFacet::Typography(ThemeTypographyProperty::FontWeight),
                    ) == Some(FamilyThemeDisposition::TypedAdapter)
            });
            Ok(selected.then(|| style.typography().font_weight()))
        };
        Ok(Self {
            node: weight(ThemeTarget::NodeLabel)?,
            edge: weight(ThemeTarget::EdgeLabel)?,
            invalid_config_weight: false,
        })
    }

    pub(crate) fn with_config(
        mut self,
        ownership: super::FlowchartTypographyConfigOwnership,
    ) -> Self {
        if let Some(weight) = ownership.font_weight {
            self.node = self.node.map(|_| weight);
            self.edge = self.edge.map(|_| weight);
        }
        self.invalid_config_weight = ownership.invalid_font_weight;
        self
    }

    pub(crate) fn config_is_verified(self) -> bool {
        !self.invalid_config_weight
    }

    pub(crate) fn get(self, target: ThemeTarget) -> Option<u16> {
        match target {
            ThemeTarget::NodeLabel => self.node,
            ThemeTarget::EdgeLabel => self.edge,
            _ => None,
        }
    }

    pub(crate) fn apply_cow<'a>(
        self,
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
        self,
        target: ThemeTarget,
        effective: &TextStyle,
        style: &mut String,
    ) {
        if self.get(target).is_none() {
            return;
        }
        if let Some(weight) = effective
            .font_weight
            .as_deref()
            .and_then(|s| s.parse::<u16>().ok())
            .filter(|weight| (1..=1000).contains(weight))
        {
            use std::fmt::Write as _;
            let _ = write!(style, ";font-weight:{weight} !important;");
        }
    }

    pub(crate) fn apply<'a>(
        self,
        target: ThemeTarget,
        source: &'a TextStyle,
    ) -> Cow<'a, TextStyle> {
        self.apply_cow(target, Cow::Borrowed(source))
    }
}
