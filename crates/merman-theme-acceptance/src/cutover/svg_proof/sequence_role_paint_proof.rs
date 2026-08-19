use super::*;

pub(super) fn sequence_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    match route.target() {
        ThemeTarget::Actor => sequence_actor_route_observation(document, route),
        ThemeTarget::ActorLabel => sequence_actor_label_route_observation(document, route),
        ThemeTarget::Lifeline => sequence_lifeline_route_observation(document, route),
        ThemeTarget::MessageLabel => sequence_message_label_route_observation(document, route),
        ThemeTarget::Loop => sequence_loop_route_observation(document, route),
        ThemeTarget::LoopLabel => sequence_loop_label_route_observation(document, route),
        ThemeTarget::NoteLabel => sequence_note_label_route_observation(document, route),
        ThemeTarget::Note | ThemeTarget::Activation => {
            sequence_static_rect_route_observation(document, route)
        }
        target => Err(C6ProofError::new(
            "route-svg-proof",
            format!("unsupported Sequence cutover target {}", target.id()),
        )),
    }
}

pub(super) fn sequence_underlay_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    c6_ensure!(
        "route-svg-proof",
        route.family_id() == DiagramFamilyId::SEQUENCE,
        "Sequence underlay proof received {}",
        route_label(route)
    );
    let region_count = target_regions.len();
    match (route.target(), route.facet()) {
        (
            ThemeTarget::Actor | ThemeTarget::Note | ThemeTarget::Activation,
            ThemeRouteCutoverFacet::Stroke,
        ) => {
            if route.target() == ThemeTarget::Activation {
                return sequence_activation_underlay_colors(document, region_count);
            }
            let root_id = sequence_root_id(document)?;
            let selector = match route.target() {
                ThemeTarget::Actor => format!("#{root_id} .actor"),
                ThemeTarget::Note => format!("#{root_id} .note"),
                _ => unreachable!("Activation returned above"),
            };
            let underlay =
                parse_optional_css_rgb(sequence_writer_property(document, &selector, "fill")?)?;
            Ok(std::iter::repeat_n(underlay.into_iter().collect(), region_count).collect())
        }
        (ThemeTarget::ActorLabel, ThemeRouteCutoverFacet::Fill) => {
            c6_ensure!(
                "route-svg-proof",
                region_count == 5,
                "Sequence ActorLabel witness expected four actor rows and one box-title row"
            );
            let root_id = sequence_root_id(document)?;
            let actor_underlay = parse_optional_css_rgb(sequence_writer_property(
                document,
                &format!("#{root_id} .actor"),
                "fill",
            )?)?;
            let box_title = document
                .descendants()
                .find(|node| node.has_tag_name("text") && class_contains(*node, "text"))
                .ok_or_else(|| {
                    C6ProofError::new(
                        "route-svg-proof",
                        "Sequence ActorLabel witness lacks its box title",
                    )
                })?;
            let box_surface = box_title
                .parent()
                .and_then(|group| {
                    group
                        .children()
                        .find(|node| node.has_tag_name("rect") && class_contains(*node, "rect"))
                })
                .ok_or_else(|| {
                    C6ProofError::new(
                        "route-svg-proof",
                        "Sequence ActorLabel box title lacks its terminal frame surface",
                    )
                })?;
            let box_underlay = style_value(box_surface, "fill")
                .or_else(|| box_surface.attribute("fill"))
                .map(parse_optional_css_rgb)
                .transpose()?
                .flatten();
            let mut underlays =
                std::iter::repeat_n(actor_underlay.into_iter().collect::<Vec<_>>(), 4)
                    .collect::<Vec<_>>();
            underlays.push(box_underlay.into_iter().collect());
            Ok(underlays)
        }
        (ThemeTarget::MessageLabel, ThemeRouteCutoverFacet::Fill) => {
            c6_ensure!(
                "route-svg-proof",
                region_count == 1,
                "Sequence MessageLabel witness expected one message-text region"
            );
            let box_groups = document
                .root_element()
                .children()
                .filter(|node| {
                    node.has_tag_name("g")
                        && node.children().any(|child| {
                            child.has_tag_name("text") && class_contains(child, "text")
                        })
                })
                .collect::<Vec<_>>();
            c6_ensure!(
                "route-svg-proof",
                box_groups.len() == 1,
                "Sequence MessageLabel witness expected one terminal box background, found {}",
                box_groups.len()
            );
            let box_surfaces = box_groups[0]
                .children()
                .filter(|node| node.has_tag_name("rect") && class_contains(*node, "rect"))
                .collect::<Vec<_>>();
            c6_ensure!(
                "route-svg-proof",
                box_surfaces.len() == 1,
                "Sequence MessageLabel box lacks one exact terminal background surface"
            );
            let box_region = raw_element_bounds(box_surfaces[0])?.rect(0.0)?;
            let message_region = target_regions[0];
            c6_ensure!(
                "route-svg-proof",
                message_region[0] >= box_region[0]
                    && message_region[1] >= box_region[1]
                    && message_region[0] + message_region[2] <= box_region[0] + box_region[2]
                    && message_region[1] + message_region[3] <= box_region[1] + box_region[3],
                "Sequence MessageLabel terminal text region lies outside its rendered box background"
            );
            let underlay = style_value(box_surfaces[0], "fill")
                .or_else(|| box_surfaces[0].attribute("fill"))
                .ok_or_else(|| {
                    C6ProofError::new(
                        "route-svg-proof",
                        "Sequence MessageLabel box background lacks a terminal fill",
                    )
                })
                .and_then(parse_optional_css_rgb)?;
            Ok(vec![underlay.into_iter().collect()])
        }
        (ThemeTarget::NoteLabel, ThemeRouteCutoverFacet::Fill) => {
            let root_id = sequence_root_id(document)?;
            let underlay = parse_optional_css_rgb(sequence_writer_property(
                document,
                &format!("#{root_id} .note"),
                "fill",
            )?)?;
            Ok(std::iter::repeat_n(underlay.into_iter().collect(), region_count).collect())
        }
        (ThemeTarget::Loop, ThemeRouteCutoverFacet::Fill) => {
            sequence_loop_fill_reveal_colors(document, route, target_regions)
        }
        (ThemeTarget::Loop, ThemeRouteCutoverFacet::Stroke) => {
            sequence_loop_stroke_reveal_colors(document, target_regions)
        }
        (ThemeTarget::LoopLabel, ThemeRouteCutoverFacet::Fill) => {
            c6_ensure!(
                "route-svg-proof",
                region_count == 3,
                "Sequence LoopLabel witness expected label-box, body, and section-title regions"
            );
            let root_id = sequence_root_id(document)?;
            let selector = format!("#{root_id} .labelBox");
            let underlay =
                parse_optional_css_rgb(sequence_writer_property(document, &selector, "fill")?)?;
            Ok(vec![underlay.into_iter().collect(), Vec::new(), Vec::new()])
        }
        _ => Ok(std::iter::repeat_n(Vec::new(), region_count).collect()),
    }
}

