use crate::{
    Alphabet, FmIndex, IndexStorage,
    text_with_rank_support::block::{Block, Block64},
};
use std::marker::PhantomData;

/// A builder-like API to configure and construct the FM-Index.
#[derive(Clone, Copy)]
pub struct FmIndexConfig<I, B = Block64> {
    pub(crate) suffix_array_sampling_rate: usize,
    pub(crate) lookup_table_depth: usize,
    pub(crate) performance_priority: PerformancePriority,
    _index_storage_marker: PhantomData<I>,
    _block_marker: PhantomData<B>,
}

impl<I: IndexStorage, B: Block> FmIndexConfig<I, B> {
    pub fn new() -> Self {
        Self::default()
    }

    /// The FM-Index internally stores a suffix array. Every entry of this array at a position
    /// divisible by `suffix_array_sampling_rate` is retained. For example, a rate of 3
    /// would retain every third entry of the suffix array.
    ///
    /// A larger rate leads to less memory usage, but higher locate running time. The default is `4`.
    pub fn suffix_array_sampling_rate(self, suffix_array_sampling_rate: usize) -> Self {
        assert!(suffix_array_sampling_rate > 0);

        Self {
            suffix_array_sampling_rate,
            ..self
        }
    }

    /// The FM-Index stores a lookup table to skip the first `lookup_table_depth` many search steps
    /// when searching a query. The size of the lookup table grows exponentially in its depth,
    /// with the number of searchable alphabet symbols as base. The default is `8`.
    ///
    /// For large texts like genomes and small alphabets like DNA alphabets with 4 searchable symbols,
    /// values up to around `13` might be reasonable choices.
    pub fn lookup_table_depth(self, lookup_table_depth: usize) -> Self {
        Self {
            lookup_table_depth,
            ..self
        }
    }

    /// See [`PerformancePriority`] for details.
    pub fn construction_performance_priority(
        self,
        performance_priority: PerformancePriority,
    ) -> Self {
        Self {
            performance_priority,
            ..self
        }
    }

    /// Construct the FM-Index.
    ///
    /// The number of threads for the build procedure is controlled by [`rayon`].
    pub fn construct_index<T: AsRef<[u8]>>(
        self,
        texts: impl IntoIterator<Item = T>,
        alphabet: Alphabet,
    ) -> FmIndex<I, B> {
        FmIndex::new(texts, alphabet, self)
    }
}

impl<I: IndexStorage, B: Block> Default for FmIndexConfig<I, B> {
    fn default() -> Self {
        Self {
            suffix_array_sampling_rate: 4,
            lookup_table_depth: 8,
            performance_priority: PerformancePriority::HighSpeed,
            _index_storage_marker: PhantomData,
            _block_marker: PhantomData,
        }
    }
}

/// This enum can be supplied to the [`FmIndexConfig`] to select different sub-algorithms during the
/// construction of the FM-Index.
///
/// The default is [`HighSpeed`](PerformancePriority::HighSpeed).
#[derive(Debug, Clone, Copy)]
pub enum PerformancePriority {
    HighSpeed,
    Balanced,
    /// A slower, not parallel suffix array construction algorithm will be used for `u32`-based FM-Indices.
    /// This can save a lot of memory when the sum of text lengths fits into a `u32`, but not into a `i32`.
    LowMemory,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_config() {
        let texts = [b"ACGT"];
        let alphabet = crate::alphabet::ascii_dna();

        let _index = FmIndexConfig::<i32>::new()
            .lookup_table_depth(5)
            .suffix_array_sampling_rate(8)
            .construct_index(texts, alphabet);
    }
}
