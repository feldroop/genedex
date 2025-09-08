pub mod alphabet;
pub mod text_with_rank_support;

mod sampled_suffix_array;
mod text_id_search_tree;

use std::{collections::HashMap, marker::PhantomData, sync::Mutex};

use bytemuck::Pod;
use libsais::{OutputElement, ThreadCount};
use num_traits::{NumCast, PrimInt};
use rayon::prelude::*;

pub use alphabet::Alphabet;
pub use text_with_rank_support::TextWithRankSupport;
use text_with_rank_support::{Block, Block512};

use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FmIndex<A, I, B = Block512> {
    count: Vec<usize>,
    text_with_rank_support: TextWithRankSupport<I, B>,
    suffix_array: SampledSuffixArray<I>,
    text_ids: TexdIdSearchTree,
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

impl<A: Alphabet, I: OutputElement, B: Block> FmIndex<A, I, B> {
    // text chars must be smaller than alphabet size and greater than 0
    // other operations use rayons configured number of threads
    pub fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let DataStructures {
            count,
            suffix_array_bytes,
            bwt,
            text_ids,
            text_border_lookup,
        } = create_data_structures::<A, I, T>(texts, suffix_array_construction_thread_count);

        let sampled_suffix_array = SampledSuffixArray::new_uncompressed(
            suffix_array_bytes,
            suffix_array_sampling_rate,
            text_border_lookup,
        );

        let occurrence_table = TextWithRankSupport::construct(&bwt, A::SIZE);

        FmIndex {
            count,
            text_with_rank_support: occurrence_table,
            suffix_array: sampled_suffix_array,
            text_ids,
            _alphabet_marker: PhantomData,
        }
    }
}

impl<A: Alphabet, B: Block> FmIndex<A, u32, B> {
    // text chars must be smaller than alphabet size + 1 and greater than 0
    // other operations use rayons configured number of threads
    pub fn new_u32_compressed<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let DataStructures {
            count,
            suffix_array_bytes,
            bwt,
            text_ids,
            text_border_lookup,
        } = create_data_structures::<A, i64, T>(texts, suffix_array_construction_thread_count);

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

        let occurrence_table = TextWithRankSupport::construct(&bwt, A::SIZE);

        FmIndex {
            count,
            text_with_rank_support: occurrence_table,
            suffix_array: sampled_suffix_array,
            text_ids,
            _alphabet_marker: PhantomData,
        }
    }
}

impl<A: Alphabet, I: PrimInt + Pod + 'static, B: Block> FmIndex<A, I, B> {
    pub fn count(&self, query: &[u8]) -> usize {
        let (start, end) = self.search_suffix_array_interval(query);
        end - start
    }

    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = Hit> {
        let (start, end) = self.search_suffix_array_interval(query);

        self.suffix_array
            .recover_range(start..end, self)
            .map(|idx| {
                // println!("concat text index: {idx}");
                let (text_id, position) = self
                    .text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap());

                Hit { text_id, position }
            })
    }

    // returns half open interval [start, end)
    fn search_suffix_array_interval(&self, query: &[u8]) -> (usize, usize) {
        assert!(!query.is_empty());

        let (mut start, mut end) = (0, self.text_with_rank_support.text_len());

        for &character in query.iter().rev() {
            let symbol = A::DENSE_ENCODING_TRANSLATION_TABLE[character as usize];
            assert!(symbol != 255 && symbol != 0);

            // it is assumed that the query doesn't contain the sentinel
            start = self.lf_mapping_step(symbol, start);
            end = self.lf_mapping_step(symbol, end);
        }

        (start, end)
    }

    fn lf_mapping_step(&self, symbol: u8, idx: usize) -> usize {
        self.count[symbol as usize] + self.text_with_rank_support.rank(symbol, idx)
    }
}

struct DataStructures<I> {
    count: Vec<usize>,
    suffix_array_bytes: Vec<u8>,
    bwt: Vec<u8>,
    text_ids: TexdIdSearchTree,
    text_border_lookup: HashMap<usize, I>,
}

