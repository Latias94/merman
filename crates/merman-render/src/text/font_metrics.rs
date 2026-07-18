//! Vendored browser/font metrics text measurer.

use super::line_break::html_break_spaces_segments;
use super::metrics::{style_requests_bold_font_weight, style_requests_italic_font_style};
use super::{
    DeterministicTextMeasurer, FLOWCHART_DEFAULT_FONT_KEY, TextMeasurer, TextMetrics, TextStyle,
    WrapMode, font_key_uses_courier_metrics, round_to_1_64_px,
    svg_wrapped_first_line_bbox_height_px,
};

const MERMAID_CALCULATE_TEXT_DIMENSIONS_FALLBACK_FONT_KEY: &str =
    "mermaid-calculate-text-dimensions-cssom-fallback";

#[derive(Debug, Clone, Default)]
pub struct VendoredFontMetricsTextMeasurer {
    fallback: DeterministicTextMeasurer,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FontMetricsTable {
    pub(crate) font_key: &'static str,
    pub(crate) variant: FontMetricsVariant,
    pub(crate) default_em: f64,
    pub(crate) entries: &'static [(char, f64)],
    pub(crate) kern_pairs: &'static [(u32, u32, f64)],
    pub(crate) space_trigrams: &'static [(u32, u32, f64)],
    pub(crate) trigrams: &'static [(u32, u32, u32, f64)],
    pub(crate) svg_scale: f64,
    pub(crate) svg_bbox_overhang_left_default_em: f64,
    pub(crate) svg_bbox_overhang_right_default_em: f64,
    pub(crate) svg_bbox_overhang_left: &'static [(char, f64)],
    pub(crate) svg_bbox_overhang_right: &'static [(char, f64)],
    pub(crate) svg_vertical_glyphs: &'static [char],
    pub(crate) svg_vertical_profiles: [SvgVerticalProfileSet; SvgVerticalDomShape::COUNT],
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SvgVerticalSizeProfile {
    pub(crate) font_size_px: u8,
    pub(crate) bbox_y_height_buckets: &'static [(f64, f64)],
    pub(crate) glyph_bucket_indices: &'static [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SvgVerticalDomShape {
    RawText,
    SingleTspan,
    CreateFormattedText,
    CreateFormattedTextMiddle,
}

impl SvgVerticalDomShape {
    pub(crate) const ALL: [Self; 4] = [
        Self::RawText,
        Self::SingleTspan,
        Self::CreateFormattedText,
        Self::CreateFormattedTextMiddle,
    ];
    pub(crate) const COUNT: usize = Self::ALL.len();

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::RawText => 0,
            Self::SingleTspan => 1,
            Self::CreateFormattedText => 2,
            Self::CreateFormattedTextMiddle => 3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SvgVerticalProfileSet {
    Approximate {
        bbox_y_em: f64,
        bbox_height_em: f64,
    },
    Profiled {
        approximate_bbox_y_em: f64,
        approximate_bbox_height_em: f64,
        pair_union_exact: bool,
        profiles: &'static [SvgVerticalSizeProfile],
    },
    Alias(SvgVerticalDomShape),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FontMetricsVariant {
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

impl FontMetricsVariant {
    pub(crate) fn from_style(style: &TextStyle) -> Self {
        match (
            style_requests_bold_font_weight(style),
            style_requests_italic_font_style(style),
        ) {
            (false, false) => Self::Regular,
            (true, false) => Self::Bold,
            (false, true) => Self::Italic,
            (true, true) => Self::BoldItalic,
        }
    }
}

impl FontMetricsTable {
    pub(crate) fn lookup_exact<'a>(
        tables: &'a [Self],
        font_key: &str,
        variant: FontMetricsVariant,
    ) -> Option<&'a Self> {
        tables
            .iter()
            .find(|table| table.font_key == font_key && table.variant == variant)
    }

    pub(crate) fn lookup<'a>(
        tables: &'a [Self],
        font_key: &str,
        variant: FontMetricsVariant,
    ) -> Option<&'a Self> {
        Self::lookup_exact(tables, font_key, variant).or_else(|| {
            tables.iter().find(|table| {
                table.font_key == font_key && table.variant == FontMetricsVariant::Regular
            })
        })
    }

    fn resolved_svg_vertical_profile_set(
        &self,
        shape: SvgVerticalDomShape,
    ) -> &SvgVerticalProfileSet {
        let mut current = shape;
        for _ in 0..SvgVerticalDomShape::COUNT {
            let profile = &self.svg_vertical_profiles[current.index()];
            match profile {
                SvgVerticalProfileSet::Alias(target) => current = *target,
                _ => return profile,
            }
        }
        unreachable!("validated SVG vertical profile aliases cannot form a cycle")
    }
}

#[derive(Clone, Copy)]
struct FontMetricProfile<'a> {
    entries: &'a [(char, f64)],
    default_em: f64,
    kern_pairs: &'a [(u32, u32, f64)],
    space_trigrams: &'a [(u32, u32, f64)],
    trigrams: &'a [(u32, u32, u32, f64)],
}

