use super::slice_compression::{
    HalfBytesCompression, NoSliceCompression, SliceCompression, half_byte_compress_text,
};
use crate::{Alphabet, IndexStorage, config::PerformancePriority};
use num_traits::NumCast;
use rayon::prelude::*;
use std::collections::HashMap;

// I1: current_suffix array indices, I2: IndexStorage we want to use for the FM-Index
pub(crate) fn bwt_from_suffix_array<'a, I1: IndexStorage, I2: IndexStorage>(
    suffix_array: &[I1],
    text: &'a mut Vec<u8>,
    maybe_bwt_buffer: &'a mut Vec<u8>,
    performance_priority: PerformancePriority,
    alphabet: &Alphabet,
) -> (&'a [u8], HashMap<usize, I2>, usize) {
    let uncompressed_text_len = text.len();

    if super::should_not_use_slice_compression(performance_priority, alphabet) {
        maybe_bwt_buffer.resize(text.len(), 0);

        let text_border_lookup = bwt_from_suffix_array_maybe_slice_compressed::<
            NoSliceCompression,
            _,
            _,
        >(
            suffix_array, text, maybe_bwt_buffer, uncompressed_text_len
        );

        return (maybe_bwt_buffer, text_border_lookup, uncompressed_text_len);
    }

    // make sure the text buffer has an even size.
    if text.len() % 2 == 1 {
        // is not not sentinel and always part of the alphabet (dense representation)
        text.push(1);
    }

    half_byte_compress_text(text);

    let half = text.len() / 2;
    let (text, bwt) = text.split_at_mut(half);

    let text_border_lookup = bwt_from_suffix_array_maybe_slice_compressed::<
        HalfBytesCompression,
        _,
        _,
    >(suffix_array, text, bwt, uncompressed_text_len);

    (bwt, text_border_lookup, uncompressed_text_len)
}

// I1: current_suffix array indices, I2: IndexStorage we want to use for the FM-Index
fn bwt_from_suffix_array_maybe_slice_compressed<
    S: SliceCompression,
    I1: IndexStorage,
    I2: IndexStorage,
>(
    suffix_array: &[I1],
    text: &[u8],
    bwt: &mut [u8],
    uncompressed_text_len: usize,
) -> HashMap<usize, I2> {
    // collecting the text border lookup values while constructing the BWT made the function
    // run much slower. this two-level chunk scheme leads to the same performance as before

    let mut outer_chunk_size =
        std::cmp::max(text.len().div_ceil(rayon::current_num_threads() * 4), 2);
    // make sure that chunk size is even for the case case of a half byte compressed text/bwt. in that case,
    // the chunk size is divided by half and that division has to work without remainder
    if outer_chunk_size % 2 == 1 {
        outer_chunk_size += 1;
    }
    let inner_chunk_size = 128;

    let bwt_outer_chunk_size = S::transform_chunk_size(outer_chunk_size);
    let bwt_inner_chunk_size = S::transform_chunk_size(inner_chunk_size);

    suffix_array
        .par_chunks(outer_chunk_size)
        .zip(bwt.par_chunks_mut(bwt_outer_chunk_size))
        .enumerate()
        .map(
            |(outer_chunk_idx, (outer_suffix_array_chunk, outer_bwt_chunk))| {
                let mut text_border_lookup = HashMap::new();

                for (inner_chunk_idx, (inner_suffix_array_chunk, inner_bwt_chunk)) in
                    outer_suffix_array_chunk
                        .chunks(inner_chunk_size)
                        .zip(outer_bwt_chunk.chunks_mut(bwt_inner_chunk_size))
                        .enumerate()
                {
                    for (inner_suffix_array_chunk_idx, &text_idx) in
                        inner_suffix_array_chunk.iter().enumerate()
                    {
                        let text_index_usize = <usize as NumCast>::from(text_idx).unwrap();

                        let text_index_usize = if text_index_usize > 0 {
                            text_index_usize
                        } else {
                            uncompressed_text_len
                        };

                        let text_value = S::get(text_index_usize - 1, text);
                        S::set(inner_suffix_array_chunk_idx, inner_bwt_chunk, text_value);
                    }

                    for i in S::iter_zero_indices(inner_bwt_chunk) {
                        let suffix_array_index = outer_chunk_size * outer_chunk_idx
                            + inner_chunk_size * inner_chunk_idx
                            + i;

                        let text_index =
                            <I2 as NumCast>::from(inner_suffix_array_chunk[i]).unwrap();
                        text_border_lookup.insert(suffix_array_index, text_index);
                    }
                }

                text_border_lookup
            },
        )
        .reduce_with(|mut m0, m1| {
            for (key, value) in m1.into_iter() {
                m0.insert(key, value);
            }
            m0
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::construction::slice_compression::{
        HalfBytesCompression, NoSliceCompression, half_byte_compress_text,
    };
    use proptest::prelude::*;

    prop_compose! {
        fn even_len_text()
            (text_len in (0usize..1500).prop_map(|len| len * 2))
            (text in prop::collection::vec(0u8..16, text_len)) -> Vec<u8> {
                text
        }
    }

    proptest! {
        // default is 256 and I'd like some more test cases that need to pass
        #![proptest_config(ProptestConfig::with_cases(2048))]

        #[test]
        fn correctness_random_texts(text in even_len_text()) {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build()
                .unwrap();

            pool.install(
                || {
                    let mut bwt = vec![0; text.len()];

                    let mut text_copy = text.clone();
                    half_byte_compress_text(&mut text_copy);
                    let half = text.len() / 2;
                    let (text_compressed, bwt_compressed) = text_copy.split_at_mut(half);

                    let suffix_array = libsais::SuffixArrayConstruction::for_text(&text)
                        .in_owned_buffer32()
                        .single_threaded()
                        .run()
                        .unwrap()
                        .into_vec();

                    let text_border_lookup = bwt_from_suffix_array_maybe_slice_compressed::<NoSliceCompression, i32, i32>(&suffix_array, &text, &mut bwt, text.len());
                    let text_border_lookup_compressed = bwt_from_suffix_array_maybe_slice_compressed::<HalfBytesCompression, i32, i32>(&suffix_array, text_compressed, bwt_compressed, text.len());

                    let bwt_recovered: Vec<_> = HalfBytesCompression::iter(bwt_compressed).collect();

                    assert_eq!(bwt, bwt_recovered);
                    assert_eq!(text_border_lookup, text_border_lookup_compressed);
                }
            );
        }
    }
}
