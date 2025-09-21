use crate::IndexStorage;
use crate::construction::slice_compression::SliceCompression;
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
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[savefile_doc_hidden]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatTextWithRankSupport<I, B = Block64> {
    text_len: usize,
    alphabet_size: usize,
    superblock_size: usize,
    interleaved_blocks: Vec<B>,
    interleaved_superblock_offsets: Vec<I>,
}

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
}

impl<I: IndexStorage, B: Block> TextWithRankSupport<I> for FlatTextWithRankSupport<I, B> {
    fn rank(&self, symbol: u8, idx: usize) -> usize {
        assert!((symbol as usize) < self.alphabet_size && idx <= self.text_len);
        unsafe { self.rank_unchecked(symbol, idx) }
    }

    unsafe fn rank_unchecked(&self, symbol: u8, idx: usize) -> usize {
        // SAFETY: all of the index accesses are in the valid range if idx is at most text.len()
        // and since the alphabet has a size of at least 2

        let symbol_usize = symbol as usize;

        let superblock_offset_index =
            (idx / self.superblock_size) * self.alphabet_size + symbol_usize;

        let superblock_offset = unsafe {
            *self
                .interleaved_superblock_offsets
                .get_unchecked(superblock_offset_index)
        };

        // SAFETY: must succeed, otherwise the construction function would have crashed
        let superblock_offset =
            unsafe { <usize as NumCast>::from(superblock_offset).unwrap_unchecked() };

        let used_bits_per_block = B::NUM_BITS - NUM_BLOCK_OFFSET_BITS;
        let block_idx = (idx / used_bits_per_block) * self.alphabet_size + symbol_usize;
        let mut block = unsafe { *self.interleaved_blocks.get_unchecked(block_idx) };

        let block_offset = block.extract_block_offset_and_then_zeroize_it();

        let index_in_block = idx % used_bits_per_block;
        let block_count = block.count_ones_before(index_in_block + NUM_BLOCK_OFFSET_BITS);

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

    fn alphabet_size(&self) -> usize {
        self.alphabet_size
    }

    fn text_len(&self) -> usize {
        self.text_len
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

            block_offsets_sum[symbol_usize] += 1;
            blocks[symbol_usize].set_bit_assuming_zero(index_in_block + NUM_BLOCK_OFFSET_BITS, 1);
        }
    }

    // annoying edge case, because the bit array we're storing is text.len() + 1 large
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