fn sequence_activation_underlay_colors(
    document: &roxmltree::Document<'_>,
    region_count: usize,
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    c6_ensure!(
        "route-svg-proof",
        region_count == 3,
        "Sequence Activation stroke witness expected three terminal regions"
    );
    let mut underlays = Vec::with_capacity(region_count);
    for class in ["activation0", "activation1", "activation2"] {
        let matching = document
            .descendants()
            .filter(|node| node.has_tag_name("rect") && class_contains(*node, class))
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            matching.len() == 1,
            "Sequence Activation underlay expected one `{class}` rect, found {}",
            matching.len()
        );
        let root_id = sequence_root_id(document)?;
        let fill = sequence_writer_property(document, &format!("#{root_id} .{class}"), "fill")?;
        underlays.push(parse_optional_css_rgb(fill)?.into_iter().collect());
    }
    Ok(underlays)
}

fn sequence_actor_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Actor,
        "unsupported Sequence cutover target {}",
        route.target().id()
    );
    let root_id = sequence_root_id(document)?;
    let participant_types = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("participant"))
        .try_fold(BTreeMap::<String, usize>::new(), |mut types, node| {
            let actor_type = node.attribute("data-type").ok_or_else(|| {
                C6ProofError::new(
                    "route-svg-proof",
                    "Sequence participant surface lacks data-type",
                )
            })?;
            *types.entry(actor_type.to_owned()).or_insert(0) += 1;
            Ok::<_, C6ProofError>(types)
        })?;
    let expected_types = match route.facet() {
        ThemeRouteCutoverFacet::Fill => &[
            "actor",
            "boundary",
            "collections",
            "database",
            "entity",
            "participant",
            "queue",
        ][..],
        ThemeRouteCutoverFacet::Stroke => &[
            "actor",
            "boundary",
            "collections",
            "entity",
            "participant",
            "queue",
        ][..],
    }
    .iter()
    .map(|value| (*value).to_owned())
    .collect::<BTreeSet<_>>();
    let actual_types = participant_types.keys().cloned().collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        actual_types == expected_types,
        "Sequence witness participant types differ: actual={actual_types:?}, expected={expected_types:?}"
    );
    let actor_surfaces = document
        .descendants()
        .filter(|node| node.is_element() && class_contains(*node, "actor"))
        .count();
    let actor_man_surfaces = document
        .descendants()
        .filter(|node| node.is_element() && class_contains(*node, "actor-man"))
        .count();
    c6_ensure!(
        "route-svg-proof",
        actor_surfaces > 0 && actor_man_surfaces > 0,
        "Sequence witness lacks actor-owned rectangle/path or actor-man surfaces"
    );

    let property = facet_property(route.facet());
    let actor_selector = format!("#{root_id} .actor");
    let actor_shape_selector = format!(
        "#{root_id} .actor-man line,#{root_id} .actor-man circle,#{root_id} .actor line,#{root_id} .actor circle"
    );
    let actor_value = expected_svg_value(route)?;
    require_typed_writer_property(document, &actor_selector, property, actor_value)?;
    require_typed_writer_property(document, &actor_shape_selector, property, actor_value)?;

    let participants = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("participant"))
        .collect::<Vec<_>>();
    let mut target_regions = Vec::new();
    let mut terminal = b"merman.c6-route-sequence-surfaces.v2\0".to_vec();
    for (actor_type, count) in &participant_types {
        append_len_prefixed(&mut terminal, actor_type.as_bytes());
        terminal.extend_from_slice(&usize_to_u64(*count).to_be_bytes());
    }
    for participant in participants {
        let actor_type = participant.attribute("data-type").unwrap_or_default();
        let paintable = participant.descendants().filter(|node| {
            node.is_element()
                && (class_contains(*node, "actor")
                    || (matches!(node.tag_name().name(), "line" | "circle")
                        && node
                            .ancestors()
                            .any(|ancestor| class_contains(ancestor, "actor-man"))))
        });
        c6_ensure!(
            "route-svg-proof",
            paintable.count() > 0,
            "Sequence {actor_type} participant lacks a typed Actor paint surface"
        );
        for node in participant.descendants().filter(|node| node.is_element()) {
            if let Some(inline) = style_value(node, property) {
                c6_ensure!(
                    "route-svg-proof",
                    inline == actor_value,
                    "Sequence {actor_type} participant overrides typed {property} with `{inline}`"
                );
            }
        }
        let region = group_bounds(participant, route.facet())?;
        append_len_prefixed(
            &mut terminal,
            participant
                .attribute("data-id")
                .unwrap_or_default()
                .as_bytes(),
        );
        append_len_prefixed(&mut terminal, actor_type.as_bytes());
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }
    append_len_prefixed(&mut terminal, actor_selector.as_bytes());
    append_len_prefixed(&mut terminal, actor_value.as_bytes());
    append_len_prefixed(&mut terminal, actor_shape_selector.as_bytes());
    append_len_prefixed(&mut terminal, actor_value.as_bytes());
    terminal.extend_from_slice(&usize_to_u64(actor_surfaces).to_be_bytes());
    terminal.extend_from_slice(&usize_to_u64(actor_man_surfaces).to_be_bytes());

    Ok(SequenceRouteObservation {
        value: actor_value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn sequence_lifeline_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Lifeline,
        "unsupported Sequence cutover target {}",
        route.target().id()
    );
    let root_id = sequence_root_id(document)?;
    let selector = format!("#{root_id} .actor-line");
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, "stroke", value)?;
    let lines = document
        .descendants()
        .filter(|node| node.attribute("data-et") == Some("life-line"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        lines.len() == 2,
        "Sequence Lifeline witness expected two terminal lines, found {}",
        lines.len()
    );
    let actual_ids = lines
        .iter()
        .filter_map(|line| line.attribute("data-id"))
        .collect::<BTreeSet<_>>();
    let expected_ids = ["Alice", "Bob"].into_iter().collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        actual_ids == expected_ids,
        "Sequence Lifeline terminal identities differ: actual={actual_ids:?}, expected={expected_ids:?}"
    );

    let mut terminal = b"merman.c6-route-sequence-lifeline-surfaces.v1\0".to_vec();
    let mut target_regions = Vec::with_capacity(lines.len());
    for line in lines {
        c6_ensure!(
            "route-svg-proof",
            line.has_tag_name("line") && class_contains(line, "actor-line"),
            "Sequence Lifeline terminal is not an actor-line"
        );
        c6_ensure!(
            "route-svg-proof",
            style_value(line, "stroke").is_none(),
            "Sequence Lifeline terminal overrides the typed actor-line stroke inline"
        );
        let base_stroke = line.attribute("stroke").ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Sequence Lifeline terminal lacks its source-backed stroke",
            )
        })?;
        c6_ensure!(
            "route-svg-proof",
            !base_stroke.trim().is_empty() && base_stroke != value,
            "Sequence Lifeline terminal does not rely on the final actor-line stroke winner"
        );
        let region = element_bounds(line, ThemeRouteCutoverFacet::Stroke)?;
        append_len_prefixed(
            &mut terminal,
            line.attribute("data-id").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(&mut terminal, base_stroke.as_bytes());
        append_len_prefixed(
            &mut terminal,
            line.attribute("stroke-width")
                .unwrap_or_default()
                .as_bytes(),
        );
        for attribute in ["x1", "y1", "x2", "y2"] {
            append_len_prefixed(
                &mut terminal,
                line.attribute(attribute).unwrap_or_default().as_bytes(),
            );
        }
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn sequence_static_rect_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    let root_id = sequence_root_id(document)?;
    let (surface_name, selector, surfaces) = match route.target() {
        ThemeTarget::Note => {
            let notes = document
                .descendants()
                .filter(|node| node.attribute("data-et") == Some("note"))
                .collect::<Vec<_>>();
            c6_ensure!(
                "route-svg-proof",
                notes.len() == 3,
                "Sequence Note witness expected 3 terminal groups, found {}",
                notes.len()
            );
            let mut surfaces = Vec::with_capacity(notes.len());
            for note in notes {
                let rects = note
                    .children()
                    .filter(|node| node.has_tag_name("rect") && class_contains(*node, "note"))
                    .collect::<Vec<_>>();
                c6_ensure!(
                    "route-svg-proof",
                    rects.len() == 1,
                    "Sequence Note group expected one terminal rect, found {}",
                    rects.len()
                );
                surfaces.push((
                    note.attribute("data-id").unwrap_or_default().to_owned(),
                    rects[0],
                ));
            }
            ("Note", format!("#{root_id} .note"), surfaces)
        }
        ThemeTarget::Activation => {
            let activations = document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("rect")
                        && ["activation0", "activation1", "activation2"]
                            .into_iter()
                            .any(|class| class_contains(*node, class))
                })
                .collect::<Vec<_>>();
            c6_ensure!(
                "route-svg-proof",
                activations.len() == 3,
                "Sequence Activation witness expected three terminal rects, found {}",
                activations.len()
            );
            let mut surfaces = Vec::with_capacity(3);
            for class in ["activation0", "activation1", "activation2"] {
                let matching = activations
                    .iter()
                    .copied()
                    .filter(|activation| activation.attribute("class") == Some(class))
                    .collect::<Vec<_>>();
                c6_ensure!(
                    "route-svg-proof",
                    matching.len() == 1,
                    "Sequence Activation witness expected one terminal {class} rect, found {}",
                    matching.len()
                );
                surfaces.push((class.to_owned(), matching[0]));
            }
            (
                "Activation",
                sequence_activation_selector(root_id),
                surfaces,
            )
        }
        target => {
            return Err(C6ProofError::new(
                "route-svg-proof",
                format!("unsupported Sequence rectangle target {}", target.id()),
            ));
        }
    };
    let property = facet_property(route.facet());
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, property, value)?;
    let mut target_regions = Vec::with_capacity(surfaces.len());
    let mut activation_fill_region = None;
    let mut terminal = b"merman.c6-route-sequence-static-rect-surfaces.v1\0".to_vec();
    append_len_prefixed(&mut terminal, route.target().id().as_bytes());
    for (identity, rect) in surfaces {
        if let Some(inline) = style_value(rect, property) {
            c6_ensure!(
                "route-svg-proof",
                inline == value,
                "Sequence {surface_name} rect overrides typed {property} with `{inline}`"
            );
        }
        let base_value = rect.attribute(property).ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence {surface_name} rect lacks its base {property} attribute"),
            )
        })?;
        let region = element_bounds(rect, route.facet())?;
        if route.target() == ThemeTarget::Activation
            && route.facet() == ThemeRouteCutoverFacet::Fill
            && identity == "activation0"
        {
            let [x, y, width, height] = raw_element_bounds(rect)?.rect(0.0)?;
            activation_fill_region = Some([x, y, width, width.min(height)]);
        }
        append_len_prefixed(&mut terminal, identity.as_bytes());
        append_len_prefixed(&mut terminal, base_value.as_bytes());
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }
    if let Some(region) = activation_fill_region {
        target_regions = vec![region];
    }
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

