mod catalog;

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
/// One freshly qualified diagram-family/output cell for a preset recipe revision.
pub struct ThemePresetQualifiedCell {
    family_id: &'static str,
    output_id: &'static str,
}

impl ThemePresetQualifiedCell {
    /// Creates a qualified cell from stable family and output identifiers.
    pub const fn new(family_id: &'static str, output_id: &'static str) -> Self {
        Self {
            family_id,
            output_id,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct ThemePresetQualificationInvalidation {
    qualification_schema_revision: u32,
    admission_revision: u32,
    invalidate_on_recipe_fingerprint_change: bool,
    invalidate_on_resource_fingerprint_change: bool,
}

impl ThemePresetQualificationInvalidation {
    pub(super) const fn current() -> Self {
        Self {
            qualification_schema_revision: 1,
            admission_revision: 1,
            invalidate_on_recipe_fingerprint_change: true,
            invalidate_on_resource_fingerprint_change: true,
        }
    }

    const fn qualification_schema_revision(self) -> u32 {
        self.qualification_schema_revision
    }

    const fn admission_revision(self) -> u32 {
        self.admission_revision
    }

    const fn invalidate_on_recipe_fingerprint_change(self) -> bool {
        self.invalidate_on_recipe_fingerprint_change
    }

    const fn invalidate_on_resource_fingerprint_change(self) -> bool {
        self.invalidate_on_resource_fingerprint_change
    }
}

pub const fn theme_preset_descriptors() -> &'static [ThemePresetDescriptor] {
    catalog::descriptors()
}

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
        ResolvedDiagramTheme, ThemeColorValue, ThemeTarget, ThemeVariant,
    };

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

            let rgba = merman_core::theme_color::ThemeColor::parse(color)
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

    #[test]
    fn built_in_presets_keep_mermaid_compatibility_without_layout_or_look() {
        for preset in all_presets() {
            let (theme, _) = compiled(preset);
            let config = theme.spec().mermaid().to_mermaid_config();

            assert_eq!(config.get_str("theme"), Some("base"));
            assert_eq!(
                config.get_bool("darkMode"),
                Some(preset.is_dark()),
                "{} dark-mode compatibility value",
                preset.id()
            );
            assert_eq!(config.get_str("look"), None);
            assert_eq!(config.get_str("flowchart.defaultRenderer"), None);
        }
    }

    #[test]
    fn preset_catalog_is_the_complete_unique_enum_projection() {
        const EXPECTED_PRESETS: [ThemePreset; 10] = [
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
            assert_eq!(entry.catalog_schema_version(), 1);
            assert_eq!(entry.authoring_schema_version(), 1);
            assert_eq!(entry.expansion_version(), 1);
            assert_eq!(entry.spec_schema_version(), 1);
            assert_eq!(entry.recipe_revision(), 4);
            assert_eq!(descriptor.maturity(), "alpha");
            assert!(descriptor.qualified_cells().is_empty());
            assert_eq!(descriptor.export_kind(), "complete_spec");
            assert_eq!(
                entry
                    .qualification_invalidation()
                    .qualification_schema_revision(),
                1
            );
            assert_eq!(entry.qualification_invalidation().admission_revision(), 1);
            assert!(
                entry
                    .qualification_invalidation()
                    .invalidate_on_recipe_fingerprint_change()
            );
            assert!(
                entry
                    .qualification_invalidation()
                    .invalidate_on_resource_fingerprint_change()
            );
            assert_eq!(descriptor.license_expression(), "MIT OR Apache-2.0");
            assert_eq!(descriptor.required_attribution(), None);
            assert!(entry.required_feature_ids().is_empty());
            assert!(entry.bundled_resource_ids().is_empty());
            assert!(entry.allowed_residual_ids().is_empty());
            assert!(entry.retained());
        }
    }

    #[test]
    fn preset_catalog_fingerprints_match_the_exact_compiled_recipes() {
        let mut recipe_fingerprints = BTreeSet::new();
        let mut mismatches = Vec::new();
        for descriptor in theme_preset_descriptors() {
            let entry = catalog::entry_for_preset(descriptor.preset());
            let catalog_fingerprint = entry.recipe_fingerprint();
            assert_eq!(catalog_fingerprint.len(), 64);
            assert!(
                catalog_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            );

            let theme = DiagramThemeCompiler::new()
                .compile_preset(descriptor.preset())
                .expect("catalog recipe must compile");
            let actual_recipe = theme.recipe_fingerprint().to_hex();
            let actual_resources = theme.report().font_catalog_fingerprint().to_hex();

            if catalog_fingerprint != actual_recipe {
                mismatches.push(format!("{}: {actual_recipe}", descriptor.preset().id()));
            }
            assert_eq!(entry.resource_fingerprint(), actual_resources.as_str());
            assert!(
                recipe_fingerprints.insert(catalog_fingerprint),
                "catalog recipes must have unique fingerprints"
            );
        }
        assert!(
            mismatches.is_empty(),
            "catalog recipe fingerprints drifted; sign these exact compiled values:\n{}",
            mismatches.join("\n")
        );
    }

