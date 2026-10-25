use merman_bindings_core::{
    BindingEngine, BindingEngineServices, BindingError, resource_options_json,
};

use crate::runtime::Limits;
use crate::wire::{Config, check_version, nonempty_options, parse};

pub(crate) fn create_engine(
    config: &[u8],
    blob: &[u8],
    limits: Limits,
    available_font_bytes: usize,
) -> Result<(BindingEngine, usize), BindingError> {
    let config: Config<'_> = parse(config, limits.json_bytes)?;
    check_version(config.version)?;
    let default_options;
    let options = if let Some(options) = nonempty_options(config.options) {
        options
    } else {
        default_options = resource_options_json(Some("interactive"), &[])?;
        &default_options
    };
    let (services, font_bytes) = services(&config, blob, limits, available_font_bytes)?;
    let engine = crate::artifact_contract().create_engine_with_services(options, services)?;
    Ok((engine, font_bytes))
}

#[cfg(not(feature = "native-fonts"))]
fn services(
    config: &Config<'_>,
    blob: &[u8],
    limits: Limits,
    available_font_bytes: usize,
) -> Result<(BindingEngineServices, usize), BindingError> {
    let _ = (config, blob, limits, available_font_bytes);
    Err(BindingError::missing_capability(
        "native-fonts",
        "WASI engine construction requires the native-fonts feature and admitted font bytes",
    ))
}

#[cfg(feature = "native-fonts")]
fn services(
    config: &Config<'_>,
    blob: &[u8],
    limits: Limits,
    available_font_bytes: usize,
) -> Result<(BindingEngineServices, usize), BindingError> {
    use merman_bindings_core::OperationControl;
    use merman_export::{NativeFontContext, NativeFontError, NativeFontLimits};

    use crate::memory::copy_font;
    use crate::runtime::limit_error;

    #[cfg(not(feature = "bundled-fonts"))]
    if config.bundled_fonts {
        return Err(BindingError::missing_capability(
            "native-fonts",
            "this WASI artifact has no bundled font resource; supply caller fonts",
        ));
    }
    let font_limits = NativeFontLimits {
        max_bytes: limits.engine_font_bytes.min(available_font_bytes),
        ..Default::default()
    };
    if config.fonts.len() > font_limits.max_files {
        return Err(limit_error(
            "wasi-font-count",
            config.fonts.len(),
            font_limits.max_files,
            "too many font buffers",
        ));
    }
    let mut total = 0_usize;
    for font in &config.fonts {
        let end = font
            .offset
            .checked_add(font.length)
            .ok_or_else(|| BindingError::invalid_argument("font range overflows the font blob"))?;
        if font.length == 0 || blob.get(font.offset as usize..end as usize).is_none() {
            return Err(BindingError::invalid_argument(
                "font range lies outside the font blob",
            ));
        }
        total = total
            .checked_add(font.length as usize)
            .ok_or_else(|| BindingError::invalid_argument("font byte length overflow"))?;
        if total > font_limits.max_bytes {
            return Err(limit_error(
                "wasi-font-bytes",
                total,
                font_limits.max_bytes,
                "engine font byte budget exhausted",
            ));
        }
    }
    let mut fonts = Vec::new();
    fonts.try_reserve_exact(config.fonts.len()).map_err(|_| {
        limit_error(
            "wasi-font-count",
            config.fonts.len(),
            font_limits.max_files,
            "font registry allocation failed",
        )
    })?;
    for font in &config.fonts {
        let start = font.offset as usize;
        let end = start + font.length as usize;
        fonts.push(copy_font(&blob[start..end])?);
    }
    #[cfg(feature = "bundled-fonts")]
    if config.bundled_fonts {
        for bytes in merman_export::bundled_font_data() {
            total = total
                .checked_add(bytes.len())
                .ok_or_else(|| BindingError::invalid_argument("font byte length overflow"))?;
            if total > font_limits.max_bytes || fonts.len() >= font_limits.max_files {
                return Err(limit_error(
                    "wasi-font-bytes",
                    total,
                    font_limits.max_bytes,
                    "bundled fonts exceed the engine font budget",
                ));
            }
            fonts.try_reserve(1).map_err(|_| {
                limit_error(
                    "wasi-font-count",
                    fonts.len() + 1,
                    font_limits.max_files,
                    "font registry allocation failed",
                )
            })?;
            fonts.push(bytes);
        }
    }
    let context = NativeFontContext::from_font_bytes(fonts, font_limits, &OperationControl::new())
        .map_err(|error| match error {
            NativeFontError::Limit {
                resource,
                actual,
                limit,
            } => limit_error(
                resource,
                actual,
                limit,
                "native font admission limit exceeded",
            ),
            error => {
                BindingError::invalid_argument(format!("native font admission failed: {error}"))
            }
        })?
        .with_strict_glyph_coverage(true);
    Ok((
        BindingEngineServices::new().with_native_font_context(context),
        total,
    ))
}
