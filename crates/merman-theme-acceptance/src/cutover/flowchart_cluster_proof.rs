use super::*;

pub(super) fn prove_flowchart_cluster_svg(
    case: CutoverCase,
    routes: &[ThemeRouteCutoverDescriptor],
    svg: &str,
) -> C6ProofResult<SvgCutoverProof> {
    let [route] = routes else {
        return Err(C6ProofError::new(
            "route-cluster-svg",
            format!("Cluster witness expected one route, found {}", routes.len()),
        ));
    };
    c6_ensure!(
        "route-cluster-svg",
        route.family_id() == DiagramFamilyId::FLOWCHART && route.target() == ThemeTarget::Cluster,
        "Cluster witness received {}",
        route_label(*route)
    );

    let document = roxmltree::Document::parse(svg)
        .map_err(|error| C6ProofError::new("route-cluster-svg", error.to_string()))?;
    let root = document.root_element();
    let view_box = parse_view_box(root.attribute("viewBox").ok_or_else(|| {
        C6ProofError::new("route-cluster-svg", "Flowchart SVG root lacks a viewBox")
    })?)?;
    let clusters = document
        .descendants()
        .filter(|node| node.is_element() && class_contains(*node, "cluster"))
        .filter(|node| !class_contains(*node, "cluster-label"))
        .collect::<Vec<_>>();
    let [cluster] = clusters.as_slice() else {
        return Err(C6ProofError::new(
            "route-cluster-svg",
            format!(
                "Cluster witness expected one terminal group, found {}",
                clusters.len()
            ),
        ));
    };
    c6_ensure!(
        "route-cluster-svg",
        cluster.attribute("data-look") == Some(case.id.profile().look()),
        "{} emitted Cluster look {:?}",
        witness_label(case.id),
        cluster.attribute("data-look")
    );

    let (surface, property) = match case.id.profile() {
        CutoverWitnessProfile::ClassicStatic => {
            let surfaces = cluster
                .children()
                .filter(|node| node.has_tag_name("rect"))
                .collect::<Vec<_>>();
            let [surface] = surfaces.as_slice() else {
                return Err(C6ProofError::new(
                    "route-cluster-svg",
                    format!(
                        "classic Cluster witness expected one direct rect, found {}",
                        surfaces.len()
                    ),
                ));
            };
            (*surface, facet_property(route.facet()))
        }
        CutoverWitnessProfile::HandDrawnStatic => {
            let surfaces = cluster
                .children()
                .find(|node| node.has_tag_name("g") && !class_contains(*node, "cluster-label"))
                .into_iter()
                .flat_map(|group| group.children().filter(|node| node.has_tag_name("path")))
                .collect::<Vec<_>>();
            let [fill, stroke] = surfaces.as_slice() else {
                return Err(C6ProofError::new(
                    "route-cluster-svg",
                    format!(
                        "hand-drawn Cluster witness expected two RoughJS paths, found {}",
                        surfaces.len()
                    ),
                ));
            };
            (
                match route.facet() {
                    ThemeRouteCutoverFacet::Fill => *fill,
                    ThemeRouteCutoverFacet::Stroke => *stroke,
                },
                "stroke",
            )
        }
        profile => {
            return Err(C6ProofError::new(
                "route-cluster-svg",
                format!("unsupported Cluster witness profile {}", profile.id()),
            ));
        }
    };

    let actual = style_value(surface, property)
        .or_else(|| surface.attribute(property))
        .ok_or_else(|| {
            C6ProofError::new(
                "route-cluster-svg",
                format!("Cluster terminal surface lacks {property}"),
            )
        })?;
    let expected = expected_svg_value(*route)?;
    c6_ensure!(
        "route-cluster-svg",
        actual == expected,
        "{} emitted `{actual}`, expected `{expected}`",
        route_label(*route)
    );

    let mut terminal = b"merman.c6-route-flowchart-cluster.v1\0".to_vec();
    append_witness(&mut terminal, case.id);
    append_len_prefixed(&mut terminal, surface.tag_name().name().as_bytes());
    append_len_prefixed(&mut terminal, property.as_bytes());
    append_len_prefixed(&mut terminal, actual.as_bytes());
    append_len_prefixed(
        &mut terminal,
        surface.attribute("d").unwrap_or_default().as_bytes(),
    );
    let assertion = sha256(terminal);

    let target_underlay_colors = match route.facet() {
        ThemeRouteCutoverFacet::Fill => Vec::new(),
        // The stroke control sits over Mermaid's source-backed default Cluster fill. The PNG
        // proof still rejects any other opaque replacement at those same control pixels.
        ThemeRouteCutoverFacet::Stroke => vec![[0xff, 0xff, 0xde]],
    };

    Ok(SvgCutoverProof {
        assertions: BTreeMap::from([(*route, assertion)]),
        view_box,
        // SVG proof binds the exact Cluster surface. PNG only confirms that the unique control
        // color appears and disappears in the same artifact, so it intentionally uses one coarse
        // document ROI instead of reimplementing Flowchart geometry.
        target_regions: vec![view_box],
        target_underlay_colors: vec![target_underlay_colors],
    })
}

fn parse_view_box(raw: &str) -> C6ProofResult<[f64; 4]> {
    let values = raw
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>().map_err(|error| {
                C6ProofError::new(
                    "route-cluster-svg",
                    format!("invalid viewBox value {part:?}: {error}"),
                )
            })
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let [left, top, width, height] = values.as_slice() else {
        return Err(C6ProofError::new(
            "route-cluster-svg",
            format!("Flowchart viewBox contains {} values", values.len()),
        ));
    };
    c6_ensure!(
        "route-cluster-svg",
        values.iter().all(|value| value.is_finite()) && *width > 0.0 && *height > 0.0,
        "Flowchart viewBox must be finite and positive"
    );
    Ok([*left, *top, *width, *height])
}
