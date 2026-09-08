use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io;

use merman_theme_contract::{
    MaterializedThemeWireV1, SpecifiedWireV1, ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1,
    ThemeDefinitionV1, ThemeLineHeightWireV1, ThemeMaterializationDiagnosticV1,
    ThemeMaterializationErrorV1, ThemeRuleSetWireV1, ThemeStylePatchWireV1,
    ThemeTextStylePatchWireV1, resolve_authoring_version,
};
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

use super::materializer::{
    COLOR_DEFAULTS, GENERATED_PALETTE_TARGETS, MAX_AUTHORED_RULES, ThemeMaterializationError,
    ThemeMaterializer, first_rejected_actual,
};
use super::{
    DiagramTheme, DiagramThemeCompiler, ThemeCompileError, ThemeResourceLimitExceeded,
    ThemeResourcePolicy,
};

const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_OBJECT_MEMBERS: usize = 64;
const MAX_JSON_ARRAY_ITEMS: usize = 1_024;
const MAX_TOTAL_COLLECTION_ITEMS: usize = 65_536;
const MAX_JSON_STRING_BYTES: usize = 64 * 1_024;
const PREFLIGHT_SENTINEL: &str = "theme definition JSON preflight failed";

fn decode_bounded_theme_definition_json(
    resources: &ThemeResourcePolicy,
    bytes: &[u8],
) -> Result<ThemeDefinitionV1, ThemeMaterializationErrorV1> {
    resources
        .check_theme_encoded_bytes(bytes.len())
        .map_err(encoded_bytes_contract_error)?;
    preflight_json(bytes)?;
    serde_json::from_slice::<ThemeDefinitionV1>(bytes).map_err(|_| {
        invalid_json_contract_error(
            "contract-shape",
            "theme definition does not match the closed version one contract",
        )
    })
}

/// Admitted, borrowed authoring input proof consumed by the pure materializer.
///
/// The proof is private to the renderer. Public callers must enter through one of the
/// `materialize_theme*` functions below so typed and JSON inputs receive the same host-owned
/// admission policy before deterministic lowering starts.
#[derive(Debug)]
pub(super) struct AdmittedThemeDefinition<'a> {
    definition: &'a ThemeDefinitionV1,
}

impl<'a> AdmittedThemeDefinition<'a> {
    fn new(definition: &'a ThemeDefinitionV1) -> Self {
        Self { definition }
    }

    pub(super) fn definition(self) -> &'a ThemeDefinitionV1 {
        self.definition
    }
}

/// Materializes one typed definition using the interactive host resource policy.
pub fn materialize_theme(
    definition: &ThemeDefinitionV1,
) -> Result<MaterializedThemeWireV1, ThemeMaterializationErrorV1> {
    materialize_theme_with_resource_policy(definition, &ThemeResourcePolicy::interactive())
}

/// Materializes one typed definition after applying the caller-owned resource policy.
pub fn materialize_theme_with_resource_policy(
    definition: &ThemeDefinitionV1,
    resources: &ThemeResourcePolicy,
) -> Result<MaterializedThemeWireV1, ThemeMaterializationErrorV1> {
    let admitted =
        admit_typed_definition(resources, definition).map_err(admission_contract_error)?;
    ThemeMaterializer::materialize(admitted).map_err(ThemeMaterializationError::into_contract_error)
}

/// Decodes and materializes one JSON definition using the interactive host resource policy.
pub fn materialize_theme_json(
    bytes: &[u8],
) -> Result<MaterializedThemeWireV1, ThemeMaterializationErrorV1> {
    materialize_theme_json_with_resource_policy(bytes, &ThemeResourcePolicy::interactive())
}

/// Decodes and materializes one JSON definition after applying the caller-owned resource policy.
pub fn materialize_theme_json_with_resource_policy(
    bytes: &[u8],
    resources: &ThemeResourcePolicy,
) -> Result<MaterializedThemeWireV1, ThemeMaterializationErrorV1> {
    let definition = decode_bounded_theme_definition_json(resources, bytes)?;
    let admitted = AdmittedThemeDefinition::new(&definition);
    ThemeMaterializer::materialize(admitted).map_err(ThemeMaterializationError::into_contract_error)
}

/// Admits, materializes, and compiles one typed versioned theme definition.
pub fn compile_theme_definition(
    compiler: &DiagramThemeCompiler,
    definition: &ThemeDefinitionV1,
) -> Result<DiagramTheme, ThemeDefinitionCompileError> {
    let materialized =
        materialize_theme_with_resource_policy(definition, compiler.resource_policy())?;
    Ok(compiler.compile_spec_wire(materialized.into_spec())?)
}