    #[test]
    fn all_ten_catalog_recipes_round_trip_as_closed_complete_specs() {
        for descriptor in theme_preset_descriptors() {
            let compiler = DiagramThemeCompiler::new();
            let merman_theme_contract::PresetExportV1::CompleteSpec {
                complete_spec: original,
            } = compiler
                .export_preset(descriptor.preset())
                .expect("catalog preset must export")
            else {
                panic!("current catalog presets must export complete specs")
            };
            let encoded = serde_json::to_vec(&original).expect("complete spec must serialize");
            let decoded: merman_theme_contract::DiagramThemeSpecWireV1 =
                serde_json::from_slice(&encoded).expect("complete spec must deserialize");
            let round_tripped = DiagramThemeCompiler::new()
                .compile_spec_wire(decoded)
                .expect("round-tripped catalog recipe must compile");

            assert_eq!(
                round_tripped.recipe_fingerprint().to_hex(),
                catalog::entry_for_preset(descriptor.preset()).recipe_fingerprint()
            );
        }
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
            crate::diagram_theme::ThemeDefinitionCompileError::Materialization(ref error)
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
    fn preset_complete_spec_exports_preserve_mermaid_compatibility() {
        let compiler = DiagramThemeCompiler::new();
        for descriptor in theme_preset_descriptors() {
            let merman_theme_contract::PresetExportV1::CompleteSpec { complete_spec } = compiler
                .export_preset(descriptor.preset())
                .expect("preset export must materialize")
            else {
                panic!("current preset recipes require the complete_spec export variant")
            };
            let mermaid = complete_spec
                .mermaid
                .as_ref()
                .expect("preset export must retain Mermaid compatibility");
            assert_eq!(mermaid.theme.as_deref(), Some("base"));
            assert_eq!(mermaid.dark_mode, Some(descriptor.is_dark()));
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
            (ThemePreset::Brutalist, "#f4f0e6", "#111111", "#ff4f00"),
            (ThemePreset::Spotless, "#f7f5ef", "#2c2416", "#8b5e34"),
            (ThemePreset::Cyberpunk, "#020617", "#22d3ee", "#22d3ee"),
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
                ThemeTarget::Loop,
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
                None,
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
            let loop_background = resolved_solid_fill(&sequence, ThemeTarget::Loop);
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

    #[test]
    fn built_in_kanban_presets_pair_each_task_palette_slot_with_a_readable_label() {
        const MINIMUM_TEXT_CONTRAST: f64 = 4.5;

        for preset in all_presets() {
            let palette = preset_palette(preset);
            let theme = DiagramThemeCompiler::new()
                .compile_preset(preset)
                .expect("built-in theme preset should compile");
            let kanban = theme.resolve(DiagramFamilyId::KANBAN);

            for item_ordinal in 1..=24 {
                let terminal_ordinal = (item_ordinal - 1) % 12 + 1;
                let task_fill = kanban
                    .series_color(ThemeTarget::Task, terminal_ordinal)
                    .map(ThemeColorValue::as_css)
                    .expect("Kanban task palette slot");
                let terminal_fill = if preset.is_dark() {
                    merman_core::theme_color::darken(&task_fill, 10.0)
                } else {
                    merman_core::theme_color::lighten(&task_fill, 10.0)
                }
                .expect("built-in Kanban task color");
                let label = kanban
                    .style(
                        ThemeTarget::TaskLabel,
                        ThemeVariant::Default,
                        Some(item_ordinal),
                    )
                    .fill()
                    .and_then(solid_color)
                    .expect("each Kanban task palette slot must own its label foreground");
                assert_eq!(
                    label,
                    palette.kanban_task_labels
                        [(terminal_ordinal - 1) % palette.kanban_task_labels.len()],
                    "{} Kanban item {item_ordinal} terminal slot {terminal_ordinal} authored label foreground",
                    preset.id()
                );
                let ratio = contrast_ratio(&label, &terminal_fill);

                assert!(
                    ratio >= MINIMUM_TEXT_CONTRAST,
                    "{} Kanban item {item_ordinal} terminal slot {terminal_ordinal} contrast {ratio:.2}:1 is below {MINIMUM_TEXT_CONTRAST}:1 ({label} on {terminal_fill})",
                    preset.id()
                );
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
