mod bwt;
pub(crate) mod slice_compression;

use bytemuck::Pod;
use libsais::{OutputElement, ThreadCount};
use num_traits::{NumCast, PrimInt};
use rayon::prelude::*;

use crate::alphabet::Alphabet;
use crate::config::PerformancePriority;
use crate::construction::slice_compression::{HalfBytesCompression, NoSliceCompression};
use crate::sampled_suffix_array::SampledSuffixArray;
use crate::text_id_search_tree::TexdIdSearchTree;
use crate::{FmIndexConfig, TextWithRankSupport, maybe_savefile, sealed};

pub(crate) struct DataStructures<I, R> {
    pub(crate) count: Vec<usize>,
    pub(crate) sampled_suffix_array: SampledSuffixArray<I>,
    pub(crate) text_ids: TexdIdSearchTree,
    pub(crate) text_with_rank_support: R,
}

pub(crate) fn create_data_structures<I: IndexStorage, R: TextWithRankSupport<I>, T: AsRef<[u8]>>(
    texts: impl IntoIterator<Item = T>,
    config: &FmIndexConfig<I, R>,
    alphabet: &Alphabet,
) -> DataStructures<I, R> {
    let (mut text, mut frequency_table, sentinel_indices) =
        create_concatenated_densely_encoded_text(texts, alphabet);

    assert!(text.len() <= <usize as NumCast>::from(I::max_value()).unwrap());

    let text_ids = TexdIdSearchTree::new_from_sentinel_indices(sentinel_indices);

    let count = frequency_table_to_count(&frequency_table, alphabet.num_dense_symbols());

    let mut maybe_bwt_buffer = Vec::new();

    let (sampled_suffix_array, text_with_rank_support) =
        I::construct_sampled_suffix_array_and_text_with_rank_support(
            &mut text,
            &mut maybe_bwt_buffer,
            &mut frequency_table,
            config,
            alphabet,
        );

    DataStructures {
        count,
        sampled_suffix_array,
        text_ids,
        text_with_rank_support,
    }
}

/// Types that can be used to store indices inside the FM-Index.
///
/// The maximum value of the type is an upper bound for the sum of lengths of indexed texts. Types with
/// larger maximum values allow indexing larger texts.
///
/// On the other hand, larger types lead to higher memory usage, especially during index
/// construction. Currently, there does not exist a suffix array construction backend for
/// `u32`, so it uses as much memory as `i64` during construction.
///
/// For example, to index the 3.3 GB large human genome, `u32` would be the best solution.
pub trait IndexStorage:
    PrimInt + Pod + maybe_savefile::MaybeSavefile + sealed::Sealed + Send + Sync + 'static
{
    #[doc(hidden)]
    type LibsaisOutput: OutputElement + IndexStorage;

    #[doc(hidden)]
    fn construct_libsais_suffix_array(
        text: &[u8],
        frequency_table: &mut [Self::LibsaisOutput],
    ) -> Vec<u8> {
        // allocate the buffer in bytes, because maybe we want to muck around with integer types later (compress i64 into u32)
        let mut suffix_array_bytes = vec![0u8; text.len() * size_of::<Self::LibsaisOutput>()];
        let suffix_array_buffer: &mut [Self::LibsaisOutput] =
            bytemuck::cast_slice_mut(&mut suffix_array_bytes);

        let mut construction = libsais::SuffixArrayConstruction::for_text(text)
            .in_borrowed_buffer(suffix_array_buffer)
            .multi_threaded(ThreadCount::fixed(
                rayon::current_num_threads()
                    .try_into()
                    .expect("Number of threads should fit into u16"),
            ));

        unsafe {
            construction = construction.with_frequency_table(frequency_table);
        }

        construction
            .run()
            .expect("libsais suffix array construction");

        suffix_array_bytes
    }

    #[doc(hidden)]
    fn construct_sampled_suffix_array_and_text_with_rank_support<
        'a,
        R: TextWithRankSupport<Self>,
    >(
        text: &'a mut Vec<u8>,
        maybe_bwt_buffer: &'a mut Vec<u8>,
        frequency_table: &mut [Self::LibsaisOutput],
        config: &FmIndexConfig<Self, R>,
        alphabet: &Alphabet,
    ) -> (SampledSuffixArray<Self>, R) {
        let suffix_array_bytes = Self::construct_libsais_suffix_array(text, frequency_table);
        let suffix_array_buffer: &[Self::LibsaisOutput] = bytemuck::cast_slice(&suffix_array_bytes);

        let (bwt, text_border_lookup, uncompressed_text_len) = bwt::bwt_from_suffix_array(
            suffix_array_buffer,
            text,
            maybe_bwt_buffer,
            config.performance_priority,
            alphabet,
        );

        let sampled_suffix_array = Self::sample_suffix_array_maybe_u32_compressed(
            suffix_array_bytes,
            config.suffix_array_sampling_rate,
            text_border_lookup,
        );

        let text_with_rank_support = construct_text_with_rank_support_maybe_slice_compressed(
            bwt,
            uncompressed_text_len,
            config.performance_priority,
            alphabet,
        );

        (sampled_suffix_array, text_with_rank_support)
    }

    #[doc(hidden)]
    fn sample_suffix_array_maybe_u32_compressed(
        suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: std::collections::HashMap<usize, Self>,
    ) -> SampledSuffixArray<Self> {
        SampledSuffixArray::new_uncompressed(suffix_array_bytes, sampling_rate, text_border_lookup)
    }
}

impl sealed::Sealed for i32 {}

impl IndexStorage for i32 {
    type LibsaisOutput = i32;
}

impl sealed::Sealed for u32 {}

