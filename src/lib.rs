pub mod alphabet;
pub mod naive_occurrence_table;

mod sampled_suffix_array;

use libsais::{OutputElement, ThreadCount};
use num_traits::{NumCast, PrimInt};

use sampled_suffix_array::SampledSuffixArray;

use crate::naive_occurrence_table::NaiveOccurrenceTable;

pub struct FmIndex<O> {
    text_len: usize,
    count: Vec<usize>,
    occurrence_table: NaiveOccurrenceTable,
    suffix_array: SampledSuffixArray<O>,
}

impl<O: OutputElement + 'static> FmIndex<O> {
    // text chars must be smaller than alphabet size and greater than 0
    pub fn new(
        text: &[u8],
        alphabet_size: usize,
        thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let (count, suffix_array, occurrence_table) =
            count_suffix_array_and_bwt(text, alphabet_size, thread_count);

        let sampled_suffix_array =
            SampledSuffixArray::new(suffix_array, suffix_array_sampling_rate);

        FmIndex {
            text_len: text.len(),
            count,
            occurrence_table,
            suffix_array: sampled_suffix_array,
        }
    }
}

impl FmIndex<i64> {
    pub fn new_u32_compressed(
        text: &[u8],
        alphabet_size: usize,
        thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        assert!((text.len() as u32) < u32::MAX);

        let (count, suffix_array, occurrence_table) =
            count_suffix_array_and_bwt::<i64>(text, alphabet_size, thread_count);

        let sampled_suffix_array =
            SampledSuffixArray::new_u32_compressed(suffix_array, suffix_array_sampling_rate);

        FmIndex {
            text_len: text.len(),
            count,
            occurrence_table,
            suffix_array: sampled_suffix_array,
        }
    }
}

impl<O: PrimInt + 'static> FmIndex<O> {
    pub fn count<'a, Q, E>(&self, query: Q) -> usize
    where
        Q: IntoIterator<IntoIter = E>,
        E: ExactSizeIterator<Item = &'a u8> + DoubleEndedIterator,
    {
        let (start, end) = self.search_suffix_array_interval(query.into_iter());
        end - start
    }

    pub fn locate<'a, Q, E>(&self, query: Q) -> impl Iterator<Item = O>
    where
        Q: IntoIterator<IntoIter = E>,
        E: ExactSizeIterator<Item = &'a u8> + DoubleEndedIterator,
    {
        assert!(!self.suffix_array.is_u32_compressed());

        let query = query.into_iter();
        let query_len = query.size_hint().0;
        let (start, end) = self.search_suffix_array_interval(query);

        // the filter needs to happen, because we are not working with a sentinel
        self.suffix_array
            .recover_range(start..end, self)
            .filter(move |&idx| <usize as NumCast>::from(idx).unwrap() + query_len <= self.text_len)
    }

    // returns half open interval [start, end)
    fn search_suffix_array_interval<'a, E>(&self, query: E) -> (usize, usize)
    where
        E: ExactSizeIterator<Item = &'a u8> + DoubleEndedIterator,
    {
        let (mut start, mut end) = (0, self.text_len);

        for &character in query.rev() {
            start = self.lf_mapping_step(start, character);
            end = self.lf_mapping_step(end, character);
        }

        (start, end)
    }

    fn lf_mapping_step(&self, index: usize, character: u8) -> usize {
        self.count[character as usize] + self.occurrence_table.occurrences(character, index)
    }
}

impl FmIndex<i64> {
    pub fn locate_u32_compressed<'a, Q, E>(&self, query: Q) -> impl Iterator<Item = u32>
    where
        Q: IntoIterator<IntoIter = E>,
        E: ExactSizeIterator<Item = &'a u8> + DoubleEndedIterator,
    {
        assert!(self.suffix_array.is_u32_compressed());

        let query = query.into_iter();
        let query_len = query.size_hint().0;
        let (start, end) = self.search_suffix_array_interval(query);

        // the filter needs to happen, because we are not working with a sentinel
        self.suffix_array
            .recover_range_u32_compressed(start..end, self)
            .filter(move |&idx| usize::try_from(idx).unwrap() + query_len <= self.text_len)
    }
}

fn frequency_table<O: OutputElement>(text: &[u8], alphabet_size: usize) -> Vec<O> {
    assert!(alphabet_size < 255);
    let mut frequency_table = vec![0usize; 256];

    for &c in text {
        frequency_table[c as usize] += 1;
    }

    frequency_table
        .into_iter()
        .map(|value| <O as NumCast>::from(value).unwrap())
        .collect()
}

