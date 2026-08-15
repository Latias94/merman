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
    let marker_assertion = match solid_route.target() {
        ThemeTarget::Edge => Some(prove_terminal_marker_png_pair(solid, transparent)?),
        _ => None,
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
        let exclusions = if route.target() == ThemeTarget::Edge {
            marker_exclusions.as_slice()
        } else {
            &[]
        };
        value.extend_from_slice(&usize_to_u64(exclusions.len()).to_be_bytes());
        for exclusion in exclusions {
            append_rect(&mut value, *exclusion);
        }
        for (index, region) in solid.target_regions.iter().copied().enumerate() {
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
                    c6_ensure!(
                        "route-png-proof",
                        underlay_pixels_in_region.saturating_mul(100)
                            >= masked_control_pixels.saturating_mul(MIN_UNDERLAY_MASK_PERCENT),
                        "{} region {index} did not retain the rendered underlay: {underlay_pixels_in_region}/{masked_control_pixels} pixels",
                        route_label(route)
                    );
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
                unexpected_opaque_pixels = require_transparent_stroke_mask(
                    masked_control_pixels,
                    non_transparent_pixels_at_control_mask,
                    underlay_pixels,
                )?;
            }
            append_rect(&mut value, region);
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
        if matches!(route.target(), ThemeTarget::Edge) {
            value.extend_from_slice(marker_assertion.as_ref().ok_or_else(|| {
                C6ProofError::new(
                    "route-marker-png",
                    format!("{} lacks its Marker PNG receipt", route_label(route)),
                )
            })?);
        }
        assertions.insert(route, sha256(value));
    }
    Ok(assertions)
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
    use super::require_transparent_stroke_mask;

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
