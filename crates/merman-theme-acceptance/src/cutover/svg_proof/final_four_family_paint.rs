use super::*;

const ZENUML_TITLE_SELECTOR: &str = ".frame-title";
const ZENUML_HEADER_FILL_SELECTOR: &str = ".frame-border-inner,.frame-header-bg";
const EVENT_MODELING_TEXT_SELECTOR: &str = ".em-swimlane text,.em-box span ";
const ISHIKAWA_HEAD_SELECTOR: &str = ".ishikawa .ishikawa-head ";
const ISHIKAWA_LABEL_BOX_SELECTOR: &str = ".ishikawa .ishikawa-label-box ";
const ISHIKAWA_BRANCH_SELECTOR: &str =
    ".ishikawa .ishikawa-spine,.ishikawa .ishikawa-branch,.ishikawa .ishikawa-sub-branch ";
const ISHIKAWA_TEXT_SELECTOR: &str = ".ishikawa text ";
const ISHIKAWA_HEAD_LABEL_SELECTOR: &str = ".ishikawa .ishikawa-head-label ";

pub(super) struct FinalFourRouteObservation {
    pub(super) value: String,
    pub(super) terminal_digest: [u8; 32],
    pub(super) target_regions: Vec<[f64; 4]>,
}

pub(super) fn final_four_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<FinalFourRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.facet() == ThemeRouteCutoverFacet::Fill
            && matches!(
                (route.family_id(), route.target()),
                (
                    DiagramFamilyId::ZENUML | DiagramFamilyId::VENN,
                    ThemeTarget::Title
                ) | (
                    DiagramFamilyId::ISHIKAWA | DiagramFamilyId::EVENT_MODELING,
                    ThemeTarget::Text
                )
            ),
        "unsupported final-four cutover route {}",
        route_label(route)
    );

    match route.family_id() {
        DiagramFamilyId::ZENUML => zenuml_title_observation(document),
        DiagramFamilyId::VENN => venn_title_observation(document),
        DiagramFamilyId::ISHIKAWA => ishikawa_text_observation(document),
        DiagramFamilyId::EVENT_MODELING => event_modeling_text_observation(document),
        family => Err(C6ProofError::new(
            "route-svg-proof",
            format!("unsupported final-four route family {family}"),
        )),
    }
}

pub(super) fn final_four_underlay_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    let root_underlay = root_underlay_color(document)?;
    let underlays = match route.family_id() {
        DiagramFamilyId::ZENUML => {
            let fill = parse_optional_css_rgb(stylesheet_property(
                document,
                ZENUML_HEADER_FILL_SELECTOR,
                "fill",
            )?)?;
            vec![underlay_colors([fill, root_underlay])]
        }
        DiagramFamilyId::VENN => vec![underlay_colors([root_underlay])],
        DiagramFamilyId::ISHIKAWA => ishikawa_underlay_colors(document, root_underlay)?,
        DiagramFamilyId::EVENT_MODELING => event_modeling_underlay_colors(document, root_underlay)?,
        family => {
            return Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported final-four underlay family {family}"),
            ));
        }
    };
    c6_ensure!(
        "route-svg-proof",
        underlays.len() == target_regions.len(),
        "{} underlay count differs from its terminal regions: {} != {}",
        route_label(route),
        underlays.len(),
        target_regions.len()
    );
    Ok(underlays)
}

