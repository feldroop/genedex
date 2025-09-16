use crate::{FmIndex, HalfOpenInterval, Hit, IndexStorage, text_with_rank_support::block::Block};

/// A cursor to the FM-Index.
///
/// The cursor API allows more flexible search procedures using the FM-Index. The cursor implicitly
/// maintains a currently searched query. Symbols can iteratively be added to the front of this query.
///
/// At any point, the number of occurrences of the currently searched query can be retrieved cheaply, and occurrences
/// can be located. Repeteadly calling [`extend_query_front`](Cursor::extend_query_front) corresponds to a typical
/// backwards search.
///
/// An example of using the cursor API can be found
/// [here](https://github.com/feldroop/genedex/blob/master/examples/cursor.rs).
#[derive(Clone, Copy)]
pub struct Cursor<'a, I, B> {
    pub(crate) index: &'a FmIndex<I, B>,
    pub(crate) interval: HalfOpenInterval,
}

impl<'a, I: IndexStorage, B: Block> Cursor<'a, I, B> {
    /// Extends the currently searched query at the front by one symbol.
    ///
    /// The running time is in O(1).
    pub fn extend_query_front(&mut self, symbol: u8) {
        let symbol = self.index.alphabet.io_to_dense_representation(symbol);

        // SAFETY: symbol was just checked
        unsafe { self.extend_front_without_alphabet_translation(symbol) };
    }

    // SAFETY precondition: symbols must be valid  in dense representation for th alphabet
    pub(crate) unsafe fn extend_front_without_alphabet_translation(&mut self, symbol: u8) {
        // SAFETY: the cursor always maintains a valid intervals of the text, and the symbol is valid in dense representation
        let (start, end) = if self.interval.start != self.interval.end {
            unsafe {
                (
                    self.index
                        .lf_mapping_step_unchecked(symbol, self.interval.start),
                    self.index
                        .lf_mapping_step_unchecked(symbol, self.interval.end),
                )
            }
        } else {
            (self.interval.start, self.interval.end)
        };

        self.interval = HalfOpenInterval { start, end };
    }

    // returns half open interval [start, end)
    pub(crate) fn interval(&self) -> HalfOpenInterval {
        self.interval
    }

    /// Returns the number of occurrences of the currently searched query in the set of indexed texts.
    ///
    /// The running time is in O(1).
    pub fn count(&self) -> usize {
        self.interval.end - self.interval.start
    }

    /// Returns the number of occurrences of the currently searched query in the set of indexed texts.
    ///
    /// The initial running time is in O(1).
    /// For each hit pulled from the iterator, a sampled suffix array lookup is performed.
    /// This operation needs `s / 2` steps on average, where `s` is the suffix array
    /// sampling rate of the index.
    pub fn locate(&self) -> impl Iterator<Item = Hit> {
        // SAFETY: the interval of the cursor is always valid for the text
        unsafe { self.index.locate_interval(self.interval) }
    }
}
