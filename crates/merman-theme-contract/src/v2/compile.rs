use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[cfg(feature = "presets")]
use super::BuiltinPreset;
use super::contrast::{
    BLACK, DARK_INK, TEXT_CONTRAST, automatic_fill_ink_over, backdrop, best_ink, is_opaque,
};
use super::document::{
    CanvasPatch, DataPatch, ElevationEffect, ElevationPreset, ElevationSpec, FamilyOverride,
    FillInk, SeedPatch, ThemeDocumentFields,
};
use super::{
    Appearance, Canvas, Color, DataPalette, Elevation, FamilyTheme, Form, Paint, ResolvedThemeV2,
    Roles, ShadowLayer, ThemeDocumentV2, ThemeNotice, Typography, json, validate,
};

const DEFAULT_MAX_BYTES: usize = 256 * 1024;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const SERIES_CONTRAST: f64 = 3.0;
const DERIVED_SERIES_CONTRAST: f64 = 3.5;

/// An invalid theme document, unavailable appearance, or failed contrast check.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub struct ThemeError {
    message: String,
    path: Option<String>,
    line: Option<usize>,
    column: Option<usize>,
}

impl ThemeError {
    /// The location in the theme, when known.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// The one-based line of a JSON decoding error.
    pub const fn line(&self) -> Option<usize> {
        self.line
    }

    /// The one-based column of a JSON decoding error.
    pub const fn column(&self) -> Option<usize> {
        self.column
    }

    pub(super) fn invalid(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            path: Some(path.into()),
            line: None,
            column: None,
        }
    }

    pub(super) fn json(path: String, error: serde_json::Error) -> Self {
        Self {
            message: error.to_string(),
            path: Some(path),
            line: Some(error.line()).filter(|line| *line > 0),
            column: Some(error.column()).filter(|column| *column > 0),
        }
    }
}

/// Compiles standalone or preset-derived v2 documents into immutable visual settings.
#[derive(Clone, Debug)]
pub struct ThemeCompiler {
    max_bytes: usize,
    strict_contrast: bool,
    auto_fix_contrast: bool,
}

