use crate::IndexStorage;
use crate::batch_computed_cursors::Buffers;
use crate::construction::slice_compression::SliceCompression;
use crate::maybe_mem_dbg::MaybeMemDbg;
use crate::maybe_savefile::MaybeSavefile;
use crate::sealed::Sealed;

use super::TextWithRankSupport;
use super::block::{Block, Block64, NUM_BLOCK_OFFSET_BITS};

use num_traits::{NumCast, PrimInt};
use rayon::prelude::*;

// Interleaved means that the respective values for different symbols of the alphabet
// for the same text position are next to each other.
// Blocks must be interleaved for efficient queries.
// (Super)block offsets are only interleaved for faster (parallel) construction.

// This is equivalent to just storing alphabet_size many bitvector with rank support and integrating
// the block offsets in the bitvectors.

/// The faster implementation of [`TextWithRankSupport`].
#[cfg_attr(feature = "mem_dbg", derive(mem_dbg::MemSize, mem_dbg::MemDbg))]
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[cfg_attr(feature = "savefile", savefile_doc_hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatTextWithRankSupport<I, B = Block64> {
    text_len: usize,
    alphabet_size: usize,
    superblock_size: usize,
    interleaved_blocks: Vec<B>,
    interleaved_superblock_offsets: Vec<I>,
}

impl<I: IndexStorage, B: Block> FlatTextWithRankSupport<I, B> {
    fn superblock_offset_idx(&self, symbol: u8, idx: usize) -> usize {
        let symbol_usize = symbol as usize;
        (idx / self.superblock_size) * self.alphabet_size + symbol_usize
    }

    fn block_idx(&self, symbol: u8, idx: usize) -> usize {
        let symbol_usize = symbol as usize;
        let used_bits_per_block = B::NUM_BITS - NUM_BLOCK_OFFSET_BITS;
        (idx / used_bits_per_block) * self.alphabet_size + symbol_usize
    }

    fn idx_in_block(idx: usize) -> usize {
        let used_bits_per_block = B::NUM_BITS - NUM_BLOCK_OFFSET_BITS;
        idx % used_bits_per_block
    }
}

impl<I: IndexStorage, B: Block> MaybeMemDbg for FlatTextWithRankSupport<I, B> {}

impl<I: IndexStorage, B: Block> MaybeSavefile for FlatTextWithRankSupport<I, B> {}

impl<I: IndexStorage, B: Block> Sealed for FlatTextWithRankSupport<I, B> {}

impl<I: IndexStorage, B: Block> super::PrivateTextWithRankSupport<I>
    for FlatTextWithRankSupport<I, B>
{
    fn construct_from_maybe_slice_compressed_text<S: SliceCompression>(
        text: &[u8],
        uncompressed_text_len: usize,
        alphabet_size: usize,
    ) -> Self {
        assert!(alphabet_size >= 2);

        // we might be storing one character b'1' to many if the text is half byte compressed and had odd length.
        let len: usize = S::transformed_slice_len(text) + 1;
        let used_bits_per_block = B::NUM_BITS - NUM_BLOCK_OFFSET_BITS;

        let max_superblock_size = 1 << NUM_BLOCK_OFFSET_BITS;
        let superblock_size = (max_superblock_size / used_bits_per_block) * used_bits_per_block;

        let num_indicator_blocks = len.div_ceil(used_bits_per_block) * alphabet_size;
        let num_superblock_offsets = len.div_ceil(superblock_size) * alphabet_size;

        let mut interleaved_blocks = vec![B::zeroes(); num_indicator_blocks];
        let mut interleaved_superblock_offsets = vec![I::zero(); num_superblock_offsets];

        let num_blocks_per_superblock = (superblock_size / used_bits_per_block) * alphabet_size;
        let blocks_per_superblock_iter =
            interleaved_blocks.par_chunks_mut(num_blocks_per_superblock);

        let superblock_offsets_iter = interleaved_superblock_offsets.par_chunks_mut(alphabet_size);

        let text_chunk_size = S::transform_chunk_size(superblock_size);

        let text_superblock_iter = text.par_chunks(text_chunk_size);

        let interleaved_superblock_iter = (
            text_superblock_iter,
            superblock_offsets_iter,
            blocks_per_superblock_iter,
        )
            .into_par_iter();

        interleaved_superblock_iter
            .for_each(|tup| fill_superblock::<I, B, S>(tup.0, tup.1, tup.2, alphabet_size));

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
            superblock_size,
            interleaved_blocks,
            interleaved_superblock_offsets,
        }
    }

    fn _alphabet_size(&self) -> usize {
        self.alphabet_size
    }

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

        // temporarily store block indices in the buffers
        for i in 0..num_remaining_unfinished_queries {
            block_offsets_starts[i] = self.block_idx(symbols[i], intervals[i].start);
            block_offsets_ends[i] = self.block_idx(symbols[i], intervals[i].end);
        }

        let mut blocks_starts: [B; N] = [B::zeroes(); N];
        let mut blocks_ends: [B; N] = [B::zeroes(); N];

        // hopefully, most of these memory loads happen in parallel on the hardware, because this is the most expensive part
        for i in 0..num_remaining_unfinished_queries {
            blocks_starts[i] = unsafe {
                *self
                    .interleaved_blocks
                    .get_unchecked(block_offsets_starts[i])
            };
            blocks_ends[i] =
                unsafe { *self.interleaved_blocks.get_unchecked(block_offsets_ends[i]) };
        }

        for i in 0..num_remaining_unfinished_queries {
            block_offsets_starts[i] = blocks_starts[i].extract_block_offset_and_then_zeroize_it();
            block_offsets_ends[i] = blocks_ends[i].extract_block_offset_and_then_zeroize_it();

            let idx_in_block_start = Self::idx_in_block(intervals[i].start);
            let idx_in_block_end = Self::idx_in_block(intervals[i].end);

            let block_count_start =
                blocks_starts[i].count_ones_before(idx_in_block_start + NUM_BLOCK_OFFSET_BITS);
            let block_count_end =
                blocks_ends[i].count_ones_before(idx_in_block_end + NUM_BLOCK_OFFSET_BITS);

            intervals[i].start =
                superblock_offsets_starts[i] + block_offsets_starts[i] + block_count_start;
            intervals[i].end = superblock_offsets_ends[i] + block_offsets_ends[i] + block_count_end;
        }
    }
}

