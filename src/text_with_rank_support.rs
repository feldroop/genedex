use std::slice;

use bitvec::prelude::*;
use num_traits::{NumCast, PrimInt};
use rayon::prelude::*;

// Interleaved means that the respective values for different symbols of the alphabet
// for the same text position are next to each other.
// Blocks must be interleaved for efficient queries.
// (Super)block offsets are only interleaved for faster (parallel) construction.
#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
#[derive(Debug)]
pub struct TextWithRankSupport<I: 'static, B = Block512>
where
    B: 'static,
{
    text_len: usize,
    alphabet_size: usize,
    interleaved_blocks: Vec<B>,
    interleaved_block_offsets: Vec<u16>,
    interleaved_superblock_offsets: Vec<I>,
}

impl<I: PrimInt + Send + Sync, B: Block> TextWithRankSupport<I, B> {
    pub fn construct(text: &[u8], alphabet_size: usize) -> Self {
        assert!(alphabet_size >= 2);

        let alphabet_num_bits = ilog2_ceil(alphabet_size);
        let len: usize = text.len() + 1;
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

        let text_superblock_iter = text.par_chunks(superblock_size);

        let interleaved_superblock_iter = (
            text_superblock_iter,
            superblock_offsets_iter,
            block_offsets_per_superblock_iter,
            blocks_per_superblock_iter,
        )
            .into_par_iter();

        interleaved_superblock_iter
            .for_each(|tup| fill_superblock::<I, B>(tup.0, tup.1, tup.2, tup.3, alphabet_size));

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
            text_len: text.len(),
            alphabet_size,
            interleaved_blocks,
            interleaved_block_offsets,
            interleaved_superblock_offsets,
        }
    }
}

impl<I: PrimInt, B: Block> TextWithRankSupport<I, B> {
    // number of occurrences of the symbol in text[0..idx]
    pub fn rank(&self, symbol: u8, idx: usize) -> usize {
        let symbol_usize = symbol as usize;
        let alphabet_num_bits = ilog2_ceil(self.alphabet_size);

        let superblock_size = u16::MAX as usize + 1;
        let superblock_offset_index = (idx / superblock_size) * self.alphabet_size + symbol_usize;
        let superblock_offset = self.interleaved_superblock_offsets[superblock_offset_index];
        let superblock_offset = <usize as NumCast>::from(superblock_offset).unwrap();

        let block_offset_index = (idx / B::NUM_BITS) * self.alphabet_size + symbol_usize;
        let block_offset = self.interleaved_block_offsets[block_offset_index] as usize;

        let interleaved_blocks_start = (idx / B::NUM_BITS) * alphabet_num_bits;
        let interleaved_blocks_end = interleaved_blocks_start + alphabet_num_bits;

        let interleaved_blocks =
            &self.interleaved_blocks[interleaved_blocks_start..interleaved_blocks_end];

        let mut accumulator_block = B::ones();
        let symbol_bits = symbol.view_bits::<Lsb0>();

        for (mut block, symbol_bit) in interleaved_blocks.iter().copied().zip(symbol_bits) {
            if !symbol_bit {
                block.negate();
            }

            accumulator_block.set_to_self_and(block);
        }

        let index_in_block = idx % B::NUM_BITS;
        accumulator_block.as_mut_bitslice()[index_in_block..].fill(false);
        let block_count = accumulator_block.count_ones();

        superblock_offset + block_offset + block_count
    }

