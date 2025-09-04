pub mod alphabet;

mod occurrence_table;
mod sampled_suffix_array;
mod text_id_search_tree;

use libsais::{OutputElement, ThreadCount};
use num_traits::{NumCast, PrimInt};
use rayon::prelude::*;

use alphabet::Alphabet;
use occurrence_table::StringRank;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FmIndex<A, S, C> {
    count: Vec<usize>,
    string_rank: StringRank<A>,
    suffix_array: SampledSuffixArray<S, C>,
    text_ids: TexdIdSearchTree,
}

pub type FmIndexU32<A> = FmIndex<A, i64, U32Compressed>;
pub type FmIndexI32<A> = FmIndex<A, i32, Uncompressed>;
pub type FmIndexI64<A> = FmIndex<A, i64, Uncompressed>;

impl<A: Alphabet, S: OutputElement + Send + Sync + 'static> FmIndex<A, S, Uncompressed> {
    // text chars must be smaller than alphabet size and greater than 0
    // other operations use rayons configured number of threads
    pub fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let (count, suffix_array, bwt, text_ids) =
            create_data_structures::<A, S, T>(texts, suffix_array_construction_thread_count);

        let sampled_suffix_array =
            SampledSuffixArray::new_uncompressed(suffix_array, suffix_array_sampling_rate);

        let occurrence_table = StringRank::construct(&bwt);

        FmIndex {
            count,
            string_rank: occurrence_table,
            suffix_array: sampled_suffix_array,
            text_ids,
        }
    }
}

impl<A: Alphabet> FmIndexU32<A> {
    // text chars must be smaller than alphabet size + 1 and greater than 0
    // other operations use rayons configured number of threads
    pub fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let (count, suffix_array, bwt, text_ids) =
            create_data_structures::<A, i64, T>(texts, suffix_array_construction_thread_count);

        let sampled_suffix_array =
            SampledSuffixArray::new_u32_compressed(suffix_array, suffix_array_sampling_rate);

        let occurrence_table = StringRank::construct(&bwt);

        FmIndex {
            count,
            string_rank: occurrence_table,
            suffix_array: sampled_suffix_array,
            text_ids,
        }
    }
}

impl<A: Alphabet, S: PrimInt + 'static, C: CompressionMode> FmIndex<A, S, C> {
    pub fn count(&self, query: &[u8]) -> usize {
        let (start, end) = self.search_suffix_array_interval(query);
        end - start
    }

    // returns half open interval [start, end)
    fn search_suffix_array_interval(&self, query: &[u8]) -> (usize, usize) {
        let (mut start, mut end) = (0, self.string_rank.len());

        for &character in query.iter().rev() {
            let symbol = A::DENSE_ENCODING_TRANSLATION_TABLE[character as usize];
            assert!(symbol != 255);

            // it is assumed that the query doesn't contain the sentinel
            start = self.lf_mapping_step(symbol, start);
            end = self.lf_mapping_step(symbol, end);
        }

        (start, end)
    }

    fn lf_mapping_step(&self, symbol: u8, idx: usize) -> usize {
        self.count[symbol as usize] + self.string_rank.rank(symbol, idx)
    }
}

impl<A: Alphabet, S: PrimInt + 'static> FmIndex<A, S, Uncompressed> {
    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = (usize, usize)> {
        let (start, end) = self.search_suffix_array_interval(query);

        self.suffix_array
            .recover_range_uncompressed(start..end, self)
            .map(|idx| {
                self.text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap())
            })
    }
}

impl<A: Alphabet> FmIndexU32<A> {
    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = (usize, usize)> {
        let (start, end) = self.search_suffix_array_interval(query);

        self.suffix_array
            .recover_range_u32_compressed(start..end, self)
            .map(|idx| {
                self.text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap())
            })
    }
}

