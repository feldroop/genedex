use num_traits::{NumCast, PrimInt};

use std::{marker::PhantomData, ops::Range};

use crate::{U32Compressed, Uncompressed, alphabet::Alphabet};

use super::FmIndex;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct SampledSuffixArray<S, C> {
    data: Vec<S>,
    sampling_rate: usize,
    _compression_marker: PhantomData<C>,
}

impl<S: PrimInt + 'static> SampledSuffixArray<S, Uncompressed> {
    pub(crate) fn new_uncompressed(mut full_suffix_array: Vec<S>, sampling_rate: usize) -> Self {
        let mut num_retained_values = 0;
        let mut write_index = 0;

        for i in 0..full_suffix_array.len() {
            if i % sampling_rate == 0 {
                full_suffix_array[write_index] = full_suffix_array[i];
                write_index += 1;
                num_retained_values += 1;
            }
        }

        full_suffix_array.truncate(num_retained_values);
        full_suffix_array.shrink_to_fit();

        Self {
            data: full_suffix_array,
            sampling_rate,
            _compression_marker: PhantomData,
        }
    }
}

impl SampledSuffixArray<i64, U32Compressed> {
    pub(crate) fn new_u32_compressed(
        mut full_suffix_array: Vec<i64>,
        sampling_rate: usize,
    ) -> Self {
        let mut num_retained_values: usize = 0;

        let mut write_index = 0;
        let mut next_write_is_little_half = true;

        for i in 0..full_suffix_array.len() {
            if i % sampling_rate == 0 {
                if next_write_is_little_half {
                    full_suffix_array[write_index] = full_suffix_array[i];
                    next_write_is_little_half = false;
                } else {
                    let read_bytes = full_suffix_array[i].to_le_bytes();
                    let mut existing_bytes = full_suffix_array[write_index].to_le_bytes();
                    existing_bytes[4..8].copy_from_slice(&read_bytes[0..4]);

                    full_suffix_array[write_index] = i64::from_le_bytes(existing_bytes);

                    next_write_is_little_half = true;
                    write_index += 1;
                }

                num_retained_values += 1;
            }
        }

        full_suffix_array.truncate(num_retained_values.div_ceil(2));
        full_suffix_array.shrink_to_fit();

        Self {
            data: full_suffix_array,
            sampling_rate,
            _compression_marker: PhantomData,
        }
    }
}

impl<S: PrimInt + 'static> SampledSuffixArray<S, Uncompressed> {
    pub(crate) fn recover_range_uncompressed<A: Alphabet>(
        &self,
        range: Range<usize>,
        index: &FmIndex<A, S, Uncompressed>,
    ) -> impl Iterator<Item = S> {
        range.map(|mut i| {
            let mut num_steps_done = S::zero();

            while i % self.sampling_rate != 0 {
                let bwt_rank = index.string_rank.symbol_at(i);
                i = index.lf_mapping_step(bwt_rank, i);
                num_steps_done = num_steps_done + S::one();
            }

            (self.data[i / self.sampling_rate] + num_steps_done)
                % <S as NumCast>::from(index.string_rank.len()).unwrap()
        })
    }
}

impl SampledSuffixArray<i64, U32Compressed> {
    pub(crate) fn recover_range_u32_compressed<A: Alphabet>(
        &self,
        range: Range<usize>,
        index: &FmIndex<A, i64, U32Compressed>,
    ) -> impl Iterator<Item = u32> {
        range.map(|mut i| {
            let mut num_steps_done = 0;

            while i % self.sampling_rate != 0 {
                let bwt_rank = index.string_rank.symbol_at(i);
                i = index.lf_mapping_step(bwt_rank, i);
                num_steps_done += 1;
            }

            let original_index = i / self.sampling_rate;
            let compressed_index = original_index / 2;

            let bytes = self.data[compressed_index].to_le_bytes();
            let extracted_data = if original_index % 2 == 0 {
                u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            } else {
                u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])
            };

            (extracted_data + num_steps_done) % u32::try_from(index.string_rank.len()).unwrap()
        })
    }
}
