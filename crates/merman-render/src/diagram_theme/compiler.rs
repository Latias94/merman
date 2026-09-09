use std::collections::BTreeSet;
use std::sync::Arc;

use super::admission::{TextLayoutCapability, ThemeCapability};
use super::application::root_theme_requirements;
use super::assets::{FontCatalog, FontCatalogError};
use super::mechanisms::{
    collect_effect_graph_capabilities, collect_style_patch_capabilities,
    collect_typography_capabilities,
};
use super::resources::{ThemeResourceLimitExceeded, ThemeResourcePolicy};
use super::spec::DiagramThemeSpec;
use super::typography::Specified;
use super::{ThemeCompileValidationError, ThemeRecipeFingerprint, ThemeRecipeReport};

/// Resource-bounded compiler for one complete typed theme recipe.
#[derive(Debug, Clone, Default)]
pub struct DiagramThemeCompiler {
    resources: ThemeResourcePolicy,
}

impl DiagramThemeCompiler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_resource_policy(mut self, resources: ThemeResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    pub const fn resource_policy(&self) -> &ThemeResourcePolicy {
        &self.resources
    }

    /// Rejects an encoded theme input before its typed object graph is allocated.
    pub fn check_encoded_input_bytes(
        &self,
        actual: usize,
    ) -> Result<(), ThemeResourceLimitExceeded> {
        self.resources.check_theme_encoded_bytes(actual)
    }

    pub fn compile(
        &self,
        spec: DiagramThemeSpec,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        spec.validate()?;
        spec.effects().check_resources(&self.resources)?;
        let catalog = match spec.assets().font_catalog() {
            Some(catalog) => catalog.clone().compile(&self.resources)?,
            None => FontCatalog::system_fonts(),
        };
        let inferred_capabilities = infer_required_capabilities(&spec);
        let inferred_text_capabilities = infer_required_text_capabilities(&spec, &catalog);
        let effective_requirements = spec
            .requirements()
            .clone()
            .with_required_capabilities(inferred_capabilities)
            .with_required_text_capabilities(inferred_text_capabilities);
        let spec = Arc::new(spec);
        let mermaid_config = super::mermaid_compatibility::compile(&spec);
        let family_programs = Arc::new(super::family_program::FamilyThemeProgramCache::new(
            Arc::clone(&spec),
        ));
        let legacy_family_theme_bridge =
            super::legacy_family_theme_bridge::LegacyFamilyThemeBridge::new(Arc::clone(
                &family_programs,
            ));
        let fingerprint =
            super::canonical::recipe_fingerprint(&spec, &catalog, &effective_requirements);
        let parse_compatibility_bridge = legacy_family_theme_bridge.clone();
        let mut parse_compatibility = merman_core::__private::ThemeCompatibilityPlan::try_new(
            fingerprint,
            mermaid_config,
            move |family, control| parse_compatibility_bridge.overlay_for_family(family, control),
        )
        .expect("compiled Mermaid compatibility must satisfy the core plan contract");
        if spec.styles().rules().iter().any(|rule| {
            rule.target() == super::ThemeTarget::Node
                && rule
                    .family()
                    .is_none_or(|family| family == crate::DiagramFamilyId::GIT_GRAPH)
                && (!matches!(rule.style().paint.fill, super::Specified::Unspecified)
                    || !matches!(rule.style().stroke.paint, super::Specified::Unspecified))
        }) {
            parse_compatibility = parse_compatibility
                .try_with_post_detection_default_paths(
                    crate::DiagramFamilyId::GIT_GRAPH.as_str(),
                    &crate::gitgraph::GITGRAPH_NODE_PAINT_PATHS,
                )
                .expect("GitGraph writer default paths satisfy the bounded core contract");
        }
        if spec.styles().rules().iter().any(|rule| {
            matches!(
                rule.target(),
                super::ThemeTarget::Text | super::ThemeTarget::Relation | super::ThemeTarget::Table
            ) && rule
                .family()
                .is_none_or(|family| family == crate::DiagramFamilyId::ER)
                && (!matches!(rule.style().paint.fill, super::Specified::Unspecified)
                    || !matches!(rule.style().stroke.paint, super::Specified::Unspecified))
        }) {
            parse_compatibility = parse_compatibility
                .try_with_post_detection_default_paths(
                    crate::DiagramFamilyId::ER.as_str(),
                    &crate::er::ER_PAINT_DEFAULT_PATHS,
                )
                .expect("ER writer default paths satisfy the bounded core contract");
        }
        let report = ThemeRecipeReport::compiled(
            ThemeRecipeFingerprint::from_bytes(fingerprint),
            catalog.fingerprint(),
            effective_requirements
                .required_capabilities()
                .collect::<BTreeSet<_>>(),
            effective_requirements
                .required_text_capabilities()
                .collect::<BTreeSet<_>>(),
        );
        Ok(super::DiagramTheme(Arc::new(super::CompiledDiagramTheme {
            spec,
            catalog,
            resource_restriction: self.resources.clone(),
            requirements: effective_requirements,
            parse_compatibility,
            family_programs,
            recipe_fingerprint: ThemeRecipeFingerprint::from_bytes(fingerprint),
            report,
        })))
    }

    /// Decodes and compiles one closed version 1 complete-spec wire under this compiler's policy.
    pub fn compile_spec_wire(
        &self,
        spec: super::DiagramThemeSpecWireV1,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        let spec = super::wire_decode::decode(spec, &self.resources)?;
        self.compile(spec)
    }

    pub fn compile_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<super::DiagramTheme, super::ThemeDefinitionCompileError> {
        let spec = super::presets::materialize_spec_wire(preset, &self.resources)?;
        Ok(self.compile_spec_wire(spec)?)
    }

    /// Exports one built-in preset as a self-contained editable recipe under this compiler's
    /// resource policy.
    pub fn export_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<
        merman_theme_contract::PresetExportV1,
        merman_theme_contract::ThemeMaterializationErrorV1,
    > {
        Ok(merman_theme_contract::PresetExportV1::CompleteSpec {
            complete_spec: super::presets::materialize_spec_wire(preset, &self.resources)?,
        })
    }
}

