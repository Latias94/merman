//! Registered icon viewport structure, with public clipping and matrix compensation.

use super::*;
use crate::drawing_list::{
    AssetStylePlacement, AssetStyleProperties, AssetStyleProperty as Property,
};

#[derive(Default)]
pub(super) struct ScopeProjections<'a> {
    scopes: BTreeMap<usize, &'a crate::drawing_list::AssetScope>,
    paints: BTreeMap<usize, (&'a PathStyle, AssetStylePlacement)>,
    leaf_omissions: BTreeMap<usize, AssetStyleProperties>,
}

struct SharedPaint<'a> {
    style: Option<&'a PathStyle>,
    uniform: bool,
    inherited: AssetStyleProperties,
}

/// Match scopes once, rather than rescanning a subtree for each source group.
pub(super) fn scope_projections<'a>(
    document: &'a DrawingListDocument,
    body: &'a crate::drawing_list::TreeViewSvgBody,
    session: &RenderSession,
) -> Result<ScopeProjections<'a>> {
    let mut result = ScopeProjections::default();
    if body.assets.scopes.is_empty() {
        return Ok(result);
    }
    let mut saves = Vec::new();
    let mut current_group = None;
    let mut paints: BTreeMap<usize, SharedPaint<'_>> = BTreeMap::new();
    let mut leaves = BTreeMap::new();
    for (index, command) in document.commands.iter().enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        match command {
            DrawingCommand::Save => {
                saves
                    .try_reserve(1)
                    .map_err(|_| crate::Error::DrawingListAllocationFailed {
                        collection: "icon SVG scopes",
                    })?;
                saves.push((index, current_group));
                if body.assets.scopes.get(&index).is_some_and(|scope| {
                    matches!(scope.kind, crate::drawing_list::AssetScopeKind::Group)
                }) {
                    // Nested groups are boundaries: the inner group may be promoted independently.
                    if let Some(parent) = current_group.and_then(|index| paints.get_mut(&index)) {
                        parent.uniform = false;
                    }
                    current_group = Some(index);
                    paints.insert(
                        index,
                        SharedPaint {
                            style: None,
                            uniform: true,
                            inherited: AssetStyleProperties::default(),
                        },
                    );
                }
            }
            DrawingCommand::Restore => {
                if let Some((start, parent)) = saves.pop() {
                    if let Some(group) = body.assets.scopes.get(&start)
                        && group.end == index
                    {
                        result.scopes.insert(start, group);
                    }
                    current_group = parent;
                }
            }
            DrawingCommand::DrawPath { path, style } => {
                if let Some((group, candidate)) = current_group
                    .and_then(|group| paints.get_mut(&group).map(|candidate| (group, candidate)))
                {
                    let Some(placement) = body.assets.styles.get(path.as_str()) else {
                        candidate.uniform = false;
                        continue;
                    };
                    if matches!(style.fill, Some(Paint::Resource { .. }))
                        || style
                            .stroke
                            .as_ref()
                            .is_some_and(|s| matches!(s.paint, Paint::Resource { .. }))
                    {
                        candidate.uniform = false;
                    }
                    if candidate.style.is_some_and(|existing| existing != style) {
                        candidate.uniform = false;
                    }
                    candidate.style.get_or_insert(style);
                    candidate.inherited = candidate
                        .inherited
                        .union(AssetStyleProperties::ALL.without(placement.all()));
                    leaves.insert(index, (group, placement.all()));
                }
            }
            DrawingCommand::DrawText { .. }
            | DrawingCommand::DrawImage { .. }
            | DrawingCommand::DrawRasterSubtree { .. }
            | DrawingCommand::BeginLayer { .. }
            | DrawingCommand::BeginSemanticGroup { .. }
            | DrawingCommand::ClipPath { .. } => {
                if let Some(candidate) = current_group.and_then(|index| paints.get_mut(&index)) {
                    candidate.uniform = false;
                }
            }
            _ => {}
        }
    }
    for (index, candidate) in paints {
        session.checkpoint(OperationPhase::Emit)?;
        let (Some(scope), Some(style)) = (result.scopes.get(&index), candidate.style) else {
            continue;
        };
        if !candidate.uniform {
            continue;
        }
        // Intrinsic color alpha and inherited paint opacity are separate public values.
        let mut inherited = candidate.inherited;
        if style.stroke.is_none() {
            inherited = inherited.without(AssetStyleProperties::of(&[
                Property::StrokeWidth,
                Property::StrokeLineCap,
                Property::StrokeLineJoin,
                Property::StrokeMiterLimit,
                Property::StrokeDashArray,
                Property::StrokeDashOffset,
            ]));
        }
        let placement = scope.style.retain(inherited);
        if !placement.all().is_empty() {
            result.paints.insert(index, (style, placement));
        }
    }
    for (index, (group, local)) in leaves {
        session.checkpoint(OperationPhase::Emit)?;
        if let Some((_, placement)) = result.paints.get(&group) {
            result
                .leaf_omissions
                .insert(index, placement.all().without(local));
        }
    }
    Ok(result)
}

