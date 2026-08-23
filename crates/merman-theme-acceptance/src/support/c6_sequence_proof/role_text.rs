use std::collections::BTreeSet;

use merman_render::__private::{SvgArtifactReceipt, SvgElementObservation};

use super::super::artifact_observation::descendant_text;
use super::*;

pub(super) fn prove_native_role_text(
    receipt: &SvgArtifactReceipt,
    root_id: &str,
    surfaces: &SequenceSurfaces<'_>,
    font_family: Option<&str>,
    message_text_count: usize,
    contract: SequenceProofContract<'_>,
    role_text: Option<SequenceRoleTextProofContract<'_>>,
) -> C6ProofResult<()> {
    c6_ensure!(
        "sequence-svg-text",
        receipt.has_native_text()
            && !receipt.has_foreign_object()
            && !receipt.has_prepared_tokens(),
        "Brutalist Sequence did not retain terminal native SVG text"
    );

    let Some(font_family) = font_family else {
        c6_ensure!(
            "sequence-svg-text-role",
            role_text.is_none(),
            "Sequence role-paint proof requires one terminal font identity"
        );
        return Ok(());
    };
    if let Some(role_text) = role_text {
        prove_exact_role_text_surfaces(
            receipt,
            root_id,
            surfaces,
            contract,
            role_text,
            font_family,
        )?;
    } else {
        for (role, minimum) in [
            ("actor", 4usize),
            ("messageText", message_text_count),
            ("noteText", 1),
            ("loopText", 1),
        ] {
            let texts = receipt
                .elements()
                .iter()
                .filter(|node| node.tag_name() == "text" && node.has_class(role))
                .collect::<Vec<_>>();
            c6_ensure!(
                "sequence-svg-font",
                texts.len() >= minimum
                    && texts.iter().all(|node| {
                        node.style_value("font-family")
                            .is_some_and(|actual| css_family_matches(actual, font_family))
                    }),
                "Sequence requires at least {minimum} native `{role}` texts with `{font_family}`"
            );
        }
    }

    let typed_font_styles = receipt
        .stylesheets()
        .iter()
        .filter(|stylesheet| stylesheet.typed_font_marker() && stylesheet.parse_valid())
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-font",
        typed_font_styles.len() == 1
            && typed_font_styles[0].font_faces().iter().any(|font_face| {
                let family = font_face
                    .declarations()
                    .iter()
                    .find(|declaration| declaration.property() == "font-family")
                    .map(|declaration| declaration.value());
                let source = font_face
                    .declarations()
                    .iter()
                    .find(|declaration| declaration.property() == "src")
                    .map(|declaration| declaration.value());
                family.is_some_and(|value| value.contains(font_family))
                    && source.is_some_and(|value| value.contains("data:font/"))
            }),
        "Sequence standalone SVG lacks its sealed embedded `{font_family}` font face"
    );
    Ok(())
}

