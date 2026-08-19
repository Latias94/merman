use super::*;

pub(super) fn prove_terminal_png_pair(
    solid: &RenderedCutoverCase,
    transparent: &RenderedCutoverCase,
) -> C6ProofResult<BTreeMap<ThemeRouteCutoverDescriptor, [u8; 32]>> {
    let solid_route = solid
        .routes
        .first()
        .copied()
        .ok_or_else(|| C6ProofError::new("route-png-proof", "solid case has no route"))?;
    let transparent_route = transparent
        .routes
        .first()
        .copied()
        .ok_or_else(|| C6ProofError::new("route-png-proof", "transparent case has no route"))?;
    c6_ensure!(
        "route-png-proof",
        solid.routes.len() == 1
            && transparent.routes.len() == 1
            && route_shape(solid_route) == route_shape(transparent_route)
            && solid_route.value() == ThemeRouteCutoverValue::Solid
            && transparent_route.value() == ThemeRouteCutoverValue::Transparent,
        "PNG control pair does not describe one route solid/transparent witness"
    );
    c6_ensure!(
        "route-png-proof",
        solid.png_target_receipt_digest != transparent.png_target_receipt_digest,
        "solid and transparent PNG target receipts are identical"
    );
    let solid_dimensions = solid.raster.dimensions();
    let transparent_dimensions = transparent.raster.dimensions();
    c6_ensure!(
        "route-png-proof",
        solid_dimensions == transparent_dimensions,
        "solid and transparent PNG dimensions differ: {solid_dimensions:?} != {transparent_dimensions:?}"
    );

    let solid_shapes = route_shapes(&solid.routes);
    let transparent_shapes = route_shapes(&transparent.routes);
    c6_ensure!(
        "route-png-proof",
        solid_shapes == transparent_shapes,
        "solid and transparent route shapes differ"
    );
    c6_ensure!(
        "route-png-proof",
        solid.svg_view_box == transparent.svg_view_box
            && solid.target_regions == transparent.target_regions
            && solid.target_underlay_colors == transparent.target_underlay_colors
            && !solid.target_regions.is_empty(),
        "solid and transparent SVG target geometry differs"
    );
    let isolated_marker_contract = solid_route
        .projections()
        .contains(ThemeRouteCutoverProjection::MarkerPaintFromEdge)
        && matches!(
            solid_route.family_id(),
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        );
    let marker_assertion = if isolated_marker_contract {
        Some(prove_terminal_marker_png_pair(solid, transparent)?)
    } else {
        c6_ensure!(
            "route-marker-png",
            solid.markers.is_empty() && transparent.markers.is_empty(),
            "non-isolated Marker route unexpectedly exposed Flowchart Marker receipts"
        );
        None
    };
    let marker_exclusions = solid
        .markers
        .iter()
        .map(|marker| marker.probe_rect)
        .collect::<Vec<_>>();

    let mut assertions = BTreeMap::new();
    for route in solid.routes.iter().chain(&transparent.routes).copied() {
        let color = route_control_color(route)?;
        let transparent_count = transparent
            .raster
            .count_opaque_pixels_near(color.rgb, COLOR_TOLERANCE);
        c6_ensure!(
            "route-png-proof",
            transparent_count <= MAX_TRANSPARENT_CONTROL_PIXELS,
            "{} transparent PNG retained {transparent_count} solid-control pixels",
            route_label(route)
        );
        let mut value = b"merman.c6-route-png-assertion.v5\0".to_vec();
        append_route(&mut value, route);
        value.extend_from_slice(&solid.png_target_receipt_digest);
        value.extend_from_slice(&transparent.png_target_receipt_digest);
        value.extend_from_slice(&color.rgb);
        value.extend_from_slice(&usize_to_u64(transparent_count).to_be_bytes());
        let global_exclusions = if marker_assertion.is_some() {
            marker_exclusions.as_slice()
        } else {
            &[]
        };
        value.extend_from_slice(&usize_to_u64(global_exclusions.len()).to_be_bytes());
        for exclusion in global_exclusions {
            append_rect(&mut value, *exclusion);
        }
        for (index, region) in solid.target_regions.iter().copied().enumerate() {
            let exclusions =
                route_region_exclusions(route, &solid.target_regions, index, global_exclusions)?;
            let solid_count = solid
                .raster
                .count_opaque_pixels_near_in_svg_rect(
                    solid.svg_view_box,
                    region,
                    color.rgb,
                    COLOR_TOLERANCE,
                )
                .ok_or_else(|| {
                    C6ProofError::new("route-png-proof", "invalid solid target region")
                })?;
            let transparent_region_count = transparent
                .raster
                .count_opaque_pixels_near_in_svg_rect(
                    transparent.svg_view_box,
                    region,
                    color.rgb,
                    COLOR_TOLERANCE,
                )
                .ok_or_else(|| {
                    C6ProofError::new("route-png-proof", "invalid transparent target region")
                })?;
            let (masked_control_pixels, non_transparent_pixels_at_control_mask) = transparent
                .raster
                .control_mask_alpha_counts_in_svg_rect(
                    &solid.raster,
                    solid.svg_view_box,
                    region,
                    color.rgb,
                    COLOR_TOLERANCE,
                    TRANSPARENT_ALPHA_TOLERANCE,
                    exclusions,
                )
                .ok_or_else(|| {
                    C6ProofError::new(
                        "route-png-proof",
                        "solid and transparent local control masks differ",
                    )
                })?;
            let minimum = minimum_control_pixels_per_region(route);
            c6_ensure!(
                "route-png-proof",
                solid_count >= masked_control_pixels && masked_control_pixels >= minimum,
                "{} region {index} retained {solid_count} local solid pixels and {masked_control_pixels} non-Marker mask pixels; minimum={minimum}",
                route_label(route)
            );
            c6_ensure!(
                "route-png-proof",
                transparent_region_count <= MAX_TRANSPARENT_CONTROL_PIXELS,
                "{} region {index} retained {transparent_region_count} solid-control pixels in transparent output",
                route_label(route)
            );
            let mut underlay_pixels = 0usize;
            let mut unexpected_opaque_pixels = 0usize;
            let underlay_colors = &transparent.target_underlay_colors[index];
            let underlay_matches = underlay_colors
                .iter()
                .copied()
                .map(|color| (color, COLOR_TOLERANCE))
                .collect::<Vec<_>>();
            if route.facet() == ThemeRouteCutoverFacet::Fill {
                let maximum_non_transparent = masked_control_pixels
                    .saturating_mul(MAX_NON_TRANSPARENT_MASK_PERCENT)
                    .div_ceil(100)
                    .max(MIN_NON_TRANSPARENT_MASK_ALLOWANCE);
                if !underlay_matches.is_empty() {
                    let (underlay_control_pixels, underlay_pixels_in_region) = transparent
                        .raster
                        .control_mask_colors_counts_in_svg_rect(
                            &solid.raster,
                            solid.svg_view_box,
                            region,
                            (color.rgb, COLOR_TOLERANCE),
                            &underlay_matches,
                            exclusions,
                        )
                        .ok_or_else(|| {
                            C6ProofError::new("route-png-proof", "invalid underlay target region")
                        })?;
                    c6_ensure!(
                        "route-png-proof",
                        underlay_control_pixels == masked_control_pixels,
                        "{} region {index} underlay mask differs: {underlay_control_pixels} != {masked_control_pixels}",
                        route_label(route)
                    );
                    underlay_pixels = underlay_pixels_in_region;
                    if is_sparse_terminal_reveal(route) {
                        c6_ensure!(
                            "route-png-proof",
                            underlay_pixels_in_region != 0
                                && underlay_pixels_in_region
                                    == non_transparent_pixels_at_control_mask,
                            "{} region {index} retained {non_transparent_pixels_at_control_mask} non-transparent pixels but only {underlay_pixels_in_region} matched its terminal reveal layers",
                            route_label(route)
                        );
                    } else if requires_exact_terminal_reveal(route) {
                        c6_ensure!(
                            "route-png-proof",
                            non_transparent_pixels_at_control_mask == underlay_pixels_in_region,
                            "{} region {index} retained {non_transparent_pixels_at_control_mask} non-transparent control-mask pixels but only {underlay_pixels_in_region} matched its verified terminal underlay {underlay_colors:?}",
                            route_label(route)
                        );
                    } else {
                        c6_ensure!(
                            "route-png-proof",
                            underlay_pixels_in_region.saturating_mul(100)
                                >= masked_control_pixels.saturating_mul(MIN_UNDERLAY_MASK_PERCENT),
                            "{} region {index} did not retain the rendered underlay: {underlay_pixels_in_region}/{masked_control_pixels} pixels",
                            route_label(route)
                        );
                    }
                } else if requires_exact_terminal_reveal(route) {
                    require_exact_terminal_reveal_mask(non_transparent_pixels_at_control_mask, 0)?;
                } else {
                    c6_ensure!(
                        "route-png-proof",
                        non_transparent_pixels_at_control_mask <= maximum_non_transparent,
                        "{} region {index} retained {non_transparent_pixels_at_control_mask} non-transparent pixels at control positions; maximum={maximum_non_transparent}",
                        route_label(route)
                    );
                }
            } else {
                if !underlay_matches.is_empty() {
                    let (underlay_control_pixels, underlay_pixels_in_region) = transparent
                        .raster
                        .control_mask_colors_counts_in_svg_rect(
                            &solid.raster,
                            solid.svg_view_box,
                            region,
                            (color.rgb, COLOR_TOLERANCE),
                            &underlay_matches,
                            exclusions,
                        )
                        .ok_or_else(|| {
                            C6ProofError::new("route-png-proof", "invalid stroke underlay region")
                        })?;
                    c6_ensure!(
                        "route-png-proof",
                        underlay_control_pixels == masked_control_pixels,
                        "{} region {index} stroke underlay mask differs: {underlay_control_pixels} != {masked_control_pixels}",
                        route_label(route)
                    );
                    underlay_pixels = underlay_pixels_in_region;
                }
                unexpected_opaque_pixels = require_transparent_stroke_mask_for_region(
                    route,
                    index,
                    masked_control_pixels,
                    non_transparent_pixels_at_control_mask,
                    underlay_pixels,
                )?;
            }
            append_rect(&mut value, region);
            value.extend_from_slice(&usize_to_u64(exclusions.len()).to_be_bytes());
            for exclusion in exclusions {
                append_rect(&mut value, *exclusion);
            }
            value.extend_from_slice(&usize_to_u64(solid_count).to_be_bytes());
            value.extend_from_slice(&usize_to_u64(transparent_region_count).to_be_bytes());
            value.extend_from_slice(&usize_to_u64(masked_control_pixels).to_be_bytes());
            value.extend_from_slice(
                &usize_to_u64(non_transparent_pixels_at_control_mask).to_be_bytes(),
            );
            value.extend_from_slice(&usize_to_u64(underlay_colors.len()).to_be_bytes());
            for underlay in underlay_colors {
                value.extend_from_slice(underlay);
            }
            value.extend_from_slice(&usize_to_u64(underlay_pixels).to_be_bytes());
            value.extend_from_slice(&usize_to_u64(unexpected_opaque_pixels).to_be_bytes());
        }
        value.extend_from_slice(&solid_dimensions.0.to_be_bytes());
        value.extend_from_slice(&solid_dimensions.1.to_be_bytes());
        value.extend_from_slice(&transparent_dimensions.0.to_be_bytes());
        value.extend_from_slice(&transparent_dimensions.1.to_be_bytes());
        if let Some(marker_assertion) = marker_assertion {
            value.extend_from_slice(&marker_assertion);
        }
        assertions.insert(route, sha256(value));
    }
    Ok(assertions)
}

