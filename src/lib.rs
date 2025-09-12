pub mod alphabet;
pub mod text_with_rank_support;

mod construction;
mod lookup_table;
mod sampled_suffix_array;
mod text_id_search_tree;

use std::marker::PhantomData;

use bytemuck::Pod;
use libsais::OutputElement;
use num_traits::{NumCast, PrimInt};

#[doc(inline)]
pub use alphabet::Alphabet;
#[doc(inline)]
pub use text_with_rank_support::TextWithRankSupport;

pub use libsais::OutputElement as LibsaisOutputElement;

use construction::DataStructures;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;
use text_with_rank_support::{Block, Block512};

use crate::lookup_table::LookupTables;

#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
pub struct FmIndex<A, I, B = Block512> {
    count: Vec<usize>,
    text_with_rank_support: TextWithRankSupport<I, B>,
    suffix_array: SampledSuffixArray<I>,
    text_ids: TexdIdSearchTree,
    lookup_tables: LookupTables<I>,
    _alphabet_marker: PhantomData<A>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hit {
    pub text_id: usize,
    pub position: usize,
}

pub type FmIndexI32<A, B = Block512> = FmIndex<A, i32, B>;
pub type FmIndexU32<A, B = Block512> = FmIndex<A, u32, B>;
pub type FmIndexI64<A, B = Block512> = FmIndex<A, i64, B>;

impl<A: Alphabet, I: OutputElement + IndexStorage, B: Block> FmIndex<A, I, B> {
    // text chars must be smaller than alphabet size and greater than 0
    // other operations use rayons configured number of threads
    pub fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
        lookup_table_depth: usize,
    ) -> Self {
        let DataStructures {
            count,
            suffix_array_bytes,
            bwt,
            text_ids,
            text_border_lookup,
        } = construction::create_data_structures::<A, I, T>(
            texts,
            suffix_array_construction_thread_count,
        );

        let sampled_suffix_array = SampledSuffixArray::new_uncompressed(
            suffix_array_bytes,
            suffix_array_sampling_rate,
            text_border_lookup,
        );

        let text_with_rank_support = TextWithRankSupport::construct(&bwt, A::SIZE);

        let mut index = FmIndex {
            count,
            text_with_rank_support,
            suffix_array: sampled_suffix_array,
            text_ids,
            lookup_tables: LookupTables::new_empty(),
            _alphabet_marker: PhantomData,
        };

        lookup_table::fill_lookup_tables(
            &mut index,
            lookup_table_depth,
            A::SIZE - A::NUM_SYMBOL_NOT_SEARCHED - 1,
        );

        index
    }
}

impl<A: Alphabet, B: Block> FmIndex<A, u32, B> {
    // text chars must be smaller than alphabet size + 1 and greater than 0
    // other operations use rayons configured number of threads
    pub fn new_u32_compressed<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
        lookup_table_depth: usize,
    ) -> Self {
        let DataStructures {
            count,
            suffix_array_bytes,
            bwt,
            text_ids,
            text_border_lookup,
        } = construction::create_data_structures::<A, i64, T>(
            texts,
            suffix_array_construction_thread_count,
        );

        assert!(bwt.len() <= u32::MAX as usize);

        let text_border_lookup = text_border_lookup
            .into_iter()
            .map(|(k, v)| (k, v as u32))
            .collect();

        let sampled_suffix_array = SampledSuffixArray::new_u32_compressed(
            suffix_array_bytes,
            suffix_array_sampling_rate,
            text_border_lookup,
        );

        let text_with_rank_support = TextWithRankSupport::construct(&bwt, A::SIZE);

        let mut index = FmIndex {
            count,
            text_with_rank_support,
            suffix_array: sampled_suffix_array,
            text_ids,
            lookup_tables: LookupTables::new_empty(),
            _alphabet_marker: PhantomData,
        };

        lookup_table::fill_lookup_tables(
            &mut index,
            lookup_table_depth,
            A::SIZE - A::NUM_SYMBOL_NOT_SEARCHED - 1,
        );

        index
    }
}

impl<A: Alphabet, I: IndexStorage, B: Block> FmIndex<A, I, B> {
    pub fn count(&self, query: &[u8]) -> usize {
        self.cursor().extend_back_to_front(query).count()
    }

    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = Hit> {
        let cursor = self.cursor().extend_back_to_front(query);

        self.locate_interval(cursor.interval)
    }