fn prove_exact_role_text_surfaces(
    receipt: &SvgArtifactReceipt,
    root_id: &str,
    surfaces: &SequenceSurfaces<'_>,
    contract: SequenceProofContract<'_>,
    role_text: SequenceRoleTextProofContract<'_>,
    font_family: &str,
) -> C6ProofResult<()> {
    let canvas = receipt
        .elements()
        .iter()
        .filter(|node| node.attribute("data-merman-theme-canvas") == Some("base"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-text-surface",
        canvas.len() == 1 && terminal_node_fill(canvas[0]) == Some(role_text.canvas),
        "Sequence role text requires one exact typed canvas surface"
    );

    let actor_selector = format!(
        "#{root_id} text.actor,#{root_id} text.actor>tspan,#{root_id} text.text,#{root_id} text.text>tspan"
    );
    let message_selector = format!("#{root_id} .messageText,#{root_id} .messageText>tspan");
    let note_selector = format!("#{root_id} .noteText,#{root_id} .noteText>tspan");
    let loop_selector = format!(
        "#{root_id} .loopText,#{root_id} .loopText>tspan,#{root_id} .sectionTitle,#{root_id} .sectionTitle>tspan,#{root_id} .labelText,#{root_id} .labelText>tspan"
    );
    super::require_exact_writer_declaration(
        receipt,
        &actor_selector,
        "fill",
        role_text.actor_text,
    )?;
    super::require_exact_writer_declaration(
        receipt,
        &message_selector,
        "fill",
        role_text.message_text,
    )?;
    super::require_exact_writer_declaration(receipt, &note_selector, "fill", role_text.note_text)?;
    super::require_exact_writer_declaration(receipt, &loop_selector, "fill", role_text.loop_text)?;
    super::require_exact_writer_declaration(
        receipt,
        &format!("#{root_id} .labelBox"),
        "fill",
        role_text.loop_label_surface,
    )?;

    prove_actor_text_occurrences(receipt, surfaces, role_text, font_family)?;
    prove_message_text_occurrences(receipt, surfaces, role_text, font_family)?;
    prove_note_text_occurrence(receipt, surfaces, role_text, font_family)?;
    prove_loop_text_occurrences(receipt, role_text, font_family)?;

    prove_role_contrast("actor-label", role_text.actor_text, contract.actor.fill)?;
    prove_role_contrast("message-label", role_text.message_text, role_text.canvas)?;
    let note = contract.note.ok_or_else(|| {
        C6ProofError::new(
            "sequence-svg-text-surface",
            "Sequence role proof requires a terminal note surface",
        )
    })?;
    prove_role_contrast("note-label", role_text.note_text, note.fill)?;
    prove_role_contrast(
        "loop-label-box",
        role_text.loop_text,
        role_text.loop_label_surface,
    )?;
    prove_role_contrast("loop-body", role_text.loop_text, role_text.canvas)?;
    Ok(())
}

fn prove_actor_text_occurrences(
    receipt: &SvgArtifactReceipt,
    surfaces: &SequenceSurfaces<'_>,
    role_text: SequenceRoleTextProofContract<'_>,
    font_family: &str,
) -> C6ProofResult<()> {
    let all_texts = receipt
        .elements()
        .iter()
        .filter(|node| node.tag_name() == "text" && node.has_class("actor"))
        .count();
    c6_ensure!(
        "sequence-svg-text-role",
        all_texts == surfaces.actor_rects.len(),
        "Sequence actor text count differs from its terminal actor surfaces"
    );
    let mut occurrences = BTreeSet::new();
    for surface in &surfaces.actor_rects {
        let owner = surface
            .parent_index()
            .and_then(|index| receipt.element(index))
            .ok_or_else(|| {
                C6ProofError::new(
                    "sequence-svg-text-surface",
                    "Sequence actor surface lacks its terminal occurrence wrapper",
                )
            })?;
        let texts = receipt
            .descendants_of(owner.index())
            .filter(|node| node.tag_name() == "text" && node.has_class("actor"))
            .collect::<Vec<_>>();
        let position = if surface.has_class("actor-top") {
            "top"
        } else {
            "bottom"
        };
        let actor_id = surface.attribute("name").unwrap_or_default();
        let occurrence_id = format!("{actor_id}:{position}");
        c6_ensure!(
            "sequence-svg-text-role",
            !actor_id.is_empty() && occurrences.insert(occurrence_id.clone()) && texts.len() == 1,
            "Sequence actor `{occurrence_id}` lacks one exact terminal text occurrence"
        );
        prove_role_text_node(
            receipt,
            texts[0],
            "actor-label",
            &occurrence_id,
            role_text.actor_text,
            font_family,
        )?;
    }
    c6_ensure!(
        "sequence-svg-text-role",
        occurrences.len() == surfaces.actor_rects.len(),
        "Sequence actor text/surface occurrence coverage is incomplete"
    );
    Ok(())
}

fn prove_message_text_occurrences(
    receipt: &SvgArtifactReceipt,
    surfaces: &SequenceSurfaces<'_>,
    role_text: SequenceRoleTextProofContract<'_>,
    font_family: &str,
) -> C6ProofResult<()> {
    let all_texts = receipt
        .elements()
        .iter()
        .filter(|node| node.tag_name() == "text" && node.has_class("messageText"))
        .count();
    c6_ensure!(
        "sequence-svg-text-role",
        all_texts == surfaces.message_lines.len(),
        "Sequence message text count differs from its terminal message surfaces"
    );
    let mut occurrences = BTreeSet::new();
    for line in &surfaces.message_lines {
        let occurrence_id = line.attribute("data-id").unwrap_or_default();
        let text = previous_element_sibling(receipt, line).ok_or_else(|| {
            C6ProofError::new(
                "sequence-svg-text-role",
                format!("Sequence message `{occurrence_id}` lacks its adjacent terminal text"),
            )
        })?;
        c6_ensure!(
            "sequence-svg-text-role",
            !occurrence_id.is_empty()
                && occurrences.insert(occurrence_id.to_string())
                && text.tag_name() == "text"
                && text.has_class("messageText"),
            "Sequence message `{occurrence_id}` is not bound to one exact text occurrence"
        );
        prove_role_text_node(
            receipt,
            text,
            "message-label",
            occurrence_id,
            role_text.message_text,
            font_family,
        )?;
    }
    Ok(())
}

fn prove_note_text_occurrence(
    receipt: &SvgArtifactReceipt,
    surfaces: &SequenceSurfaces<'_>,
    role_text: SequenceRoleTextProofContract<'_>,
    font_family: &str,
) -> C6ProofResult<()> {
    let notes = receipt
        .elements()
        .iter()
        .filter(|node| node.tag_name() == "g" && node.attribute("data-et") == Some("note"))
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-text-role",
        notes.len() == 1
            && receipt
                .descendants_of(notes[0].index())
                .filter(|node| node.tag_name() == "rect" && node.has_class("note"))
                .count()
                == 1,
        "Sequence note requires one exact text/surface occurrence"
    );
    let texts = receipt
        .descendants_of(notes[0].index())
        .filter(|node| node.tag_name() == "text" && node.has_class("noteText"))
        .collect::<Vec<_>>();
    let occurrence_id = notes[0].attribute("data-id").unwrap_or_default();
    c6_ensure!(
        "sequence-svg-text-role",
        !occurrence_id.is_empty() && texts.len() == 1 && surfaces.note.has_class("note"),
        "Sequence note `{occurrence_id}` lacks one exact terminal text occurrence"
    );
    prove_role_text_node(
        receipt,
        texts[0],
        "note-label",
        occurrence_id,
        role_text.note_text,
        font_family,
    )
}

