use num_traits::NumCast;

use crate::{
    Alphabet, FmIndex, HalfOpenInterval, IndexStorage, text_with_rank_support::TextWithRankSupport,
};

// the lookup table allows obtaining the interval for a query suffix directly, without LF-mappings
// it stores precomputed intervals for all possible suffixes up to a length of max depth

// in this implementation, the index also stores lookup tables for all values from 0 to max_depth,
// because the smaller tables require only a fraction of the memory of the larger tables,
// can speed up short queries and make the lookup table build process simple and efficient by iteratively
// constructing lookup tables for larger suffixes up to max depth

// using I as storage and not simply usize saves space if I is a 32 bit int (and usize is 64 bit)
#[cfg_attr(feature = "savefile", derive(savefile::savefile_derive::Savefile))]
#[derive(Debug, Clone)]
pub(crate) struct LookupTables<I> {
    num_symbols: usize,
    factors: Vec<usize>,
    tables: Vec<LookupTable<I>>,
}

// expands to a large match statement that performs the const currying technique.
// there is a crate for this, but it seems to be unmaintained and experimental
macro_rules! const_curry_match {
    ($n:ident, $query_suffix:ident, $self:ident, $alphabet:ident, $($i:literal),*) => {
        match $n {
            $(
                $i => compute_lookup_idx_static_len::<$i>(
                    $query_suffix.try_into().unwrap(),
                    $self.factors[..$n].try_into().unwrap(),
                    $alphabet,
                ),
            )*
            _ => $self.compute_lookup_idx_dynamic_len($query_suffix, $alphabet)
        }
    };
}

impl<I: IndexStorage> LookupTables<I> {
    pub(crate) fn new_empty() -> Self {
        Self {
            num_symbols: 0,
            factors: Vec::new(),
            tables: Vec::new(),
        }
    }

    pub(crate) fn lookup(&self, query_suffix: &[u8], alphabet: &Alphabet) -> HalfOpenInterval {
        let idx = self.compute_lookup_idx(query_suffix, alphabet);
        self.lookup_idx(query_suffix.len(), idx)
    }

    pub(crate) fn lookup_without_alphabet_translation(
        &self,
        query_suffix: &[u8],
    ) -> HalfOpenInterval {
        let idx = self.compute_lookup_idx_without_alphabet_transition(query_suffix);
        self.lookup_idx(query_suffix.len(), idx)
    }

    fn lookup_idx(&self, depth: usize, idx: usize) -> HalfOpenInterval {
        self.tables[depth].lookup(idx)
    }

    pub(crate) fn compute_lookup_idx(&self, query_suffix: &[u8], alphabet: &Alphabet) -> usize {
        let n = query_suffix.len();

        // a "const currying" optimization technique, because this function actually showed
        // up taking a significant amount of running time in the flamegraph.
        // using this actually lead to a small improvement
        const_curry_match!(
            n,
            query_suffix,
            self,
            alphabet,
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            9,
            10,
            11,
            12,
            13,
            14,
            15
        )
    }

    // fallback function for the const curried function compute_lookup_idx
    pub(crate) fn compute_lookup_idx_dynamic_len(
        &self,
        query_suffix: &[u8],
        alphabet: &Alphabet,
    ) -> usize {
        let mut idx = 0;

        for (&symbol, &factor) in query_suffix.iter().zip(&self.factors) {
            // subtract one, because the sentinel is not stored in the table
            let dense_symbol = alphabet.io_to_dense_representation(symbol) - 1;
            idx += dense_symbol as usize * factor;
        }

        idx
    }

    pub(crate) fn compute_lookup_idx_without_alphabet_transition(
        &self,
        query_suffix: &[u8],
    ) -> usize {
        let mut idx = 0;

        for (&symbol, &factor) in query_suffix.iter().zip(&self.factors) {
            // subtract one, because the sentinel is not stored in the table
            idx += (symbol as usize - 1) * factor;
        }

        idx
    }

    // rust analyzer gives false positive error for now
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

    pub(crate) fn max_depth(&self) -> usize {
        self.tables.len() - 1
    }
}

pub(crate) fn compute_lookup_idx_static_len<const N: usize>(
    query_suffix: &[u8; N],
    factors: &[usize; N],
    alphabet: &Alphabet,
) -> usize {
    let mut idx = 0;

    for (&symbol, &factor) in query_suffix.iter().zip(factors) {
        // subtract one, because the sentinel is not stored in the table
        let dense_symbol = alphabet.io_to_dense_representation(symbol) - 1;
        idx += dense_symbol as usize * factor;
    }

    idx
}

pub(crate) fn fill_lookup_tables<I: IndexStorage, R: TextWithRankSupport<I>>(
    index: &mut FmIndex<I, R>,
    max_depth: usize,
) {
    let num_symbols = index.alphabet.num_searchable_dense_symbols();
    index.lookup_tables.num_symbols = num_symbols;

    index.lookup_tables.factors = (0..=max_depth)
        .map(|exponent| num_symbols.pow(exponent as u32))
        .collect();

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
            fill_table(1, depth, num_symbols, &mut data, &mut query, index);
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
    data: &mut [(I, I)],
    query: &mut [u8],
    index: &FmIndex<I, R>,
) {
    if curr_depth == max_depth {
        for symbol in 0..num_symbols {
            query[curr_depth - 1] = symbol as u8 + 1; // +1 to offset sentinel

            let interval = index
                .cursor_for_query_without_alphabet_translation(query)
                .interval();

            let idx = index
                .lookup_tables
                .compute_lookup_idx_without_alphabet_transition(query);

            data[idx] = (
                <I as NumCast>::from(interval.start).unwrap(),
                <I as NumCast>::from(interval.end).unwrap(),
            );
        }

        return;
    }

    for symbol in 0..num_symbols {
        query[curr_depth - 1] = symbol as u8 + 1; // +1 to offset sentinel
        fill_table(curr_depth + 1, max_depth, num_symbols, data, query, index);
    }
}
