use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeRuleFacet, ResolvedDiagramTheme,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DirectStaticSelectorDomain {
    Unqualified,
    Default,
}

impl DirectStaticSelectorDomain {
    const fn accepts(self, variant: Option<ThemeVariant>) -> bool {
        match self {
            Self::Unqualified => variant.is_none(),
            Self::Default => matches!(variant, None | Some(ThemeVariant::Default)),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DirectStaticPaint {
    rule_index: usize,
    css: Box<str>,
    capability: ThemeCapability,
}

impl DirectStaticPaint {
    pub(crate) const fn rule_index(&self) -> usize {
        self.rule_index
    }

    pub(crate) fn css(&self) -> &str {
        &self.css
    }

    pub(crate) const fn capability(&self) -> ThemeCapability {
        self.capability
    }

    pub(crate) fn into_parts(self) -> (Box<str>, usize, ThemeCapability) {
        (self.css, self.rule_index, self.capability)
    }
}

pub(crate) fn resolve_direct_static_fill(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    accepted_targets: &[ThemeTarget],
    selector_domain: DirectStaticSelectorDomain,
) -> Option<DirectStaticPaint> {
    let resolution = style.fill_resolution();
    let origin = resolution.winner()?;
    if !accepted_targets.contains(&origin.target())
        || !selector_domain.accepts(origin.variant())
        || origin.ordinal().is_some()
        || theme.rule_facet_disposition(
            origin.rule_index(),
            FamilyThemeRuleFacet::fill(resolution.specified())?,
        ) != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    let (css, capability) = match resolution.specified() {
        Specified::Value(CanvasPaint::Transparent) => {
            ("transparent".into(), ThemeCapability::TransparentPaint)
        }
        Specified::Value(CanvasPaint::Solid(color)) => {
            (color.as_css().into_boxed_str(), ThemeCapability::SolidPaint)
        }
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };
    Some(DirectStaticPaint {
        rule_index: origin.rule_index(),
        css,
        capability,
    })
}
