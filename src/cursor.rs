use crate::{FmIndex, HalfOpenInterval, Hit, IndexStorage, text_with_rank_support::Block};
#[derive(Clone, Copy)]
pub struct Cursor<'a, I, B> {
    pub(crate) index: &'a FmIndex<I, B>,
    pub(crate) interval: HalfOpenInterval,
}

impl<'a, I: IndexStorage, B: Block> Cursor<'a, I, B> {
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

    pub fn is_empty(&self) -> bool {
        self.interval.start == self.interval.end
    }

    pub fn count(&self) -> usize {
        self.interval.end - self.interval.start
    }

    pub fn locate(&self) -> impl Iterator<Item = Hit> {
        self.index.locate_interval(self.interval)
    }
}