fn zenuml_title_observation(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<FinalFourRouteObservation> {
    let value = stylesheet_property(document, ZENUML_TITLE_SELECTOR, "fill")?;
    let titles = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("frame-title"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        titles.len() == 1 && has_visible_text(titles[0]),
        "ZenUML witness must emit exactly one non-empty text.frame-title"
    );
    let title = titles[0];
    c6_ensure!(
        "route-svg-proof",
        title.attribute("fill").is_none() && style_value(title, "fill").is_none(),
        "ZenUML frame title bypassed its final stylesheet owner"
    );

    let inner = exact_descendant(document, "rect", "frame-border-inner", "ZenUML frame inner")?;
    let header_line =
        exact_descendant(document, "line", "frame-header-line", "ZenUML header line")?;
    let inner_bounds = raw_element_bounds(inner)?;
    let header_bounds = raw_element_bounds(header_line)?;
    c6_ensure!(
        "route-svg-geometry",
        (header_bounds.top - header_bounds.bottom).abs() <= f64::EPSILON
            && header_bounds.left >= inner_bounds.left
            && header_bounds.right <= inner_bounds.right
            && header_bounds.top > inner_bounds.top,
        "ZenUML header line is not bounded by the inner frame"
    );
    let region = rect_from_edges(
        inner_bounds.left,
        inner_bounds.top,
        inner_bounds.right,
        header_bounds.top,
        "ZenUML title header",
    )?;
    let title_anchor = transformed_text_anchor(title)?;
    c6_ensure!(
        "route-svg-geometry",
        point_in_rect(title_anchor, region),
        "ZenUML frame-title anchor is outside the emitted header surface"
    );

    let mut terminal = b"merman.c6-route-zenuml-title-fill.v1\0".to_vec();
    append_len_prefixed(&mut terminal, ZENUML_TITLE_SELECTOR.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());
    append_len_prefixed(&mut terminal, visible_text(title).as_bytes());
    append_len_prefixed(
        &mut terminal,
        title.attribute("class").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(
        &mut terminal,
        title
            .attribute("dominant-baseline")
            .unwrap_or_default()
            .as_bytes(),
    );
    append_rect(&mut terminal, region);
    append_point(&mut terminal, title_anchor);
    Ok(FinalFourRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions: vec![region],
    })
}

fn venn_title_observation(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<FinalFourRouteObservation> {
    let root = document.root_element();
    let root_id = root
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Venn SVG root lacks an id"))?;
    let selector = format!("#{root_id} .venn-title");
    let stylesheet_fill = stylesheet_property(document, &selector, "fill")?;
    let titles = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("venn-title"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        titles.len() == 1 && has_visible_text(titles[0]),
        "Venn witness must emit exactly one non-empty text.venn-title"
    );
    let title = titles[0];
    let inline_fill = style_value(title, "fill").ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            "Venn title lacks its inline terminal fill",
        )
    })?;
    c6_ensure!(
        "route-svg-proof",
        title.attribute("fill").is_none()
            && inline_fill == stylesheet_fill
            && title.attribute("x") == Some("50%")
            && title.attribute("text-anchor") == Some("middle")
            && title.attribute("dominant-baseline") == Some("middle"),
        "Venn title stylesheet, inline terminal, or anchor contract drifted"
    );

    let view_box =
        parse_svg_view_box(root.attribute("viewBox").ok_or_else(|| {
            C6ProofError::new("route-svg-proof", "Venn SVG root lacks a viewBox")
        })?)?;
    let y = number_attribute(title, "y")?;
    let content_groups = document
        .root_element()
        .children()
        .filter(|node| node.has_tag_name("g") && node.attribute("transform").is_some())
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-geometry",
        content_groups.len() == 1,
        "Venn witness expected one translated content group, found {}",
        content_groups.len()
    );
    let content_origin = node_transform(content_groups[0])?.apply(0.0, 0.0);
    c6_ensure!(
        "route-svg-geometry",
        content_origin.0.abs() <= f64::EPSILON
            && content_origin.1 > view_box[1]
            && content_origin.1 <= view_box[1] + view_box[3]
            && y >= view_box[1]
            && y <= content_origin.1,
        "Venn title anchor or content division is outside the root viewBox"
    );
    let region = rect_from_edges(
        view_box[0],
        view_box[1],
        view_box[0] + view_box[2],
        content_origin.1,
        "Venn title band",
    )?;

    let mut terminal = b"merman.c6-route-venn-title-fill.v1\0".to_vec();
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, stylesheet_fill.as_bytes());
    append_len_prefixed(&mut terminal, inline_fill.as_bytes());
    append_len_prefixed(&mut terminal, visible_text(title).as_bytes());
    append_len_prefixed(
        &mut terminal,
        title.attribute("font-size").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(
        &mut terminal,
        title.attribute("style").unwrap_or_default().as_bytes(),
    );
    append_rect(&mut terminal, region);
    Ok(FinalFourRouteObservation {
        value: inline_fill.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions: vec![region],
    })
}

