//! TreeView source containers projected from public semantic ownership.

use super::*;
use merman_display_list::TextStyle;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct TextCssStyle<'a> {
    style: &'a TextStyle,
    anchor: TextAnchor,
    direction: TextDirection,
}

fn text_css_style(run: &TextRun) -> Option<TextCssStyle<'_>> {
    (matches!(run.obligation, TextObligation::HostText { .. })
        && run.style.font.resource.is_none()
        && run.style.stroke.is_none()
        && matches!(run.style.fill, Paint::Solid { .. })
        && !run.text.contains(['\n', '\r']))
    .then_some(TextCssStyle {
        style: &run.style,
        anchor: run.anchor,
        direction: run.direction,
    })
}

/// Class metadata only chooses the CSS scope. Every declaration comes from matching public
/// commands; one heterogeneous or unsupported sibling disables sharing for the whole class.
pub(super) fn shared_text_styles<'a>(
    document: &'a DrawingListDocument,
    body: &crate::drawing_list::TreeViewSvgBody,
    session: &RenderSession,
) -> Result<BTreeMap<String, Option<TextCssStyle<'a>>>> {
    let mut styles = BTreeMap::new();
    let mut groups = Vec::new();
    for command in &document.commands {
        session.checkpoint(OperationPhase::Emit)?;
        match command {
            DrawingCommand::BeginSemanticGroup { semantic_id } => {
                groups
                    .try_reserve(1)
                    .map_err(|_| crate::Error::DrawingListAllocationFailed {
                        collection: "TreeView SVG text scopes",
                    })?;
                groups.push(semantic_id.as_str());
            }
            DrawingCommand::EndSemanticGroup => {
                groups.pop();
            }
            DrawingCommand::DrawText { run } => {
                let Some(class) = groups.last().and_then(|id| body.text_classes.get(*id)) else {
                    continue;
                };
                // XML normalizes literal control whitespace in attribute values. Leave such
                // edited class metadata to the generic projection instead of sharing a rule
                // whose exact attribute selector would observe a different string.
                let candidate = (!class.chars().any(char::is_control))
                    .then(|| text_css_style(run))
                    .flatten();
                styles
                    .entry(class.clone())
                    .and_modify(|shared| {
                        if *shared != candidate {
                            *shared = None;
                        }
                    })
                    .or_insert(candidate);
            }
            _ => {}
        }
    }
    Ok(styles)
}