#[derive(Clone, Copy)]
pub(super) struct AssetViewport<'a> {
    clip_id: &'a ResourceId,
    viewport: Rect,
    view_box: Rect,
    compensation: Transform,
}

pub(super) fn projections<'a>(
    document: &'a DrawingListDocument,
    body: &crate::drawing_list::TreeViewSvgBody,
    resources: &BTreeMap<String, &'a DrawingResource>,
    session: &RenderSession,
) -> Result<BTreeMap<usize, AssetViewport<'a>>> {
    let mut result = BTreeMap::new();
    if body.asset_view_boxes.is_empty() {
        return Ok(result);
    }
    for (index, commands) in document.commands.windows(2).enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        let [
            DrawingCommand::ClipPath { path, .. },
            DrawingCommand::ConcatTransform { transform },
        ] = commands
        else {
            continue;
        };
        let Some(view_box) = body.asset_view_boxes.get(path.as_str()).copied() else {
            continue;
        };
        let Some(DrawingResource::Path(clip)) = resources.get(path.as_str()).copied() else {
            continue;
        };
        let Some(viewport) = rectangle_from_path(clip) else {
            continue;
        };
        let Some(compensation) = compensate_view_box(viewport, view_box, *transform) else {
            continue;
        };
        result.insert(
            index,
            AssetViewport {
                clip_id: path,
                viewport,
                view_box,
                compensation,
            },
        );
    }
    Ok(result)
}

