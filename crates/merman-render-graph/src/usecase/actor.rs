#[cfg(feature = "layout-shoal")]
use super::{UsecaseLabelPlan, UsecaseNodePlan};
#[cfg(feature = "layout-shoal")]
use kurbo::{BezPath, Rect, Shape};
#[cfg(feature = "layout-shoal")]
use merman_model::families::usecase::UsecaseActorType;
#[cfg(feature = "layout-shoal")]
use merman_render_core::graph_layout::Size;
#[cfg(feature = "layout-shoal")]
use std::sync::LazyLock;

pub const NORMAL: &str = "M 0 -12 C 6.627 -12 12 -17.373 12 -24 C 12 -30.627 6.627 -36 0 -36 C -6.627 -36 -12 -30.627 -12 -24 C -12 -17.373 -6.627 -12 0 -12 Z M 0 -12 V 8 M -17 -5 H 17 M 0 8 L -15 28 M 0 8 L 15 28";
pub const HOLLOW_BODY: &str =
    "M -22 -10 H 22 V 0 H 6 L 22 17 L 13 28 L 0 13 L -13 28 L -22 17 L -6 0 H -22 Z";
pub const AWESOME: &str = "M 0 -34 C 7.18 -34 13 -28.18 13 -21 C 13 -13.82 7.18 -8 0 -8 C -7.18 -8 -13 -13.82 -13 -21 C -13 -28.18 -7.18 -34 0 -34 Z M -24 25 C -24 7 -14 -3 0 -3 C 14 -3 24 7 24 25 C 24 28 21 30 18 30 H -18 C -21 30 -24 28 -24 25 Z";
pub const HOLLOW_HEAD_Y: f64 = -23.0;
pub const HOLLOW_HEAD_RADIUS: f64 = 9.0;
pub const ICON_WIDTH: f64 = 52.0;
pub const ICON_HEIGHT: f64 = 52.0;
pub const ICON_Y: f64 = -28.0;
pub const ICON_CORNER: f64 = 4.0;

#[cfg(feature = "layout-shoal")]
const STICK_CONTACT_Y: f64 = 5.0;

pub struct ActorPlacement {
    pub glyph_y: f64,
    pub caption_top: f64,
}

#[cfg(feature = "layout-shoal")]
pub struct MeasuredActor {
    pub placement: ActorPlacement,
    /// The glyph and caption layout bounds, including their routing clearance.
    pub layout_size: Size,
    /// Left/right attachment height from the top of the layout box.
    pub anchor_y: f64,
    /// The caption dimensions used for routing. The SVG uses the original label metrics.
    pub caption_contact_size: (f64, f64),
}

impl ActorPlacement {
    pub fn legacy(height: f64) -> Self {
        let top = -(height - 20.0) / 2.0;
        Self {
            glyph_y: top + 36.0,
            caption_top: top + 80.0,
        }
    }

    #[cfg(feature = "layout-shoal")]
    pub fn canonical(plan: &UsecaseNodePlan, variant: UsecaseActorType) -> MeasuredActor {
        const CAPTION_TOP: f64 = 44.0;
        let mut bounds = glyph_contact_bounds(variant);
        let label_bounds = |label: &UsecaseLabelPlan, top| {
            let (width, height) = (label.metrics.width, label.metrics.height);
            Rect::from_origin_size((-width / 2.0, top), (width, height))
        };
        let mut include = |rect: Rect| {
            if rect.width() > 0.0 && rect.height() > 0.0 {
                bounds = bounds.union(rect);
            }
        };
        let mut main_top = CAPTION_TOP;
        if let Some(stereotype) = &plan.stereotype {
            include(label_bounds(stereotype, main_top));
            main_top += stereotype.metrics.height + 2.0;
        }
        let caption = caption_contact_bounds(label_bounds(&plan.label, main_top));
        include(caption);
        let anchor = match variant {
            UsecaseActorType::Normal | UsecaseActorType::Hollow => STICK_CONTACT_Y,
            UsecaseActorType::Awesome => 13.0,
            UsecaseActorType::Icon => ICON_Y + ICON_HEIGHT / 2.0,
        };
        let center_y = bounds.center().y;
        MeasuredActor {
            placement: Self {
                glyph_y: -center_y,
                caption_top: CAPTION_TOP - center_y,
            },
            layout_size: Size {
                width: bounds.width(),
                height: bounds.height(),
            },
            anchor_y: anchor - bounds.y0,
            caption_contact_size: (caption.width(), caption.height()),
        }
    }
}

/// The envelope for lateral contacts, in the same coordinates as the glyph bounds.
#[cfg(feature = "layout-shoal")]
pub fn side_contact_bounds(
    variant: UsecaseActorType,
    glyph_bounds: Rect,
    glyph_y: f64,
) -> Option<Rect> {
    const SIDE_CLEARANCE: f64 = 4.0;
    const CONTACT_HALF_HEIGHT: f64 = 15.0;
    matches!(variant, UsecaseActorType::Normal | UsecaseActorType::Hollow).then(|| {
        let anchor = glyph_y + STICK_CONTACT_Y;
        Rect::new(
            glyph_bounds.x0 - SIDE_CLEARANCE,
            anchor - CONTACT_HALF_HEIGHT,
            glyph_bounds.x1 + SIDE_CLEARANCE,
            anchor + CONTACT_HALF_HEIGHT,
        )
    })
}

/// Reserve space for a bottom contact, using the painted main caption as the starting box.
#[cfg(feature = "layout-shoal")]
pub fn caption_contact_bounds(caption: Rect) -> Rect {
    const CAPTION_CLEARANCE: f64 = 5.0;
    if caption.width() > 0.0 && caption.height() > 0.0 {
        Rect::new(
            caption.x0,
            caption.y0,
            caption.x1,
            caption.y1 + CAPTION_CLEARANCE,
        )
    } else {
        caption
    }
}

/// Routing outline in glyph coordinates. The sparse stick figures also provide an envelope
/// for lateral contacts, leaving the head and caption available for vertical routes.
#[cfg(feature = "layout-shoal")]
pub fn body_path(variant: UsecaseActorType) -> Option<&'static BezPath> {
    static PATHS: LazyLock<[BezPath; 3]> = LazyLock::new(|| {
        [
            (UsecaseActorType::Normal, NORMAL),
            (UsecaseActorType::Hollow, HOLLOW_BODY),
            (UsecaseActorType::Awesome, AWESOME),
        ]
        .map(|(variant, source)| {
            let mut path = BezPath::from_svg(source).expect("static Usecase actor path");
            if let Some(rect) = side_contact_bounds(variant, path.bounding_box(), 0.0) {
                path.extend(rect.path_elements(0.1));
            }
            path
        })
    });
    let index = match variant {
        UsecaseActorType::Normal => 0,
        UsecaseActorType::Hollow => 1,
        UsecaseActorType::Awesome => 2,
        UsecaseActorType::Icon => return None,
    };
    Some(&PATHS[index])
}

#[cfg(feature = "layout-shoal")]
fn glyph_contact_bounds(variant: UsecaseActorType) -> Rect {
    let Some(path) = body_path(variant) else {
        return Rect::from_origin_size((-ICON_WIDTH / 2.0, ICON_Y), (ICON_WIDTH, ICON_HEIGHT));
    };
    let bounds = path.bounding_box();
    if variant == UsecaseActorType::Hollow {
        bounds.union(Rect::from_center_size(
            (0.0, HOLLOW_HEAD_Y),
            (2.0 * HOLLOW_HEAD_RADIUS, 2.0 * HOLLOW_HEAD_RADIUS),
        ))
    } else {
        bounds
    }
}
