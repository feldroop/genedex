pub mod alphabet;
pub mod naive_occurrence_table;

mod sampled_suffix_array;
mod text_id_search_tree;

use libsais::{OutputElement, ThreadCount};
use num_traits::{NumCast, PrimInt};

use alphabet::Alphabet;
use naive_occurrence_table::NaiveOccurrenceTable;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;

pub trait CompressionMode {}

pub struct Uncompressed {}

impl CompressionMode for Uncompressed {}

pub struct U32Compressed {}

impl CompressionMode for U32Compressed {}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FmIndex<A, S, C> {
    text_len: usize,
    count: Vec<usize>,
    occurrence_table: NaiveOccurrenceTable<A>,
    suffix_array: SampledSuffixArray<S, C>,
    text_ids: TexdIdSearchTree,
}

impl<A: Alphabet, S: OutputElement + 'static> FmIndex<A, S, Uncompressed> {
    // text chars must be smaller than alphabet size + 1 and greater than 0
    pub fn new<'a>(
        texts: impl IntoIterator<Item = &'a [u8]>,
        thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let (count, suffix_array, bwt, text_ids, text_len) =
            create_data_structures::<A, S>(texts, thread_count);

        let sampled_suffix_array =
            SampledSuffixArray::new_uncompressed(suffix_array, suffix_array_sampling_rate);

        let occurrence_table = NaiveOccurrenceTable::construct(&bwt);

        FmIndex {
            text_len,
            count,
            occurrence_table,
            suffix_array: sampled_suffix_array,
            text_ids,
        }
    }
}

impl<A: Alphabet> FmIndex<A, i64, U32Compressed> {
    pub fn new_with_u32_compressed_suffix_array<'a>(
        texts: impl IntoIterator<Item = &'a [u8]>,
        thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let (count, suffix_array, bwt, text_ids, text_len) =
            create_data_structures::<A, i64>(texts, thread_count);

        let sampled_suffix_array =
            SampledSuffixArray::new_u32_compressed(suffix_array, suffix_array_sampling_rate);

        let occurrence_table = NaiveOccurrenceTable::construct(&bwt);

        FmIndex {
            text_len,
            count,
            occurrence_table,
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
        let (mut start, mut end) = (0, self.text_len);

        for &character in query.iter().rev() {
            let rank = A::TO_RANK_TRANSLATION_TABLE[character as usize];
            assert!(rank != 255);

            // it is assumed that the query doesn't contain the sentinel
            start = self.lf_mapping_step_no_sentinel(rank, start);
            end = self.lf_mapping_step_no_sentinel(rank, end);
        }

        (start, end)
    }

    fn lf_mapping_step(&self, rank: u8, idx: usize) -> usize {
        self.count[rank as usize] + self.occurrences(rank, idx)
    }

    fn lf_mapping_step_no_sentinel(&self, rank: u8, idx: usize) -> usize {
        self.count[rank as usize] + self.occurrence_table.occurrences(rank, idx)
    }

    fn occurrences(&self, rank: u8, idx: usize) -> usize {
        if rank == 0 {
            if idx == self.text_len {
                self.text_ids.num_texts()
            } else {
                // text id is actually exactly the number of occurrences
                self.text_ids.lookup_text_id(idx)
            }
        } else {
            self.occurrence_table.occurrences(rank, idx)
        }
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

impl<A: Alphabet> FmIndex<A, i64, U32Compressed> {
    pub fn locate_u32_compressed(&self, query: &[u8]) -> impl Iterator<Item = (usize, usize)> {
        let (start, end) = self.search_suffix_array_interval(query);

        self.suffix_array
            .recover_range_u32_compressed(start..end, self)
            .map(|idx| {
                self.text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap())
            })
    }
}

fn create_data_structures<'a, A: Alphabet, S: OutputElement + 'static>(
    texts: impl IntoIterator<Item = &'a [u8]>,
    thread_count: u16,
) -> (Vec<usize>, Vec<S>, Vec<u8>, TexdIdSearchTree, usize) {
    let (text, mut frequency_table, sentinel_indices) =
        alphabet::create_concatenated_rank_text(texts, &A::TO_RANK_TRANSLATION_TABLE)
            .expect("text should be of given alphabet");

    let text_ids = TexdIdSearchTree::new_from_sentinel_indices(sentinel_indices);

    let count = frequencies_to_cumulative_count_vector(&frequency_table, A::size());

    let mut construction = libsais::SuffixArrayConstruction::for_text(&text)
        .in_owned_buffer()
        .multi_threaded(ThreadCount::fixed(thread_count));

    unsafe {
        construction = construction.with_frequency_table(&mut frequency_table);
    }

    let suffix_array = construction
        .run()
        .expect("libsais suffix array construction")
        .into_vec();

    let bwt = bwt_from_suffix_array(&suffix_array, &text);

    (count, suffix_array, bwt, text_ids, text.len())
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

fn bwt_from_suffix_array<S: OutputElement>(suffix_array: &[S], text: &[u8]) -> Vec<u8> {
    let mut bwt = vec![0; text.len()];

    for (suffix_array_index, &text_index) in suffix_array.iter().enumerate() {
        let text_index = <usize as NumCast>::from(text_index).unwrap();
        bwt[suffix_array_index] = if text_index > 0 {
            text[text_index - 1]
        } else {
            // last text character is always 0
            0
        };
    }

    bwt
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::alphabet::AsciiDna;

    use super::*;

    fn create_index() -> FmIndex<AsciiDna, i32, Uncompressed> {
        let text = b"cccaaagggttt".as_slice();

        FmIndex::new([text], 1, 3)
    }

    fn create_index_u32_compressed() -> FmIndex<AsciiDna, i64, U32Compressed> {
        let text = b"cccaaagggttt".as_slice();

        FmIndex::new_with_u32_compressed_suffix_array([text], 1, 3)
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
        let results_u32_compressed: HashSet<_> = index_u32_compressed
            .locate_u32_compressed(BASIC_QUERY)
            .collect();

        assert_eq!(results, HashSet::from_iter([(0, 6), (0, 7)]));
        assert_eq!(results_u32_compressed, HashSet::from_iter([(0, 6), (0, 7)]));
    }

    #[test]
    fn text_front_search() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(FRONT_QUERY).collect();
        let results_u32_compressed: HashSet<_> = index_u32_compressed
            .locate_u32_compressed(FRONT_QUERY)
            .collect();

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
        let results_u32_compressed: HashSet<_> = index_u32_compressed
            .locate_u32_compressed(WRAPPING_QUERY)
            .collect();

        assert!(results.is_empty());
        assert!(results_u32_compressed.is_empty());
    }

    #[test]
    fn search_multitext() {
        let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];

        let index = FmIndex::<AsciiDna, i64, U32Compressed>::new_with_u32_compressed_suffix_array(
            texts, 1, 3,
        );

        let results_basic_query: HashSet<_> = index.locate_u32_compressed(BASIC_QUERY).collect();
        assert_eq!(results_basic_query, HashSet::from_iter([(0, 6), (0, 7)]));

        let results_multi_query: HashSet<_> = index.locate_u32_compressed(MULTI_QUERY).collect();
        assert_eq!(
            results_multi_query,
            HashSet::from_iter([(0, 8), (1, 2), (1, 6), (1, 10)])
        );
    }
}