fn sequence_activation_selector(root_id: &str) -> String {
    format!("#{root_id} .activation0,#{root_id} .activation1,#{root_id} .activation2")
}

pub(super) fn sequence_actor_label_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::ActorLabel && route.facet() == ThemeRouteCutoverFacet::Fill,
        "unsupported Sequence ActorLabel cutover route {}",
        route_label(route)
    );
    let root_id = sequence_root_id(document)?;
    let selector = format!(
        "#{root_id} text.actor,#{root_id} text.actor>tspan,#{root_id} text.text,#{root_id} text.text>tspan"
    );
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, "fill", value)?;
    let surfaces = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "actor")
                && (class_contains(*node, "actor-top") || class_contains(*node, "actor-bottom"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        surfaces.len() == 4,
        "Sequence ActorLabel witness expected four terminal actor boxes, found {}",
        surfaces.len()
    );
    let all_actor_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "actor"))
        .count();
    c6_ensure!(
        "route-svg-proof",
        all_actor_texts == surfaces.len(),
        "Sequence ActorLabel text count differs from its actor boxes: {all_actor_texts} != {}",
        surfaces.len()
    );

    let mut identities = BTreeSet::new();
    let mut target_regions = Vec::with_capacity(surfaces.len());
    let mut terminal = b"merman.c6-route-sequence-actor-label-surfaces.v1\0".to_vec();
    for surface in surfaces {
        let owner = surface.parent().ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Sequence ActorLabel box lacks an occurrence wrapper",
            )
        })?;
        let texts = owner
            .descendants()
            .filter(|node| node.has_tag_name("text") && class_contains(*node, "actor"))
            .collect::<Vec<_>>();
        let actor_id = surface.attribute("name").unwrap_or_default();
        let position = if class_contains(surface, "actor-top") {
            "top"
        } else {
            "bottom"
        };
        let identity = format!("{actor_id}:{position}");
        c6_ensure!(
            "route-svg-proof",
            !actor_id.is_empty()
                && identities.insert(identity.clone())
                && texts.len() == 1
                && texts[0]
                    .children()
                    .filter(|node| node.has_tag_name("tspan"))
                    .count()
                    == 1,
            "Sequence ActorLabel `{identity}` lacks one exact terminal text"
        );
        let region = element_bounds(surface, ThemeRouteCutoverFacet::Fill)?;
        append_sequence_text_terminal(&mut terminal, texts[0], &identity, value, region)?;
        target_regions.push(region);
    }
    let expected = ["Alice:bottom", "Alice:top", "Bob:bottom", "Bob:top"]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    c6_ensure!(
        "route-svg-proof",
        identities == expected,
        "Sequence ActorLabel identities differ: actual={identities:?}, expected={expected:?}"
    );

    let box_titles = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "text"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        box_titles.len() == 1,
        "Sequence ActorLabel witness expected one terminal box title, found {}",
        box_titles.len()
    );
    let box_title = box_titles[0];
    let box_group = box_title.parent().ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            "Sequence ActorLabel box title lacks its terminal frame group",
        )
    })?;
    let box_surfaces = box_group
        .children()
        .filter(|node| node.has_tag_name("rect") && class_contains(*node, "rect"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        box_surfaces.len() == 1
            && box_title
                .children()
                .filter(|node| node.has_tag_name("tspan"))
                .count()
                == 1,
        "Sequence ActorLabel box title lacks one exact terminal frame surface"
    );
    let box_region = actor_box_title_region(document, box_surfaces[0], box_title)?;
    append_sequence_text_terminal(&mut terminal, box_title, "box:Team", value, box_region)?;
    target_regions.push(box_region);
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

pub(super) fn sequence_message_label_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::MessageLabel
            && route.facet() == ThemeRouteCutoverFacet::Fill,
        "unsupported Sequence MessageLabel cutover route {}",
        route_label(route)
    );
    let root_id = sequence_root_id(document)?;
    let selector = format!("#{root_id} .messageText,#{root_id} .messageText>tspan");
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, "fill", value)?;
    let lines = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node.attribute("data-et") == Some("message")
                && (class_contains(*node, "messageLine0") || class_contains(*node, "messageLine1"))
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        lines.len() == 1,
        "Sequence MessageLabel witness expected one terminal message, found {}",
        lines.len()
    );
    let all_message_texts = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "messageText"))
        .count();
    c6_ensure!(
        "route-svg-proof",
        all_message_texts == lines.len(),
        "Sequence MessageLabel text count differs from its message lines"
    );

    let mut target_regions = Vec::with_capacity(lines.len());
    let mut terminal = b"merman.c6-route-sequence-message-label-surfaces.v1\0".to_vec();
    for line in lines {
        let identity = line.attribute("data-id").unwrap_or_default();
        let text = previous_element_sibling(line).ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                format!("Sequence message `{identity}` lacks its adjacent terminal text"),
            )
        })?;
        c6_ensure!(
            "route-svg-proof",
            !identity.is_empty()
                && text.has_tag_name("text")
                && class_contains(text, "messageText"),
            "Sequence message `{identity}` is not bound to one exact MessageLabel text"
        );
        let region = message_label_region(text, line)?;
        append_sequence_text_terminal(&mut terminal, text, identity, value, region)?;
        append_len_prefixed(
            &mut terminal,
            line.attribute("class").unwrap_or_default().as_bytes(),
        );
        target_regions.push(region);
    }
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