impl VendoredFontMetricsTextMeasurer {
    fn metric_profile(table: &FontMetricsTable) -> FontMetricProfile<'_> {
        FontMetricProfile {
            entries: table.entries,
            default_em: table.default_em.max(0.1),
            kern_pairs: table.kern_pairs,
            space_trigrams: table.space_trigrams,
            trigrams: table.trigrams,
        }
    }

    fn exact_svg_vertical_bbox_px(
        table: &FontMetricsTable,
        shape: SvgVerticalDomShape,
        text: &str,
        font_size_px: f64,
    ) -> Option<(f64, f64)> {
        if !font_size_px.is_finite()
            || font_size_px.fract() != 0.0
            || !(1.0..=64.0).contains(&font_size_px)
        {
            return None;
        }
        let SvgVerticalProfileSet::Profiled {
            pair_union_exact,
            profiles,
            ..
        } = table.resolved_svg_vertical_profile_set(shape)
        else {
            return None;
        };
        let profile = profiles
            .binary_search_by_key(&(font_size_px as u8), |profile| profile.font_size_px)
            .ok()
            .and_then(|index| profiles.get(index))?;

        let mut union: Option<(f64, f64)> = None;
        for character in text.chars() {
            let glyph_index = table.svg_vertical_glyphs.binary_search(&character).ok()?;
            let bucket_index = usize::from(*profile.glyph_bucket_indices.get(glyph_index)?);
            let (bbox_y, bbox_height) = *profile.bbox_y_height_buckets.get(bucket_index)?;
            if bbox_height == 0.0 {
                continue;
            }
            if !pair_union_exact && union.is_some() {
                return None;
            }
            let bbox_bottom = bbox_y + bbox_height;
            union = Some(match union {
                Some((union_top, union_bottom)) => {
                    (union_top.min(bbox_y), union_bottom.max(bbox_bottom))
                }
                None => (bbox_y, bbox_bottom),
            });
        }

        let (bbox_y, bbox_height) = union
            .map(|(top, bottom)| (top, (bottom - top).max(0.0)))
            .unwrap_or((0.0, 0.0));
        Some((bbox_y, bbox_height))
    }

    fn approximate_svg_vertical_bbox_px(
        table: &FontMetricsTable,
        shape: SvgVerticalDomShape,
        font_size_px: f64,
    ) -> (f64, f64) {
        let (bbox_y_em, bbox_height_em) = match table.resolved_svg_vertical_profile_set(shape) {
            SvgVerticalProfileSet::Approximate {
                bbox_y_em,
                bbox_height_em,
                ..
            } => (*bbox_y_em, *bbox_height_em),
            SvgVerticalProfileSet::Profiled {
                approximate_bbox_y_em,
                approximate_bbox_height_em,
                ..
            } => (*approximate_bbox_y_em, *approximate_bbox_height_em),
            SvgVerticalProfileSet::Alias(_) => {
                unreachable!("profile aliases are resolved before fallback lookup")
            }
        };
        let font_size_px = font_size_px.max(1.0);
        (bbox_y_em * font_size_px, bbox_height_em * font_size_px)
    }

    fn svg_vertical_bbox_with_table_px(
        table: &FontMetricsTable,
        shape: SvgVerticalDomShape,
        text: &str,
        font_size_px: f64,
    ) -> (f64, f64) {
        if text.is_empty() {
            return (0.0, 0.0);
        }
        Self::exact_svg_vertical_bbox_px(table, shape, text, font_size_px)
            .unwrap_or_else(|| Self::approximate_svg_vertical_bbox_px(table, shape, font_size_px))
    }

    fn svg_vertical_bbox_px(
        &self,
        shape: SvgVerticalDomShape,
        text: &str,
        style: &TextStyle,
    ) -> Option<(f64, f64)> {
        let table = self.lookup_exact_table(style)?;
        Some(Self::svg_vertical_bbox_with_table_px(
            table,
            shape,
            text,
            style.font_size,
        ))
    }

    fn svg_vertical_height_with_table_px(
        table: &FontMetricsTable,
        shape: SvgVerticalDomShape,
        text: &str,
        font_size_px: f64,
    ) -> f64 {
        Self::svg_vertical_bbox_with_table_px(table, shape, text, font_size_px).1
    }

    fn quantize_svg_half_px_nearest(half_px: f64) -> f64 {
        if !(half_px.is_finite() && half_px >= 0.0) {
            return 0.0;
        }
        // SVG `getBBox()` metrics in upstream Mermaid baselines tend to behave like a truncation
        // on a power-of-two grid for the anchored half-advance. Using `floor` here avoids a
        // systematic +1/256px drift in wide titles that can bubble up into `viewBox`/`max-width`.
        (half_px * 256.0).floor() / 256.0
    }

    fn normalize_font_key(s: &str) -> String {
        s.chars()
            .filter_map(|ch| {
                // Mermaid config strings occasionally embed the trailing CSS `;` in `fontFamily`.
                // We treat it as syntactic noise so lookups work with both `...sans-serif` and
                // `...sans-serif;`.
                if ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ';' {
                    None
                } else {
                    Some(ch.to_ascii_lowercase())
                }
            })
            .collect()
    }

    fn style_font_key(style: &TextStyle) -> String {
        let key = style
            .font_family
            .as_deref()
            .map(Self::normalize_font_key)
            .unwrap_or_default();
        if key.is_empty() {
            FLOWCHART_DEFAULT_FONT_KEY.to_string()
        } else {
            key
        }
    }

    fn lookup_exact_table(&self, style: &TextStyle) -> Option<&'static FontMetricsTable> {
        crate::generated::mermaid_font_metrics_11_16_0::lookup_exact_font_metrics(
            &Self::style_font_key(style),
            FontMetricsVariant::from_style(style),
        )
    }

    fn lookup_table(&self, style: &TextStyle) -> Option<&'static FontMetricsTable> {
        let variant = FontMetricsVariant::from_style(style);
        // Mermaid defaults to `"trebuchet ms", verdana, arial, sans-serif`. Many headless layout
        // call sites omit `font_family` and rely on that implicit default.
        let key = Self::style_font_key(style);
        let key = key.as_str();
        if let Some(t) =
            crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(key, variant)
        {
            return Some(t);
        }
        // Best-effort aliases for common stacks in upstream fixtures (Mermaid measures via DOM,
        // while our vendored tables cover a small set of representative families).
        let key_lower = key;
        if font_key_uses_courier_metrics(key_lower) {
            return crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(
                "courier", variant,
            );
        }
        // Prefer explicit generic stacks. If the font family does not match a known table and
        // does not include an explicit fallback token like `sans-serif`, fall back to the
        // deterministic measurer (unknown fonts vary widely across environments).
        if key_lower.contains("sans-serif") {
            return crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(
                "sans-serif",
                variant,
            );
        }
        None
    }

    fn approximate_vendored_svg_vertical_height_px(
        &self,
        shape: SvgVerticalDomShape,
        text: &str,
        style: &TextStyle,
    ) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        self.lookup_table(style)
            .map(|table| Self::approximate_svg_vertical_bbox_px(table, shape, style.font_size).1)
            .unwrap_or_else(|| match shape {
                SvgVerticalDomShape::RawText => self
                    .fallback
                    .measure_svg_raw_text_bbox_height_px(text, style),
                SvgVerticalDomShape::SingleTspan => self
                    .fallback
                    .measure_svg_tspan_text_bbox_height_px(text, style),
                SvgVerticalDomShape::CreateFormattedText
                | SvgVerticalDomShape::CreateFormattedTextMiddle => 0.0,
            })
    }

    fn measure_svg_vertical_height_px(
        &self,
        shape: SvgVerticalDomShape,
        text: &str,
        style: &TextStyle,
    ) -> f64 {
        if let Some(table) = self.lookup_exact_table(style) {
            Self::svg_vertical_height_with_table_px(table, shape, text, style.font_size)
        } else {
            self.approximate_vendored_svg_vertical_height_px(shape, text, style)
        }
    }

    fn find_entry_em(entries: &[(char, f64)], ch: char) -> Option<f64> {
        let mut lo = 0usize;
        let mut hi = entries.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match entries[mid].0.cmp(&ch) {
                std::cmp::Ordering::Equal => return Some(entries[mid].1),
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        None
    }

    fn normalize_profile_char(entries: &[(char, f64)], ch: char) -> char {
        if ch == '\u{00A0}' && Self::find_entry_em(entries, ch).is_none() {
            ' '
        } else {
            ch
        }
    }

    fn lookup_char_em(entries: &[(char, f64)], default_em: f64, ch: char) -> f64 {
        if let Some(em) = Self::find_entry_em(entries, ch) {
            return em;
        }

        if ch.is_ascii() {
            return default_em;
        }

        Self::lookup_non_ascii_fallback_em(default_em, ch)
    }

    fn lookup_non_ascii_fallback_em(default_em: f64, ch: char) -> f64 {
        match unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) {
            0 => 0.0,
            2.. => 1.0,
            _ => default_em,
        }
    }

    fn lookup_kern_em(kern_pairs: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = kern_pairs.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = kern_pairs[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn lookup_profile_kern_em(profile: FontMetricProfile<'_>, a: char, b: char) -> f64 {
        Self::lookup_kern_em(profile.kern_pairs, a, b)
    }

    fn lookup_space_trigram_em(space_trigrams: &[(u32, u32, f64)], a: char, b: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let mut lo = 0usize;
        let mut hi = space_trigrams.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, v) = space_trigrams[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b)) {
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => return v,
                (std::cmp::Ordering::Less, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn lookup_trigram_em(trigrams: &[(u32, u32, u32, f64)], a: char, b: char, c: char) -> f64 {
        let key_a = a as u32;
        let key_b = b as u32;
        let key_c = c as u32;
        let mut lo = 0usize;
        let mut hi = trigrams.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (ma, mb, mc, v) = trigrams[mid];
            match (ma.cmp(&key_a), mb.cmp(&key_b), mc.cmp(&key_c)) {
                (
                    std::cmp::Ordering::Equal,
                    std::cmp::Ordering::Equal,
                    std::cmp::Ordering::Equal,
                ) => return v,
                (std::cmp::Ordering::Less, _, _) => lo = mid + 1,
                (std::cmp::Ordering::Equal, std::cmp::Ordering::Less, _) => lo = mid + 1,
                (
                    std::cmp::Ordering::Equal,
                    std::cmp::Ordering::Equal,
                    std::cmp::Ordering::Less,
                ) => lo = mid + 1,
                _ => hi = mid,
            }
        }
        0.0
    }

    fn lookup_overhang_em(entries: &[(char, f64)], default_em: f64, ch: char) -> f64 {
        let mut lo = 0usize;
        let mut hi = entries.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            match entries[mid].0.cmp(&ch) {
                std::cmp::Ordering::Equal => return entries[mid].1,
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
            }
        }
        default_em
    }

    fn line_svg_bbox_extents_px(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> (f64, f64) {
        let profile = Self::metric_profile(table);
        let t = text.trim_end();
        if t.is_empty() {
            return (0.0, 0.0);
        }

        let first = t.chars().next().unwrap_or(' ');
        let last = t.chars().last().unwrap_or(' ');

        // Mermaid's SVG label renderer tokenizes whitespace into multiple inner `<tspan>` runs
        // (one word per run, with a leading space on subsequent runs).
        //
        // These boundaries can affect shaping/kerning vs treating the text as one run, and those
        // small differences bubble into Dagre layout and viewBox parity. Mirror the upstream
        // behavior by summing per-run advances when whitespace tokenization would occur.
        let advance_px_unscaled = {
            let words: Vec<&str> = t.split_whitespace().filter(|s| !s.is_empty()).collect();
            if words.len() >= 2 {
                let mut sum_px = 0.0f64;
                for (idx, w) in words.iter().enumerate() {
                    if idx == 0 {
                        sum_px += Self::line_width_px(profile, w, font_size);
                    } else {
                        let seg = format!(" {w}");
                        sum_px += Self::line_width_px(profile, &seg, font_size);
                    }
                }
                sum_px
            } else {
                Self::line_width_px(profile, t, font_size)
            }
        };

        let advance_px = advance_px_unscaled * table.svg_scale;
        let half = Self::quantize_svg_half_px_nearest((advance_px / 2.0).max(0.0));
        let left_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_left,
            table.svg_bbox_overhang_left_default_em,
            first,
        );
        let right_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_right,
            table.svg_bbox_overhang_right_default_em,
            last,
        );

        let left = (half + left_oh_em * font_size).max(0.0);
        let right = (half + right_oh_em * font_size).max(0.0);
        (left, right)
    }

    fn line_svg_bbox_extents_px_single_run(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> (f64, f64) {
        let profile = Self::metric_profile(table);
        let t = text.trim_end();
        if t.is_empty() {
            return (0.0, 0.0);
        }

        let first = t.chars().next().unwrap_or(' ');
        let last = t.chars().last().unwrap_or(' ');

        // Mermaid titles (e.g. flowchartTitleText) are rendered as a single `<text>` run, without
        // whitespace-tokenized `<tspan>` segments. Measure as one run to keep viewport parity.
        let advance_px_unscaled = Self::line_width_px(profile, t, font_size);

        let advance_px = advance_px_unscaled * table.svg_scale;
        let half = Self::quantize_svg_half_px_nearest((advance_px / 2.0).max(0.0));

        let left_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_left,
            table.svg_bbox_overhang_left_default_em,
            first,
        );
        let right_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_right,
            table.svg_bbox_overhang_right_default_em,
            last,
        );

        let left = (half + left_oh_em * font_size).max(0.0);
        let right = (half + right_oh_em * font_size).max(0.0);
        (left, right)
    }

    fn line_svg_bbox_extents_px_single_run_with_ascii_overhang(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> (f64, f64) {
        let profile = Self::metric_profile(table);
        let t = text.trim_end();
        if t.is_empty() {
            return (0.0, 0.0);
        }

        let first = t.chars().next().unwrap_or(' ');
        let last = t.chars().last().unwrap_or(' ');

        let advance_px_unscaled = Self::line_width_px(profile, t, font_size);

        let advance_px = advance_px_unscaled * table.svg_scale;
        let half = Self::quantize_svg_half_px_nearest((advance_px / 2.0).max(0.0));

        let left_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_left,
            table.svg_bbox_overhang_left_default_em,
            first,
        );
        let right_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_right,
            table.svg_bbox_overhang_right_default_em,
            last,
        );

        let left = (half + left_oh_em * font_size).max(0.0);
        let right = (half + right_oh_em * font_size).max(0.0);
        (left, right)
    }

    fn line_svg_bbox_width_px(table: &FontMetricsTable, text: &str, font_size: f64) -> f64 {
        let (l, r) = Self::line_svg_bbox_extents_px(table, text, font_size);
        (l + r).max(0.0)
    }

    fn line_svg_bbox_width_single_run_px(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> f64 {
        let (l, r) = Self::line_svg_bbox_extents_px_single_run(table, text, font_size);
        (l + r).max(0.0)
    }

    fn line_svg_title_bbox_extents_px(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> (f64, f64) {
        let profile = Self::metric_profile(table);
        let t = text.trim_end();
        if t.is_empty() {
            return (0.0, 0.0);
        }

        // Flowchart titles are emitted as a centered single `<text>` node. The final upstream
        // root bbox behaves as a symmetric title advance, while simple-text probes include
        // per-edge glyph overhang. Keep these DOM measurement shapes separate.
        let advance_px = Self::line_width_px(profile, t, font_size) * table.svg_scale;
        let half = Self::quantize_svg_half_px_nearest((advance_px / 2.0).max(0.0));
        (half, half)
    }

    fn split_token_to_svg_bbox_width_px(
        table: &FontMetricsTable,
        tok: &str,
        max_width_px: f64,
        font_size: f64,
    ) -> (String, String) {
        if max_width_px <= 0.0 {
            return (tok.to_string(), String::new());
        }
        let chars = tok.chars().collect::<Vec<_>>();
        if chars.is_empty() {
            return (String::new(), String::new());
        }

        let first = Self::normalize_profile_char(table.entries, chars[0]);
        let left_oh_em = Self::lookup_overhang_em(
            table.svg_bbox_overhang_left,
            table.svg_bbox_overhang_left_default_em,
            first,
        );

        let mut em = 0.0;
        let mut prev: Option<char> = None;
        let mut split_at = 1usize;
        for (idx, ch) in chars.iter().enumerate() {
            let profile_ch = Self::normalize_profile_char(table.entries, *ch);
            em += Self::lookup_char_em(table.entries, table.default_em.max(0.1), profile_ch);
            if let Some(p) = prev {
                em += Self::lookup_kern_em(table.kern_pairs, p, profile_ch);
            }
            prev = Some(profile_ch);

            let right_oh_em = Self::lookup_overhang_em(
                table.svg_bbox_overhang_right,
                table.svg_bbox_overhang_right_default_em,
                profile_ch,
            );
            let half_px = Self::quantize_svg_half_px_nearest(
                (em * font_size * table.svg_scale / 2.0).max(0.0),
            );
            let w_px = 2.0 * half_px + (left_oh_em + right_oh_em) * font_size;
            if w_px.is_finite() && w_px <= max_width_px {
                split_at = idx + 1;
            } else if idx > 0 {
                break;
            }
        }
        let head = chars[..split_at].iter().collect::<String>();
        let tail = chars[split_at..].iter().collect::<String>();
        (head, tail)
    }

    fn wrap_text_lines_svg_bbox_px(
        table: &FontMetricsTable,
        normalized_lines: &[String],
        max_width_px: Option<f64>,
        font_size: f64,
        tokenize_whitespace: bool,
    ) -> Vec<String> {
        let max_width_px = max_width_px.filter(|w| w.is_finite() && *w > 0.0);
        let width_fn = if tokenize_whitespace {
            Self::line_svg_bbox_width_px
        } else {
            Self::line_svg_bbox_width_single_run_px
        };

        let mut lines = Vec::new();
        for line in normalized_lines {
            let Some(w) = max_width_px else {
                lines.push(line.clone());
                continue;
            };

            let mut tokens = std::collections::VecDeque::from(
                DeterministicTextMeasurer::split_line_to_words(line),
            );
            let mut out: Vec<String> = Vec::new();
            let mut cur = String::new();

            while let Some(tok) = tokens.pop_front() {
                if cur.is_empty() && tok == " " {
                    continue;
                }

                let candidate = format!("{cur}{tok}");
                let candidate_trimmed = candidate.trim_end();
                if width_fn(table, candidate_trimmed, font_size) <= w {
                    cur = candidate;
                    continue;
                }

                if !cur.trim().is_empty() {
                    out.push(cur.trim_end().to_string());
                    cur.clear();
                    tokens.push_front(tok);
                    continue;
                }

                if tok == " " {
                    continue;
                }

                if width_fn(table, tok.as_str(), font_size) <= w {
                    cur = tok;
                    continue;
                }

                // Mermaid's SVG wrapping breaks long words.
                let (head, tail) =
                    Self::split_token_to_svg_bbox_width_px(table, &tok, w, font_size);
                out.push(head);
                if !tail.is_empty() {
                    tokens.push_front(tail);
                }
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
            }

            if out.is_empty() {
                lines.push("".to_string());
            } else {
                lines.extend(out);
            }
        }

        if lines.is_empty() {
            vec!["".to_string()]
        } else {
            lines
        }
    }

    fn line_width_px(profile: FontMetricProfile<'_>, text: &str, font_size: f64) -> f64 {
        let mut em = 0.0;
        let mut prevprev: Option<char> = None;
        let mut prev: Option<char> = None;
        for ch in text.chars() {
            let ch = Self::normalize_profile_char(profile.entries, ch);
            em += Self::lookup_char_em(profile.entries, profile.default_em, ch);
            if let Some(p) = prev {
                em += Self::lookup_profile_kern_em(profile, p, ch);
            }
            if let (Some(a), Some(b)) = (prevprev, prev) {
                if b == ' ' {
                    if !(a.is_whitespace() || ch.is_whitespace()) {
                        let space_delta =
                            Self::lookup_space_trigram_em(profile.space_trigrams, a, ch);
                        em += space_delta;
                    }
                } else if !(a.is_whitespace() || b.is_whitespace() || ch.is_whitespace()) {
                    em += Self::lookup_trigram_em(profile.trigrams, a, b, ch);
                }
            }
            prevprev = prev;
            prev = Some(ch);
        }
        em * font_size
    }

    fn split_token_to_width_px(
        profile: FontMetricProfile<'_>,
        tok: &str,
        max_width_px: f64,
        font_size: f64,
    ) -> (String, String) {
        if max_width_px <= 0.0 {
            return (tok.to_string(), String::new());
        }
        let max_em = max_width_px / font_size.max(1.0);
        let mut em = 0.0;
        let mut prevprev: Option<char> = None;
        let mut prev: Option<char> = None;
        let chars = tok.chars().collect::<Vec<_>>();
        let mut split_at = 0usize;
        for (idx, ch) in chars.iter().enumerate() {
            let ch_norm = Self::normalize_profile_char(profile.entries, *ch);
            em += Self::lookup_char_em(profile.entries, profile.default_em, ch_norm);
            if let Some(p) = prev {
                em += Self::lookup_profile_kern_em(profile, p, ch_norm);
            }
            if let (Some(a), Some(b)) = (prevprev, prev)
                && !(a.is_whitespace() || b.is_whitespace() || ch_norm.is_whitespace())
            {
                em += Self::lookup_trigram_em(profile.trigrams, a, b, ch_norm);
            }
            prevprev = prev;
            prev = Some(ch_norm);
            if em > max_em && idx > 0 {
                break;
            }
            split_at = idx + 1;
            if em >= max_em {
                break;
            }
        }
        if split_at == 0 {
            split_at = 1.min(chars.len());
        }
        let head = chars.iter().take(split_at).collect::<String>();
        let tail = chars.iter().skip(split_at).collect::<String>();
        (head, tail)
    }

    fn wrap_line_to_width_px(
        profile: FontMetricProfile<'_>,
        line: &str,
        max_width_px: f64,
        font_size: f64,
        break_long_words: bool,
    ) -> Vec<String> {
        if !break_long_words {
            let mut out = Vec::new();
            let mut current = String::new();

            for segment in html_break_spaces_segments(line) {
                let mut candidate = current.clone();
                candidate.push_str(segment);
                if current.is_empty()
                    || Self::line_width_px(profile, &candidate, font_size) <= max_width_px
                {
                    current = candidate;
                    continue;
                }

                out.push(std::mem::take(&mut current));
                current.push_str(segment);
            }

            if !current.is_empty() {
                out.push(current);
            }
            return if out.is_empty() {
                vec![String::new()]
            } else {
                out
            };
        }

        let mut tokens =
            std::collections::VecDeque::from(DeterministicTextMeasurer::split_line_to_words(line));
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            let candidate_trimmed = candidate.trim_end();
            if Self::line_width_px(profile, candidate_trimmed, font_size) <= max_width_px {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
            }

            if tok == " " {
                continue;
            }

            if Self::line_width_px(profile, tok.as_str(), font_size) <= max_width_px {
                cur = tok;
                continue;
            }

            let (head, tail) =
                Self::split_token_to_width_px(profile, &tok, max_width_px, font_size);
            out.push(head);
            if !tail.is_empty() {
                tokens.push_front(tail);
            }
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
        }

        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    fn wrap_text_lines_px(
        profile: FontMetricProfile<'_>,
        normalized_lines: &[String],
        style: &TextStyle,
        max_width_px: Option<f64>,
        wrap_mode: WrapMode,
    ) -> Vec<String> {
        let font_size = style.font_size.max(1.0);
        let max_width_px = max_width_px.filter(|w| w.is_finite() && *w > 0.0);
        let break_long_words = wrap_mode == WrapMode::SvgLike;

        let mut lines = Vec::new();
        for line in normalized_lines {
            if let Some(w) = max_width_px {
                lines.extend(Self::wrap_line_to_width_px(
                    profile,
                    line,
                    w,
                    font_size,
                    break_long_words,
                ));
            } else {
                lines.push(line.clone());
            }
        }

        if lines.is_empty() {
            vec!["".to_string()]
        } else {
            lines
        }
    }

    fn raw_svg_text_bbox_width_unadjusted_px(&self, text: &str, style: &TextStyle) -> f64 {
        let Some(table) = self.lookup_table(style) else {
            return self
                .fallback
                .measure_svg_raw_text_bbox_width_px(text, style);
        };

        let font_size = style.font_size.max(1.0);
        let mut width: f64 = 0.0;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            let (left, right) = Self::line_svg_bbox_extents_px_single_run_with_ascii_overhang(
                table, &line, font_size,
            );
            width = width.max((left + right).max(0.0));
        }
        width
    }

    fn measure_svg_single_run_bbox_width_with_table(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> f64 {
        let mut width = 0.0_f64;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            let (left, right) = Self::line_svg_bbox_extents_px_single_run_with_ascii_overhang(
                table,
                &line,
                font_size.max(1.0),
            );
            width = width.max((left + right).max(0.0));
        }
        width
    }

    fn normalize_svg_tspan_whitespace(text: &str) -> String {
        let mut normalized = String::with_capacity(text.len());
        let mut pending_space = false;
        for character in text.chars() {
            if matches!(character, ' ' | '\t' | '\n' | '\r') {
                pending_space = !normalized.is_empty();
                continue;
            }
            if pending_space {
                normalized.push(' ');
                pending_space = false;
            }
            normalized.push(character);
        }
        normalized
    }

    fn measure_mermaid_calculate_text_dimensions_width_with_table(
        table: &FontMetricsTable,
        text: &str,
        font_size: f64,
    ) -> f64 {
        let profile = Self::metric_profile(table);
        let font_size = font_size.max(1.0);
        let Some(first) = text.chars().next() else {
            return 0.0;
        };
        let last = text.chars().next_back().unwrap_or(first);
        let advance = Self::line_width_px(profile, text, font_size) * table.svg_scale;
        let left_overhang = Self::lookup_overhang_em(
            table.svg_bbox_overhang_left,
            table.svg_bbox_overhang_left_default_em,
            first,
        );
        let right_overhang = Self::lookup_overhang_em(
            table.svg_bbox_overhang_right,
            table.svg_bbox_overhang_right_default_em,
            last,
        );
        (advance + (left_overhang + right_overhang) * font_size).max(0.0)
    }
}

