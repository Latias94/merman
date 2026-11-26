//! Root SVG metadata and raster sizing under the export limits.

#[cfg(any(feature = "png", feature = "jpeg"))]
use cssparser::{Delimiter, Parser, Token};

#[cfg(any(feature = "png", feature = "jpeg"))]
use crate::{DEFAULT_MAX_RASTER_PIXELS, DEFAULT_MAX_RASTER_SIDE_LENGTH, RasterOptions, RasterPlan};
use crate::{ExportError, OperationControl, Result, export_checkpoint};

/// Optional display box for target-aware rasterization.
///
/// Browser previews typically draw Mermaid SVG as vector content inside a container. A headless
/// rasterizer has to allocate a full pixmap, so UI hosts should pass the visible container size
/// here and use [`RasterOptions::scale`] for device-pixel ratio.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterFitBox {
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterFitBox {
    pub const fn new(width: Option<u32>, height: Option<u32>) -> Self {
        Self { width, height }
    }

    pub const fn width(width: u32) -> Self {
        Self {
            width: Some(width),
            height: None,
        }
    }

    pub const fn height(height: u32) -> Self {
        Self {
            width: None,
            height: Some(height),
        }
    }

    pub const fn contain(width: u32, height: u32) -> Self {
        Self {
            width: Some(width),
            height: Some(height),
        }
    }
}