pub(super) fn sequence_note_label_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::NoteLabel && route.facet() == ThemeRouteCutoverFacet::Fill,
        "unsupported Sequence NoteLabel cutover route {}",
        route_label(route)
    );
    let root_id = sequence_root_id(document)?;
    let selector = format!("#{root_id} .noteText,#{root_id} .noteText>tspan");
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, "fill", value)?;
    let notes = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("data-et") == Some("note"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        notes.len() == 3,
        "Sequence NoteLabel witness expected three terminal notes, found {}",
        notes.len()
    );

    let mut identities = BTreeSet::new();
    let mut target_regions = Vec::with_capacity(notes.len());
    let mut terminal = b"merman.c6-route-sequence-note-label-surfaces.v1\0".to_vec();
    for note in notes {
        let identity = note.attribute("data-id").unwrap_or_default();
        let rects = note
            .descendants()
            .filter(|node| node.has_tag_name("rect") && class_contains(*node, "note"))
            .collect::<Vec<_>>();
        let texts = note
            .descendants()
            .filter(|node| node.has_tag_name("text") && class_contains(*node, "noteText"))
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            !identity.is_empty()
                && identities.insert(identity.to_owned())
                && rects.len() == 1
                && texts.len() == 1,
            "Sequence note `{identity}` lacks one exact NoteLabel text/surface occurrence"
        );
        let region = element_bounds(rects[0], ThemeRouteCutoverFacet::Fill)?;
        append_sequence_text_terminal(&mut terminal, texts[0], identity, value, region)?;
        target_regions.push(region);
    }
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