fn event_modeling_text_observation(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<FinalFourRouteObservation> {
    let stylesheet_color = stylesheet_property(document, EVENT_MODELING_TEXT_SELECTOR, "color")?;
    let swimlanes = event_modeling_groups(document, "em-swimlane");
    let boxes = event_modeling_groups(document, "em-box");
    let box_fallbacks = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("data-merman-foreignobject") == Some("fallback")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "em-box")
                })
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        swimlanes.len() == 3 && boxes.len() == 3 && box_fallbacks.len() == 3,
        "Event Modeling witness expected three swimlanes, boxes, and resvg-safe text fallbacks, found {}, {}, and {}",
        swimlanes.len(),
        boxes.len(),
        box_fallbacks.len()
    );

    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(6);
    let mut terminal = b"merman.c6-route-event-modeling-text-fill.v1\0".to_vec();
    append_len_prefixed(&mut terminal, EVENT_MODELING_TEXT_SELECTOR.as_bytes());
    append_len_prefixed(&mut terminal, stylesheet_color.as_bytes());

    for (index, group) in swimlanes.into_iter().enumerate() {
        let rect = exact_direct_child(group, "rect", "Event Modeling swimlane rect")?;
        let text = exact_direct_child(group, "text", "Event Modeling swimlane text")?;
        c6_ensure!(
            "route-svg-proof",
            has_visible_text(text) && style_value(text, "fill").is_none(),
            "Event Modeling swimlane {index} lacks one non-empty SVG text terminal"
        );
        let value = text.attribute("fill").ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Event Modeling swimlane {index} lacks fill"),
            )
        })?;
        observe_common_value(&mut common_value, value, "Event Modeling text")?;
        c6_ensure!(
            "route-svg-proof",
            value == stylesheet_color,
            "Event Modeling swimlane {index} fill differs from its terminal CSS declaration"
        );
        let bounds = raw_element_bounds(rect)?;
        let anchor = transformed_text_anchor(text)?;
        c6_ensure!(
            "route-svg-geometry",
            bounds_contains_point(bounds, anchor),
            "Event Modeling swimlane {index} text anchor is outside its backing rect"
        );
        let region = bounds.rect(region_padding(ThemeRouteCutoverFacet::Fill))?;
        append_terminal_text(&mut terminal, "swimlane", index, text, value, region);
        target_regions.push(region);
    }

    for (index, (group, fallback)) in boxes.into_iter().zip(box_fallbacks.into_iter()).enumerate() {
        let rect = exact_direct_child(group, "rect", "Event Modeling box rect")?;
        let foreign_objects = group
            .descendants()
            .filter(|node| node.has_tag_name("foreignObject"))
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            foreign_objects.is_empty(),
            "Event Modeling box {index} retained {} foreignObject terminals after resvg-safe finalization",
            foreign_objects.len()
        );
        let texts = fallback
            .children()
            .filter(|node| node.has_tag_name("text"))
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            texts.len() == 1 && has_visible_text(texts[0]),
            "Event Modeling box {index} expected one non-empty resvg-safe text terminal"
        );
        let text = texts[0];
        c6_ensure!(
            "route-svg-proof",
            text.attribute("class").is_some_and(|classes| {
                let mut tokens = classes.split_ascii_whitespace();
                tokens.any(|class| class == "merman-foreignobject-fallback-text")
                    && classes
                        .split_ascii_whitespace()
                        .any(|class| class == "em-box")
            }),
            "Event Modeling box {index} fallback text lost its terminal role classes"
        );
        let value = text.attribute("fill").ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Event Modeling box {index} fallback text lacks fill"),
            )
        })?;
        observe_common_value(&mut common_value, value, "Event Modeling text")?;
        c6_ensure!(
            "route-svg-proof",
            value == stylesheet_color,
            "Event Modeling box {index} color differs from its terminal CSS declaration"
        );
        let rect_bounds = raw_element_bounds(rect)?;
        let anchor = transformed_text_anchor(text)?;
        c6_ensure!(
            "route-svg-geometry",
            bounds_contains_point(rect_bounds, anchor),
            "Event Modeling box {index} fallback text anchor is outside its backing rect"
        );
        let region = rect_bounds.rect(region_padding(ThemeRouteCutoverFacet::Fill))?;
        append_terminal_text(&mut terminal, "box", index, text, value, region);
        append_len_prefixed(
            &mut terminal,
            fallback
                .attribute("data-merman-foreignobject")
                .unwrap_or_default()
                .as_bytes(),
        );
        append_len_prefixed(
            &mut terminal,
            fallback.attribute("class").unwrap_or_default().as_bytes(),
        );
        append_point(&mut terminal, anchor);
        target_regions.push(region);
    }

    Ok(FinalFourRouteObservation {
        value: common_value
            .ok_or_else(|| C6ProofError::new("route-svg-proof", "missing Event Modeling text"))?
            .to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn ishikawa_text_observation(
    document: &roxmltree::Document<'_>,
) -> C6ProofResult<FinalFourRouteObservation> {
    const EXPECTED_TEXT: [(&str, &str); 6] = [
        ("ishikawa-head-label", "Root cause"),
        ("ishikawa-label cause", "Process"),
        ("ishikawa-label align", "Missing spec"),
        ("ishikawa-label align", "Slow step"),
        ("ishikawa-label cause", "People"),
        ("ishikawa-label align", "Missing owner"),
    ];
    let text_nodes = ishikawa_visible_text_nodes(document);
    c6_ensure!(
        "route-svg-proof",
        text_nodes.len() == EXPECTED_TEXT.len(),
        "Ishikawa witness expected six visible text terminals, found {}",
        text_nodes.len()
    );

    let mut common_value = None;
    let mut target_regions = Vec::with_capacity(text_nodes.len());
    let mut terminal = b"merman.c6-route-ishikawa-text-fill.v1\0".to_vec();
    for (index, (text, (expected_class, expected_text))) in
        text_nodes.into_iter().zip(EXPECTED_TEXT).enumerate()
    {
        let class = text.attribute("class").unwrap_or_default();
        let content = visible_text(text);
        c6_ensure!(
            "route-svg-proof",
            class == expected_class && content == expected_text && text.attribute("fill").is_none(),
            "Ishikawa terminal {index} class/text contract drifted"
        );
        let raw_style = text.attribute("style").ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Ishikawa terminal {index} lacks its typed inline style"),
            )
        })?;
        let value = style_value(text, "fill").ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Ishikawa terminal {index} lacks fill"),
            )
        })?;
        c6_ensure!(
            "route-svg-proof",
            inline_style_has_important_value(raw_style, "fill", value),
            "Ishikawa terminal {index} fill is not the writer-owned important declaration: value={value}, style={raw_style}"
        );
        observe_common_value(&mut common_value, value, "Ishikawa text")?;
        let (owner, surface, role) = ishikawa_owner_and_surface(text)?;
        let region = ishikawa_text_region(document, owner, text, role)?;
        terminal.extend_from_slice(&usize_to_u64(index).to_be_bytes());
        append_len_prefixed(&mut terminal, role.id().as_bytes());
        append_len_prefixed(&mut terminal, class.as_bytes());
        append_len_prefixed(&mut terminal, content.as_bytes());
        append_len_prefixed(&mut terminal, value.as_bytes());
        append_len_prefixed(&mut terminal, raw_style.as_bytes());
        append_len_prefixed(&mut terminal, surface.tag_name().name().as_bytes());
        append_len_prefixed(
            &mut terminal,
            surface.attribute("class").unwrap_or_default().as_bytes(),
        );
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }

    Ok(FinalFourRouteObservation {
        value: common_value
            .ok_or_else(|| C6ProofError::new("route-svg-proof", "missing Ishikawa text"))?
            .to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn event_modeling_underlay_colors(
    document: &roxmltree::Document<'_>,
    root_underlay: Option<[u8; 3]>,
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    event_modeling_groups(document, "em-swimlane")
        .into_iter()
        .chain(event_modeling_groups(document, "em-box"))
        .map(|group| {
            let rect = exact_direct_child(group, "rect", "Event Modeling backing rect")?;
            let fill = rect.attribute("fill").ok_or_else(|| {
                C6ProofError::new("route-svg-proof", "Event Modeling backing rect lacks fill")
            })?;
            Ok(underlay_colors([
                parse_optional_css_rgb(fill)?,
                root_underlay,
            ]))
        })
        .collect()
}

fn ishikawa_underlay_colors(
    document: &roxmltree::Document<'_>,
    root_underlay: Option<[u8; 3]>,
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    ishikawa_visible_text_nodes(document)
        .into_iter()
        .map(|text| {
            let (_, surface, role) = ishikawa_owner_and_surface(text)?;
            let mut colors = Vec::new();
            if surface.has_tag_name("g") {
                for geometry in surface.descendants().filter(|node| {
                    node.is_element()
                        && matches!(
                            node.tag_name().name(),
                            "rect"
                                | "circle"
                                | "ellipse"
                                | "line"
                                | "path"
                                | "polygon"
                                | "polyline"
                        )
                }) {
                    for property in ["fill", "stroke"] {
                        if let Some(raw) =
                            style_value(geometry, property).or_else(|| geometry.attribute(property))
                            && let Some(color) = parse_optional_css_rgb(raw)?
                        {
                            push_unique(&mut colors, color);
                        }
                    }
                }
            } else {
                let selector = role.surface_selector();
                for property in ["fill", "stroke"] {
                    if let Some(color) =
                        parse_optional_css_rgb(stylesheet_property(document, selector, property)?)?
                    {
                        push_unique(&mut colors, color);
                    }
                }
            }
            if let Some(root_underlay) = root_underlay {
                push_unique(&mut colors, root_underlay);
            }
            c6_ensure!(
                "route-svg-proof",
                !colors.is_empty(),
                "Ishikawa {} text lacks terminal underlay paint",
                role.id()
            );
            Ok(colors)
        })
        .collect()
}

fn root_underlay_color(document: &roxmltree::Document<'_>) -> C6ProofResult<Option<[u8; 3]>> {
    style_value(document.root_element(), "background-color")
        .map(parse_optional_css_rgb)
        .transpose()
        .map(Option::flatten)
}

fn underlay_colors<const N: usize>(colors: [Option<[u8; 3]>; N]) -> Vec<[u8; 3]> {
    let mut unique = Vec::with_capacity(N);
    for color in colors.into_iter().flatten() {
        push_unique(&mut unique, color);
    }
    unique
}

fn inline_style_has_important_value(style: &str, property: &str, expected: &str) -> bool {
    style.split(';').any(|declaration| {
        let Some((name, value)) = declaration.split_once(':') else {
            return false;
        };
        name.trim() == property
            && value
                .trim()
                .strip_suffix("!important")
                .is_some_and(|value| value.trim() == expected)
    })
}

#[derive(Clone, Copy)]
enum IshikawaTextRole {
    Head,
    Cause,
    Subgroup,
}

impl IshikawaTextRole {
    const fn id(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Cause => "cause",
            Self::Subgroup => "subgroup",
        }
    }

    const fn owner_class(self) -> &'static str {
        match self {
            Self::Head => "ishikawa-head-group",
            Self::Cause => "ishikawa-label-group",
            Self::Subgroup => "ishikawa-sub-group",
        }
    }

    const fn surface_class(self) -> &'static str {
        match self {
            Self::Head => "ishikawa-head",
            Self::Cause => "ishikawa-label-box",
            Self::Subgroup => "ishikawa-sub-branch",
        }
    }

    const fn surface_selector(self) -> &'static str {
        match self {
            Self::Head => ISHIKAWA_HEAD_SELECTOR,
            Self::Cause => ISHIKAWA_LABEL_BOX_SELECTOR,
            Self::Subgroup => ISHIKAWA_BRANCH_SELECTOR,
        }
    }
}