/// Decodes, admits, materializes, and compiles one versioned theme-definition JSON document.
pub fn compile_theme_definition_json(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, ThemeDefinitionCompileError> {
    let materialized =
        materialize_theme_json_with_resource_policy(bytes, compiler.resource_policy())?;
    Ok(compiler.compile_spec_wire(materialized.into_spec())?)
}

/// A fatal failure while materializing or compiling a theme definition.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeDefinitionCompileError {
    /// The authoring definition could not be expanded into a complete theme recipe.
    #[error("theme definition materialization failed: {0}")]
    Materialization(#[from] ThemeMaterializationErrorV1),
    /// The complete theme recipe did not satisfy compiler semantics or resource policy.
    #[error("theme definition compilation failed: {0}")]
    Compilation(#[from] ThemeCompileError),
}

/// Internal definition-input failure detected before materialization expands a recipe.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum ThemeDefinitionAdmissionError {
    /// The typed definition could not be serialized for resource accounting.
    #[error("theme definition could not be encoded for admission: {message}")]
    TypedEncoding {
        /// A fixed bounded display message; serializer text is not propagated.
        message: String,
    },
    /// One typed collection exceeded its versioned implementation ceiling.
    #[error("theme definition collection `{path}` has {actual} items; maximum is {max}")]
    CollectionLimit {
        /// JSON Pointer into the original authored input.
        path: String,
        /// The resource domain is independent of the input location.
        limit_id: &'static str,
        /// Observed item count.
        actual: usize,
        /// Maximum admitted item count.
        max: usize,
    },
    /// The caller-owned compiler resource policy rejected the input.
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
}

fn admission_contract_error(error: ThemeDefinitionAdmissionError) -> ThemeMaterializationErrorV1 {
    match error {
        ThemeDefinitionAdmissionError::TypedEncoding { .. } => {
            contract_error(ThemeMaterializationDiagnosticV1::invalid_token_value(
                "",
                "finite-theme-definition",
                "typed theme definition cannot be encoded for bounded admission",
            ))
        }
        ThemeDefinitionAdmissionError::CollectionLimit {
            path,
            limit_id,
            actual,
            max,
        } => contract_error(ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
            path,
            limit_id,
            actual,
            max,
            "theme definition exceeds a bounded collection or string limit",
        )),
        ThemeDefinitionAdmissionError::ResourceLimit(error) => encoded_bytes_contract_error(error),
    }
}

fn encoded_bytes_contract_error(error: ThemeResourceLimitExceeded) -> ThemeMaterializationErrorV1 {
    contract_error(ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
        "",
        encoded_bytes_limit_id(error.limit),
        first_rejected_actual(error.max),
        error.max,
        "theme definition exceeds the caller-owned resource policy",
    ))
}

fn encoded_bytes_limit_id(limit: &str) -> &'static str {
    if limit == "theme_encoded_bytes_hard_cap" {
        "theme_encoded_bytes_hard_cap"
    } else {
        "max_theme_encoded_bytes"
    }
}

fn invalid_json_contract_error(
    reason_id: &'static str,
    message: &'static str,
) -> ThemeMaterializationErrorV1 {
    contract_error(ThemeMaterializationDiagnosticV1::invalid_definition_json(
        reason_id, message,
    ))
}

fn contract_error(diagnostic: ThemeMaterializationDiagnosticV1) -> ThemeMaterializationErrorV1 {
    ThemeMaterializationErrorV1::from_diagnostic(diagnostic)
}

fn admit_typed_definition<'a>(
    resources: &ThemeResourcePolicy,
    definition: &'a ThemeDefinitionV1,
) -> Result<AdmittedThemeDefinition<'a>, ThemeDefinitionAdmissionError> {
    let mut usage = TypedUsage::default();

    usage.charge_collection_items(definition.styles().len())?;
    for (index, entry) in definition.styles().iter().enumerate() {
        match entry {
            ThemeRuleSetWireV1::Rule {
                target,
                family,
                variant,
                style,
                ..
            } => {
                usage.string(format_args!("/styles/{index}/target"), target)?;
                if let Some(family) = family {
                    usage.string(format_args!("/styles/{index}/family"), family)?;
                }
                if let Some(variant) = variant {
                    usage.string(format_args!("/styles/{index}/variant"), variant)?;
                }
                usage.style(format_args!("/styles/{index}/style"), style)?;
            }
            ThemeRuleSetWireV1::OrdinalPalette { target, colors } => {
                usage.string(format_args!("/styles/{index}/target"), target)?;
                usage.collection(
                    format_args!("/styles/{index}/colors"),
                    colors.len(),
                    super::semantic::MAX_THEME_PALETTE_COLORS,
                    "max_theme_palette_colors",
                )?;
                for (color_index, color) in colors.iter().enumerate() {
                    usage.string(format_args!("/styles/{index}/colors/{color_index}"), color)?;
                }
            }
        }
    }

    for (token, _, path) in COLOR_DEFAULTS {
        if let Some(color) = definition.tokens().color(token) {
            usage.string(path, color)?;
        }
    }
    if let Some(series) = definition.tokens().series() {
        usage.collection(
            "/tokens/series",
            series.len(),
            super::semantic::MAX_THEME_PALETTE_COLORS,
            "max_theme_palette_colors",
        )?;
        for (index, color) in series.iter().enumerate() {
            usage.string(format_args!("/tokens/series/{index}"), color)?;
        }
    }
    if let Some(typography) = definition.tokens().typography()
        && let Some(font_stack) = typography.font_stack()
    {
        usage.collection(
            "/tokens/typography/font_stack",
            font_stack.len(),
            super::typography::MAX_FONT_STACK_ENTRIES,
            "max_theme_font_stack_entries",
        )?;
        for (index, family) in font_stack.iter().enumerate() {
            usage.font_family(
                format_args!("/tokens/typography/font_stack/{index}"),
                family,
            )?;
        }
    }

    // This is transport admission, not canonical identity. Stream the contract-owned wire shape
    // so an over-budget typed value stops at the first inadmissible byte without allocating a
    // complete JSON buffer. Canonical JSON remains the authority for exported identity/digests.
    let mut writer = BoundedCountingWriter::new(resources);
    let encoded = serde_json::to_writer(&mut writer, definition);
    if let Some(error) = writer.take_resource_error() {
        return Err(ThemeDefinitionAdmissionError::ResourceLimit(error));
    }
    encoded.map_err(|_| ThemeDefinitionAdmissionError::TypedEncoding {
        message: "typed theme definition cannot be encoded for bounded admission".to_owned(),
    })?;
    Ok(AdmittedThemeDefinition::new(definition))
}

