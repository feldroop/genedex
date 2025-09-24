use crate::{
    IndexStorage,
    batch_computed_cursors::Buffers,
    construction::slice_compression::{NoSliceCompression, SliceCompression},
    maybe_savefile,
    sealed::Sealed,
};

/// The FM-Index and text with rank support data structures can be used with two different block configurations.
mod block;
mod condensed;
mod flat;

#[doc(inline)]
pub use block::{Block, Block64, Block512};

#[doc(inline)]
pub use condensed::CondensedTextWithRankSupport;

#[doc(inline)]
pub use flat::FlatTextWithRankSupport;

// these specific optimization are not something I want to expose to the public API, for now
pub(crate) trait PrivateTextWithRankSupport<I: IndexStorage>: Sealed {
    fn construct_from_maybe_slice_compressed_text<S: SliceCompression>(
        text: &[u8],
        uncompressed_text_len: usize,
        alphabet_size: usize,
    ) -> Self;

    fn _alphabet_size(&self) -> usize;

    fn _text_len(&self) -> usize;

    fn replace_many_interval_borders_with_ranks<const N: usize>(
        &self,
        buffers: &mut Buffers<N>,
        num_remaining_unfinished_queries: usize,
    ) {
        let all_symbols_valid = buffers
            .symbols
            .iter()
            .all(|&s| (s as usize) < self._alphabet_size());

        let all_idx_valid = buffers
            .intervals
            .iter()
            .all(|ivl| ivl.start <= self._text_len() && ivl.end <= self._text_len());

        let is_safe = all_symbols_valid && all_idx_valid;
        assert!(is_safe);

        unsafe {
            self.replace_many_interval_borders_with_ranks_unchecked(
                buffers,
                num_remaining_unfinished_queries,
            );
        }
    }

    // SAFETY: the symbols must be smaller than the alphabet size and the indices must not be greater than the text len
    unsafe fn replace_many_interval_borders_with_ranks_unchecked<const N: usize>(
        &self,
        buffers: &mut Buffers<N>,
        num_remaining_unfinished_queries: usize,
    );
}

/// A trait for data structures central to the FM-Index of this library.
///
/// They can answer rank queries similar to the ones for bitvectors with rank support,
/// but for a text with a given number of different symbols.
///
/// Currently, two different implementations exist, [`CondensedTextWithRankSupport`] and
/// [`FlatTextWithRankSupport`]. Both of them can also be used with different block sizes (more info [here](Block)).
///
/// The condensed version is more space efficient, which is especially relevant for larger alphabets.
/// The flat version is a bit faster, but has a higher memory usage.
///
/// In total, [`FlatTextWithRankSupport<Block64>`] is the fastest, and [`CondensedTextWithRankSupport<Block512>`]
/// is the smallest configuration.
///
/// An example of how this data structure is used can be found
/// [here](https://github.com/feldroop/genedex/blob/master/examples/text_with_rank_support.rs).
// I don't want to make the slice compression API public
#[allow(private_bounds)]
pub trait TextWithRankSupport<I: IndexStorage>:
    maybe_savefile::MaybeSavefile + PrivateTextWithRankSupport<I> + 'static
{
    /// Construct the data structure for the given text.
    ///
    /// All symbols are assumed to be smaller than `alphabet_size`. In the terminology of this library,
    /// they are in dense representation. The running time of this operation is linear in the text length.
    fn construct(text: &[u8], alphabet_size: usize) -> Self {
        Self::construct_from_maybe_slice_compressed_text::<NoSliceCompression>(
            text,
            text.len(),
            alphabet_size,
        )
    }

    /// Returns the number of occurrences of `symbol` in `text[0..idx]`.
    ///
    /// The running time is in O(1).
    fn rank(&self, symbol: u8, idx: usize) -> usize {
        let is_safe = (symbol as usize) < self.alphabet_size() && idx <= self.text_len();
        assert!(is_safe);
        unsafe { self.rank_unchecked(symbol, idx) }
    }

    // TODO rank_two

    /// Version of [`rank`](Self::rank) without bounds checks.
    ///
    /// The running time is in O(1).
    ///
    /// # Safety
    ///
    /// `idx` must be in the interval `[0, text.len()]` and `symbol` must be smaller than alphabet size.
    unsafe fn rank_unchecked(&self, symbol: u8, idx: usize) -> usize;

    /// Recoveres the symbol of the text at given index `idx`.
    ///
    /// The running time is in O(1).
    fn symbol_at(&self, idx: usize) -> u8;

    fn text_len(&self) -> usize {
        self._text_len()
    }

    fn alphabet_size(&self) -> usize {
        self._alphabet_size()
    }
}

