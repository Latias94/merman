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

    pub(crate) fn spec_wire(self) -> merman_theme_contract::DiagramThemeSpecWireV1 {
        catalog::build_spec_wire(self)
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

    pub const fn catalog_schema_version(self) -> u32 {
        self.entry().catalog_schema_version()
    }

    pub const fn authoring_schema_version(self) -> u32 {
        self.entry().authoring_schema_version()
    }

    pub const fn expansion_version(self) -> u32 {
        self.entry().expansion_version()
    }

    pub const fn spec_schema_version(self) -> u32 {
        self.entry().spec_schema_version()
    }

    pub const fn recipe_revision(self) -> u32 {
        self.entry().recipe_revision()
    }

    pub fn recipe_fingerprint(self) -> &'static str {
        self.entry().recipe_fingerprint()
    }

    pub const fn resource_fingerprint(self) -> &'static str {
        self.entry().resource_fingerprint()
    }

    pub const fn qualified_cell_count(self) -> usize {
        self.entry().qualified_cells().len()
    }

    pub const fn qualified_cells(self) -> &'static [ThemePresetQualifiedCell] {
        self.entry().qualified_cells()
    }

    pub const fn export_kind(self) -> &'static str {
        self.entry().export_kind()
    }

    pub const fn qualification_schema_revision(self) -> u32 {
        self.entry()
            .qualification_invalidation()
            .qualification_schema_revision()
    }

    pub const fn qualification_admission_revision(self) -> u32 {
        self.entry()
            .qualification_invalidation()
            .admission_revision()
    }

    pub const fn qualification_invalidates_recipe_changes(self) -> bool {
        self.entry()
            .qualification_invalidation()
            .invalidate_on_recipe_fingerprint_change()
    }

    pub const fn qualification_invalidates_resource_changes(self) -> bool {
        self.entry()
            .qualification_invalidation()
            .invalidate_on_resource_fingerprint_change()
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
        CanvasPaint, DiagramTheme, DiagramThemeCompiler, ResolvedDiagramTheme, ThemeColorValue,
        ThemeTarget, ThemeVariant,
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
            fn linear_channel(channel: u8) -> f64 {
                let channel = f64::from(channel) / 255.0;
                if channel <= 0.04045 {
                    channel / 12.92
                } else {
                    ((channel + 0.055) / 1.055).powf(2.4)
                }
            }

            let hex = color
                .strip_prefix('#')
                .unwrap_or_else(|| panic!("expected a hex color, got {color}"));
            assert_eq!(hex.len(), 6, "expected a six-digit hex color, got {color}");
            let channel = |offset| {
                u8::from_str_radix(&hex[offset..offset + 2], 16)
                    .unwrap_or_else(|_| panic!("expected a valid hex color, got {color}"))
            };
            0.2126 * linear_channel(channel(0))
                + 0.7152 * linear_channel(channel(2))
                + 0.0722 * linear_channel(channel(4))
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
            assert_eq!(descriptor.catalog_schema_version(), 1);
            assert_eq!(descriptor.authoring_schema_version(), 1);
            assert_eq!(descriptor.expansion_version(), 1);
            assert_eq!(descriptor.spec_schema_version(), 1);
            assert_eq!(descriptor.recipe_revision(), 1);
            assert_eq!(descriptor.maturity(), "alpha");
            assert_eq!(descriptor.qualified_cell_count(), 0);
            assert!(descriptor.qualified_cells().is_empty());
            assert_eq!(descriptor.export_kind(), "definition");
            assert_eq!(descriptor.qualification_schema_revision(), 1);
            assert_eq!(descriptor.qualification_admission_revision(), 1);
            assert!(descriptor.qualification_invalidates_recipe_changes());
            assert!(descriptor.qualification_invalidates_resource_changes());
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
        for descriptor in theme_preset_descriptors() {
            let first_catalog_fingerprint = descriptor.recipe_fingerprint();
            let second_catalog_fingerprint = descriptor.recipe_fingerprint();
            assert_eq!(first_catalog_fingerprint, second_catalog_fingerprint);
            assert_eq!(first_catalog_fingerprint.len(), 64);
            assert!(
                first_catalog_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            );

            let theme = DiagramThemeCompiler::new()
                .compile_preset(descriptor.preset())
                .expect("catalog recipe must compile");
            let actual_recipe = theme.recipe_fingerprint().to_hex();
            let actual_resources = theme.report().font_catalog_fingerprint().to_hex();

            assert_eq!(first_catalog_fingerprint, actual_recipe.as_str());
            assert_eq!(descriptor.resource_fingerprint(), actual_resources.as_str());
            assert!(
                recipe_fingerprints.insert(first_catalog_fingerprint),
                "catalog recipes must have unique fingerprints"
            );
        }
    }

    #[test]
    fn all_ten_catalog_recipes_round_trip_as_closed_complete_specs() {
        for descriptor in theme_preset_descriptors() {
            let original = descriptor.preset().spec_wire();
            let encoded = serde_json::to_vec(&original).expect("complete spec must serialize");
            let decoded: merman_theme_contract::DiagramThemeSpecWireV1 =
                serde_json::from_slice(&encoded).expect("complete spec must deserialize");
            let round_tripped = DiagramThemeCompiler::new()
                .compile_spec_wire(decoded)
                .expect("round-tripped catalog recipe must compile");

            assert_eq!(
                round_tripped.recipe_fingerprint().to_hex(),
                descriptor.recipe_fingerprint()
            );
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

            for (family, target) in [
                (DiagramFamilyId::MINDMAP, ThemeTarget::Node),
                (DiagramFamilyId::KANBAN, ThemeTarget::Task),
                (DiagramFamilyId::XY_CHART, ThemeTarget::ChartSeries),
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
