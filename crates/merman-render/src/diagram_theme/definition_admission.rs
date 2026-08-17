use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::io;

use merman_theme_contract::{
    SpecifiedWireV1, ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1, ThemeColorTokenV1,
    ThemeDefinitionV1, ThemeLineHeightWireV1, ThemeRuleSetWireV1, ThemeStylePatchWireV1,
    ThemeTextStylePatchWireV1,
};
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

use super::materializer::{
    GENERATED_PALETTE_TARGETS, MAX_AUTHORED_RULES, ThemeMaterializationError, ThemeMaterializer,
    validate_definition_shape,
};
use super::{
    DiagramTheme, DiagramThemeCompiler, ThemeCompileError, ThemeResourceLimitExceeded,
    ThemeResourcePolicy,
};

const MAX_JSON_DEPTH: usize = 32;
const MAX_JSON_OBJECT_MEMBERS: usize = 64;
const MAX_JSON_ARRAY_ITEMS: usize = 1_024;
const MAX_JSON_TOTAL_ENTRIES: usize = 65_536;
const MAX_JSON_STRING_BYTES: usize = 64 * 1_024;
const PREFLIGHT_SENTINEL: &str = "theme definition JSON preflight failed";

/// Materializes and compiles one admitted versioned theme definition.
pub fn compile_theme_definition(
    compiler: &DiagramThemeCompiler,
    definition: &ThemeDefinitionV1,
) -> Result<DiagramTheme, ThemeDefinitionCompileError> {
    admit_typed_definition(compiler.resource_policy(), definition)?;
    compile_admitted_definition(compiler, definition)
}

/// Decodes, admits, materializes, and compiles one versioned theme-definition JSON document.
pub fn compile_theme_definition_json(
    compiler: &DiagramThemeCompiler,
    bytes: &[u8],
) -> Result<DiagramTheme, ThemeDefinitionCompileError> {
    compiler.check_encoded_input_bytes(bytes.len())?;
    preflight_json(bytes)?;
    let definition = serde_json::from_slice::<ThemeDefinitionV1>(bytes).map_err(|error| {
        ThemeDefinitionAdmissionError::InvalidJson {
            message: format!("definition does not match ThemeDefinitionV1: {error}"),
        }
    })?;
    admit_typed_definition(compiler.resource_policy(), &definition)?;
    compile_admitted_definition(compiler, &definition)
}

fn compile_admitted_definition(
    compiler: &DiagramThemeCompiler,
    definition: &ThemeDefinitionV1,
) -> Result<DiagramTheme, ThemeDefinitionCompileError> {
    let materialized = ThemeMaterializer::new().materialize_theme(definition)?;
    Ok(compiler.compile_spec_wire(materialized.into_spec())?)
}

