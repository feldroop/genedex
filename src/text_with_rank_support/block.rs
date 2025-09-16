use std::slice;

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
    fn from_init_store(init_store: u64) -> Self;

    #[doc(hidden)]
    fn zeroes() -> Self {
        Self::from_init_store(0)
    }

    #[doc(hidden)]
    fn ones() -> Self {
        Self::from_init_store(u64::MAX)
    }

    #[doc(hidden)]
    fn as_raw_slice(&self) -> &[u64];
    #[doc(hidden)]
    fn as_raw_mut_slice(&mut self) -> &mut [u64];

    #[doc(hidden)]
    fn negate(&mut self) {
        for store in self.as_raw_mut_slice() {
            *store = !(*store);
        }
    }

    #[doc(hidden)]
    fn set_to_self_and(&mut self, other: Self) {
        for (store, other_store) in self.as_raw_mut_slice().iter_mut().zip(other.as_raw_slice()) {
            *store &= other_store;
        }
    }

    #[doc(hidden)]
    fn count_ones(&self) -> usize {
        self.as_raw_slice()
            .iter()
            .map(|s| s.count_ones() as usize)
            .sum()
    }

    #[doc(hidden)]
    fn get_bit(&self, index: usize) -> u8;

    #[doc(hidden)]
    fn set_bit_assuming_zero(&mut self, index: usize, bit: u8);

    #[doc(hidden)]
    fn zeroize_bits_starting_from(&mut self, idx: usize);
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

    fn from_init_store(init_store: u64) -> Self {
        Self {
            data: [init_store; 8],
        }
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

    fn set_bit_assuming_zero(&mut self, index: usize, bit: u8) {
        let store_index = index / 64;
        let index_in_store = index % 64;
        self.data[store_index] |= (bit as u64) << index_in_store;
    }

    fn zeroize_bits_starting_from(&mut self, idx: usize) {
        let mask = BLOCK512_MASKS[idx];
        for i in 0..8 {
            // SAFETY: the size of self.data and the mask is known to be 8 at compile time
            unsafe {
                *self.data.get_unchecked_mut(i) &= *mask.get_unchecked(i);
            }
        }
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

    fn from_init_store(init_store: u64) -> Self {
        Self { data: init_store }
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

    fn set_bit_assuming_zero(&mut self, index: usize, bit: u8) {
        self.data |= (bit as u64) << index;
    }

    fn zeroize_bits_starting_from(&mut self, idx: usize) {
        self.data &= BLOCK64_MASKS[idx];
    }
}

// generates 000...000, 000...001, 111...111
const BLOCK64_MASKS: [u64; 64] = const {
    let mut masks = [u64::MAX; 64];
    let mut bit_index = 0;

    while bit_index < 64 {
        masks[bit_index] <<= bit_index;
        masks[bit_index] = !masks[bit_index];
        bit_index += 1;
    }

    masks
};

// the same as BLOCK64_MASKS, but with 512 bits
const BLOCK512_MASKS: [[u64; 8]; 512] = const {
    let mut masks = [[0; 8]; 512];

    let mut block64_index = 0;

    while block64_index < 8 {
        let mut bit_index = 0;
        while bit_index < 64 {
            masks[block64_index * 64 + bit_index][block64_index] = BLOCK64_MASKS[bit_index];

            bit_index += 1;
        }

        block64_index += 1;
    }

    let mut mask_index = 0;

    while mask_index < 512 {
        let complete_64blocks_below = mask_index / 64;

        let mut block64_index = 0;
        while block64_index < complete_64blocks_below {
            masks[mask_index][block64_index] = u64::MAX;

            block64_index += 1;
        }

        mask_index += 1;
    }

    masks
};

mod sealed {
    pub trait Sealed {}
}

// #[test]
// fn feature() {
//     // for mask in BLOCK64_MASKS {
//     //     println!("{:064b}", mask);
//     // }

//     for mask in &BLOCK512_MASKS[350..] {
//         println!(
//             "{:064b} {:064b} {:064b} {:064b} {:064b} {:064b} {:064b} {:064b}\n",
//             mask[0], mask[1], mask[2], mask[3], mask[4], mask[5], mask[6], mask[7]
//         );
//     }
// }
