use merman_theme_contract::{
    ThemePresetFamilyDesignV1, ThemePresetMetadataV1, ThemePresetQualifiedCellV1,
};

use super::{DiagramThemeCompiler, FontCatalogError, ThemeCompileError, ThemeDesignCompileError};

mod brutalist;
mod catalog;
mod cyberpunk;
mod design;
mod ghibli;
mod spotless;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
/// Alpha selection handle for a built-in diagram theme preset.
///
/// Use [`theme_preset_descriptors`] for discovery so callers also observe each preset's maturity.
pub enum ThemePreset {
    #[default]
    EditorLight,
    EditorDark,
    OneDark,
    GruvboxLight,
    GruvboxDark,
    AyuLight,
    AyuDark,
    Brutalist,
    Spotless,
    Cyberpunk,
    Ghibli,
}

impl ThemePreset {
    pub const fn id(self) -> &'static str {
        catalog::entry_at(self as usize).id()
    }

    pub const fn is_dark(self) -> bool {
        catalog::entry_at(self as usize).is_dark()
    }

    pub fn from_id(id: &str) -> Result<Self, ThemePresetParseError> {
        catalog::entry_for_id(id)
            .map(catalog::PresetCatalogEntry::preset)
            .ok_or_else(|| ThemePresetParseError { id: id.to_string() })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown diagram theme preset `{id}`")]
pub struct ThemePresetParseError {
    id: String,
}

impl ThemePresetParseError {
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// One scoped catalog qualification declaration for a preset recipe revision.
///
/// This is metadata, not a proof receipt. A catalog owner must obtain fresh execution evidence
/// before publishing a cell. The profile identifies the tested scenario and resource conditions;
/// the admission status must not be inferred from the cell's presence.
pub struct ThemePresetQualifiedCell {
    family_id: &'static str,
    output_id: &'static str,
    profile_id: &'static str,
    admission_status: &'static str,
}

impl ThemePresetQualifiedCell {
    /// Describes a declared scope; construction does not grant qualification.
    ///
    /// IDs are open strings. Consumers must understand the profile and admission status before
    /// relying on the claim; an unknown value is never equivalent to portable support.
    pub const fn new(
        family_id: &'static str,
        output_id: &'static str,
        profile_id: &'static str,
        admission_status: &'static str,
    ) -> Self {
        Self {
            family_id,
            output_id,
            profile_id,
            admission_status,
        }
    }

    /// Returns the stable diagram-family identifier.
    pub const fn family_id(self) -> &'static str {
        self.family_id
    }

    /// Returns the stable render-output identifier.
    pub const fn output_id(self) -> &'static str {
        self.output_id
    }

    /// Identifies the qualification scenario and its required resource conditions.
    pub const fn profile_id(self) -> &'static str {
        self.profile_id
    }

    /// Returns the observed target admission class, such as `portable` or `host_dependent`.
    pub const fn admission_status(self) -> &'static str {
        self.admission_status
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePresetDescriptor {
    catalog_index: u8,
}

impl ThemePresetDescriptor {
    pub(super) const fn from_catalog_index(catalog_index: u8) -> Self {
        Self { catalog_index }
    }

    const fn entry(self) -> &'static catalog::PresetCatalogEntry {
        catalog::entry_at(self.catalog_index as usize)
    }

    /// Describes this recipe under the effective compiler policy without granting qualification.
    pub fn describe(self, compiler: &DiagramThemeCompiler) -> ThemePresetMetadataV1 {
        let availability_reason_ids = compiler
            .validate_preset(self.preset())
            .err()
            .map(|error| vec![theme_preset_unavailability_reason(&error).to_owned()])
            .unwrap_or_default();
        ThemePresetMetadataV1 {
            id: self.id().to_owned(),
            display_name: self.display_name().to_owned(),
            appearance: if self.is_dark() { "dark" } else { "light" }.to_owned(),
            maturity: self.maturity().to_owned(),
            available: availability_reason_ids.is_empty(),
            availability_reason_ids,
            family_designs: self
                .entry()
                .family_designs()
                .iter()
                .map(|(family_id, treatment)| ThemePresetFamilyDesignV1 {
                    family_id: (*family_id).to_owned(),
                    treatment: (*treatment).to_owned(),
                })
                .collect(),
            preview_colors: self.entry().preview_colors(),
            application_notes: self.entry().application_notes(),
            qualified_cells: self
                .qualified_cells()
                .iter()
                .map(ThemePresetQualifiedCellV1::from)
                .collect(),
            license_expression: self.license_expression().to_owned(),
            required_attribution: self.required_attribution().map(str::to_owned),
            export_kind: self.export_kind().to_owned(),
        }
    }

    pub const fn id(self) -> &'static str {
        self.entry().id()
    }

    pub const fn display_name(self) -> &'static str {
        self.entry().display_name()
    }

    pub const fn is_dark(self) -> bool {
        self.entry().is_dark()
    }

    /// Returns the open-string maturity reported by discovery surfaces.
    ///
    /// Built-in presets are usable alpha inventory, not stable compatibility promises. A future
    /// maturity promotion is backed by a different qualified descriptor revision rather than by
    /// the preset's presence in this catalog alone.
    pub const fn maturity(self) -> &'static str {
        self.entry().maturity()
    }

    pub const fn preset(self) -> ThemePreset {
        self.entry().preset()
    }

    pub const fn qualified_cells(self) -> &'static [ThemePresetQualifiedCell] {
        self.entry().qualified_cells()
    }

    pub const fn export_kind(self) -> &'static str {
        self.entry().export_kind()
    }

    pub const fn license_expression(self) -> &'static str {
        self.entry().license_expression()
    }

    pub const fn required_attribution(self) -> Option<&'static str> {
        self.entry().required_attribution()
    }
}

pub const fn theme_preset_descriptors() -> &'static [ThemePresetDescriptor] {
    catalog::descriptors()
}

/// Describes built-in presets using this artifact's effective recipe compiler policy.
///
/// Availability uses recipe validation under the compiler's resource policy. Maturity and scoped
/// qualification declarations come directly from the catalog and are not promoted by validation.
pub fn describe_theme_presets(compiler: &DiagramThemeCompiler) -> Vec<ThemePresetMetadataV1> {
    theme_preset_descriptors()
        .iter()
        .map(|descriptor| descriptor.describe(compiler))
        .collect()
}

impl From<&ThemePresetQualifiedCell> for ThemePresetQualifiedCellV1 {
    fn from(cell: &ThemePresetQualifiedCell) -> Self {
        Self {
            family_id: cell.family_id().to_owned(),
            output_id: cell.output_id().to_owned(),
            profile_id: cell.profile_id().to_owned(),
            admission_status: cell.admission_status().to_owned(),
        }
    }
}

fn theme_preset_unavailability_reason(error: &ThemeDesignCompileError) -> &'static str {
    match error {
        ThemeDesignCompileError::Materialization(error)
            if error.diagnostic().code() == "theme-authoring.resource-limit-exceeded" =>
        {
            "theme-preset.resource-policy-rejected"
        }
        ThemeDesignCompileError::Compilation(ThemeCompileError::ResourceLimit(_))
        | ThemeDesignCompileError::Compilation(ThemeCompileError::FontCatalog(
            FontCatalogError::ResourceLimit(_),
        )) => "theme-preset.resource-policy-rejected",
        _ => "theme-preset.recipe-unavailable",
    }
}

pub(super) use catalog::build_design;
pub(crate) use catalog::materialize_spec_wire;

