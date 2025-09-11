pub mod alphabet;
pub mod text_with_rank_support;

mod construction;
mod sampled_suffix_array;
mod text_id_search_tree;

use std::marker::PhantomData;

use bytemuck::Pod;
use libsais::OutputElement;
use num_traits::{NumCast, PrimInt};

#[doc(inline)]
pub use alphabet::Alphabet;
#[doc(inline)]
pub use text_with_rank_support::TextWithRankSupport;

use construction::DataStructures;
use sampled_suffix_array::SampledSuffixArray;
use text_id_search_tree::TexdIdSearchTree;
use text_with_rank_support::{Block, Block512};

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
pub struct FmIndex<A, I: 'static, B = Block512>
where
    B: 'static,
{
    count: Vec<usize>,
    text_with_rank_support: TextWithRankSupport<I, B>,
    suffix_array: SampledSuffixArray<I>,
    text_ids: TexdIdSearchTree,
    _alphabet_marker: PhantomData<A>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hit {
    pub text_id: usize,
    pub position: usize,
}

pub type FmIndexI32<A, B = Block512> = FmIndex<A, i32, B>;
pub type FmIndexU32<A, B = Block512> = FmIndex<A, u32, B>;
pub type FmIndexI64<A, B = Block512> = FmIndex<A, i64, B>;

impl<A: Alphabet, I: OutputElement, B: Block> FmIndex<A, I, B> {
    // text chars must be smaller than alphabet size and greater than 0
    // other operations use rayons configured number of threads
    pub fn new<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let DataStructures {
            count,
            suffix_array_bytes,
            bwt,
            text_ids,
            text_border_lookup,
        } = construction::create_data_structures::<A, I, T>(
            texts,
            suffix_array_construction_thread_count,
        );

        let sampled_suffix_array = SampledSuffixArray::new_uncompressed(
            suffix_array_bytes,
            suffix_array_sampling_rate,
            text_border_lookup,
        );

        let text_with_rank_support = TextWithRankSupport::construct(&bwt, A::SIZE);

        FmIndex {
            count,
            text_with_rank_support,
            suffix_array: sampled_suffix_array,
            text_ids,
            _alphabet_marker: PhantomData,
        }
    }
}

impl<A: Alphabet, B: Block> FmIndex<A, u32, B> {
    // text chars must be smaller than alphabet size + 1 and greater than 0
    // other operations use rayons configured number of threads
    pub fn new_u32_compressed<T: AsRef<[u8]>>(
        texts: impl IntoIterator<Item = T>,
        suffix_array_construction_thread_count: u16,
        suffix_array_sampling_rate: usize,
    ) -> Self {
        let DataStructures {
            count,
            suffix_array_bytes,
            bwt,
            text_ids,
            text_border_lookup,
        } = construction::create_data_structures::<A, i64, T>(
            texts,
            suffix_array_construction_thread_count,
        );

        assert!(bwt.len() <= u32::MAX as usize);

        let text_border_lookup = text_border_lookup
            .into_iter()
            .map(|(k, v)| (k, v as u32))
            .collect();

        let sampled_suffix_array = SampledSuffixArray::new_u32_compressed(
            suffix_array_bytes,
            suffix_array_sampling_rate,
            text_border_lookup,
        );

        let text_with_rank_support = TextWithRankSupport::construct(&bwt, A::SIZE);

        FmIndex {
            count,
            text_with_rank_support,
            suffix_array: sampled_suffix_array,
            text_ids,
            _alphabet_marker: PhantomData,
        }
    }
}

impl<A: Alphabet, I: PrimInt + Pod + 'static, B: Block> FmIndex<A, I, B> {
    pub fn count(&self, query: &[u8]) -> usize {
        let (start, end) = self.search_suffix_array_interval(query);
        end - start
    }

    pub fn locate(&self, query: &[u8]) -> impl Iterator<Item = Hit> {
        let (start, end) = self.search_suffix_array_interval(query);

        self.suffix_array
            .recover_range(start..end, self)
            .map(|idx| {
                // println!("concat text index: {idx}");
                let (text_id, position) = self
                    .text_ids
                    .backtransfrom_concatenated_text_index(<usize as NumCast>::from(idx).unwrap());

                Hit { text_id, position }
            })
    }

    // returns half open interval [start, end)
    fn search_suffix_array_interval(&self, query: &[u8]) -> (usize, usize) {
        assert!(!query.is_empty());

        let (mut start, mut end) = (0, self.text_with_rank_support.text_len());

        for &character in query.iter().rev() {
            let symbol = A::DENSE_ENCODING_TRANSLATION_TABLE[character as usize];
            assert!(symbol != 255 && symbol != 0);

            // it is assumed that the query doesn't contain the sentinel
            start = self.lf_mapping_step(symbol, start);
            end = self.lf_mapping_step(symbol, end);

            if start == end {
                break;
            }
        }

        (start, end)
    }

    fn lf_mapping_step(&self, symbol: u8, idx: usize) -> usize {
        self.count[symbol as usize] + self.text_with_rank_support.rank(symbol, idx)
    }
}
