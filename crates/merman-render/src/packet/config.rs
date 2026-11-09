use crate::config::{config_bool, config_css_number_or_string, config_f64};
use merman_model::families::packet::{PacketBitsPerRowError, validate_packet_bits_per_row};
use serde_json::Value;
use std::borrow::Cow;

const DEFAULT_SHOW_BITS: bool = true;
const DEFAULT_ROW_HEIGHT: f64 = 32.0;
const DEFAULT_PADDING_X: f64 = 5.0;
const DEFAULT_PADDING_Y: f64 = 5.0;
const SHOW_BITS_PADDING_Y_EXTRA: f64 = 10.0;
const DEFAULT_BIT_WIDTH: f64 = 32.0;

const DEFAULT_BYTE_FONT_SIZE: &str = "10px";
const DEFAULT_START_BYTE_COLOR: &str = "black";
const DEFAULT_END_BYTE_COLOR: &str = "black";
const DEFAULT_LABEL_COLOR: &str = "black";
const DEFAULT_LABEL_FONT_SIZE: &str = "12px";
const DEFAULT_TITLE_COLOR: &str = "black";
const DEFAULT_TITLE_FONT_SIZE: &str = "14px";
const DEFAULT_BLOCK_STROKE_COLOR: &str = "black";
const DEFAULT_BLOCK_STROKE_WIDTH: &str = "1";
const DEFAULT_BLOCK_FILL_COLOR: &str = "#efefef";

pub(crate) struct PacketConfigView<'a> {
    packet_config: &'a Value,
    packet_theme: &'a Value,
}