#[derive(Default)]
struct TypedUsage {
    total_collection_items: usize,
}

impl TypedUsage {
    fn charge_collection_items(
        &mut self,
        actual: usize,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        self.total_collection_items = self.total_collection_items.saturating_add(actual);
        if self.total_collection_items > MAX_TOTAL_COLLECTION_ITEMS {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path: String::new(),
                limit_id: "max_theme_definition_collection_items",
                actual: first_rejected_actual(MAX_TOTAL_COLLECTION_ITEMS),
                max: MAX_TOTAL_COLLECTION_ITEMS,
            });
        }
        Ok(())
    }

    fn string(
        &self,
        path: impl fmt::Display,
        value: &str,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        if value.len() > MAX_JSON_STRING_BYTES {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path: path.to_string(),
                limit_id: "max_theme_definition_string_bytes",
                actual: value.len(),
                max: MAX_JSON_STRING_BYTES,
            });
        }
        Ok(())
    }

    fn font_family(
        &self,
        path: impl fmt::Display,
        value: &str,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        self.string(&path, value)?;
        if value.len() > super::typography::MAX_FONT_FAMILY_BYTES {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path: path.to_string(),
                limit_id: "max_theme_font_family_bytes",
                actual: value.len(),
                max: super::typography::MAX_FONT_FAMILY_BYTES,
            });
        }
        Ok(())
    }

    fn collection(
        &mut self,
        path: impl fmt::Display,
        actual: usize,
        field_max: usize,
        limit_id: &'static str,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        let max = field_max.min(MAX_JSON_ARRAY_ITEMS);
        if actual > max {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path: path.to_string(),
                limit_id,
                actual: first_rejected_actual(max),
                max,
            });
        }
        self.charge_collection_items(actual)
    }

    fn style(
        &mut self,
        path: impl fmt::Display,
        style: &ThemeStylePatchWireV1,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        if let Some(stroke) = &style.stroke {
            if let SpecifiedWireV1::Value(dasharray) = &stroke.dasharray {
                self.collection(
                    format_args!("{path}/stroke/dasharray"),
                    dasharray.len(),
                    MAX_JSON_ARRAY_ITEMS,
                    "max_theme_definition_array_items",
                )?;
            }
            if let SpecifiedWireV1::Value(paint) = &stroke.paint {
                self.paint(format_args!("{path}/stroke/paint"), paint)?;
            }
            if let SpecifiedWireV1::Value(value) = &stroke.linecap {
                self.string(format_args!("{path}/stroke/linecap"), value)?;
            }
            if let SpecifiedWireV1::Value(value) = &stroke.linejoin {
                self.string(format_args!("{path}/stroke/linejoin"), value)?;
            }
        }
        if let SpecifiedWireV1::Value(paint) = &style.fill {
            self.paint(format_args!("{path}/fill"), paint)?;
        }
        if let Some(typography) = &style.typography {
            self.typography(format_args!("{path}/typography"), typography)?;
        }
        if let SpecifiedWireV1::Value(value) = &style.effect {
            self.string(format_args!("{path}/effect"), value)?;
        }
        Ok(())
    }

    fn typography(
        &mut self,
        path: impl fmt::Display,
        typography: &ThemeTextStylePatchWireV1,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        if let SpecifiedWireV1::Value(font_stack) = &typography.font_stack {
            self.collection(
                format_args!("{path}/font_stack"),
                font_stack.len(),
                super::typography::MAX_FONT_STACK_ENTRIES,
                "max_theme_font_stack_entries",
            )?;
            for (index, family) in font_stack.iter().enumerate() {
                self.font_family(format_args!("{path}/font_stack/{index}"), family)?;
            }
        }
        for (field, value) in [
            ("font_style", &typography.font_style),
            ("transform", &typography.transform),
            ("decoration", &typography.decoration),
            ("text_align", &typography.text_align),
            ("white_space", &typography.white_space),
            ("wrap", &typography.wrap),
        ] {
            if let SpecifiedWireV1::Value(value) = value {
                self.string(format_args!("{path}/{field}"), value)?;
            }
        }
        if let SpecifiedWireV1::Value(ThemeLineHeightWireV1::Keyword(value)) =
            &typography.line_height
        {
            self.string(format_args!("{path}/line_height"), value)?;
        }
        Ok(())
    }

    fn paint(
        &mut self,
        path: impl fmt::Display,
        paint: &ThemeCanvasPaintWireV1,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        let paint = match paint {
            ThemeCanvasPaintWireV1::Color(color) => {
                return self.string(path, color);
            }
            ThemeCanvasPaintWireV1::Structured(paint) => paint,
        };
        let stops = match paint {
            ThemeCanvasPaintObjectWireV1::Transparent => None,
            ThemeCanvasPaintObjectWireV1::Solid { color } => {
                self.string(format_args!("{path}/color"), color)?;
                None
            }
            ThemeCanvasPaintObjectWireV1::LinearGradient { stops, .. }
            | ThemeCanvasPaintObjectWireV1::RadialGradient { stops, .. } => Some(stops),
            ThemeCanvasPaintObjectWireV1::Pattern {
                pattern,
                foreground,
                background,
                ..
            } => {
                self.string(format_args!("{path}/pattern"), pattern)?;
                self.string(format_args!("{path}/foreground"), foreground)?;
                if let Some(background) = background {
                    self.string(format_args!("{path}/background"), background)?;
                }
                None
            }
        };
        if let Some(stops) = stops {
            self.collection(
                format_args!("{path}/stops"),
                stops.len(),
                super::canvas::MAX_GRADIENT_STOPS,
                "max_theme_gradient_stops",
            )?;
            for (index, stop) in stops.iter().enumerate() {
                self.string(format_args!("{path}/stops/{index}/color"), &stop.color)?;
            }
        }
        Ok(())
    }
}

