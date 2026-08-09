use super::{
    CLASS_LEVEL_HORIZONTAL_GAP, ClassCharset, ClassDirection, ClassNoteIndex, RenderedClassBox,
    RenderedClassBoxIndex, class_relation_summary_row, external_namespace_note_summary_rows,
    grid_overflow, layout_allocation_failed, nesting_overflow, note_relation_layouts_for_notes,
    relation_explicit_namespace_id, relation_layout, render_class_box,
    render_class_component_lines, render_class_document_lines,
    render_horizontal_class_component_lines, render_interface_box, render_note_box, work_overflow,
};
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::relation_graph::{self, RelationGraphBox, RelationGraphBoxStyle, RelationGraphLine};
use crate::resource::ResourceContext;
use crate::safe_text::normalize_terminal_text;
use crate::{AsciiError, Result};
use merman_core::entities::decode_html_entities_to_unicode;
use merman_core::models::class_diagram::{ClassDiagram, ClassNode, Namespace};
use std::collections::{HashMap, HashSet};

struct NamespaceRenderContext<'a> {
    model: &'a ClassDiagram,
    options: &'a AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &'a HashMap<String, String>,
    note_by_id: ClassNoteIndex<'a>,
}

pub(super) fn has_renderable_namespaces(model: &ClassDiagram) -> bool {
    model.namespaces.values().any(|namespace| {
        !namespace.class_ids.is_empty()
            || !namespace.note_ids.is_empty()
            || model
                .namespaces
                .values()
                .any(|child| child.parent.as_deref() == Some(namespace.id.as_str()))
    })
}

