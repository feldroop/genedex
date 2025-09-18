use crate::{
    FmIndex, HalfOpenInterval, Hit, IndexStorage, text_with_rank_support::TextWithRankSupport,
};

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
pub struct Cursor<'a, I, R> {
    pub(crate) index: &'a FmIndex<I, R>,
    pub(crate) interval: HalfOpenInterval,
}

impl<'a, I: IndexStorage, R: TextWithRankSupport<I>> Cursor<'a, I, R> {
    /// Extends the currently searched query at the front by one symbol.
    ///
    /// The running time is in O(1).
    pub fn extend_query_front(&mut self, symbol: u8) {
        let symbol = self.index.alphabet.io_to_dense_representation(symbol);

        self.extend_front_without_alphabet_translation(symbol);
    }

    pub(crate) fn extend_front_without_alphabet_translation(&mut self, symbol: u8) {
        let (start, end) = if self.interval.start != self.interval.end {
            (
                self.index.lf_mapping_step(symbol, self.interval.start),
                self.index.lf_mapping_step(symbol, self.interval.end),
            )
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
        self.index.locate_interval(self.interval)
    }
}
