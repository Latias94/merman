use merman_core::{Engine, MermaidConfig};
use serde_json::Value;

use super::{HostTheme, PresentationError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PresentationProfile {
    MermanModern,
}

impl PresentationProfile {
    pub const ALL: [Self; 1] = [Self::MermanModern];

    pub const fn id(self) -> &'static str {
        match self {
            Self::MermanModern => "merman-modern",
        }
    }

    pub fn from_id(id: &str) -> Result<Self, PresentationError> {
        Self::ALL
            .into_iter()
            .find(|profile| profile.id() == id)
            .ok_or_else(|| PresentationError::UnknownPresentationProfile(id.to_string()))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presentation {
    profile: Option<PresentationProfile>,
    theme: Option<HostTheme>,
}

impl Presentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub const fn profile(&self) -> Option<PresentationProfile> {
        self.profile
    }

    pub fn theme(&self) -> Option<&HostTheme> {
        self.theme.as_ref()
    }

    pub fn with_profile(mut self, profile: PresentationProfile) -> Self {
        self.profile = Some(profile);
        self
    }

    pub fn with_theme(mut self, theme: HostTheme) -> Self {
        self.theme = Some(theme);
        self
    }

    pub fn resolve(self) -> ResolvedPresentation {
        let mut mermaid_config = self.profile.map(profile_defaults).unwrap_or_default();
        if let Some(theme) = &self.theme {
            let theme = theme.mermaid_config_patch();
            mermaid_config.deep_merge(theme.as_value());
        }
        let flowchart_policy = self.profile.map(|_| FlowchartPresentationPolicy {
            edge_corner_radius: None,
            edge_label_padding: 4.0,
            compact_edge_corners: true,
        });
        ResolvedPresentation {
            presentation: self,
            mermaid_config,
            flowchart_policy,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedPresentation {
    presentation: Presentation,
    mermaid_config: MermaidConfig,
    flowchart_policy: Option<FlowchartPresentationPolicy>,
}

impl ResolvedPresentation {
    pub fn presentation(&self) -> &Presentation {
        &self.presentation
    }

    pub fn materialize_engine(&self, engine: Engine) -> Engine {
        engine.with_site_config(self.mermaid_config.clone())
    }

    pub(crate) fn mermaid_config(&self) -> &MermaidConfig {
        &self.mermaid_config
    }

    pub(crate) const fn flowchart_policy(&self) -> Option<FlowchartPresentationPolicy> {
        self.flowchart_policy
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlowchartPresentationPolicy {
    pub(crate) edge_corner_radius: Option<f64>,
    pub(crate) edge_label_padding: f64,
    pub(crate) compact_edge_corners: bool,
}

fn profile_defaults(profile: PresentationProfile) -> MermaidConfig {
    match profile {
        PresentationProfile::MermanModern => {
            let theme_variables = [
                ("mainBkg", "#F8FAFC"),
                ("nodeBorder", "#64748B"),
                ("nodeTextColor", "#1E293B"),
                ("primaryColor", "#F8FAFC"),
                ("primaryBorderColor", "#64748B"),
                ("primaryTextColor", "#1E293B"),
                ("lineColor", "#64748B"),
                ("arrowheadColor", "#64748B"),
                ("edgeLabelBackground", "#FFFFFF"),
                ("clusterBkg", "#F1F5F9"),
                ("clusterBorder", "#CBD5E1"),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), Value::String(value.to_string())))
            .collect();
            MermaidConfig::from_value(serde_json::json!({
                "theme": "redux",
                "look": "neo",
                "flowchart": { "defaultRenderer": "elk" },
                "themeVariables": Value::Object(theme_variables),
            }))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PresentationAspectApplicability {
    AllDiagrams,
    Family(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationAspectDescriptor {
    id: &'static str,
    applicability: PresentationAspectApplicability,
    required_capability_id: Option<&'static str>,
}

impl PresentationAspectDescriptor {
    pub const fn id(&self) -> &'static str {
        self.id
    }

    pub const fn applicability(&self) -> PresentationAspectApplicability {
        self.applicability
    }

    pub const fn required_capability_id(&self) -> Option<&'static str> {
        self.required_capability_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationProfileDescriptor {
    profile: PresentationProfile,
    aspects: &'static [PresentationAspectDescriptor],
}

impl PresentationProfileDescriptor {
    pub const fn id(&self) -> &'static str {
        self.profile.id()
    }

    pub const fn aspects(&self) -> &'static [PresentationAspectDescriptor] {
        self.aspects
    }
}

const MERMAN_MODERN_ASPECTS: [PresentationAspectDescriptor; 3] = [
    PresentationAspectDescriptor {
        id: "global-defaults",
        applicability: PresentationAspectApplicability::AllDiagrams,
        required_capability_id: None,
    },
    PresentationAspectDescriptor {
        id: "flowchart-svg",
        applicability: PresentationAspectApplicability::Family("flowchart"),
        required_capability_id: None,
    },
    PresentationAspectDescriptor {
        id: "flowchart-elk-default",
        applicability: PresentationAspectApplicability::Family("flowchart"),
        required_capability_id: Some("layout-elk"),
    },
];

const PRESENTATION_PROFILE_DESCRIPTORS: [PresentationProfileDescriptor; 1] =
    [PresentationProfileDescriptor {
        profile: PresentationProfile::MermanModern,
        aspects: &MERMAN_MODERN_ASPECTS,
    }];

pub const fn presentation_profile_descriptors() -> &'static [PresentationProfileDescriptor] {
    &PRESENTATION_PROFILE_DESCRIPTORS
}