struct BoundedCountingWriter<'a> {
    resources: &'a ThemeResourcePolicy,
    bytes: usize,
    resource_error: Option<ThemeResourceLimitExceeded>,
}

impl<'a> BoundedCountingWriter<'a> {
    const fn new(resources: &'a ThemeResourcePolicy) -> Self {
        Self {
            resources,
            bytes: 0,
            resource_error: None,
        }
    }

    fn take_resource_error(&mut self) -> Option<ThemeResourceLimitExceeded> {
        self.resource_error.take()
    }
}

impl io::Write for BoundedCountingWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.resource_error.is_some() {
            return Err(io::Error::other(
                "theme definition encoded-byte limit exceeded",
            ));
        }
        let projected = self.bytes.saturating_add(buffer.len());
        if let Err(error) = self.resources.check_theme_encoded_bytes(projected) {
            self.resource_error = Some(error);
            return Err(io::Error::other(
                "theme definition encoded-byte limit exceeded",
            ));
        }
        self.bytes = projected;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonContext {
    Other,
    Root,
    Tokens,
    TokenTypography,
    StyleTypography,
    Styles,
    StyleEntry,
    StylePatch,
    Stroke,
    Paint,
    Series,
    PaletteColors,
    TokenFontStack,
    StyleFontStack,
    TokenFontFamily,
    StyleFontFamily,
    GradientStops,
    Dasharray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StyleKind {
    Rule,
    OrdinalPalette,
}

#[derive(Debug, Clone, Default)]
struct JsonMarker {
    style_kind: Option<StyleKind>,
    replaces_generated_palette: Option<bool>,
    palette_target: Option<String>,
    authoring_schema_version: Option<u32>,
    expansion_version: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonCapture {
    None,
    StyleKind,
    PaletteTarget,
    AuthoringSchemaVersion,
    ExpansionVersion,
}

#[derive(Debug)]
enum PreflightFailure {
    InvalidJson {
        reason_id: &'static str,
        message: &'static str,
    },
    ResourceLimit {
        path: String,
        limit_id: &'static str,
        actual: usize,
        max: usize,
    },
    Materialization(ThemeMaterializationError),
}

struct PreflightState {
    path: String,
    path_frozen: bool,
    total_collection_items: usize,
    authored_rules: usize,
    materialized_palettes: usize,
    style_entries: usize,
    palette_targets: BTreeMap<String, usize>,
    failure: Option<PreflightFailure>,
}

impl PreflightState {
    const fn new() -> Self {
        Self {
            path: String::new(),
            path_frozen: false,
            total_collection_items: 0,
            authored_rules: 0,
            materialized_palettes: GENERATED_PALETTE_TARGETS.len(),
            style_entries: 0,
            palette_targets: BTreeMap::new(),
            failure: None,
        }
    }

    fn fail_invalid_json<E: de::Error>(
        &mut self,
        reason_id: &'static str,
        message: &'static str,
    ) -> E {
        if self.failure.is_none()
            || matches!(
                self.failure.as_ref(),
                Some(PreflightFailure::Materialization(_))
            )
        {
            self.failure = Some(PreflightFailure::InvalidJson { reason_id, message });
        }
        E::custom(PREFLIGHT_SENTINEL)
    }

    fn fail_resource<E: de::Error>(
        &mut self,
        limit_id: &'static str,
        actual: usize,
        max: usize,
    ) -> E {
        if self.failure.is_none()
            || matches!(
                self.failure.as_ref(),
                Some(PreflightFailure::Materialization(_))
            )
        {
            self.failure = Some(PreflightFailure::ResourceLimit {
                path: if limit_id == "max_theme_definition_collection_items" {
                    String::new()
                } else {
                    self.path.clone()
                },
                limit_id,
                actual,
                max,
            });
        }
        E::custom(PREFLIGHT_SENTINEL)
    }

    fn push_key(&mut self, key: &str) -> (usize, bool) {
        let previous = (self.path.len(), self.path_frozen);
        if self.path_frozen {
            return previous;
        }
        let escaped_len = key.len().saturating_add(
            key.bytes()
                .filter(|byte| matches!(*byte, b'~' | b'/'))
                .count(),
        );
        if self
            .path
            .len()
            .saturating_add(1)
            .saturating_add(escaped_len)
            > ThemeMaterializationDiagnosticV1::MAX_PATH_BYTES
        {
            // Preserve a complete JSON Pointer ancestor, never a partial UTF-8 segment or escape.
            self.path_frozen = true;
            return previous;
        }
        self.path.push('/');
        for character in key.chars() {
            match character {
                '~' => self.path.push_str("~0"),
                '/' => self.path.push_str("~1"),
                _ => self.path.push(character),
            }
        }
        previous
    }

    fn push_index(&mut self, index: usize) -> (usize, bool) {
        use std::fmt::Write;

        let previous = (self.path.len(), self.path_frozen);
        if !self.path_frozen {
            write!(&mut self.path, "/{index}").expect("writing to a String cannot fail");
            if self.path.len() > ThemeMaterializationDiagnosticV1::MAX_PATH_BYTES {
                self.path.truncate(previous.0);
                self.path_frozen = true;
            }
        }
        previous
    }

    fn restore_path(&mut self, previous: (usize, bool)) {
        self.path.truncate(previous.0);
        self.path_frozen = previous.1;
    }

    fn record_materialization(&mut self, error: ThemeMaterializationError) {
        let replaces_existing = matches!(
            (&self.failure, &error),
            (
                Some(PreflightFailure::Materialization(existing)),
                ThemeMaterializationError::RuleBudgetExceeded { .. }
            ) if !matches!(existing, ThemeMaterializationError::RuleBudgetExceeded { .. })
        );
        if self.failure.is_none() || replaces_existing {
            self.failure = Some(PreflightFailure::Materialization(error));
        }
    }

    fn check_depth<E: de::Error>(&mut self, depth: usize) -> Result<(), E> {
        if depth > MAX_JSON_DEPTH {
            return Err(self.fail_resource(
                "max_theme_definition_json_depth",
                depth,
                MAX_JSON_DEPTH,
            ));
        }
        Ok(())
    }

    fn check_object_members<E: de::Error>(&mut self, members: usize) -> Result<(), E> {
        if members > MAX_JSON_OBJECT_MEMBERS {
            return Err(self.fail_resource(
                "max_theme_definition_object_members",
                members,
                MAX_JSON_OBJECT_MEMBERS,
            ));
        }
        Ok(())
    }

    fn charge_collection_item<E: de::Error>(&mut self) -> Result<(), E> {
        self.total_collection_items = self.total_collection_items.saturating_add(1);
        if self.total_collection_items > MAX_TOTAL_COLLECTION_ITEMS {
            return Err(self.fail_resource(
                "max_theme_definition_collection_items",
                self.total_collection_items,
                MAX_TOTAL_COLLECTION_ITEMS,
            ));
        }
        Ok(())
    }

    fn check_string<E: de::Error>(&mut self, value: &str) -> Result<(), E> {
        if value.len() > MAX_JSON_STRING_BYTES {
            return Err(self.fail_resource(
                "max_theme_definition_string_bytes",
                value.len(),
                MAX_JSON_STRING_BYTES,
            ));
        }
        Ok(())
    }

    fn check_sequence_items<E: de::Error>(
        &mut self,
        context: JsonContext,
        items: usize,
    ) -> Result<(), E> {
        let (limit_id, max) = sequence_limit(context);
        if items <= max {
            return Ok(());
        }
        Err(self.fail_resource(limit_id, first_rejected_actual(max), max))
    }

    fn charge_style_entry(&mut self, marker: &JsonMarker) {
        let authored_index = self.style_entries;
        self.style_entries = self.style_entries.saturating_add(1);
        match marker.style_kind {
            Some(StyleKind::Rule) => {
                self.authored_rules = self.authored_rules.saturating_add(1);
                if self.authored_rules > MAX_AUTHORED_RULES {
                    self.record_materialization(ThemeMaterializationError::RuleBudgetExceeded {
                        actual: first_rejected_actual(MAX_AUTHORED_RULES),
                        max: MAX_AUTHORED_RULES,
                    });
                }
            }
            Some(StyleKind::OrdinalPalette) => {
                if let Some(target) = marker.palette_target.as_ref()
                    && let Some(first_authored_index) =
                        self.palette_targets.insert(target.clone(), authored_index)
                {
                    self.record_materialization(
                        ThemeMaterializationError::DuplicatePaletteTarget {
                            target: target.clone(),
                            first_authored_index,
                            duplicate_authored_index: authored_index,
                        },
                    );
                    return;
                }
                if marker.replaces_generated_palette != Some(false) {
                    return;
                }
                self.materialized_palettes = self.materialized_palettes.saturating_add(1);
                if self.materialized_palettes > super::semantic::MAX_THEME_ORDINAL_PALETTES {
                    self.record_materialization(
                        ThemeMaterializationError::OrdinalPaletteBudgetExceeded {
                            actual: first_rejected_actual(
                                super::semantic::MAX_THEME_ORDINAL_PALETTES,
                            ),
                            max: super::semantic::MAX_THEME_ORDINAL_PALETTES,
                        },
                    );
                }
            }
            _ => {}
        }
    }
}

fn preflight_json(bytes: &[u8]) -> Result<(), ThemeMaterializationErrorV1> {
    let mut state = PreflightState::new();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = JsonValueSeed {
        state: &mut state,
        parent_depth: 0,
        context: JsonContext::Root,
        capture: JsonCapture::None,
    }
    .deserialize(&mut deserializer);
    let marker = match result {
        Ok(marker) => marker,
        Err(_) => {
            return Err(match state.failure {
                Some(PreflightFailure::InvalidJson { reason_id, message }) => {
                    invalid_json_contract_error(reason_id, message)
                }
                Some(PreflightFailure::ResourceLimit {
                    path,
                    limit_id,
                    actual,
                    max,
                }) => contract_error(ThemeMaterializationDiagnosticV1::resource_limit_exceeded(
                    path,
                    limit_id,
                    actual,
                    max,
                    "theme definition JSON exceeds a bounded structural limit",
                )),
                Some(PreflightFailure::Materialization(_)) | None => invalid_json_contract_error(
                    "malformed-json",
                    "theme definition JSON is malformed",
                ),
            });
        }
    };
    deserializer.end().map_err(|_| {
        invalid_json_contract_error("malformed-json", "theme definition JSON is malformed")
    })?;
    if let (Some(authoring_schema_version), Some(expansion_version)) =
        (marker.authoring_schema_version, marker.expansion_version)
        && resolve_authoring_version(authoring_schema_version, expansion_version).is_err()
    {
        return Err(contract_error(
            ThemeMaterializationDiagnosticV1::unsupported_version_tuple(
                authoring_schema_version,
                expansion_version,
                "theme authoring version tuple is not supported",
            ),
        ));
    }
    if let Some(PreflightFailure::Materialization(error)) = state.failure {
        return Err(error.into_contract_error());
    }
    Ok(())
}

const fn sequence_limit(context: JsonContext) -> (&'static str, usize) {
    match context {
        JsonContext::Series | JsonContext::PaletteColors => (
            "max_theme_palette_colors",
            super::semantic::MAX_THEME_PALETTE_COLORS,
        ),
        JsonContext::TokenFontStack | JsonContext::StyleFontStack => (
            "max_theme_font_stack_entries",
            super::typography::MAX_FONT_STACK_ENTRIES,
        ),
        JsonContext::GradientStops => (
            "max_theme_gradient_stops",
            super::canvas::MAX_GRADIENT_STOPS,
        ),
        _ => ("max_theme_definition_array_items", MAX_JSON_ARRAY_ITEMS),
    }
}

struct JsonValueSeed<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
    context: JsonContext,
    capture: JsonCapture,
}

impl<'de> DeserializeSeed<'de> for JsonValueSeed<'_> {
    type Value = JsonMarker;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(JsonValueVisitor {
            state: self.state,
            parent_depth: self.parent_depth,
            context: self.context,
            capture: self.capture,
        })
    }
}

