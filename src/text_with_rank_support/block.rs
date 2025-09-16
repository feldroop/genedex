use crate::maybe_savefile::MaybeSavefile;

// this distinction of block types only exists to be able to set repr(align(64)) for the 512 bit block

/// The block used internally by data structures of this library.
///
/// Currently, this can either be [`Block64`] or [`Block512`], with [`Block64`] being the default.
///
/// The larger blocks lead to slightly higher running times of operations, but consume less memory.
/// The difference in memory usage depends on the number of dense symbols of the alphabet used.
/// For small alphabets like DNA alphabets, the difference in memory usage is almost irrelevant, so
/// [`Block64`] is recommended.
pub trait Block: sealed::Sealed + Clone + Copy + Send + Sync + MaybeSavefile + 'static {
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
}

/// Larger blocks, recommended for alphabets with many dense symbols.
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
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
        let mut sum = 0;

        let mask = BLOCK512_MASKS[idx];

        for i in 0..8 {
            sum += (self.data[i] & mask[i]).count_ones();
        }

        sum as usize
    }
}

/// Smaller blocks, recommended for alphabets with fewer dense symbols, like DNA alphabets.
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
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
        self.data = self.data & other.data;
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
}

// the same as BLOCK64_MASKS, but with 512 bits
const BLOCK512_MASKS: [[u64; 8]; 512] = const {
    let mut masks = [[0; 8]; 512];

    let mut block64_idx = 0;

    while block64_idx < 8 {
        let mut bit_idx = 0;
        while bit_idx < 64 {
            masks[block64_idx * 64 + bit_idx][block64_idx] = !(u64::MAX << bit_idx);

            bit_idx += 1;
        }

        block64_idx += 1;
    }

    let mut mask_idx = 0;

    while mask_idx < 512 {
        let complete_64blocks_below = mask_idx / 64;

        let mut block64_idx = 0;
        while block64_idx < complete_64blocks_below {
            masks[mask_idx][block64_idx] = u64::MAX;

            block64_idx += 1;
        }

        mask_idx += 1;
    }

    masks
};

mod sealed {
    pub trait Sealed {}
}
