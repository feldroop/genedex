use crate::{
    IndexStorage,
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

pub(crate) trait PrivateTextWithRankSupport<I: IndexStorage>: Sealed {
    fn construct_from_maybe_slice_compressed_text<S: SliceCompression>(
        text: &[u8],
        uncompressed_text_len: usize,
        alphabet_size: usize,
    ) -> Self;
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
        assert!((symbol as usize) < self.alphabet_size() && idx <= self.text_len());
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

    fn text_len(&self) -> usize;

    fn alphabet_size(&self) -> usize;
}

#[cfg(test)]
mod tests {
    use crate::{
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

    proptest! {
        // default is 256 and I'd like some more test cases that need to pass
        #![proptest_config(ProptestConfig::with_cases(2048))]

        #[test]
        fn correctness_random_texts(text in even_len_text()) {
            let mut text_copy = text.clone();
            half_byte_compress_text(&mut text_copy);
            let compressed = &text_copy[..text.len() / 2];

            test_with_and_without_half_byte_compression::<FlatTextWithRankSupport<u32>>(&text, compressed);
            test_with_and_without_half_byte_compression::<CondensedTextWithRankSupport<u32>>(&text, compressed);
        }
    }
}