struct JsonValueVisitor<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
    context: JsonContext,
    capture: JsonCapture,
}

impl JsonValueVisitor<'_> {
    fn visit_string_value<E: de::Error>(self, value: &str) -> Result<JsonMarker, E> {
        self.state.check_string(value)?;
        if value.len() > super::typography::MAX_FONT_FAMILY_BYTES
            && matches!(
                self.context,
                JsonContext::TokenFontFamily | JsonContext::StyleFontFamily
            )
        {
            return Err(self.state.fail_resource(
                "max_theme_font_family_bytes",
                value.len(),
                super::typography::MAX_FONT_FAMILY_BYTES,
            ));
        }
        let mut marker = JsonMarker::default();
        match self.capture {
            JsonCapture::StyleKind => {
                marker.style_kind = match value {
                    "rule" => Some(StyleKind::Rule),
                    "ordinal-palette" => Some(StyleKind::OrdinalPalette),
                    _ => None,
                };
            }
            JsonCapture::PaletteTarget => {
                marker.palette_target = Some(value.to_owned());
                marker.replaces_generated_palette = Some(
                    GENERATED_PALETTE_TARGETS
                        .iter()
                        .any(|target| target.id() == value),
                );
            }
            JsonCapture::None
            | JsonCapture::AuthoringSchemaVersion
            | JsonCapture::ExpansionVersion => {}
        }
        Ok(marker)
    }

    fn visit_unsigned_value(self, value: u64) -> JsonMarker {
        let Ok(value) = u32::try_from(value) else {
            return JsonMarker::default();
        };
        let mut marker = JsonMarker::default();
        match self.capture {
            JsonCapture::AuthoringSchemaVersion => {
                marker.authoring_schema_version = Some(value);
            }
            JsonCapture::ExpansionVersion => {
                marker.expansion_version = Some(value);
            }
            JsonCapture::None | JsonCapture::StyleKind | JsonCapture::PaletteTarget => {}
        }
        marker
    }
}

