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

pub trait TextWithRankSupport<I: IndexStorage>:
    sealed::Sealed + maybe_savefile::MaybeSavefile
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
