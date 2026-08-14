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
        solid.png_artifact_digest != transparent.png_artifact_digest,
        "solid and transparent PNG artifacts are identical"
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
        let mut value = b"merman.c6-route-png-assertion.v2\0".to_vec();
        append_route(&mut value, route);
        value.extend_from_slice(&solid.png_artifact_digest);
        value.extend_from_slice(&transparent.png_artifact_digest);
        value.extend_from_slice(&color.rgb);
        value.extend_from_slice(&usize_to_u64(transparent_count).to_be_bytes());
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
                solid_count >= minimum && masked_control_pixels == solid_count,
                "{} region {index} retained {solid_count} solid pixels and {masked_control_pixels} mask pixels; minimum={minimum}",
                route_label(route)
            );
            c6_ensure!(
                "route-png-proof",
                transparent_region_count <= MAX_TRANSPARENT_CONTROL_PIXELS,
                "{} region {index} retained {transparent_region_count} solid-control pixels in transparent output",
                route_label(route)
            );
            if route.facet() == ThemeRouteCutoverFacet::Fill {
                let maximum_non_transparent = masked_control_pixels
                    .saturating_mul(MAX_NON_TRANSPARENT_MASK_PERCENT)
                    .div_ceil(100)
                    .max(MIN_NON_TRANSPARENT_MASK_ALLOWANCE);
                if let Some(underlay) = transparent.target_underlay_colors[index] {
                    let (underlay_control_pixels, underlay_pixels) = transparent
                        .raster
                        .control_mask_color_counts_in_svg_rect(
                            &solid.raster,
                            solid.svg_view_box,
                            region,
                            (color.rgb, COLOR_TOLERANCE),
                            (underlay, COLOR_TOLERANCE),
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
                    c6_ensure!(
                        "route-png-proof",
                        underlay_pixels.saturating_mul(100)
                            >= masked_control_pixels.saturating_mul(MIN_UNDERLAY_MASK_PERCENT),
                        "{} region {index} did not retain the rendered underlay: {underlay_pixels}/{masked_control_pixels} pixels",
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
            }
            append_rect(&mut value, region);
            value.extend_from_slice(&usize_to_u64(solid_count).to_be_bytes());
            value.extend_from_slice(&usize_to_u64(transparent_region_count).to_be_bytes());
            value.extend_from_slice(&usize_to_u64(masked_control_pixels).to_be_bytes());
            value.extend_from_slice(
                &usize_to_u64(non_transparent_pixels_at_control_mask).to_be_bytes(),
            );
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
