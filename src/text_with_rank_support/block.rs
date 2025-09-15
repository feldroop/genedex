use bitvec::prelude::*;

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
    fn as_bitslice(&self) -> &BitSlice<u64>;
    #[doc(hidden)]
    fn as_mut_bitslice(&mut self) -> &mut BitSlice<u64>;

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
            .map(|&s| s.count_ones() as usize)
            .sum()
    }

    // for some reason, a manuel implementation seemed to have a tiny benefit in benchmarks
    // might be a benchmarking error, but the flamgegraph showed genereated code with atomics,
    // that might be a reason.
    #[doc(hidden)]
    fn get_bit(&self, index: usize) -> u8;
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

mod sealed {
    pub trait Sealed {}
}