fn infer_required_capabilities(spec: &DiagramThemeSpec) -> BTreeSet<ThemeCapability> {
    let mut required = BTreeSet::new();
    if !spec.styles().rules().is_empty() {
        required.insert(ThemeCapability::SemanticTokens);
        required.insert(ThemeCapability::SemanticRules);
    }
    if !spec.styles().ordinal_palettes().is_empty() {
        required.insert(ThemeCapability::OrdinalPalette);
    }
    collect_typography_capabilities(spec.typography(), &mut required);
    required.extend(
        root_theme_requirements(spec)
            .iter()
            .flat_map(|requirement| requirement.capabilities()),
    );
    for rule in spec.styles().rules() {
        collect_style_patch_capabilities(rule.style(), &mut required);
    }

    let mut referenced_effects = BTreeSet::new();
    for binding in spec.effects().bindings() {
        referenced_effects.insert(binding.effect_id());
    }
    for rule in spec.styles().rules() {
        if let Specified::Value(effect_id) = &rule.style().effects.effect {
            referenced_effects.insert(effect_id.as_str());
        }
    }
    for effect_id in referenced_effects {
        let graph = spec
            .effects()
            .graph(effect_id)
            .expect("validated effect reference must resolve");
        collect_effect_graph_capabilities(graph, &mut required);
    }
    required
}

