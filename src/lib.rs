/*! This library contains an implementation of the FM-Index data structure ([original paper]).
 *
 * It is based on an encoding for the text with rank support data structure (a.k.a. occurrence table)
 * by Simon Gene Gottlieb. This encoding attemps to provide a good trade-off between
 * memory usage and running time of queries. Another traditional encoding is provided with higher memory usage,
 * but faster query running times.
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
 * More information about the flexible [cursor](Cursor) API, build [configuration](FmIndexConfig)
 * and [variants](TextWithRankSupport) of the FM-Index can be found in the module-level and struct-level documentation.
 *
 * Optimized functions such as [`FmIndex::locate_many`] exist for searching multiple queries at once. They do not use
 * multi-threading, but can still be significantly faster (around 2x) than calling the respective functions for single
 * queries in a loop. The reason for the improved performance is that the queries are searched in batches, which allows
 * different kinds of parallelism inside the CPU to be used. An example of how such a function is used can be found
 * [here](https://github.com/feldroop/genedex/blob/master/examples/basic_usage.rs).
 *
 * [original paper]: https://doi.org/10.1109/SFCS.2000.892127
 * [`libsais-rs`]: https://github.com/feldroop/libsais-rs
 */

/// Contains functions to create various commonly used alphabets.
pub mod alphabet;

/// Different implementations of the text with rank support (a.k.a. occurrence table) data structure that powers the FM-Index.
///
/// The [`TextWithRankSupport`] and [`Block`](text_with_rank_support::Block) traits are good places to start
///  learning about this module.
pub mod text_with_rank_support;

mod batch_computed_cursors;
mod config;
mod construction;
mod cursor;
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

use batch_computed_cursors::BatchComputedCursors;
use construction::DataStructures;
use lookup_table::LookupTables;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;
use text_with_rank_support::{
    Block64, Block512, CondensedTextWithRankSupport, FlatTextWithRankSupport, TextWithRankSupport,
};

/// The FM-Index data structure.
///
/// See [crate-level documentation](self) for details.
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[savefile_doc_hidden]
#[derive(Clone)]
pub struct FmIndex<I, R = CondensedTextWithRankSupport<I, Block64>> {
    alphabet: Alphabet,
    count: Vec<usize>,
    text_with_rank_support: R,
    suffix_array: SampledSuffixArray<I>,
    text_ids: TexdIdSearchTree,
    lookup_tables: LookupTables<I>,
}

/// A little faster than [`FmIndexCondensed512`], and still space efficient for larger alphabets.
/// This is the default version.
pub type FmIndexCondensed64<I> = FmIndex<I, CondensedTextWithRankSupport<I, Block64>>;

/// The most space efficient version. Currently, this is experimental and the `*64` usually provide better trade-offs.
pub type FmIndexCondensed512<I> = FmIndex<I, CondensedTextWithRankSupport<I, Block512>>;

/// The fastest version.
pub type FmIndexFlat64<I> = FmIndex<I, FlatTextWithRankSupport<I, Block64>>;

/// A little smaller and slower than [`FmIndexFlat64`]. [`FmIndexCondensed64`] should be a better trade-off for most applications.
pub type FmIndexFlat512<I> = FmIndex<I, FlatTextWithRankSupport<I, Block512>>;

const BATCH_SIZE: usize = 64;

