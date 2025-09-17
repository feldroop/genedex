/*! This library contains an implementation of the FM-Index data structure ([original paper]).
 *
 * It is based on an encoding for the text with rank support data structure (a.k.a. occurrence table)
 * by Simon Gene Gottlieb (publication pending). This encoding attemps to provide a good trade-off between
 * memory usage and running time of queries.
 *
 * The library supports creating an FM-Index for a set of texts over an [alphabet]. The index construction
 * is based on the [`libsais-rs`] crate and parallelized.
 *
 * ## Usage
 *
 * The following is a basic example of how to use this library:
 *
 * ```
 * use genedex::{FmIndexConfig, alphabet};
 *
 * let dna_n_alphabet = alphabet::ascii_dna_with_n();
 * let texts = [b"aACGT", b"acGtn"];
 *
 * let index = FmIndexConfig::<i32>::new().construct_index(texts, dna_n_alphabet);
 *
 * let query = b"GT";
 * assert_eq!(index.count(query), 2);
 *
 * for hit in index.locate(query) {
 *     println!(
 *         "Found query in text {} at position {}.",
 *         hit.text_id, hit.position
 *     );
 * }
 * ```
 *
 * More information about the flexible [cursor](Cursor) API, build [configuration](FmIndexConfig) and [variants](block) of the FM-Index can
 * be found in the module-level and struct-level documentation.
 *
 * [original paper]: https://doi.org/10.1109/SFCS.2000.892127
 * [`libsais-rs`]: https://github.com/feldroop/libsais-rs
 */

/// Contains functions to create various commonly used alphabets.
pub mod alphabet;
mod config;
mod cursor;
mod text_with_rank_support;

mod construction;
mod lookup_table;
mod sampled_suffix_array;
mod text_id_search_tree;

use num_traits::NumCast;

#[doc(inline)]
pub use alphabet::Alphabet;
#[doc(inline)]
pub use config::FmIndexConfig;
#[doc(inline)]
pub use config::PerformancePriority;
#[doc(inline)]
pub use construction::IndexStorage;
#[doc(inline)]
pub use cursor::Cursor;
#[doc(inline)]
pub use text_with_rank_support::TextWithRankSupport;
#[doc(inline)]
pub use text_with_rank_support::block;

use block::{Block, Block64};

use construction::DataStructures;
use lookup_table::LookupTables;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;

/// The FM-Index data structure.
///
/// See [crate-level documentation](self) for details.
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

        let num_searchable_dense_symbols = alphabet.num_searchable_dense_symbols();

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
            num_searchable_dense_symbols,
        );

        index
    }

    /// Returns the number of occurrences of `query` in the set of indexed texts.
    ///
    /// Running time is in O(`query.len() - d`), where d is the depth of the lookup table of the index.
    pub fn count(&self, query: &[u8]) -> usize {
        self.cursor_for_query(query).count()
    }

    /// Returns the number of occurrences of `query` in the set of indexed texts.
    ///
    /// The initial running time is the same as for [`count`](Self::count).
    /// For each hit pulled from the iterator, a sampled suffix array lookup is performed.
    /// This operation needs `s / 2` steps on average, where `s` is the suffix array
    /// sampling rate of the index.
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

    /// Returns a cursor to the index with the empty query currently searched.
    ///
    /// See [`Cursor`] for details. Running time is in `O(1)`.
    pub fn cursor_empty<'a>(&'a self) -> Cursor<'a, I, B> {
        Cursor {
            index: self,
            interval: HalfOpenInterval {
                start: 0,
                end: self.total_text_len(),
            },
        }
    }

    /// Returns a cursor to the index with `query` currently searched.
    ///
    /// See [`Cursor`] for details. Running time is the same as for [`count`](Self::count).
    pub fn cursor_for_query<'a>(&'a self, query: &[u8]) -> Cursor<'a, I, B> {
        let query_iter = query
            .iter()
            .rev()
            .map(|&s| self.alphabet.io_to_dense_representation(s));

        self.cursor_for_iter_without_alphabet_translation(query_iter)
    }

    fn cursor_for_iter_without_alphabet_translation<'a, Q>(
        &'a self,
        query: impl IntoIterator<IntoIter = Q>,
    ) -> Cursor<'a, I, B>
    where
        Q: ExactSizeIterator<Item = u8>,
    {
        let mut query_iter = query.into_iter();

        let lookup_depth = std::cmp::min(query_iter.len(), self.lookup_tables.max_depth());
        let (start, end) = self.lookup_tables.lookup(&mut query_iter, lookup_depth);

        let mut cursor = Cursor {
            index: self,
            interval: HalfOpenInterval { start, end },
        };

        for symbol in query_iter {
            cursor.extend_front_without_alphabet_translation(symbol);

            if cursor.count() == 0 {
                break;
            }
        }

        cursor
    }

    fn lf_mapping_step(&self, symbol: u8, idx: usize) -> usize {
        self.count[symbol as usize] + self.text_with_rank_support.rank(symbol, idx)
    }

    pub fn alphabet(&self) -> &Alphabet {
        &self.alphabet
    }

    pub fn num_texts(&self) -> usize {
        self.text_ids.sentinel_indices.len()
    }

    /// The length of all the texts that this index is built on. The value includes a sentinel symbol for each text.
    pub fn total_text_len(&self) -> usize {
        self.text_with_rank_support.text_len()
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

/// Represents an occurrence of a searched query in the set of indexed texts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hit {
    pub text_id: usize,
    pub position: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct HalfOpenInterval {
    pub start: usize,
    pub end: usize,
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
