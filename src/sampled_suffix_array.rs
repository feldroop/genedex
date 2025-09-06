use bytemuck::Pod;
use libsais::OutputElement;
use num_traits::{NumCast, PrimInt};

use std::{marker::PhantomData, ops::Range};

use crate::{alphabet::Alphabet, text_with_rank_support::Block};

use super::FmIndex;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct SampledSuffixArray<I> {
    suffix_array_bytes: Vec<u8>,
    sampling_rate: usize,
    _compression_marker: PhantomData<I>,
}

impl<I: OutputElement> SampledSuffixArray<I> {
    pub(crate) fn new_uncompressed(mut suffix_array_bytes: Vec<u8>, sampling_rate: usize) -> Self {
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
            sampling_rate,
            _compression_marker: PhantomData,
        }
    }
}

impl SampledSuffixArray<u32> {
    pub(crate) fn new_u32_compressed(
        mut suffix_array_bytes: Vec<u8>,
        sampling_rate: usize,
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
            sampling_rate,
            _compression_marker: PhantomData,
        }
    }
}

impl<I: PrimInt + Pod> SampledSuffixArray<I> {
    pub(crate) fn recover_range_uncompressed<A: Alphabet, B: Block>(
        &self,
        range: Range<usize>,
        index: &FmIndex<A, I, B>,
    ) -> impl Iterator<Item = usize> {
        range.map(|mut i| {
            let mut num_steps_done = I::zero();

            while i % self.sampling_rate != 0 {
                let bwt_rank = index.text_with_rank_support.symbol_at(i);
                i = index.lf_mapping_step(bwt_rank, i);
                num_steps_done = num_steps_done + I::one();
            }

            let suffix_array_view: &[I] = bytemuck::cast_slice(&self.suffix_array_bytes);
            <usize as NumCast>::from(suffix_array_view[i / self.sampling_rate] + num_steps_done)
                .unwrap()
                % index.text_with_rank_support.text_len()
        })
    }
}
