use std::collections::BTreeMap;
use std::sync::Arc;

use sha2::{Digest, Sha256};

use super::document::{
    CanvasPatch, DataPatch, ElevationEffect, ElevationPreset, ElevationSpec, FamilyOverride,
    FillInk, SeedPatch,
};
use super::{
    Appearance, BuiltinPreset, Canvas, Color, DataPalette, Elevation, FamilyTheme, Form, Paint,
    ResolvedThemeV2, Roles, ShadowLayer, ThemeDocumentV2, ThemeNotice, Typography, json, validate,
};

const DEFAULT_MAX_BYTES: usize = 256 * 1024;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const TEXT_CONTRAST: f64 = 4.5;
const SERIES_CONTRAST: f64 = 3.0;
const DERIVED_SERIES_CONTRAST: f64 = 3.5;
const DARK_INK: Color = Color::rgb(15, 23, 42);
const LIGHT_INK: Color = Color::rgb(248, 250, 252);
const BLACK: Color = Color::rgb(0, 0, 0);
const WHITE: Color = Color::rgb(255, 255, 255);

/// An invalid theme document, unavailable appearance, or failed contrast check.
#[derive(Debug, thiserror::Error)]
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
    pub fn compile_preset(
        &self,
        preset: BuiltinPreset,
        appearance: Option<Appearance>,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        self.resolve(&preset.document()?, appearance, true, false)
    }

    fn resolve(
        &self,
        document: &ThemeDocumentV2,
        requested: Option<Appearance>,
        strict: bool,
        auto_fix: bool,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        validate::document(document)?;
        let inherited = document.extends().map(load_base).transpose()?;
        let documents: Vec<_> = inherited.iter().chain([document]).collect();
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
        for document in &documents {
            if let Some(roles_patch) = document
                .layer(appearance)
                .and_then(|layer| layer.roles.as_ref())
            {
                roles.apply(roles_patch);
            }
            if let Some(roles_patch) = &document.roles {
                roles.apply(roles_patch);
            }
        }

        let palette = derive_palette(
            &data,
            accent,
            &roles.canvas,
            form.chart.pie_opacity,
            appearance,
        );
        let mut base = FamilyTheme {
            roles,
            data: palette,
            form,
            typography,
            elevation,
            node_ordinal_palette: false,
        };
        let mut family_overrides = BTreeMap::<String, FamilyTheme>::new();
        for patch in documents.iter().flat_map(|document| &document.overrides) {
            if patch.appearance.is_none_or(|mode| mode == appearance) {
                for family in &patch.families {
                    let family = family_overrides
                        .entry(family.clone())
                        .or_insert_with(|| base.clone());
                    apply_family_override(family, patch, appearance);
                }
            }
        }

        let mut notices = Vec::new();
        for family in family_overrides.values_mut() {
            if family.roles.canvas != base.roles.canvas
                || family.form.chart.pie_opacity != base.form.chart.pie_opacity
            {
                family.data = derive_palette(
                    &data,
                    accent,
                    &family.roles.canvas,
                    family.form.chart.pie_opacity,
                    appearance,
                );
            }
            check_contrast(family, appearance, auto_fix, &mut notices);
        }
        check_contrast(&mut base, appearance, auto_fix, &mut notices);
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

        let mut result = ResolvedThemeV2 {
            id: document.id.clone(),
            appearance,
            base,
            canvas,
            family_overrides,
            fingerprint: [0; 32],
            notices,
        };
        let canonical = crate::canonical_json::canonical_json_bytes(&result)
            .map_err(|error| ThemeError::invalid(".", error.to_string()))?;
        result.fingerprint = Sha256::digest(canonical).into();
        Ok(Arc::new(result))
    }
}

fn load_base(id: &str) -> Result<ThemeDocumentV2, ThemeError> {
    let preset = BuiltinPreset::from_id(id)
        .ok_or_else(|| ThemeError::invalid("extends", format!("unknown built-in preset `{id}`")))?;
    let document = preset.document()?;
    if document.extends.is_some() {
        return Err(ThemeError::invalid(
            "extends",
            "preset extension is limited to one level",
        ));
    }
    validate::document(&document)?;
    Ok(document)
}

fn select_appearance(
    documents: &[&ThemeDocumentV2],
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
        text_title: if accent.contrast(canvas) >= TEXT_CONTRAST {
            accent
        } else {
            ink
        },
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
    opacity: f64,
    mode: Appearance,
) -> DataPalette {
    let dark = mode == Appearance::Dark;
    let canvas = paint_sample(canvas, backdrop(mode));
    let h = accent.oklch()[2];
    let hues = || (0..10).map(|index| h + f64::from(index) * 137.508);
    let ink = data.ink.clone().unwrap_or_else(|| {
        let (l, c) = if dark { (0.74, 0.13) } else { (0.48, 0.14) };
        hues()
            .map(|h| {
                let ink = Color::from_oklch(l, c, h);
                fit_contrast(ink, &[canvas], DERIVED_SERIES_CONTRAST).unwrap_or(ink)
            })
            .collect()
    });
    let fill = data.fill.clone().unwrap_or_else(|| {
        hues()
            .map(|h| Color::from_oklch(if dark { 0.34 } else { 0.84 }, 0.085, h))
            .collect()
    });
    let fill_ink = fill
        .iter()
        .map(|color| match data.fill_ink {
            Some(FillInk::Color(ink)) => ink,
            _ => {
                let background = color.composite(canvas, opacity);
                let ink = best_ink(background, DARK_INK, LIGHT_INK);
                if ink.contrast(background) >= TEXT_CONTRAST {
                    ink
                } else {
                    best_ink(background, BLACK, WHITE)
                }
            }
        })
        .collect();
    DataPalette {
        ink: ink.into(),
        fill: fill.into(),
        fill_ink,
        pattern_encoding: data.pattern_encoding.unwrap_or(false),
    }
}