fn ishikawa_owner_and_surface<'a, 'input>(
    text: roxmltree::Node<'a, 'input>,
) -> C6ProofResult<(
    roxmltree::Node<'a, 'input>,
    roxmltree::Node<'a, 'input>,
    IshikawaTextRole,
)> {
    let role = match text.attribute("class") {
        Some("ishikawa-head-label") => IshikawaTextRole::Head,
        Some("ishikawa-label cause") => IshikawaTextRole::Cause,
        Some("ishikawa-label align") => IshikawaTextRole::Subgroup,
        class => {
            return Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported Ishikawa text class {class:?}"),
            ));
        }
    };
    let owner = text.parent().ok_or_else(|| {
        C6ProofError::new("route-svg-proof", "Ishikawa text lacks its terminal owner")
    })?;
    c6_ensure!(
        "route-svg-proof",
        owner.has_tag_name("g") && owner.attribute("class") == Some(role.owner_class()),
        "Ishikawa {} text is outside its canonical owner group",
        role.id()
    );
    let surfaces = owner
        .children()
        .filter(|node| node.is_element() && node.attribute("class") == Some(role.surface_class()))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        surfaces.len() == 1,
        "Ishikawa {} text expected one terminal surface, found {}",
        role.id(),
        surfaces.len()
    );
    let surface = surfaces[0];
    let valid_tag = match role {
        IshikawaTextRole::Head => matches!(surface.tag_name().name(), "path" | "g"),
        IshikawaTextRole::Cause => matches!(surface.tag_name().name(), "rect" | "g"),
        IshikawaTextRole::Subgroup => matches!(surface.tag_name().name(), "line" | "g"),
    };
    c6_ensure!(
        "route-svg-proof",
        valid_tag,
        "Ishikawa {} surface has unexpected element {}",
        role.id(),
        surface.tag_name().name()
    );
    Ok((owner, surface, role))
}

