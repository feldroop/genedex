/// Contains functions to create various commonly used alphabets.
pub mod alphabet;
pub mod config;
pub mod cursor;
pub mod text_with_rank_support;

mod construction;
mod lookup_table;
mod sampled_suffix_array;
mod text_id_search_tree;

use bytemuck::Pod;
use libsais::OutputElement;
use num_traits::{NumCast, PrimInt};

#[doc(inline)]
pub use alphabet::Alphabet;
#[doc(inline)]
pub use config::FmIndexConfig;
#[doc(inline)]
pub use cursor::Cursor;
#[doc(inline)]
pub use text_with_rank_support::TextWithRankSupport;

use construction::DataStructures;
use lookup_table::LookupTables;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;
use text_with_rank_support::{Block, Block64};

#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
pub struct FmIndex<I, B = Block64> {
    alphabet: Alphabet,
    count: Vec<usize>,
    text_with_rank_support: TextWithRankSupport<I, B>,
    suffix_array: SampledSuffixArray<I>,
    text_ids: TexdIdSearchTree,
    lookup_tables: LookupTables<I>,
}

impl<I: IndexStorage, B: Block> FmIndex<I, B> {
    // text chars must be smaller than alphabet size and greater than 0
    // other operations use rayons configured number of threads
    fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        alphabet: Alphabet,
        config: FmIndexConfig<I, B>,
    ) -> Self {
        let DataStructures {
            count,
            sampled_suffix_array,
            text_ids,
            text_with_rank_support,
        } = construction::create_data_structures::<I, B, T>(texts, config, &alphabet);

        let num_searchable_symbols = alphabet.num_searchable_symbols();

        let mut index = FmIndex {
            alphabet,
            count,
            text_with_rank_support,
            suffix_array: sampled_suffix_array,
            text_ids,
            lookup_tables: LookupTables::new_empty(),
        };

        lookup_table::fill_lookup_tables(
            &mut index,
            config.lookup_table_depth,
            num_searchable_symbols,
        );

        index
    }

    pub fn count(&self, query: &[u8]) -> usize {
        self.cursor_for_query(query).count()
    }

    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = Hit> {
        let cursor = self.cursor_for_query(query);

        self.locate_interval(cursor.interval())
    }

    fn locate_interval(&self, interval: HalfOpenInterval) -> impl Iterator<Item = Hit> {
        self.suffix_array
            .recover_range(interval.start..interval.end, self)
            .map(|idx| {
                let (text_id, position) = self
                    .text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap());

                Hit { text_id, position }
            })
    }

    pub fn cursor_empty<'a>(&'a self) -> Cursor<'a, I, B> {
        Cursor {
            index: self,
            interval: HalfOpenInterval {
                start: 0,
                end: self.text_with_rank_support.text_len(),
            },
        }
    }

    pub fn cursor_for_query<'a>(&'a self, query: &[u8]) -> Cursor<'a, I, B> {
        let query_iter = query
            .iter()
            .rev()
            .map(|&s| self.alphabet.io_to_dense_representation(s));

        self.cursor_for_iter_without_alphabet_translation(query_iter)
    }

    fn cursor_for_iter_without_alphabet_translation<'a>(
        &'a self,
        query: impl IntoIterator<Item = u8> + ExactSizeIterator + Clone,
    ) -> Cursor<'a, I, B> {
        let lookup_depth = std::cmp::min(query.len(), self.lookup_tables.max_depth());
        let (start, end) = self
            .lookup_tables
            .lookup(query.clone().into_iter(), lookup_depth);

        let mut cursor = Cursor {
            index: self,
            interval: HalfOpenInterval { start, end },
        };

        for symbol in query.into_iter().skip(lookup_depth) {
            cursor.extend_front_without_alphabet_translation(symbol);

            if cursor.is_empty() {
                break;
            }
        }

        cursor
    }

    fn lf_mapping_step(&self, symbol: u8, idx: usize) -> usize {
        self.count[symbol as usize] + self.text_with_rank_support.rank(symbol, idx)
    }
}

#[cfg(feature = "savefile")]
impl<I: IndexStorage, B: Block> FmIndex<I, B> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hit {
    pub text_id: usize,
    pub position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HalfOpenInterval {
    pub start: usize,
    pub end: usize,
}

pub trait IndexStorage:
    PrimInt + Pod + maybe_savefile::MaybeSavefile + sealed::Sealed + Send + Sync + 'static
{
    type LibsaisOutput: OutputElement;

    fn sample_suffix_array(
        suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: std::collections::HashMap<usize, Self>,
    ) -> SampledSuffixArray<Self>;
}

impl sealed::Sealed for i32 {}

impl IndexStorage for i32 {
    type LibsaisOutput = i32;

    fn sample_suffix_array(
        suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: std::collections::HashMap<usize, Self>,
    ) -> SampledSuffixArray<Self> {
        SampledSuffixArray::new_uncompressed(suffix_array_bytes, sampling_rate, text_border_lookup)
    }
}

impl sealed::Sealed for u32 {}

impl IndexStorage for u32 {
    type LibsaisOutput = i64;

    fn sample_suffix_array(
        suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: std::collections::HashMap<usize, Self>,
    ) -> SampledSuffixArray<Self> {
        SampledSuffixArray::new_u32_compressed(
            suffix_array_bytes,
            sampling_rate,
            text_border_lookup,
        )
    }
}

impl sealed::Sealed for i64 {}

impl IndexStorage for i64 {
    type LibsaisOutput = i64;

    fn sample_suffix_array(
        suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: std::collections::HashMap<usize, Self>,
    ) -> SampledSuffixArray<Self> {
        SampledSuffixArray::new_uncompressed(suffix_array_bytes, sampling_rate, text_border_lookup)
    }
}

mod maybe_savefile {
    #[cfg(feature = "savefile")]
    pub trait MaybeSavefile: savefile::Savefile {}

    #[cfg(not(feature = "savefile"))]
    pub trait MaybeSavefile {}

    impl MaybeSavefile for i32 {}
    impl MaybeSavefile for u32 {}
    impl MaybeSavefile for i64 {}
}

mod sealed {
    pub trait Sealed {}
}