fn prove_loop_text_occurrences(
    receipt: &SvgArtifactReceipt,
    role_text: SequenceRoleTextProofContract<'_>,
    font_family: &str,
) -> C6ProofResult<()> {
    let loops = receipt
        .elements()
        .iter()
        .filter(|node| {
            node.tag_name() == "g" && node.attribute("data-et") == Some("control-structure")
        })
        .collect::<Vec<_>>();
    c6_ensure!(
        "sequence-svg-text-role",
        loops.len() == 1,
        "Sequence fixture requires one exact loop occurrence"
    );
    let loop_id = loops[0].attribute("data-id").unwrap_or_default();
    let label_boxes = receipt
        .descendants_of(loops[0].index())
        .filter(|node| node.has_class("labelBox"))
        .collect::<Vec<_>>();
    let label_texts = receipt
        .descendants_of(loops[0].index())
        .filter(|node| node.tag_name() == "text" && node.has_class("labelText"))
        .collect::<Vec<_>>();
    let body_texts = receipt
        .descendants_of(loops[0].index())
        .filter(|node| node.tag_name() == "text" && node.has_class("loopText"))
        .collect::<Vec<_>>();
    let section_titles = receipt
        .descendants_of(loops[0].index())
        .filter(|node| node.tag_name() == "text" && node.has_class("sectionTitle"))
        .count();
    c6_ensure!(
        "sequence-svg-text-role",
        !loop_id.is_empty()
            && label_boxes.len() == 1
            && label_texts.len() == 1
            && body_texts.len() == 1
            && section_titles == 0,
        "Sequence loop `{loop_id}` text/surface roles differ from the fixed fixture"
    );
    prove_role_text_node(
        receipt,
        label_texts[0],
        "loop-label",
        &format!("{loop_id}:label"),
        role_text.loop_text,
        font_family,
    )?;
    prove_role_text_node(
        receipt,
        body_texts[0],
        "loop-label",
        &format!("{loop_id}:body"),
        role_text.loop_text,
        font_family,
    )
}

fn prove_role_text_node(
    receipt: &SvgArtifactReceipt,
    node: &SvgElementObservation,
    role: &str,
    occurrence_id: &str,
    expected_fill: &str,
    font_family: &str,
) -> C6ProofResult<()> {
    c6_ensure!(
        "sequence-svg-text-role",
        !descendant_text(node).is_empty(),
        "Sequence `{role}/{occurrence_id}` has no visible text"
    );
    c6_ensure!(
        "sequence-svg-font",
        node.style_value("font-family")
            .is_some_and(|actual| css_family_matches(actual, font_family)),
        "Sequence `{role}/{occurrence_id}` lacks terminal `{font_family}` typography"
    );
    c6_ensure!(
        "sequence-svg-text-paint",
        std::iter::once(node)
            .chain(
                receipt
                    .descendants_of(node.index())
                    .filter(|descendant| descendant.tag_name() == "tspan"),
            )
            .filter_map(terminal_node_fill)
            .all(|fill| fill == expected_fill),
        "Sequence `{role}/{occurrence_id}` contains a conflicting inline paint winner"
    );
    Ok(())
}

fn previous_element_sibling<'a>(
    receipt: &'a SvgArtifactReceipt,
    node: &SvgElementObservation,
) -> Option<&'a SvgElementObservation> {
    let parent = node.parent_index()?;
    receipt
        .elements()
        .iter()
        .filter(|candidate| {
            candidate.parent_index() == Some(parent) && candidate.index() < node.index()
        })
        .next_back()
}

fn terminal_node_fill(node: &SvgElementObservation) -> Option<&str> {
    node.style_value("fill").or_else(|| node.attribute("fill"))
}

fn css_family_matches(actual: &str, expected: &str) -> bool {
    actual
        .split(',')
        .next()
        .map(str::trim)
        .map(|family| family.trim_matches(['\'', '"']))
        == Some(expected)
}

fn prove_role_contrast(role: &str, foreground: &str, background: &str) -> C6ProofResult<()> {
    let foreground = parse_c6_hex_rgb(foreground)?;
    let background = parse_c6_hex_rgb(background)?;
    let contrast = contrast_ratio(foreground, background);
    c6_ensure!(
        "sequence-svg-text-contrast",
        contrast >= 4.5,
        "Sequence `{role}` foreground/background contrast is {contrast:.2}:1, expected at least 4.5:1"
    );
    Ok(())
}

fn contrast_ratio(first: [u8; 3], second: [u8; 3]) -> f64 {
    let first = relative_luminance(first);
    let second = relative_luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

fn relative_luminance(rgb: [u8; 3]) -> f64 {
    let [red, green, blue] = rgb.map(|channel| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}