impl Default for ThemeCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeCompiler {
    /// Creates a compiler with a 256 KiB JSON limit and advisory contrast checks.
    pub const fn new() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
            strict_contrast: false,
            auto_fix_contrast: false,
        }
    }

    /// Changes the untrusted JSON byte limit, up to 4 MiB.
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes.min(MAX_BYTES);
        self
    }

    /// Byte limit for the JSON document supplied to this compiler.
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Rejects a custom theme when one of the checked contrast pairs falls below its target.
    pub const fn with_strict_contrast(mut self, strict: bool) -> Self {
        self.strict_contrast = strict;
        self
    }

    /// Allows the compiler to adjust foreground lightness before checking contrast.
    pub const fn with_auto_fix_contrast(mut self, auto_fix: bool) -> Self {
        self.auto_fix_contrast = auto_fix;
        self
    }

    /// Decodes a bounded JSON document and rejects duplicate keys on the input stream.
    pub fn parse_json(&self, bytes: &[u8]) -> Result<ThemeDocumentV2, ThemeError> {
        if bytes.len() > self.max_bytes {
            return Err(ThemeError::invalid(
                ".",
                format!("theme JSON exceeds the {} byte limit", self.max_bytes),
            ));
        }
        json::parse(bytes)
    }

    /// Decodes and compiles a theme, selecting the requested appearance or its default.
    pub fn compile_json(
        &self,
        bytes: &[u8],
        appearance: Option<Appearance>,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        self.compile(&self.parse_json(bytes)?, appearance)
    }

    /// Compiles a typed theme document.
    pub fn compile(
        &self,
        document: &ThemeDocumentV2,
        appearance: Option<Appearance>,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        self.resolve(
            document,
            appearance,
            self.strict_contrast,
            self.auto_fix_contrast,
        )
    }

    /// Compiles an embedded preset using its original colors and strict contrast checks.
    #[cfg(feature = "presets")]
    pub fn compile_preset(
        &self,
        preset: BuiltinPreset,
        appearance: Option<Appearance>,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        let appearance = appearance.unwrap_or_else(|| preset.default_appearance());
        if !preset.supported_appearances().contains(&appearance) {
            return Err(unavailable_appearance("appearance", appearance));
        }
        preset.resolved(appearance, || {
            self.resolve(preset.document()?, Some(appearance), true, false)
        })
    }

    fn resolve(
        &self,
        document: &ThemeDocumentV2,
        requested: Option<Appearance>,
        strict: bool,
        auto_fix: bool,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        let document = &document.fields;
        validate::document(document)?;
        let inherited = document.extends.as_deref().map(load_base).transpose()?;
        let documents: Vec<_> = inherited.into_iter().chain([document]).collect();
        let appearance = select_appearance(&documents, requested)?;
        let mut form = Form::default();
        let mut typography = Typography::default();
        let mut elevation = Elevation::Flat;
        let mut seed = SeedPatch::default();
        let mut data = DataPatch::default();
        let mut canvas = Canvas::default();

        for document in &documents {
            if let Some(patch) = &document.form {
                form.apply(patch);
                if let Some(spec) = &patch.elevation {
                    elevation = resolve_elevation(spec, appearance);
                }
            }
            if let Some(patch) = &document.typography {
                typography.apply(patch);
            }
            if let Some(layer) = document.layer(appearance) {
                if let Some(patch) = &layer.seed {
                    seed.merge(patch);
                }
                if let Some(patch) = &layer.data {
                    data.merge(patch);
                }
                if let Some(CanvasPatch {
                    textures,
                    padding_px,
                }) = &layer.canvas
                {
                    if let Some(textures) = textures {
                        canvas.textures.clone_from(textures);
                    }
                    if let Some(padding) = padding_px {
                        canvas.padding_px = *padding;
                    }
                }
                if let Some(spec) = &layer.elevation {
                    elevation = resolve_elevation(spec, appearance);
                }
            }
            if let Some(patch) = &document.seed {
                seed.merge(patch);
            }
        }

        let path = format!("appearances.{appearance}.seed");
        let (Some(canvas_color), Some(ink), Some(accent)) = (seed.canvas, seed.ink, seed.accent)
        else {
            return Err(ThemeError::invalid(
                path,
                "appearance requires seed canvas, ink, and accent",
            ));
        };
        let mut roles = derive_roles(&seed, canvas_color, ink, accent, appearance);
        let mut authored_title = false;
        for document in &documents {
            let layer_roles = document
                .layer(appearance)
                .and_then(|layer| layer.roles.as_ref());
            for roles_patch in layer_roles.into_iter().chain(document.roles.as_ref()) {
                roles.apply(roles_patch);
                authored_title |= roles_patch.text_title.is_some();
            }
        }

        let palette = derive_palette(&data, accent, &roles.canvas, appearance);
        let mut base = FamilyTheme {
            roles,
            data: palette,
            form,
            typography,
            elevation,
            node_ordinal_palette: false,
        };
        let mut family_overrides = BTreeMap::<String, FamilyTheme>::new();
        let mut family_authored_titles = BTreeSet::new();
        for patch in documents.iter().flat_map(|document| &document.overrides) {
            if patch.appearance.is_none_or(|mode| mode == appearance) {
                for family in &patch.families {
                    if patch
                        .roles
                        .as_ref()
                        .is_some_and(|roles| roles.text_title.is_some())
                    {
                        family_authored_titles.insert(family.as_str());
                    }
                    let family = family_overrides
                        .entry(family.clone())
                        .or_insert_with(|| base.clone());
                    apply_family_override(family, patch, appearance);
                }
            }
        }

        let mut notices = Vec::new();
        let mut shared_inputs = Vec::new();
        let base_fill_ink = Arc::clone(&base.data.fill_ink);
        let title_accent = (!authored_title).then_some(accent);
        ContrastChecks {
            appearance,
            auto_fix,
            notices: &mut notices,
            scope: NoticeScope::Shared(&mut shared_inputs),
        }
        .check_family(&mut base, 1.0, "data.fill", title_accent);
        for (id, family) in &mut family_overrides {
            let mut contrast = ContrastChecks {
                appearance,
                auto_fix,
                notices: &mut notices,
                scope: NoticeScope::Family(id, &shared_inputs),
            };
            let title_accent =
                title_accent.filter(|_| !family_authored_titles.contains(id.as_str()));
            if family.roles.canvas != base.roles.canvas {
                family.data = derive_palette(&data, accent, &family.roles.canvas, appearance);
            }
            if id == "pie" {
                let opacity = family.form.chart.pie_opacity;
                set_automatic_fill_ink(&mut family.data, &family.roles.canvas, opacity, appearance);
                contrast.check_family(family, opacity, "pie.data.fill", title_accent);
            } else {
                contrast.check_family(family, 1.0, "data.fill", title_accent);
            }
        }
        if base.form.chart.pie_opacity != 1.0 && !family_overrides.contains_key("pie") {
            let mut pie = base.clone();
            let opacity = pie.form.chart.pie_opacity;
            pie.data.fill_ink = base_fill_ink;
            set_automatic_fill_ink(&mut pie.data, &pie.roles.canvas, opacity, appearance);
            ContrastChecks {
                appearance,
                auto_fix,
                notices: &mut notices,
                scope: NoticeScope::Family("pie", &shared_inputs),
            }
            .check_fill_ink(&mut pie, opacity, "pie.data.fill");
            if pie.data.fill_ink != base.data.fill_ink {
                family_overrides.insert("pie".to_owned(), pie);
            }
        }
        if strict
            && let Some(ThemeNotice::LowContrast {
                foreground,
                background,
                ratio,
                minimum,
            }) = notices
                .iter()
                .find(|notice| matches!(notice, ThemeNotice::LowContrast { .. }))
        {
            return Err(ThemeError::invalid(
                foreground,
                format!(
                    "{foreground} on {background} has {ratio:.2}:1 contrast, below {minimum}:1"
                ),
            ));
        }

        Ok(Arc::new(ResolvedThemeV2 {
            id: document.id.clone(),
            appearance,
            base,
            canvas,
            family_overrides,
            notices,
        }))
    }
}