    pub fn symbol_at(&self, idx: usize) -> u8 {
        let alphabet_num_bits = ilog2_ceil(self.alphabet_size);
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

    pub fn text_len(&self) -> usize {
        self.text_len
    }
}

fn fill_superblock<I: PrimInt, B: Block>(
    text: &[u8],
    interleaved_superblock_offsets: &mut [I],
    interleaved_block_offsets: &mut [u16],
    interleaved_blocks: &mut [B],
    alphabet_size: usize,
) {
    let alphabet_num_bits = ilog2_ceil(alphabet_size);
    let mut block_offsets_sum = vec![0; alphabet_size];

    let text_block_iter = text.chunks(B::NUM_BITS);
    let block_offsets_iter = interleaved_block_offsets.chunks_mut(alphabet_size);
    let blocks_iter = interleaved_blocks.chunks_mut(alphabet_num_bits);

    let blocks_overshoot = text_block_iter.len() < blocks_iter.len();

    let block_package_iter = text_block_iter.zip(block_offsets_iter).zip(blocks_iter);

    for ((text_block, block_offsets), blocks) in block_package_iter {
        block_offsets.copy_from_slice(&block_offsets_sum);

        for (index_in_block, symbol) in text_block.iter().copied().enumerate() {
            let symbol_usize = <usize as NumCast>::from(symbol).unwrap();

            let superblock_count = &mut interleaved_superblock_offsets[symbol_usize];
            *superblock_count = *superblock_count + I::one();

            block_offsets_sum[symbol_usize] += 1;

            let symbol_bits = symbol.view_bits::<Lsb0>();

            for (block, bit) in blocks.iter_mut().zip(symbol_bits) {
                block.as_mut_bitslice().set(index_in_block, *bit);
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

// this distinction of block types only exists to be able to set repr(align(64)) for the 512 bit block
pub trait Block: sealed::Sealed + Clone + Copy + Send + Sync {
    const NUM_BITS: usize;
    const NUM_BYTES: usize = Self::NUM_BITS / 8;
    const NUM_U64: usize = Self::NUM_BITS / 64;

    fn from_init_store(init_store: u64) -> Self;

    fn zeroes() -> Self {
        Self::from_init_store(0)
    }

    fn ones() -> Self {
        Self::from_init_store(u64::MAX)
    }

    fn as_bitslice(&self) -> &BitSlice<u64>;
    fn as_mut_bitslice(&mut self) -> &mut BitSlice<u64>;

    fn as_raw_slice(&self) -> &[u64];
    fn as_raw_mut_slice(&mut self) -> &mut [u64];

    fn negate(&mut self) {
        for store in self.as_raw_mut_slice() {
            *store = !(*store);
        }
    }

    fn set_to_self_and(&mut self, other: Self) {
        for (store, other_store) in self.as_raw_mut_slice().iter_mut().zip(other.as_raw_slice()) {
            *store &= other_store;
        }
    }

    fn count_ones(&self) -> usize {
        self.as_raw_slice()
            .iter()
            .map(|&s| s.count_ones() as usize)
            .sum()
    }

    fn get_bit(&self, index: usize) -> u8;
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
#[derive(Debug, Clone, Copy)]
#[repr(align(64))]
pub struct Block512 {
    data: [u64; 8],
}

impl sealed::Sealed for Block512 {}

impl Block for Block512 {
    const NUM_BITS: usize = 512;

    fn from_init_store(init_store: u64) -> Self {
        Self {
            data: [init_store; 8],
        }
    }

    fn as_bitslice(&self) -> &BitSlice<u64> {
        self.data.view_bits()
    }

    fn as_mut_bitslice(&mut self) -> &mut BitSlice<u64> {
        self.data.view_bits_mut()
    }

    fn as_raw_slice(&self) -> &[u64] {
        &self.data
    }

    fn as_raw_mut_slice(&mut self) -> &mut [u64] {
        &mut self.data
    }

    fn get_bit(&self, index: usize) -> u8 {
        let store_index = index / 64;
        let index_in_store = index % 64;
        ((self.data[store_index] >> index_in_store) & 1) as u8
    }
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
#[derive(Debug, Clone, Copy)]
pub struct Block64 {
    data: u64,
}

impl sealed::Sealed for Block64 {}

impl Block for Block64 {
    const NUM_BITS: usize = 64;

    fn from_init_store(init_store: u64) -> Self {
        Self { data: init_store }
    }

    fn as_bitslice(&self) -> &BitSlice<u64> {
        self.data.view_bits()
    }

    fn as_mut_bitslice(&mut self) -> &mut BitSlice<u64> {
        self.data.view_bits_mut()
    }

    fn as_raw_slice(&self) -> &[u64] {
        slice::from_ref(&self.data)
    }

    fn as_raw_mut_slice(&mut self) -> &mut [u64] {
        slice::from_mut(&mut self.data)
    }

    fn get_bit(&self, index: usize) -> u8 {
        ((self.data >> index) & 1) as u8
    }
}

fn ilog2_ceil(value: usize) -> usize {
    if value.is_power_of_two() {
        value.ilog2() as usize
    } else {
        (value.ilog2() + 1) as usize
    }
}

mod sealed {
    pub trait Sealed {}
}
