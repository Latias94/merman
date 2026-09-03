//! Shared resolution helpers for renderer-neutral family adapters.
//!
//! This module begins after family-specific cascade/theme resolution. It deliberately accepts one
//! final CSS token at a time; selector matching, inheritance, variables, and arbitrary CSS remain
//! outside the DrawingList seam.

use crate::environment::{RenderSession, TextMeasurementPhase, TextMeasurementSource};
use crate::{Error, Result};
use merman_core::theme_color::{ColorChannel, ThemeColor};
use merman_display_list::{
    Color, LineCap, LineJoin, MeasurementProvenance, Paint, StrokeStyle, TextObligation,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct PortableStyleResolver {
    family: &'static str,
}

impl PortableStyleResolver {
    pub(super) const fn new(family: &'static str) -> Self {
        Self { family }
    }

    pub(super) fn optional_color(&self, property: &str, value: &str) -> Result<Option<Color>> {
        let value = css_token(value);
        if value.eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        if value.eq_ignore_ascii_case("transparent") {
            return Ok(Some(Color::rgba(0, 0, 0, 0)));
        }
        let parsed = ThemeColor::parse(value).map_err(|error| {
            self.unavailable(format!(
                "theme property `{property}` uses non-portable color `{value}`: {error}"
            ))
        })?;
        let channel = |kind| parsed.channel(kind).round().clamp(0.0, 255.0) as u8;
        Ok(Some(Color::rgba(
            channel(ColorChannel::Red),
            channel(ColorChannel::Green),
            channel(ColorChannel::Blue),
            (parsed.channel(ColorChannel::Alpha) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8,
        )))
    }

    pub(super) fn color(&self, property: &str, value: &str) -> Result<Color> {
        self.optional_color(property, value)?.ok_or_else(|| {
            self.unavailable(format!(
                "theme property `{property}` cannot be `none` for this DrawingList primitive"
            ))
        })
    }

    pub(super) fn length(&self, property: &str, value: &str) -> Result<f64> {
        let value = css_token(value);
        let (numeric, scale) = if let Some(numeric) = value.strip_suffix("px") {
            (numeric.trim(), 1.0)
        } else if let Some(numeric) = value.strip_suffix("pt") {
            (numeric.trim(), 4.0 / 3.0)
        } else {
            (value, 1.0)
        };
        let parsed = numeric.parse::<f64>().map_err(|_| {
            self.unavailable(format!(
                "theme property `{property}` uses non-portable length `{value}`"
            ))
        })? * scale;
        if !parsed.is_finite() || parsed < 0.0 {
            return Err(self.unavailable(format!(
                "theme property `{property}` is outside the portable length range"
            )));
        }
        Ok(parsed)
    }

    pub(super) fn positive_length(&self, property: &str, value: &str) -> Result<f64> {
        let parsed = self.length(property, value)?;
        if parsed <= 0.0 {
            return Err(self.unavailable(format!(
                "theme property `{property}` must be greater than zero"
            )));
        }
        Ok(parsed)
    }

    pub(super) fn opacity(&self, property: &str, value: &str) -> Result<f64> {
        let value = css_token(value);
        let parsed = if let Some(percent) = value.strip_suffix('%') {
            percent
                .trim()
                .parse::<f64>()
                .ok()
                .map(|value| value / 100.0)
        } else {
            value.parse::<f64>().ok()
        }
        .ok_or_else(|| {
            self.unavailable(format!(
                "theme property `{property}` uses non-portable opacity `{value}`"
            ))
        })?;
        if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
            return Err(self.unavailable(format!("theme property `{property}` is outside 0..=1")));
        }
        Ok(parsed)
    }

    fn unavailable(&self, reason: impl Into<String>) -> Error {
        Error::DrawingListUnavailable {
            family: self.family.to_string(),
            reason: reason.into(),
        }
    }
}

pub(super) fn stroke(color: Color, width: f64) -> StrokeStyle {
    StrokeStyle {
        paint: Paint::solid(color),
        width,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        line_cap: LineCap::Butt,
        line_join: LineJoin::Miter,
        miter_limit: 4.0,
    }
}

pub(super) fn svg_plain_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if crate::text::is_html_collapsible_ascii_whitespace(character) {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }
    output
}

pub(super) fn text_obligation(
    session: &RenderSession,
    phase: TextMeasurementPhase,
) -> TextObligation {
    let route = session.text_measurement_route(phase);
    let profile = profile_identity(&route.primary);
    let measurement = match route.primary_source {
        TextMeasurementSource::Host => MeasurementProvenance::HostCallback { profile },
        TextMeasurementSource::Profile => MeasurementProvenance::DeterministicFallback { profile },
    };
    TextObligation::HostText { measurement }
}

fn css_token(value: &str) -> &str {
    let value = value.trim().trim_end_matches(';').trim();
    value
        .strip_suffix("!important")
        .map(str::trim)
        .unwrap_or(value)
}

fn profile_identity(identity: &crate::environment::TextMeasurementProfileIdentity) -> String {
    let mut value = format!("{}@{}", identity.profile().as_str(), identity.version());
    for decorator in identity.decorators() {
        value.push('+');
        value.push_str(decorator);
    }
    value
}
