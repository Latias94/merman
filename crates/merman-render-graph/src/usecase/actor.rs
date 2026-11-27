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

pub struct ActorPlacement {
    pub glyph_y: f64,
    pub caption_top: f64,
}

#[cfg(feature = "layout-shoal")]
pub struct MeasuredActor {
    pub placement: ActorPlacement,
    pub size: Size,
    /// Left/right attachment height from the physical top of the measured box.
    pub anchor_y: f64,
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
        let mut bounds = glyph_bounds(variant);
        let mut include = |label: &UsecaseLabelPlan, top: f64| {
            let (width, height) = (label.metrics.width, label.metrics.height);
            if width > 0.0 && height > 0.0 {
                bounds = bounds.union(Rect::from_origin_size((-width / 2.0, top), (width, height)));
            }
        };
        let mut main_top = CAPTION_TOP;
        if let Some(stereotype) = &plan.stereotype {
            include(stereotype, main_top);
            main_top += stereotype.metrics.height + 2.0;
        }
        include(&plan.label, main_top);
        let anchor = match variant {
            UsecaseActorType::Normal | UsecaseActorType::Hollow => 5.0,
            UsecaseActorType::Awesome => 13.0,
            UsecaseActorType::Icon => ICON_Y + ICON_HEIGHT / 2.0,
        };
        let center_y = bounds.center().y;
        MeasuredActor {
            placement: Self {
                glyph_y: -center_y,
                caption_top: CAPTION_TOP - center_y,
            },
            size: Size {
                width: bounds.width(),
                height: bounds.height(),
            },
            anchor_y: anchor - bounds.y0,
        }
    }
}
#[cfg(feature = "layout-shoal")]
pub fn body_path(variant: UsecaseActorType) -> Option<&'static BezPath> {
    static PATHS: LazyLock<[BezPath; 3]> = LazyLock::new(|| {
        [NORMAL, HOLLOW_BODY, AWESOME]
            .map(|path| BezPath::from_svg(path).expect("static Usecase actor path"))
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
fn glyph_bounds(variant: UsecaseActorType) -> Rect {
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