#[cfg(feature = "presets")]
fn load_base(id: &str) -> Result<&'static ThemeDocumentFields, ThemeError> {
    let preset = BuiltinPreset::from_id(id)
        .ok_or_else(|| ThemeError::invalid("extends", format!("unknown built-in preset `{id}`")))?;
    let document = &preset.document()?.fields;
    if document.extends.is_some() {
        return Err(ThemeError::invalid(
            "extends",
            "preset extension is limited to one level",
        ));
    }
    validate::document(document)?;
    Ok(document)
}

#[cfg(not(feature = "presets"))]
fn load_base(id: &str) -> Result<&'static ThemeDocumentFields, ThemeError> {
    Err(ThemeError::invalid(
        "extends",
        format!("built-in preset `{id}` requires the `presets` feature"),
    ))
}

fn select_appearance(
    documents: &[&ThemeDocumentFields],
    requested: Option<Appearance>,
) -> Result<Appearance, ThemeError> {
    let available = |appearance| documents.iter().any(|doc| doc.layer(appearance).is_some());
    let default = documents
        .iter()
        .rev()
        .find_map(|doc| doc.default_appearance);
    if let Some(appearance) = default
        && !available(appearance)
    {
        return Err(unavailable_appearance("default_appearance", appearance));
    }
    if let Some(appearance) = requested.or(default) {
        return available(appearance)
            .then_some(appearance)
            .ok_or_else(|| unavailable_appearance("appearance", appearance));
    }
    match (available(Appearance::Light), available(Appearance::Dark)) {
        (true, false) => Ok(Appearance::Light),
        (false, true) => Ok(Appearance::Dark),
        (true, true) => Err(ThemeError::invalid(
            "default_appearance",
            "select an appearance or declare a default",
        )),
        (false, false) => Err(ThemeError::invalid(
            "appearances",
            "declare at least one appearance",
        )),
    }
}

fn unavailable_appearance(path: &str, appearance: Appearance) -> ThemeError {
    ThemeError::invalid(
        path,
        format!("theme does not have a {appearance} appearance"),
    )
}