// test functions from PrivateTextWithRankSupport here
#[cfg(test)]
mod tests {
    use crate::{
        HalfOpenInterval,
        batch_computed_cursors::Buffers,
        construction::slice_compression::{
            HalfBytesCompression, NoSliceCompression, half_byte_compress_text,
        },
        text_with_rank_support::{
            CondensedTextWithRankSupport, FlatTextWithRankSupport, TextWithRankSupport,
        },
    };
    use proptest::prelude::*;

    prop_compose! {
        fn even_len_text()
            (text_len in (0usize..500).prop_map(|len| len * 2))
            (text in prop::collection::vec(0u8..16, text_len)) -> Vec<u8> {
                text
        }
    }

    prop_compose! {
        fn text_and_alphabet_size()
            (max_char in 2u8..32)
            (text in prop::collection::vec(0u8..=max_char, 0usize..1000), max_char in Just(max_char)) -> (Vec<u8>, usize) {
                (text, max_char as usize + 1)
        }
    }

    fn test_with_and_without_half_byte_compression<R: TextWithRankSupport<u32>>(
        text: &[u8],
        half_byte_compressed_text: &[u8],
    ) {
        let ranks = R::construct_from_maybe_slice_compressed_text::<NoSliceCompression>(
            text,
            text.len(),
            16,
        );
        let ranks_compressed = R::construct_from_maybe_slice_compressed_text::<HalfBytesCompression>(
            half_byte_compressed_text,
            text.len(),
            16,
        );

        for symbol in 0..16 {
            for idx in 0..=text.len() {
                assert_eq!(
                    ranks.rank(symbol, idx),
                    ranks_compressed.rank(symbol, idx),
                    "symbol: {}, idx: {}",
                    symbol,
                    idx
                );
            }
        }
    }

    fn test_replace_many_intervals_same_as_rank<R: TextWithRankSupport<u32>>(
        text: &[u8],
        alphabet_size: usize,
    ) {
        let ranks = R::construct(text, alphabet_size);

        for _ in 0..20 {
            const N: usize = 8;

            // in the FM-Index, interval start must always be <= interval end, but here that doesn'matter
            let intervals: [HalfOpenInterval; N] = core::array::from_fn(|_| HalfOpenInterval {
                start: rand::random_range(0..=text.len()),
                end: rand::random_range(0..=text.len()),
            });
            let symbols: [u8; N] =
                core::array::from_fn(|_| rand::random_range(0..=(alphabet_size as u8 - 1)));

            let mut expected_intervals = intervals;
            for (interval, symbol) in expected_intervals.iter_mut().zip(symbols) {
                interval.start = ranks.rank(symbol, interval.start);
                interval.end = ranks.rank(symbol, interval.end);
            }

            let mut buffers = Buffers::new();
            buffers.intervals = intervals;
            buffers.symbols = symbols;

            ranks.replace_many_interval_borders_with_ranks(&mut buffers, N);

            assert_eq!(expected_intervals, buffers.intervals);
        }
    }

    proptest! {
        // default is 256 and I'd like some more test cases that need to pass
        #![proptest_config(ProptestConfig::with_cases(2048))]

        #[test]
        fn half_byte_compressed_construction(text in even_len_text()) {
            let mut text_copy = text.clone();
            half_byte_compress_text(&mut text_copy);
            let compressed = &text_copy[..text.len() / 2];

            test_with_and_without_half_byte_compression::<FlatTextWithRankSupport<u32>>(&text, compressed);
            test_with_and_without_half_byte_compression::<CondensedTextWithRankSupport<u32>>(&text, compressed);
        }

        #[test]
        fn replace_many_intervals_same_as_rank((text, alphabet_size) in text_and_alphabet_size()) {
            test_replace_many_intervals_same_as_rank::<FlatTextWithRankSupport<u32>>(&text, alphabet_size);
            test_replace_many_intervals_same_as_rank::<CondensedTextWithRankSupport<u32>>(&text, alphabet_size);
        }
    }
}
