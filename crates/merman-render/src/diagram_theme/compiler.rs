use std::collections::BTreeSet;
use std::sync::Arc;

use super::admission::ThemeCapability;
use super::application::RootThemePlan;
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

    /// Checks bounded editable-design JSON before a host normalizes its object graph.
    pub fn check_design_json_input(
        &self,
        bytes: &[u8],
    ) -> Result<(), merman_theme_contract::ThemeMaterializationErrorV1> {
        super::design_materializer::check_design_json_input(&self.resources, bytes)
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
            Some(catalog) => catalog.clone().compile(&self.resources)?,
            None => FontCatalog::system_fonts(),
        };
        let root_plan = Arc::new(RootThemePlan::from_spec(&spec));
        let inferred_capabilities = infer_required_capabilities(&spec, &root_plan);
        let effective_requirements = spec
            .requirements()
            .clone()
            .with_required_capabilities(inferred_capabilities);
        let spec = Arc::new(spec);
        let mermaid_config = super::mermaid_compatibility::compile(&spec);
        let family_programs = Arc::new(super::family_program::FamilyThemeProgramCache::new(
            Arc::clone(&spec),
        ));
        let fingerprint =
            super::canonical::recipe_fingerprint(&spec, &catalog, &effective_requirements);
        let compatibility_recipe = merman_model::presentation::ThemeCompatibilityRecipe::try_new(
            fingerprint,
            mermaid_config,
        )?;
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
            compatibility_recipe,
            family_programs,
            root_plan,
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
    ///
    /// Encoded-byte admission counts the compact JSON payload, without a recipe or transport
    /// envelope. Hosts accepting raw JSON must also admit their full encoded input before decoding.
    pub fn compile_spec_wire(
        &self,
        spec: super::DiagramThemeSpecWireV1,
    ) -> Result<super::DiagramTheme, ThemeCompileError> {
        super::definition_admission::check_typed_input_encoded_bytes(&self.resources, &spec)
            .map_err(|error| match error {
                super::definition_admission::ThemeInputEncodingError::ResourceLimit(error) => {
                    ThemeCompileError::ResourceLimit(error)
                }
                super::definition_admission::ThemeInputEncodingError::InvalidEncoding => {
                    ThemeCompileValidationError::InvalidValue {
                        field: "complete_spec",
                    }
                    .into()
                }
            })?;
        self.compile_spec_wire_with_sources(spec, super::source::ThemeWireSources::CompleteSpec)
    }

    // Designs and presets admit their authored or composed wire before using this path.
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
    ) -> Result<super::DiagramTheme, super::ThemeDesignCompileError> {
        let spec = super::presets::materialize_spec_wire(preset, &self.resources)?;
        Ok(self.compile_spec_wire_with_sources(spec, super::source::ThemeWireSources::None)?)
    }

    pub(super) fn validate_preset(
        &self,
        preset: super::ThemePreset,
    ) -> Result<(), super::ThemeDesignCompileError> {
        let spec = super::presets::materialize_spec_wire(preset, &self.resources)?;
        self.validate_preset_spec_wire(spec)?;
        Ok(())
    }

    fn validate_preset_spec_wire(
        &self,
        spec: super::DiagramThemeSpecWireV1,
    ) -> Result<(), ThemeCompileError> {
        // Built-in recipes do not supply embedded font assets. Validate their decoded spec
        // without constructing a family cache, compatibility plan, or recipe fingerprint.
        let (spec, _) = super::wire_decode::decode(
            spec,
            &self.resources,
            super::source::ThemeWireSources::None,
        )?;
        self.validate_spec(&spec)
    }

    /// Compiles a saved recipe through the same authoring and complete-spec admission paths.
    pub fn compile_recipe(
        &self,
        recipe: merman_theme_contract::ThemeRecipeV1,
    ) -> Result<super::DiagramTheme, super::ThemeDesignCompileError> {
        match recipe {
            merman_theme_contract::ThemeRecipeV1::Design { design } => {
                let spec =
                    super::materialize_theme_design_with_resource_policy(&design, &self.resources)?;
                let sources = super::source::ThemeWireSources::for_design(&design);
                Ok(self.compile_spec_wire_with_sources(spec, sources)?)
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
        let design = super::presets::build_design(preset);
        let complete_spec =
            super::materialize_theme_design_with_resource_policy(&design, &self.resources)?;
        self.validate_preset_spec_wire(complete_spec)
            .map_err(preset_export_error)?;
        Ok(merman_theme_contract::ThemeRecipeV1::Design { design })
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

fn infer_required_capabilities(
    spec: &DiagramThemeSpec,
    root_plan: &RootThemePlan,
) -> BTreeSet<ThemeCapability> {
    let mut required = BTreeSet::new();
    if !spec.styles().rules().is_empty() {
        required.insert(ThemeCapability::SemanticTokens);
        required.insert(ThemeCapability::SemanticRules);
    }
    if !spec.styles().ordinal_palettes().is_empty() {
        required.insert(ThemeCapability::OrdinalPalette);
    }
    collect_typography_capabilities(spec.typography(), &mut required);
    required.extend(root_plan.required_capabilities());
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

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeCompileError {
    #[error(transparent)]
    CompatibilityRecipe(#[from] merman_model::presentation::ThemeCompatibilityRecipeError),
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

    fn complete_spec_fixture() -> merman_theme_contract::DiagramThemeSpecWireV1 {
        let merman_theme_contract::ThemeRecipeV1::Design { design } = DiagramThemeCompiler::new()
            .export_preset(super::super::ThemePreset::Cyberpunk)
            .unwrap()
        else {
            panic!("presets export editable designs");
        };
        super::super::materialize_theme_design(&design).unwrap()
    }

    #[test]
    fn complete_spec_wire_enforces_encoded_payload_byte_boundary() {
        let complete_spec = complete_spec_fixture();
        let size = serde_json::to_vec(&complete_spec).unwrap().len();
        for limit in [size - 1, size] {
            let compiler = DiagramThemeCompiler::new().with_resource_policy(
                ThemeResourcePolicy::interactive()
                    .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, limit)
                    .unwrap(),
            );
            let result = compiler.compile_spec_wire(complete_spec.clone());
            if limit == size {
                result.expect("the exact encoded complete-spec payload must fit");
            } else {
                let ThemeCompileError::ResourceLimit(error) =
                    result.expect_err("complete-spec input must obey encoded-byte admission")
                else {
                    panic!("encoded-byte admission must report a resource limit");
                };
                assert_eq!(error.limit, "max_theme_encoded_bytes");
                assert_eq!(error.actual, size);
                assert_eq!(error.max, limit);
                assert_eq!(error.phase, ThemeResourceLimitPhase::ThemeInput);
            }
        }
    }

    #[test]
    fn complete_spec_recipe_enforces_payload_budget_without_transport_envelope() {
        let recipe = merman_theme_contract::ThemeRecipeV1::CompleteSpec {
            complete_spec: complete_spec_fixture(),
        };
        let merman_theme_contract::ThemeRecipeV1::CompleteSpec { complete_spec } = &recipe else {
            unreachable!()
        };
        let size = serde_json::to_vec(complete_spec).unwrap().len();
        assert!(serde_json::to_vec(&recipe).unwrap().len() > size);
        for limit in [size - 1, size] {
            let compiler = DiagramThemeCompiler::new().with_resource_policy(
                ThemeResourcePolicy::interactive()
                    .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, limit)
                    .unwrap(),
            );
            let result = compiler.compile_recipe(recipe.clone());
            if limit == size {
                result.expect("typed recipe admission counts the payload, not its envelope");
            } else {
                let super::super::ThemeDesignCompileError::Compilation(
                    ThemeCompileError::ResourceLimit(error),
                ) = result.expect_err("saved complete recipes must obey encoded-byte admission")
                else {
                    panic!("complete recipes must preserve the compiler resource diagnostic");
                };
                assert_eq!(error.limit, "max_theme_encoded_bytes");
                assert_eq!(error.actual, size);
                assert_eq!(error.max, limit);
            }
        }
    }

    #[test]
    fn design_recipe_bounds_authored_input_and_materialized_output() {
        let design: merman_theme_contract::ThemeDesignV1 = serde_json::from_value(serde_json::json!({
            "tokens": {"paint.shared": {"type":"paint","value":{
                "kind":"linear-gradient","angle_degrees":45,
                "stops":[{"offset":0,"color":"#123456"},{"offset":0.5,"color":"#abcdef"},{"offset":1,"color":"#ffffff"}]
            }}},
            "styles": {
                "node":{"fill":{"ref":"paint.shared"}},
                "cluster":{"fill":{"ref":"paint.shared"}},
                "actor":{"fill":{"ref":"paint.shared"}}
            }
        })).unwrap();
        let authored_bytes = serde_json::to_vec(&design).unwrap().len();
        let materialized = super::super::materialize_theme_design(&design).unwrap();
        let materialized_bytes = serde_json::to_vec(&materialized).unwrap().len();
        assert!(
            materialized_bytes > authored_bytes,
            "shared values expand into concrete consumers"
        );
        for limit in [authored_bytes - 1, authored_bytes, materialized_bytes] {
            let compiler = DiagramThemeCompiler::new().with_resource_policy(
                ThemeResourcePolicy::interactive()
                    .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, limit)
                    .unwrap(),
            );
            let result = compiler.compile_recipe(merman_theme_contract::ThemeRecipeV1::Design {
                design: design.clone(),
            });
            if limit == materialized_bytes {
                result.expect("both payloads fit the caller-owned budget");
            } else {
                let super::super::ThemeDesignCompileError::Materialization(error) =
                    result.unwrap_err()
                else {
                    panic!("admission must precede compiler allocation");
                };
                assert_eq!(
                    error.diagnostic().limit_id(),
                    Some("max_theme_encoded_bytes")
                );
                assert_eq!(error.diagnostic().actual(), Some((limit + 1) as u64));
                assert_eq!(error.diagnostic().max(), Some(limit as u64));
            }
        }
    }

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
    fn diagnostic_sources_follow_design_defaults_scoped_rules_and_palettes() {
        let design = serde_json::from_value(serde_json::json!({
            "tokens": {
                "paint.ink":{"type":"paint","value":"green"},
                "palette.series":{"type":"palette","value":["orange"]}
            },
            "styles": {"text":{"fill":{"ref":"paint.ink"}},"node":{"radius":4}},
            "rules": [{"target":"node","family":"flowchart","style":{"radius":8}}],
            "palettes": {"node":{"ref":"palette.series"},"pie-slice":["red"]},
            "canvas":{"base":{"ref":"paint.ink"}}
        }))
        .unwrap();
        let compiler = DiagramThemeCompiler::new();
        let spec = super::super::materialize_theme_design(&design).unwrap();
        let theme = compiler
            .compile_recipe(merman_theme_contract::ThemeRecipeV1::Design { design })
            .unwrap();
        let sources = theme.source_map();
        assert_eq!(sources.document().unwrap().id(), "design");
        assert_eq!(sources.rule(0).unwrap().paths, ["/styles/node"]);
        assert_eq!(sources.rule(1).unwrap().paths, ["/styles/text"]);
        assert_eq!(sources.rule(2).unwrap().paths, ["/rules/0/style"]);
        assert_eq!(
            sources.palette(ThemeTarget::Node).unwrap().paths,
            ["/palettes/node"]
        );
        assert_eq!(
            sources.palette(ThemeTarget::PieSlice).unwrap().paths,
            ["/palettes/pie-slice"]
        );
        assert_eq!(sources.canvas_base().unwrap().paths, ["/canvas/base"]);
        assert!(!sources.rule(0).unwrap().generated);
        assert!(!sources.palette(ThemeTarget::Node).unwrap().generated);
        assert!(!sources.canvas_base().unwrap().generated);
        let imported = compiler.compile_spec_wire(spec).unwrap();
        assert_eq!(theme.recipe_fingerprint(), imported.recipe_fingerprint());
        assert_eq!(imported.source_map().rule(2).unwrap().paths, ["/styles/2"]);
        assert_eq!(
            imported
                .source_map()
                .palette(ThemeTarget::Node)
                .unwrap()
                .paths,
            ["/styles/3"]
        );
    }

    #[test]
    fn empty_design_does_not_generate_legacy_defaults_or_token_locations() {
        let compiler = DiagramThemeCompiler::new();
        let empty = compiler
            .compile_recipe(merman_theme_contract::ThemeRecipeV1::Design {
                design: merman_theme_contract::ThemeDesignV1::default(),
            })
            .unwrap();
        assert_eq!(empty.source_map().document().unwrap().id(), "design");
        assert!(empty.source_map().rule(0).is_none());
        assert!(empty.source_map().palette(ThemeTarget::Node).is_none());
        assert!(empty.source_map().canvas_base().is_none());
        let direct = compiler
            .compile_spec_wire(
                serde_json::from_value(serde_json::json!({"canvas":{"base":"blue"}})).unwrap(),
            )
            .unwrap();
        assert_eq!(
            direct.source_map().canvas_base().unwrap().paths,
            ["/canvas/base"]
        );
        assert!(!direct.source_map().canvas_base().unwrap().generated);
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
        assert_eq!(imported.source_map().document().unwrap().id(), "design");
        assert!(imported.source_map().rule(0).unwrap().paths[0].starts_with("/styles/"));
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

    #[cfg(any(
        feature = "diagram-block",
        feature = "diagram-flowchart",
        feature = "diagram-sequence",
        feature = "diagram-state"
    ))]
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
        let engine = theme.install_parse_compatibility(merman_core::MermaidEngine::new());
        for source in [
            #[cfg(feature = "diagram-block")]
            "block\nA --> B",
            #[cfg(feature = "diagram-flowchart")]
            "flowchart TD\nA --> B",
            #[cfg(feature = "diagram-sequence")]
            "sequenceDiagram\nA->>B: Message",
            #[cfg(feature = "diagram-state")]
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
    fn compilation_keeps_public_style_validation_and_cross_graph_references() {
        use crate::diagram_theme::{Specified, ThemeRule, ThemeRuleSet, ThemeStylePatch};
        let compiler = DiagramThemeCompiler::new();
        let effects = DiagramEffectSet::default()
            .with_binding(EffectBinding::new(ThemeTarget::Node, "missing").unwrap())
            .unwrap();
        assert!(matches!(
            compiler.compile(DiagramThemeSpec::new().with_effects(effects)),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::UnknownId {
                    field: "effects.binding.effect_id"
                }
            ))
        ));
        let rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_effect("missing").unwrap(),
        );
        assert!(matches!(
            compiler.compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule))
            ),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::UnknownId {
                    field: "styles.rule.effect"
                }
            ))
        ));
        let mut style = ThemeStylePatch::default();
        style.paint.opacity = Specified::Value(f32::NAN);
        let rule = ThemeRule::new(ThemeTarget::Node, style);
        assert!(matches!(
            compiler.compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule))
            ),
            Err(ThemeCompileError::Validation(
                ThemeCompileValidationError::InvalidNumber {
                    field: "style.opacity"
                }
            ))
        ));
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
    fn explicit_text_layout_requirements_survive_system_catalog_compilation() {
        use crate::diagram_theme::{TextLayoutCapability, ThemeRequirements};

        let compiler = DiagramThemeCompiler::new();
        let default = compiler.compile(DiagramThemeSpec::new()).unwrap();
        assert_eq!(default.report().required_text_capabilities().len(), 0);

        let explicit = compiler
            .compile(
                DiagramThemeSpec::new()
                    .with_requirements(ThemeRequirements::new().with_required_text_capabilities(
                        TextLayoutCapability::ALL.iter().copied(),
                    )),
            )
            .unwrap();
        assert_eq!(
            explicit
                .report()
                .required_text_capabilities()
                .collect::<Vec<_>>(),
            TextLayoutCapability::ALL
        );
        assert_ne!(default.recipe_fingerprint(), explicit.recipe_fingerprint());
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