impl<'de> Visitor<'de> for JsonValueVisitor<'_> {
    type Value = JsonMarker;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one bounded JSON value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(JsonMarker::default())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(u64::try_from(value)
            .map(|value| self.visit_unsigned_value(value))
            .unwrap_or_default())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(self.visit_unsigned_value(value))
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(JsonMarker::default())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(JsonMarker::default())
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string_value(value)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string_value(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_string_value(&value)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state.check_depth(depth)?;
        let mut item_count = 0usize;
        loop {
            let next_count = item_count.saturating_add(1);
            let item_context = match self.context {
                JsonContext::Styles => JsonContext::StyleEntry,
                JsonContext::TokenFontStack => JsonContext::TokenFontFamily,
                JsonContext::StyleFontStack => JsonContext::StyleFontFamily,
                _ => JsonContext::Other,
            };
            let Some(_) = sequence.next_element_seed(JsonSequenceElementSeed {
                state: &mut *self.state,
                parent_depth: depth,
                context: item_context,
                item_count: next_count,
                sequence_context: self.context,
            })?
            else {
                break;
            };
            self.state.charge_collection_item()?;
            item_count = next_count;
        }
        Ok(JsonMarker::default())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let depth = self.parent_depth.saturating_add(1);
        self.state.check_depth(depth)?;
        let mut member_count = 0usize;
        let mut seen = HashSet::<Cow<'de, str>>::new();
        let mut marker = JsonMarker::default();
        while let Some(key) = map.next_key_seed(JsonKeySeed {
            state: &mut *self.state,
        })? {
            member_count = member_count.saturating_add(1);
            self.state.check_object_members(member_count)?;
            if !seen.insert(key.clone()) {
                return Err(self.state.fail_invalid_json(
                    "duplicate-object-key",
                    "theme definition JSON contains a duplicate object key",
                ));
            }
            let child_context = child_context(self.context, &key);
            let capture = match (self.context, key.as_ref()) {
                (JsonContext::StyleEntry, "kind") => JsonCapture::StyleKind,
                (JsonContext::StyleEntry, "target") => JsonCapture::PaletteTarget,
                (JsonContext::Root, "authoring_schema_version") => {
                    JsonCapture::AuthoringSchemaVersion
                }
                (JsonContext::Root, "expansion_version") => JsonCapture::ExpansionVersion,
                _ => JsonCapture::None,
            };
            let previous_path = self.state.push_key(&key);
            let captured = map.next_value_seed(JsonValueSeed {
                state: &mut *self.state,
                parent_depth: depth,
                context: child_context,
                capture,
            });
            self.state.restore_path(previous_path);
            let captured = captured?;
            marker.style_kind = marker.style_kind.or(captured.style_kind);
            marker.replaces_generated_palette = marker
                .replaces_generated_palette
                .or(captured.replaces_generated_palette);
            marker.palette_target = marker.palette_target.or(captured.palette_target);
            marker.authoring_schema_version = marker
                .authoring_schema_version
                .or(captured.authoring_schema_version);
            marker.expansion_version = marker.expansion_version.or(captured.expansion_version);
        }
        if self.context == JsonContext::StyleEntry {
            self.state.charge_style_entry(&marker);
        }
        Ok(marker)
    }
}