pub(super) fn sequence_loop_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::Loop,
        "unsupported Sequence Loop cutover target {}",
        route.target().id()
    );
    let root_id = sequence_root_id(document)?;
    let selector = format!("#{root_id} .labelBox");
    let property = facet_property(route.facet());
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, property, value)?;
    let controls = sequence_control_structures(document)?;
    let mut target_regions = Vec::with_capacity(controls.len());
    let mut terminal = b"merman.c6-route-sequence-loop-surfaces.v1\0".to_vec();
    for control in controls {
        let identity = control.attribute("data-id").unwrap_or_default();
        let boxes = control
            .descendants()
            .filter(|node| node.has_tag_name("polygon") && class_contains(*node, "labelBox"))
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            !identity.is_empty() && boxes.len() == 1,
            "Sequence control `{identity}` lacks one exact Loop label-box surface"
        );
        let label_box = boxes[0];
        if let Some(inline) = terminal_node_property(label_box, property) {
            c6_ensure!(
                "route-svg-proof",
                inline == value,
                "Sequence Loop `{identity}` overrides typed {property} with `{inline}`"
            );
        }
        let region = element_bounds(label_box, route.facet())?;
        append_len_prefixed(&mut terminal, identity.as_bytes());
        append_len_prefixed(
            &mut terminal,
            label_box.attribute("points").unwrap_or_default().as_bytes(),
        );
        append_len_prefixed(&mut terminal, value.as_bytes());
        append_rect(&mut terminal, region);
        target_regions.push(region);
    }
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions,
    })
}

pub(super) fn sequence_loop_fill_reveal_colors(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    validate_sequence_loop_reveal_structure(
        document,
        target_regions,
        ThemeRouteCutoverFacet::Fill,
    )?;

    let root_id = sequence_root_id(document)?;
    let label_box_selector = format!("#{root_id} .labelBox");
    require_typed_writer_property(
        document,
        &label_box_selector,
        "fill",
        expected_svg_value(route)?,
    )?;
    let mut colors = Vec::with_capacity(3);
    for (selector, property) in [
        (label_box_selector, "stroke"),
        (format!("#{root_id} .loopLine"), "stroke"),
        (
            format!("#{root_id} .labelText,#{root_id} .labelText>tspan"),
            "fill",
        ),
    ] {
        if let Some(color) =
            parse_optional_css_rgb(sequence_writer_property(document, &selector, property)?)?
            && !colors.contains(&color)
        {
            colors.push(color);
        }
    }
    c6_ensure!(
        "route-svg-proof",
        !colors.is_empty(),
        "Sequence Loop fill witness lacks terminal reveal colors"
    );
    Ok(std::iter::repeat_n(colors, target_regions.len()).collect())
}

pub(super) fn sequence_loop_stroke_reveal_colors(
    document: &roxmltree::Document<'_>,
    target_regions: &[[f64; 4]],
) -> C6ProofResult<Vec<Vec<[u8; 3]>>> {
    validate_sequence_loop_reveal_structure(
        document,
        target_regions,
        ThemeRouteCutoverFacet::Stroke,
    )?;

    let root_id = sequence_root_id(document)?;
    let mut colors = Vec::with_capacity(2);
    for (selector, property) in [
        (format!("#{root_id} .labelBox"), "fill"),
        (format!("#{root_id} .loopLine"), "stroke"),
    ] {
        if let Some(color) =
            parse_optional_css_rgb(sequence_writer_property(document, &selector, property)?)?
            && !colors.contains(&color)
        {
            colors.push(color);
        }
    }
    c6_ensure!(
        "route-svg-proof",
        !colors.is_empty(),
        "Sequence Loop stroke witness lacks terminal reveal colors"
    );
    Ok(std::iter::repeat_n(colors, target_regions.len()).collect())
}

