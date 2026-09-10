use merman_core::__private::ThemeCompatibilityPlan;

use crate::DiagramFamilyId;
use crate::diagram_theme::{DiagramThemeSpec, Specified, ThemeTarget};

pub(crate) struct FamilyPaintDefaultPaths {
    family: DiagramFamilyId,
    targets: &'static [ThemeTarget],
    paths: &'static [&'static str],
}

impl FamilyPaintDefaultPaths {
    pub(crate) const fn new(
        family: DiagramFamilyId,
        targets: &'static [ThemeTarget],
        paths: &'static [&'static str],
    ) -> Self {
        Self {
            family,
            targets,
            paths,
        }
    }

    fn requested_by(&self, spec: &DiagramThemeSpec) -> bool {
        spec.styles().rules().iter().any(|rule| {
            self.targets.contains(&rule.target())
                && rule.family().is_none_or(|family| family == self.family)
                && (!matches!(rule.style().paint.fill, Specified::Unspecified)
                    || !matches!(rule.style().stroke.paint, Specified::Unspecified))
        })
    }
}

pub(crate) fn bind_theme_parse_defaults(
    mut plan: ThemeCompatibilityPlan,
    spec: &DiagramThemeSpec,
) -> ThemeCompatibilityPlan {
    for defaults in [
        &crate::gitgraph::GITGRAPH_NODE_PAINT_DEFAULTS,
        &crate::er::ER_PAINT_DEFAULTS,
        &crate::requirement::REQUIREMENT_RELATION_PAINT_DEFAULTS,
    ] {
        if defaults.requested_by(spec) {
            plan = plan
                .try_with_post_detection_default_paths(defaults.family.as_str(), defaults.paths)
                .expect("family writer default paths satisfy the bounded core contract");
        }
    }
    plan
}
