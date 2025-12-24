use std::ops::Range;

use crate::{
    IndexStorage, TextWithRankSupport, batch_computed_cursors::Buffers,
    construction::slice_compression::SliceCompression, maybe_mem_dbg::MaybeMemDbg,
    maybe_savefile::MaybeSavefile, sealed::Sealed,
};

use super::{
    block::{Block, Block64},
    prefetch_index,
};

use num_traits::{NumCast, PrimInt};
use rayon::prelude::*;

// Interleaved means that the respective values for different symbols of the alphabet
// for the same text position are next to each other.
// Blocks must be interleaved for efficient queries.
// (Super)block offsets are only interleaved for faster (parallel) construction.

/// The more memory-efficient implementation of [`TextWithRankSupport`].
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemSize, mem_dbg::MemDbg))]
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[cfg_attr(feature = "savefile", savefile_doc_hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CondensedTextWithRankSupport<I, B = Block64> {
    text_len: usize,
    alphabet_size: usize,
    interleaved_blocks: Vec<B>,
    interleaved_block_offsets: Vec<u16>,
    interleaved_superblock_offsets: Vec<I>,
}

impl<I: IndexStorage, B: Block> CondensedTextWithRankSupport<I, B> {
    #[inline(always)]
    fn superblock_offset_idx(&self, symbol: u8, idx: usize) -> usize {
        let superblock_size = u16::MAX as usize + 1;
        (idx / superblock_size) * self.alphabet_size + symbol as usize
    }

    #[inline(always)]
    fn block_offset_idx(&self, symbol: u8, idx: usize) -> usize {
        (idx / B::NUM_BITS) * self.alphabet_size + symbol as usize
    }

    #[inline(always)]
    fn block_range(&self, idx: usize) -> Range<usize> {
        let alphabet_num_bits = ilog2_ceil_for_nonzero(self.alphabet_size);
        let interleaved_blocks_start = (idx / B::NUM_BITS) * alphabet_num_bits;
        let interleaved_blocks_end = interleaved_blocks_start + alphabet_num_bits;
        interleaved_blocks_start..interleaved_blocks_end
    }
}

impl<I: IndexStorage, B: Block> MaybeMemDbg for CondensedTextWithRankSupport<I, B> {}

impl<I: IndexStorage, B: Block> MaybeSavefile for CondensedTextWithRankSupport<I, B> {}

impl<I: IndexStorage, B: Block> Sealed for CondensedTextWithRankSupport<I, B> {}

impl<I: IndexStorage, B: Block> super::PrivateTextWithRankSupport<I>
    for CondensedTextWithRankSupport<I, B>
{
    #[inline(always)]
    fn construct_from_maybe_slice_compressed_text<S: SliceCompression>(
        text: &[u8],
        uncompressed_text_len: usize,
        alphabet_size: usize,
    ) -> Self {
        assert!(alphabet_size >= 2);

        let alphabet_num_bits = ilog2_ceil_for_nonzero(alphabet_size);

        // we might be storing one character b'1' to many if the text is half byte compressed and had odd length.
        let len: usize = S::transformed_slice_len(text) + 1;
        let superblock_size = u16::MAX as usize + 1;

        let num_indicator_blocks = len.div_ceil(B::NUM_BITS) * alphabet_num_bits;
        let num_block_offsets = len.div_ceil(B::NUM_BITS) * alphabet_size;
        let num_superblock_offsets = len.div_ceil(superblock_size) * alphabet_size;

        let mut interleaved_blocks = vec![B::zeroes(); num_indicator_blocks];
        let mut interleaved_block_offsets = vec![0; num_block_offsets];
        let mut interleaved_superblock_offsets = vec![I::zero(); num_superblock_offsets];

        let num_blocks_per_superblock = (superblock_size / B::NUM_BITS) * alphabet_num_bits;
        let blocks_per_superblock_iter =
            interleaved_blocks.par_chunks_mut(num_blocks_per_superblock);

        let num_block_offsets_per_superblock = (superblock_size / B::NUM_BITS) * alphabet_size;
        let block_offsets_per_superblock_iter =
            interleaved_block_offsets.par_chunks_mut(num_block_offsets_per_superblock);

        let superblock_offsets_iter = interleaved_superblock_offsets.par_chunks_mut(alphabet_size);

        let text_chunk_size = S::transform_chunk_size(superblock_size);
        let text_superblock_iter = text.par_chunks(text_chunk_size);

        let interleaved_superblock_iter = (
            text_superblock_iter,
            superblock_offsets_iter,
            block_offsets_per_superblock_iter,
            blocks_per_superblock_iter,
        )
            .into_par_iter();

        interleaved_superblock_iter
            .for_each(|tup| fill_superblock::<I, B, S>(tup.0, tup.1, tup.2, tup.3, alphabet_size));

        // accumulate superblocks in single thread
        let mut temp_offsets = vec![I::zero(); alphabet_size];
        let mut sum_of_previous = vec![I::zero(); alphabet_size];

        for superblock_offsets in interleaved_superblock_offsets.chunks_mut(alphabet_size) {
            temp_offsets.copy_from_slice(superblock_offsets);
            superblock_offsets.copy_from_slice(&sum_of_previous);

            for (sum, temp) in sum_of_previous.iter_mut().zip(&temp_offsets) {
                *sum = *sum + *temp;
            }
        }

        Self {
            text_len: uncompressed_text_len,
            alphabet_size,
            interleaved_blocks,
            interleaved_block_offsets,
            interleaved_superblock_offsets,
        }
    }

    #[inline(always)]
    fn _alphabet_size(&self) -> usize {
        self.alphabet_size
    }

    #[inline(always)]
    fn _text_len(&self) -> usize {
        self.text_len
    }

    // TODO: maybe refactor this to get rid of all of the doubling for start and end of intervals
    // this functions essentially does the same thing as Self::rank_unchecked for all of the
    // intervals border in the buffers struct
    unsafe fn replace_many_interval_borders_with_ranks_unchecked<Q, const N: usize>(
        &self,
        buffers: &mut Buffers<Q, N>,
        num_remaining_unfinished_queries: usize,
    ) {
        // SAFETY: all of the index accesses are in the valid range if idx is at most text.len()
        // and since the alphabet has a size of at least 2

        // I hope the compiler removes all of the bounds checks in the buffers
        assert!(num_remaining_unfinished_queries <= N);

        let symbols = &buffers.symbols;
        let intervals = &mut buffers.intervals;
        let superblock_offsets_starts = &mut buffers.buffer1;
        let superblock_offsets_ends = &mut buffers.buffer2;
        let block_offsets_starts = &mut buffers.buffer3;
        let block_offsets_ends = &mut buffers.buffer4;

        // temporarily store superblock offset indices in the buffers
        for i in 0..num_remaining_unfinished_queries {
            superblock_offsets_starts[i] =
                self.superblock_offset_idx(symbols[i], intervals[i].start);
            superblock_offsets_ends[i] = self.superblock_offset_idx(symbols[i], intervals[i].end);
        }

        // now replace indices by values
        // SAFETY: must succeed, otherwise the construction function would have crashed
        for i in 0..num_remaining_unfinished_queries {
            let superblock_offset_start = unsafe {
                *self
                    .interleaved_superblock_offsets
                    .get_unchecked(superblock_offsets_starts[i])
            };
            superblock_offsets_starts[i] =
                unsafe { <usize as NumCast>::from(superblock_offset_start).unwrap_unchecked() };

            let superblock_offset_end = unsafe {
                *self
                    .interleaved_superblock_offsets
                    .get_unchecked(superblock_offsets_ends[i])
            };

            superblock_offsets_ends[i] =
                unsafe { <usize as NumCast>::from(superblock_offset_end).unwrap_unchecked() };
        }

        // temporarily store block offset indices in the buffers
        for i in 0..num_remaining_unfinished_queries {
            block_offsets_starts[i] = self.block_offset_idx(symbols[i], intervals[i].start);
            block_offsets_ends[i] = self.block_offset_idx(symbols[i], intervals[i].end);
        }

        // now replace indices by values
        // SAFETY: must succeed, otherwise the construction function would have crashed
        for i in 0..num_remaining_unfinished_queries {
            block_offsets_starts[i] = unsafe {
                *self
                    .interleaved_block_offsets
                    .get_unchecked(block_offsets_starts[i])
            } as usize;

            block_offsets_ends[i] = unsafe {
                *self
                    .interleaved_block_offsets
                    .get_unchecked(block_offsets_ends[i])
            } as usize;
        }

        let mut block_slices_starts: [Option<&[B]>; N] = [None; N];
        let mut block_slices_ends: [Option<&[B]>; N] = [None; N];
        let mut accumulator_blocks_starts: [B; N] = [B::zeroes(); N];
        let mut accumulator_blocks_ends: [B; N] = [B::zeroes(); N];

        for i in 0..num_remaining_unfinished_queries {
            let block_range_start = self.block_range(intervals[i].start);
            block_slices_starts[i] =
                Some(unsafe { self.interleaved_blocks.get_unchecked(block_range_start) });

            let block_range_end = self.block_range(intervals[i].end);
            block_slices_ends[i] =
                Some(unsafe { self.interleaved_blocks.get_unchecked(block_range_end) });
        }

        // SAFETY: first unwrap_unchecked: this option was set to Some() in the above loop
        // second unwrap_unchecked: there must be at least one block, because the alphabet size is at least 2
        for i in 0..num_remaining_unfinished_queries {
            let (first_block_start, other_blocks_start) = unsafe {
                block_slices_starts[i]
                    .unwrap_unchecked()
                    .split_first()
                    .unwrap_unchecked()
            };

            accumulator_blocks_starts[i] = *first_block_start;
            block_slices_starts[i] = Some(other_blocks_start);

            let (first_block_end, other_blocks_end) = unsafe {
                block_slices_ends[i]
                    .unwrap_unchecked()
                    .split_first()
                    .unwrap_unchecked()
            };

            accumulator_blocks_ends[i] = *first_block_end;
            block_slices_ends[i] = Some(other_blocks_end);
        }

        for i in 0..num_remaining_unfinished_queries {
            let mut symbol = symbols[i];
            let accumulator_block_start = &mut accumulator_blocks_starts[i];
            let accumulator_block_end = &mut accumulator_blocks_ends[i];

            if symbol & 1 == 0 {
                accumulator_block_start.negate();
                accumulator_block_end.negate();
            }

            // SAFETY: the options were just set to Some() above
            let block_slices_start = unsafe { block_slices_starts[i].unwrap_unchecked() };
            let block_slices_end = unsafe { block_slices_ends[i].unwrap_unchecked() };

            for (mut block_start, mut block_end) in block_slices_start
                .iter()
                .copied()
                .zip(block_slices_end.iter().copied())
            {
                symbol >>= 1;

                if symbol & 1 == 0 {
                    block_start.negate();
                    block_end.negate();
                }

                accumulator_block_start.set_to_self_and(block_start);
                accumulator_block_end.set_to_self_and(block_end);
            }
        }

        for i in 0..num_remaining_unfinished_queries {
            let idx_in_block_start = intervals[i].start % B::NUM_BITS;
            let idx_in_block_end = intervals[i].end % B::NUM_BITS;

            let block_count_start =
                accumulator_blocks_starts[i].count_ones_before(idx_in_block_start);
            let block_count_end = accumulator_blocks_ends[i].count_ones_before(idx_in_block_end);

            intervals[i].start =
                superblock_offsets_starts[i] + block_offsets_starts[i] + block_count_start;
            intervals[i].end = superblock_offsets_ends[i] + block_offsets_ends[i] + block_count_end;
        }
    }
}

impl<I: IndexStorage, B: Block> TextWithRankSupport<I> for CondensedTextWithRankSupport<I, B> {
    #[inline(always)]
    unsafe fn rank_unchecked(&self, mut symbol: u8, idx: usize) -> usize {
        // SAFETY: all of the index accesses are in the valid range if idx is at most text.len()
        // and since the alphabet has a size of at least 2
        let superblock_offset_idx = self.superblock_offset_idx(symbol, idx);

        let superblock_offset = unsafe {
            *self
                .interleaved_superblock_offsets
                .get_unchecked(superblock_offset_idx)
        };

        // SAFETY: must succeed, otherwise the construction function would have crashed
        let superblock_offset =
            unsafe { <usize as NumCast>::from(superblock_offset).unwrap_unchecked() };

        let block_offset_idx = self.block_offset_idx(symbol, idx);
        let block_offset = unsafe {
            *self
                .interleaved_block_offsets
                .get_unchecked(block_offset_idx)
        } as usize;

        let block_range = self.block_range(idx);

        let interleaved_blocks = unsafe { self.interleaved_blocks.get_unchecked(block_range) };

        // SAFETY: there must be at least one block, because the alphabet size is at least 2
        let (first_block, other_blocks) =
            unsafe { interleaved_blocks.split_first().unwrap_unchecked() };

        let mut accumulator_block = *first_block;

        if symbol & 1 == 0 {
            accumulator_block.negate();
        }

        for mut block in other_blocks.iter().copied() {
            symbol >>= 1;

            if symbol & 1 == 0 {
                block.negate();
            }

            accumulator_block.set_to_self_and(block);
        }

        let index_in_block = idx % B::NUM_BITS;
        let block_count = accumulator_block.count_ones_before(index_in_block);

        superblock_offset + block_offset + block_count
    }

    #[inline(always)]
    fn prefetch(&self, symbol: u8, idx: usize) {
        let superblock_offset_idx = self.superblock_offset_idx(symbol, idx);
        prefetch_index(&self.interleaved_superblock_offsets, superblock_offset_idx);

        let block_offset_idx = self.block_offset_idx(symbol, idx);
        prefetch_index(&self.interleaved_block_offsets, block_offset_idx);

        let block_range = self.block_range(idx);
        prefetch_index(&self.interleaved_blocks, block_range.start);
    }

    #[inline(always)]
    fn symbol_at(&self, idx: usize) -> u8 {
        assert!(idx < self.text_len);

        let alphabet_num_bits = ilog2_ceil_for_nonzero(self.alphabet_size);
        let blocks_start = (idx / B::NUM_BITS) * alphabet_num_bits;
        let blocks_end = blocks_start + alphabet_num_bits;

        let blocks = &self.interleaved_blocks[blocks_start..blocks_end];

        let index_in_block = idx % B::NUM_BITS;

        let mut symbol = 0;

        for (i, block) in blocks.iter().enumerate() {
            let block_bit = block.get_bit(index_in_block);
            symbol |= block_bit << i;
        }

        symbol
    }
}

fn fill_superblock<I: PrimInt, B: Block, S: SliceCompression>(
    text: &[u8],
    interleaved_superblock_offsets: &mut [I],
    interleaved_block_offsets: &mut [u16],
    interleaved_blocks: &mut [B],
    alphabet_size: usize,
) {
    let alphabet_num_bits = ilog2_ceil_for_nonzero(alphabet_size);
    let mut block_offsets_sum = vec![0; alphabet_size];

    let text_chunk_size = S::transform_chunk_size(B::NUM_BITS);
    let text_block_iter = text.chunks(text_chunk_size);
    let block_offsets_iter = interleaved_block_offsets.chunks_mut(alphabet_size);
    let blocks_iter = interleaved_blocks.chunks_mut(alphabet_num_bits);

    let blocks_overshoot = text_block_iter.len() < blocks_iter.len();

    let block_package_iter = text_block_iter.zip(block_offsets_iter).zip(blocks_iter);

    for ((text_block, block_offsets), blocks) in block_package_iter {
        block_offsets.copy_from_slice(&block_offsets_sum);

        for (index_in_block, mut symbol) in S::iter(text_block).enumerate() {
            let symbol_usize = <usize as NumCast>::from(symbol).unwrap();

            let superblock_count = &mut interleaved_superblock_offsets[symbol_usize];
            *superblock_count = *superblock_count + I::one();

            block_offsets_sum[symbol_usize] += 1;

            for block in blocks.iter_mut() {
                block.set_bit_assuming_zero(index_in_block, symbol & 1);
                symbol >>= 1;
            }
        }
    }

    // annoying edge case, because the bit array we're storing is text.len() + 1 large
    if blocks_overshoot {
        interleaved_block_offsets
            .rchunks_mut(alphabet_size)
            .next()
            .unwrap()
            .copy_from_slice(&block_offsets_sum);
    }
}

#[inline(always)]
fn ilog2_ceil_for_nonzero(value: usize) -> usize {
    usize::BITS as usize - value.leading_zeros() as usize - value.is_power_of_two() as usize
}
