use serde_json::Value;

pub(crate) fn value_at<'a>(cfg: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

pub(crate) fn json_f64(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|n| n as f64))
        .or_else(|| value.as_u64().map(|n| n as f64))
        .or_else(|| {
            let n = value.as_str()?.trim().parse::<f64>().ok()?;
            n.is_finite().then_some(n)
        })
}

pub(crate) fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    value_at(cfg, path).and_then(json_f64)
}

pub(crate) fn config_f64_or(cfg: &Value, path: &[&str], default: f64) -> f64 {
    config_f64(cfg, path).unwrap_or(default)
}

pub(crate) fn json_f64_css_px(value: &Value) -> Option<f64> {
    json_f64(value).or_else(|| value.as_str().and_then(parse_css_px_to_f64))
}

pub(crate) fn config_f64_css_px(cfg: &Value, path: &[&str]) -> Option<f64> {
    value_at(cfg, path).and_then(json_f64_css_px)
}

fn parse_css_px_to_f64(text: &str) -> Option<f64> {
    let text = text.trim().trim_end_matches(';').trim();
    let text = text.trim_end_matches("!important").trim();
    let text = text.strip_suffix("px").unwrap_or(text).trim();
    let value = text.parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_f64_accepts_json_numbers_and_plain_numeric_strings() {
        assert_eq!(json_f64(&json!(100)), Some(100.0));
        assert_eq!(json_f64(&json!(70.5)), Some(70.5));
        assert_eq!(json_f64(&json!("100")), Some(100.0));
        assert_eq!(json_f64(&json!(" 70.5 ")), Some(70.5));
    }

    #[test]
    fn json_f64_rejects_css_or_nonfinite_strings() {
        assert_eq!(json_f64(&json!("100px")), None);
        assert_eq!(json_f64(&json!("NaN")), None);
        assert_eq!(json_f64(&json!("inf")), None);
    }

    #[test]
    fn config_f64_walks_paths_and_accepts_yaml_string_numbers() {
        let cfg = json!({
            "flowchart": {
                "rankSpacing": "100",
                "nodeSpacing": "70.5"
            }
        });

        assert_eq!(config_f64(&cfg, &["flowchart", "rankSpacing"]), Some(100.0));
        assert_eq!(config_f64(&cfg, &["flowchart", "nodeSpacing"]), Some(70.5));
        assert_eq!(config_f64(&cfg, &["missing", "rankSpacing"]), None);
    }

    #[test]
    fn json_f64_css_px_accepts_plain_and_css_numeric_strings() {
        assert_eq!(json_f64_css_px(&json!("24")), Some(24.0));
        assert_eq!(json_f64_css_px(&json!("24px")), Some(24.0));
        assert_eq!(json_f64_css_px(&json!("24px !important;")), Some(24.0));
        assert_eq!(json_f64_css_px(&json!("24pt")), None);
        assert_eq!(json_f64_css_px(&json!("NaNpx")), None);
    }
}