fn validate_sequence_loop_reveal_structure(
    document: &roxmltree::Document<'_>,
    target_regions: &[[f64; 4]],
    facet: ThemeRouteCutoverFacet,
) -> C6ProofResult<()> {
    let controls = sequence_control_structures(document)?;
    c6_ensure!(
        "route-svg-proof",
        controls.len() == target_regions.len(),
        "Sequence Loop reveal count differs from its target regions"
    );

    for (control, expected_region) in controls.into_iter().zip(target_regions) {
        let identity = control.attribute("data-id").unwrap_or_default();
        let boxes = control
            .descendants()
            .filter(|node| node.has_tag_name("polygon") && class_contains(*node, "labelBox"))
            .collect::<Vec<_>>();
        let labels = control
            .descendants()
            .filter(|node| node.has_tag_name("text") && class_contains(*node, "labelText"))
            .collect::<Vec<_>>();
        let frame_lines = control
            .descendants()
            .filter(|node| node.has_tag_name("line") && class_contains(*node, "loopLine"))
            .collect::<Vec<_>>();
        c6_ensure!(
            "route-svg-proof",
            !identity.is_empty() && boxes.len() == 1 && labels.len() == 1 && frame_lines.len() == 5,
            "Sequence Loop `{identity}` lacks the fixed alt label-box reveal structure"
        );

        let label_box = boxes[0];
        let label = labels[0];
        let rendered_region = element_bounds(label_box, facet)?;
        c6_ensure!(
            "route-svg-proof",
            rendered_region == *expected_region,
            "Sequence Loop `{identity}` reveal geometry differs from its {facet:?} target region"
        );
        c6_ensure!(
            "route-svg-proof",
            frame_lines
                .iter()
                .all(|line| line.range().end <= label_box.range().start)
                && label_box.range().end <= label.range().start,
            "Sequence Loop `{identity}` frame, label-box, and text paint order differs"
        );

        let [box_left, box_top, box_width, box_height] =
            raw_element_bounds(label_box)?.rect(0.0)?;
        let label_x = number_attribute(label, "x")?;
        let label_y = number_attribute(label, "y")?;
        let mut has_top_frame = false;
        let mut has_left_frame = false;
        for line in frame_lines {
            let x1 = number_attribute(line, "x1")?;
            let x2 = number_attribute(line, "x2")?;
            let y1 = number_attribute(line, "y1")?;
            let y2 = number_attribute(line, "y2")?;
            has_top_frame |= y1 == box_top
                && y2 == box_top
                && x1.min(x2) <= box_left
                && x1.max(x2) >= box_left + box_width;
            has_left_frame |= x1 == box_left
                && x2 == box_left
                && y1.min(y2) <= box_top
                && y1.max(y2) >= box_top + box_height;
        }
        c6_ensure!(
            "route-svg-proof",
            has_top_frame
                && has_left_frame
                && label_x >= box_left
                && label_x <= box_left + box_width
                && label_y >= box_top
                && label_y <= box_top + box_height,
            "Sequence Loop `{identity}` reveal layers do not overlap its label-box geometry"
        );
    }
    Ok(())
}

pub(super) fn sequence_loop_label_route_observation(
    document: &roxmltree::Document<'_>,
    route: ThemeRouteCutoverDescriptor,
) -> C6ProofResult<SequenceRouteObservation> {
    c6_ensure!(
        "route-svg-proof",
        route.target() == ThemeTarget::LoopLabel && route.facet() == ThemeRouteCutoverFacet::Fill,
        "unsupported Sequence LoopLabel cutover route {}",
        route_label(route)
    );
    let root_id = sequence_root_id(document)?;
    let selector = format!(
        "#{root_id} .loopText,#{root_id} .loopText>tspan,#{root_id} .sectionTitle,#{root_id} .sectionTitle>tspan,#{root_id} .labelText,#{root_id} .labelText>tspan"
    );
    let value = expected_svg_value(route)?;
    require_typed_writer_property(document, &selector, "fill", value)?;
    let controls = sequence_control_structures(document)?;
    let control = controls[0];
    let identity = control.attribute("data-id").unwrap_or_default();
    let boxes = control
        .descendants()
        .filter(|node| node.has_tag_name("polygon") && class_contains(*node, "labelBox"))
        .collect::<Vec<_>>();
    let label_texts = control
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "labelText"))
        .collect::<Vec<_>>();
    let body_texts = control
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "loopText"))
        .collect::<Vec<_>>();
    let section_titles = control
        .descendants()
        .filter(|node| node.has_tag_name("text") && class_contains(*node, "sectionTitle"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        !identity.is_empty()
            && boxes.len() == 1
            && label_texts.len() == 1
            && body_texts.len() == 1
            && section_titles.len() == 1,
        "Sequence loop `{identity}` roles differ from the fixed LoopLabel fixture"
    );
    c6_ensure!(
        "route-svg-proof",
        label_texts[0]
            .children()
            .all(|node| !node.has_tag_name("tspan"))
            && body_texts[0]
                .children()
                .filter(|node| node.has_tag_name("tspan"))
                .count()
                == 1
            && section_titles[0]
                .children()
                .all(|node| !node.has_tag_name("tspan")),
        "Sequence LoopLabel witness does not cover the expected direct/tspan terminal branches"
    );

    let [label_region, body_region, section_region] = loop_label_regions(
        control,
        boxes[0],
        label_texts[0],
        body_texts[0],
        section_titles[0],
    )?;
    let mut terminal = b"merman.c6-route-sequence-loop-label-surfaces.v1\0".to_vec();
    append_sequence_text_terminal(
        &mut terminal,
        label_texts[0],
        &format!("{identity}:label"),
        value,
        label_region,
    )?;
    append_sequence_text_terminal(
        &mut terminal,
        body_texts[0],
        &format!("{identity}:body"),
        value,
        body_region,
    )?;
    append_sequence_text_terminal(
        &mut terminal,
        section_titles[0],
        &format!("{identity}:section"),
        value,
        section_region,
    )?;
    append_len_prefixed(
        &mut terminal,
        boxes[0].attribute("points").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(&mut terminal, selector.as_bytes());
    append_len_prefixed(&mut terminal, value.as_bytes());

    Ok(SequenceRouteObservation {
        value: value.to_owned(),
        terminal_digest: sha256(terminal),
        target_regions: vec![label_region, body_region, section_region],
    })
}

fn sequence_root_id<'document, 'input: 'document>(
    document: &'document roxmltree::Document<'input>,
) -> C6ProofResult<&'document str> {
    document
        .root_element()
        .attribute("id")
        .ok_or_else(|| C6ProofError::new("route-svg-proof", "Sequence SVG root lacks an id"))
}

