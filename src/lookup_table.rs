use bytemuck::Pod;
use num_traits::{NumCast, PrimInt};

use crate::{Alphabet, FmIndex, text_with_rank_support::Block};

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
#[derive(Debug)]
pub(crate) struct LookupTables<I: 'static> {
    num_symbols: usize,
    tables: Vec<LookupTable<I>>,
}

impl<I: PrimInt + Pod + 'static> LookupTables<I> {
    pub(crate) fn new_empty() -> Self {
        Self {
            num_symbols: 0,
            tables: Vec::new(),
        }
    }

    pub(crate) fn lookup(
        &self,
        query: impl IntoIterator<Item = u8>,
        depth: usize,
    ) -> (usize, usize) {
        self.tables[depth].lookup(query, self.num_symbols)
    }

    pub(crate) fn max_depth(&self) -> usize {
        self.tables.len() - 1
    }
}

pub(crate) fn fill_lookup_tables<A: Alphabet, I: PrimInt + Pod + 'static, B: Block>(
    index: &mut FmIndex<A, I, B>,
    max_depth: usize,
    num_symbols: usize,
) {
    index.lookup_tables.num_symbols = num_symbols;

    // iteratively fill lookup tables, to allow using the smaller tables in the search already for the larger tables
    for depth in 0..=max_depth {
        index
            .lookup_tables
            .tables
            .push(LookupTable::new(depth, num_symbols, &index));
    }
}

#[cfg_attr(feature = "savefile", derive(savefile_derive::Savefile))]
#[derive(Debug)]
struct LookupTable<I: 'static> {
    data: Vec<(I, I)>,
    depth: usize,
}

impl<I: PrimInt + Pod + 'static> LookupTable<I> {
    fn new<A: Alphabet, B: Block>(
        depth: usize,
        num_symbols: usize,
        index: &FmIndex<A, I, B>,
    ) -> Self {
        let num_values = num_symbols.pow(depth as u32);
        let mut data = vec![(I::zero(), I::zero()); num_values];

        let mut query = vec![0; depth];

        if depth > 0 {
            fill_table(1, depth, num_symbols, 0, &mut data, &mut query, index);
        } else {
            data[0] = (
                <I as NumCast>::from(0).unwrap(),
                <I as NumCast>::from(index.text_with_rank_support.text_len()).unwrap(),
            );
        }

        Self { data, depth }
    }

    // direction should already be resolved by the iterator
    fn lookup(&self, query: impl IntoIterator<Item = u8>, num_symbols: usize) -> (usize, usize) {
        let mut idx = 0;
        let mut exponent = self.depth.saturating_sub(1);

        for symbol in query.into_iter().take(self.depth) {
            // subtract one, because the sentinel is not stored in the table
            let symbol = symbol - 1;
            idx += symbol as usize * num_symbols.pow(exponent as u32);
            exponent = exponent.saturating_sub(1);
        }

        let (start, end) = self.data[idx];

        (
            <usize as NumCast>::from(start).unwrap(),
            <usize as NumCast>::from(end).unwrap(),
        )
    }
}

fn fill_table<A: Alphabet, I: PrimInt + Pod + 'static, B: Block>(
    curr_depth: usize,
    max_depth: usize,
    num_symbols: usize,
    curr_data_idx: usize,
    data: &mut [(I, I)],
    query: &mut [u8],
    index: &FmIndex<A, I, B>,
) {
    if curr_depth == max_depth {
        for symbol in 0..num_symbols {
            query[curr_depth - 1] = symbol as u8 + 1; // +1 to offset sentinel
            let (start, end) = index.search_in_order_dense_encoded(query.iter().copied());
            data[curr_data_idx + symbol] = (
                <I as NumCast>::from(start).unwrap(),
                <I as NumCast>::from(end).unwrap(),
            );
        }

        return;
    }

    for symbol in 0..num_symbols {
        let exponent = max_depth - curr_depth;
        let next_data_index = curr_data_idx + symbol * num_symbols.pow(exponent as u32);
        query[curr_depth - 1] = symbol as u8 + 1; // +1 to offset sentinel
        fill_table(
            curr_depth + 1,
            max_depth,
            num_symbols,
            next_data_index,
            data,
            query,
            index,
        );
    }
}