fn route_region_exclusions<'a>(
    route: ThemeRouteCutoverDescriptor,
    target_regions: &'a [[f64; 4]],
    index: usize,
    global_exclusions: &'a [[f64; 4]],
) -> C6ProofResult<&'a [[f64; 4]]> {
    if !global_exclusions.is_empty() {
        return Ok(global_exclusions);
    }
    if route.family_id() == DiagramFamilyId::EVENT_MODELING
        && route.target() == ThemeTarget::Text
        && route.facet() == ThemeRouteCutoverFacet::Fill
    {
        let swimlane_count = EVENT_MODELING_SWIMLANE_OCCURRENCE_COUNT;
        c6_ensure!(
            "route-png-proof",
            target_regions.len() == swimlane_count * 2,
            "Event Modeling Text PNG witness expected {swimlane_count} swimlane and {swimlane_count} box regions, found {}",
            target_regions.len()
        );
        if index < swimlane_count {
            let box_index = swimlane_count + index;
            return Ok(&target_regions[box_index..box_index + 1]);
        }
        return Ok(&[]);
    }
    if route.family_id() != DiagramFamilyId::CLASS
        || route.target() != ThemeTarget::Edge
        || !route
            .projections()
            .contains(ThemeRouteCutoverProjection::MarkerPaintFromEdge)
    {
        return Ok(&[]);
    }

    let relation_count = CLASS_RELATION_OCCURRENCE_COUNT;
    c6_ensure!(
        "route-png-proof",
        target_regions.len() == relation_count * 2,
        "Class Edge PNG witness expected {relation_count} relation and {relation_count} marker regions, found {}",
        target_regions.len()
    );
    if index < relation_count {
        let marker_index = relation_count + index;
        Ok(&target_regions[marker_index..marker_index + 1])
    } else {
        Ok(&[])
    }
}

