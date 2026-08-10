use std::collections::BTreeSet;
use std::sync::Arc;

use merman_core::MermaidConfig;

use super::admission::{FontCatalogKind, TextLayoutCapability, ThemeCapability};
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
#[derive(Debug, Clone)]
pub struct DiagramThemeCompiler {
    resources: ThemeResourcePolicy,
}

impl Default for DiagramThemeCompiler {
    fn default() -> Self {
        Self {
            resources: ThemeResourcePolicy::default(),
        }
    }
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
        let catalog = match spec.assets().font_catalog() {
            Some(catalog) => catalog.clone().compile(&self.resources)?,
            None => FontCatalog::default_parity(),
        };
        let inferred_capabilities = infer_required_capabilities(&spec);
        let inferred_text_capabilities = infer_required_text_capabilities(&spec, &catalog);
        let effective_requirements = spec
            .requirements()
            .clone()
            .with_required_capabilities(inferred_capabilities)
            .with_required_text_capabilities(inferred_text_capabilities);
        let mermaid_config = compile_mermaid_config(&spec);
        let fingerprint =
            super::canonical::recipe_fingerprint(&spec, &catalog, &effective_requirements);
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
            requirements: effective_requirements,
            mermaid_config,
            recipe_fingerprint: ThemeRecipeFingerprint::from_bytes(fingerprint),
            report,
        })))
    }

    pub fn compile_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        self.compile(preset.spec())
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
    if catalog.kind() == FontCatalogKind::Custom {
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

fn compile_mermaid_config(spec: &DiagramThemeSpec) -> MermaidConfig {
    super::mermaid_projection::compile(spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FilterRegion,
        ThemeColorValue, ThemeTarget,
    };

    fn shadow_graph() -> EffectGraph {
        EffectGraph::new(
            "shadow",
            FilterRegion::bounded(0.0, 0.0, 64.0, 64.0),
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
}
