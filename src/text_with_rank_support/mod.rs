use crate::{IndexStorage, maybe_savefile, sealed};

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
pub trait TextWithRankSupport<I: IndexStorage>:
    sealed::Sealed + maybe_savefile::MaybeSavefile + 'static
{
    /// Construct the data structure for the given text.
    ///
    /// All symbols are assumed to be smaller than `alphabet_size`. In the terminology of this library,
    /// they are in dense representation. The running time of this operation is linear in the text length.
    fn construct(text: &[u8], alphabet_size: usize) -> Self;

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
