use krilla::geom::{Path, PathBuilder};
use krilla::mask::{Mask, MaskType};
use krilla::paint::FillRule;
use krilla::surface::Surface;
use usvg::tiny_skia_path::{PathSegment, Transform};

use crate::util::{convert_fill_rule, UsvgTransformExt};
use crate::{group, ProcessContext};

/// Render a clip path into a surface.
pub(crate) fn render(
    group: &usvg::Group,
    clip_path: &usvg::ClipPath,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) -> u16 {
    // Unfortunately, clip paths are a bit tricky to deal with, the reason being that clip paths in
    // SVGs can be much more complex than in PDF. In SVG, clip paths can have transforms, as well as
    // nested clip paths. The objects inside of the clip path can have transforms as well, making it
    // even more difficult to deal with. Because in PDF, once you start a clip path operation you
    // cannot interrupt it, because once you pop the current graphics state, the clip path will be
    // lost since it is part of the current graphics state. However, if we have various transforms
    // on the children, we need to be able to push/pop the graphics state, so that the children's
    // transforms don't affect each other. Initially, because of this, clip paths were only implemented
    // using soft masks, but Safari has a couple of issues with rendering them properly. Not to mention
    // the fact that soft masks are obviously also more expensive. Because of this, we proceed the following
    // way: We first check whether the clip path itself is too "complex" (complex being that it fulfills
    // some attributes that make it impossible to represent them in our current setup using just native
    // PDF clip paths. If it is too complex, we fall back to using soft masks. However, if it is simple
    // enough, we just use the normal clip path operator in PDF. It should be noted that in reality,
    // only very few SVGs seem to have such complex clipping paths (they are not even rendered correctly
    // by all online converters that were tested), so in most real-life scenarios, the simple version
    // should suffice. But in order to conform with the SVG specification, we also handle the case
    // of more complex clipping paths, even if this means that Safari will in some cases not
    // display them correctly.
    let is_simple_clip_path = is_simple_clip_path(clip_path.root());
    let clip_rules = collect_clip_rules(clip_path.root());

    let mut pop_count = 0;

    if is_simple_clip_path
        && (clip_rules.iter().all(|f| *f == usvg::FillRule::NonZero)
        // For even odd, there must be at most one shape in the group, because
        // overlapping shapes with evenodd render differently in PDF
        || (clip_rules.iter().all(|f| *f == usvg::FillRule::EvenOdd)
        && clip_rules.len() == 1))
    {
        let clips = create_clip_path(
            clip_path,
            clip_rules
                .first()
                .copied()
                .unwrap_or(usvg::FillRule::NonZero),
        );

        for (clip, rule) in clips {
            surface.push_clip_path(&clip, &rule);
            pop_count += 1;
        }
    } else {
        pop_count += render_complex(group, clip_path, surface, process_context);
    }

    pop_count
}

/// Create a simple clip path.
fn create_clip_path(
    clip_path: &usvg::ClipPath,
    clip_rule: usvg::FillRule,
) -> Vec<(Path, FillRule)> {
    let mut clips = vec![];
    if let Some(clip_path) = clip_path.clip_path() {
        clips.extend(create_clip_path(clip_path, clip_rule));
    }

    // Just a dummy operation, so that in case the clip path only has hidden children the clip
    // path will still be applied and everything will be hidden.
    let mut path_builder = PathBuilder::new();
    path_builder.move_to(0.0, 0.0);

    let base_transform = clip_path.transform();
    extend_segments_from_group(clip_path.root(), &base_transform, &mut path_builder);

    clips.push((
        path_builder.finish().unwrap_or_else(|| {
            let mut builder = PathBuilder::new();
            builder.move_to(0.0, 0.0);
            builder.line_to(0.0, 0.0);
            builder.finish().unwrap()
        }),
        convert_fill_rule(&clip_rule),
    ));
    clips
}