pub(super) fn render_namespaced_class_diagram(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &HashMap<String, String>,
    resources: &mut ResourceContext,
) -> Result<String> {
    let boxes = render_namespaced_class_boxes(
        model,
        options,
        charset,
        direction,
        namespace_facade_aliases,
        resources,
    )?;
    let mut external_layouts = Vec::new();
    external_layouts
        .try_reserve_exact(model.relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for relation in model.relations.iter().filter(|relation| {
        relation_explicit_namespace_id(model, relation, namespace_facade_aliases).is_none()
    }) {
        external_layouts.push(relation_layout(
            model,
            relation,
            namespace_facade_aliases,
            options.terminal_width_profile,
            resources,
        )?);
    }
    for layout in &mut external_layouts {
        layout.apply_direction(direction);
    }
    let summary_capacity = external_layouts
        .len()
        .checked_add(model.notes.len())
        .ok_or_else(|| work_overflow(resources))?;
    let mut summary_rows = Vec::new();
    summary_rows
        .try_reserve_exact(summary_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for layout in &external_layouts {
        summary_rows.push(class_relation_summary_row(layout)?);
    }
    summary_rows.extend(external_namespace_note_summary_rows(
        model,
        namespace_facade_aliases,
    )?);

    if summary_rows.is_empty() {
        if direction.is_horizontal() {
            let lines = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                direction.horizontal_direction(),
                CLASS_LEVEL_HORIZONTAL_GAP,
                options.terminal_width_profile,
                resources,
            )?;
            return render_class_document_lines(lines, options, resources);
        }
        if direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                options.terminal_width_profile,
                true,
                resources,
            )?;
            return render_class_document_lines(lines, options, resources);
        }
        return relation_graph::render_stacked_boxes_with_options(&boxes, options, resources);
    }

    if direction.is_horizontal() {
        let gap = CLASS_LEVEL_HORIZONTAL_GAP;
        let base_extent = relation_graph::horizontal_box_strip_extent(&boxes, gap, resources)?;
        let lines = relation_graph::render_relation_document_with_summary(
            base_extent,
            &summary_rows,
            None,
            options,
            resources,
            |resources| {
                relation_graph::render_horizontal_box_strip_lines(
                    &boxes,
                    direction.horizontal_direction(),
                    gap,
                    options.terminal_width_profile,
                    resources,
                )
            },
        )?;
        return render_class_document_lines(lines, options, resources);
    }
    if direction.is_reversed() {
        let base_extent = relation_graph::stacked_box_extent(&boxes, resources)?;
        let lines = relation_graph::render_relation_document_with_summary(
            base_extent,
            &summary_rows,
            None,
            options,
            resources,
            |resources| {
                relation_graph::stacked_box_lines_ordered(
                    &boxes,
                    options.terminal_width_profile,
                    true,
                    resources,
                )
            },
        )?;
        return render_class_document_lines(lines, options, resources);
    }

    relation_graph::render_stacked_boxes_with_relation_summary(
        &boxes,
        &summary_rows,
        None,
        options,
        resources,
    )
}
fn render_namespaced_class_boxes(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &HashMap<String, String>,
    resources: &mut ResourceContext,
) -> Result<Vec<RenderedClassBox>> {
    let note_by_id = ClassNoteIndex::new(&model.notes, resources)?;
    let context = NamespaceRenderContext {
        model,
        options,
        charset,
        direction,
        namespace_facade_aliases,
        note_by_id,
    };
    let mut rendered_namespace_ids = HashSet::new();
    rendered_namespace_ids
        .try_reserve(model.namespaces.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut visiting_namespace_ids = HashSet::new();
    visiting_namespace_ids
        .try_reserve(model.namespaces.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut boxes = Vec::new();
    let box_capacity = model
        .namespaces
        .len()
        .checked_add(model.classes.len())
        .and_then(|value| value.checked_add(model.interfaces.len()))
        .and_then(|value| value.checked_add(model.notes.len()))
        .ok_or_else(|| work_overflow(resources))?;
    boxes
        .try_reserve_exact(box_capacity)
        .map_err(|_| layout_allocation_failed())?;

    for namespace in model
        .namespaces
        .values()
        .filter(|namespace| namespace.parent.is_none())
    {
        boxes.push(render_namespace_box(
            &context,
            namespace,
            &mut rendered_namespace_ids,
            &mut visiting_namespace_ids,
            1,
            resources,
        )?);
    }

    for namespace in model.namespaces.values() {
        if !rendered_namespace_ids.contains(namespace.id.as_str()) {
            boxes.push(render_namespace_box(
                &context,
                namespace,
                &mut rendered_namespace_ids,
                &mut visiting_namespace_ids,
                1,
                resources,
            )?);
        }
    }

    for class in model.classes.values().filter(|class| {
        !namespace_facade_aliases.contains_key(class.id.as_str())
            && class
                .parent
                .as_ref()
                .is_none_or(|parent| !rendered_namespace_ids.contains(parent))
    }) {
        boxes.push(render_class_box(class, options, charset, resources)?);
    }
    for interface in &model.interfaces {
        boxes.push(render_interface_box(
            interface, options, charset, resources,
        )?);
    }
    for note in model.notes.iter().filter(|note| {
        note.parent
            .as_ref()
            .is_none_or(|parent| !rendered_namespace_ids.contains(parent))
    }) {
        boxes.push(render_note_box(note, options, charset, resources)?);
    }

    Ok(boxes)
}

fn render_namespace_box(
    context: &NamespaceRenderContext<'_>,
    namespace: &Namespace,
    rendered_namespace_ids: &mut HashSet<String>,
    visiting_namespace_ids: &mut HashSet<String>,
    depth: usize,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    resources.check_nesting_depth(depth)?;
    resources.charge_layout_work(context.model.namespaces.len().max(1))?;
    if !visiting_namespace_ids.insert(namespace.id.clone()) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "cyclic class namespace nesting",
        });
    }
    rendered_namespace_ids.insert(namespace.id.clone());
    let child_capacity = context
        .model
        .namespaces
        .len()
        .checked_add(namespace.class_ids.len())
        .and_then(|value| value.checked_add(namespace.note_ids.len()))
        .ok_or_else(|| work_overflow(resources))?;
    let mut children = Vec::new();
    children
        .try_reserve_exact(child_capacity)
        .map_err(|_| layout_allocation_failed())?;

    for child in context
        .model
        .namespaces
        .values()
        .filter(|child| child.parent.as_deref() == Some(namespace.id.as_str()))
    {
        children.push(render_namespace_box(
            context,
            child,
            rendered_namespace_ids,
            visiting_namespace_ids,
            depth
                .checked_add(1)
                .ok_or_else(|| nesting_overflow(resources))?,
            resources,
        )?);
    }

    let mut direct_boxes = Vec::new();
    let direct_box_capacity = namespace
        .class_ids
        .len()
        .checked_add(namespace.note_ids.len())
        .ok_or_else(|| work_overflow(resources))?;
    direct_boxes
        .try_reserve_exact(direct_box_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for class_id in &namespace.class_ids {
        if let Some(class) = context.model.classes.get(class_id)
            && !context
                .namespace_facade_aliases
                .contains_key(class.id.as_str())
        {
            direct_boxes.push(render_class_box(
                class,
                context.options,
                context.charset,
                resources,
            )?);
        }
    }

    for note_id in &namespace.note_ids {
        if let Some(note) = context.note_by_id.get(note_id, resources)? {
            direct_boxes.push(render_note_box(
                note,
                context.options,
                context.charset,
                resources,
            )?);
        }
    }
    let box_by_id = RenderedClassBoxIndex::new(&direct_boxes, resources)?;

    let direct_layout_capacity = context
        .model
        .relations
        .len()
        .checked_add(context.model.notes.len())
        .ok_or_else(|| work_overflow(resources))?;
    let mut direct_layouts = Vec::new();
    direct_layouts
        .try_reserve_exact(direct_layout_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for relation in context.model.relations.iter().filter(|relation| {
        relation_explicit_namespace_id(context.model, relation, context.namespace_facade_aliases)
            == Some(namespace.id.as_str())
    }) {
        direct_layouts.push(relation_layout(
            context.model,
            relation,
            context.namespace_facade_aliases,
            context.options.terminal_width_profile,
            resources,
        )?);
    }

    direct_layouts.extend(note_relation_layouts_for_notes(
        context.model.notes.iter().filter(|note| {
            note.parent.as_deref() == Some(namespace.id.as_str()) && note.class_id.is_some()
        }),
        context.namespace_facade_aliases,
        &box_by_id,
        resources,
    )?);
    for layout in &mut direct_layouts {
        layout.apply_direction(context.direction);
    }

    if direct_layouts.is_empty() {
        children.extend(direct_boxes);
    } else {
        let component_lines = if context.direction.is_horizontal() {
            render_horizontal_class_component_lines(
                &direct_boxes,
                &box_by_id,
                &direct_layouts,
                context.direction,
                context.options,
                context.charset,
                resources,
            )?
        } else {
            render_class_component_lines(
                &direct_boxes,
                &box_by_id,
                &direct_layouts,
                context.options,
                context.charset,
                resources,
            )?
        };
        children.push(RelationGraphBox::from_rendered_lines(
            format!("{}::relations", namespace.id),
            component_lines,
            context.options.terminal_width_profile,
            resources,
        )?);
    }

    visiting_namespace_ids.remove(namespace.id.as_str());
    render_namespace_container_box(
        namespace,
        children,
        context.options,
        context.charset,
        context.direction,
        resources,
    )
}

pub(super) fn render_namespace_container_box(
    namespace: &Namespace,
    mut children: Vec<RenderedClassBox>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    if direction.is_horizontal() && children.len() > 1 {
        let lines = relation_graph::render_horizontal_box_strip_lines(
            &children,
            direction.horizontal_direction(),
            CLASS_LEVEL_HORIZONTAL_GAP,
            options.terminal_width_profile,
            resources,
        )?;
        let contents = RelationGraphBox::from_rendered_lines(
            format!("{}::contents", namespace.id),
            lines,
            options.terminal_width_profile,
            resources,
        )?;
        children.clear();
        children.push(contents);
    } else if direction == ClassDirection::BottomUp {
        children.reverse();
    }

    let title = namespace_title(namespace);
    let inner_gap = options.box_border_padding;
    let inner_width = children
        .iter()
        .map(RelationGraphBox::width)
        .max()
        .unwrap_or_else(|| {
            crate::text::display_width_with_profile(&title, options.terminal_width_profile)
        })
        .max(crate::text::display_width_with_profile(
            &title,
            options.terminal_width_profile,
        ));
    let content_width = resources.checked_grid_add(
        inner_width,
        resources.checked_grid_mul(options.box_border_padding, 2)?,
    )?;
    let child_rows = children.iter().try_fold(0usize, |height, child| {
        resources.checked_grid_add(height, child.height())
    })?;
    let child_gaps = children.len().saturating_sub(1);
    let body_rows = if children.is_empty() {
        1
    } else {
        resources.checked_grid_add(child_rows, child_gaps)?
    };
    let height = resources.checked_grid_add(body_rows, 4)?;
    let width = resources.checked_grid_add(content_width, 2)?;
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let style = RelationGraphBoxStyle {
        top_left: charset.top_left,
        top_right: charset.top_right,
        bottom_left: charset.bottom_left,
        bottom_right: charset.bottom_right,
        horizontal: charset.horizontal,
        vertical: charset.vertical,
        separator_left: charset.separator_left,
        separator_right: charset.separator_right,
        border_role: AsciiColorRole::GroupBorder,
        text_role: AsciiColorRole::Text,
    };
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    lines.push(RelationGraphLine::try_box_border(
        style.top_left,
        style.top_right,
        style.horizontal,
        content_width,
        style.border_role,
        options.terminal_width_profile,
        resources,
    )?);
    lines.push(RelationGraphLine::box_content(
        &title,
        content_width,
        options.box_border_padding,
        style,
        options.terminal_width_profile,
        resources,
    )?);
    lines.push(RelationGraphLine::try_box_border(
        style.separator_left,
        style.separator_right,
        style.horizontal,
        content_width,
        style.border_role,
        options.terminal_width_profile,
        resources,
    )?);

    for (child_index, child) in children.iter().enumerate() {
        if child_index > 0 {
            lines.push(namespace_empty_content_line(
                content_width,
                style.vertical,
                style.border_role,
                options.terminal_width_profile,
                resources,
            )?);
        }
        lines.extend(namespace_child_lines(
            child,
            content_width,
            style.vertical,
            style.border_role,
            inner_gap,
            resources,
        )?);
    }

    if children.is_empty() {
        lines.push(namespace_empty_content_line(
            content_width,
            style.vertical,
            style.border_role,
            options.terminal_width_profile,
            resources,
        )?);
    }

    lines.push(RelationGraphLine::try_box_border(
        style.bottom_left,
        style.bottom_right,
        style.horizontal,
        content_width,
        style.border_role,
        options.terminal_width_profile,
        resources,
    )?);

    Ok(RelationGraphBox::new_with_lines(
        namespace.id.clone(),
        lines,
        width,
        options.terminal_width_profile,
    ))
}

fn namespace_child_lines(
    child: &RenderedClassBox,
    content_width: usize,
    vertical: char,
    border_role: AsciiColorRole,
    inner_gap: usize,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let used_width = resources.checked_grid_add(inner_gap, child.width())?;
    let trailing = content_width
        .checked_sub(used_width)
        .ok_or_else(|| grid_overflow(resources))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(child.height())
        .map_err(|_| layout_allocation_failed())?;
    let vertical_text = vertical.to_string();
    for line in child.lines() {
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(5)
            .map_err(|_| layout_allocation_failed())?;
        parts.push(RelationGraphLine::try_with_role(
            &vertical_text,
            border_role,
            child.width_profile(),
            resources,
        )?);
        parts.push(RelationGraphLine::try_blank(
            inner_gap,
            child.width_profile(),
            resources,
        )?);
        parts.push(line.clone());
        parts.push(RelationGraphLine::try_blank(
            trailing,
            child.width_profile(),
            resources,
        )?);
        parts.push(RelationGraphLine::try_with_role(
            &vertical_text,
            border_role,
            child.width_profile(),
            resources,
        )?);
        lines.push(relation_graph::try_concat_relation_lines(
            parts,
            child.width_profile(),
            resources,
        )?);
    }
    Ok(lines)
}

fn namespace_empty_content_line(
    content_width: usize,
    vertical: char,
    border_role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    RelationGraphLine::try_box_border(
        vertical,
        vertical,
        ' ',
        content_width,
        border_role,
        width_profile,
        resources,
    )
}

fn namespace_title(namespace: &Namespace) -> String {
    let normalized_label = normalize_terminal_text(&namespace.label);
    let raw = if normalized_label.trim().is_empty() {
        namespace.id.as_str()
    } else {
        normalized_label.as_ref()
    };
    decode_html_entities_to_unicode(raw).into_owned()
}

pub(super) fn namespace_facade_aliases(model: &ClassDiagram) -> Result<HashMap<String, String>> {
    let mut aliases = HashMap::new();
    aliases
        .try_reserve(model.classes.len())
        .map_err(|_| layout_allocation_failed())?;
    for class in model.classes.values() {
        let Some(local_id) = namespace_facade_local_id(model, class) else {
            continue;
        };
        aliases.insert(class.id.clone(), local_id.to_string());
    }
    Ok(aliases)
}

fn namespace_facade_local_id<'a>(model: &'a ClassDiagram, class: &'a ClassNode) -> Option<&'a str> {
    if class
        .parent
        .as_deref()
        .map(str::trim)
        .is_some_and(|parent| !parent.is_empty())
        || !class.annotations.is_empty()
        || !class.members.is_empty()
        || !class.methods.is_empty()
    {
        return None;
    }

    model
        .namespaces
        .values()
        .filter_map(|namespace| {
            let remainder = class
                .id
                .strip_prefix(namespace.id.as_str())?
                .strip_prefix('.')?;
            namespace
                .class_ids
                .iter()
                .any(|id| id == remainder)
                .then_some((namespace.id.len(), remainder))
        })
        .max_by_key(|(namespace_len, _)| *namespace_len)
        .and_then(|(_, local_id)| model.classes.contains_key(local_id).then_some(local_id))
}
