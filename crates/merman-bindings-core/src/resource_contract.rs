use std::collections::BTreeMap;

/// One resource limit exposed by the capabilities compiled into this binding artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingResourceLimitDescriptor {
    pub stable_id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub hard_cap: bool,
    pub minimum_value: usize,
}

/// One shared resource profile projected across every compiled resource owner.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingResourceProfileDescriptor {
    pub id: &'static str,
    pub purpose: &'static str,
    pub trust_assumption: &'static str,
    pub recommended_binding_default: bool,
    pub limits: BTreeMap<&'static str, Option<usize>>,
}

/// Complete resource contract for one concrete binding feature closure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingResourceContract {
    pub general_binding_default_profile: &'static str,
    pub cli_default_profile: &'static str,
    pub limits: Vec<BindingResourceLimitDescriptor>,
    pub profiles: Vec<BindingResourceProfileDescriptor>,
}

/// Composes resource descriptors without redefining limits owned by lower-level crates.
pub fn binding_resource_contract() -> BindingResourceContract {
    let limits = binding_resource_limit_descriptors();
    let profiles = merman::resources::RESOURCE_PROFILE_DESCRIPTORS
        .iter()
        .map(|profile| BindingResourceProfileDescriptor {
            id: profile.id,
            purpose: profile.purpose,
            trust_assumption: profile.trust_assumption,
            recommended_binding_default: profile.recommended_binding_default,
            limits: limits
                .iter()
                .map(|limit| {
                    (
                        limit.stable_id,
                        resource_profile_value(profile.profile, limit.stable_id)
                            .expect("compiled resource descriptors must have profile values"),
                    )
                })
                .collect(),
        })
        .collect();

    BindingResourceContract {
        general_binding_default_profile:
            merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE.id(),
        cli_default_profile: merman::resources::CLI_DEFAULT_RESOURCE_PROFILE.id(),
        limits,
        profiles,
    }
}

fn binding_resource_limit_descriptors() -> Vec<BindingResourceLimitDescriptor> {
    #[allow(unused_mut)]
    let mut limits = merman::resources::INPUT_RESOURCE_LIMIT_DESCRIPTORS
        .iter()
        .map(|descriptor| BindingResourceLimitDescriptor {
            stable_id: descriptor.stable_id,
            phase: descriptor.phase.as_str(),
            description: descriptor.description,
            overridable: descriptor.overridable,
            hard_cap: false,
            minimum_value: descriptor.minimum_value,
        })
        .collect::<Vec<_>>();

    #[cfg(feature = "svg")]
    limits.extend(
        merman::svg::resource_limit_descriptors()
            .iter()
            .filter(|descriptor| matches!(descriptor.id, merman::svg::ResourceLimitId::Render(_)))
            .map(|descriptor| BindingResourceLimitDescriptor {
                stable_id: descriptor.stable_id,
                phase: descriptor.phase.as_str(),
                description: descriptor.description,
                overridable: descriptor.overridable,
                hard_cap: descriptor.hard_cap,
                minimum_value: descriptor.minimum_value,
            }),
    );

    #[cfg(feature = "analysis")]
    limits.extend(
        merman_analysis::ANALYSIS_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .map(|descriptor| BindingResourceLimitDescriptor {
                stable_id: descriptor.stable_id,
                phase: descriptor.phase,
                description: descriptor.description,
                overridable: descriptor.overridable,
                hard_cap: false,
                minimum_value: descriptor.minimum_value,
            }),
    );

    #[cfg(feature = "ascii")]
    limits.extend(
        merman::ascii::ASCII_RESOURCE_LIMIT_DESCRIPTORS
            .iter()
            .map(|descriptor| BindingResourceLimitDescriptor {
                stable_id: descriptor.stable_id,
                phase: descriptor.phase.as_str(),
                description: descriptor.description,
                overridable: descriptor.overridable,
                hard_cap: false,
                minimum_value: descriptor.minimum_value,
            }),
    );

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    limits.extend(
        merman::svg::export::export_resource_limit_descriptors()
            .into_iter()
            .map(|descriptor| BindingResourceLimitDescriptor {
                stable_id: descriptor.stable_id,
                phase: descriptor.phase,
                description: descriptor.description,
                overridable: descriptor.overridable,
                hard_cap: descriptor.hard_cap,
                minimum_value: descriptor.minimum_value,
            }),
    );

    limits
}

pub(crate) fn resource_limit_descriptor(stable_id: &str) -> Option<BindingResourceLimitDescriptor> {
    binding_resource_limit_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.stable_id == stable_id)
}