impl IndexStorage for u32 {
    type LibsaisOutput = i64;

    #[cfg(feature = "u32-saca")]
    fn construct_sampled_suffix_array_and_text_with_rank_support<
        'a,
        R: TextWithRankSupport<Self>,
    >(
        text: &'a mut Vec<u8>,
        maybe_bwt_buffer: &'a mut Vec<u8>,
        frequency_table: &mut [Self::LibsaisOutput],
        config: &FmIndexConfig<Self, R>,
        alphabet: &Alphabet,
    ) -> (SampledSuffixArray<Self>, R) {
        let (sampled_suffix_array, bwt, uncompressed_text_len) = match config.performance_priority {
            PerformancePriority::HighSpeed | PerformancePriority::Balanced => {
                let suffix_array_bytes =
                    Self::construct_libsais_suffix_array(text, frequency_table);
                let suffix_array_buffer: &[Self::LibsaisOutput] =
                    bytemuck::cast_slice(&suffix_array_bytes);

                let (bwt, text_border_lookup, uncompressed_text_len) = bwt::bwt_from_suffix_array(
                    suffix_array_buffer,
                    text,
                    maybe_bwt_buffer,
                    config.performance_priority,
                    alphabet,
                );

                let sampled_suffix_array = Self::sample_suffix_array_maybe_u32_compressed(
                    suffix_array_bytes,
                    config.suffix_array_sampling_rate,
                    text_border_lookup,
                );

                (sampled_suffix_array, bwt, uncompressed_text_len)
            }
            PerformancePriority::LowMemory => {
                let mut suffix_array_bytes = vec![0u8; text.len() * size_of::<Self>()];
                let suffix_array_buffer: &mut [Self] =
                    bytemuck::cast_slice_mut(&mut suffix_array_bytes);

                sais_drum::SaisBuilder::new()
                    .with_max_char((alphabet.num_dense_symbols() - 1) as u8)
                    .construct_suffix_array_inplace(text, suffix_array_buffer);

                let (bwt, text_border_lookup, uncompressed_text_len) = bwt::bwt_from_suffix_array(
                    suffix_array_buffer,
                    text,
                    maybe_bwt_buffer,
                    config.performance_priority,
                    alphabet,
                );

                // NOT call Self::sample_suffix_array_maybe_u32_compressed, because after using u32 sais-drum
                // the suffix array does not need ot be compressed
                let sampled_suffix_array = SampledSuffixArray::new_uncompressed(
                    suffix_array_bytes,
                    config.suffix_array_sampling_rate,
                    text_border_lookup,
                );

                (sampled_suffix_array, bwt, uncompressed_text_len)
            }
        };

        let text_with_rank_support = construct_text_with_rank_support_maybe_slice_compressed(
            bwt,
            uncompressed_text_len,
            config.performance_priority,
            alphabet,
        );

        (sampled_suffix_array, text_with_rank_support)
    }

    fn sample_suffix_array_maybe_u32_compressed(
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
}

pub(crate) fn create_concatenated_densely_encoded_text<I: OutputElement, T: AsRef<[u8]>>(
    texts: impl IntoIterator<Item = T>,
    alphabet: &Alphabet,
) -> (Vec<u8>, Vec<I>, Vec<usize>) {
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

    // add one extra capacity to make sure that there does not need to be reallocation when on byte is added to the
    // text to make its size even for the slice compression in the lower memory mode for small alphabets
    let mut concatenated_text = vec![0; needed_capacity + 1];
    concatenated_text.pop();

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
            let mut frequency_table = vec![I::zero(); 256];

            for (source, target) in text.iter().zip(concatenated_text_split) {
                *target = alphabet.io_to_dense_representation(*source);
                frequency_table[*target as usize] = frequency_table[*target as usize] + I::one();
            }

            frequency_table
        })
        .reduce_with(merge_frequency_tables)
        .expect("There should be at least one texts");

    frequency_table[0] = <I as NumCast>::from(num_texts).unwrap();

    (concatenated_text, frequency_table, sentinel_indices)
}

fn merge_frequency_tables<I: OutputElement>(mut f1: Vec<I>, f2: Vec<I>) -> Vec<I> {
    for (x1, x2) in f1.iter_mut().zip(f2) {
        *x1 = *x1 + x2;
    }

    f1
}

fn frequency_table_to_count<I: OutputElement>(
    frequency_table: &[I],
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

fn construct_text_with_rank_support_maybe_slice_compressed<
    I: IndexStorage,
    R: TextWithRankSupport<I>,
>(
    bwt: &[u8],
    uncompressed_bwt_len: usize,
    performance_priority: PerformancePriority,
    alphabet: &Alphabet,
) -> R {
    if should_not_use_slice_compression(performance_priority, alphabet) {
        R::construct_from_maybe_slice_compressed_text::<NoSliceCompression>(
            bwt,
            uncompressed_bwt_len,
            alphabet.num_dense_symbols(),
        )
    } else {
        R::construct_from_maybe_slice_compressed_text::<HalfBytesCompression>(
            bwt,
            uncompressed_bwt_len,
            alphabet.num_dense_symbols(),
        )
    }
}

fn should_not_use_slice_compression(
    performance_priority: PerformancePriority,
    alphabet: &Alphabet,
) -> bool {
    performance_priority == PerformancePriority::HighSpeed || alphabet.num_dense_symbols() > 16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alphabet;

    #[test]
    fn concat_text() {
        let texts = [b"cccaaagggttt".as_slice(), b"acgtacgtacgt"];
        let alph = alphabet::ascii_dna();
        let (text, frequency_table, sentinel_indices) =
            create_concatenated_densely_encoded_text::<i32, _>(texts, &alph);

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