fn sequence_writer_property<'a>(
    document: &'a roxmltree::Document<'a>,
    selector: &str,
    property: &str,
) -> C6ProofResult<&'a str> {
    c6_ensure!(
        "route-svg-proof",
        matches!(property, "fill" | "stroke"),
        "Sequence native proof only observes fixed writer paint properties"
    );
    let selector_marker = format!("{selector}{{");
    let mut values = Vec::new();
    for css in document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
    {
        collect_exact_writer_properties(css, &selector_marker, property, &mut values);
    }
    c6_ensure!(
        "route-svg-proof",
        values.len() == 1,
        "Sequence fixed writer schema requires one `{selector}` `{property}` declaration, found {}",
        values.len()
    );
    Ok(values[0])
}

fn collect_exact_writer_properties<'a>(
    css: &'a str,
    selector_marker: &str,
    property: &str,
    values: &mut Vec<&'a str>,
) {
    let mut remaining = css;
    while let Some(rule_start) = remaining.find(selector_marker) {
        let body = &remaining[rule_start + selector_marker.len()..];
        let Some(rule_end) = body.find('}') else {
            return;
        };
        for declaration in body[..rule_end].split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            if name.trim() == property {
                values.push(value.trim());
            }
        }
        remaining = &body[rule_end + 1..];
    }
}

fn sequence_control_structures<'document, 'input: 'document>(
    document: &'document roxmltree::Document<'input>,
) -> C6ProofResult<Vec<roxmltree::Node<'document, 'input>>> {
    let controls = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g") && node.attribute("data-et") == Some("control-structure")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        controls.len() == 1,
        "Sequence Loop witness expected one control structure, found {}",
        controls.len()
    );
    Ok(controls)
}

fn append_sequence_text_terminal(
    terminal: &mut Vec<u8>,
    text: roxmltree::Node<'_, '_>,
    identity: &str,
    expected_fill: &str,
    region: [f64; 4],
) -> C6ProofResult<()> {
    let visible_text = text
        .descendants()
        .filter(|node| node.is_text())
        .filter_map(|node| node.text())
        .collect::<String>()
        .trim()
        .to_owned();
    c6_ensure!(
        "route-svg-proof",
        !visible_text.is_empty(),
        "Sequence text occurrence `{identity}` has no visible terminal text"
    );
    for owner in std::iter::once(text).chain(
        text.descendants()
            .filter(|descendant| descendant.has_tag_name("tspan")),
    ) {
        if let Some(inline) = terminal_node_property(owner, "fill") {
            c6_ensure!(
                "route-svg-proof",
                inline == expected_fill,
                "Sequence text occurrence `{identity}` overrides typed fill with `{inline}`"
            );
        }
    }
    append_len_prefixed(terminal, identity.as_bytes());
    append_len_prefixed(terminal, visible_text.as_bytes());
    append_len_prefixed(
        terminal,
        text.attribute("class").unwrap_or_default().as_bytes(),
    );
    append_len_prefixed(
        terminal,
        text.attribute("style").unwrap_or_default().as_bytes(),
    );
    for attribute in ["x", "y", "dy", "text-anchor", "dominant-baseline"] {
        append_len_prefixed(
            terminal,
            text.attribute(attribute).unwrap_or_default().as_bytes(),
        );
    }
    append_rect(terminal, region);
    Ok(())
}

fn terminal_node_property<'a>(node: roxmltree::Node<'a, '_>, property: &str) -> Option<&'a str> {
    style_value(node, property).or_else(|| node.attribute(property))
}

fn previous_element_sibling<'document, 'input: 'document>(
    mut node: roxmltree::Node<'document, 'input>,
) -> Option<roxmltree::Node<'document, 'input>> {
    while let Some(previous) = node.prev_sibling() {
        if previous.is_element() {
            return Some(previous);
        }
        node = previous;
    }
    None
}

fn require_typed_writer_property(
    document: &roxmltree::Document<'_>,
    selector: &str,
    property: &str,
    expected: &str,
) -> C6ProofResult<()> {
    let actual = sequence_writer_property(document, selector, property)?;
    c6_ensure!(
        "route-svg-proof",
        actual == expected,
        "Sequence production writer declaration `{selector}` emitted `{property}:{actual}` instead of `{property}:{expected}`"
    );
    Ok(())
}

fn message_label_region(
    text: roxmltree::Node<'_, '_>,
    line: roxmltree::Node<'_, '_>,
) -> C6ProofResult<[f64; 4]> {
    let x1 = number_attribute(line, "x1")?;
    let x2 = number_attribute(line, "x2")?;
    let y1 = number_attribute(line, "y1")?;
    let y2 = number_attribute(line, "y2")?;
    let text_y = number_attribute(text, "y")?;
    c6_ensure!(
        "route-svg-proof",
        y1 == y2 && x1 != x2 && text_y != y1,
        "Sequence MessageLabel terminal geometry is not one horizontal message row"
    );
    let left = x1.min(x2);
    let top = text_y.min(y1);
    Ok([left, top, (x2 - x1).abs(), (y1 - text_y).abs()])
}

fn actor_box_title_region(
    document: &roxmltree::Document<'_>,
    box_surface: roxmltree::Node<'_, '_>,
    box_title: roxmltree::Node<'_, '_>,
) -> C6ProofResult<[f64; 4]> {
    let [left, top, width, _height] = raw_element_bounds(box_surface)?.rect(0.0)?;
    let actor_top = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && class_contains(*node, "actor")
                && class_contains(*node, "actor-top")
        })
        .map(raw_element_bounds)
        .collect::<C6ProofResult<Vec<_>>>()?
        .into_iter()
        .map(|bounds| bounds.rect(0.0).map(|rect| rect[1]))
        .collect::<C6ProofResult<Vec<_>>>()?
        .into_iter()
        .reduce(f64::min)
        .ok_or_else(|| {
            C6ProofError::new(
                "route-svg-proof",
                "Sequence ActorLabel box-title witness lacks actor-top geometry",
            )
        })?;
    let region = [left, top, width, actor_top - top];
    let title_x = number_attribute(box_title, "x")?;
    let title_y = number_attribute(box_title, "y")?;
    c6_ensure!(
        "route-svg-proof",
        region[2] > 0.0
            && region[3] > 0.0
            && title_x >= region[0]
            && title_x <= region[0] + region[2]
            && title_y >= region[1]
            && title_y <= region[1] + region[3],
        "Sequence ActorLabel box title lies outside its terminal frame header"
    );
    Ok(region)
}

