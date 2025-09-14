use crate::{
    Alphabet, FmIndex, IndexStorage,
    text_with_rank_support::{Block, Block64},
};
use std::marker::PhantomData;

pub struct FmIndexConfig<I, B = Block64> {
    suffix_array_sampling_rate: usize,
    lookup_table_depth: usize,
    _index_storage_marker: PhantomData<I>,
    _block_marker: PhantomData<B>,
}

impl<I: IndexStorage, B: Block> FmIndexConfig<I, B> {
    /// number of threads for building is controlled by rayon
    pub fn new() -> Self {
        Self::default()
    }

    pub fn suffix_array_sampling_rate(&mut self, suffix_array_sampling_rate: usize) -> &mut Self {
        self.suffix_array_sampling_rate = suffix_array_sampling_rate;
        self
    }

    pub fn lookup_table_depth(&mut self, lookup_table_depth: usize) -> &mut Self {
        self.lookup_table_depth = lookup_table_depth;
        self
    }

    pub fn construct<T: AsRef<[u8]>>(
        &mut self,
        texts: impl IntoIterator<Item = T>,
        alphabet: Alphabet,
    ) -> FmIndex<I, B> {
        FmIndex::new(
            texts,
            alphabet,
            self.suffix_array_sampling_rate,
            self.lookup_table_depth,
        )
    }
}

impl<I: IndexStorage, B: Block> Default for FmIndexConfig<I, B> {
    fn default() -> Self {
        Self {
            suffix_array_sampling_rate: 4,
            lookup_table_depth: 13,
            _index_storage_marker: PhantomData,
            _block_marker: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_config() {
        let texts = [b"ACGT"];
        let alphabet = crate::alphabet::ascii_dna();

        let _index = FmIndexConfig::<i32>::new()
            .lookup_table_depth(5)
            .suffix_array_sampling_rate(8)
            .construct(texts, alphabet);
    }
}