fn is_sparse_terminal_reveal(route: ThemeRouteCutoverDescriptor) -> bool {
    route.family_id() == DiagramFamilyId::SEQUENCE
        && route.target() == ThemeTarget::Loop
        && route.facet() == ThemeRouteCutoverFacet::Fill
}

fn requires_exact_terminal_reveal(route: ThemeRouteCutoverDescriptor) -> bool {
    route.facet() == ThemeRouteCutoverFacet::Fill
        && matches!(
            (route.family_id(), route.target()),
            (
                DiagramFamilyId::ZENUML | DiagramFamilyId::VENN,
                ThemeTarget::Title
            ) | (
                DiagramFamilyId::ISHIKAWA | DiagramFamilyId::EVENT_MODELING,
                ThemeTarget::Text
            ) | (DiagramFamilyId::GANTT, ThemeTarget::Task)
        )
}

fn require_exact_terminal_reveal_mask(
    non_transparent_pixels: usize,
    underlay_pixels: usize,
) -> C6ProofResult<()> {
    c6_ensure!(
        "route-png-proof",
        non_transparent_pixels == underlay_pixels,
        "transparent terminal retained {non_transparent_pixels} non-transparent control-mask pixels but only {underlay_pixels} matched its verified terminal underlay"
    );
    Ok(())
}