fn infer_required_text_capabilities(
    _spec: &DiagramThemeSpec,
    catalog: &FontCatalog,
) -> BTreeSet<TextLayoutCapability> {
    let mut required = BTreeSet::new();
    if catalog.requires_prepared_text_layout() {
        // A custom catalog is an explicit geometry input. It cannot be consumed by the legacy
        // system-font measurer, so the prepared lane must attest its catalog and shaping support.
        required.extend([
            TextLayoutCapability::CatalogBinding,
            TextLayoutCapability::UnicodeClusterFallback,
            TextLayoutCapability::OpenTypeShaping,
        ]);
    }
    required
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeCompileError {
    #[error(transparent)]
    Validation(#[from] ThemeCompileValidationError),
    #[error(transparent)]
    FontCatalog(#[from] FontCatalogError),
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
        ThemeColorValue, ThemeResourceLimitId, ThemeResourceLimitPhase, ThemeResourcePolicy,
        ThemeTarget,
    };

    fn shadow_graph() -> EffectGraph {
        EffectGraph::new(
            "shadow",
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 1.0,
                offset_y: 1.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#00000080").expect("valid shadow color"),
            }],
        )
        .expect("valid shadow graph")
    }

    #[test]
    fn unused_effect_graph_does_not_expand_required_capabilities() {
        let effects = DiagramEffectSet::default()
            .with_graph(shadow_graph())
            .expect("add unused graph");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_effects(effects))
            .expect("compile theme");

        assert!(
            !theme
                .report()
                .requires_capability(ThemeCapability::SvgFilter)
        );
        assert!(!theme.report().requires_capability(ThemeCapability::Shadow));
    }

    #[test]
    fn referenced_effect_binding_expands_only_the_referenced_graph_capabilities() {
        let effects = DiagramEffectSet::default()
            .with_graph(shadow_graph())
            .expect("add graph")
            .with_binding(
                EffectBinding::new(ThemeTarget::Node, "shadow").expect("valid effect binding"),
            )
            .expect("add binding");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_effects(effects))
            .expect("compile theme");

        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::SvgFilter)
        );
        assert!(theme.report().requires_capability(ThemeCapability::Shadow));
    }

    #[test]
    fn compiler_enforces_effect_count_resource_limits() {
        assert_effect_resource_limit(
            ThemeResourceLimitId::MaxEffectGraphs,
            0,
            DiagramEffectSet::default()
                .with_graph(shadow_graph())
                .expect("add graph"),
            1,
        );

        let graph = EffectGraph::new(
            "two-primitives",
            [
                EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                },
                EffectPrimitive::GaussianBlur {
                    input: EffectInput::Previous,
                    std_deviation: 1.0,
                },
            ],
        )
        .expect("valid two-primitive graph");
        assert_effect_resource_limit(
            ThemeResourceLimitId::MaxEffectPrimitivesPerGraph,
            1,
            DiagramEffectSet::default()
                .with_graph(graph)
                .expect("add graph"),
            2,
        );

        let effects = DiagramEffectSet::default()
            .with_graph(shadow_graph())
            .expect("add graph")
            .with_binding(EffectBinding::new(ThemeTarget::Node, "shadow").expect("valid binding"))
            .expect("add binding");
        assert_effect_resource_limit(ThemeResourceLimitId::MaxEffectBindings, 0, effects, 1);
    }

    #[test]
    fn compiler_enforces_effect_parameter_resource_limits() {
        let color = || ThemeColorValue::parse("#00000080").expect("valid shadow color");
        let cases = [
            (
                ThemeResourceLimitId::MaxEffectOffsetMagnitude,
                EffectGraph::new(
                    "offset",
                    [EffectPrimitive::DropShadow {
                        input: EffectInput::SourceGraphic,
                        offset_x: -2.0,
                        offset_y: 0.0,
                        blur_radius: 0.0,
                        spread: 0.0,
                        color: color(),
                    }],
                )
                .expect("valid offset graph"),
            ),
            (
                ThemeResourceLimitId::MaxEffectBlurMagnitude,
                EffectGraph::new(
                    "blur",
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 2.0,
                    }],
                )
                .expect("valid blur graph"),
            ),
            (
                ThemeResourceLimitId::MaxEffectDisplacementScale,
                EffectGraph::new(
                    "displacement",
                    [EffectPrimitive::Displacement {
                        input: EffectInput::SourceGraphic,
                        map_input: EffectInput::SourceGraphic,
                        scale: 2.0,
                    }],
                )
                .expect("valid displacement graph"),
            ),
            (
                ThemeResourceLimitId::MaxEffectTurbulenceOctaves,
                EffectGraph::new(
                    "turbulence",
                    [EffectPrimitive::Turbulence {
                        input: EffectInput::SourceGraphic,
                        base_frequency_x: 0.1,
                        base_frequency_y: 0.1,
                        octaves: 2,
                        seed: 7,
                    }],
                )
                .expect("valid turbulence graph"),
            ),
        ];

        for (limit, graph) in cases {
            assert_effect_resource_limit(
                limit,
                1,
                DiagramEffectSet::default()
                    .with_graph(graph)
                    .expect("add graph"),
                2,
            );
        }
    }

    #[test]
    fn drop_shadow_spread_is_charged_as_effect_blur_magnitude() {
        let graph = EffectGraph::new(
            "spread",
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 0.0,
                offset_y: 0.0,
                blur_radius: 0.0,
                spread: 2.0,
                color: ThemeColorValue::parse("#00000080").expect("valid shadow color"),
            }],
        )
        .expect("valid spread graph");

        assert_effect_resource_limit(
            ThemeResourceLimitId::MaxEffectBlurMagnitude,
            1,
            DiagramEffectSet::default()
                .with_graph(graph)
                .expect("add graph"),
            2,
        );
    }

    fn assert_effect_resource_limit(
        limit: ThemeResourceLimitId,
        max: usize,
        effects: DiagramEffectSet,
        expected_actual: usize,
    ) {
        let policy = ThemeResourcePolicy::interactive()
            .with_limit(limit, max)
            .expect("valid effect resource override");
        let error = DiagramThemeCompiler::new()
            .with_resource_policy(policy)
            .compile(DiagramThemeSpec::new().with_effects(effects))
            .expect_err("effect resource limit must reject theme compilation");

        assert!(matches!(
            error,
            ThemeCompileError::ResourceLimit(ThemeResourceLimitExceeded {
                phase: ThemeResourceLimitPhase::EffectCompile,
                limit: actual_id,
                actual,
                max: actual_max,
                ..
            }) if actual_id == limit.as_str() && actual == expected_actual && actual_max == max
        ));
    }
}
