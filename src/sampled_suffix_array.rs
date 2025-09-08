use bytemuck::Pod;
use libsais::OutputElement;
use num_traits::{NumCast, PrimInt};

use std::{collections::HashMap, marker::PhantomData, ops::Range};

use crate::{alphabet::Alphabet, text_with_rank_support::Block};

use super::FmIndex;

#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub(crate) struct SampledSuffixArray<I> {
    suffix_array_bytes: Vec<u8>,
    text_border_lookup: HashMap<usize, I>,
    sampling_rate: usize,
    _compression_marker: PhantomData<I>,
}

impl<I: OutputElement> SampledSuffixArray<I> {
    pub(crate) fn new_uncompressed(
        mut suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: HashMap<usize, I>,
    ) -> Self {
        let suffix_array_view: &mut [I] = bytemuck::cast_slice_mut(&mut suffix_array_bytes);

        let mut num_retained_values = 0;
        let mut write_index = 0;

        for i in 0..suffix_array_view.len() {
            if i % sampling_rate == 0 {
                suffix_array_view[write_index] = suffix_array_view[i];
                write_index += 1;
                num_retained_values += 1;
            }
        }

        suffix_array_bytes.truncate(num_retained_values * size_of::<I>());
        suffix_array_bytes.shrink_to_fit();

        Self {
            suffix_array_bytes,
            text_border_lookup,
            sampling_rate,
            _compression_marker: PhantomData,
        }
    }
}

impl SampledSuffixArray<u32> {
    pub(crate) fn new_u32_compressed(
        mut suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
        text_border_lookup: HashMap<usize, u32>,
    ) -> Self {
        let suffix_array_view: &mut [i64] = bytemuck::cast_slice_mut(&mut suffix_array_bytes);

        let mut num_retained_values: usize = 0;

        let mut write_index = 0;
        let mut next_write_is_little_half = true;

        for i in 0..suffix_array_view.len() {
            if i % sampling_rate == 0 {
                let read_entry_bytes = suffix_array_view[i].to_le_bytes();

                if next_write_is_little_half {
                    let mut new_write_entry_bytes = [0; 8];
                    new_write_entry_bytes[0..4].copy_from_slice(&read_entry_bytes[0..4]);

                    suffix_array_view[write_index] = i64::from_le_bytes(new_write_entry_bytes);

                    next_write_is_little_half = false;
                } else {
                    let mut existing_bytes = suffix_array_view[write_index].to_le_bytes();
                    existing_bytes[4..8].copy_from_slice(&read_entry_bytes[0..4]);

                    suffix_array_view[write_index] = i64::from_le_bytes(existing_bytes);

                    next_write_is_little_half = true;
                    write_index += 1;
                }

                num_retained_values += 1;
            }
        }

        suffix_array_bytes.truncate(num_retained_values * size_of::<u32>());
        suffix_array_bytes.shrink_to_fit();

        Self {
            suffix_array_bytes,
            text_border_lookup,
            sampling_rate,
            _compression_marker: PhantomData,
        }
    }
}

impl<I: PrimInt + Pod> SampledSuffixArray<I> {
    pub(crate) fn recover_range<A: Alphabet, B: Block>(
        &self,
        range: Range<usize>,
        index: &FmIndex<A, I, B>,
    ) -> impl Iterator<Item = usize> {
        range.map(|mut i| {
            let mut num_steps_done = I::zero();

            while i % self.sampling_rate != 0 {
                let bwt_symbol = index.text_with_rank_support.symbol_at(i);

                if bwt_symbol == 0 {
                    return <usize as NumCast>::from(self.text_border_lookup[&i] + num_steps_done)
                        .unwrap();
                }

                i = index.lf_mapping_step(bwt_symbol, i);

                num_steps_done = num_steps_done + I::one();
            }

            let suffix_array_view: &[I] = bytemuck::cast_slice(&self.suffix_array_bytes);

            <usize as NumCast>::from(suffix_array_view[i / self.sampling_rate] + num_steps_done)
                .unwrap()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{FmIndexI32, alphabet::AsciiDnaWithN};
    use proptest::prelude::*;

    fn copied_and_recovered_array_must_equal<T: AsRef<[u8]>>(texts: &[T], sampling_rate: usize) {
        let n: usize = texts.iter().map(|t| t.as_ref().len()).sum();

        let sampled_index = FmIndexI32::<AsciiDnaWithN>::new(texts, 1, sampling_rate);
        let index = FmIndexI32::<AsciiDnaWithN>::new(texts, 1, 1);

        let recovered_array: Vec<_> = sampled_index
            .suffix_array
            .recover_range(0..n, &sampled_index)
            .collect();
        let copied_array: Vec<_> = index.suffix_array.recover_range(0..n, &index).collect();

        assert_eq!(copied_array, recovered_array);
    }

    #[test]
    fn walking_over_text_borders() {
        let texts = [
            [65].as_slice(),
            [].as_slice(),
            [78, 84, 78, 78, 84, 78, 78, 84, 78].as_slice(),
        ];

        let sampling_rate = 5;

        copied_and_recovered_array_must_equal(&texts, sampling_rate);
    }

    proptest! {
        // default is 256 and I'd like some more test cases that need to pass
        #![proptest_config(ProptestConfig::with_cases(2048))]

        #[test]
        fn correctness_random_texts(
            texts in prop::collection::vec(
            prop::collection::vec((0usize..5).prop_map(|i| b"ACGTN"[i]), 0..1500),
            1..5
        ),
            sampling_rate in 1usize..=8
        ) {
            copied_and_recovered_array_must_equal(&texts, sampling_rate);
        }
    }
}
