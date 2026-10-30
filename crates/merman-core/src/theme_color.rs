//! Theme evaluation uses the shared Khroma-compatible color implementation.

pub use merman_markup::theme_color::{
    ColorAdjustment, ColorChannel, ColorError, ColorSourceFormat, HslaChannels, RgbaChannels,
    ThemeColor, adjust, channel, darken, invert, invert_weighted, is_dark, lighten, rgba,
    transparentize, with_alpha,
};