impl<I: IndexStorage, R: TextWithRankSupport<I>> FmIndex<I, R> {
    fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        alphabet: Alphabet,
        config: FmIndexConfig<I, R>,
    ) -> Self {
        let DataStructures {
            count,
            sampled_suffix_array,
            text_ids,
            text_with_rank_support,
        } = construction::create_data_structures::<I, R, T>(texts, &config, &alphabet);

        let mut index = FmIndex {
            alphabet,
            count,
            text_with_rank_support,
            suffix_array: sampled_suffix_array,
            text_ids,
            lookup_tables: LookupTables::new_empty(),
        };

        lookup_table::fill_lookup_tables(&mut index, config.lookup_table_depth);

        index
    }

    /// Returns the number of occurrences of `query` in the set of indexed texts.
    ///
    /// Running time is in O(`query.len() - d`), where d is the depth of the lookup table of the index.
    pub fn count(&self, query: &[u8]) -> usize {
        self.cursor_for_query(query).count()
    }

    /// The results of [`Self::count`] for multiple queries.
    ///
    /// The order of the queries is preserved for the counts. This function can improve the running
    /// time when many queries are searched.
    pub fn count_many<Q: AsRef<[u8]>>(
        &self,
        queries: impl IntoIterator<Item = Q>,
    ) -> impl Iterator<Item = usize> {
        self.cursors_for_many_queries(queries)
            .map(|cursor| cursor.count())
    }

    /// Returns the occurrences of `query` in the set of indexed texts. The occurrences are not sorted by text id or position.
    ///
    /// The initial running time is the same as for [`count`](Self::count).
    /// For each hit pulled from the iterator, a sampled suffix array lookup is performed.
    /// This operation needs `s / 2` steps on average, where `s` is the suffix array
    /// sampling rate of the index.
    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = Hit> {
        let cursor = self.cursor_for_query(query);

        self.locate_interval(cursor.interval())
    }

    /// The results of [`Self::locate`] for multiple queries.
    ///
    /// The order of the queries is preserved for the hits. This function can improve the running
    /// time when many queries are searched.
    pub fn locate_many<Q: AsRef<[u8]>>(
        &self,
        queries: impl IntoIterator<Item = Q>,
    ) -> impl Iterator<Item: Iterator<Item = Hit>> {
        self.cursors_for_many_queries(queries)
            .map(|cursor| self.locate_interval(cursor.interval()))
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
    pub fn cursor_empty<'a>(&'a self) -> Cursor<'a, I, R> {
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
    /// This allows using a lookup table jump and therefore can be more efficient than creating
    /// an empty cursor and repeatedly calling [`Cursor::extend_query_front`].
    pub fn cursor_for_query<'a>(&'a self, query: &[u8]) -> Cursor<'a, I, R> {
        let (remaining_query, query_suffix) = self.split_query_for_lookup(query);
        let interval = self.lookup_tables.lookup(query_suffix, &self.alphabet);

        let mut cursor = Cursor {
            index: self,
            interval,
        };

        for &symbol in remaining_query.iter().rev() {
            cursor.extend_query_front(symbol);

            if cursor.count() == 0 {
                break;
            }
        }

        cursor
    }

    /// The results of [`Self::cursor_for_query`] for multiple queries.
    ///
    /// The order of the queries is preserved for the cursors. This function can improve the running
    /// time when many queries are searched.
    pub fn cursors_for_many_queries<'a, Q: AsRef<[u8]>>(
        &'a self,
        queries: impl IntoIterator<Item = Q>,
    ) -> impl Iterator<Item = Cursor<'a, I, R>> {
        BatchComputedCursors::<I, R, Q, _, BATCH_SIZE>::new(self, queries.into_iter())
    }

    fn cursor_for_query_without_alphabet_translation<'a>(
        &'a self,
        query: &[u8],
    ) -> Cursor<'a, I, R> {
        let (remaining_query, query_suffix) = self.split_query_for_lookup(query);
        let interval = self
            .lookup_tables
            .lookup_without_alphabet_translation(query_suffix);

        let mut cursor = Cursor {
            index: self,
            interval,
        };

        for &symbol in remaining_query.iter().rev() {
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

    fn split_query_for_lookup<'a>(&self, query: &'a [u8]) -> (&'a [u8], &'a [u8]) {
        let lookup_depth = std::cmp::min(query.len(), self.lookup_tables.max_depth());
        let suffix_idx = query.len() - lookup_depth;
        query.split_at(suffix_idx)
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

    #[cfg(feature = "savefile")]
    const VERSION_FOR_SAVEFILE: u32 = 0;

    #[cfg(feature = "savefile")]
    pub fn load_from_reader(
        reader: &mut impl std::io::Read,
    ) -> Result<Self, savefile::SavefileError> {
        savefile::load(reader, Self::VERSION_FOR_SAVEFILE)
    }

    #[cfg(feature = "savefile")]
    pub fn load_from_file(
        filepath: impl AsRef<std::path::Path>,
    ) -> Result<Self, savefile::SavefileError> {
        savefile::load_file(filepath, Self::VERSION_FOR_SAVEFILE)
    }

    #[cfg(feature = "savefile")]
    pub fn save_to_writer(
        &self,
        writer: &mut impl std::io::Write,
    ) -> Result<(), savefile::SavefileError> {
        savefile::save(writer, Self::VERSION_FOR_SAVEFILE, self)
    }

    #[cfg(feature = "savefile")]
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