fn vendored_measure_wrapped_impl(
    measurer: &VendoredFontMetricsTextMeasurer,
    text: &str,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> (TextMetrics, Option<f64>) {
    let Some(table) = measurer.lookup_table(style) else {
        return measurer
            .fallback
            .measure_wrapped_with_raw_width(text, style, max_width, wrap_mode);
    };

    let font_size = style.font_size.max(1.0);
    let max_width = max_width.filter(|w| w.is_finite() && *w > 0.0);
    let line_height_factor = match wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        WrapMode::HtmlLike => 1.5,
    };

    let profile = VendoredFontMetricsTextMeasurer::metric_profile(table);
    let normalized_lines = DeterministicTextMeasurer::normalized_text_lines(text);

    // Mermaid HTML labels behave differently depending on whether the content "needs" wrapping:
    // - if the unwrapped line width exceeds the configured wrapping width, Mermaid constrains
    //   the element to `width=max_width` and lets HTML wrapping determine line breaks
    //   (`white-space: break-spaces` / `width: 200px` patterns in upstream SVGs).
    // - otherwise, Mermaid uses an auto-sized container and measures the natural width.
    //
    // In headless mode we model this by computing the unwrapped width first, then forcing the
    // measured width to `max_width` when it would overflow.
    let raw_width_unscaled = if wrap_mode == WrapMode::HtmlLike {
        let mut raw_w: f64 = 0.0;
        for line in &normalized_lines {
            let line_width =
                VendoredFontMetricsTextMeasurer::line_width_px(profile, line, font_size);
            raw_w = raw_w.max(line_width);
        }
        Some(raw_w)
    } else {
        None
    };

    // Mermaid's HTML label measurements are taken from a `<div style="max-width: wpx">` that is
    // later switched to `display: table; width: wpx; white-space: break-spaces` when it hits the
    // max width.
    //
    // When a "word" (space-delimited token) is wider than the configured max width, browsers may
    // still wrap other parts of the paragraph, but the element's measured bounding box can expand
    // to accommodate the token's min-content width. Upstream Mermaid records that via
    // `getBoundingClientRect()` into `foreignObject width="..."`.
    //
    // Model this by tracking the widest UAX #14 / `break-spaces` segment as a separate
    // "min-content" contributor, without changing the width used for greedy line breaking.
    let html_min_content_width = if wrap_mode == WrapMode::HtmlLike && max_width.is_some() {
        let mut max_word_w: f64 = 0.0;
        for line in &normalized_lines {
            for segment in html_break_spaces_segments(line) {
                let segment_width =
                    VendoredFontMetricsTextMeasurer::line_width_px(profile, segment, font_size);
                max_word_w = max_word_w.max(segment_width);
            }
        }
        if max_word_w.is_finite() && max_word_w > 0.0 {
            Some(max_word_w)
        } else {
            None
        }
    } else {
        None
    };

    let lines = match wrap_mode {
        WrapMode::HtmlLike => VendoredFontMetricsTextMeasurer::wrap_text_lines_px(
            profile,
            &normalized_lines,
            style,
            max_width,
            wrap_mode,
        ),
        WrapMode::SvgLike => VendoredFontMetricsTextMeasurer::wrap_text_lines_svg_bbox_px(
            table,
            &normalized_lines,
            max_width,
            font_size,
            true,
        ),
        WrapMode::SvgLikeSingleRun => VendoredFontMetricsTextMeasurer::wrap_text_lines_svg_bbox_px(
            table,
            &normalized_lines,
            max_width,
            font_size,
            false,
        ),
    };

    let mut width: f64 = 0.0;
    match wrap_mode {
        WrapMode::HtmlLike => {
            for line in &lines {
                let line_width =
                    VendoredFontMetricsTextMeasurer::line_width_px(profile, line, font_size);
                width = width.max(line_width);
            }
        }
        WrapMode::SvgLike => {
            for line in &lines {
                let line_width =
                    VendoredFontMetricsTextMeasurer::line_svg_bbox_width_px(table, line, font_size);
                width = width.max(line_width);
            }
        }
        WrapMode::SvgLikeSingleRun => {
            for line in &lines {
                let line_width = VendoredFontMetricsTextMeasurer::line_svg_bbox_width_single_run_px(
                    table, line, font_size,
                );
                width = width.max(line_width);
            }
        }
    }

    // Mermaid HTML labels use `max-width` and can visually overflow for long words, but their
    // layout width is at least the max width in "wrapped" mode (tables), and may exceed it for
    // long unbreakable tokens.
    if wrap_mode == WrapMode::HtmlLike {
        let needs_wrap = max_width.is_some_and(|w| raw_width_unscaled.is_some_and(|rw| rw > w));
        if let Some(w) = max_width {
            if needs_wrap {
                width = width.max(w);
            } else {
                width = width.min(w);
            }
        }
        if needs_wrap && let Some(w) = html_min_content_width {
            width = width.max(w);
        }
        // Empirically, upstream HTML label widths (via `getBoundingClientRect()`) land on a 1/64px
        // lattice. Quantize to that grid to keep our layout math stable.
        width = round_to_1_64_px(width);
        if let Some(w) = max_width {
            width = if needs_wrap {
                width.max(w)
            } else {
                width.min(w)
            };
        }
    }

    let height = match wrap_mode {
        WrapMode::HtmlLike => lines.len() as f64 * font_size * line_height_factor,
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => {
            if lines.is_empty() {
                0.0
            } else {
                // Mermaid's SVG `<text>.getBBox().height` behaves as "one taller first line"
                // plus 1.1em per additional wrapped line (observed in upstream fixtures at
                // Mermaid@11.12.2).
                // Chromium often reports an integer first-line bbox height; keep ties-to-even
                // rounding so `28.5px` becomes `28px` (matching upstream class SVG probes).
                let first_line_h = svg_wrapped_first_line_bbox_height_px(style);
                let additional = (lines.len().saturating_sub(1)) as f64 * font_size * 1.1;
                first_line_h + additional
            }
        }
    };

    let metrics = TextMetrics {
        width,
        height,
        line_count: lines.len(),
    };
    let raw_width_px = if wrap_mode == WrapMode::HtmlLike {
        raw_width_unscaled
    } else {
        None
    };
    (metrics, raw_width_px)
}