fn child_context(parent: JsonContext, key: &str) -> JsonContext {
    match (parent, key) {
        (JsonContext::Root, "tokens") => JsonContext::Tokens,
        (JsonContext::Root, "styles") => JsonContext::Styles,
        (JsonContext::Tokens, "typography") => JsonContext::TokenTypography,
        (JsonContext::StylePatch, "typography") => JsonContext::StyleTypography,
        (JsonContext::Tokens, "series") => JsonContext::Series,
        (JsonContext::TokenTypography, "font_stack") => JsonContext::TokenFontStack,
        (JsonContext::StyleTypography, "font_stack") => JsonContext::StyleFontStack,
        (JsonContext::StyleEntry, "style") => JsonContext::StylePatch,
        (JsonContext::StyleEntry, "colors") => JsonContext::PaletteColors,
        (JsonContext::StylePatch, "stroke") => JsonContext::Stroke,
        (JsonContext::StylePatch, "fill") | (JsonContext::Stroke, "paint") => JsonContext::Paint,
        (JsonContext::Stroke, "dasharray") => JsonContext::Dasharray,
        (JsonContext::Paint, "stops") | (JsonContext::Other, "stops") => JsonContext::GradientStops,
        _ => JsonContext::Other,
    }
}

struct JsonSequenceElementSeed<'a> {
    state: &'a mut PreflightState,
    parent_depth: usize,
    context: JsonContext,
    item_count: usize,
    sequence_context: JsonContext,
}

