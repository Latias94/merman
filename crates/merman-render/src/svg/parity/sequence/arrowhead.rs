use super::super::{Result, SvgDiagramId, SvgOutput, escape_attr_display};
#[cfg(feature = "native-compact")]
use merman_render_core::paint_scene::ScenePathCommand;
use merman_theme_contract::v2::MarkerStyle;

pub(super) struct SequenceArrowhead {
    path: &'static str,
    pub(super) has_fill: bool,
    #[cfg(feature = "native-compact")]
    pub(super) commands: &'static [ScenePathCommand],
}

macro_rules! arrowhead {
    ([$start_x:literal, $start_y:literal] $(, [$x:literal, $y:literal])+; $end:literal $(=> $close:ident)?) => {
        SequenceArrowhead {
            path: concat!("M ", $start_x, " ", $start_y, $(" L ", $x, " ", $y,)+ $end),
            has_fill: !$end.is_empty(),
            #[cfg(feature = "native-compact")]
            commands: &[
                ScenePathCommand::MoveTo([$start_x as f64, $start_y as f64]),
                $(ScenePathCommand::LineTo([$x as f64, $y as f64]),)+
                $(ScenePathCommand::$close,)?
            ],
        }
    };
}

pub(super) const fn sequence_arrowhead(style: MarkerStyle) -> SequenceArrowhead {
    match style {
        MarkerStyle::Classic => arrowhead!([-1, 0], [10, 5], [0, 10]; " z" => Close),
        MarkerStyle::Stealth => arrowhead!([-1, 0], [10, 5], [-1, 10], [2, 5]; " Z" => Close),
        MarkerStyle::Filled => arrowhead!([0, 0], [10, 5], [0, 10]; " Z" => Close),
        MarkerStyle::Open => arrowhead!([0, 0], [10, 5], [0, 10]; ""),
    }
}

macro_rules! arrowhead_viewport {
    ($ref_x:literal, $ref_y:literal, $width:literal, $height:literal) => {
        pub(super) const ARROWHEAD_REF_X: f64 = $ref_x;
        #[cfg(feature = "native-compact")]
        pub(super) const ARROWHEAD_REFERENCE: [f64; 2] = [ARROWHEAD_REF_X, $ref_y as f64];
        #[cfg(feature = "native-compact")]
        pub(super) const ARROWHEAD_SIZE: [f64; 2] = [$width as f64, $height as f64];

        pub(super) fn write_arrowhead_def(
            out: &mut impl SvgOutput,
            id: SvgDiagramId<'_>,
            style: MarkerStyle,
        ) -> Result<()> {
            let arrowhead = sequence_arrowhead(style);
            let attributes = if arrowhead.has_fill {
                ""
            } else {
                r#" class="merman-sequence-open-arrow""#
            };
            let _ = write!(
                out,
                concat!(
                    r#"<defs><marker id="{}-arrowhead" refX=""#,
                    $ref_x,
                    r#"" refY=""#,
                    $ref_y,
                    r#"" markerUnits="userSpaceOnUse" markerWidth=""#,
                    $width,
                    r#"" markerHeight=""#,
                    $height,
                    r#"" orient="auto-start-reverse"><path d="{}"{}/></marker></defs>"#,
                ),
                escape_attr_display(id),
                arrowhead.path,
                attributes
            );
            out.checkpoint()
        }
    };
}

arrowhead_viewport!(7.9, 5, 12, 12);