fn derive_roles(
    seed: &SeedPatch,
    canvas: Color,
    ink: Color,
    accent: Color,
    mode: Appearance,
) -> Roles {
    let dark = mode == Appearance::Dark;
    let [l, c, h] = canvas.oklch();
    let surface = seed.surface.unwrap_or_else(|| {
        let l = if dark {
            l + 0.055
        } else {
            (l + 0.025).min(0.995)
        };
        Color::from_oklch(l, c * 0.85, h)
    });
    let container = Color::from_oklch(l + if dark { 0.030 } else { -0.032 }, c * 1.05, h);
    let callout = seed
        .callout
        .unwrap_or_else(|| Color::from_oklch(if dark { 0.30 } else { 0.94 }, 0.045, 82.0));
    let emphasis = surface.mix_oklab(accent, if dark { 0.22 } else { 0.12 });
    let (dark_ink, light_ink) = if dark { (canvas, ink) } else { (ink, surface) };

    Roles {
        canvas: canvas.into(),
        surface_node: surface.into(),
        surface_container: container.into(),
        surface_callout: callout.into(),
        surface_emphasis: emphasis.into(),
        stroke_border: surface
            .mix_oklab(ink, if dark { 0.28 } else { 0.24 })
            .into(),
        stroke_connector: canvas.mix_oklab(ink, if dark { 0.62 } else { 0.56 }).into(),
        stroke_guide: canvas.mix_oklab(ink, if dark { 0.16 } else { 0.14 }).into(),
        text_primary: ink,
        text_muted: canvas.mix_oklab(ink, if dark { 0.72 } else { 0.68 }),
        text_title: ink,
        text_callout: best_ink(callout, dark_ink, light_ink),
        text_on_emphasis: best_ink(emphasis, dark_ink, light_ink),
        status_info: seed.info.unwrap_or(accent),
        status_success: seed.success.unwrap_or(if dark {
            Color::rgb(74, 222, 128)
        } else {
            Color::rgb(21, 128, 61)
        }),
        status_warn: seed.warn.unwrap_or(if dark {
            Color::rgb(251, 191, 36)
        } else {
            Color::rgb(180, 83, 9)
        }),
        status_danger: seed.danger.unwrap_or(if dark {
            Color::rgb(248, 113, 113)
        } else {
            Color::rgb(185, 28, 28)
        }),
    }
}

fn derive_palette(
    data: &DataPatch,
    accent: Color,
    canvas: &Paint,
    mode: Appearance,
) -> DataPalette {
    let dark = mode == Appearance::Dark;
    let h = accent.oklch()[2];
    let hues = || (0..10).map(|index| h + f64::from(index) * 137.508);
    let ink = data.ink.clone().unwrap_or_else(|| {
        let (l, c) = if dark { (0.74, 0.13) } else { (0.48, 0.14) };
        hues()
            .map(|h| {
                let ink = Color::from_oklch(l, c, h);
                fit_contrast(ink, |color| {
                    Paint::Solid(color).minimum_contrast_over(canvas, 1.0, None, mode)
                        >= DERIVED_SERIES_CONTRAST
                })
                .unwrap_or(ink)
            })
            .collect()
    });
    let fill = data.fill.clone().unwrap_or_else(|| {
        hues()
            .map(|h| Color::from_oklch(if dark { 0.34 } else { 0.84 }, 0.085, h))
            .collect()
    });
    let authored_fill_ink = match data.fill_ink {
        Some(FillInk::Color(ink)) => Some(ink),
        Some(FillInk::Auto(_)) | None => None,
    };
    let fill_ink = fill
        .iter()
        .map(|color| {
            authored_fill_ink
                .unwrap_or_else(|| automatic_fill_ink_over(*color, canvas, 1.0, backdrop(mode)))
        })
        .collect();
    DataPalette {
        ink: ink.into(),
        fill: fill.into(),
        fill_ink,
        pattern_encoding: data.pattern_encoding.unwrap_or(false),
        automatic_fill_ink: authored_fill_ink.is_none(),
    }
}

fn set_automatic_fill_ink(data: &mut DataPalette, canvas: &Paint, opacity: f64, mode: Appearance) {
    if data.automatic_fill_ink {
        data.fill_ink = data
            .fill
            .iter()
            .map(|&color| automatic_fill_ink_over(color, canvas, opacity, backdrop(mode)))
            .collect();
    }
}

fn fit_contrast(color: Color, passes: impl Fn(Color) -> bool) -> Option<Color> {
    if passes(color) || color.channels()[3] != 255 {
        return None;
    }
    let [l, c, h] = color.oklch();
    [0.0, 1.0]
        .into_iter()
        .filter_map(|endpoint| {
            let mut good = endpoint;
            let mut bad = l;
            if !passes(Color::from_oklch(good, c, h)) {
                return None;
            }
            for _ in 0..12 {
                let middle = (good + bad) * 0.5;
                if passes(Color::from_oklch(middle, c, h)) {
                    good = middle;
                } else {
                    bad = middle;
                }
            }
            Some(((good - l).abs(), Color::from_oklch(good, c, h)))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, color)| color)
}

