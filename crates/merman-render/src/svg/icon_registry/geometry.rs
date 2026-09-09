//! One Iconify alias geometry plan shared by SVG spelling and portable asset consumers.

use super::ingest::ResolvedIcon;
use merman_display_list::Transform;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum IconQuarterTurn {
    Clockwise,
    Half,
    CounterClockwise,
}

impl IconQuarterTurn {
    pub(crate) fn degrees(self) -> i16 {
        match self {
            Self::Clockwise => 90,
            Self::Half => 180,
            Self::CounterClockwise => -90,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum IconTransform {
    Translate {
        x: f64,
        y: f64,
    },
    Scale {
        x: f64,
        y: f64,
    },
    Rotate {
        turn: IconQuarterTurn,
        x: f64,
        y: f64,
    },
}

impl IconTransform {
    /// Exact quarter-turn coefficients avoid introducing trigonometric rounding into aliases.
    pub(crate) fn matrix(self) -> Transform {
        match self {
            Self::Translate { x, y } => Transform {
                e: x,
                f: y,
                ..Transform::IDENTITY
            },
            Self::Scale { x, y } => Transform {
                a: x,
                d: y,
                ..Transform::IDENTITY
            },
            Self::Rotate { turn, x, y } => {
                let (cos, sin) = match turn {
                    IconQuarterTurn::Clockwise => (0.0, 1.0),
                    IconQuarterTurn::Half => (-1.0, 0.0),
                    IconQuarterTurn::CounterClockwise => (0.0, -1.0),
                };
                Transform {
                    a: cos,
                    b: sin,
                    c: -sin,
                    d: cos,
                    e: x - cos * x + sin * y,
                    f: y - sin * x - cos * y,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct IconGeometryPlan {
    pub(crate) left: f64,
    pub(crate) top: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    transforms: [Option<IconTransform>; 3],
}

impl IconGeometryPlan {
    /// SVG transform-list order: concatenate these matrices in iteration order. The plan
    /// maps body coordinates; outer viewport sizing/preserveAspectRatio is a caller concern.
    pub(crate) fn transforms(&self) -> impl Iterator<Item = IconTransform> + '_ {
        self.transforms.iter().copied().flatten()
    }

    pub(super) fn new(icon: &ResolvedIcon) -> crate::Result<Self> {
        let (mut left, mut top, mut width, mut height) =
            (icon.left, icon.top, icon.width, icon.height);
        let mut transforms = [None; 3];
        let mut rotation = i32::from(icon.rotate);
        if icon.h_flip {
            if icon.v_flip {
                rotation += 2;
            } else {
                transforms[1] = Some(IconTransform::Translate {
                    x: width + left,
                    y: -top,
                });
                transforms[2] = Some(IconTransform::Scale { x: -1.0, y: 1.0 });
                left = 0.0;
                top = 0.0;
            }
        } else if icon.v_flip {
            transforms[1] = Some(IconTransform::Translate {
                x: -left,
                y: height + top,
            });
            transforms[2] = Some(IconTransform::Scale { x: 1.0, y: -1.0 });
            left = 0.0;
            top = 0.0;
        }
        rotation = rotation.rem_euclid(4);
        transforms[0] = match rotation {
            1 => {
                let center = height / 2.0 + top;
                Some(IconTransform::Rotate {
                    turn: IconQuarterTurn::Clockwise,
                    x: center,
                    y: center,
                })
            }
            2 => Some(IconTransform::Rotate {
                turn: IconQuarterTurn::Half,
                x: width / 2.0 + left,
                y: height / 2.0 + top,
            }),
            3 => {
                let center = width / 2.0 + left;
                Some(IconTransform::Rotate {
                    turn: IconQuarterTurn::CounterClockwise,
                    x: center,
                    y: center,
                })
            }
            _ => None,
        };
        if rotation % 2 == 1 {
            std::mem::swap(&mut left, &mut top);
            std::mem::swap(&mut width, &mut height);
        }
        if [left, top, width, height]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(crate::Error::invalid_icon_output(
                "icon transformation produced non-finite geometry",
            ));
        }
        Ok(Self {
            left,
            top,
            width,
            height,
            transforms,
        })
    }
}