fn ishikawa_text_region(
    document: &roxmltree::Document<'_>,
    owner: roxmltree::Node<'_, '_>,
    text: roxmltree::Node<'_, '_>,
    role: IshikawaTextRole,
) -> C6ProofResult<[f64; 4]> {
    let geometry = group_bounds(owner, ThemeRouteCutoverFacet::Fill)?;
    let text_bounds = ishikawa_emitted_text_bounds(text)?;
    let anchor = transformed_text_anchor(text)?;
    const GEOMETRY_EPSILON: f64 = 1.0e-6;
    c6_ensure!(
        "route-svg-geometry",
        anchor.0 >= text_bounds[0] - GEOMETRY_EPSILON
            && anchor.0 <= text_bounds[0] + text_bounds[2] + GEOMETRY_EPSILON,
        "Ishikawa {} text anchor {:?} is outside measured bounds {:?}",
        role.id(),
        anchor,
        text_bounds
    );
    let left = geometry[0].min(text_bounds[0]);
    let top = geometry[1].min(text_bounds[1]);
    let right = (geometry[0] + geometry[2]).max(text_bounds[0] + text_bounds[2]);
    let bottom = (geometry[1] + geometry[3]).max(text_bounds[1] + text_bounds[3]);
    rect_from_edges(left, top, right, bottom, "Ishikawa text surface")
}

