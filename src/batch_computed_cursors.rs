use crate::{
    Cursor, FmIndex, HalfOpenInterval, IndexStorage, text_with_rank_support::TextWithRankSupport,
};

// the idea of this is to perform multiple LF-Mapping steps simultaneously for improved performance.
// since the normal implementation is likely memory-bound, this way of batching could make use of
// parallel fetching of memory addresses by modern CPUs. It lead to an improvement, but not as much as hoped.
// On my laptop, it was about 2x, on the linux server it was even less effective. This likely happened
// because the performance profile of the program completely changed and now suddenly there actually exist
// CPU-bound bottlenecks that could be improved in the future (see roadmap).
pub(crate) struct BatchComputedCursors<'a, I, R, Q, QS, const N: usize> {
    index: &'a FmIndex<I, R>,
    next_idx_in_batch: usize,
    curr_batch_size: usize,
    queries_iter: QS,
    buffers: Buffers<Q, N>,
}

impl<'a, I, R, Q, QS, const N: usize> BatchComputedCursors<'a, I, R, Q, QS, N>
where
    I: IndexStorage,
    R: TextWithRankSupport<I>,
    QS: Iterator<Item = Q>,
    Q: AsRef<[u8]>,
{
    pub(crate) fn new(index: &'a FmIndex<I, R>, queries_iter: QS) -> Self {
        Self {
            index,
            next_idx_in_batch: 0,
            curr_batch_size: 0,
            queries_iter,
            buffers: Buffers::new(),
        }
    }

    fn compute_next_batch(&mut self) {
        self.next_idx_in_batch = 0;
        self.curr_batch_size = 0;

        // pull queries from iterator
        while self.curr_batch_size < N
            && let Some(query) = self.queries_iter.next()
        {
            self.buffers.queries[self.curr_batch_size] = Some(query);
            self.buffers.query_at_idx[self.curr_batch_size] = self.curr_batch_size;
            self.curr_batch_size += 1;
        }

        self.batched_lookup_jumps();

        // this idx is counting from the front and has to be reversed for the actual backwards seach
        let mut next_idx_in_queries = self.index.lookup_tables.max_depth();

        let mut num_remaining_unfinished_queries = self.curr_batch_size;

        self.move_finished_queries_to_end(
            next_idx_in_queries,
            &mut num_remaining_unfinished_queries,
        );

        // one loop iteration does does up to N LF-mappings in a batch
        while num_remaining_unfinished_queries > 0 {
            self.batched_lf_mappings(next_idx_in_queries, num_remaining_unfinished_queries);

            next_idx_in_queries += 1;
            self.move_finished_queries_to_end(
                next_idx_in_queries,
                &mut num_remaining_unfinished_queries,
            );
        }

        self.move_queries_back_to_initial_order();
    }

    fn batched_lookup_jumps(&mut self) {
        let depths = &mut self.buffers.buffer1[..self.curr_batch_size];
        let idxs = &mut self.buffers.buffer2[..self.curr_batch_size];

        for ((query, depth), idx) in self.buffers.queries.iter().zip(depths).zip(idxs) {
            let query = query.as_ref().unwrap().as_ref();
            *depth = std::cmp::min(query.len(), self.index.lookup_tables.max_depth());
            let suffix_idx = query.len() - *depth;

            *idx = self
                .index
                .lookup_tables
                .compute_lookup_idx(&query[suffix_idx..], &self.index.alphabet);
        }

        let depths = &mut self.buffers.buffer1[..self.curr_batch_size];
        let idxs = &mut self.buffers.buffer2[..self.curr_batch_size];

        self.index
            .lookup_tables
            .lookup_idx_many(depths, idxs, &mut self.buffers.intervals);
    }

    fn batched_lf_mappings(
        &mut self,
        next_idx_in_queries: usize,
        num_remaining_unfinished_queries: usize,
    ) {
        let queries = &self.buffers.queries[..num_remaining_unfinished_queries];
        let symbols = &mut self.buffers.symbols[..num_remaining_unfinished_queries];

        for (query, symbol) in queries.iter().zip(symbols) {
            let query = query.as_ref().unwrap().as_ref();
            let rev_idx = query.len() - next_idx_in_queries - 1;
            *symbol = self
                .index
                .alphabet
                .io_to_dense_representation(query[rev_idx]);
        }

        self.index
            .text_with_rank_support
            .replace_many_interval_borders_with_ranks(
                &mut self.buffers,
                num_remaining_unfinished_queries,
            );

        // add counts to finalize lf mapping formula
        let symbols = &self.buffers.symbols[..num_remaining_unfinished_queries];
        let intervals = &mut self.buffers.intervals[..num_remaining_unfinished_queries];
        for (interval, &symbol) in intervals.iter_mut().zip(symbols) {
            interval.start += self.index.count[symbol as usize];
            interval.end += self.index.count[symbol as usize];
        }
    }

    fn move_finished_queries_to_end(
        &mut self,
        next_idx_in_queries: usize,
        num_remaining_unfinished_queries: &mut usize,
    ) {
        let mut i = 0;

        while i < *num_remaining_unfinished_queries {
            let interval = self.buffers.intervals[i];

            if let Some(query) = self.buffers.queries[i].as_ref()
                && query.as_ref().len() > next_idx_in_queries
                && interval.start != interval.end
            {
                // query is unfinished
                i += 1;
                continue;
            }

            // swap finished query to end
            let j = *num_remaining_unfinished_queries - 1;
            self.buffers.queries.swap(i, j);
            self.buffers.intervals.swap(i, j);
            self.buffers.query_at_idx.swap(i, j);

            *num_remaining_unfinished_queries -= 1;
        }
    }

    fn move_queries_back_to_initial_order(&mut self) {
        let mut i = 0;
        while i < self.curr_batch_size {
            // this means query j is at idx i
            let j = self.buffers.query_at_idx[i];
            if i == j {
                i += 1;
                continue;
            }
            self.buffers.intervals.swap(i, j);
            self.buffers.query_at_idx.swap(i, j);
        }
    }
}