impl<I: IndexStorage, B: Block> TextWithRankSupport<I> for FlatTextWithRankSupport<I, B> {
    unsafe fn rank_unchecked(&self, symbol: u8, idx: usize) -> usize {
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

        let block_idx = self.block_idx(symbol, idx);
        let mut block = unsafe { *self.interleaved_blocks.get_unchecked(block_idx) };

        let block_offset = block.extract_block_offset_and_then_zeroize_it();

        let idx_in_block = Self::idx_in_block(idx);
        let block_count = block.count_ones_before(idx_in_block + NUM_BLOCK_OFFSET_BITS);

        superblock_offset + block_offset + block_count
    }

    fn symbol_at(&self, idx: usize) -> u8 {
        assert!(idx < self.text_len);

        let used_bits_per_block = B::NUM_BITS - NUM_BLOCK_OFFSET_BITS;
        let blocks_start = (idx / used_bits_per_block) * self.alphabet_size;
        let blocks_end = blocks_start + self.alphabet_size;

        let blocks = &self.interleaved_blocks[blocks_start..blocks_end];

        let index_in_block = idx % used_bits_per_block + NUM_BLOCK_OFFSET_BITS;

        for (i, block) in blocks.iter().enumerate() {
            let block_bit = block.get_bit(index_in_block);
            if block_bit == 1 {
                return i as u8;
            }
        }

        unreachable!()
    }
}

fn fill_superblock<I: PrimInt, B: Block, S: SliceCompression>(
    text: &[u8],
    interleaved_superblock_offsets: &mut [I],
    interleaved_blocks: &mut [B],
    alphabet_size: usize,
) {
    let mut block_offsets_sum = vec![0u64; alphabet_size];
    let used_bits_per_block = B::NUM_BITS - NUM_BLOCK_OFFSET_BITS;

    let text_chunk_size = S::transform_chunk_size(used_bits_per_block);
    let text_block_iter = text.chunks(text_chunk_size);
    let blocks_iter = interleaved_blocks.chunks_mut(alphabet_size);

    let blocks_overshoot = text_block_iter.len() < blocks_iter.len();
    let block_package_iter = text_block_iter.zip(blocks_iter);

    for (text_block, blocks) in block_package_iter {
        for (block, &offset) in blocks.iter_mut().zip(&block_offsets_sum) {
            block.integrate_block_offset_assuming_zero(offset);
        }

        for (index_in_block, symbol) in S::iter(text_block).enumerate() {
            let symbol_usize = <usize as NumCast>::from(symbol).unwrap();

            let superblock_count = &mut interleaved_superblock_offsets[symbol_usize];
            *superblock_count = *superblock_count + I::one();

            // The wrapping add here is used for the same reason as in the condensed version at the same position,
            // even though technically, there is no need for it due to the usage of u64.
            block_offsets_sum[symbol_usize] = block_offsets_sum[symbol_usize].wrapping_add(1);
            blocks[symbol_usize].set_bit_assuming_zero(index_in_block + NUM_BLOCK_OFFSET_BITS, 1);
        }
    }

    // Annoying edge case, because the bit array we're storing is text.len() + 1 large.
    if blocks_overshoot {
        let last_blocks = interleaved_blocks
            .rchunks_mut(alphabet_size)
            .next()
            .unwrap();

        for (block, offset) in last_blocks.iter_mut().zip(block_offsets_sum) {
            block.integrate_block_offset_assuming_zero(offset);
        }
    }
}