fn ishikawa_emitted_text_bounds(text: roxmltree::Node<'_, '_>) -> C6ProofResult<[f64; 4]> {
    let raw = text.attribute("data-merman-text-bbox").ok_or_else(|| {
        C6ProofError::new(
            "route-svg-geometry",
            "Ishikawa text lacks writer-owned measured bounds",
        )
    })?;
    let values = raw
        .split(',')
        .map(|value| {
            value.parse::<f64>().map_err(|error| {
                C6ProofError::new(
                    "route-svg-geometry",
                    format!("invalid Ishikawa text bounds value {value:?}: {error}"),
                )
            })
        })
        .collect::<C6ProofResult<Vec<_>>>()?;
    let [left, top, right, bottom] = values.as_slice() else {
        return Err(C6ProofError::new(
            "route-svg-geometry",
            format!("Ishikawa text bounds contain {} values", values.len()),
        ));
    };
    rect_from_edges(*left, *top, *right, *bottom, "Ishikawa text bounds")
}

fn ishikawa_visible_text_nodes<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && matches!(
                    node.attribute("class"),
                    Some("ishikawa-head-label" | "ishikawa-label cause" | "ishikawa-label align")
                )
                && has_visible_text(*node)
        })
        .collect()
}

fn event_modeling_groups<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    class: &str,
) -> Vec<roxmltree::Node<'a, 'input>> {
    document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some(class))
        .collect()
}