/// Nested SVG applies xMidYMid meet. Its basis V is only a spelling of the public M:
/// children receive V^-1 M, so edits to either the public matrix or the hint remain truthful.
fn compensate_view_box(viewport: Rect, view_box: Rect, public: Transform) -> Option<Transform> {
    if ![
        viewport.x,
        viewport.y,
        viewport.width,
        viewport.height,
        view_box.x,
        view_box.y,
        view_box.width,
        view_box.height,
    ]
    .iter()
    .all(|n| n.is_finite())
        || viewport.width <= 0.0
        || viewport.height <= 0.0
        || view_box.width <= 0.0
        || view_box.height <= 0.0
    {
        return None;
    }
    let scale = (viewport.width / view_box.width).min(viewport.height / view_box.height);
    let basis = Transform {
        a: scale,
        d: scale,
        e: viewport.x + (viewport.width - scale * view_box.width) / 2.0 - scale * view_box.x,
        f: viewport.y + (viewport.height - scale * view_box.height) / 2.0 - scale * view_box.y,
        ..Transform::IDENTITY
    };
    if scale <= 0.0 || ![scale, basis.e, basis.f].iter().all(|n| n.is_finite()) {
        return None;
    }
    // Avoid a redundant, slightly rounded identity on the unedited source path.
    if basis == public {
        return Some(Transform::IDENTITY);
    }
    let compensation = Transform {
        a: public.a / scale,
        b: public.b / scale,
        c: public.c / scale,
        d: public.d / scale,
        e: (public.e - basis.e) / scale,
        f: (public.f - basis.f) / scale,
    };
    [
        compensation.a,
        compensation.b,
        compensation.c,
        compensation.d,
        compensation.e,
        compensation.f,
    ]
    .iter()
    .all(|n| n.is_finite())
    .then_some(compensation)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_tree_view_asset_scope(&mut self, index: usize) -> Result<Option<usize>> {
        let Some(&group) = self.tree_view_asset_scopes.scopes.get(&index) else {
            return Ok(None);
        };
        let commands = &self.document.commands[index + 1..group.end];
        let prefix = group
            .transform
            .as_ref()
            .map(|source| source.matching_prefix(commands, self.session))
            .transpose()?
            .flatten();
        let count = prefix.unwrap_or(0);
        let parent = self.state.transform;
        if let crate::drawing_list::AssetScopeKind::EmptyPrimitive(geometry) = &group.kind {
            // An inert source element never conceals a newly added public draw command.
            for command in commands {
                self.session.checkpoint(OperationPhase::Emit)?;
                if !matches!(command, DrawingCommand::ConcatTransform { .. }) {
                    return Ok(None);
                }
            }
            if !geometry.matches_path(&[], self.session)? {
                return Ok(None);
            }
            let saved = self.state;
            for command in commands {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_command(command)?;
            }
            self.output.push('<')?;
            self.output.push_str(geometry.tag)?;
            for (key, value) in &geometry.attributes {
                self.output.push(' ')?;
                self.output.push_str(key)?;
                self.output.push_str("=\"")?;
                output::escape_attr(&mut self.output, value)?;
                self.output.push('"')?;
            }
            self.write_asset_scope_attrs(group, parent, prefix == Some(commands.len()))?;
            // There is no public paint for a zero-extent/empty primitive.
            self.output.push_str(" fill=\"none\" stroke=\"none\"/>")?;
            self.state = saved;
            return Ok(Some(group.end - index + 1));
        }
        self.emit_command(&DrawingCommand::Save)?;
        for command in &commands[..count] {
            self.emit_command(command)?;
        }
        self.output.push_str("<g")?;
        self.write_asset_scope_attrs(group, parent, prefix.is_some())?;
        if let Some(&(style, placement)) = self.tree_view_asset_scopes.paints.get(&index) {
            self.write_asset_style_attributes(style, placement.attributes)?;
            self.write_asset_inline_properties(style, placement.inline, None)?;
        }
        self.output.push('>')?;
        // Group presentation does not change public graphics state; only reset the local matrix.
        self.state.transform = Transform::IDENTITY;
        self.groups.push(GroupKind::Asset);
        Ok(Some(count + 1))
    }

    fn write_asset_scope_attrs(
        &mut self,
        group: &crate::drawing_list::AssetScope,
        parent: Transform,
        source_matches: bool,
    ) -> Result<()> {
        if let Some(id) = &group.dom_id {
            self.output.push_str(" id=\"")?;
            output::escape_attr(&mut self.output, id)?;
            self.output.push('"')?;
        }
        if parent == Transform::IDENTITY
            && source_matches
            && let Some(transform) = &group.transform
        {
            write!(self.output, " transform=\"{}\"", escaped_attr(transform))?;
        } else if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform)
            )?;
        }
        Ok(())
    }

    pub(super) fn emit_tree_view_asset_primitive(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<bool> {
        let SvgStructureBody::TreeView(body) = self.svg_body else {
            return Ok(false);
        };
        let Some(geometry) = body.assets.primitives.get(path_id.as_str()) else {
            return Ok(false);
        };
        let path = self.path_resource(path_id)?;
        if geometry.matches_path(&path.segments, self.session)? {
            self.output.push('<')?;
            self.output.push_str(geometry.tag)?;
            for (key, value) in &geometry.attributes {
                self.output.push(' ')?;
                self.output.push_str(key)?;
                self.output.push_str("=\"")?;
                output::escape_attr(&mut self.output, value)?;
                self.output.push('"')?;
            }
        } else {
            // An edited path still owns its declaration placement and DOM identity.
            self.output.push_str("<path d=\"")?;
            output::escape_attr(&mut self.output, path_d(&path.segments))?;
            self.output.push('"')?;
        }
        self.write_asset_path_style(path_id, style)?;
        self.write_transform()?;
        if self.state.opacity != 1.0
            || body
                .assets
                .styles
                .get(path_id.as_str())
                .is_some_and(|placement| {
                    !placement
                        .attributes
                        .intersection(AssetStyleProperties::of(&[Property::Opacity]))
                        .is_empty()
                })
        {
            write!(self.output, " opacity=\"{}\"", fmt(self.state.opacity))?;
        }
        self.write_tree_view_asset_inline_style(path_id, style)?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        self.write_path_metadata(path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(true)
    }

    fn write_tree_view_asset_inline_style(
        &mut self,
        path_id: &ResourceId,
        style: &PathStyle,
    ) -> Result<()> {
        let SvgStructureBody::TreeView(body) = self.svg_body else {
            return Ok(());
        };
        let placement = body
            .assets
            .styles
            .get(path_id.as_str())
            .map(|p| p.inline)
            .unwrap_or_default();
        let blend = blend_css(self.state.blend_mode);
        self.write_asset_inline_properties(style, placement, blend)
    }

    fn write_asset_path_style(&mut self, path_id: &ResourceId, style: &PathStyle) -> Result<()> {
        let mut attributes =
            AssetStyleProperties::of(&[Property::FillRule, Property::Fill, Property::Stroke]);
        let mut available = attributes;
        if style.fill.is_some() {
            available = available.union(AssetStyleProperties::of(&[Property::FillOpacity]));
        }
        if style
            .fill
            .as_ref()
            .is_some_and(|paint| paint.opacity() != 1.0)
        {
            attributes = attributes.union(AssetStyleProperties::of(&[Property::FillOpacity]));
        }
        if let Some(stroke) = &style.stroke {
            available = available.union(AssetStyleProperties::of(&[
                Property::StrokeWidth,
                Property::StrokeLineCap,
                Property::StrokeLineJoin,
                Property::StrokeMiterLimit,
                Property::StrokeOpacity,
                Property::StrokeDashArray,
                Property::StrokeDashOffset,
            ]));
            attributes = attributes.union(AssetStyleProperties::of(&[
                Property::StrokeWidth,
                Property::StrokeLineCap,
                Property::StrokeLineJoin,
                Property::StrokeMiterLimit,
            ]));
            if stroke.paint.opacity() != 1.0 {
                attributes = attributes.union(AssetStyleProperties::of(&[Property::StrokeOpacity]));
            }
            if !stroke.dash_array.is_empty() {
                attributes =
                    attributes.union(AssetStyleProperties::of(&[Property::StrokeDashArray]));
            }
            if stroke.dash_offset != 0.0 {
                attributes =
                    attributes.union(AssetStyleProperties::of(&[Property::StrokeDashOffset]));
            }
        }
        if let SvgStructureBody::TreeView(body) = self.svg_body
            && let Some(placement) = body.assets.styles.get(path_id.as_str())
        {
            // Explicit defaults must not become inherited. Inactive paint parameters
            // have no public representation and are not reconstructed from source.
            attributes = attributes.union(placement.attributes.intersection(available));
        }
        let omit = self
            .tree_view_asset_scopes
            .leaf_omissions
            .get(&self.command_index)
            .copied()
            .unwrap_or_default();
        self.write_asset_style_attributes(style, attributes.without(omit))
    }

    fn write_asset_style_attributes(
        &mut self,
        style: &PathStyle,
        properties: AssetStyleProperties,
    ) -> Result<()> {
        for property in properties.properties() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.output.push(' ')?;
            self.output.push_str(property.name())?;
            self.output.push_str("=\"")?;
            self.write_asset_style_value(style, property)?;
            self.output.push('"')?;
        }
        Ok(())
    }

    fn write_asset_inline_properties(
        &mut self,
        style: &PathStyle,
        placement: AssetStyleProperties,
        blend: Option<&str>,
    ) -> Result<()> {
        if placement.is_empty() && blend.is_none() {
            return Ok(());
        }
        self.output.push_str(" style=\"")?;
        for property in placement.properties() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.output.push_str(property.name())?;
            self.output.push(':')?;
            self.write_asset_style_value(style, property)?;
            self.output.push(';')?;
        }
        if let Some(blend) = blend {
            write!(self.output, "mix-blend-mode:{blend};")?;
        }
        self.output.push('"')?;
        Ok(())
    }

    fn write_asset_style_value(&mut self, style: &PathStyle, property: Property) -> Result<()> {
        let stroke = style.stroke.as_ref();
        match property {
            Property::Fill | Property::Stroke => {
                let paint = if matches!(property, Property::Fill) {
                    style.fill.as_ref()
                } else {
                    stroke.map(|s| &s.paint)
                };
                match paint {
                    Some(Paint::Solid { color, .. }) => {
                        self.output.push_str(&color_css(*color))?;
                        if color.alpha != u8::MAX {
                            write!(self.output, "{:02x}", color.alpha)?;
                        }
                    }
                    Some(Paint::Resource { id, .. }) => {
                        let svg_id = self.svg_resource_id(id.as_str())?;
                        write!(self.output, "url(#{})", escaped_attr(&svg_id))?;
                    }
                    None => self.output.push_str("none")?,
                }
            }
            Property::FillOpacity | Property::StrokeOpacity => {
                let paint = if matches!(property, Property::FillOpacity) {
                    style.fill.as_ref()
                } else {
                    stroke.map(|s| &s.paint)
                };
                let opacity = paint.map_or(1.0, Paint::opacity);
                write!(self.output, "{}", fmt(opacity))?;
            }
            Property::FillRule => self.output.push_str(fill_rule_name(style.fill_rule))?,
            Property::StrokeWidth => {
                write!(self.output, "{}", fmt(stroke.map_or(0.0, |s| s.width)))?
            }
            Property::StrokeLineCap => self
                .output
                .push_str(stroke.map_or("butt", |s| line_cap(s.line_cap)))?,
            Property::StrokeLineJoin => self
                .output
                .push_str(stroke.map_or("miter", |s| line_join(s.line_join)))?,
            Property::StrokeMiterLimit => write!(
                self.output,
                "{}",
                fmt(stroke.map_or(4.0, |s| s.miter_limit))
            )?,
            Property::StrokeDashOffset => write!(
                self.output,
                "{}",
                fmt(stroke.map_or(0.0, |s| s.dash_offset))
            )?,
            Property::StrokeDashArray => {
                if let Some(stroke) = stroke
                    && !stroke.dash_array.is_empty()
                {
                    for (index, value) in stroke.dash_array.iter().enumerate() {
                        self.session.checkpoint(OperationPhase::Emit)?;
                        if index != 0 {
                            self.output.push(',')?;
                        }
                        write!(self.output, "{}", fmt(*value))?;
                    }
                } else {
                    self.output.push_str("none")?;
                }
            }
            Property::Opacity => write!(self.output, "{}", fmt(self.state.opacity))?,
        }
        Ok(())
    }

    pub(super) fn emit_tree_view_asset_viewport(&mut self, index: usize) -> Result<bool> {
        let Some(asset) = self.tree_view_asset_viewports.get(&index).copied() else {
            return Ok(false);
        };
        let matrix = self.state.transform;
        self.output
            .push_str("<g class=\"treeView-node-icon\" transform=\"")?;
        if matrix.a == 1.0 && matrix.d == 1.0 && matrix.b == 0.0 && matrix.c == 0.0 {
            write!(
                self.output,
                "translate({},{})",
                fmt(matrix.e),
                fmt(matrix.f)
            )?;
        } else {
            write!(self.output, "matrix({})", matrix_attr(matrix))?;
        }
        write!(
            self.output,
            "\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\"",
            fmt(asset.viewport.width),
            fmt(asset.viewport.height),
            fmt(asset.view_box.x),
            fmt(asset.view_box.y),
            fmt(asset.view_box.width),
            fmt(asset.view_box.height)
        )?;
        if asset.viewport.x != 0.0 || asset.viewport.y != 0.0 {
            write!(
                self.output,
                " x=\"{}\" y=\"{}\"",
                fmt(asset.viewport.x),
                fmt(asset.viewport.y)
            )?;
        }
        write_resource_metadata(&mut self.output, self.debug, asset.clip_id.as_str())?;
        self.output.push('>')?;
        self.state.transform = asset.compensation;
        self.groups.push(GroupKind::ViewportClip);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_box_compensation_preserves_all_public_affine_coefficients() {
        // A 20x10 basis in a 14x14 viewport contributes scale .7 and vertical offset 3.5.
        let actual = compensate_view_box(
            Rect::new(0.0, 0.0, 14.0, 14.0),
            Rect::new(0.0, 0.0, 20.0, 10.0),
            Transform {
                a: 0.7,
                b: 1.4,
                c: -0.7,
                d: 2.1,
                e: 4.2,
                f: 6.3,
            },
        )
        .unwrap();
        for (actual, expected) in [actual.a, actual.b, actual.c, actual.d, actual.e, actual.f]
            .into_iter()
            .zip([1.0, 2.0, -1.0, 3.0, 6.0, 4.0])
        {
            assert!((actual - expected).abs() < 1e-12);
        }
        for hint in [
            Rect::new(0.0, 0.0, 0.0, 10.0),
            Rect::new(f64::NAN, 0.0, 20.0, 10.0),
        ] {
            assert!(
                compensate_view_box(Rect::new(0.0, 0.0, 14.0, 14.0), hint, Transform::IDENTITY)
                    .is_none()
            );
        }
    }
}
