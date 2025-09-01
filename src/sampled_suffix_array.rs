use num_traits::{NumCast, PrimInt};

use std::ops::Range;

use super::FmIndex;

pub(crate) struct SampledSuffixArray<O> {
    data: Vec<O>,
    sampling_rate: usize,
    is_u32_compressed: bool,
}

impl<O: PrimInt + 'static> SampledSuffixArray<O> {
    pub(crate) fn new(mut full_suffix_array: Vec<O>, sampling_rate: usize) -> Self {
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
            is_u32_compressed: false,
        }
    }

    pub(crate) fn is_u32_compressed(&self) -> bool {
        self.is_u32_compressed
    }
}

impl SampledSuffixArray<i64> {
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
            is_u32_compressed: true,
        }
    }
}

impl<O: PrimInt + 'static> SampledSuffixArray<O> {
    pub(crate) fn recover_range(
        &self,
        range: Range<usize>,
        index: &FmIndex<O>,
    ) -> impl Iterator<Item = O> {
        range.map(|mut i| {
            let mut num_steps_done = O::zero();

            while i % self.sampling_rate != 0 {
                let bwt_char = index.occurrence_table.bwt_char_at(i);
                i = index.lf_mapping_step(i, bwt_char);
                num_steps_done = num_steps_done + O::one();
            }

            (self.data[i / self.sampling_rate] + num_steps_done)
                % <O as NumCast>::from(index.text_len).unwrap()
        })
    }
}

impl SampledSuffixArray<i64> {
    pub(crate) fn recover_range_u32_compressed(
        &self,
        range: Range<usize>,
        index: &FmIndex<i64>,
    ) -> impl Iterator<Item = u32> {
        range.map(|mut i| {
            let mut num_steps_done = 0;

            while i % self.sampling_rate != 0 {
                let bwt_char = index.occurrence_table.bwt_char_at(i);
                i = index.lf_mapping_step(i, bwt_char);
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

            (extracted_data + num_steps_done) % u32::try_from(index.text_len).unwrap()
        })
    }
}
