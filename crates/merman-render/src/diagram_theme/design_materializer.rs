//! Typed, bounded authoring resolution. Rendering continues to consume concrete specifications.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use merman_theme_contract::*;

use super::{ThemeColorValue, ThemeResourcePolicy};

type Result<T> = std::result::Result<T, ThemeMaterializationErrorV1>;
const MAX_TOKENS: usize = 256;
const MAX_REFERENCE_DEPTH: usize = 32;
const MAX_REFERENCE_USES: usize = 4096;

/// A failure while resolving authored values or compiling the concrete result.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ThemeDesignCompileError {
    /// The editable document failed admission or typed resolution.
    #[error("theme design materialization failed: {0}")]
    Materialization(#[from] ThemeMaterializationErrorV1),
    /// The materialized concrete specification failed compiler admission.
    #[error("theme design compilation failed: {0}")]
    Compilation(#[from] super::ThemeCompileError),
}

/// Materializes an editable design using the interactive resource policy.
pub fn materialize_theme_design(design: &ThemeDesignV1) -> Result<DiagramThemeSpecWireV1> {
    materialize_theme_design_with_resource_policy(design, &ThemeResourcePolicy::interactive())
}

/// Materializes an editable design under caller-owned resource admission.
pub fn materialize_theme_design_with_resource_policy(
    design: &ThemeDesignV1,
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1> {
    materialize(design, resources, false).map(|(spec, _)| spec)
}

/// Admits raw design JSON before normalization can discard duplicate fields.
pub fn check_design_json_input(resources: &ThemeResourcePolicy, bytes: &[u8]) -> Result<()> {
    resources
        .check_theme_encoded_bytes(bytes.len())
        .map_err(|error| {
            encoding_error(
                super::definition_admission::ThemeInputEncodingError::ResourceLimit(error),
            )
        })?;
    super::definition_admission::preflight_json(bytes)
}

/// Decodes and materializes the current editable design under caller-owned admission.
pub fn materialize_theme_design_json_with_resource_policy(
    bytes: &[u8],
    resources: &ThemeResourcePolicy,
) -> Result<DiagramThemeSpecWireV1> {
    check_design_json_input(resources, bytes)?;
    let design = serde_json::from_slice(bytes).map_err(invalid_design_json)?;
    materialize_theme_design_with_resource_policy(&design, resources)
}

/// Materializes and inspects raw authored JSON in one resolver traversal for design tools.
pub fn materialize_theme_design_json_with_bindings(
    bytes: &[u8],
    resources: &ThemeResourcePolicy,
) -> Result<MaterializedThemeWireV1> {
    check_design_json_input(resources, bytes)?;
    let design = serde_json::from_slice(bytes).map_err(invalid_design_json)?;
    let (spec, references) = materialize(&design, resources, true)?;
    MaterializedThemeWireV1::try_new(spec)
        .map(|value| value.with_references(references))
        .map_err(|error| failure("", "finite-theme-design", error.to_string()))
}

fn invalid_design_json(_: serde_json::Error) -> ThemeMaterializationErrorV1 {
    ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::invalid_definition_json(
            "contract-shape",
            "theme design does not match the closed authoring contract",
        ),
    )
}

fn encoding_error(
    error: super::definition_admission::ThemeInputEncodingError,
) -> ThemeMaterializationErrorV1 {
    match error {
        super::definition_admission::ThemeInputEncodingError::ResourceLimit(error) => {
            super::definition_admission::encoded_bytes_contract_error(error)
        }
        super::definition_admission::ThemeInputEncodingError::InvalidEncoding => failure(
            "",
            "finite-theme-design",
            "theme design cannot be serialized as finite JSON",
        ),
    }
}

/// Inspects authored references through the same typed resolver used for materialization.
pub fn inspect_theme_design(design: &ThemeDesignV1) -> Result<ThemeDesignInspection> {
    materialize(design, &ThemeResourcePolicy::interactive(), true)
        .map(|(_, references)| ThemeDesignInspection { references })
}

fn failure(path: &str, domain: &str, message: impl Into<String>) -> ThemeMaterializationErrorV1 {
    fn bounded(value: &str, max: usize) -> &str {
        let mut end = value.len().min(max);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        &value[..end]
    }
    let message = format!("{} at {path}", message.into());
    // Keep the complete pointer or its nearest complete ancestor; never split an escape.
    let mut pointer = path;
    while pointer.len() > ThemeMaterializationDiagnosticV1::MAX_PATH_BYTES {
        pointer = pointer.rsplit_once('/').map_or("", |(parent, _)| parent);
    }
    ThemeMaterializationErrorV1::from_diagnostic(
        ThemeMaterializationDiagnosticV1::invalid_token_value(
            pointer,
            domain,
            bounded(&message, 512),
        ),
    )
}

fn child(path: &str, key: impl std::fmt::Display) -> String {
    format!(
        "{path}/{}",
        key.to_string().replace('~', "~0").replace('/', "~1")
    )
}

fn domain(value_type: &'static str) -> ThemeDesignValueDomain {
    ThemeDesignValueDomain {
        value_type: value_type.to_owned(),
        ..Default::default()
    }
}

fn materialize(
    design: &ThemeDesignV1,
    resources: &ThemeResourcePolicy,
    inspect: bool,
) -> Result<(DiagramThemeSpecWireV1, Vec<ThemeDesignReferenceUse>)> {
    super::definition_admission::check_typed_input_encoded_bytes(resources, design)
        .map_err(encoding_error)?;
    if design.tokens.len() > MAX_TOKENS {
        return Err(failure(
            "/tokens",
            "bounded-design-tokens",
            "too many reusable theme values",
        ));
    }
    for (path, id) in design
        .tokens
        .keys()
        .map(|id| ("/tokens", id))
        .chain(design.styles.keys().map(|id| ("/styles", id)))
        .chain(design.palettes.keys().map(|id| ("/palettes", id)))
    {
        if id.is_empty() || id.len() > 128 {
            return Err(failure(
                path,
                "bounded-theme-identifier",
                "theme identifiers must contain 1 to 128 bytes",
            ));
        }
    }
    if design.styles.len() + design.rules.len() + design.palettes.len()
        > super::semantic::MAX_THEME_RULES
    {
        return Err(failure(
            "/rules",
            "bounded-design-rules",
            "too many authored theme rules",
        ));
    }
    let mut resolver = Resolver {
        design,
        resources,
        cache_bytes: 0,
        cache: BTreeMap::new(),
        visiting: BTreeSet::new(),
        effects: Vec::new(),
        references: Vec::new(),
        reference_uses: 0,
        inspect,
    };
    for id in design.tokens.keys() {
        resolver.token(id, &child("/tokens", id))?;
    }
    let mut styles = Vec::new();
    let mut output_bytes = 0;
    for (target, style) in &design.styles {
        let entry = ThemeRuleSetWireV1::Rule {
            target: target.clone(),
            family: None,
            variant: None,
            ordinal: None,
            style: resolver.style(style, &child("/styles", target))?,
        };
        charge_encoded(resources, &mut output_bytes, &entry)?;
        styles.push(entry);
    }
    for (index, rule) in design.rules.iter().enumerate() {
        let entry = ThemeRuleSetWireV1::Rule {
            target: rule.target.clone(),
            family: rule.family.clone(),
            variant: rule.variant.clone(),
            ordinal: rule.ordinal.clone(),
            style: resolver.style(&rule.style, &format!("/rules/{index}/style"))?,
        };
        charge_encoded(resources, &mut output_bytes, &entry)?;
        styles.push(entry);
    }
    for (target, palette) in &design.palettes {
        let colors = resolver.palette(palette, &child("/palettes", target))?;
        let entry = ThemeRuleSetWireV1::OrdinalPalette {
            target: target.clone(),
            colors,
        };
        charge_encoded(resources, &mut output_bytes, &entry)?;
        styles.push(entry);
    }
    let canvas = design
        .canvas
        .as_ref()
        .map(|canvas| resolver.canvas(canvas, "/canvas"))
        .transpose()?;
    let typography = design
        .typography
        .as_ref()
        .map(|typography| resolver.typography(typography))
        .transpose()?;
    let used_effects = styles
        .iter()
        .filter_map(|entry| match entry {
            ThemeRuleSetWireV1::Rule {
                style:
                    ThemeStylePatchWireV1 {
                        effect: SpecifiedWireV1::Value(id),
                        ..
                    },
                ..
            } => Some(id.as_str()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    resolver.effects.retain(|entry| matches!(entry, ThemeEffectEntryWireV1::Graph { id, .. } if used_effects.contains(id.as_str())));
    if resolver.effects.len() > super::resources::MAX_EFFECT_GRAPHS_HARD_CAP {
        return Err(failure(
            "/tokens",
            "bounded-effects",
            "too many consumed theme effects",
        ));
    }
    let spec = DiagramThemeSpecWireV1 {
        mermaid: design.mermaid.clone(),
        typography,
        styles: (!styles.is_empty()).then_some(styles),
        canvas,
        effects: (!resolver.effects.is_empty()).then_some(resolver.effects),
        assets: design.assets.clone(),
        requirements: design.requirements.clone(),
    };
    super::definition_admission::check_typed_input_encoded_bytes(resources, &spec)
        .map_err(encoding_error)?;
    if inspect {
        propagate_alias_domains(design, &mut resolver.references);
    }
    Ok((spec, resolver.references))
}

// Charge expanded values as they are retained, before repeated consumers can accumulate a
// large concrete document. The final exact complete-spec admission remains authoritative too.
fn charge_encoded(
    resources: &ThemeResourcePolicy,
    total: &mut usize,
    value: &impl serde::Serialize,
) -> Result<()> {
    let bytes = super::definition_admission::measure_typed_input_encoded_bytes(resources, value)
        .map_err(encoding_error)?;
    let next = total.saturating_add(bytes);
    resources
        .check_theme_encoded_bytes(next)
        .map_err(super::definition_admission::encoded_bytes_contract_error)?;
    *total = next;
    Ok(())
}

fn token_alias(value: &ThemeDesignTokenValueV1) -> Option<(&'static str, &ThemeReferenceV1)> {
    use ThemeDesignTokenValueV1 as T;
    match value {
        T::Color(ThemeValueV1::Ref(value)) => Some(("color", value)),
        T::Number(ThemeValueV1::Ref(value)) => Some(("number", value)),
        T::String(ThemeValueV1::Ref(value)) => Some(("string", value)),
        T::Length(ThemeValueV1::Ref(value)) => Some(("length", value)),
        T::FontStack(ThemeValueV1::Ref(value)) => Some(("font-stack", value)),
        T::Palette(ThemeValueV1::Ref(value)) => Some(("palette", value)),
        T::Paint(ThemeValueV1::Ref(value)) => Some(("paint", value)),
        T::Stroke(ThemeValueV1::Ref(value)) => Some(("stroke", value)),
        T::TextStyle(ThemeValueV1::Ref(value)) => Some(("text-style", value)),
        T::Effect(ThemeValueV1::Ref(value)) => Some(("effect", value)),
        T::LineHeight(ThemeValueV1::Ref(value)) => Some(("line-height", value)),
        T::Insets(ThemeValueV1::Ref(value)) => Some(("insets", value)),
        _ => None,
    }
}

fn propagate_alias_domains(design: &ThemeDesignV1, references: &mut [ThemeDesignReferenceUse]) {
    let indices = references
        .iter()
        .enumerate()
        .map(|(index, entry)| (entry.path.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for consumer in references.to_vec() {
        let mut id = consumer.token.as_str();
        // Cached resolution can admit chains deeper than the active recursion limit.
        // Missing values and cycles are already rejected; token count bounds this traversal.
        for _ in 0..design.tokens.len() {
            let Some(token) = design.tokens.get(id) else {
                break;
            };
            let Some((_, reference)) = token_alias(&token.value) else {
                break;
            };
            if let Some(index) = indices.get(&format!("{}/value", child("/tokens", id))) {
                let domain = &mut references[*index].domain;
                let source = &consumer.domain;
                if let Some(min) = source.min {
                    domain.min = Some(domain.min.map_or(min, |value| value.max(min)));
                }
                if let Some(max) = source.max {
                    domain.max = Some(domain.max.map_or(max, |value| value.min(max)));
                }
                for (target, incoming) in [
                    (&mut domain.choices, &source.choices),
                    (&mut domain.units, &source.units),
                ] {
                    if !incoming.is_empty() {
                        if target.is_empty() {
                            target.clone_from(incoming);
                        } else {
                            target.retain(|value| incoming.contains(value));
                        }
                    }
                }
            }
            id = &reference.r#ref;
        }
    }
}

#[derive(serde::Serialize)]
#[serde(untagged)]
enum Value {
    Color(String),
    Number(f32),
    String(String),
    Length(ThemeLengthWireV1),
    FontStack(Vec<String>),
    Palette(Vec<String>),
    Paint(ThemeCanvasPaintWireV1),
    Stroke(ThemeStrokePatchWireV1),
    TextStyle(ThemeTextStylePatchWireV1),
    Effect(String),
    LineHeight(ThemeLineHeightWireV1),
    Insets(ThemeInsetsWireV1),
}

impl Value {
    fn kind(&self) -> &'static str {
        match self {
            Self::Color(_) => "color",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Length(_) => "length",
            Self::FontStack(_) => "font-stack",
            Self::Palette(_) => "palette",
            Self::Paint(_) => "paint",
            Self::Stroke(_) => "stroke",
            Self::TextStyle(_) => "text-style",
            Self::Effect(_) => "effect",
            Self::LineHeight(_) => "line-height",
            Self::Insets(_) => "insets",
        }
    }
}

struct Resolver<'a> {
    design: &'a ThemeDesignV1,
    resources: &'a ThemeResourcePolicy,
    cache_bytes: usize,
    cache: BTreeMap<String, Arc<Value>>,
    visiting: BTreeSet<String>,
    effects: Vec<ThemeEffectEntryWireV1>,
    references: Vec<ThemeDesignReferenceUse>,
    reference_uses: usize,
    inspect: bool,
}

macro_rules! scalar_resolver {
    ($name:ident, $input:ty, $output:ty, $variant:ident, $kind:literal, $lower:ident) => {
        fn $name(&mut self, value: &ThemeValueV1<$input>, path: &str) -> Result<$output> {
            match value {
                ThemeValueV1::Literal(value) => self.$lower(value, path),
                ThemeValueV1::Ref(reference) => {
                    self.reference(reference, path, domain($kind))?;
                    match self.token(&reference.r#ref, path)?.as_ref() {
                        Value::$variant(value) => Ok(value.clone()),
                        _ => Err(failure(
                            path,
                            $kind,
                            "reference has an incompatible declared type",
                        )),
                    }
                }
            }
        }
    };
}

impl Resolver<'_> {
    fn reference(
        &mut self,
        reference: &ThemeReferenceV1,
        path: &str,
        domain: ThemeDesignValueDomain,
    ) -> Result<()> {
        if reference.r#ref.is_empty() || reference.r#ref.len() > 128 {
            return Err(failure(
                path,
                "bounded-theme-identifier",
                "reference identifiers must contain 1 to 128 bytes",
            ));
        }
        self.reference_uses += 1;
        if self.reference_uses > MAX_REFERENCE_USES {
            return Err(failure(
                path,
                "bounded-design-references",
                "too many theme reference uses",
            ));
        }
        if self.inspect {
            self.references.push(ThemeDesignReferenceUse {
                token: reference.r#ref.clone(),
                path: path.to_owned(),
                domain,
            });
        }
        Ok(())
    }

    fn token(&mut self, id: &str, path: &str) -> Result<Arc<Value>> {
        if let Some(value) = self.cache.get(id) {
            return Ok(value.clone());
        }
        if self.visiting.len() >= MAX_REFERENCE_DEPTH {
            return Err(failure(
                path,
                "bounded-reference-depth",
                "theme reference chain is too deep",
            ));
        }
        if !self.visiting.insert(id.to_owned()) {
            return Err(failure(
                path,
                "acyclic-theme-reference",
                "theme reference cycle",
            ));
        }
        let document = self.design;
        let token = document.tokens.get(id).ok_or_else(|| {
            failure(
                path,
                "existing-theme-reference",
                format!("missing theme value `{id}`"),
            )
        })?;
        let location = format!("{}/value", child("/tokens", id));
        use ThemeDesignTokenValueV1 as T;
        if let Some((kind, reference)) = token_alias(&token.value) {
            self.reference(reference, &location, domain(kind))?;
            let value = self.token(&reference.r#ref, &location)?;
            if value.kind() != kind {
                return Err(failure(
                    &location,
                    kind,
                    "reference has an incompatible declared type",
                ));
            }
            self.visiting.remove(id);
            self.cache.insert(id.to_owned(), value.clone());
            return Ok(value);
        }
        let value = match &token.value {
            T::Color(value) => Value::Color(self.color(value, &location)?),
            T::Number(value) => Value::Number(self.number(value, &location)?),
            T::String(value) => Value::String(self.string(value, &location)?),
            T::Length(value) => Value::Length(self.length(value, &location)?),
            T::FontStack(value) => Value::FontStack(self.font_stack(value, &location)?),
            T::Palette(value) => Value::Palette(self.palette(value, &location)?),
            T::Paint(value) => Value::Paint(self.paint(value, &location)?),
            T::Stroke(value) => Value::Stroke(self.stroke(value, &location)?),
            T::TextStyle(value) => Value::TextStyle(self.text_style(value, &location)?),
            T::Effect(value) => Value::Effect(self.effect(value, &location)?),
            T::LineHeight(value) => Value::LineHeight(self.line_height(value, &location)?),
            T::Insets(value) => Value::Insets(self.insets(value, &location)?),
        };
        self.visiting.remove(id);
        charge_encoded(self.resources, &mut self.cache_bytes, &value)?;
        let value = Arc::new(value);
        self.cache.insert(id.to_owned(), value.clone());
        Ok(value)
    }

    scalar_resolver!(color, String, String, Color, "color", color_literal);
    scalar_resolver!(number, f32, f32, Number, "number", number_literal);
    scalar_resolver!(string, String, String, String, "string", string_literal);
    scalar_resolver!(
        length,
        ThemeLengthWireV1,
        ThemeLengthWireV1,
        Length,
        "length",
        length_literal
    );
    scalar_resolver!(
        font_stack,
        Vec<String>,
        Vec<String>,
        FontStack,
        "font-stack",
        font_stack_literal
    );
    scalar_resolver!(
        palette,
        Vec<ThemeValueV1<String>>,
        Vec<String>,
        Palette,
        "palette",
        palette_literal
    );
    scalar_resolver!(
        paint,
        ThemeDesignPaintV1,
        ThemeCanvasPaintWireV1,
        Paint,
        "paint",
        paint_literal
    );
    scalar_resolver!(
        stroke,
        ThemeDesignStrokeV1,
        ThemeStrokePatchWireV1,
        Stroke,
        "stroke",
        stroke_literal
    );
    scalar_resolver!(
        text_style,
        ThemeDesignTextStyleV1,
        ThemeTextStylePatchWireV1,
        TextStyle,
        "text-style",
        text_style_literal
    );
    scalar_resolver!(
        effect,
        ThemeDesignEffectV1,
        String,
        Effect,
        "effect",
        effect_literal
    );
    scalar_resolver!(
        line_height,
        ThemeLineHeightWireV1,
        ThemeLineHeightWireV1,
        LineHeight,
        "line-height",
        line_height_literal
    );
    scalar_resolver!(
        insets,
        ThemeInsetsWireV1,
        ThemeInsetsWireV1,
        Insets,
        "insets",
        insets_literal
    );

    fn color_literal(&mut self, value: &String, path: &str) -> Result<String> {
        if value.len() > 4096 {
            return Err(failure(path, "bounded-color", "color value is too long"));
        }
        ThemeColorValue::parse(value).map_err(|error| failure(path, "color", error.to_string()))?;
        Ok(value.clone())
    }
    fn number_literal(&mut self, value: &f32, path: &str) -> Result<f32> {
        if !value.is_finite() {
            return Err(failure(path, "finite-number", "number must be finite"));
        }
        Ok(*value)
    }
    fn string_literal(&mut self, value: &String, _: &str) -> Result<String> {
        Ok(value.clone())
    }
    fn length_literal(
        &mut self,
        value: &ThemeLengthWireV1,
        path: &str,
    ) -> Result<ThemeLengthWireV1> {
        let number = match value {
            ThemeLengthWireV1::PxValue(value)
            | ThemeLengthWireV1::Px { px: value }
            | ThemeLengthWireV1::Percent { percent: value } => value,
        };
        self.number_literal(number, path)?;
        Ok(value.clone())
    }
    fn font_stack_literal(&mut self, value: &Vec<String>, path: &str) -> Result<Vec<String>> {
        super::FontStack::new(value.iter().cloned())
            .map_err(|error| failure(path, "font-stack", error.to_string()))?;
        Ok(value.clone())
    }
    fn palette_literal(
        &mut self,
        value: &Vec<ThemeValueV1<String>>,
        path: &str,
    ) -> Result<Vec<String>> {
        if value.is_empty() || value.len() > 256 {
            return Err(failure(
                path,
                "bounded-palette",
                "palette must contain 1 to 256 colors",
            ));
        }
        value
            .iter()
            .enumerate()
            .map(|(index, color)| self.color(color, &child(path, index)))
            .collect()
    }
    fn line_height_literal(
        &mut self,
        value: &ThemeLineHeightWireV1,
        path: &str,
    ) -> Result<ThemeLineHeightWireV1> {
        let height = match value {
            ThemeLineHeightWireV1::Keyword(value) if value == "normal" => super::LineHeight::Normal,
            ThemeLineHeightWireV1::Keyword(_) => {
                return Err(failure(
                    path,
                    "line-height",
                    "expected normal, a positive multiplier, or positive pixels",
                ));
            }
            ThemeLineHeightWireV1::Multiplier(value) => super::LineHeight::Multiplier(*value),
            ThemeLineHeightWireV1::Px { px } => super::LineHeight::Px(*px),
        };
        height
            .validate()
            .map_err(|error| failure(path, "line-height", error.to_string()))?;
        Ok(value.clone())
    }
    fn insets_literal(
        &mut self,
        value: &ThemeInsetsWireV1,
        path: &str,
    ) -> Result<ThemeInsetsWireV1> {
        let insets = match value {
            ThemeInsetsWireV1::All(value) => super::InsetsPx::all(*value),
            ThemeInsetsWireV1::Sides {
                top,
                right,
                bottom,
                left,
            } => super::InsetsPx {
                top: *top,
                right: *right,
                bottom: *bottom,
                left: *left,
            },
        };
        insets
            .validate("insets")
            .map_err(|error| failure(path, "insets", error.to_string()))?;
        Ok(value.clone())
    }

    fn bounded_number(
        &mut self,
        value: &ThemeValueV1<f32>,
        path: &str,
        min: Option<f32>,
        max: Option<f32>,
    ) -> Result<f32> {
        let number = self.number(value, path)?;
        if min.is_some_and(|min| number < min) || max.is_some_and(|max| number > max) {
            return Err(failure(
                path,
                "bounded-number",
                "number is outside the consumer's bounds",
            ));
        }
        self.constrain_last(path, |domain| {
            domain.min = min;
            domain.max = max;
        });
        Ok(number)
    }
    fn pixels(
        &mut self,
        value: &ThemeValueV1<ThemeLengthWireV1>,
        path: &str,
        min: Option<f32>,
    ) -> Result<f32> {
        let length = self.length(value, path)?;
        let number = match length {
            ThemeLengthWireV1::PxValue(value) | ThemeLengthWireV1::Px { px: value } => value,
            ThemeLengthWireV1::Percent { .. } => {
                return Err(failure(
                    path,
                    "pixel-length",
                    "this property requires pixels",
                ));
            }
        };
        if min.is_some_and(|min| number < min) {
            return Err(failure(
                path,
                "bounded-pixel-length",
                "pixel length is below the consumer's minimum",
            ));
        }
        self.constrain_last(path, |domain| {
            domain.units = vec!["px".to_owned()];
            domain.min = min;
        });
        Ok(number)
    }
    fn choice(
        &mut self,
        value: &ThemeValueV1<String>,
        path: &str,
        choices: &'static [&'static str],
    ) -> Result<String> {
        let value = self.string(value, path)?;
        if !choices.contains(&value.as_str()) {
            return Err(failure(
                path,
                "enum-choice",
                format!("expected one of: {}", choices.join(", ")),
            ));
        }
        self.constrain_last(path, |domain| {
            domain.choices = choices.iter().map(|value| (*value).to_owned()).collect()
        });
        Ok(value)
    }
    fn constrain_last(&mut self, path: &str, change: impl FnOnce(&mut ThemeDesignValueDomain)) {
        if let Some(reference) = self
            .references
            .iter_mut()
            .rev()
            .find(|reference| reference.path == path)
        {
            change(&mut reference.domain);
        }
    }

    fn style(&mut self, value: &ThemeDesignStyleV1, path: &str) -> Result<ThemeStylePatchWireV1> {
        Ok(ThemeStylePatchWireV1 {
            fill: self.patch(&value.fill, &child(path, "fill"), Self::paint)?,
            opacity: self.patch(&value.opacity, &child(path, "opacity"), Self::opacity)?,
            fill_opacity: self.patch(
                &value.fill_opacity,
                &child(path, "fill_opacity"),
                Self::opacity,
            )?,
            stroke: value
                .stroke
                .as_ref()
                .map(|value| self.stroke(value, &child(path, "stroke")))
                .transpose()?,
            radius: self.patch(
                &value.radius,
                &child(path, "radius"),
                Self::nonnegative_pixels,
            )?,
            padding: self.patch(&value.padding, &child(path, "padding"), Self::insets)?,
            typography: value
                .typography
                .as_ref()
                .map(|value| self.text_style(value, &child(path, "typography")))
                .transpose()?,
            effect: self.patch(&value.effect, &child(path, "effect"), Self::effect)?,
        })
    }
    fn patch<T, U>(
        &mut self,
        value: &SpecifiedWireV1<T>,
        path: &str,
        lower: fn(&mut Self, &T, &str) -> Result<U>,
    ) -> Result<SpecifiedWireV1<U>> {
        Ok(match value {
            SpecifiedWireV1::Unspecified => SpecifiedWireV1::Unspecified,
            SpecifiedWireV1::Clear => SpecifiedWireV1::Clear,
            SpecifiedWireV1::Value(value) => SpecifiedWireV1::Value(lower(self, value, path)?),
        })
    }
    fn opacity(&mut self, value: &ThemeValueV1<f32>, path: &str) -> Result<f32> {
        self.bounded_number(value, path, Some(0.0), Some(1.0))
    }
    fn nonnegative_pixels(
        &mut self,
        value: &ThemeValueV1<ThemeLengthWireV1>,
        path: &str,
    ) -> Result<f32> {
        self.pixels(value, path, Some(0.0))
    }
    fn signed_pixels(
        &mut self,
        value: &ThemeValueV1<ThemeLengthWireV1>,
        path: &str,
    ) -> Result<f32> {
        self.pixels(value, path, None)
    }
    fn cap(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(value, path, &["butt", "round", "square"])
    }
    fn join(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(value, path, &["miter", "round", "bevel"])
    }
    fn stroke_literal(
        &mut self,
        value: &ThemeDesignStrokeV1,
        path: &str,
    ) -> Result<ThemeStrokePatchWireV1> {
        let dasharray = match &value.dasharray {
            SpecifiedWireV1::Value(values) => {
                if values.len() > 64 {
                    return Err(failure(path, "bounded-dashes", "too many dash lengths"));
                }
                SpecifiedWireV1::Value(
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| {
                            self.bounded_number(
                                value,
                                &format!("{path}/dasharray/{index}"),
                                Some(0.0),
                                None,
                            )
                        })
                        .collect::<Result<_>>()?,
                )
            }
            SpecifiedWireV1::Clear => SpecifiedWireV1::Clear,
            SpecifiedWireV1::Unspecified => SpecifiedWireV1::Unspecified,
        };
        Ok(ThemeStrokePatchWireV1 {
            paint: self.patch(&value.paint, &child(path, "paint"), Self::paint)?,
            width: self.patch(
                &value.width,
                &child(path, "width"),
                Self::nonnegative_pixels,
            )?,
            dasharray,
            linecap: self.patch(&value.linecap, &child(path, "linecap"), Self::cap)?,
            linejoin: self.patch(&value.linejoin, &child(path, "linejoin"), Self::join)?,
            opacity: self.patch(&value.opacity, &child(path, "opacity"), Self::opacity)?,
        })
    }

    fn text_style_literal(
        &mut self,
        value: &ThemeDesignTextStyleV1,
        path: &str,
    ) -> Result<ThemeTextStylePatchWireV1> {
        Ok(ThemeTextStylePatchWireV1 {
            font_stack: self.patch(
                &value.font_stack,
                &child(path, "font_stack"),
                Self::font_stack,
            )?,
            font_size_px: self.patch(
                &value.font_size_px,
                &child(path, "font_size_px"),
                Self::positive_pixels,
            )?,
            font_weight: self.patch(
                &value.font_weight,
                &child(path, "font_weight"),
                Self::font_weight,
            )?,
            font_style: self.patch(
                &value.font_style,
                &child(path, "font_style"),
                Self::font_style,
            )?,
            line_height: self.patch(
                &value.line_height,
                &child(path, "line_height"),
                Self::line_height,
            )?,
            letter_spacing_px: self.patch(
                &value.letter_spacing_px,
                &child(path, "letter_spacing_px"),
                Self::signed_pixels,
            )?,
            word_spacing_px: self.patch(
                &value.word_spacing_px,
                &child(path, "word_spacing_px"),
                Self::signed_pixels,
            )?,
            transform: self.patch(&value.transform, &child(path, "transform"), Self::transform)?,
            decoration: self.patch(
                &value.decoration,
                &child(path, "decoration"),
                Self::decoration,
            )?,
            text_align: self.patch(
                &value.text_align,
                &child(path, "text_align"),
                Self::text_align,
            )?,
            white_space: self.patch(
                &value.white_space,
                &child(path, "white_space"),
                Self::white_space,
            )?,
            wrap: self.patch(&value.wrap, &child(path, "wrap"), Self::wrap)?,
        })
    }
    fn positive_pixels(
        &mut self,
        value: &ThemeValueV1<ThemeLengthWireV1>,
        path: &str,
    ) -> Result<f32> {
        self.pixels(value, path, Some(f32::MIN_POSITIVE))
    }
    fn font_weight(&mut self, value: &ThemeValueV1<f32>, path: &str) -> Result<u16> {
        let value = self.bounded_number(value, path, Some(1.0), Some(1000.0))?;
        if value.fract() != 0.0 {
            return Err(failure(
                path,
                "integer-font-weight",
                "font weight must be an integer",
            ));
        }
        Ok(value as u16)
    }
    fn font_style(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(value, path, &["normal", "italic", "oblique"])
    }
    fn transform(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(
            value,
            path,
            &["none", "uppercase", "lowercase", "capitalize"],
        )
    }
    fn decoration(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(
            value,
            path,
            &["none", "underline", "overline", "line-through"],
        )
    }
    fn text_align(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(value, path, &["start", "center", "end"])
    }
    fn white_space(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(
            value,
            path,
            &["normal", "pre", "no-wrap", "pre-wrap", "pre-line"],
        )
    }
    fn wrap(&mut self, value: &ThemeValueV1<String>, path: &str) -> Result<String> {
        self.choice(value, path, &["normal", "break-word", "anywhere"])
    }

    fn typography(&mut self, value: &ThemeDesignTypographyV1) -> Result<ThemeTypographySpecWireV1> {
        let default = value
            .default
            .as_ref()
            .map(|value| {
                self.text_style(value, "/typography/default")
                    .map(complete_text_style)
            })
            .transpose()?;
        let families = value
            .families
            .iter()
            .map(|(family, value)| {
                self.text_style(value, &child("/typography/families", family))
                    .map(complete_text_style)
                    .map(|value| (family.clone(), value))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        Ok(ThemeTypographySpecWireV1 {
            default,
            families: (!families.is_empty()).then_some(families),
        })
    }

    fn paint_literal(
        &mut self,
        value: &ThemeDesignPaintV1,
        path: &str,
    ) -> Result<ThemeCanvasPaintWireV1> {
        use ThemeCanvasPaintObjectWireV1 as W;
        use ThemeDesignPaintObjectV1 as D;
        let paint = match value {
            ThemeDesignPaintV1::Color(color) => {
                return Ok(ThemeCanvasPaintWireV1::Color(
                    self.color_literal(color, path)?,
                ));
            }
            ThemeDesignPaintV1::Structured(paint) => paint,
        };
        Ok(ThemeCanvasPaintWireV1::Structured(match paint {
            D::Transparent => W::Transparent,
            D::Solid { color } => W::Solid {
                color: self.color(color, &child(path, "color"))?,
            },
            D::LinearGradient {
                angle_degrees,
                stops,
                repetition,
            } => W::LinearGradient {
                angle_degrees: self.number(angle_degrees, &child(path, "angle_degrees"))?,
                stops: self.stops(stops, &child(path, "stops"))?,
                repetition: repetition.clone(),
            },
            D::RadialGradient {
                center_x,
                center_y,
                radius,
                stops,
                repetition,
            } => W::RadialGradient {
                center_x: self.length(center_x, &child(path, "center_x"))?,
                center_y: self.length(center_y, &child(path, "center_y"))?,
                radius: self.length(radius, &child(path, "radius"))?,
                stops: self.stops(stops, &child(path, "stops"))?,
                repetition: repetition.clone(),
            },
            D::Pattern {
                pattern,
                cell_width,
                cell_height,
                foreground,
                background,
                angle_degrees,
            } => {
                if !["grid", "dots", "stripes"].contains(&pattern.as_str()) {
                    return Err(failure(path, "pattern-kind", "unknown pattern kind"));
                }
                W::Pattern {
                    pattern: pattern.clone(),
                    cell_width: self.positive_pixels(cell_width, &child(path, "cell_width"))?,
                    cell_height: self.positive_pixels(cell_height, &child(path, "cell_height"))?,
                    foreground: self.color(foreground, &child(path, "foreground"))?,
                    background: background
                        .as_ref()
                        .map(|value| self.color(value, &child(path, "background")))
                        .transpose()?,
                    angle_degrees: angle_degrees
                        .as_ref()
                        .map(|value| self.number(value, &child(path, "angle_degrees")))
                        .transpose()?,
                }
            }
        }))
    }
    fn stops(
        &mut self,
        values: &[ThemeDesignGradientStopV1],
        path: &str,
    ) -> Result<Vec<ThemeGradientStopWireV1>> {
        if !(2..=64).contains(&values.len()) {
            return Err(failure(
                path,
                "gradient-stops",
                "a gradient requires 2 to 64 stops",
            ));
        }
        let mut previous = 0.0;
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let path = child(path, index);
                let offset = self.bounded_number(
                    &value.offset,
                    &child(&path, "offset"),
                    Some(previous),
                    Some(1.0),
                )?;
                previous = offset;
                Ok(ThemeGradientStopWireV1 {
                    offset,
                    color: self.color_alpha(&value.color, value.opacity.as_ref(), &path)?,
                })
            })
            .collect()
    }
    fn color_alpha(
        &mut self,
        color: &ThemeValueV1<String>,
        opacity: Option<&ThemeValueV1<f32>>,
        path: &str,
    ) -> Result<String> {
        let color = self.color(color, &child(path, "color"))?;
        let Some(opacity) = opacity else {
            return Ok(color);
        };
        let opacity = self.opacity(opacity, &child(path, "opacity"))?;
        if opacity == 1.0 {
            return Ok(color);
        }
        use merman_core::theme_color::{ColorChannel, ThemeColor};
        let color =
            ThemeColor::parse(&color).map_err(|error| failure(path, "color", error.to_string()))?;
        let opacity = opacity
            .to_string()
            .parse::<f64>()
            .map_err(|_| failure(path, "finite-opacity", "invalid opacity"))?;
        Ok(format!(
            "rgba({}, {}, {}, {})",
            color.channel(ColorChannel::Red),
            color.channel(ColorChannel::Green),
            color.channel(ColorChannel::Blue),
            color.channel(ColorChannel::Alpha) * opacity
        ))
    }

    fn canvas(&mut self, value: &ThemeDesignCanvasV1, path: &str) -> Result<ThemeCanvasSpecWireV1> {
        if value.layers.len() > 32 {
            return Err(failure(
                path,
                "bounded-canvas-layers",
                "too many canvas layers",
            ));
        }
        let mut layer_bytes = 0;
        let layers = value
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let path = format!("{path}/layers/{index}");
                if layer.blend_mode.as_ref().is_some_and(|mode| {
                    ![
                        "normal",
                        "multiply",
                        "screen",
                        "overlay",
                        "darken",
                        "lighten",
                        "difference",
                    ]
                    .contains(&mode.as_str())
                }) {
                    return Err(failure(&path, "blend-mode", "unknown blend mode"));
                }
                let layer = ThemeCanvasLayerWireV1 {
                    paint: self.paint(&layer.paint, &child(&path, "paint"))?,
                    opacity: layer
                        .opacity
                        .as_ref()
                        .map(|value| self.opacity(value, &child(&path, "opacity")))
                        .transpose()?,
                    blend_mode: layer.blend_mode.clone(),
                    offset_x: layer
                        .offset_x
                        .as_ref()
                        .map(|value| self.signed_pixels(value, &child(&path, "offset_x")))
                        .transpose()?,
                    offset_y: layer
                        .offset_y
                        .as_ref()
                        .map(|value| self.signed_pixels(value, &child(&path, "offset_y")))
                        .transpose()?,
                };
                charge_encoded(self.resources, &mut layer_bytes, &layer)?;
                Ok(layer)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ThemeCanvasSpecWireV1 {
            base: value
                .base
                .as_ref()
                .map(|value| self.paint(value, &child(path, "base")))
                .transpose()?,
            layers: (!layers.is_empty()).then_some(layers),
            bleed: value
                .bleed
                .as_ref()
                .map(|value| self.insets(value, &child(path, "bleed")))
                .transpose()?,
        })
    }

    fn effect_literal(&mut self, value: &ThemeDesignEffectV1, path: &str) -> Result<String> {
        if self.effects.len() >= MAX_TOKENS + super::semantic::MAX_THEME_RULES {
            return Err(failure(
                path,
                "bounded-effects",
                "too many authored theme effects",
            ));
        }
        let id = format!("design-effect-{}", self.effects.len());
        let (composition, color_space, primitives) = match value {
            ThemeDesignEffectV1::Graph {
                color_space,
                primitives,
            } => (None, color_space.clone(), primitives.clone()),
            ThemeDesignEffectV1::ShadowStack {
                composition,
                color_space,
                shadows,
            } => {
                if shadows.is_empty() || shadows.len() > 256 {
                    return Err(failure(
                        path,
                        "bounded-shadows",
                        "a shadow stack requires 1 to 256 shadows",
                    ));
                }
                let independent = *composition == ThemeShadowCompositionV1::Independent;
                let primitives = shadows
                    .iter()
                    .enumerate()
                    .map(|(index, shadow)| {
                        let path = format!("{path}/shadows/{index}");
                        Ok(ThemeEffectPrimitiveWireV1::DropShadow {
                            input: Some(
                                if independent || index == 0 {
                                    "source-graphic"
                                } else {
                                    "previous"
                                }
                                .to_owned(),
                            ),
                            offset_x: self
                                .signed_pixels(&shadow.offset_x, &child(&path, "offset_x"))?,
                            offset_y: self
                                .signed_pixels(&shadow.offset_y, &child(&path, "offset_y"))?,
                            blur_radius: self.nonnegative_pixels(
                                &shadow.std_deviation,
                                &child(&path, "std_deviation"),
                            )?,
                            spread: 0.0,
                            color: self.color_alpha(
                                &shadow.color,
                                shadow.opacity.as_ref(),
                                &path,
                            )?,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                (
                    independent.then(|| "independent".to_owned()),
                    Some(color_space.clone().unwrap_or_else(|| "srgb".to_owned())),
                    primitives,
                )
            }
        };
        let effect = ThemeEffectEntryWireV1::Graph {
            id: id.clone(),
            composition,
            color_space,
            primitives,
        };
        charge_encoded(self.resources, &mut self.cache_bytes, &effect)?;
        self.effects.push(effect);
        Ok(id)
    }
}

fn complete_text_style(style: ThemeTextStylePatchWireV1) -> ThemeTextStyleWireV1 {
    fn value<T>(patch: SpecifiedWireV1<T>) -> Option<T> {
        match patch {
            SpecifiedWireV1::Value(value) => Some(value),
            _ => None,
        }
    }
    ThemeTextStyleWireV1 {
        font_stack: value(style.font_stack),
        font_size_px: value(style.font_size_px),
        font_weight: value(style.font_weight),
        font_style: value(style.font_style),
        line_height: value(style.line_height),
        letter_spacing_px: value(style.letter_spacing_px),
        word_spacing_px: value(style.word_spacing_px),
        transform: value(style.transform),
        decoration: value(style.decoration),
        text_align: value(style.text_align),
        white_space: value(style.white_space),
        wrap: value(style.wrap),
    }
}