impl TextMeasurer for VendoredFontMetricsTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::SvgLike)
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.svg_vertical_bbox_px(SvgVerticalDomShape::CreateFormattedText, text, style)
            .map(|bbox| bbox.0)
            .unwrap_or(0.0)
    }

    fn measure_svg_create_text_middle_bbox_y_offset_px(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> f64 {
        self.svg_vertical_bbox_px(SvgVerticalDomShape::CreateFormattedTextMiddle, text, style)
            .map(|bbox| bbox.0)
            .unwrap_or(0.0)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_vertical_height_px(SvgVerticalDomShape::SingleTspan, text, style)
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        let Some(table) = self.lookup_table(style) else {
            return self
                .fallback
                .measure_svg_text_computed_length_px(text, style);
        };

        let font_size = style.font_size.max(1.0);
        let profile = VendoredFontMetricsTextMeasurer::metric_profile(table);
        let mut width: f64 = 0.0;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            width = width.max(VendoredFontMetricsTextMeasurer::line_width_px(
                profile, &line, font_size,
            ));
        }
        if width.is_finite() && width >= 0.0 {
            width
        } else {
            0.0
        }
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        let Some(table) = self.lookup_table(style) else {
            return self.fallback.measure_svg_text_bbox_x(text, style);
        };

        let font_size = style.font_size.max(1.0);
        let mut left: f64 = 0.0;
        let mut right: f64 = 0.0;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            let (l, r) = Self::line_svg_bbox_extents_px(table, &line, font_size);
            left = left.max(l);
            right = right.max(r);
        }
        (left, right)
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> (f64, f64) {
        let Some(table) = self.lookup_table(style) else {
            return self
                .fallback
                .measure_svg_text_bbox_x_with_ascii_overhang(text, style);
        };

        let font_size = style.font_size.max(1.0);
        let mut left: f64 = 0.0;
        let mut right: f64 = 0.0;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            let (l, r) = Self::line_svg_bbox_extents_px_single_run_with_ascii_overhang(
                table, &line, font_size,
            );
            left = left.max(l);
            right = right.max(r);
        }
        (left, right)
    }

    fn measure_svg_title_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        let Some(table) = self.lookup_table(style) else {
            return self.fallback.measure_svg_title_bbox_x(text, style);
        };

        let font_size = style.font_size.max(1.0);
        let mut left: f64 = 0.0;
        let mut right: f64 = 0.0;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            let (l, r) = Self::line_svg_title_bbox_extents_px(table, &line, font_size);
            left = left.max(l);
            right = right.max(r);
        }
        (left, right)
    }

    fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        let Some(table) = self.lookup_table(style) else {
            return self
                .fallback
                .measure_svg_simple_text_bbox_width_px(text, style);
        };

        Self::measure_svg_single_run_bbox_width_with_table(table, text, style.font_size)
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(&self, text: &str, style: &TextStyle) -> f64 {
        let Some(table) = self.lookup_table(style) else {
            return self
                .fallback
                .measure_svg_simple_text_bbox_width_for_wrap_px(text, style);
        };

        let font_size = style.font_size.max(1.0);
        let mut width: f64 = 0.0;
        for line in DeterministicTextMeasurer::normalized_text_lines(text) {
            let (l, r) = Self::line_svg_bbox_extents_px_single_run_with_ascii_overhang(
                table, &line, font_size,
            );
            width = width.max((l + r).max(0.0));
        }
        width
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.raw_svg_text_bbox_width_unadjusted_px(text, style)
    }

    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_vertical_height_px(SvgVerticalDomShape::RawText, text, style)
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.raw_svg_text_bbox_width_unadjusted_px(text, style)
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        let normalized_text = Self::normalize_svg_tspan_whitespace(text);
        let text = normalized_text.as_str();
        let cssom_fallback = style
            .font_family
            .as_deref()
            .is_some_and(|family| family.trim_end().ends_with(';'));
        if cssom_fallback {
            // Mermaid assigns this value through CSSOM on the body-attached probe. Its historical
            // trailing semicolon makes the declaration invalid. The generated operation profile
            // records Chrome 131's body-attached SVG fallback without exposing it as a generic
            // `serif` table to unrelated measurement operations.
            let variant = FontMetricsVariant::from_style(style);
            if let Some(table) = crate::generated::mermaid_calculate_text_dimensions_font_metrics_11_16_0::lookup_exact_font_metrics(
                MERMAID_CALCULATE_TEXT_DIMENSIONS_FALLBACK_FONT_KEY,
                variant,
            ) {
                return TextMetrics {
                    width: Self::measure_mermaid_calculate_text_dimensions_width_with_table(
                        table,
                        text,
                        style.font_size,
                    ),
                    height: if text.trim_end().is_empty() {
                        0.0
                    } else {
                        Self::svg_vertical_height_with_table_px(
                            table,
                            SvgVerticalDomShape::SingleTspan,
                            text,
                            style.font_size,
                        )
                    },
                    line_count: 1,
                };
            }
        }
        TextMetrics {
            width: self.measure_svg_simple_text_bbox_width_for_wrap_px(text, style),
            height: self.measure_svg_vertical_height_px(
                SvgVerticalDomShape::SingleTspan,
                text,
                style,
            ),
            line_count: 1,
        }
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_vertical_height_px(SvgVerticalDomShape::SingleTspan, text, style)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        vendored_measure_wrapped_impl(self, text, style, max_width, wrap_mode).0
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        vendored_measure_wrapped_impl(self, text, style, max_width, wrap_mode)
    }
}

