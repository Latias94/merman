//! Bounded admission of the resolved SVG tree, filters, and embedded images.

use crate::{
    EmbeddedImageLimit, EmbeddedImagePlan, ExportError, OperationControl, Result,
    SvgConversionLimits, SvgConversionPlan, export_checkpoint,
};
#[cfg(feature = "pdf")]
use crate::{PdfFilterImageLimit, PdfFilterImagePlan};

#[cfg(feature = "pdf")]
const KRILLA_MAX_FILTER_SIDE_PX: f64 = 5000.0;

pub(super) fn plan_svg_conversion(
    tree: &usvg::Tree,
    limits: SvgConversionLimits,
    control: &OperationControl,
) -> Result<SvgConversionPlan> {
    let mut plan = SvgConversionPlan::default();
    plan_svg_conversion_group(tree.root(), 0, 0, limits, &mut plan, control)?;
    Ok(plan)
}

fn plan_svg_conversion_group(
    root: &usvg::Group,
    parent_isolation_depth: usize,
    tree_depth: usize,
    limits: SvgConversionLimits,
    plan: &mut SvgConversionPlan,
    control: &OperationControl,
) -> Result<()> {
    let mut stack = vec![(root, parent_isolation_depth, tree_depth)];
    while let Some((group, parent_depth, tree_depth)) = stack.pop() {
        export_checkpoint(control)?;
        charge_svg_conversion_tree_node(plan)?;
        plan.max_tree_depth = plan.max_tree_depth.max(tree_depth);
        check_svg_conversion_limit(
            merman_render::resources::SVG_BACKEND_TREE_DEPTH_HARD_CAP_ID,
            plan.max_tree_depth,
            Some(merman_render::resources::MAX_RESVG_TREE_DEPTH),
        )?;

        let isolation_depth = parent_depth.saturating_add(usize::from(group.should_isolate()));
        plan.max_isolation_depth = plan.max_isolation_depth.max(isolation_depth);
        check_svg_conversion_limit(
            "max_isolation_depth",
            plan.max_isolation_depth,
            limits.max_isolation_depth,
        )?;

        if !group.filters().is_empty() {
            plan.filtered_groups = plan.filtered_groups.saturating_add(1);
        }
        for filter in group.filters() {
            export_checkpoint(control)?;
            let primitives = filter.primitives().len();
            check_svg_conversion_limit(
                "max_filter_primitives_per_filter",
                primitives,
                limits.max_filter_primitives_per_filter,
            )?;
            plan.filter_primitives = plan.filter_primitives.saturating_add(primitives);
            check_svg_conversion_limit(
                "max_total_filter_primitives",
                plan.filter_primitives,
                limits.max_total_filter_primitives,
            )?;
        }

        for node in group.children() {
            export_checkpoint(control)?;
            if let usvg::Node::Group(child) = node {
                stack.push((child, isolation_depth, tree_depth.saturating_add(1)));
            } else {
                charge_svg_conversion_tree_node(plan)?;
            }
            if let usvg::Node::Image(image) = node
                && matches!(image.kind(), usvg::ImageKind::SVG(_))
            {
                plan.nested_svg_images = plan.nested_svg_images.saturating_add(1);
                check_svg_conversion_limit(
                    "max_nested_svg_images",
                    plan.nested_svg_images,
                    limits.max_nested_svg_images,
                )?;
            }

            let mut subroot_result = Ok(());
            node.subroots(|subroot| {
                if subroot_result.is_err() {
                    return;
                }
                plan.subroots = plan.subroots.saturating_add(1);
                subroot_result =
                    check_svg_conversion_limit("max_subroots", plan.subroots, limits.max_subroots);
                if subroot_result.is_ok() {
                    subroot_result = plan_svg_conversion_group(
                        subroot,
                        isolation_depth.saturating_add(1),
                        tree_depth.saturating_add(1),
                        limits,
                        plan,
                        control,
                    );
                }
            });
            subroot_result?;
        }
    }
    Ok(())
}

fn charge_svg_conversion_tree_node(plan: &mut SvgConversionPlan) -> Result<()> {
    plan.tree_nodes = plan.tree_nodes.saturating_add(1);
    check_svg_conversion_limit(
        merman_render::resources::SVG_BACKEND_TREE_NODES_HARD_CAP_ID,
        plan.tree_nodes,
        Some(merman_render::resources::MAX_RESVG_TREE_NODES),
    )
}