impl<'a> PacketConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            packet_config: effective_config.get("packet").unwrap_or(&Value::Null),
            packet_theme: effective_config
                .get("themeVariables")
                .and_then(|variables| variables.get("packet"))
                .unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn layout_settings(
        &self,
    ) -> std::result::Result<PacketLayoutSettings, PacketBitsPerRowError> {
        let show_bits = self.packet_bool("showBits").unwrap_or(DEFAULT_SHOW_BITS);
        let padding_y = self
            .packet_f64("paddingY")
            .unwrap_or(DEFAULT_PADDING_Y)
            .max(0.0)
            + if show_bits {
                SHOW_BITS_PADDING_Y_EXTRA
            } else {
                0.0
            };

        Ok(PacketLayoutSettings {
            show_bits,
            bit_order: self.bit_order(),
            row_height: self
                .packet_f64("rowHeight")
                .unwrap_or(DEFAULT_ROW_HEIGHT)
                .max(1.0),
            padding_x: self
                .packet_f64("paddingX")
                .unwrap_or(DEFAULT_PADDING_X)
                .max(0.0),
            padding_y,
            bit_width: self
                .packet_f64("bitWidth")
                .unwrap_or(DEFAULT_BIT_WIDTH)
                .max(1.0),
            bits_per_row: validate_packet_bits_per_row(
                self.packet_config.get("bitsPerRow").and_then(Value::as_f64),
            )?,
        })
    }

    pub(crate) fn bit_order(&self) -> PacketBitOrder {
        match self.packet_config.get("bitOrder").and_then(Value::as_str) {
            Some("descending") => PacketBitOrder::Descending,
            _ => PacketBitOrder::Ascending,
        }
    }

    pub(crate) fn use_max_width(&self) -> bool {
        self.packet_bool("useMaxWidth").unwrap_or(true)
    }

    pub(crate) fn style_settings(&self) -> PacketStyleSettings {
        PacketStyleSettings {
            byte_font_size: self.packet_style("byteFontSize", DEFAULT_BYTE_FONT_SIZE),
            start_byte_color: self.packet_style("startByteColor", DEFAULT_START_BYTE_COLOR),
            end_byte_color: self.packet_style("endByteColor", DEFAULT_END_BYTE_COLOR),
            label_color: self.packet_style("labelColor", DEFAULT_LABEL_COLOR),
            label_font_size: self.packet_style("labelFontSize", DEFAULT_LABEL_FONT_SIZE),
            title_color: self.packet_style("titleColor", DEFAULT_TITLE_COLOR),
            title_font_size: self.packet_style("titleFontSize", DEFAULT_TITLE_FONT_SIZE),
            block_stroke_color: self.packet_style("blockStrokeColor", DEFAULT_BLOCK_STROKE_COLOR),
            block_stroke_width: self.packet_style("blockStrokeWidth", DEFAULT_BLOCK_STROKE_WIDTH),
            block_fill_color: self.packet_style("blockFillColor", DEFAULT_BLOCK_FILL_COLOR),
        }
    }

    fn packet_bool(&self, key: &str) -> Option<bool> {
        config_bool(self.packet_config, &[key])
    }

    fn packet_f64(&self, key: &str) -> Option<f64> {
        config_f64(self.packet_config, &[key])
    }

    fn packet_style(&self, key: &str, default_value: &'static str) -> Cow<'static, str> {
        self.style_value(key)
            .map(Cow::Owned)
            .unwrap_or(Cow::Borrowed(default_value))
    }

    pub(super) fn style_value(&self, key: &str) -> Option<String> {
        let mut value = config_css_number_or_string(self.packet_theme, &[key])?;
        // Preserve the declaration parser's second delimiter normalization after JSON projection.
        value.truncate(value.trim_end_matches(';').trim_end().len());
        merman_markup::style::is_safe_css_declaration_value(&value).then_some(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacketBitOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PacketLayoutSettings {
    pub(crate) show_bits: bool,
    pub(crate) bit_order: PacketBitOrder,
    pub(crate) row_height: f64,
    pub(crate) padding_x: f64,
    pub(crate) padding_y: f64,
    pub(crate) bit_width: f64,
    pub(crate) bits_per_row: f64,
}

#[derive(Debug)]
pub(crate) struct PacketStyleSettings {
    pub(crate) byte_font_size: Cow<'static, str>,
    pub(crate) start_byte_color: Cow<'static, str>,
    pub(crate) end_byte_color: Cow<'static, str>,
    pub(crate) label_color: Cow<'static, str>,
    pub(crate) label_font_size: Cow<'static, str>,
    pub(crate) title_color: Cow<'static, str>,
    pub(crate) title_font_size: Cow<'static, str>,
    pub(crate) block_stroke_color: Cow<'static, str>,
    pub(crate) block_stroke_width: Cow<'static, str>,
    pub(crate) block_fill_color: Cow<'static, str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::packet::DEFAULT_PACKET_BITS_PER_ROW;
    use serde_json::json;

    #[test]
    fn packet_layout_settings_preserve_defaults_and_show_bits_padding() {
        let cfg = json!({});
        let settings = PacketConfigView::new(&cfg).layout_settings().unwrap();

        assert!(settings.show_bits);
        assert_eq!(settings.bit_order, PacketBitOrder::Ascending);
        assert_eq!(settings.row_height, DEFAULT_ROW_HEIGHT);
        assert_eq!(settings.padding_x, DEFAULT_PADDING_X);
        assert_eq!(
            settings.padding_y,
            DEFAULT_PADDING_Y + SHOW_BITS_PADDING_Y_EXTRA
        );
        assert_eq!(settings.bit_width, DEFAULT_BIT_WIDTH);
        assert_eq!(settings.bits_per_row, DEFAULT_PACKET_BITS_PER_ROW);
    }

    #[test]
    fn packet_layout_settings_project_configured_values() {
        let cfg = json!({
            "packet": {
                "showBits": false,
                "bitOrder": "descending",
                "rowHeight": "40",
                "paddingX": 9,
                "paddingY": 11,
                "bitWidth": 24,
                "bitsPerRow": 16
            }
        });
        let settings = PacketConfigView::new(&cfg).layout_settings().unwrap();

        assert!(!settings.show_bits);
        assert_eq!(settings.bit_order, PacketBitOrder::Descending);
        assert_eq!(settings.row_height, 40.0);
        assert_eq!(settings.padding_x, 9.0);
        assert_eq!(settings.padding_y, 11.0);
        assert_eq!(settings.bit_width, 24.0);
        assert_eq!(settings.bits_per_row, 16.0);
    }

    #[test]
    fn packet_layout_settings_clamp_geometry_and_keep_row_width_numeric() {
        let cfg = json!({
            "packet": {
                "rowHeight": -40,
                "paddingX": -9,
                "paddingY": -11,
                "bitWidth": 0,
                "bitsPerRow": "16"
            }
        });
        let settings = PacketConfigView::new(&cfg).layout_settings().unwrap();

        assert_eq!(settings.row_height, 1.0);
        assert_eq!(settings.padding_x, 0.0);
        assert_eq!(settings.padding_y, SHOW_BITS_PADDING_Y_EXTRA);
        assert_eq!(settings.bit_width, 1.0);
        assert_eq!(settings.bits_per_row, DEFAULT_PACKET_BITS_PER_ROW);
    }

    #[test]
    fn packet_layout_settings_reject_nonpositive_bits_per_row() {
        let cfg = json!({ "packet": { "bitsPerRow": 0 } });

        let error = PacketConfigView::new(&cfg).layout_settings().unwrap_err();

        assert_eq!(error.value, 0.0);
    }

    #[test]
    fn packet_style_settings_project_css_values() {
        let cfg = json!({
            "themeVariables": { "packet": {
                "byteFontSize": "11px",
                "startByteColor": "#111111",
                "endByteColor": "#222222",
                "labelColor": "#333333",
                "labelFontSize": "13px",
                "titleColor": "#444444",
                "titleFontSize": "15px",
                "blockStrokeColor": "#555555",
                "blockStrokeWidth": 2,
                "blockFillColor": "#666666"
            } }
        });
        let settings = PacketConfigView::new(&cfg).style_settings();

        assert_eq!(settings.byte_font_size, "11px");
        assert_eq!(settings.start_byte_color, "#111111");
        assert_eq!(settings.end_byte_color, "#222222");
        assert_eq!(settings.label_color, "#333333");
        assert_eq!(settings.label_font_size, "13px");
        assert_eq!(settings.title_color, "#444444");
        assert_eq!(settings.title_font_size, "15px");
        assert_eq!(settings.block_stroke_color, "#555555");
        assert_eq!(settings.block_stroke_width, "2");
        assert_eq!(settings.block_fill_color, "#666666");
    }

    #[test]
    fn packet_layout_and_style_settings_keep_separate_config_owners() {
        let cfg = json!({
            "packet": {
                "showBits": false,
                "bitOrder": "descending",
                "rowHeight": 40,
                "labelColor": "#123456",
                "blockFillColor": "#654321"
            },
            "themeVariables": { "packet": {
                "showBits": true,
                "bitOrder": "ascending",
                "rowHeight": 80,
                "titleColor": "#abcdef"
            } }
        });
        let view = PacketConfigView::new(&cfg);
        let layout = view.layout_settings().unwrap();
        let style = view.style_settings();

        assert!(!layout.show_bits);
        assert_eq!(layout.bit_order, PacketBitOrder::Descending);
        assert_eq!(layout.row_height, 40.0);
        assert_eq!(style.label_color, DEFAULT_LABEL_COLOR);
        assert_eq!(style.block_fill_color, DEFAULT_BLOCK_FILL_COLOR);
        assert_eq!(style.title_color, "#abcdef");
    }

    #[test]
    fn packet_style_values_preserve_normalization_and_reject_declaration_injection() {
        for (value, expected) in [
            (json!(" red; ; "), "red"),
            (json!("red !important"), "red !important"),
            (json!("a:b"), "a:b"),
            (json!(-0.0), "0"),
            (json!("red;font-family:serif"), "black"),
            (json!("uRl(javascript:alert(1))"), "black"),
            (json!("ExPrEsSiOn(1)"), "black"),
            (json!("red\u{1}"), "black"),
            (json!(""), "black"),
            (Value::Null, "black"),
            (json!(true), "black"),
            (json!(["red"]), "black"),
            (json!({"color": "red"}), "black"),
        ] {
            let config = json!({"themeVariables": {"packet": {"labelColor": value}}});
            let settings = PacketConfigView::new(&config).style_settings();
            assert_eq!(settings.label_color, expected, "value={value}");
        }
    }
}
