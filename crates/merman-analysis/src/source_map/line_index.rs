use crate::retained_weight::RetainedWeight;
use std::mem::size_of;

const LINE_INDEX_CHECKPOINT_BYTES: usize = 4 * 1024;
const BITMAP_WORD_BITS: usize = u64::BITS as usize;
const BITMAP_RANK_WORDS: usize = 8;
const LINE_INDEX_BOX_ALLOCATION_OVERHEAD: usize = 2 * size_of::<usize>();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineIndexRepresentation {
    Offsets32,
    OffsetsWide,
    Bitmap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineIndexBuildPass {
    Count,
    Fill,
}

#[derive(Debug)]
pub(super) struct LineIndex(LineIndexStorage);

#[derive(Debug)]
enum LineIndexStorage {
    Offsets32(Box<[u32]>),
    OffsetsWide(Box<[usize]>),
    Bitmap(BitmapLineIndex),
}

#[derive(Debug)]
struct BitmapLineIndex {
    bits: Box<[u64]>,
    block_ranks: Box<[usize]>,
    line_count: usize,
}

impl LineIndex {
    pub(super) fn build(source: &str) -> Self {
        Self::build_with_checkpoint(source, |_, _| Ok::<_, std::convert::Infallible>(()))
            .expect("infallible line-index scan")
    }

    pub(super) fn build_cancellable(
        source: &str,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        Self::build_with_checkpoint(source, |_, _| cancellation.checkpoint())
    }

    fn build_with_checkpoint<E>(
        source: &str,
        mut checkpoint: impl FnMut(LineIndexBuildPass, usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut line_count = 0usize;
        scan_line_starts(
            source,
            |offset| checkpoint(LineIndexBuildPass::Count, offset),
            |_| {
                line_count = line_count
                    .checked_add(1)
                    .expect("source line count must fit usize");
            },
        )?;

        let representation = select_line_index_representation(source.len(), line_count);
        Self::fill_with_checkpoint(source, representation, line_count, |offset| {
            checkpoint(LineIndexBuildPass::Fill, offset)
        })
    }

    fn fill_with_checkpoint<E>(
        source: &str,
        representation: LineIndexRepresentation,
        line_count: usize,
        mut checkpoint: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        // Do not allocate the final index if cancellation became pending after
        // the count pass.
        checkpoint(0)?;
        let mut builder = LineIndexBuilder::new(representation, source.len(), line_count);
        let mut filled = 0usize;
        scan_line_starts(source, &mut checkpoint, |start| {
            builder.push(filled, start);
            filled += 1;
        })?;
        assert_eq!(
            filled, line_count,
            "line-index count and fill passes diverged"
        );
        builder.finish(line_count, source.len(), checkpoint)
    }

    pub(super) fn line_count(&self) -> usize {
        match &self.0 {
            LineIndexStorage::Offsets32(starts) => starts.len(),
            LineIndexStorage::OffsetsWide(starts) => starts.len(),
            LineIndexStorage::Bitmap(index) => index.line_count,
        }
    }

    pub(super) fn line_start(&self, line_index: usize) -> Option<usize> {
        match &self.0 {
            LineIndexStorage::Offsets32(starts) => {
                starts.get(line_index).map(|&start| start as usize)
            }
            LineIndexStorage::OffsetsWide(starts) => starts.get(line_index).copied(),
            LineIndexStorage::Bitmap(index) => index.line_start(line_index),
        }
    }

    pub(super) fn line_index_for_offset(&self, offset: usize) -> usize {
        match &self.0 {
            LineIndexStorage::Offsets32(starts) => {
                let offset = u32::try_from(offset)
                    .expect("32-bit line index cannot represent an out-of-range source offset");
                match starts.binary_search(&offset) {
                    Ok(index) => index,
                    Err(0) => 0,
                    Err(index) => index - 1,
                }
            }
            LineIndexStorage::OffsetsWide(starts) => match starts.binary_search(&offset) {
                Ok(index) => index,
                Err(0) => 0,
                Err(index) => index - 1,
            },
            LineIndexStorage::Bitmap(index) => index.line_index_for_offset(offset),
        }
    }

    pub(super) fn estimated_owned_heap_bytes(&self) -> usize {
        let mut weight = RetainedWeight::new(size_of::<Self>());
        match &self.0 {
            LineIndexStorage::Offsets32(starts) => {
                weight.add(LINE_INDEX_BOX_ALLOCATION_OVERHEAD);
                weight.add_array::<u32>(starts.len());
            }
            LineIndexStorage::OffsetsWide(starts) => {
                weight.add(LINE_INDEX_BOX_ALLOCATION_OVERHEAD);
                weight.add_array::<usize>(starts.len());
            }
            LineIndexStorage::Bitmap(index) => {
                weight.add(LINE_INDEX_BOX_ALLOCATION_OVERHEAD.saturating_mul(2));
                weight.add_array::<u64>(index.bits.len());
                weight.add_array::<usize>(index.block_ranks.len());
            }
        }
        weight.finish()
    }

    #[cfg(test)]
    fn representation(&self) -> LineIndexRepresentation {
        match &self.0 {
            LineIndexStorage::Offsets32(_) => LineIndexRepresentation::Offsets32,
            LineIndexStorage::OffsetsWide(_) => LineIndexRepresentation::OffsetsWide,
            LineIndexStorage::Bitmap(_) => LineIndexRepresentation::Bitmap,
        }
    }

    #[cfg(test)]
    fn build_forced(source: &str, representation: LineIndexRepresentation) -> Self {
        if representation == LineIndexRepresentation::Offsets32 {
            assert!(source.len() <= u32::MAX as usize);
        }
        let mut line_count = 0usize;
        scan_line_starts(
            source,
            |_| Ok::<_, std::convert::Infallible>(()),
            |_| line_count += 1,
        )
        .expect("infallible forced line-index count");
        Self::fill_with_checkpoint(source, representation, line_count, |_| {
            Ok::<_, std::convert::Infallible>(())
        })
        .expect("infallible forced line-index fill")
    }
}

impl BitmapLineIndex {
    fn line_start(&self, line_index: usize) -> Option<usize> {
        if line_index >= self.line_count {
            return None;
        }

        let block_index = self
            .block_ranks
            .partition_point(|&rank| rank <= line_index)
            .saturating_sub(1);
        let mut remaining = line_index - self.block_ranks[block_index];
        let first_word = block_index * BITMAP_RANK_WORDS;
        let end_word = (first_word + BITMAP_RANK_WORDS).min(self.bits.len());
        for word_index in first_word..end_word {
            let word = self.bits[word_index];
            let set_bits = word.count_ones() as usize;
            if remaining < set_bits {
                return Some(
                    word_index * BITMAP_WORD_BITS + select_set_bit(word, remaining) as usize,
                );
            }
            remaining -= set_bits;
        }
        unreachable!("bitmap rank metadata must locate every retained line start")
    }

    fn line_index_for_offset(&self, offset: usize) -> usize {
        let word_index = offset / BITMAP_WORD_BITS;
        let block_index = word_index / BITMAP_RANK_WORDS;
        let first_word = block_index * BITMAP_RANK_WORDS;
        let mut rank = self.block_ranks[block_index];
        for word in &self.bits[first_word..word_index] {
            rank += word.count_ones() as usize;
        }
        let bit_index = offset % BITMAP_WORD_BITS;
        let mask = if bit_index == BITMAP_WORD_BITS - 1 {
            u64::MAX
        } else {
            (1u64 << (bit_index + 1)) - 1
        };
        rank += (self.bits[word_index] & mask).count_ones() as usize;
        rank.checked_sub(1)
            .expect("every source line index must retain the initial zero offset")
    }
}

enum LineIndexBuilder {
    Offsets32(Box<[u32]>),
    OffsetsWide(Box<[usize]>),
    Bitmap {
        bits: Box<[u64]>,
        block_ranks: Box<[usize]>,
    },
}

impl LineIndexBuilder {
    fn new(representation: LineIndexRepresentation, source_len: usize, line_count: usize) -> Self {
        match representation {
            LineIndexRepresentation::Offsets32 => {
                Self::Offsets32(vec![0; line_count].into_boxed_slice())
            }
            LineIndexRepresentation::OffsetsWide => {
                Self::OffsetsWide(vec![0; line_count].into_boxed_slice())
            }
            LineIndexRepresentation::Bitmap => {
                let word_count = bitmap_word_count(source_len);
                let block_count = word_count.div_ceil(BITMAP_RANK_WORDS);
                Self::Bitmap {
                    bits: vec![0; word_count].into_boxed_slice(),
                    block_ranks: vec![0; block_count + 1].into_boxed_slice(),
                }
            }
        }
    }

    fn push(&mut self, line_index: usize, start: usize) {
        match self {
            Self::Offsets32(starts) => {
                starts[line_index] = u32::try_from(start)
                    .expect("selected 32-bit line offsets must fit their representation");
            }
            Self::OffsetsWide(starts) => starts[line_index] = start,
            Self::Bitmap { bits, .. } => {
                bits[start / BITMAP_WORD_BITS] |= 1u64 << (start % BITMAP_WORD_BITS);
            }
        }
    }

    fn finish<E>(
        mut self,
        line_count: usize,
        source_len: usize,
        mut checkpoint: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<LineIndex, E> {
        match &mut self {
            Self::Offsets32(_) | Self::OffsetsWide(_) => {}
            Self::Bitmap { bits, block_ranks } => {
                let mut rank = 0usize;
                for block_index in 0..block_ranks.len() - 1 {
                    if block_index
                        % (LINE_INDEX_CHECKPOINT_BYTES / (BITMAP_RANK_WORDS * size_of::<u64>()))
                        == 0
                    {
                        checkpoint(
                            (block_index * BITMAP_RANK_WORDS * size_of::<u64>()).min(source_len),
                        )?;
                    }
                    let first_word = block_index * BITMAP_RANK_WORDS;
                    let end_word = (first_word + BITMAP_RANK_WORDS).min(bits.len());
                    rank += bits[first_word..end_word]
                        .iter()
                        .map(|word| word.count_ones() as usize)
                        .sum::<usize>();
                    block_ranks[block_index + 1] = rank;
                }
                checkpoint(source_len)?;
                assert_eq!(rank, line_count, "bitmap rank metadata lost line starts");
            }
        }

        Ok(LineIndex(match self {
            Self::Offsets32(starts) => LineIndexStorage::Offsets32(starts),
            Self::OffsetsWide(starts) => LineIndexStorage::OffsetsWide(starts),
            Self::Bitmap { bits, block_ranks } => LineIndexStorage::Bitmap(BitmapLineIndex {
                bits,
                block_ranks,
                line_count,
            }),
        }))
    }
}

fn select_line_index_representation(
    source_len: usize,
    line_count: usize,
) -> LineIndexRepresentation {
    let bitmap_bytes = bitmap_index_storage_bytes(source_len);
    let wide_bytes = LINE_INDEX_BOX_ALLOCATION_OVERHEAD
        .saturating_add(line_count.saturating_mul(size_of::<usize>()));

    if source_len <= u32::MAX as usize {
        let offsets32_bytes = LINE_INDEX_BOX_ALLOCATION_OVERHEAD
            .saturating_add(line_count.saturating_mul(size_of::<u32>()));
        if offsets32_bytes <= bitmap_bytes && offsets32_bytes <= wide_bytes {
            return LineIndexRepresentation::Offsets32;
        }
    }

    if wide_bytes <= bitmap_bytes {
        LineIndexRepresentation::OffsetsWide
    } else {
        LineIndexRepresentation::Bitmap
    }
}

fn bitmap_word_count(source_len: usize) -> usize {
    source_len / BITMAP_WORD_BITS + 1
}

fn bitmap_index_storage_bytes(source_len: usize) -> usize {
    let word_count = bitmap_word_count(source_len);
    let block_count = word_count.div_ceil(BITMAP_RANK_WORDS);
    LINE_INDEX_BOX_ALLOCATION_OVERHEAD
        .saturating_mul(2)
        .saturating_add(word_count.saturating_mul(size_of::<u64>()))
        .saturating_add((block_count + 1).saturating_mul(size_of::<usize>()))
}

fn select_set_bit(mut word: u64, mut index: usize) -> u32 {
    loop {
        let bit = word.trailing_zeros();
        if index == 0 {
            return bit;
        }
        word &= word - 1;
        index -= 1;
    }
}

fn scan_line_starts<E>(
    source: &str,
    mut checkpoint: impl FnMut(usize) -> Result<(), E>,
    mut visit: impl FnMut(usize),
) -> Result<(), E> {
    visit(0);
    let bytes = source.as_bytes();
    let mut idx = 0usize;
    let mut next_checkpoint = 0usize;
    while idx < bytes.len() {
        if idx >= next_checkpoint {
            checkpoint(idx)?;
            next_checkpoint = idx.saturating_add(4096);
        }
        match bytes[idx] {
            b'\r' => {
                idx += 1;
                if bytes.get(idx) == Some(&b'\n') {
                    idx += 1;
                }
                visit(idx);
            }
            b'\n' => {
                idx += 1;
                visit(idx);
            }
            _ => {
                idx += 1;
            }
        }
    }
    checkpoint(idx)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oracle_line_starts(source: &str) -> Vec<usize> {
        let mut starts = vec![0];
        let mut chars = source.char_indices().peekable();
        while let Some((offset, ch)) = chars.next() {
            match ch {
                '\r' => {
                    let mut next_start = offset + 1;
                    if chars.peek().is_some_and(|(_, ch)| *ch == '\n') {
                        let (line_feed, _) = chars.next().expect("peeked line feed");
                        next_start = line_feed + 1;
                    }
                    starts.push(next_start);
                }
                '\n' => starts.push(offset + 1),
                _ => {}
            }
        }
        starts
    }

    #[test]
    fn every_representation_matches_the_mixed_newline_oracle() {
        let source = "α\r\n🤓\rb\n\n终";
        let expected = oracle_line_starts(source);

        for representation in [
            LineIndexRepresentation::Offsets32,
            LineIndexRepresentation::OffsetsWide,
            LineIndexRepresentation::Bitmap,
        ] {
            let index = LineIndex::build_forced(source, representation);
            assert_eq!(index.representation(), representation);
            assert_eq!(index.line_count(), expected.len());
            assert_eq!(
                (0..index.line_count())
                    .map(|line| index.line_start(line).unwrap())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(index.line_start(index.line_count()), None);

            for offset in 0..=source.len() {
                if !source.is_char_boundary(offset) {
                    continue;
                }
                let expected_line = expected.partition_point(|&start| start <= offset) - 1;
                assert_eq!(
                    index.line_index_for_offset(offset),
                    expected_line,
                    "{representation:?} diverged at byte {offset}"
                );
            }
        }
    }

    #[test]
    fn bitmap_rank_select_handles_crlf_and_line_starts_at_block_boundaries() {
        let source = format!("{}\r\n{}\n", "a".repeat(510), "b".repeat(511));
        let index = LineIndex::build_forced(&source, LineIndexRepresentation::Bitmap);

        assert_eq!(source.len(), 1024);
        assert_eq!(index.line_count(), 3);
        assert_eq!(index.line_start(0), Some(0));
        assert_eq!(index.line_start(1), Some(512));
        assert_eq!(index.line_start(2), Some(1024));
        assert_eq!(index.line_index_for_offset(511), 0);
        assert_eq!(index.line_index_for_offset(512), 1);
        assert_eq!(index.line_index_for_offset(1023), 1);
        assert_eq!(index.line_index_for_offset(1024), 2);
    }

    #[test]
    fn representation_selection_is_size_based_and_has_stable_ties() {
        assert_eq!(
            select_line_index_representation(1024 * 1024, 1),
            LineIndexRepresentation::Offsets32
        );
        assert_eq!(
            select_line_index_representation(1024 * 1024, 1024 * 1024 + 1),
            LineIndexRepresentation::Bitmap
        );

        let source_len = 4096;
        let bitmap_bytes = bitmap_index_storage_bytes(source_len);
        let largest_offsets32 =
            (bitmap_bytes - LINE_INDEX_BOX_ALLOCATION_OVERHEAD) / size_of::<u32>();
        assert_eq!(
            select_line_index_representation(source_len, largest_offsets32),
            LineIndexRepresentation::Offsets32
        );
        assert_eq!(
            select_line_index_representation(source_len, largest_offsets32 + 1),
            LineIndexRepresentation::Bitmap
        );

        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            select_line_index_representation(u32::MAX as usize + 1, 2),
            LineIndexRepresentation::OffsetsWide
        );
    }

    #[test]
    fn forced_representations_allocate_only_their_exact_logical_storage() {
        let source = "a\nb\r\nc\rd";
        let line_count = oracle_line_starts(source).len();

        match LineIndex::build_forced(source, LineIndexRepresentation::Offsets32).0 {
            LineIndexStorage::Offsets32(starts) => assert_eq!(starts.len(), line_count),
            other => panic!("unexpected forced representation: {other:?}"),
        }
        match LineIndex::build_forced(source, LineIndexRepresentation::OffsetsWide).0 {
            LineIndexStorage::OffsetsWide(starts) => assert_eq!(starts.len(), line_count),
            other => panic!("unexpected forced representation: {other:?}"),
        }
        match LineIndex::build_forced(source, LineIndexRepresentation::Bitmap).0 {
            LineIndexStorage::Bitmap(index) => {
                let words = bitmap_word_count(source.len());
                assert_eq!(index.bits.len(), words);
                assert_eq!(
                    index.block_ranks.len(),
                    words.div_ceil(BITMAP_RANK_WORDS) + 1
                );
            }
            other => panic!("unexpected forced representation: {other:?}"),
        }
    }

    #[test]
    fn retained_weight_covers_metadata_and_every_box_allocation() {
        let source = "a\nb\r\nc\rd";
        let line_count = oracle_line_starts(source).len();

        let offsets32 = LineIndex::build_forced(source, LineIndexRepresentation::Offsets32);
        assert_eq!(
            offsets32.estimated_owned_heap_bytes(),
            size_of::<LineIndex>()
                + LINE_INDEX_BOX_ALLOCATION_OVERHEAD
                + line_count * size_of::<u32>()
        );

        let offsets_wide = LineIndex::build_forced(source, LineIndexRepresentation::OffsetsWide);
        assert_eq!(
            offsets_wide.estimated_owned_heap_bytes(),
            size_of::<LineIndex>()
                + LINE_INDEX_BOX_ALLOCATION_OVERHEAD
                + line_count * size_of::<usize>()
        );

        let bitmap = LineIndex::build_forced(source, LineIndexRepresentation::Bitmap);
        let word_count = bitmap_word_count(source.len());
        let rank_count = word_count.div_ceil(BITMAP_RANK_WORDS) + 1;
        assert_eq!(
            bitmap.estimated_owned_heap_bytes(),
            size_of::<LineIndex>()
                + 2 * LINE_INDEX_BOX_ALLOCATION_OVERHEAD
                + word_count * size_of::<u64>()
                + rank_count * size_of::<usize>()
        );
    }

    #[test]
    fn dense_four_megabyte_newline_indexes_stay_below_one_megabyte() {
        fn assert_compact(source: String) {
            assert_eq!(source.len(), 4 * 1024 * 1024);
            let index = LineIndex::build(&source);
            assert_eq!(index.representation(), LineIndexRepresentation::Bitmap);
            assert!(index.estimated_owned_heap_bytes() < 1024 * 1024);
        }

        assert_compact("\n".repeat(4 * 1024 * 1024));
        assert_compact("\r".repeat(4 * 1024 * 1024));
        assert_compact("\r\n".repeat(2 * 1024 * 1024));
    }

    #[test]
    fn cancellation_in_either_build_pass_never_returns_a_partial_index() {
        let source = "a\n".repeat(8 * 1024);
        for (cancelled_pass, cancelled_offset) in [
            (LineIndexBuildPass::Count, LINE_INDEX_CHECKPOINT_BYTES),
            (LineIndexBuildPass::Fill, 0),
            (LineIndexBuildPass::Fill, LINE_INDEX_CHECKPOINT_BYTES),
        ] {
            let result = LineIndex::build_with_checkpoint(&source, |pass, offset| {
                if pass == cancelled_pass && offset >= cancelled_offset {
                    return Err((pass, offset));
                }
                Ok(())
            });
            assert!(matches!(result, Err((pass, _)) if pass == cancelled_pass));
        }
    }
}