fn check_svg_conversion_limit(
    limit_name: &'static str,
    actual: usize,
    max: Option<usize>,
) -> Result<()> {
    let Some(max) = max else {
        return Ok(());
    };
    if actual <= max {
        return Ok(());
    }
    Err(ExportError::SvgConversionLimit {
        limit_name,
        actual: actual as u64,
        max: max as u64,
    })
}

pub(super) fn directional_filter_transforms_supported(
    root: &usvg::Group,
    control: &OperationControl,
) -> Result<bool> {
    let mut stack = vec![(root, usvg::Transform::identity())];
    while let Some((group, parent_transform)) = stack.pop() {
        export_checkpoint(control)?;
        let transform = parent_transform.pre_concat(group.transform());
        // resvg factors rotation and reflection out of the filter layer and restores them
        // during compositing, including when Krilla renders a group into a localized image.
        let keeps_axes = transform.sx != 0.0
            && transform.sy != 0.0
            && transform.ky.abs() <= transform.sx.abs() * f32::EPSILON
            && transform.kx.abs() <= transform.sy.abs() * f32::EPSILON;
        let swaps_axes = transform.ky != 0.0
            && transform.kx != 0.0
            && transform.sx.abs() <= transform.ky.abs() * f32::EPSILON
            && transform.sy.abs() <= transform.kx.abs() * f32::EPSILON;
        if !(keeps_axes || swaps_axes) {
            for primitive in group
                .filters()
                .iter()
                .flat_map(|filter| filter.primitives())
            {
                export_checkpoint(control)?;
                let directional = match primitive.kind() {
                    usvg::filter::Kind::Offset(offset) => offset.dx() != 0.0 || offset.dy() != 0.0,
                    usvg::filter::Kind::DropShadow(shadow) => {
                        shadow.dx() != 0.0 || shadow.dy() != 0.0
                    }
                    _ => false,
                };
                if directional {
                    return Ok(false);
                }
            }
        }

        let mut mask = group.mask();
        while let Some(current) = mask {
            export_checkpoint(control)?;
            stack.push((current.root(), transform));
            mask = current.mask();
        }
        for filter in group.filters() {
            for primitive in filter.primitives() {
                export_checkpoint(control)?;
                if let usvg::filter::Kind::Image(image) = primitive.kind() {
                    stack.push((image.root(), transform));
                }
            }
        }
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => stack.push((child, transform)),
                usvg::Node::Image(image) => {
                    if let usvg::ImageKind::SVG(nested) = image.kind() {
                        stack.push((nested.root(), transform));
                    }
                }
                // Borrow these owners directly: Node::subroots limits callback borrows to the
                // callback body. resvg samples pattern paints in axis-aligned pixmaps.
                usvg::Node::Path(path) => {
                    for paint in [
                        path.fill().map(|fill| fill.paint()),
                        path.stroke().map(|stroke| stroke.paint()),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let usvg::Paint::Pattern(pattern) = paint {
                            stack.push((pattern.root(), usvg::Transform::identity()));
                        }
                    }
                }
                usvg::Node::Text(text) => {
                    stack.push((text.flattened(), usvg::Transform::identity()));
                }
            }
        }
    }
    Ok(true)
}

