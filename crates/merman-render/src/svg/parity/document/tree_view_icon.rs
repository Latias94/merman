//! Source icon spelling is available only after exact comparison with public geometry.

use super::*;
use crate::tree_view::{TreeViewBuiltinIcon, tree_view_builtin_icon};

struct BuiltinPath {
    source: TreeViewBuiltinIcon,
    segments: Vec<PathSegment>,
}

pub(super) struct IconProjection {
    builtins: [BuiltinPath; 2],
}

impl IconProjection {
    pub(super) fn new(session: &RenderSession) -> Result<Self> {
        let parse = |name: &str| -> Result<BuiltinPath> {
            session.checkpoint(OperationPhase::Emit)?;
            let source = tree_view_builtin_icon(name)
                .ok_or_else(|| invalid("TreeView builtin icon source is missing"))?;
            Ok(BuiltinPath {
                source,
                segments: crate::drawing_list::parse_svg_path(source.path_data)?,
            })
        };
        let builtins = [
            parse("mermaid-treeview:folder")?,
            parse("mermaid-treeview:file")?,
        ];
        session.checkpoint(OperationPhase::Emit)?;
        Ok(Self { builtins })
    }

    fn source(&self, segments: &[PathSegment]) -> Option<TreeViewBuiltinIcon> {
        self.builtins
            .iter()
            .find(|builtin| builtin.segments == segments)
            .map(|builtin| builtin.source)
    }
}

/// The nested viewport represents this exact positive uniform scale, without discarding a
/// small but real skew. Its finite extent is checked before any structural output is emitted.
fn viewport_extent(transform: Transform) -> Option<f64> {
    let extent = transform.a * 24.0;
    (transform.b == 0.0
        && transform.c == 0.0
        && transform.a == transform.d
        && transform.a > 0.0
        && extent.is_finite())
    .then_some(extent)
}

impl DocumentSvgEncoder<'_> {
    pub(super) fn emit_tree_view_icon(
        &mut self,
        path_id: &ResourceId,
        path: &PathResource,
        style: &PathStyle,
    ) -> Result<()> {
        self.session.checkpoint(OperationPhase::Emit)?;
        let source = self
            .tree_view_icons
            .as_ref()
            .and_then(|icons| icons.source(&path.segments));
        let (Some(source), Some(extent), Some(paint @ Paint::Solid { color, .. }), None) = (
            source,
            viewport_extent(self.state.transform),
            style.fill.as_ref(),
            style.stroke.as_ref(),
        ) else {
            // The source nested SVG clips to 24x24. Only its exact unstroked builtin fill
            // is known to fit: edited geometry/strokes must retain the unclipped public path.
            // Resource paints also keep their ordinary user-space coordinate context.
            return self.emit_path_as_standard(path_id, path, style);
        };
        let transform = self.state.transform;
        write!(
            self.output,
            "<g class=\"treeView-node-icon\" transform=\"translate({},{})\"",
            fmt(transform.e),
            fmt(transform.f),
        )?;
        if self.state.opacity != 1.0 {
            write!(self.output, " opacity=\"{}\"", fmt(self.state.opacity))?;
        }
        write_blend_style(&mut self.output, self.state.blend_mode)?;
        write!(
            self.output,
            "><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 24 24\"><path fill=\"currentColor\"",
            fmt(extent),
            fmt(extent),
        )?;
        if style.fill_rule == FillRule::EvenOdd {
            self.output.push_str(" fill-rule=\"evenodd\"")?;
        }
        write!(self.output, " d=\"{}\"", escaped_attr(source.path_data))?;
        if source.even_odd && style.fill_rule == FillRule::EvenOdd {
            self.output.push_str(" clip-rule=\"evenodd\"")?;
        }
        // Color and alpha always come from the current command, not the icon theme/source.
        // Keep shape alpha separate from group opacity to avoid double attenuation.
        write!(
            self.output,
            " style=\"color:{};fill:currentColor;fill-opacity:{};fill-rule:{};stroke:none;\"",
            color_css(*color),
            fmt(paint_opacity(paint)),
            fill_rule_name(style.fill_rule),
        )?;
        self.write_path_metadata(path_id.as_str())?;
        self.output.push_str("/></svg></g>")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::RenderEnvironment;

    #[test]
    fn source_spelling_requires_exact_public_geometry() {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let icons = IconProjection::new(&session).unwrap();
        for name in ["mermaid-treeview:folder", "mermaid-treeview:file"] {
            let source = tree_view_builtin_icon(name).unwrap();
            let mut segments = crate::drawing_list::parse_svg_path(source.path_data).unwrap();
            assert_eq!(icons.source(&segments), Some(source));
            let PathSegment::MoveTo { to } = &mut segments[0] else {
                panic!("builtin starts with MoveTo");
            };
            to.x = 30.0;
            assert_eq!(icons.source(&segments), None);
        }
    }

    #[test]
    fn viewport_projection_preserves_every_transform_coefficient() {
        let transform = Transform {
            a: 0.75,
            d: 0.75,
            e: 12.0,
            f: 20.0,
            ..Transform::IDENTITY
        };
        assert_eq!(viewport_extent(transform), Some(18.0));
        for edited in [
            Transform {
                b: 1e-12,
                ..transform
            },
            Transform {
                c: 1e-12,
                ..transform
            },
            Transform {
                d: 0.75 + 1e-12,
                ..transform
            },
            Transform {
                a: -1.0,
                d: -1.0,
                ..transform
            },
            Transform {
                a: f64::MAX,
                d: f64::MAX,
                ..transform
            },
        ] {
            assert_eq!(viewport_extent(edited), None);
        }
    }
}