/// Only source line/highlight shapes with a uniform public style can share presentation CSS.
/// Unsupported or edited geometry disables sharing for its class, including otherwise ordinary
/// siblings, so CSS never overrides a generic path's explicit paint.
pub(super) fn shared_path_styles<'a>(
    document: &'a DrawingListDocument,
    body: &crate::drawing_list::TreeViewSvgBody,
    resources: &BTreeMap<String, &'a DrawingResource>,
    session: &RenderSession,
) -> Result<BTreeMap<String, Option<&'a PathStyle>>> {
    let mut styles = BTreeMap::new();
    for command in &document.commands {
        session.checkpoint(OperationPhase::Emit)?;
        let DrawingCommand::DrawPath { path, style } = command else {
            continue;
        };
        let Some(class) = body.path_classes.get(path.as_str()) else {
            continue;
        };
        if !matches!(
            class.as_str(),
            "treeView-node-line" | "treeView-highlight-bg"
        ) {
            continue;
        }
        let geometry_matches = resources.get(path.as_str()).is_some_and(|resource| {
            let DrawingResource::Path(path) = resource else {
                return false;
            };
            if class == "treeView-node-line" {
                line_from_path(path).is_some()
            } else {
                rounded_rectangle_from_path(path).is_some()
            }
        });
        let candidate = (geometry_matches
            && style.fill_rule == FillRule::NonZero
            && style
                .fill
                .as_ref()
                .is_none_or(|paint| matches!(paint, Paint::Solid { .. }))
            && style
                .stroke
                .as_ref()
                .is_none_or(|stroke| matches!(stroke.paint, Paint::Solid { .. })))
        .then_some(style);
        styles
            .entry(class.clone())
            .and_modify(|shared| {
                if *shared != candidate {
                    *shared = None;
                }
            })
            .or_insert(candidate);
    }
    Ok(styles)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn write_tree_view_styles(&mut self) -> Result<()> {
        self.output.push_str("<style>")?;
        for (class, shared) in &self.tree_view_text_styles {
            self.session.checkpoint(OperationPhase::Emit)?;
            let Some(shared) = shared else {
                continue;
            };
            let style = shared.style;
            let Paint::Solid { color, .. } = style.fill else {
                return Err(invalid("TreeView shared text paint must be solid"));
            };
            let font = self.font_families(&style.font)?;
            // Exact class attributes prevent a plain-label rule from matching a directory or
            // custom multi-token label. Escape the metadata as a CSS string, not a selector.
            // Auto has the same inherited direction as the generic SVG direction="auto"
            // projection; CSS direction itself accepts only ltr/rtl/inherit.
            let direction = match shared.direction {
                TextDirection::Auto => "inherit",
                TextDirection::Ltr => "ltr",
                TextDirection::Rtl => "rtl",
            };
            write!(
                self.output,
                "#{} text[class=\"{}\"]{{font-family:{};font-size:{}px;font-weight:{};font-style:{};letter-spacing:{}px;line-height:{}px;fill:{};fill-opacity:{};stroke:none;white-space:pre;direction:{};text-anchor:{};}}",
                self.diagram_id,
                escaped_css_string(class).replace('&', "\\26 "),
                font,
                fmt(style.font_size),
                style.font.weight,
                font_style(style.font.style),
                fmt(style.letter_spacing),
                fmt(style.line_height),
                color_css(color),
                fmt(paint_opacity(&style.fill)),
                direction,
                text_anchor(shared.anchor),
            )?;
        }
        for (class, shared) in &self.tree_view_path_styles {
            self.session.checkpoint(OperationPhase::Emit)?;
            let Some(style) = shared else { continue };
            let tag = if class == "treeView-node-line" {
                "line"
            } else {
                "rect"
            };
            write!(
                self.output,
                "#{} {}[class=\"{}\"]{{",
                self.diagram_id, tag, class
            )?;
            if let Some(paint @ Paint::Solid { color, .. }) = &style.fill {
                write!(
                    self.output,
                    "fill:{};fill-opacity:{};",
                    color_css(*color),
                    fmt(paint_opacity(paint))
                )?;
            } else {
                self.output.push_str("fill:none;")?;
            }
            if let Some(stroke) = &style.stroke {
                let Paint::Solid { color, .. } = stroke.paint else {
                    return Err(invalid("TreeView shared stroke must be solid"));
                };
                write!(
                    self.output,
                    "stroke:{};stroke-opacity:{};stroke-linecap:{};stroke-linejoin:{};stroke-miterlimit:{};stroke-dashoffset:{};stroke-dasharray:",
                    color_css(color),
                    fmt(paint_opacity(&stroke.paint)),
                    line_cap(stroke.line_cap),
                    line_join(stroke.line_join),
                    fmt(stroke.miter_limit),
                    fmt(stroke.dash_offset)
                )?;
                if stroke.dash_array.is_empty() {
                    self.output.push_str("none")?;
                }
                for (index, dash) in stroke.dash_array.iter().enumerate() {
                    if index != 0 {
                        self.output.push(',')?;
                    }
                    write!(self.output, "{}", fmt(*dash))?;
                }
                self.output.push(';')?;
                if tag == "rect" {
                    write!(self.output, "stroke-width:{};", fmt(stroke.width))?;
                }
            } else {
                self.output.push_str("stroke:none;")?;
            }
            self.output.push('}')?;
        }
        self.output.push_str("</style>")
    }

    pub(super) fn emit_compact_tree_view_path(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        let SvgStructureBody::TreeView(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(class) = body.path_classes.get(path_id.as_str()) else {
            return Ok(false);
        };
        if self.tree_view_path_styles.get(class).copied().flatten() != Some(style) {
            return Ok(false);
        }
        let path = self.path_resource(path_id)?;
        if class == "treeView-node-line" {
            let Some((start, end)) = line_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"",
                fmt(start.x),
                fmt(start.y),
                fmt(end.x),
                fmt(end.y)
            )?;
            if let Some(stroke) = &style.stroke {
                write!(self.output, " stroke-width=\"{}\"", fmt(stroke.width))?;
            }
        } else if class == "treeView-highlight-bg" {
            let Some((bounds, radius)) = rounded_rectangle_from_path(path) else {
                return Ok(false);
            };
            write!(
                self.output,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"",
                fmt(bounds.x),
                fmt(bounds.y),
                fmt(bounds.width),
                fmt(bounds.height),
                fmt(radius)
            )?;
        } else {
            return Ok(false);
        }
        write!(self.output, " class=\"{}\"", class)?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        self.write_state_attrs()?;
        self.write_path_metadata(path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    pub(super) fn emit_compact_tree_view_text(
        &mut self,
        run: &TextRun,
        semantic_id: Option<&str>,
    ) -> Result<bool> {
        let SvgStructureBody::TreeView(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(class) = semantic_id.and_then(|id| body.text_classes.get(id)) else {
            return Ok(false);
        };
        let Some(candidate) = text_css_style(run) else {
            return Ok(false);
        };
        if self.tree_view_text_styles.get(class).copied().flatten() != Some(candidate) {
            return Ok(false);
        }
        self.session.checkpoint(OperationPhase::Emit)?;
        let baseline = match run.baseline {
            TextBaseline::Middle => "middle",
            other => text_baseline(other),
        };
        write!(
            self.output,
            "<text dominant-baseline=\"{}\" class=\"{}\" x=\"{}\" y=\"{}\"",
            baseline,
            escaped_attr(class),
            fmt(run.origin.x),
            fmt(run.origin.y),
        )?;
        if let Some(id) = semantic_id {
            self.write_sidecar_dom_id(id)?;
        }
        if let Some(language) = run.language.as_deref() {
            self.output.push_str(" xml:lang=\"")?;
            output::escape_attr(&mut self.output, language)?;
            self.output.push('"')?;
        }
        write_text_metadata(&mut self.output, self.debug, run)?;
        self.write_tree_view_leaf_metadata()?;
        self.write_state_attrs()?;
        self.output.push('>')?;
        output::escape_xml(&mut self.output, &run.text)?;
        self.output.push_str("</text>")?;
        Ok(true)
    }

    pub(super) fn begin_tree_view_semantic_group(
        &mut self,
        semantic_id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let SvgStructureBody::TreeView(body) = self.svg_body else {
            return Ok(false);
        };
        if semantic.link.is_some() || !self.debug_visibility(semantic.role) {
            return Ok(false);
        }
        let class = body.semantic_classes.get(semantic_id).map(String::as_str);
        let tail = &self.document.commands[self.command_index + 1..];
        let emitted = match (class, semantic.role) {
            (Some("tree-view"), SemanticRole::Document) if semantic_id == "treeView.document" => {
                // Root title/description already have their own labelledby/describedby targets.
                true
            }
            (Some("treeView-node"), SemanticRole::Node) => {
                if !self.tree_view_node_names_are_visible(semantic_id, semantic)? {
                    return Ok(false);
                }
                true
            }
            (
                Some("treeView-node-label-group" | "treeView-node-description-group"),
                SemanticRole::Label,
            ) => {
                let [
                    DrawingCommand::DrawText { run },
                    DrawingCommand::EndSemanticGroup,
                    ..,
                ] = tail
                else {
                    return Ok(false);
                };
                if semantic.description.is_some()
                    || semantic
                        .title
                        .as_deref()
                        .is_some_and(|name| name != run.text)
                    || !matches!(run.obligation, TextObligation::HostText { .. })
                {
                    return Ok(false);
                }
                false
            }
            (Some("treeView-node-line"), SemanticRole::Edge) => {
                if semantic.title.is_some()
                    || semantic.description.is_some()
                    || !matches!(
                        tail,
                        [
                            DrawingCommand::DrawPath { .. },
                            DrawingCommand::EndSemanticGroup,
                            ..
                        ] | [DrawingCommand::EndSemanticGroup, ..]
                    )
                {
                    return Ok(false);
                }
                false
            }
            _ => return Ok(false),
        };
        if emitted {
            self.output.push_str("<g")?;
            if semantic.role == SemanticRole::Document {
                self.output.push_str(" class=\"tree-view\"")?;
            }
            write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
            self.output.push('>')?;
        }
        self.groups.push(GroupKind::Semantic {
            linked: false,
            emitted,
            semantic_id: semantic_id.to_owned(),
            projected_transform: Transform::IDENTITY,
        });
        Ok(true)
    }

    fn tree_view_node_names_are_visible(
        &self,
        semantic_id: &str,
        semantic: &SemanticAnnotation,
    ) -> Result<bool> {
        let mut title_visible = semantic.title.is_none();
        let mut description_visible = semantic.description.is_none();
        let mut depth = 0;
        let tail = &self.document.commands[self.command_index + 1..];
        for (index, command) in tail.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            match command {
                DrawingCommand::BeginSemanticGroup { semantic_id: child } => {
                    // Source nodes are siblings, never nested containers. Refuse unexpected
                    // nesting rather than repeatedly scanning overlapping descendant subtrees.
                    if depth != 0 {
                        return Ok(false);
                    }
                    depth += 1;
                    let [
                        DrawingCommand::DrawText { run },
                        DrawingCommand::EndSemanticGroup,
                        ..,
                    ] = &tail[index + 1..]
                    else {
                        return Ok(false);
                    };
                    let Some(annotation) = self.semantics.get(child) else {
                        return Ok(false);
                    };
                    if !self.debug_visibility(SemanticRole::Label)
                        || !matches!(run.obligation, TextObligation::HostText { .. })
                        || annotation.role != SemanticRole::Label
                        || annotation.link.is_some()
                        || annotation.description.is_some()
                        || annotation
                            .title
                            .as_deref()
                            .is_some_and(|name| name != run.text)
                    {
                        return Ok(false);
                    }
                    match child.strip_prefix(semantic_id) {
                        Some(".label") => {
                            title_visible |= semantic.title.as_deref() == Some(run.text.as_str())
                        }
                        Some(".description") => {
                            description_visible |=
                                semantic.description.as_deref() == Some(run.text.as_str())
                        }
                        _ => return Ok(false),
                    }
                }
                DrawingCommand::EndSemanticGroup => {
                    if depth == 0 {
                        return Ok(title_visible && description_visible);
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        Ok(false)
    }

    /// Collapsed label/edge scopes keep their diagnostic association on the actual leaf.
    pub(super) fn write_tree_view_leaf_metadata(&mut self) -> Result<()> {
        if matches!(self.svg_body, SvgStructureBody::TreeView(_))
            && let Some(GroupKind::Semantic {
                semantic_id,
                emitted: false,
                ..
            }) = self.groups.last()
        {
            write_semantic_metadata(&mut self.output, self.debug, semantic_id)?;
        }
        Ok(())
    }
}