#[cfg(feature = "pdf")]
pub(super) fn plan_pdf_filter_images(
    tree: &usvg::Tree,
    page_scale: f32,
    requested_scale: f32,
    limit: PdfFilterImageLimit,
    control: &OperationControl,
) -> Result<(PdfFilterImagePlan, bool)> {
    let filtered_groups = pdf_filtered_group_bounds(tree, page_scale, control)?;
    let directional_residual = filtered_groups
        .iter()
        .any(|geometry| geometry.directional_transform_residual);
    let requested_pixels = pdf_filter_pixels(&filtered_groups, requested_scale, control)?.ok_or(
        ExportError::InvalidSizing(
            "PDF filter scale and backend side cap must leave every image dimension nonzero",
        ),
    )?;
    let Some(max_pixels) = limit.max_total_pixels else {
        return Ok((
            PdfFilterImagePlan {
                filtered_groups: filtered_groups.len(),
                requested_scale,
                effective_scale: requested_scale,
                requested_image_pixels: requested_pixels,
                effective_image_pixels: requested_pixels,
                limited: false,
            },
            directional_residual,
        ));
    };
    if requested_pixels <= max_pixels {
        return Ok((
            PdfFilterImagePlan {
                filtered_groups: filtered_groups.len(),
                requested_scale,
                effective_scale: requested_scale,
                requested_image_pixels: requested_pixels,
                effective_image_pixels: requested_pixels,
                limited: false,
            },
            directional_residual,
        ));
    }

    let mut lower = 0.0_f32;
    let mut accepted = None;
    let mut rejected = requested_scale;
    for _ in 0..48 {
        let candidate = lower + (rejected - lower) / 2.0;
        export_checkpoint(control)?;
        match pdf_filter_pixels(&filtered_groups, candidate, control)? {
            Some(pixels) if pixels <= max_pixels => {
                lower = candidate;
                accepted = Some((candidate, pixels));
            }
            Some(_) => rejected = candidate,
            // A zero-size backend image drops the entire group. It is not a valid budget fit.
            None => lower = candidate,
        }
    }
    let (accepted, effective_pixels) = accepted.ok_or(ExportError::PdfFilterImageLimit {
        actual: requested_pixels,
        max: max_pixels,
    })?;
    Ok((
        PdfFilterImagePlan {
            filtered_groups: filtered_groups.len(),
            requested_scale,
            effective_scale: accepted,
            requested_image_pixels: requested_pixels,
            effective_image_pixels: effective_pixels,
            limited: true,
        },
        directional_residual,
    ))
}

#[cfg(feature = "pdf")]
struct PdfFilterGeometry {
    width: f32,
    height: f32,
    scale: f32,
    directional_transform_residual: bool,
}

#[cfg(feature = "pdf")]
fn pdf_filtered_group_bounds(
    tree: &usvg::Tree,
    page_scale: f32,
    control: &OperationControl,
) -> Result<Vec<PdfFilterGeometry>> {
    let mut bounds = Vec::new();
    let mut stack = vec![(
        tree.root(),
        usvg::Transform::from_scale(page_scale, page_scale),
    )];
    while let Some((group, parent_transform)) = stack.pop() {
        export_checkpoint(control)?;
        if !group.filters().is_empty() {
            // Krilla skips the entire subtree if it cannot localize a layer.
            let Some(bbox) = group.layer_bounding_box() else {
                continue;
            };
            if group.abs_layer_bounding_box().is_none() {
                continue;
            }
            let directional_transform_residual =
                !directional_filter_transforms_supported(group, control)?;
            // Match krilla-svg's composed transform before its f32 side cap and pixel rounding.
            // Absolute bounds are insufficient: isolated PDF streams reset their current CTM.
            let local = bbox
                .transform(group.transform())
                .ok_or(ExportError::SvgDocSize)?;
            let actual = bbox
                .transform(parent_transform.pre_concat(group.transform()))
                .ok_or(ExportError::SvgDocSize)?;
            bounds.push(PdfFilterGeometry {
                width: local.width(),
                height: local.height(),
                scale: (actual.width() / local.width()).max(actual.height() / local.height()),
                directional_transform_residual,
            });
            // krilla-svg rasterizes this whole group through resvg and returns. Descendant
            // filters contribute to that localized render, but they do not allocate a second
            // krilla-owned PDF image and must not be counted as independent top-level groups.
            continue;
        }

        let mut transform = parent_transform.pre_concat(group.transform());
        if let Some(clip) = group.clip_path()
            && pdf_clip_path_uses_mask(clip, control)?
        {
            let mut clip = Some(clip);
            while let Some(current) = clip {
                export_checkpoint(control)?;
                if pdf_clip_path_uses_mask(current, control)? {
                    stack.push((current.root(), current.transform()));
                }
                clip = current.clip_path();
            }
            transform = usvg::Transform::identity();
        }
        let mut mask = group.mask();
        while let Some(current) = mask {
            export_checkpoint(control)?;
            stack.push((current.root(), usvg::Transform::identity()));
            mask = current.mask();
        }
        // These operations start new krilla content streams with an identity CTM.
        if group.isolate()
            || group.opacity() != usvg::Opacity::ONE
            || group.mask().is_some()
            || group.blend_mode() != usvg::BlendMode::Normal
        {
            transform = usvg::Transform::identity();
        }
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => stack.push((child, transform)),
                usvg::Node::Image(image) => {
                    if let usvg::ImageKind::SVG(nested) = image.kind() {
                        stack.push((nested.root(), transform));
                    }
                }
                // Pattern paints use separate streams, independent of the consuming path CTM.
                // Keep tree-owner borrows instead of escaping Node::subroots callback borrows.
                usvg::Node::Path(path) => {
                    for paint in [
                        path.fill().map(|fill| fill.paint()),
                        path.stroke().map(|stroke| stroke.paint()),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        if let usvg::Paint::Pattern(pattern) = paint {
                            stack.push((pattern.root(), usvg::Transform::identity()));
                        }
                    }
                }
                usvg::Node::Text(text) => {
                    stack.push((text.flattened(), usvg::Transform::identity()));
                }
            }
        }
    }
    Ok(bounds)
}