#[cfg(test)]
mod vertical_profile_tests {
    use super::{
        FontMetricsTable, FontMetricsVariant, SvgVerticalDomShape, SvgVerticalProfileSet,
        SvgVerticalSizeProfile, VendoredFontMetricsTextMeasurer,
    };

    fn exact_profile(glyphs: &[char], bbox_y: f64, bbox_height: f64) -> SvgVerticalProfileSet {
        let mut indices = vec![1; glyphs.len()];
        indices[glyphs.binary_search(&' ').unwrap()] = 0;
        SvgVerticalProfileSet::Profiled {
            approximate_bbox_y_em: bbox_y / 10.0,
            approximate_bbox_height_em: bbox_height / 10.0,
            pair_union_exact: true,
            profiles: Box::leak(
                vec![SvgVerticalSizeProfile {
                    font_size_px: 10,
                    bbox_y_height_buckets: Box::leak(
                        vec![(0.0, 0.0), (bbox_y, bbox_height)].into_boxed_slice(),
                    ),
                    glyph_bucket_indices: Box::leak(indices.into_boxed_slice()),
                }]
                .into_boxed_slice(),
            ),
        }
    }

    fn table(variant: FontMetricsVariant) -> FontMetricsTable {
        let glyphs = (' '..='~')
            .chain(['°', '¶', 'ß', '\u{200b}', 'ﬂ'])
            .collect::<Vec<_>>();
        let svg_vertical_profiles = [
            exact_profile(&glyphs, -9.0, 11.0),
            exact_profile(&glyphs, -8.0, 12.0),
            exact_profile(&glyphs, 1.0, 11.0),
            exact_profile(&glyphs, 5.0, 11.0),
        ];
        FontMetricsTable {
            font_key: "probe",
            variant,
            default_em: 0.5,
            entries: &[],
            kern_pairs: &[],
            space_trigrams: &[],
            trigrams: &[],
            svg_scale: 1.0,
            svg_bbox_overhang_left_default_em: 0.0,
            svg_bbox_overhang_right_default_em: 0.0,
            svg_bbox_overhang_left: &[],
            svg_bbox_overhang_right: &[],
            svg_vertical_glyphs: Box::leak(glyphs.into_boxed_slice()),
            svg_vertical_profiles,
        }
    }

