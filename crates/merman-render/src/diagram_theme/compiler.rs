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
#[derive(Debug, Clone)]
pub struct DiagramThemeCompiler {
    resources: ThemeResourcePolicy,
    embedded_fonts_allowed: bool,
}

impl Default for DiagramThemeCompiler {
    fn default() -> Self {
        Self {
            resources: ThemeResourcePolicy::default(),
            embedded_fonts_allowed: cfg!(feature = "embedded-fonts"),
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

    /// Restricts embedded font processing to the calling artifact's capability contract.
    ///
    /// Enabling this policy cannot enable code omitted by Cargo's `embedded-fonts` feature.
    pub fn with_embedded_fonts_allowed(mut self, allowed: bool) -> Self {
        self.embedded_fonts_allowed &= allowed;
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

    /// Checks raw authoring JSON before a host normalizes or allocates its typed payload.
    ///
    /// This enforces the same structural and version admission as `compile_theme_definition_json`.
    /// It does not replace materialization or semantic compilation.
    pub fn check_definition_json_input(
        &self,
        bytes: &[u8],
    ) -> Result<(), merman_theme_contract::ThemeMaterializationErrorV1> {
        super::definition_admission::check_definition_json_input(&self.resources, bytes)
    }

    pub fn compile(
        &self,
        spec: DiagramThemeSpec,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        self.compile_with_sources(spec, super::source::ThemeSourceMap::default())
    }

    fn compile_with_sources(
        &self,
        spec: DiagramThemeSpec,
        source_map: super::source::ThemeSourceMap,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        self.validate_spec(&spec)?;
        let catalog = match spec.assets().font_catalog() {
            Some(catalog) => catalog.clone().compile_with_embedded_fonts_allowed(
                &self.resources,
                self.embedded_fonts_allowed,
            )?,
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
        let fingerprint =
            super::canonical::recipe_fingerprint(&spec, &catalog, &effective_requirements);
        let parse_compatibility =
            merman_core::__private::ThemeCompatibilityPlan::try_without_family_overlays(
                fingerprint,
                mermaid_config,
            )
            .expect("compiled Mermaid compatibility must satisfy the core plan contract");
        let parse_compatibility =
            crate::family::bind_theme_parse_defaults(parse_compatibility, &spec);
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
            source_map,
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

    fn validate_spec(&self, spec: &DiagramThemeSpec) -> Result<(), ThemeCompileError> {
        spec.validate()?;
        spec.effects().check_resources(&self.resources)?;
        Ok(())
    }

    /// Decodes and compiles one closed version 1 complete-spec wire under this compiler's policy.
    pub fn compile_spec_wire(
        &self,
        spec: super::DiagramThemeSpecWireV1,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        self.compile_spec_wire_with_sources(spec, super::source::ThemeWireSources::CompleteSpec)
    }

    pub(super) fn compile_spec_wire_with_sources(
        &self,
        spec: super::DiagramThemeSpecWireV1,
        sources: super::source::ThemeWireSources,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        let (spec, sources) = super::wire_decode::decode(spec, &self.resources, sources)?;
        self.compile_with_sources(spec, sources)
    }

    pub fn compile_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<super::DiagramTheme, super::ThemeDefinitionCompileError> {
        let spec = super::presets::materialize_spec_wire(preset, &self.resources)?;
        Ok(self.compile_spec_wire_with_sources(spec, super::source::ThemeWireSources::None)?)
    }

    /// Compiles a saved recipe through the same authoring and complete-spec admission paths.
    pub fn compile_recipe(
        &self,
        recipe: merman_theme_contract::ThemeRecipeV1,
    ) -> Result<super::DiagramTheme, super::ThemeDefinitionCompileError> {
        match recipe {
            merman_theme_contract::ThemeRecipeV1::Definition { definition } => {
                super::definition_admission::compile_theme_definition(self, &definition)
            }
            merman_theme_contract::ThemeRecipeV1::CompleteSpec { complete_spec } => {
                Ok(self.compile_spec_wire(complete_spec)?)
            }
        }
    }

    /// Exports one built-in preset as a self-contained editable recipe under this compiler's
    /// resource policy.
    pub fn export_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<
        merman_theme_contract::ThemeRecipeV1,
        merman_theme_contract::ThemeMaterializationErrorV1,
    > {
        let complete_spec = super::presets::materialize_spec_wire(preset, &self.resources)?;
        // Export validates the same complete recipe without constructing a compiled theme,
        // family cache or fingerprint. Built-in recipes never supply embedded font assets.
        let (spec, _) = super::wire_decode::decode(
            complete_spec.clone(),
            &self.resources,
            super::source::ThemeWireSources::None,
        )
        .map_err(preset_export_error)?;
        self.validate_spec(&spec).map_err(preset_export_error)?;
        Ok(merman_theme_contract::ThemeRecipeV1::CompleteSpec { complete_spec })
    }
}

fn preset_export_error(
    error: ThemeCompileError,
) -> merman_theme_contract::ThemeMaterializationErrorV1 {
    use merman_theme_contract::{ThemeMaterializationDiagnosticV1, ThemeMaterializationErrorV1};
    let diagnostic = match error {
        ThemeCompileError::ResourceLimit(error) => {
            ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
                "",
                error.limit,
                error.actual,
                error.max,
                "complete preset exceeds the caller-owned resource policy",
            )
        }
        error => ThemeMaterializationDiagnosticV1::invalid_token_value(
            "",
            "complete-preset-recipe",
            error.to_string(),
        ),
    };
    ThemeMaterializationErrorV1::from_diagnostic(diagnostic)
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

    #[test]
    fn diagnostic_sources_follow_interleaved_complete_spec_entries() {
        let wire = serde_json::from_value(serde_json::json!({"styles": [
            {"kind": "ordinal-palette", "target": "node", "colors": ["red"]},
            {"kind": "rule", "target": "node", "style": {"radius": 4}},
            {"kind": "ordinal-palette", "target": "pie-slice", "colors": ["blue"]},
            {"kind": "rule", "target": "text", "style": {"fill": "green"}}
        ]}))
        .unwrap();
        let theme = DiagramThemeCompiler::new().compile_spec_wire(wire).unwrap();
        let sources = theme.source_map();
        assert_eq!(sources.document().unwrap().id(), "complete_spec");
        assert_eq!(sources.rule(0).unwrap().paths, ["/styles/1"]);
        assert_eq!(sources.rule(1).unwrap().paths, ["/styles/3"]);
        assert_eq!(
            sources.palette(ThemeTarget::Node).unwrap().paths,
            ["/styles/0"]
        );
        assert_eq!(
            sources.palette(ThemeTarget::PieSlice).unwrap().paths,
            ["/styles/2"]
        );
        assert!(!sources.rule(0).unwrap().generated);
    }

    #[test]
    fn diagnostic_sources_preserve_definition_tokens_and_authored_palette_override() {
        let definition = serde_json::from_value(serde_json::json!({
            "authoring_schema_version": 1, "expansion_version": 1,
            "tokens": {"text": "green", "series": ["orange"]},
            "styles": [
                {"kind": "ordinal-palette", "target": "node", "colors": ["red"]},
                {"kind": "rule", "target": "node", "style": {"radius": 4}}
            ]
        }))
        .unwrap();
        let compiler = DiagramThemeCompiler::new();
        let theme = super::super::compile_theme_definition(&compiler, &definition).unwrap();
        let sources = theme.source_map();
        assert_eq!(sources.document().unwrap().id(), "definition");
        assert_eq!(sources.rule(0).unwrap().paths, ["/tokens/text"]);
        assert!(sources.rule(0).unwrap().generated);
        assert!(sources.rule(2).unwrap().paths.is_empty());
        assert!(sources.rule(2).unwrap().generated);
        let authored_rule = theme.spec().styles().rules().len() - 1;
        assert_eq!(sources.rule(authored_rule).unwrap().paths, ["/styles/1"]);
        assert!(!sources.rule(authored_rule).unwrap().generated);
        assert_eq!(
            sources.palette(ThemeTarget::Node).unwrap().paths,
            ["/styles/0"]
        );
        assert!(!sources.palette(ThemeTarget::Node).unwrap().generated);
        assert_eq!(
            sources.palette(ThemeTarget::PieSlice).unwrap().paths,
            ["/tokens/series"]
        );
        assert!(sources.palette(ThemeTarget::PieSlice).unwrap().generated);

        let materialized = super::super::materialize_theme(&definition).unwrap();
        let imported = compiler
            .compile_spec_wire(materialized.into_spec())
            .unwrap();
        assert_eq!(theme.recipe_fingerprint(), imported.recipe_fingerprint());
        assert_eq!(
            imported.source_map().rule(authored_rule).unwrap().paths,
            [format!("/styles/{authored_rule}")]
        );
    }

    #[test]
    fn diagnostic_sources_distinguish_generated_defaults_from_explicit_tokens() {
        let compiler = DiagramThemeCompiler::new();
        let default_definition =
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#;
        let default =
            super::super::compile_theme_definition_json(&compiler, default_definition).unwrap();
        let source = default.source_map().palette(ThemeTarget::Node).unwrap();
        assert!(source.generated);
        assert!(source.paths.is_empty());
        assert!(default.source_map().canvas_base().unwrap().paths.is_empty());
        assert!(default.source_map().canvas_base().unwrap().generated);

        let explicit = super::super::compile_theme_definition_json(&compiler,
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"line":"red","canvas":"blue"}}"#
        ).unwrap();
        let marker_index = explicit
            .spec()
            .styles()
            .rules()
            .iter()
            .position(|rule| rule.target() == ThemeTarget::TransitionMarker)
            .unwrap();
        assert_eq!(
            explicit.source_map().rule(marker_index).unwrap().paths,
            ["/tokens/line"]
        );
        assert_eq!(
            explicit.source_map().canvas_base().unwrap().paths,
            ["/tokens/canvas"]
        );
        let imported = compiler
            .compile_spec_wire(
                serde_json::from_value(serde_json::json!({"canvas": {"base": "blue"}})).unwrap(),
            )
            .unwrap();
        assert_eq!(
            imported.source_map().canvas_base().unwrap().paths,
            ["/canvas/base"]
        );
        assert!(!imported.source_map().canvas_base().unwrap().generated);
    }

    #[test]
    fn diagnostic_sources_do_not_invent_documents_for_typed_or_builtin_themes() {
        let compiler = DiagramThemeCompiler::new();
        let typed = compiler.compile(DiagramThemeSpec::new()).unwrap();
        assert!(typed.source_map().document().is_none());
        assert!(typed.source_map().rule(0).is_none());
        let preset = compiler
            .compile_preset(super::super::ThemePreset::Cyberpunk)
            .unwrap();
        assert!(preset.source_map().document().is_none());
        assert!(preset.source_map().rule(0).is_none());
        let imported = compiler
            .compile_recipe(
                compiler
                    .export_preset(super::super::ThemePreset::Cyberpunk)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(
            imported.source_map().document().unwrap().id(),
            "complete_spec"
        );
        assert_eq!(imported.source_map().rule(0).unwrap().paths, ["/styles/0"]);
        assert_eq!(preset.recipe_fingerprint(), imported.recipe_fingerprint());
    }

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
    fn parsing_a_compiled_theme_does_not_prepare_legacy_family_programs() {
        use crate::diagram_theme::{CanvasPaint, ThemeRule, ThemeRuleSet, ThemeStylePatch};

        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                    ),
                )),
            )
            .unwrap();
        let engine = theme.install_parse_compatibility(merman_core::Engine::new());
        for source in [
            "block\nA --> B",
            "flowchart TD\nA --> B",
            "sequenceDiagram\nA->>B: Message",
            "stateDiagram-v2\nA --> B",
        ] {
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
                .unwrap();
            assert!(parsed.is_some(), "fixture must reach family detection");
            assert_eq!(theme.0.family_programs.len(), 0, "{source}");
        }
        theme.resolve(crate::DiagramFamilyId::BLOCK);
        assert_eq!(theme.0.family_programs.len(), 1);
        assert!(
            theme
                .0
                .family_programs
                .contains(crate::DiagramFamilyId::BLOCK)
        );
    }

    #[test]
    fn direct_invalid_ordinal_variants_are_rejected_at_compile_time() {
        use crate::diagram_theme::{
            CanvasPaint, OrdinalSelector, ThemeRule, ThemeRuleSet, ThemeStylePatch,
        };
        for (ordinal, field) in [
            (OrdinalSelector::Exact(0), "style.ordinal.index"),
            (
                OrdinalSelector::Cycle {
                    period: 0,
                    offset: 0,
                },
                "style.ordinal",
            ),
            (
                OrdinalSelector::Cycle {
                    period: 2,
                    offset: 2,
                },
                "style.ordinal",
            ),
        ] {
            let rule = ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
            )
            .with_ordinal(ordinal);
            let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
            assert!(
                matches!(DiagramThemeCompiler::new().compile(spec),
                Err(ThemeCompileError::Validation(ThemeCompileValidationError::InvalidNumber { field: actual }))
                    if actual == field),
                "invalid ordinal {ordinal:?} must not reach rendering"
            );
        }
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