/// Collect the paths of a group so that they can be used in the clip path.
fn extend_segments_from_group(
    group: &usvg::Group,
    transform: &Transform,
    path_builder: &mut PathBuilder,
) {
    for child in group.children() {
        match child {
            usvg::Node::Path(ref path) => {
                if path.is_visible() {
                    path.data().segments().for_each(|segment| match segment {
                        PathSegment::MoveTo(mut p) => {
                            transform.map_point(&mut p);
                            path_builder.move_to(p.x, p.y);
                        }
                        PathSegment::LineTo(mut p) => {
                            transform.map_point(&mut p);
                            path_builder.line_to(p.x, p.y)
                        }
                        PathSegment::QuadTo(p1, p2) => {
                            let mut points = [p1, p2];
                            transform.map_points(&mut points);
                            path_builder.quad_to(
                                points[0].x,
                                points[0].y,
                                points[1].x,
                                points[1].y,
                            );
                        }
                        PathSegment::CubicTo(p1, p2, p3) => {
                            let mut points = [p1, p2, p3];
                            transform.map_points(&mut points);
                            path_builder.cubic_to(
                                points[0].x,
                                points[0].y,
                                points[1].x,
                                points[1].y,
                                points[2].x,
                                points[2].y,
                            );
                        }
                        PathSegment::Close => path_builder.close(),
                    })
                }
            }
            usvg::Node::Group(ref group) => {
                let group_transform = transform.pre_concat(group.transform());
                extend_segments_from_group(group, &group_transform, path_builder);
            }
            usvg::Node::Text(ref text) => {
                // We could in theory preserve text in clip paths by using the appropriate
                // rendering mode, but for now we just use the flattened version.
                extend_segments_from_group(text.flattened(), transform, path_builder);
            }
            // Images are not valid in a clip path.
            _ => {}
        }
    }
}

/// Check if the clip path is simple, i.e. it can be translated into a PDF clip path.
fn is_simple_clip_path(group: &usvg::Group) -> bool {
    group.children().iter().all(|n| {
        match n {
            usvg::Node::Group(ref group) => {
                // We can only intersect one clipping path with another one, meaning that we
                // can convert nested clip paths if a second clip path is defined on the clip
                // path itself, but not if it is defined on a child.
                group.clip_path().is_none() && is_simple_clip_path(group)
            }
            _ => true,
        }
    })
}

/// Collect the filling rules used in the clip path.
fn collect_clip_rules(group: &usvg::Group) -> Vec<usvg::FillRule> {
    let mut clip_rules = vec![];
    group.children().iter().for_each(|n| match n {
        usvg::Node::Path(ref path) => {
            if let Some(fill) = &path.fill() {
                clip_rules.push(fill.rule());
            }
        }
        usvg::Node::Text(ref text) => clip_rules.extend(collect_clip_rules(text.flattened())),
        usvg::Node::Group(ref group) => {
            clip_rules.extend(collect_clip_rules(group));
        }
        _ => {}
    });

    clip_rules
}

/// Render a clip path by using alpha masks.
fn render_complex(
    parent: &usvg::Group,
    clip_path: &usvg::ClipPath,
    surface: &mut Surface,
    process_context: &mut ProcessContext,
) -> u16 {
    let mut stream_builder = surface.stream_builder();
    let mut sub_surface = stream_builder.surface();

    let mut pop_count = 0;

    if let Some(clip_path) = clip_path.clip_path() {
        pop_count += render(parent, clip_path, &mut sub_surface, process_context);
    }

    sub_surface.push_transform(&clip_path.transform().to_krilla());
    pop_count += 1;
    group::render(clip_path.root(), &mut sub_surface, process_context);

    for _ in 0..pop_count {
        sub_surface.pop();
    }

    sub_surface.finish();
    let stream = stream_builder.finish();

    surface.push_mask(Mask::new(stream, MaskType::Alpha));
    1
}