    #[test]
    fn raw_text_and_single_tspan_use_their_own_synthetic_profiles() {
        let table = table(FontMetricsVariant::Regular);

        assert_eq!(
            VendoredFontMetricsTextMeasurer::exact_svg_vertical_bbox_px(
                &table,
                SvgVerticalDomShape::RawText,
                "Note",
                10.0,
            ),
            Some((-9.0, 11.0))
        );
        assert_eq!(
            VendoredFontMetricsTextMeasurer::exact_svg_vertical_bbox_px(
                &table,
                SvgVerticalDomShape::SingleTspan,
                "Note",
                10.0,
            ),
            Some((-8.0, 12.0))
        );
    }

    #[test]
    fn formatted_text_ordinary_and_middle_bboxes_are_independent_profile_facts() {
        let table = table(FontMetricsVariant::Regular);

        assert_eq!(
            VendoredFontMetricsTextMeasurer::exact_svg_vertical_bbox_px(
                &table,
                SvgVerticalDomShape::CreateFormattedText,
                "g",
                10.0,
            ),
            Some((1.0, 11.0))
        );
        assert_eq!(
            VendoredFontMetricsTextMeasurer::exact_svg_vertical_bbox_px(
                &table,
                SvgVerticalDomShape::CreateFormattedTextMiddle,
                "g",
                10.0,
            ),
            Some((5.0, 11.0))
        );
    }