fn require_transparent_stroke_mask(
    masked_control_pixels: usize,
    non_transparent_pixels: usize,
    underlay_pixels: usize,
) -> C6ProofResult<usize> {
    c6_ensure!(
        "route-png-proof",
        underlay_pixels <= non_transparent_pixels,
        "transparent Stroke underlay pixels exceed its non-transparent control mask"
    );
    let unexpected_opaque_pixels = non_transparent_pixels - underlay_pixels;
    let maximum_unexpected = masked_control_pixels
        .saturating_mul(MAX_NON_TRANSPARENT_MASK_PERCENT)
        .div_ceil(100);
    c6_ensure!(
        "route-png-proof",
        unexpected_opaque_pixels <= maximum_unexpected,
        "transparent Stroke retained {unexpected_opaque_pixels} unexpected opaque pixels at {masked_control_pixels} control positions; maximum={maximum_unexpected}"
    );
    Ok(unexpected_opaque_pixels)
}

fn require_transparent_stroke_mask_for_region(
    route: ThemeRouteCutoverDescriptor,
    index: usize,
    masked_control_pixels: usize,
    non_transparent_pixels: usize,
    underlay_pixels: usize,
) -> C6ProofResult<usize> {
    c6_ensure!(
        "route-png-proof",
        underlay_pixels <= non_transparent_pixels,
        "{} region {index} transparent Stroke underlay pixels exceed its non-transparent control mask: {underlay_pixels} > {non_transparent_pixels}",
        route_label(route)
    );
    let unexpected_opaque_pixels = non_transparent_pixels - underlay_pixels;
    let maximum_unexpected = masked_control_pixels
        .saturating_mul(MAX_NON_TRANSPARENT_MASK_PERCENT)
        .div_ceil(100);
    c6_ensure!(
        "route-png-proof",
        unexpected_opaque_pixels <= maximum_unexpected,
        "{} region {index} transparent Stroke retained {unexpected_opaque_pixels} unexpected opaque pixels at {masked_control_pixels} control positions with {underlay_pixels} verified underlay pixels; maximum={maximum_unexpected}",
        route_label(route)
    );
    Ok(unexpected_opaque_pixels)
}