fn best_ink(background: Color, first: Color, second: Color) -> Color {
    if first.contrast(background) >= second.contrast(background) {
        first
    } else {
        second
    }
}

fn backdrop(appearance: Appearance) -> Color {
    match appearance {
        Appearance::Light => WHITE,
        Appearance::Dark => BLACK,
    }
}

fn paint_colors(paint: &Paint, background: Color) -> impl Iterator<Item = Color> + '_ {
    let (solid, stops) = match paint {
        Paint::Solid(color) => (Some(color.composite(background, 1.0)), &[][..]),
        Paint::LinearGradient(gradient) => (None, gradient.stops.as_slice()),
    };
    solid.into_iter().chain(
        stops
            .iter()
            .map(move |stop| stop.color.composite(background, stop.opacity)),
    )
}

fn paint_sample(paint: &Paint, background: Color) -> Color {
    paint_colors(paint, background).next().unwrap_or(background)
}

fn fit_contrast(color: Color, backgrounds: &[Color], minimum: f64) -> Option<Color> {
    let passes = |color: Color| {
        backgrounds
            .iter()
            .all(|&background| color.composite(background, 1.0).contrast(background) >= minimum)
    };
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

fn check_contrast(
    family: &mut FamilyTheme,
    appearance: Appearance,
    auto_fix: bool,
    notices: &mut Vec<ThemeNotice>,
) {
    let roles = &mut family.roles;
    let canvas: Vec<_> = paint_colors(&roles.canvas, backdrop(appearance)).collect();
    let underlying = canvas[0];
    let node: Vec<_> = paint_colors(&roles.surface_node, underlying).collect();
    let container: Vec<_> = paint_colors(&roles.surface_container, underlying).collect();
    let callout: Vec<_> = paint_colors(&roles.surface_callout, underlying).collect();
    let emphasis: Vec<_> = paint_colors(&roles.surface_emphasis, underlying).collect();
    let mut check =
        |name: &str, color: &mut Color, backgrounds: &[(&'static str, &[Color])], minimum: f64| {
            if auto_fix {
                let samples: Vec<_> = backgrounds
                    .iter()
                    .flat_map(|(_, colors)| colors.iter().copied())
                    .collect();
                if let Some(adjusted) = fit_contrast(*color, &samples, minimum) {
                    *color = adjusted;
                    notices.push(ThemeNotice::ContrastAdjusted {
                        foreground: name.to_owned(),
                        color: adjusted,
                    });
                }
            }
            for &(background, colors) in backgrounds {
                let ratio = colors
                    .iter()
                    .map(|&bg| color.composite(bg, 1.0).contrast(bg))
                    .fold(f64::INFINITY, f64::min);
                if ratio < minimum {
                    notices.push(ThemeNotice::LowContrast {
                        foreground: name.to_owned(),
                        background,
                        ratio,
                        minimum,
                    });
                }
            }
        };

    check(
        "text.primary",
        &mut roles.text_primary,
        &[
            ("canvas", &canvas),
            ("surface.node", &node),
            ("surface.container", &container),
        ],
        TEXT_CONTRAST,
    );
    check(
        "text.muted",
        &mut roles.text_muted,
        &[("canvas", &canvas), ("surface.node", &node)],
        TEXT_CONTRAST,
    );
    check(
        "text.title",
        &mut roles.text_title,
        &[("canvas", &canvas), ("surface.container", &container)],
        TEXT_CONTRAST,
    );
    check(
        "text.callout",
        &mut roles.text_callout,
        &[("surface.callout", &callout)],
        TEXT_CONTRAST,
    );
    check(
        "text.on_emphasis",
        &mut roles.text_on_emphasis,
        &[("surface.emphasis", &emphasis)],
        TEXT_CONTRAST,
    );

    let mut series = family.data.ink.iter().copied().collect::<Vec<_>>();
    for (index, color) in series.iter_mut().enumerate() {
        check(
            &format!("data.ink[{index}]"),
            color,
            &[("canvas", &canvas)],
            SERIES_CONTRAST,
        );
    }
    if auto_fix {
        family.data.ink = series.into();
    }
    let mut fill_ink = family.data.fill_ink.iter().copied().collect::<Vec<_>>();
    for (index, (fill, ink)) in family.data.fill.iter().zip(&mut fill_ink).enumerate() {
        let opacity = family.form.chart.pie_opacity;
        let fills: Vec<_> = canvas
            .iter()
            .map(|color| fill.composite(*color, opacity))
            .collect();
        check(
            &format!("data.fill_ink[{index}]"),
            ink,
            &[("data.fill", &fills)],
            TEXT_CONTRAST,
        );
    }
    if auto_fix {
        family.data.fill_ink = fill_ink.into();
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