fn exact_descendant<'a, 'input>(
    document: &'a roxmltree::Document<'input>,
    tag: &str,
    class: &str,
    label: &str,
) -> C6ProofResult<roxmltree::Node<'a, 'input>> {
    let nodes = document
        .descendants()
        .filter(|node| node.has_tag_name(tag) && node.attribute("class") == Some(class))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        nodes.len() == 1,
        "{label} expected one {tag}.{class}, found {}",
        nodes.len()
    );
    Ok(nodes[0])
}

fn exact_direct_child<'a, 'input>(
    parent: roxmltree::Node<'a, 'input>,
    tag: &str,
    label: &str,
) -> C6ProofResult<roxmltree::Node<'a, 'input>> {
    let nodes = parent
        .children()
        .filter(|node| node.has_tag_name(tag))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        nodes.len() == 1,
        "{label} expected one direct {tag}, found {}",
        nodes.len()
    );
    Ok(nodes[0])
}

fn observe_common_value<'a>(
    common: &mut Option<&'a str>,
    value: &'a str,
    label: &str,
) -> C6ProofResult<()> {
    if let Some(previous) = *common {
        c6_ensure!(
            "route-svg-proof",
            previous == value,
            "{label} terminals disagree: `{previous}` != `{value}`"
        );
    } else {
        *common = Some(value);
    }
    Ok(())
}

fn append_terminal_text(
    terminal: &mut Vec<u8>,
    role: &str,
    index: usize,
    text: roxmltree::Node<'_, '_>,
    value: &str,
    region: [f64; 4],
) {
    append_len_prefixed(terminal, role.as_bytes());
    terminal.extend_from_slice(&usize_to_u64(index).to_be_bytes());
    append_len_prefixed(terminal, visible_text(text).as_bytes());
    append_len_prefixed(terminal, value.as_bytes());
    append_len_prefixed(
        terminal,
        text.attribute("style").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(
        terminal,
        text.attribute("fill").unwrap_or_default().as_bytes(),
    );
    append_rect(terminal, region);
}

fn visible_text(node: roxmltree::Node<'_, '_>) -> String {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|node| node.text())
        .collect()
}

fn has_visible_text(node: roxmltree::Node<'_, '_>) -> bool {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|node| node.text())
        .any(|text| !text.trim().is_empty())
}

fn transformed_text_anchor(node: roxmltree::Node<'_, '_>) -> C6ProofResult<(f64, f64)> {
    let x = number_attribute_or(node, "x", 0.0)?;
    let y = number_attribute_or(node, "y", 0.0)?;
    Ok(node_transform(node)?.apply(x, y))
}