fn prove_terminal_marker_png_pair(
    solid: &RenderedCutoverCase,
    transparent: &RenderedCutoverCase,
) -> C6ProofResult<[u8; 32]> {
    c6_ensure!(
        "route-marker-png",
        !solid.markers.is_empty() && solid.markers.len() == transparent.markers.len(),
        "solid and transparent cases expose different Marker receipt counts"
    );
    let mut assertion = b"merman.c6-route-marker-png.v2\0".to_vec();
    for (index, (solid_marker, transparent_marker)) in
        solid.markers.iter().zip(&transparent.markers).enumerate()
    {
        c6_ensure!(
            "route-marker-png",
            solid_marker.geometry_digest == transparent_marker.geometry_digest
                && solid_marker.view_box == transparent_marker.view_box
                && solid_marker.probe_rect == transparent_marker.probe_rect
                && solid_marker.fill == transparent_marker.fill
                && solid_marker.stroke == transparent_marker.stroke,
            "solid and transparent Edge cases emitted different Marker identities"
        );

        let solid_count = solid
            .raster
            .count_opaque_pixels_near_in_svg_rect(
                solid_marker.view_box,
                solid_marker.probe_rect,
                DEFAULT_MARKER.rgb,
                COLOR_TOLERANCE,
            )
            .ok_or_else(|| C6ProofError::new("route-marker-png", "invalid solid Marker probe"))?;
        let transparent_count = transparent
            .raster
            .count_opaque_pixels_near_in_svg_rect(
                transparent_marker.view_box,
                transparent_marker.probe_rect,
                DEFAULT_MARKER.rgb,
                COLOR_TOLERANCE,
            )
            .ok_or_else(|| {
                C6ProofError::new("route-marker-png", "invalid transparent Marker probe")
            })?;
        require_marker_pixel_counts(solid_count, transparent_count)?;
        assertion.extend_from_slice(&usize_to_u64(index).to_be_bytes());
        assertion.extend_from_slice(&solid_marker.digest);
        assertion.extend_from_slice(&transparent_marker.digest);
        assertion.extend_from_slice(&usize_to_u64(solid_count).to_be_bytes());
        assertion.extend_from_slice(&usize_to_u64(transparent_count).to_be_bytes());
    }
    Ok(sha256(assertion))
}

pub(super) fn require_marker_pixel_counts(
    solid_count: usize,
    transparent_count: usize,
) -> C6ProofResult<()> {
    c6_ensure!(
        "route-marker-png",
        solid_count >= MINIMUM_MARKER_PIXELS,
        "solid Edge Marker retained only {solid_count} default-paint pixels; minimum={MINIMUM_MARKER_PIXELS}"
    );
    c6_ensure!(
        "route-marker-png",
        transparent_count >= MINIMUM_MARKER_PIXELS,
        "transparent Edge Marker retained only {transparent_count} default-paint pixels; minimum={MINIMUM_MARKER_PIXELS}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{require_exact_terminal_reveal_mask, require_transparent_stroke_mask};

    #[test]
    fn exact_terminal_reveal_rejects_unknown_opaque_pixels() {
        assert!(require_exact_terminal_reveal_mask(1, 0).is_err());
        assert!(require_exact_terminal_reveal_mask(32, 31).is_err());
        assert!(require_exact_terminal_reveal_mask(0, 0).is_ok());
        assert!(require_exact_terminal_reveal_mask(32, 32).is_ok());
    }

    #[test]
    fn transparent_stroke_rejects_an_opaque_replacement_color() {
        assert!(require_transparent_stroke_mask(32, 32, 0).is_err());
    }

    #[test]
    fn transparent_stroke_accepts_transparency_or_the_verified_underlay() {
        assert_eq!(
            require_transparent_stroke_mask(32, 0, 0).expect("transparent stroke"),
            0
        );
        assert_eq!(
            require_transparent_stroke_mask(32, 32, 32).expect("underlay stroke"),
            0
        );
        assert_eq!(
            require_transparent_stroke_mask(32, 2, 0).expect("anti-alias allowance"),
            2
        );
        assert!(require_transparent_stroke_mask(32, 3, 0).is_err());
    }
}