struct ContrastChecks<'a> {
    appearance: Appearance,
    auto_fix: bool,
    notices: &'a mut Vec<ThemeNotice>,
    scope: NoticeScope<'a>,
}

enum NoticeScope<'a> {
    Shared(&'a mut Vec<ContrastNoticeInput>),
    Family(&'a str, &'a [ContrastNoticeInput]),
}

type ContrastBackground<'a> = (&'static str, &'a Paint, Option<&'a Paint>);

struct ContrastNoticeInput {
    color: Color,
    opacity: f64,
    paints: Vec<(Paint, Option<Paint>)>,
}

fn same_notice(shared: &ThemeNotice, family: &ThemeNotice) -> bool {
    match (shared, family) {
        (
            ThemeNotice::LowContrast {
                foreground: shared_name,
                background: "data.fill",
                ratio: shared_ratio,
                minimum: shared_minimum,
            },
            ThemeNotice::LowContrast {
                foreground,
                background: "pie.data.fill",
                ratio,
                minimum,
            },
        ) => shared_name == foreground && shared_ratio == ratio && shared_minimum == minimum,
        _ => shared == family,
    }
}

impl ContrastChecks<'_> {
    fn push_notice(
        &mut self,
        mut notice: ThemeNotice,
        color: Color,
        backgrounds: &[ContrastBackground<'_>],
        opacity: f64,
    ) {
        let paints = || {
            backgrounds
                .iter()
                .map(|&(_, paint, canvas)| (paint, canvas.filter(|_| !is_opaque(paint, opacity))))
        };
        match &mut self.scope {
            NoticeScope::Shared(inputs) => inputs.push(ContrastNoticeInput {
                color,
                opacity,
                paints: paints()
                    .map(|(paint, canvas)| (paint.clone(), canvas.cloned()))
                    .collect(),
            }),
            NoticeScope::Family(id, inputs) => {
                // Shared inputs correspond to the leading notices. A ratio (especially 1:1)
                // or a replacement color alone can coincide for different contrast inputs.
                if inputs.iter().zip(&*self.notices).any(|(input, shared)| {
                    input.opacity == opacity
                        && input.color == color
                        && same_notice(shared, &notice)
                        && input
                            .paints
                            .iter()
                            .map(|(paint, canvas)| (paint, canvas.as_ref()))
                            .eq(paints())
                }) {
                    return;
                }
                let foreground = match &mut notice {
                    ThemeNotice::LowContrast { foreground, .. }
                    | ThemeNotice::ContrastAdjusted { foreground, .. } => foreground,
                };
                *foreground = format!("family_overrides[{id:?}].{foreground}");
            }
        }
        self.notices.push(notice);
    }

    fn check(
        &mut self,
        name: &str,
        color: &mut Color,
        backgrounds: &[ContrastBackground<'_>],
        opacity: f64,
        minimum: f64,
    ) {
        let appearance = self.appearance;
        let ratio = |color, background: &Paint, canvas: Option<&Paint>| {
            Paint::Solid(color).minimum_contrast_over(background, opacity, canvas, appearance)
        };
        if self.auto_fix
            && let Some(adjusted) = fit_contrast(*color, |candidate| {
                backgrounds
                    .iter()
                    .all(|&(_, background, canvas)| ratio(candidate, background, canvas) >= minimum)
            })
        {
            let original = *color;
            *color = adjusted;
            self.push_notice(
                ThemeNotice::ContrastAdjusted {
                    foreground: name.to_owned(),
                    color: adjusted,
                },
                original,
                backgrounds,
                opacity,
            );
        }
        for &(background, paint, canvas) in backgrounds {
            let ratio = ratio(*color, paint, canvas);
            if ratio < minimum {
                self.push_notice(
                    ThemeNotice::LowContrast {
                        foreground: name.to_owned(),
                        background,
                        ratio,
                        minimum,
                    },
                    *color,
                    &[(background, paint, canvas)],
                    opacity,
                );
            }
        }
    }

    fn check_family(
        &mut self,
        family: &mut FamilyTheme,
        fill_opacity: f64,
        fill_background: &'static str,
        title_accent: Option<Color>,
    ) {
        let roles = &mut family.roles;
        let canvas = ("canvas", &roles.canvas, None);
        let node = ("surface.node", &roles.surface_node, Some(&roles.canvas));
        let container = (
            "surface.container",
            &roles.surface_container,
            Some(&roles.canvas),
        );
        let callout = (
            "surface.callout",
            &roles.surface_callout,
            Some(&roles.canvas),
        );
        let emphasis = (
            "surface.emphasis",
            &roles.surface_emphasis,
            Some(&roles.canvas),
        );

        self.check(
            "text.primary",
            &mut roles.text_primary,
            &[canvas, node, container],
            1.0,
            TEXT_CONTRAST,
        );
        self.check(
            "text.muted",
            &mut roles.text_muted,
            &[canvas, node],
            1.0,
            TEXT_CONTRAST,
        );
        if let Some(accent) = title_accent {
            let ratio = Paint::Solid(accent).minimum_contrast_over(
                &roles.canvas,
                1.0,
                None,
                self.appearance,
            );
            roles.text_title = if ratio >= TEXT_CONTRAST {
                accent
            } else {
                roles.text_primary
            };
        }
        self.check(
            "text.title",
            &mut roles.text_title,
            &[canvas, container],
            1.0,
            TEXT_CONTRAST,
        );
        self.check(
            "text.callout",
            &mut roles.text_callout,
            &[callout],
            1.0,
            TEXT_CONTRAST,
        );
        self.check(
            "text.on_emphasis",
            &mut roles.text_on_emphasis,
            &[emphasis],
            1.0,
            TEXT_CONTRAST,
        );

        let mut series = family.data.ink.to_vec();
        for (index, color) in series.iter_mut().enumerate() {
            self.check(
                &format!("data.ink[{index}]"),
                color,
                &[canvas],
                1.0,
                SERIES_CONTRAST,
            );
        }
        if self.auto_fix {
            family.data.ink = series.into();
        }
        self.check_fill_ink(family, fill_opacity, fill_background);
    }

    fn check_fill_ink(&mut self, family: &mut FamilyTheme, opacity: f64, background: &'static str) {
        let mut fill_ink = family.data.fill_ink.to_vec();
        for (index, (fill, ink)) in family.data.fill.iter().zip(&mut fill_ink).enumerate() {
            self.check(
                &format!("data.fill_ink[{index}]"),
                ink,
                &[(background, &Paint::Solid(*fill), Some(&family.roles.canvas))],
                opacity,
                TEXT_CONTRAST,
            );
        }
        if self.auto_fix {
            family.data.fill_ink = fill_ink.into();
        }
    }
}

fn apply_family_override(family: &mut FamilyTheme, patch: &FamilyOverride, appearance: Appearance) {
    if let Some(roles) = &patch.roles {
        family.roles.apply(roles);
    }
    if let Some(form) = &patch.form {
        family.form.apply(form);
    }
    if let Some(elevation) = patch
        .elevation
        .as_ref()
        .or_else(|| patch.form.as_ref()?.elevation.as_ref())
    {
        family.elevation = resolve_elevation(elevation, appearance);
    }
    if let Some(typography) = &patch.typography {
        family.typography.apply(typography);
    }
    if let Some(ordinal) = patch.node_ordinal_palette {
        family.node_ordinal_palette = ordinal;
    }
}

fn resolve_elevation(elevation: &ElevationSpec, mode: Appearance) -> Elevation {
    let shadow = |dy, blur, color, opacity| Elevation::Shadows {
        layers: Arc::new([ShadowLayer {
            dx: 0.0,
            dy,
            blur,
            color,
            opacity,
        }]),
    };
    match elevation {
        ElevationSpec::Preset(ElevationPreset::Flat) => Elevation::Flat,
        ElevationSpec::Preset(ElevationPreset::Soft) if mode == Appearance::Dark => {
            shadow(3.0, 6.0, BLACK, 0.32)
        }
        ElevationSpec::Preset(ElevationPreset::Soft) => shadow(2.0, 4.0, DARK_INK, 0.10),
        ElevationSpec::Preset(ElevationPreset::Crisp) => shadow(1.5, 1.5, DARK_INK, 0.14),
        ElevationSpec::Effect(ElevationEffect::LayeredStack { layers }) => Elevation::Shadows {
            layers: layers.as_slice().into(),
        },
        ElevationSpec::Effect(ElevationEffect::Glow {
            color,
            blur,
            opacity,
        }) => Elevation::Glow {
            color: *color,
            blur: *blur,
            opacity: *opacity,
        },
    }
}