    #[test]
    fn zero_height_space_glyphs_do_not_expand_non_empty_bbox_unions() {
        let table = table(FontMetricsVariant::Regular);
        for text in ["A", " A", "A ", " A "] {
            assert_eq!(
                VendoredFontMetricsTextMeasurer::exact_svg_vertical_bbox_px(
                    &table,
                    SvgVerticalDomShape::RawText,
                    text,
                    10.0,
                ),
                Some((-9.0, 11.0))
            );
        }
        assert_eq!(
            VendoredFontMetricsTextMeasurer::exact_svg_vertical_bbox_px(
                &table,
                SvgVerticalDomShape::RawText,
                " ",
                10.0,
            ),
            Some((0.0, 0.0))
        );
    }

    #[test]
    fn unsupported_vertical_inputs_use_explicitly_approximate_shape_fallbacks() {
        let table = table(FontMetricsVariant::Regular);
        assert_eq!(
            VendoredFontMetricsTextMeasurer::svg_vertical_bbox_with_table_px(
                &table,
                SvgVerticalDomShape::RawText,
                "é",
                10.0,
            ),
            (-9.0, 11.0)
        );
    }

    #[test]
    fn exact_vertical_lookup_never_substitutes_a_font_or_variant() {
        let table = table(FontMetricsVariant::Regular);
        let tables = [table];

        assert!(
            FontMetricsTable::lookup_exact(&tables, "probe", FontMetricsVariant::Regular).is_some()
        );
        assert!(
            FontMetricsTable::lookup_exact(&tables, "missing", FontMetricsVariant::Regular)
                .is_none()
        );
        assert!(
            FontMetricsTable::lookup_exact(&tables, "probe", FontMetricsVariant::Bold).is_none()
        );
    }
}