fn rect_like_bounds(node: roxmltree::Node<'_, '_>) -> C6ProofResult<SvgBounds> {
    let x = number_attribute_or(node, "x", 0.0)?;
    let y = number_attribute_or(node, "y", 0.0)?;
    let width = number_attribute(node, "width")?;
    let height = number_attribute(node, "height")?;
    c6_ensure!(
        "route-svg-geometry",
        width > 0.0 && height > 0.0,
        "{} geometry must be positive",
        node.tag_name().name()
    );
    let transform = node_transform(node)?;
    let mut bounds = SvgBounds::from_point(transform.apply(x, y));
    bounds.include(transform.apply(x + width, y));
    bounds.include(transform.apply(x, y + height));
    bounds.include(transform.apply(x + width, y + height));
    Ok(bounds)
}

fn bounds_contains(outer: SvgBounds, inner: SvgBounds) -> bool {
    inner.left >= outer.left
        && inner.top >= outer.top
        && inner.right <= outer.right
        && inner.bottom <= outer.bottom
}

fn bounds_contains_point(bounds: SvgBounds, point: (f64, f64)) -> bool {
    point.0 >= bounds.left
        && point.0 <= bounds.right
        && point.1 >= bounds.top
        && point.1 <= bounds.bottom
}

fn point_in_rect(point: (f64, f64), rect: [f64; 4]) -> bool {
    point.0 >= rect[0]
        && point.0 <= rect[0] + rect[2]
        && point.1 >= rect[1]
        && point.1 <= rect[1] + rect[3]
}

fn rect_from_edges(
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    label: &str,
) -> C6ProofResult<[f64; 4]> {
    c6_ensure!(
        "route-svg-geometry",
        [left, top, right, bottom].into_iter().all(f64::is_finite) && right > left && bottom > top,
        "{label} bounds must be finite and positive"
    );
    Ok([left, top, right - left, bottom - top])
}

fn parse_css_dimension(raw: &str, label: &str) -> C6ProofResult<f64> {
    let number = raw.trim().strip_suffix("px").unwrap_or(raw.trim());
    let value = parse_geometry_number(number, label)?;
    c6_ensure!(
        "route-svg-geometry",
        value > 0.0,
        "{label} must be positive"
    );
    Ok(value)
}

fn append_point(output: &mut Vec<u8>, point: (f64, f64)) {
    output.extend_from_slice(&point.0.to_bits().to_be_bytes());
    output.extend_from_slice(&point.1.to_bits().to_be_bytes());
}

fn push_unique(colors: &mut Vec<[u8; 3]>, color: [u8; 3]) {
    if !colors.contains(&color) {
        colors.push(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn venn_title_observer_rejects_stylesheet_inline_drift() {
        let document = roxmltree::Document::parse(
            r##"<svg id="fixture" viewBox="0 0 200 120">
<style>#fixture .venn-title{fill:#dc2626;}</style>
<text class="venn-title" font-size="32px" text-anchor="middle" dominant-baseline="middle" x="50%" y="32" style="fill: #abcdef;">Title</text>
</svg>"##,
        )
        .expect("parse Venn mutation fixture");

        assert!(venn_title_observation(&document).is_err());
    }

    #[test]
    fn zenuml_title_observer_rejects_duplicate_terminal_text() {
        let document = roxmltree::Document::parse(
            r##"<svg id="fixture" viewBox="0 0 200 120">
<style>.frame-border-inner,.frame-header-bg{fill:#fff}.frame-title{fill:#dc2626}</style>
<rect class="frame-border-inner" x="1" y="1" width="198" height="118"/>
<line class="frame-header-line" x1="1" y1="33.5" x2="199" y2="33.5"/>
<text x="5" y="16.75" dominant-baseline="central" class="frame-title">One</text>
<text x="5" y="16.75" dominant-baseline="central" class="frame-title">Two</text>
</svg>"##,
        )
        .expect("parse ZenUML mutation fixture");

        assert!(zenuml_title_observation(&document).is_err());
    }
}