fn create_data_structures<A: Alphabet, I: OutputElement, T: AsRef<[u8]>>(
    texts: impl IntoIterator<Item = T>,
    suffix_array_construction_thread_count: u16,
) -> DataStructures<I> {
    let (text, mut frequency_table, sentinel_indices) =
        create_concatenated_densely_encoded_text(texts, &A::DENSE_ENCODING_TRANSLATION_TABLE);

    let text_ids = TexdIdSearchTree::new_from_sentinel_indices(sentinel_indices);

    let count = frequency_table_to_count(&frequency_table, A::SIZE);

    // allocate the buffer in bytes, because maybe we want to muck around with integer types later (compress i64 into u32)
    let mut suffix_array_bytes = vec![0u8; text.len() * size_of::<I>()];
    let suffix_array_buffer: &mut [I] = bytemuck::cast_slice_mut(&mut suffix_array_bytes);

    let mut construction = libsais::SuffixArrayConstruction::for_text(&text)
        .in_borrowed_buffer(suffix_array_buffer)
        .multi_threaded(ThreadCount::fixed(suffix_array_construction_thread_count));

    unsafe {
        construction = construction.with_frequency_table(&mut frequency_table);
    }

    construction
        .run()
        .expect("libsais suffix array construction");

    let (bwt, text_border_lookup) = bwt_from_suffix_array(suffix_array_buffer, &text);

    DataStructures {
        count,
        suffix_array_bytes,
        bwt,
        text_ids,
        text_border_lookup,
    }
}

fn create_concatenated_densely_encoded_text<S: OutputElement, T: AsRef<[u8]>>(
    texts: impl IntoIterator<Item = T>,
    translation_table: &[u8; 256],
) -> (Vec<u8>, Vec<S>, Vec<usize>) {
    // this generic texts owned vec is needed for the as_ref interface
    let generic_texts: Vec<_> = texts.into_iter().collect();
    let texts: Vec<&[u8]> = generic_texts.iter().map(|t| t.as_ref()).collect();
    let num_texts = texts.len();

    let needed_capacity = texts.iter().map(|t| t.len()).sum::<usize>() + num_texts;

    let sentinel_indices: Vec<_> = texts
        .iter()
        .scan(0, |state, t| {
            let temp = *state + t.len();
            *state += t.len() + 1;
            Some(temp)
        })
        .collect();

    let mut concatenated_text = vec![0; needed_capacity];
    let mut concatenated_text_splits = Vec::with_capacity(num_texts);
    let mut remaining_slice = concatenated_text.as_mut_slice();

    for t in texts.iter() {
        let (this, remaining) = remaining_slice.split_at_mut(t.len() + 1);
        concatenated_text_splits.push(this);
        remaining_slice = remaining;
    }

    let mut frequency_table = texts
        .into_par_iter()
        .zip(concatenated_text_splits)
        .map(|(text, concatenated_text_split)| {
            let mut frequency_table = vec![S::zero(); 256];

            for (source, target) in text.iter().zip(concatenated_text_split) {
                *target = translation_table[*source as usize];
                frequency_table[*target as usize] = frequency_table[*target as usize] + S::one();
            }

            frequency_table
        })
        .reduce_with(merge_frequency_tables)
        .expect("There should be at least one texts");

    frequency_table[0] = <S as NumCast>::from(num_texts).unwrap();

    (concatenated_text, frequency_table, sentinel_indices)
}

fn merge_frequency_tables<S: OutputElement>(mut f1: Vec<S>, f2: Vec<S>) -> Vec<S> {
    for (x1, x2) in f1.iter_mut().zip(f2) {
        *x1 = *x1 + x2;
    }

    f1
}

fn frequency_table_to_count<S: OutputElement>(
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

fn bwt_from_suffix_array<I: OutputElement>(
    suffix_array: &[I],
    text: &[u8],
) -> (Vec<u8>, HashMap<usize, I>) {
    let mut bwt = vec![0; text.len()];

    let text_border_lookup = Mutex::new(HashMap::new());

    suffix_array
        .par_iter()
        .zip(&mut bwt)
        .enumerate()
        // type named to fix rust-analyzer problem
        .for_each(
            |(suffix_array_index, (&text_index, bwt_entry)): (usize, (&I, &mut u8))| {
                let text_index_usize = <usize as NumCast>::from(text_index).unwrap();

                *bwt_entry = if text_index_usize > 0 {
                    text[text_index_usize - 1]
                } else {
                    // last text character is always 0
                    0
                };

                if *bwt_entry == 0 {
                    text_border_lookup
                        .lock()
                        .unwrap()
                        .insert(suffix_array_index, text_index);
                }
            },
        );

    (bwt, text_border_lookup.into_inner().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concat_text() {
        let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];
        let (text, frequency_table, sentinel_indices) =
            create_concatenated_densely_encoded_text::<i32, _>(
                texts,
                &alphabet::ASCII_DNA_TRANSLATION_TABLE,
            );

        assert_eq!(
            text,
            [
                2, 2, 2, 1, 1, 1, 3, 3, 3, 4, 4, 4, 0, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 0
            ]
        );

        assert_eq!(&sentinel_indices, &[12, 25]);

        let mut expected_frequency_table = vec![0; 256];
        expected_frequency_table[0] = 2;
        expected_frequency_table[1] = 6;
        expected_frequency_table[2] = 6;
        expected_frequency_table[3] = 6;
        expected_frequency_table[4] = 6;

        assert_eq!(expected_frequency_table, frequency_table);
    }
}