pub(crate) fn resource_profile_value(
    profile: merman::resources::ResourceProfile,
    stable_id: &str,
) -> Option<Option<usize>> {
    if let Some(id) = merman::resources::InputResourceLimitId::from_stable_id(stable_id) {
        return Some(merman::resources::InputResourcePolicy::for_profile(profile).value(id));
    }

    #[cfg(feature = "svg")]
    if let Some(id @ merman::svg::ResourceLimitId::Render(_)) =
        merman::svg::ResourceLimitId::from_stable_id(stable_id)
    {
        return Some(merman::svg::RenderResourcePolicy::for_profile(profile).value(id));
    }

    #[cfg(feature = "analysis")]
    if stable_id == merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID {
        return Some(merman_analysis::analysis_resource_profile_value(
            profile, stable_id,
        ));
    }

    #[cfg(feature = "ascii")]
    if merman::ascii::AsciiResourceLimitId::from_stable_id(stable_id).is_some() {
        return Some(merman::ascii::ascii_resource_profile_value(
            profile, stable_id,
        ));
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    if let Some(value) = merman::svg::export::export_resource_profile_value(profile, stable_id) {
        return Some(value);
    }

    None
}

pub(crate) fn resource_profile_value_for_target(
    profile: merman::resources::ResourceProfile,
    stable_id: &str,
    target: crate::TargetKey,
) -> Option<Option<usize>> {
    let value = resource_profile_value(profile, stable_id)?;

    #[cfg(feature = "svg")]
    if matches!(target, crate::TargetKey::Web)
        && stable_id == merman::svg::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID
    {
        return Some(Some(merman::svg::WASM_RESVG_TREE_DEPTH_HARD_CAP));
    }

    #[cfg(not(feature = "svg"))]
    let _ = target;

    Some(value)
}

#[allow(dead_code)]
pub(crate) enum BindingResourceOwner {
    Artifact,
    Capability(&'static str),
    Outputs(&'static [&'static str]),
}

pub(crate) fn resource_limit_owner(stable_id: &str) -> BindingResourceOwner {
    if merman::resources::InputResourceLimitId::from_stable_id(stable_id).is_some() {
        return BindingResourceOwner::Artifact;
    }

    #[cfg(feature = "analysis")]
    if stable_id == merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID {
        return BindingResourceOwner::Capability("analysis");
    }

    #[cfg(feature = "ascii")]
    if merman::ascii::AsciiResourceLimitId::from_stable_id(stable_id).is_some() {
        return BindingResourceOwner::Capability("ascii");
    }

    #[cfg(feature = "svg")]
    if matches!(
        merman::svg::ResourceLimitId::from_stable_id(stable_id),
        Some(merman::svg::ResourceLimitId::Render(_))
    ) {
        return BindingResourceOwner::Capability("svg");
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    if let Some(output_ids) = merman::svg::export::export_resource_limit_output_ids(stable_id) {
        return BindingResourceOwner::Outputs(output_ids);
    }

    unreachable!("compiled resource descriptor `{stable_id}` must have an owner")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingResourceScope {
    AnalysisDiagram,
    DocumentAnalysis,
    Model,
    Ascii,
    Layout,
    Svg,
    Png,
    Jpeg,
    Pdf,
}

impl BindingResourceScope {
    pub(crate) fn accepts(self, stable_id: &str) -> bool {
        let input = merman::resources::InputResourceLimitId::from_stable_id(stable_id);
        let analysis_document = is_analysis_document_limit(stable_id);
        let ascii = is_ascii_limit(stable_id);
        let render = is_render_limit(stable_id);
        let export_outputs = export_limit_output_ids(stable_id);

        match self {
            Self::AnalysisDiagram => {
                input == Some(merman::resources::InputResourceLimitId::MaxSourceBytes)
            }
            Self::DocumentAnalysis => {
                analysis_document
                    || input == Some(merman::resources::InputResourceLimitId::MaxSourceBytes)
            }
            Self::Model => input.is_some(),
            Self::Ascii => input.is_some() || ascii,
            Self::Layout => input.is_some() || stable_id == "max_layout_work_units",
            Self::Svg => input.is_some() || render,
            Self::Png => {
                input.is_some()
                    || render
                    || export_outputs.is_some_and(|outputs| outputs.contains(&"png"))
            }
            Self::Jpeg => {
                input.is_some()
                    || render
                    || export_outputs.is_some_and(|outputs| outputs.contains(&"jpeg"))
            }
            Self::Pdf => {
                input.is_some()
                    || render
                    || export_outputs.is_some_and(|outputs| outputs.contains(&"pdf"))
            }
        }
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::AnalysisDiagram => "single-diagram analysis",
            Self::DocumentAnalysis => "host-document analysis",
            Self::Model => "semantic-model",
            Self::Ascii => "ASCII render",
            Self::Layout => "layout",
            Self::Svg => "SVG render",
            Self::Png => "PNG export",
            Self::Jpeg => "JPEG export",
            Self::Pdf => "PDF export",
        }
    }
}

fn is_analysis_document_limit(stable_id: &str) -> bool {
    #[cfg(feature = "analysis")]
    {
        stable_id == merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID
    }
    #[cfg(not(feature = "analysis"))]
    {
        let _ = stable_id;
        false
    }
}

fn is_ascii_limit(stable_id: &str) -> bool {
    #[cfg(feature = "ascii")]
    {
        merman::ascii::AsciiResourceLimitId::from_stable_id(stable_id).is_some()
    }
    #[cfg(not(feature = "ascii"))]
    {
        let _ = stable_id;
        false
    }
}

fn is_render_limit(stable_id: &str) -> bool {
    #[cfg(feature = "svg")]
    {
        matches!(
            merman::svg::ResourceLimitId::from_stable_id(stable_id),
            Some(merman::svg::ResourceLimitId::Render(_))
        )
    }
    #[cfg(not(feature = "svg"))]
    {
        let _ = stable_id;
        false
    }
}

fn export_limit_output_ids(stable_id: &str) -> Option<&'static [&'static str]> {
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    {
        merman::svg::export::export_resource_limit_output_ids(stable_id)
    }
    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    {
        let _ = stable_id;
        None
    }
}

#[cfg(all(test, feature = "ascii"))]
mod ascii_tests {
    use super::*;

    #[test]
    fn ascii_resource_descriptors_have_profile_values_owner_and_scope() {
        let contract = binding_resource_contract();
        let ascii_limits = contract
            .limits
            .iter()
            .filter(|limit| {
                merman::ascii::AsciiResourceLimitId::from_stable_id(limit.stable_id).is_some()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            ascii_limits.len(),
            merman::ascii::ASCII_RESOURCE_LIMIT_COUNT
        );
        for descriptor in ascii_limits {
            let id = merman::ascii::AsciiResourceLimitId::from_stable_id(descriptor.stable_id)
                .expect("ASCII descriptor must resolve by stable id");
            assert_eq!(descriptor.phase, id.descriptor().phase.as_str());
            assert!(matches!(
                resource_limit_owner(descriptor.stable_id),
                BindingResourceOwner::Capability("ascii")
            ));
            assert!(BindingResourceScope::Ascii.accepts(descriptor.stable_id));
            for profile in &contract.profiles {
                assert!(profile.limits.contains_key(descriptor.stable_id));
            }
        }
    }
}

#[cfg(all(test, feature = "png", feature = "jpeg", feature = "pdf"))]
mod tests {
    use super::*;

    #[test]
    fn svg_backend_tree_caps_are_render_owned_and_apply_to_svg_and_exports() {
        let contract = binding_resource_contract();
        for stable_id in ["svg_backend_tree_nodes", "svg_backend_tree_depth"] {
            let descriptor = contract
                .limits
                .iter()
                .find(|descriptor| descriptor.stable_id == stable_id)
                .expect("SVG backend hard cap descriptor");
            assert_eq!(descriptor.phase, "svg_postprocess");
            assert!(!descriptor.overridable);
            assert!(descriptor.hard_cap);
            assert!(matches!(
                resource_limit_owner(stable_id),
                BindingResourceOwner::Capability("svg")
            ));
            for scope in [
                BindingResourceScope::Svg,
                BindingResourceScope::Png,
                BindingResourceScope::Jpeg,
                BindingResourceScope::Pdf,
            ] {
                assert!(scope.accepts(stable_id));
            }
        }
    }

    #[test]
    fn export_contract_separates_policy_limits_from_backend_hard_caps() {
        let contract = binding_resource_contract();
        let export_limits = contract
            .limits
            .iter()
            .filter(|limit| {
                matches!(
                    resource_limit_owner(limit.stable_id),
                    BindingResourceOwner::Outputs(_)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            export_limits
                .iter()
                .filter(|limit| limit.overridable)
                .count(),
            8
        );
        assert_eq!(
            export_limits.iter().filter(|limit| limit.hard_cap).count(),
            5
        );
        assert!(
            export_limits
                .iter()
                .all(|limit| limit.overridable != limit.hard_cap)
        );
        assert!(matches!(
            resource_limit_owner("max_pdf_filter_image_pixels"),
            BindingResourceOwner::Outputs(["pdf"])
        ));

        let unbounded = contract
            .profiles
            .iter()
            .find(|profile| profile.id == "unbounded-for-trusted-input")
            .expect("unbounded profile");
        for limit in export_limits.iter().filter(|limit| limit.hard_cap) {
            assert!(
                unbounded.limits[limit.stable_id].is_some(),
                "{} must remain active for trusted input",
                limit.stable_id
            );
        }
        for limit in export_limits.iter().filter(|limit| limit.overridable) {
            assert_eq!(unbounded.limits[limit.stable_id], None);
        }
    }
}