fn loop_label_regions(
    control: roxmltree::Node<'_, '_>,
    label_box: roxmltree::Node<'_, '_>,
    label_text: roxmltree::Node<'_, '_>,
    body_text: roxmltree::Node<'_, '_>,
    section_title: roxmltree::Node<'_, '_>,
) -> C6ProofResult<[[f64; 4]; 3]> {
    let label_region = raw_element_bounds(label_box)?.rect(0.0)?;
    let lines = control
        .descendants()
        .filter(|node| node.has_tag_name("line") && class_contains(*node, "loopLine"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "route-svg-proof",
        lines.len() == 5,
        "Sequence LoopLabel alt witness expected four frame lines and one separator, found {}",
        lines.len()
    );

    let mut frame_left = f64::INFINITY;
    let mut frame_right = f64::NEG_INFINITY;
    let mut frame_top = f64::INFINITY;
    let mut frame_bottom = f64::NEG_INFINITY;
    let mut separator_y = None;
    for line in lines {
        let x1 = number_attribute(line, "x1")?;
        let x2 = number_attribute(line, "x2")?;
        let y1 = number_attribute(line, "y1")?;
        let y2 = number_attribute(line, "y2")?;
        frame_left = frame_left.min(x1).min(x2);
        frame_right = frame_right.max(x1).max(x2);
        frame_top = frame_top.min(y1).min(y2);
        frame_bottom = frame_bottom.max(y1).max(y2);
        if style_value(line, "stroke-dasharray") == Some("3, 3") {
            c6_ensure!(
                "route-svg-proof",
                separator_y.is_none() && y1 == y2,
                "Sequence LoopLabel witness has an ambiguous section separator"
            );
            separator_y = Some(y1);
        }
    }
    let separator_y = separator_y.ok_or_else(|| {
        C6ProofError::new(
            "route-svg-proof",
            "Sequence LoopLabel witness lacks its terminal section separator",
        )
    })?;
    let [label_left, label_top, label_width, label_height] = label_region;
    let body_left = label_left + label_width;
    c6_ensure!(
        "route-svg-proof",
        frame_left.is_finite()
            && frame_right.is_finite()
            && frame_top.is_finite()
            && frame_bottom.is_finite()
            && label_top == frame_top
            && body_left < frame_right
            && separator_y > frame_top
            && separator_y < frame_bottom,
        "Sequence LoopLabel frame, label-box, and section geometry disagree"
    );
    let body_region = [body_left, frame_top, frame_right - body_left, label_height];
    let section_region = [
        frame_left,
        separator_y,
        frame_right - frame_left,
        label_height,
    ];
    for (name, text, region) in [
        ("label", label_text, label_region),
        ("body", body_text, body_region),
        ("section", section_title, section_region),
    ] {
        let x = number_attribute(text, "x")?;
        let y = number_attribute(text, "y")?;
        c6_ensure!(
            "route-svg-proof",
            x >= region[0]
                && x <= region[0] + region[2]
                && y >= region[1]
                && y <= region[1] + region[3],
            "Sequence LoopLabel {name} anchor lies outside its terminal structural region"
        );
    }
    Ok([label_region, body_region, section_region])
}

#[cfg(test)]
mod tests {
    use super::*;

    const NESTED_ACTIVATION_SVG: &str = r##"<svg id="fixture" viewBox="0 0 100 120">
<style>#fixture .activation0,#fixture .activation1,#fixture .activation2{fill:#dc2626;}</style>
<rect x="10" y="10" width="10" height="100" fill="#edf2ae" class="activation0"/>
<rect x="15" y="40" width="10" height="50" fill="#edf2ae" class="activation1"/>
<rect x="20" y="60" width="10" height="20" fill="#edf2ae" class="activation2"/>
</svg>"##;

    fn sequence_activation_fill_route() -> ThemeRouteCutoverDescriptor {
        legacy_replacing_typed_theme_routes()
            .expect("derive route inventory")
            .into_iter()
            .find(|route| {
                route.family_id() == DiagramFamilyId::SEQUENCE
                    && route.target() == ThemeTarget::Activation
                    && route.facet() == ThemeRouteCutoverFacet::Fill
                    && route.value() == ThemeRouteCutoverValue::Solid
            })
            .expect("Sequence Activation fill route")
    }

    fn nested_activation_observation(svg: &str) -> SequenceRouteObservation {
        let document = roxmltree::Document::parse(svg).expect("parse nested activation SVG");
        sequence_static_rect_route_observation(&document, sequence_activation_fill_route())
            .expect("observe nested activation fill")
    }

    #[test]
    fn sequence_activation_fill_uses_one_non_overlapping_png_region() {
        let observation = nested_activation_observation(NESTED_ACTIVATION_SVG);

        assert_eq!(observation.value, SOLID_FILL.css);
        assert_eq!(observation.target_regions, vec![[10.0, 10.0, 10.0, 10.0]]);
    }

    #[test]
    fn sequence_activation_fill_roi_does_not_narrow_terminal_svg_proof() {
        let baseline = nested_activation_observation(NESTED_ACTIVATION_SVG);
        let changed_svg = NESTED_ACTIVATION_SVG.replace(
            r##"fill="#edf2ae" class="activation2""##,
            r##"fill="#fef3c7" class="activation2""##,
        );
        let changed = nested_activation_observation(&changed_svg);

        assert_eq!(changed.target_regions, baseline.target_regions);
        assert_ne!(changed.terminal_digest, baseline.terminal_digest);
    }
}
