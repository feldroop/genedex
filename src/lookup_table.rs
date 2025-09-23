use num_traits::NumCast;

use crate::{FmIndex, HalfOpenInterval, IndexStorage, text_with_rank_support::TextWithRankSupport};

#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone)]
pub(crate) struct LookupTables<I> {
    num_symbols: usize,
    tables: Vec<LookupTable<I>>,
}

impl<I: IndexStorage> LookupTables<I> {
    pub(crate) fn new_empty() -> Self {
        Self {
            num_symbols: 0,
            tables: Vec::new(),
        }
    }

    pub(crate) fn lookup<Q>(&self, query_iter: &mut Q, depth: usize) -> HalfOpenInterval
    where
        Q: Iterator<Item = u8>,
    {
        let idx = self.compute_lookup_idx(query_iter, depth);
        self.lookup_idx(depth, idx)
    }

    pub(crate) fn lookup_idx(&self, depth: usize, idx: usize) -> HalfOpenInterval {
        self.tables[depth].lookup(idx)
    }

    // gives false positive error for now
    #[rust_analyzer::skip]
    pub(crate) fn lookup_idx_many(
        &self,
        depths: &[usize],
        idxs: &[usize],
        outs: &mut [HalfOpenInterval],
    ) {
        for ((&depth, &idx), out) in depths.iter().zip(idxs).zip(outs) {
            *out = self.lookup_idx(depth, idx);
        }
    }

    pub(crate) fn compute_lookup_idx<Q>(&self, query_iter: &mut Q, depth: usize) -> usize
    where
        Q: Iterator<Item = u8>,
    {
        let mut idx = 0;
        let mut exponent = depth.saturating_sub(1);

        for symbol in query_iter.take(depth) {
            // subtract one, because the sentinel is not stored in the table
            let symbol = symbol - 1;
            idx += symbol as usize * self.num_symbols.pow(exponent as u32);
            exponent = exponent.saturating_sub(1);
        }

        idx
    }

    pub(crate) fn max_depth(&self) -> usize {
        self.tables.len() - 1
    }
}

pub(crate) fn fill_lookup_tables<I: IndexStorage, R: TextWithRankSupport<I>>(
    index: &mut FmIndex<I, R>,
    max_depth: usize,
    num_symbols: usize,
) {
    index.lookup_tables.num_symbols = num_symbols;

    // iteratively fill lookup tables, to allow using the smaller tables in the search already for the larger tables
    for depth in 0..=max_depth {
        index
            .lookup_tables
            .tables
            .push(LookupTable::new(depth, num_symbols, index));
    }
}

#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone)]
struct LookupTable<I> {
    data: Vec<(I, I)>,
    depth: usize,
}

impl<I: IndexStorage> LookupTable<I> {
    fn new<R: TextWithRankSupport<I>>(
        depth: usize,
        num_symbols: usize,
        index: &FmIndex<I, R>,
    ) -> Self {
        let num_values = num_symbols.pow(depth as u32);
        let mut data = vec![(I::zero(), I::zero()); num_values];

        let mut query = vec![0; depth];

        if depth > 0 {
            fill_table(1, depth, num_symbols, 0, &mut data, &mut query, index);
        } else {
            data[0] = (
                <I as NumCast>::from(0).unwrap(),
                <I as NumCast>::from(index.total_text_len()).unwrap(),
            );
        }

        Self { data, depth }
    }

    // direction should already be resolved by the iterator
    fn lookup(&self, idx: usize) -> HalfOpenInterval {
        let (start, end) = self.data[idx];

        HalfOpenInterval {
            start: <usize as NumCast>::from(start).unwrap(),
            end: <usize as NumCast>::from(end).unwrap(),
        }
    }
}

fn fill_table<I: IndexStorage, R: TextWithRankSupport<I>>(
    curr_depth: usize,
    max_depth: usize,
    num_symbols: usize,
    curr_data_idx: usize,
    data: &mut [(I, I)],
    query: &mut [u8],
    index: &FmIndex<I, R>,
) {
    if curr_depth == max_depth {
        for symbol in 0..num_symbols {
            query[curr_depth - 1] = symbol as u8 + 1; // +1 to offset sentinel

            let interval = index
                .cursor_for_iter_without_alphabet_translation(query.iter().copied())
                .interval();

            data[curr_data_idx + symbol] = (
                <I as NumCast>::from(interval.start).unwrap(),
                <I as NumCast>::from(interval.end).unwrap(),
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
