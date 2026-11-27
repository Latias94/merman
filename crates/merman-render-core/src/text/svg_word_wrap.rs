//! SVG source-word wrapping with private streaming probes.

use super::measurement::{BuiltinSvgComputedLength, TRUSTED_MEASUREMENT};
use super::{TextMeasurer, TextStyle};
use unicode_segmentation::UnicodeSegmentation;

pub fn flowchart_wrap_svg_source_word_lines(
    measurer: &dyn TextMeasurer,
    lines: &[Vec<String>],
    style: &TextStyle,
    max_width_px: Option<f64>,
    break_long_words: bool,
) -> Vec<Vec<String>> {
    #[derive(Clone)]
    struct ProbeCheckpoint {
        builtin: Option<BuiltinSvgComputedLength>,
        visible_len: usize,
    }

    struct ComputedLengthProbe<'a> {
        measurer: &'a dyn TextMeasurer,
        style: &'a TextStyle,
        builtin: Option<BuiltinSvgComputedLength>,
        visible: String,
    }

    impl<'a> ComputedLengthProbe<'a> {
        fn new(measurer: &'a dyn TextMeasurer, style: &'a TextStyle) -> Self {
            Self {
                measurer,
                style,
                builtin: measurer.begin_svg_text_computed_length(TRUSTED_MEASUREMENT, style),
                visible: String::new(),
            }
        }

        fn has_builtin(&self) -> bool {
            self.builtin.is_some()
        }

        fn reset(&mut self) {
            if let Some(builtin) = self.builtin.as_mut() {
                builtin.reset();
            }
            self.visible.clear();
        }

        fn checkpoint(&self) -> ProbeCheckpoint {
            ProbeCheckpoint {
                builtin: self.builtin.clone(),
                visible_len: self.visible.len(),
            }
        }

        fn restore(&mut self, checkpoint: ProbeCheckpoint) {
            self.builtin = checkpoint.builtin;
            self.visible.truncate(checkpoint.visible_len);
        }

        fn push_visible(&mut self, text: &str) {
            if let Some(builtin) = self.builtin.as_mut() {
                builtin.push_text(text);
            } else {
                self.visible.push_str(text);
            }
        }

        fn push_source_word(&mut self, word: &str, prepend_space: bool) {
            if prepend_space {
                self.push_visible(" ");
            }
            let visible = crate::entities::decode_svg_text_content_entities(word);
            self.push_visible(visible.as_ref());
        }

        fn width_px(&self) -> f64 {
            self.builtin.as_ref().map_or_else(
                || {
                    self.measurer
                        .measure_svg_text_computed_length_px(&self.visible, self.style)
                },
                BuiltinSvgComputedLength::width_px,
            )
        }

        fn measure_source(&mut self, source: &str) -> f64 {
            self.reset();
            let visible = crate::entities::decode_svg_text_content_entities(source);
            self.push_visible(visible.as_ref());
            self.width_px()
        }
    }

    fn split_source_word_to_fit(
        probe: &mut ComputedLengthProbe<'_>,
        word: &str,
        max_width_px: f64,
    ) -> Vec<String> {
        let boundaries = word
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .chain(std::iter::once(word.len()))
            .collect::<Vec<_>>();
        if boundaries.len() <= 1 {
            return vec![word.to_string()];
        }

        let has_decoded_entities =
            word.contains("&lt;") || word.contains("&gt;") || word.contains("&amp;");
        if probe.has_builtin() && !has_decoded_entities {
            let mut chunks = Vec::new();
            let mut start_index = 0usize;
            while start_index + 1 < boundaries.len() {
                if probe.measurer.cancellation_requested() {
                    return Vec::new();
                }
                probe.reset();
                let mut split_index = start_index + 1;
                let mut previous_end = boundaries[start_index];
                for (end_index, &end) in boundaries.iter().enumerate().skip(start_index + 1) {
                    if probe.measurer.cancellation_requested() {
                        return Vec::new();
                    }
                    probe.push_visible(&word[previous_end..end]);
                    if probe.width_px() <= max_width_px {
                        split_index = end_index;
                        previous_end = end;
                    } else {
                        break;
                    }
                }
                chunks.push(word[boundaries[start_index]..boundaries[split_index]].to_string());
                start_index = split_index;
            }
            return chunks;
        }

        // Opaque/custom measurers retain Mermaid's exact growing-prefix request order. The
        // grapheme boundaries are collected once and output chunks borrow the original word, so
        // the compatibility path no longer recopies/resegments every shrinking tail.
        let mut chunks = Vec::new();
        let mut start_index = 0usize;
        let mut first_tail_already_measured = true;
        while start_index + 1 < boundaries.len() {
            if probe.measurer.cancellation_requested() {
                return Vec::new();
            }
            let start = boundaries[start_index];
            if !first_tail_already_measured && probe.measure_source(&word[start..]) <= max_width_px
            {
                chunks.push(word[start..].to_string());
                break;
            }
            first_tail_already_measured = false;

            let mut split_index = start_index + 1;
            for (end_index, &end) in boundaries.iter().enumerate().skip(start_index + 1) {
                if probe.measurer.cancellation_requested() {
                    return Vec::new();
                }
                if probe.measure_source(&word[start..end]) <= max_width_px {
                    split_index = end_index;
                } else {
                    break;
                }
            }
            chunks.push(word[start..boundaries[split_index]].to_string());
            start_index = split_index;
        }
        chunks
    }

    let Some(max_width_px) = max_width_px.filter(|width| width.is_finite() && *width > 0.0) else {
        if measurer.cancellation_requested() {
            return Vec::new();
        }
        return lines.to_vec();
    };

    let mut wrapped = Vec::new();
    let mut probe = ComputedLengthProbe::new(measurer, style);
    for source_line in lines {
        if measurer.cancellation_requested() {
            return Vec::new();
        }
        if source_line.is_empty() {
            wrapped.push(Vec::new());
            continue;
        }
        probe.reset();
        for (word_index, word) in source_line.iter().enumerate() {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            probe.push_source_word(word, word_index > 0);
        }
        if probe.width_px() <= max_width_px {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            wrapped.push(source_line.clone());
            continue;
        }

        probe.reset();
        let mut current = Vec::new();
        let mut word_index = 0usize;
        while let Some(word) = source_line.get(word_index) {
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            let checkpoint = probe.checkpoint();
            probe.push_source_word(word, !current.is_empty());
            if probe.width_px() <= max_width_px {
                if measurer.cancellation_requested() {
                    return Vec::new();
                }
                current.push(word.clone());
                word_index += 1;
                continue;
            }
            probe.restore(checkpoint);
            if !current.is_empty() {
                wrapped.push(std::mem::take(&mut current));
                probe.reset();
                continue;
            }
            if !break_long_words {
                wrapped.push(vec![word.clone()]);
                word_index += 1;
                continue;
            }

            let chunks = split_source_word_to_fit(&mut probe, word, max_width_px);
            if measurer.cancellation_requested() {
                return Vec::new();
            }
            for chunk in chunks {
                wrapped.push(vec![chunk]);
            }
            probe.reset();
            word_index += 1;
        }
        if !current.is_empty() {
            wrapped.push(current);
        }
    }

    if wrapped.is_empty() {
        vec![Vec::new()]
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{TextMetrics, measurement::TrustedMeasurementQuery};

    #[test]
    fn builtin_svg_word_wrapping_streams_long_grapheme_labels() {
        struct StreamingOnlyMeasurer;

        impl TextMeasurer for StreamingOnlyMeasurer {
            #[allow(private_interfaces)]
            fn begin_svg_text_computed_length(
                &self,
                _: TrustedMeasurementQuery,
                style: &TextStyle,
            ) -> Option<BuiltinSvgComputedLength> {
                Some(BuiltinSvgComputedLength::deterministic(style))
            }

            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 0.0,
                    height: 10.0,
                    line_count: 1,
                }
            }

            fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
                panic!("the built-in streaming path must not rescan complete prefixes")
            }
        }

        let word = "a".repeat(4_096);
        let wrapped = flowchart_wrap_svg_source_word_lines(
            &StreamingOnlyMeasurer,
            &[vec![word]],
            &TextStyle::default(),
            Some(1.0),
            true,
        );

        assert_eq!(wrapped.len(), 4_096);
        assert!(wrapped.iter().all(|line| line == &["a".to_string()]));
    }
}