/// Resource budget applied before allocating the output pixmap.
#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterSizeLimit {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub max_pixels: Option<u64>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterSizeLimit {
    pub const fn new(
        max_width: Option<u32>,
        max_height: Option<u32>,
        max_pixels: Option<u64>,
    ) -> Self {
        Self {
            max_width,
            max_height,
            max_pixels,
        }
    }

    pub const fn max_side_length(max_side_length: u32) -> Self {
        Self {
            max_width: Some(max_side_length),
            max_height: Some(max_side_length),
            max_pixels: None,
        }
    }

    pub const fn default_safe() -> Self {
        Self {
            max_width: Some(DEFAULT_MAX_RASTER_SIDE_LENGTH),
            max_height: Some(DEFAULT_MAX_RASTER_SIDE_LENGTH),
            max_pixels: Some(DEFAULT_MAX_RASTER_PIXELS),
        }
    }

    pub const fn unbounded() -> Self {
        Self {
            max_width: None,
            max_height: None,
            max_pixels: None,
        }
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl Default for RasterSizeLimit {
    fn default() -> Self {
        Self::default_safe()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RootSvgMetadata {
    pub(super) has_view_box: bool,
    pub(super) has_explicit_size: bool,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    max_width_px: Option<f32>,
}

pub(super) fn parse_root_svg_metadata(
    svg: &str,
    control: &OperationControl,
) -> Result<RootSvgMetadata> {
    use quick_xml::{XmlVersion, events::Event, name::ResolveResult, reader::NsReader};

    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    loop {
        export_checkpoint(control)?;
        let event = reader.read_event().map_err(|_| ExportError::SvgParse)?;
        let element = match event {
            Event::Start(element) | Event::Empty(element) => element,
            Event::Eof => return Err(ExportError::SvgParse),
            _ => continue,
        };

        let (namespace, local_name) = reader.resolver().resolve_element(element.name());
        let is_svg_namespace = matches!(namespace, ResolveResult::Unbound)
            || matches!(
                namespace,
                ResolveResult::Bound(namespace)
                    if namespace.as_ref() == "http://www.w3.org/2000/svg"
            );
        if !is_svg_namespace || local_name.as_ref() != "svg" {
            return Err(ExportError::SvgParse);
        }

        let mut metadata = RootSvgMetadata::default();
        let mut view_box_seen = false;
        let mut width = None;
        let mut height = None;
        #[cfg(any(feature = "png", feature = "jpeg"))]
        let mut style_seen = false;
        for attribute in element.attributes() {
            export_checkpoint(control)?;
            let attribute = attribute.map_err(|_| ExportError::SvgParse)?;
            if attribute.key.as_namespace_binding().is_some() {
                continue;
            }
            let (namespace, local_name) = reader.resolver().resolve_attribute(attribute.key);
            #[cfg(any(feature = "png", feature = "jpeg"))]
            let is_unbound_attribute = matches!(&namespace, ResolveResult::Unbound);
            let consumed_by_usvg = match namespace {
                ResolveResult::Unknown(_) => return Err(ExportError::SvgParse),
                ResolveResult::Unbound => true,
                ResolveResult::Bound(namespace) => matches!(
                    namespace.as_ref(),
                    "http://www.w3.org/2000/svg"
                        | "http://www.w3.org/1999/xlink"
                        | "http://www.w3.org/XML/1998/namespace"
                ),
            };
            if !consumed_by_usvg {
                continue;
            }

            let value = attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(|_| ExportError::SvgParse)?;
            match local_name.as_ref() {
                "viewBox" if !view_box_seen => {
                    view_box_seen = true;
                    metadata.has_view_box = has_valid_svg_view_box(value.as_ref());
                }
                "width" if width.is_none() => {
                    width = Some(has_explicit_svg_length(value.as_ref()));
                }
                "height" if height.is_none() => {
                    height = Some(has_explicit_svg_length(value.as_ref()));
                }
                // usvg projects namespaced presentation attributes by local name, but parses the
                // `style` declaration list only from the unbound XML attribute.
                #[cfg(any(feature = "png", feature = "jpeg"))]
                "style" if is_unbound_attribute && !style_seen => {
                    style_seen = true;
                    metadata.max_width_px = parse_inline_max_width_px(value.as_ref(), control)?;
                }
                _ => {}
            }
        }
        metadata.has_explicit_size = width == Some(true) && height == Some(true);
        return Ok(metadata);
    }
}

fn has_explicit_svg_length(value: &str) -> bool {
    value.parse::<svgtypes::Length>().is_ok_and(|length| {
        length.unit != svgtypes::LengthUnit::Percent
            && length.number.is_finite()
            && length.number > 0.0
    })
}

fn has_valid_svg_view_box(value: &str) -> bool {
    let Ok(view_box) = value.parse::<svgtypes::ViewBox>() else {
        return false;
    };
    usvg::NonZeroRect::from_xywh(
        view_box.x as f32,
        view_box.y as f32,
        view_box.w as f32,
        view_box.h as f32,
    )
    .is_some()
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn parse_inline_max_width_px(style: &str, control: &OperationControl) -> Result<Option<f32>> {
    let mut parser = Parser::new(style);
    let mut max_width = None;

    while !parser.is_exhausted() {
        export_checkpoint(control)?;
        let declaration = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?;
            declaration.expect_colon()?;

            if !property.eq_ignore_ascii_case("max-width") {
                declaration.expect_no_error_token()?;
                return Ok::<_, cssparser::ParseError<()>>(None);
            }

            let token = declaration.next()?.clone();
            declaration.expect_exhausted()?;
            let Token::Dimension { value, unit, .. } = token else {
                return Ok(None);
            };
            if unit.eq_ignore_ascii_case("px") && value.is_finite() && value > 0.0 {
                Ok(Some(value))
            } else {
                Ok(None)
            }
        });

        if let Ok(Some(value)) = declaration {
            max_width = Some(value);
        }
    }
    Ok(max_width)
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone, Copy)]
pub(super) struct RasterGeometry {
    pub(super) min_x: f32,
    pub(super) min_y: f32,
    width: f32,
    height: f32,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn raster_geometry_for_svg(
    metadata: RootSvgMetadata,
    tree: &usvg::Tree,
) -> (RasterGeometry, bool) {
    if metadata.has_view_box || metadata.has_explicit_size {
        // `usvg`/`resvg` already apply the root viewBox transform (including translating the
        // viewBox min corner to (0,0)) when building/rendering the tree. If we also translate
        // by `-min_x/-min_y` here, diagrams with negative viewBox mins (e.g. kanban, gitGraph)
        // get shifted fully out of the viewport and render as a blank/transparent pixmap.
        let size = tree.size();
        return (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: size.width(),
                height: size.height(),
            },
            false,
        );
    }

    // Some Mermaid diagrams (e.g. `info`) don't emit a viewBox upstream.
    // For raster formats, fall back to the rendered content bounds as computed by usvg.
    let Some(bbox) = tree.root().abs_layer_bounding_box() else {
        // Preserve the minimum raster canvas for an empty SVG without an explicit viewport.
        return (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            false,
        );
    };
    let w = bbox.width().max(1.0);
    let h = bbox.height().max(1.0);
    if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
        (
            RasterGeometry {
                min_x: bbox.x(),
                min_y: bbox.y(),
                width: w,
                height: h,
            },
            true,
        )
    } else {
        let size = tree.size();
        (
            RasterGeometry {
                min_x: 0.0,
                min_y: 0.0,
                width: size.width(),
                height: size.height(),
            },
            false,
        )
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn raster_plan_for_geometry(
    geo: RasterGeometry,
    options: &RasterOptions,
    control: &OperationControl,
) -> Result<RasterPlan> {
    export_checkpoint(control)?;
    if !(options.scale.is_finite() && options.scale > 0.0) {
        return Err(ExportError::InvalidScale);
    }

    validate_fit_box(options.fit_to)?;
    validate_size_limit(options.size_limit)?;

    // Make scaling more intuitive/stable: at scale=1 we already round up to whole pixels, so for
    // scale>1 prefer scaling the *rounded* base size. This avoids surprising off-by-one shrinkage
    // when the viewBox/bounds are fractional (e.g. 342.36 * 2 = 684.72 -> ceil = 685, while
    // ceil(342.36) * 2 = 686).
    let base_width_px = f64::from(geo.width).ceil().max(1.0);
    let base_height_px = f64::from(geo.height).ceil().max(1.0);

    let fit_scale = fit_scale_for_base_size(base_width_px, base_height_px, options.fit_to);
    let requested_scale = fit_scale * f64::from(options.scale);
    let requested_width_px = requested_raster_dim_px(base_width_px * requested_scale)?;
    let requested_height_px = requested_raster_dim_px(base_height_px * requested_scale)?;

    let limit_scale = size_limit_scale(
        base_width_px * requested_scale,
        base_height_px * requested_scale,
        options.size_limit,
    );
    let mut effective_scale = requested_scale * limit_scale;
    let (mut width_px, mut height_px) = raster_limited_dims(
        base_width_px,
        base_height_px,
        effective_scale,
        options.size_limit,
    )?;

    if let Some(max_pixels) = options.size_limit.max_pixels {
        for _ in 0..8 {
            export_checkpoint(control)?;
            if u64::from(width_px) * u64::from(height_px) <= max_pixels {
                break;
            }
            let pixels = f64::from(width_px) * f64::from(height_px);
            let shrink = ((max_pixels as f64) / pixels).sqrt() * 0.999_999;
            effective_scale *= shrink;
            (width_px, height_px) = raster_limited_dims(
                base_width_px,
                base_height_px,
                effective_scale,
                options.size_limit,
            )?;
        }
    }

    Ok(RasterPlan {
        requested_width_px,
        requested_height_px,
        width_px,
        height_px,
        requested_scale,
        effective_scale,
        limited: f64::from(width_px) != requested_width_px
            || f64::from(height_px) != requested_height_px,
    })
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn validate_fit_box(fit: Option<RasterFitBox>) -> Result<()> {
    let Some(fit) = fit else {
        return Ok(());
    };

    if fit.width.is_none() && fit.height.is_none() {
        return Err(ExportError::InvalidSizing(
            "fit_to must include a positive width or height",
        ));
    }
    if fit.width == Some(0) || fit.height == Some(0) {
        return Err(ExportError::InvalidSizing(
            "fit_to width and height must be positive",
        ));
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn validate_size_limit(limit: RasterSizeLimit) -> Result<()> {
    if limit.max_width == Some(0) || limit.max_height == Some(0) {
        return Err(ExportError::InvalidSizing(
            "size_limit max_width and max_height must be positive",
        ));
    }
    if limit.max_pixels == Some(0) {
        return Err(ExportError::InvalidSizing(
            "size_limit max_pixels must be positive",
        ));
    }
    Ok(())
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn fit_scale_for_base_size(width: f64, height: f64, fit: Option<RasterFitBox>) -> f64 {
    let Some(fit) = fit else {
        return 1.0;
    };

    let mut scale: f64 = 1.0;
    if let Some(target_width) = fit.width {
        scale = scale.min(f64::from(target_width) / width);
    }
    if let Some(target_height) = fit.height {
        scale = scale.min(f64::from(target_height) / height);
    }
    if scale.is_nan() {
        1.0
    } else {
        scale.clamp(0.0, 1.0)
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn size_limit_scale(width: f64, height: f64, limit: RasterSizeLimit) -> f64 {
    let mut scale: f64 = 1.0;
    if let Some(max_width) = limit.max_width {
        scale = scale.min(f64::from(max_width) / width);
    }
    if let Some(max_height) = limit.max_height {
        scale = scale.min(f64::from(max_height) / height);
    }
    if let Some(max_pixels) = limit.max_pixels {
        let pixels = width * height * scale * scale;
        if pixels > max_pixels as f64 {
            scale *= ((max_pixels as f64) / pixels).sqrt();
        }
    }
    if scale.is_nan() {
        1.0
    } else {
        scale.clamp(0.0, 1.0)
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_limited_dims(
    base_width_px: f64,
    base_height_px: f64,
    scale: f64,
    limit: RasterSizeLimit,
) -> Result<(u32, u32)> {
    Ok((
        raster_dim_px(base_width_px * scale, limit.max_width)?,
        raster_dim_px(base_height_px * scale, limit.max_height)?,
    ))
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_dim_px(value: f64, max: Option<u32>) -> Result<u32> {
    let value = requested_raster_dim_px(value)?;
    let value = max.map_or(value, |max| value.min(f64::from(max)));
    if value > f64::from(u32::MAX) {
        return Err(ExportError::InvalidSizing(
            "final raster dimension exceeds the u32 encoder capability",
        ));
    }
    Ok(value as u32)
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn requested_raster_dim_px(value: f64) -> Result<f64> {
    if !(value.is_finite() && value > 0.0) {
        return Err(ExportError::InvalidSizing(
            "computed raster dimension must be finite and positive",
        ));
    }
    Ok(value.ceil().max(1.0))
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn raster_usvg_options(metadata: RootSvgMetadata) -> usvg::Options<'static> {
    let mut opt = usvg::Options::default();
    if !metadata.has_view_box
        && let Some(max_width) = metadata.max_width_px
        && max_width.is_finite()
        && max_width > 0.0
        && let Some(size) = usvg::Size::from_wh(max_width, opt.default_size.height())
    {
        opt.default_size = size;
    }

    opt
}

#[cfg(all(test, any(feature = "png", feature = "jpeg")))]
#[path = "tests/root_svg_metadata.rs"]
mod tests;
