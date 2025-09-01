use libsais::OutputElement;
use num_traits::NumCast;

use std::ops::Range;

use super::{FmIndex, OccurrenceTable};

pub(crate) struct SampledSuffixArray<O> {
    data: Vec<O>,
    sampling_rate: usize,
}

impl<O: OutputElement + 'static> SampledSuffixArray<O> {
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
        }
    }

    pub(crate) fn recover_range<Occ: OccurrenceTable>(
        &self,
        range: Range<usize>,
        index: &FmIndex<Occ, O>,
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