#[cfg(test)]
fn preset_palette(preset: ThemePreset) -> catalog::PresetPalette {
    catalog::entry_for_preset(preset).palette()
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashSet};

    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramTheme, DiagramThemeCompiler, FamilyThemeDisposition,
        ResolvedDiagramTheme, ThemeColorValue, ThemeResourceLimitId, ThemeResourcePolicy,
        ThemeTarget, ThemeVariant,
    };

    #[test]
    fn preset_discovery_preserves_catalog_claims_when_resource_policy_rejects_recipes() {
        let default = describe_theme_presets(&DiagramThemeCompiler::new());
        let policy = ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
            .unwrap();
        let rejected =
            describe_theme_presets(&DiagramThemeCompiler::new().with_resource_policy(policy));

        assert_eq!(default.len(), theme_preset_descriptors().len());
        assert_eq!(rejected.len(), default.len());
        for ((available, rejected), descriptor) in
            default.iter().zip(rejected).zip(theme_preset_descriptors())
        {
            assert_eq!(available.id, descriptor.id());
            assert!(available.available);
            assert!(available.availability_reason_ids.is_empty());
            assert_eq!(available.maturity, "alpha");
            assert!(available.qualified_cells.is_empty());
            assert!(!rejected.available);
            assert_eq!(
                rejected.availability_reason_ids,
                ["theme-preset.resource-policy-rejected"]
            );
            let mut expected = available.clone();
            expected.available = false;
            expected.availability_reason_ids =
                vec!["theme-preset.resource-policy-rejected".to_owned()];
            assert_eq!(rejected, expected);
        }
    }

    fn solid_color(paint: &CanvasPaint) -> Option<String> {
        match paint {
            CanvasPaint::Solid(color) => Some(color.as_css()),
            _ => None,
        }
    }

    fn compiled(preset: ThemePreset) -> (crate::diagram_theme::DiagramTheme, ResolvedDiagramTheme) {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(preset)
            .expect("built-in theme preset should compile");
        let resolved = theme.resolve(DiagramFamilyId::FLOWCHART);
        (theme, resolved)
    }

    fn all_presets() -> impl ExactSizeIterator<Item = ThemePreset> {
        theme_preset_descriptors()
            .iter()
            .map(|descriptor| descriptor.preset())
    }

    fn assert_style(
        theme: &DiagramTheme,
        family: DiagramFamilyId,
        target: ThemeTarget,
        expected_fill: Option<&str>,
        expected_stroke: Option<&str>,
    ) {
        let resolved = theme.resolve(family);
        let style = resolved.style(target, ThemeVariant::Default, None);
        assert_eq!(
            style.fill().and_then(solid_color).as_deref(),
            expected_fill,
            "{family} {target:?} fill"
        );
        assert_eq!(
            style.stroke().and_then(solid_color).as_deref(),
            expected_stroke,
            "{family} {target:?} stroke"
        );
    }

    fn resolved_solid_fill(theme: &ResolvedDiagramTheme, target: ThemeTarget) -> String {
        theme
            .style(target, ThemeVariant::Default, None)
            .fill()
            .and_then(solid_color)
            .unwrap_or_else(|| panic!("{target:?} must resolve to a solid fill"))
    }

    fn resolved_solid_stroke(theme: &ResolvedDiagramTheme, target: ThemeTarget) -> String {
        theme
            .style(target, ThemeVariant::Default, None)
            .stroke()
            .and_then(solid_color)
            .unwrap_or_else(|| panic!("{target:?} must resolve to a solid stroke"))
    }

    fn contrast_ratio(foreground: &str, background: &str) -> f64 {
        fn relative_luminance(color: &str) -> f64 {
            fn linear_channel(channel: f64) -> f64 {
                let channel = channel / 255.0;
                if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            }

            let rgba = merman_markup::theme_color::ThemeColor::parse(color)
                .unwrap_or_else(|_| panic!("expected a Mermaid-compatible color, got {color}"))
                .rgba_channels();
            0.2126 * linear_channel(rgba.red)
                + 0.7152 * linear_channel(rgba.green)
                + 0.0722 * linear_channel(rgba.blue)
        }

        let foreground = relative_luminance(foreground);
        let background = relative_luminance(background);
        (foreground.max(background) + 0.05) / (foreground.min(background) + 0.05)
    }

    fn is_native_candidate(preset: ThemePreset) -> bool {
        matches!(
            preset,
            ThemePreset::Brutalist
                | ThemePreset::Spotless
                | ThemePreset::Cyberpunk
                | ThemePreset::Ghibli
        )
    }

    #[test]
    fn built_in_presets_scope_mermaid_compatibility_without_layout_or_look() {
        for preset in all_presets() {
            let (theme, _) = compiled(preset);
            let config = theme.spec().mermaid().to_mermaid_config();

            assert_eq!(
                config.get_str("theme"),
                (!is_native_candidate(preset)).then_some(if preset.is_dark() {
                    "dark"
                } else {
                    "base"
                })
            );
            assert_eq!(
                config.get_bool("darkMode"),
                None,
                "{} must not claim derived dark-mode paints",
                preset.id()
            );
            assert_eq!(config.get_str("look"), None);
            assert_eq!(config.get_str("flowchart.defaultRenderer"), None);
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn retained_presets_keep_dark_fallbacks_without_owning_typed_paints() {
        for preset in all_presets().filter(|preset| !is_native_candidate(*preset)) {
            let (theme, _) = compiled(preset);
            let selected = if preset.is_dark() { "dark" } else { "base" };
            let parse = |engine: merman_core::MermaidEngine| {
                engine
                    .parse_diagram_for_render_model_sync(
                        "flowchart LR\nA[Alpha]\n",
                        merman_core::ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap()
                    .into_presentation()
            };
            let themed =
                parse(theme.install_parse_compatibility(merman_core::MermaidEngine::new()));
            let fallback = parse(merman_core::MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"theme": selected}),
                ),
            ));
            let config = &themed.metadata().effective_config;
            assert_eq!(config.get_str("theme"), Some(selected));
            // Unadapted terminals still consume the selected Mermaid palette.
            for key in ["themeVariables.cScale0", "themeVariables.mainBkg"] {
                assert_eq!(
                    config.get_str(key),
                    fallback.metadata().effective_config.get_str(key),
                    "{} {key}",
                    preset.id()
                );
            }
            for path in ["themeVariables.nodeBorder", "themeVariables.git0"] {
                assert!(
                    !config.config_path_overrides_typed_default(path),
                    "{} must leave {path} available to typed paint",
                    preset.id()
                );
            }
        }
    }

    #[test]
    fn preset_catalog_is_the_complete_unique_enum_projection() {
        const EXPECTED_PRESETS: [ThemePreset; 11] = [
            ThemePreset::EditorLight,
            ThemePreset::EditorDark,
            ThemePreset::OneDark,
            ThemePreset::GruvboxLight,
            ThemePreset::GruvboxDark,
            ThemePreset::AyuLight,
            ThemePreset::AyuDark,
            ThemePreset::Brutalist,
            ThemePreset::Spotless,
            ThemePreset::Cyberpunk,
            ThemePreset::Ghibli,
        ];

        let descriptors = theme_preset_descriptors();
        assert_eq!(descriptors.len(), EXPECTED_PRESETS.len());

        let mut ids = BTreeSet::new();
        let mut display_names = BTreeSet::new();
        let mut presets = HashSet::new();
        for (descriptor, expected_preset) in descriptors.iter().zip(EXPECTED_PRESETS) {
            assert_eq!(descriptor.preset(), expected_preset);
            assert_eq!(ThemePreset::from_id(descriptor.id()), Ok(expected_preset));
            assert_eq!(expected_preset.id(), descriptor.id());
            assert_eq!(expected_preset.is_dark(), descriptor.is_dark());
            assert!(ids.insert(descriptor.id()), "duplicate preset ID");
            assert!(
                display_names.insert(descriptor.display_name()),
                "duplicate preset display name"
            );
            assert!(
                presets.insert(descriptor.preset()),
                "duplicate enum projection"
            );
        }
    }

    #[test]
    fn alpha_preset_metadata_starts_unqualified_and_fail_closed() {
        for descriptor in theme_preset_descriptors() {
            let entry = catalog::entry_for_preset(descriptor.preset());
            assert_eq!(descriptor.maturity(), "alpha");
            assert!(descriptor.qualified_cells().is_empty());
            assert_eq!(descriptor.export_kind(), "design");
            assert_eq!(descriptor.license_expression(), "MIT OR Apache-2.0");
            assert_eq!(descriptor.required_attribution(), None);
            assert!(entry.required_feature_ids().is_empty());
            assert!(entry.bundled_resource_ids().is_empty());
            assert!(entry.allowed_residual_ids().is_empty());
            assert!(entry.retained());
        }
    }

    #[test]
    fn discovery_guidance_keeps_design_intent_and_explicit_config_separate() {
        let compiler = DiagramThemeCompiler::new();
        let catalog = describe_theme_presets(&compiler);
        let brutalist = catalog
            .iter()
            .find(|preset| preset.id == "brutalist")
            .unwrap();
        assert!(
            brutalist
                .preview_colors
                .iter()
                .any(|color| color == "#FFE66D")
        );
        let note = &brutalist.application_notes[0];
        assert_eq!(note.family_id, "flowchart");
        assert_eq!(
            note.recommended_config.as_ref().unwrap()["flowchart"]["look"],
            "classic"
        );
        let theme = compiler.compile_preset(ThemePreset::Brutalist).unwrap();
        assert_eq!(
            theme
                .spec()
                .mermaid()
                .to_mermaid_config()
                .get_str("flowchart.look"),
            None
        );
        let ghibli = catalog.iter().find(|preset| preset.id == "ghibli").unwrap();
        assert_eq!(
            ghibli.family_designs,
            [ThemePresetFamilyDesignV1 {
                family_id: "pie".into(),
                treatment: "dedicated".into(),
            }]
        );
        assert!(ghibli.qualified_cells.is_empty());
    }

    #[test]
    fn ghibli_exports_editable_paper_and_scoped_pie_text_roles() {
        let compiler = DiagramThemeCompiler::new();
        let theme = compiler.compile_preset(ThemePreset::Ghibli).unwrap();
        assert_eq!(theme.spec().canvas().layers().len(), 4);
        let resolved = theme.resolve(DiagramFamilyId::PIE);
        for (target, size, color) in [
            (ThemeTarget::Title, 25.0, "#3A2E2C"),
            (ThemeTarget::Text, 17.0, "#3A2E2C"),
            (ThemeTarget::Legend, 12.0, "#6B5B4F"),
        ] {
            let style = resolved.text_style(target, ThemeVariant::Default, None);
            assert_eq!(style.typography().font_size_px(), size);
            assert_eq!(style.fill().and_then(solid_color).as_deref(), Some(color));
        }
        assert!(
            theme
                .spec()
                .mermaid()
                .to_mermaid_config()
                .as_value()
                .as_object()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn brutalist_and_spotless_materialize_distinct_visual_mechanisms() {
        use merman_theme_contract::{ThemeEffectEntryWireV1, ThemeEffectPrimitiveWireV1};

        let compiler = DiagramThemeCompiler::new();
        for (preset, expected_grid_layers) in
            [(ThemePreset::Brutalist, 0), (ThemePreset::Spotless, 2)]
        {
            let theme = compiler.compile_preset(preset).unwrap();
            assert_eq!(theme.spec().canvas().layers().len(), expected_grid_layers);
            let recipe =
                catalog::materialize_spec_wire(preset, &ThemeResourcePolicy::interactive())
                    .unwrap();
            if preset == ThemePreset::Brutalist {
                let effects = recipe.effects.as_ref().unwrap();
                assert!(
                    matches!(effects.as_slice(), [ThemeEffectEntryWireV1::Graph {
                        primitives, ..
                    }] if matches!(primitives.as_slice(), [ThemeEffectPrimitiveWireV1::DropShadow {
                        offset_x: 6.0, offset_y: 6.0, blur_radius: 0.0, ..
                    }]))
                );
            } else {
                assert!(recipe.effects.as_ref().is_none_or(Vec::is_empty));
            }
            for rule in recipe.styles.as_ref().unwrap() {
                if let merman_theme_contract::ThemeRuleSetWireV1::Rule { target, style, .. } = rule
                    && (target.ends_with("label") || target == "title")
                {
                    assert!(matches!(
                        style.effect,
                        merman_theme_contract::SpecifiedWireV1::Unspecified
                    ));
                }
            }

            let designs = theme_preset_descriptors()
                .iter()
                .find(|descriptor| descriptor.preset() == preset)
                .unwrap()
                .describe(&compiler)
                .family_designs
                .into_iter()
                .map(|design| (design.family_id, design.treatment))
                .collect::<Vec<_>>();
            assert_eq!(
                designs,
                [
                    ("class".to_owned(), "base_only".to_owned()),
                    ("flowchart".to_owned(), "dedicated".to_owned()),
                    ("sequence".to_owned(), "dedicated".to_owned()),
                    ("xychart".to_owned(), "dedicated".to_owned()),
                ]
            );
        }
    }

    #[test]
    fn spotless_typography_rules_use_supported_family_mechanisms() {
        use crate::diagram_theme::{FamilyThemeMechanism, FamilyThemeRuleFacet};

        let theme = DiagramThemeCompiler::new()
            .compile_preset(ThemePreset::Spotless)
            .unwrap();
        for (family, target, expected_weight) in [
            (DiagramFamilyId::FLOWCHART, ThemeTarget::NodeLabel, 700),
            (DiagramFamilyId::FLOWCHART, ThemeTarget::EdgeLabel, 600),
            (DiagramFamilyId::SEQUENCE, ThemeTarget::ActorLabel, 700),
            (DiagramFamilyId::SEQUENCE, ThemeTarget::NoteLabel, 600),
            (DiagramFamilyId::XY_CHART, ThemeTarget::Title, 700),
        ] {
            let resolved = theme.resolve(family);
            let routes = resolved
                .family_mechanism_routes()
                .iter()
                .filter(|route| {
                    matches!(
                        route.mechanism(),
                        FamilyThemeMechanism::RuleFacet {
                            target: route_target,
                            facet: FamilyThemeRuleFacet::Typography(_),
                            ..
                        } if route_target == target
                    )
                })
                .collect::<Vec<_>>();
            assert!(
                !routes.is_empty(),
                "{family} {target:?} typography is retained"
            );
            assert!(
                routes
                    .iter()
                    .all(|route| { route.disposition() == FamilyThemeDisposition::TypedAdapter }),
                "{family} {target:?} must not request unsupported typography: {routes:?}"
            );
            assert_eq!(
                resolved
                    .style(target, ThemeVariant::Default, None)
                    .typography()
                    .font_weight(),
                expected_weight,
                "{family} {target:?} retains its visual hierarchy"
            );
        }
    }

    #[test]
    fn all_catalog_recipes_round_trip_as_editable_designs() {
        for descriptor in theme_preset_descriptors() {
            let compiler = DiagramThemeCompiler::new();
            let recipe = compiler
                .export_preset(descriptor.preset())
                .expect("catalog preset exports");
            let merman_theme_contract::ThemeRecipeV1::Design { design } = &recipe else {
                panic!("presets must retain explicit authored bindings")
            };
            assert!(!design.tokens.is_empty());
            assert!(
                !crate::diagram_theme::inspect_theme_design(design)
                    .unwrap()
                    .references
                    .is_empty()
            );
            let encoded = serde_json::to_vec(&recipe).expect("design serializes");
            let decoded = serde_json::from_slice(&encoded).expect("design deserializes");
            let round_tripped = compiler
                .compile_recipe(decoded)
                .expect("imported design compiles");
            let palette = preset_palette(descriptor.preset());
            for (family, target, expected) in [
                (
                    DiagramFamilyId::SEQUENCE,
                    ThemeTarget::ActorLabel,
                    palette.actor_text,
                ),
                (
                    DiagramFamilyId::SEQUENCE,
                    ThemeTarget::NoteLabel,
                    palette.note_text,
                ),
                (
                    DiagramFamilyId::STATE,
                    ThemeTarget::NoteLabel,
                    palette.note_text,
                ),
                (
                    DiagramFamilyId::SEQUENCE,
                    ThemeTarget::SequenceNumberLabel,
                    palette.sequence_number_text,
                ),
                (
                    DiagramFamilyId::PACKET,
                    ThemeTarget::PacketFieldLabel,
                    palette.packet_field_label_text,
                ),
                (
                    DiagramFamilyId::XY_CHART,
                    ThemeTarget::Legend,
                    palette.subtle_text,
                ),
            ] {
                assert_eq!(
                    round_tripped
                        .resolve(family)
                        .text_style(target, ThemeVariant::Default, None)
                        .fill()
                        .and_then(solid_color)
                        .as_deref(),
                    Some(expected),
                    "{} {family} {target:?} must override generic Text after JSON round-trip",
                    descriptor.id(),
                );
            }
            assert_eq!(
                round_tripped.recipe_fingerprint(),
                compiler
                    .compile_preset(descriptor.preset())
                    .unwrap()
                    .recipe_fingerprint()
            );
        }
    }

    #[test]
    fn preset_presentation_metadata_does_not_change_values_or_declared_bindings() {
        use merman_theme_contract::ThemeRecipeV1;

        let compiler = DiagramThemeCompiler::new();
        for preset in [ThemePreset::EditorLight, ThemePreset::Cyberpunk] {
            let ThemeRecipeV1::Design { mut design } = compiler.export_preset(preset).unwrap()
            else {
                panic!("editable preset design")
            };
            let groups = design
                .tokens
                .values()
                .filter_map(|token| token.group.as_deref())
                .collect::<BTreeSet<_>>();
            for group in [
                "Core colors",
                "Diagram details",
                "Series palette",
                "Typography",
            ] {
                assert!(groups.contains(group), "{} is missing {group}", preset.id());
            }
            if preset == ThemePreset::Cyberpunk {
                assert!(groups.contains("Effects"));
                assert!(groups.contains("Geometry"));
            }
            assert!(design.tokens.values().all(|token| {
                token
                    .description
                    .as_deref()
                    .is_some_and(|text| !text.is_empty())
            }));

            let authored = serde_json::to_value(&design).unwrap();
            let bindings = crate::diagram_theme::inspect_theme_design(&design).unwrap();
            for (id, token) in &mut design.tokens {
                token.label = Some(format!("Renamed {id}"));
                token.group = Some("An author's custom group".to_owned());
                token.description = None;
            }
            assert_ne!(serde_json::to_value(&design).unwrap(), authored);
            assert_eq!(
                crate::diagram_theme::inspect_theme_design(&design).unwrap(),
                bindings
            );
            assert_eq!(
                compiler
                    .compile_recipe(ThemeRecipeV1::Design { design })
                    .unwrap()
                    .recipe_fingerprint(),
                compiler
                    .compile_preset(preset)
                    .unwrap()
                    .recipe_fingerprint()
            );
        }
    }

    #[test]
    fn preset_shared_color_edit_and_local_detachment_survive_exchange() {
        use merman_theme_contract::*;

        let compiler = DiagramThemeCompiler::new();
        let ThemeRecipeV1::Design { mut design } =
            compiler.export_preset(ThemePreset::Cyberpunk).unwrap()
        else {
            panic!("editable preset design")
        };
        design.tokens.get_mut("neon-cyan").unwrap().value =
            ThemeDesignTokenValueV1::Color(ThemeValueV1::Literal("#123456".to_owned()));
        // Detach exactly the explicit Flowchart edge override, retaining every other binding.
        let edge = design
            .rules
            .iter_mut()
            .rev()
            .find(|rule| rule.family.as_deref() == Some("flowchart") && rule.target == "edge")
            .unwrap();
        let Some(ThemeValueV1::Literal(stroke)) = edge.style.stroke.as_mut() else {
            panic!("authored stroke")
        };
        stroke.paint = SpecifiedWireV1::Value(ThemeValueV1::Literal(ThemeDesignPaintV1::Color(
            "#fedcba".to_owned(),
        )));
        let recipe = ThemeRecipeV1::Design { design };
        let encoded = serde_json::to_vec(&recipe).unwrap();
        let imported = compiler
            .compile_recipe(serde_json::from_slice(&encoded).unwrap())
            .unwrap();
        let flowchart = imported.resolve(DiagramFamilyId::FLOWCHART);
        assert_eq!(
            resolved_solid_stroke(&flowchart, ThemeTarget::Node),
            "#123456"
        );
        assert_eq!(
            resolved_solid_stroke(&flowchart, ThemeTarget::Edge),
            "#fedcba"
        );
        let sequence = imported.resolve(DiagramFamilyId::SEQUENCE);
        assert_eq!(
            resolved_solid_stroke(&sequence, ThemeTarget::Actor),
            "#123456"
        );
        assert_eq!(
            resolved_solid_fill(&sequence, ThemeTarget::ActorLabel),
            "#123456"
        );

        let spec = catalog::materialize_spec_wire(
            ThemePreset::Cyberpunk,
            &ThemeResourcePolicy::interactive(),
        )
        .unwrap();
        let ThemeRecipeV1::Design { design } = recipe else {
            unreachable!()
        };
        let changed = crate::diagram_theme::materialize_theme_design(&design).unwrap();
        assert_ne!(
            spec.canvas, changed.canvas,
            "gradient stops share the declared neon role"
        );
        assert_ne!(
            spec.effects, changed.effects,
            "glows share the declared neon role"
        );
    }

    #[test]
    fn preset_shadow_values_remain_editable_without_leaking_into_labels() {
        use merman_theme_contract::*;

        let compiler = DiagramThemeCompiler::new();
        let ThemeRecipeV1::Design { mut design } =
            compiler.export_preset(ThemePreset::Brutalist).unwrap()
        else {
            panic!("editable preset design")
        };
        design.tokens.get_mut("shadow-offset").unwrap().value = ThemeDesignTokenValueV1::Length(
            ThemeValueV1::Literal(ThemeLengthWireV1::PxValue(11.0)),
        );
        let encoded = serde_json::to_vec(&ThemeRecipeV1::Design { design }).unwrap();
        let theme = compiler
            .compile_recipe(serde_json::from_slice(&encoded).unwrap())
            .unwrap();
        let flowchart = theme.resolve(DiagramFamilyId::FLOWCHART);
        let node = flowchart.style(ThemeTarget::Node, ThemeVariant::Default, None);
        let effect_id = node.effect_resolution().value().expect("body shadow");
        let graph = theme
            .spec()
            .effects()
            .graphs()
            .iter()
            .find(|graph| graph.id() == effect_id)
            .unwrap();
        assert!(matches!(
            graph.primitives(),
            [crate::diagram_theme::EffectPrimitive::DropShadow {
                offset_x: 11.0,
                offset_y: 11.0,
                blur_radius: 0.0,
                ..
            }]
        ));
        let label = flowchart.style(ThemeTarget::NodeLabel, ThemeVariant::Default, None);
        assert!(label.effect_resolution().value().is_none());
    }

    #[test]
    fn preset_materialization_obeys_the_callers_resource_policy() {
        let policy = crate::diagram_theme::ThemeResourcePolicy::interactive()
            .with_limit(
                crate::diagram_theme::ThemeResourceLimitId::MaxThemeEncodedBytes,
                1,
            )
            .expect("valid restrictive theme input limit");
        let compiler = DiagramThemeCompiler::new().with_resource_policy(policy);

        let compile_error = compiler
            .compile_preset(ThemePreset::EditorLight)
            .expect_err("preset compilation must not bypass definition admission");
        assert!(matches!(
            compile_error,
            crate::diagram_theme::ThemeDesignCompileError::Materialization(ref error)
                if error.diagnostic().code() == "theme-authoring.resource-limit-exceeded"
                    && error.diagnostic().limit_id() == Some("max_theme_encoded_bytes")
        ));

        let export_error = compiler
            .export_preset(ThemePreset::EditorLight)
            .expect_err("preset export must use the same caller-owned policy");
        assert_eq!(
            export_error.diagnostic().code(),
            "theme-authoring.resource-limit-exceeded"
        );
        assert_eq!(
            export_error.diagnostic().limit_id(),
            Some("max_theme_encoded_bytes")
        );
    }

    #[test]
    fn cyberpunk_complete_recipe_retains_canvas_and_scoped_ordered_glow() {
        use merman_theme_contract::{
            SpecifiedWireV1, ThemeEffectEntryWireV1, ThemeEffectPrimitiveWireV1, ThemeRuleSetWireV1,
        };
        let compiler = DiagramThemeCompiler::new();
        let recipe = compiler.export_preset(ThemePreset::Cyberpunk).unwrap();
        let complete_spec = catalog::materialize_spec_wire(
            ThemePreset::Cyberpunk,
            &ThemeResourcePolicy::interactive(),
        )
        .unwrap();
        assert_eq!(
            complete_spec
                .canvas
                .as_ref()
                .unwrap()
                .layers
                .as_ref()
                .unwrap()
                .len(),
            3
        );
        let sized_roles: Vec<_> = complete_spec
            .styles
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|entry| match entry {
                ThemeRuleSetWireV1::Rule {
                    family,
                    target,
                    style,
                    ..
                } if style.typography.as_ref().is_some_and(|typography| {
                    matches!(typography.font_size_px, SpecifiedWireV1::Value(_))
                }) =>
                {
                    Some((family.as_deref(), target.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            sized_roles,
            [
                (Some("xychart"), "legend"),
                (Some("xychart"), "title"),
                (Some("xychart"), "axis-title"),
            ],
            "role sizes must not leak into other families"
        );
        let effects = complete_spec
            .effects
            .as_ref()
            .expect("public recipe owns the glow");
        let effect_for_target = |target: &str| {
            complete_spec
                .styles
                .as_ref()
                .unwrap()
                .iter()
                .find_map(|entry| match entry {
                    ThemeRuleSetWireV1::Rule {
                        family,
                        target: role,
                        style,
                        ..
                    } if family.as_deref() == Some("flowchart") && role == target => {
                        if let SpecifiedWireV1::Value(id) = &style.effect {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }
                    _ => None,
                })
                .expect("effect binding on target")
        };
        let shape_id = effect_for_target("node");
        let expected_edge_id = effect_for_target("edge");
        let ThemeEffectEntryWireV1::Graph { id, primitives, color_space, .. } = effects.iter()
            .find(|entry| matches!(entry, ThemeEffectEntryWireV1::Graph { id, .. } if id == &shape_id))
            .expect("shape glow graph") else { unreachable!() };
        let ThemeEffectEntryWireV1::Graph { id: edge_id, primitives: edge_primitives, color_space: edge_color_space, .. } = effects.iter()
            .find(|entry| matches!(entry, ThemeEffectEntryWireV1::Graph { id, .. } if id == &expected_edge_id))
            .expect("edge glow graph") else { unreachable!() };
        let [
            ThemeEffectPrimitiveWireV1::DropShadow {
                input: first,
                blur_radius: first_blur,
                ..
            },
            ThemeEffectPrimitiveWireV1::DropShadow {
                input: second,
                blur_radius: second_blur,
                ..
            },
        ] = primitives.as_slice()
        else {
            panic!("the reference shape glow has two ordered shadows")
        };
        assert_eq!(color_space.as_deref(), Some("srgb"));
        assert_eq!(first.as_deref(), Some("source-graphic"));
        assert_eq!(second.as_deref(), Some("previous"));
        assert_eq!((*first_blur, *second_blur), (8.0, 16.0));
        assert_ne!(
            edge_id, id,
            "edge glow must not reuse the two-stage shape graph"
        );
        assert_eq!(edge_color_space.as_deref(), Some("srgb"));
        let [
            ThemeEffectPrimitiveWireV1::DropShadow {
                input,
                offset_x,
                offset_y,
                blur_radius,
                spread,
                color,
            },
        ] = edge_primitives.as_slice()
        else {
            panic!("the reference edge glow has one shadow")
        };
        assert_eq!(input.as_deref(), Some("source-graphic"));
        assert_eq!(
            (*offset_x, *offset_y, *blur_radius, *spread),
            (0.0, 0.0, 6.0, 0.0)
        );
        assert_eq!(
            ThemeColorValue::parse(color).unwrap(),
            ThemeColorValue::parse("rgba(0, 242, 255, 0.6)").unwrap()
        );
        let scoped_targets: Vec<_> = complete_spec
            .styles
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|entry| match entry {
                ThemeRuleSetWireV1::Rule {
                    family,
                    target,
                    style,
                    ..
                } if style.effect == SpecifiedWireV1::Value(id.clone()) => {
                    Some((family.as_deref(), target.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            scoped_targets,
            [
                (Some("flowchart"), "node"),
                (Some("sequence"), "actor"),
                (Some("sequence"), "actor-label")
            ]
        );
        let edge_scopes: Vec<_> = complete_spec
            .styles
            .as_ref()
            .unwrap()
            .iter()
            .filter_map(|entry| match entry {
                ThemeRuleSetWireV1::Rule {
                    family,
                    target,
                    style,
                    ..
                } if style.effect == SpecifiedWireV1::Value(edge_id.clone()) => {
                    Some((family.as_deref(), target.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            edge_scopes,
            [
                (Some("flowchart"), "edge"),
                (Some("sequence"), "message"),
                (Some("sequence"), "lifeline")
            ]
        );
        let encoded = serde_json::to_vec(&recipe).unwrap();
        let imported = DiagramThemeCompiler::new()
            .compile_recipe(serde_json::from_slice(&encoded).unwrap())
            .unwrap();
        let selected = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
        assert_eq!(selected.recipe_fingerprint(), imported.recipe_fingerprint());
        for theme in [&selected, &imported] {
            let flowchart = theme.resolve(DiagramFamilyId::FLOWCHART);
            let edge = flowchart.style(ThemeTarget::Edge, ThemeVariant::Default, None);
            assert_eq!(edge.effect_resolution().value(), Some(edge_id));
            assert_eq!(edge.stroke_width(), Some(2.0));
            let graph = theme
                .spec()
                .effects()
                .graphs()
                .iter()
                .find(|graph| graph.id() == edge_id)
                .expect("resolved edge binding must retain its graph");
            assert_eq!(
                graph.color_space(),
                crate::diagram_theme::EffectColorSpace::Srgb
            );
            assert_eq!(
                graph.primitives(),
                [crate::diagram_theme::EffectPrimitive::DropShadow {
                    input: crate::diagram_theme::EffectInput::SourceGraphic,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    blur_radius: 6.0,
                    spread: 0.0,
                    color: crate::diagram_theme::ThemeColorValue::parse("rgba(0, 242, 255, 0.6)")
                        .unwrap(),
                }]
            );
            for family in [
                DiagramFamilyId::CLASS,
                DiagramFamilyId::SEQUENCE,
                DiagramFamilyId::ER,
            ] {
                let resolved = theme.resolve(family);
                assert!(
                    resolved
                        .style(ThemeTarget::Edge, ThemeVariant::Default, None)
                        .effect_resolution()
                        .value()
                        .is_none(),
                    "Flowchart edge glow must not leak into {family}"
                );
            }
        }
    }

    #[test]
    fn complete_preset_exchange_preserves_scoped_clear_and_transparency() {
        use merman_theme_contract::{
            SpecifiedWireV1, ThemeCanvasPaintWireV1, ThemeRecipeV1, ThemeRuleSetWireV1,
            ThemeStylePatchWireV1,
        };
        let compiler = DiagramThemeCompiler::new();
        let original = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
        let mut complete_spec = catalog::materialize_spec_wire(
            ThemePreset::Cyberpunk,
            &ThemeResourcePolicy::interactive(),
        )
        .unwrap();
        complete_spec
            .styles
            .as_mut()
            .unwrap()
            .push(ThemeRuleSetWireV1::Rule {
                target: "node".to_owned(),
                family: Some("flowchart".to_owned()),
                variant: None,
                ordinal: None,
                style: ThemeStylePatchWireV1 {
                    fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(
                        "transparent".to_owned(),
                    )),
                    effect: SpecifiedWireV1::Clear,
                    ..Default::default()
                },
            });
        let encoded = serde_json::to_vec(&ThemeRecipeV1::CompleteSpec { complete_spec }).unwrap();
        let theme = compiler
            .compile_recipe(serde_json::from_slice(&encoded).unwrap())
            .unwrap();
        let flowchart = theme.resolve(DiagramFamilyId::FLOWCHART);
        let node = flowchart.style(ThemeTarget::Node, ThemeVariant::Default, None);
        assert!(matches!(node.fill(), Some(CanvasPaint::Transparent)));
        assert!(matches!(
            node.effect_resolution().specified(),
            crate::diagram_theme::Specified::Clear
        ));
        let sequence = theme.resolve(DiagramFamilyId::SEQUENCE);
        let actor = sequence.style(ThemeTarget::Actor, ThemeVariant::Default, None);
        assert_eq!(
            actor.effect_resolution().value().map(String::as_str),
            original
                .resolve(DiagramFamilyId::SEQUENCE)
                .style(ThemeTarget::Actor, ThemeVariant::Default, None)
                .effect_resolution()
                .value()
                .map(String::as_str)
        );
        assert_eq!(theme.spec().canvas().layers().len(), 3);
        let note_label = sequence.style(ThemeTarget::NoteLabel, ThemeVariant::Default, None);
        assert_eq!(
            note_label.effect_resolution().value().map(String::as_str),
            original
                .resolve(DiagramFamilyId::SEQUENCE)
                .style(ThemeTarget::NoteLabel, ThemeVariant::Default, None)
                .effect_resolution()
                .value()
                .map(String::as_str)
        );
        let loop_label = sequence.style(ThemeTarget::LoopLabel, ThemeVariant::Default, None);
        assert_eq!(
            loop_label.effect_resolution().value().map(String::as_str),
            original
                .resolve(DiagramFamilyId::SEQUENCE)
                .style(ThemeTarget::LoopLabel, ThemeVariant::Default, None)
                .effect_resolution()
                .value()
                .map(String::as_str)
        );
        assert_eq!(theme.spec().effects().graphs().len(), 16);
    }

    #[test]
    fn preset_final_composition_obeys_encoded_byte_budget() {
        let compiler = DiagramThemeCompiler::new();
        let recipe = compiler.export_preset(ThemePreset::Cyberpunk).unwrap();
        let merman_theme_contract::ThemeRecipeV1::Design { design } = recipe else {
            unreachable!()
        };
        let complete_spec = catalog::materialize_spec_wire(
            ThemePreset::Cyberpunk,
            &ThemeResourcePolicy::interactive(),
        )
        .unwrap();
        // Both authored input and concrete expansion count toward the host's encoded-byte limit.
        let size = serde_json::to_vec(&complete_spec)
            .unwrap()
            .len()
            .max(serde_json::to_vec(&design).unwrap().len());
        let limited = compiler.with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, size - 1)
                .unwrap(),
        );
        let error = limited
            .export_preset(ThemePreset::Cyberpunk)
            .expect_err("final recipe bytes must be counted, not only its small token input");
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_encoded_bytes")
        );
        assert!(limited.compile_preset(ThemePreset::Cyberpunk).is_err());
        let exact = DiagramThemeCompiler::new().with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, size)
                .unwrap(),
        );
        assert!(exact.export_preset(ThemePreset::Cyberpunk).is_ok());
        assert!(exact.compile_preset(ThemePreset::Cyberpunk).is_ok());
    }

    #[test]
    fn preset_final_composition_obeys_effect_budget_on_export_compile_and_discovery() {
        let compiler = DiagramThemeCompiler::new().with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxEffectPrimitivesPerGraph, 1)
                .unwrap(),
        );
        let error = compiler
            .export_preset(ThemePreset::Cyberpunk)
            .expect_err("the second composed shadow must count during export");
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_effect_primitives_per_graph")
        );
        assert!(compiler.compile_preset(ThemePreset::Cyberpunk).is_err());
        assert!(compiler.export_preset(ThemePreset::EditorLight).is_ok());
        let descriptions = describe_theme_presets(&compiler);
        let cyberpunk = descriptions
            .iter()
            .find(|descriptor| descriptor.id == ThemePreset::Cyberpunk.id())
            .unwrap();
        assert!(!cyberpunk.available);
        assert_eq!(
            cyberpunk.availability_reason_ids,
            ["theme-preset.resource-policy-rejected"]
        );
        let editor_light = descriptions
            .iter()
            .find(|descriptor| descriptor.id == ThemePreset::EditorLight.id())
            .unwrap();
        assert!(editor_light.available);
        assert!(editor_light.availability_reason_ids.is_empty());
    }

    #[test]
    fn preset_design_exports_preserve_each_recipe_profile() {
        let compiler = DiagramThemeCompiler::new();
        for descriptor in theme_preset_descriptors() {
            let merman_theme_contract::ThemeRecipeV1::Design {
                design: complete_spec,
            } = compiler
                .export_preset(descriptor.preset())
                .expect("preset export must materialize")
            else {
                panic!("preset recipes must retain authoring bindings")
            };
            assert!(
                complete_spec.assets.is_none(),
                "presets must not bundle fonts or choose host font policy"
            );
            if is_native_candidate(descriptor.preset()) {
                assert!(complete_spec.mermaid.is_none());
                continue;
            }
            let mermaid = complete_spec
                .mermaid
                .as_ref()
                .expect("preset export must retain Mermaid compatibility");
            assert_eq!(
                mermaid.theme.as_deref(),
                Some(if descriptor.is_dark() { "dark" } else { "base" })
            );
            assert_eq!(mermaid.dark_mode, None);
        }
    }

    #[test]
    fn built_in_alpha_presets_materialize_expected_semantic_representatives() {
        let expected = [
            (ThemePreset::EditorLight, "#ffffff", "#64748b", "#2563eb"),
            (ThemePreset::EditorDark, "#0f172a", "#94a3b8", "#60a5fa"),
            (ThemePreset::OneDark, "#282c34", "#61afef", "#61afef"),
            (ThemePreset::GruvboxLight, "#fbf1c7", "#7c6f64", "#458588"),
            (ThemePreset::GruvboxDark, "#282828", "#d5c4a1", "#83a598"),
            (ThemePreset::AyuLight, "#fcfcfc", "#5c6166", "#55b4d4"),
            (ThemePreset::AyuDark, "#0b0e14", "#59c2ff", "#59c2ff"),
            (ThemePreset::Brutalist, "#f6f3e9", "#000000", "#FF6B35"),
            (ThemePreset::Spotless, "#EDE8DC", "#2c2416", "#8b5e34"),
            (ThemePreset::Cyberpunk, "#051423", "#00f2ff", "#22d3ee"),
            (ThemePreset::Ghibli, "#FDF6E3", "#6B5B4F", "#FFFFFF"),
        ];

        for (preset, canvas, line, first_series) in expected {
            let (theme, flowchart) = compiled(preset);
            assert_eq!(
                solid_color(theme.spec().canvas().base()).as_deref(),
                Some(canvas)
            );
            assert_eq!(
                flowchart
                    .style(ThemeTarget::Edge, ThemeVariant::Default, None)
                    .stroke()
                    .and_then(solid_color)
                    .as_deref(),
                Some(line)
            );
            let chart = theme.resolve(DiagramFamilyId::XY_CHART);
            assert_eq!(
                chart
                    .series_color(ThemeTarget::ChartSeries, 1)
                    .map(ThemeColorValue::as_css)
                    .as_deref(),
                Some(first_series)
            );
            let mindmap = theme.resolve(DiagramFamilyId::MINDMAP);
            assert_eq!(
                mindmap
                    .series_color(ThemeTarget::Node, 1)
                    .map(ThemeColorValue::as_css)
                    .as_deref(),
                Some(first_series)
            );
            let kanban = theme.resolve(DiagramFamilyId::KANBAN);
            assert_eq!(
                kanban
                    .series_color(ThemeTarget::Task, 1)
                    .map(ThemeColorValue::as_css)
                    .as_deref(),
                Some(first_series)
            );
        }
    }

    #[test]
    fn built_in_presets_preserve_family_specific_winners_and_all_ordinal_palettes() {
        for preset in all_presets() {
            let palette = preset_palette(preset);
            let theme = DiagramThemeCompiler::new()
                .compile_preset(preset)
                .expect("built-in theme preset should compile");

            assert_style(
                &theme,
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Node,
                Some(palette.surface),
                Some(palette.border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::MINDMAP,
                ThemeTarget::Node,
                None,
                Some(palette.border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::GIT_GRAPH,
                ThemeTarget::Node,
                None,
                Some(palette.border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::SANKEY,
                ThemeTarget::Node,
                None,
                Some(palette.border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::FLOWCHART,
                ThemeTarget::Cluster,
                Some(palette.cluster_background),
                Some(palette.cluster_border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Actor,
                Some(palette.actor_background),
                Some(palette.actor_border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Lifeline,
                Some(palette.actor_border),
                Some(palette.actor_border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::SequenceNumberLabel,
                Some(palette.sequence_number_text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::MessageLabel,
                Some(palette.text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::LoopLabelBackground,
                Some(palette.surface_alt),
                Some(palette.border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::LoopLabel,
                Some(palette.text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Note,
                Some(palette.note_background),
                Some(palette.note_border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::NoteLabel,
                Some(palette.note_text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::SEQUENCE,
                ThemeTarget::Activation,
                Some(palette.activation_background),
                Some(palette.activation_border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::PACKET,
                ThemeTarget::PacketByteLabel,
                Some(palette.text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::PACKET,
                ThemeTarget::PacketFieldLabel,
                Some(palette.packet_field_label_text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::XY_CHART,
                ThemeTarget::Axis,
                Some(palette.text),
                Some(palette.line),
            );
            assert_style(
                &theme,
                DiagramFamilyId::XY_CHART,
                ThemeTarget::Legend,
                Some(palette.subtle_text),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::ER,
                ThemeTarget::Relation,
                None,
                Some(palette.line),
            );
            assert_style(
                &theme,
                DiagramFamilyId::REQUIREMENT,
                ThemeTarget::Requirement,
                Some(palette.surface),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::PIE,
                ThemeTarget::PieSlice,
                None,
                Some(palette.border),
            );
            assert_style(
                &theme,
                DiagramFamilyId::QUADRANT_CHART,
                ThemeTarget::ChartSeries,
                palette.series.first().copied(),
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::GANTT,
                ThemeTarget::Task,
                Some(palette.surface),
                Some(palette.border),
            );

            // Family-qualified fallbacks must not become static winners for palette-owned
            // families that share the same semantic target.
            assert_style(
                &theme,
                DiagramFamilyId::KANBAN,
                ThemeTarget::Task,
                None,
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::XY_CHART,
                ThemeTarget::ChartSeries,
                None,
                None,
            );
            assert_style(
                &theme,
                DiagramFamilyId::RADAR,
                ThemeTarget::ChartSeries,
                None,
                None,
            );

            assert_eq!(
                theme
                    .resolve(DiagramFamilyId::GANTT)
                    .ordinal_palette_disposition(ThemeTarget::Task),
                Some(FamilyThemeDisposition::Unsupported),
                "{} Gantt palette residual",
                preset.id()
            );
            assert_eq!(
                theme
                    .resolve(DiagramFamilyId::QUADRANT_CHART)
                    .ordinal_palette_disposition(ThemeTarget::ChartSeries),
                Some(FamilyThemeDisposition::Unsupported),
                "{} Quadrant palette residual",
                preset.id()
            );
            for (family, target) in [
                (DiagramFamilyId::KANBAN, ThemeTarget::Task),
                (DiagramFamilyId::XY_CHART, ThemeTarget::ChartSeries),
                (DiagramFamilyId::RADAR, ThemeTarget::ChartSeries),
            ] {
                assert_eq!(
                    theme.resolve(family).ordinal_palette_disposition(target),
                    Some(FamilyThemeDisposition::TypedAdapter),
                    "{} {family} {target:?} palette route",
                    preset.id()
                );
            }

            for (family, target) in [
                (DiagramFamilyId::MINDMAP, ThemeTarget::Node),
                (DiagramFamilyId::KANBAN, ThemeTarget::Task),
                (DiagramFamilyId::XY_CHART, ThemeTarget::ChartSeries),
                (DiagramFamilyId::RADAR, ThemeTarget::ChartSeries),
                (DiagramFamilyId::PIE, ThemeTarget::PieSlice),
                (DiagramFamilyId::TIMELINE, ThemeTarget::TimelineEvent),
                (DiagramFamilyId::JOURNEY, ThemeTarget::JourneyTask),
            ] {
                let resolved = theme.resolve(family);
                assert_eq!(
                    resolved
                        .series_color(target, 1)
                        .map(ThemeColorValue::as_css)
                        .as_deref(),
                    palette.series.first().copied(),
                    "{family} {target:?} palette"
                );
            }
        }
    }

    #[test]
    fn built_in_sequence_presets_pair_role_foregrounds_and_backgrounds() {
        const MINIMUM_TEXT_CONTRAST: f64 = 4.5;

        for preset in all_presets() {
            let palette = preset_palette(preset);
            let theme = DiagramThemeCompiler::new()
                .compile_preset(preset)
                .expect("built-in theme preset should compile");
            let sequence = theme.resolve(DiagramFamilyId::SEQUENCE);
            let canvas = solid_color(theme.spec().canvas().base())
                .expect("built-in preset canvas must be a solid color");
            let sequence_number = resolved_solid_fill(&sequence, ThemeTarget::SequenceNumberLabel);
            let sequence_number_background = resolved_solid_stroke(&sequence, ThemeTarget::Message);
            let message_text = resolved_solid_fill(&sequence, ThemeTarget::MessageLabel);
            let loop_text = resolved_solid_fill(&sequence, ThemeTarget::LoopLabel);
            let loop_background = resolved_solid_fill(&sequence, ThemeTarget::LoopLabelBackground);
            let note_text = resolved_solid_fill(&sequence, ThemeTarget::NoteLabel);
            let note_background = resolved_solid_fill(&sequence, ThemeTarget::Note);

            let pairs = [
                (
                    "sequence-number/message-stroke",
                    sequence_number.as_str(),
                    sequence_number_background.as_str(),
                    palette.sequence_number_text,
                    palette.line,
                ),
                (
                    "message-label/canvas",
                    message_text.as_str(),
                    canvas.as_str(),
                    palette.text,
                    palette.canvas,
                ),
                (
                    "loop-label/loop",
                    loop_text.as_str(),
                    loop_background.as_str(),
                    palette.text,
                    palette.surface_alt,
                ),
                (
                    "loop-label/canvas",
                    loop_text.as_str(),
                    canvas.as_str(),
                    palette.text,
                    palette.canvas,
                ),
                (
                    "note-label/note",
                    note_text.as_str(),
                    note_background.as_str(),
                    palette.note_text,
                    palette.note_background,
                ),
            ];

            for (role, foreground, background, expected_foreground, expected_background) in pairs {
                assert_eq!(
                    foreground,
                    expected_foreground,
                    "{} {role} foreground recipe",
                    preset.id()
                );
                assert_eq!(
                    background,
                    expected_background,
                    "{} {role} background recipe",
                    preset.id()
                );
                let ratio = contrast_ratio(foreground, background);
                assert!(
                    ratio >= MINIMUM_TEXT_CONTRAST,
                    "{} {role} contrast {ratio:.2}:1 is below {MINIMUM_TEXT_CONTRAST}:1 ({foreground} on {background})",
                    preset.id()
                );
            }
        }
    }

    #[cfg(feature = "diagram-kanban")]
    #[test]
    fn built_in_kanban_presets_pair_each_task_palette_slot_with_a_readable_label() {
        const MINIMUM_TEXT_CONTRAST: f64 = 4.5;
        let mut source = "kanban\n  todo[Todo]\n".to_owned();
        for ordinal in 1..=24 {
            source.push_str(&format!("    task{ordinal}[Task {ordinal}]\n"));
        }
        for preset in all_presets() {
            let palette = preset_palette(preset);
            let theme = DiagramThemeCompiler::new().compile_preset(preset).unwrap();
            let dark_modes: &[Option<bool>] = if preset == ThemePreset::Cyberpunk {
                &[None, Some(false), Some(true)]
            } else {
                &[None]
            };
            for &dark_mode in dark_modes {
                let engine = match dark_mode {
                    Some(value) => merman_core::MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({"darkMode": value}),
                        ),
                    ),
                    None => merman_core::MermaidEngine::new(),
                };
                let parsed = crate::test_support::install_parse_compatibility(&theme, engine)
                    .parse_diagram_for_render_model_sync(
                        &source,
                        merman_core::ParseOptions::strict(),
                    )
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let session = crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let rendered =
                    crate::family::prepare(&parsed, &crate::LayoutOptions::default(), session)
                        .unwrap()
                        .render_svg(
                            &crate::svg::SvgRenderOptions {
                                diagram_id: Some("kanban-contrast".into()),
                                ..Default::default()
                            },
                            &crate::svg::SvgDebugOptions::default(),
                        )
                        .unwrap();
                let document = crate::xml_dom::Document::parse(rendered.svg()).unwrap();
                for ordinal in 1..=24 {
                    let id = format!("kanban-contrast-task{ordinal}");
                    let group = document
                        .descendants()
                        .find(|node| node.attribute("id") == Some(id.as_str()))
                        .unwrap();
                    let fill = group
                        .children()
                        .find(|node| node.has_tag_name("rect"))
                        .and_then(|node| node.attribute("style"))
                        .and_then(|style| {
                            style
                                .split(';')
                                .filter_map(|declaration| declaration.split_once(':'))
                                .find_map(|(property, value)| {
                                    (property.trim() == "fill").then_some(value.trim())
                                })
                        })
                        .unwrap();
                    let label = group
                        .descendants()
                        .filter_map(|node| node.attribute("style"))
                        .find_map(|style| {
                            style
                                .split(';')
                                .filter_map(|declaration| declaration.split_once(':'))
                                .find_map(|(property, value)| {
                                    (property.trim() == "color").then_some(value.trim())
                                })
                        })
                        .unwrap();
                    let slot = (ordinal - 1) % 12;
                    assert_eq!(
                        label,
                        palette.kanban_task_labels[slot % palette.kanban_task_labels.len()]
                    );
                    let ratio = contrast_ratio(label, fill);
                    assert!(
                        ratio >= MINIMUM_TEXT_CONTRAST,
                        "{} darkMode={dark_mode:?} item {ordinal} terminal contrast {ratio:.2}:1 ({label} on {fill})",
                        preset.id()
                    );
                }
            }
        }
    }

    #[test]
    fn built_in_packet_presets_keep_field_labels_readable_on_the_light_block() {
        const PACKET_BLOCK_BACKGROUND: &str = "#efefef";
        const MINIMUM_TEXT_CONTRAST: f64 = 4.5;

        for preset in all_presets() {
            let palette = preset_palette(preset);
            let theme = DiagramThemeCompiler::new()
                .compile_preset(preset)
                .expect("built-in theme preset should compile");
            let packet = theme.resolve(DiagramFamilyId::PACKET);
            let field_label = resolved_solid_fill(&packet, ThemeTarget::PacketFieldLabel);

            assert_eq!(
                field_label.as_str(),
                palette.packet_field_label_text,
                "{} Packet field-label recipe",
                preset.id()
            );
            let ratio = contrast_ratio(&field_label, PACKET_BLOCK_BACKGROUND);
            assert!(
                ratio >= MINIMUM_TEXT_CONTRAST,
                "{} Packet field-label contrast {ratio:.2}:1 is below {MINIMUM_TEXT_CONTRAST}:1 ({field_label} on {PACKET_BLOCK_BACKGROUND})",
                preset.id()
            );
        }
    }
}