#[cfg(feature = "pdf")]
fn pdf_clip_path_uses_mask(clip: &usvg::ClipPath, control: &OperationControl) -> Result<bool> {
    // Match krilla-svg's native-clip admission; complex clips use a new masked content stream.
    let mut stack = vec![clip.root()];
    let mut rules = 0_usize;
    let mut nonzero = 0_usize;
    while let Some(group) = stack.pop() {
        export_checkpoint(control)?;
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => {
                    if child.clip_path().is_some() {
                        return Ok(true);
                    }
                    stack.push(child);
                }
                usvg::Node::Text(text) => stack.push(text.flattened()),
                usvg::Node::Path(path) => {
                    if let Some(fill) = path.fill() {
                        rules += 1;
                        nonzero += usize::from(fill.rule() == usvg::FillRule::NonZero);
                    }
                }
                usvg::Node::Image(_) => {}
            }
        }
    }
    Ok(nonzero != rules && !(nonzero == 0 && rules == 1))
}

#[cfg(feature = "pdf")]
fn pdf_filter_pixels(
    bounds: &[PdfFilterGeometry],
    filter_scale: f32,
    control: &OperationControl,
) -> Result<Option<u64>> {
    let mut total = 0_u64;
    for geometry in bounds {
        export_checkpoint(control)?;
        let max_scale = ((KRILLA_MAX_FILTER_SIDE_PX as f32) / geometry.width)
            .min((KRILLA_MAX_FILTER_SIDE_PX as f32) / geometry.height);
        let scale = (geometry.scale * filter_scale).min(max_scale);
        let width = geometry.width * scale;
        let height = geometry.height * scale;
        if !width.is_finite() || !height.is_finite() {
            return Err(ExportError::InvalidSizing(
                "PDF filter image dimensions must be finite",
            ));
        }
        let width_px = width.round() as u64;
        let height_px = height.round() as u64;
        if width_px == 0 || height_px == 0 {
            return Ok(None);
        }
        total = total.saturating_add(width_px.saturating_mul(height_px));
    }
    Ok(Some(total))
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct EmbeddedDataPlan {
    pub(super) resources: usize,
    pub(super) largest_bytes: u64,
    pub(super) total_bytes: u64,
}

pub(super) fn plan_embedded_data_resource_bytes<'a>(
    sources: impl Iterator<Item = (&'a str, usize)>,
    limit: EmbeddedImageLimit,
    control: &OperationControl,
) -> Result<EmbeddedDataPlan> {
    validate_embedded_image_limit(limit)?;
    let mut plan = EmbeddedDataPlan::default();
    for (href, occurrences) in sources {
        export_checkpoint(control)?;
        let data_url = data_url::DataUrl::process(href).map_err(|_| ExportError::SvgParse)?;
        let mut resource_bytes = 0_u64;
        let mut limit_error = None;
        let _ = data_url.decode(|chunk| {
            if let Err(error) = export_checkpoint(control) {
                limit_error = Some(error);
                return Err(());
            }
            resource_bytes = resource_bytes.saturating_add(chunk.len() as u64);
            let aggregate = plan
                .total_bytes
                .saturating_add(resource_bytes.saturating_mul(occurrences as u64));
            if let Some(max) = limit.max_bytes_per_image
                && resource_bytes > max
            {
                limit_error = Some(ExportError::EmbeddedImageLimit {
                    limit_name: "max_bytes_per_image",
                    actual: resource_bytes,
                    max,
                });
                return Err(());
            }
            if let Some(max) = limit.max_total_bytes
                && aggregate > max
            {
                limit_error = Some(ExportError::EmbeddedImageLimit {
                    limit_name: "max_total_bytes",
                    actual: aggregate,
                    max,
                });
                return Err(());
            }
            Ok(())
        });
        if let Some(error) = limit_error {
            return Err(error);
        }

        plan.resources = plan.resources.saturating_add(occurrences);
        plan.largest_bytes = plan.largest_bytes.max(resource_bytes);
        plan.total_bytes = plan
            .total_bytes
            .saturating_add(resource_bytes.saturating_mul(occurrences as u64));
    }
    export_checkpoint(control)?;
    Ok(plan)
}

pub(super) fn plan_embedded_images(
    tree: &usvg::Tree,
    limit: EmbeddedImageLimit,
    data: EmbeddedDataPlan,
    control: &OperationControl,
) -> Result<EmbeddedImagePlan> {
    validate_embedded_image_limit(limit)?;
    let mut plan = EmbeddedImagePlan {
        data_resources: data.resources,
        raster_images: 0,
        largest_data_bytes: data.largest_bytes,
        total_data_bytes: data.total_bytes,
        largest_raster_pixels: 0,
        total_pixels: 0,
    };
    plan_embedded_images_in_group(tree.root(), limit, &mut plan, control)?;
    Ok(plan)
}

fn plan_embedded_images_in_group(
    root: &usvg::Group,
    limit: EmbeddedImageLimit,
    plan: &mut EmbeddedImagePlan,
    control: &OperationControl,
) -> Result<()> {
    let mut stack = vec![root];
    while let Some(group) = stack.pop() {
        export_checkpoint(control)?;
        for node in group.children() {
            export_checkpoint(control)?;
            match node {
                usvg::Node::Group(child) => stack.push(child),
                usvg::Node::Image(image) => match image.kind() {
                    usvg::ImageKind::SVG(_) => {}
                    usvg::ImageKind::JPEG(_)
                    | usvg::ImageKind::PNG(_)
                    | usvg::ImageKind::GIF(_)
                    | usvg::ImageKind::WEBP(_) => {
                        let pixels = intrinsic_image_pixels(image.size())?;
                        plan.raster_images = plan.raster_images.saturating_add(1);
                        plan.largest_raster_pixels = plan.largest_raster_pixels.max(pixels);
                        plan.total_pixels = plan.total_pixels.saturating_add(pixels);
                        if let Some(max) = limit.max_pixels_per_image
                            && pixels > max
                        {
                            return Err(ExportError::EmbeddedImageLimit {
                                limit_name: "max_pixels_per_image",
                                actual: pixels,
                                max,
                            });
                        }
                        if let Some(max) = limit.max_total_pixels
                            && plan.total_pixels > max
                        {
                            return Err(ExportError::EmbeddedImageLimit {
                                limit_name: "max_total_pixels",
                                actual: plan.total_pixels,
                                max,
                            });
                        }
                    }
                },
                _ => {}
            }

            // SVG images, clip paths, masks, patterns, and filter image primitives can own
            // additional renderable trees. Walk those subroots as well so a hidden definition
            // cannot bypass the decode budget.
            let mut subroot_result = Ok(());
            node.subroots(|subroot| {
                if subroot_result.is_ok() {
                    subroot_result = plan_embedded_images_in_group(subroot, limit, plan, control);
                }
            });
            subroot_result?;
        }
    }
    Ok(())
}

fn intrinsic_image_pixels(size: usvg::Size) -> Result<u64> {
    let width = f64::from(size.width()).ceil();
    let height = f64::from(size.height()).ceil();
    if !(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0) {
        return Err(ExportError::InvalidSizing(
            "embedded image dimensions must be finite and positive",
        ));
    }
    if width > u64::MAX as f64 || height > u64::MAX as f64 {
        return Err(ExportError::InvalidSizing(
            "embedded image dimensions exceed the planner capability",
        ));
    }
    Ok((width as u64).saturating_mul(height as u64))
}

pub(super) fn validate_embedded_image_limit(limit: EmbeddedImageLimit) -> Result<()> {
    if limit.max_bytes_per_image == Some(0)
        || limit.max_total_bytes == Some(0)
        || limit.max_pixels_per_image == Some(0)
        || limit.max_total_pixels == Some(0)
    {
        return Err(ExportError::InvalidSizing(
            "embedded image byte and pixel limits must be positive",
        ));
    }
    Ok(())
}