fn create_data_structures<A: Alphabet, S: OutputElement + Send + Sync + 'static, T: AsRef<[u8]>>(
    texts: impl IntoIterator<Item = T>,
    suffix_array_construction_thread_count: u16,
) -> (Vec<usize>, Vec<S>, Vec<u8>, TexdIdSearchTree) {
    let (text, mut frequency_table, sentinel_indices) =
        alphabet::create_concatenated_rank_text(texts, &A::DENSE_ENCODING_TRANSLATION_TABLE);

    let text_ids = TexdIdSearchTree::new_from_sentinel_indices(sentinel_indices);

    let count = frequencies_to_cumulative_count_vector(&frequency_table, A::size());

    let mut construction = libsais::SuffixArrayConstruction::for_text(&text)
        .in_owned_buffer()
        .multi_threaded(ThreadCount::fixed(suffix_array_construction_thread_count));

    unsafe {
        construction = construction.with_frequency_table(&mut frequency_table);
    }

    let suffix_array = construction
        .run()
        .expect("libsais suffix array construction")
        .into_vec();

    let bwt = bwt_from_suffix_array(&suffix_array, &text);

    (count, suffix_array, bwt, text_ids)
}

fn frequencies_to_cumulative_count_vector<S: OutputElement>(
    frequency_table: &[S],
    alphabet_size: usize,
) -> Vec<usize> {
    let mut count: Vec<_> = frequency_table[..alphabet_size + 1]
        .iter()
        .map(|&value| <usize as NumCast>::from(value).unwrap())
        .collect();

    let mut sum = 0;

    for entry in count.iter_mut() {
        let temp = *entry;
        *entry = sum;
        sum += temp;
    }

    count
}

fn bwt_from_suffix_array<S: OutputElement + Sync>(suffix_array: &[S], text: &[u8]) -> Vec<u8> {
    let mut bwt = vec![0; text.len()];

    suffix_array
        .par_iter()
        .zip(&mut bwt)
        // type named to fix rust-analyzer problem
        .for_each(|(&text_index, bwt_entry): (&S, &mut u8)| {
            let text_index = <usize as NumCast>::from(text_index).unwrap();

            *bwt_entry = if text_index > 0 {
                text[text_index - 1]
            } else {
                // last text character is always 0
                0
            };
        });

    bwt
}

pub trait CompressionMode {}

pub struct Uncompressed {}

impl CompressionMode for Uncompressed {}

pub struct U32Compressed {}

impl CompressionMode for U32Compressed {}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::alphabet::AsciiDna;

    use super::*;

    fn create_index() -> FmIndex<AsciiDna, i32, Uncompressed> {
        let text = b"cccaaagggttt".as_slice();

        FmIndexI32::new([text], 1, 3)
    }

    fn create_index_u32_compressed() -> FmIndex<AsciiDna, i64, U32Compressed> {
        let text = b"cccaaagggttt".as_slice();

        FmIndexU32::new([text], 1, 3)
    }

    static BASIC_QUERY: &[u8] = b"gg";
    static FRONT_QUERY: &[u8] = b"c";
    static WRAPPING_QUERY: &[u8] = b"ta";
    static MULTI_QUERY: &[u8] = b"gt";

    #[test]
    fn basic_search() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(BASIC_QUERY).collect();
        let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(BASIC_QUERY).collect();

        assert_eq!(results, HashSet::from_iter([(0, 6), (0, 7)]));
        assert_eq!(results_u32_compressed, HashSet::from_iter([(0, 6), (0, 7)]));
    }

    #[test]
    fn text_front_search() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(FRONT_QUERY).collect();
        let results_u32_compressed: HashSet<_> = index_u32_compressed.locate(FRONT_QUERY).collect();

        assert_eq!(results, HashSet::from_iter([(0, 0), (0, 1), (0, 2)]));
        assert_eq!(
            results_u32_compressed,
            HashSet::from_iter([(0, 0), (0, 1), (0, 2)])
        );
    }

    #[test]
    fn search_no_wrapping() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(WRAPPING_QUERY).collect();
        let results_u32_compressed: HashSet<_> =
            index_u32_compressed.locate(WRAPPING_QUERY).collect();

        assert!(results.is_empty());
        assert!(results_u32_compressed.is_empty());
    }

    #[test]
    fn search_multitext() {
        let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];

        let index = FmIndexU32::<AsciiDna>::new(texts, 1, 3);

        let results_basic_query: HashSet<_> = index.locate(BASIC_QUERY).collect();
        assert_eq!(results_basic_query, HashSet::from_iter([(0, 6), (0, 7)]));

        let results_multi_query: HashSet<_> = index.locate(MULTI_QUERY).collect();
        assert_eq!(
            results_multi_query,
            HashSet::from_iter([(0, 8), (1, 2), (1, 6), (1, 10)])
        );
    }
}
