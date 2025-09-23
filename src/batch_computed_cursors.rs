use crate::{
    Cursor, FmIndex, HalfOpenInterval, IndexStorage, text_with_rank_support::TextWithRankSupport,
};

pub(crate) struct BatchComputedCursors<'a, I, R, Q, const B: usize> {
    index: &'a FmIndex<I, R>,
    next_idx_in_batch: usize,
    curr_batch_size: usize,
    buffers: Buffers<'a, B>,
    queries_iter: Q,
}

impl<'a, I, R, Q, const B: usize> BatchComputedCursors<'a, I, R, Q, B>
where
    I: IndexStorage,
    R: TextWithRankSupport<I>,
    Q: Iterator<Item = &'a [u8]>,
{
    pub(crate) fn new(index: &'a FmIndex<I, R>, queries_iter: Q) -> Self {
        Self {
            index,
            next_idx_in_batch: 0,
            curr_batch_size: 0,
            buffers: Buffers::new(),
            queries_iter,
        }
    }

    fn compute_next_batch(&mut self) {
        self.next_idx_in_batch = 0;
        self.curr_batch_size = 0;

        while self.curr_batch_size < B
            && let Some(query) = self.queries_iter.next()
        {
            self.buffers.queries[self.curr_batch_size] = Some(query);
            self.curr_batch_size += 1;
        }

        // buffer 1 contains depths and intervals are updated
        self.batched_lookup_jumps();

        loop {
            let next_idx_in_queries = &mut self.buffers.buffer1;

            let queries = &self.buffers.queries;
            let intervals = &mut self.buffers.intervals;

            let mut all_queries_done = true;
            for ((query, interval), next_idx) in
                queries.iter().zip(intervals).zip(next_idx_in_queries)
            {
                if query.is_none() {
                    continue;
                }

                let query = query.unwrap();
                if *next_idx < query.len() && interval.start != interval.end {
                    all_queries_done = false;

                    let rev_idx = query.len() - *next_idx - 1;
                    let symbol = self
                        .index
                        .alphabet
                        .io_to_dense_representation(query[rev_idx]);
                    interval.start = self.index.lf_mapping_step(symbol, interval.start);
                    interval.end = self.index.lf_mapping_step(symbol, interval.end);

                    *next_idx += 1;
                }
            }

            if all_queries_done {
                break;
            }
        }
    }

    fn batched_lookup_jumps(&mut self) {
        let depths = &mut self.buffers.buffer1[..self.curr_batch_size];
        let idxs = &mut self.buffers.buffer2[..self.curr_batch_size];

        for ((&query, depth), idx) in self
            .buffers
            .queries
            .iter()
            .take(self.curr_batch_size)
            .zip(depths)
            .zip(idxs)
        {
            let query = query.unwrap();
            *depth = std::cmp::min(query.len(), self.index.lookup_tables.max_depth());
            *idx = self
                .index
                .lookup_tables
                .compute_lookup_idx(&mut self.index.get_query_iter(query), *depth);
        }

        let depths = &mut self.buffers.buffer1[..self.curr_batch_size];
        let idxs = &mut self.buffers.buffer2[..self.curr_batch_size];

        self.index
            .lookup_tables
            .lookup_idx_many(depths, idxs, &mut self.buffers.intervals);
    }
}

impl<'a, I, R, Q, const B: usize> Iterator for BatchComputedCursors<'a, I, R, Q, B>
where
    I: IndexStorage,
    R: TextWithRankSupport<I>,
    Q: Iterator<Item = &'a [u8]>,
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

struct Buffers<'a, const B: usize> {
    intervals: [HalfOpenInterval; B],
    queries: [Option<&'a [u8]>; B],
    buffer1: [usize; B],
    buffer2: [usize; B],
}

impl<'a, const B: usize> Buffers<'a, B> {
    fn new() -> Self {
        let intervals = [HalfOpenInterval { start: 0, end: 0 }; B];
        let queries = [None; B];
        let buffer1 = [0; B];
        let buffer2 = [0; B];

        Self {
            intervals,
            queries,
            buffer1,
            buffer2,
        }
    }
}