impl<'a, I, R, Q, QS, const N: usize> Iterator for BatchComputedCursors<'a, I, R, Q, QS, N>
where
    I: IndexStorage,
    R: TextWithRankSupport<I>,
    QS: Iterator<Item = Q>,
    Q: AsRef<[u8]>,
{
    type Item = Cursor<'a, I, R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_idx_in_batch == self.curr_batch_size {
            self.compute_next_batch();
        }

        if self.curr_batch_size == 0 {
            None
        } else {
            self.next_idx_in_batch += 1;
            Some(Cursor {
                index: self.index,
                interval: self.buffers.intervals[self.next_idx_in_batch - 1],
            })
        }
    }
}

// the 4 buffers are used to store different values throughout the batched search
pub(crate) struct Buffers<Q, const N: usize> {
    pub(crate) intervals: [HalfOpenInterval; N],
    queries: [Option<Q>; N],
    query_at_idx: [usize; N],
    pub(crate) symbols: [u8; N],
    pub(crate) buffer1: [usize; N],
    pub(crate) buffer2: [usize; N],
    pub(crate) buffer3: [usize; N],
    pub(crate) buffer4: [usize; N],
}

impl<Q, const N: usize> Buffers<Q, N> {
    pub(crate) fn new() -> Self {
        let intervals = [HalfOpenInterval { start: 0, end: 0 }; N];
        let queries = std::array::from_fn(|_| None);
        let query_at_idx = [0; N];
        let symbols = [0; N];
        let buffer1 = [0; N];
        let buffer2 = [0; N];
        let buffer3 = [0; N];
        let buffer4 = [0; N];

        Self {
            intervals,
            queries,
            query_at_idx,
            symbols,
            buffer1,
            buffer2,
            buffer3,
            buffer4,
        }
    }
}