/// A fatal failure while admitting, materializing, or compiling a theme definition.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeDefinitionCompileError {
    /// The encoded or typed definition exceeded an input admission limit.
    #[error(transparent)]
    Admission(#[from] ThemeDefinitionAdmissionError),
    /// The authoring definition could not be expanded into a complete theme recipe.
    #[error("theme definition materialization failed: {0}")]
    Materialization(#[from] ThemeMaterializationError),
    /// The complete theme recipe did not satisfy compiler semantics or resource policy.
    #[error("theme definition compilation failed: {0}")]
    Compilation(#[from] ThemeCompileError),
}

impl From<ThemeResourceLimitExceeded> for ThemeDefinitionCompileError {
    fn from(error: ThemeResourceLimitExceeded) -> Self {
        Self::Admission(ThemeDefinitionAdmissionError::ResourceLimit(error))
    }
}

/// One definition-input failure detected before materialization allocates an expanded recipe.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeDefinitionAdmissionError {
    /// The JSON document was malformed, duplicated a member, or violated a structural ceiling.
    #[error("invalid theme definition JSON: {message}")]
    InvalidJson {
        /// A bounded display message. It is not a stable diagnostic identity.
        message: String,
    },
    /// The typed definition could not be serialized for resource accounting.
    #[error("theme definition could not be encoded for admission: {message}")]
    TypedEncoding {
        /// A bounded display message from the typed serializer.
        message: String,
    },
    /// One typed collection exceeded its versioned implementation ceiling.
    #[error("theme definition collection `{path}` has {actual} items; maximum is {max}")]
    CollectionLimit {
        /// Stable authoring path for the rejected collection class.
        path: &'static str,
        /// Observed item count.
        actual: usize,
        /// Maximum admitted item count.
        max: usize,
    },
    /// The caller-owned compiler resource policy rejected the input.
    #[error(transparent)]
    ResourceLimit(#[from] ThemeResourceLimitExceeded),
}

fn admit_typed_definition(
    resources: &ThemeResourcePolicy,
    definition: &ThemeDefinitionV1,
) -> Result<(), ThemeDefinitionCompileError> {
    validate_definition_shape(definition)?;
    let mut usage = TypedUsage::default();

    usage.charge_entries(definition.styles().len())?;
    for entry in definition.styles() {
        match entry {
            ThemeRuleSetWireV1::Rule {
                target,
                family,
                variant,
                style,
                ..
            } => {
                usage.string("/styles/rule/target", target)?;
                if let Some(family) = family {
                    usage.string("/styles/rule/family", family)?;
                }
                if let Some(variant) = variant {
                    usage.string("/styles/rule/variant", variant)?;
                }
                usage.style(style)?;
            }
            ThemeRuleSetWireV1::OrdinalPalette { target, colors } => {
                usage.string("/styles/ordinal-palette/target", target)?;
                usage.collection(
                    "/styles/ordinal-palette/colors",
                    colors.len(),
                    super::semantic::MAX_THEME_PALETTE_COLORS,
                )?;
                for color in colors {
                    usage.string("/styles/ordinal-palette/colors", color)?;
                }
            }
        }
    }

    for token in ThemeColorTokenV1::ALL {
        if let Some(color) = definition.tokens().color(token) {
            usage.string("/tokens", color)?;
        }
    }
    if let Some(series) = definition.tokens().series() {
        usage.collection(
            "/tokens/series",
            series.len(),
            super::semantic::MAX_THEME_PALETTE_COLORS,
        )?;
        for color in series {
            usage.string("/tokens/series", color)?;
        }
    }
    if let Some(typography) = definition.tokens().typography() {
        if let Some(font_stack) = typography.font_stack() {
            usage.collection(
                "/tokens/typography/font_stack",
                font_stack.len(),
                super::typography::MAX_FONT_STACK_ENTRIES,
            )?;
            for family in font_stack {
                usage.font_family("/tokens/typography/font_stack", family)?;
            }
        }
        if let Some(ThemeLineHeightWireV1::Keyword(value)) = typography.line_height() {
            usage.string("/tokens/typography/line_height", value)?;
        }
    }

    let mut writer = CountingWriter::default();
    serde_json::to_writer(&mut writer, definition).map_err(|error| {
        ThemeDefinitionAdmissionError::TypedEncoding {
            message: error.to_string(),
        }
    })?;
    resources.check_theme_encoded_bytes(writer.bytes)?;
    Ok(())
}

#[derive(Default)]
struct TypedUsage {
    total_entries: usize,
}

impl TypedUsage {
    fn charge_entries(&mut self, actual: usize) -> Result<(), ThemeDefinitionAdmissionError> {
        self.total_entries = self.total_entries.saturating_add(actual);
        if self.total_entries > MAX_JSON_TOTAL_ENTRIES {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path: "/",
                actual: self.total_entries,
                max: MAX_JSON_TOTAL_ENTRIES,
            });
        }
        Ok(())
    }

    fn string(&self, path: &'static str, value: &str) -> Result<(), ThemeDefinitionAdmissionError> {
        if value.len() > MAX_JSON_STRING_BYTES {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path,
                actual: value.len(),
                max: MAX_JSON_STRING_BYTES,
            });
        }
        Ok(())
    }

    fn font_family(
        &self,
        path: &'static str,
        value: &str,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        if value.len() > super::typography::MAX_FONT_FAMILY_BYTES {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit {
                path,
                actual: value.len(),
                max: super::typography::MAX_FONT_FAMILY_BYTES,
            });
        }
        self.string(path, value)
    }

    fn collection(
        &mut self,
        path: &'static str,
        actual: usize,
        field_max: usize,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        let max = field_max.min(MAX_JSON_ARRAY_ITEMS);
        if actual > max {
            return Err(ThemeDefinitionAdmissionError::CollectionLimit { path, actual, max });
        }
        self.charge_entries(actual)
    }

    fn style(
        &mut self,
        style: &ThemeStylePatchWireV1,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        if let Some(stroke) = &style.stroke {
            if let SpecifiedWireV1::Value(dasharray) = &stroke.dasharray {
                self.collection(
                    "/styles/rule/style/stroke/dasharray",
                    dasharray.len(),
                    MAX_JSON_ARRAY_ITEMS,
                )?;
            }
            if let SpecifiedWireV1::Value(paint) = &stroke.paint {
                self.paint(paint)?;
            }
            if let SpecifiedWireV1::Value(value) = &stroke.linecap {
                self.string("/styles/rule/style/stroke/linecap", value)?;
            }
            if let SpecifiedWireV1::Value(value) = &stroke.linejoin {
                self.string("/styles/rule/style/stroke/linejoin", value)?;
            }
        }
        if let SpecifiedWireV1::Value(paint) = &style.fill {
            self.paint(paint)?;
        }
        if let Some(typography) = &style.typography {
            self.typography(typography)?;
        }
        if let SpecifiedWireV1::Value(value) = &style.effect {
            self.string("/styles/rule/style/effect", value)?;
        }
        Ok(())
    }

    fn typography(
        &mut self,
        typography: &ThemeTextStylePatchWireV1,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        if let SpecifiedWireV1::Value(font_stack) = &typography.font_stack {
            self.collection(
                "/styles/rule/style/typography/font_stack",
                font_stack.len(),
                super::typography::MAX_FONT_STACK_ENTRIES,
            )?;
            for family in font_stack {
                self.font_family("/styles/rule/style/typography/font_stack", family)?;
            }
        }
        for (path, value) in [
            (
                "/styles/rule/style/typography/font_style",
                &typography.font_style,
            ),
            (
                "/styles/rule/style/typography/transform",
                &typography.transform,
            ),
            (
                "/styles/rule/style/typography/decoration",
                &typography.decoration,
            ),
            (
                "/styles/rule/style/typography/text_align",
                &typography.text_align,
            ),
            (
                "/styles/rule/style/typography/white_space",
                &typography.white_space,
            ),
            ("/styles/rule/style/typography/wrap", &typography.wrap),
        ] {
            if let SpecifiedWireV1::Value(value) = value {
                self.string(path, value)?;
            }
        }
        if let SpecifiedWireV1::Value(ThemeLineHeightWireV1::Keyword(value)) =
            &typography.line_height
        {
            self.string("/styles/rule/style/typography/line_height", value)?;
        }
        Ok(())
    }

    fn paint(
        &mut self,
        paint: &ThemeCanvasPaintWireV1,
    ) -> Result<(), ThemeDefinitionAdmissionError> {
        let ThemeCanvasPaintWireV1::Structured(paint) = paint else {
            return Ok(());
        };
        let stops = match paint {
            ThemeCanvasPaintObjectWireV1::Transparent => None,
            ThemeCanvasPaintObjectWireV1::Solid { color } => {
                self.string("/styles/rule/style/paint/color", color)?;
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
                self.string("/styles/rule/style/paint/pattern", pattern)?;
                self.string("/styles/rule/style/paint/foreground", foreground)?;
                if let Some(background) = background {
                    self.string("/styles/rule/style/paint/background", background)?;
                }
                None
            }
        };
        if let Some(stops) = stops {
            self.collection(
                "/styles/rule/style/paint/stops",
                stops.len(),
                super::canvas::MAX_GRADIENT_STOPS,
            )?;
            for stop in stops {
                self.string("/styles/rule/style/paint/stops/color", &stop.color)?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl io::Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.saturating_add(buffer.len());
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

#[derive(Debug, Clone, Copy, Default)]
struct JsonMarker {
    style_kind: Option<StyleKind>,
    replaces_generated_palette: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonCapture {
    None,
    StyleKind,
    PaletteTarget,
}

#[derive(Debug)]
enum PreflightFailure {
    Json(String),
    Materialization(ThemeMaterializationError),
}

struct PreflightState {
    total_entries: usize,
    authored_rules: usize,
    materialized_palettes: usize,
    failure: Option<PreflightFailure>,
}

impl PreflightState {
    const fn new() -> Self {
        Self {
            total_entries: 0,
            authored_rules: 0,
            materialized_palettes: GENERATED_PALETTE_TARGETS.len(),
            failure: None,
        }
    }

    fn fail_json<E: de::Error>(&mut self, message: impl Into<String>) -> E {
        if self.failure.is_none() {
            self.failure = Some(PreflightFailure::Json(message.into()));
        }
        E::custom(PREFLIGHT_SENTINEL)
    }

    fn fail_materialization<E: de::Error>(&mut self, error: ThemeMaterializationError) -> E {
        if self.failure.is_none() {
            self.failure = Some(PreflightFailure::Materialization(error));
        }
        E::custom(PREFLIGHT_SENTINEL)
    }

    fn check_depth<E: de::Error>(&mut self, depth: usize) -> Result<(), E> {
        if depth > MAX_JSON_DEPTH {
            return Err(self.fail_json("nesting depth limit exceeded"));
        }
        Ok(())
    }

    fn check_object_members<E: de::Error>(&mut self, members: usize) -> Result<(), E> {
        if members > MAX_JSON_OBJECT_MEMBERS {
            return Err(self.fail_json("object member limit exceeded"));
        }
        Ok(())
    }

    fn charge_entry<E: de::Error>(&mut self) -> Result<(), E> {
        self.total_entries = self.total_entries.saturating_add(1);
        if self.total_entries > MAX_JSON_TOTAL_ENTRIES {
            return Err(self.fail_json("total collection entry limit exceeded"));
        }
        Ok(())
    }

    fn check_string<E: de::Error>(&mut self, value: &str) -> Result<(), E> {
        if value.len() > MAX_JSON_STRING_BYTES {
            return Err(self.fail_json("decoded string byte limit exceeded"));
        }
        Ok(())
    }

    fn check_sequence_items<E: de::Error>(
        &mut self,
        context: JsonContext,
        items: usize,
    ) -> Result<(), E> {
        let max = match context {
            JsonContext::Series | JsonContext::PaletteColors => {
                super::semantic::MAX_THEME_PALETTE_COLORS
            }
            JsonContext::TokenFontStack | JsonContext::StyleFontStack => {
                super::typography::MAX_FONT_STACK_ENTRIES
            }
            JsonContext::GradientStops => super::canvas::MAX_GRADIENT_STOPS,
            _ => MAX_JSON_ARRAY_ITEMS,
        };
        if items <= max {
            return Ok(());
        }
        match context {
            JsonContext::Series => Err(self.fail_materialization(
                ThemeMaterializationError::InvalidTokenValue {
                    path: "/tokens/series",
                },
            )),
            JsonContext::TokenFontStack => Err(self.fail_materialization(
                ThemeMaterializationError::InvalidTokenValue {
                    path: "/tokens/typography/font_stack",
                },
            )),
            JsonContext::StyleFontStack => Err(self.fail_materialization(
                ThemeMaterializationError::InvalidTokenValue {
                    path: "/styles/rule/style/typography/font_stack",
                },
            )),
            JsonContext::PaletteColors => {
                Err(self.fail_json("ordinal palette color limit exceeded"))
            }
            JsonContext::GradientStops => Err(self.fail_json("gradient stop limit exceeded")),
            _ => Err(self.fail_json("array item limit exceeded")),
        }
    }

    fn charge_style_entry<E: de::Error>(&mut self, marker: JsonMarker) -> Result<(), E> {
        match marker.style_kind {
            Some(StyleKind::Rule) => {
                self.authored_rules = self.authored_rules.saturating_add(1);
                if self.authored_rules > MAX_AUTHORED_RULES {
                    return Err(self.fail_materialization(
                        ThemeMaterializationError::RuleBudgetExceeded {
                            actual: self.authored_rules,
                            max: MAX_AUTHORED_RULES,
                        },
                    ));
                }
            }
            Some(StyleKind::OrdinalPalette) if marker.replaces_generated_palette == Some(false) => {
                self.materialized_palettes = self.materialized_palettes.saturating_add(1);
                if self.materialized_palettes > super::semantic::MAX_THEME_ORDINAL_PALETTES {
                    return Err(self.fail_materialization(
                        ThemeMaterializationError::OrdinalPaletteBudgetExceeded {
                            actual: self.materialized_palettes,
                            max: super::semantic::MAX_THEME_ORDINAL_PALETTES,
                        },
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn preflight_json(bytes: &[u8]) -> Result<(), ThemeDefinitionCompileError> {
    let mut state = PreflightState::new();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = JsonValueSeed {
        state: &mut state,
        parent_depth: 0,
        context: JsonContext::Root,
        capture: JsonCapture::None,
    }
    .deserialize(&mut deserializer);
    if let Err(error) = result {
        return Err(match state.failure {
            Some(PreflightFailure::Json(message)) => {
                ThemeDefinitionAdmissionError::InvalidJson { message }.into()
            }
            Some(PreflightFailure::Materialization(error)) => error.into(),
            None => ThemeDefinitionAdmissionError::InvalidJson {
                message: format!("invalid JSON: {error}"),
            }
            .into(),
        });
    }
    deserializer.end().map_err(|error| {
        ThemeDefinitionAdmissionError::InvalidJson {
            message: format!("invalid JSON: {error}"),
        }
        .into()
    })
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
        let font_family_path = match self.context {
            JsonContext::TokenFontFamily => Some("/tokens/typography/font_stack"),
            JsonContext::StyleFontFamily => Some("/styles/rule/style/typography/font_stack"),
            _ => None,
        };
        if value.len() > super::typography::MAX_FONT_FAMILY_BYTES
            && let Some(path) = font_family_path
        {
            return Err(self
                .state
                .fail_materialization(ThemeMaterializationError::InvalidTokenValue { path }));
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
                marker.replaces_generated_palette = Some(
                    GENERATED_PALETTE_TARGETS
                        .iter()
                        .any(|target| *target == value),
                );
            }
            JsonCapture::None => {}
        }
        Ok(marker)
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

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(JsonMarker::default())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(JsonMarker::default())
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
            self.state.charge_entry()?;
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
            self.state.charge_entry()?;
            if !seen.insert(key.clone()) {
                return Err(self.state.fail_json("duplicate object key"));
            }
            let child_context = child_context(self.context, &key);
            let capture = match (self.context, key.as_ref()) {
                (JsonContext::StyleEntry, "kind") => JsonCapture::StyleKind,
                (JsonContext::StyleEntry, "target") => JsonCapture::PaletteTarget,
                _ => JsonCapture::None,
            };
            let captured = map.next_value_seed(JsonValueSeed {
                state: &mut *self.state,
                parent_depth: depth,
                context: child_context,
                capture,
            })?;
            marker.style_kind = marker.style_kind.or(captured.style_kind);
            marker.replaces_generated_palette = marker
                .replaces_generated_palette
                .or(captured.replaces_generated_palette);
        }
        if self.context == JsonContext::StyleEntry {
            self.state.charge_style_entry(marker)?;
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
        JsonValueSeed {
            state: self.state,
            parent_depth: self.parent_depth,
            context: self.context,
            capture: JsonCapture::None,
        }
        .deserialize(deserializer)
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
