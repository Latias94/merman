pub(crate) const NORMAL: &str = "M 0 -12 C 6.627 -12 12 -17.373 12 -24 C 12 -30.627 6.627 -36 0 -36 C -6.627 -36 -12 -30.627 -12 -24 C -12 -17.373 -6.627 -12 0 -12 Z M 0 -12 V 8 M -17 -5 H 17 M 0 8 L -15 28 M 0 8 L 15 28";
pub(crate) const HOLLOW_BODY: &str =
    "M -22 -10 H 22 V 0 H 6 L 22 17 L 13 28 L 0 13 L -13 28 L -22 17 L -6 0 H -22 Z";
pub(crate) const AWESOME: &str = "M 0 -34 C 7.18 -34 13 -28.18 13 -21 C 13 -13.82 7.18 -8 0 -8 C -7.18 -8 -13 -13.82 -13 -21 C -13 -28.18 -7.18 -34 0 -34 Z M -24 25 C -24 7 -14 -3 0 -3 C 14 -3 24 7 24 25 C 24 28 21 30 18 30 H -18 C -21 30 -24 28 -24 25 Z";
pub(crate) const HOLLOW_HEAD_Y: f64 = -23.0;
pub(crate) const HOLLOW_HEAD_RADIUS: f64 = 9.0;
pub(crate) const ICON_WIDTH: f64 = 52.0;
pub(crate) const ICON_HEIGHT: f64 = 52.0;
pub(crate) const ICON_Y: f64 = -28.0;
pub(crate) const ICON_CORNER: f64 = 4.0;

pub(crate) struct ActorPlacement {
    pub(crate) glyph_y: f64,
    pub(crate) caption_top: f64,
}

impl ActorPlacement {
    pub(crate) fn legacy(height: f64) -> Self {
        let top = -(height - 20.0) / 2.0;
        Self {
            glyph_y: top + 36.0,
            caption_top: top + 80.0,
        }
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn canonical(
        height: f64,
        variant: merman_model::families::usecase::UsecaseActorType,
    ) -> (Self, f64) {
        use merman_model::families::usecase::UsecaseActorType;
        let anchor = match variant {
            UsecaseActorType::Normal | UsecaseActorType::Hollow => 5.0,
            UsecaseActorType::Awesome => 13.0,
            UsecaseActorType::Icon => ICON_Y + ICON_HEIGHT / 2.0,
        };
        let legacy = Self::legacy(height);
        let shift = -anchor - legacy.glyph_y;
        (
            Self {
                glyph_y: -anchor,
                caption_top: legacy.caption_top + shift,
            },
            height + 2.0 * shift.abs(),
        )
    }
}