impl<'de> DeserializeSeed<'de> for JsonSequenceElementSeed<'_> {
    type Value = JsonMarker;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        self.state
            .check_sequence_items(self.sequence_context, self.item_count)?;
        let previous_path = self.state.push_index(self.item_count - 1);
        let result = JsonValueSeed {
            state: &mut *self.state,
            parent_depth: self.parent_depth,
            context: self.context,
            capture: JsonCapture::None,
        }
        .deserialize(deserializer);
        self.state.restore_path(previous_path);
        result
    }
}

struct JsonKeySeed<'a> {
    state: &'a mut PreflightState,
}

impl<'de> DeserializeSeed<'de> for JsonKeySeed<'_> {
    type Value = Cow<'de, str>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(JsonKeyVisitor { state: self.state })
    }
}

struct JsonKeyVisitor<'a> {
    state: &'a mut PreflightState,
}

impl<'de> Visitor<'de> for JsonKeyVisitor<'_> {
    type Value = Cow<'de, str>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one bounded JSON object key")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(value)?;
        Ok(Cow::Borrowed(value))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(value)?;
        Ok(Cow::Owned(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.state.check_string(&value)?;
        Ok(Cow::Owned(value))
    }
}

#[cfg(test)]
mod tests {
    use merman_theme_contract::{ThemeGradientStopWireV1, ThemeTokensV1};

    use super::*;

    const DENSE_RULE_COUNT: usize = 328;

    #[test]
    fn json_and_typed_admission_count_only_dynamic_collection_items() {
        // The arrays contain 21,320 items. Counting their surrounding object members as collection
        // items would incorrectly push this valid definition beyond the shared 65,536-item budget.
        let stops = (0..super::super::canvas::MAX_GRADIENT_STOPS)
            .map(|index| ThemeGradientStopWireV1 {
                offset: index as f32 / (super::super::canvas::MAX_GRADIENT_STOPS - 1) as f32,
                color: "#123456".to_owned(),
            })
            .collect();
        let rule = ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: None,
            style: ThemeStylePatchWireV1 {
                fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Structured(
                    ThemeCanvasPaintObjectWireV1::LinearGradient {
                        angle_degrees: 0.0,
                        stops,
                        repetition: None,
                    },
                )),
                ..ThemeStylePatchWireV1::default()
            },
        };
        let definition = ThemeDefinitionV1::new(ThemeTokensV1::default())
            .with_styles(vec![rule; DENSE_RULE_COUNT]);
        let json = serde_json::to_vec(&definition).expect("the typed wire should serialize");

        preflight_json(&json)
            .expect("JSON object members must not consume the dynamic-collection budget");
        admit_typed_definition(&ThemeResourcePolicy::interactive(), &definition)
            .expect("typed and JSON inputs must share collection-item accounting");
    }
}
