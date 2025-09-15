use crate::{FmIndex, HalfOpenInterval, Hit, IndexStorage, text_with_rank_support::Block};
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Cursor<'a, C, I, B> {
    index: &'a FmIndex<I, B>,
    interval: HalfOpenInterval,
    _marker: PhantomData<C>,
}

impl<'a, C: CursorState, I: IndexStorage, B: Block> Cursor<'a, C, I, B> {
    pub fn extend_front(self, symbol: u8) -> Cursor<'a, AfterSteps, I, B> {
        let symbol = self.index.alphabet.io_to_dense_representation(symbol);
        debug_assert!(symbol != 255 && symbol != 0);

        self.extend_front_without_alphabet_translation(symbol)
    }

    pub(crate) fn extend_front_without_alphabet_translation(
        self,
        symbol: u8,
    ) -> Cursor<'a, AfterSteps, I, B> {
        let (start, end) = if self.interval.start != self.interval.end {
            (
                self.index.lf_mapping_step(symbol, self.interval.start),
                self.index.lf_mapping_step(symbol, self.interval.end),
            )
        } else {
            (self.interval.start, self.interval.end)
        };

        Cursor {
            index: self.index,
            interval: HalfOpenInterval { start, end },
            _marker: PhantomData,
        }
    }

    // returns half open interval [start, end)
    pub fn interval(&self) -> HalfOpenInterval {
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

impl<'a, I: IndexStorage, B: Block> Cursor<'a, Init, I, B> {
    pub(crate) fn new(index: &'a FmIndex<I, B>, text_len: usize) -> Self {
        Self {
            index,
            interval: HalfOpenInterval {
                start: 0,
                end: text_len,
            },
            _marker: PhantomData,
        }
    }

    pub fn search(self, query: &[u8]) -> Cursor<'a, AfterSteps, I, B> {
        let query_iter = query
            .iter()
            .rev()
            .map(|&s| self.index.alphabet.io_to_dense_representation(s));

        self.search_iter_without_alphabet_translation(query_iter)
    }

    pub(crate) fn search_iter_without_alphabet_translation(
        self,
        query: impl IntoIterator<Item = u8> + ExactSizeIterator + Clone,
    ) -> Cursor<'a, AfterSteps, I, B> {
        let lookup_depth = std::cmp::min(query.len(), self.index.lookup_tables.max_depth());
        let (start, end) = self
            .index
            .lookup_tables
            .lookup(query.clone().into_iter(), lookup_depth);

        let mut cursor = Cursor {
            index: self.index,
            interval: HalfOpenInterval { start, end },
            _marker: PhantomData,
        };

        for symbol in query.into_iter().skip(lookup_depth) {
            cursor = cursor.extend_front_without_alphabet_translation(symbol);

            if cursor.is_empty() {
                break;
            }
        }

        cursor
    }
}

pub(crate) use typestate::*;

mod typestate {
    pub trait CursorState {}

    pub struct Init {}

    impl CursorState for Init {}

    pub struct AfterSteps {}

    impl CursorState for AfterSteps {}
}
