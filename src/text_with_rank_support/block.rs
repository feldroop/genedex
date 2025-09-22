use crate::{maybe_savefile::MaybeSavefile, sealed};

pub(crate) const NUM_BLOCK_OFFSET_BITS: usize = 16;

// this distinction of block types only exists to be able to set repr(align(64)) for the 512 bit block

/// The block configurations used internally by data structures of this library.
///
/// Currently, this can either be [`Block64`] or [`Block512`], with [`Block64`] being the default.
///
/// The larger blocks lead to slightly higher running times of operations, but consume less memory.
/// The difference in memory usage depends on the number of dense symbols of the alphabet used.
/// For small alphabets like DNA alphabets, the difference in memory usage is almost irrelevant, so
/// [`Block64`] is recommended.
pub trait Block:
    sealed::Sealed + std::fmt::Debug + Clone + Copy + Send + Sync + MaybeSavefile + 'static
{
    #[doc(hidden)]
    const NUM_BITS: usize;
    #[doc(hidden)]
    const NUM_BYTES: usize = Self::NUM_BITS / 8;
    #[doc(hidden)]
    const NUM_U64: usize = Self::NUM_BITS / 64;

    #[doc(hidden)]
    fn zeroes() -> Self;

    #[doc(hidden)]
    fn negate(&mut self);

    #[doc(hidden)]
    fn set_to_self_and(&mut self, other: Self);

    #[doc(hidden)]
    fn count_ones_before(&self, idx: usize) -> usize;

    #[doc(hidden)]
    fn get_bit(&self, idx: usize) -> u8;

    #[doc(hidden)]
    fn set_bit_assuming_zero(&mut self, idx: usize, bit: u8);

    #[doc(hidden)]
    fn integrate_block_offset_assuming_zero(&mut self, block_offset: u64);

    #[doc(hidden)]
    fn extract_block_offset_and_then_zeroize_it(&mut self) -> usize;
}

/// Larger blocks, recommended for alphabets with many dense symbols.
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[savefile_doc_hidden]
#[derive(Debug, Clone, Copy)]
#[repr(align(64))]
pub struct Block512 {
    data: [u64; 8],
}

impl sealed::Sealed for Block512 {}

impl MaybeSavefile for Block512 {}

impl Block for Block512 {
    const NUM_BITS: usize = 512;

    fn zeroes() -> Self {
        Self { data: [0; 8] }
    }

    #[doc(hidden)]
    fn negate(&mut self) {
        for i in 0..8 {
            self.data[i] = !self.data[i];
        }
    }

    #[doc(hidden)]
    fn set_to_self_and(&mut self, other: Self) {
        for i in 0..8 {
            self.data[i] &= other.data[i];
        }
    }

    fn get_bit(&self, idx: usize) -> u8 {
        let store_idx = idx / 64;
        let idx_in_store = idx % 64;
        ((self.data[store_idx] >> idx_in_store) & 1) as u8
    }

    fn set_bit_assuming_zero(&mut self, idx: usize, bit: u8) {
        let store_idx = idx / 64;
        let idx_in_store = idx % 64;
        self.data[store_idx] |= (bit as u64) << idx_in_store;
    }

    fn count_ones_before(&self, idx: usize) -> usize {
        let store_idx = idx / 64;
        let idx_in_store = idx % 64;

        let mut mask = [0; 8];
        for mask_part in &mut mask[..store_idx] {
            *mask_part = u64::MAX;
        }
        mask[store_idx] = !(u64::MAX << idx_in_store);

        let mut sum = 0;

        for (data_part, mask_part) in self.data.iter().zip(mask) {
            sum += (data_part & mask_part).count_ones();
        }

        sum as usize
    }

    fn integrate_block_offset_assuming_zero(&mut self, block_offset: u64) {
        self.data[0] = block_offset;
    }

    fn extract_block_offset_and_then_zeroize_it(&mut self) -> usize {
        let mask = !(u64::MAX << NUM_BLOCK_OFFSET_BITS);
        let block_offset = self.data[0] & mask;

        self.data[0] &= !mask;

        block_offset as usize
    }
}

/// Smaller blocks, recommended for alphabets with fewer dense symbols, like DNA alphabets.
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[savefile_doc_hidden]
#[derive(Debug, Clone, Copy)]
pub struct Block64 {
    data: u64,
}

impl sealed::Sealed for Block64 {}

impl MaybeSavefile for Block64 {}

impl Block for Block64 {
    const NUM_BITS: usize = 64;

    fn zeroes() -> Self {
        Self { data: 0 }
    }

    fn negate(&mut self) {
        self.data = !self.data;
    }

    fn set_to_self_and(&mut self, other: Self) {
        self.data &= other.data;
    }

    fn get_bit(&self, idx: usize) -> u8 {
        ((self.data >> idx) & 1) as u8
    }

    fn set_bit_assuming_zero(&mut self, idx: usize, bit: u8) {
        self.data |= (bit as u64) << idx;
    }

    fn count_ones_before(&self, idx: usize) -> usize {
        let masked_data = self.data & !(u64::MAX << idx);
        masked_data.count_ones() as usize
    }

    fn integrate_block_offset_assuming_zero(&mut self, block_offset: u64) {
        self.data = block_offset;
    }

    fn extract_block_offset_and_then_zeroize_it(&mut self) -> usize {
        let mask = !(u64::MAX << NUM_BLOCK_OFFSET_BITS);
        let block_offset = self.data & mask;

        self.data &= !mask;

        block_offset as usize
    }
}