    fn locate_interval(&self, interval: Interval) -> impl Iterator<Item = Hit> {
        self.suffix_array
            .recover_range(interval.start..interval.end, self)
            .map(|idx| {
                let (text_id, position) = self
                    .text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap());

                Hit { text_id, position }
            })
    }

    pub fn cursor<'a>(&'a self) -> Cursor<'a, Init, A, I, B> {
        Cursor {
            index: self,
            interval: Interval {
                start: 0,
                end: self.text_with_rank_support.text_len(),
            },
            _marker: PhantomData,
        }
    }

    fn lf_mapping_step(&self, symbol: u8, idx: usize) -> usize {
        self.count[symbol as usize] + self.text_with_rank_support.rank(symbol, idx)
    }
}

#[cfg(feature = "savefile")]
impl<A: Alphabet, I: IndexStorage, B: Block> FmIndex<A, I, B> {
    const VERSION_FOR_SAVEFILE: u32 = 0;

    pub fn load_from_reader(
        reader: &mut impl std::io::Read,
    ) -> Result<Self, savefile::SavefileError> {
        savefile::load(reader, Self::VERSION_FOR_SAVEFILE)
    }

    pub fn load_from_file(
        filepath: impl AsRef<std::path::Path>,
    ) -> Result<Self, savefile::SavefileError> {
        savefile::load_file(filepath, Self::VERSION_FOR_SAVEFILE)
    }

    pub fn save_to_writer(
        &self,
        writer: &mut impl std::io::Write,
    ) -> Result<(), savefile::SavefileError> {
        savefile::save(writer, Self::VERSION_FOR_SAVEFILE, self)
    }

    pub fn save_to_file(
        &self,
        filepath: impl AsRef<std::path::Path>,
    ) -> Result<(), savefile::SavefileError> {
        savefile::save_file(filepath, Self::VERSION_FOR_SAVEFILE, self)
    }
}

#[derive(Clone)]
pub struct Cursor<'a, C, A, I, B> {
    index: &'a FmIndex<A, I, B>,
    interval: Interval,
    _marker: PhantomData<C>,
}

impl<'a, C: CursorState, A: Alphabet, I: IndexStorage, B: Block> Cursor<'a, C, A, I, B> {
    pub fn extend_front(self, symbol: u8) -> Cursor<'a, StepsDone, A, I, B> {
        let symbol = A::DENSE_ENCODING_TRANSLATION_TABLE[symbol as usize];
        debug_assert!(symbol != 255 && symbol != 0);

        self.extend_front_without_alphabet_translation(symbol)
    }

    fn extend_front_without_alphabet_translation(
        self,
        symbol: u8,
    ) -> Cursor<'a, StepsDone, A, I, B> {
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
            interval: Interval { start, end },
            _marker: PhantomData,
        }
    }

    // returns half open interval [start, end)
    pub fn interval(&self) -> Interval {
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

impl<'a, A: Alphabet, I: IndexStorage, B: Block> Cursor<'a, Init, A, I, B> {
    pub fn extend_back_to_front(self, query: &[u8]) -> Cursor<'a, StepsDone, A, I, B> {
        let query_iter = query.iter().rev().map(|&s| {
            let symbol = A::DENSE_ENCODING_TRANSLATION_TABLE[s as usize];
            debug_assert!(symbol != 255 && symbol != 0);
            symbol
        });

        self.extend_iter_without_alphabet_translation(query_iter)
    }

    fn extend_iter_without_alphabet_translation(
        self,
        query: impl IntoIterator<Item = u8> + ExactSizeIterator + Clone,
    ) -> Cursor<'a, StepsDone, A, I, B> {
        let lookup_depth = std::cmp::min(query.len(), self.index.lookup_tables.max_depth());
        let (start, end) = self
            .index
            .lookup_tables
            .lookup(query.clone().into_iter(), lookup_depth);

        let mut cursor = Cursor {
            index: self.index,
            interval: Interval { start, end },
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

pub trait IndexStorage: PrimInt + Pod + maybe_savefile::MaybeSavefile + 'static {}

impl IndexStorage for i32 {}
impl IndexStorage for u32 {}
impl IndexStorage for i64 {}

mod maybe_savefile {
    #[cfg(feature = "savefile")]
    pub trait MaybeSavefile: savefile::Savefile {}

    #[cfg(not(feature = "savefile"))]
    pub trait MaybeSavefile {}

    impl MaybeSavefile for i32 {}
    impl MaybeSavefile for u32 {}
    impl MaybeSavefile for i64 {}
}

#[derive(Debug, Clone, Copy)]
pub struct Interval {
    start: usize,
    end: usize,
}

use typestate::*;

mod typestate {
    pub trait CursorState {}

    pub struct Init {}

    impl CursorState for Init {}

    pub struct StepsDone {}

    impl CursorState for StepsDone {}
}
