use merman_theme_contract::v2::MarkerStyle;

#[cfg(feature = "paint-scene")]
use crate::paint_scene::ScenePathCommand;

struct ArrowShape {
    path: &'static str,
    #[cfg(feature = "paint-scene")]
    commands: &'static [ScenePathCommand],
}

macro_rules! arrow_shape {
    ([$start_x:literal, $start_y:literal] $(, [$x:literal, $y:literal])+ $(; $end:literal => $close:ident)?) => {
        ArrowShape {
            path: concat!("M", $start_x, " ", $start_y, $("L", $x, " ", $y,)+ $($end,)?),
            #[cfg(feature = "paint-scene")]
            commands: &[
                ScenePathCommand::MoveTo([$start_x as f64, $start_y as f64]),
                $(ScenePathCommand::LineTo([$x as f64, $y as f64]),)+
                $(ScenePathCommand::$close,)?
            ],
        }
    };
}

fn arrow_shape(style: MarkerStyle) -> Option<ArrowShape> {
    match style {
        MarkerStyle::Stealth => Some(arrow_shape!([0, 1], [10, 5], [0, 9], [2.5, 5]; "Z" => Close)),
        MarkerStyle::Filled => Some(arrow_shape!([0, 0.75], [10, 5], [0, 9.25]; "Z" => Close)),
        MarkerStyle::Open => Some(arrow_shape!([1, 1], [10, 5], [1, 9])),
        MarkerStyle::Classic => None,
    }
}

/// An end-facing arrowhead in a 10 by 10 view box. A start marker mirrors it horizontally.
pub fn arrow_path(style: MarkerStyle) -> Option<&'static str> {
    arrow_shape(style).map(|shape| shape.path)
}

/// Typed commands for the same end-facing arrowhead.
#[cfg(feature = "paint-scene")]
pub fn arrow_commands(style: MarkerStyle) -> Option<&'static [ScenePathCommand]> {
    arrow_shape(style).map(|shape| shape.commands)
}
