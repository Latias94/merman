//! Registered icon viewport structure, with public clipping and matrix compensation.

use super::*;

/// Match scopes once, rather than rescanning a subtree for each source group.
pub(super) fn group_projections<'a>(
    document: &DrawingListDocument,
    body: &'a crate::drawing_list::TreeViewSvgBody,
    session: &RenderSession,
) -> Result<BTreeMap<usize, &'a crate::drawing_list::AssetGroup>> {
    let mut result = BTreeMap::new();
    if body.assets.groups.is_empty() {
        return Ok(result);
    }
    let mut saves = Vec::new();
    for (index, command) in document.commands.iter().enumerate() {
        session.checkpoint(OperationPhase::Emit)?;
        match command {
            DrawingCommand::Save => {
                saves
                    .try_reserve(1)
                    .map_err(|_| crate::Error::DrawingListAllocationFailed {
                        collection: "icon SVG scopes",
                    })?;
                saves.push(index);
            }
            DrawingCommand::Restore => {
                if let Some(start) = saves.pop()
                    && let Some(group) = body.assets.groups.get(&start)
                    && group.end == index
                {
                    result.insert(start, group);
                }
            }
            _ => {}
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
    pub(super) fn emit_tree_view_asset_group(&mut self, index: usize) -> Result<Option<usize>> {
        let Some(&group) = self.tree_view_asset_groups.get(&index) else {
            return Ok(None);
        };
        let commands = &self.document.commands[index + 1..group.end];
        let prefix = group
            .transform
            .as_deref()
            .map(|source| {
                crate::drawing_list::matching_transform_prefix(source, commands, self.session)
            })
            .transpose()?
            .flatten();
        let count = prefix.unwrap_or(0);
        let parent = self.state.transform;
        self.emit_command(&DrawingCommand::Save)?;
        for command in &commands[..count] {
            self.emit_command(command)?;
        }
        self.output.push_str("<g")?;
        if let Some(id) = &group.dom_id {
            self.output.push_str(" id=\"")?;
            output::escape_attr(&mut self.output, id)?;
            self.output.push('"')?;
        }
        if parent == Transform::IDENTITY && prefix.is_some() {
            self.output.push_str(" transform=\"")?;
            output::escape_attr(
                &mut self.output,
                group.transform.as_deref().unwrap_or_default(),
            )?;
            self.output.push('"')?;
        } else if self.state.transform != Transform::IDENTITY {
            write!(
                self.output,
                " transform=\"matrix({})\"",
                matrix_attr(self.state.transform)
            )?;
        }
        self.output.push('>')?;
        // Only the current public matrix moves onto the group. Paint/opacity stay on leaves.
        self.state.transform = Transform::IDENTITY;
        self.groups.push(GroupKind::Asset);
        Ok(Some(count + 1))
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
        if !geometry.matches_path(&path.segments, self.session)? {
            return Ok(false);
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
        self.write_path_style(style)?;
        self.write_state_attrs()?;
        self.write_sidecar_dom_id(path_id.as_str())?;
        self.write_path_metadata(path_id.as_str())?;
        self.output.push_str("/>")?;
        Ok(true)
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