fn frequencies_to_cumulative_count_vector<O: OutputElement>(
    frequency_table: &[O],
    alphabet_size: usize,
) -> Vec<usize> {
    let mut count: Vec<_> = frequency_table[..alphabet_size]
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

fn bwt_from_suffix_array<O: OutputElement>(suffix_array: &[O], text: &[u8]) -> Vec<u8> {
    let mut bwt = vec![0; text.len()];

    for (suffix_array_index, &text_index) in suffix_array.iter().enumerate() {
        let text_index = <usize as NumCast>::from(text_index).unwrap();
        bwt[suffix_array_index] = if text_index > 0 {
            text[text_index - 1]
        } else {
            *text.last().unwrap()
        };
    }

    bwt
}

fn count_suffix_array_and_bwt<O: OutputElement + 'static>(
    text: &[u8],
    alphabet_size: usize,
    thread_count: u16,
) -> (Vec<usize>, Vec<O>, NaiveOccurrenceTable) {
    let mut frequency_table = frequency_table::<O>(text, alphabet_size);
    let count = frequencies_to_cumulative_count_vector(&frequency_table, alphabet_size);

    let mut construction = libsais::SuffixArrayConstruction::for_text(text)
        .in_owned_buffer()
        .multi_threaded(ThreadCount::fixed(thread_count));

    unsafe {
        construction = construction.with_frequency_table(&mut frequency_table);
    }

    let suffix_array = construction
        .run()
        .expect("libsais suffix array construction")
        .into_vec();

    let bwt = bwt_from_suffix_array(&suffix_array, text);

    let occurrence_table = NaiveOccurrenceTable::construct(alphabet_size, &bwt);

    (count, suffix_array, occurrence_table)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::alphabet::ASCII_DNA_TRANSLATION_TABLE;

    use super::*;

    fn create_index() -> FmIndex<i64> {
        let mut text = Vec::from(b"cccaaagggttt");
        alphabet::transfrom_into_ranks_inplace(&mut text, &ASCII_DNA_TRANSLATION_TABLE).unwrap();

        FmIndex::new(&text, 4, 1, 3)
    }

    fn create_index_u32_compressed() -> FmIndex<i64> {
        let mut text = Vec::from(b"cccaaagggttt");
        alphabet::transfrom_into_ranks_inplace(&mut text, &ASCII_DNA_TRANSLATION_TABLE).unwrap();

        FmIndex::new_u32_compressed(&text, 4, 1, 3)
    }

    fn basic_query() -> impl ExactSizeIterator<Item = &'static u8> + DoubleEndedIterator {
        alphabet::iter_ranks(b"gg", &ASCII_DNA_TRANSLATION_TABLE)
    }

    fn front_query() -> impl ExactSizeIterator<Item = &'static u8> + DoubleEndedIterator {
        alphabet::iter_ranks(b"c", &ASCII_DNA_TRANSLATION_TABLE)
    }

    fn wrapping_query() -> impl ExactSizeIterator<Item = &'static u8> + DoubleEndedIterator {
        alphabet::iter_ranks(b"ta", &ASCII_DNA_TRANSLATION_TABLE)
    }

    #[test]
    fn basic_search() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(basic_query()).collect();
        let results_u32_compressed: HashSet<_> = index_u32_compressed
            .locate_u32_compressed(basic_query())
            .collect();

        assert_eq!(results, HashSet::from_iter([6, 7]));
        assert_eq!(results_u32_compressed, HashSet::from_iter([6, 7]));
    }

    #[test]
    fn text_front_search() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(front_query()).collect();
        let results_u32_compressed: HashSet<_> = index_u32_compressed
            .locate_u32_compressed(front_query())
            .collect();

        assert_eq!(results, HashSet::from_iter([0, 1, 2]));
        assert_eq!(results_u32_compressed, HashSet::from_iter([0, 1, 2]));
    }

    #[test]
    fn search_no_wrapping() {
        let index = create_index();
        let index_u32_compressed = create_index_u32_compressed();

        let results: HashSet<_> = index.locate(wrapping_query()).collect();
        let results_u32_compressed: HashSet<_> = index_u32_compressed
            .locate_u32_compressed(wrapping_query())
            .collect();

        assert!(results.is_empty());
        assert!(results_u32_compressed.is_empty());
    }
}
